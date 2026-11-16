# Art for games a store gives none for (#80): a fake Epic Games Launcher, whose games have no art, beside the fake
# Steam; a stand-in SteamGridDB on 127.0.0.1 (`standin serve-art`), which the Dashboard is pointed at with
# `--art-service`, so nothing here asks the real service. With no key nothing is looked up. A key is typed in through
# Settings > Art on the Console keyboard with the pad; the Dashboard looks the art up, and the cover card of an Epic
# game's Game hub, its name on the plain tile before, is the stand-in's orange cover. Clear downloaded art, after its
# question, takes it away again: the card is the plain tile once more.
#
# Judged by the Dashboard's log: the focus log after each press (`focus: Settings: Art: Type`, `focus: TextEntry: Q;
# "****|" lower letters`, `focus: GameHub: <id> Hub Play`), `settings: art.steamgriddb_key set` as the key is kept,
# `art: <n> looked up, <n> found, <n> none, in <s> s` as the lookup ends, and `art: cleared <n> pictures`; by
# serve-art's own lines in standin.log (a line a request, `serve-art: <path> (the key)`); and by a pixel of the cover
# card (x 80..455, y 270.4..770.4 of the canvas: crates/screens/src/game_hub/cards.rs), above where a name would be.
# The key is never in dashboard.log. Passes when every press gives the line expected of it, the card turns orange with
# the key and back without the art, and the Dashboard stays up.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
# C:\e2e\epic and a folder under %TEMP% on the VM; under the payload's own folder in safe mode (the TV PC).
$epicRoot = Get-FakeRoot 'epic'
$data = Get-TestFolder 'dashboard-art-test'
$port = 47811
# Text entry opens on Q, upper case for the first letter: four presses of A type this.
$key = 'Qqqq'
# The stand-in's cover colour (crates/standin/src/main.rs, ART_COLOURS), and where on the cover card to look.
$orange = @(0xF0, 0x80, 0x10)
$cardX = 267
$cardY = 330

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

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
    Press Down 'Settings: Theme settings: Games & apps' | Out-Null
    Press Right 'Settings: Theme settings: Art' | Out-Null
    Press A 'Settings: Art: Type' | Out-Null
}

# Out of Settings, a level at a time, to Home's top bar.
function Leave-Settings {
    Press B 'Settings: Theme settings: Art' | Out-Null
    Press B 'Settings: Rail: Theme settings' | Out-Null
    Press B 'TopBar\(Settings\)' | Out-Null
}

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

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    $epic = Initialize-FakeEpic $epicRoot
    $game = @($epic.games) | Select-Object -First 1
    if (-not $game) { throw 'the fake Epic should have a game' }
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }

    # The stand-in SteamGridDB, until the test ends.
    $standin = Join-Path $PSScriptRoot 'standin.exe'
    if (-not (Test-Path $standin)) { throw "No standin.exe in $PSScriptRoot" }
    $server = Start-Process $standin -ArgumentList @('serve-art', '--port', "$port", '--key', $key) -PassThru
    $listening = Wait-Until {
        Get-Content (Join-Path $env:E2E_OUT 'standin.log') -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "serve-art: listening on http://127\.0\.0\.1:$port" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the stand-in SteamGridDB never said it was listening' }
    Say "stand-in: $listening"

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--epic', "`"$epicRoot`"", '--data-dir', "`"$data`"",
                                          '--art-service', "http://127.0.0.1:$port")
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }

    # --- With no key, nothing is asked, and the card is the plain tile with the game's name.
    $before = Look-AtHub $game '1-hub-no-key' 2
    if (Test-Colour $before $orange 40) { $failures += "with no key the cover card should not be the stand-in's cover" }
    $asked = @(Get-Content (Join-Path $env:E2E_OUT 'standin.log') | Where-Object { $_ -match '^.*serve-art: /' })
    if ($asked.Count) { $failures += "with no key nothing should be asked, and the stand-in had $($asked.Count) requests" }
    if (Get-DashboardLog | Where-Object { $_ -match '^art: \d+ looked up' }) { $failures += 'with no key there should be no lookup' }

    # --- The key, typed with the pad and kept with Menu: the lookup runs.
    Open-Art
    Press A 'TextEntry: Q; "\|" upper letters' '2-key-entry' | Out-Null
    foreach ($n in 1..$key.Length) {
        Press A "TextEntry: Q; `"$('\*' * $n)\|`" .+" | Out-Null
    }
    Save-Screen '3-key-typed'
    $mark = Get-DashboardLogLength
    Press Start 'Settings: Art: Type' '4-key-kept' | Out-Null
    $set = Wait-DashboardLog '^settings: art\.steamgriddb_key set$' 10 $mark
    if (-not $set) { $failures += "keeping the key should have logged 'settings: art.steamgriddb_key set'" }
    $done = Wait-DashboardLog '^art: \d+ looked up, \d+ found, \d+ none(, \d+ failed)?, in [\d.]+ s$' 180 $mark
    Say "lookup: $(if ($done) { $done } else { 'no art: line in 180 s' })"
    if (-not $done) { $failures += 'the lookup never ended' }
    elseif ($done -match '^art: \d+ looked up, 0 found') { $failures += "the lookup found nothing: $done" }
    Leave-Settings

    # --- The Epic game's cover card is the stand-in's orange cover.
    $after = Look-AtHub $game '5-hub-with-art'
    if (-not (Test-Colour $after $orange 40)) { $failures += "with the key the cover card should be orange, and is $(Format-Colour $after)" }
    $keyed = @(Get-Content (Join-Path $env:E2E_OUT 'standin.log') | Where-Object { $_ -match 'serve-art: /(games|search|grids|heroes)/.* \(the key\)$' })
    $pictures = @(Get-Content (Join-Path $env:E2E_OUT 'standin.log') | Where-Object { $_ -match 'serve-art: /pictures/' })
    Say "the stand-in was asked $($keyed.Count) times with the key, and for $($pictures.Count) pictures"
    if (-not $keyed.Count) { $failures += 'the stand-in was never asked with the key' }
    if (@($pictures | Where-Object { $_ -notmatch '\(no key\)$' }).Count) { $failures += 'a picture was asked for with the key on it' }

    # --- Clear downloaded art, after its question: the card is the plain tile again.
    Open-Art
    Press Down 'Settings: Art: Clear downloaded art' | Out-Null
    Press A 'Settings: Art: Prompt\(Clear\) of Clear downloaded art' '6-clear-prompt' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Settings: Art: Clear downloaded art' | Out-Null
    $cleared = Wait-DashboardLog '^art: cleared \d+ pictures$' 10 $mark
    Say "cleared: $(if ($cleared) { $cleared } else { 'no line' })"
    if (-not $cleared -or $cleared -match 'cleared 0 ') { $failures += "Clear downloaded art should have cleared pictures: '$cleared'" }
    Press Up 'Settings: Art: Type' | Out-Null
    Leave-Settings
    $again = Look-AtHub $game '7-hub-cleared' 2
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
