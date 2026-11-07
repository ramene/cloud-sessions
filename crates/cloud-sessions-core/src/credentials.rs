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

/// Read Claude Code's own login credential. On macOS that is the login keychain item "Claude Code-credentials"
/// (read with the `security` tool at `security_bin`, which may prompt the user); elsewhere the file `file`
/// (normally `~/.claude/.credentials.json`). An expired token is an error: this crate does not refresh it, so
/// run `claude` once to renew it.
pub fn claude_login_token(use_keychain: bool, security_bin: &Path, file: &Path, now_ms: i64) -> Result<String> {
    let text = if use_keychain {
        let mut command = std::process::Command::new(security_bin);
        command.args(["find-generic-password", "-s", "Claude Code-credentials", "-w"]);
        let out = crate::cli::run_bounded(command, Duration::from_secs(60), 64 * 1024).map_err(|_| "Could not read Claude Code's login from the keychain.".to_string())?;
        String::from_utf8_lossy(&out).into_owned()
    } else {
        std::fs::read_to_string(file).map_err(|_| "Claude Code's login credential was not found. Sign in with `claude` first.".to_string())?
    };
    let (token, expires) = parse_claude_credentials(&text).ok_or("Claude Code's login credential was not found or is not readable. Sign in with `claude` first.")?;
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
}
