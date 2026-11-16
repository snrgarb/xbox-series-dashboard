# The Game hub, walked with the virtual pad on a fake Steam library: A on a game's tile opens its hub once the double
# tap's time has passed; focus goes along its actions and up to its cards; the last action turns between its two
# pages; Manage and Menu open the game's Tile menu over the hub, and B closes it onto the hub; B is Home again with
# focus on the tile; a game that is not installed has Install where Play is, and A there closes the hub and asks
# Steam for the game; and a double tap on a tile plays the game with no hub.
#
# Judged by what --log-focus says after each press, a press at a time: `focus: GameHub: 9100010 Hub Play` is the hub
# of the game with that appid, on its Game hub page, focus on Play (the page's name, the game, the page of the two,
# what has focus; the main action is named by what it is: Play, Running or Install). Passes when every press is
# followed by the line expected of it, the double tap starts the game, and the Dashboard stays up. The screenshots are
# to look at, beside the Bundle's pictures of its hub.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then a rest for the picture, if one is asked for: the software renderer takes most of a second a frame.
# The hub opens 300 ms after A, which the wait for its line covers.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    # Two taps closer than a poll of the pad are one: the button has to be seen up between them.
    Start-Sleep -Milliseconds 130
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($Name) {
        Start-Sleep -Milliseconds 4500
        Save-Screen $Name
    }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
}

# One press that must leave focus where it is: an edge, or a button with nothing to do there.
function Press-Nothing([string] $Button, [string] $Why) {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    Start-Sleep -Milliseconds 1500
    if (@(Focus-Lines).Count -eq $before) {
        Say "$Button -> nothing ($Why)"
    } else {
        Say "$Button -> moved focus, and should not have ($Why): $(Focus-Now)"
        $script:failures += "$Button should have done nothing ($Why), and the log's last is '$(Focus-Now)'"
    }
}

# A line the Dashboard must log after the mark $After, by this pattern.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What) {
    $line = Wait-DashboardLog $Pattern 10 $After
    if ($line) { Say "${What}: $line" } else { Say "${What}: no line like '$Pattern'"; $script:failures += "${What}: the Dashboard logged no '$Pattern'" }
}

# A line the stand-in Steam must have written: what it was started with.
function Expect-Steam([string] $Pattern, [string] $What) {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    $line = Wait-Until { if (Test-Path $log) { Get-Content $log | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } } 10
    if ($line) { Say "${What}: $line" } else { Say "${What}: the stand-in Steam has no line like '$Pattern'"; $script:failures += "${What}: the stand-in Steam was not started with '$Pattern'" }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    # Nothing in this test is a question put to the user, however slow the VM.
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus')
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    # The greeting's Toast comes and goes: it is not in the pictures, and the Guide button is the Guide's again.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # The Recent row's first game, which is installed and has been played. One A: its hub, focus on Play.
    Press Down 'Recent\(0\)'
    Press A 'GameHub: \d+ Hub Play' '01-hub-play-focused'
    $game = if ((Focus-Now) -match 'GameHub: (\d+) ') { $Matches[1] } else { throw "No hub opened: $(Focus-Now)" }
    $hub = "GameHub: $game"

    # Along the actions and back, stopping at both ends. Manage says its name over itself.
    Press-Nothing Left 'Play is the first action'
    Press Right "$hub Hub Manage" '02-hub-manage-focused'
    Press Right "$hub Hub Toggle"
    Press-Nothing Right 'the toggle is the last action'
    Press-Nothing Down 'nothing is under the actions'

    # Up to the cards and down again: the stats card is over the toggle, the cover over Play.
    Press Up "$hub Hub Stats" '03-hub-card-focused'
    Press-Nothing A 'a card does nothing'
    Press-Nothing Up 'nothing over the cards takes focus'
    Press Left "$hub Hub Cover"
    Press-Nothing Left 'the cover is the first card'
    Press Down "$hub Hub Play"
    Press Up "$hub Hub Cover"
    Press Down "$hub Hub Play"

    # The toggle turns to Game details and back; focus stays on it.
    Press Right "$hub Hub Manage"
    Press Right "$hub Hub Toggle"
    Press A "$hub Details Toggle" '04-hub-details'
    Press Up "$hub Details Trailer"
    Press Down "$hub Details Toggle"
    Press A "$hub Hub Toggle"
    # Y is Search on Home, and nothing here.
    Press-Nothing Y 'Y does nothing in the hub'

    # Manage, and Menu wherever focus is, open the game's Tile menu over the hub, which has no Manage game row; B
    # closes it, and focus is where it was.
    Press Left "$hub Hub Manage"
    Press A 'TileMenu: Menu\((Play game|Install)\) of .+' '04b-hub-tile-menu'
    Press Down 'TileMenu: Menu\((Add to|Remove from) Favourites\) of .+'
    Press Down 'TileMenu: Menu\((Uninstall|Customise Home)\) of .+'
    Press B "$hub Hub Manage"
    Press Up "$hub Hub Stats"
    Press Start 'TileMenu: Menu\((Play game|Install)\) of .+'
    Press B "$hub Hub Stats"
    Press Down "$hub Hub Toggle"

    # B: Home, focus on the tile.
    Press B 'Recent\(0\)' '05-back-on-the-tile'

    # Along the Recent row to a game that is not installed, a hub at a time: its main action is Install, with the
    # store's name under it. Which tile that is is the library's to say.
    $install = $false
    foreach ($tile in 1..6) {
        Press Right "Recent\($tile\)"
        Press A 'GameHub: \d+ Hub (Play|Install)'
        if ((Focus-Now) -match 'Hub Install$') {
            $install = $true
            Start-Sleep -Milliseconds 4500
            Save-Screen '06-hub-install'
            # A on Install is the Tile menu's Install: the hub closes, and Steam is asked for the game.
            $mark = Get-DashboardLogLength
            Press A "Recent\($tile\)"
            Expect-Logged '^store: asking Steam to install .+: app_install \d+$' $mark 'Install'
            Expect-Steam 'steam: asked to install \d+: -silent \+app_install \d+' 'Install'
            if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }
            break
        }
        Press B "Recent\($tile\)"
    }
    if (-not $install) { $failures += 'no game of the Recent row had Install in its hub' }

    # A double tap on the first tile plays the game, and no hub opens. The two presses are 60 ms long and as close
    # as the pad can make them; on a VM too slow to tell them from two taps the hub opens between them and the
    # second A is its Play, which is said and is not a failure: the game starts either way.
    foreach ($press in 1..8) {
        if ((Focus-Now) -match 'Recent\(0\)$') { break }
        Push-Button Left
        Start-Sleep -Milliseconds 800
    }
    if ((Focus-Now) -notmatch 'Recent\(0\)$') { throw "Left never reached the Recent row's first tile: $(Focus-Now)" }
    $before = @(Focus-Lines).Count
    Push-Button A 60
    # Up long enough for the pad's reader to see the button let go, or the two are one press.
    Start-Sleep -Milliseconds 110
    Push-Button A 60
    $up = Wait-DashboardLog 'Launch screen up' 10
    Say "after the double tap: $(if ($up) { $up } else { 'no Launch screen up line' })"
    if (-not $up) { throw "A double tap did not start a launch (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    if ($up -notmatch "\($game\)") { $failures += "the double tap launched something other than the game $game`: $up" }
    $between = @(Focus-Lines | Select-Object -Skip $before)
    if ($between.Count) { Say "note: the hub opened between the two presses, too far apart on this VM to be a double tap: $($between -join ' | ')" }
    if (-not (Wait-DashboardLog 'the game has booted' 60)) { throw 'The game never counted as booted' }
    Start-Sleep -Milliseconds 1500
    Save-Screen '07-game-after-a-double-tap'

    # The game ends: Home again, and the pad is Home's.
    if (-not @(Get-StandInGame).Count) { $failures += 'the game was not running when the test came to stop it' }
    Stop-StandInGame
    $ended = Wait-DashboardLog 'the game has ended, Home back' 20
    if (-not $ended) { $failures += 'Home did not come back after the game ended' }
    Start-Sleep -Milliseconds 1500
    Press Right 'Recent\(1\)' '08-home-again'

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
