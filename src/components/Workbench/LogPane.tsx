// T1 placeholder, wired by T7-thin to stream live dry-run events.
import { useSyncExternalStore } from "react";
import { dryRunStore } from "../../lib/orchestrator/dryRun";
import { redact } from "../../lib/redact";

export default function LogPane() {
  useSyncExternalStore(dryRunStore.subscribe, dryRunStore.getSnapshot);
  const sess = dryRunStore.getActive();

  if (!sess || sess.logs.length === 0) {
    return (
      <p style={{ color: "var(--fg-2)", fontSize: 12 }}>
        Logs will stream here once a session is running.
      </p>
    );
  }

  return (
    <div>
      {sess.logs.map((l, i) => (
        <div key={i} className="log-row">
          <span className="log-ts">{l.ts}</span>
          <span className={`log-lane ${l.lane}`}>{l.lane}</span>
          <span className="log-msg">{redact(l.msg)}</span>
        </div>
      ))}
    </div>
  );
}
