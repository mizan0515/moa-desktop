import type { ClaimLedgerEntry } from "../lib/synthesisTypes";

const MAX_CLAIMS = 5;

export interface ClaimLedgerProps {
  entries?: ClaimLedgerEntry[];
}

export default function ClaimLedger({ entries = [] }: ClaimLedgerProps) {
  const visible = entries.slice(0, MAX_CLAIMS);

  return (
    <section className="results-section claim-ledger">
      <h4>Claim Ledger {entries.length > 0 ? `(${visible.length}/${entries.length})` : ""}</h4>
      {visible.length === 0 ? (
        <p className="synthesis-empty">no claims yet</p>
      ) : (
        <ol className="claim-list">
          {visible.map((c, i) => (
            <li key={i} className="claim-row">
              <span className="claim-text">{c.claim}</span>
              <span className="claim-evidence" title="evidence">
                {c.evidence || "—"}
              </span>
              <span className={`claim-level lvl-${c.level}`}>{c.level}</span>
              <span className={`claim-conf conf-${c.confidence}`}>{c.confidence}</span>
              {c.residualRisk ? (
                <span className="claim-risk" title="residual risk">
                  risk: {c.residualRisk}
                </span>
              ) : null}
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
