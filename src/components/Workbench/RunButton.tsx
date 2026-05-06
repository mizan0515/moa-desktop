// T7-thin — Run / Cancel control wired to the dryRun store.
import { useSyncExternalStore } from "react";
import { dryRunStore } from "../../lib/orchestrator/dryRun";

interface Props {
  task: string;
  onStarted?: (sessionId: string) => void;
}

export default function RunButton({ task, onStarted }: Props) {
  useSyncExternalStore(dryRunStore.subscribe, dryRunStore.getSnapshot);
  const active = dryRunStore.getActive();
  const isRunning = active?.status === "running";

  async function handleRun() {
    if (!task.trim()) return;
    const sid = await dryRunStore.start(task.trim());
    onStarted?.(sid);
  }

  async function handleCancel() {
    if (active && isRunning) await dryRunStore.cancel(active.id);
  }

  return (
    <>
      <button type="button" onClick={handleRun} disabled={isRunning}>
        {isRunning ? "Running…" : "Run dry-run"}
      </button>
      <button type="button" onClick={handleCancel} disabled={!isRunning}>
        Cancel
      </button>
    </>
  );
}
