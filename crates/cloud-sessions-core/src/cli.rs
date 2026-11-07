//! The two CLI mechanisms: START in a pseudo-terminal and SEND with `-p`.
use crate::events::{clean, one_line};
use crate::policy::{parse_session_id, session_url};
use crate::Result;
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StartStatus {
    Started,
    /// The CLI asked whether to trust the folder. The default answer is "No, exit" and is never typed for the user.
    NeedsTrust,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Started {
    pub status: StartStatus,
    pub session_id: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
    /// The CLI's last output (cleaned), for a trust prompt.
    pub output: String,
}

pub fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.peek() {
                Some('[') => { chars.next(); for n in chars.by_ref() { if ('@'..='~').contains(&n) { break; } } }
                Some(']') => { chars.next(); for n in chars.by_ref() { if n == '\u{7}' || n == '\u{1b}' { break; } } }
                _ => { chars.next(); }
            }
        } else if c != '\r' { out.push(c); }
    }
    out
}
/// `(title, url, session id)` from the CLI's output, once both lines are there.
pub fn parse_started(output: &str) -> Option<(String, String, String)> {
    let text = strip_ansi(output);
    let title = text.lines().find_map(|l| l.trim().strip_prefix("Created cloud session:")).map(|t| t.trim().to_string())?;
    let url = text.lines().find_map(|l| l.trim().strip_prefix("View:")).map(|u| u.trim().to_string())?;
    let id = parse_session_id(&url).ok()?;
    Some((clean(&title, 200), url, id))
}
pub fn looks_like_trust_prompt(output: &str) -> bool {
    let text = strip_ansi(output).to_lowercase();
    ["do you trust", "trust the files", "trust this folder", "no, exit"].iter().any(|p| text.contains(p))
}

fn started(title: String, url: String, id: String) -> Started {
    Started { status: StartStatus::Started, session_id: Some(id), title: Some(title), url: Some(url), output: String::new() }
}

/// Run `claude --cloud <prompt>` in a PTY inside `dir`, read until both result lines appear, then stop the
/// process (the session lives in the cloud). Never types into the terminal.
pub fn start(bin: &Path, dir: &Path, prompt: &str, timeout: Duration) -> Result<Started> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    if prompt.trim().is_empty() || prompt.starts_with('-') { return Err("The prompt must not be empty or start with a dash.".into()); }
    let pair = native_pty_system().openpty(PtySize { rows: 40, cols: 120, pixel_width: 0, pixel_height: 0 }).map_err(|e| format!("Could not open a terminal: {e}"))?;
    let mut command = CommandBuilder::new(bin);
    command.args(["--cloud", prompt]);
    command.cwd(dir);
    command.env("TERM", "dumb");
    command.env("NO_COLOR", "1");
    let mut child = pair.slave.spawn_command(command).map_err(|e| format!("Could not start Claude Code: {e}"))?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let (sender, receiver) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buffer) { if n == 0 || sender.send(buffer[..n].to_vec()).is_err() { break; } }
    });
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    let result: Result<Started> = loop {
        let text = String::from_utf8_lossy(&output).into_owned();
        if let Some((title, url, id)) = parse_started(&text) { break Ok(started(title, url, id)); }
        if looks_like_trust_prompt(&text) {
            // Let the rest of the prompt arrive so the user sees the whole question.
            while let Ok(chunk) = receiver.recv_timeout(Duration::from_millis(400)) { output.extend(chunk); if output.len() > 64 * 1024 { break; } }
            let text = String::from_utf8_lossy(&output).into_owned();
            break Ok(Started { status: StartStatus::NeedsTrust, session_id: None, title: None, url: None, output: one_line(&text, 600) });
        }
        let Some(left) = deadline.checked_duration_since(Instant::now()) else { break Err(format!("Claude Code did not report a session in time. Output: {}", one_line(&text, 300))) };
        match receiver.recv_timeout(left.min(Duration::from_millis(250))) {
            Ok(chunk) => { output.extend(chunk); if output.len() > 256 * 1024 { break Err("Claude Code printed too much output.".into()); } }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                let text = String::from_utf8_lossy(&output).into_owned();
                break match parse_started(&text) { Some((title, url, id)) => Ok(started(title, url, id)), None => Err(format!("Claude Code ended without starting a session: {}", one_line(&text, 400))) };
            }
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    result
}

/// Run a command with a deadline and a stdout cap; returns its stdout.
pub(crate) fn run_bounded(mut command: Command, timeout: Duration, limit: usize) -> Result<Vec<u8>> {
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| format!("Could not start Claude Code: {e}"))?;
    let mut stdout = child.stdout.take().ok_or("No output pipe.")?;
    let reader = std::thread::spawn(move || { let mut bytes = Vec::new(); let _ = (&mut stdout).take(limit as u64).read_to_end(&mut bytes); bytes });
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(_) => break,
            None if Instant::now() >= deadline => { let _ = child.kill(); let _ = child.wait(); return Err("Claude Code did not answer in time.".into()); }
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    }
    Ok(reader.join().unwrap_or_default())
}

/// `claude -p <message> --cloud <id> --output-format json`; the CLI's own `error` text on failure. The message
/// is one argument (no shell), so nothing in it is interpreted. Returns the session URL.
pub fn send(bin: &Path, dir: Option<&Path>, session_id: &str, message: &str, timeout: Duration) -> Result<String> {
    let id = parse_session_id(session_id)?;
    if message.trim().is_empty() { return Err("The message is empty.".into()); }
    let mut command = Command::new(bin);
    command.args(["-p", message, "--cloud", &id, "--output-format", "json"]);
    if let Some(dir) = dir.filter(|d| d.is_dir()) { command.current_dir(dir); }
    let stdout = run_bounded(command, timeout, 1024 * 1024)?;
    let text = String::from_utf8_lossy(&stdout).into_owned();
    let parsed = text.lines().rev().find_map(|line| serde_json::from_str::<Value>(line.trim()).ok().filter(Value::is_object)).or_else(|| serde_json::from_str::<Value>(text.trim()).ok());
    match parsed {
        Some(v) if v["ok"] == true => Ok(v["url"].as_str().map(str::to_string).unwrap_or_else(|| session_url(&id))),
        Some(v) => Err(one_line(v["error"].as_str().unwrap_or("Claude Code could not send the message."), 400)),
        None => Err(format!("Claude Code gave an unreadable answer: {}", one_line(&text, 200))),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    pub fn scratch(name: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("cs-core-{name}-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
    pub fn fake_claude(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn cli_output_is_parsed_through_ansi() {
        let out = "\u{1b}[32mCreated cloud session: Fix flaky\u{1b}[0m\r\nView: https://claude.ai/code/session_01Zz9Yy8\r\n";
        assert_eq!(parse_started(out), Some(("Fix flaky".into(), "https://claude.ai/code/session_01Zz9Yy8".into(), "session_01Zz9Yy8".into())));
        assert_eq!(parse_started("Created cloud session: x\n"), None);
        assert!(looks_like_trust_prompt("Do you trust the files in this folder?\n 1. Yes\n 2. No, exit"));
    }

    #[test]
    fn start_uses_a_terminal_and_never_answers_the_trust_prompt() {
        let dir = scratch("start");
        let ok = fake_claude(&dir, r#"[ -t 0 ] && [ -t 1 ] || { echo "--cloud requires an interactive terminal"; exit 1; }
[ "$1" = "--cloud" ] || exit 2
echo "Created cloud session: $2"; echo "View: https://claude.ai/code/session_01Pty12345"; sleep 30"#);
        let s = start(&ok, &dir, "Fix tests", Duration::from_secs(20)).unwrap();
        assert_eq!((s.status, s.session_id.as_deref(), s.title.as_deref()), (StartStatus::Started, Some("session_01Pty12345"), Some("Fix tests")));

        let trust_dir = scratch("trust");
        let trust = fake_claude(&trust_dir, r#"echo "Do you trust the files in this folder?"; echo "  1. Yes, proceed"; echo "  2. No, exit"
if read -r answer; then echo "ANSWERED $answer" > answered.txt; fi; sleep 30"#);
        let outcome = start(&trust, &trust_dir, "Fix tests", Duration::from_secs(20)).unwrap();
        assert_eq!(outcome.status, StartStatus::NeedsTrust);
        assert!(outcome.output.contains("No, exit"));
        std::thread::sleep(Duration::from_millis(300));
        assert!(!trust_dir.join("answered.txt").exists(), "nothing was typed into the prompt");

        let broken = fake_claude(&dir, "echo boom; exit 1");
        assert!(start(&broken, &dir, "x", Duration::from_secs(10)).unwrap_err().contains("boom"));
        assert!(start(&ok, &dir, "--dangerous", Duration::from_secs(10)).is_err());
    }

    #[test]
    fn send_runs_the_documented_command_and_surfaces_errors() {
        let dir = scratch("send");
        let log = dir.join("args.txt");
        let ok = fake_claude(&dir, &format!(r#"printf '%s\n' "$@" > {}
echo '{{"ok":true,"session_id":"session_01Send1234","url":"https://claude.ai/code/session_01Send1234"}}'"#, log.display()));
        assert_eq!(send(&ok, Some(&dir), "session_01Send1234", "also add a test; $(rm -rf /)", Duration::from_secs(20)).unwrap(), "https://claude.ai/code/session_01Send1234");
        let args = std::fs::read_to_string(&log).unwrap();
        assert_eq!(args.lines().collect::<Vec<_>>(), ["-p", "also add a test; $(rm -rf /)", "--cloud", "session_01Send1234", "--output-format", "json"]);
        let refused = fake_claude(&dir, r#"echo '{"ok":false,"error":"Session is archived"}'; exit 1"#);
        assert_eq!(send(&refused, None, "session_01Send1234", "hi", Duration::from_secs(20)).unwrap_err(), "Session is archived");
        let slow = fake_claude(&dir, "sleep 30");
        assert!(send(&slow, None, "session_01Send1234", "hi", Duration::from_millis(300)).unwrap_err().contains("in time"));
        assert!(send(&ok, None, "bad id", "hi", Duration::from_secs(5)).is_err());
    }
}
