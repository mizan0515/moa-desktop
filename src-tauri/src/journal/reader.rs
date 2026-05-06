//! Read journal back into entries. Tolerates a torn last line (treats it as
//! "lost", per durability policy in `writer.rs`).

use std::path::Path;

use super::schema::Entry;
use super::writer::JournalError;

pub fn read_all(path: &Path) -> Result<Vec<Entry>, JournalError> {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Entry>(line) {
            Ok(e) => out.push(e),
            Err(_) => {
                // Torn write at tail — stop, do not propagate. Earlier good
                // lines remain valid.
                break;
            }
        }
    }
    Ok(out)
}
