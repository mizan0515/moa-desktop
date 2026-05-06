use tauri::Manager;

pub mod adapters;
pub mod cancel;
pub mod decomposer;
pub mod git;
pub mod integrator;
pub mod journal;
pub mod lock;
pub mod mock;
pub mod orchestrator;
pub mod parallel;
pub mod process;
pub mod safety;
pub mod settings;
pub mod synthesis;
pub mod telemetry;

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

    builder
        .manage(orchestrator::dryrun::DryRunCoordinator::new())
        .invoke_handler(tauri::generate_handler![
            orchestrator::dryrun::dryrun_start,
            orchestrator::dryrun::dryrun_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
