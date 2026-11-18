# GOG's library from GOG's library service with gogdl's sign-in, where GOG Galaxy's database is not on the PC (#276,
# docs/sources.md "GOG"), against a stand-in of GOG's web services on 127.0.0.1 (`standin serve-gog-web`, named to the
# Dashboard by DASHBOARD_STORE_WEB, which it takes only for an address on this PC) and the stand-in gogdl signed in
# with its made-up tokens. The fake GOG (Initialize-FakeGog) with its Galaxy database taken away: its two installed
# games stay. Judged by the log, and nothing of GOG's own is asked.
#
# What it checks:
# - At start the library is taken and kept: the stand-in asked for every page with the stand-in's sign-in, and the
#   Dashboard says `gog: owned: 4 games (from 127.0.0.1:<port>/galaxy-library, <when> UTC)`, then `helpers: owned
#   games listed again for gog`; `<data>\owned\gog.json` holds the four product ids and no token.
# - Quarrel Fen, which only the service knows (named by the product API), is listed and not installed; from its Game
#   hub, Install goes through gogdl (`gog: gogdl: <id>: installing into ...`) to the end (`queue: Quarrel Fen
#   installed`).
#
# Every press is judged by --log-focus before the next. Runs on the VM and in safe mode on the TV PC; every folder is
# the test's own.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$started = @()
$title = 'Xbox Series Dashboard'
$name = 'Quarrel Fen'
$id = '1900000104'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect; a press that goes elsewhere ends the walk.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    Start-Sleep -Milliseconds 150
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if (-not $got) { Say "$Button -> NOT '$Expect': $(Focus-Now)"; throw "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'" }
    Say "$Button -> $got"
    if ($Name) { Start-Sleep -Milliseconds 2500; Save-Screen $Name }
    $got
}

# Presses $Button until focus is where $Expect says, $Most times at most.
function Walk([string] $Button, [string] $Expect, [int] $Most = 10) {
    $pattern = "^focus: $Expect`$"
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match $pattern) { break }
        Push-Button $Button
        Start-Sleep -Milliseconds 700
    }
    if ((Focus-Now) -notmatch $pattern) { throw "$Button never reached 'focus: $Expect': '$(Focus-Now)'" }
    Say "$Button until $(Focus-Now)"
}

# A line the Dashboard must log after the mark $After, within $Seconds; the walk stops without it.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 30) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if (-not $line) { Say "${What}: no line like '$Pattern'"; throw "${What}: the Dashboard logged no '$Pattern'" }
    Say "${What}: $line"
    $line
}

function In-Front { if ((Get-Foreground).Title -ne $title) { Say "note: put in front: $(Set-Foreground $title)" } }

try {
    Close-WindowsPrompts
    Connect-Pad
    $standinLog = Join-Path $env:E2E_OUT 'standin.log'
    $gogRoot = Get-FakeRoot 'gog'
    Initialize-FakeGog $gogRoot | Out-Null
    # No Galaxy on this PC: its database taken away.
    Remove-Item (Join-Path $gogRoot 'Galaxy\storage\galaxy-2.0.db') -Force
    $helperHome = Get-FakeRoot 'gog-owned-home'
    $data = Get-FakeRoot 'gog-owned-data'
    foreach ($dir in $helperHome, $data) {
        if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
        New-Item $dir -ItemType Directory -Force | Out-Null
    }
    Register-Fake 'gog-owned-home' $helperHome
    Register-Fake 'gog-owned-data' $data
    $own = Join-Path $helperHome 'gogdl'
    $games = Join-Path $helperHome 'games'

    # gogdl as Settings > Helpers keeps it once added, signed in with its own `auth --code`.
    $kept = Join-Path $data 'helpers\gogdl'
    New-Item $kept -ItemType Directory -Force | Out-Null
    $gogdl = Join-Path $kept 'gogdl.exe'
    Copy-Item (Join-Path $PSScriptRoot 'standin.exe') $gogdl
    $hash = (Get-FileHash $gogdl -Algorithm SHA256).Hash.ToLower()
    [IO.File]::WriteAllText((Join-Path $kept 'helper.json'), "{`"version`":`"1.4.0`",`"sha256`":`"$hash`"}", (New-Object Text.UTF8Encoding $false))
    $env:GOGDL_CONFIG_PATH = $own
    $auth = Start-Process $gogdl -ArgumentList @('--auth-config-path', "`"$(Join-Path $own 'auth.json')`"", 'auth', '--code', 'stand-in-code') -Wait -PassThru -WindowStyle Hidden
    Remove-Item Env:\GOGDL_CONFIG_PATH
    if ($auth.ExitCode -ne 0) { throw "the stand-in gogdl's auth ended $($auth.ExitCode)" }

    # The stand-in of GOG's services on a free port.
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start(); $port = $listener.LocalEndpoint.Port; $listener.Stop()
    $server = Start-Process (Join-Path $PSScriptRoot 'standin.exe') -ArgumentList @('serve-gog-web', '--port', "$port") -PassThru -WindowStyle Hidden
    $started += $server.Id
    $listening = Wait-Until { if (Test-Path $standinLog) { Get-Content $standinLog | Where-Object { $_ -match "serve-gog-web: listening on http://127\.0\.0\.1:$port" } | Select-Object -Last 1 } } 20
    if (-not $listening) { throw "the stand-in GOG services on port $port never said they were listening" }
    Say "stand-in: $listening"

    $env:DASHBOARD_STORE_WEB = "http://127.0.0.1:$port"
    $env:STANDIN_HELPER_STEP_MS = '500'
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--gog', "`"$gogRoot`"", '--data-dir', "`"$data`"", '--helper-home', "`"$helperHome`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }

    # --- Taken and kept at start.
    Expect-Logged "^gog: owned: 4 games \(from 127\.0\.0\.1:$port/galaxy-library, \d{4}-\d\d-\d\d \d\d:\d\d UTC\)$" 0 'Owned list' 60 | Out-Null
    Expect-Logged '^helpers: owned games listed again for .*gog.*$' 0 'Read again' 60 | Out-Null
    $file = Join-Path $data 'owned\gog.json'
    if (-not (Test-Path $file)) { throw "$file was not written" }
    $text = Get-Content $file -Raw
    $ids = ((ConvertFrom-Json $text).games | ForEach-Object { $_.id }) -join ','
    if ($ids -ne '1900000101,1900000102,1900000103,1900000104') { throw "the kept list holds $ids" }
    if ($text -match 'stand-in-access|stand-in-refresh') { throw 'a token is in the kept list' }
    $pages = @(Get-Content $standinLog | Where-Object { $_ -match "serve-gog-web: /galaxy-library/users/\d+/releases\S* \(the stand-in's sign-in\)" })
    if ($pages.Count -lt 3) { throw "the stand-in was asked for $($pages.Count) pages with the sign-in, not 3" }
    Say "pages asked: $($pages.Count)"
    if (Get-DashboardLog | Where-Object { $_ -match 'stand-in-access' }) { throw "gogdl's token is in the Dashboard's log" }

    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 3
    In-Front

    # --- Quarrel Fen, from the service alone: found by Search, installed through gogdl from its hub.
    Press Y 'Search: Key\(Q\); .*' | Out-Null
    Press A 'Search: Key\(Q\); "Q\|" .*' | Out-Null
    Press B 'Search: \w+\(0\) .+; "Q" away.*' | Out-Null
    Walk Right "Search: \w+\(\d+\) $name;.*" 12
    Press A "GameHub: $id Hub Install" '01-hub-install' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^gog: gogdl: ${id}: installing into $([regex]::Escape($games))$" $mark 'Install' | Out-Null
    Expect-Logged "^queue: $name installed$" $mark 'The Queue' 120 | Out-Null
    Start-Sleep -Milliseconds 2500
    Save-Screen '02-installed'

    if ($script:dashboard.HasExited) { throw "The Dashboard exited with $($script:dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Dashboard
    foreach ($pid_ in $started) { Stop-Process -Id $pid_ -Force -ErrorAction SilentlyContinue }
    foreach ($n in 'gogdl', 'scriptinterpreter', 'unins000') { Get-OwnProcess $n | Stop-Process -Force -ErrorAction SilentlyContinue }
    Remove-Item Env:\DASHBOARD_STORE_WEB -ErrorAction SilentlyContinue
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
