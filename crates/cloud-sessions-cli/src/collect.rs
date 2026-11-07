//! `collect`: gather the results of finished cloud sessions into one folder, plus an INDEX.
//!
//! Per session: the final `result` event (status, turns, duration, cost, final text), the pointers in the
//! text (`REPORT: <branch> <path> <sha>` first, else GitHub PR / branch / blob links), the files those point
//! at (downloaded at the exact head sha by shelling out to `gh`, which honours `GH_TOKEN`; no token is handled
//! here), `final.md`, `events.json`, and finally `index.json` + `INDEX.md` for all of them.
//!
//! Event text is untrusted: it is never executed or followed. Only strict, hand-written matchers read links and
//! lines out of it, every path is checked before a file is written, and `gh` is started without a shell.
use crate::{state, Args, Ctx, Result};
use cloud_sessions_core::events::one_line;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The last `result` event of a session, from the raw event list.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultInfo {
    pub status: String,
    pub turns: Option<u64>,
    pub duration_ms: Option<f64>,
    pub cost: Option<f64>,
    pub text: String,
}

fn payload(e: &Value) -> &Value { if e["payload"].is_object() { &e["payload"] } else { e } }
fn is_result(e: &Value) -> bool { e["event_type"] == "result" || payload(e)["type"] == "result" }

/// The last `result` event (list order: use `events::event_list` first so it is oldest first), or None while it runs.
pub fn parse_result(events: &[Value]) -> Option<ResultInfo> {
    let p = payload(events.iter().rev().find(|e| is_result(e))?);
    let sub = p["subtype"].as_str().unwrap_or("unknown");
    let status = if p["is_error"] == true && sub == "success" { "error".to_string() } else { sub.to_string() };
    Some(ResultInfo { status, turns: p["num_turns"].as_u64(), duration_ms: p["duration_ms"].as_f64(), cost: p["total_cost_usd"].as_f64(), text: p["result"].as_str().unwrap_or("").to_string() })
}

/// What the text points at.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pointers {
    /// `REPORT: <branch> <path> <sha>` (last one wins).
    pub report: Option<(String, String, String)>,
    pub prs: Vec<(String, u64)>,
    pub branches: Vec<(String, String)>,
    pub blobs: Vec<(String, String, String)>,
}
fn name_ok(s: &str) -> bool { !s.is_empty() && s.len() <= 100 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) && s != "." && s != ".." }
fn ref_ok(s: &str) -> bool { !s.is_empty() && s.len() <= 200 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b)) && !s.split('/').any(|p| p.is_empty() || p == "." || p == "..") && !s.starts_with('-') }
fn path_ok(s: &str) -> bool { sanitize_rel(s).is_some() }
/// A relative path with only plain components; None for anything that could escape or surprise.
pub fn sanitize_rel(p: &str) -> Option<PathBuf> {
    if p.is_empty() || p.len() > 300 || p.starts_with('/') || p.contains('\\') || p.chars().any(char::is_control) { return None; }
    let mut out = PathBuf::new();
    for seg in p.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." { return None; }
        out.push(seg);
    }
    Some(out)
}
fn is_sha(s: &str) -> bool { (7..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) }

pub fn find_pointers(text: &str) -> Pointers {
    let mut found = Pointers::default();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("REPORT:") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let [branch, path, sha] = parts[..] {
                if ref_ok(branch) && path_ok(path) && is_sha(sha) { found.report = Some((branch.into(), path.into(), sha.into())); }
            }
        }
    }
    const MARK: &str = "https://github.com/";
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        let tail = &rest[at + MARK.len()..];
        rest = tail;
        let end = tail.find(|c: char| !(c.is_ascii_alphanumeric() || "._-/".contains(c))).unwrap_or(tail.len());
        let segs: Vec<&str> = tail[..end].trim_end_matches(['.', '/']).split('/').collect();
        if segs.len() < 4 || !name_ok(segs[0]) || !name_ok(segs[1]) { continue; }
        let slug = format!("{}/{}", segs[0].to_ascii_lowercase(), segs[1].to_ascii_lowercase());
        match segs[2] {
            "pull" => if let Ok(n) = segs[3].parse::<u64>() { if !found.prs.contains(&(slug.clone(), n)) { found.prs.push((slug, n)); } },
            "tree" => { let b = segs[3..].join("/"); if ref_ok(&b) && !found.branches.contains(&(slug.clone(), b.clone())) { found.branches.push((slug, b)); } }
            "blob" if segs.len() >= 5 => {
                let (r, p) = (segs[3].to_string(), segs[4..].join("/"));
                if ref_ok(&r) && path_ok(&p) && !found.blobs.contains(&(slug.clone(), r.clone(), p.clone())) { found.blobs.push((slug, r, p)); }
            }
            _ => {}
        }
    }
    found
}

/// The first line that starts with VERDICT, PASS, PARTIAL or FAIL (markdown decoration in front is ignored).
pub fn verdict_line(text: &str) -> Option<String> {
    text.lines().map(|l| l.trim().trim_start_matches(|c: char| "*#>_` ".contains(c))).find(|l| ["VERDICT", "PASS", "PARTIAL", "FAIL"].iter().any(|k| l.starts_with(k))).map(|l| one_line(l, 160))
}

/// Every string anywhere in a JSON value (the places pointers can hide), for a fallback scan.
fn all_strings(v: &Value, out: &mut String) {
    match v {
        Value::String(s) => { out.push_str(s); out.push('\n'); }
        Value::Array(a) => a.iter().for_each(|x| all_strings(x, out)),
        Value::Object(m) => m.values().for_each(|x| all_strings(x, out)),
        _ => {}
    }
}

/// `*`/`?` glob on the whole path, or on the file name when the pattern has no `/`.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    fn go(p: &[u8], s: &[u8]) -> bool {
        match p.split_first() {
            None => s.is_empty(),
            Some((b'*', rest)) => (0..=s.len()).any(|i| go(rest, &s[i..])),
            Some((b'?', rest)) => !s.is_empty() && go(rest, &s[1..]),
            Some((c, rest)) => s.first() == Some(c) && go(rest, &s[1..]),
        }
    }
    let target = if pattern.contains('/') { path } else { path.rsplit('/').next().unwrap_or(path) };
    go(pattern.as_bytes(), target.as_bytes())
}

/// One row of the index.
#[derive(Clone, Debug)]
pub struct Row {
    pub session: String,
    pub title: String,
    pub repo: String,
    pub status: String,
    pub turns: Option<u64>,
    pub duration_ms: Option<f64>,
    pub cost: Option<f64>,
    pub verdict: String,
    pub prs: Vec<String>,
    pub files: usize,
    pub notes: Vec<String>,
}
impl Row {
    pub fn to_json(&self) -> Value {
        json!({"session": self.session, "title": self.title, "repo": self.repo, "status": self.status, "turns": self.turns, "duration_s": self.duration_ms.map(|m| (m / 100.0).round() / 10.0),
               "cost_usd": self.cost, "verdict": self.verdict, "prs": self.prs, "files": self.files, "notes": self.notes})
    }
}
fn cell(text: &str) -> String { text.replace('|', "\\|").replace('\n', " ") }
pub fn render_index(rows: &[Row]) -> String {
    let mut out = String::from("# Collected cloud sessions\n\n| session | title | repo | status | turns | duration | cost | verdict | PR | files |\n|---|---|---|---|---|---|---|---|---|---|\n");
    let mut total = 0.0;
    let mut files = 0;
    for r in rows {
        total += r.cost.unwrap_or(0.0);
        files += r.files;
        out.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n", cell(&r.session), cell(&r.title), cell(&r.repo), cell(&r.status),
            r.turns.map_or("-".into(), |t| t.to_string()), r.duration_ms.map_or("-".into(), |m| format!("{:.1}s", m / 1000.0)),
            r.cost.map_or("-".into(), |c| format!("${c:.3}")), cell(&r.verdict), if r.prs.is_empty() { "-".into() } else { r.prs.join(" ") }, r.files));
    }
    out.push_str(&format!("| **total ({} sessions)** | | | | | | **${total:.3}** | | | {files} |\n", rows.len()));
    let notes: Vec<String> = rows.iter().flat_map(|r| r.notes.iter().map(move |n| format!("- {}: {}", r.session, cell(n)))).collect();
    if !notes.is_empty() { out.push_str("\nNotes\n\n"); out.push_str(&notes.join("\n")); out.push('\n'); }
    out
}

// ---- gh (a subprocess started without a shell; GH_TOKEN comes from the caller's environment) ----
fn gh_bin() -> PathBuf { std::env::var_os("CLOUD_SESSIONS_GH").map(PathBuf::from).unwrap_or_else(|| "gh".into()) }
fn gh(args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new(gh_bin()).args(args).stdin(Stdio::null()).output().map_err(|e| format!("could not run gh: {e}"))?;
    if !out.status.success() { return Err(format!("gh {} failed: {}", args.first().unwrap_or(&""), String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").chars().take(200).collect::<String>())); }
    Ok(out.stdout)
}
fn enc_path(p: &str) -> String {
    p.split('/').map(|seg| seg.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>()).collect::<Vec<_>>().join("/")
}
/// Download `path` of `slug` at `reference` into `dest`.
fn fetch_file(slug: &str, reference: &str, path: &str, dest: &Path) -> Result<()> {
    let rel = sanitize_rel(path).ok_or("unsafe path")?;
    let endpoint = format!("repos/{slug}/contents/{}?ref={reference}", enc_path(path));
    let bytes = gh(&["api", "-H", "Accept: application/vnd.github.raw", &endpoint])?;
    let target = dest.join(rel);
    if let Some(dir) = target.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
    std::fs::write(&target, bytes).map_err(|e| e.to_string())
}

/// Files of a PR at its head sha: (head sha, paths).
fn pr_files(slug: &str, n: u64) -> Result<(String, Vec<String>)> {
    let raw = gh(&["pr", "view", &n.to_string(), "-R", slug, "--json", "headRefOid,files"])?;
    let v: Value = serde_json::from_slice(&raw).map_err(|_| "gh pr view returned no JSON".to_string())?;
    let sha = v["headRefOid"].as_str().filter(|s| is_sha(s)).ok_or("no head sha")?.to_string();
    Ok((sha, v["files"].as_array().map(|a| a.iter().filter_map(|f| f["path"].as_str().map(str::to_string)).collect()).unwrap_or_default()))
}
/// Files changed on a branch against the default branch: (head sha, paths).
fn branch_files(slug: &str, branch: &str) -> Result<(String, Vec<String>)> {
    let repo: Value = serde_json::from_slice(&gh(&["api", &format!("repos/{slug}")])?).map_err(|_| "no JSON".to_string())?;
    let default = repo["default_branch"].as_str().filter(|b| ref_ok(b)).ok_or("no default branch")?.to_string();
    let cmp: Value = serde_json::from_slice(&gh(&["api", &format!("repos/{slug}/compare/{default}...{branch}")])?).map_err(|_| "no JSON".to_string())?;
    let sha = cmp["commits"].as_array().and_then(|c| c.last()).and_then(|c| c["sha"].as_str()).filter(|s| is_sha(s)).ok_or("no head sha")?.to_string();
    Ok((sha, cmp["files"].as_array().map(|a| a.iter().filter(|f| f["status"] != "removed").filter_map(|f| f["filename"].as_str().map(str::to_string)).collect()).unwrap_or_default()))
}

fn short(id: &str) -> String {
    let rest = id.strip_prefix("session_").unwrap_or(id);
    rest.chars().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect()
}

/// Collect one session into `<out>/<short>/`.
fn collect_one(ctx: &Ctx, id: &str, tracked: (&str, &str), out: &Path, glob: Option<&str>) -> Result<Row> {
    let raw = ctx.client()?.events_raw(id)?;
    let list = cloud_sessions_core::events::event_list(&raw);
    let dir = out.join(short(id));
    std::fs::create_dir_all(dir.join("files")).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    std::fs::write(dir.join("events.json"), serde_json::to_string_pretty(&raw).unwrap_or_default()).map_err(|e| e.to_string())?;
    let mut row = Row { session: id.into(), title: tracked.0.into(), repo: tracked.1.into(), status: "no result yet".into(), turns: None, duration_ms: None, cost: None, verdict: "-".into(), prs: vec![], files: 0, notes: vec![] };
    if row.title.is_empty() {
        if let Some(e) = cloud_sessions_core::events::parse_events(&raw).into_iter().find(|e| e.user) { row.title = one_line(&e.text, 60); }
    }
    let result = parse_result(&list);
    let final_text = match &result {
        Some(r) => { row.status = r.status.clone(); row.turns = r.turns; row.duration_ms = r.duration_ms; row.cost = r.cost; r.text.clone() }
        None => { row.notes.push("the run has no result event yet".into()); String::new() }
    };
    std::fs::write(dir.join("final.md"), &final_text).map_err(|e| e.to_string())?;
    if let Some(v) = verdict_line(&final_text) { row.verdict = v; }
    // The final text first; only when it points nowhere, everything the run wrote.
    let mut pointers = find_pointers(&final_text);
    if pointers == Pointers::default() { let mut all = String::new(); all_strings(&raw, &mut all); pointers = find_pointers(&all); }
    if row.repo.is_empty() {
        if let Some(slug) = pointers.prs.first().map(|p| p.0.clone()).or_else(|| pointers.branches.first().map(|b| b.0.clone())).or_else(|| pointers.blobs.first().map(|b| b.0.clone())) { row.repo = slug; }
    }
    let wants = |p: &str| glob.map_or(true, |g| glob_match(g, p));
    let sources = pointers.prs.len() + pointers.branches.len() + pointers.blobs.len() + usize::from(pointers.report.is_some());
    let fetch = |label: &str, slug: &str, sha: &str, paths: Vec<String>, row: &mut Row| {
        let sub = if sources > 1 { dir.join("files").join(label) } else { dir.join("files") };
        for p in paths.iter().filter(|p| wants(p)) {
            match fetch_file(slug, sha, p, &sub) { Ok(()) => row.files += 1, Err(e) => row.notes.push(format!("{p}: {e}")) }
        }
    };
    if let Some((branch, path, sha)) = &pointers.report {
        if row.repo.is_empty() { row.notes.push(format!("REPORT {branch} {path} {sha}: repository unknown (track the session with --repo)")); }
        else { let repo = row.repo.clone(); fetch("report", &repo, sha, vec![path.clone()], &mut row); row.notes.push(format!("REPORT: {branch} {path} {sha}")); }
    }
    for (slug, n) in pointers.prs.iter().take(5) {
        row.prs.push(format!("https://github.com/{slug}/pull/{n}"));
        match pr_files(slug, *n) {
            Ok((sha, paths)) => { fetch(&format!("pr{n}"), slug, &sha, paths, &mut row); row.notes.push(format!("PR {slug}#{n} head {sha}")); }
            Err(e) => row.notes.push(format!("PR {slug}#{n}: {e}")),
        }
    }
    for (slug, branch) in pointers.branches.iter().take(3) {
        match branch_files(slug, branch) {
            Ok((sha, paths)) => { fetch(&format!("branch-{}", branch.replace('/', "_")), slug, &sha, paths, &mut row); row.notes.push(format!("branch {slug}:{branch} head {sha}")); }
            Err(e) => row.notes.push(format!("branch {slug}:{branch}: {e}")),
        }
    }
    for (slug, reference, path) in pointers.blobs.iter().take(10) { fetch("blobs", slug, reference, vec![path.clone()], &mut row); }
    // No verdict in the final text: the first line of a collected top-level RESULT.md may carry it.
    if row.verdict == "-" {
        let found = std::fs::read_dir(dir.join("files")).ok().into_iter().flatten().flatten().map(|e| e.path()).chain(std::fs::read_dir(dir.join("files")).ok().into_iter().flatten().flatten().flat_map(|e| std::fs::read_dir(e.path()).ok().into_iter().flatten().flatten().map(|f| f.path())));
        for path in found { if path.file_name().is_some_and(|n| n == "RESULT.md") { if let Some(v) = std::fs::read_to_string(&path).ok().as_deref().and_then(verdict_line) { row.verdict = v; break; } } }
    }
    Ok(row)
}

/// `collect <session…> | --all-tracked | --batch <name> [--out <dir>] [--files <glob>]`
pub fn collect(ctx: &Ctx, args: &Args) -> Result<Value> {
    let st = state::load(&ctx.home);
    let mut ids: Vec<String> = Vec::new();
    for p in &args.positional { ids.push(cloud_sessions_core::policy::parse_session_id(p)?); }
    if args.switch("all-tracked") { ids.extend(st.sessions_json().as_array().into_iter().flatten().filter_map(|s| s["session_id"].as_str().map(str::to_string))); }
    if let Some(name) = args.values.get("batch") {
        let batch = st.batch_ids(name);
        if batch.is_empty() { return Err(format!("No sessions are tracked under the batch {name}.")); }
        ids.extend(batch);
    }
    let mut seen = Vec::new();
    ids.retain(|i| if seen.contains(i) { false } else { seen.push(i.clone()); true });
    if ids.is_empty() { return Err("Give session links or IDs, --all-tracked, or --batch <name>.".into()); }
    let out = PathBuf::from(args.values.get("out").cloned().unwrap_or_else(|| "collect-out".into()));
    std::fs::create_dir_all(&out).map_err(|e| format!("Could not create --out: {e}"))?;
    let glob = args.values.get("files").map(String::as_str);
    let tracked = st.sessions_json();
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for id in &ids {
        let t = tracked.as_array().and_then(|a| a.iter().find(|s| s["session_id"] == id.as_str())).cloned().unwrap_or(Value::Null);
        match collect_one(ctx, id, (t["title"].as_str().unwrap_or(""), t["repo"].as_str().unwrap_or("")), &out, glob) {
            Ok(row) => rows.push(row),
            Err(e) => errors.push(json!({"session": id, "error": e})),
        }
    }
    let index = render_index(&rows);
    std::fs::write(out.join("INDEX.md"), &index).map_err(|e| e.to_string())?;
    let total: f64 = rows.iter().filter_map(|r| r.cost).sum();
    let doc = json!({"sessions": rows.iter().map(Row::to_json).collect::<Vec<_>>(), "total_cost_usd": total, "total_files": rows.iter().map(|r| r.files).sum::<usize>(), "errors": errors});
    std::fs::write(out.join("index.json"), serde_json::to_string_pretty(&doc).unwrap_or_default()).map_err(|e| e.to_string())?;
    Ok(json!({"ok": errors.is_empty(), "out": out, "index": out.join("INDEX.md"), "collected": rows.len(), "errors": errors, "total_cost_usd": total, "untrusted": crate::UNTRUSTED}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloud_sessions_core::events::event_list;

    const CLOUD: &str = include_str!("../tests/fixtures/cloud-events-session.json");
    const FINAL: &str = include_str!("../tests/fixtures/final-result.json");
    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn the_result_event_is_parsed_from_a_real_session() {
        let list = event_list(&serde_json::from_str(CLOUD).unwrap());
        let r = parse_result(&list).unwrap();
        assert_eq!((r.status.as_str(), r.turns, r.duration_ms, r.cost), ("success", Some(3), Some(27254.0), Some(0.1553181)));
        assert!(r.text.starts_with("**Step 1 facts**"));
        assert!(verdict_line(&r.text).is_none() || verdict_line(&r.text).unwrap().starts_with("VERDICT"));
        assert!(parse_result(&list[..list.len() - 1]).is_none(), "no result event while it runs");
    }
    #[test]
    fn the_final_text_names_its_pull_request_and_a_verdict() {
        let list = event_list(&serde_json::from_str(FINAL).unwrap());
        let r = parse_result(&list).unwrap();
        assert_eq!((r.status.as_str(), r.turns, r.duration_ms, r.cost), ("success", Some(12), Some(120000.0), Some(1.25)));
        let p = find_pointers(&r.text);
        assert!(p.prs.contains(&("your-org/example-repo".to_string(), 1)), "{:?}", p);
        assert!(p.report.is_none());
    }
    #[test]
    fn pointers_come_from_strict_matchers_only() {
        let text = format!("done\nREPORT: rl/a1 reports/a1.md {SHA}\nsee https://github.com/Acme/Widgets/pull/12, and https://github.com/acme/widgets/tree/rl/a1 plus https://github.com/acme/widgets/blob/{SHA}/reports/a1.md.\nhttps://github.com/acme/widgets/pull/12 again https://evil.example/https://github.com/x/y/pull/9");
        let p = find_pointers(&text);
        assert_eq!(p.report, Some(("rl/a1".into(), "reports/a1.md".into(), SHA.into())));
        assert_eq!(p.prs, vec![("acme/widgets".to_string(), 12), ("x/y".to_string(), 9)], "deduplicated; a github link inside another URL is still only a link, never run");
        assert_eq!(p.branches, vec![("acme/widgets".to_string(), "rl/a1".to_string())]);
        assert_eq!(p.blobs, vec![("acme/widgets".to_string(), SHA.to_string(), "reports/a1.md".to_string())]);
        // Hostile or malformed REPORT lines are ignored.
        for bad in ["REPORT: rl/a ../../etc/passwd abcdef1", "REPORT: rl/a /abs/path abcdef1", "REPORT: -x reports/a.md abcdef1", "REPORT: rl/a reports/a.md nothex!", "REPORT: rl/a reports/a.md", "REPORT: a b c d"] {
            assert!(find_pointers(bad).report.is_none(), "{bad}");
        }
        assert!(sanitize_rel("a/../b").is_none() && sanitize_rel("a//b").is_none() && sanitize_rel("a/b.md").is_some());
    }
    #[test]
    fn verdict_is_the_first_matching_line_and_globs_work() {
        assert_eq!(verdict_line("intro\n**VERDICT: PASS** all good\nFAIL later").as_deref(), Some("VERDICT: PASS** all good"));
        assert_eq!(verdict_line("nothing here"), None);
        assert!(glob_match("*.md", "docs/a.md") && !glob_match("*.md", "a.rs") && glob_match("docs/*", "docs/x/y.md") && glob_match("a?.md", "a1.md"));
    }
    #[test]
    fn the_index_has_a_row_per_session_and_a_total() {
        let row = |s: &str, cost| Row { session: s.into(), title: "T | x".into(), repo: "o/r".into(), status: "success".into(), turns: Some(34), duration_ms: Some(448600.0), cost: Some(cost), verdict: "PASS".into(), prs: vec!["https://github.com/o/r/pull/1".into()], files: 8, notes: vec![] };
        let md = render_index(&[row("session_A", 4.664), row("session_B", 0.155)]);
        assert!(md.contains("| session_A | T \\| x | o/r | success | 34 | 448.6s | $4.664 | PASS | https://github.com/o/r/pull/1 | 8 |"), "{md}");
        assert!(md.contains("| **total (2 sessions)** | | | | | | **$4.819** | | | 16 |"), "{md}");
    }
}
