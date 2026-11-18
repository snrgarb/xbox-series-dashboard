# Voice through the web session (#297; docs/discord-web.md "Voice"), against the stand-in discord.com (`standin
# serve-discord-web`): its fake stores hold three made-up servers with voice channels, voice states, speaking, the
# user's mute and deafen, and Discord's own actions (selectVoiceChannel, toggleSelfMute/Deaf, ring/stopRinging). The
# voice connection (RPC) is pointed at a pipe nobody serves, so everything on the tab comes from the web session.
#
#   1. Signed in by the stand-in's QR code; `discord web: voice stores found: ...`.
#   2. The Discord tab: Servers, its Flyout lists the fake's servers; A on the first, A on its first voice channel.
#      `discord web: joined in n ms`, `discord web: in a call, n people`; focus on Mute.
#   3. Mute and Deafen pressed: `discord web: mute on`, `deafen on`; Leave: `discord web: left the call`.
#   4. A DM call rings (`POST /standin/ring`): the call Toast (`toasts: shown (DiscordCall)`), the Guide on the tab's
#      Answer, A answers it: joined, then left.
#   5. The stand-in received join, mute, deafen, leave and a second join, in that order.
#
# Safe mode: nothing registered, nothing of the user's touched; the port and the data folder are the test's own.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-discord-voice-test'
$port = 47815
$base = "http://127.0.0.1:$port"
$standinLog = Join-Path $env:E2E_OUT 'standin.log'

function Fail([string] $What) { $script:failures += $What; Say "WRONG: $What" }
function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { [string] (Focus-Lines | Select-Object -Last 1) }
function Get-GuideLine {
    $lines = @(Get-DashboardLog | Where-Object { $_ -match '^guide: ' })
    if ($lines.Count) { $lines[-1] -replace '^guide: ', '' } else { 'closed' }
}

$Gap = 150
function Tap([string] $Button) { Push-Button $Button; Start-Sleep -Milliseconds $Gap }

function Wait-Focus([int] $Before, [string] $Pattern, [double] $Seconds) {
    Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before) { $lines[$Before..($lines.Count - 1)] | Where-Object { $_ -match "^focus: $Pattern`$" } | Select-Object -Last 1 } } $Seconds
}

function Press([string] $Button, [string] $Expect, [double] $Seconds = 8) {
    $before = @(Focus-Lines).Count
    Tap $Button
    $got = Wait-Focus $before $Expect $Seconds
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Tap $Button
        $got = Wait-Focus $before $Expect $Seconds
    }
    if ($got) { Say "$Button -> $got" } else { Fail "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'" }
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

function Expect-Log([string] $Pattern, [double] $Seconds, [int] $Mark, [string] $What) {
    $line = Wait-DashboardLog $Pattern $Seconds $Mark
    if ($line) { Say "$What`: $line" } else { Fail "$What`: no line matching '$Pattern' in $Seconds s" }
    $line
}

function Expect-Guide([string] $Guide, [double] $Seconds, [int] $Mark, [string] $What, [string] $Name = '') {
    $line = Wait-DashboardLog "^guide: $Guide" $Seconds $Mark
    if ($line) { Say "$What`: $line" } else { Fail "$What`: no line 'guide: $Guide' in $Seconds s (the Guide is at: $(Get-GuideLine))" }
    if ($Name) { Start-Sleep -Milliseconds 2500; Save-Screen $Name }
    $line
}

function Step([string] $Button, [string] $Guide, [string] $Name = '') {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    Expect-Guide $Guide 8 $mark $Button $Name
}

# Presses $Button until a `guide:` line matches $Guide, at most $Most times.
function Walk-Guide([string] $Button, [string] $Guide, [int] $Most = 8) {
    foreach ($press in 1..$Most) {
        if ((Get-GuideLine) -match "^$Guide") { break }
        Push-Button $Button
        Start-Sleep -Milliseconds 650
    }
    if ((Get-GuideLine) -notmatch "^$Guide") { throw "$Button never reached 'guide: $Guide': $(Get-GuideLine)" }
    Say "$Button until guide: $(Get-GuideLine)"
}

function Stop-Everything {
    Stop-Dashboard
    Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -and $_.CommandLine.Contains($data) } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    New-Item $data -ItemType Directory -Force | Out-Null
    $standin = Join-Path $PSScriptRoot 'standin.exe'
    if (-not (Test-Path $standin)) { throw "No standin.exe in $PSScriptRoot" }
    $fake = Start-Process $standin -ArgumentList @('serve-discord-web', '--port', "$port") -PassThru
    $listening = Wait-Until {
        Get-Content $standinLog -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "serve-discord-web: listening on $([regex]::Escape($base))" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the stand-in discord.com never said it was listening' }
    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"", '--discord-web-url', $base,
        '--discord-cdn-url', $base, '--discord-pipe', 'e2e-nobody')
    if (-not (Wait-DashboardLog 'first frame presented' 60 0)) { throw 'The Dashboard never presented a frame' }
    Wait-Greeting | Out-Null

    # --- 1. Signed in; the voice stores found.
    Walk Right 'TopBar\(Settings\)' 6
    Press A 'Settings: Rail: General' | Out-Null
    Press Down 'Settings: Rail: Account' | Out-Null
    Press Right 'Settings: Account: Profiles' | Out-Null
    Press Right "Settings: Account: Who's playing today\?" | Out-Null
    Press Down 'Settings: Account: Direct Messages' | Out-Null
    Press A 'Settings: Direct Messages: Direct Messages from discord.com; dms off, off' 30 | Out-Null
    Press A 'Settings: Direct Messages: Direct Messages from discord.com; dms on, .*' | Out-Null
    Press Down 'Settings: Direct Messages: Sign in; dms on, signed out' | Out-Null
    $mark = Get-DashboardLogLength
    Press A "DiscordSignIn: Discord's page" | Out-Null
    if (-not (Expect-Log '^discord web: QR code shown yes$' 30 $mark 'the QR code')) { throw 'no sign-in page' }
    Invoke-WebRequest -Uri "$base/standin/qr-confirm" -Method Post -UseBasicParsing | Out-Null
    if (-not (Expect-Log '^discord web: signed in$' 20 $mark 'signed in')) { throw 'never signed in' }
    if (-not (Expect-Log '^discord web: voice stores found: ' 30 $mark 'the voice stores')) { throw 'no voice stores' }
    Walk B 'TopBar\(.+\)' 6

    # --- 2. The tab, Servers, a server, its first voice channel: joined.
    Step Guide 'Landing Entry\(0\)' | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button RightShoulder
    Expect-Guide 'Discord Entry\(\d+\) ' 10 $mark 'the tab' '1-tab' | Out-Null
    Walk-Guide Down 'Discord Entry\(\d+\) Servers;' 12
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Guide '.*flyout Discord Item\(0\) "Servers" .* of 3$' 10 $mark 'the servers' '2-servers' | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Guide '.*flyout Discord Item\(0\) ".+" ".+" of \d+: .+$' 10 $mark "a server's voice channels" '3-channels' | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord web: joined in \d+ ms$' 25 $mark 'joined' | Out-Null
    Expect-Log '^discord web: in a call, \d+ (person|people)$' 10 $mark 'in a call' | Out-Null
    Expect-Guide 'Discord Entry\(\d+\) Mute; in ' 10 $mark 'the Call on the tab, focus on Mute' '4-call' | Out-Null

    # --- 3. Mute, Deafen, Leave.
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord web: mute on$' 10 $mark 'muted' | Out-Null
    Walk-Guide Right 'Discord Entry\(\d+\) Deafen;' 3
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord web: deafen on$' 10 $mark 'deafened' | Out-Null
    Walk-Guide Right 'Discord Entry\(\d+\) Leave;' 3
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord web: left the call$' 10 $mark 'left' | Out-Null
    # The Guide closed, so the ring's Toast shows over Home.
    Push-Button B
    Start-Sleep -Milliseconds 1500

    # --- 4. A DM call rings: the Toast, the Guide on Answer, answered.
    $mark = Get-DashboardLogLength
    Invoke-WebRequest -Uri "$base/standin/ring?channel=100000000000000001&on=1" -Method Post -UseBasicParsing | Out-Null
    Expect-Log '^toasts: shown \(DiscordCall\)' 15 $mark 'the call Toast' | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '5-ringing'
    $mark = Get-DashboardLogLength
    Push-Button Guide
    Expect-Guide 'Discord Entry\(\d+\) Answer; ringing' 10 $mark 'the Guide on Answer' '6-answer' | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord web: joined in \d+ ms$' 25 $mark 'answered' | Out-Null
    Walk-Guide Right 'Discord Entry\(\d+\) Leave;' 4
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord web: left the call$' 10 $mark 'left again' | Out-Null

    # --- 5. What the stand-in received.
    $kinds = @((Invoke-WebRequest -Uri "$base/standin/sent" -UseBasicParsing).Content | ConvertFrom-Json | ForEach-Object { $_.kind })
    $said = $kinds -join ' '
    Say "the stand-in received: $said"
    if ($said -notmatch 'join.*mute.*deafen.*leave.*join.*leave') { Fail "the stand-in should have received join, mute, deafen, leave, join, leave: $said" }

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
    Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
