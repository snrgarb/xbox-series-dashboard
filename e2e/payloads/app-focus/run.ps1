# An App and the foreground (docs/spec/apps.md): an App started from My games & apps' Apps comes in front of the
# Dashboard, and Home chosen in the Guide over it puts the Dashboard back in front, the pad Home's again, the App
# running on behind it. A on the App's tile then brings the running App back in front.
#
# The App is a copy of the stand-in game in the payload's own folder (a full-screen window titled "Focus App"), added
# by typing its path in Settings' Games & apps: nothing of the machine's own is started or closed.
#
# Judged by the Dashboard's log (`focus:` and `guide:` lines with --log-focus) and by who has the foreground.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Join-Path $env:TEMP 'dashboard-app-focus-test'
$folder = Join-Path $PSScriptRoot 'focusapp'
$program = Join-Path $folder 'game.exe'
$app = 'Focus App'
$dashboardTitle = 'Xbox Series Dashboard'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }
function Guide-Now { Get-DashboardLog | Where-Object { $_ -match '^guide: ' } | Select-Object -Last 1 }
function Esc([string] $Text) { [regex]::Escape($Text) }
function Front { $fg = Get-Foreground; "$($fg.Process) '$($fg.Title)'" }

function Press([string] $Button, [string] $Expect) {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($got) { Say "$Button -> $got" } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
}

function Walk([string] $Button, [string] $Expect, [int] $Most = 12) {
    $pattern = "^focus: $Expect`$"
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match $pattern) { break }
        Push-Button $Button
        Start-Sleep -Milliseconds 800
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
        Start-Sleep -Milliseconds 1500
        if ((Focus-Now) -eq $was) { break }
    }
}

function Go-TopBar([string] $Button) {
    Walk-ToEdge Up 4
    Walk-ToEdge Left 6
    Walk Right "TopBar\($Button\)" 5
}

function Type-Into([string] $Was, [string] $Text, [string] $Then) {
    Press A 'TextEntry: .+'
    if ($Was.Length) {
        [System.Windows.Forms.SendKeys]::SendWait('{BS}' * $Was.Length)
        Start-Sleep -Milliseconds 1500
    }
    [System.Windows.Forms.SendKeys]::SendWait($Text)
    Start-Sleep -Milliseconds 2000
    Press Start $Then
}

function Typed([string] $Path, [string] $Name, [string] $Ready = 'ready') {
    "; path `"$(Esc $Path)`", name `"$(Esc $Name)`", picture `"`", $Ready"
}

# Who must be in front, by window title, within a few seconds.
function Expect-Front([string] $Title, [string] $When) {
    $ok = Wait-Until { (Get-Foreground).Title -eq $Title } 8
    Say "${When}: foreground $(Front)"
    if (-not $ok) { $script:failures += "${When}: '$Title' is not in front: $(Front)" }
}

# One press, then the `guide:` line it must bring.
function Step([string] $Button, [string] $Guide) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $line = Wait-DashboardLog "^guide: $Guide" 8 $mark
    Say "$Button`: $(if ($line) { $line } else { "NO line 'guide: $Guide' (the Guide is at: $(Guide-Now))" })"
    if (-not $line) { $script:failures += "$Button did not bring 'guide: $Guide'" }
}

function App-Running { @(Get-OwnProcess 'game' | Where-Object { $_.Path -like '*\focusapp\game.exe' }) }

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force }
    New-Item -ItemType Directory $folder -Force | Out-Null
    Copy-Item (Join-Path $PSScriptRoot 'standin.exe') $program
    Set-Content (Join-Path $folder 'game-args.txt') "--title`r`n$app`r`n" -Encoding ASCII
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', $data)
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }

    # --- The App added in Settings > Games & apps.
    Go-TopBar 'Settings'
    Press A 'Settings: Rail: General'
    Press Down 'Settings: Rail: Account'
    Press Down 'Settings: Rail: Games & apps'
    Press Right 'Settings: Games & apps: Launchers'
    Press Down 'Settings: Games & apps: Games & apps'
    Press A 'Settings: GamesAndApps: Add a game; .+'
    Press Down 'Settings: GamesAndApps: Add an app; 0 games, 0 apps'
    Press A "Settings: AddApp: Path$(Typed '' '' 'not ready')"
    Type-Into '' $program "Settings: AddApp: Path$(Typed $program 'game')"
    Press Down "Settings: AddApp: Name$(Typed $program 'game')"
    Type-Into 'game' $app "Settings: AddApp: Name$(Typed $program $app)"
    Walk Down "Settings: AddApp: Add$(Typed $program $app)" 4
    $mark = Get-DashboardLogLength
    Press A 'Settings: GamesAndApps: Add an app; .+'
    if (-not (Wait-DashboardLog "^library: added the App $(Esc $app)`$" 10 $mark)) { throw 'The App was not added' }
    Press B 'Settings: Games & apps: Games & apps'
    Press B 'Settings: Rail: Games & apps'
    Press B 'TopBar\(Settings\)'

    # --- My games & apps > Apps, on the App's tile.
    Go-TopBar 'MyGames'
    Press A 'MyGames: Games: Tile\(0\)'
    Press Left 'MyGames: Games: Nav\(Games\)'
    Press Down 'MyGames: \w+: Nav\(Apps\)(; .*)?'
    Start-Sleep -Milliseconds 2000
    Press Right "MyGames: Apps: Tile\(0\); `"$(Esc $app)`", 1 apps"
    Expect-Front $dashboardTitle 'before the App starts'

    # --- 1. A starts it: the App in front.
    $mark = Get-DashboardLogLength
    Push-Button A
    if (-not (Wait-DashboardLog "^apps: started $(Esc $app): " 10 $mark)) { throw 'The App was not started' }
    if (-not (Wait-Until { (App-Running).Count -gt 0 } 15)) { throw 'The App never ran' }
    Expect-Front $app '1. the App started'
    Save-Screen '01-app-started'

    # --- 2. The Guide over it, then Home: the Dashboard in front, the App running behind, the pad Home's.
    Step Guide 'Landing Entry\(0\)$'
    Step A 'closed$'
    Expect-Front $dashboardTitle '2. Home from the Guide over the App'
    Save-Screen '02-home-over-the-app'
    if ((App-Running).Count -eq 0) { $failures += '2. the App is no longer running behind Home' }
    $before = @(Focus-Lines).Count
    Push-Button Left
    Start-Sleep -Milliseconds 1500
    if (@(Focus-Lines).Count -le $before) { $failures += "2. the pad is not Home's: Left moved nothing ($(Focus-Now))" } else { Say "2. Left at Home: $(Focus-Now)" }

    # --- 3. Back on the App's tile, A brings the running App in front again.
    Walk Right "MyGames: Apps: Tile\(0\); .+" 3
    Push-Button A
    Expect-Front $app '3. A on the running App'

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    App-Running | Stop-Process -Force -ErrorAction SilentlyContinue
    Stop-Dashboard
    Disconnect-Pad
    if (Test-Path $data) { Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue }
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
