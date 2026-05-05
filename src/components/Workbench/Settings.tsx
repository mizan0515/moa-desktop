export default function Settings() {
  return (
    <form className="settings-form" onSubmit={(e) => e.preventDefault()}>
      <h2 style={{ margin: 0 }}>Settings</h2>
      <p className="empty-note">
        Form fields land in T7/T8/T9. Stub here so navigation works.
      </p>
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
