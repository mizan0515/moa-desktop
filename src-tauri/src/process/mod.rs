//! T2 — generic CLI process runner.
//!
//! Public surface re-exported here. Adapters (T5a/T5b) and the orchestrator
//! (T7) only depend on these types — never on `runner` internals.

pub mod errors;
pub mod kill;
pub mod runner;
pub mod traits;

pub use errors::{redact_env_pair, ProcessError, ProcessErrorKind};
pub use runner::TokioProcessRunner;
pub use traits::{
    ProcessControl, ProcessExit, ProcessHandle, ProcessLine, ProcessRunner, ProcessSpec,
    StdinPolicy, Stream,
};
