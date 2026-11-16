# Full library, walked with the virtual pad and the VM's own keyboard on the fake Steam's full shelf: See all on
# Home's first two Groups, each landing on its Group's tab and B back on See all; the tabs stepped with Left and
# Right to their ends and with LB and RB; the Sort list opened, walked to its end and chosen from; and, on Full
# library as the nav opens it (no tab lit, every game), the name box opened with Y, a name typed in text entry
# narrowing the grid, kept, cleared with B, and a name that finds nothing.
#
# Judged by what --log-focus says after each press: `focus: MyGames: FullLibrary: Tile(0); tab 2/5 "Couch co-op",
# sort Name, name "", 4 games` is the item that is up, where focus is on it, then the lit tab's place among the tabs
# (0 with none lit) and its name, the sort the grid is in, the name it is narrowed by and how many games it shows.
# No place is counted: walks go to an edge or to a named place, and how many tabs and games there are is read from
# the log and the library. Passes when every press is followed by the line expected of it, a press at an edge by
# none, and the Dashboard stays up.
#
# The pictures 07 and 09 to 14 are named as the Bundle's of the same states
# (~/projects/xbox-series-theme/artifacts/e2e/port-reference-20261115-192720/), to be looked at beside them.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
Add-Type -AssemblyName System.Windows.Forms
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# The picture of the screen now, once it has come to rest: the grid flies in and slides over a few frames, and the
# software renderer takes a second or so a frame.
function Picture([string] $Name) {
    Start-Sleep -Milliseconds 4500
    Save-Screen $Name
}

# Waits for a new focus line, after the first $Before, matching $Expect: a regular expression for what comes after
# "focus: ". Returns the line, or records a failure. Then the picture, if one is asked for.
function Expect([string] $Did, [int] $Before, [string] $Expect, [string] $Name = '') {
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($Name) { Picture $Name }
    if ($got) {
        Say "$Did -> $got"
    } else {
        Say "$Did -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Did should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# One press that must be followed by a new focus line matching $Expect. A press the slow VM lost is pressed again,
# once, if the log has not moved at all.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $moved = Wait-Until { if (@(Focus-Lines).Count -gt $before) { $true } } 6
    if (-not $moved) {
        Say "${Button}: no line yet, pressed again"
        Push-Button $Button
    }
    Expect $Button $before $Expect $Name
}

# One press that must change nothing the log says: an edge, or a button that is swallowed.
function Press-Stays([string] $Button, [string] $Why) {
    $was = Focus-Now
    Push-Button $Button
    Start-Sleep -Milliseconds 1500
    if ((Focus-Now) -eq $was) {
        Say "$Button -> stays ($Why)"
    } else {
        Say "$Button -> MOVED ($Why): $(Focus-Now)"
        $script:failures += "$Button should have left '$was' ($Why), and it is '$(Focus-Now)'"
    }
}

# Presses a button until the log's last line matches $Expect, $Most times at most: a walk is judged by where it ends,
# not by how many presses it took, since a press can be lost or count twice where the VM's clock jumps.
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

# Presses a button until the log says nothing new: an edge, which does not wrap. Returns what follows "focus: "
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

# Types on the VM's keyboard into text entry, which must then say the text with the caret at its end.
function Type-Keys([string] $Keys, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Say "typed $Keys"
    Expect "typing $Keys" $before "TextEntry: .+; `"$([regex]::Escape($Keys))\|`" .+" $Name | Out-Null
}

# Full library's line: focus at $Focus, then its state, each part a regular expression.
function Line([string] $Focus, [string] $Tab = '\d+/\d+ ".*"', [string] $Sort = '\w+( reversed)?', [string] $Name = '', [string] $Games = '\d+') {
    "MyGames: FullLibrary: $Focus; tab $Tab, sort $Sort, name `"$Name`", $Games games"
}

# The parts of Full library's line: the lit tab's place, how many tabs, its name, the sort and the count.
function Parts([string] $Line) {
    if ($Line -match 'tab (\d+)/(\d+) "(.*)", sort (\w+)( reversed)?, name "(.*)", (\d+) games$') {
        [pscustomobject] @{ Tab = [int] $Matches[1]; Tabs = [int] $Matches[2]; Name = $Matches[3]; Sort = $Matches[4]; Games = [int] $Matches[7] }
    }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam -FullShelf
    $all = @($library.games).Count
    $script:dashboard = Start-Dashboard @('--log-focus')
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # Y and B first, so that the log says where Home's focus is; then down to the first Group and along it to See
    # all, its last card.
    Press Y 'Search: Key\(Q\); "\|" upper letters, RecentlyAdded \d+' | Out-Null
    # B puts Search's keyboard away, and B again leaves it.
    Press B 'Search: RecentlyAdded\(0\) .+; "" away, RecentlyAdded \d+' | Out-Null
    Press B 'Recent\(0\)' | Out-Null
    Walk Down 'Group \{ group: 0, card: 0 \}' 4
    $seeAll = Walk-ToEdge Right 'Group \{ group: 0, card: [1-9]\d* \}'

    # See all: Full library on the first tab, focus on its first game.
    $line = Press A (Line 'Tile\(0\)' '1/\d+ ".+"')
    $first = Parts $line
    if (-not $first) { throw "See all did not open Full library on the first Group's tab: '$(Focus-Now)'" }
    Say "the tabs: $($first.Tabs), the first `"$($first.Name)`" with $($first.Games) games"

    # Up to the row, on Sort, and up again to the tabs, which shows nothing.
    Press Up (Line 'Sort' '1/\d+ ".+"') | Out-Null
    Press Up (Line 'Tabs' '1/\d+ ".+"') '09-my-games-full-library-tab-focused' | Out-Null
    Press-Stays Up 'nothing is over the tabs'
    if ($first.Tabs -ge 2) {
        Press Right (Line 'Tabs' "2/$($first.Tabs) `".+`"") '10-my-games-full-library-another-tab' | Out-Null
        # Right to the last tab, where it stops; Left back to the first, and Left there is the nav.
        Walk-ToEdge Right (Line 'Tabs' "$($first.Tabs)/$($first.Tabs) `".+`"") | Out-Null
        Walk Left (Line 'Tabs' '1/\d+ ".+"') ($first.Tabs + 2)
    } else {
        $failures += 'the fake Steam gives one tab only: nothing to step between'
    }
    Press Left (Line 'Nav\(FullLibrary\)' '1/\d+ ".+"') | Out-Null
    Press Right (Line 'Tile\(0\)' '1/\d+ ".+"') | Out-Null

    # RB and LB from the grid step the lit tab, and stop at the first.
    Press RightShoulder (Line 'Tile\(0\)' '2/\d+ ".+"') | Out-Null
    Press LeftShoulder (Line 'Tile\(0\)' '1/\d+ ".+"') | Out-Null
    Press-Stays LeftShoulder 'the first tab is the first'

    # The Sort list: A on Sort opens it on the option the grid is in; down to its end, where it stops; the bumpers
    # do nothing under it; A chooses, and focus is on Sort with the grid in that order.
    Press Up (Line 'Sort' '1/\d+ ".+"') | Out-Null
    Press A (Line 'SortList\(\w+\)' '1/\d+ ".+"') | Out-Null
    Walk-ToEdge Up (Line 'SortList\(Name\)') | Out-Null
    Picture '11-my-games-full-library-sort-list'
    Walk-ToEdge Down (Line 'SortList\(Playtime\)') | Out-Null
    Press-Stays RightShoulder 'the Sort list takes every button'
    Press A (Line 'Sort' '1/\d+ ".+"' 'Playtime') 'sort-chosen' | Out-Null
    # It orders the next tab too.
    Press RightShoulder (Line 'Sort' '2/\d+ ".+"' 'Playtime') | Out-Null
    # B with no name set is Home, back on See all, Home still at the Group's rest.
    Press B ([regex]::Escape($seeAll)) 'back-on-see-all' | Out-Null

    # The second Group's See all is the second tab.
    if ($first.Tabs -ge 2) {
        Press Down 'Group \{ group: 1, card: \d+ \}' | Out-Null
        $seeAll = Walk-ToEdge Right 'Group \{ group: 1, card: [1-9]\d* \}'
        Press A (Line 'Tile\(0\)' "2/$($first.Tabs) `".+`"") 'see-all-second-group' | Out-Null
        Press B ([regex]::Escape($seeAll)) | Out-Null
    }

    # Full library as the nav opens it: back on the Recent row, up to the top bar's My games & apps, then down the
    # nav to Full library. No tab is lit, and the grid is every game.
    Press B 'Recent\(\d+\)' | Out-Null
    Press Up 'TopBar\(Profile\)' | Out-Null
    Walk Right 'TopBar\(MyGames\)' 2
    Press A 'MyGames: Games: Tile\(0\)' | Out-Null
    Press Left 'MyGames: Games: Nav\(Games\)' | Out-Null
    Walk Down 'MyGames: .+: Nav\(FullLibrary\)(; .*)?' 5
    Start-Sleep -Milliseconds 1500
    $line = Focus-Now
    if ($line -notmatch [regex]::Escape((Line 'Nav(FullLibrary)' "0/$($first.Tabs) `"`"" 'Name' '' "$all"))) {
        $failures += "Full library from the nav should show every game, $all, with no tab lit, and says '$line'"
    }
    Picture '07-my-games-full-library-from-nav'
    Press Right (Line 'Tile\(0\)' '0/\d+ ""') | Out-Null

    # The name: Y on a game opens the box, empty; A on it is text entry over the page. A few letters of a game's
    # name are typed on the VM's keyboard, and kept with Menu: the grid narrows to the games whose name has them,
    # focus on the first.
    $needle = ((@($library.games)[0].name -replace '[^A-Za-z]', '').ToLower()).Substring(0, 3)
    $found = @($library.games | Where-Object { $_.name.ToLower().Contains($needle) }).Count
    Say "the name `"$needle`" is in $found of the $all games' names"
    Press Y (Line 'Name' '0/\d+ ""' 'Name' '' "$all") '12-my-games-full-library-name-box' | Out-Null
    Press A 'TextEntry: .+' | Out-Null
    Type-Keys $needle '13-my-games-full-library-name-entry'
    Press Start (Line 'Tile\(0\)' '0/\d+ ""' 'Name' $needle "$found") '14-my-games-full-library-name-kept' | Out-Null
    # B clears the name; focus stays on the game.
    Press B (Line 'Tile\(\d+\)' '0/\d+ ""' 'Name' '' "$all") | Out-Null

    # A name that finds nothing: focus is on the box still, the grid empty; B there clears it, focus on Filter.
    Press Y (Line 'Name' '0/\d+ ""') | Out-Null
    Press A 'TextEntry: .+' | Out-Null
    Type-Keys 'zqxj'
    Press Start (Line 'Name' '0/\d+ ""' 'Name' 'zqxj' '0') 'nothing-found' | Out-Null
    Press-Stays Down 'nothing is in the grid'
    Press B (Line 'Filter' '0/\d+ ""' 'Name' '' "$all") | Out-Null

    # B with no name is Home.
    Press B 'Recent\(\d+\)' 'home-again' | Out-Null

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
