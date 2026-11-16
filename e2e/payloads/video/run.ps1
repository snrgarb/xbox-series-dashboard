# The video spike (issue #6, spikes/video): makes its test clip with Media Foundation's Sink Writer, then plays it full
# screen and in a window, with and without sound, and the same for the .wmv fallback. What each run logs, its exit code
# and a screenshot about 2 s into each playback come back. It passes when the .mp4 clip is made and plays to its end
# with exit 0; everything else is reported for docs/research/video-media-foundation.md, not required. The VM has no
# sound device, so `audio: none` is expected and is not a failure.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$exe = Join-Path $PSScriptRoot 'video-spike.exe'

# One run: its stderr to <name>.log, a screenshot $ShotAfter ms in (0 for none), then its exit code, which is returned
# ($null when it had to be killed). The log's lines that matter are said.
function Run([string] $Name, [string[]] $Arguments, [int] $ShotAfter = 2000, [int] $Seconds = 60) {
    $log = Join-Path $Out "$Name.log"
    Say "$Name`: video-spike $($Arguments -join ' ')"
    $p = Start-Process -FilePath $exe -ArgumentList $Arguments -WorkingDirectory $Out `
        -RedirectStandardError $log -NoNewWindow -PassThru
    $p.Handle | Out-Null
    if ($ShotAfter -gt 0) {
        Start-Sleep -Milliseconds $ShotAfter
        if (-not $p.HasExited) { Save-Screen $Name } else { Say "$Name`: had exited before the screenshot" }
    }
    $code = $null
    if ($p.WaitForExit($Seconds * 1000)) { $code = $p.ExitCode } else { $p.Kill(); Say "$Name`: killed after $Seconds s" }
    Start-Sleep -Milliseconds 300
    if (Test-Path $log) {
        Get-Content $log | Where-Object { $_ -match 'video:|audio:|make-clip:|error:|adapter:|panic' } |
            ForEach-Object { Say "  $_" }
    }
    Say "$Name`: exit $code"
    return $code
}

try {
    Close-WindowsPrompts

    # Required: the H.264 + AAC clip is made, and plays to the end of its stream.
    $made = Run 'make-mp4' @('--make-clip', 'clip.mp4') -ShotAfter 0 -Seconds 120
    if ($made -ne 0) { $failures += "make-mp4 exited $made" }
    if ($made -eq 0) {
        $played = Run 'play-mp4' @('clip.mp4')
        if ($played -ne 0) { $failures += "play-mp4 exited $played" }
        elseif (-not (Select-String -Path (Join-Path $Out 'play-mp4.log') -Pattern 'video: end of stream' -Quiet)) {
            $failures += 'play-mp4 exited 0 without reaching the end of the stream'
        }
        # Reported only.
        Run 'play-mp4-windowed' @('clip.mp4', '--no-audio', '--windowed', '--seconds', '2') -ShotAfter 1200 | Out-Null
        Run 'play-mp4-basic' @('clip.mp4', '--basic-processing', '--seconds', '3') | Out-Null
        Run 'play-mp4-dxva' @('clip.mp4', '--dxva', '--seconds', '3') | Out-Null
    }

    # Reported only: the fallback for a Windows with no H.264 encoder.
    $wmv = Run 'make-wmv' @('--make-clip', 'clip.wmv') -ShotAfter 0 -Seconds 120
    if ($wmv -eq 0) {
        Run 'play-wmv' @('clip.wmv') | Out-Null
        Run 'play-wmv-windowed' @('clip.wmv', '--no-audio', '--windowed', '--seconds', '2') -ShotAfter 1200 | Out-Null
    }
    Get-ChildItem $Out -Filter 'clip.*' | ForEach-Object { Say "clip: $($_.Name) $($_.Length) bytes" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Get-OwnProcess video-spike | Stop-Process -Force -ErrorAction SilentlyContinue
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
