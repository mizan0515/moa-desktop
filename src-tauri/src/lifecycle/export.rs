use std::fs;
use std::path::{Path, PathBuf};

use crate::lifecycle::ResumePacket;

pub fn write_resume_packet(packet: &ResumePacket, output_dir: &Path) -> Result<PathBuf, String> {
    packet.validate()?;
    fs::create_dir_all(output_dir).map_err(|e| format!("resume mkdir: {e}"))?;
    let json_path = output_dir.join("resume.json");
    let md_path = output_dir.join("resume.md");
    let json = serde_json::to_string_pretty(packet).map_err(|e| format!("resume json: {e}"))?;
    fs::write(&json_path, json).map_err(|e| format!("resume write json: {e}"))?;
    fs::write(&md_path, render_markdown(packet)).map_err(|e| format!("resume write md: {e}"))?;
    Ok(json_path)
}

pub fn render_markdown(packet: &ResumePacket) -> String {
    format!(
        "# ResumePacket\n\n- session_id: `{}`\n- project_id: `{}`\n- branch: `{}`\n- worktree_path: `{}`\n- current_step: `{}`\n- last_phase: `{}`\n- primary_role: `{}`\n- version_pin: `{}`\n\n## Open Questions\n{}\n",
        packet.session_id,
        packet.project_id,
        packet.branch,
        packet.worktree_path.display(),
        packet.current_step,
        packet.last_phase,
        packet.primary_role.as_str(),
        packet.version_pin,
        packet
            .open_questions
            .iter()
            .map(|q| format!("- {q}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::ResumePacket;
    use tempfile::tempdir;

    #[test]
    fn write_resume_packet_creates_json_and_markdown() {
        let td = tempdir().unwrap();
        let mut packet = ResumePacket::minimal("s1", "p1");
        packet.open_questions.push("next?".into());
        let path = write_resume_packet(&packet, td.path()).unwrap();
        assert!(path.exists());
        assert!(td.path().join("resume.md").exists());
    }
}
