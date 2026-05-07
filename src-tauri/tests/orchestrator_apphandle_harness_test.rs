#[path = "../src/test_support/mod.rs"]
mod test_support;

use serde_json::json;
use tauri::ipc::InvokeBody;
use tauri::Emitter;

const EVENT: &str = "orch://event";

#[tauri::command]
fn t20_mock_dispatch(app: test_support::TestAppHandle) -> &'static str {
    app.emit(
        EVENT,
        json!({
            "sid": "t20",
            "phase": "Preflight",
            "lane": "System",
            "kind": "mock_dispatch"
        }),
    )
    .expect("mock AppHandle should emit command event");
    "ok"
}

#[test]
fn orchestrator_apphandle_command_dispatch_emits_mock_event() {
    let app = test_support::mock_app_with_handler(tauri::generate_handler![t20_mock_dispatch]);
    let probe = test_support::EventProbe::listen(&app, EVENT);
    let webview = test_support::main_webview(&app);

    let response: String =
        test_support::invoke_json(&webview, "t20_mock_dispatch", InvokeBody::default());
    assert_eq!(response, "ok");

    let event = probe.recv();
    assert_eq!(event["sid"], "t20");
    assert_eq!(event["kind"], "mock_dispatch");
}

#[test]
fn orchestrator_apphandle_cleanup_fixture_observes_drop_sequence() {
    let harness = test_support::AppHandleHarness::new();
    let cleanup = harness.cleanup_observer();
    let _handle = harness.handle();

    drop(harness);

    assert_eq!(
        cleanup.snapshot(),
        vec!["child_process_abort", "journal_close", "lock_release"]
    );
}
