use std::path::PathBuf;
use std::sync::Arc;

use tauri::Manager;

pub mod adapters;
pub mod cancel;
pub mod commands;
pub mod decomposer;
pub mod git;
pub mod integrator;
pub mod journal;
pub mod lifecycle;
pub mod lock;
pub mod mock;
pub mod orchestrator;
pub mod parallel;
pub mod pi;
pub mod policy;
pub mod process;
pub mod safety;
pub mod settings;
pub mod synthesis;
pub mod telemetry;
pub mod util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }

    let mut builder = builder
        .manage(orchestrator::dryrun::DryRunCoordinator::new())
        .manage(orchestrator::OrchestrationCoordinator::new())
        // FIX-F — register T9 telemetry store as managed state so future
        // adapter/usage callbacks can record into it without recreating an
        // ad-hoc instance per session. Cap eviction is unnecessary: the
        // store keys by (project, session) and a single app run aggregates
        // at most O(N_sessions) entries, well below any sensible ceiling.
        .manage(Arc::new(telemetry::TelemetryStore::new()));

    // T7-full deps — best-effort. If prompt templates are missing (dev env
    // without `prompts/workers/`), `orch_start` returns an error pointing
    // the user at the missing files; `orch_cancel`/state/etc still work.
    if let Some(deps) = build_orch_deps() {
        builder = builder.manage(Arc::new(deps));
    }

    builder
        .invoke_handler(tauri::generate_handler![
            orchestrator::dryrun::dryrun_start,
            orchestrator::dryrun::dryrun_ack,
            orchestrator::dryrun::dryrun_cancel,
            orchestrator::orch_start,
            orchestrator::orch_ack,
            orchestrator::orch_cancel,
            orchestrator::orch_submit_synthesis,
            orchestrator::orch_confirm_mutation,
            orchestrator::orch_get_state,
            settings::settings_load,
            settings::settings_save,
            commands::slash_dispatch_preview,
            commands::slash_confirm_step,
            commands::slash_confirm_step_with_review,
            commands::review_remember_run,
            safety::command_guard::worker_command_guard_check,
            lifecycle::export_resume_packet,
            lifecycle::import_resume_packet,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Construct `OrchestrationDeps` from sensible defaults. Returns `None` if
/// templates are missing — orchestrator commands will surface a clear
/// error instead of crashing app startup.
fn build_orch_deps() -> Option<orchestrator::OrchestrationDeps> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(manifest);
    let prompts_dir = repo_root.join("prompts").join("workers");

    let claude_program = util::which::which("claude").unwrap_or_else(|| PathBuf::from("claude"));
    let codex_program = util::which::codex_native_exe()?;

    let claude_config =
        adapters::claude::ClaudeConfig::from_dir(claude_program, &prompts_dir, "claude-opus-4-7")
            .ok()?;
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs_home().map(|h| h.join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"));
    let codex_config =
        adapters::codex::CodexConfig::from_dir(codex_program, &prompts_dir, codex_home).ok()?;

    let real_runner: Arc<dyn process::ProcessRunner> = Arc::new(process::TokioProcessRunner::new());
    // Default mock runner targets a no-op file; real mock_mode picks per-task
    // files in the orchestrator. T8 §B: passing a missing path makes the
    // runner emit zero lines and exit 0, which is the desired no-op default.
    let mock_runner: Arc<dyn process::ProcessRunner> = Arc::new(mock::MockRunner::new(
        repo_root.join("mockResponses").join("noop.json"),
    ));

    // FIX-F — journal base dir defaults to `~/.moa-desktop`. Falls back to
    // a process-local temp subdir if HOME is unresolvable so tests / CI
    // never crash spawn time on an absent env var.
    let journal_base_dir = dirs_home()
        .map(|h| h.join(".moa-desktop"))
        .or_else(|| Some(std::env::temp_dir().join("moa-desktop")));

    Some(orchestrator::OrchestrationDeps {
        real_runner,
        mock_runner,
        lock_manager: lock::manager::LockManager::new(),
        claude_config,
        codex_config,
        journal_base_dir,
    })
}

fn dirs_home() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
