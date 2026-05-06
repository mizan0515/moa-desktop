// owned by T9 — actionable error banner.
//
// Driven by T2's `ProcessErrorKind` enum (kebab-case wire format). The banner
// renders nothing until an error is supplied so it is safe to mount
// unconditionally in `Results`.
import { adviceForErrorKind } from "../lib/telemetry";
import type { ProcessErrorKind } from "../lib/processEvents";

export interface ErrorBannerProps {
  kind?: ProcessErrorKind | null;
  /** Optional extra detail (e.g. raw stderr_tail) to disclose. */
  detail?: string;
  /** Called when the user dismisses the banner. */
  onDismiss?: () => void;
}

export default function ErrorBanner({ kind, detail, onDismiss }: ErrorBannerProps) {
  if (!kind) return null;
  const advice = adviceForErrorKind(kind);
  return (
    <div
      className="error-banner"
      role="alert"
      data-testid="error-banner"
      data-kind={kind}
    >
      <div className="error-banner-head">
        <strong>{advice.title}</strong>
        <span className="error-banner-kind">[{kind}]</span>
        {onDismiss && (
          <button
            type="button"
            className="error-banner-dismiss"
            onClick={onDismiss}
            aria-label="Dismiss error"
          >
            ×
          </button>
        )}
      </div>
      <p className="error-banner-detail">{advice.detail}</p>
      <p className="error-banner-remedy">
        <strong>Try:</strong> {advice.remedy}
      </p>
      {detail && (
        <details className="error-banner-raw">
          <summary>Raw output</summary>
          <pre>{detail}</pre>
        </details>
      )}
    </div>
  );
}
