# cloud-sessions

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) [![Release](https://img.shields.io/github/v/release/ramene/cloud-sessions?sort=semver&label=release)](https://github.com/ramene/cloud-sessions/releases) [![Homebrew](https://img.shields.io/badge/homebrew-ramene%2Ftap-orange?logo=homebrew)](https://github.com/ramene/homebrew-tap) [![Claude Code plugin](https://img.shields.io/badge/Claude%20Code-plugin-d97757)](#install) [![Rust](https://img.shields.io/badge/rust-2021-dea584?logo=rust)](Cargo.toml) [![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux-lightgrey)](#install)

[![M8ven Score](https://m8ven.ai/badge/mcp/ramene-cloud-sessions-ao35me?v=d270abd3597037faacf1ce48dd035fff)](https://m8ven.ai/mcp/ramene-cloud-sessions-ao35me?s=readme)

[English](README.md) · [中文 (README.zh-CN.md)](README.zh-CN.md)

## Getting started

![Install flow: brew, binaries on PATH, claude plugin, then the control room](docs/images/install-flow.svg)

Install with `brew install ramene/tap/cloud-sessions`, add the plugin, log in, and start your first cloud session. The [getting-started guide](docs/getting-started.md) shows each step with a picture.

## Install

Two parts. The plugin is enough for read, watch, send and track. The program is needed for start, list, collect, cast, routines, stop and the MCP server.

**1. The plugin** (needs only Claude Code):

```bash
claude plugin marketplace add ramene/cloud-sessions
claude plugin install cloud-sessions@cloud-sessions
```

To try it from a checkout instead: `claude plugin marketplace add ./` or `claude --plugin-dir ./claude-mod/cloud-sessions`.

**2. The program.** The plugin's launcher (`claude-mod/cloud-sessions/scripts/cloud-sessions`) uses, in order: `$CLOUD_SESSIONS_BIN`, a copy bundled in the plugin, an earlier download, an installed `cloud-sessions` on `PATH`, or a download of the release for the plugin's version (macOS universal, Linux x86_64, Linux arm64) whose SHA-256 must match the published checksum. If it cannot, it prints one sentence of JSON and installs nothing.

To build it yourself (Rust toolchain; the Linux build needs no libdbus and no OpenSSL):

```bash
git clone https://github.com/ramene/cloud-sessions
cd cloud-sessions
cargo build --release --locked        # target/release/cloud-sessions
cargo test --workspace --locked
```

Release archives and `SHA256SUMS` are built by `cloud/release/build-all.sh`; see `cloud/release/RELEASE.md`. Or with Homebrew: `brew install ramene/tap/cloud-sessions`. `collect` also needs the GitHub CLI (`gh`) on the path.

Before the first `start`: allow the repository once, and run `claude` once in that folder to accept its trust prompt. The mod never answers that prompt.

```bash
cloud-sessions allow --dir ~/code/my-repo        # or /cloud-allow in Claude Code, or CLOUD_SESSIONS_REPOS=owner/name
```

Other settings: `CLOUD_SESSIONS_DAILY_CAP` (starts per day, default 5), `CLOUD_SESSIONS_HOME` (config and state folder, default `~/.config/cloud-sessions`), `CLOUD_SESSIONS_CLAUDE` (path of the `claude` program).

## Authentication

| What | Needs | Where the credential comes from |
| --- | --- | --- |
| `start`, `send` | `claude` signed in | the `claude` CLI's own login; this program never sees it |
| `/cloud-read`, `/cloud-watch` (inside Claude Code) | nothing extra | Claude Code's built-in `RemoteTrigger` tool (`get_run_log`) |
| `events`, `watch`, `cast`, `collect`, `list` (account view), `routines` (standalone program) | a token that can read sessions | see below |
| `collect` (files from GitHub) | `gh`, with `GH_TOKEN` or `gh auth login` | `gh`; the program handles no GitHub token |

**A token from `claude setup-token` cannot read sessions.** It is inference-only; the events endpoint answers HTTP 401 ("OAuth token lacks a scope this endpoint accepts"). Standalone reading therefore uses Claude Code's own login, after you opt in once, at a terminal:

```
$ cloud-sessions auth use-claude-login
This lets cloud-sessions read the access token Claude Code keeps after you sign in
(macOS: the login keychain item "Claude Code-credentials", else the file below; elsewhere the file
$CLAUDE_CONFIG_DIR/.credentials.json, or ~/.claude/.credentials.json when CLAUDE_CONFIG_DIR is not set),
only to call GET /v1/code/sessions/<id>/events. It is read each time it is needed; it is never printed,
copied, refreshed or stored by cloud-sessions. macOS may ask you to allow keychain access.
Undo with: cloud-sessions auth revoke-claude-login
Type yes to allow: yes
```

(Text from `auth.rs`; the JSON result follows on stdout.) The command refuses to run unless stdin is a terminal, so a script, a flag or a model tool call cannot grant it.

Other forms:

```bash
cloud-sessions auth status                 # {"ok":true,"claude_login_allowed":…,"token_file":…,"keychain":…}  (values are never shown)
cloud-sessions auth revoke-claude-login    # remove the opt-in
printf '%s' "$TOKEN" | cloud-sessions auth set --file   # a token that can read, kept in a private 0600 file (stdin, never argv)
cloud-sessions auth clear                  # forget the keychain item and the token file
```

Lookup order when reading: Claude Code's login (if you opted in), then the 0600 token file (`$CLOUD_SESSIONS_TOKEN_FILE` or `<home>/token`), then the keychain (macOS and Windows only). Claude Code's login is read from the macOS keychain first; when the keychain has no login, it is read from the login file. On Linux only the file is used. The login file is `$CLAUDE_CONFIG_DIR/.credentials.json` when `CLAUDE_CONFIG_DIR` is set, else `~/.claude/.credentials.json`. The expiry date is checked: an expired login is not sent. Over `ssh` a macOS login keychain is locked, so the keychain read fails there; if there is no login file either, use the token file or run the command in a local terminal. Nothing ever prints a token.

## Examples, one per verb

All commands print one JSON value; a failure prints `{"ok":false,"error":"…"}` and exits 1. `<session>` is a link (`https://claude.ai/code/session_…`), a `session_…` id, a `cse_…` id or a bare id; all forms map to the same session.

### start

```bash
cloud-sessions start --dir ~/code/my-repo --prompt "Add a build badge to the README and open a pull request"
printf '%s' "$PROMPT" | cloud-sessions start --dir ~/code/my-repo --prompt -     # prompt on stdin
```

Runs `claude --cloud` in a pseudo-terminal. The checkout's `origin` must be on your allowlist (the allowlist comes from `CLOUD_SESSIONS_REPOS` or the allowlist file, never from a flag). Prompt: 1 to 20000 characters. Billed like any cloud session; capped at 5 starts a day by default.

Result (**shape**, from the code; fill-ins in angle brackets):

```json
{"ok":true,"status":"started","session_id":"session_<id>","title":"<title>","url":"https://claude.ai/code/session_<id>","repo":"<owner/repo>","note":"Billed like your other Claude Code cloud sessions."}
```

If Claude Code asks whether to trust the folder, you get `{"ok":false,"status":"needs_trust",…}`: run `claude` once in that folder yourself and accept.

### start --batch

One session per line of a JSONL file, each told to write a report and end with a `REPORT:` line that `collect` reads.

```bash
cat > prompts.jsonl <<'EOF'
{"id":"docs","prompt":"Improve the API documentation."}
{"id":"tests","prompt":"Add missing unit tests."}
EOF
cloud-sessions start --batch batch1 --dir ~/code/my-repo --prompts prompts.jsonl
```

Each prompt gets this appended: `When done: write your report to reports/<id>.md, commit it to branch rl/<id>, push, and end with exactly one line: REPORT: rl/<id> reports/<id>.md <commit sha>`. `id` is 1 to 40 characters of letters, digits, `.`, `_`, `-`, unique in the file. The batch is all or nothing against the daily cap. Result (**shape**): `{"ok":true,"batch":"batch1","repo":"…","started":[{"id":"docs","session_id":"…","url":"…"},…]}`. The code path is covered by tests with a fake `claude`.

### track, untrack

For sessions that were started elsewhere (the app, another machine, a routine).

```bash
cloud-sessions track https://claude.ai/code/session_01ExampleAAAAAAAAAAAAAAA --title "example session" --repo your-org/your-repo
cloud-sessions track --from-file notes.md       # every https://claude.ai/code/session_… link in the file, deduplicated
cloud-sessions untrack session_01ExampleAAAAAAAAAAAAAAA     # local list only; the session is untouched
```

Result (**shape**): `{"ok":true,"added":["session_…"],"already_tracked":[],"tracked":[{"session_id","title","url","repo","started_at","last_seen"}]}`.

### list

```bash
cloud-sessions list                  # your account's sessions and routines (after auth use-claude-login), else the tracked ones
cloud-sessions list --tracked        # only the sessions this computer started or tracks; no request to the API
cloud-sessions list --json           # JSON at a terminal too (scripts and pipes always get JSON)
cloud-sessions list --unofficial     # the account view plus the raw API body (fails when the account list fails)
```

**Default view.** When you allowed the Claude Code login (`cloud-sessions auth use-claude-login`), `list` shows the sessions of your account, newest activity first: title, state, repository, last activity and link. The account's routines, with their newest runs, follow as a second section. A tracked session that is also in the account list shows once, with its title. A tracked session without a title shows its short id (`session_` plus 10 characters).

**First run.** Each state says what to do next, in the `hint` field and in the text output:

| State | What `list` shows | Hint |
| --- | --- | --- |
| Login not allowed | the tracked sessions (no token read, no request) | `cloud-sessions auth use-claude-login (asks for a typed yes)` |
| Login allowed, no sessions | an empty account list | start one with `/cloud-start` or `claude --cloud`, or track one with `/cloud-track` |
| Account list failed | the tracked sessions | one `fallback` line: the reason and the fix |

Result (**shape**): `{"ok":true,"view":"account"|"tracked","sessions":[{"session_id","title","label","state","repo","last_activity","url","tracked","in_account"}],"routines":[…],"hint":null|"…","fallback":null|"…","tracked":[…]}`. The account view reads every page of the account list (20 sessions a page, up to 50 pages or 1000 sessions); it also carries `pages`, `truncated` (a cap was reached), `partial` (a later page failed; the pages before it are shown), `untrusted`, `unofficial`, `routines_error` and `routine_runs_hidden: true`: runs started by a routine are **not** in the sessions list; they are under their routine. See [`routines`](#routines). An example text line, **trimmed**:

```
  example session  [completed]  your-org/your-repo  2026-11-05T10:30:00Z
    https://claude.ai/code/session_01ExampleAAAAAAAAAAAAAAA
```

### events (plugin: /cloud-read)

```bash
cloud-sessions events session_01ExampleAAAAAAAAAAAAAAA
cloud-sessions events session_01ExampleAAAAAAAAAAAAAAA --after <event id>     # only newer events
cloud-sessions events session_01ExampleAAAAAAAAAAAAAAA --raw                  # the unparsed API body
```

Result: `{"ok":true,"session_id":…,"untrusted":"…","events":[{"id","kind","user","text","at"}…],"cursor":"<newest event id>","finished":true|false}`. Events are decoded from the cloud envelope (`event_type`, `payload`, `created_at`, `sequence_num`) and ordered oldest first (the API returns newest first). Decoded kinds: user prompt, assistant text, `tool_use` (name and compact input), `tool_result` (first line), `result` (status, turns, duration, cost), provisioning `env[level]`, system notices; unknown types keep a `· <type>` label. `finished` is true once a `result` event is present. Example decoded lines:

```
10:00:01   assistant  ▸ Bash: {"command":"ls"}
10:00:04  tool_result  ◂ result: README.md (+1 lines)
10:00:06   assistant  Done.
```

Inside Claude Code, `/cloud-read <session> [cursor]` does the same with the built-in `RemoteTrigger` tool and needs no token.

### watch (plugin: /cloud-watch)

```bash
cloud-sessions watch session_01ExampleAAAAAAAAAAAAAAA --interval 3 --max-seconds 600
```

Prints each new event as one JSON line, `{"event":{…},"untrusted":true}`, then a final line `{"ok":true,"done":true,"reason":"finished"|"timeout","events":N,"cursor":"…"}`. Defaults: `--interval 5` (1 to 300 s), `--max-seconds 600` (at most 6 hours). `--after <id>` resumes from a cursor. `--stream` tries the undocumented event stream first and falls back to polling. **This is polling**: see [Limits](#limits-read-these).

### send (plugin: /cloud-send)

```bash
cloud-sessions send session_01ExampleAAAAAAAAAAAAAAA --message "Also mention the license in the README"
```

Example result:

```json
{"ok":true,"session_id":"session_01ExampleAAAAAAAAAAAAAAA","url":"https://claude.ai/code/session_01ExampleAAAAAAAAAAAAAAA?from=cli&m=0"}
```

Runs `claude -p … --cloud <id>` with your own login (from the checkout the session was started in, when known). Messages: 1 to 20000 characters; at least 2 seconds apart; at most 60 an hour per session. A sent message is a **peer message**: it can steer a session, it cannot approve a permission prompt for it. It is read between the session's steps.

### stop (plugin: /cloud-stop)

```bash
cloud-sessions stop session_01ExampleBBBBBBBBBBBBBBB
```

`stop` sends one interrupt (SIGINT) to the **local worker** of a **Remote Control** session on this machine. Example result (**trimmed** to the fields shown):

```json
{"interrupted_pids":[12345],"ok":true,"session_id":"session_01ExampleBBBBBBBBBBBBBBB","note":"Stopping ENDS the Remote Control session: one interrupt stops the turn, the worker exits and the server archives the session (unarchive it in the app to continue, or start a new one)."}
```

Stopping **ends** the session: the worker exits and the session is archived; later sends are refused until you unarchive it on its page. The same command against a **cloud** session has no local worker and returns `ok:false` with the session's page, because a cloud session can only be interrupted with the Stop button there. A Remote Control session on another machine: run `stop` on that machine (for example over `ssh`).

### collect

Gathers the final result of finished sessions, the files they point at, and a table.

```bash
cloud-sessions collect session_01ExampleCCCCCCCCCCCCCCC session_01ExampleAAAAAAAAAAAAAAA session_01ExampleDDDDDDDDDDDDDDD --out ./collected
cloud-sessions collect --all-tracked --out ./collected
cloud-sessions collect --batch batch1 --out ./collected --files 'reports/*'
```

Per session it reads the last `result` event (status, turns, duration, cost, final text); finds pointers in the text (`REPORT: <branch> <path> <sha>` first, else GitHub PR, branch and blob links); downloads those files with `gh` at the exact head commit into `<out>/<last 8 characters of id>/files/`; and writes `final.md` and `events.json`. Then `<out>/index.json` and `<out>/INDEX.md`. Only strict matchers read links out of event text, every path is checked before a file is written (`..` and absolute paths are rejected), and nothing in the text is executed.

`INDEX.md` has one row per session (session, title, repo, status, turns, duration, cost, verdict, PR, files) and a total row.

The verdict is the first line of the final text that starts with `VERDICT`, `PASS`, `PARTIAL` or `FAIL`; if there is none, the first line of a collected top-level `RESULT.md`.  `gh` may cap very large pull requests; files a pull request deleted are skipped with a note.

### cast

```bash
cloud-sessions cast session_01ExampleAAAAAAAAAAAAAAA --out example.cast
cloud-sessions cast session_01ExampleAAAAAAAAAAAAAAA --out example.cast --idle-max 2 --mp4 example.mp4
asciinema play example.cast
```

Writes the session's events as an [asciicast v2](https://docs.asciinema.org/manual/asciicast/v2/) file. Timing is the events' own timestamps; a gap longer than `--idle-max` seconds (default 2) is shortened to it. Colors: cyan prompt, yellow tool use, grey results and notices, green final result. Read-only on the session. Without `--out` the file is `<session id>.cast` in the current folder. `--mp4` renders an MP4 with `cast2mp4` (needs `agg` and `ffmpeg`); if they are missing the cast is still written and the error says which tools are missing. Result: `{"ok":true,"session_id","out","events","bytes","idle_max","play":"asciinema play <file>"[,"mp4"]}`.

### routines

Routines (scheduled or one-shot jobs) start sessions, and those runs are not in the main sessions list. `routines` lists them with their newest runs.

```bash
cloud-sessions routines                  # newest 5 runs per routine (default)
cloud-sessions routines --runs 20        # 1 to 100
```

Uses undocumented endpoints (`GET /v1/code/triggers`, `GET /v1/code/sessions?trigger_id=`). Both answer HTTP 404 unless the request carries `anthropic-beta: ccr-triggers-2026-01-30` (the feature Claude Code's own `RemoteTrigger` sends); the client adds it. Shape of one routine in the result:

```json
{"ok":true,"unofficial":"…","routines":[{"id":"trig_<id>","name":"<name>","enabled":true,"schedule":{"kind":"cron","cron":"0 3 * * *","run_once_at":null},"next_run_at":"<time>","last_fired_at":"<time>","url":"https://claude.ai/code/routines/trig_<id>","runs":[{"id":"cse_<id>","title":"<title>","status":"<status>","worker_status":"<status>","created_at":"<time>","last_event_at":"<time>","url":"https://claude.ai/code/session_<id>"}]}]}
```

`schedule.kind` is `cron` when the routine has a cron expression, else `once`. A routine whose runs cannot be read gets `runs_error` and does not fail the call. Only the first page of runs is read. A run id can be passed to `events`, `watch`, `cast` and `collect` like any other session.

### allow, mark, mcp

```bash
cloud-sessions allow --dir ~/code/my-repo                 # add the checkout's origin to the allowlist (you run this)
cloud-sessions mark session_… --seen 2026-01-01T00:00:00Z # remember the newest event a journal already wrote (/cloud-journal uses it)
cloud-sessions mcp                                        # MCP server over stdio
```

MCP tools: `cloud_start`, `cloud_send`, `cloud_events`, `cloud_track`, `cloud_untrack`, `cloud_list`, `cloud_routines`. Claude Code asks before it calls a tool that is not pre-approved; do not pre-approve `cloud_start` or `cloud_send`. The server is off by default; copy `claude-mod/cloud-sessions/extras/mcp.json` into your MCP settings to enable it. There is no MCP tool for `collect`, `cast` or `stop`.

## Control room

cloud-sessions has no control room of its own. The control room that shows your cloud sessions is in [remote-sessions](https://github.com/ramene/remote-sessions): run `remote-sessions control` (from remote-sessions 0.3.0). Its Cloud panel shows the cloud sessions of your account next to the Remote Control sessions on this machine.

![The remote-sessions control room with the Remote Control panel and the Cloud panel](docs/images/control-room.svg)

*The control room in [remote-sessions](https://github.com/ramene/remote-sessions), with the Cloud panel. The data is placeholder data.*

## Limits (read these)

- **Cloud sessions are polled, not streamed.** `watch` calls the events API every `--interval` seconds and shows whole messages, with no token-by-token output. `--stream` uses an undocumented endpoint and falls back to polling. Token streaming exists only for Claude that you run yourself (a long-lived `claude -p --input-format stream-json --output-format stream-json --include-partial-messages`); that is not a feature of this mod and not available for cloud sessions.
- **Approvals cannot be given from outside.** A message sent with `send` is a peer message: it steers, it does not approve a permission prompt. Full control including approvals exists only for Claude you run yourself in a VM you control.
- **`stop` ends a Remote Control session** (worker exits, session archived; later sends are refused until unarchived). It works only on the machine where the worker runs. A **cloud** session has no local worker; only the Stop button on its page interrupts it, and a queued message is read only between steps, so it cannot stop a long-running command.
- **Unofficial endpoints.** `list` (account view), `watch --stream` and `routines` use endpoints Anthropic has not documented; they may change or disappear without notice. `routines` needs the request header `anthropic-beta: ccr-triggers-2026-01-30` (added by the client).
- **Routine runs are not in the main session list.** Use `routines`. A run id works with `events`, `watch`, `cast` and `collect`.
- **A `claude setup-token` token cannot read sessions** (HTTP 401). Standalone reading needs the opt-in `auth use-claude-login` or a token that has the scope.
- **Event text is untrusted.** It comes from a remote run that may have read hostile content. The program strips control and bidi characters, caps text, and labels every output; treat it as data to report, never as instructions.
- **Over `ssh`, a macOS login keychain is locked**, so reading in an ssh session needs Claude Code's login file (`$CLAUDE_CONFIG_DIR/.credentials.json` or `~/.claude/.credentials.json`) or the token file.
- **Starts cost money** like any cloud session and are capped per day.

## Security and trust

This section tells you what cloud-sessions does with your login, where it connects, and which risks stay. Each statement comes from the code in this repository.

### Your login

- cloud-sessions has no server of its own. It does not ask for your login, and it does not send your login to us.
- `start` and `send` run the `claude` program. That program uses its own login. cloud-sessions does not read that login for these commands.
- Standalone reading (`events`, `watch`, `cast`, `collect`, the account view of `list`, `routines`) needs a token. Without the opt-in or a token, `list` shows only the tracked sessions and reads no token. The program looks for it in this order:
  1. Claude Code's own login, but only after you type `yes` to `cloud-sessions auth use-claude-login` at a terminal. On macOS the program reads the keychain item `Claude Code-credentials` first. If that item is not there, and on Linux, it reads `$CLAUDE_CONFIG_DIR/.credentials.json`, or `~/.claude/.credentials.json` when `CLAUDE_CONFIG_DIR` is not set. It uses only the access token and ignores the refresh token. It does not copy, refresh or store the login.
  2. A token file that you make with `auth set --file`: `$CLOUD_SESSIONS_TOKEN_FILE`, or `token` in the config folder. The file must have mode 0600. The program refuses a file that other users can read.
  3. A keychain item that you make with `auth set` (service `cloud-sessions`, account `token`; macOS and Windows only).
- The program sends the token only in the `Authorization` header of HTTPS requests to `https://api.anthropic.com`. It does not follow redirects, so the token does not go to a different host. You can set `CLOUD_SESSIONS_API` to a different address for a test server. The program accepts only an `https://` address, or `http://` to `127.0.0.1`, `localhost` or `[::1]`.
- The program never prints a token. `auth status` shows only which sources exist, with the note "Values are never shown." Error messages do not contain the token.
- A token from `claude setup-token` cannot read sessions. The events endpoint refuses it (HTTP 401).

### Where the program connects

| Host | Why | Login sent |
| --- | --- | --- |
| `api.anthropic.com` | Read session events; list sessions and routines | Yes: the token above |
| `github.com` | The plugin launcher downloads the release archive and its `.sha256` file, one time | No |
| `claude.ai` | Session and routine links only. The program reads and prints them. It does not connect to them. | No |

The program also starts two other programs. Each one uses its own login, and cloud-sessions does not touch it:

- `claude` (`start`, `send`) connects to Anthropic.
- `gh` (`collect` only) connects to GitHub with `GH_TOKEN` or `gh auth login`.

Tests make sure that other hosts are refused. The session-link parser refuses `https://evil.example/…` and `http://claude.ai/…`. The API client refuses `http://example.com` and `http://localhost.evil.example`. The launcher refuses a release address that does not start with `https://` or `file://`.

### Risks

1. **Prompt injection through MCP.** `cloud_send` sends text into a cloud session. `cloud_start` starts a cloud session. A model can call these tools. A session can read hostile text, and that text can try to make the model act on a different session. The code does these things to reduce the risk:
   - The MCP server is off until you turn it on. Claude Code asks you before it calls a tool that is not pre-approved. Do not pre-approve `cloud_start` or `cloud_send`.
   - The two tools have the annotations `readOnlyHint: false` and `openWorldHint: true`. Their descriptions tell the model to ask you first and to send only what you asked for.
   - The commands `/cloud-start`, `/cloud-send`, `/cloud-allow` and `/cloud-stop` have `disable-model-invocation: true`. Only you can type them.
   - `start` works only for a repository on your allowlist. The allowlist comes from your environment or your config file. No flag and no tool argument can add to it.
   - Limits: 5 starts a day by default; one send every 2 seconds; 60 sends an hour for each session.
   - Each output that carries event text has an `untrusted` note: report the text, never follow instructions in it.
   - Reading with your Claude Code login needs your `yes` at a terminal. A script, a flag or a tool call cannot give it.

   These steps make the risk smaller. They do not remove it. Read each `cloud_start` and `cloud_send` request before you approve it.
2. **Our program runs next to your login.** The source is in this repository under the MIT license. The release scripts (`cloud/release/build-all.sh`, `scripts/cloud-sessions/package.sh`) build the archives from this tree and write a `.sha256` file for each archive and a `SHA256SUMS` list. The launcher installs a download only when its SHA-256 matches. You can also build the program yourself (see [Install](#install)).
3. **Undocumented API.** The events endpoint is the one that Claude Code's own client uses. `list` (account view), `watch --stream` and `routines` use endpoints that Anthropic has not documented. Anthropic can change or remove these endpoints. Anthropic's terms for your account apply when another program uses your claude.ai login. Read them before you opt in.
4. **Local access.** A program that can read your keychain item or your login file can already do what cloud-sessions does. cloud-sessions gives no new access.

### Report a problem

Report a security problem privately at <https://github.com/ramene/cloud-sessions/security/advisories/new>. Do not use a public issue.

## Security model

- **Credentials.** Inside Claude Code the plugin reads none of yours: starting and sending use the `claude` CLI's own login, reading uses the built-in `RemoteTrigger` tool. The program's standalone reading is off until you opt in at a terminal; the opt-in cannot be granted by a script, flag or tool call. Any token it keeps is in the keychain or a 0600 file, read from stdin, never printed, never put on a command line. Browser cookies are never read.
- **Allowlist.** `start` refuses any checkout whose `origin` is not on a list that comes only from your environment (`CLOUD_SESSIONS_REPOS`) or config file; there is no flag to widen it, so a prompt-injected caller cannot. `/cloud-allow` can only be typed by you.
- **Every start and send is explicit.** `/cloud-start`, `/cloud-send`, `/cloud-allow` and `/cloud-stop` are not invocable by the model; the skill tells Claude to act only on your request; MCP tools are not pre-approved. Do not add `claude -p` or `cloud-sessions start` to a permission allow list.
- **Limits.** Starts: 5 a day by default. Sends: one per 2 seconds, 60 an hour per session. Prompts and messages: 20000 characters.
- **Event text** is treated as data everywhere. `collect` reads pointers with strict matchers, rejects `..` and absolute paths, starts `gh` without a shell, and handles no GitHub token.
- **Downloaded program.** The launcher installs a release only if its SHA-256 matches the published checksum.

## License

MIT. Copyright (c) 2026 the cloud-sessions contributors. See `LICENSE` (also in the plugin and in each crate).

## Contributing

Issues and pull requests are welcome. Build, test and style commands, the layout, and the rules for changes (JSON contract, explicit write actions, no tokens in output, placeholder-only examples) are in [CONTRIBUTING.md](CONTRIBUTING.md). Report security issues privately through GitHub's **Security → Report a vulnerability**, not in a public issue.
