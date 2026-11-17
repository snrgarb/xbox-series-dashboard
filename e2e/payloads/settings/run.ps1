# Settings, walked with the virtual pad on a fake Steam library: the rail's four categories (General, Account, Games &
# apps, System), General's boxes, the Idle and Guide pages, Games & apps' two sections (Library and Installs), each
# category keeping where focus was, and Personalisation; Background delay, Reduce motion and Show launcher on recent
# games turned over; then the Dashboard killed and started again, to see that all three are as they were left.
#
# Judged by the Dashboard's log, a press at a time. --log-focus says where focus is after every press that moved it
# (`focus: Settings: Personalisation: Reduce motion`); a checkbox turned over is `settings: <key> = <value>` as it is
# saved; and at start-up every option that has been set is `settings: <key> is <value>`. Passes when every press is
# followed by the line expected of it, the second start reads back what the first saved, and the Dashboard stays up.
#
# The pictures are to look at beside the Bundle's of the same number (docs/spec/pictures.md, 28 to 35), and the
# sectioned page beside the console's own Settings (docs/spec/settings.md, 16.1). The categories are the console's,
# not the Bundle's one "Theme settings" (#194).
#
# The settings are in a data folder of the test's own, empty as it starts, so every option starts at its default and
# nothing is left changed for the next test.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-settings-test'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then a rest for the picture, if one is asked for: the software renderer takes most of a second a frame.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($Name) {
        Start-Sleep -Milliseconds 3000
        Save-Screen $Name
    }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
}

# Presses a direction until focus is where $Expect says, $Most times at most: a press held 120 ms can count twice
# where the VM's clock jumps, so a walk is judged by where it ends, not by how many presses it took.
function Walk([string] $Button, [string] $Expect, [int] $Most = 8) {
    $pattern = "^focus: $Expect`$"
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match $pattern) { break }
        Push-Button $Button
        Start-Sleep -Milliseconds 800
    }
    if ((Focus-Now) -match $pattern) {
        Say "$Button until $(Focus-Now)"
    } else {
        Say "$Button never reached '$Expect': $(Focus-Now)"
        $script:failures += "$Button never reached 'focus: $Expect': '$(Focus-Now)'"
    }
}

# Presses a direction until it moves focus no more: the end of a row or a column, which does not wrap. How many
# boxes a pane has is not this test's to say, so its last is found, never counted. It must match $Expect.
function Walk-ToEdge([string] $Button, [string] $Expect, [int] $Most = 10) {
    $reached = $false
    foreach ($press in 1..$Most) {
        $was = Focus-Now
        Push-Button $Button
        Start-Sleep -Milliseconds 1500
        if ((Focus-Now) -eq $was) { $reached = $true; break }
    }
    $at = (Focus-Now) -replace '^focus: ', ''
    if ($reached -and $at -match "^$Expect`$") {
        Say "$Button to the end: $at"
    } else {
        Say "$Button did not end on '$Expect': $at"
        $script:failures += "$Button should have ended on '$Expect', and focus is '$at'"
    }
}

# A on the checkbox focus is on: the option must be saved as $Value, which the log says as it is. Focus stays, so
# there is no focus line to wait for.
function Turn-Over([string] $Key, [string] $Value, [string] $Name = '') {
    $mark = Get-DashboardLogLength
    $focus = Focus-Now
    Push-Button A
    $got = Wait-DashboardLog "^settings: $([regex]::Escape($Key)) = $Value`$" 8 $mark
    if ($Name) {
        Start-Sleep -Milliseconds 3000
        Save-Screen $Name
    }
    if ($got -and (Focus-Now) -eq $focus) {
        Say "A -> $got"
    } else {
        Say "A -> NOT '$Key = $Value': $(Get-DashboardLog | Where-Object { $_ -match '^settings: ' } | Select-Object -Last 1); $(Focus-Now)"
        $script:failures += "A on '$focus' should have saved '$Key = $Value' and left focus where it was"
    }
}

# Starts the Dashboard on the test's data folder and waits for its first frame.
function Start-OnData {
    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"")
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    # The greeting's Toast comes and goes: it is not in the pictures, and the Guide button is the Guide's again.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }
}

# From Home as it starts, focus on the profile, to Settings' rail by the top bar's button.
function Open-Settings([string] $Name = '') {
    Walk Right 'TopBar\(Settings\)' 5
    Press A 'Settings: Rail: General' $Name
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    Start-OnData
    $set = @(Get-DashboardLog | Where-Object { $_ -match '^settings: .* is ' })
    if ($set.Count) { $failures += "a new data folder should have no option set, and the log says: $($set -join '; ')" }

    # The rail: the four categories, each showing beside it as focus lands on it, with no ring. Down to the foot, where
    # it stops, and Up to the top again.
    Open-Settings '28-settings-general'
    Press Down 'Settings: Rail: Account' '28-settings-account'
    Press Down 'Settings: Rail: Games & apps' '28-settings-games-and-apps'
    Press Down 'Settings: Rail: System' '28-settings-system'
    Walk-ToEdge Down 'Settings: Rail: System'
    Walk-ToEdge Up 'Settings: Rail: General'

    # Into General: its first box. Along the row to its end, the Idle box; its page (the header: General, then Idle),
    # and B back on the box. Then down to the Guide box, its page, and back.
    Press Right 'Settings: General: Personalisation' '29-settings-general-box-focused'
    Walk-ToEdge Right 'Settings: General: Idle'
    Press A 'Settings: Idle: Dim after' '33-settings-idle'
    Press B 'Settings: General: Idle'
    Press Down 'Settings: General: Boot video'
    Press Left 'Settings: General: Guide'
    Press A 'Settings: Guide: Use the Guide' '35-settings-guide'
    Press B 'Settings: General: Guide'

    # Games & apps, from the rail: two sections, Library and Installs. Down along Library to its foot, Right across to
    # Installs, as far down as it goes, Left back to Library, Left again to the rail.
    Press B 'Settings: Rail: General'
    Press Down 'Settings: Rail: Account'
    Press Down 'Settings: Rail: Games & apps'
    Press Right 'Settings: Games & apps: Launchers' '29-settings-games-and-apps-sections'
    Walk-ToEdge Down 'Settings: Games & apps: Art'
    Press Right 'Settings: Games & apps: Set up this PC'
    Press Up 'Settings: Games & apps: Helpers'
    Press Up 'Settings: Games & apps: Storage'
    Press Left 'Settings: Games & apps: Launchers'
    Press Left 'Settings: Rail: Games & apps'

    # Each category keeps where focus was: General is still on Guide.
    Walk-ToEdge Up 'Settings: Rail: General'
    Press Right 'Settings: General: Guide'

    # Personalisation: a checkbox box a row, the focused one's info beside it. Background delay off, Reduce motion on.
    Press Up 'Settings: General: Personalisation'
    Press A 'Settings: Personalisation: Background delay' '31-settings-personalisation-checkbox'
    Turn-Over 'home.background_delay' 'false' '31-background-delay-off'
    Press Down 'Settings: Personalisation: Reduce motion'
    Turn-Over 'motion.reduce' 'true' '31-reduce-motion-on'
    # Show launcher icons (#248), beside Show Play history: off, the Recent row's tiles lose their marks.
    Press Down 'Settings: Personalisation: Show Play history'
    Press Down 'Settings: Personalisation: Show launcher icons'
    Turn-Over 'home.recent_launcher' 'false' '31-recent-launcher-off'
    Walk-ToEdge Down 'Settings: Personalisation: Show battery percentages'
    Start-Sleep -Milliseconds 2000
    Save-Screen '31-personalisation-last-box'

    # B, a level at a time: the box that opened the page, the rail, Home on the button that opened Settings.
    Press B 'Settings: General: Personalisation'
    Press B 'Settings: Rail: General'
    Press B 'TopBar\(Settings\)' 'home-after-settings'

    # Killed, not closed: an option is saved as it is turned over. The second start reads both back.
    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    Stop-Dashboard
    Say 'stopped the Dashboard; starting it again on the same data folder'
    Start-OnData
    foreach ($held in @('home.background_delay is false', 'motion.reduce is true', 'home.recent_launcher is false')) {
        if (Get-DashboardLog | Where-Object { $_ -eq "settings: $held" }) {
            Say "held: $held"
        } else {
            Say "NOT held: $held"
            $failures += "after the restart the log should say 'settings: $held'"
        }
    }

    # And the page shows them so: the first box unticked, the second ticked. Then both put back as they were.
    Open-Settings
    Press Right 'Settings: General: Personalisation'
    Press A 'Settings: Personalisation: Background delay' '31-after-restart'
    Turn-Over 'home.background_delay' 'true'
    Press Down 'Settings: Personalisation: Reduce motion'
    Turn-Over 'motion.reduce' 'false' '31-put-back'
    Press Down 'Settings: Personalisation: Show Play history'
    Press Down 'Settings: Personalisation: Show launcher icons'
    Turn-Over 'home.recent_launcher' 'true'


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
