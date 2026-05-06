// T6 placeholder. T7-thin renders the first-pass claims (max 5) so the
// demo loop produces something visible. T6 will replace with the proper
// load-bearing-only ledger UI.
import { useSyncExternalStore } from "react";
import { dryRunStore } from "../lib/orchestrator/dryRun";

export default function ClaimLedger() {
  useSyncExternalStore(dryRunStore.subscribe, dryRunStore.getSnapshot);
  const sess = dryRunStore.getActive();
  const claims = (sess?.claims ?? []).slice(0, 5);

  return (
    <section className="results-section">
      <h4>Claim Ledger</h4>
      {claims.length === 0 ? (
        <p>placeholder — first-pass claims (load-bearing only, max 5) will land here.</p>
      ) : (
        <ol style={{ margin: 0, paddingLeft: 18, fontSize: 13 }}>
          {claims.map((c, i) => (
            <li key={i}>
              <span>{c.text}</span>
              <span style={{ color: "var(--fg-2)", fontSize: 11, marginLeft: 6 }}>
                — {c.lane}
                {c.confidence ? ` · conf: ${c.confidence}` : ""}
                {c.citations && c.citations.length > 0
                  ? ` · ${c.citations.length} cite${c.citations.length === 1 ? "" : "s"}`
                  : ""}
              </span>
            </li>
          ))}
        </ol>
      )}
      {sess?.finalReport.recommendations.length ? (
        <div style={{ marginTop: 10 }}>
          <div style={{ fontSize: 11, color: "var(--fg-2)", textTransform: "uppercase" }}>
            Final recommendations
          </div>
          <ul style={{ margin: 0, paddingLeft: 18, fontSize: 13 }}>
            {sess.finalReport.recommendations.map((r, i) => (
              <li key={i}>{r.text}</li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
