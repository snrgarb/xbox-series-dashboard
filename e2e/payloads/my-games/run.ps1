# My games & apps, walked with the virtual pad on the fake Steam's full shelf (twenty installed games, four rows of
# its grid): Games' grid to each of its edges and down to SEE FULL LIBRARY, the nav from its top to its foot with the
# item under focus showing, Play history's rows, Manage's box, Y to Search and back, and B home.
#
# Judged by what --log-focus says after each press: `focus: MyGames: Games: Tile(3)`, the page, the item that is up,
# and where focus is on it; `focus: MyGames: Apps: Nav(Apps)` with focus on the nav. No place is counted: a walk goes
# to an edge or to a named place, and how many games or rows there are is the library's to say. Passes when every
# press is followed by the line expected of it, a press at an edge by none, and the Dashboard stays up.
#
# The pictures 01 to 08 are named as the Bundle's of the same places
# (~/projects/xbox-series-theme/artifacts/e2e/port-reference-20261115-192720/), to be looked at beside them.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# The picture of where focus is now, once the page has come to rest: a pane flies in and the grid slides over a few
# frames, and the software renderer takes a second or so a frame.
function Picture([string] $Name) {
    Start-Sleep -Milliseconds 4500
    Save-Screen $Name
}

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then the picture, if one is asked for.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($Name) { Picture $Name }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
}

# One press that must move focus nowhere: an edge, or a button with nothing behind it yet.
function Press-Stays([string] $Button, [string] $Why) {
    $was = Focus-Now
    Push-Button $Button
    Start-Sleep -Milliseconds 1500
    if ((Focus-Now) -eq $was) {
        Say "$Button -> stays ($Why)"
    } else {
        Say "$Button -> MOVED ($Why): $(Focus-Now)"
        $script:failures += "$Button should have left focus on '$was' ($Why), and it is '$(Focus-Now)'"
    }
}

# Presses a direction until focus is where $Expect says, $Most times at most: a press held 120 ms can count twice
# where the VM's clock jumps, so a walk is judged by where it ends, not by how many presses it took.
function Walk([string] $Button, [string] $Expect, [int] $Most = 10) {
    $pattern = "^focus: $Expect`$"
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match $pattern) { break }
        Push-Button $Button
        Start-Sleep -Milliseconds 1500
    }
    if ((Focus-Now) -match $pattern) {
        Say "$Button until $(Focus-Now)"
    } else {
        Say "$Button never reached '$Expect': $(Focus-Now)"
        $script:failures += "$Button never reached 'focus: $Expect': '$(Focus-Now)'"
    }
}

# Presses a direction until it moves focus no more: an edge, which does not wrap. Returns what follows "focus: "
# there, which must match $Expect.
function Walk-ToEdge([string] $Button, [string] $Expect, [int] $Most = 12) {
    $reached = $false
    foreach ($press in 1..$Most) {
        $was = Focus-Now
        Push-Button $Button
        Start-Sleep -Milliseconds 1500
        if ((Focus-Now) -eq $was) { $reached = $true; break }
    }
    $at = (Focus-Now) -replace '^focus: ', ''
    if ($reached -and $at -match "^$Expect`$") {
        Say "$Button to the edge: $at"
    } else {
        Say "$Button did not end on '$Expect': $at"
        $script:failures += "$Button should have ended on '$Expect', and focus is '$at'"
    }
    $at
}

# The number in the focus line's `Tile(n)` or `Row(n)`.
function Place([string] $At) { [int] ($At -replace '^.*\((\d+)\)$', '$1') }

# With focus on the nav the item that is up trails focus by the settle wait when focus is passing along it, so a
# line there is judged by the item focus is on, whatever is showing.
function Nav([string] $Item) { "MyGames: .+: Nav\($Item\)" }

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam -FullShelf
    $installed = @($library.games | Where-Object installed).Count
    $played = @($library.games | Where-Object last_played -gt 0).Count
    $script:dashboard = Start-Dashboard @('--log-focus')
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    # The greeting's Toast comes and goes: it is not in the pictures, and the Guide button is the Guide's again.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # In by the top bar's button: Games, focus on its first game. (Y and B first, so that the log says where Home's
    # focus is.)
    Press Y 'Search: Open'
    Press B 'Recent\(0\)'
    Press Up 'TopBar\(Profile\)'
    Walk Right 'TopBar\(MyGames\)' 2
    Press A 'MyGames: Games: Tile\(0\)'

    # Left from the first column is the nav, on the item that is up; Right is the game again.
    Press Left 'MyGames: Games: Nav\(Games\)' '01-my-games-games-first-rows'
    Press Right 'MyGames: Games: Tile\(0\)' '02-my-games-games-tile-focused'

    # The grid's edges stop. A has nothing to open until the Game hub is there; Menu is the game's Tile menu, and B closes it.
    Press-Stays Up 'nothing is over the first row'
    Press-Stays A 'no Game hub yet'
    Press Start 'TileMenu: Menu\(.*'
    Press B 'MyGames: Games: Tile\(0\)'
    $rowEnd = Walk-ToEdge Right 'MyGames: Games: Tile\([1-9]\d*\)'
    $columns = (Place $rowEnd) + 1
    Say "the grid has $columns columns, and $installed games are installed"
    Walk Left 'MyGames: Games: Tile\(0\)' 8

    # Down the first column to the last row, the grid scrolling a row at a time, and on to SEE FULL LIBRARY under
    # it; Up from there is the last row again, the grid still at its end.
    Walk Down 'MyGames: Games: SeeFullLibrary' 12
    Picture '04-my-games-games-see-full-library'
    Press-Stays Down 'nothing is under SEE FULL LIBRARY'
    Press-Stays Right 'nothing is beside SEE FULL LIBRARY'
    Press Up 'MyGames: Games: Tile\(\d+\)' '03-my-games-games-scrolled-down'
    $lastRow = Place (Focus-Now)
    $expected = [int] ([math]::Floor(($installed - 1) / $columns) * $columns)
    if ($lastRow -ne $expected) { $failures += "Up from SEE FULL LIBRARY should be tile $expected, the last row's first, and is tile $lastRow" }
    if ($lastRow -lt 3 * $columns) { $failures += "the last row starts at tile ${lastRow}: too few rows for the grid to have scrolled" }
    # Along the last row to its end, which is the last game.
    $last = Walk-ToEdge Right 'MyGames: Games: Tile\(\d+\)'
    if ((Place $last) -ne $installed - 1) { $failures += "Right along the last row should end on tile $($installed - 1), and ends on '$last'" }
    Picture 'games-last-tile'

    # SEE FULL LIBRARY: Full library, focus on its nav item (its content is #53's).
    Press Down 'MyGames: Games: SeeFullLibrary'
    Press A 'MyGames: FullLibrary \{ filter: None \}: Nav\(FullLibrary\)' 'full-library-from-see-full-library'

    # The nav: up to its top, where it stops, and down it an item at a time, each showing as focus steps onto it.
    Walk-ToEdge Up (Nav 'Games') | Out-Null
    Picture 'nav-top-games'
    Press Down (Nav 'Apps') '05-my-games-apps'
    Press-Stays Right 'Apps has nothing to focus'
    Press Down (Nav 'PlayHistory')

    # Play history: Right is its first row; down to its last, where it stops; Left is the nav.
    if ($played -gt 0) {
        Press Right 'MyGames: PlayHistory: Row\(0\)' '06-my-games-play-history'
        Press-Stays Up 'nothing is over the first row'
        Press-Stays Right 'nothing is beside a row'
        $lastRow = Walk-ToEdge Down 'MyGames: PlayHistory: Row\(\d+\)'
        if ((Place $lastRow) -ne $played - 1) { $failures += "Play history should end on row $($played - 1), $played games having been played, and ends on '$lastRow'" }
        Picture 'play-history-last-row'
        Press Left 'MyGames: PlayHistory: Nav\(PlayHistory\)'
    } else {
        $failures += 'the fake Steam has no played game for Play history to list'
    }
    Press Down (Nav 'FullLibrary') '07-my-games-full-library-from-nav'
    Press Down (Nav 'Manage')
    Press-Stays Down 'Manage is the last item'

    # Manage: its one box, which opens nothing until there is a Queue.
    Press Right 'MyGames: Manage: Queue' '08-my-games-manage'
    Press-Stays A 'no Manage Queue yet'
    Press-Stays Right 'nothing is beside the box'
    Press Left 'MyGames: Manage: Nav\(Manage\)'

    # Back to Games: Y on a game is Search over the page, and B there comes back to the game.
    Walk-ToEdge Up (Nav 'Games') | Out-Null
    Press A 'MyGames: Games: Tile\(0\)'
    Press Right 'MyGames: Games: Tile\(1\)'
    Press Y 'Search: Open' 'search-over-my-games'
    Press B 'MyGames: Games: Tile\(1\)' 'back-on-the-game'

    # B is Home, wherever focus is on the page.
    Press B 'Recent\(0\)' 'home-again'

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
