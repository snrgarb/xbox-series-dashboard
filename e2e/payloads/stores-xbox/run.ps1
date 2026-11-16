# Xbox (#76): a packaged game, found as the Dashboard finds one on a real machine, launched from its Game hub by its
# app user model ID, and Home back when it ends.
#
# On the VM: `standin make-xbox` writes a fake drive of Xbox games (crates/source-xbox/src/fixture.rs), whose Tidewater
# Trials is a whole package of loose files: its appxmanifest.xml, its MicrosoftGame.config, plain-colour logos made by
# the stand-in, and the stand-in game as its application's game.exe. Nothing of it is in the repository: it is written
# here, at run time. The test registers it for this user with `Add-AppxPackage -Register`, which takes loose files
# only in developer mode (turned on for the run if it is off, and back off after), and needs no certificate. The
# Dashboard then reads this machine as it would any (no --xbox): the package manager lists the package, its
# MicrosoftGame.config makes it a game, and A on Play opens `shell:AppsFolder\<family name>!Game`, which Windows
# activates: the stand-in game runs from the package's folder, as a packaged game does. The package is removed at the
# end.
#
# If the package cannot be registered (developer mode cannot be turned on, or Windows refuses the package), the test
# says why, and the real package's activation moves to the TV PC's list; the rest runs on the fake drive (--xbox <dir>)
# with --open-addresses-with standin.exe: the games are read, the game's Game hub opens, and A on Play gives its
# `shell:AppsFolder\<AUMID>` to `standin open-address`, which starts the fake's game from its package's folder as
# Windows would, and Home comes back when it ends.
#
# In safe mode (E2E_SAFE=1, the TV PC) nothing is registered and developer mode is never touched: the fake drive only,
# under the payload's folder, the Dashboard started on it by Start-Dashboard.
#
# Judged by the Dashboard's log lines and --log-focus's after each press, a press at a time (taps 150 ms apart). No
# position in a row or on the keyboard is counted: a walk goes until the log names the key or the game.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$notes = @()

$root = Get-FakeRoot 'xbox'
$gameName = 'Tidewater Trials'
$identity = 'MadeUpStudio.TidewaterTrials'
# Its first letter, and the key that starts that letter's row on the keyboard (Q W E R T Y ...).
$letter = 'T'
$row = 'Q'
$dashboardTitle = 'Xbox Series Dashboard'
$unlockKey = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock'
$script:package = $null
$script:developerModeTurnedOn = $false

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One tap, and the rest after it that keeps the next from running into it: two taps closer than a poll of the pad are
# one press.
function Tap([string] $Button) {
    Push-Button $Button
    Start-Sleep -Milliseconds 150
}

# Waits for a focus line matching $Pattern (what follows "focus: ", a regular expression) among those after the first
# $Before; returns it, or $null after $Seconds.
function Wait-Focus([int] $Before, [string] $Pattern, [double] $Seconds) {
    Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before -and $lines[-1] -match "^focus: $Pattern`$") { $lines[-1] } } $Seconds
}

# One press that must be followed by a new focus line matching $Expect. A press the Dashboard never saw (no new focus
# line at all) is pressed once more; one that gave another line is a failure. Returns the line.
function Press([string] $Button, [string] $Expect) {
    $before = @(Focus-Lines).Count
    Tap $Button
    $got = Wait-Focus $before $Expect 8
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Tap $Button
        $got = Wait-Focus $before $Expect 8
    }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# Presses $Button until the focus log matches $Pattern (a whole focus line), $Most times at most, then $Back until it
# does: rows and keys do not wrap, so going on to the edge and coming back finds what was passed.
function Walk-To([string] $Button, [string] $Back, [string] $Pattern, [string] $What, [int] $Most = 12) {
    foreach ($way in @($Button, $Back)) {
        foreach ($press in 1..$Most) {
            if ((Focus-Now) -match $Pattern) { break }
            Tap $way
            Start-Sleep -Milliseconds 650
        }
        if ((Focus-Now) -match $Pattern) { break }
    }
    if ((Focus-Now) -match $Pattern) {
        Say "walked to ${What}: $(Focus-Now)"
        return $true
    }
    Say "never reached ${What}: $(Focus-Now)"
    $script:failures += "never reached ${What}: '$(Focus-Now)'"
    return $false
}

function Walk-ToKey([string] $Button, [string] $Back, [string] $Key) {
    Walk-To $Button $Back "^focus: Search: Key\($([regex]::Escape($Key))\);" "the key $Key" | Out-Null
}

# Registers the fake's whole package, at $Content, for this user. Returns $null when it is registered (with
# $script:package the package Windows lists), else why it is not.
function Register-TestPackage([string] $Content) {
    $on = (Get-ItemProperty $unlockKey -Name AllowDevelopmentWithoutDevLicense -ErrorAction SilentlyContinue).AllowDevelopmentWithoutDevLicense -eq 1
    Say "developer mode: $(if ($on) { 'on' } else { 'off' })"
    if (-not $on) {
        try {
            Set-RegistryValue $unlockKey AllowDevelopmentWithoutDevLicense 1 'DWord'
            $script:developerModeTurnedOn = $true
            Say 'developer mode turned on for the run'
        } catch {
            return "developer mode is off, and this user could not turn it on ($($_.Exception.Message)); Windows registers a package of loose files only in developer mode"
        }
    }
    $manifest = Join-Path $Content 'appxmanifest.xml'
    try {
        Add-AppxPackage -Register $manifest -ErrorAction Stop
    } catch {
        $why = $_.Exception.Message -replace '\s+', ' '
        return "Add-AppxPackage -Register $manifest was refused: $why"
    }
    $script:package = Get-AppxPackage -Name $identity | Select-Object -First 1
    if (-not $script:package) { return "Add-AppxPackage -Register said nothing wrong, and Get-AppxPackage does not list $identity" }
    Say "registered: $($script:package.PackageFullName) at $($script:package.InstallLocation)"
    return $null
}

function Unregister-TestPackage {
    if ($script:package) {
        try {
            Remove-AppxPackage $script:package.PackageFullName -ErrorAction Stop
            Say "removed $($script:package.PackageFullName)"
        } catch { Say "note: could not remove $($script:package.PackageFullName): $($_.Exception.Message)" }
    }
    if ($script:developerModeTurnedOn) {
        try { Set-RegistryValue $unlockKey AllowDevelopmentWithoutDevLicense 0 'DWord'; Say 'developer mode back off' } catch {}
    }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    $library = Initialize-FakeXbox $root
    $fakeGames = @($library.games).Count
    $game = @($library.games | Where-Object { $_.name -eq $gameName }) | Select-Object -First 1
    if (-not $game) { throw "The fake Xbox drive has no $gameName" }
    $content = $game.install_dir
    $id = $game.source_id
    Say "the fake's ${gameName}: $id at $content"

    # The real thing where it can be had: the package registered, the machine read.
    $real = $false
    if (Test-SafeMode) {
        Say 'safe mode: no package is registered and developer mode is left alone; the fake drive only'
    } else {
        $why = Register-TestPackage $content
        if ($why) {
            Say "NOT registered: $why"
            $notes += "the package could not be registered on the VM ($why): Windows' activation of a packaged game by its app user model ID moves to the TV PC's list"
        } else {
            $real = $true
            # The family name Windows gave the package is the one the source computes for a fake: the same address.
            if ($script:package.PackageFamilyName -ne $id) {
                $failures += "Windows named the package's family $($script:package.PackageFamilyName), and the source computes $id"
            }
            $id = $script:package.PackageFamilyName
            $content = $script:package.InstallLocation
        }
    }

    $arguments = @('--log-focus')
    if (-not $real -and -not (Test-SafeMode)) {
        # The fake drive, its games' addresses taken by the stand-in (safe mode's Start-Dashboard adds both itself).
        Register-Fake 'xbox' $root 'xbox' -Always
        $arguments += @('--xbox', "`"$root`"", '--open-addresses-with', "`"$(Join-Path $PSScriptRoot 'standin.exe')`"")
    }
    # Nothing in this test is a question put to the user, however slow the VM.
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard $arguments
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"

    # Read: the machine's packages, or the fake drive.
    $readAs = if ($real) { "^xbox: (\d+) games from the user's packages( and .+)? in \d+ ms$" } else { "^xbox: $fakeGames games from $([regex]::Escape($root)) in \d+ ms$" }
    $read = Wait-DashboardLog $readAs 60
    if ($read) { Say "read: $read" } else {
        $said = @(Get-DashboardLog | Where-Object { $_ -match '^xbox: ' }) -join ' | '
        $failures += "the library did not read the Xbox games as '$readAs': '$said'"
    }

    Start-Sleep -Seconds 4
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    Press Down 'Recent\(0\)' | Out-Null

    # Search, its first letter typed: down the keyboard's first column to its row, along the row to it.
    $escaped = [regex]::Escape($gameName)
    $opened = Press Y 'Search: Key\(Q\); "\|" upper letters, RecentlyAdded \d+'
    if (-not $opened) { throw "Search did not open" }
    Walk-ToKey Down Up $row
    Walk-ToKey Right Left $letter
    $typed = Press A "Search: Key\($letter\); `"$letter\|`" lower letters, Games \d+"
    $found = if ($typed -match ', Games (\d+)$') { [int] $Matches[1] } else { 12 }

    # B: the keyboard away, focus on the first game found; along the row to the game, by its name.
    Press B "Search: Games\(0\) .+; `"$letter`" away, Games \d+" | Out-Null
    if (-not (Walk-To Right Left "^focus: Search: Games\(\d+\) $escaped; " $gameName ($found + 1))) { throw "$gameName is not among the games Search found" }

    # A: its Game hub, focus on Play. Its cover is the package's largest square logo; the mark beside it, Xbox's plus.
    $hub = Press A "GameHub: $([regex]::Escape($id)) Hub Play"
    if (-not $hub) { throw "$gameName's Game hub did not open" }
    Start-Sleep -Milliseconds 4500
    Save-Screen 'xbox-hub'

    # A on Play: Windows is asked for the app user model ID, and the Launch screen comes up.
    $mark = Get-DashboardLogLength
    Tap A
    $asked = "^launch: $escaped \($([regex]::Escape($id))\) asked of Xbox, Launch screen up at \d+ ms$"
    $up = Wait-DashboardLog $asked 20 $mark
    if (-not $up) {
        $said = @(Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^launch: ' }) -join ' | '
        $failures += "$gameName was not launched through Xbox: '$said'"
    } else {
        Say "launched: $up"
        if (-not $real) {
            $log = Join-Path $env:E2E_OUT 'standin.log'
            $by = Wait-Until { if (Test-Path $log) { Get-Content $log | Where-Object { $_ -match "xbox: $([regex]::Escape($id))!Game running" } | Select-Object -Last 1 } } 15
            if ($by) { Say "started by the stand-in: $by" } else { $failures += "standin open-address did not start $gameName from the fake drive" }
        }
        $running = Wait-Until { @(Get-StandInGame | Where-Object { $_.Path -and $_.Path.StartsWith($content.TrimEnd('\', '/'), [StringComparison]::OrdinalIgnoreCase) }).Count } 60
        if (-not $running) { $failures += "no stand-in game ran from the package at $content" } else { Say "the stand-in game runs from the package at $content" }
        $booted = Wait-DashboardLog '^launch: the game has booted, Launch screen fading at \d+ ms$' 90 $mark
        if ($booted) { Say "booted: $booted" } else { $failures += "$gameName never counted as booted" }
        $front = Wait-Until { (Get-Foreground).Title -eq $gameName } 10
        Say "the game in front: $([bool] $front); foreground '$((Get-Foreground).Title)'"
        Start-Sleep -Milliseconds 1500
        Save-Screen 'xbox-game'

        # It ends: Home again.
        Stop-StandInGame
        $ended = Wait-DashboardLog '^launch: the game has ended, Home back at \d+ ms$' 30 $mark
        if ($ended) { Say "ended: $ended" } else { $failures += "Home did not come back after $gameName ended" }
        Start-Sleep -Milliseconds 1500
        Save-Screen 'xbox-home-again'
    }

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $notes | ForEach-Object { Say "for the TV PC: $_" }
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Unregister-TestPackage
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
