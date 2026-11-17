# GOG through gogdl (#176, docs/spec/helpers.md 5), walked with the virtual pad against the stand-in gogdl: standin.exe
# kept as Settings > Helpers keeps gogdl (<data>\helpers\gogdl\gogdl.exe, with its helper.json at the table's version),
# which answers the commands the Dashboard runs in gogdl 1.4.0's own lines and writes gogdl's own files (auth.json,
# the build's manifest, .gogdl-resume) in the test's folder named by --helper-home, never the user's. The build it
# "downloads" (48 MiB counted out in eight steps STANDIN_HELPER_STEP_MS apart; nothing is downloaded) brings a copy of
# the stand-in as the game's program, its goggame-<id>.info and its unins000.exe, and asks for the script interpreter
# as its set-up step, which `redist --ids ISI` gives as another copy. Beside it the fake GOG (Initialize-FakeGog),
# whose Galaxy database owns Pewter Isles, not installed.
#
# What it walks:
# - Not signed in: `helpers: gogdl: not signed in`; Settings > Helpers on gogdl (`gogdl; Remove; added 1.4.0`), whose
#   words say how signing in is done (the picture).
# - Signed in as the user would, with gogdl's own `auth --code` (the stand-in's takes any code); the Dashboard started
#   again (focus logged from there): `helpers: gogdl: signed in`.
# - Pewter Isles found by Search (P), its Game hub on Install; A: `gog: gogdl: <id>: installing into ...`, the Queue's
#   `joined, asked for` and `downloading, <n>% of 50331648 bytes`, from gogdl's own lines.
# - The hub's Queue action to Manage Queue, its row; View (`Back`) on it: Pause installation (gogdl stopped, its
#   .gogdl-resume kept), Resume installation (gogdl run again, `<n> bytes to download, 50331648 in all`), Cancel
#   installation after its question (`cancelled, and what it fetched deleted`; `queue: Pewter Isles dropped`; its
#   folder gone).
# - Installed to the end: the set-up step, the script interpreter run with /VERYSILENT and the game's folder (the
#   stand-in's line in standin.log, and its standin-set-up.txt), `queue: Pewter Isles installed`.
# - Play from its hub: the game's own program, the stand-in game in front; ended, Home back.
# - Uninstall from its Tile menu (Menu, `Start`), after the Dashboard's question: `unins000.exe /VERYSILENT
#   /dir=<folder>`, its folder gone, `queue: Pewter Isles uninstalled`.
#
# The stand-in's set-up step asks for no administrator, so Windows' permission prompt does not come up here: a real
# build's may (docs/spec/helpers.md 5). Every press is judged by --log-focus before the next, and the walk stops at the
# first that goes elsewhere. Runs on the VM and in safe mode on the TV PC; every folder is the test's own.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$title = 'Xbox Series Dashboard'
$name = 'Pewter Isles'
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

function Start-Ours {
    $script:dashboard = Start-Dashboard @('--log-focus', '--gog', "`"$script:gogRoot`"", '--data-dir', "`"$script:data`"", '--helper-home', "`"$script:helperHome`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 3
    In-Front
    Say "focus after the start: $(Focus-Now)"
}

# Y on Home (Search opens on Q), Right along the top row to P, A, B, then Right along the results until the log names
# Pewter Isles.
function Find-Game {
    Press Y 'Search: Key\(Q\); .*' | Out-Null
    Walk Right 'Search: Key\(P\);.*' 12
    Press A 'Search: Key\(P\); "P\|" .*' | Out-Null
    Press B 'Search: \w+\(0\) .+; "P" away.*' | Out-Null
    Walk Right "Search: \w+\(\d+\) $name;.*" 12
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $script:gogRoot = Get-FakeRoot 'gog'
    $gog = Initialize-FakeGog $script:gogRoot
    $game = @($gog.games | Where-Object { $_.name -eq $name -and -not $_.installed }) | Select-Object -First 1
    if (-not $game) { throw "the fake GOG should own $name, not installed" }
    $id = $game.source_id
    $script:helperHome = Get-FakeRoot 'gogdl-home'
    $script:data = Get-FakeRoot 'gogdl-data'
    foreach ($dir in $script:helperHome, $script:data) {
        if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
        New-Item $dir -ItemType Directory -Force | Out-Null
    }
    Register-Fake 'gogdl-home' $script:helperHome
    Register-Fake 'gogdl-data' $script:data
    $own = Join-Path $script:helperHome 'gogdl'
    $games = Join-Path $script:helperHome 'games'
    $folder = Join-Path $games $name

    # gogdl as Settings > Helpers keeps it once added (the helpers payload walks the adding).
    $kept = Join-Path $script:data 'helpers\gogdl'
    New-Item $kept -ItemType Directory -Force | Out-Null
    $gogdl = Join-Path $kept 'gogdl.exe'
    Copy-Item (Join-Path $PSScriptRoot 'standin.exe') $gogdl
    $hash = (Get-FileHash $gogdl -Algorithm SHA256).Hash.ToLower()
    [IO.File]::WriteAllText((Join-Path $kept 'helper.json'), "{`"version`":`"1.4.0`",`"sha256`":`"$hash`"}", (New-Object Text.UTF8Encoding $false))

    $env:STANDIN_HELPER_STEP_MS = '3000'
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    # --- Not signed in: said, and shown on Settings > Helpers.
    Start-Ours
    Expect-Logged '^helpers: legendary 0\.21\.1 not added, gogdl 1\.4\.0 added$' 0 'Helpers read' | Out-Null
    Expect-Logged '^helpers: gogdl: not signed in$' 0 'Not signed in' | Out-Null
    Walk Right 'TopBar\(Settings\)' 6
    # Games & apps > Installs > Helpers, beside Storage (#221).
    Press A 'Settings: Rail: General' | Out-Null
    Press Down 'Settings: Rail: Account' | Out-Null
    Press Down 'Settings: Rail: Games & apps' | Out-Null
    Press Right 'Settings: Games & apps: Launchers' | Out-Null
    Press Right 'Settings: Games & apps: Storage' | Out-Null
    Press Down 'Settings: Games & apps: Helpers' | Out-Null
    Press A 'Settings: Helpers: legendary; Add; not added' | Out-Null
    Press Down 'Settings: Helpers: gogdl; Remove; added 1\.4\.0' '01-not-signed-in' | Out-Null
    Stop-Dashboard

    # --- Signed in with gogdl's own auth, as the user would; then the Dashboard again.
    $env:GOGDL_CONFIG_PATH = $own
    $auth = Start-Process $gogdl -ArgumentList @('--auth-config-path', "`"$(Join-Path $own 'auth.json')`"", 'auth', '--code', 'stand-in-code') -Wait -PassThru -WindowStyle Hidden
    Remove-Item Env:\GOGDL_CONFIG_PATH
    if ($auth.ExitCode -ne 0 -or -not (Test-Path (Join-Path $own 'auth.json'))) { throw "the stand-in gogdl's auth ended $($auth.ExitCode)" }
    Say 'signed in with gogdl auth --code'
    Start-Ours
    Expect-Logged '^helpers: gogdl: signed in$' 0 'Signed in' | Out-Null

    # --- Install from its Game hub.
    Find-Game
    Press A "GameHub: $id Hub Install" '02-hub-install' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^gog: gogdl: ${id}: installing into $([regex]::Escape($games))$" $mark 'Install' | Out-Null
    Expect-Logged "^gog: gogdl: ${id}: started \(pid \d+\): --auth-config-path .+ download $id --platform windows --path .+ --support .+ --skip-dlcs --lang en-US$" $mark 'gogdl run' | Out-Null
    Expect-Logged "^queue: $name joined, asked for$" $mark 'The Queue' | Out-Null
    Expect-Logged "^queue: $name downloading, \d+% of $whole bytes$" $mark 'Its bytes' | Out-Null

    # --- Manage Queue from its hub's Queue action; its row; View on it.
    Walk Right "Search: \w+\(\d+\) $name;.*" 1
    Press A "GameHub: $id Hub (Queued|Installing)" | Out-Null
    Walk Right "GameHub: $id Hub Queue" 3
    Press A 'ManageQueue: .+' | Out-Null
    $row = "ManageQueue: Row\(0\) of $name; 1 installing, \d+ recently installed"
    Walk Down "$row; Installing\.\.\. [\d.]+%" 3
    Save-Screen '03-row'

    # --- Pause: gogdl stopped, its resume file kept.
    # The menu starts with Install, or (#219) with Pause installation itself.
    Press Back "TileMenu: Menu\((Install|Pause installation)\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Pause installation\) of $name" 2
    $mark = Get-DashboardLogLength
    Press A "$row; .+" | Out-Null
    Expect-Logged "^gog: gogdl: ${id}: paused at \d+ of $whole bytes$" $mark 'Paused' | Out-Null
    Expect-Logged "^focus: $row; Paused [\d.]+%$" $mark 'Paused, the row' | Out-Null
    if (-not (Test-Path (Join-Path $folder '.gogdl-resume'))) { throw "gogdl's resume file is not there" }
    Start-Sleep -Milliseconds 2500
    Save-Screen '04-paused'

    # --- Resume.
    Press Back "TileMenu: Menu\((Install|Resume installation)\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Resume installation\) of $name" 2
    $mark = Get-DashboardLogLength
    Press A "$row; .+" | Out-Null
    Expect-Logged "^gog: gogdl: ${id}: \d+ bytes to download, $whole in all$" $mark 'Resumed, what is left' | Out-Null
    Expect-Logged "^focus: $row; Installing\.\.\. [\d.]+%$" $mark 'Resumed, the row' | Out-Null

    # --- Cancel, after its question.
    Press Back "TileMenu: Menu\((Install|Pause installation)\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Cancel installation\) of $name" 3
    Press A "TileMenu: Prompt\(Cancel installation\) of $name" '05-cancel-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'ManageQueue: .+' | Out-Null
    Expect-Logged "^gog: gogdl: ${id}: cancelled, and what it fetched deleted$" $mark 'Cancelled' | Out-Null
    Expect-Logged "^queue: $name dropped$" $mark 'Cancelled, the Queue' | Out-Null
    if (Test-Path $folder) { throw 'its folder is still there after Cancel' }

    # --- Installed to the end, its set-up step run.
    Press B 'GameHub: .+' | Out-Null
    Walk Left "GameHub: $id Hub Install" 4
    $mark = Get-DashboardLogLength
    $standinMark = Standin-LogLength
    Press A '.+' | Out-Null
    Expect-Logged "^queue: $name joined, asked for$" $mark 'Second install' | Out-Null
    Expect-Logged "^gog: gogdl: ${id}: running scriptinterpreter\.exe /VERYSILENT /DIR=.+ /ProductId=$id /galaxyclient .+$" $mark 'The set-up step' 90 | Out-Null
    Expect-Logged "^gog: gogdl: ${id}: scriptinterpreter\.exe ended with 0 after \d+ s$" $mark 'Set up' | Out-Null
    Expect-Standin "\] scriptinterpreter: /VERYSILENT /DIR=" $standinMark 'The script interpreter ran'
    Expect-Logged "^gog: gogdl: ${id}: installed in $([regex]::Escape($folder))$" $mark 'Recorded' | Out-Null
    Expect-Logged "^queue: $name installed$" $mark 'Installed' 60 | Out-Null
    if (-not (Test-Path (Join-Path $folder 'standin-set-up.txt'))) { throw 'the set-up step wrote nothing in the game folder' }

    # --- Play: the game's own program.
    # A on the hub's Install closed the hub (`tile_menu::install`): focus is on Search's result already.
    if ((Focus-Now) -notmatch "^focus: Search: \w+\(\d+\) $name;") { throw "after Install, focus is not on $name's result: '$(Focus-Now)'" }
    Press A "GameHub: $id Hub Play" '06-hub-play' | Out-Null
    $mark = Get-DashboardLogLength
    Start-Sleep -Milliseconds 150
    Push-Button A
    Expect-Logged "^launch: $name \($id\) asked of GOG, Launch screen up at \d+ ms$" $mark 'Play' 15 | Out-Null
    if (-not (Wait-Until { @(Get-StandInGame).Count } 30)) { throw "$name's stand-in game never started" }
    Expect-Logged '^launch: the game has booted' $mark 'Booted' 60 | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '07-playing'
    Stop-StandInGame
    Expect-Logged '^launch: the game has ended, Home back at \d+ ms$' $mark 'Ended' 20 | Out-Null
    Start-Sleep -Milliseconds 1500
    In-Front
    Say "focus after the game: $(Focus-Now)"

    # --- Uninstall from its Tile menu, after the Dashboard's question: the game's own uninstaller, quiet.
    Find-Game
    Press A "GameHub: $id Hub Play" | Out-Null
    Press Start "TileMenu: Menu\(Play game\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Uninstall\) of $name" 6
    Press A "TileMenu: Prompt\(Uninstall\) of $name" '08-uninstall-question' | Out-Null
    $mark = Get-DashboardLogLength
    $standinMark = Standin-LogLength
    Press A '.+' | Out-Null
    Expect-Logged "^gog: gogdl: ${id}: running unins000\.exe /VERYSILENT /dir=$([regex]::Escape($folder))$" $mark 'Uninstall' | Out-Null
    Expect-Standin "\] unins000: /VERYSILENT /dir=" $standinMark 'The uninstaller ran'
    Expect-Logged "^gog: gogdl: ${id}: uninstalled from $([regex]::Escape($folder))$" $mark 'Uninstalled' | Out-Null
    Expect-Logged "^queue: $name uninstalled$" $mark 'The Queue' 60 | Out-Null
    if (Test-Path $folder) { throw 'its folder is still there after Uninstall' }
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
    foreach ($n in 'gogdl', 'scriptinterpreter', 'unins000') { Get-OwnProcess $n | Stop-Process -Force -ErrorAction SilentlyContinue }
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
