# Art for games a store's files give none for (#80, #110): a fake Xbox drive and a fake Epic Games Launcher, and no
# Steam (an empty folder), so Home's Recent row is their games. A stand-in on 127.0.0.1 (`standin serve-art`) is both
# SteamGridDB (`--art-service`) and the stores' own art online (`--store-art-service <url>/stores`: the Microsoft
# Store's catalogue among them), so nothing here asks a real service.
#
# With no key: the stores' own art is asked for, and SteamGridDB never. Lumen Hollow, an Xbox game whose package has
# only small icons and whose MicrosoftGame.config names a product in the Store, gets the Store's box art as its square:
# its Recent tile is the stand-in Store's magenta. A key is typed in through Settings > Art on the Console keyboard with
# the pad; SteamGridDB is asked for what the stores have not: an Epic game's square (Epic has none online) is the
# stand-in SteamGridDB's yellow on its Recent tile, and its Game hub's cover card, its name on the plain tile before,
# SteamGridDB's orange cover. Clear downloaded art, after its question, takes it away again: the card is the plain tile
# once more.
#
# Judged by the Dashboard's log: the focus log after each press (`focus: Recent(<i>)`, `focus: Settings: Art: Type`,
# `focus: TextEntry: Q; "****|" lower letters`, `focus: GameHub: <id> Hub Play`), `settings: art.steamgriddb_key set`
# as the key is kept, the line each lookup ends with (`art: <n> looked up, <n> found, <n> none, in <s> s; pictures:
# <n> store files, <n> cache, <n> store online, <n> SteamGridDB`), and `art: cleared <n> pictures`; by serve-art's own
# lines in standin.log (a line a request, `serve-art: <path> (no key|the key)`); and by pixels: of the focused Recent
# tile (its top-left at x 105 + 203.87 i, y 533, 272.94 square, for `Recent(<i>)`: crates/screens/src/home/recent.rs),
# and of the cover card (x 80..455, y 270.4..770.4 of the canvas: crates/screens/src/game_hub/cards.rs). The key is
# never in dashboard.log. Passes when every press gives the line expected of it, the Recent row shows the Store's square
# with no key and SteamGridDB's with it, the card turns orange with the key and back without the art, and the
# Dashboard stays up. Safe mode: every fake under the payload's folder, nothing registered.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
# C:\e2e\<store> and folders under %TEMP% on the VM; under the payload's own folder in safe mode (the TV PC).
$epicRoot = Get-FakeRoot 'epic'
$xboxRoot = Get-FakeRoot 'xbox'
$noSteam = Get-TestFolder 'art-test-no-steam'
$data = Get-TestFolder 'dashboard-art-test'
$port = 47811
# Text entry opens on Q, upper case for the first letter: four presses of A type this.
$key = 'Qqqq'
# The stand-ins' colours (crates/standin/src/main.rs, ART_COLOURS and STORE_ART_COLOURS), and where on the cover card
# to look.
$orange = @(0xF0, 0x80, 0x10)
$yellow = @(0xE0, 0xD0, 0x20)
$magenta = @(0xD0, 0x20, 0x90)
$cardX = 267
$cardY = 330
# The rows' scale and a focused Recent tile's place (crates/screens/src/home/recent.rs, theme.rs).
$rowScale = 1700 / 1526
$storeLog = Join-Path $env:E2E_OUT 'standin.log'
# Named, so that the optional failures are no group of their own to count past.
$lookupLine = '^art: \d+ looked up, \d+ found, \d+ none(?:, (?<failed>\d+) failed)?, in [\d.]+ s; pictures: \d+ store files, \d+ cache, (?<store>\d+) store online, (?<grid>\d+) SteamGridDB$'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
# Always a string: before the first focus line (Home starts on the profile and says nothing) it is '', which
# -match and -notmatch compare as a string; nothing at all would make them filter an empty list, and be false both.
function Focus-Now { [string] (Focus-Lines | Select-Object -Last 1) }

# Two taps closer than this can be seen as one held press on a VM whose frames take most of a second.
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

function Walk-ToKey([string] $Button, [string] $Back, [string] $Key, [int] $Most = 12) {
    $pattern = "^focus: Search: Key\($([regex]::Escape($Key))\);"
    foreach ($way in @($Button, $Back)) {
        foreach ($press in 1..$Most) {
            if ((Focus-Now) -match $pattern) { break }
            Tap $way
            Start-Sleep -Milliseconds 650
        }
        if ((Focus-Now) -match $pattern) { break }
    }
    if ((Focus-Now) -notmatch $pattern) { throw "never reached the key $Key on the Console keyboard: '$(Focus-Now)'" }
}

# From Home, focus on the Recent row or the top bar, to Settings > Art, focus on Type.
function Open-Art {
    Walk Up 'TopBar\(.+\)' 4
    Walk Right 'TopBar\(Settings\)' 6
    Press A 'Settings: Rail: Theme settings' | Out-Null
    Press Right 'Settings: Theme settings: Personalisation' | Out-Null
    Press Down 'Settings: Theme settings: Guide' | Out-Null
    Press Down 'Settings: Theme settings: Games & apps' | Out-Null
    Press Down 'Settings: Theme settings: Art' | Out-Null
    Press A 'Settings: Art: Type' | Out-Null
}

# Out of Settings, a level at a time, to Home's top bar.
function Leave-Settings {
    Press B 'Settings: Theme settings: Art' | Out-Null
    Press B 'Settings: Rail: Theme settings' | Out-Null
    Press B 'TopBar\(Settings\)' | Out-Null
}

# The Recent row's tiles, focused one by one from the first to the last, as the log says: each tile's colour near its
# top middle, above the title band, once its growth has settled. Back on the top bar after.
function Look-AtRecent([string] $Name) {
    if ((Focus-Now) -notmatch '^focus: Recent\(\d+\)$') { Press Down 'Recent\(\d+\)' | Out-Null }
    Walk Left 'Recent\(0\)' 10
    $seen = @()
    foreach ($tile in 0..8) {
        if ($tile -gt 0) {
            $before = @(Focus-Lines).Count
            Tap Right
            if (-not (Wait-Focus $before "Recent\($tile\)" 4)) { break }
        }
        Start-Sleep -Milliseconds 900
        $x = 105 + $tile * 183 * $rowScale + 245 * $rowScale / 2
        $y = 533 + 60
        $pixel = Get-DesignPixel $x $y
        Say "$Name`: Recent($tile) at ($([int] $x), $y) is $(Format-Colour $pixel)"
        $seen += $pixel
    }
    Save-Screen $Name
    Walk Up 'TopBar\(.+\)' 4
    $seen
}

function Test-Seen($Seen, $Colour) { @($Seen | Where-Object { Test-Colour $_ $Colour 40 }).Count -gt 0 }

# From Home's top bar, the Game hub of $Game, found through Search by its name's first letter (as stores-epic-gog
# does); then B back to Home. Gives the cover card's pixel while the hub is up, after $Wait seconds for art to come.
function Look-AtHub($Game, [string] $Name, [double] $Wait = 3) {
    $title = $Game.name
    $letter = $title.Substring(0, 1).ToUpper()
    Press Y 'Search: Key\(Q\); .*' | Out-Null
    if ('ASDFGHJKL'.Contains($letter)) { Walk-ToKey Down Up 'A' }
    elseif ('ZXCVBNM'.Contains($letter)) { Walk-ToKey Down Up 'Z' }
    Walk-ToKey Right Left $letter
    Press A "Search: Key\($letter\); `"$letter\|`" .*" | Out-Null
    Press B "Search: \w+\(0\) .+; `"$letter`" away.*" | Out-Null
    $named = "^focus: Search: \w+\(\d+\) $([regex]::Escape($title));"
    foreach ($press in 1..12) {
        if ((Focus-Now) -match $named) { break }
        Tap Right
        Start-Sleep -Milliseconds 650
    }
    if ((Focus-Now) -notmatch $named) { throw "Search for '$letter' never put focus on $title`: $(Focus-Now)" }
    $id = [regex]::Escape($Game.source_id)
    $hub = Press A "GameHub: $id Hub Play"
    if (-not $hub) { throw "A did not open $title's hub: $(Focus-Now)" }
    Start-Sleep -Seconds $Wait
    Save-Screen $Name
    $pixel = Get-DesignPixel $cardX $cardY
    Say "$Name`: the cover card at ($cardX, $cardY) is $(Format-Colour $pixel)"
    # Back to Home: the hub, Search's results, Search.
    foreach ($press in 1..4) {
        if ((Focus-Now) -match '^focus: (Recent|TopBar|Wide|Group)') { break }
        Tap B
        Start-Sleep -Milliseconds 900
    }
    $pixel
}

function Get-StandInLines([string] $Pattern) {
    @(Get-Content $storeLog -ErrorAction SilentlyContinue | Where-Object { $_ -match $Pattern })
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $xbox = Initialize-FakeXbox $xboxRoot
    if (-not @($xbox.games | Where-Object { $_.name -eq 'Lumen Hollow' })) { throw 'the fake Xbox drive should have Lumen Hollow' }
    $epic = Initialize-FakeEpic $epicRoot
    $game = @($epic.games) | Select-Object -First 1
    if (-not $game) { throw 'the fake Epic should have a game' }
    foreach ($folder in @($data, $noSteam)) { if (Test-Path $folder) { Remove-Item $folder -Recurse -Force } }
    New-Item $noSteam -ItemType Directory -Force | Out-Null

    # The stand-in SteamGridDB and stores, until the test ends.
    $standin = Join-Path $PSScriptRoot 'standin.exe'
    if (-not (Test-Path $standin)) { throw "No standin.exe in $PSScriptRoot" }
    $server = Start-Process $standin -ArgumentList @('serve-art', '--port', "$port", '--key', $key) -PassThru
    $listening = Wait-Until {
        Get-Content $storeLog -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "serve-art: listening on http://127\.0\.0\.1:$port" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the stand-in SteamGridDB never said it was listening' }
    Say "stand-in: $listening"

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--epic', "`"$epicRoot`"", '--xbox', "`"$xboxRoot`"",
                                          '--steam', "`"$noSteam`"", '--data-dir', "`"$data`"",
                                          '--art-service', "http://127.0.0.1:$port",
                                          '--store-art-service', "http://127.0.0.1:$port/stores")
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"

    # --- With no key: the stores' own art online, and SteamGridDB never.
    $done = Wait-DashboardLog $lookupLine 120
    Say "lookup with no key: $(if ($done) { $done } else { 'no art: line in 120 s' })"
    if (-not $done) { $failures += 'the first lookup never ended' }
    elseif ($done -notmatch $lookupLine) { $failures += "the lookup's line is not as expected: $done" }
    elseif ([int] $Matches['grid'] -ne 0 -or [int] $Matches['store'] -eq 0 -or $Matches['failed']) {
        # Every address the fakes name is the stand-in's: a request that fails is a fault, not a store with none.
        $failures += "with no key the pictures should be the stores' alone, some, and none failed: $done"
    }
    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    $asked = Get-StandInLines 'serve-art: /(games|search|grids|heroes)/'
    if ($asked.Count) { $failures += "with no key SteamGridDB should be asked nothing, and had $($asked.Count) requests" }
    $catalogue = Get-StandInLines 'serve-art: /stores/microsoft/v7\.0/products\?bigIds=9ZZZMADEUP02&.* \(no key\)$'
    Say "the stand-in Store was asked $($catalogue.Count) times for Lumen Hollow's product"
    if (-not $catalogue.Count) { $failures += "the Microsoft Store's catalogue was never asked for Lumen Hollow (9ZZZMADEUP02)" }
    $row = Look-AtRecent '1-recent-no-key'
    if (-not (Test-Seen $row $magenta)) { $failures += "with no key a Recent tile should be the Store's magenta box art (Lumen Hollow's square)" }
    if (Test-Seen $row $yellow) { $failures += "with no key no Recent tile should be SteamGridDB's yellow square" }
    $before = Look-AtHub $game '2-hub-no-key' 2
    if (Test-Colour $before $orange 40) { $failures += "with no key the cover card should not be SteamGridDB's cover" }

    # --- The key, typed with the pad and kept with Menu: SteamGridDB is asked for what the stores had not.
    Open-Art
    Press A 'TextEntry: Q; "\|" upper letters' '3-key-entry' | Out-Null
    foreach ($n in 1..$key.Length) {
        Press A "TextEntry: Q; `"$('\*' * $n)\|`" .+" | Out-Null
    }
    Save-Screen '4-key-typed'
    $mark = Get-DashboardLogLength
    Press Start 'Settings: Art: Type' '5-key-kept' | Out-Null
    $set = Wait-DashboardLog '^settings: art\.steamgriddb_key set$' 10 $mark
    if (-not $set) { $failures += "keeping the key should have logged 'settings: art.steamgriddb_key set'" }
    $done = Wait-DashboardLog $lookupLine 180 $mark
    Say "lookup with the key: $(if ($done) { $done } else { 'no art: line in 180 s' })"
    if (-not $done) { $failures += 'the lookup with the key never ended' }
    elseif ($done -notmatch $lookupLine) { $failures += "the lookup's line is not as expected: $done" }
    elseif ([int] $Matches['grid'] -eq 0 -or $Matches['failed']) { $failures += "SteamGridDB should have given pictures, none failing: $done" }
    Leave-Settings

    # --- The Recent row: the Store's square for Lumen Hollow still, and SteamGridDB's for an Epic game.
    $row = Look-AtRecent '6-recent-with-key'
    if (-not (Test-Seen $row $magenta)) { $failures += "with the key Lumen Hollow's tile should still be the Store's magenta box art" }
    if (-not (Test-Seen $row $yellow)) { $failures += "with the key an Epic game's tile should be SteamGridDB's yellow square" }

    # --- The Epic game's cover card is the stand-in's orange cover.
    $after = Look-AtHub $game '7-hub-with-art'
    if (-not (Test-Colour $after $orange 40)) { $failures += "with the key the cover card should be orange, and is $(Format-Colour $after)" }
    $keyed = Get-StandInLines 'serve-art: /(games|search|grids|heroes)/.* \(the key\)$'
    $squares = Get-StandInLines 'serve-art: /grids/game/.*dimensions=1024x1024,512x512.* \(the key\)$'
    $pictures = Get-StandInLines 'serve-art: /(pictures|stores/(steam-cdn|ms-images|gog-images))/'
    Say "SteamGridDB was asked $($keyed.Count) times with the key ($($squares.Count) for squares), and $($pictures.Count) pictures were fetched"
    if (-not $keyed.Count) { $failures += 'the stand-in SteamGridDB was never asked with the key' }
    if (-not $squares.Count) { $failures += 'SteamGridDB was never asked for a square' }
    if (@($pictures | Where-Object { $_ -notmatch '\(no key\)$' }).Count) { $failures += 'a picture was asked for with the key on it' }
    if (@(Get-StandInLines 'serve-art: /stores/.*\((the|another) key\)$').Count) { $failures += 'a store was sent the key' }

    # --- Clear downloaded art, after its question: the card is the plain tile again.
    Open-Art
    Press Down 'Settings: Art: Clear downloaded art' | Out-Null
    Press A 'Settings: Art: Prompt\(Clear\) of Clear downloaded art' '8-clear-prompt' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Settings: Art: Clear downloaded art' | Out-Null
    $cleared = Wait-DashboardLog '^art: cleared \d+ pictures$' 10 $mark
    Say "cleared: $(if ($cleared) { $cleared } else { 'no line' })"
    if (-not $cleared -or $cleared -match 'cleared 0 ') { $failures += "Clear downloaded art should have cleared pictures: '$cleared'" }
    Press Up 'Settings: Art: Type' | Out-Null
    Leave-Settings
    $again = Look-AtHub $game '9-hub-cleared' 2
    if (Test-Colour $again $orange 40) { $failures += 'after clearing the art the cover card should not be orange' }

    # The key was never written to the log.
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
