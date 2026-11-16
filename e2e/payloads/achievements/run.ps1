# Achievements (#140): Steam's, from the fake Steam's own files (`appcache/stats`, written by `standin make-steam`:
# crates/source-steam/src/fixture.rs, ACHIEVEMENTS), how rare each is from a stand-in of Steam's global percentages
# (`standin serve-art`, `--store-art-service <url>/stores`: nothing here asks a real service), and a new unlock seen
# while the game runs; then the screens that show them (#141): judged by the Dashboard's log, with pictures to look at.
#
# Ashfall Courier (9100010), the Recent row's first game, has six achievements, two unlocked: as the Dashboard starts
# it says `achievements: 9100010 2 of 6, score 20` (no rarity yet: each 10), asks the stand-in, says
# `achievements: rarity of 9100010: 6 of 6 known` and `achievements: 9100010 2 of 6, score 25` (82.5% is 10, 31%
# 15). The game is launched from its Game hub; once it counts as booted the Dashboard says
# `achievements: watching 9100010 (Ashfall Courier)`. `standin unlock-achievement` then unlocks No Detours (7.4% of
# players: Rare, Score 30) as Steam writes its stats file while a game runs, and within 5 s the Dashboard says
# `achievements: 9100010 unlocked ACH_NO_DETOURS "No Detours": score 30, 7.4% of players, Rare` and
# `achievements: 9100010 3 of 6, score 55`.
#
# The screens (#141, docs/spec/achievements-page.md), once the game has ended and Home is back on its tile: A opens
# the game's hub again, Right and Up put focus on Your game stats (`focus: GameHub: 9100010 Hub Stats`), whose
# Achievements row shows; A opens the Achievements page in the hub's place, the Dashboard reads the game's list
# (`achievements: the page lists 6 (3 unlocked, <i> with icons)`) and focus is on the newest unlock,
# `focus: Achievements: 9100010 ACH_NO_DETOURS; 3 of 6 unlocked`. Right is Ash Runner; Down twice is the last row's
# secret one, Perfect Ledger (`... ACH_PERFECT_LEDGER; ...`); A there does nothing. B is the hub on its card again,
# B Home on the tile. Up is the profile and A on it the Guide on Profile & system: under Switch profile the totals and
# the three last unlocks are entries of their own, named in the Guide's line newest first
# (`guide: ProfileAndSystem Entry(2) Achievement "No Detours"`, then Ash Runner and First Delivery), and Down six
# times reaches Power (`guide: ProfileAndSystem Entry(6) Power`); A on an unlock does nothing. The totals' line,
# "55 G   3 achievements", is not in the log: the picture `07-guide-achievements` is for a person to read.
# Passes when every line comes in time and the Dashboard stays up.
# Safe mode: every fake under the payload's folder, nothing registered.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$port = 47812
$appid = '9100010'
$storeLog = Join-Path $env:E2E_OUT 'standin.log'
$server = $null

function Expect([string] $Pattern, [double] $Seconds, [string] $What, [int] $After = 0) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    Say "$What`: $(if ($line) { $line } else { "no line in $Seconds s" })"
    if (-not $line) { $script:failures += "$What`: no line matching '$Pattern' in $Seconds s" }
    $line
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $steamRoot = Get-FakeRoot 'steam'
    $library = Initialize-FakeSteam $steamRoot
    $game = @($library.games | Where-Object source_id -eq $appid)[0]
    if (-not $game -or -not $game.installed) { throw "The fake Steam has no installed game $appid" }

    $standin = Join-Path $PSScriptRoot 'standin.exe'
    $server = Start-Process $standin -ArgumentList @('serve-art', '--port', "$port") -PassThru
    $listening = Wait-Until {
        Get-Content $storeLog -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "serve-art: listening on http://127\.0\.0\.1:$port" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the stand-in never said it was listening' }

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--store-art-service', "http://127.0.0.1:$port/stores")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }

    Expect "^achievements: $appid 2 of 6, score 20`$" 60 'the read' | Out-Null
    Expect '^achievements: 1 games, 2 of 6 unlocked, score 20, read in \d+ ms$' 10 'the totals' | Out-Null
    Expect "^achievements: rarity of $appid`: 6 of 6 known`$" 30 'the rarity' | Out-Null
    Expect "^achievements: $appid 2 of 6, score 25`$" 10 'the Score by rarity' | Out-Null
    $asked = Get-Content $storeLog -ErrorAction SilentlyContinue | Where-Object { $_ -match 'GetGlobalAchievementPercentagesForApp' }
    Say "asked of the stand-in: $(@($asked).Count) percentages requests"

    Wait-Greeting | Out-Null
    # Home starts on the profile: Down is the Recent row's first tile, Ashfall Courier; A its Game hub; A Play.
    Push-Button Down
    if (-not (Wait-DashboardLog '^focus: Recent\(0\)$' 10)) { throw 'Down did not reach the Recent row' }
    Push-Button A
    if (-not (Wait-DashboardLog "^focus: GameHub: $appid Hub Play`$" 10)) { throw "A did not open $($game.name)'s Game hub" }
    Start-Sleep -Milliseconds 500
    Push-Button A
    if (-not (Wait-DashboardLog 'the game has booted' 60)) { throw "$($game.name) never counted as booted" }
    Expect "^achievements: watching $appid \(Ashfall Courier\)`$" 10 'watching the game' | Out-Null
    # The watch reads the files once as they are before anything is new.
    Start-Sleep -Seconds 2

    $mark = Get-DashboardLogLength
    $unlocked = Start-Process $standin -ArgumentList @('unlock-achievement', "`"$steamRoot`"", 'ACH_NO_DETOURS') -Wait -PassThru
    if ($unlocked.ExitCode -ne 0) { throw "standin unlock-achievement exited with $($unlocked.ExitCode)" }
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $told = Expect "^achievements: $appid unlocked ACH_NO_DETOURS `"No Detours`": score 30, 7\.4% of players, Rare`$" 5 'the unlock' $mark
    if ($told) { Say "known $([int] $watch.ElapsedMilliseconds) ms after the stats file was written" }
    Expect "^achievements: $appid 3 of 6, score 55`$" 5 'the Score after it' $mark | Out-Null
    # Told of once.
    Start-Sleep -Seconds 3
    $again = @(Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match 'unlocked ACH_NO_DETOURS' }).Count
    if ($again -ne 1) { $failures += "the unlock was said $again times" }

    Stop-StandInGame
    if (-not (Wait-DashboardLog 'the game has ended, Home back' 20)) { throw 'Home never came back after the game' }

    # The screens: the hub's row, the Achievements page, the Guide's Profile & system; each step judged by the line
    # its press brings, each picture for a person to look at.
    function Step([string] $Button, [string] $Pattern, [string] $What, [string] $Picture = '') {
        $mark = Get-DashboardLogLength
        Push-Button $Button
        $line = Expect $Pattern 10 $What $mark
        if ($Picture) { Start-Sleep -Milliseconds 800; Save-Screen $Picture }
        $line
    }
    Start-Sleep -Seconds 2
    Push-Button A
    if (-not (Wait-DashboardLog "^focus: GameHub: $appid Hub Play`$" 10)) { throw "A did not open $($game.name)'s Game hub again" }
    Start-Sleep -Milliseconds 500
    Step Right "^focus: GameHub: $appid Hub Manage`$" 'Manage' | Out-Null
    Step Up "^focus: GameHub: $appid Hub Stats`$" 'Your game stats, with its Achievements row' '01-hub-stats' | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect '^achievements: the page lists 6 \(3 unlocked, \d+ with icons\)$' 10 'the list read' $mark | Out-Null
    Expect "^focus: Achievements: $appid ACH_NO_DETOURS; 3 of 6 unlocked`$" 10 'the Achievements page' $mark | Out-Null
    Start-Sleep -Milliseconds 800
    Save-Screen '02-achievements-page'
    Step Right "^focus: Achievements: $appid ACH_ASH_RUNNER; 3 of 6 unlocked`$" 'Right: Ash Runner' | Out-Null
    Step Down "^focus: Achievements: $appid ACH_NIGHT_ROUTE; 3 of 6 unlocked`$" 'Down: Night Route' | Out-Null
    Step Down "^focus: Achievements: $appid ACH_PERFECT_LEDGER; 3 of 6 unlocked`$" 'Down: the secret one' '03-achievements-secret' | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Start-Sleep -Seconds 1
    $moved = @(Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^focus: ' })
    if ($moved.Count -gt 0) { $failures += "A on an achievement did something: $($moved -join '; ')" }
    Step B "^focus: GameHub: $appid Hub Stats`$" 'B: the hub on its card' '04-hub-again' | Out-Null
    Step B '^focus: Recent\(0\)$' 'B: Home on the tile' '05-home-score' | Out-Null
    Step Up '^focus: TopBar\(Profile\)$' 'Up: the profile' | Out-Null
    Step A '^guide: ProfileAndSystem Entry\(0\) Profile$' 'A: the Guide on Profile & system' '06-guide' | Out-Null
    # The entries by name: the last unlocks, newest first, between Switch profile and Settings.
    $names = @('SwitchProfile', 'Achievement "No Detours"', 'Achievement "Ash Runner"', 'Achievement "First Delivery"', 'Settings', 'Power')
    for ($entry = 1; $entry -le 6; $entry++) {
        $picture = if ($entry -eq 2) { '07-guide-achievements' } else { '' }
        $name = [regex]::Escape($names[$entry - 1])
        Step Down "^guide: ProfileAndSystem Entry\($entry\) $name`$" "Down: $($names[$entry - 1])" $picture | Out-Null
    }
    # A on an unlock does nothing: up to the newest, A, and the Guide stays on it.
    for ($entry = 5; $entry -ge 2; $entry--) { Step Up "^guide: ProfileAndSystem Entry\($entry\) " "Up: entry $entry" | Out-Null }
    $mark = Get-DashboardLogLength
    Push-Button A
    Start-Sleep -Seconds 1
    $moved = @(Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^(guide|focus): ' })
    if ($moved.Count -gt 0) { $failures += "A on an unlock in the Guide did something: $($moved -join '; ')" }
    Push-Button B
    Wait-DashboardLog '^guide: closed$' 10 | Out-Null

    if ($script:dashboard.HasExited) { $failures += "the Dashboard exited with $($script:dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
