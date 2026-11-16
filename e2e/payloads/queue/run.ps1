# Install, uninstall and the Queue (#78), walked with the virtual pad on a fake Steam library. Nothing is ever
# downloaded or uninstalled but the fake's own files: the Dashboard asks Steam's client as the Bundle does
# (`steam.exe -silent +app_install <appid>`), here the stand-in client in the fake's folder, which writes the game's
# app manifest as Steam does as it takes an install on; then this test moves the install along by rewriting that
# manifest (`standin steam-install <root> <appid> --bytes n/m | --paused n/m | --done | --drop`), as Steam's download
# would, and reads what the Dashboard makes of it.
#
# What it walks:
# - Install from a game's hub: the hub closes, Steam's client is asked, the game joins the Queue (`queue: <name>
#   joined, asked for`) and its tile reads Queued; downloading, it reads Installing... (`queue: <name> downloading,
#   25% of 100 bytes`). Its hub reads Installing, A does nothing there, and Queue stands between Manage and the toggle.
# - The ways into Manage Queue that #79 will finish, as far as they go now (until then each opens My games & apps on
#   Manage, focus on its Queue box): the hub's Queue action, B back to the hub on Queue; Home's Downloads tile, which
#   counts the Queue, B back to it; and Manage's Queue box itself, where A does nothing yet.
# - Install from the Tile menu of a game already queued asks nothing more.
# - Paused (`queue: <name> paused, 50% of 100 bytes`); meanwhile an install started in Steam's own client, of a game
#   the Dashboard was not asked for, joins as Steam reports it (`queue: <name> joined, from its store's client`),
#   Downloads counts two, and it leaves as Steam drops it (`queue: <name> dropped`).
# - Done: the game leaves the Queue installed (`queue: <name> installed`), and its hub reads Play.
# - Install from the Tile menu of another game, which Steam then drops.
# - Uninstall from the Tile menu: the question, cancelled first; then yes, and Steam's client is asked (`steam.exe
#   -silent -console +app_uninstall <appid>`); the stand-in removes the fake's manifest, and the Dashboard sees the
#   game go (`queue: <name> uninstalled`).
#
# Judged by --log-focus a press at a time (`focus: GameHub: 9100040 Hub Queued`, `... Hub Installing`, `... Hub
# Queue`), the Dashboard's `store:` and `queue:` lines, and the stand-in client's `steam: asked to install <appid>`.
# Runs on the VM and in safe mode on the TV PC: the fake and the stand-in client are in the test's own folder either
# way, and no handler is involved. The screenshots are to look at: the Queued and Installing... tiles, the hub's Queue
# action, Downloads' count.
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
    $got
}

# One press that must leave focus where it is: a button with nothing to do there.
function Press-Nothing([string] $Button, [string] $Why) {
    Start-Sleep -Milliseconds 130
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

# Presses a direction until focus is where $Expect says, $Most times at most: a walk is judged by where it ends.
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

# A line the stand-in Steam client must have written, after the first $After of standin.log.
function Expect-Steam([string] $Pattern, [int] $After, [string] $What) {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    $line = Wait-Until { if (Test-Path $log) { Get-Content $log | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } } 10
    if ($line) { Say "${What}: $line" } else { Say "${What}: the stand-in Steam has no line like '$Pattern'"; $script:failures += "${What}: the stand-in Steam client was not asked '$Pattern'" }
}

function Steam-LogLength {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    if (Test-Path $log) { @(Get-Content $log).Count } else { 0 }
}

# Moves the fake Steam's install of $Appid along, as Steam's download would: `standin steam-install`.
function Advance([string] $Appid, [string[]] $To) {
    $standin = Join-Path $PSScriptRoot 'standin.exe'
    $arguments = @('steam-install', "`"$script:root`"", $Appid) + $To
    $done = Start-Process $standin -ArgumentList $arguments -Wait -PassThru
    Say "steam-install $Appid $($To -join ' '): exit $($done.ExitCode)"
    if ($done.ExitCode -ne 0) { $script:failures += "standin steam-install $Appid $($To -join ' ') exited with $($done.ExitCode)" }
}

# The game and the row the Tile menu's line names: `focus: TileMenu: Menu(Install) of Quiet Orbit`.
function Menu-Game([string] $Line) { if ($Line -match ' of (.+)$') { $Matches[1] } }
function Menu-Row([string] $Line) { if ($Line -match '\((.+?)\) of ') { $Matches[1] } }

# From the Recent row's first tile rightwards, opens each game's Tile menu until one is the menu of $Name; leaves it
# open and returns its line. Which tile has which game is the library's to say.
function Open-MenuOf([string] $Name) {
    Walk Left 'Recent\(0\)'
    foreach ($tile in 1..8) {
        $line = Press Start 'TileMenu: Menu\((Play game|Install)\) of .+'
        if (-not $line) { return $null }
        if ((Menu-Game $line) -eq $Name) { return $line }
        Press B 'Recent\(\d+\)' | Out-Null
        Press Right 'Recent\(\d+\)' | Out-Null
    }
    $script:failures += "no tile of the Recent row is $Name's"
    $null
}

function In-Front { if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" } }

try {
    Close-WindowsPrompts
    Connect-Pad
    $script:root = Get-FakeRoot 'steam'
    $library = Initialize-FakeSteam $script:root
    $games = @($library.games)
    # The fake's games that are not installed: two the Recent row has (played), and one never played that only an
    # install started in Steam's client brings up.
    $played = @($games | Where-Object { -not $_.installed -and $_.last_played -gt 0 })
    $unplayed = @($games | Where-Object { -not $_.installed -and $_.last_played -eq 0 })
    if ($played.Count -lt 2 -or $unplayed.Count -lt 1) { throw "the fake has too few games that are not installed: $($played.Count) played, $($unplayed.Count) not" }
    $first, $second, $client = $played[0], $played[1], $unplayed[0]
    Say "to install: $($first.name) ($($first.source_id)) and $($second.name) ($($second.source_id)); started in the client: $($client.name) ($($client.source_id))"
    $firstName, $secondName, $clientName = [regex]::Escape($first.name), [regex]::Escape($second.name), [regex]::Escape($client.name)
    $hub = "GameHub: $($first.source_id)"

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus')
    $started = Wait-DashboardLog 'first frame presented' 60
    if (-not $started) { throw 'The Dashboard never presented a frame' }
    Say "started: $started"
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 4
    In-Front
    Press Down 'Recent\(0\)' | Out-Null

    # --- Install from the hub: A on Install closes it, Steam's client is asked, and the game joins the Queue.
    Open-MenuOf $first.name | Out-Null
    Press B 'Recent\(\d+\)' | Out-Null
    $tile = (Focus-Now) -replace '^focus: ', ''
    $tilePattern = [regex]::Escape($tile)
    Press A "$hub Hub Install" '01-hub-install' | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A $tilePattern | Out-Null
    Expect-Logged "^store: asked Steam to install ${firstName}: .*steam\.exe -silent \+app_install $($first.source_id)$" $mark 'Install from the hub' | Out-Null
    Expect-Steam "steam: asked to install $($first.source_id): -silent \+app_install $($first.source_id)" $steamMark 'Install from the hub'
    Expect-Logged "^queue: $firstName joined, asked for$" $mark 'Install from the hub, the Queue' | Out-Null
    In-Front
    Start-Sleep -Milliseconds 3000
    Save-Screen '02-tile-queued'

    # --- Downloading: the tile reads Installing...
    $mark = Get-DashboardLogLength
    Advance $first.source_id @('--bytes', '25/100')
    Expect-Logged "^queue: $firstName downloading, 25% of 100 bytes$" $mark 'Downloading' | Out-Null
    Press Right 'Recent\(\d+\)' | Out-Null
    Start-Sleep -Milliseconds 3000
    Save-Screen '03-tile-installing'
    Press Left $tilePattern | Out-Null

    # --- Its hub: Installing, which A leaves be; Queue between Manage and the toggle.
    Press A "$hub Hub Installing" '04-hub-installing' | Out-Null
    Press-Nothing A 'Installing... does nothing'
    Press Right "$hub Hub Manage" | Out-Null
    Press Right "$hub Hub Queue" '05-hub-queue' | Out-Null
    Press Right "$hub Hub Toggle" | Out-Null
    Press Left "$hub Hub Queue" | Out-Null

    # --- The first way into Manage Queue: the hub's Queue action, and B back to it.
    Press A 'MyGames: Manage: Queue' '06-manage-from-the-hub' | Out-Null
    # The third way, Manage's Queue box itself, where A opens nothing until #79.
    Press-Nothing A 'Manage Queue is #79s'
    Press B "$hub Hub Queue" | Out-Null
    Press B $tilePattern | Out-Null

    # --- The second way: Home's Downloads tile, which counts the Queue; B back to it.
    Press Down 'Wide\(\d\)' | Out-Null
    Walk Right 'Wide\(3\)' 4
    Start-Sleep -Milliseconds 3000
    Save-Screen '07-downloads-1-in-queue'
    Press A 'MyGames: Manage: Queue' | Out-Null
    Press B 'Wide\(3\)' | Out-Null
    Press Up 'Recent\(\d+\)' | Out-Null

    # --- Install from the Tile menu of the game already queued: the menu closes and nothing more is asked.
    $menu = Open-MenuOf $first.name
    if ($menu -and (Menu-Row $menu) -ne 'Install') { $failures += "a queued game's menu starts with $(Menu-Row $menu), not Install" }
    $mark = Get-DashboardLogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-NotLogged '^store: asked Steam' $mark 'Install of a queued game'

    # --- Paused, then done: the game leaves the Queue installed, and its hub reads Play.
    $mark = Get-DashboardLogLength
    Advance $first.source_id @('--paused', '50/100')
    Expect-Logged "^queue: $firstName paused, 50% of 100 bytes$" $mark 'Paused' | Out-Null

    # --- An install started in Steam's own client while the Queue is followed: it joins as Steam reports it, and
    # leaves as Steam drops it. (One started while the Queue is empty joins at the Dashboard's next start or library
    # read: the Queue is looked at once a second only while it holds something.)
    $mark = Get-DashboardLogLength
    Advance $client.source_id @('--bytes', '10/100')
    Expect-Logged "^queue: $clientName joined, from its store's client$" $mark 'Started in the client' | Out-Null
    Expect-Logged "^queue: $clientName downloading, 10% of 100 bytes$" $mark 'Started in the client, downloading' | Out-Null
    Press Down 'Wide\(\d\)' | Out-Null
    Walk Right 'Wide\(3\)' 4
    Start-Sleep -Milliseconds 3000
    Save-Screen '09-downloads-2-in-queue'
    Press Up 'Recent\(\d+\)' | Out-Null
    $mark = Get-DashboardLogLength
    Advance $client.source_id @('--drop')
    Expect-Logged "^queue: $clientName dropped$" $mark 'Started in the client, dropped' | Out-Null

    $mark = Get-DashboardLogLength
    Advance $first.source_id @('--done')
    Expect-Logged "^queue: $firstName installed$" $mark 'Done' | Out-Null
    Start-Sleep -Milliseconds 2000
    Open-MenuOf $first.name | Out-Null
    Press B 'Recent\(\d+\)' | Out-Null
    Press A "$hub Hub Play" '08-hub-play-once-installed' | Out-Null
    Press B 'Recent\(\d+\)' | Out-Null

    # --- Install from the Tile menu of another game, which Steam drops: it leaves the Queue at once.
    $menu = Open-MenuOf $second.name
    if ($menu -and (Menu-Row $menu) -ne 'Install') { $failures += "$($second.name)'s menu starts with $(Menu-Row $menu), not Install" }
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^store: asked Steam to install ${secondName}: .*steam\.exe -silent \+app_install $($second.source_id)$" $mark 'Install from the menu' | Out-Null
    Expect-Steam "steam: asked to install $($second.source_id)" $steamMark 'Install from the menu'
    Expect-Logged "^queue: $secondName joined, asked for$" $mark 'Install from the menu, the Queue' | Out-Null
    $mark = Get-DashboardLogLength
    Advance $second.source_id @('--drop')
    Expect-Logged "^queue: $secondName dropped$" $mark 'Dropped' | Out-Null
    In-Front

    # --- Uninstall from the Tile menu, of the game just installed: cancelled first, then yes. Against the fake the
    # stand-in client removes its manifest, and the Dashboard sees the game go.
    $menu = Open-MenuOf $first.name
    if ($menu -and (Menu-Row $menu) -ne 'Play game') { $failures += "$($first.name)'s menu starts with $(Menu-Row $menu) once installed" }
    Walk Down "TileMenu: Menu\(Uninstall\) of $firstName" 6
    Press A "TileMenu: Prompt\(Uninstall\) of $firstName" '10-uninstall-question' | Out-Null
    Press Down "TileMenu: Prompt\(Cancel\) of $firstName" | Out-Null
    $mark = Get-DashboardLogLength
    Press A "TileMenu: Menu\(Uninstall\) of $firstName" | Out-Null
    Expect-NotLogged '^store: asked Steam' $mark 'Uninstall cancelled' 1
    Press A "TileMenu: Prompt\(Uninstall\) of $firstName" | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^store: asked Steam to uninstall ${firstName}: .*steam\.exe -silent -console \+app_uninstall $($first.source_id)$" $mark 'Uninstall' | Out-Null
    Expect-Steam "steam: asked to uninstall $($first.source_id)" $steamMark 'Uninstall'
    Expect-Logged "^queue: $firstName uninstalled$" $mark 'Uninstalled' 30 | Out-Null
    In-Front
    Start-Sleep -Milliseconds 2000
    Save-Screen '11-uninstalled'

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
