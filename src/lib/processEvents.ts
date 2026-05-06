/**
 * T2 — Process runner event types crossing the Tauri bridge.
 *
 * Mirrors `src-tauri/src/process/{traits,errors}.rs`. Adapters (T5a/T5b)
 * and the orchestrator (T7) consume these in the renderer.
 *
 * Wire format is `serde(rename_all = "kebab-case")` for the kind enum,
 * matching the Rust `Serialize` impl.
 */

export type ProcessErrorKind =
  | "cli-missing"
  | "auth-expired"
  | "quota"
  | "network"
  | "sandbox-denied"
  | "malformed-json"
  | "timeout"
  | "oom"
  | "killed"
  | "test-fail";

export type ProcessStream = "stdout" | "stderr";

export interface ProcessLine {
  seq: number;
  stream: ProcessStream;
  line: string;
  /**
   * `true` when this line was emitted by EOF-without-newline OR by a
   * force-split because the line exceeded `max_line_bytes`. Adapter parsers
   * must concatenate consecutive `partial:true` chunks of the same stream
   * before treating them as a complete line.
   */
  partial: boolean;
}

export interface ProcessExit {
  /** Real exit code if the process exited naturally; `null` if killed. */
  code: number | null;
  aborted: boolean;
  timedOut: boolean;
  /**
   * Last N bytes of stderr (cap negotiated by spec, default 64 KiB). Raw —
   * adapters need fidelity for protocol classification (e.g. matching
   * `auth-expired` patterns). NEVER pre-redacted — UI layers redact at
   * render time when displaying to the user.
   */
  stderrTail: string;
  /** Runner-classified kind, or null if classification is adapter responsibility. */
  kind: ProcessErrorKind | null;
}

export interface ProcessError {
  kind: ProcessErrorKind;
  message: string;
  exitCode: number | null;
  stderrTail: string;
}

/**
 * The supervisor emits these event variants over a Tauri channel/event. T7
 * orchestrator wraps them into per-worker streams.
 */
export type ProcessEvent =
  | { type: "started"; runId: string; pid: number; cwd: string }
  | ({ type: "line"; runId: string } & ProcessLine)
  | ({ type: "exit"; runId: string } & ProcessExit)
  | { type: "error"; runId: string; error: ProcessError };
