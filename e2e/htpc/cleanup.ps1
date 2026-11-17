# Run over ssh by e2e/htpc/run.sh after every test, whatever happened: ends what the test left running, and only that.
# A process is the test's when its program is under this folder (the Dashboard, the stand-ins, the fake games), or when
# it is the launcher, run.cmd or the payload's run.ps1 from this folder. Nothing is stopped by its name alone. Then the
# live Dashboard, if the launcher stopped it for the test and it is not back, is started again as the shell.
$ErrorActionPreference = 'Continue'
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class XsdPaths {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern uint GetLongPathName(string s, StringBuilder b, uint n);
    // The path with no 8.3 short names in it, as a process's own path has it.
    public static string Long(string p) { var b = new StringBuilder(1024); return GetLongPathName(p, b, 1024) > 0 ? b.ToString() : p; }
}
'@
$folders = @($PSScriptRoot, [XsdPaths]::Long($PSScriptRoot)) | ForEach-Object { $_.TrimEnd('\') } | Sort-Object -Unique
$ours = @(Get-CimInstance Win32_Process | Where-Object {
    $process = $_
    if ($process.ProcessId -eq $PID) { return $false }
    foreach ($folder in $folders) {
        if ($process.ExecutablePath -and $process.ExecutablePath.StartsWith("$folder\", [StringComparison]::OrdinalIgnoreCase)) { return $true }
        foreach ($script in 'launcher.ps1', 'run.cmd', 'run.ps1') {
            if ($process.CommandLine -and $process.CommandLine.IndexOf("$folder\$script", [StringComparison]::OrdinalIgnoreCase) -ge 0) { return $true }
        }
    }
    $false
})
foreach ($process in $ours) {
    Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue
    "stopped $($process.Name) (pid $($process.ProcessId)): $($process.ExecutablePath)"
}
if (-not $ours.Count) { 'nothing of the test was running' }

# The live shell, if the launcher stopped it and did not start it again (it was ended by a timeout, or the run was
# stopped from the Linux side): started again here, in the signed-in session, through a task of its own (shell.ps1).
$stoppedMarker = Join-Path $PSScriptRoot 'shell-stopped.txt'
if (Test-Path $stoppedMarker) {
    . (Join-Path $PSScriptRoot 'shell.ps1')
    # Write-Host: the lines must not become Start-LiveShell's answer.
    if (Start-LiveShell { param($Text) Write-Host "shell: $Text" }) { Remove-Item $stoppedMarker -Force -ErrorAction SilentlyContinue }
    else { "shell: NOT started again: start it by hand on the TV PC: `"$LiveShellExe`" --shell" }
}
