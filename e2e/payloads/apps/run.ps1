# Apps, and a game added by hand (docs/spec/apps.md), walked with the virtual pad on the fake Steam library: a copy of
# the stand-in game added as a game by typing its path in Settings' Games & apps, and Notepad added as an App; each
# seen where it belongs (the game in Search's Games row and its Game hub, the App in My games & apps' Apps and not in
# Search's games); the game launched (Launch screen, the stand-in up, ended, Home back, first in the Recent row) and
# the App started (Notepad in front, no Launch screen); both removed; and everything as it began.
#
# Judged by the Dashboard's log, a press at a time. --log-focus says where focus is after every press that moved it
# or changed what the page says beside it: `focus: Settings: GamesAndApps: App Notepad; 1 game, 1 app`,
# `focus: Settings: AddGame: Path; path "C:\e2e\byhand\game.exe", name "game", picture "", ready`,
# `focus: MyGames: Apps: Tile(0); "Notepad", 1 apps`, `focus: GameHub: C:\e2e\byhand\game.exe Hub Play`. What the
# library database did is `library: added the game Hand Game`, `library: removed the App Notepad`; an App started is
# `apps: started Notepad: C:\Windows\System32\notepad.exe`.
#
# The pictures are to look at beside the Bundle's of Settings (docs/spec/pictures.md, 30 and 32) and of My games &
# apps; the Bundle has no picture of its Apps pane with Apps in it (05 shows it empty), so 08 is the first of one.
#
# The data folder and the stand-in's copy are the test's own, made as it starts and removed at the end.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Join-Path $env:TEMP 'dashboard-apps-test'
# The game added by hand: on the VM at C:\e2e\byhand as it always was; in safe mode under the payload's own folder,
# since nothing of the test's goes anywhere else on someone's machine, and only a process from there is the test's.
$folder = if (Test-SafeMode) { Join-Path $PSScriptRoot 'byhand' } else { 'C:\e2e\byhand' }
$program = Join-Path $folder 'game.exe'
$game = 'Hand Game'
$notepad = 'C:\Windows\System32\notepad.exe'
$app = 'Notepad'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }
function Esc([string] $Text) { [regex]::Escape($Text) }

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

# One press that must leave focus where it is: an edge, or a button with nothing to do there.
function Press-Nothing([string] $Button, [string] $Why) {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    Start-Sleep -Milliseconds 1500
    if (@(Focus-Lines).Count -eq $before) {
        Say "$Button -> nothing ($Why)"
    } else {
        Say "$Button -> moved focus, and should not have ($Why): $(Focus-Now)"
        $script:failures += "$Button should have done nothing ($Why), and the log's last is '$(Focus-Now)'"
    }
}

# Home's top bar, on the button $Button: up to the bar, to its left end, then right until it is there.
function Go-TopBar([string] $Button) {
    Walk-ToEdge Up 4
    Walk-ToEdge Left 6
    Walk Right "TopBar\($Button\)" 5
}

# Waits for the last focus line to match $Expect, which a change of the library (not a press) may bring.
function Expect-Focus([string] $Expect, [string] $Why) {
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $now = Focus-Now; if ($now -match $pattern) { $now } } 10
    if ($got) { Say "${Why}: $got" } else {
        Say "${Why}: NOT '$Expect': $(Focus-Now)"
        $script:failures += "${Why}: focus should be '$Expect', and the log's last is '$(Focus-Now)'"
    }
}

# A line the Dashboard must log after the mark $After, by this pattern.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What) {
    $line = Wait-DashboardLog $Pattern 10 $After
    if ($line) { Say "${What}: $line" } else { Say "${What}: no line like '$Pattern'"; $script:failures += "${What}: the Dashboard logged no '$Pattern'" }
    $line
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

# Presses a direction until it moves focus no more: an end of a list or a row, which do not wrap.
function Walk-ToEdge([string] $Button, [int] $Most = 14) {
    foreach ($press in 1..$Most) {
        $was = Focus-Now
        Push-Button $Button
        Start-Sleep -Milliseconds 1500
        if ((Focus-Now) -eq $was) { break }
    }
    Say "$Button to the end: $(Focus-Now)"
}

# Types $Text on the VM's keyboard into text entry, which is up, over what it holds now ($Was): that is taken back a
# character at a time first. Then Start keeps it, and focus must be as $Then says.
function Type-Into([string] $Was, [string] $Text, [string] $Then, [string] $Name = '') {
    Press A 'TextEntry: .+'
    if ($Was.Length) {
        [System.Windows.Forms.SendKeys]::SendWait('{BS}' * $Was.Length)
        Start-Sleep -Milliseconds 1500
    }
    [System.Windows.Forms.SendKeys]::SendWait($Text)
    Start-Sleep -Milliseconds 2000
    Say "typed: $(Focus-Now)"
    if ($Name) { Save-Screen $Name }
    Press Start $Then
}

# What an Add page's focus line says after the control's name: what is typed so far, and whether Add would add it.
function Typed([string] $Path, [string] $Name, [string] $Ready = 'ready') {
    "; path `"$(Esc $Path)`", name `"$(Esc $Name)`", picture `"`", $Ready"
}

# Settings, from Home's top bar, on Games & apps' page, focus on its first Row.
function Open-GamesAndApps {
    Go-TopBar 'Settings'
    Press A 'Settings: Rail: Theme settings'
    Press Right 'Settings: Theme settings: Personalisation'
    Press Down 'Settings: Theme settings: Guide'
    Press Down 'Settings: Theme settings: Games & apps'
    Press A 'Settings: GamesAndApps: Add a game; .+'
}

# From Games & apps' page back to Home, a level at a time.
function Leave-Settings {
    Press B 'Settings: Theme settings: Games & apps'
    Press B 'Settings: Rail: Theme settings'
    Press B 'TopBar\(Settings\)'
}

# A Row's menu on Games & apps' page, then Remove, its question, and Remove: the Row goes.
function Remove-Row([string] $Row, [string] $Name, [string] $Left, [string] $Picture = '') {
    Walk-ToEdge Up
    Walk Down "Settings: GamesAndApps: $Row $(Esc $Name); .+" 8
    Press A "Settings: GamesAndApps: Menu\(Rename\) of $(Esc $Name)"
    Press Down "Settings: GamesAndApps: Menu\(Remove\) of $(Esc $Name)"
    Press A "Settings: GamesAndApps: Prompt\(Remove\) of $(Esc $Name)" $Picture
    $mark = Get-DashboardLogLength
    Press A "Settings: GamesAndApps: .+; $Left"
    $what = if ($Row -eq 'Game') { 'game' } else { 'App' }
    Expect-Logged "^library: removed the $what $(Esc $Name)`$" $mark "removing $Name" | Out-Null
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    # The game added by hand: a copy of the stand-in game, its window titled with the game's name.
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force }
    New-Item -ItemType Directory $folder -Force | Out-Null
    Copy-Item (Join-Path $PSScriptRoot 'standin.exe') $program
    Set-Content (Join-Path $folder 'game-args.txt') "--title`r`n$game`r`n" -Encoding ASCII
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    # Nothing in this test is a question put to the user, however slow the VM.
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', $data)
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    # The greeting's Toast comes and goes: it is not in the pictures.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }

    # --- Settings > Games & apps: nothing added yet.
    Open-GamesAndApps
    Expect-Focus 'Settings: GamesAndApps: Add a game; 0 games, 0 apps' 'as it begins'
    Save-Screen '01-games-and-apps-empty'

    # --- Add a game: its path typed, the name it offers changed, and Add.
    Press A "Settings: AddGame: Path$(Typed '' '' 'not ready')" '02-add-a-game'
    Type-Into '' $program "Settings: AddGame: Path$(Typed $program 'game')"
    Press Down "Settings: AddGame: Name$(Typed $program 'game')"
    Type-Into 'game' $game "Settings: AddGame: Name$(Typed $program $game)"
    Walk Down "Settings: AddGame: Add$(Typed $program $game)" 4
    Start-Sleep -Milliseconds 2500
    Save-Screen '03-add-a-game-ready'
    $mark = Get-DashboardLogLength
    Press A 'Settings: GamesAndApps: Add a game; .+'
    Expect-Logged "^library: added the game $(Esc $game)`$" $mark 'adding the game' | Out-Null
    Expect-Focus 'Settings: GamesAndApps: Add a game; 1 game, 0 apps' 'the game added'

    # --- Add an app: Notepad, by its path; the Store apps listed under it are only looked at.
    Press Down 'Settings: GamesAndApps: Add an app; 1 game, 0 apps'
    Press A "Settings: AddApp: Path$(Typed '' '' 'not ready')"
    Type-Into '' $notepad "Settings: AddApp: Path$(Typed $notepad 'notepad')"
    Press Down "Settings: AddApp: Name$(Typed $notepad 'notepad')"
    Type-Into 'notepad' $app "Settings: AddApp: Name$(Typed $notepad $app)"
    Walk Down "Settings: AddApp: Add$(Typed $notepad $app)" 4
    $listed = Get-DashboardLog | Where-Object { $_ -match '^apps: \d+ Microsoft Store apps listed' } | Select-Object -First 1
    Say "Store apps: $(if ($listed) { $listed } else { 'none listed (not a failure: docs/spec/apps.md, 4.3)' })"
    Start-Sleep -Milliseconds 2500
    Save-Screen '04-add-an-app-ready'
    if ($listed) {
        Press Down 'Settings: AddApp: Store\(0\); .+' '05-add-an-app-store-apps'
        Press Up "Settings: AddApp: Add$(Typed $notepad $app)"
    }
    $mark = Get-DashboardLogLength
    Press A 'Settings: GamesAndApps: Add an app; .+'
    Expect-Logged "^library: added the App $(Esc $app)`$" $mark 'adding the App' | Out-Null
    Expect-Focus 'Settings: GamesAndApps: Add an app; 1 game, 1 app' 'the App added'
    Walk Down "Settings: GamesAndApps: App $(Esc $app); 1 game, 1 app" 4
    Start-Sleep -Milliseconds 2500
    Save-Screen '06-games-and-apps-both'
    Leave-Settings

    # --- The App is in My games & apps' Apps.
    Go-TopBar 'MyGames'
    Press A 'MyGames: Games: Tile\(0\)'
    Press Left 'MyGames: Games: Nav\(Games\)'
    Press Down 'MyGames: \w+: Nav\(Apps\)(; .*)?'
    Start-Sleep -Milliseconds 2500
    Save-Screen '07-apps-pane'
    Press Right "MyGames: Apps: Tile\(0\); `"$(Esc $app)`", 1 apps" '08-apps-pane-focused'

    # --- A starts it: Notepad in front, no Launch screen, no Play session.
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Logged "^apps: started $(Esc $app): $(Esc $notepad)`$" $mark 'starting the App' | Out-Null
    $front = Wait-Until { $fg = Get-Foreground; if ($fg.Process -match '^notepad$') { $fg } } 15
    if ($front) { Say "in front: $($front.Process) '$($front.Title)'" } else { $failures += "Notepad should be in front, and '$((Get-Foreground).Process)' is" }
    Start-Sleep -Milliseconds 1500
    Save-Screen '09-notepad-in-front'
    $after = @(Get-DashboardLog | Select-Object -Skip $mark)
    if ($after -match 'Launch screen up') { $failures += 'starting an App put the Launch screen up' }
    if ($after -match 'Play session started') { $failures += 'starting an App started a Play session' }
    Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put back in front: $(Set-Foreground 'Xbox Series Dashboard')" }
    Start-Sleep -Milliseconds 1000
    # B on My games & apps is Home on the Recent row's game last focused, not the top bar: only Search and
    # Settings give focus back to their button (docs/spec/home.md, 7.4; the pages payload).
    Press B 'Recent\(\d+\)'

    # --- Search: the App is not a game; the game added by hand is, and its Game hub plays it.
    Press Y 'Search: Key\(Q\); .+'
    [System.Windows.Forms.SendKeys]::SendWait($app)
    Start-Sleep -Milliseconds 2500
    if ((Focus-Now) -notmatch "`"$(Esc $app)\|`" .+, no results`$") { $failures += "Search should find no game for '$app': '$(Focus-Now)'" } else { Say "Search for ${app}: $(Focus-Now)" }
    [System.Windows.Forms.SendKeys]::SendWait('{BS}' * $app.Length)
    Start-Sleep -Milliseconds 1500
    [System.Windows.Forms.SendKeys]::SendWait($game)
    Start-Sleep -Milliseconds 2500
    Say "Search for ${game}: $(Focus-Now)"
    Press B "Search: Games\(0\) $(Esc $game); `"$(Esc $game)`" away, Games \d+" '10-search-the-game'
    Press A "GameHub: $(Esc $program) Hub Play" '11-game-hub'

    $mark = Get-DashboardLogLength
    Push-Button A
    $up = Expect-Logged "^launch: $(Esc $game) \($(Esc $program)\) asked of Added by hand, Launch screen up" $mark 'the launch'
    if (-not (Wait-DashboardLog 'the game has booted' 60 $mark)) { throw 'The game never counted as booted' }
    Expect-Logged "^library: $(Esc $game)'s Play session started`$" $mark 'its Play session' | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Screen '12-the-game'
    $running = @(Get-StandInGame | Where-Object { $_.Path -eq $program })
    if (-not $running.Count) { $failures += "the stand-in at $program was not running" } else { Say "running: $($running[0].Path)" }
    Stop-StandInGame
    $ended = Wait-DashboardLog 'the game has ended, Home back' 20 $mark
    if (-not $ended) { $failures += 'Home did not come back after the game ended' }
    Expect-Logged '^library: the Play session ended$' $mark 'its Play session' | Out-Null
    Start-Sleep -Milliseconds 2000

    # First in the Recent row: its tile's Game hub is the game's.
    if ((Focus-Now) -notmatch 'Recent\(\d+\)$') { Press Down 'Recent\(\d+\)' }
    Walk Left 'Recent\(0\)' 10
    Save-Screen '13-home-after-the-game'
    Press A "GameHub: $(Esc $program) Hub Play"
    Press B 'Recent\(0\)'

    # --- Both removed: Settings' page is as it began, and so is Home.
    Open-GamesAndApps
    Remove-Row 'Game' $game '0 games, 1 app' '14-remove-prompt'
    Remove-Row 'App' $app '0 games, 0 apps'
    Expect-Focus 'Settings: GamesAndApps: .+; 0 games, 0 apps' 'all removed'
    Save-Screen '15-games-and-apps-after'
    Leave-Settings
    Go-TopBar 'MyGames'
    Press A 'MyGames: Games: Tile\(0\)'
    Press Left 'MyGames: Games: Nav\(Games\)'
    Press Down 'MyGames: \w+: Nav\(Apps\)(; .*)?'
    # Apps shows once focus has rested on its item: a line of its own.
    Start-Sleep -Milliseconds 2000
    Press-Nothing Right 'Apps is empty: focus stays on the nav'
    Press B 'Recent\(\d+\)'

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Stop-Dashboard
    Disconnect-Pad
    if (Test-Path $data) { Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue }
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
