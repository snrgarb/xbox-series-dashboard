# M4's mixed library (#81): one Dashboard on every source's fake at once, Steam, Epic, GOG, EA, Ubisoft, Battle.net and
# Xbox, with an App and a game added by hand, walked with the virtual pad and a picture at each rest, to set beside
# the Bundle's. Nothing is launched: the per-store tests launch each store's game (stores-epic-gog,
# stores-ea-ubisoft-battlenet, stores-xbox); this one is about every store's games together.
#
# What it walks, in order:
# - Settings' Games & apps: a copy of the stand-in game added as a game by typing its path, and Notepad as an App
#   (as the apps test adds them; nothing is started).
# - Home and its Recent row, each of whose games' Tile menu names it (`focus: TileMenu: Menu(Play game) of <name>`):
#   the row must hold games of at least two stores.
# - My games & apps' Games, a picture per screenful of its grid (focus on the first game, then every third row down,
#   then SEE FULL LIBRARY), so that every store's mark is seen; Full library, which counts every game of every fake
#   and the hand-added one (`focus: MyGames: FullLibrary: Tile(0); tab 0/<n> "", sort <s>, name "", <count> games`);
#   Apps with the App (`focus: MyGames: Apps: Tile(0); "Notepad", 1 apps`).
# - A Game hub of one installed game from each store and of the hand-added game, each found by its whole name typed
#   in Search with the machine's keyboard (`focus: Search: Games(<i>) <name>; "<name>" away, Games <n>`), then A
#   (`focus: GameHub: <its id> Hub Play`): the pictures show the hub's store chip and mark.
# - Search by one word, "Courier", finding games of two stores (Steam's Ashfall Courier, Xbox's Pebble Courier): the
#   Games row walked to its end, each game's store looked up.
# - An install of a fake Steam game from its hub, as the queue test does it: `store: asking Steam to install <name>`,
#   `queue: <name> joined, asked for`, `queue: <name> queued`, the tile Queued; `standin steam-install --bytes 25/100`,
#   `queue: <name> downloading, 25% of 100 bytes`, the tile Installing...; Manage with its Queue box counted
#   (`focus: MyGames: Manage: Queue`); Manage Queue on the game's row (`focus: ManageQueue: Row(0) of <name>; 1
#   installing, <m> recently installed; Installing... 25.0%`) following the bytes to 50.0%; `--done`, `queue: <name>
#   installed`, and focus going with it to the head of Recently installed (`ManageQueue: Cover(0) of <name>; 0
#   installing, ...`).
# - Uninstall from the game's Tile menu: the prompt opened (`TileMenu: Prompt(Uninstall) of <name>`), Cancel, and no
#   `store: asking Steam to uninstall` line. It is never confirmed.
#
# Safe mode (the TV PC, E2E_SAFE=1): every fake is under the payload's folder (Get-FakeRoot), the hand-added game too,
# and the data folder (Get-TestFolder); nothing is registered and nothing is started but the Dashboard. Each store
# runs there as on the VM, on its fake, since nothing here launches through a store's handler; what does not run in
# safe mode is said in steps.txt rather than failed:
# - Xbox: never the machine's packages. The fake drive is read with --xbox on the VM and in safe mode alike; a package
#   registered with Windows (developer mode, Add-AppxPackage) is the stores-xbox test's, on the VM only.
# - Epic, Ubisoft: their links' handlers are registered on the VM (Initialize-FakeEpic, Initialize-FakeUbisoft) and
#   left to `standin open-address` in safe mode; no game is launched here, so neither is used.
# - Steam: on the VM found through the registry's SteamPath as on a real machine; in safe mode by --steam. The install
#   is the same on both: the stand-in steam.exe in the fake's folder, no handler involved.
#
# It ends as it began: the Steam game's fake install dropped again (`standin steam-install --drop`), the data folder
# (with the App and the hand-added game) and the hand-added game's folder removed.
#
# The pictures are named for docs/reports/m4: 01 to 22, the Games grid's as 03-games-<k>.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$script:installAsked = $false
$dashboardTitle = 'Xbox Series Dashboard'
# The Dashboard's own data folder for the run: the App and the hand-added game live in its database, and go with it.
$data = Get-TestFolder 'dashboard-mixed-data'
# The game added by hand: on the VM at C:\e2e\byhand as the apps test has it; in safe mode under the payload's folder.
$folder = if (Test-SafeMode) { Join-Path $PSScriptRoot 'byhand' } else { 'C:\e2e\byhand' }
$program = Join-Path $folder 'game.exe'
$handGame = 'Hand Game'
$notepad = 'C:\Windows\System32\notepad.exe'
$app = 'Notepad'
# The word Search is to find games of two stores by.
$word = 'Courier'
# The Games grid shows three rows at a time (crates/screens/src/my_games/grid.rs, ROWS): a picture every third row.
$rowsShown = 3

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }
function Esc([string] $Text) { [regex]::Escape($Text) }

# The picture of the screen now, once what moved has come to rest.
function Picture([string] $Name) {
    Start-Sleep -Milliseconds 3000
    Save-Screen $Name
}

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". A press the Dashboard never saw (no new focus line at all) is pressed once more; one that gave another
# line is a failure. Then the picture, if one is asked for. Returns the line, or nothing.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    # Two taps closer than a poll of the pad are one: the button has to be seen up between them.
    Start-Sleep -Milliseconds 150
    $before = @(Focus-Lines).Count
    $pattern = "^focus: $Expect`$"
    Push-Button $Button
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if (-not $got -and @(Focus-Lines).Count -eq $before) {
        Say "$Button -> no line at all: pressed again"
        Start-Sleep -Milliseconds 150
        Push-Button $Button
        $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    }
    if ($Name) { Picture $Name }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# Presses a direction until focus is where $Expect says, $Most times at most: a walk is judged by where it ends.
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

# Presses $Button until the focus log matches $Pattern (a whole focus line), $Most times at most, then $Back until it
# does: rows do not wrap, so going on to the edge and coming back finds what was passed. Returns whether it got there.
function Walk-To([string] $Button, [string] $Back, [string] $Pattern, [string] $What, [int] $Most = 12) {
    foreach ($way in @($Button, $Back)) {
        foreach ($press in 1..$Most) {
            if ((Focus-Now) -match $Pattern) { break }
            Push-Button $way
            Start-Sleep -Milliseconds 800
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

# Waits for the last focus line to match $Expect, which a change of the library (not a press) may bring.
function Expect-Focus([string] $Expect, [string] $Why) {
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $now = Focus-Now; if ($now -match $pattern) { $now } } 10
    if ($got) { Say "${Why}: $got" } else {
        Say "${Why}: NOT '$Expect': $(Focus-Now)"
        $script:failures += "${Why}: focus should be '$Expect', and the log's last is '$(Focus-Now)'"
    }
    $got
}

# A line the Dashboard must log after the mark $After, by this pattern, within $Seconds.
function Expect-Logged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 15) {
    $line = Wait-DashboardLog $Pattern $Seconds $After
    if ($line) { Say "${What}: $line" } else { Say "${What}: no line like '$Pattern'"; $script:failures += "${What}: the Dashboard logged no '$Pattern'" }
    $line
}

# No line by this pattern after the mark $After, $Seconds on.
function Expect-NotLogged([string] $Pattern, [int] $After, [string] $What, [double] $Seconds = 3) {
    Start-Sleep -Seconds $Seconds
    $line = Get-DashboardLog | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -First 1
    if ($line) { Say "${What}: $line, and should not be"; $script:failures += "${What}: the Dashboard logged '$line'" } else { Say "${What}: nothing, as it should be" }
}

# A line the stand-in Steam client must have written, after the first $After of standin.log.
function Expect-Steam([string] $Pattern, [int] $After, [string] $What) {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    $line = Wait-Until { if (Test-Path $log) { Get-Content $log | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -Last 1 } } 10
    if ($line) { Say "${What}: $line" } else { Say "${What}: the stand-in Steam has no line like '$Pattern'"; $script:failures += "${What}: the stand-in Steam client was not asked '$Pattern'" }
}

function Steam-LogLength {
    $log = Join-Path $env:E2E_OUT 'standin.log'
    if (Test-Path $log) { @(Get-Content $log).Count } else { 0 }
}

# Moves the fake Steam's install of $Appid along, as Steam's download would: `standin steam-install`.
function Advance([string] $Appid, [string[]] $To) {
    $standin = Join-Path $PSScriptRoot 'standin.exe'
    $arguments = @('steam-install', "`"$script:steamRoot`"", $Appid) + $To
    $done = Start-Process $standin -ArgumentList $arguments -Wait -PassThru
    Say "steam-install $Appid $($To -join ' '): exit $($done.ExitCode)"
    if ($done.ExitCode -ne 0) { $script:failures += "standin steam-install $Appid $($To -join ' ') exited with $($done.ExitCode)" }
}

# The source's line as it was read, `<key>: <n> games from <dir> in <ms> ms`, which must say as many games as the fake
# was read back with, from $Root (unless $Games is -1: any number, from anywhere); and no other line of the source's,
# which would be one saying it could not be read (a fake's one broken entry, `<key>: passed over: ...`, is meant). After <dir> a source may say which of its files gave what, in
# brackets (Epic's).
function Expect-Read([string] $Key, [string] $Root, [int] $Games) {
    $line = Wait-DashboardLog "^${Key}: \d+ games from .+ in \d+ ms`$" 60
    if (-not $line) { $script:failures += "the Dashboard never said it read ${Key}: no '${Key}: <n> games from' line"; return }
    Say "read: $line"
    if ($Games -ge 0 -and $line -notmatch "^${Key}: $Games games from $([regex]::Escape($Root))( \(.+\))? in \d+ ms`$") {
        $script:failures += "${Key} should have been read as $Games games from ${Root}: '$line'"
    }
    $other = @(Get-DashboardLog | Where-Object { $_ -match "^${Key}: " -and $_ -notmatch "^${Key}: \d+ games from " -and
            $_ -notmatch "^${Key}: passed over: " })
    if ($other.Count) { $script:failures += "${Key} said more than what it read: $($other -join ' | ')" }
}

# Text for SendKeys, its own special characters braced.
function Send-Text([string] $Text) {
    [System.Windows.Forms.SendKeys]::SendWait(($Text -replace '([+^%~(){}\[\]])', '{$1}'))
}

# Types $Text on the machine's keyboard into text entry, which is up, over what it holds now ($Was): that is taken back
# a character at a time first. Then Start keeps it, and focus must be as $Then says.
function Type-Into([string] $Was, [string] $Text, [string] $Then) {
    Press A 'TextEntry: .+' | Out-Null
    if ($Was.Length) {
        [System.Windows.Forms.SendKeys]::SendWait('{BS}' * $Was.Length)
        Start-Sleep -Milliseconds 1500
    }
    Send-Text $Text
    Start-Sleep -Milliseconds 2000
    Say "typed: $(Focus-Now)"
    Press Start $Then | Out-Null
}

# What an Add page's focus line says after the control's name: what is typed so far, and whether Add would add it.
function Typed([string] $Path, [string] $Name, [string] $Ready = 'ready') {
    "; path `"$(Esc $Path)`", name `"$(Esc $Name)`", picture `"`", $Ready"
}

# Home's top bar, on the button $Button: up to the bar, to its left end, then right until it is there.
function Go-TopBar([string] $Button) {
    Walk-ToEdge Up 4
    Walk-ToEdge Left 6
    Walk Right "TopBar\($Button\)" 5
}

# Settings, from Home's top bar, on Games & apps' page, focus on its first Row.
function Open-GamesAndApps {
    Go-TopBar 'Settings'
    Press A 'Settings: Rail: Theme settings' | Out-Null
    Press Right 'Settings: Theme settings: Personalisation' | Out-Null
    Press Down 'Settings: Theme settings: Games & apps' | Out-Null
    Press A 'Settings: GamesAndApps: Add a game; .+' | Out-Null
}

# From Games & apps' page back to Home's top bar, a level at a time.
function Leave-Settings {
    Press B 'Settings: Theme settings: Games & apps' | Out-Null
    Press B 'Settings: Rail: Theme settings' | Out-Null
    Press B 'TopBar\(Settings\)' | Out-Null
}

# The game and the row the Tile menu's line names: `focus: TileMenu: Menu(Install) of Quiet Orbit`.
function Menu-Game([string] $Line) { if ($Line -match ' of (.+)$') { $Matches[1] } }
function Menu-Row([string] $Line) { if ($Line -match '\((.+?)\) of ') { $Matches[1] } }

# The number in a focus line's `Tile(n)`.
function Place([string] $At) { [int] ($At -replace '^.*Tile\((\d+)\).*$', '$1') }

# With focus on My games & apps' nav the item that is up trails focus by the settle wait when focus is passing along
# it, so a line there is judged by the item focus is on, whatever is showing.
function Nav([string] $Item) { "MyGames: .+: Nav\($Item\)(; .*)?" }

# From the Recent row's first tile rightwards, opens each game's Tile menu until one is the menu of $Name; leaves it
# open and returns its line. Which tile has which game is the library's to say.
function Open-MenuOf([string] $Name) {
    Walk Left 'Recent\(0\)'
    foreach ($tile in 1..8) {
        $line = Press Start 'TileMenu: Menu\((Play game|Install)\) of .+'
        if (-not $line) { return $null }
        if ((Menu-Game $line) -eq $Name) { return $line }
        Press B 'Recent\(\d+\)' | Out-Null
        Press Right 'Recent\(\d+\)' | Out-Null
    }
    $script:failures += "no tile of the Recent row is $Name's"
    $null
}

# Finds $Name by its whole name typed in Search, opens its Game hub (whose id is $Id), takes the picture $Picture,
# and comes back to Home. Returns the hub's line.
function Open-HubByName([string] $Name, [string] $Id, [string] $Picture) {
    Press Y 'Search: Key\(Q\); .+' | Out-Null
    Send-Text $Name
    Start-Sleep -Milliseconds 2000
    Say "typed: $(Focus-Now)"
    $found = if ((Focus-Now) -match ', Games (\d+)$') { [int] $Matches[1] } else { 12 }
    Press B "Search: \w+\(0\) .+; `"$(Esc $Name)`" away, .+" | Out-Null
    $hub = $null
    if (Walk-To Right Left "^focus: Search: Games\(\d+\) $(Esc $Name); " $Name ($found + 1)) {
        $hub = Press A "GameHub: $(Esc $Id) Hub \w+" $Picture
        if ($hub) { Press B "Search: Games\(\d+\) $(Esc $Name); .+" | Out-Null }
    }
    Press B 'Recent\(\d+\)' | Out-Null
    $hub
}

function In-Front { if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" } }

try {
    Close-WindowsPrompts
    Connect-Pad

    # --- Every store's fake. C:\e2e\<store> on the VM; under the payload's own folder in safe mode.
    $script:steamRoot = Get-FakeRoot 'steam'
    $stores = [ordered] @{}
    $stores['steam'] = @{ Name = 'Steam'; Root = $script:steamRoot; Library = (Initialize-FakeSteam $script:steamRoot) }
    $stores['epic'] = @{ Name = 'Epic Games'; Root = (Get-FakeRoot 'epic') }
    $stores['gog'] = @{ Name = 'GOG'; Root = (Get-FakeRoot 'gog') }
    $stores['ea'] = @{ Name = 'EA app'; Root = (Get-FakeRoot 'ea') }
    $stores['ubisoft'] = @{ Name = 'Ubisoft Connect'; Root = (Get-FakeRoot 'ubisoft') }
    $stores['battlenet'] = @{ Name = 'Battle.net'; Root = (Get-FakeRoot 'battlenet') }
    $stores['xbox'] = @{ Name = 'Xbox'; Root = (Get-FakeRoot 'xbox') }
    $stores['epic'].Library = Initialize-FakeEpic $stores['epic'].Root
    $stores['gog'].Library = Initialize-FakeGog $stores['gog'].Root
    $stores['ea'].Library = Initialize-FakeEa $stores['ea'].Root
    $stores['ubisoft'].Library = Initialize-FakeUbisoft $stores['ubisoft'].Root
    $stores['battlenet'].Library = Initialize-FakeBattleNet $stores['battlenet'].Root
    $stores['xbox'].Library = Initialize-FakeXbox $stores['xbox'].Root
    if (Test-SafeMode) {
        Say 'safe mode: every store on its fake under the payload''s folder, nothing registered; Steam by --steam, not the registry'
    }
    Say 'Xbox: the fake drive by --xbox, on the VM and in safe mode alike; a package registered with Windows is the stores-xbox test''s (VM only)'

    # Which store each game is of, by name, and the game of each whose hub is opened: its first installed one.
    $storeOf = @{}
    $allGames = 0
    foreach ($key in $stores.Keys) {
        $games = @($stores[$key].Library.games)
        $allGames += $games.Count
        foreach ($g in $games) { $storeOf[$g.name] = $key }
        $stores[$key].Hub = @($games | Where-Object installed) | Select-Object -First 1
        if (-not $stores[$key].Hub) { throw "the fake $key has no installed game" }
    }
    $storeOf[$handGame] = 'byhand'
    $allGames += 1
    $installedStores = @($stores.Keys | Where-Object { @($stores[$_].Library.games | Where-Object installed).Count })
    Say "the mixed library: $allGames games with the hand-added one; installed games from $($installedStores -join ', ') and by hand"

    # The Steam game to install: one the Recent row has (played), not installed.
    $target = @($stores['steam'].Library.games | Where-Object { -not $_.installed -and $_.last_played -gt 0 }) | Select-Object -First 1
    if (-not $target) { throw 'the fake Steam has no played game that is not installed' }
    $targetName = Esc $target.name
    $hub = "GameHub: $($target.source_id)"
    Say "to install: $($target.name) ($($target.source_id))"

    # The word's games: two stores' at least, by the fakes.
    $byWord = @($storeOf.Keys | Where-Object { $_ -match "\b$word" })
    $wordStores = @($byWord | ForEach-Object { $storeOf[$_] } | Sort-Object -Unique)
    if ($wordStores.Count -lt 2) { throw "'$word' is in the names of games of $($wordStores.Count) store(s) only: $($byWord -join ', ')" }
    Say "'$word' should find: $(($byWord | ForEach-Object { "$_ ($($storeOf[$_]))" }) -join ', ')"

    # --- The game added by hand: a copy of the stand-in game, its window titled with the game's name.
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force }
    New-Item -ItemType Directory $folder -Force | Out-Null
    Copy-Item (Join-Path $PSScriptRoot 'standin.exe') $program
    Set-Content (Join-Path $folder 'game-args.txt') "--title`r`n$handGame`r`n" -Encoding ASCII
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    New-Item -ItemType Directory $data -Force | Out-Null

    # Nothing in this test is a question put to the user, however slow the machine.
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $arguments = @('--log-focus', '--data-dir', "`"$data`"")
    foreach ($key in @('epic', 'gog', 'ea', 'ubisoft', 'battlenet', 'xbox')) { $arguments += @("--$key", "`"$($stores[$key].Root)`"") }
    $script:dashboard = Start-Dashboard $arguments
    $started = Wait-DashboardLog 'first frame presented' 60
    if (-not $started) { throw 'The Dashboard never presented a frame' }
    Say "started: $started"

    # Each store read, with all its games.
    Expect-Read 'steam' '' -1
    foreach ($key in @('epic', 'gog', 'ea', 'ubisoft', 'battlenet', 'xbox')) {
        Expect-Read $key $stores[$key].Root (@($stores[$key].Library.games).Count)
    }

    Start-Sleep -Seconds 4
    # The greeting's Toast comes and goes: it is not in the pictures.
    Wait-Greeting | Out-Null
    In-Front

    # --- Settings > Games & apps: the game by hand and the App, by typing their paths (the apps test's way).
    Open-GamesAndApps
    Expect-Focus 'Settings: GamesAndApps: Add a game; 0 games, 0 apps' 'as it begins' | Out-Null
    Press A "Settings: AddGame: Path$(Typed '' '' 'not ready')" | Out-Null
    Type-Into '' $program "Settings: AddGame: Path$(Typed $program 'game')"
    Press Down "Settings: AddGame: Name$(Typed $program 'game')" | Out-Null
    Type-Into 'game' $handGame "Settings: AddGame: Name$(Typed $program $handGame)"
    Walk Down "Settings: AddGame: Add$(Typed $program $handGame)" 4
    $mark = Get-DashboardLogLength
    Press A 'Settings: GamesAndApps: Add a game; .+' | Out-Null
    Expect-Logged "^library: added the game $(Esc $handGame)`$" $mark 'adding the game' | Out-Null
    Expect-Focus 'Settings: GamesAndApps: Add a game; 1 game, 0 apps' 'the game added' | Out-Null
    Press Down 'Settings: GamesAndApps: Add an app; 1 game, 0 apps' | Out-Null
    Press A "Settings: AddApp: Path$(Typed '' '' 'not ready')" | Out-Null
    Type-Into '' $notepad "Settings: AddApp: Path$(Typed $notepad 'notepad')"
    Press Down "Settings: AddApp: Name$(Typed $notepad 'notepad')" | Out-Null
    Type-Into 'notepad' $app "Settings: AddApp: Name$(Typed $notepad $app)"
    Walk Down "Settings: AddApp: Add$(Typed $notepad $app)" 4
    $mark = Get-DashboardLogLength
    Press A 'Settings: GamesAndApps: Add an app; .+' | Out-Null
    Expect-Logged "^library: added the App $(Esc $app)`$" $mark 'adding the App' | Out-Null
    Expect-Focus 'Settings: GamesAndApps: Add an app; 1 game, 1 app' 'the App added' | Out-Null
    Leave-Settings

    # --- Home: the Recent row, each game's store named by its Tile menu.
    Press Down 'Recent\(\d+\)' | Out-Null
    Walk Left 'Recent\(0\)'
    Picture '01-home-recent-row'
    $recent = @()
    foreach ($tile in 1..10) {
        $before = @(Focus-Lines).Count
        Start-Sleep -Milliseconds 150
        Push-Button Start
        $line = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before) { $lines[-1] } } 4
        if ($line -match '^focus: TileMenu: Menu\(.+\) of (.+)$') {
            $name = $Matches[1]
            $recent += "$name ($(if ($storeOf.ContainsKey($name)) { $storeOf[$name] } else { '?' }))"
            Press B 'Recent\(\d+\)' | Out-Null
        } elseif ($line) {
            # Not a game's tile (Play history's): back, and the row's end.
            Say "Menu on the row's last tile: $line"
            Press B 'Recent\(\d+\)' | Out-Null
            break
        } else {
            Say "Menu on the row's last tile: nothing"
            break
        }
        $was = Focus-Now
        Start-Sleep -Milliseconds 150
        Push-Button Right
        Start-Sleep -Milliseconds 1200
        if ((Focus-Now) -eq $was) { break }
    }
    Say "the Recent row: $($recent -join ', ')"
    $recentStores = @($recent | ForEach-Object { if ($_ -match '\((\w+)\)$') { $Matches[1] } } | Where-Object { $_ -ne '?' } | Sort-Object -Unique)
    if ($recentStores.Count -lt 2) { $failures += "the Recent row should hold games of two stores at least, and holds $($recentStores -join ', ')" }
    Picture '02-home-recent-row-end'
    Walk Left 'Recent\(0\)'

    # --- My games & apps, Games: a picture per screenful, every store's games and their marks.
    Go-TopBar 'MyGames'
    Press A 'MyGames: Games: Tile\(0\)' '03-games-1' | Out-Null
    Walk-ToEdge Right
    $columns = (Place (Focus-Now)) + 1
    Walk Left 'MyGames: Games: Tile\(0\)' 8
    $screen = 1
    foreach ($row in 1..20) {
        $line = Press Down 'MyGames: Games: (Tile\(\d+\)|SeeFullLibrary)'
        if (-not $line) { break }
        if ($line -match 'SeeFullLibrary$') {
            $screen += 1
            Picture "03-games-$screen"
            break
        }
        $at = [math]::Floor((Place $line) / $columns)
        # A new screenful: the grid scrolls a row at a time, so the rows under the first screenful's are all in view
        # once focus is on the last of them.
        if ($at -ge 2 * $rowsShown - 1 -and ($at + 1) % $rowsShown -eq 0) {
            $screen += 1
            Picture "03-games-$screen"
        }
    }
    Say "Games: $columns columns, $screen pictures"

    # --- Full library from SEE FULL LIBRARY: every game of every store and the one by hand.
    $full = Press A 'MyGames: FullLibrary: Tile\(0\); tab 0/\d+ "", sort \w+( reversed)?, name "", \d+ games' '04-full-library'
    if ($full -match ', (\d+) games$' -and [int] $Matches[1] -ne $allGames) {
        $failures += "Full library should count every fake's games and the hand-added one, $allGames, and counts $($Matches[1])"
    }

    # --- Apps, with the App.
    Press Left (Nav 'FullLibrary') | Out-Null
    Walk Up (Nav 'Apps') 4
    # Apps shows once focus has rested on its item.
    Start-Sleep -Milliseconds 2500
    Press Right "MyGames: Apps: Tile\(0\); `"$(Esc $app)`", 1 apps" '05-apps' | Out-Null
    Press B 'Recent\(\d+\)' | Out-Null

    # --- A Game hub of each store's game, and of the game added by hand: the hub's store chip and mark.
    $number = 6
    foreach ($key in $stores.Keys) {
        $game = $stores[$key].Hub
        Say "--- $($stores[$key].Name): $($game.name) ($($game.source_id))"
        In-Front
        Open-HubByName $game.name $game.source_id ('{0:d2}-hub-{1}' -f $number, $key) | Out-Null
        $number += 1
    }
    Say "--- by hand: $handGame ($program)"
    Open-HubByName $handGame $program ('{0:d2}-hub-by-hand' -f $number) | Out-Null

    # --- Search by one word: games of two stores.
    Press Y 'Search: Key\(Q\); .+' | Out-Null
    Send-Text $word
    Start-Sleep -Milliseconds 2000
    Say "typed: $(Focus-Now)"
    Press B "Search: Games\(0\) .+; `"$word`" away, Games \d+" '14-search-two-stores' | Out-Null
    $foundStores = @()
    foreach ($press in 1..12) {
        if ((Focus-Now) -match '^focus: Search: Games\(\d+\) (.+); ') {
            $name = $Matches[1]
            if ($storeOf.ContainsKey($name)) { $foundStores += $storeOf[$name] }
            Say "found: $name ($(if ($storeOf.ContainsKey($name)) { $storeOf[$name] } else { '?' }))"
        }
        $was = Focus-Now
        Start-Sleep -Milliseconds 150
        Push-Button Right
        Start-Sleep -Milliseconds 1000
        if ((Focus-Now) -eq $was) { break }
    }
    $foundStores = @($foundStores | Sort-Object -Unique)
    if ($foundStores.Count -lt 2) { $failures += "Search for '$word' should find games of two stores, and found $($foundStores -join ', ')" }
    Press B 'Recent\(\d+\)' | Out-Null

    # --- Install a fake Steam game from its hub: the hub closes, Steam's client is asked, the game joins the Queue.
    In-Front
    Open-MenuOf $target.name | Out-Null
    Press B 'Recent\(\d+\)' | Out-Null
    $tile = (Focus-Now) -replace '^focus: ', ''
    $tilePattern = [regex]::Escape($tile)
    Press A "$hub Hub Install" '15-hub-install' | Out-Null
    $mark = Get-DashboardLogLength
    $steamMark = Steam-LogLength
    $script:installAsked = $true
    Press A $tilePattern | Out-Null
    Expect-Logged "^store: asking Steam to install ${targetName}: app_install $($target.source_id)$" $mark 'Install from the hub' | Out-Null
    Expect-Steam "steam: asked to install $($target.source_id): -silent \+app_install $($target.source_id)" $steamMark 'Install from the hub'
    Expect-Logged "^queue: $targetName joined, asked for$" $mark 'the Queue' | Out-Null
    Expect-Logged "^queue: $targetName queued$" $mark 'Steam took it on' | Out-Null
    In-Front
    Picture '16-tile-queued'

    # Downloading: the tile reads Installing...
    $mark = Get-DashboardLogLength
    Advance $target.source_id @('--bytes', '25/100')
    Expect-Logged "^queue: $targetName downloading, 25% of 100 bytes$" $mark 'Downloading' | Out-Null
    Press Right 'Recent\(\d+\)' | Out-Null
    Picture '17-tile-installing'
    Press Left $tilePattern | Out-Null

    # Manage, its Queue box counting the Queue; Manage Queue on the game's row, following the bytes.
    Go-TopBar 'MyGames'
    Press A 'MyGames: Games: Tile\(0\)' | Out-Null
    Press Left (Nav 'Games') | Out-Null
    Walk Down (Nav 'Manage') 6
    Press Right 'MyGames: Manage: Queue' '18-manage-queue-box' | Out-Null
    $row = "ManageQueue: Row\(0\) of $targetName; 1 installing, \d+ recently installed"
    $opened = Press A "ManageQueue: (Cover\(0\) of .+|Row\(0\) of $targetName); 1 installing, \d+ recently installed(; .+)?"
    if ($opened -notmatch 'Row\(0\)') { Press Down "$row; Installing\.\.\. 25\.0%" | Out-Null }
    $mark = Get-DashboardLogLength
    Advance $target.source_id @('--bytes', '50/100')
    Expect-Logged "^queue: $targetName downloading, 50% of 100 bytes$" $mark 'Downloading, with Manage Queue up' | Out-Null
    Expect-Logged "^focus: $row; Installing\.\.\. 50\.0%$" $mark 'Manage Queue follows the bytes' | Out-Null
    Picture '19-manage-queue-50'

    # Done: the game leaves the Queue installed and heads Recently installed, focus going with it.
    $mark = Get-DashboardLogLength
    Advance $target.source_id @('--done')
    Expect-Logged "^queue: $targetName installed$" $mark 'Done' | Out-Null
    Expect-Logged "^focus: ManageQueue: Cover\(0\) of $targetName; 0 installing, \d+ recently installed$" $mark 'Done, its cover heads Recently installed' | Out-Null
    Picture '20-recently-installed'
    Press B 'MyGames: Manage: Queue' | Out-Null
    Press B 'Recent\(\d+\)' | Out-Null

    # --- Uninstall: the prompt opened and declined, never confirmed.
    $menu = Open-MenuOf $target.name
    if ($menu -and (Menu-Row $menu) -ne 'Play game') { $failures += "$($target.name)'s menu starts with $(Menu-Row $menu) once installed" }
    Walk Down "TileMenu: Menu\(Uninstall\) of $targetName" 6
    Press A "TileMenu: Prompt\(Uninstall\) of $targetName" '21-uninstall-prompt' | Out-Null
    Press Down "TileMenu: Prompt\(Cancel\) of $targetName" | Out-Null
    $mark = Get-DashboardLogLength
    Press A "TileMenu: Menu\(Uninstall\) of $targetName" | Out-Null
    Expect-NotLogged '^store: asking Steam to uninstall' $mark 'Uninstall declined' 2
    Press B 'Recent\(\d+\)' | Out-Null
    Walk Left 'Recent\(0\)'
    Picture '22-home-end'

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
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
    # As it began: the fake Steam's game not installed, the App and the hand-added game gone with the data folder.
    if ($script:installAsked) {
        try {
            $arguments = @('steam-install', "`"$script:steamRoot`"", $target.source_id, '--drop')
            $dropped = Start-Process (Join-Path $PSScriptRoot 'standin.exe') -ArgumentList $arguments -Wait -PassThru
            Say "as it began: steam-install $($target.source_id) --drop: exit $($dropped.ExitCode)"
        } catch { Say "note: could not drop $($target.name) again: $_" }
    }
    if (Test-Path $data) { Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue }
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
