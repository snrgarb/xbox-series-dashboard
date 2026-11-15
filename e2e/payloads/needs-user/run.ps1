# Needs the user: a game whose launcher stays up, as one asking for a sign-in or showing a EULA does. The Launch screen
# comes up over it, and steps aside once the launcher has held the foreground long enough to count as needing the user
# (5 s of looks that came on time: crates/shell/src/launch.rs), leaving the launcher in front and on screen. When the
# game is closed there, Home comes back.
#
# The game is the stand-in with a launcher that never goes on its own (`--splash-ms` of ten minutes).
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
# Brass Meridian: the Recent row's third game in the fixture, as in the smoke test.
$moves = 2
$dashboardTitle = 'Xbox Series Dashboard'
$overlayTitle = 'Xbox Series Dashboard Overlay'
# The stand-in's windows, the launcher's included: crates/standin/src/win.rs, BLUE.
$gameBlue = @(16, 32, 80)
# Home's Discord tile, which the Launch screen covers and the launcher's small window does not.
$beside = @{ X = 200; Y = 880 }
# Inside the launcher's 400x200 window in the middle of the screen, above its title.
$inLauncher = @{ X = 780; Y = 460 }

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }
function LaunchLog { (Get-DashboardLog | Where-Object { $_ -match '^launch:|^overlay: (shown|hidden)' }) -join ' | ' }

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam
    $recent = @($library.games | Where-Object last_played -gt 0 | Select-Object -First 8)
    $game = $recent[$moves]
    if (-not $game -or -not $game.installed) { throw "The Recent row's game $moves to the right isn't an installed one" }
    $launcherTitle = "$($game.name) launcher"
    Add-Content (Join-Path $game.install_dir 'game-args.txt') @('--splash-ms', '600000') -Encoding Ascii

    $dashboard = Start-Dashboard
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Start-Sleep -Seconds 3
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) {
        $taken = Set-Foreground $dashboardTitle
        Say "note: the Dashboard was not in front after starting; put in front: $taken; foreground $(Front)"
        if (-not $taken) { $failures += "the Dashboard could not be put in front: $(Front)" }
    }
    # Home starts on the profile, and Down is the Recent row's first tile.
    Push-Button Down; Start-Sleep -Milliseconds 400
    1..$moves | ForEach-Object { Push-Button Right; Start-Sleep -Milliseconds 400 }
    Start-Sleep -Milliseconds 600
    if (-not (Wait-DashboardLog '^overlay: configured' 10)) { throw 'The Overlay was not made, so there is no Launch screen to step aside' }
    $home_ = Get-DesignPixel $beside.X $beside.Y
    Say "Home at ($($beside.X), $($beside.Y)): $(Format-Colour $home_)"

    # A: the Launch screen over the launcher.
    Push-Button A
    $up = Wait-DashboardLog 'Launch screen up' 10
    Say "after A: $(if ($up) { $up } else { 'no Launch screen up line' })"
    if (-not $up) { throw "A did not start a launch (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    $front = Wait-Until { (Get-Foreground).Title -eq $launcherTitle } 30
    Say "the launcher in front: $([bool] $front); foreground $(Front)"
    if (-not $front) { $failures += "the game's launcher did not take the foreground: $(Front)" }
    $hidden = Get-DesignPixel $inLauncher.X $inLauncher.Y
    $screen = Get-DesignPixel $beside.X $beside.Y
    Save-Screen 'launch-screen'
    Say "Launch screen: at ($($inLauncher.X), $($inLauncher.Y)) seen $(Format-Colour $hidden), the launcher is $(Format-Colour $gameBlue); at ($($beside.X), $($beside.Y)) seen $(Format-Colour $screen), Home was $(Format-Colour $home_)"
    # Unless it has stepped aside already, the Launch screen hides the launcher and Home.
    if (-not (Wait-DashboardLog 'a window wants the user' 0.1)) {
        if (Test-Colour $hidden $gameBlue 6) { $failures += "the Launch screen does not hide the launcher: seen its colour $(Format-Colour $hidden) at ($($inLauncher.X), $($inLauncher.Y))" }
        if (Test-Colour $screen $home_ 16) { $failures += "the Launch screen is not on screen: Home's colour $(Format-Colour $screen) at ($($beside.X), $($beside.Y))" }
    } else { Say 'note: the Launch screen had stepped aside before it was looked at' }

    # It steps aside for the launcher.
    $aside = Wait-DashboardLog 'a window wants the user, Launch screen stepping aside' 90
    Say "stepping aside: $(if ($aside) { $aside } else { 'no line in 90 s' })"
    if (-not $aside) { throw "The Launch screen never stepped aside (log: $(LaunchLog))" }
    $gone = Wait-DashboardLog 'Launch screen gone' 20
    Say "Launch screen: $(if ($gone) { $gone } else { 'not gone in 20 s' })"
    if (-not $gone) { $failures += 'the Launch screen did not go after stepping aside' }
    $shows = Wait-Until { Test-Colour (Get-DesignPixel $inLauncher.X $inLauncher.Y) $gameBlue 6 } 6
    Save-Screen 'launcher'
    $seen = Get-DesignPixel $inLauncher.X $inLauncher.Y
    $screen = Get-DesignPixel $beside.X $beside.Y
    Say "after it: at ($($inLauncher.X), $($inLauncher.Y)) seen $(Format-Colour $seen); at ($($beside.X), $($beside.Y)) seen $(Format-Colour $screen); foreground $(Front)"
    if (-not $shows) { $failures += "the launcher is not on screen after the Launch screen stepped aside: at ($($inLauncher.X), $($inLauncher.Y)) seen $(Format-Colour $seen), the launcher is $(Format-Colour $gameBlue)" }
    if (Test-Colour $screen $home_ 16) { $failures += "Home shows beside the launcher, where it should have hidden: $(Format-Colour $screen) at ($($beside.X), $($beside.Y))" }
    if ((Get-Foreground).Title -ne $launcherTitle) { $failures += "the launcher is not in front after the Launch screen stepped aside: $(Front)" }
    if ((Get-Foreground).Title -eq $overlayTitle) { $failures += 'the Overlay has the foreground' }
    if (Wait-DashboardLog 'the game has booted' 0.1) { $failures += 'the launcher counted as the game booting' }

    # Closed at its launcher: Home again.
    Stop-StandInGame
    Say 'stopped the game at its launcher'
    $ended = Wait-DashboardLog 'the game has ended, Home back' 20
    Start-Sleep -Milliseconds 900
    Save-Screen 'home-again'
    Say "after the game: $(if ($ended) { $ended } else { 'no game has ended line in 20 s' }); foreground $(Front)"
    if (-not $ended) { $failures += 'Home did not come back after the game was closed at its launcher' }
    if ((Get-Foreground).Title -ne $dashboardTitle) { $failures += "the Dashboard is not in front after the game: $(Front)" }
    if ($dashboard.HasExited) { $failures += "the Dashboard exited with $($dashboard.ExitCode)" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
    Say "at the error: foreground $(Front); launch log: $(LaunchLog)"
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
