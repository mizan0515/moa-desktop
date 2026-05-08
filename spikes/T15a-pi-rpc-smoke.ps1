param(
  [int]$TimeoutSeconds = 8
)

$ErrorActionPreference = "Stop"

$piCommand = Get-Command pi -ErrorAction SilentlyContinue
if (-not $piCommand) {
  [pscustomobject]@{
    result = "UNVERIFIED"
    reason = "cli-missing"
    command = "pi --mode rpc --no-session"
  } | ConvertTo-Json -Depth 4
  exit 0
}

$psi = [System.Diagnostics.ProcessStartInfo]::new()
if ($piCommand.Source -like "*.ps1") {
  $psi.FileName = "powershell"
  $psi.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$($piCommand.Source)`" --mode rpc --no-session"
} else {
  $psi.FileName = $piCommand.Source
  $psi.Arguments = "--mode rpc --no-session"
}
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true

$process = [System.Diagnostics.Process]::new()
$process.StartInfo = $psi

[void]$process.Start()

$requests = @(
  @{ id = "t15a-get-state"; method = "get_state"; params = @{} },
  @{ id = "t15a-set-model"; method = "set_model"; params = @{ model = "default" } },
  @{ id = "t15a-prompt"; method = "prompt"; params = @{ prompt = "Reply with the exact text T15A_RPC_SMOKE." } },
  @{ id = "t15a-compact"; method = "compact"; params = @{} },
  @{ id = "t15a-abort"; method = "abort"; params = @{ requestId = "t15a-prompt" } }
)

foreach ($request in $requests) {
  $line = $request | ConvertTo-Json -Compress -Depth 8
  $process.StandardInput.WriteLine($line)
  $process.StandardInput.Flush()
}

$deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
while (-not $process.HasExited -and [DateTimeOffset]::UtcNow -lt $deadline) {
  Start-Sleep -Milliseconds 100
}

$wasKilled = $false
if (-not $process.HasExited) {
  $wasKilled = $true
  $process.Kill()
  $process.WaitForExit(2000) | Out-Null
}

$stdoutText = $process.StandardOutput.ReadToEnd()
$stderrText = $process.StandardError.ReadToEnd()
$stdoutLines = @($stdoutText -split "`r?`n" | Where-Object { $_ -ne "" })
$stderrLines = @($stderrText -split "`r?`n" | Where-Object { $_ -ne "" })

[pscustomobject]@{
  result = if ($stdoutLines.Count -gt 0) { "PASS" } else { "UNVERIFIED" }
  reason = if ($stdoutLines.Count -gt 0) { $null } else { "no-jsonl-before-timeout" }
  command = "pi --mode rpc --no-session"
  exitCode = if ($process.HasExited) { $process.ExitCode } else { $null }
  timeoutSeconds = $TimeoutSeconds
  killedAfterTimeout = $wasKilled
  stdout = $stdoutLines
  stderr = $stderrLines
  requestedMethods = @("get_state", "set_model", "prompt", "compact", "abort")
} | ConvertTo-Json -Depth 8

exit 0
