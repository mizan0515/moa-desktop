// T1 placeholder, wired by T7-thin to list real dry-run sessions.
import { useSyncExternalStore } from "react";
import { dryRunStore } from "../../lib/orchestrator/dryRun";

export default function SessionList() {
  useSyncExternalStore(dryRunStore.subscribe, dryRunStore.getSnapshot);
  const { sessions, activeSessionId } = dryRunStore.getSnapshot();

  if (sessions.length === 0) {
    return (
      <p style={{ color: "var(--fg-2)", fontSize: 12 }}>
        No sessions yet — run a task.
      </p>
    );
  }

  return (
    <ul className="session-list">
      {sessions.map((s) => (
        <li
          key={s.id}
          className={s.id === activeSessionId ? "active" : undefined}
          onClick={() => dryRunStore.setActive(s.id)}
          title={s.id}
        >
          <div style={{ fontSize: 13, lineHeight: 1.2 }}>
            {s.task.slice(0, 60) || "(empty task)"}
          </div>
          <div style={{ fontSize: 11, color: "var(--fg-2)" }}>
            {s.status} · {s.currentPhase}
          </div>
        </li>
      ))}
    </ul>
  );
}
