# Search, walked with the virtual pad on the fake Steam library: Y on Home opens it empty on Q with Recently added;
# "Co" is typed on the Console keyboard by walking to each key and pressing A, the games found following each key;
# LT's symbols key turns the letters to symbols and back; B puts the keyboard away with focus on the first game
# found; A there opens its Game hub at once, and B comes back to the game; B again is Home.
#
# Judged by what --log-focus says after each press, a press at a time (docs/spec/search.md, "The focus log"):
# `focus: Search: Key(C); "C|" lower letters, Games 3` is the page, the key with focus, the text with a bar at the
# caret, Shift and the layer, then each row of results and how many it holds; with the keyboard away the text alone
# and `away`, and focus on a tile is its row, its place and its game: `focus: Search: Games(0) Copperhead Station;
# "Co" away, Games 3`. No place on the keys is counted: a walk goes until the log names the key. What "Co" finds is
# the fixture's to say (source_steam::fixture::Fixture::standard): Copperhead Station starts with it, Ashfall
# Courier has a word that does, Nightjar Protocol has it inside a word; the first is checked by name, the count is
# said. Passes when every press is followed by the line expected of it and the Dashboard stays up.
#
# The pictures 15, 16, 17 and 18 are named as the Bundle's of the same states
# (~/projects/xbox-series-theme/artifacts/e2e/port-reference-20261115-192720/), to be looked at beside them; here the
# fonts and the keys' glyphs are Windows' own, which no test on Linux draws.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# The Dashboard reads the pad every 4 ms, but on the VM a frame can take most of a second: two taps closer than this
# can be seen as one held press.
$Gap = 150

# One tap, and the rest after it that keeps the next from running into it.
function Tap([string] $Button) {
    Push-Button $Button
    Start-Sleep -Milliseconds $Gap
}

# The picture of the screen now, once it has come to rest: the software renderer takes a second or so a frame.
function Picture([string] $Name) {
    Start-Sleep -Milliseconds 4500
    Save-Screen $Name
}

# Waits for a focus line matching $Pattern (what follows "focus: ", a regular expression) among those after the first
# $Before; returns it, or $null after $Seconds.
function Wait-Focus([int] $Before, [string] $Pattern, [double] $Seconds) {
    Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before -and $lines[-1] -match "^focus: $Pattern`$") { $lines[-1] } } $Seconds
}

# One press that must be followed by a new focus line matching $Expect. A press the Dashboard never saw (no new focus
# line at all, where one was due) is pressed once more; one that was seen and gave another line is a failure. Then
# the picture, if one is asked for.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Tap $Button
    $got = Wait-Focus $before $Expect 8
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Tap $Button
        $got = Wait-Focus $before $Expect 8
    }
    if ($Name) { Picture $Name }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# Presses $Button until the focus log names the key $Key, $Most times at most, then, if it went past it (a press that
# counted twice where the VM's clock jumped), $Back until it does. The keys do not wrap, so going on to the edge and
# coming back finds a key that was passed. A walk is judged by where it ends, never by how many presses it took.
function Walk-ToKey([string] $Button, [string] $Back, [string] $Key, [int] $Most = 12) {
    $pattern = "^focus: Search: Key\($([regex]::Escape($Key))\);"
    foreach ($way in @($Button, $Back)) {
        foreach ($press in 1..$Most) {
            if ((Focus-Now) -match $pattern) { break }
            Tap $way
            Start-Sleep -Milliseconds 650
        }
        if ((Focus-Now) -match $pattern) { break }
    }
    if ((Focus-Now) -match $pattern) {
        Say "walked to $Key`: $(Focus-Now)"
    } else {
        Say "never reached the key $Key`: $(Focus-Now)"
        $script:failures += "never reached the key $Key on the Console keyboard: '$(Focus-Now)'"
    }
}

# How many games the Games row holds, from a focus line; -1 when it has no such row.
function Games-In([string] $Line) {
    if ($Line -match ', Games (\d+)') { [int] $Matches[1] } else { -1 }
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
    # The greeting's Toast comes and goes: it is not in the pictures.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # Home's Recent row, then Y: Search, empty, on Q with Shift on, Recently added under the box.
    Press Down 'Recent\(0\)' | Out-Null
    $opened = Press Y 'Search: Key\(Q\); "\|" upper letters, RecentlyAdded \d+' '15-search-empty-keyboard-shift'
    if ($opened -match 'RecentlyAdded (\d+)$') { Say "Recently added holds $($Matches[1]) games" }

    # "C": down the first column to Z's row, along it to C. Shift goes off with the first character.
    Walk-ToKey Down Up 'Z'
    Walk-ToKey Right Left 'C'
    $c = Press A 'Search: Key\(C\); "C\|" lower letters, (Games \d+|no results)'
    Say "C finds $(Games-In $c) games"

    # "o": up the column to E's row, along it to O. The games found follow at once; focus stays on the keys.
    Walk-ToKey Up Down 'E'
    Walk-ToKey Right Left 'O'
    $co = Press A 'Search: Key\(O\); "Co\|" lower letters, Games \d+' '16-search-typed-keyboard-letters'
    Say "Co finds $(Games-In $co) games"

    # The symbols key, at the top row's start: the letters turn to symbols, and back.
    Walk-ToKey Up Down '9'
    Walk-ToKey Left Right 'Symbols'
    Press A 'Search: Key\(Symbols\); "Co\|" lower symbols, Games \d+' '17-search-keyboard-symbols' | Out-Null
    Press A 'Search: Key\(Symbols\); "Co\|" lower letters, Games \d+' | Out-Null

    # B: the keyboard goes, the text stays, and focus is on the first game found, the one whose name starts with it.
    Press B 'Search: Games\(0\) Copperhead Station; "Co" away, Games \d+' '18-search-result-focused' | Out-Null

    # A: its Game hub, at once; B: back on the same game, the keyboard still away.
    Press A 'GameHub: \d+ Hub (Play|Install)' '19-search-game-hub' | Out-Null
    Press B 'Search: Games\(0\) Copperhead Station; "Co" away, Games \d+' | Out-Null

    # B again: Home, focus on the Recent row where Y was pressed.
    Press B 'Recent\(0\)' '20-home-again' | Out-Null

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
