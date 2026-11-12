//! `cloud-sessions start|send|events|watch|list|routines|auth|mcp`. JSON on stdout; failures `{"ok":false,"error":…}` with exit 1.
use cloud_sessions_cli::{auth::auth, ctx_from_env, list_text, mcp, run, watch};
use serde_json::{json, Value};
use std::io::{stdin, stdout, IsTerminal, Write};

const USAGE: &str = "usage: cloud-sessions <command>
  start  --dir <repo checkout> --prompt <text|->        start a cloud session (repo must be allowlisted)
  start  --batch <name> --dir <checkout> --prompts <file.jsonl>   one session per line (id, prompt); each ends with a REPORT: line
  send   <session> --message <text|->                   send a follow-up into a session
  events <session> [--after <event id>]                 read events after a cursor
  watch  <session> [--after id] [--interval s] [--max-seconds s] [--stream]   print new events as JSON lines
  cast   <session> [--out f] [--idle-max s] [--mp4 f]   the events as an asciicast v2 recording
  collect <session…>|--all-tracked|--batch <name> [--out dir] [--files glob]   final result + PR/branch files + INDEX.md (needs gh)
  allow  --dir <repo checkout>                          add that repository to the allowlist (you run this)
  track  <session link|id> [--title t] [--repo owner/repo] | --from-file <path>   track sessions started elsewhere (links in a file too)
  stop   <session>                                     interrupt a Remote Control session running on this machine
  mark   <session> --seen <utc>                         remember the newest event a journal already wrote
  untrack <session>                                     stop tracking one
  list   [--tracked] [--json]                           the account's sessions and routines (after auth use-claude-login),
                                                        else the sessions tracked here, with a hint for the next step;
                                                        --tracked: only the tracked ones; text at a terminal, --json for JSON
  routines [--runs N] [--unofficial]                    the account's routines with their newest N runs each (unofficial)
  auth   set [--file]|clear|status|use-claude-login|revoke-claude-login   manage how events are read (set reads stdin)
  mcp                                                   run as an MCP server over stdio";

fn print(value: &Value) {
    println!("{value}");
}
fn fail(message: &str) -> ! {
    print(&json!({"ok": false, "error": message}));
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else { eprintln!("{USAGE}"); std::process::exit(2) };
    let rest = &args[1..];
    let ctx = ctx_from_env();
    let outcome = match command {
        "-h" | "--help" | "help" => { println!("{USAGE}"); return; }
        "-V" | "--version" | "version" => { println!("cloud-sessions {}", env!("CARGO_PKG_VERSION")); return; }
        "mcp" => {
            if let Err(error) = mcp::serve(&ctx, &mut stdin().lock(), &mut stdout().lock()) { fail(&error.to_string()); }
            return;
        }
        "auth" => auth(&ctx.home, rest, &mut stdin().lock(), stdin().is_terminal(), &mut std::io::stderr()),
        "watch" => watch(&ctx, rest, &mut |line| { print(&line); let _ = stdout().flush(); }),
        other => run(&ctx, other, rest, &mut stdin().lock()),
    };
    match outcome {
        Ok(value) => {
            // `list` at a terminal prints text for a person; scripts, pipes and --json get the JSON.
            if command == "list" && value["ok"] == true && stdout().is_terminal() && !rest.iter().any(|a| a == "--json") { print!("{}", list_text(&value)); return; }
            print(&value);
            if value["ok"] == false { std::process::exit(1); }
        }
        Err(error) => fail(&error),
    }
}
