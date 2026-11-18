# Epic and GOG beside Steam: a fake Epic Games Launcher and a fake GOG are made next to the fake Steam
# (Initialize-FakeEpic, Initialize-FakeGog), and the Dashboard, started with `--epic` and `--gog` naming them, reads
# all three. For each of the two stores, one installed game is found by name through Search (Y on Home, its name's
# first letter typed on the Console keyboard, B, then along the results until the log names it), its Game hub is
# opened, and Play starts it: an Epic game through the launcher's link, which the stand-in launcher takes; a GOG game as
# its own program. The Launch screen comes up, the stand-in game starts, it is ended, and Home comes back.
#
# Judged by the Dashboard's log: each source's line as it is read (`epic: 3 games from C:\e2e\epic (...) in 4 ms`), the
# focus log after each press (`focus: Search: Games(0) Kiteborne; "K" away, Games 1`, `focus: GameHub: <id> Hub Play`),
# the launch's lines (`launch: Kiteborne (<id>) asked of Epic Games, Launch screen up at ...`, `launch: the game has
# booted, ...`, `launch: the game has ended, Home back at ...`); and for Epic the stand-in launcher's own line in
# standin.log. No place on the keys or among the results is counted: a walk goes until the log names what it wants.
# Which games and how many there are is the fixtures' to say (crates/source-epic/src/fixture.rs,
# crates/source-gog/src/fixture.rs), read from what the fakes were read back as. Passes when every press is followed by
# the line expected of it, both games start and end, and the Dashboard stays up. The pictures show each store's
# own mark on its game's tile and its name in the Game hub, to be looked at.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
# C:\e2e\<store> on the VM; under the payload's own folder in safe mode.
$epicRoot = Get-FakeRoot 'epic'
$gogRoot = Get-FakeRoot 'gog'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# Two taps closer than this can be seen as one held press on a VM whose frames take most of a second.
$Gap = 150

function Tap([string] $Button) {
    Push-Button $Button
    Start-Sleep -Milliseconds $Gap
}

function Picture([string] $Name) {
    Start-Sleep -Milliseconds 4500
    Save-Screen $Name
}

# Waits for a focus line matching $Pattern (what follows "focus: ", a regular expression, whole) among those after the
# first $Before; returns it, or $null after $Seconds.
function Wait-Focus([int] $Before, [string] $Pattern, [double] $Seconds) {
    Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before -and $lines[-1] -match "^focus: $Pattern`$") { $lines[-1] } } $Seconds
}

# One press that must be followed by a new focus line matching $Expect. A press the Dashboard never saw (no new focus
# line at all) is pressed once more; one that gave another line is a failure. Returns the line.
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

# Presses $Button until the focus log names the key $Key, $Most times at most, then $Back until it does, if it went
# past it. Judged by where the walk ends, never by how many presses it took.
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
    if ((Focus-Now) -notmatch $pattern) { throw "never reached the key $Key on the Console keyboard: '$(Focus-Now)'" }
    Say "walked to ${Key}: $(Focus-Now)"
}

# The source's line as it was read, `<key>: <n> games from <dir> in <ms> ms`, which must say as many games as the fake
# was read back with, from $Root (unless $Games is -1: any number, from anywhere); and no other line of the source's,
# which would be one saying it could not be read. After <dir> a source may say which of its files gave what, in
# brackets (Epic: `C:\e2e\epic (InstallHelper 1 of 2, manifests 1 of 3, catcache 3 of 5)`).
function Expect-Read([string] $Key, [string] $Root, [int] $Games) {
    $line = Wait-DashboardLog "^${Key}: \d+ games from .+ in \d+ ms`$" 60
    if (-not $line) { $script:failures += "the Dashboard never said it read ${Key}: no '${Key}: <n> games from' line"; return }
    Say "read: $line"
    if ($Games -ge 0 -and $line -notmatch "^${Key}: $Games games from $([regex]::Escape($Root))( \(.+\))? in \d+ ms`$") {
        $script:failures += "${Key} should have been read as $Games games from ${Root}: '$line'"
    }
    $other = @(Get-DashboardLog | Where-Object { $_ -match "^${Key}: " -and $_ -notmatch "^${Key}: \d+ games from " })
    if ($other.Count) { $script:failures += "${Key} said more than what it read: $($other -join ' | ')" }
}

# Finds $Game by name through Search, opens its Game hub, plays it, and ends it: the Launch screen, the stand-in game
# in front, Home again. $Store is the store's name as the launch line says it; $Tag names the pictures.
function Play-ByName($Game, [string] $Store, [string] $Tag) {
    $name = $Game.name
    $letter = $name.Substring(0, 1).ToUpper()
    Say "--- $Store`: $name ($($Game.source_id))"

    # Y on Home: Search, empty, on Q.
    Press Y 'Search: Key\(Q\); .*' | Out-Null
    if ((Focus-Now) -notmatch '^focus: Search: Key\(Q\);') { throw "Y did not open Search on Q: $(Focus-Now)" }

    # The name's first letter, by its row: down the first column to the row's first key, then along it.
    if ('ASDFGHJKL'.Contains($letter)) { Walk-ToKey Down Up 'A' }
    elseif ('ZXCVBNM'.Contains($letter)) { Walk-ToKey Down Up 'Z' }
    Walk-ToKey Right Left $letter
    $typed = Press A "Search: Key\($letter\); `"$letter\|`" .*"
    if (-not $typed) { throw "typing $letter did not show in the log: $(Focus-Now)" }

    # B: the keyboard goes, focus on the first game found; then along the results until the log names the game.
    Press B "Search: \w+\(0\) .+; `"$letter`" away.*" | Out-Null
    $named = "^focus: Search: \w+\(\d+\) $([regex]::Escape($name));"
    foreach ($press in 1..12) {
        if ((Focus-Now) -match $named) { break }
        Tap Right
        Start-Sleep -Milliseconds 650
    }
    if ((Focus-Now) -notmatch $named) { throw "Search for '$letter' never put focus on $name`: $(Focus-Now)" }
    Say "found: $(Focus-Now)"
    Picture "$Tag-1-found"

    # A: its Game hub, on Play. The hub writes the store's name.
    $id = [regex]::Escape($Game.source_id)
    $hub = Press A "GameHub: $id Hub Play" "$Tag-2-hub"
    if (-not $hub) { throw "A did not open $name's hub on Play: $(Focus-Now)" }

    # A on Play: the Launch screen, and the stand-in game starts and comes in front.
    $mark = Get-DashboardLogLength
    Start-Sleep -Milliseconds 130
    Push-Button A
    $up = Wait-DashboardLog "^launch: $([regex]::Escape($name)) \($id\) asked of $([regex]::Escape($Store)), Launch screen up at \d+ ms`$" 15 $mark
    Say "after A on Play: $(if ($up) { $up } else { 'no Launch screen up line' })"
    if (-not $up) { throw "A on Play did not start $name with the Launch screen (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    $running = Wait-Until { @(Get-StandInGame).Count } 30
    if (-not $running) { $script:failures += "$name's stand-in game never started" }
    $booted = Wait-DashboardLog '^launch: the game has booted' 60 $mark
    Say "booted: $(if ($booted) { $booted } else { 'no line in 60 s' })"
    if (-not $booted) { $script:failures += "$name never counted as booted" }
    $front = Wait-Until { (Get-Foreground).Title -eq $name } 20
    Say "$name in front: $([bool] $front); foreground '$((Get-Foreground).Title)'"
    if (-not $front) { $script:failures += "$name's window did not come in front: '$((Get-Foreground).Title)'" }
    Start-Sleep -Milliseconds 1500
    Save-Screen "$Tag-3-playing"

    # Ended: Home again, in front.
    Stop-StandInGame
    $ended = Wait-DashboardLog '^launch: the game has ended, Home back at \d+ ms$' 20 $mark
    Say "after the game: $(if ($ended) { $ended } else { 'no game has ended line in 20 s' })"
    if (-not $ended) { $script:failures += "Home did not come back after $name ended" }
    Start-Sleep -Milliseconds 1500
    if ((Get-Foreground).Title -ne $dashboardTitle) {
        Say "note: the Dashboard was not in front after $name; put in front: $(Set-Foreground $dashboardTitle)"
    }
    Save-Screen "$Tag-4-home-again"
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    $epic = Initialize-FakeEpic $epicRoot
    $gog = Initialize-FakeGog $gogRoot
    $epicGame = @($epic.games | Where-Object installed) | Select-Object -First 1
    $gogGame = @($gog.games | Where-Object installed) | Select-Object -First 1
    if (-not $epicGame -or -not $gogGame) { throw 'the fakes should each have an installed game' }

    # Nothing in this test is a question put to the user, however slow the VM.
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--epic', "`"$epicRoot`"", '--gog', "`"$gogRoot`"")
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"

    # Each store read, with all its games.
    Expect-Read 'steam' '' -1
    Expect-Read 'epic' $epicRoot (@($epic.games).Count)
    Expect-Read 'gog' $gogRoot (@($gog.games).Count)

    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    Press Down 'Recent\(0\)' '0-home' | Out-Null

    Play-ByName $epicGame 'Epic Games' 'epic'
    $line = Get-Content (Join-Path $env:E2E_OUT 'standin.log') -ErrorAction SilentlyContinue |
        Where-Object { $_ -match "epic-url: $([regex]::Escape($epicGame.source_id)) running, " } | Select-Object -Last 1
    if ($line) { Say "the stand-in launcher: $line" } else { $failures += 'the stand-in launcher never said it started the Epic game' }

    Play-ByName $gogGame 'GOG' 'gog'

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
