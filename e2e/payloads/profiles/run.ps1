# Profiles (docs/spec/profiles.md), walked with the virtual pad in a data folder of the test's own: the first Profile
# made from the Windows user; a Profile added by its gamertag through text entry, renamed, given a real name and a
# plain picture; the Windows user's Profile removed after its question, which signs the new one in (Who's playing
# today?, which will switch Profiles, is #127's: until then removing the one signed in is the switch); Home's top bar
# and the Guide's Profile & system seen to follow; the Dashboard started again and the new Profile still signed in,
# greeted by its real name's first word, and with no Remove profile now that it is the only one.
#
# Judged by the Dashboard's log. `profile: "<gamertag>", real name "<real name>", <picture>` is the Profile signed in
# as Home's top bar, the Guide and the greeting draw it, said with the first frame and whenever it changes;
# `profiles: ...` is each change carried out on the database; --log-focus's `focus: Settings: Profiles: ...`,
# `Profile: ...` and `Picture: ...` say where focus is, the Profile's page adding the Profile as it is now after a
# `; `; `guide: ProfileAndSystem Entry(0)` is the Guide open on the profile entry.
#
# The pictures are to look at beside the Bundle's Personalisation (docs/spec/pictures.md, 30); the Bundle has no
# picture of its Profiles pages. Home's top bar and the Guide must show the new gamertag and picture.
#
# The data folder is the test's own, empty as it starts, and is removed at the end.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-profiles-test'
$added = 'Moth Runner'
$renamed = 'Moth Rider'
$realName = 'Ann Smith'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }
function Profile-Now { Get-DashboardLog | Where-Object { $_ -match '^profile: ' } | Select-Object -Last 1 }

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
# where the VM's clock jumps, and one can be lost, so a walk is judged by where it ends, not by how many presses.
function Walk([string] $Button, [string] $Expect, [int] $Most = 12) {
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

# A log line after the first $After lines matching $Pattern, waited for; a failure named $What if none comes.
function Expect-Log([string] $Pattern, [int] $After, [string] $What) {
    $line = Wait-DashboardLog $Pattern 10 $After
    Say "${What}: $(if ($line) { $line } else { "NO line '$Pattern'" })"
    if (-not $line) { $script:failures += "$What should have logged '$Pattern' (the last profile line is '$(Profile-Now)')" }
    $line
}

# The Profile signed in must be this one, as the `profile:` line says after the first $After lines.
function Expect-Profile([string] $Gamertag, [string] $Real, [string] $Picture, [int] $After, [string] $What) {
    $want = "profile: `"$Gamertag`", real name `"$Real`", $Picture"
    Expect-Log "^$([regex]::Escape($want))`$" $After $What | Out-Null
}

# Text entry is up: what it starts from is rubbed out ($Length Backspaces), $Text typed on the keyboard, and Menu
# keeps it, which must put focus where $Expect says.
function Type-Over([int] $Length, [string] $Text, [string] $Expect, [string] $Name = '') {
    if ($Length) { [System.Windows.Forms.SendKeys]::SendWait('{BS}' * $Length) }
    Start-Sleep -Milliseconds 1500
    [System.Windows.Forms.SendKeys]::SendWait($Text)
    Start-Sleep -Milliseconds 2000
    Say "typed: $(Focus-Now)"
    if ($Name) { Save-Screen $Name }
    Press Start $Expect
}

# Started on the test's data folder; the greeting comes and goes, so that the Guide button is the Guide's again.
function Start-Here {
    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"")
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }
}

# From Home's top bar into Settings > Personalisation > Profiles, focus on its first Profile.
function Open-Profiles([string] $First, [string] $Name = '') {
    Walk Right 'TopBar\(Settings\)' 5
    Press A 'Settings: Rail: Theme settings'
    Press Right 'Settings: Theme settings: Personalisation'
    Press A 'Settings: Personalisation: Boot video'
    Press Right 'Settings: Personalisation: Profiles' $Name
    Press A "Settings: Profiles: $([regex]::Escape($First))"
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }

    # --- The first Profile, made from the Windows user as the database is made.
    Start-Here
    $made = Expect-Log '^profiles: made \d+ ".+" from the Windows user, wearing (its account picture|a plain picture)$' 0 'the first Profile'
    $first = Expect-Log '^profile: ' 0 'the Profile signed in'
    if (-not ($first -match '^profile: "(.+)", real name "", ')) { throw "the first Profile is not the Windows user's: '$first'" }
    $me = $Matches[1]
    $myPicture = ($first -replace '^profile: ".+", real name "", ', '')
    Say "the Windows user's Profile: $me, wearing $myPicture"
    Wait-Greeting | Out-Null
    $greeted = Wait-DashboardLog "^toasts: shown `"Hi $([regex]::Escape($me))!`"" 5
    if (-not $greeted) { $failures += "the greeting should have said 'Hi $me!'" }
    Save-Screen '01-home'

    Open-Profiles $me '02-personalisation-profiles'
    Save-Screen '03-profiles'

    # --- Add profile: a gamertag typed and kept, the new Profile last and focus on it.
    Press Right 'Settings: Profiles: Add profile'
    Press A 'TextEntry: .+'
    $mark = Get-DashboardLogLength
    Type-Over 0 $added "Settings: Profiles: $([regex]::Escape($added))" '04-add-profile-typed'
    Expect-Log "^profiles: added \d+ `"$([regex]::Escape($added))`"$" $mark "adding $added" | Out-Null
    Start-Sleep -Milliseconds 2000
    Save-Screen '05-profile-added'

    # --- Its page: renamed, a real name, a plain picture.
    Press A "Settings: Profile: Gamertag; `"$([regex]::Escape($added))`", real name `"`", shape 0" '06-profile-page'
    Press A 'TextEntry: .+'
    $mark = Get-DashboardLogLength
    Type-Over $added.Length $renamed "Settings: Profile: Gamertag; `"$([regex]::Escape($renamed))`", real name `"`", shape 0"
    Expect-Log "^profiles: \d+ is now `"$([regex]::Escape($renamed))`"$" $mark "renaming $added" | Out-Null
    Press Right "Settings: Profile: Real name; .+"
    Press A 'TextEntry: .+'
    $mark = Get-DashboardLogLength
    Type-Over 0 $realName "Settings: Profile: Real name; `"$([regex]::Escape($renamed))`", real name `"$realName`", shape 0"
    Expect-Log "^profiles: \d+'s real name is `"$realName`"$" $mark 'the real name' | Out-Null
    Press Down 'Settings: Profile: Remove profile; .+'
    Press Left 'Settings: Profile: Picture; .+'
    # It wears plain picture 0, focused as the page opens; with the account picture at most before it (the test's
    # Gamerpics folder is empty), plain picture 2 is on the first row, along to the right.
    Press A 'Settings: Picture: Shape 0; worn' '07-picture-page'
    Walk Right 'Settings: Picture: Shape 2' 4
    $mark = Get-DashboardLogLength
    Press A "Settings: Profile: Picture; `"$([regex]::Escape($renamed))`", real name `"$realName`", shape 2" '08-picture-worn'
    Expect-Log '^profiles: \d+ wears shape 2$' $mark 'the picture' | Out-Null
    # None of it is the Profile signed in's: the top bar's Profile is the same.
    if ((Profile-Now) -ne $first) { $failures += "changing another Profile changed the one signed in: '$(Profile-Now)'" }

    # --- The Windows user's Profile removed, after its question: the other signs in, and every place shows it.
    Press B "Settings: Profiles: $([regex]::Escape($renamed))"
    Walk Left "Settings: Profiles: $([regex]::Escape($me))" 4
    Press A "Settings: Profile: Gamertag; .+"
    Press Down 'Settings: Profile: Picture; .+'
    Press Right 'Settings: Profile: Remove profile; .+'
    Press A "Settings: Profile: Prompt\(Remove\) of $([regex]::Escape($me))" '09-remove-prompt'
    $mark = Get-DashboardLogLength
    Press A "Settings: Profiles: $([regex]::Escape($renamed))" '10-removed'
    Expect-Log "^profiles: removed \d+ `"$([regex]::Escape($me))`"; \d+ is signed in$" $mark "removing $me" | Out-Null
    Expect-Profile $renamed $realName 'shape 2' $mark 'the Profile signed in after the removal'

    # Home's top bar and the Guide's Profile & system draw it: the pictures show both.
    Press B 'Settings: Personalisation: Profiles'
    Press B 'Settings: Theme settings: Personalisation'
    Press B 'Settings: Rail: Theme settings'
    Press B 'TopBar\(Settings\)' '11-home-after'
    Walk Left 'TopBar\(Profile\)' 6
    $mark = Get-DashboardLogLength
    Push-Button A
    $guide = Wait-DashboardLog '^guide: ProfileAndSystem Entry\(0\)$' 8 $mark
    Start-Sleep -Milliseconds 3000
    Save-Screen '12-guide-profile'
    Say "A on the profile: $(if ($guide) { $guide } else { 'no Guide' })"
    if (-not $guide) { $failures += "A on Home's profile should have opened the Guide on its profile entry" }
    Push-Button B
    Start-Sleep -Milliseconds 2000

    # --- Started again: the Profile signed in last is remembered, and greeted by its real name's first word.
    Stop-Dashboard
    Start-Sleep -Seconds 2
    Start-Here
    Expect-Log "^profiles: 1 of them, \d+ `"$([regex]::Escape($renamed))`" signed in last$" 0 'the Profiles at the next start' | Out-Null
    Expect-Profile $renamed $realName 'shape 2' 0 'the Profile signed in at the next start'
    if (Get-DashboardLog | Where-Object { $_ -match '^profiles: made ' }) { $failures += 'a second first Profile was made' }
    $greeted = Wait-Greeting
    $hi = Wait-DashboardLog '^toasts: shown "Hi Ann!"' 5
    if (-not $hi) { $failures += "the greeting should have said 'Hi Ann!': $greeted" }
    Save-Screen '13-home-again'

    # --- The only Profile has no Remove profile: along its last row, Picture is the end.
    Open-Profiles $renamed
    Press A "Settings: Profile: Gamertag; .+"
    Press Down 'Settings: Profile: Picture; .+'
    Push-Button Right
    Start-Sleep -Milliseconds 1500
    if ((Focus-Now) -notmatch '^focus: Settings: Profile: Picture; ') { $failures += "the only Profile should have no Remove profile: '$(Focus-Now)'" }
    Save-Screen '14-only-profile'

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
    if (Test-Path $data) { Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
