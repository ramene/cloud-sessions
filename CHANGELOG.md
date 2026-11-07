# Changelog: cloud-sessions mod

The Cloud sessions mod (`cloud-sessions-core`, the `cloud-sessions` CLI and the Claude Code plugin) is versioned
together and released on tags named `cloud-sessions-vX.Y.Z`.

## 0.2.1

privacy: examples, fixtures and defaults no longer name the authors' machines, sessions or private repositories.

## 0.2.0

Since 0.1.1: new verbs (routines, cast, collect, start --batch), a cloud-event decoder that fixes "· event" output, an asciicast and MP4 export with an optional end card, one canonical id per session, and a detailed README. Prebuilt archives now ship for macOS (universal), Linux x86_64 and Linux arm64.

- New: collect (a session result plus its PR, branch and REPORT files, with an INDEX) and start --batch; sessions have one canonical id (bare ids and routines-API cse_ ids map to session_<id>).
- New: cast output can end with a styleable 5 s end card (tool, cast sha256, sponsor box, CTA) in exported MP4s.

- New (s299): `routines [--runs N]` lists the account's routines (`GET /v1/code/triggers`, unofficial) with each one's newest N runs
  (`GET /v1/code/sessions?trigger_id=`), as `{ok, unofficial, routines:[{id,name,enabled,schedule:{kind,cron,run_once_at},next_run_at,last_fired_at,url,runs:[{id,title,status,worker_status,created_at,last_event_at,url}]}]}`;
  a routine whose runs cannot be read gets `runs_error` instead of failing the call. MCP tool `cloud_routines`.
- `list --unofficial` now says `routine_runs_hidden: true`: runs started by a routine are not in the main sessions list.

- Fix: events from cloud sessions (envelope with event_type and payload) were all shown as "· event". The decoder now reads the envelope: user prompt, assistant text, tool_use (name and compact input), tool_result (first line), result (status, turns, duration, cost), provisioning (env[level]) and system notices; unknown types keep a "· <type>" label. Events are put in order by sequence_num (the API returns newest first), so a finished run is recognized.
- New: cast <session> [--out <file>] [--idle-max <s>] [--mp4 <file>] writes the session as an asciicast v2 recording (the events own timestamps, idle gaps capped, default 2 s) for asciinema play. Read-only on the session. --mp4 renders it with cast2mp4 (agg and ffmpeg) where they exist.
- New: events --raw prints the unparsed API body (untrusted data).

## 0.1.1

- **Stop** (`cloud-sessions stop <id>`, `/cloud-stop`): stops a Remote Control session running on this machine with one
  SIGINT to its local worker (`claude --print --sdk-url …/sessions/cse_<id>`). The turn is interrupted, the
  worker exits, the server logs `Session completed` and the session is **archived** (later sends are refused); the
  result says so and gives the session's page to unarchive it. A cloud session has no local worker: `stop` returns its
  page, where the Stop button is the only interrupt. From another machine, run it on the host (e.g. over ssh).
- **Ids**: every form of one session (claude.ai `session_<x>`, API `cse_<x>`, a bare id, a URL) maps to the same id.
- **Track**: `track --repo`, titles and repos kept; `mark` stores the newest event already journaled (`last_seen`).
- **/cloud-journal**: reads only events newer than `last_seen`, runs readers on Sonnet, and can run a user-configured
  sync command afterwards (`CLOUD_SESSIONS_JOURNAL_SYNC`).
- **Launcher**: prefers an installed `cloud-sessions` on PATH over a download; the plugin-root fallback picks the newest
  install; the MCP server loads by default through the launcher.

## 0.1.0

First release. MIT licensed (`LICENSE` in the plugin and in each crate). **No prerequisites beyond Claude Code**: install is
`claude plugin marketplace add ramene/cloud-sessions` then `claude plugin install cloud-sessions@cloud-sessions`.

- **Core** (`cloud-sessions-core`, no Tauri): start a session with `claude --cloud` in a pseudo-terminal (the
  trust prompt is surfaced, never answered); send with `claude -p … --cloud <id> --output-format json`; read
  events from `GET /v1/code/sessions/<id>/events` with a client-side cursor; repository allowlist, session-id
  validation and a send rate limit. Opt-in, unofficial: `list` (`GET /v1/code/sessions`) and the SSE `stream`
  (`…/events/stream`), both falling back to polling or erroring cleanly.
- **CLI** (`cloud-sessions start|send|events|watch|list|auth|mcp`): JSON output, allowlist only from `CLOUD_SESSIONS_REPOS` or the config file, persisted
  daily start cap and per-session send limit, MCP server over stdio. Standalone reading uses a token from the
  keychain (macOS, Windows) or a private 0600 file (`auth set [--file]`; the file is the only store on Linux, so the Linux binary needs no libdbus, and it works where an ssh login keychain is locked), or,
  only after a terminal-only opt-in (`auth use-claude-login`), Claude Code's own login credential (read at use,
  never printed or stored).
- **Plugin** (`cloud-sessions`): `/cloud-start`, `/cloud-allow`, `/cloud-track`, `/cloud-send`, `/cloud-read`, `/cloud-watch`, `/cloud-list`, a
  skill with the rules, and MCP configuration. The plugin's launcher fetches the prebuilt `cloud-sessions` program on first use (bundled copy, else a download for macOS universal / Linux x86_64 / Linux arm64 whose SHA-256 must match the published checksum; one plain-sentence error otherwise); only the optional commands (`/cloud-start`, `/cloud-allow`, `/cloud-track`, `/cloud-list`) need it. `/cloud-send` uses the user's own `claude -p … --cloud`. Reading (`/cloud-read`, `/cloud-watch`) uses Claude Code's built-in
  `RemoteTrigger` tool (`get_run_log`), which works with your own login and needs no token. `/cloud-start` and `/cloud-send` cannot be invoked by the model.
- **Track**: `cloud-sessions track <link|id> [--title t]`, `track --from-file <path>` (every `https://claude.ai/code/session_…` link, deduplicated and validated) and `untrack`, as CLI commands, MCP tools and `/cloud-track`; `/cloud-list` shows titles and links.
- **Release tooling**: `cloud/release/build-all.sh` builds every target the machine can and writes `SHA256SUMS`; `cloud/release/RELEASE.md` describes the 0.1.0 tag (not pushed).
- **Known limits**: a `claude
  setup-token` token is inference-only and is refused by the events endpoint (HTTP 401), so it cannot be used to read;
  standalone reading with Claude Code's own login is covered by fixtures only; the
  unofficial endpoints may change without notice.
