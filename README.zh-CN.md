# cloud-sessions

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) [![Release](https://img.shields.io/github/v/release/ramene/cloud-sessions?sort=semver&label=release)](https://github.com/ramene/cloud-sessions/releases) [![Homebrew](https://img.shields.io/badge/homebrew-ramene%2Ftap-orange?logo=homebrew)](https://github.com/ramene/homebrew-tap) [![Claude Code plugin](https://img.shields.io/badge/Claude%20Code-plugin-d97757)](#安装) [![Rust](https://img.shields.io/badge/rust-2021-dea584?logo=rust)](Cargo.toml) [![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux-lightgrey)](#安装)

[English](README.md) · 中文

## 安装

分两部分。只装插件就足以完成 read、watch、send 和 track。start、list、collect、cast、routines、stop 以及 MCP 服务器则需要安装程序本体。

**1. 插件**（只需要 Claude Code）：

```bash
claude plugin marketplace add ramene/cloud-sessions
claude plugin install cloud-sessions@cloud-sessions
```

如果想直接从本地检出的代码试用：`claude plugin marketplace add ./` 或 `claude --plugin-dir ./claude-mod/cloud-sessions`。

**2. 程序本体。** 插件的启动器（`claude-mod/cloud-sessions/scripts/cloud-sessions`）按以下顺序查找：`$CLOUD_SESSIONS_BIN`、插件内置的副本、之前下载的副本、`PATH` 上已安装的 `cloud-sessions`，或者下载与插件版本对应的发布版（macOS universal、Linux x86_64、Linux arm64），其 SHA-256 必须与公布的校验和一致。如果以上都做不到，它会输出一句 JSON，并且不会安装任何东西。

如需自行构建（需要 Rust 工具链；Linux 构建不需要 libdbus 和 OpenSSL）：

```bash
git clone https://github.com/ramene/cloud-sessions
cd cloud-sessions
cargo build --release --locked        # target/release/cloud-sessions
cargo test --workspace --locked
```

发布压缩包和 `SHA256SUMS` 由 `cloud/release/build-all.sh` 构建；参见 `cloud/release/RELEASE.md`。也可以使用 Homebrew：`brew install ramene/tap/cloud-sessions`。`collect` 还需要路径中有 GitHub CLI（`gh`）。

在第一次 `start` 之前：先允许该仓库一次，并在该目录中运行一次 `claude`，以接受它的信任提示。本 mod 绝不会代替你回答这个提示。

```bash
cloud-sessions allow --dir ~/code/my-repo        # or /cloud-allow in Claude Code, or CLOUD_SESSIONS_REPOS=owner/name
```

其他设置：`CLOUD_SESSIONS_DAILY_CAP`（每日启动次数，默认 5）、`CLOUD_SESSIONS_HOME`（配置和状态目录，默认 `~/.config/cloud-sessions`）、`CLOUD_SESSIONS_CLAUDE`（`claude` 程序的路径）。

## 认证

| 内容 | 需要 | 凭据来源 |
| --- | --- | --- |
| `start`、`send` | 已登录的 `claude` | `claude` CLI 自己的登录；本程序从不接触它 |
| `/cloud-read`、`/cloud-watch`（在 Claude Code 内部） | 无需额外条件 | Claude Code 内置的 `RemoteTrigger` 工具（`get_run_log`） |
| `events`、`watch`、`cast`、`collect`、`list --unofficial`、`routines`（独立程序） | 一个能够读取会话的 token | 见下文 |
| `collect`（从 GitHub 获取文件） | `gh`，配合 `GH_TOKEN` 或 `gh auth login` | `gh`；本程序不处理任何 GitHub token |

**由 `claude setup-token` 生成的 token 不能读取会话。** 它仅用于推理；events 端点会返回 HTTP 401（"OAuth token lacks a scope this endpoint accepts"）。因此独立读取使用的是 Claude Code 自己的登录凭据，且需要你在终端中事先明确同意一次：

```
$ cloud-sessions auth use-claude-login
This lets cloud-sessions read the access token Claude Code keeps after you sign in
(macOS: the login keychain item "Claude Code-credentials"; elsewhere ~/.claude/.credentials.json),
only to call GET /v1/code/sessions/<id>/events. It is read each time it is needed; it is never printed,
copied, refreshed or stored by cloud-sessions. macOS may ask you to allow keychain access.
Undo with: cloud-sessions auth revoke-claude-login
Type yes to allow: yes
```

（以上文本来自 `auth.rs`；随后标准输出会给出 JSON 结果。）除非标准输入是终端，否则该命令会拒绝运行，所以脚本、命令行标志或模型的工具调用都无法授予这项权限。

其他形式：

```bash
cloud-sessions auth status                 # {"ok":true,"claude_login_allowed":…,"token_file":…,"keychain":…}  (values are never shown)
cloud-sessions auth revoke-claude-login    # remove the opt-in
printf '%s' "$TOKEN" | cloud-sessions auth set --file   # a token that can read, kept in a private 0600 file (stdin, never argv)
cloud-sessions auth clear                  # forget the keychain item and the token file
```

读取时的查找顺序：先是 Claude Code 的登录凭据（如果你已同意使用），然后是权限为 0600 的 token 文件（`$CLOUD_SESSIONS_TOKEN_FILE` 或 `<home>/token`），最后是钥匙串（仅限 macOS 和 Windows）。通过 `ssh` 登录时，macOS 的登录钥匙串是锁定的，因此在 ssh 会话中独立读取会失败；请改用 token 文件，或在本地终端中运行该命令。任何情况下都不会打印 token。

## 各子命令示例

所有命令都只打印一个 JSON 值；失败时打印 `{"ok":false,"error":"…"}` 并以状态码 1 退出。`<session>` 可以是链接（`https://claude.ai/code/session_…`）、`session_…` id、`cse_…` id 或裸 id；所有形式都对应同一个会话。

### start

```bash
cloud-sessions start --dir ~/code/my-repo --prompt "Add a build badge to the README and open a pull request"
printf '%s' "$PROMPT" | cloud-sessions start --dir ~/code/my-repo --prompt -     # prompt on stdin
```

在伪终端中运行 `claude --cloud`。检出目录的 `origin` 必须在你的允许列表中（允许列表只来自 `CLOUD_SESSIONS_REPOS` 或允许列表文件，绝不来自命令行标志）。提示词：1 到 20000 个字符。与任何云端会话一样计费；默认每天最多启动 5 次。

结果（**结构**，取自代码；需要填入的内容用尖括号标出）：

```json
{"ok":true,"status":"started","session_id":"session_<id>","title":"<title>","url":"https://claude.ai/code/session_<id>","repo":"<owner/repo>","note":"Billed like your other Claude Code cloud sessions."}
```

如果 Claude Code 询问是否信任该目录，你会得到 `{"ok":false,"status":"needs_trust",…}`：请自己在该目录中运行一次 `claude` 并接受。

### start --batch

JSONL 文件中的每一行对应一个会话，每个会话都被要求撰写一份报告，并以一行 `REPORT:` 结尾，供 `collect` 读取。

```bash
cat > prompts.jsonl <<'EOF'
{"id":"docs","prompt":"Improve the API documentation."}
{"id":"tests","prompt":"Add missing unit tests."}
EOF
cloud-sessions start --batch batch1 --dir ~/code/my-repo --prompts prompts.jsonl
```

每个提示词都会被追加这样一段话：`When done: write your report to reports/<id>.md, commit it to branch rl/<id>, push, and end with exactly one line: REPORT: rl/<id> reports/<id>.md <commit sha>`。`id` 由 1 到 40 个字符组成，可用字母、数字、`.`、`_`、`-`，且在文件中必须唯一。批量启动对每日上限而言是全有或全无的。结果（**结构**）：`{"ok":true,"batch":"batch1","repo":"…","started":[{"id":"docs","session_id":"…","url":"…"},…]}`。该代码路径由使用假 `claude` 的测试覆盖。

### track、untrack

用于在别处启动的会话（应用、另一台机器、某个 routine）。

```bash
cloud-sessions track https://claude.ai/code/session_01ExampleAAAAAAAAAAAAAAA --title "example session" --repo your-org/your-repo
cloud-sessions track --from-file notes.md       # every https://claude.ai/code/session_… link in the file, deduplicated
cloud-sessions untrack session_01ExampleAAAAAAAAAAAAAAA     # local list only; the session is untouched
```

结果（**结构**）：`{"ok":true,"added":["session_…"],"already_tracked":[],"tracked":[{"session_id","title","url","repo","started_at","last_seen"}]}`。

### list

```bash
cloud-sessions list                  # the sessions this machine started or tracks
cloud-sessions list --unofficial     # also the account's sessions (undocumented endpoint)
```

不带 `--unofficial` 时，结果为 `{"ok":true,"tracked":[…]}`。下面是一个已跟踪条目的示例，**已精简**为实际打印的字段：

```
session_01ExampleAAAAAAAAAAAAAAA · example session
```

带 `--unofficial` 时，结果还会包含 `remote`（原始 API 响应体，不可信）、`unofficial`，以及 `routine_runs_hidden: true`：由 routine 启动的运行**不会**出现在此列表中（账号的主列表本来就不显示其 routine 运行）。请使用 [`routines`](#routines)。

### events（插件：/cloud-read）

```bash
cloud-sessions events session_01ExampleAAAAAAAAAAAAAAA
cloud-sessions events session_01ExampleAAAAAAAAAAAAAAA --after <event id>     # only newer events
cloud-sessions events session_01ExampleAAAAAAAAAAAAAAA --raw                  # the unparsed API body
```

结果：`{"ok":true,"session_id":…,"untrusted":"…","events":[{"id","kind","user","text","at"}…],"cursor":"<newest event id>","finished":true|false}`。事件从云端信封（`event_type`、`payload`、`created_at`、`sequence_num`）中解码，并按从旧到新排序（API 返回的是从新到旧）。已解码的类型包括：用户提示、assistant 文本、`tool_use`（名称和精简的输入）、`tool_result`（首行）、`result`（状态、回合数、耗时、费用）、provisioning 的 `env[level]`、系统通知；未知类型会保留 `· <type>` 标签。一旦出现 `result` 事件，`finished` 即为 true。解码后的示例行：

```
10:00:01   assistant  ▸ Bash: {"command":"ls"}
10:00:04  tool_result  ◂ result: README.md (+1 lines)
10:00:06   assistant  Done.
```

在 Claude Code 内部，`/cloud-read <session> [cursor]` 通过内置的 `RemoteTrigger` 工具完成同样的事，并且不需要 token。

### watch（插件：/cloud-watch）

```bash
cloud-sessions watch session_01ExampleAAAAAAAAAAAAAAA --interval 3 --max-seconds 600
```

每个新事件打印为一行 JSON，即 `{"event":{…},"untrusted":true}`，最后再打印一行 `{"ok":true,"done":true,"reason":"finished"|"timeout","events":N,"cursor":"…"}`。默认值：`--interval 5`（1 到 300 秒）、`--max-seconds 600`（最多 6 小时）。`--after <id>` 从某个 cursor 继续。`--stream` 会先尝试未文档化的事件流，失败则回退为轮询。**这是轮询**：参见[限制](#限制请务必阅读)。

### send（插件：/cloud-send）

```bash
cloud-sessions send session_01ExampleAAAAAAAAAAAAAAA --message "Also mention the license in the README"
```

示例结果：

```json
{"ok":true,"session_id":"session_01ExampleAAAAAAAAAAAAAAA","url":"https://claude.ai/code/session_01ExampleAAAAAAAAAAAAAAA?from=cli&m=0"}
```

使用你自己的登录凭据运行 `claude -p … --cloud <id>`（已知时，会在会话启动时所在的检出目录中运行）。消息：1 到 20000 个字符；相邻两条至少间隔 2 秒；每个会话每小时最多 60 条。发送出去的消息是**对等消息（peer message）**：它可以引导会话，但不能为会话批准权限提示。会话会在其步骤之间读取该消息。

### stop（插件：/cloud-stop）

```bash
cloud-sessions stop session_01ExampleBBBBBBBBBBBBBBB
```

`stop` 向本机上某个 **Remote Control** 会话的**本地 worker** 发送一次中断（SIGINT）。示例结果（**已精简**为所示字段）：

```json
{"interrupted_pids":[12345],"ok":true,"session_id":"session_01ExampleBBBBBBBBBBBBBBB","note":"Stopping ENDS the Remote Control session: one interrupt stops the turn, the worker exits and the server archives the session (unarchive it in the app to continue, or start a new one)."}
```

停止会**结束**会话：worker 退出，会话被归档；在你于其页面上取消归档之前，之后的 send 都会被拒绝。对**云端**会话执行同样的命令时，由于没有本地 worker，会返回 `ok:false` 并附上该会话的页面，因为云端会话只能通过那里的 Stop 按钮来中断。对于另一台机器上的 Remote Control 会话：请在那台机器上运行 `stop`（例如通过 `ssh`）。

### collect

收集已完成会话的最终结果、它们所指向的文件，以及一张汇总表。

```bash
cloud-sessions collect session_01ExampleCCCCCCCCCCCCCCC session_01ExampleAAAAAAAAAAAAAAA session_01ExampleDDDDDDDDDDDDDDD --out ./collected
cloud-sessions collect --all-tracked --out ./collected
cloud-sessions collect --batch batch1 --out ./collected --files 'reports/*'
```

对每个会话，它读取最后一个 `result` 事件（状态、回合数、耗时、费用、最终文本）；在文本中查找指针（首选 `REPORT: <branch> <path> <sha>`，否则是 GitHub 的 PR、分支和 blob 链接）；用 `gh` 在确切的 head 提交上下载这些文件，存入 `<out>/<last 8 characters of id>/files/`；并写出 `final.md` 和 `events.json`。随后生成 `<out>/index.json` 和 `<out>/INDEX.md`。只有严格的匹配器才会从事件文本中读取链接，写入任何文件之前都会检查每个路径（`..` 和绝对路径会被拒绝），并且文本中的任何内容都不会被执行。

`INDEX.md` 每个会话一行（session、title、repo、status、turns、duration、cost、verdict、PR、files），外加一行合计。

verdict 取最终文本中第一行以 `VERDICT`、`PASS`、`PARTIAL` 或 `FAIL` 开头的内容；如果没有，则取所收集的顶层 `RESULT.md` 的第一行。`gh` 可能会对特别大的 pull request 设置上限；pull request 中被删除的文件会被跳过并附带说明。

### cast

```bash
cloud-sessions cast session_01ExampleAAAAAAAAAAAAAAA --out example.cast
cloud-sessions cast session_01ExampleAAAAAAAAAAAAAAA --out example.cast --idle-max 2 --mp4 example.mp4
asciinema play example.cast
```

把会话的事件写成 [asciicast v2](https://docs.asciinema.org/manual/asciicast/v2/) 文件。时间轴采用事件自身的时间戳；超过 `--idle-max` 秒（默认 2）的间隔会被缩短到该值。配色：青色为提示、黄色为工具调用、灰色为结果和通知、绿色为最终结果。对会话只读。不带 `--out` 时，文件为当前目录下的 `<session id>.cast`。`--mp4` 使用 `cast2mp4` 渲染 MP4（需要 `agg` 和 `ffmpeg`）；如果缺少它们，cast 仍会写出，并且错误信息会指明缺少哪些工具。结果：`{"ok":true,"session_id","out","events","bytes","idle_max","play":"asciinema play <file>"[,"mp4"]}`。

### routines

Routines（定时或一次性任务）会启动会话，而这些运行不在主会话列表中。`routines` 会列出它们及其最新的运行。

```bash
cloud-sessions routines                  # newest 5 runs per routine (default)
cloud-sessions routines --runs 20        # 1 to 100
```

使用未文档化的端点（`GET /v1/code/triggers`、`GET /v1/code/sessions?trigger_id=`）。除非请求带有 `anthropic-beta: ccr-triggers-2026-01-30`（Claude Code 自己的 `RemoteTrigger` 所发送的功能标识），否则两者都会返回 HTTP 404；客户端会自动加上它。结果中单个 routine 的结构：

```json
{"ok":true,"unofficial":"…","routines":[{"id":"trig_<id>","name":"<name>","enabled":true,"schedule":{"kind":"cron","cron":"0 3 * * *","run_once_at":null},"next_run_at":"<time>","last_fired_at":"<time>","url":"https://claude.ai/code/routines/trig_<id>","runs":[{"id":"cse_<id>","title":"<title>","status":"<status>","worker_status":"<status>","created_at":"<time>","last_event_at":"<time>","url":"https://claude.ai/code/session_<id>"}]}]}
```

当 routine 带有 cron 表达式时，`schedule.kind` 为 `cron`，否则为 `once`。如果某个 routine 的运行无法读取，它会带上 `runs_error`，但不会导致整个调用失败。只读取运行的第一页。运行 id 可以像任何其他会话一样传给 `events`、`watch`、`cast` 和 `collect`。

### allow、mark、mcp

```bash
cloud-sessions allow --dir ~/code/my-repo                 # add the checkout's origin to the allowlist (you run this)
cloud-sessions mark session_… --seen 2026-01-01T00:00:00Z # remember the newest event a journal already wrote (/cloud-journal uses it)
cloud-sessions mcp                                        # MCP server over stdio
```

MCP 工具：`cloud_start`、`cloud_send`、`cloud_events`、`cloud_track`、`cloud_untrack`、`cloud_list`、`cloud_routines`。Claude Code 在调用未预先批准的工具之前会先询问；请不要预先批准 `cloud_start` 或 `cloud_send`。该服务器默认关闭；如需启用，请把 `claude-mod/cloud-sessions/extras/mcp.json` 复制到你的 MCP 设置中。`collect`、`cast` 和 `stop` 没有对应的 MCP 工具。

## 限制（请务必阅读）

- **云端会话是轮询的，而不是流式的。** `watch` 每隔 `--interval` 秒调用一次 events API，显示的是完整的消息，没有逐 token 的输出。`--stream` 使用未文档化的端点，并会回退为轮询。逐 token 的流式输出只适用于你自己运行的 Claude（一个长期运行的 `claude -p --input-format stream-json --output-format stream-json --include-partial-messages`）；这不是本 mod 的功能，云端会话也无法使用。
- **无法从外部给出批准。** 通过 `send` 发出的消息是对等消息：它可以引导，但不能批准权限提示。包含批准在内的完整控制，只适用于你在自己掌控的虚拟机中运行的 Claude。
- **`stop` 会结束 Remote Control 会话**（worker 退出，会话被归档；取消归档之前，后续的 send 都会被拒绝）。它只能在 worker 所在的机器上起作用。**云端**会话没有本地 worker；只有其页面上的 Stop 按钮能中断它，而排队的消息只在步骤之间被读取，所以无法中止一个长时间运行的命令。
- **未文档化的端点。** `list --unofficial`、`watch --stream` 和 `routines` 使用 Anthropic 尚未文档化的端点；它们可能在没有通知的情况下变化或消失。`routines` 需要请求头 `anthropic-beta: ccr-triggers-2026-01-30`（由客户端添加）。
- **Routine 的运行不在主会话列表中。** 请使用 `routines`。运行 id 可用于 `events`、`watch`、`cast` 和 `collect`。
- **由 `claude setup-token` 生成的 token 不能读取会话**（HTTP 401）。独立读取需要事先同意的 `auth use-claude-login`，或一个具有相应权限范围的 token。
- **事件文本不可信。** 它来自远端的运行，而该运行可能读取过恶意内容。程序会去除控制字符和双向（bidi）字符、限制文本长度，并为每一项输出加上标注；请把它当作需要报告的数据，绝不能当作指令。
- **通过 `ssh` 时，macOS 的登录钥匙串是锁定的**，因此除非使用 token 文件，否则在 ssh 会话中读取会失败。
- **启动是要花钱的**，与任何云端会话一样，并且每天有上限。

## 安全模型

- **凭据。** 在 Claude Code 内部，插件不会读取你的任何凭据：启动和发送使用 `claude` CLI 自己的登录，读取则使用内置的 `RemoteTrigger` 工具。程序的独立读取功能在你于终端中明确同意之前一直是关闭的；这项同意无法由脚本、命令行标志或工具调用授予。它保存的任何 token 都在钥匙串或 0600 权限的文件中，从标准输入读取，绝不打印，也绝不放在命令行上。浏览器 cookie 绝不会被读取。
- **允许列表。** `start` 会拒绝任何 `origin` 不在列表中的检出目录，而该列表只来自你的环境变量（`CLOUD_SESSIONS_REPOS`）或配置文件；没有任何命令行标志可以放宽它，因此被提示词注入的调用方也做不到。`/cloud-allow` 只能由你亲自输入。
- **每一次 start 和 send 都是显式的。** `/cloud-start`、`/cloud-send`、`/cloud-allow` 和 `/cloud-stop` 不能由模型调用；skill 会告诉 Claude 只在你提出请求时才行动；MCP 工具不会被预先批准。请不要把 `claude -p` 或 `cloud-sessions start` 加入权限允许列表。
- **上限。** 启动：默认每天 5 次。发送：每 2 秒一条，每个会话每小时 60 条。提示词和消息：20000 个字符。
- **事件文本**在任何地方都被当作数据对待。`collect` 用严格的匹配器读取指针，拒绝 `..` 和绝对路径，不经过 shell 启动 `gh`，并且不处理任何 GitHub token。
- **下载的程序。** 只有当发布版的 SHA-256 与公布的校验和一致时，启动器才会安装它。

## 许可证

MIT。Copyright (c) 2026 the cloud-sessions contributors。详见 `LICENSE`（插件和各个 crate 中也有一份）。

## 参与贡献

欢迎提交 Issue 和 Pull Request。构建、测试和代码风格命令、目录结构，以及变更规则（JSON 契约、显式的写操作、输出中不含 token、示例只使用占位符）都写在 [CONTRIBUTING.md](CONTRIBUTING.md) 中。安全问题请通过 GitHub 的 **Security → Report a vulnerability** 私下报告，不要公开提 Issue。
