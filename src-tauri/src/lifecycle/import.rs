use std::fs;
use std::path::Path;

use crate::lifecycle::ResumePacket;

pub fn read_resume_packet(path: &Path) -> Result<ResumePacket, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("resume read: {e}"))?;
    let packet: ResumePacket =
        serde_json::from_str(&text).map_err(|e| format!("resume parse: {e}"))?;
    packet.validate()?;
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_resume_packet_roundtrips() {
        let td = tempdir().unwrap();
        let packet = ResumePacket::minimal("s1", "p1");
        let text = serde_json::to_string(&packet).unwrap();
        let path = td.path().join("resume.json");
        std::fs::write(&path, text).unwrap();
        let out = read_resume_packet(&path).unwrap();
        assert_eq!(out.session_id, "s1");
    }
}
