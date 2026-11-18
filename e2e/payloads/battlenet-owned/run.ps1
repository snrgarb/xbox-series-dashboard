# Battle.net's owned games from the client's own cache (#276, docs/sources.md "Battle.net"), against a fake Battle.net
# whose CachedData.db holds made-up licences and whose Cache holds a made-up catalogue in the client's own shapes
# (`standin make-battlenet <dir> --owned`: source_battlenet::fixture::Fixture::owned). Judged by the log.
#
# What it checks:
# - At start the owned list is worked out and kept: `battlenet: owned: 6 of 7 catalogue games (from CachedData.db,
#   <when> UTC), 1 of them free to play`, the library read again once it is kept (`helpers: owned games listed again
#   for battlenet`), and `<data>\owned\battlenet.json` written with those six, no licence number in it.
# - Quarry Bell, in the catalogue and not owned, is not listed; Thistle Arena (free to play) and Moorlight Saga
#   (owned, known to the cache alone) are, not installed.
# - Moorlight Saga from its Game hub installs through the stand-in Agent under the uid the cache gives (`the client
#   knows it as moorlight (the client's own catalogue in its cache ...)`), to the end (`queue: Moorlight Saga
#   installed`).
#
# Every press is judged by --log-focus before the next. Runs on the VM and in safe mode on the TV PC; every folder is
# the test's own, and the machine's own Battle.net, its cache and its Agent are never read or asked.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$title = 'Xbox Series Dashboard'
$name = 'Moorlight Saga'
$code = 'mrl'
$uid = 'moorlight'

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

# Presses $Button, then $Back, until focus is where $Expect says.
function Walk-Either([string] $Button, [string] $Back, [string] $Expect, [int] $Most = 12) {
    $pattern = "^focus: $Expect`$"
    foreach ($way in @($Button, $Back)) {
        foreach ($press in 1..$Most) {
            if ((Focus-Now) -match $pattern) { break }
            Push-Button $way
            Start-Sleep -Milliseconds 650
        }
        if ((Focus-Now) -match $pattern) { break }
    }
    if ((Focus-Now) -notmatch $pattern) { throw "never reached 'focus: $Expect': '$(Focus-Now)'" }
    Say "walked to $(Focus-Now)"
}

# A line the Dashboard must log after the mark $After, within $Seconds; the walk stops without it.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 30) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if (-not $line) { Say "${What}: no line like '$Pattern'"; throw "${What}: the Dashboard logged no '$Pattern'" }
    Say "${What}: $line"
    $line
}

function In-Front { if ((Get-Foreground).Title -ne $title) { Say "note: put in front: $(Set-Foreground $title)" } }

# Y on Home, the game's first letter on the Console keyboard, B, then along the results until the log names it.
function Find-Game([string] $Game, [string] $Letter, [string] $Row) {
    Press Y 'Search: Key\(Q\); .*' | Out-Null
    Walk-Either Down Up "Search: Key\($Row\);.*"
    Walk-Either Right Left "Search: Key\($Letter\);.*"
    Press A "Search: Key\($Letter\); `"$Letter\|`" .*" | Out-Null
    Press B "Search: \w+\(0\) .+; `"$Letter`" away.*" | Out-Null
    Walk-Either Right Left "Search: \w+\(\d+\) $([regex]::Escape($Game));.*" 14
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $script:root = Get-FakeRoot 'battlenet'
    $library = Initialize-FakeBattleNet $script:root -Owned
    $names = @($library.games | ForEach-Object { $_.name })
    foreach ($want in 'Moorlight Saga', 'Thistle Arena', 'Larkspur Hollow') {
        if ($names -notcontains $want) { throw "the fake Battle.net should list ${want}: $($names -join ', ')" }
    }
    if ($names -contains 'Quarry Bell') { throw "Quarry Bell is not owned, and was listed: $($names -join ', ')" }
    $script:data = Get-FakeRoot 'battlenet-data'
    if (Test-Path $script:data) { Remove-Item $script:data -Recurse -Force }
    New-Item $script:data -ItemType Directory -Force | Out-Null
    Register-Fake 'battlenet-data' $script:data
    $env:STANDIN_HELPER_STEP_MS = '500'

    $script:dashboard = Start-Dashboard @('--log-focus', '--battlenet', "`"$script:root`"", '--data-dir', "`"$script:data`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    $count = @($library.games).Count
    Expect-Logged "^battlenet: $count games from $([regex]::Escape($script:root)) in \d+ ms$" 0 'Read, its owned games alone offered' 60 | Out-Null
    Expect-Logged '^battlenet: owned: 6 of 7 catalogue games \(from CachedData\.db, \d{4}-\d\d-\d\d \d\d:\d\d UTC\), 1 of them free to play$' 0 'The owned list' 60 | Out-Null
    Expect-Logged '^helpers: owned games listed again for .*battlenet.*$' 0 'Read again once kept' 60 | Out-Null
    $kept = Join-Path $script:data 'owned\battlenet.json'
    if (-not (Test-Path $kept)) { throw "no owned list kept at $kept" }
    $list = Get-Content $kept -Raw -Encoding UTF8 | ConvertFrom-Json
    $ids = @($list.games | ForEach-Object { $_.id }) -join ','
    if ($ids -ne 'emb,glm,jnv,lkh,mrl,ths') { throw "the kept list has $ids" }
    if ((Get-Content $kept -Raw) -match '80000\d') { throw 'a licence number is in the kept list' }
    Say "kept: $kept ($ids)"
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 3
    In-Front

    # --- Moorlight Saga, known to the cache alone: installed through the Agent under the cache's uid.
    Find-Game $name 'M' 'Z'
    Press A "GameHub: $code Hub Install" '01-hub-install' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^battlenet: ${code}: installing $uid \($name\) under .+ through the Agent$" $mark 'Install' | Out-Null
    Expect-Logged "^battlenet: ${code}: the client knows it as $uid \(the client's own catalogue in its cache \(CachedData\.db's catalog_cache\)\)$" $mark 'The cache''s uid' | Out-Null
    Expect-Logged "^battlenet: ${code}: the Agent took the install$" $mark 'Taken' | Out-Null
    Expect-Logged "^queue: $name installed$" $mark 'Installed' 120 | Out-Null
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
    foreach ($own in 'Agent', 'Battle.net') { Get-OwnProcess $own | Stop-Process -Force -ErrorAction SilentlyContinue }
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
