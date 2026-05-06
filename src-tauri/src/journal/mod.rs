//! T4 — recovery journal (per-(project, session) JSONL).

pub mod reader;
pub mod reconcile;
pub mod schema;
pub mod writer;

pub use reader::read_all;
pub use reconcile::{scan, UnfinishedSession};
pub use schema::{Entry, Phase};
pub use writer::{JournalError, JournalWriter};
