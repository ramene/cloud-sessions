//! Where Claude Code's login file is looked for: `$CLAUDE_CONFIG_DIR/.credentials.json`, else
//! `~/.claude/.credentials.json`. The environment is injected; the process environment is not changed.
#![cfg(unix)]
use cloud_sessions_cli::auth::claude_creds_file_from;
use cloud_sessions_core::credentials::claude_login_token;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_800_000_000_000;

fn home(name: &str) -> PathBuf {
    let run = format!("{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cs-login-file").join(run).join(name);
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    dir
}
fn login(token: &str, expires: i64) -> String {
    format!(r#"{{"claudeAiOauth":{{"accessToken":"{token}","refreshToken":"refresh-never-used","expiresAt":{expires}}}}}"#)
}
fn path(vars: &[(&str, String)]) -> PathBuf {
    let map: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    claude_creds_file_from(&|k| map.get(k).cloned())
}
fn h(home: &Path) -> (&'static str, String) { ("HOME", home.display().to_string()) }

#[test]
fn default_is_the_home_claude_folder() {
    let home = home("default");
    assert_eq!(path(&[h(&home)]), home.join(".claude/.credentials.json"));
    assert_eq!(path(&[]), PathBuf::new());
}

#[test]
fn claude_config_dir_moves_the_login_file_and_empty_is_ignored() {
    let home = home("configdir");
    let work = home.join("claude-work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(home.join(".claude/.credentials.json"), login("default-dir-tok", NOW + 1)).unwrap();
    std::fs::write(work.join(".credentials.json"), login("work-dir-tok", NOW + 1)).unwrap();
    let with_dir = path(&[h(&home), ("CLAUDE_CONFIG_DIR", work.display().to_string())]);
    assert_eq!(with_dir, work.join(".credentials.json"));
    assert_eq!(claude_login_token(false, Path::new("security"), &with_dir, NOW).unwrap(), "work-dir-tok");
    let empty = path(&[h(&home), ("CLAUDE_CONFIG_DIR", "  ".into())]);
    assert_eq!(claude_login_token(false, Path::new("security"), &empty, NOW).unwrap(), "default-dir-tok");
    // CLAUDE_CONFIG_DIR without HOME still works.
    assert_eq!(path(&[("CLAUDE_CONFIG_DIR", work.display().to_string())]), work.join(".credentials.json"));
}

#[test]
fn expired_login_in_the_config_dir_is_refused_without_the_token() {
    let home = home("expired");
    let work = home.join("w");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join(".credentials.json"), login("old-secret-tok", NOW - 1)).unwrap();
    let file = path(&[h(&home), ("CLAUDE_CONFIG_DIR", work.display().to_string())]);
    let err = claude_login_token(false, Path::new("security"), &file, NOW).unwrap_err();
    assert!(err.contains("expired") && !err.contains("old-secret-tok"), "{err}");
}

#[test]
fn macos_fallback_reads_the_config_dir_file_when_the_keychain_has_no_login() {
    use std::os::unix::fs::PermissionsExt;
    let home = home("macfallback");
    let work = home.join("w");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join(".credentials.json"), login("fallback-tok", NOW + 1)).unwrap();
    // A stand-in for /usr/bin/security that finds no item (exit 44, no output).
    let security = home.join("security");
    std::fs::write(&security, "#!/bin/sh\nexit 44\n").unwrap();
    std::fs::set_permissions(&security, std::fs::Permissions::from_mode(0o755)).unwrap();
    let file = path(&[h(&home), ("CLAUDE_CONFIG_DIR", work.display().to_string())]);
    assert_eq!(claude_login_token(true, &security, &file, NOW).unwrap(), "fallback-tok");
}
