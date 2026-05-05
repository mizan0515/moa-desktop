const dummySessions = [
  { id: "s1", title: "T1 scaffold review", state: "active" },
  { id: "s2", title: "Refactor synthesis merge", state: "idle" },
  { id: "s3", title: "Investigate Chzzk auth", state: "idle" },
];

export default function SessionList() {
  return (
    <ul className="session-list">
      {dummySessions.map((s) => (
        <li key={s.id} className={s.state === "active" ? "active" : undefined}>
          {s.title}
        </li>
      ))}
    </ul>
  );
}
