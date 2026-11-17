# Steam's install wizard past its licence agreements and its room check (#230), walked with the virtual pad on a fake
# Steam library through the stand-in client's CEF debugger, as queue-steer is (`source_steam::debugger`). Nothing is
# ever downloaded but the fake's own files.
#
# The stand-in's wizard is set up with `standin steam-wizard`: Saltwind Rally (9100060) shows two licence agreements
# (eInstallState 8), each a page of the stand-in's own with a head, scripts, a nav and a footer that must not show,
# and is marked accepted only by `MarkEulaAccepted`; later the room its install manager says there is is made too
# small for Quiet Orbit (9100040).
#
# What it walks, stopping at the first thing that is not as it should be:
# - Decline. Install Saltwind Rally from its Tile menu: `steam client: install 9100060 ...: eInstallState 8
#   (ShowEULAs)`, and the Dashboard's own prompt over Home, focus on Decline (`focus: Agreement: Decline of Saltwind
#   Rally; agreement 1 of 2, page 1 of <n>`). Down and RB scroll it (its page moves on), Left is Accept, Right back on
#   Decline, and A on it declines: the wizard cancelled (`steam: CEF: install wizard cancelled: eInstallState 8 ->
#   0`), nothing marked accepted, the Queue's `queue: Saltwind Rally stopped: Licence agreement declined: ...`, and
#   its Toast.
# - Accept. Install it again: both agreements one after another, Left onto Accept and A on each; the stand-in's two
#   `marked accepted`, Steam's client took it, `queue: Saltwind Rally queued`, and its download moved to 25%.
# - Too big. With the room made small, install Quiet Orbit: `steam client: install 9100040 ...: not enough space`,
#   the wizard cancelled from 7 before ContinueInstall, the Toast "Not enough space", and in Manage Queue its row
#   under Stopped (`Row(1) of Quiet Orbit; 1 installing, ...; Stopped: Not enough space`), where A opens Settings >
#   Storage.
#
# Judged by --log-focus a press at a time, the Dashboard's `steam client:`, `queue:` and `toasts:` lines, and the
# stand-in client's `steam: CEF: ...` lines. The screenshots are to look at: the prompt, the declined Toast, the
# stopped row. Windows PowerShell 5.1.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Fail([string] $Why) { throw $Why }

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect (what comes after "focus: "), else the run
# stops. Then a rest for the picture, if one is asked for.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    Start-Sleep -Milliseconds 130
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if (-not $got) { Fail "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'" }
    Say "$Button -> $got"
    if ($Name) {
        Start-Sleep -Milliseconds 4500
        Save-Screen $Name
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
    if ((Focus-Now) -notmatch $pattern) { Fail "$Button never reached 'focus: $Expect': '$(Focus-Now)'" }
    Say "$Button until $(Focus-Now)"
}

# A line the Dashboard must log after the mark $After, within $Seconds.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 15) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if (-not $line) { Fail "${What}: the Dashboard logged no '$Pattern'" }
    Say "${What}: $line"
    $line
}

# No line like this after the mark $After, $Seconds on.
function Expect-NotLogged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 2) {
    Start-Sleep -Seconds $Seconds
    $line = Get-DashboardLog | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -First 1
    if ($line) { Fail "${What}: the Dashboard logged '$line'" }
    Say "${What}: nothing like '$Pattern', as it should be"
}

function Steam-Log {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    if (Test-Path $log) { @(Get-Content $log) } else { @() }
}
function Steam-LogLength { @(Steam-Log).Count }

# A line the stand-in Steam client must have written, after the first $After of its log.
function Expect-Steam([string] $Pattern, [int] $After, [string] $What) {
    $line = Wait-Until { Steam-Log | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } 15
    if (-not $line) { Fail "${What}: the stand-in Steam has no line like '$Pattern'" }
    Say "${What}: $line"
}

# How many lines of the stand-in's, after the first $After, match.
function Count-Steam([string] $Pattern, [int] $After) {
    @(Steam-Log | Select-Object -Skip $After | Where-Object { $_ -match $Pattern }).Count
}

# Runs `standin <arguments>`, which must exit 0.
function Standin([string[]] $Arguments) {
    $standin = Join-Path $PSScriptRoot 'standin.exe'
    $done = Start-Process $standin -ArgumentList $Arguments -Wait -PassThru
    Say "standin $($Arguments -join ' '): exit $($done.ExitCode)"
    if ($done.ExitCode -ne 0) { Fail "standin $($Arguments -join ' ') exited with $($done.ExitCode)" }
}
function Wizard([string[]] $Rest) { Standin (@('steam-wizard', "`"$script:root`"") + $Rest) }

function Menu-Game([string] $Line) { if ($Line -match ' of (.+)$') { $Matches[1] } }

# From the Recent row's first tile rightwards, opens each game's Tile menu until one is the menu of $Name; leaves it
# open on Install.
function Open-InstallOf([string] $Name) {
    Walk Left 'Recent\(0\)'
    foreach ($tile in 1..8) {
        $line = Press Start 'TileMenu: Menu\(.+?\) of .+'
        if ((Menu-Game $line) -eq $Name) {
            if ($line -notmatch '^focus: TileMenu: Menu\(Install\) of ') { Fail "$Name's menu starts '$line', not Install" }
            return
        }
        Press B 'Recent\(\d+\)' | Out-Null
        Press Right 'Recent\(\d+\)' | Out-Null
    }
    Fail "no tile of the Recent row is $Name's"
}

function In-Front { if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" } }

try {
    Close-WindowsPrompts
    Connect-Pad
    $script:root = Get-FakeRoot 'steam'
    $library = Initialize-FakeSteam $script:root
    if (-not (Test-Path (Join-Path $script:root 'standin-client.txt'))) { Fail "make-steam did not mark $script:root as the stand-in Steam" }
    $agreed = @(@($library.games) | Where-Object { $_.source_id -eq '9100060' })[0]
    $big = @(@($library.games) | Where-Object { $_.source_id -eq '9100040' })[0]
    if (-not $agreed -or -not $big) { Fail 'the fake has no Saltwind Rally (9100060) or Quiet Orbit (9100040)' }
    if ($agreed.installed -or $big.installed) { Fail 'Saltwind Rally or Quiet Orbit is installed in the fake already' }
    $agreedName = [regex]::Escape($agreed.name)
    $bigName = [regex]::Escape($big.name)
    $agreedId = $agreed.source_id
    $bigId = $big.source_id
    Say "agreements: $($agreed.name) ($agreedId); too big: $($big.name) ($bigId)"
    Wizard @('--agreements', "${agreedId}:2")

    # --- The fake's client running, its debugger on.
    New-Item (Join-Path $script:root '.cef-enable-remote-debugging') -ItemType File -Force | Out-Null
    $client = Start-Process (Join-Path $script:root 'steam.exe') -ArgumentList @('--standin-client', '--seconds', '1500') -WorkingDirectory $script:root -PassThru
    $portFile = Join-Path $script:root 'standin-cef-port.txt'
    $port = Wait-Until { if (Test-Path $portFile) { (Get-Content $portFile -Raw).Trim() } } 15
    if (-not $port) { Fail "the stand-in Steam's client wrote no debugger port" }
    if ($port -eq '8080' -or $port -eq '8081') { Fail "the stand-in's debugger took a real Steam's port, $port" }
    Say "the stand-in Steam's client, pid $($client.Id), its debugger on 127.0.0.1:$port"

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--steam', "`"$script:root`"")
    $started = Wait-DashboardLog 'first frame presented' 60
    if (-not $started) { Fail 'The Dashboard never presented a frame' }
    Say "started: $started"
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 4
    In-Front
    Press Down 'Recent\(0\)' | Out-Null

    $install = "install $agreedId into Steam's default library"
    $prompt = "Agreement: (Accept|Decline) of $agreedName; agreement (\d) of 2, page (\d+) of (\d+)"

    # --- Decline: the prompt over Home, focus on Decline; it scrolls; A on Decline cancels the wizard.
    Open-InstallOf $agreed.name
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^steam client: ${install}: .+: eInstallState 8 \(ShowEULAs\) at \d+ ms$" $mark 'Decline, the wizard at its agreements' | Out-Null
    $first = Expect-Logged "^focus: Agreement: Decline of $agreedName; agreement 1 of 2, page 1 of \d+$" $mark 'Decline, the prompt with focus on Decline'
    In-Front
    Start-Sleep -Milliseconds 4500
    Save-Screen '01-agreement'
    Press Down "Agreement: Decline of $agreedName; agreement 1 of 2, page \d+ of \d+" | Out-Null
    $paged = Press RB "Agreement: Decline of $agreedName; agreement 1 of 2, page \d+ of \d+"
    if ($paged -notmatch 'page ([2-9]|\d\d+) of ') { Fail "a bumper should have moved the agreement on a page: '$paged'" }
    Save-Screen '02-agreement-scrolled'
    Press Left "Agreement: Accept of $agreedName; agreement 1 of 2, page \d+ of \d+" | Out-Null
    Press Right "Agreement: Decline of $agreedName; agreement 1 of 2, page \d+ of \d+" | Out-Null
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^steam client: ${install}: licence agreement 1 of 2: the user answered Decline$" $mark 'Decline, answered' | Out-Null
    Expect-Steam 'steam: CEF: install wizard cancelled: eInstallState 8 -> 0' $steamMark 'Decline, the wizard cancelled'
    Expect-Logged "^queue: $agreedName stopped: Licence agreement declined: Nothing was installed\.$" $mark 'Decline, the Queue says why' | Out-Null
    Expect-Logged '^toasts: shown "Licence agreement declined" \(Stopped\)' $mark 'Decline, its Toast' | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '03-declined-toast'
    if ((Count-Steam 'marked accepted' $steamMark) -ne 0) { Fail 'Decline: the stand-in marked an agreement accepted' }
    if ((Count-Steam "asked to install $agreedId" $steamMark) -ne 0) { Fail 'Decline: the stand-in queued the install all the same' }

    # --- Accept: both agreements, one after another, Accept on each; then the download starts.
    Start-Sleep -Seconds 6
    In-Front
    Open-InstallOf $agreed.name
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^focus: Agreement: Decline of $agreedName; agreement 1 of 2, page 1 of \d+$" $mark 'Accept, the first agreement' | Out-Null
    In-Front
    Press Left "Agreement: Accept of $agreedName; agreement 1 of 2, page 1 of \d+" | Out-Null
    $second = Get-DashboardLogLength
    Push-Button A
    Expect-Logged "^focus: Agreement: Decline of $agreedName; agreement 2 of 2, page 1 of \d+$" $second 'Accept, the second agreement' | Out-Null
    Start-Sleep -Milliseconds 4500
    Save-Screen '04-second-agreement'
    Press Left "Agreement: Accept of $agreedName; agreement 2 of 2, page 1 of \d+" | Out-Null
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Steam "steam: CEF: licence agreement `"${agreedId}_eula_2`" version 3 of $agreedId marked accepted" $steamMark 'Accept, the second marked accepted'
    if ((Count-Steam 'marked accepted' $steamMark) -ne 2) { Fail "Accept: the stand-in marked $(Count-Steam 'marked accepted' $steamMark) agreements accepted, not 2" }
    Expect-Logged "^steam client: ${install}: Steam's client took it$" $mark 'Accept, Steam''s client took it' | Out-Null
    Expect-Steam "steam: CEF: asked to ${install}: $agreedId queued in" $steamMark 'Accept, queued'
    Expect-Logged "^queue: $agreedName queued$" $mark 'Accept, the Queue' | Out-Null
    $mark = Get-DashboardLogLength
    Standin @('steam-install', "`"$script:root`"", $agreedId, '--bytes', '25/100')
    Expect-Logged "^queue: $agreedName downloading, 25% of 100 bytes$" $mark 'Accept, the download started' | Out-Null
    $front = Get-Foreground
    if ($front.Title -ne 'Xbox Series Dashboard') { Fail "after the install, '$($front.Title)' ($($front.Process)) has the foreground" }

    # --- Too big: the room made smaller than Quiet Orbit; refused before ContinueInstall, told why, and its row.
    Wizard @('--agreements', "${agreedId}:2", '--free', '1000000000')
    In-Front
    Open-InstallOf $big.name
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    $bigInstall = "install $bigId into Steam's default library"
    Expect-Logged "^steam client: ${bigInstall}: not enough space: \d+ bytes needed, 1000000000 free; not continued$" $mark 'Too big, refused' | Out-Null
    Expect-Steam 'steam: CEF: install wizard cancelled: eInstallState 7 -> 0' $steamMark 'Too big, the wizard cancelled'
    if ((Count-Steam "asked to install $bigId" $steamMark) -ne 0) { Fail 'Too big: the stand-in queued the install all the same' }
    Expect-Logged "^queue: $bigName stopped: Not enough space: It needs [\d.]+ GB and 953\.7 MB is free\. Free up space in Settings > Storage, or install it somewhere else\.$" $mark 'Too big, the Queue says why' | Out-Null
    Expect-Logged '^toasts: shown "Not enough space" \(Stopped\)' $mark 'Too big, its Toast' | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '05-no-space-toast'
    Start-Sleep -Seconds 6
    In-Front

    # --- Manage Queue: Saltwind Rally installing, then Quiet Orbit under Stopped; A on it opens Settings > Storage.
    Walk Left 'Recent\(0\)'
    Press Down 'Wide\(\d\)' | Out-Null
    Walk Right 'Wide\(3\)' 4
    Press A 'ManageQueue: .*1 installing, \d+ recently installed.*' | Out-Null
    Walk Down "ManageQueue: Row\(1\) of $bigName; 1 installing, \d+ recently installed; Stopped: Not enough space" 4
    Start-Sleep -Milliseconds 4500
    Save-Screen '06-stopped-row'
    Press A 'Settings: .+' '07-storage' | Out-Null
    Press B 'ManageQueue: Row\(1\) of .+' | Out-Null

    if ($dashboard.HasExited) { Fail "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($client.HasExited) { Fail "the stand-in Steam's client exited with $($client.ExitCode)" }
    $result = 'PASS'
} catch {
    $failures += "$_"
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
