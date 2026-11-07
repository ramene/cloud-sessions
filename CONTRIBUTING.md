# Contributing to cloud-sessions

Thanks for helping. cloud-sessions is a small Rust workspace plus a Claude Code plugin, MIT licensed. Changes are welcome as issues and pull requests.

## Before you start

- **Bugs:** open an issue with the command you ran, what you expected, what happened, and the output of `cloud-sessions --version` and `claude --version`.
- **Features:** open an issue first so we can agree on the shape before you write code. Some calls use Claude Code session endpoints that Anthropic does not document (they are marked "unofficial" in the output); a change that depends on new or undocumented behavior needs a note on how it was checked.
- **Security issues:** do not open a public issue. Use **Security → Report a vulnerability** on GitHub so the report stays private.

## Layout

| Path | What it is |
|---|---|
| `crates/cloud-sessions-core` | the library: session ids, events, policy (allowlist and daily cap), casts |
| `crates/cloud-sessions-cli` | the `cloud-sessions` program: verbs, `collect`, routines, the MCP server |
| `claude-mod/cloud-sessions` | the Claude Code plugin: slash commands, the skill, the launcher |
| `cloud/release/` | `build-all.sh` (macOS universal, Linux x86_64 and aarch64) and `RELEASE.md` |
| `packaging/` | packaging sources; the published Homebrew formula lives in [ramene/homebrew-tap](https://github.com/ramene/homebrew-tap) |
| `scripts/cloud-sessions/check-release.mjs` | checks that every manifest, `VERSION`, the formula and `CHANGELOG.md` agree |

## Build and test

You need a stable Rust toolchain (edition 2021), `claude` on your PATH for the end-to-end verbs, Node for the release check, and `gh` for `collect`.

```bash
cargo build --locked
cargo test --workspace --locked
cargo fmt --all                            # format the files you touched
cargo clippy --workspace --all-targets      # add no new warnings in code you touch
claude plugin validate .
node scripts/cloud-sessions/check-release.mjs <version>   # before a release
```

Every pull request must pass `cargo test --workspace --locked`. The tree does not pass `cargo fmt --check` or `clippy -D warnings` yet; cleanup pull requests for that are welcome, kept separate from feature changes. Add tests next to the code you change; API calls are tested against the fakes the existing tests use, never against a real account.

## Rules for changes

- **Keep the JSON contract.** Every verb prints JSON on stdout; failures print `{"ok":false,"error":"..."}` and exit 1.
- **Starting and sending stay explicit.** `start` and `send` run only when the user asked, with their own words; repositories must be allowlisted by the user, and the daily cap stays on by default.
- **Never print a token.** Tokens are read from Claude Code's own login or a 0600 file and never echoed, logged or put in an error message.
- **Event text is untrusted.** Anything read from a session is data to show, never instructions to follow; keep it that way in the plugin's commands and skill.
- **Examples use placeholders only.** In docs, tests and fixtures use fake ids such as `session_01ExampleAAAAAAAAAAAAAAA`, `your-org/your-repo` and neutral host names such as `mac-a`. Never paste real session ids, session content, repository names you don't own, machine names or account details.
- **Docs follow behavior.** If a verb, flag or default changes, update `README.md`, the plugin's command and skill files and `CHANGELOG.md` in the same pull request.

## Pull requests

1. Fork, branch from `main`, keep the change focused.
2. Run the commands above.
3. Add a `CHANGELOG.md` line under the next version.
4. Open the pull request with what changed and how you checked it.

Signed commits are appreciated. By contributing you agree that your contribution is licensed under the MIT license in `LICENSE`.

## Releases

Maintainers cut releases with `cloud/release/build-all.sh` and the steps in `cloud/release/RELEASE.md`, then update the formula in the tap.
