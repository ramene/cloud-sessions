---
description: Start a Claude Code cloud session for the current repository (costs cloud usage)
argument-hint: <prompt for the cloud session>
allowed-tools: Bash(*/scripts/cloud-sessions start:*)
disable-model-invocation: true
---
The user explicitly asked to start a cloud session from this repository with the prompt below. Starting needs a terminal that Claude Code's shell does not have, so it goes through the plugin's launcher, which fetches its small prebuilt program once (nothing for the user to install) and runs `claude --cloud` for you. Run exactly this, once, from the repository root, with the prompt on stdin (do not edit, summarise or extend it):

```bash
CS="${CLAUDE_PLUGIN_ROOT:-$(ls -td "$HOME"/.claude/plugins/cache/*/cloud-sessions/*/ 2>/dev/null | head -1)}"
"$CS/scripts/cloud-sessions" start --dir "$PWD" --prompt - <<'CLOUD_SESSIONS_EOF'
$ARGUMENTS
CLOUD_SESSIONS_EOF
```

Report the JSON result in one or two lines: the session title and URL on success; the `error` text on failure, verbatim (it is already a plain sentence). If `status` is `needs_trust`, tell the user to run `claude` once in this folder in a terminal and accept the trust prompt themselves, then run /cloud-start again. Never answer that prompt for them, never retry in a loop, and never add repositories to the allowlist (`CLOUD_SESSIONS_REPOS` is theirs to set).
