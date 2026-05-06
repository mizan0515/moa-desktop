// T1 placeholder, wired by T7-thin to drive the dry-run orchestrator.
import { useState } from "react";
import RunButton from "./RunButton";
import Timeline from "./Timeline";

export default function TaskInput() {
  const [task, setTask] = useState("");

  return (
    <form className="task-input-form" onSubmit={(e) => e.preventDefault()}>
      <textarea
        placeholder="Describe what you want both AIs to do…"
        value={task}
        onChange={(e) => setTask(e.target.value)}
      />
      <div className="task-input-row">
        <RunButton task={task} />
        <span style={{ color: "var(--fg-2)", fontSize: 12 }}>
          Flow: <strong>dry-run (mock)</strong>
        </span>
      </div>
      <Timeline />
    </form>
  );
}
