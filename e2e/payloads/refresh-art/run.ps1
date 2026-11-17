# Refresh all art and details (#190): the fake Steam (fourteen made-up games, their covers, heroes and headers in its
# librarycache, nothing square, as the real Steam keeps) and a stand-in on 127.0.0.1 (`standin serve-art`) that is
# both SteamGridDB (`--art-service`) and the stores' own art online (`--store-art-service <url>/stores`), so nothing
# here asks a real service.
#
# 1. With no key no source has a square picture for a Steam game: a Recent tile shows the cover whole, over its own
#    blur darkened, never cropped (docs/spec/home.md 3.3): the tile's left edge is darker than its middle.
# 2. A key is typed in through Settings > Art; SteamGridDB gives every game a square: the Recent tiles are its
#    yellow.
# 3. Settings > Art's Refresh all art and details, after its question: the log says what was forgotten (pictures,
#    more than none) and the cache is emptied, the page's line says how far it has got, and the Dashboard is killed
#    part way. Started again it goes on (`art: refresh: under way since an earlier start`) and ends
#    (`art: refresh: done, <n> pictures downloaded`): the cache is filled again, as many pictures as before, SteamGridDB
#    asked for squares again, and the Recent tiles are yellow once more. Home answers the pad while it works.
# 4. The Tile menu's More options > Refresh art on the first Recent tile: that game's pictures forgotten and fetched
#    again first, its tile yellow after.
#
# Judged by the Dashboard's log: the focus log after each press (`focus: Settings: Art: Refresh all art and details`,
# with `; refreshing <n> of <m>` or `; refreshed: <n> pictures` after it once a refresh is asked for; `focus:
# TileMenu: MoreOptions(Refresh art) of <game>`), `art: refresh: forgot <n> pictures, ...`, `art: refresh: done, <n>
# pictures downloaded`, `art: refresh: one game's art: forgot <n> pictures` and the lookup's own line; by serve-art's
# lines in standin.log; by the files in the data folder's `art` folder; and by pixels of the focused Recent tile (its
# top-left at x 105 + 203.87 i, y 533, 272.94 square: crates/screens/src/home/recent.rs). Passes when every press
# gives the line expected of it and every check holds, and the Dashboard stays up. Safe mode: every fake under the
# payload's folder, nothing registered.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
$steamRoot = Get-FakeRoot 'steam'
$data = Get-TestFolder 'dashboard-refresh-art-test'
$artFolder = Join-Path $data 'art'
$port = 47812
# Text entry opens on Q, upper case for the first letter: four presses of A type this.
$key = 'Qqqq'
# The stand-in SteamGridDB's square (crates/standin/src/main.rs, ART_COLOURS).
$yellow = @(0xE0, 0xD0, 0x20)
$rowScale = 1700 / 1526
$storeLog = Join-Path $env:E2E_OUT 'standin.log'
$lookupLine = '^art: \d+ looked up, \d+ found, \d+ none(?:, (?<failed>\d+) failed)?, in [\d.]+ s; pictures: \d+ store files, \d+ cache, (?<store>\d+) store online, (?<grid>\d+) SteamGridDB$'
$refreshing = 'Settings: Art: Refresh all art and details; refreshing \d+ of \d+'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { [string] (Focus-Lines | Select-Object -Last 1) }

$Gap = 150

function Tap([string] $Button) {
    Push-Button $Button
    Start-Sleep -Milliseconds $Gap
}

function Picture([string] $Name) {
    Start-Sleep -Milliseconds 4500
    Save-Screen $Name
}

function Wait-Focus([int] $Before, [string] $Pattern, [double] $Seconds) {
    Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before -and $lines[-1] -match "^focus: $Pattern`$") { $lines[-1] } } $Seconds
}

# One press that must be followed by a new focus line matching $Expect (what follows "focus: ", whole). A press the
# Dashboard never saw (no new line at all) is pressed once more; one that gave another line is a failure.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Tap $Button
    $got = Wait-Focus $before $Expect 8
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Tap $Button
        $got = Wait-Focus $before $Expect 8
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

# Presses a direction until focus is where $Expect says, $Most times at most: judged by where it ends.
function Walk([string] $Button, [string] $Expect, [int] $Most = 8) {
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match "^focus: $Expect`$") { break }
        Tap $Button
        Start-Sleep -Milliseconds 650
    }
    if ((Focus-Now) -notmatch "^focus: $Expect`$") { throw "$Button never reached '$Expect': $(Focus-Now)" }
    Say "$Button until $(Focus-Now)"
}

# From Home, focus on the Recent row or the top bar, to Settings > Art, focus on Type. The page's state, once a
# refresh has been asked for, follows its focus after a `;`.
function Open-Art {
    Walk Up 'TopBar\(.+\)' 4
    Walk Right 'TopBar\(Settings\)' 6
    Press A 'Settings: Rail: Theme settings' | Out-Null
    Press Right 'Settings: Theme settings: Personalisation' | Out-Null
    Press Down 'Settings: Theme settings: Guide' | Out-Null
    Press Down 'Settings: Theme settings: Games & apps' | Out-Null
    Press Down 'Settings: Theme settings: Art' | Out-Null
    Press A 'Settings: Art: Type(; .+)?' | Out-Null
}

function Leave-Settings {
    Press B 'Settings: Theme settings: Art' | Out-Null
    Press B 'Settings: Rail: Theme settings' | Out-Null
    Press B 'TopBar\(Settings\)' | Out-Null
}

# The pixel at ($X, $Y) once two samples 300 ms apart agree (a tile grows, a picture fades in), six seconds at most.
function Settled-Pixel([double] $X, [double] $Y) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Milliseconds 300
    $pixel = Get-DesignPixel $X $Y
    while ($watch.Elapsed.TotalSeconds -lt 6) {
        Start-Sleep -Milliseconds 300
        $next = Get-DesignPixel $X $Y
        if ($next.R -eq $pixel.R -and $next.G -eq $pixel.G -and $next.B -eq $pixel.B) { break }
        $pixel = $next
    }
    $pixel
}

function Light($Pixel) { [int] $Pixel.R + [int] $Pixel.G + [int] $Pixel.B }

# The Recent row's game tiles, focused one by one from the first, as the log says: each tile's colour near its top
# middle, above the title band, and near its left edge, once its growth has settled. Back on the top bar after.
function Look-AtRecent([string] $Name, [int] $Tiles = 6) {
    if ((Focus-Now) -notmatch '^focus: Recent\(\d+\)$') { Press Down 'Recent\(\d+\)' | Out-Null }
    Walk Left 'Recent\(0\)' 10
    $seen = @()
    foreach ($tile in 0..($Tiles - 1)) {
        if ($tile -gt 0) {
            $before = @(Focus-Lines).Count
            Tap Right
            if (-not (Wait-Focus $before "Recent\($tile\)" 4)) { break }
        }
        $left = 105 + $tile * 183 * $rowScale
        $middle = Settled-Pixel ($left + 245 * $rowScale / 2) (533 + 60)
        $edge = Get-DesignPixel ($left + 245 * $rowScale * 0.06) (533 + 60)
        Say "$Name`: Recent($tile) is $(Format-Colour $middle) in the middle, $(Format-Colour $edge) at the left edge"
        $seen += [pscustomobject] @{ Middle = $middle; Edge = $edge }
    }
    Save-Screen $Name
    Walk Up 'TopBar\(.+\)' 4
    $seen
}

function Count-Yellow($Seen) { @($Seen | Where-Object { Test-Colour $_.Middle $yellow 40 }).Count }

function Count-Pictures {
    if (-not (Test-Path $artFolder)) { return 0 }
    @(Get-ChildItem $artFolder -Recurse -File | Where-Object { $_.Extension -in '.png', '.jpg', '.webp' }).Count
}

function Get-StandInLines([string] $Pattern, [int] $After = 0) {
    @(Get-Content $storeLog -ErrorAction SilentlyContinue | Select-Object -Skip $After | Where-Object { $_ -match $Pattern })
}

function Start-ThisDashboard {
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--steam', "`"$steamRoot`"", '--data-dir', "`"$data`"",
                                          '--art-service', "http://127.0.0.1:$port",
                                          '--store-art-service', "http://127.0.0.1:$port/stores")
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $steam = Initialize-FakeSteam $steamRoot
    if (@($steam.games | Where-Object last_played -gt 0).Count -lt 6) { throw 'the fake Steam should have six played games' }
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }

    $standin = Join-Path $PSScriptRoot 'standin.exe'
    if (-not (Test-Path $standin)) { throw "No standin.exe in $PSScriptRoot" }
    $server = Start-Process $standin -ArgumentList @('serve-art', '--port', "$port", '--key', $key) -PassThru
    $listening = Wait-Until {
        Get-Content $storeLog -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "serve-art: listening on http://127\.0\.0\.1:$port" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the stand-in SteamGridDB never said it was listening' }
    Say "stand-in: $listening"

    Start-ThisDashboard
    $done = Wait-DashboardLog $lookupLine 120
    Say "lookup with no key: $(if ($done) { $done } else { 'no art: line in 120 s' })"
    if (-not $done) { $failures += 'the first lookup never ended' }
    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }

    # --- 1. No square anywhere: the cover whole, never cropped.
    $row = Look-AtRecent '1-recent-no-square'
    if (Count-Yellow $row) { $failures += 'with no key no Recent tile should have a square picture' }
    $whole = @($row | Where-Object { (Light $_.Edge) -lt 0.8 * (Light $_.Middle) }).Count
    Say "$whole of $($row.Count) tiles show the cover whole, their edges darker than their middles"
    if ($whole -lt [math]::Ceiling($row.Count / 2)) {
        $failures += "a Recent tile with no square picture should show its cover whole over its own darkened blur ($whole of $($row.Count) did)"
    }

    # --- 2. The key: SteamGridDB's squares.
    Open-Art
    Press A 'TextEntry: Q; "\|" upper letters' | Out-Null
    foreach ($n in 1..$key.Length) { Press A "TextEntry: Q; `"$('\*' * $n)\|`" .+" | Out-Null }
    $mark = Get-DashboardLogLength
    Press Start 'Settings: Art: Type' | Out-Null
    $done = Wait-DashboardLog $lookupLine 180 $mark
    Say "lookup with the key: $(if ($done) { $done } else { 'no art: line in 180 s' })"
    if (-not $done -or $done -notmatch $lookupLine -or [int] $Matches['grid'] -eq 0) { $failures += "SteamGridDB should have given pictures: '$done'" }
    Leave-Settings
    $row = Look-AtRecent '2-recent-with-key'
    if ((Count-Yellow $row) -lt $row.Count) { $failures += "with the key every Recent tile should be SteamGridDB's yellow square ($(Count-Yellow $row) of $($row.Count))" }
    $cached = Count-Pictures
    Say "the art cache holds $cached pictures"
    if ($cached -eq 0) { $failures += 'the art cache should hold pictures after the lookup with the key' }

    # --- 3. Refresh all art and details: asked first, the cache emptied, killed part way, going on at the next start.
    Open-Art
    Press Down 'Settings: Art: Clear downloaded art' | Out-Null
    Press Right 'Settings: Art: Refresh all art and details' '3-refresh-button' | Out-Null
    Press A 'Settings: Art: Prompt\(Refresh\) of Refresh all art and details' '4-refresh-prompt' | Out-Null
    $mark = Get-DashboardLogLength
    $standinMark = @(Get-Content $storeLog -ErrorAction SilentlyContinue).Count
    Press A $refreshing | Out-Null
    $forgot = Wait-DashboardLog '^art: refresh: forgot (\d+) pictures' 10 $mark
    Say "forgot: $(if ($forgot) { $forgot } else { 'no line' })"
    if (-not $forgot -or $forgot -notmatch 'forgot (\d+) pictures' -or [int] $Matches[1] -lt $cached) {
        $failures += "the refresh should have forgotten every picture in the cache ($cached): '$forgot'"
    }
    # How far it has got, said on the page as it goes, with no press.
    $going = Wait-Until { Focus-Lines | Select-Object -Skip 0 | Where-Object { $_ -match "^focus: Settings: Art: Refresh all art and details; refreshing [1-9]\d* of \d+$" } | Select-Object -Last 1 } 60
    Picture '5-refreshing'
    Say "going: $(if ($going) { $going } else { 'no line' })"
    if (-not $going) { $failures += 'the page should say how far the refresh has got' }
    # Home answers the pad while it works.
    Leave-Settings
    Press Down 'Recent\(\d+\)' | Out-Null
    Press Right 'Recent\(\d+\)' | Out-Null
    $early = Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^art: refresh: done' } | Select-Object -First 1
    if ($early) {
        Say "note: the refresh was over before the Dashboard could be stopped part way: $early"
    } else {
        Say 'stopping the Dashboard part way through the refresh'
        Stop-Dashboard
        Start-ThisDashboard
        $again = Wait-DashboardLog '^art: refresh: under way since an earlier start$' 30
        if (-not $again) { $failures += "started again, the Dashboard should have said the refresh is still under way" }
        $mark = 0
    }
    $over = Wait-DashboardLog '^art: refresh: done, (\d+) pictures downloaded$' 240 $mark
    Say "refreshed: $(if ($over) { $over } else { 'no line in 240 s' })"
    if (-not $over) { $failures += 'the refresh never said it was done' }
    Start-Sleep -Seconds 3
    $refilled = Count-Pictures
    Say "the art cache holds $refilled pictures again"
    if ($refilled -lt $cached) { $failures += "the cache should be filled again: $refilled pictures, $cached before" }
    $squares = Get-StandInLines 'serve-art: /grids/game/.*dimensions=1024x1024,512x512.* \(the key\)$' $standinMark
    Say "SteamGridDB was asked for $($squares.Count) squares during the refresh"
    if (-not $squares.Count) { $failures += 'the refresh should have asked SteamGridDB for squares again' }
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    # A new start's log has no focus line yet: Home starts on the top bar's profile, which it does not log, and Up
    # from there moves nothing. Down to the Recent row says where focus is, as at the first start.
    if (-not (Focus-Now)) { Press Down 'Recent\(\d+\)' | Out-Null }
    Open-Art
    Press Down 'Settings: Art: Clear downloaded art; refreshed: \d+ pictures' | Out-Null
    Press Right 'Settings: Art: Refresh all art and details; refreshed: \d+ pictures' '6-refreshed' | Out-Null
    Leave-Settings
    $row = Look-AtRecent '7-recent-refreshed'
    if ((Count-Yellow $row) -lt $row.Count) { $failures += "after the refresh every Recent tile should be yellow again ($(Count-Yellow $row) of $($row.Count))" }

    # --- 4. The Tile menu's Refresh art, on the first Recent tile.
    Press Down 'Recent\(\d+\)' | Out-Null
    Walk Left 'Recent\(0\)' 10
    $menu = Press Menu 'TileMenu: Menu\(.+\) of .+'
    $game = if ($menu -match ' of (.+)$') { $Matches[1] } else { '' }
    Walk Down "TileMenu: Menu\(More options\) of $([regex]::Escape($game))" 7
    Press A "TileMenu: MoreOptions\(Hide this game\) of $([regex]::Escape($game))" | Out-Null
    Press Down "TileMenu: MoreOptions\(Refresh art\) of $([regex]::Escape($game))" '8-refresh-art-row' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Recent\(0\)' | Out-Null
    $one = Wait-DashboardLog "^art: refresh: one game's art: forgot (\d+) pictures$" 10 $mark
    Say "one game: $(if ($one) { $one } else { 'no line' })"
    if (-not $one -or $one -match 'forgot 0 ') { $failures += "Refresh art should have forgotten the game's pictures: '$one'" }
    $done = Wait-DashboardLog $lookupLine 120 $mark
    if (-not $done -or $done -notmatch $lookupLine -or [int] $Matches['grid'] -eq 0) { $failures += "the game's art should have been fetched again: '$done'" }
    $tile = Settled-Pixel (105 + 245 * $rowScale / 2) (533 + 60)
    Save-Screen '9-recent-one-refreshed'
    Say "Recent(0) after Refresh art is $(Format-Colour $tile)"
    if (-not (Test-Colour $tile $yellow 40)) { $failures += "after Refresh art the first Recent tile should be yellow again, and is $(Format-Colour $tile)" }

    if (Get-DashboardLog | Where-Object { $_ -cmatch [regex]::Escape($key) }) { $failures += 'the key is in dashboard.log' }
    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Dashboard
    if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
