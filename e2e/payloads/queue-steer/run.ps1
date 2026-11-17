# Steam's own client calls over its CEF debugger (#171) and the Workshop phase (#188), walked with the virtual pad on
# a fake Steam library: install, pause, resume, cancel, a second install to the end with its Workshop items coming in
# while Play is offered, and uninstall with no prompt. Nothing is ever downloaded or deleted but the fake's own files.
#
# The fake's client runs as `steam.exe --standin-client` from the fake's folder, with `.cef-enable-remote-debugging`
# there, so it opens the stand-in's CEF debugger (`source_steam::debugger`) on a free port of 127.0.0.1 that it writes
# in `standin-cef-port.txt`, never 8080 or 8081, where a real Steam's may be. The Dashboard asks it exactly as it asks
# the real client (`source_steam::client`: the same scripts, the same log lines); the stand-in reads what each script
# asks from its first line and does it to the fake's files. The fake is named with --steam, on the VM too: its
# debugger's port is looked for only in an install named on the command line, never in the registry's.
#
# What it walks:
# - Install from a game's Tile menu: `steam client: install <appid> into Steam's default library: asking ...`, a line
#   a call, `... Steam's client took it`, never `asking through steam.exe instead`; the stand-in's `steam: CEF: asked
#   to install <appid> ...`; the Queue's `joined, asked for` and `queued`. The stand-in holds the scripts to the real
#   client's signatures (#171, seen on the TV PC 2026-11-17: every call with the client id "0", the uninstall with
#   three arguments) and, as the real one, brings its main window up on the install wizard: the Dashboard puts it
#   back hidden (`steam: CEF: the main window, asked to Hide`) and keeps the foreground. Its download moved to 25% by
#   `standin steam-install`.
# - Manage Queue from Home's Downloads tile, Down to the row (`Installing... 25.0%`), View on it (`Back` on the virtual
#   Xbox 360 pad, as Menu is `Start`): Pause installation first, then Cancel installation, and no Install (#171); the
#   same on the game's Home tile while it downloads.
# - Pause, Resume (then the bytes on to 60%), and Cancel after its question (Keep installing first): each `steam
#   client: <verb> <appid>: Steam's client took it` and the stand-in's `steam: CEF: asked to <verb> <appid>`; the row
#   Paused 25.0%, Installing... again, and gone (`queue: <name> dropped`, focus to the first cover).
# - A second install of the same game, to the end: Steam's client takes it; its download at half; the stand-in's
#   Workshop record written with four items, none in (`standin steam-workshop`); then its own files done. The Queue
#   says it is playable (`queue: <name> installed; its store is still fetching more for it`), the Ready to play Toast
#   fires, and its Tile menu on Home starts with Play game while the items are still coming. In Manage Queue its row
#   reads `Downloading Workshop content 0 of 4 items 0.0%`, then 2 of 4 (30.0%: the stand-in's items are 4, 8, 12 and
#   16 MB), and once all four are in it leaves (`queue: <name> has all its store was fetching for it`).
# - Uninstall from the Tile menu, after the Dashboard's own question: `steam client: uninstall <appid>: Steam's client
#   took it` (`Installs.OpenUninstallWizard([appid], "0", true)`, which passes over Steam's own question, taken only
#   once the manifest says so), and `queue: <name> uninstalled`; no window of Steam's comes forward (the Dashboard keeps the foreground).
#
# Judged by --log-focus a press at a time, the Dashboard's `steam client:`, `store:` and `queue:` lines, and the stand-in
# client's `steam: CEF: ...` lines. Runs on the VM and in safe mode on the TV PC. The screenshots are to look at.
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

# No line like this after the mark $After, $Seconds on.
function Expect-NotLogged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 2) {
    Start-Sleep -Seconds $Seconds
    $line = Get-DashboardLog | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -First 1
    if ($line) { Say "${What}: $line, and should not be"; $script:failures += "${What}: the Dashboard logged '$line'" } else { Say "${What}: nothing like '$Pattern', as it should be" }
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

# Runs `standin <Command> <fake> <Appid> <Rest>`, as Steam's client would move its files along.
function Standin([string] $Command, [string] $Appid, [string[]] $Rest) {
    $standin = Join-Path $PSScriptRoot 'standin.exe'
    $arguments = @($Command, "`"$script:root`"", $Appid) + $Rest
    $done = Start-Process $standin -ArgumentList $arguments -Wait -PassThru
    Say "$Command $Appid $($Rest -join ' '): exit $($done.ExitCode)"
    if ($done.ExitCode -ne 0) { $script:failures += "standin $Command $Appid $($Rest -join ' ') exited with $($done.ExitCode)" }
}
function Advance([string] $Appid, [string[]] $To) { Standin 'steam-install' $Appid $To }
function Workshop([string] $Appid, [string] $Of) { Standin 'steam-workshop' $Appid @($Of) }

function Menu-Game([string] $Line) { if ($Line -match ' of (.+)$') { $Matches[1] } }
function Menu-Row([string] $Line) { if ($Line -match '\((.+?)\) of ') { $Matches[1] } }

# From the Recent row's first tile rightwards, opens each game's Tile menu until one is the menu of $Name; leaves it
# open and returns its line.
function Open-MenuOf([string] $Name) {
    Walk Left 'Recent\(0\)'
    foreach ($tile in 1..8) {
        $line = Press Start 'TileMenu: Menu\((Play game|Install|Pause installation|Resume installation)\) of .+'
        if (-not $line) { return $null }
        if ((Menu-Game $line) -eq $Name) { return $line }
        Press B 'Recent\(\d+\)' | Out-Null
        Press Right 'Recent\(\d+\)' | Out-Null
    }
    $script:failures += "no tile of the Recent row is $Name's"
    $null
}

function In-Front { if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" } }

# Manage Queue from Home's Downloads tile, with focus on the Recent row.
function Open-Queue([string] $Count) {
    Press Down 'Wide\(\d\)' | Out-Null
    Walk Right 'Wide\(3\)' 4
    Press A "ManageQueue: (Cover\(0\) of .+; )?.*$Count installing, \d+ recently installed.*" | Out-Null
}

# Each of Steam's own calls must be taken by the client, and never fall back to steam.exe.
function Expect-Took([string] $What, [int] $Mark, [int] $SteamMark, [string] $Steam, [string] $Label) {
    $what = [regex]::Escape($What)
    Expect-Logged "^steam client: ${what}: asking Steam's client over its CEF debugger on 127\.0\.0\.1:\d+$" $Mark "$Label, asked" | Out-Null
    Expect-Logged "^steam client: ${what}: Steam's client took it$" $Mark "$Label, taken" | Out-Null
    Expect-Steam "steam: CEF: asked to ${what}: $Steam" $SteamMark $Label
    $fallback = Get-DashboardLog | Select-Object -Skip $Mark | Where-Object { $_ -match "^steam client: ${what}: (not taken|lost|asking through steam\.exe)" } | Select-Object -First 1
    if ($fallback) { $script:failures += "${Label}: $fallback" }
}

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
    Say "to install, pause, resume, cancel, install with Workshop items, and uninstall: $($game.name) ($appid)"

    # --- The fake's client running, its debugger on.
    New-Item (Join-Path $script:root '.cef-enable-remote-debugging') -ItemType File -Force | Out-Null
    $client = Start-Process (Join-Path $script:root 'steam.exe') -ArgumentList @('--standin-client', '--seconds', '1500') -WorkingDirectory $script:root -PassThru
    $portFile = Join-Path $script:root 'standin-cef-port.txt'
    $port = Wait-Until { if (Test-Path $portFile) { (Get-Content $portFile -Raw).Trim() } } 15
    if (-not $port) { throw "the stand-in Steam's client wrote no debugger port" }
    if ($port -eq '8080' -or $port -eq '8081') { throw "the stand-in's debugger took a real Steam's port, $port" }
    Say "the stand-in Steam's client, pid $($client.Id), its debugger on 127.0.0.1:$port"

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--steam', "`"$script:root`"")
    $started = Wait-DashboardLog 'first frame presented' 60
    if (-not $started) { throw 'The Dashboard never presented a frame' }
    Say "started: $started"
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 4
    In-Front
    Press Down 'Recent\(0\)' | Out-Null

    # --- Install from the Tile menu, through Steam's own calls.
    Open-MenuOf $game.name | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Took "install $appid into Steam's default library" $mark $steamMark "$appid queued in" 'Install'
    Expect-Steam 'steam: CEF: the main window, asked to Hide: it was Showing, it is Hidden' $steamMark 'Install, Steam''s main window put back'
    $front = Get-Foreground
    if ($front.Title -ne 'Xbox Series Dashboard') { $failures += "after the install, '$($front.Title)' ($($front.Process)) has the foreground" }
    Expect-Logged "^queue: $name joined, asked for$" $mark 'Install, the Queue' | Out-Null
    Expect-Logged "^queue: $name queued$" $mark 'Install, Steam took it on' | Out-Null
    $mark = Get-DashboardLogLength
    Advance $appid @('--bytes', '25/100')
    Expect-Logged "^queue: $name downloading, 25% of 100 bytes$" $mark 'Downloading' | Out-Null
    In-Front

    # --- Its Home tile's menu while it downloads (#171): Pause installation first, then Cancel installation; no Install.
    $menu = Open-MenuOf $game.name
    if ($menu -and (Menu-Row $menu) -ne 'Pause installation') { $failures += "$($game.name)'s Home menu while it downloads starts with $(Menu-Row $menu)" }
    Press Down "TileMenu: Menu\(Cancel installation\) of $name" '00-home-menu-installing' | Out-Null
    Press B 'Recent\(\d+\)' | Out-Null
    Walk Left 'Recent\(0\)'

    # --- Manage Queue from Home's Downloads tile, down to the row.
    Open-Queue '1'
    $row = "ManageQueue: Row\(0\) of $name; 1 installing, \d+ recently installed"
    Walk Down "$row; Installing\.\.\. 25\.0%" 2

    # --- The queued game's menu: Pause installation first, then Cancel installation; no Install (#171).
    Press Back "TileMenu: Menu\(Pause installation\) of $name" '01-menu-pause' | Out-Null
    Press Down "TileMenu: Menu\(Cancel installation\) of $name" | Out-Null
    Press Up "TileMenu: Menu\(Pause installation\) of $name" | Out-Null

    # --- Pause.
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    # The row as it was, or already paused: Steam is asked on a thread of its own.
    Press A "$row; (Installing\.\.\.|Paused) 25\.0%" | Out-Null
    Expect-Took "pause $appid" $mark $steamMark "$appid is Paused" 'Pause'
    Expect-Logged "^queue: $name paused, 25% of 100 bytes$" $mark 'Pause, the Queue' | Out-Null
    Expect-Logged "^focus: $row; Paused 25\.0%$" $mark 'Pause, the row' | Out-Null
    Start-Sleep -Milliseconds 4500
    Save-Screen '02-row-paused'

    # --- Resume, from the same menu.
    Press Back "TileMenu: Menu\(Resume installation\) of $name" '03-menu-resume' | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A "$row; (Paused|Installing\.\.\.) 25\.0%" | Out-Null
    Expect-Took "resume $appid" $mark $steamMark "$appid is Downloading" 'Resume'
    Expect-Logged "^queue: $name downloading, 25% of 100 bytes$" $mark 'Resume, the Queue' | Out-Null
    Expect-Logged "^focus: $row; Installing\.\.\. 25\.0%$" $mark 'Resume, the row' | Out-Null
    $mark = Get-DashboardLogLength
    Advance $appid @('--bytes', '60/100')
    Expect-Logged "^focus: $row; Installing\.\.\. 60\.0%$" $mark 'Resumed, the bytes move on' | Out-Null

    # --- Cancel: its question, Keep installing first, then yes.
    Press Back "TileMenu: Menu\(Pause installation\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Cancel installation\) of $name" 3
    Press A "TileMenu: Prompt\(Cancel installation\) of $name" '04-cancel-question' | Out-Null
    Press Down "TileMenu: Prompt\(Keep installing\) of $name" | Out-Null
    $mark = Get-DashboardLogLength
    Press A "TileMenu: Menu\(Cancel installation\) of $name" | Out-Null
    Expect-NotLogged "^steam client: cancel" $mark 'Keep installing' 1
    Press A "TileMenu: Prompt\(Cancel installation\) of $name" | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A "ManageQueue: .+" | Out-Null
    Expect-Took "cancel $appid" $mark $steamMark "$appid is Dropped" 'Cancel'
    # In mid-download: paused first and let stop, so the stand-in drops the manifest rather than leaving it at 514.
    Expect-Logged "^steam client: cancel ${appid}: the download stopped \(its manifest says StateFlags \d+\) \d+ ms after the pause$" $mark 'Cancel, the pause let settle' | Out-Null
    Expect-Logged "^steam client: cancel ${appid}: its manifest is gone, \d+ ms on$" $mark 'Cancel, the manifest gone' | Out-Null
    Expect-Logged "^queue: $name dropped$" $mark 'Cancel, the Queue' | Out-Null
    Expect-Logged '^focus: ManageQueue: Cover\(0\) of .+; 0 installing, \d+ recently installed$' $mark 'Cancel, focus to the first cover' | Out-Null
    Start-Sleep -Milliseconds 4500
    Save-Screen '05-cancelled'

    # --- Back on Home, the game is not installed: its menu starts with Install. Install it again, to the end.
    Press B 'Wide\(3\)' | Out-Null
    Press Up 'Recent\(\d+\)' | Out-Null
    $menu = Open-MenuOf $game.name
    if ($menu -and (Menu-Row $menu) -ne 'Install') { $failures += "after Cancel, $($game.name)'s menu is '$menu'" }
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Took "install $appid into Steam's default library" $mark $steamMark "$appid queued in" 'Second install'
    Expect-Logged "^queue: $name joined, asked for$" $mark 'Second install, the Queue' | Out-Null
    $mark = Get-DashboardLogLength
    Advance $appid @('--bytes', '50/100')
    Expect-Logged "^queue: $name downloading, 50% of 100 bytes$" $mark 'Second install, downloading' | Out-Null

    # --- Its own files done while its four Workshop items are still to come: playable, told of, Play offered.
    Workshop $appid '0/4'
    $mark = Get-DashboardLogLength
    Advance $appid @('--done')
    Expect-Logged "^queue: $name installed; its store is still fetching more for it$" $mark 'Playable, its Workshop items coming' 20 | Out-Null
    Expect-Logged '^toasts: shown "Ready to play" \(Installed\)' $mark 'Playable, its Toast' 20 | Out-Null
    $notDone = Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match "^queue: $name (installed$|has all )" } | Select-Object -First 1
    if ($notDone) { $failures += "the Queue let it go with its Workshop items still coming: $notDone" }
    In-Front
    $menu = Open-MenuOf $game.name
    if ($menu -and (Menu-Row $menu) -ne 'Play game') { $failures += "while its Workshop items come in, $($game.name)'s menu starts with $(Menu-Row $menu), not Play game" }
    Start-Sleep -Milliseconds 3000
    Save-Screen '06-play-offered-during-workshop'
    Press B 'Recent\(\d+\)' | Out-Null

    # --- Its row in Manage Queue: Downloading Workshop content, the items in, no speed; then on, then gone.
    Open-Queue '1'
    $workshopRow = "ManageQueue: Row\(0\) of $name; 1 installing, \d+ recently installed; Downloading Workshop content"
    Walk Down "$workshopRow 0 of 4 items 0\.0%" 3
    Start-Sleep -Milliseconds 4500
    Save-Screen '07-workshop-row'
    $mark = Get-DashboardLogLength
    Workshop $appid '2/4'
    Expect-Logged "^queue: $name downloading Workshop content, 2 of 4 items, \d+ of \d+ bytes$" $mark 'Workshop, two of four' | Out-Null
    Expect-Logged "^focus: $workshopRow 2 of 4 items 30\.0%$" $mark 'Workshop, the row moves on' | Out-Null
    Start-Sleep -Milliseconds 4500
    Save-Screen '08-workshop-row-half'
    $mark = Get-DashboardLogLength
    Workshop $appid '4/4'
    Expect-Logged "^queue: $name has all its store was fetching for it$" $mark 'Workshop, all in' 20 | Out-Null
    Expect-Logged "^focus: ManageQueue: Cover\(0\) of $name; 0 installing, \d+ recently installed$" $mark 'Workshop, all in: the row goes' | Out-Null
    $second = Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^toasts: shown "Ready to play"' } | Select-Object -First 1
    if ($second) { $failures += "a second Ready to play Toast as the Workshop items came in: $second" }
    Press B 'Wide\(3\)' | Out-Null
    Press Up 'Recent\(\d+\)' | Out-Null

    # --- Uninstall from the Tile menu: the Dashboard's own question, then Steam's client, which asks nothing.
    $menu = Open-MenuOf $game.name
    if ($menu -and (Menu-Row $menu) -ne 'Play game') { $failures += "$($game.name)'s menu starts with $(Menu-Row $menu) once installed" }
    Walk Down "TileMenu: Menu\(Uninstall\) of $name" 6
    Press A "TileMenu: Prompt\(Uninstall\) of $name" '09-uninstall-question' | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Took "uninstall $appid" $mark $steamMark "$appid uninstalled" 'Uninstall'
    Expect-Logged "^queue: $name uninstalled$" $mark 'Uninstalled' 30 | Out-Null
    Expect-NotLogged '^store: asking Steam to uninstall' $mark 'Uninstall, no steam.exe' 0
    $front = Get-Foreground
    if ($front.Title -ne 'Xbox Series Dashboard') { $failures += "after the uninstall, '$($front.Title)' ($($front.Process)) has the foreground" }
    Start-Sleep -Milliseconds 4500
    Save-Screen '10-uninstalled'

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($client.HasExited) { $failures += "the stand-in Steam's client exited with $($client.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Get-OwnProcess steam | Stop-Process -Force -ErrorAction SilentlyContinue
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
