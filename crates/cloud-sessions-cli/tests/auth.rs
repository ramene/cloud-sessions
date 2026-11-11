//! Token sources and the Claude-login consent flow. One test: it sets HOME and CLOUD_SESSIONS_* for the process.
#![cfg(unix)]
use cloud_sessions_cli::auth::{auth, resolve};
use std::os::unix::fs::PermissionsExt;

fn args(list: &[&str]) -> Vec<String> { list.iter().map(|s| s.to_string()).collect() }

#[test]
fn token_file_and_opt_in_claude_login_never_leak_the_token() {
    let root = std::env::temp_dir().join(format!("cs-auth-{}", std::process::id()));
    let (home, fake_home) = (root.join("cfg"), root.join("home"));
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::env::set_var("HOME", &fake_home);
    std::env::remove_var("CLOUD_SESSIONS_TOKEN_FILE");
    std::env::remove_var("CLOUD_SESSIONS_USE_CLAUDE_LOGIN");
    std::env::remove_var("CLAUDE_CONFIG_DIR");
    let mut sink = Vec::new();

    // set --file: private file, and resolve reads it. Status never shows a value.
    let out = auth(&home, &args(&["set", "--file"]), &mut "file-token-123\n".as_bytes(), false, &mut sink).unwrap();
    assert_eq!(out["stored"], "file");
    assert_eq!(std::fs::metadata(home.join("token")).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(resolve(&home).unwrap(), "file-token-123");
    let status = auth(&home, &args(&["status"]), &mut std::io::empty(), false, &mut sink).unwrap().to_string();
    assert!(status.contains("\"token_file\":true") && !status.contains("file-token-123"));
    assert!(auth(&home, &args(&["set", "--file"]), &mut "two words".as_bytes(), false, &mut sink).is_err());

    // A token file others can read is refused.
    std::fs::set_permissions(home.join("token"), std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(resolve(&home).unwrap_err().contains("chmod 600"));
    std::fs::set_permissions(home.join("token"), std::fs::Permissions::from_mode(0o600)).unwrap();

    // Consent cannot be given without a terminal, nor with the wrong answer.
    assert!(auth(&home, &args(&["use-claude-login"]), &mut "yes\n".as_bytes(), false, &mut sink).unwrap_err().contains("at a terminal"));
    assert!(!home.join("claude-login-consent").exists());
    let no = auth(&home, &args(&["use-claude-login"]), &mut "no\n".as_bytes(), true, &mut sink).unwrap();
    assert_eq!(no["ok"], false);
    assert!(String::from_utf8_lossy(&sink).contains("never printed"));
    assert_eq!(resolve(&home).unwrap(), "file-token-123", "still the file: no consent yet");

    // With consent (Linux/other: ~/.claude/.credentials.json) the Claude login wins and is not echoed.
    #[cfg(not(target_os = "macos"))]
    {
        std::fs::write(fake_home.join(".claude/.credentials.json"), r#"{"claudeAiOauth":{"accessToken":"login-token-456","refreshToken":"refresh-never","expiresAt":99999999999999}}"#).unwrap();
        let yes = auth(&home, &args(&["use-claude-login"]), &mut "yes\n".as_bytes(), true, &mut sink).unwrap();
        assert_eq!((yes["ok"].clone(), yes["readable_now"].clone()), (serde_json::json!(true), serde_json::json!(true)));
        assert!(!yes.to_string().contains("login-token-456"));
        assert_eq!(resolve(&home).unwrap(), "login-token-456");
        // CLAUDE_CONFIG_DIR moves the login file; set but empty, it is ignored.
        let work = root.join("claude-work");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join(".credentials.json"), r#"{"claudeAiOauth":{"accessToken":"work-token-789","expiresAt":99999999999999}}"#).unwrap();
        std::env::set_var("CLAUDE_CONFIG_DIR", &work);
        assert_eq!(resolve(&home).unwrap(), "work-token-789");
        std::env::set_var("CLAUDE_CONFIG_DIR", "");
        assert_eq!(resolve(&home).unwrap(), "login-token-456");
        std::env::remove_var("CLAUDE_CONFIG_DIR");
        std::fs::write(fake_home.join(".claude/.credentials.json"), r#"{"claudeAiOauth":{"accessToken":"old","expiresAt":1}}"#).unwrap();
        assert!(resolve(&home).unwrap_err().contains("expired"));
    }
    #[cfg(target_os = "macos")]
    std::fs::write(home.join("claude-login-consent"), "allowed\n").unwrap();

    // Revoking returns to the file; clearing removes it, and the error then explains the options.
    auth(&home, &args(&["revoke-claude-login"]), &mut std::io::empty(), false, &mut sink).unwrap();
    assert_eq!(resolve(&home).unwrap(), "file-token-123");
    auth(&home, &args(&["clear"]), &mut std::io::empty(), false, &mut sink).unwrap();
    assert!(resolve(&home).unwrap_err().contains("use-claude-login"));
    let all = String::from_utf8_lossy(&sink);
    assert!(!all.contains("file-token-123") && !all.contains("login-token-456") && !all.contains("work-token-789"));
}
