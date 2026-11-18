# The PC Game Pass catalogue (#277, docs/spec/launchers.md 8, docs/sources.md "Xbox"): one Dashboard on the fake Xbox
# drive and a local catalogue, walked with the virtual pad. No request goes to Microsoft: `--gamepass <dir>` names the
# catalogue `standin make-gamepass` wrote (US: Tidewater Trials, Saltmarsh Run, Ember Orchard; GB: those and Kestrel
# Line; crates/source-xbox/src/fixture.rs, GAME_PASS), and DASHBOARD_WINDOWS_REGION=US stands in for Windows' region so
# the run is the same on any PC (Windows' real region is said in the log all the same).
#
# What it does, in order:
# - Reads Xbox with the catalogue: `gamepass: 3 games for US (en-US) from <dir>`, `xbox: 5 games from ... and the PC
#   Game Pass catalogue for US (3 games: 1 installed, 2 not) in <ms> ms`.
# - Search finds Saltmarsh Run (not installed) once, and Tidewater Trials (installed, and in the catalogue) once: not
#   doubled.
# - Settings > Games & apps > Launchers, Down to Game Pass region (`...; Game Pass on, US (Windows')`), A opens the
#   list on "Windows' region (United States)", Down to "United Kingdom", A: `settings: gamepass.region = GB`, Xbox read
#   again (`gamepass: 4 games for GB ...`), and Search finds Kestrel Line.
# - Up to Show Game Pass games, A: `settings: gamepass.shown = false`, Xbox read again with none: Search finds no
#   Saltmarsh Run, and Tidewater Trials still once. A again: they are back.
# - Saltmarsh Run's Game hub: its main button is Install (`GameHub: <id> Hub Install`); A asks the Store's page,
#   `store: asked Xbox to install Saltmarsh Run: ms-windows-store://pdp/?ProductId=9ZZZGPASS101`, which the stand-in
#   takes (`open-address: asked to open ms-windows-store://pdp/?ProductId=9ZZZGPASS101` in standin.log): the real
#   Store never opens.
# Judged by the `--log-focus` lines and the `gamepass:`, `xbox:`, `settings:` and `store:` lines; a picture at each rest.
#
# Safe mode (the TV PC, E2E_SAFE=1): the fakes and the data folder under the payload's folder, every other store an
# empty folder, store addresses to `standin open-address`; nothing registered. It ends with the data folder removed.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
Add-Type -AssemblyName System.Windows.Forms
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
$data = Get-TestFolder 'dashboard-gamepass-data'
$catalogue = Get-FakeRoot 'gamepass'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

function Picture([string] $Name) {
    Start-Sleep -Milliseconds 2500
    Save-Screen $Name
}

# One press that must be followed by a new focus line matching $Expect (what follows "focus: "); pressed once more if
# the Dashboard never saw it. Returns the line.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    Start-Sleep -Milliseconds 150
    $before = @(Focus-Lines).Count
    $pattern = "^focus: $Expect`$"
    Push-Button $Button
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Start-Sleep -Milliseconds 150
        Push-Button $Button
        $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    }
    if ($Name) { Picture $Name }
    if ($got) { Say "$Button -> $got" } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# Presses $Button until the last focus line matches $Expect, $Most times at most.
function Walk([string] $Button, [string] $Expect, [int] $Most = 12, [int] $Pause = 700) {
    $pattern = "^focus: $Expect`$"
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match $pattern) { break }
        Push-Button $Button
        Start-Sleep -Milliseconds $Pause
    }
    if ((Focus-Now) -match $pattern) { Say "$Button until $(Focus-Now)" } else {
        Say "$Button never reached '$Expect': $(Focus-Now)"
        $script:failures += "$Button never reached 'focus: $Expect': '$(Focus-Now)'"
    }
}

function Walk-ToEdge([string] $Button, [int] $Most = 14) {
    foreach ($press in 1..$Most) {
        $was = Focus-Now
        Push-Button $Button
        Start-Sleep -Milliseconds 1200
        if ((Focus-Now) -eq $was) { break }
    }
}

function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 20) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if ($line) { Say "${What}: $line" } else { Say "${What}: no line like '$Pattern'"; $script:failures += "${What}: the Dashboard logged no '$Pattern'" }
    $line
}

function Send-Text([string] $Text) {
    [System.Windows.Forms.SendKeys]::SendWait(($Text -replace '([+^%~(){}\[\]])', '{$1}'))
}

# How many games Search finds for $Name typed whole; with -Open, the first result's Game hub is left open.
function Search-Count([string] $Name, [string] $Picture = '', [switch] $Open) {
    Press Y 'Search: Key\(Q\); .+' | Out-Null
    Send-Text $Name
    Start-Sleep -Milliseconds 2500
    $line = Focus-Now
    if ($Picture) { Picture $Picture }
    $found = if ($line -match ', Games (\d+)') { [int] $Matches[1] } else { 0 }
    Say "Search for '$Name': $found games ($line)"
    if ($Open -and $found -gt 0) {
        Press B "Search: Games\(0\) $([regex]::Escape($Name)); .+" | Out-Null
        return $found
    }
    Walk B '(?!Search: |TextEntry: ).+' 4
    $found
}

function Go-Launchers {
    Walk-ToEdge Up 4
    Walk-ToEdge Left 6
    Walk Right 'TopBar\(Settings\)' 5
    Press A 'Settings: Rail: General' | Out-Null
    Press Down 'Settings: Rail: Account' | Out-Null
    Press Down 'Settings: Rail: Games & apps' | Out-Null
    Press Right 'Settings: Games & apps: Launchers' | Out-Null
    Press A 'Settings: Launchers: .+' | Out-Null
}

function Leave-Launchers {
    Press B 'Settings: Games & apps: Launchers' | Out-Null
    Press B 'Settings: Rail: Games & apps' | Out-Null
    Press B 'TopBar\(Settings\)' | Out-Null
    Press Down '.+' | Out-Null
}

try {
    Close-WindowsPrompts
    Connect-Pad

    $xboxRoot = Get-FakeRoot 'xbox'
    Initialize-FakeXbox $xboxRoot | Out-Null
    if (Test-Path $catalogue) { Remove-Item $catalogue -Recurse -Force }
    $made = Start-Process (Join-Path $PSScriptRoot 'standin.exe') -ArgumentList @('make-gamepass', "`"$catalogue`"") -Wait -PassThru
    if ($made.ExitCode -ne 0) { throw "standin make-gamepass exited with $($made.ExitCode)" }
    Say "the local catalogue at $catalogue"

    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    New-Item -ItemType Directory $data -Force | Out-Null
    $env:DASHBOARD_WINDOWS_REGION = 'US en-US'
    # Install as the Store's page, the route this test is for: the Store's queue (#173) is xbox-install's.
    $env:DASHBOARD_XBOX_QUEUE = 'off'
    $arguments = @('--log-focus', '--data-dir', "`"$data`"", '--xbox', "`"$xboxRoot`"", '--gamepass', "`"$catalogue`"",
                   '--open-addresses-with', "`"$(Join-Path $PSScriptRoot 'standin.exe')`"")
    $script:dashboard = Start-Dashboard $arguments
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Expect-Logged "^gamepass: Windows' region is US, its language en-US$" 0 'Windows region (the stand-in)' | Out-Null
    Expect-Logged '^gamepass: 3 games for US \(en-US\) from ' 0 'the US catalogue' 60 | Out-Null
    Expect-Logged '^xbox: \d+ games from .+ and the PC Game Pass catalogue for US \(3 games: 1 installed, 2 not\) in \d+ ms$' 0 'Xbox read with it' 60 | Out-Null
    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }

    # --- Listed, and the installed one once.
    if ((Search-Count 'Saltmarsh Run' '01-search-saltmarsh') -ne 1) { $failures += 'Search should find Saltmarsh Run once' }
    if ((Search-Count 'Tidewater Trials') -ne 1) { $failures += 'Tidewater Trials, installed and in the catalogue, should be found once' }
    if ((Search-Count 'Kestrel Line') -ne 0) { $failures += "Kestrel Line is GB's alone, and Search found it for US" }

    # --- The region, by pad.
    Go-Launchers
    Walk Down "Settings: Launchers: Show Game Pass games; .+; Game Pass on, US \(Windows'\)" 14
    Press Down "Settings: Launchers: Game Pass region; .+; Game Pass on, US \(Windows'\)" '02-launchers-game-pass' | Out-Null
    Press A "Settings: Launchers: Game Pass region, list on `"Windows' region \(United States\)`"; .+" '03-region-list' | Out-Null
    Walk Down 'Settings: Launchers: Game Pass region, list on "United Kingdom"; .+' 90 350
    $mark = Get-DashboardLogLength
    Press A 'Settings: Launchers: Game Pass region; .+; Game Pass on, GB' '04-region-gb' | Out-Null
    Expect-Logged '^settings: gamepass\.region = GB$' $mark 'the region saved' | Out-Null
    Expect-Logged '^gamepass: 4 games for GB \(en-US\) from ' $mark "GB's catalogue read" 60 | Out-Null
    Expect-Logged '^xbox: \d+ games from .+ \(4 games: 1 installed, 3 not\) in \d+ ms$' $mark 'Xbox read again' 60 | Out-Null
    Leave-Launchers
    if ((Search-Count 'Kestrel Line' '05-search-kestrel') -ne 1) { $failures += 'GB chosen, and Search does not find Kestrel Line' }

    # --- Off, then on.
    Go-Launchers
    Walk Down 'Settings: Launchers: Show Game Pass games; .+; Game Pass on, GB' 14
    $mark = Get-DashboardLogLength
    Press A 'Settings: Launchers: Show Game Pass games; .+; Game Pass off, GB' '06-game-pass-off' | Out-Null
    Expect-Logged '^settings: gamepass\.shown = false$' $mark 'off saved' | Out-Null
    Expect-Logged '^gamepass: Show Game Pass games is off: none listed$' $mark 'Xbox read with none' 60 | Out-Null
    Leave-Launchers
    if ((Search-Count 'Saltmarsh Run') -ne 0) { $failures += 'Show Game Pass games is off, and Search still finds Saltmarsh Run' }
    if ((Search-Count 'Tidewater Trials') -ne 1) { $failures += 'Show Game Pass games is off, and Tidewater Trials (installed) is not found once' }
    Go-Launchers
    Walk Down 'Settings: Launchers: Show Game Pass games; .+; Game Pass off, GB' 14
    $mark = Get-DashboardLogLength
    Press A 'Settings: Launchers: Show Game Pass games; .+; Game Pass on, GB' | Out-Null
    Expect-Logged '^settings: gamepass\.shown = true$' $mark 'on saved' | Out-Null
    Expect-Logged '^xbox: \d+ games from .+ \(4 games: 1 installed, 3 not\) in \d+ ms$' $mark 'Xbox read with them again' 60 | Out-Null
    Leave-Launchers

    # --- Install: the Store's page by its product id, the stand-in taking it.
    Search-Count 'Saltmarsh Run' -Open | Out-Null
    Press A 'GameHub: \d+ Hub Install' '07-game-hub-install' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged '^store: asked Xbox to install Saltmarsh Run: ms-windows-store://pdp/\?ProductId=9ZZZGPASS101$' $mark 'Install' | Out-Null
    $standin = Join-Path $script:Out 'standin.log'
    $taken = Wait-Until { Get-Content $standin -ErrorAction SilentlyContinue | Where-Object { $_ -match 'open-address: asked to open ms-windows-store://pdp/\?ProductId=9ZZZGPASS101' } | Select-Object -First 1 } 15
    if ($taken) { Say "the stand-in took it: $taken" } else { $failures += 'the stand-in was never asked to open the Store page of 9ZZZGPASS101' }

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Dashboard
    Disconnect-Pad
    Remove-Item Env:\DASHBOARD_WINDOWS_REGION, Env:\DASHBOARD_XBOX_QUEUE -ErrorAction SilentlyContinue
    if (Test-Path $data) { Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
