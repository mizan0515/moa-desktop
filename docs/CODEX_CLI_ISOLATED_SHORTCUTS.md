# Isolated Codex CLI Desktop Shortcuts

These helper scripts create a desktop shortcut that launches Codex CLI with an
isolated `CODEX_HOME` and a folder picker.

`moa-desktop` is public, so the scripts do not embed private machine paths or
credentials. For a full isolated profile with local skills, clone the private
snapshot repo next to this repo:

```text
<parent>/
  moa-desktop/
  codex-moa-isolated-env-snapshots/
```

The launchers automatically prefer
`../codex-moa-isolated-env-snapshots/codex-home` when it exists. You can also
pass an explicit isolated profile path.

## Windows

From this repo:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\New-CodexCliDesktopShortcut.ps1 -Force
```

With an explicit isolated profile:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\New-CodexCliDesktopShortcut.ps1 -CodexHomePath <path-to-isolated-codex-home> -Force
```

For a dry run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\New-CodexCliDesktopShortcut.ps1 -PrintCommandOnly
```

The shortcut runs `scripts\Start-CodexCliIsolatedPicker.ps1`, defaults to
`workspace-write` plus `on-request`, and enables the experimental `goals`
feature. `-Mode Automation` exists for externally isolated automation runs and
uses `danger-full-access` plus `never`.

## macOS

From this repo:

```bash
./scripts/new-codex-cli-desktop-shortcut.macos.sh --force
```

With an explicit isolated profile:

```bash
./scripts/new-codex-cli-desktop-shortcut.macos.sh --codex-home <path-to-isolated-codex-home> --force
```

For a dry run:

```bash
./scripts/new-codex-cli-desktop-shortcut.macos.sh --print-command-only
```

The installer writes `~/Desktop/Codex CLI (Isolated - Folder Picker).command`.
The launcher uses AppleScript folder selection when available and falls back to a
terminal prompt. If Codex CLI is not discoverable from a GUI-launched shell,
pass `--codex-path <path-to-codex>` when installing.

## Safety

The launchers refuse to use host-global Codex, Claude, or SSH
profile/credential folders as workspaces or `CODEX_HOME`.

In Codex CLI, local skills are model-visible instructions. Trigger them with
plain text such as `parallel-ticket-planner`, `easy-briefing`, `ticket-review`,
or the Korean trigger phrases. Leading `/` is reserved for CLI slash commands and
may not invoke repo-local skills.
