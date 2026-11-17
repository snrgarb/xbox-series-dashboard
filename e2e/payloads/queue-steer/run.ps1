# Pause, Resume and Cancel in the Queue (#170), walked with the virtual pad on a fake Steam library. Nothing is ever
# downloaded or deleted but the fake's own files: the Dashboard asks Steam's client through Steam's installer
# (`model::Installer`), here the stand-in client in the fake's folder, which `make-steam` marks as the stand-in
# (`standin-client.txt`) and which alone takes the stand-in's own commands (`steam.exe -silent +standin_pause
# <appid>`, `+standin_resume`, `+standin_cancel`): it rewrites the game's app manifest paused or downloading at the
# bytes it has, or removes it, as Steam's own files would show it. The real Steam answers "not supported" until #171,
# and its rows are not shown.
#
# The fake is named with --steam, on the VM too: Steam's installer looks for the stand-in's mark only in an install
# named on the command line, never in the one the registry names.
#
# What it walks:
# - Install from a game's Tile menu (`queue: <name> joined, asked for`, then `queued`), its download moved to 25%
#   by `standin steam-install`.
# - Manage Queue from Home's Downloads tile, Down to the download's row (`Installing... 25.0%`), View on it (`Back`
#   on the virtual Xbox 360 pad, as Menu is `Start`): the queued game's menu, Install first and focus on it, then
#   Pause installation (`focus: TileMenu: Menu(Pause installation) of <name>`) and Cancel installation.
# - Pause: the menu closes on the row, Steam's installer asks the stand-in (`store: asking Steam to pause <name>:
#   standin_pause <appid>`, the stand-in's `steam: asked to pause <appid>`), the Queue sees it paused (`queue: <name>
#   paused, 25% of 100 bytes`) and the row reads Paused 25.0%.
# - Resume, from the same menu, now Resume installation: `queue: <name> downloading, 25% of 100 bytes`, the row
#   Installing... again; the bytes move on to 60%.
# - Cancel: its question, Keep installing first (back to the menu on Cancel installation), then yes; the stand-in
#   drops the install (`queue: <name> dropped`), the row goes, focus goes to the first cover, and the game's Tile menu
#   on Home starts with Install again.
#
# Judged by --log-focus a press at a time, the Dashboard's `store:` and `queue:` lines, and the stand-in client's
# `steam: asked to ...` lines. Runs on the VM and in safe mode on the TV PC. The screenshots are to look at: the
# queued game's menu with Pause, the paused row, the menu with Resume, and the Cancel question.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then a rest for the picture, if one is asked for. Returns the line, or nothing.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
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

# Presses a direction until focus is where $Expect says, $Most times at most.
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

# A line the stand-in Steam client must have written, after the first $After of standin.log.
function Expect-Steam([string] $Pattern, [int] $After, [string] $What) {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    $line = Wait-Until { if (Test-Path $log) { Get-Content $log | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } } 15
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

function Menu-Game([string] $Line) { if ($Line -match ' of (.+)$') { $Matches[1] } }

# From the Recent row's first tile rightwards, opens each game's Tile menu until one is the menu of $Name; leaves it
# open and returns its line.
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
    if (-not (Test-Path (Join-Path $script:root 'standin-client.txt'))) { throw "make-steam did not mark $script:root as the stand-in Steam" }
    $played = @(@($library.games) | Where-Object { -not $_.installed -and $_.last_played -gt 0 })
    if ($played.Count -lt 1) { throw 'the fake has no played game that is not installed' }
    $game = $played[0]
    $name = [regex]::Escape($game.name)
    $appid = $game.source_id
    Say "to install, pause, resume and cancel: $($game.name) ($appid)"

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--steam', "`"$script:root`"")
    $started = Wait-DashboardLog 'first frame presented' 60
    if (-not $started) { throw 'The Dashboard never presented a frame' }
    Say "started: $started"
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 4
    In-Front
    Press Down 'Recent\(0\)' | Out-Null

    # --- Install from the Tile menu, and the download under way.
    Open-MenuOf $game.name | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^store: asking Steam to install ${name}: app_install $appid$" $mark 'Install' | Out-Null
    Expect-Logged "^queue: $name joined, asked for$" $mark 'Install, the Queue' | Out-Null
    Expect-Logged "^queue: $name queued$" $mark 'Install, Steam took it on' | Out-Null
    $mark = Get-DashboardLogLength
    Advance $appid @('--bytes', '25/100')
    Expect-Logged "^queue: $name downloading, 25% of 100 bytes$" $mark 'Downloading' | Out-Null
    In-Front

    # --- Manage Queue from Home's Downloads tile, down to the row.
    Press Down 'Wide\(\d\)' | Out-Null
    Walk Right 'Wide\(3\)' 4
    Press A 'ManageQueue: (Cover\(0\) of .+; )?.*1 installing, \d+ recently installed.*' | Out-Null
    $row = "ManageQueue: Row\(0\) of $name; 1 installing, \d+ recently installed"
    Walk Down "$row; Installing\.\.\. 25\.0%" 2

    # --- The queued game's menu: Install first, then Pause installation and Cancel installation.
    Press Back "TileMenu: Menu\(Install\) of $name" | Out-Null
    Press Down "TileMenu: Menu\(Pause installation\) of $name" '01-menu-pause' | Out-Null
    Press Down "TileMenu: Menu\(Cancel installation\) of $name" | Out-Null
    Press Up "TileMenu: Menu\(Pause installation\) of $name" | Out-Null

    # --- Pause.
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    # The row as it was, or already paused: Steam is asked on a thread of its own.
    Press A "$row; (Installing\.\.\.|Paused) 25\.0%" | Out-Null
    Expect-Logged "^store: asking Steam to pause ${name}: standin_pause $appid$" $mark 'Pause' | Out-Null
    Expect-Steam "steam: asked to pause ${appid}: -silent \+standin_pause $appid" $steamMark 'Pause'
    Expect-Logged "^queue: $name paused, 25% of 100 bytes$" $mark 'Pause, the Queue' | Out-Null
    Expect-Logged "^focus: $row; Paused 25\.0%$" $mark 'Pause, the row' | Out-Null
    Start-Sleep -Milliseconds 4500
    Save-Screen '02-row-paused'

    # --- Resume, from the same menu.
    Press Back "TileMenu: Menu\(Install\) of $name" | Out-Null
    Press Down "TileMenu: Menu\(Resume installation\) of $name" '03-menu-resume' | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A "$row; (Paused|Installing\.\.\.) 25\.0%" | Out-Null
    Expect-Logged "^store: asking Steam to resume ${name}: standin_resume $appid$" $mark 'Resume' | Out-Null
    Expect-Steam "steam: asked to resume ${appid}: -silent \+standin_resume $appid" $steamMark 'Resume'
    Expect-Logged "^queue: $name downloading, 25% of 100 bytes$" $mark 'Resume, the Queue' | Out-Null
    Expect-Logged "^focus: $row; Installing\.\.\. 25\.0%$" $mark 'Resume, the row' | Out-Null
    $mark = Get-DashboardLogLength
    Advance $appid @('--bytes', '60/100')
    Expect-Logged "^focus: $row; Installing\.\.\. 60\.0%$" $mark 'Resumed, the bytes move on' | Out-Null

    # --- Cancel: its question, Keep installing first, then yes.
    Press Back "TileMenu: Menu\(Install\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Cancel installation\) of $name" 3
    Press A "TileMenu: Prompt\(Cancel installation\) of $name" '04-cancel-question' | Out-Null
    Press Down "TileMenu: Prompt\(Keep installing\) of $name" | Out-Null
    $mark = Get-DashboardLogLength
    Press A "TileMenu: Menu\(Cancel installation\) of $name" | Out-Null
    Start-Sleep -Seconds 1
    if (Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^store: asking Steam to cancel' }) {
        $failures += 'Keep installing asked Steam to cancel'
    }
    Press A "TileMenu: Prompt\(Cancel installation\) of $name" | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A "ManageQueue: .+" | Out-Null
    Expect-Logged "^store: asking Steam to cancel ${name}: standin_cancel $appid$" $mark 'Cancel' | Out-Null
    Expect-Steam "steam: asked to cancel ${appid}: -silent \+standin_cancel $appid" $steamMark 'Cancel'
    Expect-Logged "^queue: $name dropped$" $mark 'Cancel, the Queue' | Out-Null
    Expect-Logged '^focus: ManageQueue: Cover\(0\) of .+; 0 installing, \d+ recently installed$' $mark 'Cancel, focus to the first cover' | Out-Null
    Start-Sleep -Milliseconds 4500
    Save-Screen '05-cancelled'

    # --- Back on Home, the game is not installed: its menu starts with Install.
    Press B 'Wide\(3\)' | Out-Null
    Press Up 'Recent\(\d+\)' | Out-Null
    $menu = Open-MenuOf $game.name
    if ($menu -and $menu -notmatch 'Menu\(Install\)') { $failures += "after Cancel, $($game.name)'s menu is '$menu'" }
    Press B 'Recent\(\d+\)' | Out-Null

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
