# Games from the Guide (docs/spec/guide.md, 3.3, 6.3, 6.4, 7.3, 7.4), with two stand-in games:
#
#   1. X is started from Home. The Guide over it: A on its entry, the Running one, goes back to it.
#   2. A on Y, a game played before, asks "Quit X and start Y?". Cancel leaves X running. "Quit and start" quits X,
#      and when X has stopped the Guide closes and Y starts.
#   3. Home from the Guide over Y: the Dashboard in front of Y, which runs on behind it; Home takes the pad. The
#      Guide again lists Y as Running, and A on it goes back to it.
#   4. Quit in Y's menu: Y is asked to close, the Guide stays open, and closes onto Home once Y has stopped.
#   5. Y started again as a game that does not close when asked (`--ignore-close`): "Force quit Y?" ten seconds
#      after Quit. Cancel leaves it running; Quit again, and "Force quit" ends it.
#
# Judged by the Dashboard's `guide:` lines (--log-focus), a press at a time, its `launch:`, `overlay:` and `freeze:`
# lines, the window in front, which stand-in games run, and pixels over the stand-in game.
#
# Which entry of the Guide a game is comes from the library, not from a number written here: Home, My games & apps,
# then the running game, then the games played, most recently first (`Get-GuideEntry`).
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
$overlayTitle = 'Xbox Series Dashboard Overlay'
$gameBlue = @(16, 32, 80)
# The Guide at rest over the stand-in game, as the smoke test looks for it.
$panel = @(21, 38, 17)
$dimmedGame = @(7, 14, 35)
# Under a dialog: the menu panel beside its title, and the game under the Guide's dim and the dialog's
# (crates/screens/tests/shots.rs, guide_quit_and_start_prompt, asserts the same two points).
$menuPanel = @(43, 43, 43)
$dimmedTwice = @(2, 4, 11)

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }
function Fail([string] $What) { $script:failures += $What; Say "WRONG: $What" }

# The last `guide:` line the Dashboard logged, without its prefix; 'closed' before it has logged any.
function Get-GuideLine {
    $lines = @(Get-DashboardLog | Where-Object { $_ -match '^guide: ' })
    if ($lines.Count) { $lines[-1] -replace '^guide: ', '' } else { 'closed' }
}

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

# Opens the Guide and waits for it to be at rest on the landing tab, focus on Home.
function Open-Guide([string] $Name = '') {
    if (-not (Step Guide 'Landing Entry\(0\)$' $Name)) { throw "The Guide did not open (it is at: $(Get-GuideLine))" }
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

# Walks Down, a press at a time, until focus is on the Guide's entry $Entry.
function Move-ToEntry([int] $Entry) {
    for ($i = 0; $i -lt 12; $i++) {
        if ((Get-GuideLine) -match "^Landing Entry\($Entry\)") { return }
        if ((Get-GuideLine) -notmatch '^Landing Entry\((\d+)\)') { throw "The Guide is not on its list: $(Get-GuideLine)" }
        $next = [int] $Matches[1] + 1
        Walk Down "Landing Entry\($next\)"
    }
    throw "Focus never reached entry $Entry of the Guide: $(Get-GuideLine)"
}

# Where the Guide lists $Game while $Running runs: Home, My games & apps, the running game, then the games played,
# most recently first, the running one left out; four games at most. $null if it is not among them.
function Get-GuideEntry($Game, $Running, $Played) {
    $listed = @($Running) + @($Played | Where-Object { $_.source_id -ne $Running.source_id }) | Select-Object -First 4
    for ($i = 0; $i -lt $listed.Count; $i++) { if ($listed[$i].source_id -eq $Game.source_id) { return 2 + $i } }
    $null
}

# The stand-in games running now, as their folders' names.
function Get-RunningGames { @(Get-StandInGame | ForEach-Object { Split-Path (Split-Path $_.Path -Parent) -Leaf }) }

function Expect-Log([string] $Pattern, [int] $Mark, [string] $What, [double] $Seconds = 10) {
    $line = Wait-DashboardLog $Pattern $Seconds $Mark
    Say "${What}: $(if ($line) { $line } else { "NO line '$Pattern'" })"
    if (-not $line) { $script:failures += "$What was not logged ('$Pattern')" }
    $line
}

function Expect-Pixel([double] $X, [double] $Y, $Colour, [string] $What) {
    $ok = Wait-Until { Test-Colour (Get-DesignPixel $X $Y) $Colour 6 } 8
    $seen = Get-DesignPixel $X $Y
    Say "at ($X, $Y), ${What}: seen $(Format-Colour $seen), expected $(Format-Colour $Colour)"
    if (-not $ok) { $script:failures += "at ($X, $Y) expected $What $(Format-Colour $Colour), seen $(Format-Colour $seen)" }
}

function Expect-Front([string] $Title, [string] $When) {
    $ok = Wait-Until { (Get-Foreground).Title -eq $Title } 8
    Say "${When}: foreground $(Front)"
    if (-not $ok) { $script:failures += "${When}: '$Title' is not in front: $(Front)" }
}

# Waits for the game named $Name, started by the Dashboard, to be the one stand-in game running and in front.
function Wait-Game($Game, [int] $Mark) {
    if (-not (Wait-DashboardLog "^launch: $([regex]::Escape($Game.name)) \($($Game.source_id)\) asked of Steam" 15 $Mark)) {
        throw "$($Game.name) was not asked of Steam (launch log: $((Get-DashboardLog | Select-Object -Skip $Mark | Where-Object { $_ -match '^launch:' }) -join ' | '))"
    }
    if (-not (Wait-DashboardLog 'the game has booted' 60 $Mark)) { throw "$($Game.name) never counted as booted" }
    if (-not (Wait-DashboardLog 'Launch screen gone' 15 $Mark)) { Fail "the Launch screen did not go for $($Game.name)" }
    Start-Sleep -Milliseconds 500
    Expect-Front $Game.name "$($Game.name) started"
    $running = @(Get-RunningGames)
    Say "games running: $($running -join ', ')"
    if ($running.Count -ne 1) { Fail "$($running.Count) stand-in games are running after $($Game.name) started, not one: $($running -join ', ')" }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam
    $played = @($library.games | Where-Object last_played -gt 0)
    $x = $played[0]
    $y = @($played | Where-Object { $_.installed -and $_.source_id -ne $x.source_id })[0]
    if (-not $x -or -not $x.installed -or -not $y) { throw "The library has no two installed games that were played, the first of them the Recent row's first" }
    $yEntry = Get-GuideEntry $y $x $played
    if ($null -eq $yEntry) { throw "$($y.name) is not among the four games the Guide lists while $($x.name) runs" }
    Say "X: $($x.name) ($($x.source_id)); Y: $($y.name) ($($y.source_id)), the Guide's entry $yEntry while X runs"
    # Nothing in this test is a question put to the user, however slow the VM.
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    $dashboard = Start-Dashboard @('--log-focus')
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    # The greeting's Toast comes and goes: it is not in the pictures, and the Guide button is the Guide's again.
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 3
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    if (-not (Wait-DashboardLog '^overlay: configured' 15)) { throw 'The Overlay was not made' }

    # ---- 1. X from Home, and back to it from the Guide.
    Push-Button Down
    if (-not (Wait-DashboardLog '^focus: Recent\(0\)' 10)) { throw "Down did not put focus on the Recent row's first tile" }
    $mark = Get-DashboardLogLength
    # A is the game's Game hub, after the double tap's 300 ms; A on its Play is the launch.
    Push-Button A
    if (-not (Wait-DashboardLog '^focus: GameHub: \d+ Hub Play$' 10 $mark)) { throw "A did not open the game's hub (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    Push-Button A
    Wait-Game $x $mark

    Open-Guide
    Move-ToEntry 2
    Start-Sleep -Milliseconds 3000
    Save-Screen '01-running-entry'
    Expect-Pixel 270 900 $panel "the Guide's panel"
    Expect-Pixel 1230 700 $dimmedGame 'the game dimmed'
    $mark = Get-DashboardLogLength
    Step A 'closed$' | Out-Null
    $back = Expect-Log '^overlay: foreground given back to ' $mark 'A on the running game'
    if ($back -notmatch [regex]::Escape("title `"$($x.name)`"") -or $back -notmatch ': true at ') { Fail "the running game was not handed the foreground: $back" }
    Expect-Front $x.name 'after A on the running game'
    Expect-Pixel 1230 700 $gameBlue 'the game, the Guide gone'

    # ---- 2. Y while X runs: the question, Cancel, then Quit and start.
    $question = "Landing Entry\($yEntry\), dialog Menu \{ prompt: true, row: 0 \} `"Quit $([regex]::Escape($x.name)) and start $([regex]::Escape($y.name))\?`"$"
    Open-Guide
    Move-ToEntry $yEntry
    Step A $question '02-quit-and-start' | Out-Null
    Expect-Pixel 745 420 $menuPanel 'the prompt'
    Expect-Pixel 1230 900 $dimmedTwice "the game under the Guide's dim and the dialog's"
    Step Down "Landing Entry\($yEntry\), dialog Menu \{ prompt: true, row: 1 \}" | Out-Null
    Step A "Landing Entry\($yEntry\)$" | Out-Null
    $running = @(Get-RunningGames)
    if ($running.Count -ne 1) { Fail "after Cancel $($running.Count) stand-in games are running, not X alone" }
    if (@(Get-DashboardLog | Where-Object { $_ -match 'asked the game to quit' }).Count) { Fail 'Cancel asked the game to quit' }

    Step A $question | Out-Null
    $mark = Get-DashboardLogLength
    Step A "Landing Entry\($yEntry\), quitting, then $([regex]::Escape($y.name))$" | Out-Null
    Expect-Log '^launch: asked the game to quit' $mark 'Quit and start' | Out-Null
    # X stops; the Guide closes at once and Y is asked of Steam.
    Expect-Log '^launch: the game has ended' $mark 'X stopped' 30 | Out-Null
    $closed = Wait-DashboardLog '^guide: closed$' 10 $mark
    Say "after X stopped: $(if ($closed) { $closed } else { 'NO guide: closed line' })"
    if (-not $closed) { Fail 'the Guide did not close when the game quit for another had stopped' }
    Wait-Game $y $mark
    Save-Screen '03-y-started'
    if (@(Get-RunningGames) -notcontains (Split-Path $y.install_dir -Leaf)) { Fail "the game running is not Y: $((Get-RunningGames) -join ', ')" }

    # ---- 3. Home from the Guide over Y, and back to Y.
    Open-Guide
    $mark = Get-DashboardLogLength
    Step A 'closed$' | Out-Null
    Expect-Log '^launch: Home in front of the game' $mark 'Home from the Guide' | Out-Null
    $back = Expect-Log '^overlay: foreground given back to ' $mark 'Home from the Guide'
    if ($back -notmatch [regex]::Escape("title `"$dashboardTitle`"")) { Fail "Home's window was not handed the foreground: $back" }
    Expect-Front $dashboardTitle 'after Home from the Guide'
    Start-Sleep -Milliseconds 2000
    Save-Screen '04-home-in-front-of-y'
    if (Test-Colour (Get-DesignPixel 1230 700) $gameBlue 6) { Fail 'the game still shows where Home should be' }
    if (@(Get-RunningGames).Count -ne 1) { Fail "Y is not running behind Home: $((Get-RunningGames) -join ', ')" }
    # Home takes the pad: Up from the Recent row is the top bar, wherever in the row focus was, and Down comes back.
    $mark = Get-DashboardLogLength
    Push-Button Up
    $moved = Wait-DashboardLog '^focus: TopBar' 8 $mark
    Say "Up at Home in front of the game: $(if ($moved) { $moved } else { 'NO focus: TopBar line' })"
    if (-not $moved) { Fail 'Home did not take the pad in front of the running game' }
    $mark = Get-DashboardLogLength
    Push-Button Down
    if (-not (Wait-DashboardLog '^focus: Recent' 8 $mark)) { Fail 'Down did not bring focus back to the Recent row' }

    Open-Guide
    Move-ToEntry 2
    Start-Sleep -Milliseconds 3000
    Save-Screen '05-guide-over-home-y-running'
    $mark = Get-DashboardLogLength
    Step A 'closed$' | Out-Null
    Expect-Log '^launch: back to the game' $mark 'A on the running game, from Home' | Out-Null
    Expect-Front $y.name 'after A on the running game, from Home'
    Expect-Pixel 1230 700 $gameBlue 'the game in front again'

    # ---- 4. Quit from Y's menu: the Guide stays until Y has stopped, then closes onto Home.
    Open-Guide
    Move-ToEntry 2
    Step Start "Landing Entry\(2\), dialog Menu \{ prompt: false, row: 0 \} `"$([regex]::Escape($y.name))`"$" '06-running-game-menu' | Out-Null
    $mark = Get-DashboardLogLength
    Step A 'Landing Entry\(2\), quitting$' | Out-Null
    Expect-Log '^launch: asked the game to quit' $mark 'Quit' | Out-Null
    Expect-Log '^launch: the game has ended, Home back behind the Guide' $mark 'Y stopped under the Guide' 30 | Out-Null
    # The Guide's line without the game, then, half a second on, closed.
    $closed = Wait-DashboardLog '^guide: closed$' 10 $mark
    Say "after Y stopped: $(if ($closed) { $closed } else { "NO guide: closed line (the Guide is at: $(Get-GuideLine))" })"
    if (-not $closed) { Fail 'the Guide did not close after the game quit from it had stopped' }
    $back = Expect-Log '^overlay: foreground given back to ' $mark 'the Guide closing onto Home'
    if ($back -notmatch [regex]::Escape("title `"$dashboardTitle`"")) { Fail "Home's window was not handed the foreground after the quit: $back" }
    Expect-Front $dashboardTitle 'after the quit'
    if (@(Get-RunningGames).Count) { Fail "a stand-in game is still running after Quit: $((Get-RunningGames) -join ', ')" }
    Start-Sleep -Milliseconds 2000
    Save-Screen '07-home-after-quit'

    # ---- 5. Y again, as a game that does not close when asked: Force quit.
    Add-Content (Join-Path $y.install_dir 'game-args.txt') '--ignore-close' -Encoding ASCII
    # Home's focus is on the tile of the game just played.
    $mark = Get-DashboardLogLength
    # A is the game's Game hub, after the double tap's 300 ms; A on its Play is the launch.
    Push-Button A
    if (-not (Wait-DashboardLog '^focus: GameHub: \d+ Hub Play$' 10 $mark)) { throw "A did not open the game's hub (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    Push-Button A
    Wait-Game $y $mark
    Open-Guide
    Move-ToEntry 2
    Step Start 'Landing Entry\(2\), dialog Menu \{ prompt: false, row: 0 \}' | Out-Null
    $mark = Get-DashboardLogLength
    Step A 'Landing Entry\(2\), quitting$' | Out-Null
    $stayed = Wait-Until { Get-Content (Join-Path $Out 'standin.log') -ErrorAction SilentlyContinue | Where-Object { $_ -match 'game: asked to close, and staying' } } 10
    Say "the game's own account: $(if ($stayed) { @($stayed)[-1] } else { 'no asked to close line' })"
    if (-not $stayed) { Fail 'the game does not say that it was asked to close' }
    $force = "Landing Entry\(2\), dialog Menu \{ prompt: true, row: 0 \} `"Force quit $([regex]::Escape($y.name))\?`", quitting$"
    $asked = Wait-DashboardLog "^guide: $force" 20 $mark
    Say "ten seconds on: $(if ($asked) { $asked } else { "NO Force quit prompt (the Guide is at: $(Get-GuideLine))" })"
    if (-not $asked) { throw 'Force quit was not asked ten seconds after Quit' }
    Start-Sleep -Milliseconds 3000
    Save-Screen '08-force-quit'
    Expect-Pixel 745 420 $menuPanel 'the prompt'
    # Cancel: the game is left running, and the quit is over.
    Step Down 'Landing Entry\(2\), dialog Menu \{ prompt: true, row: 1 \}' | Out-Null
    Step A 'Landing Entry\(2\)$' | Out-Null
    if (@(Get-RunningGames).Count -ne 1) { Fail 'Cancel did not leave the game running' }
    # Quit again, and this time Force quit.
    Step Start 'Landing Entry\(2\), dialog Menu \{ prompt: false, row: 0 \}' | Out-Null
    $mark = Get-DashboardLogLength
    Step A 'Landing Entry\(2\), quitting$' | Out-Null
    if (-not (Wait-DashboardLog "^guide: $force" 20 $mark)) { throw 'Force quit was not asked the second time' }
    $mark = Get-DashboardLogLength
    Step A 'Landing Entry\(2\), quitting$' | Out-Null
    Expect-Log '^launch: force quit: process \d+ ended: true' $mark 'Force quit' | Out-Null
    Expect-Log '^launch: the game has ended' $mark 'Y stopped, forced' 30 | Out-Null
    $closed = Wait-DashboardLog '^guide: closed$' 10 $mark
    if (-not $closed) { Fail "the Guide did not close after the game was force quit (it is at: $(Get-GuideLine))" }
    Expect-Front $dashboardTitle 'after Force quit'
    if (@(Get-RunningGames).Count) { Fail "a stand-in game is still running after Force quit: $((Get-RunningGames) -join ', ')" }
    Start-Sleep -Milliseconds 2000
    Save-Screen '09-home-after-force-quit'
    if ($dashboard.HasExited) { Fail "the Dashboard exited with $($dashboard.ExitCode)" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
    Say "at the error: foreground $(Front); the Guide at: $(Get-GuideLine)"
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
