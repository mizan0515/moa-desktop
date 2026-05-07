# T20-GATE: AppHandle Integration Harness

Status: ready for review gate
GitHub: #20

## Goal

Provide the minimal AppHandle-bearing integration test harness needed before T13 policy lifecycle work.

## Windows Loader Finding

The initial AppHandle test binary failed before the Rust test harness could list tests:

```text
exit code 0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND
```

The failure was narrowed to adding `tauri::Builder::default()` or Tauri's mock AppHandle runtime to an integration test binary. Plain Rust tests and tests importing the `tauri::AppHandle` type could list successfully.

The failing binaries imported common-controls entrypoints from `comctl32.dll`, including `SetWindowSubclass` / related Tauri-Wry windowing symbols. Cargo-built test executables do not automatically inherit the normal Tauri application manifest, so Windows can resolve common-controls without v6 activation and terminate during process startup.

## Prevention

`src-tauri/build.rs` embeds a Windows common-controls v6 manifest for Cargo test targets via `cargo:rustc-link-arg-tests`. This keeps AppHandle-bearing test binaries from failing at loader startup before `--list`.

## Harness Coverage

- AppHandle mock runtime boot/drop.
- Command dispatch through Tauri IPC using a mock command.
- Mock event emission on `orch://event`.
- Cleanup observation fixture for child process abort, journal close, and lock release ordering.

## Boundaries

- No real Claude CLI call.
- No real Codex CLI call from product/test harness/runtime code.
- No Claude/Codex MCP or AI peer runtime call.
- No production orchestrator rewrite.
- No T13 policy implementation.
