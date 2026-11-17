# The Game hub, walked with the virtual pad on a fake Steam library: A on a game's tile opens its hub at once (no
# double tap since 2026-11-17, docs/spec/game-hub.md 16.2), its first frame within 50 ms of A; focus goes along its
# actions and up to its cards; the last action turns between its two pages; Manage and Menu open the game's Tile menu over the hub, and B closes it onto the hub; B is Home again with
# focus on the tile; a game that is not installed has Install where Play is, and A there closes the hub and asks
# Steam for the game; A on Your game stats opens the game's Achievements page and B comes back; and A twice, as close
# as the pad can make them, on a tile plays the game by way of its hub: the second A is not swallowed. Over the
# running game, the Guide's Home and A on the game's tile is its hub on Running, and A there is the game again.
#
# Judged by what --log-focus says after each press, a press at a time: `focus: GameHub: 9100010 Hub Play` is the hub
# of the game with that appid, on its Game hub page, focus on Play (the page's name, the game, the page of the two,
# what has focus; the main action is named by what it is: Play, Running or Install). Passes when every press is
# followed by the line expected of it, A twice starts the game, and the Dashboard stays up. The screenshots are
# to look at, beside the Bundle's pictures of its hub.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then a rest for the picture, if one is asked for: the software renderer takes most of a second a frame.
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

# How long the hub took to open (#214), by the line the Dashboard logs for every hub once it has come to rest:
# `hub: opened <game> in N ms: first frame N ms, last moving frame N ms after A (at rest, drawn on the GPU); A
# answered by a frame at N ms; put up N ms after A ...`. There is no double tap to wait for: what is judged is the
# time from A to the hub's first frame being done, which nothing the hub waits on (a picture, its achievements) may
# hold: at most $HubFrameGpuMs on a GPU, and $HubFrameCpuMs on the processor (WARP on a VM, where a frame takes most
# of a second).
$HubFrameGpuMs = 50
$HubFrameCpuMs = 3000
function Expect-HubOpenedQuickly([int] $After, [string] $What) {
    $line = Wait-DashboardLog '^hub: opened ' 15 $After
    if (-not $line) { Say "${What}: no 'hub: opened' line"; $script:failures += "${What}: the Dashboard logged no 'hub: opened' line"; return }
    Say "${What}: $line"
    if ($line -notmatch 'first frame (\d+) ms, last moving frame (\d+) ms after A') { $script:failures += "${What}: the hub's line does not count from A: $line"; return }
    $first = [double] $Matches[1]
    if ($line -match 'double tap wait') { $script:failures += "${What}: the hub's line still has a double tap's wait: $line" }
    $bound = if ($line -match 'drawn on the processor') { $HubFrameCpuMs } else { $HubFrameGpuMs }
    if ($first -gt $bound) {
        $script:failures += "${What}: the hub's first frame was done $first ms after A, over the bound of $bound ms: $line"
    } else {
        Say "${What}: first frame $first ms after A (bound $bound ms)"
    }
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
    $mark = Get-DashboardLogLength
    Press A 'GameHub: \d+ Hub Play' '01-hub-play-focused'
    Expect-HubOpenedQuickly $mark 'the first hub since start-up'
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
    # A on Your game stats of a game with achievements (the fake Steam's Ashfall Courier has six) opens its
    # Achievements page in the hub's place, focus on the newest unlock (#141); B is the hub again, on the card.
    Press A "Achievements: $game ACH_\w+; \d+ of \d+ unlocked" '03b-achievements-page'
    Press B "$hub Hub Stats"
    Press-Nothing Up 'nothing over the cards takes focus'
    Press Left "$hub Hub Cover"
    Press-Nothing A 'the cover card does nothing'
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
    Press A 'TileMenu: Menu\((Play game|Install|Pause installation|Resume installation|Add to Favourites|Remove from Favourites)\) of .+' '04b-hub-tile-menu'
    Press Down 'TileMenu: Menu\((Add to|Remove from) Favourites\) of .+'
    Press Down 'TileMenu: Menu\((Uninstall|Customise Home)\) of .+'
    Press B "$hub Hub Manage"
    Press Up "$hub Hub Stats"
    Press Start 'TileMenu: Menu\((Play game|Install|Pause installation|Resume installation|Add to Favourites|Remove from Favourites)\) of .+'
    Press B "$hub Hub Stats"
    Press Down "$hub Hub Toggle"

    # B: Home, focus on the tile.
    Press B 'Recent\(0\)' '05-back-on-the-tile'

    # Along the Recent row to a game that is not installed, a hub at a time: its main action is Install, with the
    # store's name under it. Which tile that is is the library's to say.
    $install = $false
    foreach ($tile in 1..6) {
        Press Right "Recent\($tile\)"
        $mark = Get-DashboardLogLength
        Press A 'GameHub: \d+ Hub (Play|Install)'
        Expect-HubOpenedQuickly $mark "the hub of Recent($tile)"
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

    # A twice on the first tile, as close as the pad can make them (60 ms each, 110 ms apart): the first opens the
    # hub at once, focus on Play, and the second is not swallowed by the hub's first frames: it is Play, and the game
    # starts.
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
    Say "after A twice: $(if ($up) { $up } else { 'no Launch screen up line' })"
    if (-not $up) { throw "A twice did not start a launch (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    if ($up -notmatch "\($game\)") { $failures += "A twice launched something other than the game $game`: $up" }
    $between = @(Focus-Lines | Select-Object -Skip $before)
    Say "focus between: $($between -join ' | ')"
    if (-not ($between | Where-Object { $_ -match "^focus: $hub Hub Play$" })) { $failures += "the first A did not open the hub on Play: $($between -join ' | ')" }
    if (-not (Wait-DashboardLog 'the game has booted' 60)) { throw 'The game never counted as booted' }
    Start-Sleep -Milliseconds 1500
    Save-Screen '07-game-after-a-twice'

    # The running game's own hub: the Guide's Home puts Home in front of the game, A on the game's tile is its hub
    # with focus on Running, and A there is the game again, as the tile's double tap used to be (#214).
    $mark = Get-DashboardLogLength
    Push-Button Guide
    if (-not (Wait-DashboardLog '^guide: Landing Entry\(0\)$' 10 $mark)) { throw "The Guide did not open over the game (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Logged '^launch: Home in front of the game' $mark "the Guide's Home over the game"
    foreach ($press in 1..8) {
        if ((Focus-Now) -match 'Recent\(0\)$') { break }
        Push-Button Left
        Start-Sleep -Milliseconds 800
    }
    Press A "$hub Hub Running" '07b-hub-running'
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Logged '^launch: back to the game' $mark 'A on Running'
    Start-Sleep -Milliseconds 1500
    if ((Get-Foreground).Title -eq 'Xbox Series Dashboard') { $failures += 'the Dashboard is still in front after A on Running' }

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
