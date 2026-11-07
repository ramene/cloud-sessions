# Tiers — free mod, paid services (design, baked in from 0.1.x)

The mod is MIT. Anything in this repo can be changed by anyone, so **paid features are services we run, not locks in the code**. That keeps the license honest and the business real.

| Tier | What you get | Where it runs |
|---|---|---|
| **Free (this repo)** | start, send, track, list, read, watch (polling), journal, MCP tools | your machine + your own Claude login |
| **Pro (service)** | hosted live dashboard of all your cloud sessions (streaming), history + search across sessions, notifications when a session finishes, signed receipts of every send/read (verifiable cloud work) | our hosted service, behind a sign-in |
| **Team (service)** | shared session boards, who-sent-what audit, approval before a teammate's message is sent, org-wide usage view | same, per organization |

## How it plugs in
- The CLI and plugin stay fully functional offline. A paid account adds a **sync target**: `cloud-sessions connect` (future) signs in (passkey/FIDO2) and gives the CLI a short-lived token to push session metadata + receipts to the service. Nothing is sent without that explicit connect.
- **Receipts**: each `send` and `read` produces a local Ed25519-signed receipt (session id, action, time, hash of the message/events). Free users keep them locally; Pro syncs and verifies them in the dashboard.
- **Metering**: our proxy meters service usage (dashboard sessions, stored history), never the user's Claude usage.
- **Sponsorship** (optional, free tier only, off by default): a clearly labeled sponsor card in the hosted dashboard — never inside the CLI output, never in paid tiers.

## Not doing
- No license checks or obfuscated gates in the MIT code.
- No collection of the user's prompts or session content unless they connect a paid account and opt in.
