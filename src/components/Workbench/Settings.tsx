import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type PrimaryRole = "claude" | "codex";

type AppSettings = {
  primaryRole: PrimaryRole;
  policySyncMode: "manual" | "trusted-safe-auto";
};

export default function Settings() {
  const [settings, setSettings] = useState<AppSettings>({
    primaryRole: "claude",
    policySyncMode: "manual",
  });
  const [status, setStatus] = useState<string>("");

  useEffect(() => {
    invoke<AppSettings>("settings_load")
      .then(setSettings)
      .catch(() => setStatus("Settings unavailable in browser preview"));
  }, []);

  function save(next: AppSettings) {
    setSettings(next);
    window.localStorage.setItem("moa.settings", JSON.stringify(next));
    invoke<AppSettings>("settings_save", { settings: next })
      .then(setSettings)
      .then(() => setStatus("Applies to the next session"))
      .catch(() => setStatus("Settings unavailable in browser preview"));
  }

  return (
    <form className="settings-form" onSubmit={(e) => e.preventDefault()}>
      <h2 style={{ margin: 0 }}>Settings</h2>
      <p className="empty-note">{status || "Session policy is captured when a new session starts."}</p>
      <label>
        <span>Primary role</span>
        <select
          value={settings.primaryRole}
          onChange={(e) => save({ ...settings, primaryRole: e.target.value as PrimaryRole })}
        >
          <option value="claude">Claude</option>
          <option value="codex">Codex</option>
        </select>
      </label>
      <label>
        <span>Claude command</span>
        <input type="text" placeholder="claude" disabled />
      </label>
      <label>
        <span>Codex command</span>
        <input type="text" placeholder="codex" disabled />
      </label>
      <label>
        <span>Default flow</span>
        <select disabled>
          <option>auto</option>
        </select>
      </label>
      <label>
        <span>Cost cap (USD / session)</span>
        <input type="number" placeholder="10" disabled />
      </label>
    </form>
  );
}
