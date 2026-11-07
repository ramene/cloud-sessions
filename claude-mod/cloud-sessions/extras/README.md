# Extras

`mcp.json` declares the optional MCP server (`cloud-sessions mcp`: `cloud_start`, `cloud_send`, `cloud_events`, `cloud_track`,
`cloud_untrack`, `cloud_list`, `cloud_routines`). It is not enabled by default so that nothing is downloaded or started when Claude Code
launches. To use it, copy the entry into your own MCP settings and point `command` at the program the launcher installed
(or at `scripts/cloud-sessions mcp`). Claude Code asks before calling its tools; do not pre-approve `cloud_start` or `cloud_send`.
