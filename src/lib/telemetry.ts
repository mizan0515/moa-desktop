/**
 * T9 — telemetry types + pure helpers.
 *
 * Mirrors `src-tauri/src/telemetry/*`. Wire format is JSON over Tauri invoke;
 * field naming is snake_case to match serde defaults.
 *
 * Estimates only — Anthropic billing is the source of truth. UI must say so.
 */
import type { ProcessErrorKind } from "./processEvents";

export interface Usage {
  input: number;
  output: number;
  cache_read: number;
  cache_create: number;
}

export interface SessionTelemetry {
  claude: Usage;
  codex: Usage;
  claude_usd: number;
  codex_usd: number;
}

export type Worker = "claude" | "codex";

export interface AggKey {
  project_id: string;
  session_id: string;
}

export interface CostCap {
  per_session_usd: number;
  daily_usd: number;
  /** Soft warning threshold as a fraction of either cap (0..1). */
  warn_at: number;
}

export const DEFAULT_COST_CAP: CostCap = {
  per_session_usd: 10,
  daily_usd: 30,
  warn_at: 0.8,
};

export type CapStatus = "ok" | "warn" | "exceeded";

export interface DriftItem {
  field: string;
  previous: string | null;
  current: string | null;
}

export interface VersionSnapshot {
  claude_cli: string | null;
  codex_cli: string | null;
  app: string;
  plugin: string | null;
  captured_at: string;
}

export const EMPTY_USAGE: Usage = {
  input: 0,
  output: 0,
  cache_read: 0,
  cache_create: 0,
};

export const EMPTY_SESSION_TELEMETRY: SessionTelemetry = {
  claude: { ...EMPTY_USAGE },
  codex: { ...EMPTY_USAGE },
  claude_usd: 0,
  codex_usd: 0,
};

export function totalTokens(t: SessionTelemetry): number {
  const u = t.claude;
  const v = t.codex;
  return (
    u.input + u.output + u.cache_read + u.cache_create + v.input + v.output + v.cache_read + v.cache_create
  );
}

export function totalUsd(t: SessionTelemetry): number {
  return t.claude_usd + t.codex_usd;
}

/**
 * Evaluate cap status given a session-level USD and a global daily USD.
 * Mirrors `src-tauri/src/telemetry/cost.rs::evaluate_cap`.
 */
export function evaluateCap(sessionUsd: number, dailyUsd: number, cap: CostCap = DEFAULT_COST_CAP): CapStatus {
  if (sessionUsd >= cap.per_session_usd || dailyUsd >= cap.daily_usd) return "exceeded";
  const sessionRatio = sessionUsd / Math.max(cap.per_session_usd, Number.EPSILON);
  const dailyRatio = dailyUsd / Math.max(cap.daily_usd, Number.EPSILON);
  if (sessionRatio >= cap.warn_at || dailyRatio >= cap.warn_at) return "warn";
  return "ok";
}

/**
 * Render a process error kind as actionable user-facing copy.
 *
 * Returned shape is `{ title, detail, remedy }` so the banner can lay out
 * each piece independently.
 */
export interface ErrorAdvice {
  title: string;
  detail: string;
  /** A short imperative remediation step, e.g. "Run `gh auth refresh`". */
  remedy: string;
}

export function adviceForErrorKind(kind: ProcessErrorKind): ErrorAdvice {
  switch (kind) {
    case "cli-missing":
      return {
        title: "CLI not found",
        detail: "The Claude or Codex CLI binary could not be spawned. PATH may be missing the install dir.",
        remedy: "Verify `claude.exe` / `codex.exe` are installed and on PATH.",
      };
    case "permission-denied":
      return {
        title: "Permission denied",
        detail: "The OS refused to spawn the CLI binary (EACCES). It may be blocked by AV/EDR, lack the execute bit, or live on a non-executable path.",
        remedy: "Check file permissions / antivirus quarantine for the CLI binary and retry.",
      };
    case "spawn":
      return {
        title: "Spawn failed",
        detail: "The OS rejected the spawn for an unclassified reason (not missing, not permission). See Logs for the underlying io::Error.",
        remedy: "Inspect the error detail; common causes are too many open processes or a corrupt binary.",
      };
    case "auth-expired":
      return {
        title: "Authentication expired",
        detail: "Your session token is no longer valid. The worker stopped before any work was done.",
        remedy: "Run `claude login` (Claude) or `codex login` (Codex) and retry.",
      };
    case "quota":
      return {
        title: "Quota exhausted",
        detail: "The provider rejected the request — your subscription / API quota is at its cap.",
        remedy: "Wait for the next billing window or raise your plan.",
      };
    case "network":
      return {
        title: "Network error",
        detail: "The request to the provider failed before a response arrived.",
        remedy: "Check your internet connection, then retry.",
      };
    case "sandbox-denied":
      return {
        title: "Sandbox denied a tool call",
        detail: "The worker tried to perform an action outside the allowed sandbox (write outside worktree, MCP tool, etc.).",
        remedy: "If the action is expected, broaden the worker's allowed-tools list (T5a/T5b config).",
      };
    case "malformed-json":
      return {
        title: "Worker emitted malformed output",
        detail: "A line on stdout was not valid JSON. The CLI may have crashed mid-stream.",
        remedy: "Check the Logs pane for context, then retry. If repeated, file an issue.",
      };
    case "timeout":
      return {
        title: "Worker timed out",
        detail: "The worker did not finish before its deadline. The orchestrator killed it.",
        remedy: "Retry with a smaller scope, or extend the per-worker timeout in Settings.",
      };
    case "oom":
      return {
        title: "Worker ran out of memory",
        detail: "The OS killed the worker (Windows STATUS_NO_MEMORY).",
        remedy: "Close other memory-heavy apps, or split the task into smaller pieces.",
      };
    case "killed":
      return {
        title: "Worker was cancelled",
        detail: "Either you pressed Stop, or the orchestrator aborted the run (timeout, sibling failure).",
        remedy: "If unintended, retry. The process tree was killed cleanly (T2 verified).",
      };
    case "test-fail":
      return {
        title: "Worker tests failed",
        detail: "The worker ran successfully but its post-mutation test command exited non-zero.",
        remedy: "Open Logs and inspect the failing test output. Synthesis will not advance until tests pass.",
      };
    default: {
      const exhaustive: never = kind;
      return {
        title: "Unknown error",
        detail: `Unhandled error kind: ${exhaustive as string}`,
        remedy: "File an issue with the Logs pane contents.",
      };
    }
  }
}

/** Format a USD amount for compact display. */
export function fmtUsd(n: number): string {
  if (n < 0.01) return "< $0.01";
  return `$${n.toFixed(2)}`;
}

/** Format an integer token count with thousands separators. */
export function fmtTokens(n: number): string {
  return n.toLocaleString();
}
