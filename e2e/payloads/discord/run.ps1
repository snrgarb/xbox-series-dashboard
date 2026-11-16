# Discord's connection (#143, docs/discord.md), against a fake Discord: `standin discord` on a named pipe of the test's
# own (`e2e-discord`, never Discord's `discord-ipc-N`), with its own token endpoint on 127.0.0.1, so the user's own
# Discord on the TV PC is never reached and nothing is sent to discord.com. The Dashboard is pointed at it with
# `--discord-pipe` and `--discord-token-url`.
#
# Settings > Theme settings > Discord is opened from the top bar's Settings (the rail, Personalisation, Down to Guide,
# Right to Discord); a Client ID and a Secret are typed on the Console keyboard with the pad (one letter each: the fake
# takes any), and kept with Menu; Connect. The fake approves at once and signs the user in as "E2E Tester". Then the
# fake goes away, as Discord quitting does: the Dashboard says it lost Discord and tries again after 2 s, 4 s and so on;
# the fake comes back, and the Dashboard signs in again by itself with the tokens it saved. Disconnect at the end
# forgets them.
#
# The clipboard is the user's: Paste is not pressed here (the unit tests have it, crates/discord/tests/client.rs).
#
# Judged by the Dashboard's log: the focus log after each press (`focus: Settings: Discord: Client ID Type; client id
# not set, secret not set, "Not connected"`, and the state after it as it changes), and the connection's own lines:
# `discord: connecting`, `discord: connected as "E2E Tester"`, `discord: lost (Discord isn't running)`, `discord: retry
# in 2 s`, `discord: disconnected: the tokens are forgotten`; by the fake's lines in standin.log (`discord fake: ...`).
# Neither the Secret nor a token is ever in dashboard.log. Passes when every press gives the line expected of it, the
# Dashboard connects, sees the loss, tries again and connects again, and stays up. Safe mode: nothing registered,
# nothing of the user's touched; the fake's pipe and port are the test's own.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-discord-test'
$pipe = 'e2e-discord'
$port = 47812
$tokenUrl = "http://127.0.0.1:$port/api/oauth2/token"
$standinLog = Join-Path $env:E2E_OUT 'standin.log'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { [string] (Focus-Lines | Select-Object -Last 1) }

$Gap = 150

function Tap([string] $Button) {
    Push-Button $Button
    Start-Sleep -Milliseconds $Gap
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

# A line the connection logs, after $Mark: the line, or a failure named $What.
function Expect-Log([string] $Pattern, [double] $Seconds, [int] $Mark, [string] $What) {
    $line = Wait-DashboardLog $Pattern $Seconds $Mark
    if ($line) { Say "$What`: $line" } else { $script:failures += "$What`: no line matching '$Pattern' in $Seconds s" }
    $line
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

    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"",
                                          '--discord-pipe', $pipe, '--discord-token-url', $tokenUrl)
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Wait-Greeting | Out-Null
    if (-not (Wait-DashboardLog "^discord: a test's pipe, $pipe, in place of Discord's$" 5)) {
        $failures += "the Dashboard should have said it uses the test's pipe"
    }

    # --- To Settings > Discord.
    Walk Right 'TopBar\(Settings\)' 6
    Press A 'Settings: Rail: Theme settings' | Out-Null
    Press Right 'Settings: Theme settings: Personalisation' | Out-Null
    Press Down 'Settings: Theme settings: Guide' | Out-Null
    Press Right 'Settings: Theme settings: Discord' | Out-Null
    Press A 'Settings: Discord: Client ID Type; client id not set, secret not set, "Not connected"' '1-discord' | Out-Null

    # --- The Client ID and the Secret, one letter each on the Console keyboard, kept with Menu.
    Press A 'TextEntry: Q; "\|" .+' | Out-Null
    Press A 'TextEntry: Q; "Q\|" .+' | Out-Null
    Press Start 'Settings: Discord: Client ID Type; client id set, secret not set, "Not connected"' | Out-Null
    Press Down 'Settings: Discord: Client Secret Type; .*' | Out-Null
    Press A 'TextEntry: Q; "\|" .+' | Out-Null
    Press A 'TextEntry: Q; "\*\|" .+' | Out-Null
    Press Start 'Settings: Discord: Client Secret Type; client id set, secret saved, "Not connected"' '2-app-set' | Out-Null

    # --- Connect: the fake approves, and the user is signed in.
    $mark = Get-DashboardLogLength
    Press Down 'Settings: Discord: Connect; .*' | Out-Null
    Tap A
    Expect-Log '^discord: connecting$' 10 $mark 'connect' | Out-Null
    Expect-Log '^discord: tokens saved$' 20 $mark 'the code exchanged' | Out-Null
    $connected = Expect-Log '^discord: connected as "E2E Tester"$' 20 $mark 'signed in'
    if ($connected) {
        $line = Wait-Until { Focus-Lines | Where-Object { $_ -match '"Connected as E2E Tester"$' } | Select-Object -Last 1 } 5
        if (-not $line) { $failures += "the page's state should have said Connected as E2E Tester: $(Focus-Now)" }
        Start-Sleep -Milliseconds 1500
        Save-Screen '3-connected'
    }

    # --- Discord goes away: lost, tried again with back-off.
    $mark = Get-DashboardLogLength
    Tell-Fake @('go-away')
    Expect-Log "^discord: lost \(Discord isn't running\)$" 10 $mark 'lost' | Out-Null
    Expect-Log '^discord: retry in 2 s$' 10 $mark 'first retry' | Out-Null
    Expect-Log '^discord: retry in 4 s$' 15 $mark 'second retry' | Out-Null
    Save-Screen '4-lost'

    # --- It comes back: signed in again by itself, with the tokens saved.
    $mark = Get-DashboardLogLength
    Tell-Fake @('come-back')
    Expect-Log '^discord: connected as "E2E Tester"$' 30 $mark 'signed in again' | Out-Null
    if (Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^discord: asking Discord to authorize' }) {
        $failures += 'signing in again should have used the saved tokens, not asked Discord to authorize'
    }

    # --- Disconnect forgets the tokens.
    $mark = Get-DashboardLogLength
    Press Right 'Settings: Discord: Disconnect; .*' | Out-Null
    Tap A
    Expect-Log '^discord: disconnected: the tokens are forgotten$' 10 $mark 'disconnect' | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '5-disconnected'

    # Neither the Secret nor a token was ever written to the log.
    if (Get-DashboardLog | Where-Object { $_ -match 'fake-access-|fake-refresh-|fake-code-' }) { $failures += 'a token or code is in dashboard.log' }

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
