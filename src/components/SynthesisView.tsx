// T6 placeholder. T7-thin renders a minimal 5-column readout from the
// dryRun store so the Phase 1 demo is visible end-to-end. T6 will refine
// layout / styling without changing the data source.
import { useSyncExternalStore } from "react";
import { dryRunStore } from "../lib/orchestrator/dryRun";

const COLUMNS: Array<{ key: string; label: string }> = [
  { key: "verified", label: "Verified" },
  { key: "codex_only", label: "Codex-only" },
  { key: "claude_only", label: "Claude-only" },
  { key: "disagreement", label: "Disagreement" },
  { key: "open", label: "Open" },
];

export default function SynthesisView() {
  useSyncExternalStore(dryRunStore.subscribe, dryRunStore.getSnapshot);
  const sess = dryRunStore.getActive();
  const rows = sess?.synthesisRows ?? [];

  return (
    <section className="results-section">
      <h4>Synthesis (5 columns)</h4>
      {rows.length === 0 ? (
        <p>placeholder — runs will populate verified / Codex-only / Claude-only / disagreement / open.</p>
      ) : (
        <div style={{ display: "grid", gap: 8 }}>
          {COLUMNS.map((c) => {
            const colRows = rows.filter((r) => r.column === c.key);
            if (colRows.length === 0) return null;
            return (
              <div key={c.key}>
                <div style={{ fontSize: 11, color: "var(--fg-2)", textTransform: "uppercase" }}>
                  {c.label}
                </div>
                <ul style={{ margin: 0, paddingLeft: 16, fontSize: 13 }}>
                  {colRows.map((r, i) => (
                    <li key={i}>{describeRow(r.raw)}</li>
                  ))}
                </ul>
              </div>
            );
          })}
        </div>
      )}
      {sess?.finalReport.verdict && (
        <div style={{ marginTop: 8, fontSize: 12 }}>
          Verdict: <strong>{sess.finalReport.verdict}</strong>
        </div>
      )}
    </section>
  );
}

function describeRow(raw: Record<string, unknown>): string {
  if (typeof raw.claim === "string") return raw.claim;
  if (typeof raw.topic === "string") {
    const claudePos = (raw.claude_position as string) ?? "?";
    const codexPos = (raw.codex_position as string) ?? "?";
    return `${raw.topic} — claude: ${claudePos} | codex: ${codexPos}`;
  }
  if (typeof raw.question === "string") return raw.question;
  return JSON.stringify(raw);
}
