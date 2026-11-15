# The Overlay spike (issue #2, spikes/overlay) in each of its modes over a window standing in for a game: what it logs,
# its exit code, a screenshot while it is shown and what has the foreground during and after. Nothing is asserted but
# that each run exits 0: the findings are read from the logs into docs/research/overlay-window.md.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$exe = Join-Path $PSScriptRoot 'overlay-spike.exe'

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }

# One run: its stderr to <name>.log, a screenshot 2.5 s in, then its exit code.
function Run([string] $Name, [string[]] $Arguments, [int] $Seconds = 30) {
    $log = Join-Path $Out "$Name.log"
    Say "$Name`: overlay-spike $($Arguments -join ' '); foreground before $(Front)"
    $p = Start-Process -FilePath $exe -ArgumentList $Arguments -RedirectStandardError $log -NoNewWindow -PassThru
    $p.Handle | Out-Null
    Start-Sleep -Milliseconds 2500
    Save-Screen $Name
    Say "$Name`: foreground while shown $(Front)"
    if (-not $p.WaitForExit($Seconds * 1000)) { $p.Kill(); $script:failures += "$Name did not exit in $Seconds s"; return }
    Start-Sleep -Milliseconds 500
    Say "$Name`: exit $($p.ExitCode); foreground after $(Front)"
    if ($p.ExitCode -ne 0) { $script:failures += "$Name exited $($p.ExitCode)" }
    Get-Content $log | Where-Object { $_ -match 'selfcheck|foreground|ERROR|panic|hidden|activat' } | ForEach-Object { Say "  $_" }
}

try {
    Close-WindowsPrompts
    # Something light with a title to be the game behind the Overlay, in front as a game is.
    $behind = Start-Process notepad -WindowStyle Maximized -PassThru
    Start-Sleep -Seconds 4
    Save-Screen 'behind'
    Say "behind: foreground $(Front)"

    Run 'guide' @('--mode', 'guide', '--selfcheck', '--seconds', '6')
    Run 'guide-cycle' @('--mode', 'guide', '--cycle', '2')
    Run 'guide-no-alt' @('--mode', 'guide', '--no-alt', '--seconds', '4')
    Run 'passive' @('--mode', 'passive', '--selfcheck', '--seconds', '6')
    Run 'passive-no-layered' @('--mode', 'passive', '--no-layered', '--selfcheck', '--seconds', '6')
    Run 'visual' @('--mode', 'guide', '--visual', '--selfcheck', '--seconds', '6')
    Run 'hwnd-swapchain' @('--mode', 'guide', '--hwnd-swapchain', '--selfcheck', '--seconds', '6')
    Run 'passive-cycle' @('--mode', 'passive', '--cycle', '3')

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
