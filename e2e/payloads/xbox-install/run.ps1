# Xbox installs through the Store's queue (#173, docs/sources.md "Xbox", docs/research/game-pass.md 8): one
# Dashboard on the fake Xbox drive and the local PC Game Pass catalogue (`standin make-gamepass`), its installs taken
# by the queue's stand-in (`source_xbox::store_queue::Fake`, the one the Dashboard uses on a fake drive: the real Store
# is never asked). Walked with the virtual pad, judged by --log-focus and the `xbox:` and `queue:` lines.
#
# What it does, in order:
# - The subscription judged once at start: `gamepass: subscription: can't tell (no route tells without a sign-in...)`.
# - Saltmarsh Run (not installed, in the catalogue) from its Game hub: `xbox: 9ZZZGPASS101: asking the Store's queue to
#   install Saltmarsh Run onto <drive>\XboxGames`, taken (`the Store's queue took Saltmarsh Run (Pending)`), in the
#   Queue (`queue: Saltmarsh Run joined, asked for`, `downloading, <n>% of 2000000000 bytes`).
# - Manage Queue, its row: Pause installation, Resume installation, Cancel installation after its question, each said
#   (`pause Saltmarsh Run: the Store's queue took it`), the Queue following (`paused`, `dropped`).
# - Installed to the end from its hub again: `Saltmarsh Run installed: the Store says complete`, written onto the fake
#   drive by the stand-in, `queue: Saltmarsh Run installed`; Search finds it once.
# - Uninstalled from its Tile menu after the Dashboard's question: its package removed (`Saltmarsh Run uninstalled (1
#   package)`), `queue: Saltmarsh Run uninstalled`, its folder gone; still listed, from the catalogue, not installed.
# - Ember Orchard, which the stand-in refuses as the Store refuses a game it will not license (0x803F8001): the
#   Store's page opened instead (`opening ms-windows-store://pdp/?ProductId=9ZZZGPASS102 instead`), taken by the
#   stand-in (`open-address: asked to open ...` in standin.log).
#
# With `gamepass`, one build and the two runs one after the other (e2e/README.md, "The TV PC"):
#   for t in gamepass xbox-install; do e2e/htpc/run.sh $t --exe <dashboard.exe> --standin <standin.exe>; done
# Each starts its own Dashboard on its own data folder. Safe mode on the TV PC: every folder is the payload's own.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$title = 'Xbox Series Dashboard'
$name = 'Saltmarsh Run'
$product = '9ZZZGPASS101'
$whole = 2000000000

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

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

function Walk-Either([string] $Button, [string] $Back, [string] $Expect, [int] $Most = 12) {
    $pattern = "^focus: $Expect`$"
    foreach ($way in @($Button, $Back)) {
        foreach ($press in 1..$Most) {
            if ((Focus-Now) -match $pattern) { break }
            Push-Button $way
            Start-Sleep -Milliseconds 650
        }
        if ((Focus-Now) -match $pattern) { break }
    }
    if ((Focus-Now) -notmatch $pattern) { throw "never reached 'focus: $Expect': '$(Focus-Now)'" }
    Say "walked to $(Focus-Now)"
}

function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 30) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if (-not $line) { Say "${What}: no line like '$Pattern'"; throw "${What}: the Dashboard logged no '$Pattern'" }
    Say "${What}: $line"
    $line
}

function Expect-Standin([string] $Pattern, [int] $After, [string] $What) {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    $line = Wait-Until { if (Test-Path $log) { Get-Content $log | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } } 15
    if (-not $line) { throw "${What}: the stand-in wrote no '$Pattern'" }
    Say "${What}: $line"
}
function Standin-LogLength { $log = Join-Path $env:E2E_OUT 'standin.log'; if (Test-Path $log) { @(Get-Content $log).Count } else { 0 } }

function In-Front { if ((Get-Foreground).Title -ne $title) { Say "note: put in front: $(Set-Foreground $title)" } }

# Y on Home, the game's first letter on the Console keyboard, B, then along the results until the log names it.
function Find-Game([string] $Game, [string] $Letter, [string] $Row) {
    Press Y 'Search: Key\(Q\); .*' | Out-Null
    Walk-Either Down Up "Search: Key\($Row\);.*"
    Walk-Either Right Left "Search: Key\($Letter\);.*"
    Press A "Search: Key\($Letter\); `"$Letter\|`" .*" | Out-Null
    Press B "Search: \w+\(0\) .+; `"$Letter`" away.*" | Out-Null
    Walk-Either Right Left "Search: \w+\(\d+\) $([regex]::Escape($Game));.*" 14
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $xboxRoot = Get-FakeRoot 'xbox'
    Initialize-FakeXbox $xboxRoot | Out-Null
    $gameDir = Join-Path $xboxRoot "XboxGames\$name"
    if (Test-Path $gameDir) { throw "$name is on the fake drive before the test" }
    $catalogue = Get-FakeRoot 'gamepass'
    if (Test-Path $catalogue) { Remove-Item $catalogue -Recurse -Force }
    $made = Start-Process (Join-Path $PSScriptRoot 'standin.exe') -ArgumentList @('make-gamepass', "`"$catalogue`"") -Wait -PassThru
    if ($made.ExitCode -ne 0) { throw "standin make-gamepass exited with $($made.ExitCode)" }
    $script:data = Get-TestFolder 'dashboard-xbox-install-data'
    if (Test-Path $script:data) { Remove-Item $script:data -Recurse -Force }
    New-Item $script:data -ItemType Directory -Force | Out-Null

    # The stand-in queue's pace: a whole download in 90 s, slow enough to steer with the pad.
    $env:DASHBOARD_XBOX_QUEUE_SECONDS = '90'
    $env:DASHBOARD_WINDOWS_REGION = 'US en-US'
    Remove-Item Env:\DASHBOARD_GAMEPASS_SUBSCRIPTION -ErrorAction SilentlyContinue

    $arguments = @('--log-focus', '--data-dir', "`"$script:data`"", '--xbox', "`"$xboxRoot`"", '--gamepass', "`"$catalogue`"",
        '--open-addresses-with', "`"$(Join-Path $PSScriptRoot 'standin.exe')`"")
    $script:dashboard = Start-Dashboard $arguments
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Expect-Logged "^gamepass: subscription: can't tell \(no route tells without a sign-in: .+\)$" 0 'The subscription, judged' | Out-Null
    Expect-Logged '^xbox: \d+ games from .+ \(3 games: 1 installed, 2 not\) in \d+ ms$' 0 'Read with the catalogue' 60 | Out-Null
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 3
    In-Front

    # --- Install from its Game hub, through the queue.
    Find-Game $name 'S' 'A'
    Press A 'GameHub: .+ Hub Install' '01-hub-install' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^xbox: ${product}: asking the Store's queue to install $name onto $([regex]::Escape((Join-Path $xboxRoot 'XboxGames')))$" $mark 'Asked of the queue' | Out-Null
    Expect-Logged "^xbox: store queue \(stand-in\): start $product -> Ok\(`"Pending`"\)$" $mark 'The stand-in queue' | Out-Null
    Expect-Logged "^xbox: ${product}: the Store's queue took $name \(Pending\)$" $mark 'Taken' | Out-Null
    Expect-Logged "^queue: $name joined, asked for$" $mark 'The Queue' | Out-Null
    Expect-Logged "^xbox: ${product}: $name is Installing\.\.\. \(Downloading, \d+%\)$" $mark 'Downloading' | Out-Null
    Expect-Logged "^queue: $name downloading, \d+% of $whole bytes$" $mark 'Its bytes in the Queue' | Out-Null

    # --- Manage Queue from its hub's Queue action, its row.
    Walk-Either Right Left "Search: \w+\(\d+\) $name;.*" 1
    Press A 'GameHub: .+ Hub (Queued|Installing)' | Out-Null
    Walk Right 'GameHub: .+ Hub Queue' 3
    Press A 'ManageQueue: .+' | Out-Null
    $row = "ManageQueue: Row\(0\) of $name; 1 installing, \d+ recently installed"
    Walk Down "$row; Installing\.\.\. [\d.]+%" 3
    Save-Screen '02-row'

    # --- Pause.
    Press Back "TileMenu: Menu\((Install|Pause installation)\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Pause installation\) of $name" 2
    $mark = Get-DashboardLogLength
    Press A "$row; .+" | Out-Null
    Expect-Logged "^xbox: store queue \(stand-in\): pause $product -> Ok\(\(\)\)$" $mark 'Pause asked' | Out-Null
    Expect-Logged "^xbox: ${product}: pause ${name}: the Store's queue took it$" $mark 'Paused' | Out-Null
    Expect-Logged "^queue: $name paused(, \d+% of $whole bytes)?$" $mark 'Paused, the Queue' | Out-Null
    Expect-Logged "^focus: $row; Paused [\d.]+%$" $mark 'Paused, the row' | Out-Null
    Start-Sleep -Milliseconds 2500
    Save-Screen '03-paused'

    # --- Resume.
    Press Back "TileMenu: Menu\((Install|Resume installation)\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Resume installation\) of $name" 2
    $mark = Get-DashboardLogLength
    Press A "$row; .+" | Out-Null
    Expect-Logged "^xbox: ${product}: resume ${name}: the Store's queue took it$" $mark 'Resumed' | Out-Null
    Expect-Logged "^focus: $row; Installing\.\.\. [\d.]+%$" $mark 'Resumed, the row' | Out-Null

    # --- Cancel, after its question.
    Press Back "TileMenu: Menu\((Install|Pause installation)\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Cancel installation\) of $name" 3
    Press A "TileMenu: Prompt\(Cancel installation\) of $name" '04-cancel-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'ManageQueue: .+' | Out-Null
    Expect-Logged "^xbox: ${product}: cancel ${name}: the Store's queue took it$" $mark 'Cancelled' | Out-Null
    Expect-Logged "^queue: $name dropped$" $mark 'Cancelled, the Queue' | Out-Null
    if (Test-Path $gameDir) { throw 'its folder is on the drive after Cancel' }

    # --- Installed to the end, from its hub again.
    Press B 'GameHub: .+' | Out-Null
    Walk Left 'GameHub: .+ Hub Install' 4
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^queue: $name joined, asked for$" $mark 'Second install' | Out-Null
    Expect-Logged "^xbox: ${product}: $name installed: the Store says complete$" $mark 'Complete' 150 | Out-Null
    Expect-Logged "^xbox: store queue \(stand-in\): $product complete, written onto the drive: Ok\(\(\)\)$" $mark 'Written onto the drive' | Out-Null
    Expect-Logged "^queue: $name installed$" $mark 'Installed' 60 | Out-Null
    if (-not (Test-Path (Join-Path $gameDir 'Content\MicrosoftGame.config'))) { throw "no package at $gameDir" }
    Start-Sleep -Milliseconds 2500
    Save-Screen '05-installed'

    # --- Uninstall from its Tile menu, after the Dashboard's question.
    Walk B 'Recent\(\d+\).*' 4
    Find-Game $name 'S' 'A'
    Press A 'GameHub: .+ Hub Play' | Out-Null
    Press Start "TileMenu: Menu\(Play game\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Uninstall\) of $name" 6
    Press A "TileMenu: Prompt\(Uninstall\) of $name" '06-uninstall-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^xbox: .+_.+: removing $name's package for this user$" $mark 'Uninstall' | Out-Null
    Expect-Logged "^xbox: .+: $name uninstalled \(1 package\)$" $mark 'Removed' | Out-Null
    Expect-Logged "^queue: $name uninstalled$" $mark 'Uninstalled' 60 | Out-Null
    if (Test-Path $gameDir) { throw 'its folder is still there after Uninstall' }
    Start-Sleep -Milliseconds 2500
    Save-Screen '07-uninstalled'

    # --- Ember Orchard, refused by the queue: the Store's page instead.
    Walk B 'Recent\(\d+\).*' 4
    Find-Game 'Ember Orchard' 'E' 'Q'
    Press A 'GameHub: .+ Hub Install' | Out-Null
    $mark = Get-DashboardLogLength
    $standinMark = Standin-LogLength
    Press A '.+' | Out-Null
    Expect-Logged "^xbox: 9ZZZGPASS102: the Store's queue did not take Ember Orchard \(0x803F8001 .+\): opening ms-windows-store://pdp/\?ProductId=9ZZZGPASS102 instead$" $mark 'Refused, the page instead' | Out-Null
    Expect-Standin 'open-address: asked to open ms-windows-store://pdp/\?ProductId=9ZZZGPASS102' $standinMark 'The page, taken by the stand-in'

    if ($script:dashboard.HasExited) { throw "The Dashboard exited with $($script:dashboard.ExitCode)" }
    $result = 'PASS'
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Dashboard
    Disconnect-Pad
    Remove-Item Env:\DASHBOARD_XBOX_QUEUE_SECONDS, Env:\DASHBOARD_WINDOWS_REGION -ErrorAction SilentlyContinue
    if ($script:data -and (Test-Path $script:data)) { Remove-Item $script:data -Recurse -Force -ErrorAction SilentlyContinue }
    if ($gameDir -and (Test-Path $gameDir)) { Remove-Item $gameDir -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
