const dummyLogs = [
  { ts: "12:00:01", lane: "system", msg: "preflight ok (claude.exe, codex.exe resolved)" },
  { ts: "12:00:02", lane: "claude", msg: "first-pass start (read-only)" },
  { ts: "12:00:02", lane: "codex", msg: "first-pass start (sandbox=read-only)" },
  { ts: "12:00:18", lane: "claude", msg: "diagnosis emitted, 4 risks flagged" },
  { ts: "12:00:21", lane: "codex", msg: "diagnosis emitted, alternative B preferred" },
  { ts: "12:00:25", lane: "system", msg: "synthesis (5-column) ready" },
];

type Lane = "system" | "claude" | "codex";

export default function LogPane() {
  return (
    <div>
      {dummyLogs.map((l, i) => (
        <div key={i} className="log-row">
          <span className="log-ts">{l.ts}</span>
          <span className={`log-lane ${l.lane as Lane}`}>{l.lane}</span>
          <span className="log-msg">{l.msg}</span>
        </div>
      ))}
    </div>
  );
}
