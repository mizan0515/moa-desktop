#![allow(dead_code)]

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, AppHandle, Listener, WebviewWindow, WebviewWindowBuilder};

pub type TestApp = App<MockRuntime>;
pub type TestAppHandle = AppHandle<MockRuntime>;

const EVENT_TIMEOUT: Duration = Duration::from_secs(2);

pub fn mock_app() -> TestApp {
    mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock Tauri app should build")
}

pub fn mock_app_with_handler<F>(handler: F) -> TestApp
where
    F: Fn(tauri::ipc::Invoke<MockRuntime>) -> bool + Send + Sync + 'static,
{
    mock_builder()
        .invoke_handler(handler)
        .build(mock_context(noop_assets()))
        .expect("mock Tauri app with invoke handler should build")
}

pub fn main_webview(app: &TestApp) -> WebviewWindow<MockRuntime> {
    WebviewWindowBuilder::new(app, "main", Default::default())
        .build()
        .expect("mock webview should build")
}

pub fn invoke_request(cmd: &str, body: InvokeBody) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body,
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

pub fn invoke_json<T: DeserializeOwned>(
    webview: &WebviewWindow<MockRuntime>,
    cmd: &str,
    body: InvokeBody,
) -> T {
    tauri::test::get_ipc_response(webview, invoke_request(cmd, body))
        .expect("IPC command should return ok")
        .deserialize()
        .expect("IPC response should deserialize")
}

pub struct EventProbe {
    rx: mpsc::Receiver<Value>,
}

impl EventProbe {
    pub fn listen(app: &TestApp, event: &'static str) -> Self {
        let (tx, rx) = mpsc::sync_channel(8);
        app.listen_any(event, move |event| {
            let payload = serde_json::from_str(event.payload()).unwrap_or(Value::Null);
            let _ = tx.send(payload);
        });
        Self { rx }
    }

    pub fn recv(&self) -> Value {
        self.rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("expected Tauri event payload")
    }
}

#[derive(Clone, Default)]
pub struct CleanupObserver {
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl CleanupObserver {
    pub fn record_child_abort(&self) {
        self.record("child_process_abort");
    }

    pub fn record_journal_close(&self) {
        self.record("journal_close");
    }

    pub fn record_lock_release(&self) {
        self.record("lock_release");
    }

    pub fn snapshot(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }

    fn record(&self, event: &'static str) {
        self.events.lock().unwrap().push(event);
    }
}

pub struct AppHandleHarness {
    app: Option<TestApp>,
    cleanup: CleanupObserver,
}

impl AppHandleHarness {
    pub fn new() -> Self {
        Self {
            app: Some(mock_app()),
            cleanup: CleanupObserver::default(),
        }
    }

    pub fn handle(&self) -> TestAppHandle {
        self.app
            .as_ref()
            .expect("app should be alive")
            .handle()
            .clone()
    }

    pub fn cleanup_observer(&self) -> CleanupObserver {
        self.cleanup.clone()
    }
}

impl Drop for AppHandleHarness {
    fn drop(&mut self) {
        self.cleanup.record_child_abort();
        self.cleanup.record_journal_close();
        self.cleanup.record_lock_release();
        drop(self.app.take());
    }
}
