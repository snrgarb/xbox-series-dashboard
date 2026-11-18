# Install from (#249, docs/spec/install-from.md), walked with the virtual pad: the same game owned on Steam and on Epic,
# neither installed. The fake Steam is the standard one (Quiet Orbit, not installed, the Recent row's fourth game); the
# fake Epic Games Launcher (Initialize-FakeEpic -Owning) owns a Quiet Orbit too, in its catalogue only. Nothing is
# downloaded: Epic's install is its launcher's link, which the stand-in launcher only says it was asked (`epic-url:
# asked to install <app>`), as stores-epic-gog has it.
#
# What it walks:
# - The Dashboard reads both and the drives, and says which stores can install the game it has from both:
#   `storage: Quiet Orbit can be installed from Steam, Epic Games`.
# - Home's Recent row to Quiet Orbit's Tile menu (`TileMenu: Menu(Install) of Quiet Orbit`), A on Install: "Install
#   from" in the menu's place, focus on Steam, the launcher of the tile pressed (`InstallFrom: Launcher(0) Steam, the
#   one pressed of Quiet Orbit`). Down to Epic Games (`InstallFrom: Launcher(1) Epic Games of Quiet Orbit`), Down again
#   stopping there, B: nothing asked of either store, Home back on the tile.
# - Again, Down to Epic Games and A: Epic's own install begins (`store: asked Epic Games to install Quiet Orbit:
#   com.epicgames.launcher://...?action=install`, the stand-in launcher's `epic-url: asked to install <app>`), and
#   Steam is asked nothing (no `store: asking Steam to install Quiet Orbit`).
#
# Judged by --log-focus a press at a time and the `storage:` and `store:` lines, and the stand-in launcher's line in
# standin.log. Runs on the VM and in safe mode on the TV PC; every fake is the test's own. The screenshots are to look
# at, beside crates/screens/tests/reference/install-from.png.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$name = 'Quiet Orbit'
$nameRx = [regex]::Escape($name)

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: "; then a rest for the picture, if one is asked for. A press that goes elsewhere ends the walk.
function Press([string] $Button, [string] $Expect, [string] $Picture = '') {
    Start-Sleep -Milliseconds 150
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if (-not $got) { Say "$Button -> NOT '$Expect': $(Focus-Now)"; throw "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'" }
    Say "$Button -> $got"
    if ($Picture) { Start-Sleep -Milliseconds 2500; Save-Screen $Picture }
    $got
}

# A press that must move focus nowhere: no new focus line within a second and a half.
function Press-Stays([string] $Button) {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    Start-Sleep -Milliseconds 1500
    if (@(Focus-Lines).Count -ne $before) { $script:failures += "$Button should have stopped, and focus went to '$(Focus-Now)'" } else { Say "$Button -> stays" }
}

# A line the Dashboard must log after the mark $After, by this pattern, within $Seconds.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 15) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if ($line) { Say "${What}: $line" } else { Say "${What}: no line like '$Pattern'"; $script:failures += "${What}: the Dashboard logged no '$Pattern'" }
    $line
}

# A line the Dashboard must not have logged after the mark $After.
function Expect-NotLogged([string] $Pattern, [int] $After, [string] $What) {
    $line = Get-DashboardLog | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -First 1
    if ($line) { Say "${What}: $line"; $script:failures += "${What}: the Dashboard logged '$line'" } else { Say "${What}: none, as it should be" }
}

function Standin-LogLength {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    if (Test-Path $log) { @(Get-Content $log).Count } else { 0 }
}

function Menu-Game([string] $Line) { if ($Line -match ' of (.+)$') { $Matches[1] } }

# From the Recent row's first tile, where focus must be, rightwards: opens each game's Tile menu until one is the menu
# of $Name; leaves it open and returns its line. B on a menu goes back to its tile.
function Open-MenuOf([string] $Name) {
    if ((Focus-Now) -notmatch '^focus: Recent\(0\)$') { throw "the Recent row's walk starts on Recent(0), not '$(Focus-Now)'" }
    foreach ($tile in 0..7) {
        $line = Press Start "TileMenu: Menu\((Play game|Install|Pause installation|Resume installation|Add to Favourites|Remove from Favourites)\) of .+"
        if ((Menu-Game $line) -eq $Name) { return $line }
        Press B "Recent\($tile\)" | Out-Null
        Press Right "Recent\($($tile + 1)\)" | Out-Null
    }
    throw "none of the Recent row's first eight tiles is $Name's"
}

# Back from a menu or question to the Recent row's first tile.
function Back-ToFirst {
    foreach ($press in 1..10) {
        if ((Focus-Now) -match '^focus: Recent\(0\)$') { return }
        Push-Button Left
        Start-Sleep -Milliseconds 500
    }
    throw "Left never reached Recent(0): '$(Focus-Now)'"
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $steamRoot = Get-FakeRoot 'steam'
    $steam = Initialize-FakeSteam $steamRoot
    $quiet = @($steam.games) | Where-Object { $_.name -eq $name } | Select-Object -First 1
    if (-not $quiet -or $quiet.installed) { throw "the fake Steam has no $name that is not installed" }
    $epicRoot = Get-FakeRoot 'epic'
    $epic = Initialize-FakeEpic $epicRoot -Owning $name
    $owned = @($epic.games) | Where-Object { $_.name -eq $name } | Select-Object -First 1
    if (-not $owned -or $owned.installed) { throw "the fake Epic does not own $name, not installed" }
    $app = [regex]::Escape($owned.source_id)
    Say "$name on Steam ($($quiet.source_id)) and on Epic ($($owned.source_id)), neither installed"

    $data = Get-TestFolder 'install-from-data'
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    New-Item $data -ItemType Directory -Force | Out-Null
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $dashboard = Start-Dashboard @('--log-focus', '--steam', "`"$steamRoot`"", '--epic', "`"$epicRoot`"", '--data-dir', "`"$data`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Expect-Logged "^storage: $nameRx can be installed from Steam, Epic Games$" 0 'Both stores can install it' 60 | Out-Null
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 2
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # --- Home starts on the profile: Down is the Recent row's first tile.
    Push-Button Down
    if (-not (Wait-DashboardLog '^focus: Recent\(0\)$' 10)) { throw 'Down did not reach the Recent row' }

    # --- Install from: focus on Steam, the tile's; Down to Epic Games, the foot stopping; B asks nothing.
    $mark = Get-DashboardLogLength
    Open-MenuOf $name | Out-Null
    if ((Focus-Now) -notmatch "^focus: TileMenu: Menu\(Install\) of $nameRx$") { throw "$name's menu has no Install first: '$(Focus-Now)'" }
    Press A "InstallFrom: Launcher\(0\) Steam, the one pressed of $nameRx" '01-install-from' | Out-Null
    Press Down "InstallFrom: Launcher\(1\) Epic Games of $nameRx" '02-install-from-epic' | Out-Null
    Press-Stays Down
    Press B 'Recent\(\d+\)' | Out-Null
    Start-Sleep -Seconds 2
    Expect-NotLogged "^store: ask(ing|ed) .+ to install $nameRx" $mark 'B asked no store'

    # --- Again, and A on Epic Games: Epic's install begins, Steam is asked nothing.
    Back-ToFirst
    Open-MenuOf $name | Out-Null
    Press A "InstallFrom: Launcher\(0\) Steam, the one pressed of $nameRx" | Out-Null
    Press Down "InstallFrom: Launcher\(1\) Epic Games of $nameRx" | Out-Null
    $mark = Get-DashboardLogLength
    $standinMark = Standin-LogLength
    Press A 'Recent\(\d+\)' | Out-Null
    Expect-Logged "^store: asked Epic Games to install ${nameRx}: com\.epicgames\.launcher://apps/.*$app\?action=install$" $mark 'Epic asked to install it' | Out-Null
    $log = Join-Path $env:E2E_OUT 'standin.log'
    $took = Wait-Until { if (Test-Path $log) { Get-Content $log | Select-Object -Skip $standinMark | Where-Object { $_ -match "epic-url: asked to install .*$app" } | Select-Object -Last 1 } } 15
    if ($took) { Say "The stand-in launcher: $took" } else { $failures += "the stand-in Epic launcher was never asked to install $($owned.source_id)" }
    Start-Sleep -Seconds 2
    Save-Screen '03-after-epic'
    Expect-NotLogged "^store: ask(ing|ed) Steam to install $nameRx" $mark 'Steam asked nothing'

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
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
