---
description: Send a follow-up message into an existing Claude Code cloud session
argument-hint: <session link or ID> <message>
allowed-tools: Bash(claude -p:*)
disable-model-invocation: true
---
The user explicitly asked to send a message into a cloud session. The first word of the arguments below is the session link or ID; the rest is the message. Take the session ID from the link (its last path segment, `session_…`); it must contain only letters, digits, `_` and `-`, otherwise stop and say so. This uses the user's own `claude` command; no extra program is involved. Run, once, with the message passed through a quoted heredoc (unchanged):

```bash
cloud_msg="$(cat <<'CLOUD_SESSIONS_EOF'
<the rest of the arguments, unchanged>
CLOUD_SESSIONS_EOF
)"
claude -p "$cloud_msg" --cloud <SESSION_ID> --output-format json
```

Arguments: $ARGUMENTS

The command prints `{"ok":true,"session_id":…,"url":…}` or `{"ok":false,…,"error":…}`. Report which, with the URL or the error. The message runs more cloud work and is billed like any cloud session; do not send anything the user did not write, and do not send twice.
