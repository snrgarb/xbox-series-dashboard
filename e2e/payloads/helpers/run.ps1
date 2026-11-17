# Settings > Helpers (#174, docs/spec/helpers.md), walked with the virtual pad against the stand-in helpers' release pages
# (`standin serve-helpers` on 127.0.0.1, which serves standin.exe itself and writes the table `--helper-table` reads, its
# addresses the stand-in's and its SHA-256 standin.exe's). Nothing asks GitHub, and no real helper is fetched.
#
# What it walks:
# - Theme settings > Helpers, under Art: focus on legendary's Add (`Settings: Helpers: legendary; Add; not added`).
# - Add asks first (`asking Add legendary?`), and Yes fetches: the stand-in asked `/helpers/good` with the plain
#   User-Agent and no Authorization; `helpers: legendary: checked and kept at <data>\helpers\legendary\legendary.exe`;
#   the shell's runner runs it once with --version, its lines said one by one (`helpers: legendary: says legendary
#   0.21.1 (stand-in)`), `ended with 0`; the page then says Remove (`legendary; Remove; added 0.21.1`).
# - gogdl, whose address serves other bytes: `helpers: gogdl: refused: its SHA-256 is ...`, nothing kept in its folder,
#   the page saying Add again (`gogdl; Add; failed`).
# - legendary's Remove asks, and Remove deletes its folder (`helpers: legendary: removed ...`; `legendary; Add; not
#   added`).
#
# Every press is judged by --log-focus before the next, and the walk stops at the first that goes elsewhere. Runs on
# the VM and in safe mode on the TV PC; the data folder is the test's own. The screenshots are to look at.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$started = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: "; then a rest for the picture, if one is asked for. A press that goes elsewhere ends the walk.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    Start-Sleep -Milliseconds 130
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if (-not $got) { Say "$Button -> NOT '$Expect': $(Focus-Now)"; throw "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'" }
    Say "$Button -> $got"
    if ($Name) { Start-Sleep -Milliseconds 2500; Save-Screen $Name }
    $got
}

# A line the Dashboard must log after the mark $After, by this pattern, within $Seconds; the walk stops without it.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 30) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if (-not $line) { Say "${What}: no line like '$Pattern'"; throw "${What}: the Dashboard logged no '$Pattern'" }
    Say "${What}: $line"
    $line
}

function In-Front { if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" } }

try {
    Close-WindowsPrompts
    Connect-Pad
    $standinLog = Join-Path $env:E2E_OUT 'standin.log'

    # A data folder of the test's own: the helpers are kept there, and nothing from another test is.
    $data = Get-TestFolder 'helpers-data'
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    New-Item $data -ItemType Directory -Force | Out-Null
    $table = Join-Path (Get-TestFolder 'helpers-table') 'table.json'
    New-Item (Split-Path $table) -ItemType Directory -Force | Out-Null
    if (Test-Path $table) { Remove-Item $table -Force }

    # --- The stand-in release pages on a free port, and the table that points at them.
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start(); $port = $listener.LocalEndpoint.Port; $listener.Stop()
    $server = Start-Process (Join-Path $PSScriptRoot 'standin.exe') -ArgumentList @('serve-helpers', '--table', "`"$table`"", '--port', "$port") -PassThru -WindowStyle Hidden
    $started += $server.Id
    $listening = Wait-Until { if (Test-Path $standinLog) { Get-Content $standinLog | Where-Object { $_ -match "serve-helpers: listening on http://127\.0\.0\.1:$port" } | Select-Object -Last 1 } } 20
    if (-not $listening -or -not (Test-Path $table)) { throw "the stand-in helpers' pages on port $port never said they were listening" }
    Say "stand-in: $listening"

    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"", '--helper-table', "`"$table`"") | Out-Null
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Expect-Logged '^helpers: legendary 0\.21\.1 not added, gogdl 1\.4\.0 not added$' 0 'Helpers read' | Out-Null
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 2
    In-Front

    # --- Theme settings > Helpers: Home starts on the profile, which logs no focus; each Right logs where it went.
    foreach ($i in 1..5) {
        if ((Focus-Now) -match '^focus: TopBar\(Settings\)$') { break }
        Push-Button Right; Start-Sleep -Milliseconds 800
    }
    if ((Focus-Now) -notmatch '^focus: TopBar\(Settings\)$') { throw "Right never reached the top bar's Settings: '$(Focus-Now)'" }
    Press A 'Settings: Rail: Theme settings' | Out-Null
    Press Right 'Settings: Theme settings: Personalisation' | Out-Null
    Press Down 'Settings: Theme settings: Guide' | Out-Null
    Press Down 'Settings: Theme settings: Games & apps' | Out-Null
    Press Down 'Settings: Theme settings: Art' | Out-Null
    Press Down 'Settings: Theme settings: Helpers' | Out-Null
    Press A 'Settings: Helpers: legendary; Add; not added' '01-helpers' | Out-Null

    # --- legendary: Add asks, Yes fetches, checks, keeps and runs it once.
    Press A 'Settings: Helpers: legendary; Add; not added; asking Add legendary\?' '02-add-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Settings: Helpers: legendary; (busy; fetching|Remove; added 0\.21\.1)' | Out-Null
    $kept = [regex]::Escape((Join-Path $data 'helpers\legendary\legendary.exe'))
    Expect-Logged '^helpers: legendary: fetching 0\.21\.1 \(GPL-3\.0 licence, from https://github\.com/legendary-gl/legendary\)$' $mark 'Fetching' | Out-Null
    Expect-Logged "^helpers: legendary: checked and kept at $kept$" $mark 'Kept' | Out-Null
    Expect-Logged '^helpers: legendary: running --version \(pid \d+\)$' $mark 'Run by the shell' | Out-Null
    Expect-Logged '^helpers: legendary: says legendary 0\.21\.1 \(stand-in\)$' $mark 'Its standard output' | Out-Null
    Expect-Logged '^helpers: legendary: says \[cli\] INFO: stand-in legendary, nothing downloaded$' $mark 'Its standard error' | Out-Null
    Expect-Logged '^helpers: legendary: ended with 0$' $mark 'Ended' | Out-Null
    Expect-Logged '^focus: Settings: Helpers: legendary; Remove; added 0\.21\.1$' $mark 'The page says Remove' | Out-Null
    if (-not (Test-Path (Join-Path $data 'helpers\legendary\legendary.exe'))) { throw 'legendary.exe is not in the data folder' }
    $asked = @(Get-Content $standinLog | Where-Object { $_ -match 'serve-helpers: /helpers/good ' })
    if ($asked.Count -ne 1) { throw "the stand-in was asked for legendary $($asked.Count) times, not once" }
    if ($asked[0] -notmatch '\(User-Agent dashboard-helpers, no Authorization\)$') { throw "legendary's request carried more than it should: $($asked[0])" }
    Say "asked: $($asked[0])"
    if (-not (Get-Content $standinLog | Where-Object { $_ -match '\] legendary: --version$' })) { throw 'the stand-in helper never ran as legendary' }
    Start-Sleep -Milliseconds 2000
    Save-Screen '03-legendary-added'

    # --- gogdl: the address serves other bytes; refused, nothing kept.
    Press Down 'Settings: Helpers: gogdl; Add; not added' | Out-Null
    Press A 'Settings: Helpers: gogdl; Add; not added; asking Add gogdl\?' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Settings: Helpers: gogdl; (busy; fetching|Add; failed)' | Out-Null
    Expect-Logged '^helpers: gogdl: refused: its SHA-256 is [0-9a-f]{64}, not the [0-9a-f]{64} the Dashboard expects$' $mark 'Refused' | Out-Null
    Expect-Logged '^focus: Settings: Helpers: gogdl; Add; failed$' $mark 'The page says why' | Out-Null
    if (Get-ChildItem (Join-Path $data 'helpers\gogdl') -ErrorAction SilentlyContinue) { throw "something of gogdl's was kept: $(Get-ChildItem (Join-Path $data 'helpers\gogdl') -Name)" }
    if (Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^helpers: gogdl: running' }) { throw 'gogdl was run, refused' }
    Start-Sleep -Milliseconds 2000
    Save-Screen '04-gogdl-refused'

    # --- legendary removed, after asking.
    Press Up 'Settings: Helpers: legendary; Remove; added 0\.21\.1' | Out-Null
    Press A 'Settings: Helpers: legendary; Remove; added 0\.21\.1; asking Remove legendary\?' '05-remove-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Settings: Helpers: legendary; Add; not added' | Out-Null
    Expect-Logged '^helpers: legendary: removed .+\\helpers\\legendary$' $mark 'Removed' | Out-Null
    if (Test-Path (Join-Path $data 'helpers\legendary')) { throw "legendary's folder is still there" }
    Press B 'Settings: Theme settings: Helpers' | Out-Null

    if ($script:Dashboard.HasExited) { throw "The Dashboard exited with $($script:Dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Dashboard
    foreach ($id in $started) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
    Get-OwnProcess legendary | Stop-Process -Force -ErrorAction SilentlyContinue
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
