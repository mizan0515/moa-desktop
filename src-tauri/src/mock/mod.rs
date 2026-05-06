//! T8 — Mock mode. File-backed `ProcessRunner` that streams canned worker
//! output for `settings.mockMode = true`. Used by dry-run flows and tests so
//! the orchestrator (T7) and adapters (T5) can exercise the full pipeline
//! without spawning real Claude / Codex CLIs.
//!
//! Canned files live under `<repo>/mockResponses/` (six JSONL files matching
//! the worker schema: claude_firstpass, codex_firstpass, synthesis,
//! claude_adversarial, codex_adversarial, final_report).

pub mod runner;

pub use runner::{MockRunner, DEFAULT_LINE_DELAY};
