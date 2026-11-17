# Settings > Launchers (#163, docs/spec/launchers.md): one Dashboard on the fake Steam and the fake Epic, walked with
# the virtual pad.
#
# What it does, in order:
# - Opens Games & apps > Launchers with the pad: `focus: Settings: Launchers: Steam; on, <S> games; <S+E> games from
#   2 launchers; ready`.
# - Notes Home's Recent row (each tile's game named by its Tile menu: `focus: TileMenu: Menu(<row>) of <name>`) and
#   what Search finds for a played Steam game's whole name (`focus: Search: Key(..); "<name>|" ..., Games <n>`).
# - Switches Steam off: `launchers: steam off: <S> games hidden`, the Row `off, <S> games`, the count line `<E> games
#   from 1 launcher`; Home's Recent row has no Steam game and Search finds fewer games for the name.
# - Switches Steam on: `launchers: steam on: <S> games back`, the count line as it began, Steam read again at once
#   (`launchers: update: reading steam`, `launchers: update: steam read in <ms> ms: nothing new`); the Recent row as it
#   was, in the same order, which is the games' play history back (the row is the played games, last played first).
# - Changes the fake Steam while the Dashboard runs (`standin steam-change`): Paper Lighthouse added, installed, and
#   the third game of the Recent row played 45 minutes more, just now. Nothing comes in by itself.
# - Update now: `focus: Settings: Launchers: Update now; ...; updating` (it may be too quick to be seen: not failed),
#   then `launchers: update: reading every launcher switched on`, `launchers: update: <keys> read in <ms> ms: 1 new
#   game, ..., play time for 1`, and the focus line `...; <S+1+E> games from 2 launchers; updated: <the same>`. Then
#   Search finds Paper Lighthouse, and the game played heads the Recent row.
# Judged by the `--log-focus` lines and the `launchers:` lines of the Dashboard's log; a picture at each rest.
#
# Safe mode (the TV PC, E2E_SAFE=1): both fakes under the payload's folder (Get-FakeRoot), the data folder too
# (Get-TestFolder), every other store an empty folder (Start-Dashboard); nothing registered, nothing started but the
# Dashboard and the stand-in's own steam-change. No game is launched. The user's own launchers are never read.
#
# It ends as it began: the fake Steam written afresh by `standin make-steam`, which forgets the changes, and the data
# folder removed.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
Add-Type -AssemblyName System.Windows.Forms
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
$data = Get-TestFolder 'dashboard-launchers-data'
# The game `standin steam-change add-game` adds (crates/source-steam/src/fixture.rs, ADDED_GAME).
$added = 'Paper Lighthouse'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }
function Esc([string] $Text) { [regex]::Escape($Text) }

# The picture of the screen now, once what moved has come to rest.
function Picture([string] $Name) {
    Start-Sleep -Milliseconds 3000
    Save-Screen $Name
}

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". A press the Dashboard never saw (no new focus line at all) is pressed once more; one that gave another
# line is a failure. Then the picture, if one is asked for. Returns the line, or nothing.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    # Two taps closer than a poll of the pad are one: the button has to be seen up between them.
    Start-Sleep -Milliseconds 150
    $before = @(Focus-Lines).Count
    $pattern = "^focus: $Expect`$"
    Push-Button $Button
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Start-Sleep -Milliseconds 150
        Push-Button $Button
        $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
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

# Presses a direction until focus is where $Expect says, $Most times at most: a walk is judged by where it ends.
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

# Presses a direction until it moves focus no more: an end of a list or a row, which do not wrap.
function Walk-ToEdge([string] $Button, [int] $Most = 14) {
    foreach ($press in 1..$Most) {
        $was = Focus-Now
        Push-Button $Button
        Start-Sleep -Milliseconds 1500
        if ((Focus-Now) -eq $was) { break }
    }
    Say "$Button to the end: $(Focus-Now)"
}

# Waits for the last focus line to match $Expect, which a change of the library (not a press) may bring.
function Expect-Focus([string] $Expect, [string] $Why, [double] $Seconds = 10) {
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $now = Focus-Now; if ($now -match $pattern) { $now } } $Seconds
    if ($got) { Say "${Why}: $got" } else {
        Say "${Why}: NOT '$Expect': $(Focus-Now)"
        $script:failures += "${Why}: focus should be '$Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# A line the Dashboard must log after the mark $After, by this pattern, within $Seconds.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 15) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if ($line) { Say "${What}: $line" } else { Say "${What}: no line like '$Pattern'"; $script:failures += "${What}: the Dashboard logged no '$Pattern'" }
    $line
}

# No line by this pattern after the mark $After, $Seconds on.
function Expect-NotLogged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 3) {
    Start-Sleep -Seconds $Seconds
    $line = Get-DashboardLog | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -First 1
    if ($line) { Say "${What}: $line, and should not be"; $script:failures += "${What}: the Dashboard logged '$line'" } else { Say "${What}: nothing, as it should be" }
}

# The source's line as it was read, `<key>: <n> games from <dir> in <ms> ms`.
function Expect-Read([string] $Key, [int] $Games) {
    $line = Wait-DashboardLog "^${Key}: \d+ games from .+ in \d+ ms`$" 60
    if (-not $line) { $script:failures += "the Dashboard never said it read ${Key}"; return }
    Say "read: $line"
    if ($line -notmatch "^${Key}: $Games games from ") { $script:failures += "${Key} should have been read as $Games games: '$line'" }
}

# Text for SendKeys, its own special characters braced.
function Send-Text([string] $Text) {
    [System.Windows.Forms.SendKeys]::SendWait(($Text -replace '([+^%~(){}\[\]])', '{$1}'))
}

function In-Front { if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" } }

# Changes the fake Steam as Steam's own files would change: `standin steam-change <root> <what...>`.
function Change-Steam([string[]] $What) {
    $arguments = @('steam-change', "`"$script:steamRoot`"") + $What
    $done = Start-Process (Join-Path $PSScriptRoot 'standin.exe') -ArgumentList $arguments -Wait -PassThru
    Say "steam-change $($What -join ' '): exit $($done.ExitCode)"
    if ($done.ExitCode -ne 0) { $script:failures += "standin steam-change $($What -join ' ') exited with $($done.ExitCode)" }
}

# Home's top bar, on the button $Button: up to the bar, to its left end, then right until it is there.
function Go-TopBar([string] $Button) {
    Walk-ToEdge Up 4
    Walk-ToEdge Left 6
    Walk Right "TopBar\($Button\)" 5
}

# From Home to Settings > Launchers, focus on its first Row, Steam.
function Open-Launchers([string] $Steam) {
    Go-TopBar 'Settings'
    Press A 'Settings: Rail: General' | Out-Null
    Press Down 'Settings: Rail: Account' | Out-Null
    Press Down 'Settings: Rail: Games & apps' | Out-Null
    Press Right 'Settings: Games & apps: Launchers' | Out-Null
    Press A "Settings: Launchers: Steam; $Steam" | Out-Null
}

# From the Launchers page to Home's Recent row, a level at a time.
function Leave-Launchers {
    Press B 'Settings: Games & apps: Launchers' | Out-Null
    Press B 'Settings: Rail: Games & apps' | Out-Null
    Press B 'TopBar\(Settings\)' | Out-Null
    Press Down '(Recent\(\d+\)|.+)' | Out-Null
}

# The games of Home's Recent row, first to last, each named by its Tile menu. Ends on the row's first tile.
function Recent-Games {
    Walk-ToEdge Left 10
    $names = @()
    foreach ($tile in 1..10) {
        $before = @(Focus-Lines).Count
        Start-Sleep -Milliseconds 150
        Push-Button Start
        $line = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before) { $lines[-1] } } 4
        if ($line -match '^focus: TileMenu: Menu\(.+\) of (.+)$') {
            $names += $Matches[1]
            Press B 'Recent\(\d+\)' | Out-Null
        } elseif ($line) {
            # Not a game's tile (Play history's): back, and the row's end.
            Press B '.+' | Out-Null
            break
        } else {
            break
        }
        $was = Focus-Now
        Start-Sleep -Milliseconds 150
        Push-Button Right
        Start-Sleep -Milliseconds 1200
        if ((Focus-Now) -eq $was) { break }
    }
    Walk-ToEdge Left 10
    Say "the Recent row: $($names -join ', ')"
    , $names
}

# How many games Search finds for $Name typed whole (0 when it shows no Games row), and back to Home.
function Search-Count([string] $Name, [string] $Picture = '') {
    Press Y 'Search: Key\(Q\); .+' | Out-Null
    Send-Text $Name
    Start-Sleep -Milliseconds 2500
    $line = Focus-Now
    if ($Picture) { Picture $Picture }
    $found = if ($line -match ', Games (\d+)') { [int] $Matches[1] } else { 0 }
    Say "Search for '$Name': $found games ($line)"
    Walk B '(?!Search: |TextEntry: ).+' 4
    $found
}

try {
    Close-WindowsPrompts
    Connect-Pad

    # --- The fakes: Steam's standard library and Epic's. Every other store is an empty folder in safe mode.
    $script:steamRoot = Get-FakeRoot 'steam'
    $steam = Initialize-FakeSteam $script:steamRoot
    $epicRoot = Get-FakeRoot 'epic'
    $epic = Initialize-FakeEpic $epicRoot
    $S = @($steam.games).Count
    $E = @($epic.games).Count
    if ($S -lt 3 -or $E -lt 1) { throw "the fakes have $S Steam games and $E Epic games" }
    $played = @($steam.games | Where-Object { $_.last_played -gt 0 })
    if ($played.Count -lt 3) { throw "the fake Steam has $($played.Count) played games" }
    # The game to play more: the third most recently played, so that being played now puts it first.
    $more = $played[2]
    $searched = $played[0].name
    Say "Steam: $S games; Epic: $E games; to play more: $($more.name) ($($more.source_id)); searched: $searched"

    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    New-Item -ItemType Directory $data -Force | Out-Null
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $arguments = @('--log-focus', '--data-dir', "`"$data`"", '--epic', "`"$epicRoot`"")
    if (Test-SafeMode) { $arguments += @('--steam', "`"$script:steamRoot`"") }
    $script:dashboard = Start-Dashboard $arguments
    $started = Wait-DashboardLog 'first frame presented' 60
    if (-not $started) { throw 'The Dashboard never presented a frame' }
    Say "started: $started"
    Expect-Read 'steam' $S
    Expect-Read 'epic' $E
    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    In-Front

    $count = "$($S + $E) games from 2 launchers"

    # --- Before: Home's Recent row and Search.
    Press Down 'Recent\(\d+\)' | Out-Null
    $recentBefore = Recent-Games
    $steamNames = @($steam.games | ForEach-Object { $_.name })
    if (-not @($recentBefore | Where-Object { $steamNames -contains $_ }).Count) { $failures += "the Recent row has no Steam game as it begins: $($recentBefore -join ', ')" }
    Picture '01-home'
    $foundBefore = Search-Count $searched
    if ($foundBefore -lt 1) { $failures += "Search found nothing for $searched as it begins" }

    # --- The page.
    Open-Launchers "on, $S games; $count; ready"
    Picture '02-launchers'

    # --- Steam off.
    $mark = Get-DashboardLogLength
    Press A "Settings: Launchers: Steam; off, $S games; $E games? from 1 launcher; ready" '03-steam-off' | Out-Null
    Expect-Logged "^launchers: steam off: $S games hidden`$" $mark 'Steam off' | Out-Null
    Expect-NotLogged '^launchers: update: reading' $mark 'Steam off reads nothing' 2
    Leave-Launchers
    $recentOff = Recent-Games
    $left = @($recentOff | Where-Object { $steamNames -contains $_ })
    if ($left.Count) { $failures += "Steam is off, and the Recent row still has $($left -join ', ')" }
    Picture '04-home-steam-off'
    $foundOff = Search-Count $searched '05-search-steam-off'
    if ($foundOff -ge $foundBefore) { $failures += "Steam is off, and Search still finds $foundOff games for $searched ($foundBefore before)" }

    # --- Steam on again: everything back, and Steam read again at once.
    Open-Launchers "off, $S games; $E games? from 1 launcher; ready"
    $mark = Get-DashboardLogLength
    Press A "Settings: Launchers: Steam; on, $S games; $count; (ready|updating|updated: .+)" | Out-Null
    Expect-Logged "^launchers: steam on: $S games back`$" $mark 'Steam on' | Out-Null
    Expect-Logged '^launchers: update: reading steam$' $mark 'Steam read again' | Out-Null
    Expect-Logged '^launchers: update: steam read in \d+ ms: nothing new$' $mark 'nothing new in it' 30 | Out-Null
    Expect-Focus "Settings: Launchers: Steam; on, $S games; $count; updated: nothing new" 'the page after' | Out-Null
    Picture '06-steam-on'
    Leave-Launchers
    $recentOn = Recent-Games
    if (($recentOn -join '|') -ne ($recentBefore -join '|')) {
        $failures += "Steam is on again, and the Recent row is '$($recentOn -join ', ')', not '$($recentBefore -join ', ')' as before"
    }
    $foundOn = Search-Count $searched
    if ($foundOn -ne $foundBefore) { $failures += "Steam is on again, and Search finds $foundOn games for $searched, not $foundBefore" }

    # --- The fake Steam changes while the Dashboard runs. A game installed in Steam's own client comes in by itself
    # (the Queue's watch of what is installed, M8); play time does not: that is Update now's.
    $mark = Get-DashboardLogLength
    Change-Steam @('add-game')
    Expect-Logged "^queue: steam's installed games changed in its own client .+: reading it again$" $mark 'the game installed in Steam is seen' 30 | Out-Null
    Expect-Logged '^steam: \d+ games from ' $mark 'Steam read again' 15 | Out-Null
    if ((Search-Count $added) -lt 1) { $failures += "$added, installed in Steam, is not found by itself" }
    $mark = Get-DashboardLogLength
    Change-Steam @('played', $more.source_id, '45')
    Expect-NotLogged '^(launchers: update: |steam: \d+ games from )' $mark 'play time is not read by itself' 5
    $count = "$($S + 1 + $E) games from 2 launchers"
    $S = $S + 1

    # --- Update now.
    Open-Launchers "on, $S games; $count; .+"
    Walk Down "Settings: Launchers: Update now; $count; .+" 10
    $mark = Get-DashboardLogLength
    Press A "Settings: Launchers: Update now; .+" | Out-Null
    Expect-Logged '^launchers: update: reading every launcher switched on$' $mark 'Update now' | Out-Null
    $summary = Expect-Logged '^launchers: update: .*steam.* read in \d+ ms: .+$' $mark 'Update now read' 60
    if ($summary -and ($summary -match '\bnew game\b' -or $summary -notmatch '\bplay time for 1\b')) {
        $failures += "the update's summary should say play time for 1 and no new game: '$summary'"
    }
    $said = if ($summary -match ' ms: (.+)$') { Esc $Matches[1] } else { '.+' }
    $newCount = $count
    Expect-Focus "Settings: Launchers: Update now; $newCount; updated: $said" 'the summary on the page' 30 | Out-Null
    Picture '07-updated'
    # Pressed again once it is done, it updates again, and finds nothing new.
    $mark = Get-DashboardLogLength
    Press A "Settings: Launchers: Update now; .+" | Out-Null
    Expect-Logged '^launchers: update: .*steam.* read in \d+ ms: nothing new$' $mark 'Update now again' 60 | Out-Null
    Expect-Focus "Settings: Launchers: Update now; $newCount; updated: nothing new" 'nothing new on the page' 30 | Out-Null

    # --- Both are there: the game added found by Search; the game played heads the Recent row.
    Press B 'Settings: Games & apps: Launchers' | Out-Null
    Press B 'Settings: Rail: Games & apps' | Out-Null
    Press B 'TopBar\(Settings\)' | Out-Null
    Press Down 'Recent\(\d+\)' | Out-Null
    $recentAfter = Recent-Games
    if ($recentAfter[0] -ne $more.name) { $failures += "$($more.name) was played just now, and the Recent row begins '$($recentAfter -join ', ')'" }
    Picture '08-home-after'
    if ((Search-Count $added '09-search-added') -lt 1) { $failures += "Search does not find $added after Update now" }

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
    # As it began: the fake Steam written afresh, its changes forgotten; the data folder gone.
    try { Initialize-FakeSteam $script:steamRoot | Out-Null } catch { Say "note: could not write the fake Steam afresh: $_" }
    if (Test-Path $data) { Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
