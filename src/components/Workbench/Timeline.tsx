// T7-thin — phase timeline for the active dry-run session.
import { useSyncExternalStore } from "react";
import { dryRunStore, PHASE_ORDER } from "../../lib/orchestrator/dryRun";

export default function Timeline() {
  useSyncExternalStore(dryRunStore.subscribe, dryRunStore.getSnapshot);
  const sess = dryRunStore.getActive();

  return (
    <div className="progress-track">
      {PHASE_ORDER.map((p) => {
        const ps = sess?.phases[p];
        const cls = ps
          ? ps.status === "active"
            ? "progress-step active"
            : ps.status === "done"
              ? "progress-step done"
              : ps.status === "error"
                ? "progress-step error"
                : "progress-step"
          : "progress-step";
        return (
          <span key={p} className={cls}>
            {p}
          </span>
        );
      })}
      {sess && (
        <span style={{ marginLeft: 8, fontSize: 11, color: "var(--fg-2)" }}>
          status: <strong>{sess.status}</strong>
        </span>
      )}
    </div>
  );
}
