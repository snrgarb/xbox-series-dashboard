# Who's playing today?, Signing you in and Switch profile (docs/spec/start-up.md, #127), walked with the virtual pad in
# a data folder of the test's own, with Start-up (`Start-Dashboard -Startup`) and the Boot video Off: --media names an
# empty folder of the test's own, so no video is found and Start-up goes straight on.
#
#  1. Started at Home (--no-startup): the first Profile is made from the Windows user, and a second is added on
#     Settings > Account > Profiles by its gamertag, as the profiles payload does.
#  2. Started again with Start-up: Who's playing today? shows both, focus on the Windows user's, signed in last; Right
#     until the other, A; Signing you in, then Home, the `profile:` line and the greeting for it.
#  3. The Guide's Switch profile: the Guide goes and Who's playing today? asks, focus on the one signed in; Left until
#     the Windows user's, A; signed in and greeted again.
#  4. A wake from sleep, pretended by `standin wake` (--allow-toast-posts): Who's playing today? asks again; A.
#  5. Settings > Account > Who's playing today?: Sign in the last profile. A pretended wake asks nothing now;
#     started again, no Who's playing today?: Signing you in for the one signed in last, then Home and its greeting.
#
# Judged by the Dashboard's log, times in ms since the process started:
#   startup: who's playing shown at <N> ms, <n> profiles, focus on "<gamertag>"
#   startup: signing in "<gamertag>" at <N> ms
#   startup: signed in at <N> ms
#   startup: woke from sleep at <N> ms: Who's playing today? asks | nothing asked
#   profile: "<gamertag>", real name "...", <picture>     (the Profile signed in)
#   toasts: shown "Hi <name>!" ... (Greeting)
#   settings: startup.ask_whos_playing = false
# and --log-focus's `focus: Startup: WhosPlaying "<gamertag>"`, `focus: Startup: SigningIn "<gamertag>"`, `guide: ...`.
# Positions along the row are walked by the focus lines, never counted.
#
# Pictures to look at beside the Bundle's (its artifacts' port-reference-*/whos-playing.jpg) and the console's
# (the Bundle's docs/reference/startup/): Who's playing today? with two Profiles, Signing you in, and the greeting over Home.
#
# The data and media folders are the test's own, empty as it starts, and removed at the end. Nothing of the machine's
# is read or changed: in safe mode on the TV PC it runs as it does on the VM.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$folder = Get-TestFolder 'dashboard-whos-playing-test'
$data = Join-Path $folder 'data'
$media = Join-Path $folder 'media'
$added = 'Moth Runner'

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

# A log line matching $Pattern after the first $After lines, waited for $Seconds; a failure named $What if none comes.
function Expect-Log([string] $Pattern, [string] $What, [double] $Seconds = 15, [int] $After = 0) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    Say "${What}: $(if ($line) { $line } else { "NO line '$Pattern'" })"
    if (-not $line) { $script:failures += "$What should have logged '$Pattern'" }
    $line
}

# Text entry is up: $Text typed on the keyboard, and Menu keeps it, which must put focus where $Expect says.
function Type-In([string] $Text, [string] $Expect) {
    Start-Sleep -Milliseconds 1500
    [System.Windows.Forms.SendKeys]::SendWait($Text)
    Start-Sleep -Milliseconds 2000
    Say "typed: $(Focus-Now)"
    Press Start $Expect
}

# Started on the test's folders: at Home, or with Start-up.
function Start-Here([switch] $Startup) {
    $arguments = @('--log-focus', '--allow-toast-posts', '--data-dir', "`"$data`"", '--media', "`"$media`"")
    $script:dashboard = if ($Startup) { Start-Dashboard -Startup $arguments } else { Start-Dashboard $arguments }
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
}

# The window in front, for the pad.
function Ensure-Front {
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }
}

# Who's playing today? is up, after the first $After lines, with $Count Profiles and focus on $Focus: the line says so,
# and the focus line too. Returns whether it is.
function Expect-Asked([int] $After, [int] $Count, [string] $Focus, [string] $What, [string] $Name = '') {
    $tag = [regex]::Escape($Focus)
    $shown = Expect-Log "^startup: who's playing shown at \d+ ms, $Count profiles, focus on `"$tag`"$" $What 60 $After
    if ($shown -and $Name) {
        Start-Sleep -Milliseconds 2500
        Save-Screen $Name
    }
    if ($shown -and (Focus-Now) -notmatch "^focus: Startup: WhosPlaying `"$tag`"$") {
        $script:failures += "${What}: the focus line should be on $Focus, and is '$(Focus-Now)'"
    }
    [bool] $shown
}

# A on Who's playing today? with focus on $Gamertag: Signing you in, then Home and the greeting for it.
function Sign-In([string] $Gamertag, [string] $What, [string] $Name = '', [switch] $Same) {
    $tag = [regex]::Escape($Gamertag)
    $mark = Get-DashboardLogLength
    Press A "Startup: SigningIn `"$tag`""
    $signing = Expect-Log "^startup: signing in `"$tag`" at \d+ ms$" "Signing you in $What" 10 $mark
    if ($signing -and $Name) {
        Start-Sleep -Milliseconds 800
        Save-Screen $Name
    }
    $signed = Expect-Log '^startup: signed in at \d+ ms$' "signed in $What" 30 $mark
    if ($signing -and $signed) {
        $took = [double] ($signed -replace '^.* at (\d+) ms$', '$1') - [double] ($signing -replace '^.* at (\d+) ms$', '$1')
        Say "Signing you in showed for $took ms"
        if ($took -lt 2500) { $script:failures += "${What}: Signing you in should show about 3 s, and showed $took ms" }
    }
    # The `profile:` line is said only when the Profile changes.
    if (-not $Same) { Expect-Log "^profile: `"$tag`", " "the Profile signed in $What" 10 $mark | Out-Null }
    $hi = Expect-Log '^toasts: shown "Hi .+!".*\(Greeting\)' "the greeting $What" 30 $mark
    Wait-DashboardLog '^toasts: gone' 20 $mark | Out-Null
    $hi
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force }
    New-Item $data -ItemType Directory -Force | Out-Null
    New-Item $media -ItemType Directory -Force | Out-Null

    # --- 1. The first Profile, from the Windows user; a second added by its gamertag.
    Start-Here
    $first = Expect-Log '^profile: ' 'the Profile signed in' 30
    if (-not ($first -match '^profile: "(.+)", real name "", ')) { throw "the first Profile is not the Windows user's: '$first'" }
    $me = $Matches[1]
    Say "the Windows user's Profile: $me"
    Wait-Greeting | Out-Null
    Ensure-Front
    Walk Right 'TopBar\(Settings\)' 5
    Press A 'Settings: Rail: General'
    Press Down 'Settings: Rail: Account'
    Press Right 'Settings: Account: Profiles'
    Press A "Settings: Profiles: $([regex]::Escape($me))"
    Press Right 'Settings: Profiles: Add profile'
    Press A 'TextEntry: .+'
    $mark = Get-DashboardLogLength
    Type-In $added "Settings: Profiles: $([regex]::Escape($added))"
    Expect-Log "^profiles: added \d+ `"$([regex]::Escape($added))`"$" "adding $added" 10 $mark | Out-Null

    # --- 2. Started with Start-up: the video Off, Who's playing today? with both, focus on the Windows user's.
    Stop-Dashboard
    Start-Sleep -Seconds 2
    Start-Here -Startup
    Expect-Log '^startup: boot video \S+ not found, going straight on$' 'no Boot video' 30 | Out-Null
    if (Expect-Asked 0 2 $me 'Who''s playing today? at the start' '01-whos-playing') {
        Ensure-Front
        Walk Right "Startup: WhosPlaying `"$([regex]::Escape($added))`"" 4
        Save-Screen '02-whos-playing-moved'
        Sign-In $added 'at the start' '03-signing-in' | Out-Null
        Save-Screen '04-home'
    }
    $homeLine = Expect-Log "^startup: Home's first frame at \d+ ms$" "Home's first frame" 10
    $order = @(Get-DashboardLog | Where-Object { $_ -match "^startup: (who's playing shown|signing in|signed in|Home's first frame)" })
    Say "the order: $($order -join ' | ')"
    $kinds = @($order | ForEach-Object { $_ -replace '^startup: (who''s playing shown|signing in|signed in|Home''s first frame).*$', '$1' })
    if (($kinds -join ',') -ne "who's playing shown,signing in,signed in,Home's first frame") {
        $failures += "Start-up should go Who's playing today?, Signing you in, signed in, Home: $($kinds -join ', ')"
    }

    # --- 3. Switch profile on the Guide: it goes, and Who's playing today? asks, focus on the one signed in.
    Ensure-Front
    $mark = Get-DashboardLogLength
    Push-Button Guide
    $guide = Expect-Log '^guide: Landing ' 'the Guide' 10 $mark
    Push-Button RightShoulder
    Expect-Log '^guide: ProfileAndSystem Entry\(0\) Profile$' 'Profile & system' 10 $mark | Out-Null
    Push-Button Down
    Expect-Log '^guide: ProfileAndSystem Entry\(1\) SwitchProfile$' 'Switch profile' 10 $mark | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '05-switch-profile'
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^guide: closed$' 'the Guide gone' 10 $mark | Out-Null
    if (Expect-Asked $mark 2 $added 'Who''s playing today? from Switch profile' '06-switch-asks') {
        # B does nothing: someone has to sign in.
        Push-Button B
        Start-Sleep -Milliseconds 1500
        if ((Focus-Now) -notmatch "^focus: Startup: WhosPlaying ") { $failures += "B on Who's playing today? should do nothing: '$(Focus-Now)'" }
        Walk Left "Startup: WhosPlaying `"$([regex]::Escape($me))`"" 4
        Sign-In $me 'from Switch profile' | Out-Null
    }
    if ($guide -and (Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match "^startup: boot video|^startup: first video frame" })) {
        $failures += 'Switch profile should not play the Boot video'
    }

    # --- 4. A wake from sleep, pretended: asked again, as Settings has it ask.
    $mark = Get-DashboardLogLength
    $sent = Start-Process (Join-Path $Payload 'standin.exe') -ArgumentList 'wake' -Wait -PassThru
    if ($sent.ExitCode -ne 0) { $failures += "standin wake exited with $($sent.ExitCode)" }
    Expect-Log "^startup: woke from sleep at \d+ ms: Who's playing today\? asks$" 'the wake' 10 $mark | Out-Null
    if (Expect-Asked $mark 2 $me 'Who''s playing today? on waking' '07-woken') {
        Ensure-Front
        Sign-In $me 'on waking' -Same | Out-Null
    }

    # --- 5. Settings: Sign in the last profile. Waking asks nothing; the next start signs the last one in by itself.
    Ensure-Front
    Walk Right 'TopBar\(Settings\)' 5
    Press A 'Settings: Rail: General'
    Press Down 'Settings: Rail: Account'
    Press Right 'Settings: Account: Profiles'
    Press Right "Settings: Account: Who's playing today\?"
    Press A "Settings: Who's playing today\?: Ask every time; ticked" '08-setting'
    Press Down "Settings: Who's playing today\?: Sign in the last profile"
    $mark = Get-DashboardLogLength
    Press A "Settings: Who's playing today\?: Sign in the last profile; ticked" '09-setting-off'
    Expect-Log '^settings: startup\.ask_whos_playing = false$' 'the setting saved' 10 $mark | Out-Null
    $mark = Get-DashboardLogLength
    $sent = Start-Process (Join-Path $Payload 'standin.exe') -ArgumentList 'wake' -Wait -PassThru
    if ($sent.ExitCode -ne 0) { $failures += "standin wake exited with $($sent.ExitCode)" }
    Expect-Log '^startup: woke from sleep at \d+ ms: nothing asked$' 'the wake with it off' 10 $mark | Out-Null

    Stop-Dashboard
    Start-Sleep -Seconds 2
    Start-Here -Startup
    Expect-Log '^settings: startup\.ask_whos_playing is false$' 'the setting at the next start' 30 | Out-Null
    $signing = Expect-Log "^startup: signing in `"$([regex]::Escape($me))`" at \d+ ms$" 'Signing you in by itself' 60
    if ($signing) {
        Start-Sleep -Milliseconds 800
        Save-Screen '10-signing-in-by-itself'
    }
    Expect-Log '^startup: signed in at \d+ ms$' 'signed in by itself' 30 | Out-Null
    Expect-Log '^toasts: shown "Hi .+!".*\(Greeting\)' 'the greeting with it off' 30 | Out-Null
    if (Get-DashboardLog | Where-Object { $_ -match "^startup: who's playing shown" }) { $failures += "with it off Who's playing today? should not ask" }
    Save-Screen '11-home-with-it-off'

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
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
