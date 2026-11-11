//! Where a read token can come from besides the system keychain. This crate never writes secrets and never
//! prints them; the callers decide which sources are allowed.
//!
//! * [`file_token`]: a token file, accepted only when it is private to the user (no group or other access).
//! * [`claude_login_token`]: the access token Claude Code itself keeps after `claude` login. Callers must have
//!   the user's explicit opt-in before calling it. It is read, never copied, refreshed or stored here. A token
//!   from `claude setup-token` is inference-only and cannot read sessions; Claude Code's own login can.
use crate::Result;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

fn plausible(token: &str) -> bool {
    !token.is_empty() && token.len() <= 4096 && !token.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// Ok(None) when the file does not exist; Err when it is readable by others (unix) or malformed.
pub fn file_token(path: &Path) -> Result<Option<String>> {
    let meta = match std::fs::metadata(path) { Ok(m) => m, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None), Err(_) => return Err("The token file could not be read.".into()) };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 { return Err("The token file must be private: run `chmod 600` on it.".into()); }
    }
    if !meta.is_file() || meta.len() > 8192 { return Err("The token file is not a small regular file.".into()); }
    let text = std::fs::read_to_string(path).map_err(|_| "The token file could not be read.".to_string())?;
    let token = text.trim();
    if plausible(token) { Ok(Some(token.to_string())) } else { Err("The token file does not hold a token.".into()) }
}

/// The access token (and its expiry in ms since the epoch, if present) from Claude Code's credential JSON:
/// `{"claudeAiOauth":{"accessToken":…,"expiresAt":…}}`. The refresh token is ignored on purpose.
pub fn parse_claude_credentials(text: &str) -> Option<(String, Option<i64>)> {
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    let oauth = if value["claudeAiOauth"].is_object() { &value["claudeAiOauth"] } else { &value };
    let token = oauth["accessToken"].as_str()?.trim().to_string();
    plausible(&token).then(|| (token, oauth["expiresAt"].as_i64()))
}

/// Claude Code's login from the macOS keychain item "Claude Code-credentials" (read with the `security` tool at
/// `security_bin`, which may prompt the user). None when the keychain has no readable login (no item, a locked
/// keychain over ssh, or text that is not a credential).
fn keychain_login(security_bin: &Path) -> Option<(String, Option<i64>)> {
    let mut command = std::process::Command::new(security_bin);
    command.args(["find-generic-password", "-s", "Claude Code-credentials", "-w"]);
    let out = crate::cli::run_bounded(command, Duration::from_secs(60), 64 * 1024).ok()?;
    parse_claude_credentials(&String::from_utf8_lossy(&out))
}

/// Read Claude Code's own login credential. With `use_keychain` (macOS) the login keychain item
/// "Claude Code-credentials" comes first; when the keychain has no login, the file `file` is used. Elsewhere only
/// the file (`$CLAUDE_CONFIG_DIR/.credentials.json`, else `~/.claude/.credentials.json`; the caller picks it).
/// An expired token is an error: this crate does not refresh it, so run `claude` once to renew it.
pub fn claude_login_token(use_keychain: bool, security_bin: &Path, file: &Path, now_ms: i64) -> Result<String> {
    let found = if use_keychain { keychain_login(security_bin) } else { None };
    let (token, expires) = match found {
        Some(login) => login,
        None => {
            let text = std::fs::read_to_string(file).map_err(|_| if use_keychain {
                "Claude Code's login credential was not found in the keychain or in its credentials file. Sign in with `claude` first.".to_string()
            } else {
                "Claude Code's login credential was not found. Sign in with `claude` first.".to_string()
            })?;
            parse_claude_credentials(&text).ok_or("Claude Code's login credential was not found or is not readable. Sign in with `claude` first.")?
        }
    };
    if expires.is_some_and(|e| e <= now_ms) { return Err("Claude Code's login has expired. Run `claude` once to renew it, then try again.".into()); }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::tests::{fake_claude, scratch};
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn token_files_must_be_private() {
        let dir = scratch("tok");
        let path = dir.join("token");
        assert_eq!(file_token(&path).unwrap(), None);
        std::fs::write(&path, "sk-ant-oat01-abc\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(file_token(&path).unwrap_err().contains("chmod 600"));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(file_token(&path).unwrap().as_deref(), Some("sk-ant-oat01-abc"));
        std::fs::write(&path, "two words").unwrap();
        assert!(file_token(&path).is_err());
    }

    #[test]
    fn claude_credentials_yield_only_the_access_token_and_expiry_is_enforced() {
        let json = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-login","refreshToken":"sk-ant-ort01-never","expiresAt":2000}}"#;
        assert_eq!(parse_claude_credentials(json), Some(("sk-ant-oat01-login".into(), Some(2000))));
        assert_eq!(parse_claude_credentials(r#"{"accessToken":"t-1"}"#), Some(("t-1".into(), None)));
        assert_eq!(parse_claude_credentials("not json"), None);
        let dir = scratch("login");
        let file = dir.join("creds.json");
        std::fs::write(&file, json).unwrap();
        assert_eq!(claude_login_token(false, Path::new("security"), &file, 1000).unwrap(), "sk-ant-oat01-login");
        assert!(claude_login_token(false, Path::new("security"), &file, 3000).unwrap_err().contains("expired"));
        assert!(claude_login_token(false, Path::new("security"), &dir.join("missing"), 0).unwrap_err().contains("not found"));
        // The macOS path runs `security find-generic-password -s "Claude Code-credentials" -w`.
        let args = dir.join("args.txt");
        let security = fake_claude(&dir, &format!("printf '%s\\n' \"$@\" > {}\ncat {}", args.display(), file.display()));
        assert_eq!(claude_login_token(true, &security, &dir.join("unused"), 1000).unwrap(), "sk-ant-oat01-login");
        assert_eq!(std::fs::read_to_string(&args).unwrap().lines().collect::<Vec<_>>(), ["find-generic-password", "-s", "Claude Code-credentials", "-w"]);
    }

    #[test]
    fn macos_keychain_without_a_login_falls_back_to_the_credentials_file() {
        let dir = scratch("fallback");
        let file = dir.join("creds.json");
        std::fs::write(&file, r#"{"claudeAiOauth":{"accessToken":"file-login","expiresAt":2000}}"#).unwrap();
        // `security` exits 44 with no output when the item does not exist.
        let sub = |n: &str| { let d = dir.join(n); std::fs::create_dir_all(&d).unwrap(); d };
        let none = fake_claude(&sub("none"), "exit 44");
        assert_eq!(claude_login_token(true, &none, &file, 1000).unwrap(), "file-login");
        // A keychain answer that is not a credential falls back too.
        let junk = fake_claude(&sub("junk"), "echo not-json");
        assert_eq!(claude_login_token(true, &junk, &file, 1000).unwrap(), "file-login");
        // A security tool that cannot start (missing binary) falls back too.
        assert_eq!(claude_login_token(true, &dir.join("no-such-security"), &file, 1000).unwrap(), "file-login");
        // The expiry check applies to the file token.
        assert!(claude_login_token(true, &none, &file, 3000).unwrap_err().contains("expired"));
        // Nothing in either place: one clear error, no token text.
        let err = claude_login_token(true, &none, &dir.join("missing.json"), 0).unwrap_err();
        assert!(err.contains("keychain or in its credentials file"), "{err}");
        // The keychain wins over the file when it has a login.
        let other = dir.join("kc.json");
        std::fs::write(&other, r#"{"claudeAiOauth":{"accessToken":"keychain-login","expiresAt":2000}}"#).unwrap();
        let kc = fake_claude(&sub("kc"), &format!("cat {}", other.display()));
        assert_eq!(claude_login_token(true, &kc, &file, 1000).unwrap(), "keychain-login");
        // An expired keychain login is an error (it is not replaced by the file).
        assert!(claude_login_token(true, &kc, &file, 3000).unwrap_err().contains("expired"));
    }
}
