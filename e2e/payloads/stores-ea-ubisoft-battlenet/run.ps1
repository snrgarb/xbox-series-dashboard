# EA, Ubisoft and Battle.net (#75): a fake of each beside the fake Steam, read by the Dashboard from its own folder
# (--ea, --ubisoft, --battlenet). Each store's games are in the library: the line the library logs for each source as
# it reads it, `ea: 2 games from C:\e2e\ea in 3 ms`, says how many. Then one game of each, found by name through Search
# (its first letter typed on the Console keyboard, then along the Games row to it, by the focus log), is launched from
# its Game hub: the Launch screen's line says it was asked of its store; the stand-in game starts, from the store's own
# way of starting it (EA: the game's own program; Ubisoft: uplay://, the stand-in Ubisoft Connect; Battle.net: the
# stand-in Battle.net.exe, --exec="launch <code>"); it is ended, and Home comes back.
#
# Judged by the Dashboard's log lines and --log-focus's after each press, a press at a time. No position in a row or on
# the keyboard is counted: a walk goes until the log names the key or the game. Passes when every store's games were
# read, each of the three games launched, booted, ended with Home back, and the Dashboard stayed up. The screenshots of
# each Game hub show the store's placeholder mark on the cover card (a square, a ring, a bar; Steam's is a disc).
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

# The game of each store to launch, its name's first letter (a key no other is walked to through) and the key that
# starts that letter's row on the keyboard: Q, A or Z, all three down its first column.
$stores = @(
    @{ Store = 'ea'; Make = 'Initialize-FakeEa'; Name = 'EA app'; Root = 'C:\e2e\ea'; Game = 'Vantage Reef'; Letter = 'V'; Row = 'Z' },
    @{ Store = 'ubisoft'; Make = 'Initialize-FakeUbisoft'; Name = 'Ubisoft Connect'; Root = 'C:\e2e\ubisoft'; Game = 'Oxbow Frontier'; Letter = 'X'; Row = 'Z' },
    @{ Store = 'battlenet'; Make = 'Initialize-FakeBattleNet'; Name = 'Battle.net'; Root = 'C:\e2e\battlenet'; Game = 'Juniper Vault'; Letter = 'J'; Row = 'A' }
)
$dashboardTitle = 'Xbox Series Dashboard'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One tap, and the rest after it that keeps the next from running into it: two taps closer than a poll of the pad are
# one press.
function Tap([string] $Button) {
    Push-Button $Button
    Start-Sleep -Milliseconds 150
}

# Waits for a focus line matching $Pattern (what follows "focus: ", a regular expression) among those after the first
# $Before; returns it, or $null after $Seconds.
function Wait-Focus([int] $Before, [string] $Pattern, [double] $Seconds) {
    Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before -and $lines[-1] -match "^focus: $Pattern`$") { $lines[-1] } } $Seconds
}

# One press that must be followed by a new focus line matching $Expect. A press the Dashboard never saw (no new focus
# line at all) is pressed once more; one that gave another line is a failure. Returns the line.
function Press([string] $Button, [string] $Expect) {
    $before = @(Focus-Lines).Count
    Tap $Button
    $got = Wait-Focus $before $Expect 8
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Tap $Button
        $got = Wait-Focus $before $Expect 8
    }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# Presses $Button until the focus log matches $Pattern (a whole focus line), $Most times at most, then $Back until it
# does: rows and keys do not wrap, so going on to the edge and coming back finds what was passed. Returns whether it
# got there.
function Walk-To([string] $Button, [string] $Back, [string] $Pattern, [string] $What, [int] $Most = 12) {
    foreach ($way in @($Button, $Back)) {
        foreach ($press in 1..$Most) {
            if ((Focus-Now) -match $Pattern) { break }
            Tap $way
            Start-Sleep -Milliseconds 650
        }
        if ((Focus-Now) -match $Pattern) { break }
    }
    if ((Focus-Now) -match $Pattern) {
        Say "walked to ${What}: $(Focus-Now)"
        return $true
    }
    Say "never reached ${What}: $(Focus-Now)"
    $script:failures += "never reached ${What}: '$(Focus-Now)'"
    return $false
}

function Walk-ToKey([string] $Button, [string] $Back, [string] $Key) {
    Walk-To $Button $Back "^focus: Search: Key\($([regex]::Escape($Key))\);" "the key $Key" | Out-Null
}

# A line the stand-ins wrote in standin.log, matching $Pattern, within $Seconds.
function Wait-StandIn([string] $Pattern, [double] $Seconds = 15) {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    Wait-Until { if (Test-Path $log) { Get-Content $log | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } } $Seconds
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    $arguments = @('--log-focus')
    foreach ($store in $stores) {
        $library = & $store.Make $store.Root
        $store.Games = @($library.games).Count
        $game = @($library.games | Where-Object { $_.name -eq $store.Game }) | Select-Object -First 1
        if (-not $game -or -not $game.installed) { throw "The fake $($store.Store) has no installed $($store.Game)" }
        $store.Id = $game.source_id
        $store.Dir = $game.install_dir
        $arguments += @("--$($store.Store)", $store.Root)
    }
    # Nothing in this test is a question put to the user, however slow the VM.
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard $arguments
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"

    # Each store read, with as many games as its reader read from the fake when it was made.
    foreach ($store in $stores) {
        $line = Wait-DashboardLog "^$($store.Store): $($store.Games) games from $([regex]::Escape($store.Root)) in \d+ ms`$" 60
        if ($line) { Say "read: $line" } else {
            $said = @(Get-DashboardLog | Where-Object { $_ -match "^$($store.Store): " }) -join ' | '
            $failures += "the library did not read $($store.Games) games of $($store.Store) from $($store.Root): '$said'"
        }
    }

    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    Press Down 'Recent\(0\)' | Out-Null

    foreach ($store in $stores) {
        $game = [regex]::Escape($store.Game)
        Say "--- $($store.Name): $($store.Game) ($($store.Id))"
        if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }

        # Search, its first letter typed: down the keyboard's first column to its row, along the row to it.
        $opened = Press Y 'Search: Key\(Q\); "\|" upper letters, RecentlyAdded \d+'
        if (-not $opened) { $failures += "Search did not open for $($store.Game)"; continue }
        Walk-ToKey Down Up $store.Row
        Walk-ToKey Right Left $store.Letter
        $typed = Press A "Search: Key\($($store.Letter)\); `"$($store.Letter)\|`" lower letters, Games \d+"
        $found = if ($typed -match ', Games (\d+)$') { [int] $Matches[1] } else { 12 }

        # B: the keyboard away, focus on the first game found; along the row to the game, by its name.
        Press B "Search: Games\(0\) .+; `"$($store.Letter)`" away, Games \d+" | Out-Null
        if (-not (Walk-To Right Left "^focus: Search: Games\(\d+\) $game; " $store.Game ($found + 1))) { continue }

        # A: its Game hub, focus on Play.
        $hub = Press A "GameHub: $([regex]::Escape($store.Id)) Hub Play"
        if (-not $hub) { continue }
        Start-Sleep -Milliseconds 4500
        Save-Screen "$($store.Store)-hub"

        # A on Play: the store is asked for it, and the Launch screen comes up.
        $mark = Get-DashboardLogLength
        Tap A
        $asked = "^launch: $game \($([regex]::Escape($store.Id))\) asked of $([regex]::Escape($store.Name)), Launch screen up at \d+ ms`$"
        $up = Wait-DashboardLog $asked 20 $mark
        if (-not $up) {
            $said = @(Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^launch: ' }) -join ' | '
            $failures += "$($store.Game) was not launched through $($store.Name): '$said'"
            continue
        }
        Say "launched: $up"

        # The stand-in game, from the game's folder; through the store's stand-in where it has one.
        switch ($store.Store) {
            'ubisoft' { $by = Wait-StandIn "uplay-url: $($store.Id) running" }
            'battlenet' { $by = Wait-StandIn "battlenet: $($store.Id) running" }
            default { $by = 'its own game.exe' }
        }
        if (-not $by) { $failures += "the stand-in $($store.Name) did not start $($store.Game)" } else { Say "started by: $by" }
        $running = Wait-Until { @(Get-StandInGame | Where-Object { $_.Path -and $_.Path.StartsWith($store.Dir.TrimEnd('\', '/'), [StringComparison]::OrdinalIgnoreCase) }).Count } 30
        if (-not $running) { $failures += "no stand-in game ran from $($store.Dir)" }
        $booted = Wait-DashboardLog '^launch: the game has booted, Launch screen fading at \d+ ms$' 60 $mark
        if ($booted) { Say "booted: $booted" } else { $failures += "$($store.Game) never counted as booted" }
        $front = Wait-Until { (Get-Foreground).Title -eq $store.Game } 10
        Say "the game in front: $([bool] $front); foreground '$((Get-Foreground).Title)'"
        Start-Sleep -Milliseconds 1500
        Save-Screen "$($store.Store)-game"

        # It ends: Home again.
        Stop-StandInGame
        $ended = Wait-DashboardLog '^launch: the game has ended, Home back at \d+ ms$' 30 $mark
        if ($ended) { Say "ended: $ended" } else { $failures += "Home did not come back after $($store.Game) ended" }
        Start-Sleep -Milliseconds 1500
        Save-Screen "$($store.Store)-home-again"
    }

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Get-Process 'Battle.net' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
