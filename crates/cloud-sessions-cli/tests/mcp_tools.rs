//! Every MCP tool, by name: `tools/list` gives it a title and all four annotation hints as booleans
//! (MCP spec 2025-06-18, ToolAnnotations), and `tools/call` checks its arguments before it does anything.
//! No network: the context has no API base, no token and no claude binary.
use cloud_sessions_cli::{mcp, Ctx};
use serde_json::{json, Value};

const TOOLS: [&str; 7] = ["cloud_start", "cloud_send", "cloud_events", "cloud_track", "cloud_untrack", "cloud_list", "cloud_routines"];

fn ctx() -> Ctx {
    let home = std::env::temp_dir().join(format!("cs-mcp-tools-{}", std::process::id()));
    Ctx { home, claude: None, api: None, token: Box::new(|| Err("no token in tests".into())), now: || 1_800_000_000, repos: vec![], daily_cap: 5 }
}

fn listed() -> Vec<Value> {
    let reply = mcp::handle(&ctx(), &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
    reply["result"]["tools"].as_array().unwrap().clone()
}

/// The tool's (readOnly, destructive, idempotent, openWorld) hints; each must be a boolean, and the title must be set twice the same.
fn hints(name: &str) -> (bool, bool, bool, bool) {
    let tool = listed().into_iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("{name} is not listed"));
    let a = &tool["annotations"];
    let title = tool["title"].as_str().unwrap_or_else(|| panic!("{name} has no title"));
    assert!(!title.is_empty());
    assert_eq!(a["title"], title, "{name}: annotations.title must equal title");
    let hint = |k: &str| a[k].as_bool().unwrap_or_else(|| panic!("{name}: {k} must be a boolean"));
    (hint("readOnlyHint"), hint("destructiveHint"), hint("idempotentHint"), hint("openWorldHint"))
}

/// Call a tool; returns (isError, text).
fn call(name: &str, arguments: Value) -> (bool, String) {
    let reply = mcp::handle(&ctx(), &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": name, "arguments": arguments}})).unwrap();
    (reply["result"]["isError"].as_bool().unwrap(), reply["result"]["content"][0]["text"].as_str().unwrap().to_string())
}

fn refuses(name: &str, arguments: Value, needle: &str) {
    let (is_error, text) = call(name, arguments);
    assert!(is_error, "{name} must refuse: {text}");
    assert!(text.contains(needle), "{name}: expected {needle:?} in {text}");
}

#[test]
fn lists_exactly_the_seven_tools() {
    let names: Vec<String> = listed().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, TOOLS);
}

#[test]
fn cloud_start() {
    // Starts paid cloud work; each call starts a new session.
    assert_eq!(hints("cloud_start"), (false, false, false, true));
    refuses("cloud_start", json!({"prompt": "hi"}), "dir is required");
}

#[test]
fn cloud_send() {
    // Adds one message (and more cloud work) per call; removes nothing.
    assert_eq!(hints("cloud_send"), (false, false, false, true));
    refuses("cloud_send", json!({"session": "session_01AbCdEfGh"}), "message is required");
}

#[test]
fn cloud_events() {
    assert_eq!(hints("cloud_events"), (true, false, true, true));
    refuses("cloud_events", json!({}), "session is required");
}

#[test]
fn cloud_track() {
    // Adds to the local tracked list only; tracking the same session again changes nothing more.
    assert_eq!(hints("cloud_track"), (false, false, true, false));
    refuses("cloud_track", json!({}), "session is required");
}

#[test]
fn cloud_untrack() {
    // Removes the entry from the local tracked list; the session itself is untouched.
    assert_eq!(hints("cloud_untrack"), (false, true, true, false));
    refuses("cloud_untrack", json!({}), "session is required");
}

#[test]
fn cloud_list() {
    assert_eq!(hints("cloud_list"), (true, false, true, true));
}

#[test]
fn cloud_routines() {
    assert_eq!(hints("cloud_routines"), (true, false, true, true));
}
