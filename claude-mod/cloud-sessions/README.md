# cloud-sessions (Claude Code plugin)

Start, track, list, read, watch and send into Claude Code **cloud sessions** from any Claude Code session.
**Nothing to install but the plugin**: no Rust, no cargo, no Homebrew, no token to paste.

## Install (two commands)

```bash
claude plugin marketplace add ramene/cloud-sessions
claude plugin install cloud-sessions@cloud-sessions
```

That is all. Open the repository you want to work on and use the commands below. (To try it from a checkout instead:
`claude plugin marketplace add ./`, or `claude --plugin-dir ./claude-mod/cloud-sessions`.)

## What it uses, so you know what runs

| Command | Does | Runs on |
| --- | --- | --- |
| `/cloud-read <session> [cursor]` | Events, summarised | Claude Code's built-in **`RemoteTrigger`** tool (`get_run_log`) with your own login |
| `/cloud-watch <session> [seconds]` | Follow a session until it finishes | the same built-in tool |
| `/cloud-send <session> <message>` | Send a follow-up into a running session | your own **`claude`** command (`claude -p … --cloud <id>`) |
| `/cloud-allow` | Allow cloud sessions for the current repository (once per repo) | the optional program¹ |
| `/cloud-start <prompt>` | Start a cloud session for the current repo (costs cloud usage) | the optional program¹ (it needs a terminal that Claude Code's shell lacks) |
| `/cloud-track <link\|id> [--title t] \| --from-file <path>` | Track sessions started elsewhere (every claude.ai link in a file, deduplicated) | the optional program¹ |
| `/cloud-list` | Tracked sessions with title and link | the optional program¹ |

¹ **The optional program** is a small prebuilt `cloud-sessions` binary (about 4 MB, MIT licensed, built from `crates/` in this
repository). The plugin's launcher (`scripts/cloud-sessions`) gets it **by itself the first time** one of those commands runs:
it uses a copy bundled in the plugin if the release has one, otherwise it downloads the release for your computer (macOS
universal, Linux x86_64 or arm64) into the plugin's data folder and **checks its SHA-256 against the checksum published with
the release** before installing it. It needs only what macOS and Linux already have (`curl` or `wget`, `tar`, and `shasum`,
`sha256sum` or `openssl`). If it cannot (offline, release not available), you get one plain sentence and nothing is installed.
Reading, watching and sending never need it.

The `cloud-sessions` skill teaches Claude the same rules when you ask in plain language ("check on my cloud session"). An
optional MCP server for the same actions is in `extras/mcp.json` (off by default, so nothing starts or downloads at launch).

Before the first `/cloud-start`: run `/cloud-allow` in that repository, and make sure `claude` itself works and you have
accepted its trust prompt once in that folder (the mod never answers that prompt for you).

> A token from `claude setup-token` is **inference-only**: the events endpoint answers 401 ("OAuth token lacks a scope this
> endpoint accepts"), which is why reading goes through Claude Code's own `RemoteTrigger`. Reading from a plain terminal or
> another program is optional and off by default; see *Standalone reading* in `docs/CLOUD-SESSIONS-MOD.md`.

Optional: `CLOUD_SESSIONS_DAILY_CAP` (default 5 starts a day). More: `docs/CLOUD-SESSIONS-INSTALL.md`; building releases is in `cloud/release/RELEASE.md`.

## License

MIT, Copyright (c) 2026 the cloud-sessions contributors (see `LICENSE`).

## Security model

- **Credentials**: inside Claude Code nothing of yours is read by this plugin: starting and sending use the `claude`
  CLI's own login, and reading uses the built-in `RemoteTrigger` tool. The CLI's standalone reading is off unless
  you opt in; any token it keeps is in the keychain or a 0600 file, read from stdin, never printed. No browser
  cookies are read, ever.
- **Allowlist**: `start` refuses any checkout whose `origin` is not on your list. You add a repository with `/cloud-allow`
  (which only you can run: it is `disable-model-invocation`) or `CLOUD_SESSIONS_REPOS`; there is no flag on `start` to
  widen it.
- **Every start and send is explicit**: `/cloud-start`, `/cloud-send` and `/cloud-allow` are `disable-model-invocation`, so
  only you can trigger them; the skill tells Claude to act only on your request; the optional MCP tools are
  not pre-approved, so Claude Code asks first. Do not add `claude -p`/`cloud-sessions start` to your permission allow
  list.
- **Event text is untrusted**: it is shown as data (cleaned of control and bidi characters, capped) and every
  output says so; the skill and commands tell Claude to report it, not obey it.
- **Limits**: starts are capped per day (default 5). `/cloud-send` sends one message per run; the optional program's own
  `send` also spaces sends and caps them at 60 an hour per session.
  Sessions started with `claude --cloud` bill like your other cloud sessions.
- **Unofficial endpoints** (`list --unofficial`, `watch --stream`) are opt-in, off by default, and fall back
  safely.

See `docs/CLOUD-SESSIONS-MOD.md` in the repository for the architecture.
