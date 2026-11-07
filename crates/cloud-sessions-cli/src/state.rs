//! Small persisted state (`state.json` in the config folder): send history, today's starts, and the
//! sessions this tool started (so a send runs from the right checkout). No secrets.
use crate::Result;
use cloud_sessions_core::policy;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Default, Debug)]
pub struct State(Value);

pub fn load(home: &Path) -> State {
    State(std::fs::read_to_string(home.join("state.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).filter(Value::is_object).unwrap_or_else(|| json!({})))
}
pub fn save(home: &Path, state: &State) -> Result<()> {
    std::fs::create_dir_all(home).map_err(|e| format!("Could not create the config folder: {e}"))?;
    std::fs::write(home.join("state.json"), serde_json::to_string_pretty(&state.0).unwrap_or_default()).map_err(|e| format!("Could not save state: {e}"))
}

impl State {
    fn day(now: u64) -> u64 { now / 86_400 }
    pub fn check_start_cap(&self, now: u64, cap: u64) -> Result<()> {
        let today = self.0["starts"]["day"].as_u64() == Some(Self::day(now));
        let used = if today { self.0["starts"]["count"].as_u64().unwrap_or(0) } else { 0 };
        if used >= cap { Err(format!("The daily cap of {cap} cloud sessions is reached (CLOUD_SESSIONS_DAILY_CAP). Try again tomorrow.")) } else { Ok(()) }
    }
    pub fn starts_left(&self, now: u64, cap: u64) -> u64 {
        let used = if self.0["starts"]["day"].as_u64() == Some(Self::day(now)) { self.0["starts"]["count"].as_u64().unwrap_or(0) } else { 0 };
        cap.saturating_sub(used)
    }
    /// Sessions started together under one name (`start --batch`), in start order.
    pub fn add_to_batch(&mut self, name: &str, id: &str) {
        if !self.0["batches"][name].is_array() { self.0["batches"][name] = json!([]); }
        if let Some(list) = self.0["batches"][name].as_array_mut() { if !list.iter().any(|v| v == id) { list.push(json!(id)); } }
    }
    pub fn batch_ids(&self, name: &str) -> Vec<String> {
        self.0["batches"][name].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default()
    }
    pub fn record_start(&mut self, now: u64, id: &str, dir: &Path, repo: &str, title: &str, url: &str) {
        let today = self.0["starts"]["day"].as_u64() == Some(Self::day(now));
        let used = if today { self.0["starts"]["count"].as_u64().unwrap_or(0) } else { 0 };
        self.0["starts"] = json!({"day": Self::day(now), "count": used + 1});
        self.0["sessions"][id] = json!({"dir": dir.to_string_lossy(), "repo": repo, "title": title, "url": url, "started_at": now});
    }
    /// Add a session by id (no checkout known). A title given later replaces the old one; the checkout is kept.
    /// True when the session was not tracked before.
    pub fn track(&mut self, id: &str, title: Option<&str>, repo: Option<&str>, url: &str, now: u64) -> bool {
        let fresh = self.0["sessions"][id].is_null();
        if fresh { self.0["sessions"][id] = json!({"title": "", "url": url, "repo": null, "started_at": now}); }
        if let Some(title) = title { self.0["sessions"][id]["title"] = json!(title); }
        // A repo given later replaces an unknown one (sessions imported from a file have none).
        if let Some(repo) = repo { self.0["sessions"][id]["repo"] = json!(repo); }
        fresh
    }
    pub fn untrack(&mut self, id: &str) -> bool {
        self.0["sessions"].as_object_mut().is_some_and(|m| m.remove(id).is_some())
    }
    pub fn admit_send(&mut self, id: &str, now: u64) -> Result<()> {
        let mut history: Vec<u64> = self.0["sends"][id].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
        policy::admit_send(&mut history, now)?;
        self.0["sends"][id] = json!(history);
        Ok(())
    }
    pub fn dir_of(&self, id: &str) -> Option<PathBuf> { self.0["sessions"][id]["dir"].as_str().map(PathBuf::from) }
    /// The newest event a journal run already wrote for this session (UTC timestamp text).
    pub fn mark_seen(&mut self, id: &str, seen: &str) {
        if self.0["sessions"][id].is_null() { self.0["sessions"][id] = json!({"title": "", "url": policy::session_url(id), "repo": null, "started_at": 0}); }
        self.0["sessions"][id]["last_seen"] = json!(seen);
    }
    pub fn sessions_json(&self) -> Value {
        let list: Vec<Value> = self.0["sessions"].as_object().map(|m| m.iter().map(|(id, v)| json!({"session_id": id, "title": v["title"], "url": v["url"], "repo": v["repo"], "started_at": v["started_at"], "last_seen": v["last_seen"]})).collect()).unwrap_or_default();
        json!(list)
    }
}
