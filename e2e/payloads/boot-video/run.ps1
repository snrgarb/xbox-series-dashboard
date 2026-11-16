# The Boot video (docs/spec/start-up.md, #126), with a clip of the test's own: `video-spike.exe --make-clip` writes a
# 4 s H.264 + AAC test pattern as XboxSeriesX.mp4 into a media folder of the test's own, and the Dashboard is started
# on it (`--media`) and on a data folder of the test's own, with Start-up (`Start-Dashboard -Startup`). With no choice
# made, Xbox Series X plays: its first frame and its end are logged with their times, and Home's first frame after.
# Then Settings > Personalisation > Boot video, walked with the pad, sets Off; the Dashboard is started again, and goes
# straight to Home with no video.
#
# Judged by the Dashboard's log, times in ms since the process started:
#   startup: boot video XboxSeriesX from <file>
#   startup: first video frame at <N> ms
#   startup: video ended at <N> ms (<n> frames shown, <n> dropped)
#   startup: Home's first frame at <N> ms
#   settings: startup.boot_video = Off            (A on Off)
#   settings: startup.boot_video is Off           (the next start)
#   startup: boot video Off
# and --log-focus's `focus: Settings: Boot video: <name>[; ticked]`.
#
# Needs video-spike.exe in the stage: build it with the Dashboard (`-p video-spike`). Everything it writes is in the
# test's folder (Get-TestFolder), removed at the end; nothing of the machine's is read or changed. On the VM there is
# no sound device: the video plays by the system's clock, which is not a failure.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$folder = Get-TestFolder 'dashboard-boot-video-test'
$data = Join-Path $folder 'data'
$media = Join-Path $folder 'media'
$videos = Join-Path $media 'videos'
$spike = Join-Path $PSScriptRoot 'video-spike.exe'
$clipSeconds = 4

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then a rest for the picture, if one is asked for.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($Name) {
        Start-Sleep -Milliseconds 3000
        Save-Screen $Name
    }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
}

# Presses a direction until focus is where $Expect says, $Most times at most.
function Walk([string] $Button, [string] $Expect, [int] $Most = 12) {
    $pattern = "^focus: $Expect`$"
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match $pattern) { break }
        Push-Button $Button
        Start-Sleep -Milliseconds 800
    }
    if ((Focus-Now) -match $pattern) {
        Say "$Button until $(Focus-Now)"
    } else {
        Say "$Button never reached '$Expect': $(Focus-Now)"
        $script:failures += "$Button never reached 'focus: $Expect': '$(Focus-Now)'"
    }
}

# A log line matching $Pattern after the first $After lines, waited for $Seconds; a failure named $What if none comes.
function Expect-Log([string] $Pattern, [string] $What, [double] $Seconds = 15, [int] $After = 0) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    Say "${What}: $(if ($line) { $line } else { "NO line '$Pattern'" })"
    if (-not $line) { $script:failures += "$What should have logged '$Pattern'" }
    $line
}

# The time a `startup: ... at <N> ms` line gives, or $null.
function Time-Of([string] $Line) {
    if ($Line -match ' at (\d+) ms') { [double] $Matches[1] } else { $null }
}

# The Dashboard on the test's data and media folders, with Start-up.
function Start-Here {
    $script:dashboard = Start-Dashboard -Startup @('--log-focus', '--data-dir', "`"$data`"", '--media', "`"$media`"")
}

try {
    Close-WindowsPrompts
    Connect-Pad
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force }
    New-Item $videos -ItemType Directory -Force | Out-Null
    New-Item $data -ItemType Directory -Force | Out-Null

    # --- The clip: the spike's test pattern, under the Bundle's name for the default.
    if (-not (Test-Path $spike)) { throw "No video-spike.exe in the stage: build it with -p video-spike" }
    $clip = Join-Path $videos 'XboxSeriesX.mp4'
    $made = Start-Process -FilePath $spike -ArgumentList @('--make-clip', "`"$clip`"", '--seconds', "$clipSeconds") `
        -WorkingDirectory $folder -RedirectStandardError (Join-Path $Out 'make-clip.log') -NoNewWindow -PassThru
    $made.Handle | Out-Null
    if (-not $made.WaitForExit(120000)) { $made.Kill(); throw 'make-clip took over two minutes' }
    Get-Content (Join-Path $Out 'make-clip.log') | ForEach-Object { Say "  $_" }
    if ($made.ExitCode -ne 0 -or -not (Test-Path $clip)) { throw "make-clip exited $($made.ExitCode)" }

    # --- With no choice made, Xbox Series X plays to its end, and then Home.
    Start-Here
    Expect-Log '^startup: boot videos XboxSeriesX from ' 'the videos found' 30 | Out-Null
    Expect-Log '^startup: boot video XboxSeriesX from .*XboxSeriesX\.mp4$' 'the video chosen' 30 | Out-Null
    $first = Expect-Log '^startup: first video frame at \d+ ms$' 'its first frame' 60
    if ($first) {
        Start-Sleep -Milliseconds 1500
        Save-Screen '01-boot-video'
    }
    $end = Expect-Log '^startup: video (ended|skipped|failed|stopped) ' 'its end' ($clipSeconds + 30)
    if ($end -and $end -notmatch '^startup: video ended at \d+ ms') { $failures += "the video should have played to its end: $end" }
    $homeLine = Expect-Log "^startup: Home's first frame at \d+ ms$" "Home's first frame" 30
    Get-DashboardLog | Where-Object { $_ -match '^video: ' } | ForEach-Object { Say "  $_" }
    $times = @((Time-Of $first), (Time-Of $end), (Time-Of $homeLine))
    if ($times -notcontains $null) {
        Say "times: first frame $($times[0]) ms, end $($times[1]) ms, Home $($times[2]) ms; played $($times[1] - $times[0]) ms"
        if (-not ($times[0] -lt $times[1] -and $times[1] -le $times[2])) { $failures += "the order should be first frame, end, Home: $($times -join ', ')" }
        # A slow renderer drops frames but keeps to the clock: the clip lasts about as long however it is drawn.
        $played = $times[1] - $times[0]
        if ($played -lt ($clipSeconds - 1) * 1000) { $failures += "a $clipSeconds s clip ended after $played ms" }
    }
    Start-Sleep -Seconds 4
    Save-Screen '02-home-after-the-video'
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # --- Settings > Personalisation > Boot video: the one found ticked; Off chosen with A.
    Walk Right 'TopBar\(Settings\)' 5
    Press A 'Settings: Rail: Theme settings'
    Press Right 'Settings: Theme settings: Personalisation'
    Press A 'Settings: Personalisation: Boot video' '03-personalisation'
    Press A 'Settings: Boot video: Xbox Series X \(2020\); ticked' '04-boot-video-page'
    Press Up 'Settings: Boot video: Off'
    $mark = Get-DashboardLogLength
    Press A 'Settings: Boot video: Off; ticked' '05-off-chosen'
    Expect-Log '^settings: startup\.boot_video = Off$' 'Off saved' 10 $mark | Out-Null

    # --- Started again: Off, so straight to Home.
    Stop-Dashboard
    Start-Sleep -Seconds 2
    Start-Here
    Expect-Log '^settings: startup\.boot_video is Off$' 'the choice at the next start' 30 | Out-Null
    Expect-Log '^startup: boot video Off$' 'Start-up with it Off' 30 | Out-Null
    Expect-Log "^startup: Home's first frame at \d+ ms$" "Home's first frame with it Off" 60 | Out-Null
    $played = @(Get-DashboardLog | Where-Object { $_ -match '^startup: (first video frame|video )' })
    if ($played.Count) { $failures += "with it Off no video should play: $($played -join '; ')" }
    Start-Sleep -Seconds 3
    Save-Screen '06-home-with-it-off'

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Dashboard
    Get-OwnProcess video-spike | Stop-Process -Force -ErrorAction SilentlyContinue
    Disconnect-Pad
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
