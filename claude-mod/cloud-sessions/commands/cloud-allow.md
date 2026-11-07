---
description: Allow cloud sessions for the current repository (one time per repository)
argument-hint:
allowed-tools: Bash(*/scripts/cloud-sessions allow:*)
disable-model-invocation: true
---
The user asked to allow cloud sessions to be started for the repository in the current folder. This only records the repository's `origin` (`owner/name`) in the user's allowlist; it starts nothing. Run:

```bash
CS="${CLAUDE_PLUGIN_ROOT:-$(ls -td "$HOME"/.claude/plugins/cache/*/cloud-sessions/*/ 2>/dev/null | head -1)}"
"$CS/scripts/cloud-sessions" allow --dir "$PWD"
```

Report which repository was allowed, or the error as is (for example: the folder has no GitHub `origin`). Then the user can run /cloud-start. Only the user decides which repositories are allowed: do not run this yourself.
