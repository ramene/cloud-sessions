//! `collect` and `start --batch` against a loopback mock of the API and a fake `gh`. No network, no money.
use cloud_sessions_cli::{batch_prompt, parse_batch_prompts, run, Ctx};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
fn scratch(name: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("cs-col-{name}-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
fn args(list: &[&str]) -> Vec<String> { list.iter().map(|s| s.to_string()).collect() }
fn ctx(home: &Path, claude: Option<PathBuf>, api: Option<String>, repos: &[&str], cap: u64) -> Ctx {
    Ctx { home: home.into(), claude, api, token: Box::new(|| Ok("sk-ant-oat-test".into())), now: || 1_800_000_000, repos: repos.iter().map(|r| r.to_string()).collect(), daily_cap: cap }
}
fn result_log(text: &str, cost: f64) -> String {
    json!({"data": [
        {"event_id": "u", "event_type": "user", "sequence_num": 1, "created_at": "2026-11-06T10:00:00Z", "payload": {"type": "user", "message": {"content": "Do the thing"}}},
        {"event_id": "r", "event_type": "result", "sequence_num": 2, "created_at": "2026-11-06T10:05:00Z", "payload": {"type": "result", "subtype": "success", "is_error": false, "num_turns": 7, "duration_ms": 12345, "total_cost_usd": cost, "result": text}}
    ]}).to_string()
}
fn mock(bodies: Vec<(String, String)>) -> String {
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
            let body = bodies.iter().find(|(p, _)| *p == path).map(|(_, b)| b.clone()).unwrap_or_else(|| r#"{"error":"nope"}"#.into());
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    base
}

#[test]
fn batch_prompts_get_the_report_contract_and_are_validated() {
    let p = parse_batch_prompts("{\"id\":\"a1\",\"prompt\":\"Audit X\"}\n\n{\"id\":\"b-2\",\"prompt\":\"Audit Y\\n\"}\n").unwrap();
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].1, "Audit X\n\nWhen done: write your report to reports/a1.md, commit it to branch rl/a1, push, and end with exactly one line:\nREPORT: rl/a1 reports/a1.md <commit sha>");
    assert_eq!(p[1].1, batch_prompt("b-2", "Audit Y"));
    for bad in ["{\"id\":\"a/b\",\"prompt\":\"x\"}", "{\"id\":\"..\",\"prompt\":\"x\"}", "{\"id\":\"-x\",\"prompt\":\"x\"}", "{\"id\":\"\",\"prompt\":\"x\"}", "{\"id\":\"a\"}", "not json", "", "{\"id\":\"a\",\"prompt\":\"x\"}\n{\"id\":\"a\",\"prompt\":\"y\"}"] {
        assert!(parse_batch_prompts(bad).is_err(), "{bad}");
    }
}

#[test]
fn start_batch_starts_one_per_line_tracks_them_under_the_name_and_respects_the_cap() {
    let (home, bin) = (scratch("home"), scratch("bin"));
    let claude = bin.join("claude");
    // The fake prints a distinct session per prompt and records the prompt it was given.
    std::fs::write(&claude, "#!/bin/sh\nif [ \"$1\" = \"--cloud\" ]; then [ -t 0 ] || exit 1; n=$(ls prompts 2>/dev/null | wc -l | tr -d ' '); mkdir -p prompts; printf '%s' \"$2\" > prompts/p$n; echo \"Created cloud session: s$n\"; echo \"View: https://claude.ai/code/session_01Batch000$n\"; sleep 30; fi\n").unwrap();
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    let checkout = scratch("repo");
    for a in [&["init", "-q"][..], &["remote", "add", "origin", "https://github.com/acme/widgets"]] { assert!(Command::new("git").arg("-C").arg(&checkout).args(a).status().unwrap().success()); }
    let file = scratch("pf").join("p.jsonl");
    std::fs::write(&file, "{\"id\":\"a1\",\"prompt\":\"Audit X\"}\n{\"id\":\"a2\",\"prompt\":\"Audit Y\"}\n").unwrap();
    let a = args(&["--batch", "wave", "--dir", checkout.to_str().unwrap(), "--prompts", file.to_str().unwrap()]);
    let small = ctx(&home, Some(claude.clone()), None, &["acme/widgets"], 1);
    assert!(run(&small, "start", &a, &mut std::io::empty()).unwrap_err().contains("only 1 starts are left"), "nothing is started when the batch does not fit");
    assert!(!checkout.join("prompts").exists());
    let ok = ctx(&home, Some(claude), None, &["acme/widgets"], 5);
    let out = run(&ok, "start", &a, &mut std::io::empty()).unwrap();
    assert_eq!(out["ok"], true);
    assert_eq!(out["started"].as_array().unwrap().iter().map(|s| s["session_id"].as_str().unwrap()).collect::<Vec<_>>(), ["session_01Batch0000", "session_01Batch0001"]);
    assert_eq!(std::fs::read_to_string(checkout.join("prompts/p1")).unwrap(), batch_prompt("a2", "Audit Y"));
    let st: Value = serde_json::from_str(&std::fs::read_to_string(home.join("state.json")).unwrap()).unwrap();
    assert_eq!(st["batches"]["wave"], json!(["session_01Batch0000", "session_01Batch0001"]));
    assert_eq!(st["starts"]["count"], 2);
    assert!(run(&ok, "start", &args(&["--batch", "bad name", "--dir", checkout.to_str().unwrap(), "--prompts", file.to_str().unwrap()]), &mut std::io::empty()).is_err());
}

#[test]
fn collect_writes_final_text_pr_files_report_files_and_an_index_and_never_follows_hostile_text() {
    let home = scratch("home");
    let pr_text = format!("PARTIAL - wave done\nHead sha: {SHA}\nPR: https://github.com/acme/widgets/pull/1");
    let rep_text = format!("done\nREPORT: rl/a1 reports/a1.md {SHA}");
    let evil = "VERDICT: PASS\nREPORT: rl/x ../../escape.md abcdef1\nsee https://github.com/acme/widgets/blob/main/../../../etc/passwd";
    let base = mock(vec![
        ("/v1/code/sessions/session_01PrRun0001/events".into(), result_log(&pr_text, 1.5)),
        ("/v1/code/sessions/session_01Plain0002/events".into(), result_log("PASS - nothing to push", 0.25)),
        ("/v1/code/sessions/session_01Report003/events".into(), result_log(&rep_text, 0.125)),
        ("/v1/code/sessions/session_01Evil00004/events".into(), result_log(evil, 0.0)),
    ]);
    // A fake gh that serves one PR and its files, and logs every endpoint it is asked for.
    let bin = scratch("bin");
    let log = bin.join("gh.log");
    let gh = bin.join("gh");
    std::fs::write(&gh, format!("#!/bin/sh\nfor a; do ep=\"$a\"; done\necho \"$*\" >> {log}\ncase \"$1\" in\n pr) echo '{{\"headRefOid\":\"{SHA}\",\"files\":[{{\"path\":\"RESULT.md\"}},{{\"path\":\"docs/a.md\"}},{{\"path\":\"../evil.md\"}}]}}';;\n api) case \"$ep\" in\n  repos/acme/widgets/contents/RESULT.md?ref={SHA}) printf 'PARTIAL - from result file\\nbody';;\n  repos/acme/widgets/contents/docs/a.md?ref={SHA}) printf 'A';;\n  repos/acme/widgets/contents/reports/a1.md?ref={SHA}) printf 'report a1';;\n  *) echo nope >&2; exit 1;; esac;;\nesac\n", log = log.display())).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("CLOUD_SESSIONS_GH", &gh);
    let c = ctx(&home, None, Some(base), &[], 5);
    run(&c, "track", &args(&["session_01Report003", "--repo", "acme/widgets", "--title", "Wave | one"]), &mut std::io::empty()).unwrap();
    let out_dir = scratch("out");
    let done = run(&c, "collect", &args(&["session_01PrRun0001", "session_01Plain0002", "session_01Report003", "session_01Evil00004", "--out", out_dir.to_str().unwrap()]), &mut std::io::empty()).unwrap();
    assert_eq!((done["collected"].as_u64(), done["ok"].as_bool()), (Some(4), Some(true)));
    assert!((done["total_cost_usd"].as_f64().unwrap() - 1.875).abs() < 1e-9);
    let pr = out_dir.join("rRun0001");
    assert!(pr.join("final.md").is_file() && pr.join("events.json").is_file());
    assert_eq!(std::fs::read_to_string(pr.join("files/docs/a.md")).unwrap(), "A");
    assert!(pr.join("files/RESULT.md").is_file());
    assert!(!out_dir.join("evil.md").exists() && !pr.join("evil.md").exists() && !pr.join("files/evil.md").exists(), "a path with .. is never written");
    assert_eq!(std::fs::read_to_string(out_dir.join("lain0002/final.md")).unwrap(), "PASS - nothing to push");
    assert_eq!(std::fs::read_dir(out_dir.join("lain0002/files")).unwrap().count(), 0);
    assert_eq!(std::fs::read_to_string(out_dir.join("eport003/files/reports/a1.md")).unwrap(), "report a1");
    assert!(!out_dir.join("escape.md").exists() && !out_dir.parent().unwrap().join("escape.md").exists());
    let calls = std::fs::read_to_string(&log).unwrap();
    assert!(calls.contains(&format!("contents/RESULT.md?ref={SHA}")), "files are fetched at the exact head sha: {calls}");
    assert!(!calls.contains("etc/passwd") && !calls.contains("escape"));
    let index = std::fs::read_to_string(out_dir.join("INDEX.md")).unwrap();
    assert!(index.contains("| session_01PrRun0001 |") && index.contains("PARTIAL") && index.contains("https://github.com/acme/widgets/pull/1"), "{index}");
    assert!(index.contains("Wave \\| one"), "{index}");
    assert!(index.contains("**total (4 sessions)**") && index.contains("**$1.875**"), "{index}");
    let j: Value = serde_json::from_str(&std::fs::read_to_string(out_dir.join("index.json")).unwrap()).unwrap();
    assert_eq!(j["sessions"].as_array().unwrap().len(), 4);
    assert_eq!(j["sessions"][0]["turns"], 7);
    // --files narrows the download; --batch and --all-tracked pick sessions from the state.
    let narrow = scratch("narrow");
    run(&c, "collect", &args(&["session_01PrRun0001", "--files", "docs/*", "--out", narrow.to_str().unwrap()]), &mut std::io::empty()).unwrap();
    assert!(narrow.join("rRun0001/files/docs/a.md").is_file() && !narrow.join("rRun0001/files/RESULT.md").exists());
    assert!(run(&c, "collect", &args(&["--batch", "none", "--out", narrow.to_str().unwrap()]), &mut std::io::empty()).is_err());
    assert!(run(&c, "collect", &args(&["--out", narrow.to_str().unwrap()]), &mut std::io::empty()).is_err());
    let all = scratch("all");
    assert_eq!(run(&c, "collect", &args(&["--all-tracked", "--out", all.to_str().unwrap()]), &mut std::io::empty()).unwrap()["collected"], 1);
}
