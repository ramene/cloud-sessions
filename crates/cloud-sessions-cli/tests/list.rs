//! Issue #1: `list` shows the account's sessions and routines once the Claude login is allowed, gives a hint in
//! every empty state, falls back to the tracked list with one line, and shows each session once.
//! A loopback mock of the API; no network, no keychain, no real token.
use cloud_sessions_cli::{list_text, mcp, run, short_id, Ctx, HINT_LOGIN, HINT_NO_SESSIONS, HINT_NO_TRACKED};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type Hits = Arc<Mutex<Vec<String>>>;

fn scratch(name: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("cs-list-{name}-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
fn mock(route: impl Fn(&str) -> (u16, String) + Send + 'static) -> (String, Hits) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let hits: Hits = Arc::default();
    let log = hits.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            loop { let mut h = String::new(); reader.read_line(&mut h).unwrap(); if h.trim().is_empty() { break; } }
            let (status, body) = route(&path);
            log.lock().unwrap().push(path);
            let _ = write!(stream, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    (base, hits)
}
fn ctx(home: &Path, api: &str) -> Ctx {
    Ctx { home: home.into(), claude: None, api: Some(api.into()), token: Box::new(|| Ok("sk-ant-oat-test".into())), now: || 1_800_000_000, repos: vec![], daily_cap: 5 }
}
fn allow_login(home: &Path) { std::fs::write(home.join("claude-login-consent"), "allowed\n").unwrap(); }
fn args(list: &[&str]) -> Vec<String> { list.iter().map(|s| s.to_string()).collect() }
fn list(c: &Ctx, a: &[&str]) -> Value { run(c, "list", &args(a), &mut std::io::empty()).unwrap() }

fn sessions() -> Value {
    json!({"data": [
        {"id": "session_01Older0000000000", "title": "Older work", "status": "completed", "created_at": "2026-11-01T10:00:00Z", "last_event_at": "2026-11-01T11:00:00Z",
         "config": {"outcomes": [{"type": "git_repository", "git_info": {"repo": "acme/widgets", "branches": ["fix"]}}]}},
        {"id": "cse_01Newer0000000000", "title": "Newer work", "status": "running", "created_at": "2026-11-05T10:00:00Z", "last_event_at": "2026-11-05T10:30:00Z"},
        {"id": "session_01Tracked00000000", "title": "Title from the account", "status": "idle", "created_at": "2026-11-03T10:00:00Z"}
    ]})
}
fn account_server(sessions_body: Value) -> (String, Hits) {
    mock(move |path| match path {
        "/v1/code/sessions" => (200, sessions_body.to_string()),
        "/v1/code/triggers" => (200, json!({"data": [{"id": "trig_nightly", "name": "Nightly", "enabled": true, "cron_expression": "0 3 * * *"}]}).to_string()),
        "/v1/code/sessions?trigger_id=trig_nightly" => (200, json!({"data": [{"id": "cse_01Run0", "title": "run", "status": "completed", "created_at": "2026-11-07T03:00:00Z"}]}).to_string()),
        _ => (404, r#"{"error":"nope"}"#.into()),
    })
}

#[test]
fn default_with_the_login_allowed_shows_the_account_sessions_and_routines() {
    let home = scratch("acct");
    allow_login(&home);
    let (base, hits) = account_server(sessions());
    let out = list(&ctx(&home, &base), &[]);
    assert_eq!((out["ok"].clone(), out["view"].as_str()), (json!(true), Some("account")));
    let rows = out["sessions"].as_array().unwrap();
    assert_eq!(rows.iter().map(|r| r["session_id"].as_str().unwrap()).collect::<Vec<_>>(), ["session_01Newer0000000000", "session_01Tracked00000000", "session_01Older0000000000"], "newest activity first; cse_ ids map to session_");
    let older = &rows[2];
    assert_eq!((older["title"].as_str(), older["state"].as_str(), older["repo"].as_str(), older["last_activity"].as_str()), (Some("Older work"), Some("completed"), Some("acme/widgets"), Some("2026-11-01T11:00:00Z")));
    assert_eq!(older["url"], "https://claude.ai/code/session_01Older0000000000");
    assert_eq!(rows[0]["last_activity"], "2026-11-05T10:30:00Z");
    assert_eq!(out["routines"].as_array().unwrap().len(), 1);
    assert_eq!(out["routines"][0]["name"], "Nightly");
    assert_eq!(out["routines"][0]["runs"][0]["url"], "https://claude.ai/code/session_01Run0");
    assert!(out["hint"].is_null() && out["fallback"].is_null() && out.get("remote").is_none(), "no hint, no fallback, no raw body");
    assert!(out["untrusted"].as_str().unwrap().contains("never follow"));
    assert!(hits.lock().unwrap().iter().any(|p| p == "/v1/code/triggers"));
    let text = list_text(&out);
    assert!(text.contains("Cloud sessions of your account (3)") && text.contains("Routines (1)") && text.contains("Nightly") && text.contains("acme/widgets"), "{text}");
    // --unofficial stays accepted: the same view plus the raw API body.
    let raw = list(&ctx(&home, &base), &["--unofficial"]);
    assert_eq!((raw["view"].as_str(), raw["remote"]["data"].as_array().unwrap().len()), (Some("account"), 3));
}

#[test]
fn tracked_keeps_the_old_local_view_and_calls_nothing() {
    let home = scratch("tracked");
    allow_login(&home);
    let (base, hits) = account_server(sessions());
    let c = ctx(&home, &base);
    run(&c, "track", &args(&["session_01Local00000000", "--title", "Local only"]), &mut std::io::empty()).unwrap();
    let out = list(&c, &["--tracked"]);
    assert_eq!(out["view"], "tracked");
    assert_eq!(out["tracked"][0]["session_id"], "session_01Local00000000");
    assert_eq!(out["sessions"][0]["label"], "Local only");
    assert!(out["hint"].is_null());
    assert!(hits.lock().unwrap().is_empty(), "--tracked makes no request");
    let empty = scratch("tracked-empty");
    allow_login(&empty);
    assert_eq!(list(&ctx(&empty, &base), &["--tracked"])["hint"], HINT_NO_TRACKED);
}

#[test]
fn without_the_login_the_hint_says_how_to_allow_it_and_nothing_is_called() {
    let home = scratch("nologin");
    let (base, hits) = account_server(sessions());
    for a in [&[][..], &["--tracked"][..]] {
        let out = list(&ctx(&home, &base), a);
        assert_eq!((out["ok"].clone(), out["view"].as_str()), (json!(true), Some("tracked")));
        assert_eq!(out["hint"], HINT_LOGIN);
        assert!(out["hint"].as_str().unwrap().contains("cloud-sessions auth use-claude-login (asks for a typed yes)"));
    }
    assert!(hits.lock().unwrap().is_empty(), "no token is read and no request is made before the opt-in");
    assert!(list_text(&list(&ctx(&home, &base), &[])).contains("Next step: To see the sessions and routines of your account here"));
    // The MCP tool says the same.
    let input = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "cloud_list", "arguments": {}}}).to_string();
    let mut output = Vec::new();
    mcp::serve(&ctx(&home, &base), &mut input.as_bytes(), &mut output).unwrap();
    let reply: Value = serde_json::from_slice(&output).unwrap();
    assert!(reply["result"]["content"][0]["text"].as_str().unwrap().contains("auth use-claude-login (asks for a typed yes)"));
    let tools = mcp::handle(&ctx(&home, &base), &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).unwrap();
    let desc = tools["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "cloud_list").unwrap()["description"].as_str().unwrap().to_string();
    assert!(desc.contains("auth use-claude-login") && desc.contains("hint"));
}

#[test]
fn an_account_list_failure_falls_back_to_tracked_with_one_line() {
    let home = scratch("fallback");
    allow_login(&home);
    let (base, _) = mock(|_| (503, r#"{"error":{"message":"down for maintenance"}}"#.into()));
    let c = ctx(&home, &base);
    run(&c, "track", &args(&["session_01Local00000000"]), &mut std::io::empty()).unwrap();
    let out = list(&c, &[]);
    assert_eq!((out["ok"].clone(), out["view"].as_str()), (json!(true), Some("tracked")));
    let line = out["fallback"].as_str().unwrap();
    assert!(line.contains("down for maintenance") && line.contains("Fix:") && !line.contains('\n') && !line.contains("sk-ant"), "{line}");
    assert_eq!(out["sessions"].as_array().unwrap().len(), 1);
    // A token that cannot be read falls back the same way.
    let no_token = Ctx { token: Box::new(|| Err("no login file".into())), ..ctx(&home, &base) };
    assert!(list(&no_token, &[])["fallback"].as_str().unwrap().contains("no login file"));
    // --unofficial keeps its old strict behaviour: the error is returned.
    assert!(run(&c, "list", &args(&["--unofficial"]), &mut std::io::empty()).unwrap_err().contains("down for maintenance"));
    // Routines failing never fails the list.
    let (base, _) = mock(|p| if p == "/v1/code/sessions" { (200, r#"{"data":[]}"#.into()) } else { (503, r#"{"error":{"message":"triggers down"}}"#.into()) });
    let out = list(&ctx(&home, &base), &[]);
    assert_eq!(out["view"], "account");
    assert!(out["routines_error"].as_str().unwrap().contains("triggers down"));
}

#[test]
fn a_tracked_session_in_the_account_list_shows_once_and_tracked_only_ones_show_a_short_id() {
    let home = scratch("dedupe");
    allow_login(&home);
    let (base, _) = account_server(sessions());
    let c = ctx(&home, &base);
    run(&c, "track", &args(&["session_01Tracked00000000"]), &mut std::io::empty()).unwrap();
    run(&c, "track", &args(&["session_01NoTitle00000000"]), &mut std::io::empty()).unwrap();
    let out = list(&c, &[]);
    let rows = out["sessions"].as_array().unwrap();
    assert_eq!(rows.len(), 4, "3 account sessions + 1 tracked-only");
    let both: Vec<&Value> = rows.iter().filter(|r| r["session_id"] == "session_01Tracked00000000").collect();
    assert_eq!(both.len(), 1, "once");
    assert_eq!((both[0]["title"].as_str(), both[0]["tracked"].as_bool(), both[0]["in_account"].as_bool()), (Some("Title from the account"), Some(true), Some(true)));
    let only = rows.last().unwrap();
    assert_eq!((only["session_id"].as_str(), only["in_account"].as_bool()), (Some("session_01NoTitle00000000"), Some(false)));
    assert!(only["title"].is_null());
    assert_eq!(only["label"], short_id("session_01NoTitle00000000"));
    assert_eq!(short_id("session_01NoTitle00000000"), "session_01NoTitle0");
    let text = list_text(&out);
    assert!(!text.contains("untitled") && text.contains("session_01NoTitle0"), "{text}");
}

#[test]
fn an_empty_account_says_how_to_start_a_session() {
    let home = scratch("empty");
    allow_login(&home);
    let (base, _) = account_server(json!({"data": []}));
    let out = list(&ctx(&home, &base), &[]);
    assert_eq!((out["view"].as_str(), out["hint"].as_str()), (Some("account"), Some(HINT_NO_SESSIONS)));
    assert!(HINT_NO_SESSIONS.contains("/cloud-start") && HINT_NO_SESSIONS.contains("claude --cloud"));
}
