# Settings > Storage and the folder question (#179, docs/spec/storage.md), walked with the virtual pad on a fake Steam
# with two libraries: its own folder and a second one added as Steam's Storage Manager adds one (`standin
# steam-library`). Nothing is downloaded: the stand-in Steam's client takes Steam's own calls over its CEF debugger, as
# in queue-steer, and does them to the fake's files.
#
# What it walks:
# - The Dashboard reads the drives and Steam's folders after its first frame: `storage: <n> drives; Steam: 2 folders,
#   installs into <the Steam folder> unless asked; ring on C:\`.
# - Settings > Storage, from Games & apps' Installs section: focus on Steam's ticked folder, the Steam folder
#   (`Settings: Storage: Steam <root>; the default; <n> drives`); the drives read again as it opens. Up to the second
#   library, A: it is the default from now (`settings: storage.default.steam = <second>`, the row ticked).
# - My games & apps: the storage ring at the nav's foot (a picture to look at).
# - Install from a game's Tile menu: "Where do you want to install <game>?" in the menu's place, focus on the second
#   library, the default now (`InstallWhere: Folder(0) <second>, the default of <game>`); Down to the Steam folder and Up
#   again; A installs there: `steam client: install <appid> into <second>: ... Steam's client took it` and the stand-in's
#   `steam: CEF: asked to install <appid> into <second>: <appid> queued in <second>`; the Queue's `joined, asked for`.
# - The Dashboard started again on the same data folder: focus logged first, then Settings > Storage opens on Steam's
#   ticked folder, still the second library.
#
# Judged by --log-focus a press at a time and the `storage:`, `settings:`, `steam client:` and `queue:` lines, and the
# stand-in client's `steam: CEF: ...` lines. Runs on the VM and in safe mode on the TV PC. The screenshots are to look at.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then a rest for the picture, if one is asked for. Returns the line. A press that went elsewhere ends the
# walk: every step after it would start from the wrong place.
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
        throw "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# Presses a direction along a row until focus is where $Expect says, $Most times at most; ends the walk if it never is.
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
        throw "$Button never reached 'focus: $Expect': '$(Focus-Now)'"
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

function Menu-Game([string] $Line) { if ($Line -match ' of (.+)$') { $Matches[1] } }

# From the Recent row's first tile, where focus must be, rightwards: opens each game's Tile menu until one is the menu
# of $Name; leaves it open and returns its line. B on a menu goes back to its tile.
function Open-MenuOf([string] $Name) {
    if ((Focus-Now) -notmatch '^focus: Recent\(0\)$') { throw "the Recent row's walk starts on Recent(0), not '$(Focus-Now)'" }
    foreach ($tile in 0..7) {
        $line = Press Start "TileMenu: Menu\((Play game|Install)\) of .+"
        if ((Menu-Game $line) -eq $Name) { return $line }
        Press B "Recent\($tile\)" | Out-Null
        Press Right "Recent\($($tile + 1)\)" | Out-Null
    }
    throw "none of the Recent row's first eight tiles is $Name's"
}

function In-Front { if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" } }

# The Dashboard on the fake, its own data folder, waited for until it has read the drives and Steam's folders. Each
# start writes its log afresh, so the lines looked for are from the first. Home starts on the profile and logs no focus
# until a press moves it.
function Start-OnFake {
    $mark = 0
    $script:dashboard = Start-Dashboard @('--log-focus', '--steam', "`"$script:root`"", '--data-dir', "`"$script:data`"")
    $started = Wait-DashboardLog 'first frame presented' 60 $mark
    if (-not $started) { throw 'The Dashboard never presented a frame' }
    Say "started: $started"
    Expect-Logged '^storage: \d+ drives?; Steam: 2 folders, installs into .+ unless asked; ' $mark 'Storage read' 30 | Out-Null
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 2
    In-Front
}

# Settings > Storage from Home as it starts, focus on the profile: the top bar's Settings (each Right logs where focus
# went), the rail down to Games & apps, right to Launchers and Storage, A.
function Open-Storage([string] $Expect) {
    Walk Right 'TopBar\(Settings\)' 5
    Press A 'Settings: Rail: General' | Out-Null
    Press Down 'Settings: Rail: Account' | Out-Null
    Press Down 'Settings: Rail: Games & apps' | Out-Null
    Press Right 'Settings: Games & apps: Launchers' | Out-Null
    Press Right 'Settings: Games & apps: Storage' | Out-Null
    $mark = Get-DashboardLogLength
    Press A "Settings: Storage: Steam $Expect; the default; \d+ drives?" | Out-Null
    Expect-Logged '^storage: \d+ drives?; Steam: 2 folders' $mark 'Storage read again as the page opened' | Out-Null
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $script:root = Get-FakeRoot 'steam'
    $library = Initialize-FakeSteam $script:root
    if (-not (Test-Path (Join-Path $script:root 'standin-client.txt'))) { throw "make-steam did not mark $script:root as the stand-in Steam" }
    # The most recently played of the games not installed: the furthest left of them on the Recent row, which is every
    # played game, newest first.
    $played = @(@($library.games) | Where-Object { -not $_.installed -and $_.last_played -gt 0 } | Sort-Object { [long]$_.last_played } -Descending)
    if ($played.Count -lt 1) { throw 'the fake has no played game that is not installed' }
    $game = $played[0]
    $name = [regex]::Escape($game.name)
    $appid = $game.source_id

    # --- A second library, as Steam's Storage Manager adds one.
    $second = Get-FakeRoot 'steam-library-2'
    if (Test-Path $second) { Remove-Item $second -Recurse -Force }
    $standin = Join-Path $PSScriptRoot 'standin.exe'
    $added = Start-Process $standin -ArgumentList @('steam-library', "`"$script:root`"", "`"$second`"") -Wait -PassThru
    if ($added.ExitCode -ne 0) { throw "standin steam-library exited with $($added.ExitCode)" }
    $rootRx = [regex]::Escape([IO.Path]::GetFullPath($script:root))
    $secondRx = [regex]::Escape([IO.Path]::GetFullPath($second))
    Say "to install $($game.name) ($appid) into $second"

    # --- The fake's client running, its debugger on: Steam can be told which library.
    New-Item (Join-Path $script:root '.cef-enable-remote-debugging') -ItemType File -Force | Out-Null
    $client = Start-Process (Join-Path $script:root 'steam.exe') -ArgumentList @('--standin-client', '--seconds', '1500') -WorkingDirectory $script:root -PassThru
    $portFile = Join-Path $script:root 'standin-cef-port.txt'
    $port = Wait-Until { if (Test-Path $portFile) { (Get-Content $portFile -Raw).Trim() } } 15
    if (-not $port) { throw "the stand-in Steam's client wrote no debugger port" }
    if ($port -eq '8080' -or $port -eq '8081') { throw "the stand-in's debugger took a real Steam's port, $port" }
    Say "the stand-in Steam's client, pid $($client.Id), its debugger on 127.0.0.1:$port"

    # A data folder of the test's own: the default chosen here is kept there, and nothing from another test is.
    $script:data = Get-TestFolder 'storage-data'
    if (Test-Path $script:data) { Remove-Item $script:data -Recurse -Force }
    New-Item $script:data -ItemType Directory -Force | Out-Null

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    Start-OnFake

    # --- Settings > Storage: Steam's own folder ticked; the second library chosen.
    Open-Storage $rootRx
    Start-Sleep -Milliseconds 3000
    Save-Screen '01-storage'
    Press Up "Settings: Storage: Steam $secondRx; not the default; \d+ drives?" | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Logged "^settings: storage\.default\.steam = $secondRx$" $mark 'The second library chosen' | Out-Null
    Expect-Logged "^focus: Settings: Storage: Steam $secondRx; the default; \d+ drives?$" $mark 'Ticked' | Out-Null
    Start-Sleep -Milliseconds 3000
    Save-Screen '02-storage-chosen'

    # --- My games & apps: the storage ring at the nav's foot. B climbs Settings a level a press: the page to its box,
    # the boxes to the rail, the rail out to the top bar's Settings, which opened it.
    Press B 'Settings: Games & apps: Storage' | Out-Null
    Press B 'Settings: Rail: Games & apps' | Out-Null
    Press B 'TopBar\(Settings\)' | Out-Null
    Walk Left 'TopBar\(MyGames\)' 4
    Press A 'MyGames: Games: Tile\(0\)' '03-my-games-storage-ring' | Out-Null
    # B from My games & apps is Home, focus on the Recent row's first tile.
    Press B 'Recent\(0\)' | Out-Null

    # --- Install: which folder, focus on the default chosen; the second library.
    Open-MenuOf $game.name | Out-Null
    Press A "InstallWhere: Folder\(0\) $secondRx, the default of $name" '04-which-folder' | Out-Null
    Press Down "InstallWhere: Folder\(1\) $rootRx of $name" | Out-Null
    Press Up "InstallWhere: Folder\(0\) $secondRx, the default of $name" | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    $what = [regex]::Escape("install $appid into ") + $secondRx
    Expect-Logged "^steam client: ${what}: asking Steam's client over its CEF debugger on 127\.0\.0\.1:\d+$" $mark 'Install into the second library, asked' | Out-Null
    Expect-Logged "^steam client: ${what}: Steam's client took it$" $mark 'Install into the second library, taken' | Out-Null
    Expect-Steam "steam: CEF: asked to ${what}: $appid queued in $secondRx" $steamMark 'The stand-in put it in the second library'
    Expect-Logged "^queue: $name joined, asked for$" $mark 'Install, the Queue' | Out-Null
    $fallback = Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match "^steam client: install $appid .*(not taken|lost|asking through steam\.exe)" } | Select-Object -First 1
    if ($fallback) { $failures += "Install: $fallback" }
    Start-Sleep -Milliseconds 3000
    Save-Screen '05-installing'

    # --- Started again on the same data folder: the choice kept. Focus is on the profile, logged by the first Right.
    Stop-Dashboard
    Start-OnFake
    Say "after the restart, focus logged so far: '$(Focus-Now)'"
    Open-Storage $secondRx
    Save-Screen '06-storage-after-restart'

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
