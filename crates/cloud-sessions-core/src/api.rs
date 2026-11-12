//! Reading sessions over HTTPS with a bearer token the caller supplies (this crate stores no secrets).
//!
//! OFFICIAL-ISH: `events` (`GET /v1/code/sessions/<id>/events`), as used by Claude Code's own client.
//! UNOFFICIAL, opt-in: `list` (`GET /v1/code/sessions`) and `stream` (`GET …/events/stream`, server-sent
//! events). Both may change or vanish; callers must fall back to `events` on any error.
use crate::events::{self, Event};
use crate::policy::parse_session_id;
use crate::Result;
use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::time::Duration;

pub const API: &str = "https://api.anthropic.com";
/// The `anthropic-beta` feature the routines routes require (the routes answer 404 without it).
pub const TRIGGERS_BETA: &str = "ccr-triggers-2026-01-30";
const LIMIT: u64 = 16 * 1024 * 1024;
const MAX_FRAME: usize = 4 * 1024 * 1024;
const SERVICE: &str = "Claude cloud sessions";

/// New events since a cursor, plus where the cursor now is.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Page {
    pub events: Vec<Event>,
    /// Id of the newest event seen; pass it back as `after`.
    pub cursor: Option<String>,
    /// The log ends with a `result` event.
    pub finished: bool,
}

/// Safety caps for following pages.
#[derive(Clone, Copy, Debug)]
pub struct PageCaps {
    pub max_pages: usize,
    pub max_rows: usize,
}
impl Default for PageCaps {
    fn default() -> Self { Self { max_pages: 50, max_rows: 1000 } }
}
/// The rows of all pages read, how many pages, and whether the list is complete.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pages {
    pub data: Vec<Value>,
    pub pages: usize,
    /// A cap was reached (or the API said there is more but sent no cursor).
    pub truncated: bool,
    /// The cursor of the first page not read when a cap was reached.
    pub next_cursor: Option<String>,
    /// A later page failed; `data` holds the pages before it.
    pub partial: Option<String>,
}

/// Percent-encode a cursor for a query string (unreserved characters stay).
pub fn query_escape(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

pub struct Client {
    base: String,
    token: String,
    beta: Option<String>,
}

fn loopback(base: &str) -> bool {
    ["http://127.0.0.1", "http://localhost", "http://[::1]"].iter().any(|p| base.strip_prefix(p).is_some_and(|rest| rest.is_empty() || rest.starts_with(':') || rest.starts_with('/')))
}

/// The service's own message from an error body, else the status. Credentials never appear here.
pub fn error_message(status: u16, body: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let message = parsed.as_ref().and_then(|v| v["message"].as_str().or(v["error"]["message"].as_str()).or(v["error"].as_str())).map(str::to_string)
        .or_else(|| Some(body.trim().chars().take(300).collect::<String>()).filter(|s| !s.is_empty() && !s.starts_with('<')));
    match (status, message) {
        (401 | 403, _) => format!("{SERVICE} refused the credentials (HTTP {status}). A token from `claude setup-token` is inference-only and cannot read sessions; Claude Code's own login can (inside Claude Code use its RemoteTrigger get_run_log, or opt in to reusing the claude login)."),
        (_, Some(message)) => format!("{SERVICE}: {message}"),
        _ => format!("{SERVICE} returned HTTP {status}."),
    }
}

impl Client {
    /// `base` is HTTPS (default `https://api.anthropic.com`); plain HTTP is accepted only for a loopback test server.
    /// `beta` is sent as `anthropic-beta`; `sk-ant-oat…` tokens (from `claude setup-token`) default to `oauth-2025-04-20`.
    pub fn new(base: Option<&str>, token: &str, beta: Option<&str>) -> Result<Self> {
        let base = base.unwrap_or(API).trim_end_matches('/').to_string();
        if !(base.starts_with("https://") || loopback(&base)) { return Err("The API address must be https.".into()); }
        let token = token.trim().to_string();
        if token.is_empty() || token.chars().any(char::is_control) { return Err("The token is empty or malformed.".into()); }
        let beta = beta.filter(|b| !b.is_empty()).map(str::to_string).or_else(|| token.starts_with("sk-ant-oat").then(|| "oauth-2025-04-20".to_string()));
        Ok(Self { base, token, beta })
    }

    fn agent(&self, read: Duration, total: Option<Duration>) -> ureq::Agent {
        let mut builder = ureq::AgentBuilder::new().redirects(0).timeout_connect(Duration::from_secs(10)).timeout_read(read).user_agent(concat!("cloud-sessions-core/", env!("CARGO_PKG_VERSION")));
        if let Some(total) = total { builder = builder.timeout(total); }
        if !loopback(&self.base) { builder = builder.try_proxy_from_env(true); }
        builder.build()
    }
    fn request(&self, agent: &ureq::Agent, path: &str) -> ureq::Request {
        let mut request = agent.get(&format!("{}{path}", self.base)).set("Authorization", &format!("Bearer {}", self.token)).set("anthropic-version", "2023-06-01");
        if let Some(beta) = &self.beta { request = request.set("anthropic-beta", beta); }
        request
    }
    fn failure(error: ureq::Error) -> String {
        match error {
            ureq::Error::Status(code, response) => error_message(code, &response.into_string().unwrap_or_default()),
            ureq::Error::Transport(t) if matches!(t.kind(), ureq::ErrorKind::Dns | ureq::ErrorKind::ConnectionFailed) => format!("Could not reach {SERVICE}. Check your connection."),
            ureq::Error::Transport(_) => format!("{SERVICE} request failed."),
        }
    }
    fn get_json(&self, path: &str) -> Result<Value> {
        self.get_json_beta(path, None)
    }
    /// Like `get_json`, with an extra `anthropic-beta` feature appended (the routines routes answer 404 without theirs).
    fn get_json_beta(&self, path: &str, extra_beta: Option<&str>) -> Result<Value> {
        let mut request = self.request(&self.agent(Duration::from_secs(60), Some(Duration::from_secs(60))), path);
        if let Some(extra) = extra_beta {
            let beta = match &self.beta { Some(b) => format!("{b},{extra}"), None => extra.to_string() };
            request = request.set("anthropic-beta", &beta);
        }
        let response = request.call().map_err(Self::failure)?;
        let mut body = String::new();
        response.into_reader().take(LIMIT + 1).read_to_string(&mut body).map_err(|_| format!("{SERVICE} response was cut off."))?;
        if body.len() as u64 > LIMIT { return Err(format!("{SERVICE} sent a response that is too large.")); }
        if body.trim().is_empty() { return Ok(Value::Null); }
        serde_json::from_str(&body).map_err(|_| format!("{SERVICE} returned an unreadable response."))
    }

    /// The whole condensed event log, raw.
    pub fn events_raw(&self, session_id: &str) -> Result<Value> {
        self.get_json(&format!("/v1/code/sessions/{}/events", parse_session_id(session_id)?))
    }
    /// Events after `after` (an event id from an earlier page; unknown or absent → the whole log). The cursor is
    /// client-side because a server-side one is unconfirmed.
    pub fn events(&self, session_id: &str, after: Option<&str>) -> Result<Page> {
        let body = self.events_raw(session_id)?;
        let mut entries = events::parse_events(&body);
        if let Some(position) = after.and_then(|a| entries.iter().position(|e| e.id == a)) { entries.drain(..=position); }
        let cursor = entries.last().map(|e| e.id.clone()).or_else(|| after.map(str::to_string));
        Ok(Page { events: entries, cursor, finished: events::finished(&body) })
    }

    /// UNOFFICIAL: `GET /v1/code/sessions` — the account's sessions, raw JSON.
    pub fn list(&self) -> Result<Value> {
        self.get_json("/v1/code/sessions")
    }

    /// UNOFFICIAL: every page of `GET /v1/code/sessions`. The API sends 20 rows a page and `next_cursor`; the next
    /// page is `?cursor=<next_cursor>`. Stops at an empty cursor or at the caps (then `truncated`). An error on page 1
    /// is the error; an error on a later page keeps the rows read so far and says so in `partial`.
    pub fn list_all(&self, caps: PageCaps) -> Result<Pages> {
        self.pages("/v1/code/sessions", None, caps)
    }

    /// Follow `next_cursor` (`?cursor=`) from `path` until it ends or the caps are reached.
    fn pages(&self, path: &str, beta: Option<&str>, caps: PageCaps) -> Result<Pages> {
        let mut out = Pages::default();
        let mut cursor: Option<String> = None;
        loop {
            let url = match &cursor { Some(c) => format!("{path}{}cursor={}", if path.contains('?') { '&' } else { '?' }, query_escape(c)), None => path.to_string() };
            let body = match self.get_json_beta(&url, beta) {
                Ok(body) => body,
                Err(e) if out.pages == 0 => return Err(e),
                Err(e) => { out.partial = Some(format!("Only the first {} pages ({} rows) could be read; page {} failed: {}", out.pages, out.data.len(), out.pages + 1, e)); return Ok(out); }
            };
            out.pages += 1;
            out.data.extend(body["data"].as_array().cloned().unwrap_or_default());
            let next = body["next_cursor"].as_str().map(str::trim).filter(|c| !c.is_empty()).map(str::to_string);
            let more = next.is_some() || body["has_more"] == true;
            match next {
                Some(_) if out.pages >= caps.max_pages || out.data.len() >= caps.max_rows => { out.truncated = true; out.next_cursor = next; return Ok(out); }
                Some(n) if Some(&n) == cursor.as_ref() => { out.partial = Some("The API sent the same cursor twice; stopped there.".into()); return Ok(out); }
                Some(n) => cursor = Some(n),
                None => { out.truncated = more; return Ok(out); }
            }
        }
    }

    /// UNOFFICIAL (s299): `GET /v1/code/triggers` — the account's routines (scheduled or one-shot jobs), raw JSON.
    /// Routine runs are sessions that the main `list` does NOT include; see `trigger_runs`.
    pub fn triggers(&self) -> Result<Value> {
        self.get_json_beta("/v1/code/triggers", Some(TRIGGERS_BETA))
    }
    /// Every page of the routines (the API sends `has_more`; it follows `next_cursor` when one is sent).
    pub fn triggers_all(&self, caps: PageCaps) -> Result<Pages> {
        self.pages("/v1/code/triggers", Some(TRIGGERS_BETA), caps)
    }
    /// UNOFFICIAL (s299): `GET /v1/code/sessions?trigger_id=<id>` — the runs of one routine, raw JSON (first page,
    /// same session shape as `list`). The id is validated so nothing but an id reaches the query string.
    pub fn trigger_runs(&self, trigger_id: &str) -> Result<Value> {
        let id = trigger_id.trim();
        if id.is_empty() || id.len() > 128 || !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')) {
            return Err("The routine id is malformed.".into());
        }
        self.get_json_beta(&format!("/v1/code/sessions?trigger_id={id}"), Some(TRIGGERS_BETA))
    }

    /// UNOFFICIAL: `GET /v1/code/sessions/<id>/events/stream` (server-sent events). Calls `on_event` for each event
    /// object (arrays are flattened) and returns when the stream ends or fails; the caller then falls back to
    /// `events`. A stream silent for `idle` is treated as dead.
    pub fn stream(&self, session_id: &str, idle: Duration, mut on_event: impl FnMut(Value) -> Result<()>) -> Result<()> {
        let id = parse_session_id(session_id)?;
        let response = self.request(&self.agent(idle, None), &format!("/v1/code/sessions/{id}/events/stream")).set("accept", "text/event-stream").call().map_err(Self::failure)?;
        let mut reader = BufReader::new(response.into_reader());
        let (mut data, mut size) = (String::new(), 0usize);
        loop {
            let mut line = String::new();
            let read = reader.read_line(&mut line).map_err(|e| if e.kind() == std::io::ErrorKind::TimedOut || e.kind() == std::io::ErrorKind::WouldBlock { "The event stream went quiet.".to_string() } else { "The event stream was cut off.".to_string() })?;
            if read == 0 { return Ok(()); }
            size += read;
            if size > MAX_FRAME { return Err("The event stream sent an oversized frame.".into()); }
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if !data.is_empty() {
                    if let Ok(value) = serde_json::from_str::<Value>(&data) {
                        match value { Value::Array(list) => for item in list { on_event(item)?; }, other => on_event(other)? }
                    }
                }
                data.clear(); size = 0;
            } else if let Some(rest) = line.strip_prefix("data:") {
                if !data.is_empty() { data.push('\n'); }
                data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod mock {
    //! A loopback HTTP server that answers from a routing function and records (path, headers).
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    pub type Seen = Arc<Mutex<Vec<(String, Vec<(String, String)>)>>>;
    pub fn serve(route: impl Fn(&str) -> (u16, String) + Send + 'static) -> (String, Seen) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen: Seen = Arc::default();
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line.is_empty() { continue; }
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let mut headers = Vec::new();
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end();
                    if header.is_empty() { break; }
                    if let Some((n, v)) = header.split_once(':') { headers.push((n.trim().to_ascii_lowercase(), v.trim().to_string())); }
                }
                let (status, body) = route(&path);
                log.lock().unwrap().push((path, headers));
                let _ = write!(stream, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        (base, seen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::tests::sample;
    use serde_json::json;

    const ID: &str = "session_01Mock1234";

    fn server() -> (String, mock::Seen) {
        mock::serve(|path| match path {
            "/v1/code/sessions/session_01Mock1234/events" => (200, sample().to_string()),
            "/v1/code/sessions/session_01Mock1234/events/stream" => (200, "id: 1\ndata: {\"uuid\":\"s1\",\"type\":\"user\",\"message\":{\"content\":\"hello\"}}\n\n: ping\n\nevent: event\ndata: [{\"uuid\":\"s2\",\"type\":\"assistant\",\"message\":{\"content\":\"hi\"}}]\n\ndata: not json\n\n".into()),
            "/v1/code/sessions" => (200, json!({"data": [{"id": ID, "title": "Fix flaky"}]}).to_string()),
            "/v1/code/triggers" => (200, json!({"data": [{"id": "trig_1", "name": "Nightly"}]}).to_string()),
            "/v1/code/sessions?trigger_id=trig_1" => (200, json!({"data": [{"id": "cse_01Run", "title": "Nightly run"}]}).to_string()),
            _ => (503, r#"{"error":{"message":"down"}}"#.into()),
        })
    }

    #[test]
    fn events_carry_the_token_and_the_cursor_skips_what_was_seen() {
        let (base, seen) = server();
        let client = Client::new(Some(&base), "sk-ant-oat-secret", None).unwrap();
        let all = client.events(ID, None).unwrap();
        assert_eq!((all.events.len(), all.cursor.as_deref(), all.finished), (4, Some("e4"), true));
        let none = client.events(ID, Some("e4")).unwrap();
        assert_eq!((none.events.len(), none.cursor.as_deref()), (0, Some("e4")));
        assert_eq!(client.events(ID, Some("e2")).unwrap().events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["e3", "e4"]);
        assert!(client.events(ID, Some("unknown")).unwrap().events.len() == 4, "an unknown cursor reads the whole log");
        for (_, headers) in seen.lock().unwrap().iter() {
            let get = |n: &str| headers.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
            assert_eq!(get("authorization"), Some("Bearer sk-ant-oat-secret"));
            assert_eq!(get("anthropic-beta"), Some("oauth-2025-04-20"));
            assert!(get("cookie").is_none());
        }
        let err = client.events("session_01Other999", None).unwrap_err();
        assert!(err.contains("down") && !err.contains("secret"));
    }

    fn paged(fail_page: Option<usize>) -> (String, mock::Seen) {
        // Page n (1-based) has 2 rows; page n points to cursor "c/n+1=" until page 3. The cursor needs escaping.
        mock::serve(move |path| {
            let n = match path { "/v1/code/sessions" => 1, p if p.starts_with("/v1/code/sessions?cursor=c%2F") => p.trim_start_matches("/v1/code/sessions?cursor=c%2F").trim_end_matches("%3D").parse().unwrap_or(99), _ => 99 };
            if Some(n) == fail_page || n == 99 { return (503, r#"{"error":{"message":"page down"}}"#.into()); }
            let next = if n < 3 { json!(format!("c/{}=", n + 1)) } else { json!(null) };
            (200, json!({"data": [{"id": format!("session_01P{n}a")}, {"id": format!("session_01P{n}b")}], "next_cursor": next, "resume_token": "r"}).to_string())
        })
    }

    #[test]
    fn list_all_follows_the_cursor_chain_with_caps_and_keeps_pages_before_an_error() {
        let (base, seen) = paged(None);
        let client = Client::new(Some(&base), "tok", None).unwrap();
        let all = client.list_all(PageCaps::default()).unwrap();
        assert_eq!((all.pages, all.data.len(), all.truncated, all.partial.clone()), (3, 6, false, None));
        assert_eq!(seen.lock().unwrap().iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(), ["/v1/code/sessions", "/v1/code/sessions?cursor=c%2F2%3D", "/v1/code/sessions?cursor=c%2F3%3D"]);
        let capped = client.list_all(PageCaps { max_pages: 2, max_rows: 1000 }).unwrap();
        assert_eq!((capped.pages, capped.data.len(), capped.truncated, capped.next_cursor.as_deref()), (2, 4, true, Some("c/3=")));
        let rows = client.list_all(PageCaps { max_pages: 50, max_rows: 2 }).unwrap();
        assert_eq!((rows.pages, rows.truncated), (1, true));
        let (base, _) = paged(Some(2));
        let part = Client::new(Some(&base), "tok", None).unwrap().list_all(PageCaps::default()).unwrap();
        assert_eq!((part.pages, part.data.len(), part.truncated), (1, 2, false));
        assert!(part.partial.unwrap().contains("page 2 failed") );
        let (base, _) = paged(Some(1));
        assert!(Client::new(Some(&base), "tok", None).unwrap().list_all(PageCaps::default()).unwrap_err().contains("page down"), "page 1 failing is the error");
        assert_eq!(query_escape("a+b/c=d_~"), "a%2Bb%2Fc%3Dd_~");
    }

    #[test]
    fn plain_http_is_refused_unless_it_is_loopback() {
        assert!(Client::new(Some("http://example.com"), "t", None).is_err());
        assert!(Client::new(Some("http://localhost.evil.example"), "t", None).is_err());
        assert!(Client::new(Some("http://127.0.0.1:9"), "t", None).is_ok());
        assert!(Client::new(None, "  ", None).is_err());
    }

    #[test]
    fn list_and_stream_are_opt_in_calls_and_stream_failures_surface_for_fallback() {
        let (base, _) = server();
        let client = Client::new(Some(&base), "tok", None).unwrap();
        assert_eq!(client.list().unwrap()["data"][0]["id"], ID);
        assert_eq!(client.triggers().unwrap()["data"][0]["name"], "Nightly");
        assert_eq!(client.trigger_runs("trig_1").unwrap()["data"][0]["id"], "cse_01Run");
        assert!(client.trigger_runs("x/../y").is_err() && client.trigger_runs("").is_err());
        let (base, seen) = server();
        let client = Client::new(Some(&base), "sk-ant-oat-x", None).unwrap();
        client.triggers().unwrap();
        let beta = seen.lock().unwrap().last().unwrap().1.iter().find(|(k, _)| k == "anthropic-beta").map(|(_, v)| v.clone()).unwrap();
        assert_eq!(beta, "oauth-2025-04-20,ccr-triggers-2026-01-30", "the routines beta rides with the token's own");
        let mut ids = Vec::new();
        client.stream(ID, Duration::from_secs(5), |value| { ids.push(crate::events::parse_event(&value).unwrap().id); Ok(()) }).unwrap();
        assert_eq!(ids, ["s1", "s2"], "comments, arrays and malformed frames are handled");
        assert!(client.stream("session_01Other999", Duration::from_secs(5), |_| Ok(())).unwrap_err().contains("down"));
        assert!(client.stream(ID, Duration::from_secs(5), |_| Err("stop".into())).unwrap_err() == "stop");
    }
}
