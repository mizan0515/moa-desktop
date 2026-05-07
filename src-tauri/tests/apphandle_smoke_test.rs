#[path = "../src/test_support/mod.rs"]
mod test_support;

#[test]
fn apphandle_mock_runtime_boots_and_drops() {
    let harness = test_support::AppHandleHarness::new();
    let _handle = harness.handle();
    drop(harness);
}

#[test]
fn apphandle_drop_records_cleanup_observations() {
    let harness = test_support::AppHandleHarness::new();
    let cleanup = harness.cleanup_observer();

    drop(harness);

    let events = cleanup.snapshot();
    assert_eq!(
        events,
        vec!["child_process_abort", "journal_close", "lock_release"]
    );
}
