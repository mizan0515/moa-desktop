//! Cross-platform `which` — finds an executable on `PATH`.
//!
//! Windows-aware: walks `PATHEXT` (or fallback `.COM;.EXE;.BAT;.CMD`) plus
//! the bare name, in that order. This matters because npm-installed CLIs
//! (`claude`, `codex`) ship as `*.cmd` shims, NOT as `.exe`. Restricting the
//! search to `cmd.exe` would silently fall back to a relative
//! `PathBuf::from("cmd")` and explode at spawn time.
//!
//! Spawn-unsafe extensions (`.PS1`, `.PSM1`, `.VBS`) are filtered even when
//! present in the user's PATHEXT — `Command::new("foo.ps1")` cannot be
//! launched directly by `CreateProcessW`; a wrapper resolver is required.
//! See `SPAWN_UNSAFE_EXTENSIONS` below.

use std::path::{Path, PathBuf};

/// Locate `cmd` on the current `PATH`. Returns `None` if no candidate
/// resolves to a regular file.
pub fn which(cmd: &str) -> Option<PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    which_in(cmd, &path_env, std::env::var_os("PATHEXT").as_deref())
}

/// Locate a native Codex executable on Windows, avoiding npm `codex.cmd`
/// shims. The Codex adapter passes the resolved path directly to
/// `Command::new`, and the S2/S8 Windows spike established that the native
/// binary is the stable spawn target.
#[cfg(windows)]
pub fn codex_native_exe() -> Option<PathBuf> {
    codex_native_exe_in(
        std::env::var_os("APPDATA").as_deref(),
        std::env::var_os("LOCALAPPDATA").as_deref(),
        std::env::var_os("PATH").as_deref(),
    )
}

#[cfg(not(windows))]
pub fn codex_native_exe() -> Option<PathBuf> {
    which("codex")
}

pub fn codex_fallback_name() -> &'static str {
    if cfg!(windows) {
        "codex.exe"
    } else {
        "codex"
    }
}

#[cfg(windows)]
fn codex_native_exe_in(
    appdata: Option<&std::ffi::OsStr>,
    localappdata: Option<&std::ffi::OsStr>,
    path_env: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(appdata) = appdata {
        candidates.push(
            PathBuf::from(appdata)
                .join("npm")
                .join("node_modules")
                .join("@openai")
                .join("codex")
                .join("node_modules")
                .join("@openai")
                .join("codex-win32-x64")
                .join("vendor")
                .join("x86_64-pc-windows-msvc")
                .join("codex")
                .join("codex.exe"),
        );
    }

    if let Some(path_env) = path_env {
        for dir in std::env::split_paths(path_env) {
            if dir.is_absolute() {
                candidates.push(dir.join("codex.exe"));
            }
        }
    }

    if let Some(localappdata) = localappdata {
        let local = PathBuf::from(localappdata);
        candidates.push(local.join("Programs").join("codex").join("codex.exe"));
        candidates.push(
            local
                .join("OpenAI")
                .join("Codex")
                .join("bin")
                .join("codex.exe"),
        );
    }

    candidates.into_iter().find(|p| is_regular_file(p))
}

/// Same as [`which`] but takes the search PATH and PATHEXT explicitly,
/// so callers (and tests) don't have to mutate the process environment.
pub fn which_in(
    cmd: &str,
    path_env: &std::ffi::OsStr,
    pathext_env: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let extensions = candidate_extensions(cmd, pathext_env);
    for dir in std::env::split_paths(path_env) {
        for ext in &extensions {
            let candidate = if ext.is_empty() {
                dir.join(cmd)
            } else {
                dir.join(format!("{cmd}{ext}"))
            };
            if is_regular_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_regular_file(p: &Path) -> bool {
    p.is_file()
}

/// Extensions we refuse to return as direct-spawn candidates. `.ps1`
/// cannot be passed to `Command::new` — it requires a PowerShell host
/// plus an execution-policy decision the runner has no way to make.
/// A future call site that wants `.ps1` must opt in explicitly via a
/// wrapper resolver (not yet built).
const SPAWN_UNSAFE_EXTS: &[&str] = &[".PS1", ".PSM1", ".PSD1", ".VBS", ".JS"];

/// Build the list of extensions to try, in priority order.
///
/// * Unix: just the bare name.
/// * Windows with `PATHEXT` set: each PATHEXT entry that's safe to
///   `Command::new` directly, then the bare name. Spawn-unsafe entries
///   like `.PS1` are filtered out.
/// * Windows without `PATHEXT`: hardcoded fallback `.COM;.EXE;.BAT;.CMD`
///   (Windows native order), then bare.
///
/// If `cmd` already carries any extension at all (e.g. `tool.cmd` or
/// `tool.foo`), the bare form is tried FIRST. Microsoft `where` only
/// appends PATHEXT extensions when no extension is specified — we match.
fn candidate_extensions(cmd: &str, pathext_env: Option<&std::ffi::OsStr>) -> Vec<String> {
    if !cfg!(windows) {
        return vec![String::new()];
    }

    let mut exts: Vec<String> = match pathext_env {
        Some(raw) => {
            let s = raw.to_string_lossy();
            s.split(';')
                .map(|e| e.trim().to_string())
                .filter(|e| !e.is_empty())
                .collect()
        }
        // Windows native default order when PATHEXT is unset: COM > EXE >
        // BAT > CMD. PS1 deliberately omitted — see SPAWN_UNSAFE_EXTS.
        None => vec![
            ".COM".to_string(),
            ".EXE".to_string(),
            ".BAT".to_string(),
            ".CMD".to_string(),
        ],
    };

    // Normalize: ensure dot prefix, drop dups (case-insensitive), drop
    // anything we can't directly `Command::new` (`.ps1`, `.vbs`, ...).
    let unsafe_lc: std::collections::HashSet<String> = SPAWN_UNSAFE_EXTS
        .iter()
        .map(|e| e.to_ascii_lowercase())
        .collect();
    let mut seen = std::collections::HashSet::new();
    let normalized: Vec<String> = exts
        .drain(..)
        .map(|e| {
            if e.starts_with('.') {
                e
            } else {
                format!(".{e}")
            }
        })
        .filter(|e| !unsafe_lc.contains(&e.to_ascii_lowercase()))
        .filter(|e| seen.insert(e.to_ascii_lowercase()))
        .collect();
    exts = normalized;

    // Per `where.exe` semantics: any explicit extension on the input
    // suppresses PATHEXT suffixing. We still try the suffixed forms
    // afterward as a forgiving fallback, but the bare input wins.
    let has_explicit_ext = std::path::Path::new(cmd).extension().is_some();

    let mut out = Vec::with_capacity(exts.len() + 1);
    if has_explicit_ext {
        out.push(String::new());
        out.extend(exts);
    } else {
        out.extend(exts);
        out.push(String::new());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;

    fn touch(p: &Path) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, b"").unwrap();
    }

    fn ends_with_ci(p: &Path, suffix: &str) -> bool {
        p.to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(&suffix.to_ascii_lowercase())
    }

    #[test]
    fn finds_bare_executable_in_path() {
        let tmp = tempfile::tempdir().unwrap();
        let exe_name = if cfg!(windows) { "tool.exe" } else { "tool" };
        touch(&tmp.path().join(exe_name));

        let path = OsString::from(tmp.path());
        let pathext = OsString::from(".EXE");
        let found = which_in("tool", &path, Some(&pathext)).expect("found");
        // Windows file system is case-insensitive — compare case-insensitively
        // because PATHEXT is conventionally upper while files are often lower.
        assert!(ends_with_ci(&found, exe_name), "got {found:?}");
    }

    #[cfg(windows)]
    #[test]
    fn finds_npm_cmd_shim_when_only_cmd_present() {
        // Regression: npm shims ship as `*.cmd`. Pre-fix `which()` only
        // tried `.exe` and silently returned None, falling back to a bare
        // name and exploding at spawn time.
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("claude.cmd"));

        let path = OsString::from(tmp.path());
        let pathext = OsString::from(".COM;.EXE;.BAT;.CMD;.PS1");
        let found = which_in("claude", &path, Some(&pathext)).expect("shim found");
        assert!(ends_with_ci(&found, "claude.cmd"), "got {found:?}");
    }

    #[cfg(windows)]
    #[test]
    fn ps1_shim_is_filtered_out_even_if_pathext_lists_it() {
        // Regression guard (Codex review P1): `.ps1` cannot be passed to
        // `Command::new` directly. If only a `.ps1` shim is on PATH, we
        // must return None rather than handing the caller something that
        // will spawn-fail.
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("codex.ps1"));

        let path = OsString::from(tmp.path());
        let pathext = OsString::from(".EXE;.CMD;.PS1");
        let found = which_in("codex", &path, Some(&pathext));
        assert!(found.is_none(), "must NOT return .ps1; got {found:?}");
    }

    #[cfg(windows)]
    #[test]
    fn ps1_does_not_shadow_cmd_shim_when_both_present() {
        // If both `.ps1` and `.cmd` exist, `.cmd` wins — `.ps1` is filtered
        // before priority resolution.
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("codex.ps1"));
        touch(&tmp.path().join("codex.cmd"));

        let path = OsString::from(tmp.path());
        // PS1 listed first in PATHEXT — would have won pre-fix.
        let pathext = OsString::from(".PS1;.CMD;.EXE");
        let found = which_in("codex", &path, Some(&pathext)).expect("cmd shim");
        assert!(ends_with_ci(&found, "codex.cmd"), "got {found:?}");
    }

    #[cfg(windows)]
    #[test]
    fn prefers_pathext_priority_order() {
        // Both .exe and .cmd present — PATHEXT order wins.
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("foo.cmd"));
        touch(&tmp.path().join("foo.exe"));

        let path = OsString::from(tmp.path());
        // .CMD listed before .EXE
        let pathext = OsString::from(".CMD;.EXE");
        let found = which_in("foo", &path, Some(&pathext)).unwrap();
        assert!(ends_with_ci(&found, "foo.cmd"), "got {found:?}");
    }

    #[cfg(windows)]
    #[test]
    fn explicit_extension_resolves_directly() {
        // User passed "tool.cmd" — we should not double-suffix to "tool.cmd.cmd".
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("tool.cmd"));

        let path = OsString::from(tmp.path());
        let pathext = OsString::from(".CMD");
        let found = which_in("tool.cmd", &path, Some(&pathext)).unwrap();
        assert!(ends_with_ci(&found, "tool.cmd"), "got {found:?}");
    }

    #[cfg(windows)]
    #[test]
    fn explicit_unknown_extension_tries_bare_first() {
        // Codex review P2: `where.exe` only appends PATHEXT when no
        // extension is given. `tool.foo` exists — we must return it as-is,
        // not try `tool.foo.EXE` first (which would still work via the
        // bare-form fallback, but is wrong order).
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("tool.foo"));
        // Also make a `tool.foo.EXE` so we can detect wrong-order resolution.
        touch(&tmp.path().join("tool.foo.EXE"));

        let path = OsString::from(tmp.path());
        let pathext = OsString::from(".EXE;.CMD");
        let found = which_in("tool.foo", &path, Some(&pathext)).expect("bare wins");
        let s = found.to_string_lossy().to_ascii_lowercase();
        assert!(
            s.ends_with("tool.foo"),
            "explicit extension must resolve bare-first; got {found:?}"
        );
    }

    #[test]
    fn returns_none_when_not_on_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = OsString::from(tmp.path());
        let pathext = OsString::from(".EXE;.CMD");
        assert!(which_in("definitely-not-here", &path, Some(&pathext)).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn codex_native_prefers_npm_vendor_exe_over_cmd_shim_and_app_bundle() {
        let appdata = tempfile::tempdir().unwrap();
        let localappdata = tempfile::tempdir().unwrap();
        let path_dir = tempfile::tempdir().unwrap();

        touch(&appdata.path().join("npm").join("codex.cmd"));
        let npm_native = appdata
            .path()
            .join("npm")
            .join("node_modules")
            .join("@openai")
            .join("codex")
            .join("node_modules")
            .join("@openai")
            .join("codex-win32-x64")
            .join("vendor")
            .join("x86_64-pc-windows-msvc")
            .join("codex")
            .join("codex.exe");
        touch(&npm_native);
        touch(
            &localappdata
                .path()
                .join("OpenAI")
                .join("Codex")
                .join("bin")
                .join("codex.exe"),
        );
        touch(&path_dir.path().join("codex.exe"));

        let path = std::env::join_paths([path_dir.path()]).unwrap();
        let found = codex_native_exe_in(
            Some(appdata.path().as_os_str()),
            Some(localappdata.path().as_os_str()),
            Some(path.as_os_str()),
        )
        .expect("native codex");

        assert_eq!(found, npm_native);
    }

    #[cfg(windows)]
    #[test]
    fn codex_native_uses_path_exe_but_never_cmd_shim() {
        let appdata = tempfile::tempdir().unwrap();
        let path_dir = tempfile::tempdir().unwrap();

        touch(&appdata.path().join("npm").join("codex.cmd"));
        touch(&path_dir.path().join("codex.ps1"));
        let path_exe = path_dir.path().join("codex.exe");
        touch(&path_exe);

        let path = std::env::join_paths([path_dir.path()]).unwrap();
        let found = codex_native_exe_in(
            Some(appdata.path().as_os_str()),
            None,
            Some(path.as_os_str()),
        )
        .expect("native codex");

        assert_eq!(found, path_exe);
    }

    #[cfg(windows)]
    #[test]
    fn codex_native_prefers_path_exe_over_app_bundle_alpha() {
        let localappdata = tempfile::tempdir().unwrap();
        let path_dir = tempfile::tempdir().unwrap();

        let path_exe = path_dir.path().join("codex.exe");
        touch(&path_exe);
        touch(
            &localappdata
                .path()
                .join("OpenAI")
                .join("Codex")
                .join("bin")
                .join("codex.exe"),
        );

        let path = std::env::join_paths([path_dir.path()]).unwrap();
        let found = codex_native_exe_in(
            None,
            Some(localappdata.path().as_os_str()),
            Some(path.as_os_str()),
        )
        .expect("native codex");

        assert_eq!(found, path_exe);
    }

    #[cfg(windows)]
    #[test]
    fn codex_native_rejects_relative_path_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let rel_dir = tmp.path().join("tools");
        fs::create_dir_all(&rel_dir).unwrap();
        touch(&rel_dir.join("codex.exe"));

        let path = OsString::from("tools");
        let found = codex_native_exe_in(None, None, Some(path.as_os_str()));
        assert!(
            found.is_none(),
            "relative PATH entry must be rejected; got {found:?}"
        );
    }
}
