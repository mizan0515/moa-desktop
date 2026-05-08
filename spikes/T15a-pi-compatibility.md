# T15a Pi Compatibility Spike

Issue: #37
Date: 2026-05-08
Scope: Windows/Tauri/MoA read-only compatibility spike for Pi CLI, RPC, SDK, package, extension UI, and hot reload surfaces.

## Ground Rules

- No Pi package install, package update, global install, or project-local `.pi/settings.json` trust was used.
- No production code was changed.
- Evidence is limited to npm metadata, local CLI availability, read-only smoke helpers, and project design constraints.

## Matrix

| Surface | Result | Evidence | Notes |
| --- | --- | --- | --- |
| npm metadata: `@earendil-works/pi-coding-agent` | PASS | `npm view @earendil-works/pi-coding-agent version name --json` returned `{"version":"0.74.0","name":"@earendil-works/pi-coding-agent"}`. | Current package name and version confirmed on 2026-05-08. |
| npm metadata: deprecated `@mariozechner/pi-coding-agent` | PASS | `npm view @mariozechner/pi-coding-agent version name deprecated --json` returned version `0.73.1` and deprecation text: `please use @earendil-works/pi-coding-agent instead going forward`. | Migration path confirmed through npm metadata. |
| Pi CLI availability | PASS | `pi --version` returned `0.74.0`. | CLI is installed in the current user PATH as an npm PowerShell shim at `%APPDATA%\npm\pi.ps1`. |
| Pi RPC mode flag | PASS | `pi --help` and `pi --mode rpc --no-session --help` list `--mode <mode>` with `rpc` as a valid output mode and `--no-session`. | Flag surface exists in the installed CLI. |
| Pi RPC process startup | UNVERIFIED | `spikes/T15a-pi-rpc-smoke.ps1` started `pi --mode rpc --no-session`, wrote bounded JSONL requests, then killed after 8s with no stdout/stderr. | Process launch works through the shim, but no JSONL protocol response was observed. |
| RPC command: `prompt` | UNVERIFIED | RPC smoke sent method `prompt`; no JSONL response before timeout. | T15b should keep this behind protocol discovery and timeout handling. |
| RPC command: `set_model` | UNVERIFIED | RPC smoke sent method `set_model`; no JSONL response before timeout. | T15b should treat unsupported command responses as capability findings, not adapter assumptions. |
| RPC command: `compact` | UNVERIFIED | RPC smoke sent method `compact`; no JSONL response before timeout. | T15b should map this as session/control capability only after observed protocol support. |
| RPC command: `abort` | UNVERIFIED | RPC smoke sent method `abort`; no JSONL response before timeout. | T15b should implement cancellation through process and request correlation safeguards. |
| RPC command: `get_state` | UNVERIFIED | RPC smoke sent method `get_state`; no JSONL response before timeout. | T15b should not depend on this until the response schema is observed. |
| Extension UI request: `confirm` | UNVERIFIED | Installed package exports extension UI types, but no live extension host was exercised. | T15e must route through MoA UI capability policy before showing prompts. |
| Extension UI request: `input` | UNVERIFIED | Installed package exports extension UI types, but no live extension host was exercised. | T15e must validate prompt origin and capability manifest. |
| Extension UI request: `select` | UNVERIFIED | Installed package exports extension UI types, but no live extension host was exercised. | T15e must avoid trusting package-provided labels as commands. |
| Extension UI request: `notify` | UNVERIFIED | Installed package exports extension UI types, but no live extension host was exercised. | T15e can map to informational events after policy approval. |
| Extension UI request: `setStatus` | UNVERIFIED | Installed package exports extension UI types, but no live extension host was exercised. | T15e should keep status lane-local and non-authoritative. |
| Extension UI request: `setWidget` | UNVERIFIED | Installed package exports extension UI types, but no live extension host was exercised. | T15e should sandbox widget content and avoid direct host privileges. |
| SDK export: `createAgentSession` | PASS | Installed package `dist/index.d.ts` exports `createAgentSession`; repo-local `node spikes/T15a-pi-sdk-smoke.mjs` returned `UNVERIFIED/package-not-installed`. | T15c can plan for this export, but dependency addition remains future work. |
| SDK export: `DefaultResourceLoader` | PASS | Installed package `dist/index.d.ts` exports `DefaultResourceLoader`; repo-local SDK smoke cannot import without an approved dependency. | Must be constrained to MoA-approved resource roots. |
| SDK export: `createEventBus` | PASS | Installed package `dist/index.d.ts` exports `createEventBus`; repo-local SDK smoke cannot import without an approved dependency. | Event bridge should be typed and capability-gated. |
| SDK export: `ModelRegistry` | PASS | Installed package `dist/index.d.ts` exports `ModelRegistry`; repo-local SDK smoke cannot import without an approved dependency. | Registry contents should be mirrored into MoA policy, not trusted directly. |
| SDK export: `SessionManager` | PASS | Installed package `dist/index.d.ts` exports `SessionManager`; repo-local SDK smoke cannot import without an approved dependency. | Session tree should remain lane-local until T15f. |
| Package full system access risk | PASS | CLI help exposes built-in `bash`, `edit`, `write` tools; SDK exports tool factories and extension runtime APIs. | Any package capability expansion requires pinned source, sha256, capability manifest, user confirm, and mutation lock check. |
| Package hot reload risk | PASS | CLI supports extension/package install, remove, update, config, explicit extension loading, and package resource discovery. | Hot reload or package activation must remain blocked until T15d/T15e policy gates exist. |
| Project-local `.pi/settings.json` auto install risk | PASS | CLI help documents package installation into settings and package directory overrides. | Project-local settings must be treated as untrusted input, not activation authority. |

## SDK Import Plan

`spikes/T15a-pi-sdk-smoke.mjs` performs an import-only check for `@earendil-works/pi-coding-agent` from the current Node resolution context. It does not install dependencies, edit package manifests, or write lockfiles. In this worktree the result is `UNVERIFIED/package-not-installed`, while the globally installed CLI package's public `dist/index.d.ts` confirms the planned exports exist in version `0.74.0`. T15c should perform the import in `sidecars/moa-pi-host` after dependency approval.

## RPC Smoke Plan

`spikes/T15a-pi-rpc-smoke.ps1` starts `pi --mode rpc --no-session`, writes JSONL requests for `get_state`, `set_model`, `prompt`, `compact`, and `abort`, captures bounded stdout/stderr, and kills the process after a short timeout. In this environment the CLI is present, but the smoke result is `UNVERIFIED/no-jsonl-before-timeout`. If `pi` is not available, the helper reports `UNVERIFIED/cli-missing` and does not install anything.

## T15b/T15c Guidance

- T15b should prefer RPC first with strict LF-delimited JSONL framing, request-id correlation, bounded startup/read timeouts, and explicit capability discovery before enabling command-specific behavior.
- T15c should keep SDK usage in a Node sidecar, constrain resources through MoA-approved roots, and report SDK capability metadata to MoA policy rather than allowing SDK/package state to become authority.
- Package full system access, hot reload, and project-local auto install are blocked surfaces until T15d/T15e define pinned-source, sha256, capability manifest, user confirmation, and mutation-lock checks.
