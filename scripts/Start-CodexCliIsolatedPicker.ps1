param(
  [string]$RuntimeRoot,
  [string]$WorkspacePath,
  [string]$CodexHomePath,
  [ValidateSet('Workspace', 'Automation')]
  [string]$Mode = 'Workspace',
  [switch]$PrintCommandOnly
)

$ErrorActionPreference = 'Stop'

function Get-FullPath {
  param([Parameter(Mandatory = $true)][string]$Path)
  return [System.IO.Path]::GetFullPath($Path)
}

function Test-PathUnderOrEqual {
  param(
    [Parameter(Mandatory = $true)][string]$ChildPath,
    [Parameter(Mandatory = $true)][string]$RootPath
  )

  $childFull = (Get-FullPath -Path $ChildPath).TrimEnd('\', '/')
  $rootFull = (Get-FullPath -Path $RootPath).TrimEnd('\', '/')
  $rootPrefix = $rootFull + [System.IO.Path]::DirectorySeparatorChar
  return ($childFull -eq $rootFull -or $childFull.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase))
}

function Assert-NotForbiddenPath {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Purpose
  )

  $userProfile = [Environment]::GetFolderPath('UserProfile')
  if ([string]::IsNullOrWhiteSpace($userProfile)) {
    return
  }

  $forbidden = @('codex', 'claude', 'ssh') | ForEach-Object {
    Join-Path $userProfile ('.' + $_)
  }

  foreach ($root in $forbidden) {
    if ((Test-PathUnderOrEqual -ChildPath $Path -RootPath $root) -or (Test-PathUnderOrEqual -ChildPath $root -RootPath $Path)) {
      throw "Refusing to use forbidden host-profile/credential path for ${Purpose}: $Path"
    }
  }
}

function Resolve-RepoRoot {
  if (-not [string]::IsNullOrWhiteSpace($RuntimeRoot)) {
    return (Resolve-Path -LiteralPath $RuntimeRoot).Path
  }

  $scriptDir = Split-Path -Parent $PSCommandPath
  return (Resolve-Path -LiteralPath (Join-Path $scriptDir '..')).Path
}

function Resolve-IsolatedCodexHome {
  param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [string]$ExplicitCodexHome
  )

  $candidates = New-Object System.Collections.Generic.List[string]
  if (-not [string]::IsNullOrWhiteSpace($ExplicitCodexHome)) {
    $candidates.Add($ExplicitCodexHome)
  }
  if (-not [string]::IsNullOrWhiteSpace($env:CODEX_MOA_ISOLATED_CODEX_HOME)) {
    $candidates.Add($env:CODEX_MOA_ISOLATED_CODEX_HOME)
  }

  $candidates.Add((Join-Path $RepoRoot 'codex-home'))
  $parent = Split-Path -Parent $RepoRoot
  if (-not [string]::IsNullOrWhiteSpace($parent)) {
    $candidates.Add((Join-Path $parent 'codex-moa-isolated-env-snapshots\codex-home'))
  }
  $fallback = Join-Path $RepoRoot '.runtime\codex-home'
  $candidates.Add($fallback)

  foreach ($candidate in $candidates) {
    if ([string]::IsNullOrWhiteSpace($candidate)) {
      continue
    }
    $full = Get-FullPath -Path $candidate
    Assert-NotForbiddenPath -Path $full -Purpose 'CODEX_HOME'
    if (Test-Path -LiteralPath $full -PathType Container) {
      return $full
    }
  }

  $fallbackFull = Get-FullPath -Path $fallback
  Assert-NotForbiddenPath -Path $fallbackFull -Purpose 'CODEX_HOME'
  New-Item -Path $fallbackFull -ItemType Directory -Force | Out-Null
  return $fallbackFull
}

function Select-WorkspacePath {
  param(
    [string]$InitialPath,
    [string]$FallbackPath
  )

  if (-not [string]::IsNullOrWhiteSpace($InitialPath)) {
    return $InitialPath
  }

  try {
    Add-Type -AssemblyName System.Windows.Forms
    $dialog = [System.Windows.Forms.FolderBrowserDialog]::new()
    $dialog.Description = 'Choose a folder for isolated Codex CLI.'
    $dialog.ShowNewFolderButton = $false
    if (-not [string]::IsNullOrWhiteSpace($FallbackPath) -and (Test-Path -LiteralPath $FallbackPath)) {
      $dialog.SelectedPath = $FallbackPath
    }

    $result = $dialog.ShowDialog()
    if ($result -eq [System.Windows.Forms.DialogResult]::OK -and -not [string]::IsNullOrWhiteSpace($dialog.SelectedPath)) {
      return $dialog.SelectedPath
    }
  } catch {
    Write-Warning "Folder picker unavailable: $($_.Exception.Message)"
  }

  $typed = Read-Host 'Workspace folder path. Leave empty to exit'
  if ([string]::IsNullOrWhiteSpace($typed)) {
    exit 2
  }
  return $typed
}

function Resolve-CodexCommand {
  $candidates = @(
    (Join-Path $env:APPDATA 'npm\codex.cmd'),
    (Join-Path $env:LOCALAPPDATA 'OpenAI\Codex\bin\codex.exe')
  )

  foreach ($candidate in $candidates) {
    if (-not [string]::IsNullOrWhiteSpace($candidate) -and (Test-Path -LiteralPath $candidate)) {
      return (Get-FullPath -Path $candidate)
    }
  }

  foreach ($name in @('codex.cmd', 'codex.exe', 'codex')) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $cmd) {
      return $cmd.Source
    }
  }

  throw 'Codex CLI was not found in PATH.'
}

$repoRoot = Resolve-RepoRoot
$workspace = Get-FullPath -Path (Select-WorkspacePath -InitialPath $WorkspacePath -FallbackPath $repoRoot)
if (-not (Test-Path -LiteralPath $workspace -PathType Container)) {
  throw "Workspace folder not found: $workspace"
}
Assert-NotForbiddenPath -Path $workspace -Purpose 'workspace'

$codexHome = Resolve-IsolatedCodexHome -RepoRoot $repoRoot -ExplicitCodexHome $CodexHomePath
$env:CODEX_HOME = $codexHome
$env:CODEX_MOA_RUNTIME_HOME = (Split-Path -Parent $codexHome)
$env:PYTHONUTF8 = '1'

$codex = Resolve-CodexCommand
$sandbox = if ($Mode -eq 'Automation') { 'danger-full-access' } else { 'workspace-write' }
$approval = if ($Mode -eq 'Automation') { 'never' } else { 'on-request' }
$codexArgs = @(
  '--enable', 'goals',
  '--cd', $workspace,
  '--sandbox', $sandbox,
  '--ask-for-approval', $approval,
  '--search',
  '--no-alt-screen'
)

$summary = [ordered]@{
  workspace = $workspace
  codex_home = $codexHome
  codex_moa_runtime_home = $env:CODEX_MOA_RUNTIME_HOME
  codex = $codex
  mode = $Mode
  sandbox = $sandbox
  approval = $approval
  command = "$codex $($codexArgs -join ' ')"
}

Write-Host ''
Write-Host 'Codex CLI isolated profile launch'
Write-Host "  workspace:  $($summary.workspace)"
Write-Host "  CODEX_HOME: $($summary.codex_home)"
Write-Host "  mode:       $Mode ($sandbox / approval=$approval)"
Write-Host '  skills:     use plain text triggers, e.g. parallel-ticket-planner, easy-briefing, ticket-review. Leading / is reserved for CLI slash commands.'
Write-Host ''

if ($PrintCommandOnly) {
  [pscustomobject]$summary | ConvertTo-Json -Depth 4
  exit 0
}

& $codex @codexArgs
exit $LASTEXITCODE
