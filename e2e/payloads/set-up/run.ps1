# Settings > Set up this PC (#178, docs/spec/set-up.md), walked with the virtual pad against the stand-in launchers'
# installers: `standin serve-helpers --clients` on 127.0.0.1 serves standin.exe itself as every app's installer and
# writes the table `--client-table` reads (the addresses the stand-in's, the SHA-256 standin.exe's, GOG Galaxy's
# address serving other bytes). Run as `standin-setup-<key>.exe`, the stand-in installer writes only in the test's
# own folder that STANDIN_SETUP names: what it was run with (runs.txt), the app's folder, and the app's Uninstall key
# in registry.json, the file the Dashboard reads in place of Windows' list (`--setup-registry`). Nothing real is
# downloaded, installed, listed or removed, and the machine's own Uninstall keys are never read: safe on a PC in
# daily use.
#
# What it walks:
# - Games & apps > Set up this PC, under Helpers: every app not installed, as the test's empty list says.
# - Battle.net (walked to by name): Install asks first (`asking Install Battle.net?`); Install downloads from the
#   stand-in with the plain User-Agent and no Authorization, checks it, and runs it with Battle.net's own switches
#   (`--lang=enUS --installpath="<sandbox>\battlenet"`); the list then has its key, the page says installed, and
#   Battle.net is read again, its two games coming in.
# - GOG Galaxy, whose address serves other bytes: refused, nothing kept and nothing run.
# - Battle.net's Remove asks, and Remove runs the uninstaller its key names; the key goes, the page says not
#   installed, and Battle.net is read again.
#
# Every press is judged by --log-focus before the next, and the walk stops at the first that goes elsewhere.
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

# Up or Down until focus is on the app named $App, by the log, at most $Most presses; each must stay on the page.
function Walk-To([string] $Button, [string] $App, [int] $Most = 8) {
    foreach ($i in 1..$Most) {
        if ((Focus-Now) -match "^focus: Settings: Set up this PC: $([regex]::Escape($App)); ") { return }
        Press $Button 'Settings: Set up this PC: [^;]+; .+' | Out-Null
    }
    if ((Focus-Now) -notmatch "^focus: Settings: Set up this PC: $([regex]::Escape($App)); ") { throw "$Button never reached ${App}: '$(Focus-Now)'" }
}

function Runs { if (Test-Path $runs) { @(Get-Content $runs) } else { @() } }

function In-Front { if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" } }

try {
    Close-WindowsPrompts
    Connect-Pad
    $standinLog = Join-Path $env:E2E_OUT 'standin.log'

    # The test's own folders: the data folder (where an installer is kept while it runs), and the sandbox the stand-in
    # installers write in, with the list standing in for Windows' and an empty Battle.net folder (no Battle.net yet).
    $data = Get-TestFolder 'set-up-data'
    $sandbox = Get-TestFolder 'set-up-sandbox'
    foreach ($dir in @($data, $sandbox)) {
        if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
        New-Item $dir -ItemType Directory -Force | Out-Null
    }
    $battlenet = Join-Path $sandbox 'battlenet'
    New-Item $battlenet -ItemType Directory -Force | Out-Null
    Register-Fake 'battlenet' $battlenet
    $list = Join-Path $sandbox 'registry.json'
    $runs = Join-Path $sandbox 'runs.txt'
    $tables = Get-TestFolder 'set-up-tables'
    New-Item $tables -ItemType Directory -Force | Out-Null
    $helperTable = Join-Path $tables 'helpers.json'
    $clientTable = Join-Path $tables 'clients.json'
    foreach ($file in @($helperTable, $clientTable)) { if (Test-Path $file) { Remove-Item $file -Force } }

    # --- The stand-in release pages on a free port, and the tables that point at them.
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start(); $port = $listener.LocalEndpoint.Port; $listener.Stop()
    $serve = @('serve-helpers', '--table', "`"$helperTable`"", '--clients', "`"$clientTable`"", '--sandbox', "`"$sandbox`"", '--port', "$port")
    $server = Start-Process (Join-Path $PSScriptRoot 'standin.exe') -ArgumentList $serve -PassThru -WindowStyle Hidden
    $started += $server.Id
    $listening = Wait-Until { if (Test-Path $standinLog) { Get-Content $standinLog | Where-Object { $_ -match "serve-helpers: listening on http://127\.0\.0\.1:$port" } | Select-Object -Last 1 } } 20
    if (-not $listening -or -not (Test-Path $clientTable)) { throw "the stand-in release pages on port $port never said they were listening" }
    Say "stand-in: $listening"

    # The stand-in installers' folder, for every program the Dashboard starts; a short wait in each, to see it work.
    $env:STANDIN_SETUP = $sandbox
    $env:STANDIN_SETUP_MS = '2500'
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"", '--client-table', "`"$clientTable`"", '--setup-registry', "`"$list`"", '--battlenet', "`"$battlenet`"") | Out-Null
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Expect-Logged '^set-up: steam not installed, epic not installed, gog not installed, ea not installed, ubisoft not installed, battlenet not installed$' 0 'Windows'' list, as the test''s file has it' | Out-Null
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 2
    In-Front

    # --- Games & apps > Set up this PC: Home starts on the profile, which logs no focus; each Right logs where it went.
    foreach ($i in 1..5) {
        if ((Focus-Now) -match '^focus: TopBar\(Settings\)$') { break }
        Push-Button Right; Start-Sleep -Milliseconds 800
    }
    if ((Focus-Now) -notmatch '^focus: TopBar\(Settings\)$') { throw "Right never reached the top bar's Settings: '$(Focus-Now)'" }
    Press A 'Settings: Rail: General' | Out-Null
    Press Down 'Settings: Rail: Account' | Out-Null
    Press Down 'Settings: Rail: Games & apps' | Out-Null
    Press Right 'Settings: Games & apps: Launchers' | Out-Null
    Press Right 'Settings: Games & apps: Storage' | Out-Null
    Press Down 'Settings: Games & apps: Helpers' | Out-Null
    Press Down 'Settings: Games & apps: Set up this PC' | Out-Null
    Press A 'Settings: Set up this PC: Steam; not installed' '01-set-up' | Out-Null

    # --- Battle.net: Install asks, Install downloads, checks and runs it; the list has it; its games come in.
    Walk-To Down 'Battle.net'
    Press A 'Settings: Set up this PC: Battle\.net; not installed; asking Install Battle\.net\?' '02-install-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Settings: Set up this PC: Battle\.net; (downloading|installing|installed)' | Out-Null
    Expect-Logged "^set-up: battlenet: downloading Battle\.net [\d.]+ from http://127\.0\.0\.1:$port/helpers/good \(\d+ bytes\)$" $mark 'Downloading' | Out-Null
    $kept = [regex]::Escape((Join-Path $data 'set-up\battlenet\standin-setup-battlenet.exe'))
    Expect-Logged "^set-up: battlenet: checked and kept at $kept$" $mark 'Checked and kept' | Out-Null
    $switches = [regex]::Escape("--lang=enUS --installpath=`"$battlenet`"")
    Expect-Logged "^set-up: battlenet: running standin-setup-battlenet\.exe $switches$" $mark 'Run with its switches' | Out-Null
    Expect-Logged '^focus: Settings: Set up this PC: Battle\.net; installing$' $mark 'The page says installing' | Out-Null
    Start-Sleep -Milliseconds 800
    Save-Screen '03-installing'
    Expect-Logged '^set-up: battlenet: standin-setup-battlenet\.exe ended with 0$' $mark 'Ended' 60 | Out-Null
    Expect-Logged '^set-up: battlenet: installed: HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Battle\.net$' $mark 'In the list' 60 | Out-Null
    Expect-Logged '^focus: Settings: Set up this PC: Battle\.net; installed$' $mark 'The page says installed' | Out-Null
    Expect-Logged '^launchers: update: reading battlenet$' $mark 'Battle.net read again' | Out-Null
    Expect-Logged '^launchers: update: battlenet read in \d+ ms: .*new game' $mark 'Its games came in' | Out-Null
    $asked = @(Get-Content $standinLog | Where-Object { $_ -match 'serve-helpers: /helpers/good ' })
    if ($asked.Count -ne 1) { throw "the stand-in was asked for Battle.net's installer $($asked.Count) times, not once" }
    if ($asked[0] -notmatch '\(User-Agent dashboard-helpers, no Authorization\)$') { throw "the request carried more than it should: $($asked[0])" }
    $ran = @(Runs)
    if ($ran.Count -ne 1 -or $ran[0] -notmatch '^install battlenet --lang=enUS --installpath=') { throw "the stand-in installer was run as '$($ran -join ' | ')'" }
    Say "the stand-in installer was run as: $($ran[0])"
    if (-not (Select-String -Path $list -Pattern 'Battle\.net' -Quiet)) { throw "the test's list has no Battle.net key" }
    if (Test-Path (Join-Path $data 'set-up\battlenet\standin-setup-battlenet.exe')) { throw 'the installer was kept after its run' }
    Start-Sleep -Milliseconds 1500
    Save-Screen '04-installed'

    # --- GOG Galaxy: its address serves other bytes; refused, nothing kept, nothing run.
    Walk-To Up 'GOG Galaxy'
    Press A 'Settings: Set up this PC: GOG Galaxy; not installed; asking Install GOG Galaxy\?' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Settings: Set up this PC: GOG Galaxy; (downloading|not installed; failed: .+)' | Out-Null
    Expect-Logged '^set-up: gog: refused: its SHA-256 is [0-9a-f]{64}, not the [0-9a-f]{64} the Dashboard expects$' $mark 'Refused' | Out-Null
    Expect-Logged "^focus: Settings: Set up this PC: GOG Galaxy; not installed; failed: The download wasn't the file expected, so it wasn't kept\.$" $mark 'The page says why' | Out-Null
    if (Get-ChildItem (Join-Path $data 'set-up\gog') -ErrorAction SilentlyContinue) { throw "something of GOG Galaxy's was kept: $(Get-ChildItem (Join-Path $data 'set-up\gog') -Name)" }
    if (Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^set-up: gog: running' }) { throw 'GOG Galaxy''s installer was run, refused' }
    if (@(Runs) -match '^install gog') { throw 'the stand-in installer ran as GOG Galaxy' }
    Start-Sleep -Milliseconds 1500
    Save-Screen '05-gog-refused'

    # --- Battle.net removed by its own uninstaller, after asking what happens to its games.
    Walk-To Down 'Battle.net'
    Press A 'Settings: Set up this PC: Battle\.net; installed; asking Remove Battle\.net\?' '06-remove-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'Settings: Set up this PC: Battle\.net; (removing|not installed)' | Out-Null
    $uninstaller = [regex]::Escape('standin-uninstall-battlenet.exe')
    Expect-Logged "^set-up: battlenet: running $uninstaller$" $mark 'Its own uninstaller' | Out-Null
    Expect-Logged '^set-up: battlenet: removed$' $mark 'Gone from the list' 90 | Out-Null
    Expect-Logged '^focus: Settings: Set up this PC: Battle\.net; not installed$' $mark 'The page says not installed' | Out-Null
    Expect-Logged '^launchers: update: reading battlenet$' $mark 'Battle.net read again' | Out-Null
    if ((@(Runs) | Select-Object -Last 1) -ne 'uninstall battlenet') { throw "the stand-in uninstaller was run as '$(@(Runs) | Select-Object -Last 1)'" }
    if (Select-String -Path $list -Pattern 'Battle\.net' -Quiet) { throw "the test's list still has Battle.net's key" }
    Start-Sleep -Milliseconds 1500
    Save-Screen '07-removed'
    Press B 'Settings: Games & apps: Set up this PC' | Out-Null

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
    foreach ($name in @('standin-setup-battlenet', 'standin-uninstall-battlenet')) {
        Get-Process $name -ErrorAction SilentlyContinue | Where-Object { $_.Path -and ((Test-OwnPath $_.Path) -or $_.Path.StartsWith($sandbox, [StringComparison]::OrdinalIgnoreCase) -or $_.Path.StartsWith($data, [StringComparison]::OrdinalIgnoreCase)) } | Stop-Process -Force -ErrorAction SilentlyContinue
    }
    Remove-Item Env:STANDIN_SETUP, Env:STANDIN_SETUP_MS -ErrorAction SilentlyContinue
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
