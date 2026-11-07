//! Where the read token comes from, and how the user manages it. Nothing here ever prints a token.
//!
//! Sources, in order: (1) Claude Code's own login, only after the user opted in at a terminal; (2) a private
//! token file (`$CLOUD_SESSIONS_TOKEN_FILE` or `<home>/token`, mode 0600); (3) the system keychain. A token from
//! `claude setup-token` is inference-only and cannot read sessions, so reading normally needs (1); starting and
//! sending use the `claude` CLI's own login and need none of this.
use crate::Result;
use cloud_sessions_core::credentials;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub const SERVICE: &str = "cloud-sessions";
pub const ACCOUNT: &str = "token";
const CONSENT: &str = "claude-login-consent";

pub fn consent_path(home: &Path) -> PathBuf { home.join(CONSENT) }
pub fn token_file(home: &Path) -> PathBuf {
    std::env::var_os("CLOUD_SESSIONS_TOKEN_FILE").map(PathBuf::from).unwrap_or_else(|| home.join("token"))
}
pub fn has_consent(home: &Path) -> bool {
    consent_path(home).is_file() || std::env::var("CLOUD_SESSIONS_USE_CLAUDE_LOGIN").is_ok_and(|v| v == "1")
}
fn claude_creds_file() -> PathBuf {
    std::env::var_os("HOME").map(|h| Path::new(&h).join(".claude/.credentials.json")).unwrap_or_default()
}
fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn keychain() -> Option<String> {
    keyring::Entry::new(SERVICE, ACCOUNT).ok()?.get_password().ok().filter(|t| !t.trim().is_empty())
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn keychain() -> Option<String> { None }
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn keychain_set(token: &str) -> Result<()> {
    keyring::Entry::new(SERVICE, ACCOUNT).and_then(|e| e.set_password(token)).map_err(|_| "Could not save the token in the system keychain (a locked login keychain, for example over ssh, does this). Use `cloud-sessions auth set --file` to keep it in a private 0600 file instead.".to_string())
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn keychain_set(_: &str) -> Result<()> { Err("This build has no system keychain on Linux. Use `cloud-sessions auth set --file` to keep the token in a private 0600 file.".into()) }
fn keychain_clear() {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Ok(e) = keyring::Entry::new(SERVICE, ACCOUNT) { let _ = e.delete_credential(); }
}

/// The token to read events with, or why there is none.
pub fn resolve(home: &Path) -> Result<String> {
    if has_consent(home) {
        return credentials::claude_login_token(cfg!(target_os = "macos"), Path::new("/usr/bin/security"), &claude_creds_file(), now_ms());
    }
    if let Some(token) = credentials::file_token(&token_file(home))? { return Ok(token); }
    keychain().ok_or_else(|| "Reading events needs a token. Run `cloud-sessions auth use-claude-login` in a terminal to reuse Claude Code's own login (a `claude setup-token` token cannot read sessions), or store a token that can with `cloud-sessions auth set` (stdin; add --file where the keychain is locked).".into())
}

const EXPLAIN: &str = "This lets cloud-sessions read the access token Claude Code keeps after you sign in\n(macOS: the login keychain item \"Claude Code-credentials\"; elsewhere ~/.claude/.credentials.json),\nonly to call GET /v1/code/sessions/<id>/events. It is read each time it is needed; it is never printed,\ncopied, refreshed or stored by cloud-sessions. macOS may ask you to allow keychain access.\nUndo with: cloud-sessions auth revoke-claude-login\nType yes to allow: ";

fn write_private(path: &Path, token: &str) -> Result<()> {
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir).map_err(|e| format!("Could not create the config folder: {e}"))?; }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path).map_err(|e| format!("Could not write the token file: {e}"))?;
        // An existing file keeps its old mode on open: tighten it too.
        std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).map_err(|e| e.to_string())?;
        writeln!(file, "{token}").map_err(|e| e.to_string())
    }
    #[cfg(not(unix))]
    { std::fs::write(path, format!("{token}\n")).map_err(|e| format!("Could not write the token file: {e}")) }
}

/// `auth set [--file]` (token on stdin), `clear`, `status`, `use-claude-login`, `revoke-claude-login`.
/// `interactive` is true only when stdin is a terminal: consent cannot be given by a script or a tool call.
pub fn auth(home: &Path, args: &[String], input: &mut dyn BufRead, interactive: bool, prompt: &mut dyn Write) -> Result<Value> {
    let action = args.first().map_or("", String::as_str);
    let to_file = args.iter().any(|a| a == "--file");
    match action {
        "set" => {
            let mut token = String::new();
            input.read_to_string(&mut token).map_err(|e| e.to_string())?;
            let token = token.trim();
            if token.is_empty() || token.len() > 4096 || token.chars().any(|c| c.is_control() || c.is_whitespace()) { return Err("That token looks empty or malformed.".into()); }
            if to_file {
                write_private(&token_file(home), token)?;
                return Ok(json!({"ok": true, "stored": "file", "mode": "0600"}));
            }
            keychain_set(token)?;
            Ok(json!({"ok": true, "stored": "keychain"}))
        }
        "clear" => {
            keychain_clear();
            let _ = std::fs::remove_file(token_file(home));
            Ok(json!({"ok": true}))
        }
        "status" => Ok(json!({"ok": true, "claude_login_allowed": has_consent(home), "token_file": token_file(home).is_file(), "keychain": keychain().is_some(),
            "note": "Values are never shown. A `claude setup-token` token cannot read sessions."})),
        "use-claude-login" => {
            if !interactive { return Err("Allowing this needs you at a terminal: run `cloud-sessions auth use-claude-login` yourself and answer the question. It cannot be granted by a script, a flag or a tool call.".into()); }
            write!(prompt, "{EXPLAIN}").and_then(|_| prompt.flush()).map_err(|e| e.to_string())?;
            let mut answer = String::new();
            input.read_line(&mut answer).map_err(|e| e.to_string())?;
            if answer.trim() != "yes" { return Ok(json!({"ok": false, "error": "Not allowed (you did not type yes)."})); }
            std::fs::create_dir_all(home).map_err(|e| format!("Could not create the config folder: {e}"))?;
            std::fs::write(consent_path(home), "allowed\n").map_err(|e| format!("Could not save your choice: {e}"))?;
            let readable = credentials::claude_login_token(cfg!(target_os = "macos"), Path::new("/usr/bin/security"), &claude_creds_file(), now_ms());
            Ok(json!({"ok": true, "claude_login_allowed": true, "readable_now": readable.is_ok(), "note": readable.err()}))
        }
        "revoke-claude-login" => { let _ = std::fs::remove_file(consent_path(home)); Ok(json!({"ok": true, "claude_login_allowed": false})) }
        _ => Err("Use `auth set [--file]` (token on stdin), `clear`, `status`, `use-claude-login` or `revoke-claude-login`.".into()),
    }
}
