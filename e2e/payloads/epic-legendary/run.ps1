# Epic Games through legendary (#175, docs/spec/helpers.md 5), walked with the virtual pad against the stand-in
# legendary: standin.exe kept as Settings > Helpers keeps legendary (<data>\helpers\legendary\legendary.exe, with its
# helper.json at the table's version), which answers the commands the Dashboard runs in legendary 0.21.1's own lines
# and writes legendary's own records (user.json, metadata, installed.json, the resume file) in the test's folder named
# by --helper-home, never the user's. Its one game, Zinnia Gate, is 48 MiB counted out in eight steps
# STANDIN_HELPER_STEP_MS apart; nothing is downloaded. Beside it a fake Epic Games Launcher (Initialize-FakeEpic),
# whose Data\Manifests legendary's egl-sync writes the installed game into.
#
# What it walks:
# - Not signed in: `helpers: legendary: not signed in`; Settings > Helpers on legendary (`legendary; Remove; added
#   0.21.1`), whose words say how signing in is done (the picture). Epic's games are the launcher's alone.
# - Signed in as the user would, with legendary's own `auth` (the stand-in's takes any code); the Dashboard started
#   again (focus logged from there): `helpers: legendary: signed in as Stand-in Player`; `status --json` run, and Epic
#   read again with legendary's library (`legendary 0 installed, 1 owned`).
# - Zinnia Gate found by Search (Z), its Game hub on Install; A: `epic: legendary: ZinniaGate: installing into ...`, the
#   Queue's `joined, asked for` and `downloading, <n>% of 50331648 bytes`, from legendary's own lines.
# - The hub's Queue action to Manage Queue, its row; View (`Back`) on it: Pause installation (legendary stopped, its
#   resume file kept, `paused at <n> of 50331648 bytes`), Resume installation (legendary run again, `<n> bytes to
#   download, 50331648 in all`), Cancel installation after its question (`cancelled, and what it fetched deleted`;
#   `queue: Zinnia Gate dropped`; its folder gone).
# - Installed to the end: `done`, egl-sync's record in the fake launcher's Manifests, `queue: Zinnia Gate installed`.
# - Play from its hub: started through `legendary launch ZinniaGate` (the stand-in's line), the stand-in game in front;
#   ended, Home back.
# - Uninstall from its Tile menu (Menu, `Start`), after the Dashboard's question: `-y uninstall ZinniaGate` ended 0,
#   the old launcher's record removed, its folder gone, `queue: Zinnia Gate uninstalled`.
#
# Every press is judged by --log-focus before the next, and the walk stops at the first that goes elsewhere. Runs on
# the VM and in safe mode on the TV PC; every folder is the test's own. The screenshots are to look at.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$title = 'Xbox Series Dashboard'
$name = 'Zinnia Gate'
$app = 'ZinniaGate'
$whole = 50331648

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: "; then a rest for the picture, if one is asked for. A press that goes elsewhere ends the walk.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    Start-Sleep -Milliseconds 150
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if (-not $got) { Say "$Button -> NOT '$Expect': $(Focus-Now)"; throw "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'" }
    Say "$Button -> $got"
    if ($Name) { Start-Sleep -Milliseconds 2500; Save-Screen $Name }
    $got
}

# Presses $Button until focus is where $Expect says, $Most times at most; the walk stops if it never gets there.
function Walk([string] $Button, [string] $Expect, [int] $Most = 10) {
    $pattern = "^focus: $Expect`$"
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match $pattern) { break }
        Push-Button $Button
        Start-Sleep -Milliseconds 700
    }
    if ((Focus-Now) -notmatch $pattern) { throw "$Button never reached 'focus: $Expect': '$(Focus-Now)'" }
    Say "$Button until $(Focus-Now)"
}

# A line the Dashboard must log after the mark $After, by this pattern, within $Seconds; the walk stops without it.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 30) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if (-not $line) { Say "${What}: no line like '$Pattern'"; throw "${What}: the Dashboard logged no '$Pattern'" }
    Say "${What}: $line"
    $line
}

# A line the stand-in must have written to standin.log after its first $After lines.
function Expect-Standin([string] $Pattern, [int] $After, [string] $What) {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    $line = Wait-Until { if (Test-Path $log) { Get-Content $log | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } } 15
    if (-not $line) { throw "${What}: the stand-in wrote no '$Pattern'" }
    Say "${What}: $line"
}
function Standin-LogLength { $log = Join-Path $env:E2E_OUT 'standin.log'; if (Test-Path $log) { @(Get-Content $log).Count } else { 0 } }

function In-Front { if ((Get-Foreground).Title -ne $title) { Say "note: put in front: $(Set-Foreground $title)" } }

# Starts the Dashboard on the test's folders and waits for Home; focus is logged from its first press.
function Start-Ours {
    $script:dashboard = Start-Dashboard @('--log-focus', '--epic', "`"$script:epicRoot`"", '--data-dir', "`"$script:data`"", '--helper-home', "`"$script:helperHome`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 3
    In-Front
    Say "focus after the start: $(Focus-Now)"
}

# Y on Home, Z on the Console keyboard, B, then Right along the results until the log names Zinnia Gate.
function Find-Game {
    Press Y 'Search: Key\(Q\); .*' | Out-Null
    foreach ($way in 'Down', 'Up') {
        foreach ($i in 1..6) { if ((Focus-Now) -match '^focus: Search: Key\(Z\);') { break }; Push-Button $way; Start-Sleep -Milliseconds 650 }
        if ((Focus-Now) -match '^focus: Search: Key\(Z\);') { break }
    }
    if ((Focus-Now) -notmatch '^focus: Search: Key\(Z\);') { throw "never reached Z on the Console keyboard: '$(Focus-Now)'" }
    Press A 'Search: Key\(Z\); "Z\|" .*' | Out-Null
    Press B 'Search: \w+\(0\) .+; "Z" away.*' | Out-Null
    Walk Right "Search: \w+\(\d+\) $name;.*" 12
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $script:epicRoot = Get-FakeRoot 'epic'
    Initialize-FakeEpic $script:epicRoot | Out-Null
    # The helper's folder (its records and the games it installs) and the data folder: the test's own, empty.
    $script:helperHome = Get-FakeRoot 'legendary-home'
    $script:data = Get-FakeRoot 'legendary-data'
    foreach ($dir in $script:helperHome, $script:data) {
        if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
        New-Item $dir -ItemType Directory -Force | Out-Null
    }
    Register-Fake 'legendary-home' $script:helperHome
    Register-Fake 'legendary-data' $script:data
    $records = Join-Path $script:helperHome 'legendary'
    $games = Join-Path $script:helperHome 'games'

    # legendary as Settings > Helpers keeps it once added (the helpers payload walks the adding).
    $kept = Join-Path $script:data 'helpers\legendary'
    New-Item $kept -ItemType Directory -Force | Out-Null
    $legendary = Join-Path $kept 'legendary.exe'
    Copy-Item (Join-Path $PSScriptRoot 'standin.exe') $legendary
    $hash = (Get-FileHash $legendary -Algorithm SHA256).Hash.ToLower()
    [IO.File]::WriteAllText((Join-Path $kept 'helper.json'), "{`"version`":`"0.21.1`",`"sha256`":`"$hash`"}", (New-Object Text.UTF8Encoding $false))

    # The stand-in's pace, for every run of it the Dashboard makes: slow enough to steer with the pad.
    $env:STANDIN_HELPER_STEP_MS = '3000'
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    # --- Not signed in: said, and shown on Settings > Helpers.
    Start-Ours
    Expect-Logged '^helpers: legendary 0\.21\.1 added, gogdl 1\.4\.0 not added$' 0 'Helpers read' | Out-Null
    Expect-Logged '^helpers: legendary: not signed in$' 0 'Not signed in' | Out-Null
    Walk Right 'TopBar\(Settings\)' 6
    Press A 'Settings: Rail: Theme settings' | Out-Null
    Press Right 'Settings: Theme settings: Personalisation' | Out-Null
    Walk Down 'Settings: Theme settings: Helpers' 8
    Press A 'Settings: Helpers: legendary; Remove; added 0\.21\.1' '01-not-signed-in' | Out-Null
    if (Get-DashboardLog | Where-Object { $_ -match '^epic: legendary: ' }) { throw "legendary was run, not signed in: $(Get-DashboardLog | Where-Object { $_ -match '^epic: legendary: ' } | Select-Object -First 1)" }
    Stop-Dashboard

    # --- Signed in with legendary's own auth, as the user would; then the Dashboard again.
    $env:LEGENDARY_CONFIG_PATH = $records
    $auth = Start-Process $legendary -ArgumentList @('auth', '--code', 'stand-in-code') -Wait -PassThru -WindowStyle Hidden
    Remove-Item Env:\LEGENDARY_CONFIG_PATH
    if ($auth.ExitCode -ne 0 -or -not (Test-Path (Join-Path $records 'user.json'))) { throw "the stand-in legendary's auth ended $($auth.ExitCode), user.json $(Test-Path (Join-Path $records 'user.json'))" }
    Say 'signed in with legendary auth'
    Start-Ours
    Expect-Logged '^helpers: legendary: signed in as Stand-in Player$' 0 'Signed in' | Out-Null
    Expect-Logged '^epic: legendary: library: started \(pid \d+\): status --json$' 0 'The library listed' | Out-Null
    Expect-Logged '^helpers: owned games listed again for epic$' 0 'Listed' 60 | Out-Null
    Expect-Logged '^epic: \d+ games from .+legendary 0 installed, 1 owned\) in \d+ ms$' 0 'Epic read with it' 60 | Out-Null

    # --- Install from its Game hub.
    Find-Game
    Press A "GameHub: $app Hub Install" '02-hub-install' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^epic: legendary: ${app}: installing into $([regex]::Escape($games))$" $mark 'Install' | Out-Null
    Expect-Logged "^epic: legendary: ${app}: started \(pid \d+\): -y install $app --base-path .+ --skip-sdl --skip-dlcs$" $mark 'legendary run' | Out-Null
    Expect-Logged "^queue: $name joined, asked for$" $mark 'The Queue' | Out-Null
    Expect-Logged "^queue: $name downloading, \d+% of $whole bytes$" $mark 'Its bytes' | Out-Null

    # --- Manage Queue from its hub's Queue action; its row; View on it.
    Walk Right "Search: \w+\(\d+\) $name;.*" 1
    Press A "GameHub: $app Hub (Queued|Installing)" | Out-Null
    Walk Right "GameHub: $app Hub Queue" 3
    Press A 'ManageQueue: .+' | Out-Null
    $row = "ManageQueue: Row\(0\) of $name; 1 installing, \d+ recently installed"
    Walk Down "$row; Installing\.\.\. [\d.]+%" 3
    Save-Screen '03-row'

    # --- Pause: legendary stopped, its resume file kept.
    Press Back "TileMenu: Menu\(Install\) of $name" | Out-Null
    Press Down "TileMenu: Menu\(Pause installation\) of $name" | Out-Null
    $mark = Get-DashboardLogLength
    Press A "$row; .+" | Out-Null
    Expect-Logged "^epic: legendary: ${app}: paused at \d+ of $whole bytes$" $mark 'Paused' | Out-Null
    Expect-Logged "^queue: $name paused(, \d+% of $whole bytes)?$" $mark 'Paused, the Queue' | Out-Null
    Expect-Logged "^focus: $row; Paused [\d.]+%$" $mark 'Paused, the row' | Out-Null
    if (-not (Test-Path (Join-Path $records "tmp\$app.resume"))) { throw "legendary's resume file is not there" }
    Start-Sleep -Milliseconds 2500
    Save-Screen '04-paused'

    # --- Resume: legendary run again, from where it was.
    Press Back "TileMenu: Menu\(Install\) of $name" | Out-Null
    Press Down "TileMenu: Menu\(Resume installation\) of $name" | Out-Null
    $mark = Get-DashboardLogLength
    Press A "$row; .+" | Out-Null
    Expect-Logged "^epic: legendary: ${app}: started \(pid \d+\): -y install $app .+$" $mark 'Resumed' | Out-Null
    Expect-Logged "^epic: legendary: ${app}: \d+ bytes to download, $whole in all$" $mark 'Resumed, what is left' | Out-Null
    Expect-Logged "^focus: $row; Installing\.\.\. [\d.]+%$" $mark 'Resumed, the row' | Out-Null

    # --- Cancel, after its question: legendary stopped, what it fetched deleted.
    Press Back "TileMenu: Menu\(Install\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Cancel installation\) of $name" 3
    Press A "TileMenu: Prompt\(Cancel installation\) of $name" '05-cancel-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'ManageQueue: .+' | Out-Null
    Expect-Logged "^epic: legendary: ${app}: cancelled, and what it fetched deleted$" $mark 'Cancelled' | Out-Null
    Expect-Logged "^queue: $name dropped$" $mark 'Cancelled, the Queue' | Out-Null
    if (Test-Path (Join-Path $games $name)) { throw "its folder is still there after Cancel" }
    if (Test-Path (Join-Path $records "tmp\$app.resume")) { throw "legendary's resume file is still there after Cancel" }

    # --- Installed to the end, from its hub again.
    Press B 'GameHub: .+' | Out-Null
    Walk Left "GameHub: $app Hub Install" 4
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^queue: $name joined, asked for$" $mark 'Second install' | Out-Null
    Expect-Logged "^epic: legendary: ${app}: says \[cli\] INFO: Finished installation process in [\d.]+ seconds\.$" $mark 'legendary finished' 90 | Out-Null
    Expect-Logged "^epic: legendary: ${app}: egl-sync: ended with 0$" $mark 'The old launcher told' | Out-Null
    Expect-Logged "^epic: legendary: ${app}: done$" $mark 'Done' | Out-Null
    Expect-Logged "^queue: $name installed$" $mark 'Installed' 60 | Out-Null
    $item = @(Get-ChildItem $script:epicRoot -Recurse -Filter '*.item' | Where-Object { (Get-Content $_.FullName -Raw) -match "`"AppName`":\s*`"$app`"" })
    if ($item.Count -ne 1) { throw "egl-sync should have written one record of $app in the fake launcher's Manifests, and there are $($item.Count)" }
    Say "the old launcher's record: $($item[0].FullName)"

    # --- Play, through legendary launch.
    Press B "Search: \w+\(\d+\) $name;.*" | Out-Null
    Press A "GameHub: $app Hub Play" '06-hub-play' | Out-Null
    $mark = Get-DashboardLogLength
    $standinMark = Standin-LogLength
    Start-Sleep -Milliseconds 150
    Push-Button A
    Expect-Logged "^launch: $name \($app\) asked of Epic Games, Launch screen up at \d+ ms$" $mark 'Play' 15 | Out-Null
    Expect-Standin "\] legendary: launch $app$" $standinMark 'Started through legendary'
    if (-not (Wait-Until { @(Get-StandInGame).Count } 30)) { throw "$name's stand-in game never started" }
    Expect-Logged '^launch: the game has booted' $mark 'Booted' 60 | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '07-playing'
    Stop-StandInGame
    Expect-Logged '^launch: the game has ended, Home back at \d+ ms$' $mark 'Ended' 20 | Out-Null
    Start-Sleep -Milliseconds 1500
    In-Front
    Say "focus after the game: $(Focus-Now)"

    # --- Uninstall from its Tile menu, after the Dashboard's question.
    Find-Game
    Press A "GameHub: $app Hub Play" | Out-Null
    Press Start "TileMenu: Menu\(Play game\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Uninstall\) of $name" 6
    Press A "TileMenu: Prompt\(Uninstall\) of $name" '08-uninstall-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^epic: legendary: ${app}: started \(pid \d+\): -y uninstall $app$" $mark 'Uninstall' | Out-Null
    Expect-Logged "^epic: legendary: ${app}: ended with 0$" $mark 'Uninstalled by legendary' | Out-Null
    Expect-Logged "^epic: legendary: ${app}: the old launcher's record .+ removed$" $mark 'The old record removed' | Out-Null
    Expect-Logged "^queue: $name uninstalled$" $mark 'Uninstalled' 60 | Out-Null
    if (Test-Path (Join-Path $games $name)) { throw 'its folder is still there after Uninstall' }
    Start-Sleep -Milliseconds 2500
    Save-Screen '09-uninstalled'

    if ($script:dashboard.HasExited) { throw "The Dashboard exited with $($script:dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Get-OwnProcess legendary | Stop-Process -Force -ErrorAction SilentlyContinue
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
