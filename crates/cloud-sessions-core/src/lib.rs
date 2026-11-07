//! Cloud sessions core: the mechanisms for driving Claude Code cloud sessions, with no UI and no Tauri.
//!
//! * [`cli::start`]: `claude --cloud "<prompt>"` in a pseudo-terminal (the CLI refuses without a TTY).
//! * [`cli::send`]: `claude -p "<message>" --cloud <id> --output-format json`.
//! * [`api::Client`]: `GET /v1/code/sessions/<id>/events` with a bearer token (the token comes from the
//!   caller; this crate never stores one), plus two UNOFFICIAL, opt-in calls: `list` and `stream`.
//! * [`events`]: turns the condensed event log into clean, bounded, untrusted text entries.
//! * [`cast`]: events as an asciicast v2 recording.
//! * [`credentials`]: token-file and opt-in Claude-login credential readers (never printed, never stored).
//! * [`policy`]: repository allowlist, session-id validation, send rate limit.
//!
//! Event text comes from a remote run that may have read hostile content. Nothing here executes it.
pub mod api;
pub mod cast;
pub mod endcard;
pub mod cli;
pub mod credentials;
pub mod events;
pub mod policy;

pub type Result<T> = std::result::Result<T, String>;

pub use api::{Client, Page};
pub use cli::{Started, StartStatus};
pub use events::Event;
pub use policy::parse_session_id;
