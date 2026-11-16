# Power, with the virtual pad: from the profile to the Guide's Profile & system tab, Power from its entry, its tiles,
# an option chosen, Power by a hold of the Guide button, and Exit to desktop. A screenshot at each rest, to set beside
# the Bundle's.
#
# The Dashboard runs with --dry-power: Turn off, Restart and Sleep are a line in its log (`power: would run ...`) and
# nothing else, so the VM stays up. Exit to desktop is real: it is the Dashboard's own exit.
#
# Passes when each press puts the Guide where it should (--log-focus's `guide:` lines), each sound is the one the
# Bundle plays there (--log-sounds, on `standin make-tones`' tones), the dry run logged the Bundle's commands, and
# the Dashboard exited by itself with 0 after the sound of leaving.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

# One press, then the `guide:` line it must bring (a regular expression) and, if named, the sound. A press at a time,
# judged by the log: a press held 120 ms can repeat where the VM's clock jumps.
function Step([string] $Button, [string] $Guide, [string] $Sound = '', [string] $Name = '', [int] $HoldMs = 120) {
    $mark = Get-DashboardLogLength
    Push-Button $Button $HoldMs
    $line = Wait-DashboardLog "^guide: $Guide" 8 $mark
    $heard = if ($Sound) { Wait-DashboardLog "audio: played $Sound" 4 $mark } else { 'none asked' }
    # The software renderer takes most of a second a frame: a picture waits for the slide and the fly-in.
    Start-Sleep -Milliseconds 3000
    if ($Name) { Save-Screen $Name }
    Say "$Button$(if ($Name) { " -> $Name" }): $(if ($line) { $line } else { "NO line 'guide: $Guide'" })"
    if (-not $line) { $script:failures += "$Button did not bring 'guide: $Guide'" }
    if (-not $heard) { $script:failures += "$Button was not followed by $Sound" }
}

# A line the dry run must have logged after $Mark.
function Expect-Log([string] $Pattern, [int] $Mark, [string] $What) {
    $line = Wait-DashboardLog $Pattern 6 $Mark
    Say "${What}: $(if ($line) { $line } else { "NO line '$Pattern'" })"
    if (-not $line) { $script:failures += "$What was not logged ('$Pattern')" }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    $media = Join-Path $Out 'media'
    & (Join-Path $PSScriptRoot 'standin.exe') make-tones $media | Out-Null
    $arguments = @('--media', "`"$media`"", '--log-sounds', '--log-focus', '--dry-power')

    $script:dashboard = Start-Dashboard $arguments
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Start-Sleep -Seconds 4
    # The greeting's Toast comes and goes: it is not in the pictures, and the Guide button is the Guide's again.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # Home starts on the profile, and A on it opens the Guide on Profile & system.
    Step A 'ProfileAndSystem Entry\(0\) Profile$' 'OpenMenu\.wav' '01-profile-and-system'
    Step Down 'ProfileAndSystem Entry\(1\) SwitchProfile$' 'Navigate\d\.wav'
    # Down to Power by its name: the last unlocks stand between Switch profile and Settings while there are
    # achievements (the fake Steam's Ashfall Courier has two), so its place is not fixed.
    for ($press = 0; $press -lt 8; $press++) {
        $mark = Get-DashboardLogLength
        Push-Button Down
        $line = Wait-DashboardLog '^guide: ProfileAndSystem Entry\(\d+\) \w+' 8 $mark
        Say "Down: $line"
        if ($line -match ' Power$') { break }
    }
    if ($line -notmatch ' Power$') { throw "Down never reached Power (the Guide is at: $line)" }
    Start-Sleep -Milliseconds 3000
    Save-Screen '02-power-entry'

    # Power from its entry: Enter, then Prompt open; focus on Close; the panel out of focus behind it.
    $mark = Get-DashboardLogLength
    Step A 'ProfileAndSystem Entry\(\d+\) Power, dialog Power\(Close\)$' 'PromptOpen\.wav' '03-power'
    Expect-Log 'audio: played Enter\.wav' $mark 'Enter before Prompt open'
    Step Up 'ProfileAndSystem Entry\(\d+\) Power, dialog Power\(Tile\(3\)\)$' 'Navigate\d\.wav' '04-power-exit-tile'
    Step Up 'ProfileAndSystem Entry\(\d+\) Power, dialog Power\(Tile\(0\)\)$' 'Navigate\d\.wav' '05-power-turn-off-tile'
    Step Right 'ProfileAndSystem Entry\(\d+\) Power, dialog Power\(Tile\(1\)\)$' 'Navigate\d\.wav'
    Step Right 'ProfileAndSystem Entry\(\d+\) Power, dialog Power\(Tile\(2\)\)$' 'Navigate\d\.wav' '06-power-sleep-tile'

    # B puts Power away and leaves the Guide on its entry.
    Step B 'ProfileAndSystem Entry\(\d+\) Power$' 'PromptClose\.wav' '07-power-closed'

    # Restart, dry: the Guide closes, then the Bundle's command is logged and nothing happens.
    Step A 'ProfileAndSystem Entry\(\d+\) Power, dialog Power\(Close\)$' 'PromptOpen\.wav'
    Step Up '.*dialog Power\(Tile\(3\)\)$'
    Step Up '.*dialog Power\(Tile\(0\)\)$'
    Step Right '.*dialog Power\(Tile\(1\)\)$'
    $mark = Get-DashboardLogLength
    Step A 'closed$' 'Enter\.wav' '08-after-restart-dry'
    Expect-Log 'power: would run shutdown\.exe -r -t 0 \(Restart\)' $mark 'Restart, dry'

    # A hold of the Guide button at Home: Power alone, no panel. Nothing shows before 0.7 s.
    Step Guide 'Landing Entry\(0\), no panel, dialog Power\(Close\)$' 'PromptOpen\.wav' '09-power-held' 1000
    Step Up '.*no panel, dialog Power\(Tile\(3\)\)$'
    Step Up '.*no panel, dialog Power\(Tile\(0\)\)$' '' '10-power-held-turn-off-tile'
    $mark = Get-DashboardLogLength
    Step A 'closed$' 'Enter\.wav'
    Expect-Log 'power: would run shutdown\.exe /s /hybrid /t 0 \(TurnOff\)' $mark 'Turn off, dry'

    # Held again, and Sleep.
    Step Guide '.*no panel, dialog Power\(Close\)$' 'PromptOpen\.wav' '' 1000
    Step Up '.*dialog Power\(Tile\(3\)\)$'
    Step Up '.*dialog Power\(Tile\(0\)\)$'
    Step Right '.*dialog Power\(Tile\(1\)\)$'
    Step Right '.*dialog Power\(Tile\(2\)\)$'
    $mark = Get-DashboardLogLength
    Step A 'closed$' 'Enter\.wav'
    Expect-Log 'power: would run SetSuspendState\(sleep\) \(Sleep\)' $mark 'Sleep, dry'

    # Held with the Guide open: the panel slides away under Power, and B then closes everything.
    Step Guide 'Landing Entry\(0\)$' 'OpenMenu\.wav' '11-guide'
    Step Guide 'Landing Entry\(0\), no panel, dialog Power\(Close\)$' 'PromptOpen\.wav' '12-power-held-over-guide' 1000
    Step B 'closed$' 'PromptClose\.wav' '13-home-again'
    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode) before it was asked to" }

    # Exit to desktop, which is real: the sound of leaving, then the Dashboard gone by itself.
    Step Guide '.*no panel, dialog Power\(Close\)$' 'PromptOpen\.wav' '' 1000
    Step Up '.*dialog Power\(Tile\(3\)\)$' '' '14-power-held-exit-tile'
    $mark = Get-DashboardLogLength
    Push-Button A
    $gone = Wait-Until { $dashboard.HasExited } 15
    Say "Exit to desktop: $(if ($gone) { "exited with $($dashboard.ExitCode)" } else { 'still running after 15 s' })"
    if (-not $gone) { $failures += 'Exit to desktop did not end the Dashboard' }
    elseif ($dashboard.ExitCode -ne 0) { $failures += "Exit to desktop ended it with $($dashboard.ExitCode)" }
    $log = @(Get-DashboardLog | Select-Object -Skip $mark)
    foreach ($pattern in 'audio: played Enter\.wav', 'audio: played PowerOff\.wav', 'power: exit to desktop') {
        $line = $log | Where-Object { $_ -match $pattern } | Select-Object -First 1
        Say "  $(if ($line) { $line } else { "NO line '$pattern'" })"
        if (-not $line) { $failures += "Exit to desktop did not log '$pattern'" }
    }
    Start-Sleep -Seconds 2
    Save-Screen '15-desktop'

    # Nothing but the dry run's three lines ever spoke of the machine's power.
    $real = @(Get-DashboardLog | Where-Object { $_ -match '^power: ' -and $_ -notmatch 'would run|exit to desktop' })
    $real | ForEach-Object { $failures += "a power line that is not a dry run: $_" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
