# Smoke: the Dashboard starts on a fake Steam library and draws Home, the virtual pad moves focus, A launches a game
# through the Launch screen, the Guide button opens the Guide over the game and closes it, and Home comes back when the
# game ends. The game is the stand-in, started by the stand-in Steam (lib\E2E.psm1, Initialize-FakeSteam).
#
# The Launch screen and the Guide over the game are on the Overlay, and are checked by the colour of the screen at
# points of the Dashboard's 1920x1080 canvas where it is known what each draws: the log saying so is not enough (#23).
#
# The sounds are tones `standin make-tones` writes under the console's sounds' names: the Dashboard is pointed at them
# with --media and says what it plays with --log-sounds. Whether anything is heard is not something this can tell.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
# How far Right the test goes along the Recent row before pressing A. In the fixture the Recent row is Ashfall Courier,
# Lanternfish Deep, Brass Meridian, Quiet Orbit (not installed), Thimble & Thorn, Saltwind Rally (not installed).
$moves = 2
$dashboardTitle = 'Xbox Series Dashboard'
$overlayTitle = 'Xbox Series Dashboard Overlay'
# The stand-in game's window, and its launcher's: crates/standin/src/win.rs, BLUE.
$gameBlue = @(16, 32, 80)
# How long the game's launcher shows before the game's own window, the Launch screen over it.
$splashMs = 3000

# Where the Launch screen is looked for. At each, Home shows something bright that the Launch screen covers with the
# game's darkened art: Home's colour is read before A, and the Launch screen must be neither that nor the game's blue.
# Offscreen is what `dashboard --launch-screen --focus 2` draws there on this fake library, said beside what is seen.
$launchPoints = @(
    @{ X = 200; Y = 880; What = "Home's Discord tile"; Offscreen = @(14, 8, 40) }
    @{ X = 1070; Y = 880; What = "Home's Achievements tile"; Offscreen = @(12, 7, 36) }
    # Inside the launcher's 400x200 window in the middle of the screen, left of its title: the Launch screen hides it.
    @{ X = 780; Y = 540; What = "Home's focused tile, and the game's launcher"; Offscreen = @(16, 10, 49) }
)
# The Guide over the game, at rest: its panel is x 78..614, y 33..1045 of the canvas, grey turning dark green down
# it, and beside it the game under the Guide's dim, black at 56%. The panel's points are at y 900, where it is
# #152611: under the list's last entry (four games end at y 860) and over the Utilities (y 942). The game's are clear
# of its title in its middle, of the shade across the screen's top 420 and of the hints and the Status area in it.
# The screenshot test guide_over_a_game (crates/screens/tests/shots.rs) asserts the same colours at the same points.
$panel = @(21, 38, 17)
$dimmedGame = @(7, 14, 35)
$guidePoints = @(
    @{ X = 270; Y = 900; What = "the Guide's panel"; Colour = $panel }
    @{ X = 604; Y = 900; What = "the Guide's panel, 10 inside its right edge"; Colour = $panel }
    @{ X = 634; Y = 900; What = 'the game dimmed, 20 beside the panel'; Colour = $dimmedGame }
    @{ X = 1230; Y = 700; What = 'the game dimmed'; Colour = $dimmedGame }
)

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }

# Reads the screen at each of $Points into its Seen, and returns the points that are not $Colour, or, with none
# given, not their own Colour. Callers wrap the call in @( ) to count what comes back.
function Find-WrongColours($Points, $Colour) {
    foreach ($p in $Points) { $p.Seen = Get-DesignPixel $p.X $p.Y }
    $Points | Where-Object { -not (Test-Colour $_.Seen $(if ($Colour) { $Colour } else { $_.Colour }) 6) }
}

# The Launch screen's points that show Home as it was, or the game: where the Launch screen is not on screen.
function Find-NoLaunchScreen {
    foreach ($p in $launchPoints) { $p.Seen = Get-DesignPixel $p.X $p.Y }
    $launchPoints | Where-Object { (Test-Colour $_.Seen $_.Home 16) -or (Test-Colour $_.Seen $gameBlue 12) }
}

try {
    Close-WindowsPrompts
    # Connected before the Dashboard starts, as a pad already on is.
    Connect-Pad
    $library = Initialize-FakeSteam
    $recent = @($library.games | Where-Object last_played -gt 0 | Select-Object -First 8)
    Say "Recent row: $(($recent | ForEach-Object { "$($_.name)$(if (-not $_.installed) { ' (not installed)' })" }) -join ', ')"
    $game = $recent[$moves]
    if (-not $game -or -not $game.installed) { throw "The Recent row's game $moves to the right isn't an installed one" }
    # The game shows a launcher first, so the Launch screen has something to hide and is up long enough to look at.
    Add-Content (Join-Path $game.install_dir 'game-args.txt') @('--splash-ms', "$splashMs") -Encoding Ascii

    # The launcher is no question put to the user, however long a slow VM keeps it up: that rule has a test of its
    # own (needs-user).
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    # Tones for the sounds.
    $media = Join-Path $Out 'media'
    $toned = Start-Process (Join-Path $Payload 'standin.exe') -ArgumentList 'make-tones', "`"$media`"" -Wait -PassThru
    if ($toned.ExitCode -ne 0) { throw "standin make-tones exited with $($toned.ExitCode)" }
    $tones = @(Get-ChildItem $media -Filter *.wav).Count

    # Home, first frame.
    $dashboard = Start-Dashboard @('--media', "`"$media`"", '--log-sounds')
    $first = Wait-DashboardLog 'first frame presented' 60
    $adapter = Wait-DashboardLog '^adapter:' 1
    Say "adapter line: $(if ($adapter) { $adapter } else { 'none' })"
    $steam = Wait-DashboardLog '^steam:' 1
    Say "steam line: $(if ($steam) { $steam } else { 'none' })"
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    if ($first -match 'first frame presented at (\d+) ms') { Say "first frame presented at $($Matches[1]) ms since process start" }
    else { $failures += "the first frame line has no time: $first" }
    if (-not $adapter) { $failures += 'no adapter line in the log' }
    $startUp = Wait-DashboardLog '^start-up:' 1
    Say "start-up line: $(if ($startUp) { $startUp } else { 'none' })"
    $sounds = Wait-DashboardLog '^audio: \d+ sounds from ' 1
    Say "sounds line: $(if ($sounds) { $sounds } else { 'none' })"
    if ($tones -lt 1 -or $sounds -notmatch "^audio: $tones sounds from ") { $failures += "the Dashboard did not load the $tones tones: $sounds" }
    if ($steam -notmatch "^steam: $(@($library.games).Count) games ") { $failures += "the Dashboard did not read the fake Steam's $(@($library.games).Count) games: $steam" }
    # Art decodes after the first frame.
    Start-Sleep -Seconds 3
    Save-Screen 'home'
    Say "Home up; foreground $(Front)"
    if ((Get-Foreground).Title -ne $dashboardTitle) {
        # Started from a test with no window of its own, the Dashboard may not be given the foreground a user's
        # double-click gives it. Not what this test is about, so said and put right rather than failed.
        $taken = Set-Foreground $dashboardTitle
        Say "note: the Dashboard was not in front after starting; put in front: $taken; foreground $(Front)"
        if (-not $taken) { $failures += "the Dashboard could not be put in front: $(Front)" }
    }

    # Focus moves: Home starts on the profile, and Down is the Recent row's first tile.
    Push-Button Down; Start-Sleep -Milliseconds 400
    1..$moves | ForEach-Object { Push-Button Right; Start-Sleep -Milliseconds 400 }
    Start-Sleep -Milliseconds 600
    Save-Screen 'home-moved'
    Say "moved Right $moves times, to $($game.name)"
    $overlay = Wait-DashboardLog '^overlay: configured' 10
    Say "overlay line: $(if ($overlay) { $overlay } else { "none: $((Get-DashboardLog | Where-Object { $_ -match '^overlay:|^wgpu:' }) -join ' | ')" })"
    if (-not $overlay) { $failures += 'the Overlay was not made' }
    foreach ($p in $launchPoints) {
        $p.Home = Get-DesignPixel $p.X $p.Y
        Say "Home at ($($p.X), $($p.Y)), $($p.What): $(Format-Colour $p.Home)"
        if (Test-Colour $p.Home $gameBlue 12) { $failures += "Home at ($($p.X), $($p.Y)) is the game's colour, so the point says nothing: $(Format-Colour $p.Home)" }
    }
    # The first sound starts the device; every move that landed on another tile is the move sound, once.
    $device = Wait-DashboardLog '^audio: (device ".*" started|no sound)' 5
    Say "sound device: $(if ($device) { $device } else { 'no line' })"
    if ($device -notmatch '^audio: device ".*" started') { $failures += "no sound device was started: $device" }
    $moved = @(Get-DashboardLog | Where-Object { $_ -eq 'audio: played navigation.wav' }).Count
    Say "move sounds played: $moved"
    # Down from the profile, then along the row.
    if ($moved -ne $moves + 1) { $failures += "$moved move sounds for $($moves + 1) moves" }

    # A: the Launch screen, then the game.
    Push-Button A
    $up = Wait-DashboardLog 'Launch screen up' 10
    Say "after A: $(if ($up) { $up } else { 'no Launch screen up line' })"
    $selected = Wait-DashboardLog '^audio: played activation\.wav$' 5
    Say "select sound: $(if ($selected) { $selected } else { 'none' })"
    if (-not $selected) { $failures += 'A played no select sound' }
    if (-not $up) { throw "A did not start a launch (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    if ($up -notmatch [regex]::Escape("launch: $($game.name) ($($game.appid))")) { $failures += "launched something other than $($game.name): $up" }
    # On screen: at every point neither Home nor the game, within the time the launcher alone gives it.
    $covered = Wait-Until { -not @(Find-NoLaunchScreen).Count } ($splashMs / 1000)
    Start-Sleep -Milliseconds 700  # its fade in, the rest of the way
    Save-Screen 'launch-screen'
    $missing = @(Find-NoLaunchScreen)
    foreach ($p in $launchPoints) {
        Say "Launch screen at ($($p.X), $($p.Y)): seen $(Format-Colour $p.Seen); Home was $(Format-Colour $p.Home), the game is $(Format-Colour $gameBlue), drawn offscreen it is $(Format-Colour $p.Offscreen)"
    }
    foreach ($p in $missing) {
        $is = if (Test-Colour $p.Seen $p.Home 16) { "Home's colour there ($($p.What))" } else { "the game's colour" }
        $failures += "the Launch screen is not on screen at ($($p.X), $($p.Y)): seen $(Format-Colour $p.Seen), which is $is"
    }
    if (-not $covered -and -not $missing.Count) { Say "note: the Launch screen took more than $splashMs ms to cover Home" }
    Say "under the Launch screen: foreground $(Front)"
    if ((Get-Foreground).Title -eq $overlayTitle) { $failures += 'the Launch screen took the foreground: it must never be activated' }
    $booted = Wait-DashboardLog 'the game has booted' 60
    Say "booted: $(if ($booted) { $booted } else { 'no line in 60 s' }); foreground $(Front)"
    if (-not $booted) { throw "The game never counted as booted (launch log: $((Get-DashboardLog | Where-Object { $_ -match '^launch:' }) -join ' | '))" }
    $gone = Wait-DashboardLog 'Launch screen gone' 10
    Say "Launch screen: $(if ($gone) { $gone } else { 'not gone in 10 s' })"
    if (-not $gone) { $failures += 'the Launch screen did not go' }
    Start-Sleep -Milliseconds 500
    Save-Screen 'game'
    Say "in the game; foreground $(Front); game processes: $(@(Get-StandInGame).Count)"
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the game is not in front after the Launch screen: $(Front)" }
    $p = $launchPoints[0]
    $seen = Get-DesignPixel $p.X $p.Y
    Say "the game at ($($p.X), $($p.Y)): seen $(Format-Colour $seen)"
    if (-not (Test-Colour $seen $gameBlue 6)) { $failures += "the game does not show at ($($p.X), $($p.Y)) after the Launch screen: seen $(Format-Colour $seen), the game is $(Format-Colour $gameBlue)" }

    # Guide over the game, and away again.
    Push-Button Guide
    $opened = Wait-DashboardLog 'Guide over the game' 10
    # On screen and at rest: the panel slid all the way in, the game dimmed beside it.
    $atRest = Wait-Until { -not @(Find-WrongColours $guidePoints).Count } 6
    Save-Screen 'guide-over-game'
    Say "after Guide: $(if ($opened) { $opened } else { 'no Guide over the game line' }); foreground $(Front)"
    if (-not $opened) { $failures += 'the Guide did not open over the game' }
    $wrong = @(Find-WrongColours $guidePoints)
    foreach ($p in $guidePoints) { Say "Guide at ($($p.X), $($p.Y)), $($p.What): seen $(Format-Colour $p.Seen), expected $(Format-Colour $p.Colour)" }
    foreach ($p in $wrong) {
        $failures += "the Guide is not on screen at rest: at ($($p.X), $($p.Y)) expected $($p.What) $(Format-Colour $p.Colour), seen $(Format-Colour $p.Seen)"
    }
    if (-not $atRest -and -not $wrong.Count) { Say 'note: the Guide took more than 6 s to come to rest' }
    if ((Get-Foreground).Title -ne $overlayTitle) { $failures += "the Guide did not take the foreground from the game: $(Front)" }

    Push-Button Guide
    $closed = Wait-DashboardLog 'Guide closed' 10
    # The panel slides away, then the Overlay hides: the game alone at every point.
    $away = Wait-Until { -not @(Find-WrongColours $guidePoints $gameBlue).Count } 6
    Save-Screen 'guide-closed'
    Say "after Guide again: $(if ($closed) { $closed } else { 'no Guide closed line' }); foreground $(Front)"
    if (-not $closed) { $failures += 'the Guide did not close' }
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the game did not get the foreground back: $(Front)" }
    foreach ($p in @(Find-WrongColours $guidePoints $gameBlue)) {
        $failures += "the Guide is still on screen at ($($p.X), $($p.Y)): seen $(Format-Colour $p.Seen), the game is $(Format-Colour $gameBlue)"
    }
    if (-not $away) { Say 'note: the Guide had not gone from the screen 6 s after closing' }

    # The game ends: Home again.
    if (-not @(Get-StandInGame).Count) { $failures += 'the game was not running when the test came to stop it' }
    Stop-StandInGame
    Say 'stopped the game'
    $ended = Wait-DashboardLog 'the game has ended, Home back' 20
    Start-Sleep -Milliseconds 900
    Save-Screen 'home-again'
    Say "after the game: $(if ($ended) { $ended } else { 'no game has ended line in 20 s' }); foreground $(Front)"
    if (-not $ended) { $failures += 'Home did not come back after the game ended' }
    if ((Get-Foreground).Title -ne $dashboardTitle) { $failures += "the Dashboard is not in front after the game: $(Front)" }
    if ($dashboard.HasExited) { $failures += "the Dashboard exited with $($dashboard.ExitCode)" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
    Say "at the error: foreground $(Front)"
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
