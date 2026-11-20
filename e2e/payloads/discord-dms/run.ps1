# Direct Messages in the Guide, end to end (#298; docs/spec/party-chats.md, docs/discord-web.md), against a stand-in
# discord.com: `standin serve-discord-web` on 127.0.0.1, whose signed-in page carries a fake webpack chunk global
# and fake Flux stores of Discord's shape (made-up people and messages only), and the Dashboard pointed at it with
# `--discord-web-url` and `--discord-cdn-url`. discord.com is never reached; no account of anyone's is used; the data
# folder is the test's own, and the voice connection is pointed at a pipe nobody serves.
#
#   1. Settings > Account > Discord: the switch on, Sign in, the stand-in's phone confirms the QR code.
#      The session finds the stores (`discord web: stores found: ...`) and reads the list (`discord web: n DMs, ...`).
#   2. The Guide, the right bumper to the Discord tab: focus on the first DM (`guide: Discord Entry(k) DM(0) of n`).
#   3. A message arrives in another DM (`POST /standin/dm-message`): the counts on the tab's line change.
#   4. A on the first DM: the conversation opens (`flyout Conversation`), down to the reply box.
#   5. A: text entry (`focus: TextEntry: ...`), one letter typed, Menu sends it: `discord web: sent in n ms`, the
#      stand-in received exactly that text in that DM (`GET /standin/sent`), the reply box empty again.
#   6. B back to the tab; Right along the row to the sixth circle; A opens All Direct Messages.
#   7. Nothing personal in the Dashboard's log: none of the stand-in's made-up names or message words.
#
# Safe mode: nothing registered, nothing of the user's touched; the port and the data folder are the test's own.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-discord-dms-test'
$port = 47814
$base = "http://127.0.0.1:$port"
$standinLog = Join-Path $env:E2E_OUT 'standin.log'
# The stand-in's made-up people and words: never in the Dashboard's log.
$private = @('Stand-in Friend', 'Test Pal', 'Made-up Mate', 'Made-up Group', 'stand_in_friend', 'hello there', 'welcome',
    'e2e-arrives', '100000000000000001', '300000000000000001')

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
    if (-not (Test-Path (Join-Path $PSScriptRoot 'WebView2Loader.dll'))) { throw 'No WebView2Loader.dll beside dashboard.exe' }
    $fake = Start-Process $standin -ArgumentList @('serve-discord-web', '--port', "$port") -PassThru
    $listening = Wait-Until {
        Get-Content $standinLog -ErrorAction SilentlyContinue |
            Where-Object { $_ -match "serve-discord-web: listening on $([regex]::Escape($base))" } | Select-Object -First 1
    } 20
    if (-not $listening) { throw 'the stand-in discord.com never said it was listening' }

    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"", '--discord-web-url', $base,
        '--discord-cdn-url', $base)
    if (-not (Wait-DashboardLog 'first frame presented' 60 0)) { throw 'The Dashboard never presented a frame' }
    Wait-Greeting | Out-Null

    # --- 1. Signed in by the stand-in's QR code; the stores found and the list read.
    Walk Right 'TopBar\(Settings\)' 6
    Press A 'Settings: Rail: General' | Out-Null
    Press Down 'Settings: Rail: Account' | Out-Null
    Press Right 'Settings: Account: Profiles' | Out-Null
    Press Right "Settings: Account: Who's playing today\?" | Out-Null
    Press Down 'Settings: Account: Discord' | Out-Null
    Press A 'Settings: Discord: Direct Messages from discord.com; dms off, off' 30 | Out-Null
    Press A 'Settings: Discord: Direct Messages from discord.com; dms on, .*' | Out-Null
    Press Down 'Settings: Discord: Sign in; dms on, signed out' | Out-Null
    $mark = Get-DashboardLogLength
    Press A "DiscordSignIn: Discord's page" | Out-Null
    if (-not (Expect-Log '^discord web: QR code shown yes$' 30 $mark 'the QR code')) { throw 'no sign-in page' }
    Invoke-WebRequest -Uri "$base/standin/qr-confirm" -Method Post -UseBasicParsing | Out-Null
    if (-not (Expect-Log '^discord web: signed in$' 20 $mark 'signed in')) { throw 'never signed in' }
    Expect-Log '^discord web: stores found: ' 30 $mark 'the stores' | Out-Null
    $listed = Expect-Log '^discord web: \d+ DMs, \d+ unread in \d+$' 30 $mark 'the list'
    if (-not $listed) { throw 'the DM list was never read' }
    Walk B 'TopBar\(.+\)' 6

    # --- 2. The Discord tab, focus on the first DM.
    Step Guide 'Landing Entry\(0\)' | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button RightShoulder
    $tab = Expect-Guide 'Discord Entry\(\d+\) DM\(0\) of \d+' 10 $mark 'the tab, on the first DM' '1-tab'
    if (-not $tab) { throw 'the tab never showed Direct Messages' }
    $counts = ($tab -replace '^.*; (dms \d+, unread \d+ in \d+)$', '$1')

    # --- 3. A message arrives in another DM: the tab's counts follow.
    $mark = Get-DashboardLogLength
    Invoke-WebRequest -Uri "$base/standin/dm-message?channel=100000000000000002&text=e2e-arrives" -Method Post -UseBasicParsing | Out-Null
    $after = Expect-Guide 'Discord Entry\(\d+\) DM\(0\) of \d+.*; dms \d+, unread \d+ in \d+$' 15 $mark 'a message arrived'
    if ($after -and ($after -replace '^.*; (dms \d+, unread \d+ in \d+)$', '$1') -eq $counts) {
        # The line may come before the count changes: the newest one is what counts.
        Start-Sleep -Seconds 2
        if (((Get-GuideLine) -replace '^.*; (dms \d+, unread \d+ in \d+)$', '$1') -eq $counts) { Fail "the counts never changed: still '$counts'" }
    }
    Save-Screen '2-arrived'

    # --- 4. The conversation, down to the reply box.
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Guide '.*flyout Conversation' 10 $mark 'the conversation opened' | Out-Null
    Expect-Log '^discord web: conversation open, \d+ messages$' 15 $mark 'its messages read' | Out-Null
    Walk-Guide Down '.*flyout Conversation ReplyBox; draft 0 chars' 40
    Start-Sleep -Milliseconds 2500
    Save-Screen '3-conversation'

    # --- 5. Written and sent.
    Press A 'TextEntry: .*; 0 chars, .*' | Out-Null
    Press A 'TextEntry: .*; 1 chars, .*' | Out-Null
    Save-Screen '4-writing'
    $mark = Get-DashboardLogLength
    Tap Start
    Expect-Log '^discord web: sent in \d+ ms$' 20 $mark 'sent' | Out-Null
    Expect-Guide '.*flyout Conversation ReplyBox; draft 0 chars' 10 $mark 'the reply box empty again' '5-sent' | Out-Null
    $sent = @((Invoke-WebRequest -Uri "$base/standin/sent" -UseBasicParsing).Content | ConvertFrom-Json)
    $messages = @($sent | Where-Object { $_.kind -eq 'message' })
    if ($messages.Count -ne 1) { Fail "the stand-in should have received one message, not $($messages.Count)" }
    elseif ($messages[0].channel -ne '100000000000000001' -or $messages[0].text.Length -ne 1) {
        Fail "the message went to the wrong DM or with the wrong text ($($messages[0].text.Length) chars)"
    }

    # --- 6. Back to the tab; All Direct Messages from the sixth circle.
    Step B 'Discord Entry\(\d+\) DM\(0\) of \d+' | Out-Null
    Walk-Guide Right 'Discord Entry\(\d+\) AllDMs' 8
    Step A '.*flyout DirectMessages DM\(\d+\) of \d+' '6-all' | Out-Null
    Step B 'Discord Entry\(\d+\) AllDMs' | Out-Null

    # --- 7. Nothing personal in the log.
    $log = @(Get-DashboardLog)
    foreach ($word in $private) {
        $hit = $log | Where-Object { $_.Contains($word) } | Select-Object -First 1
        if ($hit) { Fail "the log holds a made-up private word ($($word.Length) chars) in: $($hit.Substring(0, [Math]::Min(40, $hit.Length)))" }
    }

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
