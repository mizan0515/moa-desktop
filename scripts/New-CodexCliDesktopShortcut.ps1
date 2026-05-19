param(
  [string]$RepoRoot,
  [string]$CodexHomePath,
  [string]$ShortcutName = 'Codex CLI (Isolated - Folder Picker)',
  [string]$ShortcutDirectory,
  [string]$OutputPath,
  [ValidateSet('Workspace', 'Automation')]
  [string]$Mode = 'Workspace',
  [switch]$Force,
  [switch]$PrintCommandOnly
)

$ErrorActionPreference = 'Stop'

function Get-FullPath {
  param([Parameter(Mandatory = $true)][string]$Path)
  return [System.IO.Path]::GetFullPath($Path)
}

function Quote-ShortcutArg {
  param([Parameter(Mandatory = $true)][string]$Value)
  return '"' + ($Value -replace '"', '\"') + '"'
}

function Resolve-RepoRoot {
  if (-not [string]::IsNullOrWhiteSpace($RepoRoot)) {
    return (Resolve-Path -LiteralPath $RepoRoot).Path
  }

  $scriptDir = Split-Path -Parent $PSCommandPath
  return (Resolve-Path -LiteralPath (Join-Path $scriptDir '..')).Path
}

function Resolve-PowerShellExe {
  $windowsPowerShell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
  if (Test-Path -LiteralPath $windowsPowerShell) {
    return $windowsPowerShell
  }

  $cmd = Get-Command powershell.exe -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($null -ne $cmd) {
    return $cmd.Source
  }

  throw 'powershell.exe was not found.'
}

function Resolve-CodexIcon {
  $candidates = @(
    (Join-Path $env:LOCALAPPDATA 'OpenAI\Codex\bin\codex.exe'),
    (Join-Path $env:APPDATA 'npm\codex.cmd')
  )
  foreach ($candidate in $candidates) {
    if (-not [string]::IsNullOrWhiteSpace($candidate) -and (Test-Path -LiteralPath $candidate)) {
      return $candidate
    }
  }
  return (Resolve-PowerShellExe)
}

$root = Resolve-RepoRoot
$launcher = Join-Path $root 'scripts\Start-CodexCliIsolatedPicker.ps1'
if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) {
  throw "Launcher script not found: $launcher"
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
  if ([string]::IsNullOrWhiteSpace($ShortcutDirectory)) {
    $ShortcutDirectory = [Environment]::GetFolderPath('DesktopDirectory')
  }
  if ([string]::IsNullOrWhiteSpace($ShortcutDirectory)) {
    throw 'Desktop directory could not be resolved. Pass -ShortcutDirectory or -OutputPath.'
  }
  $fileName = if ($ShortcutName.EndsWith('.lnk', [System.StringComparison]::OrdinalIgnoreCase)) { $ShortcutName } else { "$ShortcutName.lnk" }
  $OutputPath = Join-Path $ShortcutDirectory $fileName
}

$outputFull = Get-FullPath -Path $OutputPath
$outputParent = Split-Path -Parent $outputFull
if (-not (Test-Path -LiteralPath $outputParent -PathType Container)) {
  New-Item -Path $outputParent -ItemType Directory -Force | Out-Null
}

if ((Test-Path -LiteralPath $outputFull) -and -not $Force -and -not $PrintCommandOnly) {
  throw "Shortcut already exists: $outputFull. Re-run with -Force to overwrite."
}

$target = Resolve-PowerShellExe
$args = @(
  '-NoExit',
  '-NoProfile',
  '-ExecutionPolicy', 'Bypass',
  '-STA',
  '-File', (Quote-ShortcutArg $launcher),
  '-RuntimeRoot', (Quote-ShortcutArg $root),
  '-Mode', (Quote-ShortcutArg $Mode)
)
if (-not [string]::IsNullOrWhiteSpace($CodexHomePath)) {
  $args += @('-CodexHomePath', (Quote-ShortcutArg (Get-FullPath -Path $CodexHomePath)))
}
$argumentString = $args -join ' '
$icon = Resolve-CodexIcon

$summary = [ordered]@{
  output_path = $outputFull
  target_path = $target
  arguments = $argumentString
  working_directory = $root
  icon = $icon
  mode = $Mode
}

if ($PrintCommandOnly) {
  [pscustomobject]$summary | ConvertTo-Json -Depth 4
  exit 0
}

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($outputFull)
$shortcut.TargetPath = $target
$shortcut.Arguments = $argumentString
$shortcut.WorkingDirectory = $root
$shortcut.IconLocation = "$icon,0"
$shortcut.Description = 'Launch Codex CLI with an isolated CODEX_HOME and a folder picker.'
$shortcut.Save()

[pscustomobject]$summary | ConvertTo-Json -Depth 4
