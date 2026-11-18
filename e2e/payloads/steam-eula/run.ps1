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
#   Rally; agreement 1 of 2, page 1`, its count said from the first press on). RB scrolls it until its page number
#   rises (a press within a page logs no new line, so none is waited for), Left is Accept, Right back on
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
# Throughout each wizard (#245) Steam's windows are never seen over the Dashboard and never keep the foreground. The
# stand-in client, as the real one, opens its Install dialog and brings its main window up as the wizard opens, real
# windows that go to the top and ask for the foreground (`steam: window: Install shown and brought to the top; ...`):
# the dialog orange (255, 140, 0), the main window magenta (255, 0, 255), colours the fake's art never has (its
# channels stay within 43..216). From each A that sets an install going until the wizard is over, three points of the
# screen's middle are looked at again and again (`Watch-Front`): any of them in either colour fails the run, and so
# does the stand-in holding the foreground at two looks running. The Dashboard says it kept Home's window in front
# (`front: Home's window kept in front for install <appid> ...`) and let go after (`front: Home's window let go after
# <n> ms (the install is over): topmost false, ...`); the prompt has the foreground; and once the wizard is over both
# of the stand-in's windows are hidden again, as they were found.
#
# Judged by --log-focus a press at a time, the Dashboard's `steam client:`, `front:`, `queue:` and `toasts:` lines,
# the stand-in client's `steam: CEF: ...` and `steam: window: ...` lines, the screen's pixels and the foreground. The
# screenshots are to look at: the prompt, the declined Toast, the stopped row. Windows PowerShell 5.1.
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

# The page number a prompt's focus line says (1 where it has no count yet).
function Page-Of([string] $Line) { if ($Line -match ', page (\d+)') { [int]$Matches[1] } else { 1 } }

# Presses $Button, $Most times at most, until a focus line like "$Expect, page <n> of <m>" says page $Page or more.
# A press that scrolls within a page logs no new line: none is waited for but the one with the page risen.
function Scroll-On([string] $Button, [string] $Expect, [int] $Page, [int] $Most = 6) {
    $pattern = "^focus: $Expect, page (\d+) of (\d+)`$"
    foreach ($press in 1..$Most) {
        Start-Sleep -Milliseconds 130
        Push-Button $Button
        $got = Wait-Until { $line = Focus-Now; if ($line -match $pattern -and (Page-Of $line) -ge $Page) { $line } } 3
        if ($got) {
            Say "$Button x$press -> $got"
            return $got
        }
    }
    Fail "$Button $Most times never brought '$Expect' to page ${Page}: '$(Focus-Now)'"
}

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

# The Dashboard must have the foreground now.
function Expect-Front([string] $What) {
    $front = Get-Foreground
    if ($front.Title -ne 'Xbox Series Dashboard') { Fail "${What}: '$($front.Title)' ($($front.Process)) has the foreground, not the Dashboard" }
    Say "${What}: the Dashboard has the foreground"
}

# The stand-in Steam's windows by their colours (`standin`'s STEAM_COLOUR and INSTALL_COLOUR): its main window
# magenta, its Install dialog orange. Both are over the screen's middle when they show.
$script:SteamColours = @(@(255, 0, 255), @(255, 140, 0))
$script:Points = @(@(960, 540), @(960, 250), @(700, 700))

# Looks at three points of the screen's middle and at the foreground, again and again, from now until the Dashboard
# logs $Until after the mark $After, and $Tail seconds more (Steam's main window is put back up to 1.2 s after a
# step). Any point in a colour of the stand-in Steam's windows fails the run: one of them was seen over the
# Dashboard. So does the stand-in holding the foreground at two looks running (a look is some 50 ms).
function Watch-Front([string] $Until, [int] $After, [string] $What, [double] $Tail = 0, [double] $Seconds = 25) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $looks = 0; $theirs = 0; $running = 0; $ended = $null
    while ($true) {
        foreach ($at in $script:Points) {
            $pixel = Get-DesignPixel $at[0] $at[1]
            foreach ($colour in $script:SteamColours) {
                if (Test-Colour $pixel $colour 20) {
                    Save-Screen 'error-steam-seen'
                    Fail "${What}: a window of the stand-in Steam's was seen over the Dashboard at ($($at[0]), $($at[1])), $(Format-Colour $pixel), $($watch.ElapsedMilliseconds) ms in"
                }
            }
        }
        if ([E2EDesk]::ForegroundPid() -eq $script:client.Id) {
            $theirs++; $running++
            if ($running -ge 2) { Fail "${What}: the stand-in Steam kept the foreground ('$([E2EDesk]::ForegroundTitle())') two looks running, $($watch.ElapsedMilliseconds) ms in" }
        } else { $running = 0 }
        $looks++
        if (-not $ended -and ($looks % 3 -eq 0) -and (Get-DashboardLog | Select-Object -Skip $After | Where-Object { $_ -match $Until } | Select-Object -First 1)) {
            $ended = $watch.Elapsed.TotalSeconds
        }
        if ($ended -and $watch.Elapsed.TotalSeconds -ge $ended + $Tail) { break }
        if ($watch.Elapsed.TotalSeconds -ge $Seconds) { Fail "${What}: the Dashboard logged no '$Until' within $Seconds s" }
    }
    Say "${What}: $looks looks at the screen and the foreground in $($watch.ElapsedMilliseconds) ms, nothing of the stand-in Steam's over the Dashboard; it had the foreground at $theirs"
}

# Home's window kept in front for $Install from the mark $After, and let go again, the foreground unlocked.
function Expect-Kept([string] $Install, [int] $After, [string] $What) {
    Expect-Logged "^steam client: ${Install}: the Dashboard kept in front of Steam's windows while its wizard runs$" $After "$What, kept in front" | Out-Null
    $kept = Expect-Logged "^front: Home's window kept in front for ${Install}: topmost true, the foreground " $After "$What, Home's window topmost"
    if ($kept -match 'could not be locked') { Say "note: ${What}: the foreground could not be locked; the looks took it back instead" }
    Expect-Logged "^front: Home's window let go after \d+ ms \(the install is over\): topmost false, " $After "$What, let go" | Out-Null
    $took = @(Get-DashboardLog | Select-Object -Skip $After | Where-Object { $_ -match '^front: .+ took the foreground ' })
    if ($took.Count) { Say "note: ${What}: the foreground was taken back $($took.Count) time(s): $($took[0])" }
}

# Both of the stand-in Steam's windows hidden again, as they were before the wizard, within five seconds.
function Expect-SteamHidden([string] $What) {
    foreach ($window in @(@('StandInSteam', 'Steam'), @('StandInSteamInstall', 'Install'))) {
        $shown = Wait-Until {
            $handle = [E2EDesk]::FindWindow($window[0], $window[1])
            if ($handle -ne [IntPtr]::Zero -and -not [E2EDesk]::IsWindowVisible($handle)) { 'hidden' }
        } 5
        if (-not $shown) { Fail "${What}: the stand-in Steam's '$($window[1])' is not hidden again (or was never made)" }
    }
    Say "${What}: the stand-in Steam's main window and Install dialog are hidden, as they were found"
}

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

    # --- Decline: the prompt over Home, focus on Decline; it scrolls; A on Decline cancels the wizard.
    Open-InstallOf $agreed.name
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Watch-Front "^focus: Agreement: Decline of $agreedName; agreement 1 of 2" $mark 'Decline, the wizard up to the prompt'
    Expect-Steam 'steam: window: Install shown and brought to the top' $steamMark 'Decline, the stand-in''s Install dialog came up'
    Expect-Steam 'steam: window: Steam shown and brought to the top' $steamMark 'Decline, the stand-in''s main window came up'
    Expect-Logged "^steam client: ${install}: .+: eInstallState 8 \(ShowEULAs\) at \d+ ms$" $mark 'Decline, the wizard at its agreements' | Out-Null
    $first = Expect-Logged "^focus: Agreement: Decline of $agreedName; agreement 1 of 2, page 1( of \d+)?$" $mark 'Decline, the prompt with focus on Decline'
    Expect-Front 'Decline, the prompt'
    Watch-Front '.' $mark 'Decline, the prompt up' 1.5
    Start-Sleep -Milliseconds 3000
    Save-Screen '01-agreement'
    Scroll-On RightShoulder "Agreement: Decline of $agreedName; agreement 1 of 2" 2 | Out-Null
    Save-Screen '02-agreement-scrolled'
    Press Left "Agreement: Accept of $agreedName; agreement 1 of 2, page \d+ of \d+" | Out-Null
    Press Right "Agreement: Decline of $agreedName; agreement 1 of 2, page \d+ of \d+" | Out-Null
    Press A 'Recent\(\d+\)' | Out-Null
    Watch-Front "^queue: $agreedName stopped: " $mark 'Decline, the wizard cancelled' 2
    Expect-Logged "^steam client: ${install}: licence agreement 1 of 2: the user answered Decline$" $mark 'Decline, answered' | Out-Null
    Expect-Steam 'steam: CEF: install wizard cancelled: eInstallState 8 -> 0' $steamMark 'Decline, the wizard cancelled'
    Expect-Kept $install $mark 'Decline'
    Expect-SteamHidden 'Decline'
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
    Watch-Front "^focus: Agreement: Decline of $agreedName; agreement 1 of 2" $mark 'Accept, the wizard up to the prompt'
    Expect-Logged "^focus: Agreement: Decline of $agreedName; agreement 1 of 2, page 1( of \d+)?$" $mark 'Accept, the first agreement' | Out-Null
    Expect-Front 'Accept, the first agreement'
    Press Left "Agreement: Accept of $agreedName; agreement 1 of 2, page 1( of \d+)?" | Out-Null
    $second = Get-DashboardLogLength
    Push-Button A
    Watch-Front "^focus: Agreement: Decline of $agreedName; agreement 2 of 2" $second 'Accept, on to the second agreement'
    Expect-Logged "^focus: Agreement: Decline of $agreedName; agreement 2 of 2, page 1( of \d+)?$" $second 'Accept, the second agreement' | Out-Null
    Expect-Front 'Accept, the second agreement'
    Start-Sleep -Milliseconds 4500
    Save-Screen '04-second-agreement'
    Press Left "Agreement: Accept of $agreedName; agreement 2 of 2, page 1( of \d+)?" | Out-Null
    Press A 'Recent\(\d+\)' | Out-Null
    Watch-Front "^steam client: ${install}: Steam's client took it$" $mark 'Accept, the install going on' 2
    Expect-Kept $install $mark 'Accept'
    Expect-SteamHidden 'Accept'
    Expect-Steam "steam: CEF: licence agreement `"${agreedId}_eula_1`" version 3 of $agreedId marked accepted" $steamMark 'Accept, the second marked accepted'
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
    Watch-Front "^queue: $bigName stopped: " $mark 'Too big, the wizard opened and cancelled' 2
    Expect-Kept $bigInstall $mark 'Too big'
    Expect-SteamHidden 'Too big'
    Expect-Logged "^steam client: ${bigInstall}: not enough space: \d+ bytes needed, 1000000000 free; not continued$" $mark 'Too big, refused' | Out-Null
    Expect-Steam 'steam: CEF: install wizard cancelled: eInstallState 7 -> 16' $steamMark 'Too big, the wizard cancelled'
    Expect-Logged "^steam client: ${bigInstall}: the wizard is closed, eInstallState 16 \(Canceled\)$" $mark 'Too big, a clean cancel at 16' | Out-Null
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
    # Opened from the row, Settings goes straight back to it on B, not up through its own boxes and rail.
    Press B "ManageQueue: Row\(1\) of $bigName; 1 installing, \d+ recently installed; Stopped: Not enough space" | Out-Null

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
