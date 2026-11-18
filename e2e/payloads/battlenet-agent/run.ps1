# Battle.net through its Agent (#172, docs/sources.md "Battle.net through its Agent"), walked with the virtual pad
# against the stand-in Agent: a fake Battle.net (Initialize-FakeBattleNet) whose Agent\Agent.9999\Agent.exe is
# standin.exe, which the Dashboard starts itself (--internalclienttools) when nothing answers on the port its
# Agent\Agent.dat names, and which then answers the Agent's HTTP calls on a free port of 127.0.0.1, writing the fake's
# product.db, aggregate.json and registry as an install finishes. Its catalogue (catalogue.json, read in place of the
# built-in table) offers Larkspur Hollow (owned) and Quarry Bell (not owned); 48 MiB counted out in eight steps
# STANDIN_HELPER_STEP_MS apart. Nothing is downloaded. DASHBOARD_BATTLENET_STEER=1 turns on the Dashboard's guesses at
# pause, resume and cancel, which the stand-in answers (the real Agent's are not known: docs/sources.md).
#
# What it walks:
# - Battle.net read with its catalogue: Larkspur Hollow listed, not installed.
# - Larkspur Hollow from its Game hub: the Agent started (`agent: not answering ...: starting ...Agent.exe
#   --internalclienttools`, `up on port <n> ... (pid <n>)`), a token taken (`battlenet: agent: session: GET /agent ->
#   200 {"authorization":"<token>",...,"session":"<masked>",...}`: never the token, the session or the user's names);
#   its form as the real Agent first answers it (`POST /install -> 201`, language 704, its own folder name
#   LarkspurHollow), filled in unfinalized with a locale of its list, posted again until sized (1005, then 1009),
#   then finalized (`POST /install/larkspur {"finalized":true,...}`), `the Agent took the install`. Then, as the real
#   Agent answered the TV PC's first real install: `/install/larkspur` 404 `{}` (logged whole, and never the end),
#   the progress read on `/update/larkspur` (`progress read on /update/larkspur`), the bytes and speed with the answer
#   read; the Queue's `joined, asked for` and `downloading, <n>% of 50331648 bytes`.
# - Manage Queue, its row; Pause installation (`POST /update/larkspur {"paused":true}`, `pause: the Agent took it`,
#   `queue: ... paused`), Resume installation, Cancel installation after its question (`DELETE /update/larkspur`,
#   `cancelled: the Agent took it`, `queue: ... dropped`, its folder gone; the Agent the Dashboard started stopped:
#   `it exited <n> ms after DELETE /agent`).
# - Installed to the end from its hub again: the Agent the Dashboard started running all the while, no `stopped:`
#   line and no Queue stop, `done: product.db says installed`, `queue: Larkspur Hollow installed`, only then the
#   Agent stopped, and no Agent left running.
# - Play from its hub: `Battle.net.exe --exec=launch lkh` (the stand-in Battle.net's line), the stand-in game; ended.
# - Uninstall from its Tile menu after the Dashboard's question: `POST /uninstall`, `uninstalled: product.db no longer
#   says installed`, `queue: Larkspur Hollow uninstalled`, its folder gone, the Agent stopped.
# - Quarry Bell from its Game hub: the install refused (`error 2221`), the install abandoned (`DELETE
#   /install/quarrybell`), and the client asked on the game instead (`asking Battle.net instead: ...Battle.net.exe
#   --game=quarrybell`, the stand-in Battle.net's `asked for game quarrybell`).
# - Last, Battle.net's own Agent running (the stand-in started with `--session=`, 401 to anyone else, as the TV PC's
#   own answered): Larkspur Hollow's Install starts no second Agent (`no second Agent is started beside it`) and asks
#   the client instead.
#
# Every press is judged by --log-focus before the next, and the walk stops at the first that goes elsewhere. Runs on
# the VM and in safe mode on the TV PC; every folder is the test's own, and the machine's own Battle.net and Agent are
# never read or asked (the fake is found by its folder, its Agent only by its Agent.dat). The screenshots are to look
# at.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$title = 'Xbox Series Dashboard'
$name = 'Larkspur Hollow'
$code = 'lkh'
$uid = 'larkspur'
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

# Presses $Button, then $Back, until focus is where $Expect says: rows and keys do not wrap, so going on to the edge
# and coming back finds what was passed.
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

# A line the Dashboard must log after the mark $After, by this pattern, within $Seconds; the walk stops without it.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 30) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if (-not $line) { Say "${What}: no line like '$Pattern'"; throw "${What}: the Dashboard logged no '$Pattern'" }
    Say "${What}: $line"
    $line
}

# A line the stand-ins must have written to standin.log after its first $After lines.
function Expect-Standin([string] $Pattern, [int] $After, [string] $What) {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    $line = Wait-Until { if (Test-Path $log) { Get-Content $log | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } } 15
    if (-not $line) { throw "${What}: the stand-in wrote no '$Pattern'" }
    Say "${What}: $line"
}
function Standin-LogLength { $log = Join-Path $env:E2E_OUT 'standin.log'; if (Test-Path $log) { @(Get-Content $log).Count } else { 0 } }

function In-Front { if ((Get-Foreground).Title -ne $title) { Say "note: put in front: $(Set-Foreground $title)" } }

# Y on Home, the game's first letter on the Console keyboard (down its first column to the row, along the row to the
# key), B, then along the results until the log names the game. Ends on its result.
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
    $script:root = Get-FakeRoot 'battlenet'
    $library = Initialize-FakeBattleNet $script:root
    $listed = @($library.games | Where-Object { $_.name -eq $name })
    if ($listed.Count -ne 1 -or $listed[0].installed) { throw "the fake Battle.net should list $name, not installed" }
    $script:data = Get-FakeRoot 'battlenet-data'
    if (Test-Path $script:data) { Remove-Item $script:data -Recurse -Force }
    New-Item $script:data -ItemType Directory -Force | Out-Null
    Register-Fake 'battlenet-data' $script:data
    $agentDir = Join-Path $script:root 'Agent'
    # The folder the stand-in Agent's form names Larkspur Hollow's: not its name, as the real one's for Tony Hawk's.
    $dirname = 'LarkspurHollow'
    $gameDir = Join-Path $script:root "games\$dirname"
    $gameJson = [regex]::Escape(($gameDir -replace '\\', '\\'))

    # The stand-in Agent's pace, slow enough to steer with the pad; the Dashboard's guesses at steering on.
    $env:STANDIN_HELPER_STEP_MS = '3000'
    $env:DASHBOARD_BATTLENET_STEER = '1'
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    $script:dashboard = Start-Dashboard @('--log-focus', '--battlenet', "`"$script:root`"", '--data-dir', "`"$script:data`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Expect-Logged "^battlenet: $(@($library.games).Count) games from $([regex]::Escape($script:root)) in \d+ ms$" 0 'Read with its catalogue' 60 | Out-Null
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 3
    In-Front
    Say "focus after the start: $(Focus-Now)"

    # --- Larkspur Hollow from its Game hub: the Agent installs it into the fake's games folder.
    Find-Game $name 'L' 'A'
    Press A "GameHub: $code Hub Install" '02-hub-install' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^battlenet: ${code}: installing $uid \($name\) under $([regex]::Escape((Join-Path $script:root 'games'))) through the Agent$" $mark 'Install' | Out-Null
    Expect-Logged "^battlenet: agent: not answering \(.+\): starting $([regex]::Escape($agentDir))\\Agent\.9999\\Agent\.exe --internalclienttools$" $mark 'The Agent started' | Out-Null
    Expect-Logged '^battlenet: agent: up on port \d+ after \d+ ms \(pid \d+\)$' $mark 'The Agent up' | Out-Null
    $session = Expect-Logged '^battlenet: agent: session: GET /agent -> 200 \{.*"authorization":"<token>".*\}$' $mark 'A token, masked'
    foreach ($field in 'authorization', 'session', 'user_id', 'user_name') {
        if ($session -notmatch "`"$field`":`"<(token|masked)>`"") { throw "$field was said: $session" }
    }
    # The form, as the real Agent first answers it: no language chosen (704), its own folder name; then filled in
    # unfinalized, again until the size is worked out (1005, then 1009), then finalized.
    Expect-Logged "^battlenet: agent: install ${code}: POST /install \{.*\} -> 201 \{.*`"error`":704.*\}$" $mark 'The first form' | Out-Null
    Expect-Logged "^battlenet: ${code}: the Agent's form: language 704 \(no language chosen yet\); locales enUS, enGB, frFR, deDE; its folder `"$dirname`" .*size state 1005; not finalized$" $mark 'The form read' | Out-Null
    $filling = Expect-Logged "^battlenet: ${code}: filling it in: locale (enUS|enGB|frFR|deDE) \(Windows' .+\), into $([regex]::Escape($gameDir))$" $mark 'Filled in'
    $locale = if ($filling -match 'locale (\w+) ') { $Matches[1] } else { 'enUS' }
    Expect-Logged "^battlenet: agent: install ${code}: POST /install/$uid \{.*`"finalized`":false,`"game_dir`":`"$gameJson`".*`"selected_locale`":`"$locale`".*\} -> 200 .+$" $mark 'Unfinalized, its folder and locale' | Out-Null
    Expect-Logged "^battlenet: ${code}: the form filled in: locales .*size state 1009; not finalized$" $mark 'Sized' | Out-Null
    Expect-Logged "^battlenet: agent: install ${code}: POST /install/$uid \{.*`"finalized`":true.*\} -> 200 \{.*`"finalized`":true.*\}$" $mark 'Finalized' | Out-Null
    Expect-Logged "^battlenet: ${code}: the Agent took the install$" $mark 'Taken' | Out-Null
    Expect-Logged "^battlenet: ${code}: its progress read on /update/$uid, /install/$uid, the game on /game/$uid$" $mark 'Where its progress is' | Out-Null
    # As the TV PC's real Agent: /install/<uid> is 404 {} once finalized, and that is not the end.
    Expect-Logged "^battlenet: ${code}: /install/${uid}: first answer: 404 \{\}$" $mark 'The install''s path, 404' | Out-Null
    Expect-Logged "^battlenet: ${code}: /update/${uid}: first answer: 200 \{.+\}$" $mark 'The update''s path, whole' | Out-Null
    Expect-Logged "^battlenet: ${code}: progress read on /update/$uid$" $mark 'Read on the update''s path' | Out-Null
    Expect-Logged "^queue: $name joined, asked for$" $mark 'The Queue' | Out-Null
    Expect-Logged "^battlenet: ${code}: downloading, \d+ of $whole bytes, \d+ bytes a second; /update/$uid said \{.+\}$" $mark 'The Agent''s bytes and speed' | Out-Null
    Expect-Logged "^queue: $name downloading, \d+% of $whole bytes$" $mark 'Its bytes in the Queue' | Out-Null

    # --- Manage Queue from its hub's Queue action; its row; View on it.
    Walk-Either Right Left "Search: \w+\(\d+\) $name;.*" 1
    Press A "GameHub: $code Hub (Queued|Installing)" | Out-Null
    Walk Right "GameHub: $code Hub Queue" 3
    Press A 'ManageQueue: .+' | Out-Null
    $row = "ManageQueue: Row\(0\) of $name; 1 installing, \d+ recently installed"
    Walk Down "$row; Installing\.\.\. [\d.]+%" 3
    Save-Screen '03-row'

    # --- Pause.
    Press Back "TileMenu: Menu\((Install|Pause installation)\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Pause installation\) of $name" 2
    $mark = Get-DashboardLogLength
    Press A "$row; .+" | Out-Null
    Expect-Logged "^battlenet: agent: pause ${code}: POST /update/$uid \{`"paused`":true\} -> 200 .+$" $mark 'Pause asked' | Out-Null
    Expect-Logged "^battlenet: ${code}: pause: the Agent took it$" $mark 'Paused, the Agent' | Out-Null
    Expect-Logged "^battlenet: ${code}: paused at \d+ of $whole bytes; /update/$uid said .+$" $mark 'Paused, its bytes' | Out-Null
    Expect-Logged "^queue: $name paused(, \d+% of $whole bytes)?$" $mark 'Paused, the Queue' | Out-Null
    Expect-Logged "^focus: $row; Paused [\d.]+%$" $mark 'Paused, the row' | Out-Null
    Start-Sleep -Milliseconds 2500
    Save-Screen '04-paused'

    # --- Resume.
    Press Back "TileMenu: Menu\((Install|Resume installation)\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Resume installation\) of $name" 2
    $mark = Get-DashboardLogLength
    Press A "$row; .+" | Out-Null
    Expect-Logged "^battlenet: ${code}: resume: the Agent took it$" $mark 'Resumed' | Out-Null
    Expect-Logged "^battlenet: ${code}: downloading, \d+ of $whole bytes, \d+ bytes a second; .+$" $mark 'Downloading again' | Out-Null
    Expect-Logged "^focus: $row; Installing\.\.\. [\d.]+%$" $mark 'Resumed, the row' | Out-Null

    # --- Cancel, after its question.
    Press Back "TileMenu: Menu\((Install|Pause installation)\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Cancel installation\) of $name" 3
    Press A "TileMenu: Prompt\(Cancel installation\) of $name" '05-cancel-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A 'ManageQueue: .+' | Out-Null
    Expect-Logged "^battlenet: agent: cancel ${code}: DELETE /update/$uid -> 200 .+$" $mark 'Cancel asked' | Out-Null
    Expect-Logged "^battlenet: ${code}: cancelled: the Agent took it$" $mark 'Cancelled' | Out-Null
    Expect-Logged "^queue: $name dropped$" $mark 'Cancelled, the Queue' | Out-Null
    Expect-Logged '^battlenet: agent: the Agent the Dashboard started \(pid \d+, port \d+\) stopped: it exited \d+ ms after DELETE /agent$' $mark 'Its Agent stopped' 20 | Out-Null
    if (Test-Path $gameDir) { throw "its folder is still there after Cancel" }

    # --- Installed to the end, from its hub again.
    Press B 'GameHub: .+' | Out-Null
    Walk Left "GameHub: $code Hub Install" 4
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^queue: $name joined, asked for$" $mark 'Second install' | Out-Null
    Expect-Logged "^battlenet: ${code}: downloading, \d+ of $whole bytes, \d+ bytes a second; .+$" $mark 'Downloading' 60 | Out-Null
    if (-not @(Get-OwnProcess Agent).Count) { throw 'no Agent running while the install is under way' }
    Expect-Logged "^battlenet: ${code}: done: product\.db says installed$" $mark 'Done' 90 | Out-Null
    # Nothing took it for stopped, and the Agent the Dashboard started was let go only once it was done.
    $after = @(Get-DashboardLog | Select-Object -Skip $mark)
    $stops = @($after | Where-Object { $_ -match "^battlenet: ${code}: stopped: |^queue: $name stopped" })
    if ($stops.Count) { throw "the install was taken for stopped: $($stops[0])" }
    $doneAt = [array]::FindIndex([string[]] $after, [Predicate[string]] { param($l) $l -match "^battlenet: ${code}: done: " })
    $letGoAt = [array]::FindIndex([string[]] $after, [Predicate[string]] { param($l) $l -match '^battlenet: agent: the Agent the Dashboard started \(pid' })
    if ($letGoAt -ge 0 -and $letGoAt -lt $doneAt) { throw "the Agent the Dashboard started was stopped before the install was done: $($after[$letGoAt])" }
    Expect-Logged "^queue: $name installed$" $mark 'Installed' 60 | Out-Null
    Expect-Logged '^battlenet: agent: the Agent the Dashboard started \(pid \d+, port \d+\) stopped: it exited \d+ ms after DELETE /agent$' $mark 'Its Agent stopped' 20 | Out-Null
    if (Wait-Until { -not @(Get-OwnProcess Agent).Count } 10) { Say 'no Agent left running' } else { throw 'an Agent is left running after the install' }
    if (-not (Test-Path (Join-Path $gameDir 'game.exe'))) { throw "no game.exe in $gameDir" }

    # --- Play, through the stand-in Battle.net.
    if ((Focus-Now) -notmatch "^focus: Search: \w+\(\d+\) $name;") { throw "after Install, focus is not on $name's result: '$(Focus-Now)'" }
    Press A "GameHub: $code Hub Play" '06-hub-play' | Out-Null
    $mark = Get-DashboardLogLength
    $standinMark = Standin-LogLength
    Start-Sleep -Milliseconds 150
    Push-Button A
    Expect-Logged "^launch: $name \($code\) asked of Battle\.net, Launch screen up at \d+ ms$" $mark 'Play' 15 | Out-Null
    Expect-Standin "battlenet: asked to launch $code" $standinMark 'Started through Battle.net'
    if (-not (Wait-Until { @(Get-StandInGame).Count } 30)) { throw "$name's stand-in game never started" }
    Expect-Logged '^launch: the game has booted' $mark 'Booted' 60 | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '07-playing'
    Stop-StandInGame
    Expect-Logged '^launch: the game has ended, Home back at \d+ ms$' $mark 'Ended' 20 | Out-Null
    Start-Sleep -Milliseconds 1500
    In-Front

    # --- Uninstall from its Tile menu, after the Dashboard's question.
    Find-Game $name 'L' 'A'
    Press A "GameHub: $code Hub Play" | Out-Null
    Press Start "TileMenu: Menu\(Play game\) of $name" | Out-Null
    Walk Down "TileMenu: Menu\(Uninstall\) of $name" 6
    Press A "TileMenu: Prompt\(Uninstall\) of $name" '08-uninstall-question' | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^battlenet: ${code}: uninstalling $uid through the Agent$" $mark 'Uninstall' | Out-Null
    Expect-Logged "^battlenet: agent: uninstall ${code}: POST /uninstall \{.*`"uid`":`"$uid`".*\} -> 200 .+$" $mark 'Asked of the Agent' | Out-Null
    Expect-Logged "^battlenet: ${code}: uninstalled: product\.db no longer says installed, \d+ ms on$" $mark 'Gone from product.db' | Out-Null
    Expect-Logged "^queue: $name uninstalled$" $mark 'Uninstalled' 60 | Out-Null
    Expect-Logged '^battlenet: agent: the Agent the Dashboard started \(pid \d+, port \d+\) stopped: .+$' $mark 'Its Agent stopped' 20 | Out-Null
    if (Test-Path $gameDir) { throw 'its folder is still there after Uninstall' }
    Start-Sleep -Milliseconds 2500
    Save-Screen '09-uninstalled'

    # --- Quarry Bell, not owned: the Agent asked, refused; the client opened on the game instead. Last: its row stays
    # in the Queue as asked for until the Queue gives it up, a minute on.
    # The uninstall was asked from the hub, which stays: back out to Home, where Y is Search.
    Walk B 'Recent\(\d+\).*' 4
    Find-Game 'Quarry Bell' 'Q' 'Q'
    Press A 'GameHub: qbl Hub Install' '10-hub-not-owned' | Out-Null
    $mark = Get-DashboardLogLength
    $standinMark = Standin-LogLength
    Press A '.+' | Out-Null
    Expect-Logged '^battlenet: qbl: installing quarrybell \(Quarry Bell\) under .+ through the Agent$' $mark 'Asked of the Agent' | Out-Null
    Expect-Logged '^battlenet: agent: install qbl: POST /install \{.*"uid":"quarrybell".*\} -> 200 \{.*"error":2221.*\}$' $mark 'Refused' | Out-Null
    Expect-Logged '^battlenet: qbl: the install abandoned \(error 2221: the product is not available to this account\): DELETE /install/quarrybell -> \d+.*; the Agent the Dashboard started is stopped$' $mark 'Nothing left in the Agent' | Out-Null
    Expect-Logged "^battlenet: qbl: the Agent did not take the install \(error 2221: the product is not available to this account\): asking Battle\.net instead: .+Battle\.net\.exe --game=quarrybell$" $mark 'The client asked' | Out-Null
    Expect-Standin 'battlenet: asked for game quarrybell' $standinMark 'The stand-in client'
    Start-Sleep -Seconds 2
    In-Front

    # --- Battle.net's own Agent running (the stand-in started as the client starts its own, with a session: 401 to
    # anyone else, as the TV PC's): no second Agent is started, and the client is asked instead.
    $standinMark = Standin-LogLength
    $clients = Start-Process (Join-Path $agentDir 'Agent.9999\Agent.exe') -ArgumentList '--session=standin-client' -PassThru -WindowStyle Hidden
    Expect-Standin 'battlenet agent: serving .+ as Clients' $standinMark "Battle.net's own Agent (stand-in)"
    Walk B 'Recent\(\d+\).*' 4
    Find-Game $name 'L' 'A'
    Press A "GameHub: $code Hub Install" | Out-Null
    $mark = Get-DashboardLogLength
    Press A '.+' | Out-Null
    Expect-Logged "^battlenet: agent: an Agent the Dashboard did not start answers on port \d+ and gives it no token \(port \d+ \(Agent\.dat\): 401, no token.*\): Battle\.net's own; no second Agent is started beside it$" $mark 'Not beside it' | Out-Null
    Expect-Logged "^battlenet: ${code}: the Agent did not take the install \(.+\): asking Battle\.net instead: .+Battle\.net\.exe --game=$uid$" $mark 'The client asked' | Out-Null
    $agents = @(Get-OwnProcess Agent)
    if ($agents.Count -ne 1 -or $agents[0].Id -ne $clients.Id) { throw "an Agent was started beside Battle.net's own: $(@($agents | ForEach-Object { $_.Id }) -join ', ')" }
    Stop-Process -Id $clients.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    In-Front

    # The token, the session and the user's names were never said in the Dashboard's log.
    $said = @(Get-DashboardLog | Where-Object { $_ -match 'standin-\d+-\d+|48151623421234|S-1-5-21-1111|standin-user' })
    if ($said.Count) { throw "a secret of the Agent's is in the Dashboard's log: $($said[0])" }

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
    foreach ($own in 'Agent', 'Battle.net') { Get-OwnProcess $own | Stop-Process -Force -ErrorAction SilentlyContinue }
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
