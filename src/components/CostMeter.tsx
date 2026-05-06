// owned by T9 — current-project cost meter with cap status + cache-reuse note.
//
// Real telemetry is filled by the orchestrator (T7) via Tauri events; for v1
// this component exposes its render state through props with a sensible
// `EMPTY_SESSION_TELEMETRY` default so it renders the moment a project tab
// becomes active.
import { useState } from "react";
import { useProject } from "../lib/projectContext";
import {
  CapStatus,
  CostCap,
  DEFAULT_COST_CAP,
  EMPTY_SESSION_TELEMETRY,
  SessionTelemetry,
  evaluateCap,
  fmtTokens,
  fmtUsd,
  totalTokens,
  totalUsd,
} from "../lib/telemetry";

export interface CostMeterProps {
  /** Telemetry for the active project's current session (defaults to empty). */
  session?: SessionTelemetry;
  /** Aggregated telemetry across all sessions for the active project. */
  projectTotal?: SessionTelemetry;
  /** Aggregated telemetry across every project (for the All-projects toggle). */
  allTotal?: SessionTelemetry;
  /** Today's daily USD total (for cap evaluation). Defaults to project session USD. */
  dailyUsd?: number;
  /** Cost caps; defaults to ticket spec ($10/session, $30/day). */
  cap?: CostCap;
}

export default function CostMeter(props: CostMeterProps) {
  const { active } = useProject();
  const session = props.session ?? EMPTY_SESSION_TELEMETRY;
  const projectTotal = props.projectTotal ?? session;
  const allTotal = props.allTotal ?? projectTotal;
  const cap = props.cap ?? DEFAULT_COST_CAP;
  const [showAll, setShowAll] = useState(false);

  const view = showAll ? allTotal : projectTotal;
  const sessionUsd = totalUsd(session);
  const dailyUsd = props.dailyUsd ?? sessionUsd;
  const status = evaluateCap(sessionUsd, dailyUsd, cap);

  const cacheReadZero = view.claude.cache_read === 0 && view.codex.cache_read === 0;

  return (
    <section className="results-section cost-meter" data-testid="cost-meter">
      <h4>
        Cost <span className="cost-meter-est">(estimated)</span>
      </h4>
      <div className="cost-meter-toggle">
        <label>
          <input
            type="checkbox"
            checked={showAll}
            onChange={(e) => setShowAll(e.target.checked)}
          />{" "}
          All projects
        </label>
        <span className="cost-meter-scope">
          {showAll ? "all projects" : `project: ${active.title}`}
        </span>
      </div>
      <CapBanner status={status} sessionUsd={sessionUsd} dailyUsd={dailyUsd} cap={cap} />
      <table className="cost-meter-table">
        <thead>
          <tr>
            <th>worker</th>
            <th>input</th>
            <th>output</th>
            <th>cache_read</th>
            <th>cache_create</th>
            <th>USD</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>claude</td>
            <td>{fmtTokens(view.claude.input)}</td>
            <td>{fmtTokens(view.claude.output)}</td>
            <td>{fmtTokens(view.claude.cache_read)}</td>
            <td>{fmtTokens(view.claude.cache_create)}</td>
            <td>{fmtUsd(view.claude_usd)}</td>
          </tr>
          <tr>
            <td>codex</td>
            <td>{fmtTokens(view.codex.input)}</td>
            <td>{fmtTokens(view.codex.output)}</td>
            <td>{fmtTokens(view.codex.cache_read)}</td>
            <td>{fmtTokens(view.codex.cache_create)}</td>
            <td>
              {fmtUsd(view.codex_usd)}{" "}
              <span className="cost-meter-sub">(subscription)</span>
            </td>
          </tr>
          <tr>
            <td>
              <strong>total</strong>
            </td>
            <td colSpan={4}>{fmtTokens(totalTokens(view))} tokens</td>
            <td>
              <strong>{fmtUsd(totalUsd(view))}</strong>
            </td>
          </tr>
        </tbody>
      </table>
      {cacheReadZero && (
        <p className="cost-meter-note">
          <em>Note:</em> <code>cache_read = 0</code> is expected — every <code>claude -p</code>{" "}
          run is a fresh session, so prompt cache reuse is 0.
        </p>
      )}
    </section>
  );
}

function CapBanner({
  status,
  sessionUsd,
  dailyUsd,
  cap,
}: {
  status: CapStatus;
  sessionUsd: number;
  dailyUsd: number;
  cap: CostCap;
}) {
  if (status === "ok") return null;
  const isExceeded = status === "exceeded";
  return (
    <div
      className={isExceeded ? "cap-banner cap-exceeded" : "cap-banner cap-warn"}
      role={isExceeded ? "alert" : "status"}
      data-testid="cap-banner"
      data-status={status}
    >
      <strong>{isExceeded ? "Cost cap reached" : "Approaching cost cap"}:</strong>{" "}
      session {fmtUsd(sessionUsd)} / {fmtUsd(cap.per_session_usd)}, daily{" "}
      {fmtUsd(dailyUsd)} / {fmtUsd(cap.daily_usd)}.
      {isExceeded && " New runs require explicit confirmation."}
    </div>
  );
}
