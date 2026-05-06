// owned by T9 — dismissible warning that the toolchain has drifted between
// sessions. Driven by `detect_drift` (Rust) → `DriftItem[]` over Tauri.
import { useState } from "react";
import type { DriftItem } from "../lib/telemetry";

export interface VersionDriftWarningProps {
  items: DriftItem[];
}

export default function VersionDriftWarning({ items }: VersionDriftWarningProps) {
  const [dismissed, setDismissed] = useState(false);
  if (dismissed || items.length === 0) return null;
  return (
    <div className="version-drift" role="status" data-testid="version-drift">
      <div className="version-drift-head">
        <strong>Toolchain version drift</strong>
        <button
          type="button"
          className="version-drift-dismiss"
          onClick={() => setDismissed(true)}
          aria-label="Dismiss drift warning"
        >
          ×
        </button>
      </div>
      <p>
        One or more CLI / app versions changed since the last recorded session. Behavior may
        differ.
      </p>
      <ul>
        {items.map((it) => (
          <li key={it.field}>
            <code>{it.field}</code>: {fmtVersion(it.previous)} → {fmtVersion(it.current)}
          </li>
        ))}
      </ul>
    </div>
  );
}

function fmtVersion(v: string | null | undefined): string {
  return v == null ? "(missing)" : v;
}
