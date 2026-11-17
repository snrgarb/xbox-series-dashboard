# The Tile menu, walked with the virtual pad on a fake Steam library (docs/spec/tile-menu.md): Menu on a Recent tile
# and on a Group's card, its rows by a game's state, More options and its flyout, "Uninstall X?" and its answers, B a
# level at a time, and what each row then does: Add to and Remove from Favourites and Hide this game (the library
# database), Install and Uninstall (what the stand-in Steam client is asked), Customise Home (Settings on
# Games & apps on Groups, B back to the tile), Manage game (the game's Game hub in the menu's place, whose own Tile menu has
# no Manage game row, and B back to the tile) and Play game (a launch).
#
# Judged by what --log-focus says after each press, a press at a time: `focus: TileMenu: Menu(Play game) of <game>`,
# `focus: TileMenu: MoreOptions(Hide this game) of <game>`, `focus: TileMenu: Prompt(Uninstall) of <game>`, and
# `focus: Recent(2)` once the menu has closed; and by the Dashboard's and the stand-in Steam's own lines for what was
# asked of them. Which tile has which game is the library's to say: a game is found by what its menu says, never by a
# place in the row. Passes when every press is followed by the line expected of it. The screenshots are to look at.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then a rest for the picture, if one is asked for: the software renderer takes most of a second a frame.
# Returns the line, or nothing.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($Name) {
        Start-Sleep -Milliseconds 3500
        Save-Screen $Name
    }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# One press that must move focus nowhere: the end of the rows, or a button the menu has no use for.
function Press-Nowhere([string] $Button, [string] $Why) {
    $was = @(Focus-Lines).Count
    Push-Button $Button
    Start-Sleep -Milliseconds 1200
    if (@(Focus-Lines).Count -eq $was) {
        Say "$Button -> nothing ($Why)"
    } else {
        Say "$Button -> '$(Focus-Now)', where nothing was expected ($Why)"
        $script:failures += "$Button should have done nothing ($Why), and the log's last is '$(Focus-Now)'"
    }
}

# Presses a direction until focus is where $Expect says, $Most times at most: a press held 120 ms can count twice
# where the VM's clock jumps, so a walk is judged by where it ends, not by how many presses it took.
function Walk([string] $Button, [string] $Expect, [int] $Most = 8) {
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

# A line the Dashboard must log after the mark $After, by this pattern.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What) {
    $line = Wait-DashboardLog $Pattern 10 $After
    if ($line) { Say "${What}: $line" } else { Say "${What}: no line like '$Pattern'"; $script:failures += "${What}: the Dashboard logged no '$Pattern'" }
    $line
}

# A line the stand-in Steam must have written: what it was started with.
function Expect-Steam([string] $Pattern, [string] $What) {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    $line = Wait-Until { if (Test-Path $log) { Get-Content $log | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } } 10
    if ($line) { Say "${What}: $line" } else { Say "${What}: the stand-in Steam has no line like '$Pattern'"; $script:failures += "${What}: the stand-in Steam was not started with '$Pattern'" }
}

# The game and the row the menu's line names: `focus: TileMenu: Menu(Play game) of Ashfall Courier`.
function Menu-Game([string] $Line) { if ($Line -match ' of (.+)$') { $Matches[1] } }
function Menu-Row([string] $Line) { if ($Line -match '\((.+?)\) of ') { $Matches[1] } }

# Opens the menu of the Recent row's games in turn, from the tile focus is on rightwards, until one's first row is
# $First ("Play game" or "Install"); leaves that menu open and returns its line. A menu that is not it is closed
# with B, which must give focus back to the tile.
function Open-MenuWhoseFirstRowIs([string] $First) {
    foreach ($tile in 1..8) {
        $line = Press Start 'TileMenu: Menu\((Play game|Install)\) of .+'
        if (-not $line) { return $null }
        if ((Menu-Row $line) -eq $First) { return $line }
        Press B 'Recent\(\d+\)' | Out-Null
        # The Play history tile ends the row: Menu opens nothing on it, which the next turn would report.
        Press Right 'Recent\(\d+\)' | Out-Null
    }
    $script:failures += "no game on the Recent row has a menu that starts with $First"
    $null
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    $script:dashboard = Start-Dashboard @('--log-focus')
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    # The greeting's Toast comes and goes: it is not in the pictures, and the Guide button is the Guide's again.
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 4
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # Menu on the profile, where Home starts, opens nothing. Down is the Recent row.
    Press-Nowhere Start 'the profile has no menu'
    Press Down 'Recent\(0\)' | Out-Null

    # --- An installed game's menu: six rows, Up and Down stopping at the ends, the buttons it is deaf to.
    $menu = Open-MenuWhoseFirstRowIs 'Play game'
    $installed = Menu-Game $menu
    $game = [regex]::Escape($installed)
    Start-Sleep -Milliseconds 3500
    Save-Screen '01-menu-installed'
    Press-Nowhere Up 'the first row'
    $favourite = Press Down "TileMenu: Menu\((Add to|Remove from) Favourites\) of $game"
    Press Down "TileMenu: Menu\(Manage game\) of $game" | Out-Null
    Press Down "TileMenu: Menu\(Uninstall\) of $game" | Out-Null
    Press Down "TileMenu: Menu\(Customise Home\) of $game" | Out-Null
    Press Down "TileMenu: Menu\(More options\) of $game" '02-menu-more-options-focused' | Out-Null
    Press-Nowhere Down 'the last row'
    Press-Nowhere Right 'Right does not open the flyout'
    Press-Nowhere Start 'Menu opens no second menu'
    Press-Nowhere Y 'Y opens no Search under the menu'

    # --- More options: the flyout, and B back to the menu with focus on More options.
    Press A "TileMenu: MoreOptions\(Hide this game\) of $game" '03-more-options' | Out-Null
    Press-Nowhere Left 'Left does not close the flyout'
    Press B "TileMenu: Menu\(More options\) of $game" '04-flyout-closed' | Out-Null

    # --- Uninstall: the question in the menu's place; Cancel and B come back to the menu on Uninstall. Saying yes
    # comes last of what is done with this game, as it really uninstalls the fake's.
    Walk Up "TileMenu: Menu\(Uninstall\) of $game"
    Press A "TileMenu: Prompt\(Uninstall\) of $game" '05-uninstall-question' | Out-Null
    Press Down "TileMenu: Prompt\(Cancel\) of $game" | Out-Null
    Press A "TileMenu: Menu\(Uninstall\) of $game" '06-question-cancelled' | Out-Null
    Press A "TileMenu: Prompt\(Uninstall\) of $game" | Out-Null
    Press B "TileMenu: Menu\(Uninstall\) of $game" | Out-Null
    $tile = Press B 'Recent\(\d+\)'

    # --- Favourites: the row marks the game in the library database, and the next menu reads the other way. Then
    # back again, so the library is as it was.
    $was = if ((Menu-Row $favourite) -eq 'Add to Favourites') { 'Add to', 'Remove from', 'is a favourite' } else { 'Remove from', 'Add to', 'is no longer a favourite' }
    Press Start "TileMenu: Menu\(Play game\) of $game" | Out-Null
    Press Down "TileMenu: Menu\($($was[0]) Favourites\) of $game" | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^library: $game $($was[2])" $mark 'Favourites' | Out-Null
    Start-Sleep -Milliseconds 1500
    Press Start "TileMenu: Menu\(Play game\) of $game" | Out-Null
    Press Down "TileMenu: Menu\($($was[1]) Favourites\) of $game" '08-favourites-row-the-other-way' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^library: $game is (a|no longer a) favourite" $mark 'Favourites, back' | Out-Null

    # --- Customise Home: Settings on Games & apps' Groups, and B back on the tile. Manage game: the game's Game hub in
    # the menu's place; Menu there is the same game's Tile menu without its Manage game row; B a level at a time
    # back to the tile.
    Press Start "TileMenu: Menu\(Play game\) of $game" | Out-Null
    Walk Down "TileMenu: Menu\(Customise Home\) of $game"
    Press A 'Settings: Games & apps: Groups' '09-settings-on-groups' | Out-Null
    # Settings' own way back: its rail, then the tile.
    Press B 'Settings: Rail: Games & apps' | Out-Null
    $back = ($tile -replace '^focus: ', '')
    if ($back) { Press B ([regex]::Escape($back)) | Out-Null }
    Press Start "TileMenu: Menu\(Play game\) of $game" | Out-Null
    Walk Down "TileMenu: Menu\(Manage game\) of $game"
    Press A 'GameHub: \d+ Hub (Play|Install)' '09b-manage-game-opens-the-hub' | Out-Null
    Press Start "TileMenu: Menu\((Play game|Install)\) of $game" | Out-Null
    Press Down "TileMenu: Menu\((Add to|Remove from) Favourites\) of $game" | Out-Null
    $next = Press Down "TileMenu: Menu\(.+\) of $game"
    if ((Menu-Row $next) -eq 'Manage game') { $script:failures += "the hub's Tile menu has a Manage game row" }
    Press B 'GameHub: \d+ Hub (Play|Install)' | Out-Null
    if ($back) { Press B ([regex]::Escape($back)) | Out-Null }

    # --- Uninstall, yes: the menu closes and Steam's client is asked, as the Bundle asks it (steam.exe -silent
    # -console +app_uninstall), the stand-in client in the fake's folder. It uninstalls the fake's game, and the
    # Dashboard sees it go.
    Press Start "TileMenu: Menu\(Play game\) of $game" | Out-Null
    Walk Down "TileMenu: Menu\(Uninstall\) of $game"
    Press A "TileMenu: Prompt\(Uninstall\) of $game" | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Recent\(\d+\)' '07-uninstall-asked' | Out-Null
    Expect-Logged "^store: asking Steam to uninstall ${game}: app_uninstall \d+$" $mark 'Uninstall' | Out-Null
    Expect-Steam 'steam: asked to uninstall \d+: -silent -console \+app_uninstall \d+' 'Uninstall'
    $gone = Wait-DashboardLog "^queue: $game uninstalled$" 30 $mark
    if ($gone) { Say "Uninstall: $gone" } else { $failures += "the Dashboard never saw $installed uninstalled" }
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # --- A game that is not installed: Install first and no Uninstall, five rows. Install closes the menu and
    # starts Steam with the game's install address.
    Walk Left 'Recent\(0\)' 10
    $menu = Open-MenuWhoseFirstRowIs 'Install'
    $absent = [regex]::Escape((Menu-Game $menu))
    Start-Sleep -Milliseconds 3500
    Save-Screen '10-menu-not-installed'
    Press Down "TileMenu: Menu\((Add to|Remove from) Favourites\) of $absent" | Out-Null
    Press Down "TileMenu: Menu\(Manage game\) of $absent" | Out-Null
    Press Down "TileMenu: Menu\(Customise Home\) of $absent" | Out-Null
    Walk Up "TileMenu: Menu\(Install\) of $absent"
    $mark = Get-DashboardLogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^store: asking Steam to install ${absent}: app_install \d+$" $mark 'Install' | Out-Null
    Expect-Steam 'steam: asked to install \d+: -silent \+app_install \d+' 'Install'
    Expect-Logged "^queue: $absent joined, asked for$" $mark 'Install, the Queue' | Out-Null
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # --- A Group's game card: the hint bar says "More options" there, and is gone while the menu is up. B closes
    # the menu and focus is on the card still, the page risen to the Group.
    Press Down 'Wide\(\d\)' | Out-Null
    Press Down 'Group \{ group: 0, card: 0 \}' '11-group-card-hint-bar' | Out-Null
    Press Start 'TileMenu: Menu\((Play game|Install)\) of .+' '12-menu-on-a-group-card' | Out-Null
    Press B 'Group \{ group: 0, card: 0 \}' '13-back-on-the-card' | Out-Null
    Press B 'Recent\(\d+\)' | Out-Null

    # --- Hide this game, on the row's first tile: the game leaves the Recent row, and the tile's menu is the next
    # game's.
    Walk Left 'Recent\(0\)' 10
    $menu = Press Start 'TileMenu: Menu\((Play game|Install)\) of .+'
    $hiddenGame = Menu-Game $menu
    $hidden = [regex]::Escape($hiddenGame)
    Walk Down "TileMenu: Menu\(More options\) of $hidden"
    Press A "TileMenu: MoreOptions\(Hide this game\) of $hidden" | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Recent\(0\)' | Out-Null
    Expect-Logged "^library: $hidden is hidden" $mark 'Hide this game' | Out-Null
    Start-Sleep -Milliseconds 3500
    Save-Screen '14-hidden-game-gone-from-the-row'
    $menu = Press Start 'TileMenu: Menu\((Play game|Install)\) of .+'
    if ($menu -and (Menu-Game $menu) -eq $hiddenGame) { $failures += "$hiddenGame is hidden and still on the Recent row's first tile" }
    Press B 'Recent\(0\)' | Out-Null

    # --- Play game, last: the menu closes and the game is launched as A on its tile launches it.
    $menu = Open-MenuWhoseFirstRowIs 'Play game'
    $played = [regex]::Escape((Menu-Game $menu))
    $mark = Get-DashboardLogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^launch: $played \(\d+\) asked of Steam" $mark 'Play game' | Out-Null
    $up = Wait-DashboardLog "the game's window is up" 60 $mark
    Say "the game: $(if ($up) { $up } else { 'no window in 60 s' }); game processes: $(@(Get-StandInGame).Count)"
    if (-not $up) { $failures += 'the game Play game launched never showed a window' }
    Start-Sleep -Seconds 3
    Save-Screen '15-the-game-play-game-launched'
    Stop-StandInGame
    $ended = Wait-DashboardLog 'the game has ended, Home back' 20 $mark
    if (-not $ended) { $failures += 'Home did not come back after the game ended' }

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
