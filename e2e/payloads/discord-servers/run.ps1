# The Discord tab's Servers and Shortcuts (#145, docs/spec/discord-tab.md), against a fake Discord: `standin discord`
# on a named pipe of the test's own (`e2e-discord`, never Discord's `discord-ipc-N`), with its token endpoint and
# stand-in CDN on 127.0.0.1, so the user's own Discord on the TV PC is never reached and nothing is sent to
# discord.com or Discord's CDN. The Dashboard is pointed at it with `--discord-pipe`, `--discord-token-url` and
# `--discord-cdn-url`, with a data folder of its own, so there are no Shortcuts to begin with.
#
#   1. Connect in Settings > Discord as e2e/payloads/discord does, then B back to Home's top bar. The fake puts Ana
#      and Bo in Late night, without the user.
#   2. The Guide's Discord tab: out of a Call with no Shortcuts, focus on Servers.
#   3. A on Servers: its Flyout opens at once on the spinner, then lists the fake's three servers, none joined, by
#      name ignoring case: Another Server, art club, Stand-in Server.
#   4. Down to Stand-in Server, A: its voice channels, General Empty and Late night with Ana and Bo. B goes back to
#      the servers, on Stand-in Server; A opens it again, shown at once.
#   5. Down to Late night, A: joined. The Flyout closes, the Call arrives, focus on Mute.
#   6. Down to Leave, A: out of the Call, and Late night's Shortcut is there, focus on it.
#   7. A on the Shortcut: joined again in one press. The fake hangs up: back on the Shortcut.
#   8. Menu on the Shortcut: "Late night" with Remove shortcut. A: it is gone, focus on Servers.
#
# Judged by the Dashboard's log: the `guide:` lines (--log-focus), which on this tab say the entry focus is on and
# what Discord says, and with the Flyout open what it shows (`guide: Discord Entry(2) Servers; not in a call; mute
# off, deafen off, flyout Discord Item(1) "Stand-in Server" "Late night" of 2: 2 in it`), and the connection's
# `discord:` lines (`discord: asking Discord for the servers`, `discord: servers: "Another Server", "art club",
# "Stand-in Server"`, `discord: voice channels of "Stand-in Server": "General", "Late night"`, `discord: in "Late
# night": Ana, Bo`, `discord: asking Discord to join "Late night"`, `discord: shortcut "Late night" removed`).
# Passes when every press and every change the fake makes gives the line expected of it, and the Dashboard stays up.
# Safe mode: nothing registered, nothing of the user's touched; the fake's pipe and port are the test's own. Home has
# no library: focus starts on the top bar.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-discord-servers-test'
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

function Tell-Fake([string[]] $Words) {
    $said = Start-Process $standin -ArgumentList (@('discord-say', $pipe) + $Words) -PassThru -Wait
    Say "discord-say $($Words -join ' '): exit $($said.ExitCode)"
    if ($said.ExitCode -ne 0) { throw "the fake Discord would not $($Words -join ' ')" }
}

# The tab's line out of a Call, before what the Flyout shows.
$idle = 'not in a call; mute off, deafen off'
$servers = '"Servers" "Another Server" of 3'

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

    # --- 1. Connect in Settings > Discord, then back to the top bar; Ana and Bo in Late night.
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
    Tell-Fake @('in', 'Late_night', 'Ana', 'Bo')

    # --- 2. The Discord tab: no Shortcuts yet, focus on Servers.
    Step Guide 'Landing Entry\(0\)' | Out-Null
    Step RightShoulder "Discord Entry\(2\) Servers; $idle`$" '1-no-shortcuts' | Out-Null

    # --- 3. Servers: the spinner, then the servers by name.
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord: asking Discord for the servers$' 5 $mark 'servers asked' | Out-Null
    Expect-Guide "Discord Entry\(2\) Servers; $idle, flyout Discord Nothing ""Servers"" loading$" 5 $mark 'the spinner' | Out-Null
    Expect-Log '^discord: servers: "Another Server", "art club", "Stand-in Server"$' 10 $mark 'the servers' | Out-Null
    Expect-Guide "Discord Entry\(2\) Servers; $idle, flyout Discord Item\(0\) $servers`$" 10 $mark 'listed' '2-servers' | Out-Null
    Expect-Log '^discord: icon of "?Stand-in Server"? fetched$' 15 $mark "Stand-in Server's icon" | Out-Null

    # --- 4. Stand-in Server: its voice channels and who is in them; B back onto it; A again, at once.
    Step Down "Discord Entry\(2\) Servers; $idle, flyout Discord Item\(1\) ""Servers"" ""art club"" of 3`$" | Out-Null
    Step Down "Discord Entry\(2\) Servers; $idle, flyout Discord Item\(2\) ""Servers"" ""Stand-in Server"" of 3`$" | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord: asking Discord for "Stand-in Server"''s voice channels$' 5 $mark 'channels asked' | Out-Null
    Expect-Log '^discord: voice channels of "Stand-in Server": "General", "Late night"$' 10 $mark 'the channels' | Out-Null
    Expect-Log '^discord: in "General": nobody$' 10 $mark 'nobody in General' | Out-Null
    Expect-Log '^discord: in "Late night": Ana, Bo$' 10 $mark 'Ana and Bo in Late night' | Out-Null
    Expect-Guide "Discord Entry\(2\) Servers; $idle, flyout Discord Item\(0\) ""Stand-in Server"" ""General"" of 2: Empty`$" 10 $mark 'General, Empty' '3-channels' | Out-Null
    Step B "Discord Entry\(2\) Servers; $idle, flyout Discord Item\(2\) ""Servers"" ""Stand-in Server"" of 3`$" | Out-Null
    Step A "Discord Entry\(2\) Servers; $idle, flyout Discord Item\(0\) ""Stand-in Server"" ""General"" of 2: Empty`$" | Out-Null

    # --- 5. Late night joined: the Flyout closes, the Call arrives, focus on Mute.
    Step Down "Discord Entry\(2\) Servers; $idle, flyout Discord Item\(1\) ""Stand-in Server"" ""Late night"" of 2: 2 in it`$" | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord: asking Discord to join "Late night"$' 5 $mark 'join asked' | Out-Null
    Expect-Log '^discord: in a call in "Late night" with E2E Tester, Ana, Bo$' 10 $mark 'the call' | Out-Null
    Expect-Guide 'Discord Entry\(4\) Mute; in "Late night" with E2E Tester, Ana, Bo; mute off, deafen off$' 10 $mark 'the call on the tab' '4-joined' | Out-Null

    # --- 6. Leave: Late night's Shortcut, focus on it.
    Step Down 'Discord Entry\(5\) Deafen;' | Out-Null
    Step Down 'Discord Entry\(6\) Leave;' | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord: out of the call$' 10 $mark 'left' | Out-Null
    Expect-Guide "Discord Entry\(2\) Shortcut ""Late night"" \(Stand-in Server\); $idle`$" 10 $mark 'the shortcut' '5-shortcut' | Out-Null

    # --- 7. Rejoined in one press; the fake hangs up.
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord: asking Discord to join "Late night"$' 5 $mark 'rejoin asked' | Out-Null
    Expect-Guide 'Discord Entry\(4\) Mute; in "Late night" with E2E Tester, Ana, Bo;' 10 $mark 'rejoined' | Out-Null
    $mark = Get-DashboardLogLength
    Tell-Fake @('hang-up')
    Expect-Guide "Discord Entry\(2\) Shortcut ""Late night"" \(Stand-in Server\); $idle`$" 10 $mark 'the shortcut again' | Out-Null

    # --- 8. Remove shortcut, from its menu.
    Step Start 'Discord Entry\(2\), dialog Menu \{ prompt: false, row: 0 \} "Late night"$' '6-menu' | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^discord: shortcut "Late night" removed$' 5 $mark 'removed' | Out-Null
    Expect-Guide "Discord Entry\(2\) Servers; $idle`$" 10 $mark 'gone, focus on Servers' '7-removed' | Out-Null
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
