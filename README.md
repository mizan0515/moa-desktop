# MoA Desktop

Tauri v2 + React + TypeScript desktop app that runs Claude Code and Codex CLI as sibling workers (no nested calls). The app drives the MoA flow — parallel read-only first-pass → 5-column synthesis → adversarial review → single-owner mutation in an isolated worktree → verification — and the user only acts at start (run) and end (apply patch). See `DESIGN.md` and `PLAN.md` for the full spec, and `TICKETS/` for ticket-by-ticket scope. T1 here ships only the shell, static workbench UI, and module stubs; live worker logic lands in T2/T5/T7.

## Develop

```
npm install
npm run tauri dev
```
