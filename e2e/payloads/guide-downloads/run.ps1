# The Guide's Downloads Utility (docs/spec/guide.md, section 16), with the pad, at Home and over a stand-in game:
#
#   1. At Home: the Guide button, Down the landing tab to the Utilities, Right along them to the fourth, Downloads
#      (`guide: Landing Utility(3)`); Right again goes nowhere. A closes the Guide onto Manage Queue (`focus:
#      ManageQueue: ...`), the Dashboard in front. B closes the page onto Home's rows.
#   2. Over the stand-in game, launched from its Game hub: the same walk, and A brings the Dashboard in front of the
#      game with Manage Queue on it, the game still running behind. B is Home, still in front of the game.
#
# Judged by the Dashboard's `guide:` and `focus:` lines (--log-focus), a press at a time, and the foreground. The
# Queue is empty here: the badge is the screenshot tests' (guide-downloads-queued).
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
# Manage Queue as it opens on the fake Steam: Recently installed's first cover, or nothing, and the Queue empty.
$manageQueue = 'ManageQueue: (Cover\(0\) of .+|Nothing); 0 installing, \d+ recently installed$'

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }
function Fail([string] $What) { $script:failures += $What; Say "WRONG: $What" }

# The last line with this prefix the Dashboard logged, without the prefix.
function Get-LastLine([string] $Prefix) {
    $lines = @(Get-DashboardLog | Where-Object { $_ -match "^${Prefix}: " })
    if ($lines.Count) { $lines[-1] -replace "^${Prefix}: ", '' } else { '' }
}
function Get-GuideLine { $line = Get-LastLine 'guide'; if ($line) { $line } else { 'closed' } }

# One press, then the line it must bring: `$Prefix: $Expect` (a regular expression), among those logged after it.
function Step([string] $Button, [string] $Prefix, [string] $Expect, [string] $Name = '', [double] $Seconds = 8) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $line = Wait-DashboardLog "^${Prefix}: $Expect" $Seconds $mark
    Say "$Button$(if ($Name) { " -> $Name" }): $(if ($line) { $line } else { "NO line '${Prefix}: $Expect' (last: $(Get-LastLine $Prefix))" })"
    if (-not $line) { $script:failures += "$Button did not bring '${Prefix}: $Expect'" }
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

# One press while walking somewhere named. A press that brings no line is said and is no failure by itself: a loaded
# VM now and then never shows the Dashboard a 120 ms press, and the walk, which goes by the log, presses again.
function Walk([string] $Button, [string] $Guide) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $line = Wait-DashboardLog "^guide: $Guide" 5 $mark
    Say "$Button`: $(if ($line) { $line } else { "note: no line 'guide: $Guide' of this press (the Guide is at: $(Get-GuideLine))" })"
}

# From the open Guide's landing tab, a press at a time, to the Downloads Utility: Down to the first Utility, then
# Right along the row until the log says Utility(3).
function Move-ToDownloads {
    for ($i = 0; $i -lt 12; $i++) {
        if ((Get-GuideLine) -match '^Landing Utility\(') { break }
        if ((Get-GuideLine) -notmatch '^Landing Entry\(\d+\)') { throw "The Guide is not on its list: $(Get-GuideLine)" }
        Walk Down 'Landing (Entry|Utility)\(\d+\)'
    }
    for ($i = 0; $i -lt 8; $i++) {
        if ((Get-GuideLine) -match '^Landing Utility\(3\)$') { return }
        if ((Get-GuideLine) -notmatch '^Landing Utility\(\d\)') { throw "Focus is not on the Utilities: $(Get-GuideLine)" }
        Walk Right 'Landing Utility\(\d\)$'
    }
    throw "Focus never reached the Downloads Utility: $(Get-GuideLine)"
}

# The Guide open, focus walked to Downloads, a picture, and Right at the row's end doing nothing.
function Open-ToDownloads([string] $Name) {
    if (-not (Step Guide 'guide' 'Landing Entry\(0\)$')) { throw "The Guide did not open (it is at: $(Get-GuideLine))" }
    Move-ToDownloads
    Start-Sleep -Milliseconds 3000
    Save-Screen $Name
    Step-Nothing Right 'Downloads is the last Utility'
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam
    $game = @($library.games | Where-Object { $_.last_played -gt 0 })[0]
    if (-not $game -or -not $game.installed) { throw "The Recent row's first game is not installed" }
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    $dashboard = Start-Dashboard @('--log-focus')
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    # The greeting's Toast comes and goes: a tap while it shows would open the Guide on Profile & system.
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 3
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    if (-not (Wait-DashboardLog '^overlay: configured' 15)) { throw 'The Overlay was not made' }

    # ---- 1. At Home.
    Push-Button Down
    if (-not (Wait-DashboardLog '^focus: Recent\(0\)' 10)) { throw "Down did not put focus on the Recent row's first tile" }
    Open-ToDownloads '01-home-downloads-focused'
    $mark = Get-DashboardLogLength
    Step A 'focus' $manageQueue '02-home-manage-queue' | Out-Null
    if (-not (Wait-DashboardLog '^guide: closed$' 5 $mark)) { Fail "the Guide did not close (it is at: $(Get-GuideLine))" }
    if (-not (Wait-Until { (Get-Foreground).Title -eq $dashboardTitle } 8)) { Fail "the Dashboard is not in front on Manage Queue: $(Front)" }
    Step B 'focus' 'Recent\(0\)$' '03-home-back' | Out-Null

    # ---- 2. Over the stand-in game. A is the game's Game hub, after the double tap's 300 ms; A on its Play the launch.
    $mark = Get-DashboardLogLength
    Push-Button A
    if (-not (Wait-DashboardLog '^focus: GameHub: \d+ Hub Play$' 10 $mark)) { throw "A did not open the game's hub (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    Push-Button A
    if (-not (Wait-DashboardLog 'the game has booted' 60 $mark)) { throw "$($game.name) never counted as booted" }
    if (-not (Wait-DashboardLog 'Launch screen gone' 15 $mark)) { Fail 'the Launch screen did not go' }
    if (-not (Wait-Until { (Get-Foreground).Title -eq $game.name } 8)) { Fail "the game is not in front: $(Front)" }
    Start-Sleep -Milliseconds 500

    Open-ToDownloads '04-game-downloads-focused'
    $mark = Get-DashboardLogLength
    Step A 'focus' $manageQueue '05-game-manage-queue' | Out-Null
    if (-not (Wait-DashboardLog '^guide: closed$' 5 $mark)) { Fail "the Guide did not close (it is at: $(Get-GuideLine))" }
    if (-not (Wait-Until { (Get-Foreground).Title -eq $dashboardTitle } 8)) { Fail "the Dashboard did not come in front of the game: $(Front)" }
    if (-not @(Get-StandInGame).Count) { Fail 'the game is not running behind Manage Queue' }
    Step B 'focus' '(Recent|Wide|TopBar|Group)' '06-game-home-in-front' | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Fail "Home is not in front of the game after B: $(Front)" }
    if (-not @(Get-StandInGame).Count) { Fail 'the game stopped' }

    if ($dashboard.HasExited) { Fail "the Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
    Say "at the error: foreground $(Front); the Guide at: $(Get-GuideLine); focus: $(Get-LastLine 'focus')"
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
