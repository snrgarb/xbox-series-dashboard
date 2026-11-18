# Discord messages' web session (#197, docs/discord-web.md), against a stand-in discord.com: `standin
# serve-discord-web` on 127.0.0.1 (a made-up sign-in form, a session cookie, an app page in Discord's DOM shape), and
# the Dashboard pointed at it with `--discord-web-url`, so discord.com is never reached and no account of anyone's is
# used. The data folder is the test's own.
#
# Settings > Account > Discord messages is opened from the top bar's Settings; the switch is turned on (the session
# starts, finds it is signed out); Sign in shows the stand-in's page over the Dashboard. **The QR code first**: the
# page opens with its QR block shown (`discord web: QR code shown yes`), no field focused though the stand-in's e-mail
# field takes focus by itself as Discord's does, and no keyboard; the test plays the phone (`POST
# /standin/qr-confirm`, "scanned and confirmed") and, with no input at all, the session is signed in, the sign-in
# page closes by itself and Settings says Signed in. The Dashboard is closed and started again: the session, kept in
# <data>\discord-web\profile-<id>, is still signed in. Sign out then removes that folder: nothing is left.
#
# **Typing second**: Sign in again; Down steps to the e-mail field, A opens the Console keyboard, one letter is typed
# and kept with Menu; the same for the password (typed as dots); Down to the form's button and A presses it. Signed in
# again, then signed out again.
#
# Judged by the Dashboard's log: the focus log (`focus: Settings: Discord messages: Use Discord messages; on, signed
# in, kept`, `focus: DiscordSignIn: Tab`) and the session's own lines (`discord web: signed out`, `discord web: focus
# on a password field`, `discord web: signed in`, `discord web: signed out: nothing is left, ... is gone`); and by the
# folder. What was typed is never in dashboard.log or standin.log. Needs Microsoft Edge WebView2 and
# WebView2Loader.dll beside dashboard.exe (e2e/stage.sh copies it). Safe mode: nothing registered, nothing of the
# user's touched; the port and the data folder are the test's own.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-discord-web-test'
$port = 47813
$base = "http://127.0.0.1:$port"
$standinLog = Join-Path $env:E2E_OUT 'standin.log'
# The letters typed: what the log must never hold.
$typed = 'Q'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { [string] (Focus-Lines | Select-Object -Last 1) }

$Gap = 150

# The guard against a web view started over and over (#291 froze the TV PC that way): more than this many attempts
# in one run of the Dashboard, and the test kills it, and the web views of its own data folder, at once and fails.
$MostAttempts = 5

# The Dashboard and the msedgewebview2 processes of this test's own data folder (never another program's), stopped.
function Stop-Everything {
    Stop-Dashboard
    Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -and $_.CommandLine.Contains($data) } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
}

function Test-Storm {
    $log = @(Get-DashboardLog)
    $checking = @($log | Where-Object { $_ -match "^discord web: checking$" }).Count
    $opening = @($log | Where-Object { $_ -match "^discord web: opening the web view" }).Count
    if ($checking -gt $MostAttempts -or $opening -gt $MostAttempts) {
        Stop-Everything
        throw "a web view started over and over ($checking checking, $opening opening lines): the Dashboard was killed"
    }
}

function Tap([string] $Button) {
    Test-Storm
    Push-Button $Button
    Start-Sleep -Milliseconds $Gap
}

function Wait-Focus([int] $Before, [string] $Pattern, [double] $Seconds) {
    Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before) { $lines[$Before..($lines.Count - 1)] | Where-Object { $_ -match "^focus: $Pattern`$" } | Select-Object -Last 1 } } $Seconds
}

# One press that must be followed by a focus line matching $Expect (what follows "focus: ", whole).
function Press([string] $Button, [string] $Expect, [string] $Name = '', [double] $Seconds = 8) {
    $before = @(Focus-Lines).Count
    Tap $Button
    $got = Wait-Focus $before $Expect $Seconds
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Tap $Button
        $got = Wait-Focus $before $Expect $Seconds
    }
    if ($Name) {
        Start-Sleep -Milliseconds 1500
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

function Walk([string] $Button, [string] $Expect, [int] $Most = 8) {
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match "^focus: $Expect`$") { break }
        Tap $Button
        Start-Sleep -Milliseconds 650
    }
    if ((Focus-Now) -notmatch "^focus: $Expect`$") { throw "$Button never reached '$Expect': $(Focus-Now)" }
    Say "$Button until $(Focus-Now)"
}

# Waits for a log line after $Mark, looking for a storm of attempts every quarter second meanwhile.
function Expect-Log([string] $Pattern, [double] $Seconds, [int] $Mark, [string] $What) {
    $line = $null
    $until = (Get-Date).AddSeconds($Seconds)
    while (-not $line -and (Get-Date) -lt $until) {
        Test-Storm
        $line = Get-DashboardLog | Select-Object -Skip $Mark | Where-Object { $_ -match $Pattern } | Select-Object -First 1
        if (-not $line) { Start-Sleep -Milliseconds 250 }
    }
    Test-Storm
    if ($line) { Say "$What`: $line" } else { $script:failures += "$What`: no line matching '$Pattern' in $Seconds s" }
    $line
}

function Start-Test-Dashboard {
    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"", '--discord-web-url', $base)
    if (-not (Wait-DashboardLog 'first frame presented' 60 $script:startMark)) { throw 'The Dashboard never presented a frame' }
    Wait-Greeting | Out-Null
}

# From Home to Settings > Account > Discord messages.
function Open-DiscordMessages([string] $State) {
    Walk Right 'TopBar\(Settings\)' 6
    Press A 'Settings: Rail: General' | Out-Null
    Press Down 'Settings: Rail: Account' | Out-Null
    Press Right 'Settings: Account: Profiles' | Out-Null
    Press Right "Settings: Account: Who's playing today\?" | Out-Null
    Press Down 'Settings: Account: Discord messages' | Out-Null
    Press A "Settings: Discord messages: Use Discord messages; $State" '' 30 | Out-Null
}

# One letter on the Console keyboard, over the sign-in page, kept with Menu.
function Type-Letter([string] $Field) {
    Press A 'TextEntry: Q; "\|" .+' | Out-Null
    Press A 'TextEntry: Q; "(Q|\*)\|" .+' | Out-Null
    $mark = Get-DashboardLogLength
    Tap Start
    Expect-Log '^discord web: the sign-in page is shown' 10 $mark "the sign-in page back after the $Field" | Out-Null
}

try {
    Close-WindowsPrompts
    Connect-Pad
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    New-Item $data -ItemType Directory -Force | Out-Null

    $standin = Join-Path $PSScriptRoot 'standin.exe'
    if (-not (Test-Path $standin)) { throw "No standin.exe in $PSScriptRoot" }
    if (-not (Test-Path (Join-Path $PSScriptRoot 'WebView2Loader.dll'))) { throw 'No WebView2Loader.dll beside dashboard.exe' }
    $fake = Start-Process $standin -ArgumentList @('serve-discord-web', '--port', "$port") -PassThru
    $listening = Wait-Until {
        Get-Content $standinLog -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "serve-discord-web: listening on $([regex]::Escape($base))" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the stand-in discord.com never said it was listening' }
    Say "stand-in: $listening"

    # --- First run: the switch on, signed out; signed in by the QR code, with no input on the page at all.
    $script:startMark = 0
    Start-Test-Dashboard
    Open-DiscordMessages 'off, off'
    $mark = Get-DashboardLogLength
    Press A 'Settings: Discord messages: Use Discord messages; on, .*' | Out-Null
    Expect-Log '^discord web: switched on$' 5 $mark 'the switch' | Out-Null
    # Nothing kept: signed out at once, and nothing runs until Sign in.
    Expect-Log '^discord web: signed out$' 10 $mark 'signed out at first' | Out-Null
    if (Test-Path (Join-Path $data 'discord-web')) { $failures += 'the switch alone should make no session folder' }
    Save-Screen '1-switched-on'
    Press Down 'Settings: Discord messages: Sign in; on, signed out' | Out-Null
    $mark = Get-DashboardLogLength
    $focusMark = @(Focus-Lines).Count
    Press A "DiscordSignIn: Discord's page" | Out-Null
    Expect-Log '^discord web: WebView2 runtime ' 15 $mark 'the runtime' | Out-Null
    Expect-Log '^discord web: the sign-in page is shown' 20 $mark 'the sign-in page' | Out-Null
    Expect-Log '^discord web: page zoom ' 5 $mark 'the zoom' | Out-Null
    Expect-Log '^discord web: QR code shown yes$' 20 $mark 'the QR code' | Out-Null
    Start-Sleep -Seconds 2
    Save-Screen '2-qr-code'
    $focused = @(Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^discord web: focus on a (text|password) field$' })
    if ($focused.Count -gt 0) { $failures += "the page should open with no field focused: $($focused[0])" }

    # The phone: scanned and confirmed. Nothing is pressed.
    $phone = Invoke-WebRequest -Uri "$base/standin/qr-confirm" -Method Post -UseBasicParsing
    Say "the phone: $($phone.Content)"
    Expect-Log '^discord web: signed in$' 20 $mark 'signed in by the QR code' | Out-Null
    $back = Wait-Until { Focus-Lines | Where-Object { $_ -match '^focus: Settings: Discord messages: .*; on, signed in, kept$' } | Select-Object -Last 1 } 10
    if ($back) { Say "back on Settings: $back" } else { $failures += "the sign-in page should have closed on Settings, signed in: $(Focus-Now)" }
    $lines = @(Focus-Lines)
    $keyboard = @($lines[$focusMark..($lines.Count - 1)] | Where-Object { $_ -match '^focus: (TextEntry|DiscordSignIn: keyboard)' })
    if ($keyboard.Count -gt 0) { $failures += "the QR way should need no keyboard: $($keyboard[0])" }
    Start-Sleep -Milliseconds 1500
    Save-Screen '3-signed-in-by-qr'
    $kept = @(Get-ChildItem (Join-Path $data 'discord-web') -Directory -ErrorAction SilentlyContinue)
    if ($kept.Count -ne 1 -or $kept[0].Name -notmatch '^profile-\d+$') { $failures += "one session folder, profile-<id>, should be kept: $($kept.Name -join ', ')" }

    # --- Restarted: still signed in, from the kept session alone.
    Close-Dashboard | Out-Null
    # Each start writes a new log (stderr is redirected afresh), so the restart reads it from its first line.
    $script:startMark = 0
    Start-Test-Dashboard
    Expect-Log '^discord web: signed in$' 40 $script:startMark 'still signed in after a restart' | Out-Null

    # --- Sign out: nothing is left.
    Open-DiscordMessages 'on, signed in, kept'
    Press Down 'Settings: Discord messages: Sign in; .*' | Out-Null
    Press Right 'Settings: Discord messages: Sign out; .*' | Out-Null
    $mark = Get-DashboardLogLength
    Tap A
    Expect-Log '^discord web: signed out: nothing is left, .* is gone$' 30 $mark 'signed out' | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '4-signed-out'
    $left = @(Get-ChildItem (Join-Path $data 'discord-web') -Recurse -Force -ErrorAction SilentlyContinue)
    if ($left.Count -gt 0) { $failures += "nothing of the session should be left: $($left.Count) files and folders under discord-web" }
    $back = Wait-Until { Focus-Lines | Where-Object { $_ -match '^focus: Settings: Discord messages: .*; on, signed out$' } | Select-Object -Last 1 } 10
    if (-not $back) { $failures += "Settings should say signed out, nothing kept: $(Focus-Now)" }

    # --- Typing second: signed in on the stand-in's form with the pad and the Console keyboard.
    Walk Left 'Settings: Discord messages: Sign in; on, signed out' 2
    $mark = Get-DashboardLogLength
    Press A "DiscordSignIn: Discord's page" | Out-Null
    Expect-Log '^discord web: the sign-in page is shown' 20 $mark 'the sign-in page again' | Out-Null
    Expect-Log '^discord web: QR code shown yes$' 20 $mark 'the QR code again' | Out-Null
    Start-Sleep -Seconds 1
    $mark = Get-DashboardLogLength
    Press Down 'DiscordSignIn: Tab' | Out-Null
    Expect-Log '^discord web: focus on a text field$' 10 $mark 'the e-mail field' | Out-Null
    Type-Letter 'e-mail'
    $mark = Get-DashboardLogLength
    Press Down 'DiscordSignIn: Tab' | Out-Null
    Expect-Log '^discord web: focus on a password field$' 10 $mark 'the password field' | Out-Null
    Type-Letter 'password'
    Save-Screen '5-filled'
    $mark = Get-DashboardLogLength
    Press Down 'DiscordSignIn: Tab' | Out-Null
    Expect-Log '^discord web: focus on no field$' 10 $mark 'the button' | Out-Null
    Tap A
    Expect-Log '^discord web: signed in$' 20 $mark 'signed in by typing' | Out-Null
    $back = Wait-Until { Focus-Lines | Where-Object { $_ -match '^focus: Settings: Discord messages: .*; on, signed in, kept$' } | Select-Object -Last 1 } 10
    if ($back) { Say "back on Settings: $back" } else { $failures += "the sign-in page should have closed on Settings, signed in: $(Focus-Now)" }
    $asked = @(Get-Content $standinLog -ErrorAction SilentlyContinue | Where-Object { $_ -match '^.*serve-discord-web: POST /login' })
    if ($asked.Count -ne 1) { $failures += "the form should have been sent once, not $($asked.Count) times" }
    Start-Sleep -Milliseconds 1500
    Save-Screen '6-signed-in-by-typing'

    # And signed out again: nothing left.
    Press Right 'Settings: Discord messages: Sign out; .*' | Out-Null
    $mark = Get-DashboardLogLength
    Tap A
    Expect-Log '^discord web: signed out: nothing is left, .* is gone$' 30 $mark 'signed out again' | Out-Null
    $left = @(Get-ChildItem (Join-Path $data 'discord-web') -Recurse -Force -ErrorAction SilentlyContinue)
    if ($left.Count -gt 0) { $failures += "nothing of the session should be left the second time: $($left.Count) under discord-web" }

    # Nothing typed in the log.
    $logged = @(Get-DashboardLog) + @(Get-Content $standinLog -ErrorAction SilentlyContinue)
    if ($logged | Where-Object { $_ -match 'password=|email=' }) { $failures += 'what was typed is in a log' }

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Everything
    if ($fake -and -not $fake.HasExited) { Stop-Process -Id $fake.Id -Force -ErrorAction SilentlyContinue }
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
