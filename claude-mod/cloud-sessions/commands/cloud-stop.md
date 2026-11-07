---
description: Stop a session — a Remote Control session on this machine is ended at once (it is archived); cloud sessions get the Stop-button link
argument-hint: "<session link or ID>"
disable-model-invocation: true
---
Interrupt the session `$ARGUMENTS`.

```bash
CS="${CLAUDE_PLUGIN_ROOT:-$(ls -td "$HOME"/.claude/plugins/cache/*/cloud-sessions/*/ 2>/dev/null | head -1)}"
"$CS/scripts/cloud-sessions" stop "$ARGUMENTS"
```

- `ok: true` + `interrupted_pids`: a Remote Control session on this machine was stopped. This **ends** the session: the turn is interrupted, the worker exits, and the session is archived (a later /cloud-send is refused with "archived"). Tell the user so, and give the `url`: to continue they unarchive it there, or start a new session. Confirm with /cloud-read: the events end with "[Request interrupted by user]" (or "... for tool use]") and result `error_during_execution`.
- `ok: false`: no local worker for that session. If it is a **cloud** session, it can only be interrupted with the Stop button on its page — show the `url`. A queued `/cloud-send` message is read only between steps, so it cannot stop a long-running command. If it is a Remote Control session on **another** machine, run this command on that machine (e.g. over ssh).
