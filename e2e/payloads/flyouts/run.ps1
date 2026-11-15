# The Guide's Flyouts (docs/spec/guide.md, section 5), over a stand-in game:
#
#   1. Down the landing tab to the Utilities. A on Notifications opens its Flyout, which lists the two notifications
#      the Dashboard was started with (`--notify`), newest first; the hints hide. B closes it, focus back on the
#      Utility.
#   2. A on Audio opens its Flyout on Output. The `audio:` line says what Windows has: the devices, the volume, mute.
#   3. With an output device: Volume moves by 2 a press, the real master volume with it (the Dashboard reads it
#      back from Windows after each change and logs it), and is put back where it was. Mute goes on and off again.
#      Output's dropdown opens its list on the default device, and B closes it with nothing changed.
#      With no output device (a VM without sound): Output says "None", and A, Left and Right do nothing.
#   4. B closes the Flyout, B again the Guide; the game has the foreground back.
#
# Judged by the Dashboard's `guide:` and `audio:` lines (--log-focus), a press at a time, and pixels over the
# stand-in game. The volume this test leaves is the one it found; the default devices are never changed.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
$gameBlue = @(16, 32, 80)
# The stand-in game under the Guide's dim, and the Flyout's flat grey (#1F1F1F).
$dimmedGame = @(7, 14, 35)
$flyout = @(31, 31, 31)
$notifications = @('The first thing that happened', 'The second thing, which is the newer and so the first listed')

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }
function Fail([string] $What) { $script:failures += $What; Say "WRONG: $What" }

# The last line with this prefix the Dashboard logged, without the prefix.
function Get-LastLine([string] $Prefix) {
    $lines = @(Get-DashboardLog | Where-Object { $_ -match "^${Prefix}: " })
    if ($lines.Count) { $lines[-1] -replace "^${Prefix}: ", '' } else { '' }
}
function Get-GuideLine { $line = Get-LastLine 'guide'; if ($line) { $line } else { 'closed' } }

# One press, then the `guide:` line it must bring (a regular expression). A press at a time, judged by the log.
function Step([string] $Button, [string] $Guide, [string] $Name = '', [double] $Seconds = 8) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $line = Wait-DashboardLog "^guide: $Guide" $Seconds $mark
    Say "$Button$(if ($Name) { " -> $Name" }): $(if ($line) { $line } else { "NO line 'guide: $Guide' (the Guide is at: $(Get-GuideLine))" })"
    if (-not $line) { $script:failures += "$Button did not bring 'guide: $Guide'" }
    if ($Name) {
        # The software renderer takes most of a second a frame: a picture waits for the slide.
        Start-Sleep -Milliseconds 3000
        Save-Screen $Name
    }
    [bool] $line
}

# One press that must change nothing of the Guide: no `guide:` line comes of it.
function Step-Nothing([string] $Button, [string] $Why) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $line = Wait-DashboardLog '^guide: ' 2 $mark
    Say "$Button, $Why`: $(if ($line) { "WRONG, it brought '$line'" } else { 'nothing, as it should' })"
    if ($line) { $script:failures += "$Button did something though $Why`: $line" }
}

# One press while walking to somewhere named: the `guide:` line it should bring. A press that brings none is said
# and is no failure by itself: a loaded VM now and then never shows the Dashboard a 120 ms press, and the walk, which
# goes by where the log says focus is, presses again.
function Walk([string] $Button, [string] $Guide) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $line = Wait-DashboardLog "^guide: $Guide" 5 $mark
    Say "$Button`: $(if ($line) { $line } else { "note: no line 'guide: $Guide' of this press (the Guide is at: $(Get-GuideLine)); the walk goes on" })"
}

# Walks Down the landing tab, a press at a time, until focus is on the first Utility: the entry after the last.
function Move-ToUtilities {
    for ($i = 0; $i -lt 12; $i++) {
        if ((Get-GuideLine) -match '^Landing Utility\(0\)') { return }
        if ((Get-GuideLine) -notmatch '^Landing Entry\((\d+)\)') { throw "The Guide is not on its list: $(Get-GuideLine)" }
        Walk Down 'Landing (Entry|Utility)\(\d+\)'
    }
    throw "Focus never reached the Utilities: $(Get-GuideLine)"
}

# Walks Up or Down the open Flyout, a press at a time, until focus is on its entry $Item.
function Move-ToItem([int] $Item) {
    for ($i = 0; $i -lt 8; $i++) {
        if ((Get-GuideLine) -notmatch 'flyout \w+ Item\((\d+)\)') { throw "The Flyout's focus is not on an entry: $(Get-GuideLine)" }
        $at = [int] $Matches[1]
        if ($at -eq $Item) { return }
        $next = if ($at -lt $Item) { $at + 1 } else { $at - 1 }
        Walk $(if ($at -lt $Item) { 'Down' } else { 'Up' }) "Landing Utility\(\d\), flyout \w+ Item\($next\)"
    }
    throw "Focus never reached entry $Item of the Flyout: $(Get-GuideLine)"
}

function Expect-Pixel([double] $X, [double] $Y, $Colour, [string] $What) {
    $ok = Wait-Until { Test-Colour (Get-DesignPixel $X $Y) $Colour 6 } 8
    $seen = Get-DesignPixel $X $Y
    Say "at ($X, $Y), ${What}: seen $(Format-Colour $seen), expected $(Format-Colour $Colour)"
    if (-not $ok) { $script:failures += "at ($X, $Y) expected $What $(Format-Colour $Colour), seen $(Format-Colour $seen)" }
}

# The volume Windows has, as the Dashboard last read it back: the `audio:` line's.
function Get-ReadVolume { if ((Get-LastLine 'audio') -match '^volume (\d+)') { [int] $Matches[1] } else { -1 } }

# One press on Volume, then the level the Guide must show and Windows must have, read back.
function Step-Volume([string] $Button, [int] $Level) {
    $mark = Get-DashboardLogLength
    Step $Button "Landing Utility\(1\), flyout Audio Item\(2\), volume $Level$" | Out-Null
    $read = Wait-DashboardLog "^audio: volume $Level," 8 $mark
    Say "Windows' master volume after ${Button}: $(if ($read) { $read } else { "NOT $Level (last read: $(Get-LastLine 'audio'))" })"
    if (-not $read) { $script:failures += "$Button on Volume did not move Windows' master volume to $Level" }
}

$volumeFound = -1
try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam
    $game = @($library.games | Where-Object { $_.last_played -gt 0 })[0]
    if (-not $game -or -not $game.installed) { throw "The Recent row's first game is not installed" }
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    # Quoted here: Start-Process joins its arguments with spaces and quotes none of them.
    $dashboard = Start-Dashboard @('--log-focus', '--notify', "`"$($notifications[0])`"", '--notify', "`"$($notifications[1])`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    # The greeting's Toast comes and goes: it is not in the pictures, and the Guide button is the Guide's again.
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 3
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    if (-not (Wait-DashboardLog '^overlay: configured' 15)) { throw 'The Overlay was not made' }

    # The game, so that what is beside the Guide is a colour this test knows.
    Push-Button Down
    if (-not (Wait-DashboardLog '^focus: Recent\(0\)' 10)) { throw "Down did not put focus on the Recent row's first tile" }
    $mark = Get-DashboardLogLength
    # A is the game's Game hub, after the double tap's 300 ms; A on its Play is the launch.
    Push-Button A
    if (-not (Wait-DashboardLog '^focus: GameHub: \d+ Hub Play$' 10 $mark)) { throw "A did not open the game's hub (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    Push-Button A
    if (-not (Wait-DashboardLog 'the game has booted' 60 $mark)) { throw "$($game.name) never counted as booted" }
    if (-not (Wait-DashboardLog 'Launch screen gone' 15 $mark)) { Fail 'the Launch screen did not go' }
    Start-Sleep -Milliseconds 500

    # ---- 1. Notifications.
    if (-not (Step Guide 'Landing Entry\(0\)$')) { throw "The Guide did not open (it is at: $(Get-GuideLine))" }
    Move-ToUtilities
    Start-Sleep -Milliseconds 3000
    Save-Screen '01-utility-focused'
    Expect-Pixel 1230 700 $dimmedGame 'the game dimmed, beside the Guide'
    Step A 'Landing Utility\(0\), flyout Notifications Item\(0\)$' '02-notifications' | Out-Null
    Expect-Pixel 400 950 $flyout 'the Flyout'
    Expect-Pixel 1230 700 $dimmedGame 'the game dimmed, beside the Flyout'
    # The hints hide for a Flyout: where the Guide button's mark was, 688 across and 190 down, nothing white.
    $hint = Get-DesignPixel 688 190
    Say "where the hint's mark was: $(Format-Colour $hint)"
    if (($hint.R + $hint.G + $hint.B) -gt 300) { Fail "the hints still show beside the Flyout: $(Format-Colour $hint) where the Guide button's mark is" }
    Step Down 'Landing Utility\(0\), flyout Notifications Item\(1\)$' | Out-Null
    Step-Nothing Down 'that is the last notification'
    Step-Nothing RightShoulder 'the bumpers do nothing with a Flyout open'
    Step B 'Landing Utility\(0\)$' | Out-Null

    # ---- 2. Audio.
    Step Right 'Landing Utility\(1\)$' | Out-Null
    $mark = Get-DashboardLogLength
    Step A 'Landing Utility\(1\), flyout Audio Item\(0\), volume \d+' '03-audio' | Out-Null
    Expect-Pixel 400 950 $flyout 'the Flyout'
    $read = Wait-DashboardLog '^audio: volume \d+, ' 8 0
    Say "what Windows has: $(Get-LastLine 'audio')"
    if (-not $read) { throw 'The Dashboard never logged what it read of the system''s sound' }
    $hasOutput = (Get-LastLine 'audio') -notmatch 'output None'

    if ($hasOutput) {
        # ---- 3. The volume, there and back; mute, on and off; the dropdown's list, opened and closed.
        $volumeFound = Get-ReadVolume
        $wasMuted = (Get-LastLine 'audio') -match 'muted true'
        Say "found: volume $volumeFound, muted $wasMuted"
        Move-ToItem 2
        # Up by 2 and back, or down and back from the top of the scale.
        $first, $second, $moved = if ($volumeFound -le 98) { 'Right', 'Left', ($volumeFound + 2) } else { 'Left', 'Right', ($volumeFound - 2) }
        Step-Volume $first $moved
        Start-Sleep -Milliseconds 3000
        Save-Screen '04-volume-moved'
        Step-Volume $second $volumeFound

        Move-ToItem 3
        $on = 'Landing Utility\(1\), flyout Audio Item\(3\), volume \d+, muted$'
        $off = 'Landing Utility\(1\), flyout Audio Item\(3\), volume \d+$'
        $mark = Get-DashboardLogLength
        Step A $(if ($wasMuted) { $off } else { $on }) '05-mute-changed' | Out-Null
        $read = Wait-DashboardLog "^audio: volume \d+, muted $(if ($wasMuted) { 'false' } else { 'true' })" 8 $mark
        if (-not $read) { Fail "A on Mute did not change Windows' mute (last read: $(Get-LastLine 'audio'))" }
        $mark = Get-DashboardLogLength
        Step A $(if ($wasMuted) { $on } else { $off }) | Out-Null
        $read = Wait-DashboardLog "^audio: volume \d+, muted $(if ($wasMuted) { 'true' } else { 'false' })" 8 $mark
        if (-not $read) { Fail "A on Mute again did not put Windows' mute back (last read: $(Get-LastLine 'audio'))" }

        Move-ToItem 0
        Step A 'Landing Utility\(1\), flyout Audio Device\(Output, \d+\)' '06-output-list' | Out-Null
        Step B 'Landing Utility\(1\), flyout Audio Item\(0\)' | Out-Null
        if (@(Get-DashboardLog | Where-Object { $_ -match '^audio: default ' }).Count) { Fail 'a default device was changed, and nothing chose one' }
    } else {
        Say 'note: this machine has no output device: the volume, mute and the dropdown are only checked to do nothing'
        Step-Nothing A 'Output has no devices'
        Move-ToItem 2
        Step-Nothing Right 'there is no output to turn up'
        Move-ToItem 3
        Step-Nothing A 'there is no output to mute'
    }

    # ---- 4. Out again.
    Step B 'Landing Utility\(1\)$' | Out-Null
    $mark = Get-DashboardLogLength
    Step B 'closed$' | Out-Null
    if (-not (Wait-Until { (Get-Foreground).Title -eq $game.name } 8)) { Fail "the game is not in front after the Guide closed: $(Front)" }
    Expect-Pixel 1230 700 $gameBlue 'the game, the Guide gone'
    if ($dashboard.HasExited) { Fail "the Dashboard exited with $($dashboard.ExitCode)" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
    Say "at the error: foreground $(Front); the Guide at: $(Get-GuideLine); audio: $(Get-LastLine 'audio')"
} finally {
    if ($volumeFound -ge 0 -and (Get-ReadVolume) -ne $volumeFound) { Say "note: the volume was found at $volumeFound and is left at $(Get-ReadVolume)" }
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
