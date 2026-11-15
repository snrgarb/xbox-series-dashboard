# The page stack, walked with the virtual pad on a fake Steam library: every way from Home into My games & apps,
# Search and Settings, and B back from each, with focus where it should be; and the hint bar on a Group's game card.
# Search and Settings are stand-ins until their own tickets, which say only which page they are; My games & apps
# says which of its items is up and where focus is on it, and is walked in my-games/run.ps1.
#
# Judged by what --log-focus says after each press, a press at a time: `focus: Recent(2)` on Home, and
# `focus: Settings: Rail: Theme settings` with a page on top (the page's name, then where it says its focus is). Passes when every
# press is followed by the line expected of it and the Dashboard stays up. The screenshots are to look at.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then a rest for the picture, if one is asked for: the software renderer takes most of a second a frame.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($Name) {
        # Home slides to its rest over a few frames, and the software renderer takes a second or so a frame.
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

# Presses a direction until focus is where $Expect says, $Most times at most: a press held 120 ms can count twice
# where the VM's clock jumps, so a walk along a row is judged by where it ends, not by how many presses it took.
function Walk([string] $Button, [string] $Expect, [int] $Most = 10) {
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

# Presses a direction until it moves focus no more: the end of a row, which does not wrap. How long a row is is the
# library's to say, so its last place is found, never counted. Returns what follows "focus: " there, which must
# match $Expect.
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
        Say "$Button to the row's end: $at"
    } else {
        Say "$Button did not end on '$Expect': $at"
        $script:failures += "$Button should have ended on '$Expect', and focus is '$at'"
    }
    $at
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    $script:dashboard = Start-Dashboard @('--log-focus')
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    # The greeting's Toast comes and goes: it is not in the pictures, and the Guide button is the Guide's again.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # Y from the profile, where Home starts: Search, and B back on the Recent row, not on the top bar.
    Press Y 'Search: Open' '01-search-from-y'
    Press B 'Recent\(0\)' '02-back-on-the-recent-row'

    # The top bar's three buttons. My games & apps gives focus back to the Recent row; Search and Settings to
    # their button.
    Press Up 'TopBar\(Profile\)'
    Walk Right 'TopBar\(MyGames\)' 2
    Press A 'MyGames: Games: Tile\(0\)' '03-my-games'
    Press B 'Recent\(0\)'
    Press Up 'TopBar\(Profile\)'
    Walk Right 'TopBar\(Search\)' 3
    Press A 'Search: Open'
    Press B 'TopBar\(Search\)' '04-back-on-the-search-button'
    Walk Right 'TopBar\(Settings\)' 2
    Press A 'Settings: Rail: Theme settings' '05-settings'
    Press B 'TopBar\(Settings\)'

    # The wide tiles: Discord (Settings, there being no Discord tab), Settings, Downloads. Each gives focus back to
    # its tile; Settings opens on Theme settings' first box, so B is its rail first. Achievements opens the Guide and
    # is not pressed here.
    Press Down 'Recent\(\d+\)'
    Press Down 'Wide\(\d\)'
    Walk Left 'Wide\(0\)' 4
    Press A 'Settings: Theme settings: Personalisation'
    Press B 'Settings: Rail: Theme settings'
    Press B 'Wide\(0\)'
    Walk Right 'Wide\(1\)' 2
    Press A 'Settings: Theme settings: Personalisation'
    Press B 'Settings: Rail: Theme settings'
    Press B 'Wide\(1\)'
    Walk Right 'Wide\(3\)' 3
    Press A 'MyGames: Manage: Queue'
    Press B 'Wide\(3\)' '06-back-on-downloads'

    # The first Group: the hint bar is up on a game's card, bottom right. Y there is Search, and B comes back to
    # the card with the page still risen to the Group.
    Press Down 'Group \{ group: 0, card: 0 \}' '07-hint-bar-on-a-group-card'
    Press Y 'Search: Open'
    Press B 'Group \{ group: 0, card: 0 \}' '08-back-on-the-group-card'
    # See all, the Group's last card, wherever that is: no hint bar; Full library on the Group's filter; B back on
    # See all.
    $seeAll = Walk-ToEdge Right 'Group \{ group: 0, card: [1-9]\d* \}'
    Start-Sleep -Milliseconds 3000
    Save-Screen '09-see-all-no-hint-bar'
    Press A 'MyGames: FullLibrary \{ filter: Some\(\d+\) \}: Nav\(FullLibrary\)' '10-full-library'
    Press B ([regex]::Escape($seeAll)) '11-back-on-see-all'

    # B on the Group is the Recent row; along it to the Play history tile, its last. Play history, and B back on
    # the game's tile before it.
    Press B 'Recent\(\d+\)'
    $history = Walk-ToEdge Right 'Recent\([1-9]\d*\)'
    $lastGame = "Recent\($([int] ($history -replace '\D', '') - 1)\)"
    Press A 'MyGames: PlayHistory: Row\(0\)' '12-play-history'
    Press B $lastGame '13-back-on-the-last-game'

    # The Guide's My games & apps, its second entry: the Guide closes onto the page, and B is Home's rows.
    Push-Button Guide
    Start-Sleep -Milliseconds 2500
    Save-Screen '14-guide'
    Push-Button Down
    Start-Sleep -Milliseconds 800
    Press A 'MyGames: Games: Tile\(0\)' '15-my-games-from-the-guide'
    Press B $lastGame

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
