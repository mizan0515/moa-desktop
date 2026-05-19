#!/usr/bin/env bash
set -euo pipefail

repo_root=""
codex_home=""
shortcut_name="Codex CLI (Isolated - Folder Picker)"
output_path=""
mode="Workspace"
force=0
print_only=0
self_test_guard=0

usage() {
  cat <<'USAGE'
Usage: scripts/new-codex-cli-desktop-shortcut.macos.sh [options]

Options:
  --repo-root PATH       Repository root. Defaults to the parent of this script.
  --codex-home PATH      Existing isolated CODEX_HOME to use.
  --shortcut-name NAME   Desktop shortcut name without .command.
  --output-path PATH     Exact .command file path to create.
  --mode MODE            Workspace or Automation. Default: Workspace.
  --force                Overwrite an existing shortcut.
  --print-command-only   Print planned values without writing a shortcut.
  --self-test-guard      Verify forbidden workspace/CODEX_HOME path checks.
  -h, --help             Show this help.
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo-root) repo_root="$2"; shift 2 ;;
    --codex-home) codex_home="$2"; shift 2 ;;
    --shortcut-name) shortcut_name="$2"; shift 2 ;;
    --output-path) output_path="$2"; shift 2 ;;
    --mode) mode="$2"; shift 2 ;;
    --force) force=1; shift ;;
    --print-command-only) print_only=1; shift ;;
    --self-test-guard) self_test_guard=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -z "$repo_root" ]; then
  repo_root="$(cd "$script_dir/.." && pwd)"
else
  repo_root="$(cd "$repo_root" && pwd)"
fi

abs_path() {
  local path="$1"
  if [ -d "$path" ]; then
    (cd "$path" && pwd)
  else
    local parent
    parent="$(dirname "$path")"
    local base
    base="$(basename "$path")"
    mkdir -p "$parent"
    parent="$(cd "$parent" && pwd)"
    printf '%s/%s\n' "$parent" "$base"
  fi
}

assert_not_forbidden() {
  local path
  path="$(abs_path "$1")"
  local purpose="$2"
  local home_root
  home_root="$(cd "$HOME" && pwd)"
  local marker
  for marker in codex claude ssh; do
    local forbidden
    forbidden="$(abs_path "$home_root/.$marker")"
    case "$path/" in
      "$forbidden/"*)
      echo "Refusing forbidden host-profile/credential path for $purpose: $path" >&2
      return 2
      ;;
    esac
    case "$forbidden/" in
      "$path/"*)
      echo "Refusing workspace that contains host-profile/credential path for $purpose: $path" >&2
      return 2
      ;;
    esac
  done
}

run_guard_self_test() {
  local failed=0
  local normal_project="${TMPDIR:-/tmp}/codex-cli-isolated-normal-project"
  mkdir -p "$normal_project"

  if ! assert_not_forbidden "$normal_project" "self-test normal project" >/dev/null 2>&1; then
    echo "FAIL: normal project folder was rejected" >&2
    failed=1
  fi

  local marker
  for marker in codex claude ssh; do
    if assert_not_forbidden "$HOME/.$marker" "self-test direct forbidden" >/dev/null 2>&1; then
      echo "FAIL: direct host-profile/credential folder was accepted: $marker" >&2
      failed=1
    fi
  done

  if assert_not_forbidden "$HOME" "self-test containing parent" >/dev/null 2>&1; then
    echo "FAIL: containing parent folder was accepted" >&2
    failed=1
  fi

  if [ "$failed" -ne 0 ]; then
    exit 1
  fi

  echo "CODEX_CLI_MACOS_GUARD_SELF_TEST_PASS"
}

resolve_codex_home() {
  local candidate
  local candidates=()
  [ -n "$codex_home" ] && candidates+=("$codex_home")
  [ -n "${CODEX_MOA_ISOLATED_CODEX_HOME:-}" ] && candidates+=("$CODEX_MOA_ISOLATED_CODEX_HOME")
  candidates+=("$repo_root/codex-home")
  candidates+=("$(dirname "$repo_root")/codex-moa-isolated-env-snapshots/codex-home")
  candidates+=("$repo_root/.runtime/codex-home")

  for candidate in "${candidates[@]}"; do
    [ -z "$candidate" ] && continue
    assert_not_forbidden "$candidate" "CODEX_HOME"
    if [ -d "$candidate" ]; then
      abs_path "$candidate"
      return
    fi
  done

  candidate="$repo_root/.runtime/codex-home"
  assert_not_forbidden "$candidate" "CODEX_HOME"
  mkdir -p "$candidate"
  abs_path "$candidate"
}

if [ "$mode" != "Workspace" ] && [ "$mode" != "Automation" ]; then
  echo "--mode must be Workspace or Automation" >&2
  exit 2
fi

if [ "$self_test_guard" -eq 1 ]; then
  run_guard_self_test
  exit 0
fi

resolved_codex_home="$(resolve_codex_home)"
if [ -z "$output_path" ]; then
  output_path="$HOME/Desktop/$shortcut_name.command"
fi
output_path="$(abs_path "$output_path")"

if [ -e "$output_path" ] && [ "$force" -ne 1 ] && [ "$print_only" -ne 1 ]; then
  echo "Shortcut already exists: $output_path. Re-run with --force to overwrite." >&2
  exit 2
fi

if [ "$print_only" -eq 1 ]; then
  cat <<JSON
{
  "output_path": "$output_path",
  "repo_root": "$repo_root",
  "codex_home": "$resolved_codex_home",
  "mode": "$mode"
}
JSON
  exit 0
fi

cat > "$output_path" <<LAUNCHER
#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$repo_root"
CODEX_HOME_PATH="$resolved_codex_home"
MODE="$mode"

abs_path() {
  local path="\$1"
  if [ -d "\$path" ]; then
    (cd "\$path" && pwd)
  else
    local parent
    parent="\$(dirname "\$path")"
    local base
    base="\$(basename "\$path")"
    mkdir -p "\$parent"
    parent="\$(cd "\$parent" && pwd)"
    printf '%s/%s\n' "\$parent" "\$base"
  fi
}

assert_not_forbidden() {
  local path
  path="\$(abs_path "\$1")"
  local purpose="\$2"
  local home_root
  home_root="\$(cd "\$HOME" && pwd)"
  local marker
  for marker in codex claude ssh; do
    local forbidden="\$home_root/.\$marker"
    case "\$path/" in
      "\$forbidden/"*)
      echo "Refusing forbidden host-profile/credential path for \$purpose: \$path" >&2
      return 2
      ;;
    esac
    case "\$forbidden/" in
      "\$path/"*)
      echo "Refusing workspace that contains host-profile/credential path for \$purpose: \$path" >&2
      return 2
      ;;
    esac
  done
}

choose_workspace() {
  local selected=""
  if command -v osascript >/dev/null 2>&1; then
    selected="\$(osascript -e 'POSIX path of (choose folder with prompt "Choose a folder for isolated Codex CLI")' 2>/dev/null || true)"
  fi
  if [ -z "\$selected" ]; then
    printf 'Workspace path [%s]: ' "\$REPO_ROOT"
    read -r selected
    [ -z "\$selected" ] && selected="\$REPO_ROOT"
  fi
  abs_path "\$selected"
}

workspace="\$(choose_workspace)"
assert_not_forbidden "\$workspace" "workspace"
assert_not_forbidden "\$CODEX_HOME_PATH" "CODEX_HOME"
mkdir -p "\$CODEX_HOME_PATH"

export CODEX_HOME="\$CODEX_HOME_PATH"
export CODEX_MOA_RUNTIME_HOME="\$(dirname "\$CODEX_HOME_PATH")"
export PYTHONUTF8=1

if [ "\$MODE" = "Automation" ]; then
  sandbox="danger-full-access"
  approval="never"
else
  sandbox="workspace-write"
  approval="on-request"
fi

echo
echo "Codex CLI isolated profile launch"
echo "  workspace:  \$workspace"
echo "  CODEX_HOME: \$CODEX_HOME"
echo "  mode:       \$MODE (\$sandbox / approval=\$approval)"
echo "  skills:     use plain text triggers, e.g. parallel-ticket-planner, easy-briefing, ticket-review. Leading / is reserved for CLI slash commands."
echo

exec codex --enable goals --cd "\$workspace" --sandbox "\$sandbox" --ask-for-approval "\$approval" --search --no-alt-screen
LAUNCHER

chmod +x "$output_path"

cat <<JSON
{
  "output_path": "$output_path",
  "repo_root": "$repo_root",
  "codex_home": "$resolved_codex_home",
  "mode": "$mode"
}
JSON
