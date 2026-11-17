# The art picker (#213, docs/spec/art-picker.md): the fake Steam and a stand-in on 127.0.0.1 (`standin serve-art`)
# that is both SteamGridDB (`--art-service`, 60 candidates of every kind, served 50 a page as SteamGridDB serves them)
# and the stores' own art online, so nothing here asks a real service.
#
# 1. A key is typed in through Settings > Art.
# 2. The Tile menu's More options > Change art on the first Recent tile: the picker on its Square tab. Right past the
#    grid's edge, page after page, to the last page ("Page n of n"), and A on a candidate there. The stand-in was asked
#    for the square list's pages 0 and 1, each once and in that order, and for nothing past them; the Dashboard logs
#    `art: picker: square is SteamGridDB picture <id>`.
# 3. Change art again, RB twice to Background, Right to its third page, and A on a candidate there.
# 4. Opened again: both marked in use (`in use: Picture <id>` on each tab).
# 5. Settings > Art's Refresh all art and details, asked and confirmed, to its end; then the Tile menu's Refresh art
#    on the same game. After each, the picker still says both are in use.
#
# The pad has no triggers in E2E.psm1, so pages are changed by Right past the grid's edge, which the picker takes as
# RT. Judged by the Dashboard's log: the focus log after each press (`focus: ArtPicker: <Tab>: Page n of N: <cell>;
# in use: <what>`), `art: picker: <kind> is SteamGridDB picture <id>`, `art: refresh: done, ...`; and by serve-art's
# lines in standin.log. Passes when every press gives the line expected of it and every check holds, and the
# Dashboard stays up. Safe mode: every fake under the payload's folder, nothing registered.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
$steamRoot = Get-FakeRoot 'steam'
$data = Get-TestFolder 'dashboard-art-picker-test'
$port = 47813
# Text entry opens on Q, upper case for the first letter: four presses of A type this.
$key = 'Qqqq'
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

# Waits, with no press, for a focus line matching $Expect: a page that has come.
function Await([string] $Expect, [double] $Seconds = 20) {
    $got = Wait-Until { $now = Focus-Now; if ($now -match "^focus: $Expect`$") { $now } } $Seconds
    if (-not $got) { throw "never '$Expect': $(Focus-Now)" }
    Say "came: $got"
    $got
}

function Open-Art {
    Walk Up 'TopBar\(.+\)' 4
    Walk Right 'TopBar\(Settings\)' 6
    Press A 'Settings: Rail: General' | Out-Null
    Press Down 'Settings: Rail: Account' | Out-Null
    Press Down 'Settings: Rail: Games & apps' | Out-Null
    Press Right 'Settings: Games & apps: Launchers' | Out-Null
    Press Down 'Settings: Games & apps: Games & apps' | Out-Null
    Press Down 'Settings: Games & apps: Groups' | Out-Null
    Press Down 'Settings: Games & apps: Art' | Out-Null
    Press A 'Settings: Art: Type(; .+)?' | Out-Null
}

function Leave-Settings {
    Press B 'Settings: Games & apps: Art' | Out-Null
    Press B 'Settings: Rail: Games & apps' | Out-Null
    Press B 'TopBar\(Settings\)' | Out-Null
}

# From Home to the first Recent tile's Tile menu, focus on More options' $Row.
function Open-MoreOptions([string] $Row) {
    if ((Focus-Now) -notmatch '^focus: Recent\(\d+\)$') { Press Down 'Recent\(\d+\)' | Out-Null }
    Walk Left 'Recent\(0\)' 10
    $menu = Press Start 'TileMenu: Menu\(.+\) of .+'
    $script:game = if ($menu -match ' of (.+)$') { $Matches[1] } else { throw "no game in '$menu'" }
    $name = [regex]::Escape($script:game)
    Walk Down "TileMenu: Menu\(More options\) of $name" 7
    Press A "TileMenu: MoreOptions\(Hide this game\) of $name" | Out-Null
    Press Down "TileMenu: MoreOptions\(Refresh art\) of $name" | Out-Null
    if ($Row -eq 'Change art') { Press Down "TileMenu: MoreOptions\(Change art\) of $name" | Out-Null }
}

# Change art on the first Recent tile: the picker on its Square tab once its first page has come. Gives the line.
function Open-Picker([string] $Name = '') {
    Open-MoreOptions 'Change art'
    Press A 'ArtPicker: Square: Page 1(?: of \d+)?: Automatic; .+' | Out-Null
    $line = Await 'ArtPicker: Square: Page 1 of \d+: Automatic; in use: [^;]+'
    if ($Name) { Picture $Name }
    $line
}

# What is in use on the tab the picker shows: `Automatic`, `Store` or `Picture <id>`.
function In-Use { if ((Focus-Now) -match '; in use: ([^;]+)') { $Matches[1] } }

function Get-StandInLines([string] $Pattern, [int] $After = 0) {
    @(Get-Content $storeLog -ErrorAction SilentlyContinue | Select-Object -Skip $After | Where-Object { $_ -match $Pattern })
}

# The pages of a list the picker asked the stand-in for since line $After, in the order asked: the `page=` of each
# request with the picker's limit=50 (the lookup asks with none).
function Pages-Asked([string] $Path, [int] $After) {
    @(Get-StandInLines "serve-art: /$Path.*limit=50&page=\d+" $After | ForEach-Object { if ($_ -match 'page=(\d+)') { [int] $Matches[1] } })
}

# Opens the picker and checks both choices are in use, then B back to the tile.
function Check-Kept([string] $When) {
    Open-Picker "$When-square" | Out-Null
    $square = In-Use
    Say "$When`: Square in use: $square"
    if ($square -ne "Picture $script:squareId") { $script:failures += "$When the Square tab should have Picture $script:squareId in use, not '$square'" }
    Press RightShoulder 'ArtPicker: Cover: .+' | Out-Null
    Press RightShoulder 'ArtPicker: Background: .+' | Out-Null
    $background = In-Use
    Say "$When`: Background in use: $background"
    if ($background -ne "Picture $script:backgroundId") { $script:failures += "$When the Background tab should have Picture $script:backgroundId in use, not '$background'" }
    Press B 'Recent\(0\)' | Out-Null
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $steam = Initialize-FakeSteam $steamRoot
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }

    $standin = Join-Path $PSScriptRoot 'standin.exe'
    if (-not (Test-Path $standin)) { throw "No standin.exe in $PSScriptRoot" }
    $server = Start-Process $standin -ArgumentList @('serve-art', '--port', "$port", '--key', $key, '--candidates', '60') -PassThru
    $listening = Wait-Until {
        Get-Content $storeLog -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "serve-art: listening on http://127\.0\.0\.1:$port" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the stand-in SteamGridDB never said it was listening' }
    Say "stand-in: $listening"

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $dashboard = Start-Dashboard @('--log-focus', '--steam', "`"$steamRoot`"", '--data-dir', "`"$data`"",
                                   '--art-service', "http://127.0.0.1:$port",
                                   '--store-art-service', "http://127.0.0.1:$port/stores")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Wait-DashboardLog $lookupLine 120 | Out-Null
    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    # A new start's log has no focus line yet: Home starts on the profile, which it does not log. Down to the Recent
    # row says where focus is before any walking.
    Press Down 'Recent\(\d+\)' | Out-Null

    # --- 1. The key.
    Open-Art
    Press A 'TextEntry: Q; "\|" upper letters' | Out-Null
    foreach ($n in 1..$key.Length) { Press A "TextEntry: Q; `"$('\*' * $n)\|`" .+" | Out-Null }
    $mark = Get-DashboardLogLength
    Press Start 'Settings: Art: Type' | Out-Null
    $done = Wait-DashboardLog $lookupLine 180 $mark
    Say "lookup with the key: $(if ($done) { $done } else { 'no art: line in 180 s' })"
    Leave-Settings
    Press Down 'Recent\(\d+\)' | Out-Null

    # --- 2. Square: to the last page, and a candidate there.
    $standinMark = @(Get-Content $storeLog -ErrorAction SilentlyContinue).Count
    $first = Open-Picker '1-square-first-page'
    $pages = if ($first -match 'Page 1 of (\d+)') { [int] $Matches[1] } else { 0 }
    Say "the Square tab has $pages pages"
    if ($pages -lt 2) { $failures += "60 squares should be more than one page, and the picker says '$first'" }
    Walk Right 'ArtPicker: Square: Page (\d+) of \1: Picture \d+; in use: .+' 80
    Picture '2-square-last-page'
    $squareId = if ((Focus-Now) -match 'Picture (\d+);') { $Matches[1] } else { throw 'no picture focused' }
    $mark = Get-DashboardLogLength
    Press A 'Recent\(0\)' | Out-Null
    $kept = Wait-DashboardLog "^art: picker: square is SteamGridDB picture $squareId$" 30 $mark
    Say "square: $(if ($kept) { $kept } else { 'no line' })"
    if (-not $kept) { $failures += "choosing square picture $squareId should have been logged as kept" }
    $asked = Pages-Asked 'grids/game/.*dimensions=1024x1024,512x512' $standinMark
    Say "the square list's pages asked, in order: $($asked -join ', ')"
    if (($asked -join ',') -ne '0,1') { $failures += "the square list's pages should have been asked 0 then 1, once each and nothing past: $($asked -join ', ')" }
    Picture '3-home-square-chosen'

    # --- 3. Background: a candidate on its third page.
    $standinMark = @(Get-Content $storeLog -ErrorAction SilentlyContinue).Count
    Open-Picker | Out-Null
    Press RightShoulder 'ArtPicker: Cover: .+' | Out-Null
    Press RightShoulder 'ArtPicker: Background: Page 1(?: of \d+)?: Automatic; .+' | Out-Null
    Await 'ArtPicker: Background: Page 1 of \d+: Automatic; in use: [^;]+' | Out-Null
    Walk Right 'ArtPicker: Background: Page 3 of \d+: Picture \d+; in use: .+' 30
    Picture '4-background-third-page'
    $backgroundId = if ((Focus-Now) -match 'Picture (\d+);') { $Matches[1] } else { throw 'no picture focused' }
    $mark = Get-DashboardLogLength
    Press A 'Recent\(0\)' | Out-Null
    $kept = Wait-DashboardLog "^art: picker: background is SteamGridDB picture $backgroundId$" 30 $mark
    Say "background: $(if ($kept) { $kept } else { 'no line' })"
    if (-not $kept) { $failures += "choosing background picture $backgroundId should have been logged as kept" }
    $asked = Pages-Asked 'heroes/game/' $standinMark
    Say "the hero list's pages asked, in order: $($asked -join ', ')"
    if (($asked -join ',') -ne '0') { $failures += "three pages of twelve are within the hero list's first 50: page 0 alone, once: $($asked -join ', ')" }
    Picture '5-home-background-chosen'

    # --- 4. Both in use.
    Check-Kept '6-chosen'

    # --- 5. Refresh all art and details, then one game's Refresh art: neither replaces them.
    Open-Art
    Press Down 'Settings: Art: Clear downloaded art(; .+)?' | Out-Null
    Press Right 'Settings: Art: Refresh all art and details(; .+)?' | Out-Null
    Press A 'Settings: Art: Prompt\(Refresh\) of Refresh all art and details' | Out-Null
    $mark = Get-DashboardLogLength
    Press A $refreshing | Out-Null
    $over = Wait-DashboardLog '^art: refresh: done, (\d+) pictures downloaded$' 240 $mark
    Say "refreshed: $(if ($over) { $over } else { 'no line in 240 s' })"
    if (-not $over) { $failures += 'the refresh never said it was done' }
    Leave-Settings
    Press Down 'Recent\(\d+\)' | Out-Null
    Check-Kept '7-after-refresh-all'

    Open-MoreOptions 'Refresh art'
    $mark = Get-DashboardLogLength
    Press A 'Recent\(0\)' | Out-Null
    $done = Wait-DashboardLog $lookupLine 120 $mark
    Say "one game's art fetched again: $(if ($done) { $done } else { 'no line' })"
    Check-Kept '8-after-refresh-art'

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
