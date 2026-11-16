# The Guide's Discord tab (#144, docs/spec/discord-tab.md), against a fake Discord: `standin discord` on a named pipe
# of the test's own (`e2e-discord`, never Discord's `discord-ipc-N`), with its token endpoint and stand-in CDN on
# 127.0.0.1, so the user's own Discord on the TV PC is never reached and nothing is sent to discord.com or Discord's
# CDN. The Dashboard is pointed at it with `--discord-pipe`, `--discord-token-url` and `--discord-cdn-url`.
#
#   1. Connect in Settings > Discord as e2e/payloads/discord does (a Client ID and a Secret of one letter each), then
#      B back to Home's top bar.
#   2. The Guide, and the right bumper to its Discord tab: signed in, out of a Call (Account, focus on it).
#   3. The fake puts the user in General with Ana and Bo: the Call arrives, focus goes to Mute, the avatars are fetched
#      from the stand-in CDN. Ana speaks: her ring.
#   4. The fake holds the voice commands. A on Mute: asked, and the switch stays off until the fake confirms; then on.
#   5. Down to Leave, held again: A asks, the Call stays until the fake confirms; then out of it, focus on Account.
#   6. Into Late night with Ana; the fake holds and refuses Mute (now unmute): nothing changes, the connection stays.
#   7. The fake goes away: the tab says "Discord isn't running".
#
# Judged by the Dashboard's log: the `guide:` lines (--log-focus), which on this tab say the entry focus is on and
# what Discord says (`guide: Discord Entry(4) Mute; in "General" with E2E Tester, Ana (speaking), Bo; mute off,
# deafen off`), and the connection's `discord:` lines (`discord: in a call in "General" with E2E Tester, Ana, Bo`,
# `discord: asking Discord to mute`, `discord: Discord says mute on, deafen off`, `discord: out of the
# call`, `discord: Discord refused SET_VOICE_SETTINGS: Nope`, `discord: avatar of Ana fetched`). Passes when every
# press and every change the fake makes gives the line expected of it, nothing shows done before the fake confirms,
# and the Dashboard stays up. Safe mode: nothing registered, nothing of the user's touched; the fake's pipe and port
# are the test's own. Home has no library: focus starts on the top bar.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-discord-tab-test'
$pipe = 'e2e-discord'
$port = 47813
$tokenUrl = "http://127.0.0.1:$port/api/oauth2/token"
$cdnUrl = "http://127.0.0.1:$port"
$standinLog = Join-Path $env:E2E_OUT 'standin.log'

function Fail([string] $What) { $script:failures += $What; Say "WRONG: $What" }

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { [string] (Focus-Lines | Select-Object -Last 1) }

# The last `guide:` line, without its prefix.
function Get-GuideLine {
    $lines = @(Get-DashboardLog | Where-Object { $_ -match '^guide: ' })
    if ($lines.Count) { $lines[-1] -replace '^guide: ', '' } else { 'closed' }
}

$Gap = 150

function Tap([string] $Button) {
    Push-Button $Button
    Start-Sleep -Milliseconds $Gap
}

function Wait-Focus([int] $Before, [string] $Pattern, [double] $Seconds) {
    Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before -and $lines[-1] -match "^focus: $Pattern`$") { $lines[-1] } } $Seconds
}

# One press in Settings that must be followed by a new focus line matching $Expect (what follows "focus: ", whole). A
# press the Dashboard never saw (no new line at all) is pressed once more; one that gave another line is a failure.
function Press([string] $Button, [string] $Expect) {
    $before = @(Focus-Lines).Count
    Tap $Button
    $got = Wait-Focus $before $Expect 8
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Tap $Button
        $got = Wait-Focus $before $Expect 8
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

# Waits for a `guide:` line after $Mark matching $Guide (a regular expression, from the line's start): the line, or a
# failure named $What.
function Expect-Guide([string] $Guide, [double] $Seconds, [int] $Mark, [string] $What, [string] $Name = '') {
    $line = Wait-DashboardLog "^guide: $Guide" $Seconds $Mark
    if ($line) { Say "$What`: $line" } else { Fail "$What`: no line 'guide: $Guide' in $Seconds s (the Guide is at: $(Get-GuideLine))" }
    if ($Name) {
        # The software renderer takes most of a second a frame: a picture waits for the slide.
        Start-Sleep -Milliseconds 3000
        Save-Screen $Name
    }
    $line
}

# One press in the Guide, then the `guide:` line it must bring.
function Step([string] $Button, [string] $Guide, [string] $Name = '') {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    Expect-Guide $Guide 8 $mark $Button $Name
}

# A line the connection logs, after $Mark: the line, or a failure named $What.
function Expect-Log([string] $Pattern, [double] $Seconds, [int] $Mark, [string] $What) {
    $line = Wait-DashboardLog $Pattern $Seconds $Mark
    if ($line) { Say "$What`: $line" } else { Fail "$What`: no line matching '$Pattern' in $Seconds s" }
    $line
}

# Nothing matching $Pattern is logged after $Mark for $Seconds: what is asked of Discord does not show done before it
# says so.
function Expect-NoLog([string] $Pattern, [double] $Seconds, [int] $Mark, [string] $What) {
    $line = Wait-DashboardLog $Pattern $Seconds $Mark
    if ($line) { Fail "$What`: '$line' came before the fake confirmed" } else { Say "$What`: nothing yet, as it should" }
}

function Tell-Fake([string[]] $Words) {
    $said = Start-Process $standin -ArgumentList (@('discord-say', $pipe) + $Words) -PassThru -Wait
    Say "discord-say $($Words -join ' '): exit $($said.ExitCode)"
    if ($said.ExitCode -ne 0) { throw "the fake Discord would not $($Words -join ' ')" }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    New-Item $data -ItemType Directory -Force | Out-Null

    # The fake Discord, until the test ends.
    $standin = Join-Path $PSScriptRoot 'standin.exe'
    if (-not (Test-Path $standin)) { throw "No standin.exe in $PSScriptRoot" }
    $fake = Start-Process $standin -ArgumentList @('discord', '--pipe', $pipe, '--port', "$port") -PassThru
    $listening = Wait-Until {
        Get-Content $standinLog -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "discord fake: listening on $pipe" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the fake Discord never said it was listening' }
    Say "fake: $listening"

    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"", '--discord-pipe', $pipe,
                                          '--discord-token-url', $tokenUrl, '--discord-cdn-url', $cdnUrl)
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Wait-Greeting | Out-Null

    # --- 1. Connect in Settings > Discord, then back to the top bar.
    Walk Right 'TopBar\(Settings\)' 6
    Press A 'Settings: Rail: Theme settings' | Out-Null
    Press Right 'Settings: Theme settings: Personalisation' | Out-Null
    Press Down 'Settings: Theme settings: Guide' | Out-Null
    Press Right 'Settings: Theme settings: Discord' | Out-Null
    Press A 'Settings: Discord: Client ID Type; .*' | Out-Null
    Press A 'TextEntry: Q; "\|" .+' | Out-Null
    Press A 'TextEntry: Q; "Q\|" .+' | Out-Null
    Press Start 'Settings: Discord: Client ID Type; client id set, .*' | Out-Null
    Press Down 'Settings: Discord: Client Secret Type; .*' | Out-Null
    Press A 'TextEntry: Q; "\|" .+' | Out-Null
    Press A 'TextEntry: Q; "\*\|" .+' | Out-Null
    Press Start 'Settings: Discord: Client Secret Type; client id set, secret saved, .*' | Out-Null
    $mark = Get-DashboardLogLength
    Press Down 'Settings: Discord: Connect; .*' | Out-Null
    Tap A
    if (-not (Expect-Log '^discord: connected as "E2E Tester"$' 30 $mark 'signed in')) { throw 'never signed in' }
    Walk B 'TopBar\(.+\)' 6

    # --- 2. The Guide's Discord tab: signed in, out of a Call.
    Step Guide 'Landing Entry\(0\)' | Out-Null
    Step RightShoulder 'Discord Entry\(0\) Account; not in a call; mute off, deafen off$' '1-signed-in' | Out-Null

    # --- 3. The Call arrives: focus on Mute; the avatars come; Ana speaks.
    $mark = Get-DashboardLogLength
    Tell-Fake @('call', 'General', 'Ana', 'Bo')
    Expect-Log '^discord: in a call in "General" with E2E Tester, Ana, Bo$' 10 $mark 'the call' | Out-Null
    Expect-Guide 'Discord Entry\(4\) Mute; in "General" with E2E Tester, Ana, Bo; mute off, deafen off$' 10 $mark 'the call on the tab' | Out-Null
    Expect-Log '^discord: avatar of Ana fetched$' 15 $mark "Ana's avatar" | Out-Null
    Expect-Log '^discord: avatar of Bo fetched$' 15 $mark "Bo's avatar" | Out-Null
    $mark = Get-DashboardLogLength
    Tell-Fake @('speak', 'Ana')
    Expect-Guide 'Discord Entry\(4\) Mute; in "General" with E2E Tester, Ana \(speaking\), Bo;' 10 $mark "Ana's ring" '2-call-speaking' | Out-Null

    # --- 4. Mute, held: off until the fake confirms.
    Tell-Fake @('hold')
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord: asking Discord to mute$' 5 $mark 'mute asked' | Out-Null
    Expect-NoLog '^guide: .*mute on' 3 $mark 'mute before the fake confirms'
    $mark = Get-DashboardLogLength
    Tell-Fake @('confirm')
    Expect-Log '^discord: Discord says mute on, deafen off$' 10 $mark 'muted' | Out-Null
    Expect-Guide 'Discord Entry\(4\) Mute; in "General" with E2E Tester \(muted\), .*; mute on, deafen off$' 10 $mark 'the switch on' '3-muted' | Out-Null
    Tell-Fake @('hush', 'Ana')

    # --- 5. Leave, held: the Call stays until the fake confirms.
    Step Down 'Discord Entry\(5\) Deafen;' | Out-Null
    Step Down 'Discord Entry\(6\) Leave;' | Out-Null
    Tell-Fake @('hold')
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord: asking Discord to leave the call$' 5 $mark 'leave asked' | Out-Null
    Expect-NoLog '^discord: out of the call' 3 $mark 'leaving before the fake confirms'
    $mark = Get-DashboardLogLength
    Tell-Fake @('confirm')
    Expect-Log '^discord: out of the call$' 10 $mark 'left' | Out-Null
    Expect-Guide 'Discord Entry\(0\) Account; not in a call; mute on, deafen off$' 10 $mark 'out of the call on the tab' '4-left' | Out-Null

    # --- 6. A refused command changes nothing.
    $mark = Get-DashboardLogLength
    Tell-Fake @('call', 'Late_night', 'Ana')
    Expect-Guide 'Discord Entry\(3\) Mute; in "Late night" with E2E Tester \(muted\), Ana; mute on, deafen off$' 10 $mark 'a call again' | Out-Null
    Tell-Fake @('hold')
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord: asking Discord to unmute$' 5 $mark 'unmute asked' | Out-Null
    Tell-Fake @('refuse', 'Nope')
    Expect-Log '^discord: Discord refused SET_VOICE_SETTINGS: Nope$' 10 $mark 'refused' | Out-Null
    Expect-NoLog '^guide: .*mute off' 2 $mark 'unmuted though refused'
    if ((Get-GuideLine) -notmatch 'mute on, deafen off$') { Fail "the tab should still say mute on: $(Get-GuideLine)" }

    # --- 7. The fake goes away: the tab says so.
    $mark = Get-DashboardLogLength
    Tell-Fake @('go-away')
    Expect-Log '^discord: out of the call: Discord has gone$' 10 $mark 'the call ended with Discord' | Out-Null
    Expect-Guide 'Discord Entry\(0\) Unavailable; "Discord isn''t running"$' 10 $mark "the tab's line" '5-not-running' | Out-Null
    Step B 'closed' | Out-Null

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Dashboard
    if ($fake -and -not $fake.HasExited) { Stop-Process -Id $fake.Id -Force -ErrorAction SilentlyContinue }
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
