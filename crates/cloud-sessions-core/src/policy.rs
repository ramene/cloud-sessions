//! Validation and limits shared by every front end.
use crate::Result;
use std::path::Path;

/// `https://claude.ai/code/session_01Ab…`, `https://claude.ai/…/session_…` or a bare id → the id.
pub fn parse_session_id(input: &str) -> Result<String> {
    let text = input.trim();
    let last = if text.contains('/') {
        let rest = text.strip_prefix("https://").ok_or("Use the session's https://claude.ai/code/… link, or its ID.")?;
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if host != "claude.ai" { return Err("Only claude.ai session links are accepted.".into()); }
        path.split(['?', '#']).next().unwrap_or("").trim_end_matches('/').rsplit('/').next().unwrap_or("")
    } else { text };
    if !((8..=100).contains(&last.len()) && last.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')) {
        return Err("That is not a valid session link or ID.".into());
    }
    // One canonical form per session: `session_<id>`. The routines API reports the same session as `cse_<id>`,
    // and users copy the bare `<id>` from links; both map to `session_<id>` so a session is never tracked twice.
    Ok(if let Some(rest) = last.strip_prefix("cse_") {
        format!("session_{rest}")
    } else if last.starts_with("session_") {
        last.to_string()
    } else if last.starts_with("01") && !last.contains('_') {
        format!("session_{last}")
    } else {
        last.to_string()
    })
}

#[cfg(test)]
mod canonical_id_tests {
    use super::parse_session_id;
    #[test]
    fn every_form_of_one_session_maps_to_the_same_id() {
        let want = "session_01ExampleFFFFFFFFFFFFFFF";
        for form in [
            "session_01ExampleFFFFFFFFFFFFFFF",
            "01ExampleFFFFFFFFFFFFFFF",
            "cse_01ExampleFFFFFFFFFFFFFFF",
            "https://claude.ai/code/session_01ExampleFFFFFFFFFFFFFFF",
            "https://claude.ai/code/session_01ExampleFFFFFFFFFFFFFFF?from=cli&m=0",
            "  https://claude.ai/code/session_01ExampleFFFFFFFFFFFFFFF/  ",
        ] {
            assert_eq!(parse_session_id(form).unwrap(), want, "form: {form}");
        }
    }
}
pub fn session_url(id: &str) -> String {
    format!("https://claude.ai/code/{id}")
}

/// `https://github.com/Owner/Repo.git` or `owner/repo` → `owner/repo` (lower case).
pub fn repo_slug(text: &str) -> Option<String> {
    let text = text.trim().trim_end_matches('/');
    let text = text.strip_suffix(".git").or_else(|| text.strip_suffix(".GIT")).unwrap_or(text);
    let text = text.strip_prefix("https://github.com/").or_else(|| text.strip_prefix("git@github.com:")).unwrap_or(text);
    let (owner, name) = text.split_once('/')?;
    let owner_ok = (1..=39).contains(&owner.len()) && owner.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') && !owner.starts_with('-');
    let name_ok = (1..=100).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) && name != "." && name != "..";
    (owner_ok && name_ok).then(|| format!("{owner}/{name}").to_ascii_lowercase())
}

/// The checkout's `origin` as `owner/name`.
pub fn origin_slug(dir: &Path) -> Result<String> {
    if !dir.is_dir() { return Err("Choose a repository folder.".into()); }
    let output = std::process::Command::new("git").arg("-C").arg(dir).args(["remote", "get-url", "origin"]).output().map_err(|e| format!("Could not run git: {e}"))?;
    if !output.status.success() { return Err("This folder has no origin remote.".into()); }
    repo_slug(String::from_utf8_lossy(&output.stdout).trim()).ok_or_else(|| "This folder's origin is not a GitHub repository.".into())
}

/// The checkout's `origin` as `owner/name`, which must be on `allowed` (an empty list refuses everything).
pub fn allowed_repo(dir: &Path, allowed: &[String]) -> Result<String> {
    let slug = origin_slug(dir)?;
    let allowed: Vec<String> = allowed.iter().filter_map(|r| repo_slug(r)).collect();
    if allowed.contains(&slug) { Ok(slug) } else { Err(format!("{slug} is not on the repository allowlist.")) }
}

pub const SEND_GAP_SECS: u64 = 2;
pub const SEND_PER_HOUR: usize = 60;

/// Admit one send for a session given its past send times (unix seconds); records it on success.
/// The caller persists `history` if it needs the limit to span processes.
pub fn admit_send(history: &mut Vec<u64>, now: u64) -> Result<()> {
    history.retain(|t| now.saturating_sub(*t) <= 3600);
    if history.last().is_some_and(|t| now.saturating_sub(*t) < SEND_GAP_SECS) { return Err("Wait a moment before sending again.".into()); }
    if history.len() >= SEND_PER_HOUR { return Err(format!("At most {SEND_PER_HOUR} messages an hour can be sent to one cloud session.")); }
    history.push(now);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_and_ids_are_validated() {
        assert_eq!(parse_session_id("https://claude.ai/code/session_01AbCdEf?x=1").unwrap(), "session_01AbCdEf");
        assert_eq!(parse_session_id(" session_01AbCdEf ").unwrap(), "session_01AbCdEf");
        for bad in ["https://evil.example/code/session_01AbCdEf", "http://claude.ai/code/session_01AbCdEf", "../../etc", "abc", ""] {
            assert!(parse_session_id(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn slugs_accept_github_only() {
        assert_eq!(repo_slug("git@github.com:Acme/Widgets.git").as_deref(), Some("acme/widgets"));
        assert_eq!(repo_slug("https://evil.example/acme/widgets"), None);
        assert_eq!(repo_slug("acme/.."), None);
    }
    #[test]
    fn sends_are_spaced_and_capped() {
        let mut h = Vec::new();
        admit_send(&mut h, 1000).unwrap();
        assert!(admit_send(&mut h, 1001).is_err());
        for n in 1..SEND_PER_HOUR as u64 { admit_send(&mut h, 1000 + n * 2).unwrap(); }
        assert!(admit_send(&mut h, 1000 + SEND_PER_HOUR as u64 * 2 + 2).unwrap_err().contains("an hour"));
        assert!(admit_send(&mut h, 1000 + 4000).is_ok(), "old sends age out");
    }
    #[test]
    fn allowlist_checks_the_checkout_origin() {
        let dir = std::env::temp_dir().join(format!("cs-core-repo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(&dir).args(args).status().unwrap().success());
        git(&["init", "-q"]);
        git(&["remote", "add", "origin", "https://github.com/Acme/Widgets.git"]);
        assert_eq!(allowed_repo(&dir, &["acme/widgets".into()]).unwrap(), "acme/widgets");
        assert!(allowed_repo(&dir, &["other/repo".into()]).unwrap_err().contains("allowlist"));
        assert!(allowed_repo(&dir, &[]).is_err());
    }
}
