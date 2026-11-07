//! s299: `routines` against a loopback mock of the API. No network, no money, no token.
use cloud_sessions_cli::{routine_json, run, run_url, schedule_of, Ctx};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

fn ctx(api: String) -> Ctx {
    Ctx { home: std::env::temp_dir().join(format!("cs-rt-{}", std::process::id())), claude: None, api: Some(api), token: Box::new(|| Ok("sk-ant-oat-test".into())), now: || 1_800_000_000, repos: vec![], daily_cap: 5 }
}
fn mock(route: impl Fn(&str) -> (u16, String) + Send + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            loop { let mut h = String::new(); reader.read_line(&mut h).unwrap(); if h.trim().is_empty() { break; } }
            let (status, body) = route(&path);
            let _ = write!(stream, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    base
}
fn triggers() -> Value {
    json!({"data": [
        {"id": "trig_cron", "name": "Nightly audit", "enabled": true, "cron_expression": "0 3 * * *", "run_once_at": null, "next_run_at": "2026-11-08T03:00:00Z", "last_fired_at": "2026-11-07T03:00:00Z"},
        {"id": "trig_once", "name": "One shot", "enabled": false, "cron_expression": null, "run_once_at": "2026-11-06T12:00:00Z", "next_run_at": null, "last_fired_at": "2026-11-06T12:00:00Z"},
        {"id": "trig_down", "name": "Broken", "enabled": false, "cron_expression": null, "run_once_at": null, "next_run_at": null, "last_fired_at": null}
    ]})
}
fn runs(n: usize) -> Value {
    json!({"data": (0..n).map(|i| json!({"id": format!("cse_01Run{i}"), "title": format!("run {i}"), "status": "completed", "worker_status": "idle", "created_at": format!("2026-11-0{}T10:00:00Z", 1 + i), "last_event_at": "2026-11-07T10:05:00Z"})).collect::<Vec<_>>(), "next_cursor": null})
}
fn server() -> String {
    mock(|path| match path {
        "/v1/code/triggers" => (200, triggers().to_string()),
        "/v1/code/sessions?trigger_id=trig_cron" => (200, runs(7).to_string()),
        "/v1/code/sessions?trigger_id=trig_once" => (200, runs(1).to_string()),
        "/v1/code/sessions?trigger_id=trig_down" => (503, r#"{"error":{"message":"runs down"}}"#.into()),
        _ => (404, r#"{"error":"nope"}"#.into()),
    })
}

#[test]
fn routines_contract_shape_newest_n_runs_and_per_routine_runs_error() {
    let ctx = ctx(server());
    let out = run(&ctx, "routines", &[], &mut std::io::empty()).unwrap();
    assert_eq!(out["ok"], true);
    assert!(out["unofficial"].as_str().unwrap().contains("/v1/code/triggers"));
    assert_eq!(out.as_object().unwrap().keys().collect::<Vec<_>>(), ["ok", "routines", "unofficial"], "exact top-level keys");
    let routines = out["routines"].as_array().unwrap();
    assert_eq!(routines.len(), 3);
    let cron = &routines[0];
    assert_eq!(cron.as_object().unwrap().keys().collect::<Vec<_>>(), ["enabled", "id", "last_fired_at", "name", "next_run_at", "runs", "schedule", "url"]);
    assert_eq!(cron["schedule"], json!({"kind": "cron", "cron": "0 3 * * *", "run_once_at": null}));
    assert_eq!(cron["url"], "https://claude.ai/code/routines/trig_cron");
    assert_eq!(cron["runs"].as_array().unwrap().len(), 5, "default --runs 5 of 7");
    assert_eq!(cron["runs"][0]["created_at"], "2026-11-07T10:00:00Z", "newest first");
    assert_eq!(cron["runs"][0].as_object().unwrap().keys().collect::<Vec<_>>(), ["created_at", "id", "last_event_at", "status", "title", "url", "worker_status"]);
    assert_eq!(cron["runs"][0]["url"], "https://claude.ai/code/session_01Run6");
    assert_eq!(routines[1]["schedule"], json!({"kind": "once", "cron": null, "run_once_at": "2026-11-06T12:00:00Z"}));
    assert_eq!(routines[1]["enabled"], false);
    assert_eq!(routines[2]["runs"], json!([]));
    assert!(routines[2]["runs_error"].as_str().unwrap().contains("runs down"), "one routine's failure never fails the call");
    let two = run(&ctx, "routines", &["--runs".into(), "2".into()], &mut std::io::empty()).unwrap();
    assert_eq!(two["routines"][0]["runs"].as_array().unwrap().len(), 2);
    assert!(run(&ctx, "routines", &["--runs".into(), "x".into()], &mut std::io::empty()).is_err());
}

#[test]
fn triggers_failure_fails_the_call_and_list_notes_hidden_runs() {
    let down = ctx(mock(|_| (503, r#"{"error":{"message":"triggers down"}}"#.into())));
    assert!(run(&down, "routines", &[], &mut std::io::empty()).unwrap_err().contains("triggers down"));
    let ctx = ctx(mock(|path| if path == "/v1/code/sessions" { (200, r#"{"data":[]}"#.into()) } else { (404, "{}".into()) }));
    let list = run(&ctx, "list", &["--unofficial".into()], &mut std::io::empty()).unwrap();
    assert_eq!(list["routine_runs_hidden"], true);
    assert!(list["routine_runs_note"].as_str().unwrap().contains("cloud-sessions routines"));
    let plain = run(&ctx, "list", &[], &mut std::io::empty()).unwrap();
    assert!(plain.get("routine_runs_hidden").is_none(), "the note rides with the unofficial list only");
}

#[test]
fn schedule_kind_and_url_forms() {
    assert_eq!(schedule_of(&json!({"cron_expression": "*/5 * * * *"}))["kind"], "cron");
    assert_eq!(schedule_of(&json!({"cron_expression": "", "run_once_at": "t"}))["kind"], "once");
    assert_eq!(schedule_of(&json!({}))["kind"], "once");
    assert_eq!(run_url("cse_01ABC"), "https://claude.ai/code/session_01ABC");
    assert_eq!(run_url("session_01ABC"), "https://claude.ai/code/session_01ABC");
    assert_eq!(run_url("01ABC"), "https://claude.ai/code/session_01ABC");
    let r = routine_json(&json!({"id": "t", "name": "n"}), Err("boom".into()), 5);
    assert_eq!((r["enabled"].as_bool(), r["runs_error"].as_str(), r["runs"].as_array().map(Vec::len)), (Some(false), Some("boom"), Some(0)));
}
