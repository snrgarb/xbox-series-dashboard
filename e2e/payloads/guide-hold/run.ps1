# Guide hold: the Guide is one Guide, on the Overlay, over Home and over a game alike; it opens on the monitor the
# window in front is on; and while it is open over a game the game is held still (docs/spec/guide.md, 7.2): frozen as
# the Guide takes the foreground, thawed before the foreground goes back, and thawed at the Dashboard's next start if
# the Dashboard was killed with the Guide open. Last, with Settings' Pause the game while the Guide is open turned off
# (#270) in a data folder of the test's own: the Guide opens over the game, the log says `guide: the game is not
# frozen (Settings)` once, nothing is frozen, and the game runs on under the Guide (its pulse changing under the dim,
# its "game: beat N" lines counting, its threads not suspended).
#
# The game is the stand-in, whose pulse (a square towards its window's bottom right, crates/standin/src/win.rs) changes
# colour four times a second while it runs. A game that is frozen paints nothing, so the pulse standing still on the
# screen is the proof by pixels; the Dashboard's `freeze:` and `overlay:` lines, the stand-in's own "stood still" line
# and the state of the game's threads are the proof by what was logged.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
$overlayTitle = 'Xbox Series Dashboard Overlay'
$gameBlue = @(16, 32, 80)
# The Guide at rest, as the smoke test looks for it: its panel, and the game beside it under the Guide's dim.
$panel = @(21, 38, 17)
$dimmedGame = @(7, 14, 35)
$guidePoints = @(
    @{ X = 270; Y = 900; What = "the Guide's panel"; Colour = $panel }
    @{ X = 634; Y = 900; What = 'the game dimmed, 20 beside the panel'; Colour = $dimmedGame }
    @{ X = 1230; Y = 700; What = 'the game dimmed'; Colour = $dimmedGame }
)
# The record of a frozen game, in the Dashboard's data folder (crates/shell/src/freeze.rs).
$record = Join-Path (Get-DashboardDataDir) 'frozen-game.txt'

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }
function Format-Colours($Colours) { (@($Colours) | ForEach-Object { Format-Colour $_ }) -join ', ' }

# The lines the Dashboard logged after the first $After that match $Pattern. Callers wrap the call in @( ) to count or
# index what comes back: one line alone comes back as a string.
function Find-DashboardLog([string] $Pattern, [int] $After = 0) {
    @(Get-DashboardLog | Select-Object -Skip $After | Where-Object { $_ -match $Pattern })
}

# Watches the pulse for $Seconds and says in $What what it saw. $Moving is what is expected of it; $Dimmed whether the
# Guide's dim is over it. Returns what is wrong, as lines for $failures: none when it is as expected.
function Assert-Pulse([string] $What, [bool] $Moving, [bool] $Dimmed, [double] $Seconds = 3) {
    $seen = @(Watch-Pulse $Seconds)
    Say "${What}: the pulse showed $($seen.Count) colour(s) in $Seconds s: $(Format-Colours $seen)"
    $wrong = @()
    $either = @($seen | Where-Object { -not (Test-PulseColour $_ $Dimmed) })
    if ($either.Count) { $wrong += "${What}: the pulse's place shows $(Format-Colours $either), which is not the pulse$(if ($Dimmed) { ' under the dim' })" }
    if ($Moving -and $seen.Count -lt 2) { $wrong += "${What}: the game's pulse stood still for $Seconds s: the game is not running" }
    if (-not $Moving -and $seen.Count -ne 1) { $wrong += "${What}: the game's pulse changed colour: the game is not frozen" }
    $wrong
}

# The stand-in's "game: beat N" lines, one a second while it runs.
function Get-Beats {
    @(Get-Content (Join-Path $Out 'standin.log') -ErrorAction SilentlyContinue | Where-Object { $_ -match 'game: beat \d+' })
}

# The last place --log-focus said focus is, without its prefix.
function Get-LastFocus {
    $lines = @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' })
    if ($lines.Count) { $lines[-1] -replace '^focus: ', '' } else { '' }
}

# One press, and the line it must bring.
function Step([string] $Button, [string] $Line) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $hit = Wait-DashboardLog $Line 8 -After $mark
    Say "$Button`: $(if ($hit) { $hit } else { "NO line '$Line' (focus: $(Get-LastFocus))" })"
    if (-not $hit) { throw "$Button did not bring '$Line'" }
}

# A press while walking somewhere: one that brings no line is said, and the walk goes by the log again.
function Walk([string] $Button, [string] $Line) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $hit = Wait-DashboardLog $Line 5 -After $mark
    Say "$Button`: $(if ($hit) { $hit } else { "note: no line '$Line' of this press; the walk goes on" })"
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam
    $game = @($library.games | Where-Object last_played -gt 0)[0]
    if (-not $game -or -not $game.installed) { throw "The Recent row's first game isn't an installed one" }
    Say "the game: $($game.name) ($($game.source_id)), in $($game.install_dir)"
    # Nothing in this test is a question put to the user, however slow the VM.
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    if (Test-Path $record) { Remove-Item $record -Force; Say "note: a record of a frozen game was there before the test: removed" }

    $dashboard = Start-Dashboard @('--log-focus')
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Start-Sleep -Seconds 3
    # The greeting's Toast comes and goes: a tap of the Guide button during it opens the Guide somewhere else.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) {
        $taken = Set-Foreground $dashboardTitle
        Say "note: the Dashboard was not in front after starting; put in front: $taken; foreground $(Front)"
        if (-not $taken) { $failures += "the Dashboard could not be put in front: $(Front)" }
    }
    $made = Wait-DashboardLog '^overlay: configured' 15
    Say "overlay line: $(if ($made) { $made } else { 'none' })"
    if (-not $made) { throw 'The Overlay was not made' }
    $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $monitor = "over the monitor at \($($screen.X), $($screen.Y)\), $($screen.Width)x$($screen.Height),"

    # ---- The Guide at Home: on the Overlay too, which takes the foreground from Home's window and gives it back.
    $homeAt = Get-DesignPixel 270 900
    Say "Home at (270, 900): $(Format-Colour $homeAt)"
    $mark = Get-DashboardLogLength
    Push-Button Guide
    $shown = Wait-DashboardLog '^overlay: shown \(Guide\)' 10 -After $mark
    $panelUp = Wait-Until { Test-Colour (Get-DesignPixel 270 900) $panel 6 } 6
    Save-Screen 'guide-at-home'
    Say "Guide at Home: $(if ($shown) { $shown } else { 'no overlay: shown (Guide) line' }); foreground $(Front); at (270, 900) $(Format-Colour (Get-DesignPixel 270 900))"
    if (-not $shown) { $failures += 'at Home the Guide did not open on the Overlay' }
    if (-not $panelUp) { $failures += "at Home the Guide's panel is not on screen at (270, 900): seen $(Format-Colour (Get-DesignPixel 270 900)), expected $(Format-Colour $panel)" }
    if ((Get-Foreground).Title -ne $overlayTitle) { $failures += "at Home the Guide did not take the foreground: $(Front)" }
    $where = @(Find-DashboardLog '^overlay: over the monitor' $mark)
    Say "which monitor: $(if ($where.Count) { $where[-1] } else { 'no line' })"
    if (-not $where.Count -or $where[-1] -notmatch $monitor -or $where[-1] -notmatch [regex]::Escape("title `"$dashboardTitle`"")) {
        $failures += "at Home the Overlay did not go over the monitor Home's window is on ($($screen.Width)x$($screen.Height) at $($screen.X), $($screen.Y)): $($where -join ' | ')"
    }
    if (@(Find-DashboardLog '^freeze:').Count) { $failures += "something was frozen at Home, with no game running: $((Find-DashboardLog '^freeze:') -join ' | ')" }

    $mark = Get-DashboardLogLength
    Push-Button B
    $back = Wait-DashboardLog '^overlay: foreground given back to ' 10 -After $mark
    $hidden = Wait-DashboardLog '^overlay: hidden' 10 -After $mark
    $gone = Wait-Until { -not (Test-Colour (Get-DesignPixel 270 900) $panel 6) } 6
    Say "after B at Home: $(if ($back) { $back } else { 'no foreground given back line' }); $(if ($hidden) { $hidden } else { 'no hidden line' }); foreground $(Front)"
    if ($back -notmatch [regex]::Escape("title `"$dashboardTitle`"") -or $back -notmatch ': true at ') { $failures += "Home's window did not get the foreground back from the Guide: $back" }
    if (-not $hidden -or -not $gone) { $failures += 'the Guide did not leave the screen at Home' }
    if ((Get-Foreground).Title -ne $dashboardTitle) { $failures += "the Dashboard is not in front after the Guide closed at Home: $(Front)" }

    # ---- The game.
    Push-Button Down
    $focus = Wait-DashboardLog '^focus: Recent\(0\)' 10
    if (-not $focus) { throw "Down did not put focus on the Recent row's first tile" }
    # A is the game's Game hub, after the double tap's 300 ms; A on its Play is the launch.
    Push-Button A
    if (-not (Wait-DashboardLog '^focus: GameHub: \d+ Hub Play$' 10)) { throw "A did not open the game's hub (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    Push-Button A
    $up = Wait-DashboardLog 'Launch screen up' 10
    if (-not $up) { throw "A on Play did not start a launch (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    if (-not (Wait-DashboardLog 'the game has booted' 60)) { throw "The game never counted as booted (launch log: $((Find-DashboardLog '^launch:') -join ' | '))" }
    if (-not (Wait-DashboardLog 'Launch screen gone' 10)) { $failures += 'the Launch screen did not go' }
    Start-Sleep -Milliseconds 500
    Save-Screen 'game'
    $games = @(Get-StandInGame)
    Say "in the game; foreground $(Front); game processes: $(($games | ForEach-Object Id) -join ', ')"
    if ($games.Count -ne 1) { throw "There are $($games.Count) stand-in games running, not one" }
    $gamePid = $games[0].Id
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the game is not in front after the Launch screen: $(Front)" }
    $failures += @(Assert-Pulse 'in the game' $true $false)
    if (Test-Suspended $gamePid) { $failures += 'the game is suspended before the Guide has opened' }
    $stoodBefore = @(Get-StoodStill).Count
    if ($stoodBefore) { Say "note: the game says it stood still $stoodBefore time(s) before the Guide opened: a slow machine" }

    # ---- The Guide over the game: on its monitor, the foreground taken, the game frozen.
    $mark = Get-DashboardLogLength
    Push-Button Guide
    $opened = Wait-DashboardLog 'Guide over the game' 10 -After $mark
    $froze = Wait-DashboardLog '^freeze: froze ' 10 -After $mark
    $atRest = Wait-Until { -not @($guidePoints | Where-Object { -not (Test-Colour (Get-DesignPixel $_.X $_.Y) $_.Colour 6) }).Count } 6
    Save-Screen 'guide-over-game'
    Say "after Guide: $(if ($opened) { $opened } else { 'no Guide over the game line' }); $(if ($froze) { $froze } else { 'no freeze: froze line' }); foreground $(Front)"
    if (-not $opened) { $failures += 'the Guide did not open over the game' }
    foreach ($p in $guidePoints) {
        $seen = Get-DesignPixel $p.X $p.Y
        Say "Guide at ($($p.X), $($p.Y)), $($p.What): seen $(Format-Colour $seen), expected $(Format-Colour $p.Colour)"
        if (-not (Test-Colour $seen $p.Colour 6)) { $failures += "the Guide is not on screen over the game: at ($($p.X), $($p.Y)) expected $($p.What) $(Format-Colour $p.Colour), seen $(Format-Colour $seen)" }
    }
    if (-not $atRest) { Say 'note: the Guide took more than 6 s to come to rest' }
    if ((Get-Foreground).Title -ne $overlayTitle) { $failures += "the Guide did not take the foreground from the game: $(Front)" }
    $where = @(Find-DashboardLog '^overlay: over the monitor' $mark)
    Say "which monitor: $(if ($where.Count) { $where[-1] } else { 'no line' })"
    if (-not $where.Count -or $where[-1] -notmatch $monitor -or $where[-1] -notmatch [regex]::Escape("title `"$($game.name)`"")) {
        $failures += "the Overlay did not go over the monitor the game is on: $($where -join ' | ')"
    }
    if ($froze -notmatch "game \($gamePid\)") { $failures += "the Dashboard did not freeze the game, process ${gamePid}: $froze" }
    # In the log's order: the foreground taken first, then the game frozen.
    $lines = @(Get-DashboardLog | Select-Object -Skip $mark)
    $took = [array]::FindIndex([string[]] $lines, [Predicate[string]] { param($l) $l -match '^overlay: foreground was ' })
    $frozeAt = [array]::FindIndex([string[]] $lines, [Predicate[string]] { param($l) $l -match '^freeze: froze ' })
    if ($took -lt 0 -or $frozeAt -lt $took) { $failures += "the game was not frozen after the Overlay took the foreground (lines $took and $frozeAt)" }
    $recorded = if (Test-Path $record) { (Get-Content $record -Raw).Trim() } else { $null }
    Say "the record: $(if ($recorded) { $recorded } else { 'none' })"
    if ($recorded -notmatch "^$gamePid \d+ game$") { $failures += "the record of the frozen game does not name process ${gamePid}: $recorded" }
    $suspended = Test-Suspended $gamePid
    Say "the game's threads all suspended: $suspended"
    if (-not $suspended) { $failures += "the game's threads are not all suspended with the Guide open" }
    $failures += @(Assert-Pulse 'under the Guide' $false $true)

    # ---- B: the game thawed, then handed the foreground, once B is up.
    $mark = Get-DashboardLogLength
    Push-Button B
    $thawed = Wait-DashboardLog '^freeze: thawed ' 10 -After $mark
    $back = Wait-DashboardLog '^overlay: foreground given back to ' 10 -After $mark
    $closed = Wait-DashboardLog 'Guide closed' 10 -After $mark
    $away = Wait-Until { -not @($guidePoints | Where-Object { -not (Test-Colour (Get-DesignPixel $_.X $_.Y) $gameBlue 6) }).Count } 6
    Save-Screen 'guide-closed'
    Say "after B: $(if ($thawed) { $thawed } else { 'no freeze: thawed line' }); $(if ($back) { $back } else { 'no foreground given back line' }); foreground $(Front)"
    if (-not $closed) { $failures += 'the Guide did not close' }
    if ($thawed -notmatch "game \($gamePid\)") { $failures += "the Dashboard did not thaw the game as the Guide closed: $thawed" }
    if ($back -notmatch [regex]::Escape("title `"$($game.name)`"") -or $back -notmatch ': true at ') { $failures += "the game was not handed the foreground back: $back" }
    $lines = @(Get-DashboardLog | Select-Object -Skip $mark)
    $thawedAt = [array]::FindIndex([string[]] $lines, [Predicate[string]] { param($l) $l -match '^freeze: thawed ' })
    $backAt = [array]::FindIndex([string[]] $lines, [Predicate[string]] { param($l) $l -match '^overlay: foreground given back to ' })
    if ($thawedAt -lt 0 -or $backAt -lt $thawedAt) { $failures += "the game was not thawed before it was handed the foreground (lines $thawedAt and $backAt)" }
    if (-not $away) { $failures += "the Guide is still on screen 6 s after closing" }
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the game did not get the foreground back: $(Front)" }
    if (Test-Path $record) { $failures += "the record of the frozen game is still there after the thaw: $((Get-Content $record -Raw).Trim())" }
    if (Test-Suspended $gamePid) { $failures += "the game's threads are still suspended after the Guide closed" }
    $failures += @(Assert-Pulse 'after the Guide closed' $true $false)
    $stood = @(Get-StoodStill)
    Say "the game's own account: $(if ($stood.Count) { $stood -join ' | ' } else { 'no stood still line' })"
    if ($stood.Count -le $stoodBefore) { $failures += 'the game does not say that it stood still while the Guide was open' }

    # ---- The Dashboard killed with the Guide open: the game stays frozen, until the Dashboard's next start.
    $mark = Get-DashboardLogLength
    Push-Button Guide
    $froze = Wait-DashboardLog '^freeze: froze ' 10 -After $mark
    Say "Guide again: $(if ($froze) { $froze } else { 'no freeze: froze line' })"
    if ($froze -notmatch "game \($gamePid\)") { throw "The game was not frozen the second time the Guide opened: $froze" }
    Start-Sleep -Milliseconds 500
    Stop-Process -Id $dashboard.Id -Force
    $dashboard.WaitForExit(5000) | Out-Null
    Say "killed the Dashboard with the Guide open: exited $($dashboard.HasExited)"
    Start-Sleep -Seconds 1
    Save-Screen 'dashboard-killed'
    $recorded = if (Test-Path $record) { (Get-Content $record -Raw).Trim() } else { $null }
    Say "the record after the kill: $(if ($recorded) { $recorded } else { 'none' }); the game's threads all suspended: $(Test-Suspended $gamePid); foreground $(Front)"
    if ($recorded -notmatch "^$gamePid \d+ game$") { $failures += "no record of the frozen game was left by the Dashboard that was killed: $recorded" }
    if (-not (Test-Suspended $gamePid)) { $failures += 'the game is not frozen after the Dashboard was killed: the test proves nothing of the next start' }
    # The Overlay went with the Dashboard: the game shows as it stood, undimmed.
    $failures += @(Assert-Pulse 'with the Dashboard killed' $false $false)

    Copy-Item $DashboardLog (Join-Path $Out 'dashboard-first.log') -Force
    $dashboard = Start-Dashboard @('--log-focus')
    $leftover = Wait-DashboardLog '^freeze: the Dashboard stopped last time with a game frozen: thawing ' 60
    $thawed = Wait-DashboardLog '^freeze: thawed ' 10
    $first = Wait-DashboardLog 'first frame presented' 60
    Say "the Dashboard started again: $(if ($leftover) { $leftover } else { 'no leftover line' }); $(if ($thawed) { $thawed } else { 'no freeze: thawed line' })"
    if ($leftover -notmatch "game \($gamePid\)" -or $thawed -notmatch "game \($gamePid\)") { $failures += "the Dashboard did not thaw at its start the game its last run left frozen: $leftover | $thawed" }
    if (-not $first) { $failures += 'the Dashboard did not draw Home after starting again' }
    if (Test-Path $record) { $failures += "the record is still there after the start that thawed the game: $((Get-Content $record -Raw).Trim())" }
    if (Test-Suspended $gamePid) { $failures += "the game's threads are still suspended after the Dashboard started again" }
    $stoodAgain = @(Get-StoodStill)
    Say "the game's own account: $($stoodAgain -join ' | ')"
    if ($stoodAgain.Count -le $stood.Count) { $failures += 'the game does not say that it stood still until the Dashboard started again' }
    # The greeting's Toast of this start comes and goes, over the game's pulse.
    if ($first) { Wait-Greeting | Out-Null }
    # Home's window is over the game now: the game in front again, to see its pulse.
    $front = Set-Foreground $game.name
    Say "the game put in front: $front; foreground $(Front)"
    if (-not $front) { $failures += "the game could not be put in front after the Dashboard started again: $(Front)" }
    Save-Screen 'thawed-after-restart'
    $failures += @(Assert-Pulse 'after the Dashboard started again' $true $false)
    if ($dashboard.HasExited) { $failures += "the Dashboard exited with $($dashboard.ExitCode)" }

    # ---- Settings' Pause the game off (#270): the Guide over the game, and the game runs on under it.
    # A data folder of the test's own, so the switch turned off here is never left off for the Dashboard's own.
    Copy-Item $DashboardLog (Join-Path $Out 'dashboard-second.log') -Force
    Stop-StandInGame
    Stop-Dashboard
    $data = Get-TestFolder 'guide-hold-unpaused'
    Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue
    New-Item $data -ItemType Directory -Force | Out-Null
    $unpausedRecord = Join-Path $data 'frozen-game.txt'
    $dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame (Pause the game off)' }
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 2
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    for ($i = 0; $i -lt 6 -and (Get-LastFocus) -notmatch 'TopBar\(Settings\)$'; $i++) { Walk Right '^focus: ' }
    if ((Get-LastFocus) -notmatch 'TopBar\(Settings\)$') { throw "Focus never reached the top bar's Settings (focus: $(Get-LastFocus))" }
    Step A '^focus: Settings: Rail: General$'
    Step Right '^focus: Settings: General: Personalisation$'
    Step Down '^focus: Settings: General: Guide$'
    Step A '^focus: Settings: Guide: Use the Guide$'
    Step Down '^focus: Settings: Guide: Pause the game while the Guide is open$'
    $mark = Get-DashboardLogLength
    Push-Button A
    $saved = Wait-DashboardLog '^settings: guide\.pause_game = false$' 8 -After $mark
    Save-Screen 'pause-the-game-off'
    Say "Pause the game turned off: $(if ($saved) { $saved } else { 'no settings: guide.pause_game = false line' })"
    if (-not $saved) { throw 'A on Pause the game did not turn it off' }
    Step B '^focus: Settings: General: Guide$'
    Step B '^focus: Settings: Rail: General$'
    # Out of Settings, Home's focus where it was: a line if it says one.
    Walk B '^focus: TopBar\('
    # Down to the Recent row, then Left to its first tile, by the log.
    Step Down '^focus: Recent\(\d+\)$'
    for ($i = 0; $i -lt 10 -and (Get-LastFocus) -ne 'Recent(0)'; $i++) { Walk Left '^focus: Recent\(' }
    if ((Get-LastFocus) -ne 'Recent(0)') { throw "Focus never reached the Recent row's first tile (focus: $(Get-LastFocus))" }
    Push-Button A
    if (-not (Wait-DashboardLog '^focus: GameHub: \d+ Hub Play$' 10)) { throw "A did not open the game's hub (Pause the game off)" }
    $mark = Get-DashboardLogLength
    Push-Button A
    if (-not (Wait-DashboardLog 'the game has booted' 60 -After $mark)) { throw "The game never counted as booted (Pause the game off)" }
    if (-not (Wait-DashboardLog 'Launch screen gone' 10 -After $mark)) { $failures += 'the Launch screen did not go (Pause the game off)' }
    Start-Sleep -Milliseconds 500
    $games = @(Get-StandInGame)
    if ($games.Count -ne 1) { throw "There are $($games.Count) stand-in games running, not one (Pause the game off)" }
    $gamePid = $games[0].Id
    Say "the game again, with Pause the game off: process $gamePid; foreground $(Front)"
    $stoodBefore = @(Get-StoodStill).Count

    $mark = Get-DashboardLogLength
    $beatsBefore = @(Get-Beats).Count
    Push-Button Guide
    $opened = Wait-DashboardLog 'Guide over the game' 10 -After $mark
    $notFrozen = Wait-DashboardLog '^guide: the game is not frozen \(Settings\)$' 10 -After $mark
    $atRest = Wait-Until { Test-Colour (Get-DesignPixel 270 900) $panel 6 } 6
    Save-Screen 'guide-over-game-unpaused'
    Say "Guide with Pause the game off: $(if ($opened) { $opened } else { 'no Guide over the game line' }); $(if ($notFrozen) { $notFrozen } else { 'no not frozen line' }); foreground $(Front)"
    if (-not $opened) { $failures += 'the Guide did not open over the game with Pause the game off' }
    if (-not $atRest) { $failures += "the Guide's panel is not on screen over the game with Pause the game off" }
    if ((Get-Foreground).Title -ne $overlayTitle) { $failures += "the Guide did not take the foreground with Pause the game off: $(Front)" }
    if (-not $notFrozen) { $failures += "no 'guide: the game is not frozen (Settings)' line as the Guide opened with Pause the game off" }
    # The game under the Guide: its pulse moving under the dim, its own log counting, its threads running.
    $failures += @(Assert-Pulse 'under the Guide, Pause the game off' $true $true)
    $beatsUnder = @(Get-Beats).Count - $beatsBefore
    Say "the game's beats said since the Guide opened: $beatsUnder; its threads all suspended: $(Test-Suspended $gamePid)"
    if ($beatsUnder -lt 2) { $failures += "the game's log did not keep counting under the Guide with Pause the game off ($beatsUnder beat lines)" }
    if (Test-Suspended $gamePid) { $failures += "the game's threads are suspended under the Guide with Pause the game off" }
    if (Test-Path $unpausedRecord) { $failures += "a record of a frozen game was written with Pause the game off: $((Get-Content $unpausedRecord -Raw).Trim())" }
    $saidTimes = @(Find-DashboardLog '^guide: the game is not frozen \(Settings\)$' $mark).Count
    if ($saidTimes -ne 1) { $failures += "the not frozen line came $saidTimes times in one Guide, not once" }

    $mark2 = Get-DashboardLogLength
    Push-Button B
    $closed = Wait-DashboardLog 'Guide closed' 10 -After $mark2
    $back = Wait-DashboardLog '^overlay: foreground given back to ' 10 -After $mark2
    Save-Screen 'guide-closed-unpaused'
    Say "after B, Pause the game off: $(if ($back) { $back } else { 'no foreground given back line' }); foreground $(Front)"
    if (-not $closed) { $failures += 'the Guide did not close with Pause the game off' }
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the game did not get the foreground back with Pause the game off: $(Front)" }
    $freezeLines = @(Find-DashboardLog '^freeze: ' $mark)
    if ($freezeLines.Count) { $failures += "the game was frozen or thawed with Pause the game off: $($freezeLines -join ' | ')" }
    if (@(Get-StoodStill).Count -gt $stoodBefore) { $failures += "the game says it stood still under the Guide with Pause the game off: $((Get-StoodStill) -join ' | ')" }
    if ($dashboard.HasExited) { $failures += "the Dashboard exited with $($dashboard.ExitCode) (Pause the game off)" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
    Say "at the error: foreground $(Front)"
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Remove-Item $record -Force -ErrorAction SilentlyContinue
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
