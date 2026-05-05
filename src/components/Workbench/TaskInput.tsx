const phases = [
  "preflight",
  "first-pass",
  "synthesis",
  "adversarial",
  "mutation",
  "verification",
  "final",
];

export default function TaskInput() {
  const activePhase = "first-pass";
  return (
    <form className="task-input-form" onSubmit={(e) => e.preventDefault()}>
      <textarea
        placeholder="Describe what you want both AIs to do…"
        defaultValue=""
      />
      <div className="task-input-row">
        <button type="submit">Run</button>
        <button type="button">Cancel</button>
        <span style={{ color: "var(--fg-2)", fontSize: 12 }}>
          Flow: <strong>auto (C)</strong>
        </span>
      </div>
      <div className="progress-track">
        {phases.map((p) => (
          <span
            key={p}
            className={p === activePhase ? "progress-step active" : "progress-step"}
          >
            {p}
          </span>
        ))}
      </div>
    </form>
  );
}
