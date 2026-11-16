# The Achievement unlocked Toast (#142, docs/spec/achievement-toast.md), over the stand-in game, on the fake Steam's
# Ashfall Courier (9100010; crates/source-steam/src/fixture.rs, ACHIEVEMENTS), its rarity from the stand-in of
# Steam's global percentages (`standin serve-art`): nothing here asks a real service.
#
#   1. The game is launched from its Game hub; once it counts as booted the Dashboard watches its stats file.
#   2. A common unlock: `standin unlock-achievement` unlocks Night Route (18.2% of players, Score 20). The Dashboard
#      says `achievements: 9100010 unlocked ACH_NIGHT_ROUTE "Night Route": score 20, 18.2% of players`, then a second
#      later `toasts: shown "Achievement unlocked" (Achievement) "Night Route", score 20, ...` with
#      `audio: played Notification.wav`. The pill is green over the game, which keeps the foreground; pictures of its
#      first page and its description's. It goes 8.5 s after it showed, and is gone 1.12 s later.
#   3. A Rare unlock: No Detours (7.4%, Score 30): `toasts: shown "Rare Achievement unlocked" (RareAchievement)
#      "No Detours", score 30, 7.4%, ...` and `audio: played RareAchievement.wav`; the pill is blue. A tap of the
#      Guide button while it shows opens the Guide on Profile & system, where the newest unlock listed is now No
#      Detours (`guide: ProfileAndSystem Entry(2) Achievement "No Detours"`). B closes the Guide.
#   4. The Notifications Flyout: the Guide again, down to the Utilities, A on Notifications. Both are listed, newest
#      first (a picture); A on the second, Night Route's, closes the Guide onto Ashfall Courier's Achievements page
#      in front of the game (`focus: Achievements: 9100010 ACH_NO_DETOURS; 4 of 6 unlocked`: focus starts on the newest
#      unlock). B is Home.
#
# The sounds are tones `standin make-tones` writes (RareAchievement.wav among them); --log-sounds says each played.
# Passes when every line comes in time, the pill's colours are on the screen, and the Dashboard stays up.
# Safe mode: every fake under the payload's folder, nothing registered.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$port = 47813
$appid = '9100010'
$storeLog = Join-Path $env:E2E_OUT 'standin.log'
$server = $null
$dashboardTitle = 'Xbox Series Dashboard'
$stripTitle = 'Xbox Series Dashboard Toasts'
# The pill: 778 x 128 at x 571..1349, y 888..1016. These points are clear of its circle and its text: under its top
# edge in the middle, and in its right end. Its green (#107C10) and the rare one's blue (#0F4E7E).
$green = @(16, 124, 16)
$blue = @(15, 78, 126)
$pillPoints = @(
    @{ X = 960; Y = 893; What = 'the pill, under its top edge' }
    @{ X = 1320; Y = 960; What = "the pill's right end" }
)

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }

function Expect([string] $Pattern, [double] $Seconds, [string] $What, [int] $After = 0) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    Say "$What`: $(if ($line) { $line } else { "no line in $Seconds s" })"
    if (-not $line) { $script:failures += "$What`: no line matching '$Pattern' in $Seconds s" }
    $line
}

# The last line with this prefix the Dashboard logged, without the prefix.
function Get-LastLine([string] $Prefix) {
    $lines = @(Get-DashboardLog | Where-Object { $_ -match "^${Prefix}: " })
    if ($lines.Count) { $lines[-1] -replace "^${Prefix}: ", '' } else { '' }
}
function Get-GuideLine { $line = Get-LastLine 'guide'; if ($line) { $line } else { 'closed' } }

# One press, then the `guide:` line it must bring.
function Step([string] $Button, [string] $Guide, [string] $Name = '') {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $line = Expect "^guide: $Guide" 8 "$Button" $mark
    if ($Name) { Start-Sleep -Milliseconds 3000; Save-Screen $Name }
    [bool] $line
}

# One press while walking to somewhere named; a press the VM never showed the Dashboard is pressed again.
function Walk([string] $Button, [string] $Guide) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $line = Wait-DashboardLog "^guide: $Guide" 5 $mark
    Say "$Button`: $(if ($line) { $line } else { "note: no line of this press (the Guide is at: $(Get-GuideLine))" })"
}

function Move-ToUtilities {
    for ($i = 0; $i -lt 12; $i++) {
        if ((Get-GuideLine) -match '^Landing Utility\(0\)') { return }
        if ((Get-GuideLine) -notmatch '^Landing Entry\((\d+)\)') { throw "The Guide is not on its list: $(Get-GuideLine)" }
        Walk Down 'Landing (Entry|Utility)\(\d+\)'
    }
    throw "Focus never reached the Utilities: $(Get-GuideLine)"
}

function Move-ToItem([int] $Item) {
    for ($i = 0; $i -lt 8; $i++) {
        if ((Get-GuideLine) -notmatch 'flyout \w+ Item\((\d+)\)') { throw "The Flyout's focus is not on an entry: $(Get-GuideLine)" }
        $at = [int] $Matches[1]
        if ($at -eq $Item) { return }
        $next = if ($at -lt $Item) { $at + 1 } else { $at - 1 }
        Walk $(if ($at -lt $Item) { 'Down' } else { 'Up' }) "Landing Utility\(\d\), flyout \w+ Item\($next\)"
    }
    throw "Focus never reached entry $Item of the Flyout: $(Get-GuideLine)"
}

# The pill on the screen in this colour, at every point: waits for it up to $Seconds, then says what was seen.
function Test-Pill($Colour, [string] $What, [double] $Seconds = 4) {
    $seen = Wait-Until {
        foreach ($p in $pillPoints) { $p.Seen = Get-DesignPixel $p.X $p.Y }
        -not @($pillPoints | Where-Object { -not (Test-Colour $_.Seen $Colour 10) }).Count
    } $Seconds
    foreach ($p in $pillPoints) { Say "  $What at ($($p.X), $($p.Y)), $($p.What): $(Format-Colour $p.Seen)" }
    if (-not $seen) { $script:failures += "$What is not on the screen in $(Format-Colour $Colour)" }
}

# Unlocks one of the fake's achievements as Steam would while the game runs, and returns the Toast's shown line.
function Unlock([string] $Key, [string] $Unlocked, [string] $Score, [string] $Toast, [string] $Sound) {
    $mark = Get-DashboardLogLength
    $run = Start-Process $script:standin -ArgumentList @('unlock-achievement', "`"$script:steamRoot`"", $Key) -Wait -PassThru
    if ($run.ExitCode -ne 0) { throw "standin unlock-achievement $Key exited with $($run.ExitCode)" }
    Expect $Unlocked 5 "the unlock of $Key" $mark | Out-Null
    Expect $Score 5 "the Score after $Key" $mark | Out-Null
    $shown = Expect $Toast 10 "the Toast of $Key" $mark
    Expect "^audio: played $([regex]::Escape($Sound))`$" 5 "its sound" $mark | Out-Null
    if ((Get-Foreground).Title -eq $stripTitle) { $script:failures += 'the Toast strip took the foreground' }
    Say "foreground with the Toast up: $(Front)"
    $shown
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $script:steamRoot = Get-FakeRoot 'steam'
    $library = Initialize-FakeSteam $steamRoot
    $game = @($library.games | Where-Object source_id -eq $appid)[0]
    if (-not $game -or -not $game.installed) { throw "The fake Steam has no installed game $appid" }

    $script:standin = Join-Path $PSScriptRoot 'standin.exe'
    $server = Start-Process $standin -ArgumentList @('serve-art', '--port', "$port") -PassThru
    $listening = Wait-Until {
        Get-Content $storeLog -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "serve-art: listening on http://127\.0\.0\.1:$port" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the stand-in never said it was listening' }
    $media = Join-Path $env:E2E_OUT 'media'
    $toned = Start-Process $standin -ArgumentList 'make-tones', "`"$media`"" -Wait -PassThru
    if ($toned.ExitCode -ne 0) { throw "standin make-tones exited with $($toned.ExitCode)" }

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $arguments = @('--log-focus', '--log-sounds', '--media', "`"$media`"", '--store-art-service', "http://127.0.0.1:$port/stores")
    $script:dashboard = Start-Dashboard $arguments
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    # The rarity known before anything is unlocked: Night Route is common and No Detours Rare only once it is.
    Expect "^achievements: $appid 2 of 6, score 25`$" 60 'the Score by rarity' | Out-Null

    # ---- 1. The game.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    Push-Button Down
    if (-not (Wait-DashboardLog '^focus: Recent\(0\)$' 10)) { throw 'Down did not reach the Recent row' }
    Push-Button A
    if (-not (Wait-DashboardLog "^focus: GameHub: $appid Hub Play`$" 10)) { throw "A did not open $($game.name)'s Game hub" }
    Start-Sleep -Milliseconds 500
    Push-Button A
    if (-not (Wait-DashboardLog 'the game has booted' 60)) { throw "$($game.name) never counted as booted" }
    Expect "^achievements: watching $appid \(Ashfall Courier\)`$" 10 'watching the game' | Out-Null
    # The watch reads the files once as they are before anything is new; the greeting's Toast is long gone.
    Start-Sleep -Seconds 3
    Say "foreground over the game: $(Front)"

    # ---- 2. A common unlock: the green pill.
    $mark = Get-DashboardLogLength
    $shown = Unlock 'ACH_NIGHT_ROUTE' "^achievements: $appid unlocked ACH_NIGHT_ROUTE `"Night Route`": score 20, 18\.2% of players`$" `
        "^achievements: $appid 3 of 6, score 45`$" `
        '^toasts: shown "Achievement unlocked" \(Achievement\) "Night Route", score 20, 0 waiting at \d+ ms$' 'Notification.wav'
    # The first page is in 2.4 s after it showed.
    Start-Sleep -Milliseconds 2600
    Save-Screen '01-toast-over-game'
    Test-Pill $green 'the green pill'
    # The description's page, from 4.7 s.
    Start-Sleep -Milliseconds 2400
    Save-Screen '02-toast-description'
    $going = Expect '^toasts: going at (\d+) ms$' 10 'its going' $mark
    $gone = Expect '^toasts: gone at (\d+) ms$' 5 'gone' $mark
    if ($shown -match ' at (\d+) ms$') {
        $shownAt = [int] $Matches[1]
        if ($going -match ' at (\d+) ms$') {
            $for = [int] $Matches[1] - $shownAt
            Say "shown for $for ms"
            if ($for -lt 8300 -or $for -gt 9500) { $failures += "the pill showed for $for ms, not 8.5 s" }
        }
    }

    # ---- 3. A Rare unlock: the blue pill; the Guide button opens Profile & system on it.
    Start-Sleep -Seconds 1
    $mark = Get-DashboardLogLength
    Unlock 'ACH_NO_DETOURS' "^achievements: $appid unlocked ACH_NO_DETOURS `"No Detours`": score 30, 7\.4% of players, Rare`$" `
        "^achievements: $appid 4 of 6, score 75`$" `
        '^toasts: shown "Rare Achievement unlocked" \(RareAchievement\) "No Detours", score 30, 7\.4%, 0 waiting at \d+ ms$' `
        'RareAchievement.wav' | Out-Null
    Start-Sleep -Milliseconds 2600
    Save-Screen '03-rare-toast-over-game'
    Test-Pill $blue 'the blue pill'
    Step Guide 'ProfileAndSystem Entry\(0\) ' '04-guide-from-rare-toast' | Out-Null
    Expect '^toasts: going at \d+ ms$' 5 'the pill going as the Guide opened' $mark | Out-Null
    Step Down 'ProfileAndSystem Entry\(1\) ' | Out-Null
    Step Down 'ProfileAndSystem Entry\(2\) Achievement "No Detours"$' | Out-Null
    Step B 'closed$' | Out-Null
    Start-Sleep -Seconds 2

    # ---- 4. The Notifications Flyout: both listed; A on Night Route's opens the Achievements page.
    Step Guide 'Landing Entry\(0\)' | Out-Null
    Move-ToUtilities
    Step A 'Landing Utility\(0\), flyout Notifications Item\(0\)$' '05-notifications' | Out-Null
    Move-ToItem 1
    Save-Screen '06-notifications-night-route'
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect '^guide: closed$' 8 'A on the row closes the Guide' $mark | Out-Null
    Expect "^focus: Achievements: $appid ACH_NO_DETOURS; 4 of 6 unlocked`$" 10 'the Achievements page' $mark | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '07-achievements-page'
    Say "foreground on the page: $(Front)"
    if ((Get-Foreground).Title -ne $dashboardTitle) { $failures += "the Dashboard is not in front on the Achievements page: $(Front)" }
    $mark = Get-DashboardLogLength
    Push-Button B
    Expect '^focus: (?!Achievements)' 8 'B: Home' $mark | Out-Null

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
