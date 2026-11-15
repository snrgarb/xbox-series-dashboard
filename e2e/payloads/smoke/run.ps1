# Smoke: the Dashboard starts on a fake Steam library and draws Home, the virtual pad moves focus, A launches a game
# through the Launch screen, the Guide button opens the Guide over the game and closes it, and Home comes back when the
# game ends. The game is the stand-in, started by the stand-in Steam (lib\E2E.psm1, Initialize-FakeSteam).
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

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }

try {
    Close-WindowsPrompts
    # Connected before the Dashboard starts, as a pad already on is.
    Connect-Pad
    $library = Initialize-FakeSteam
    $recent = @($library.games | Where-Object last_played -gt 0 | Select-Object -First 8)
    Say "Recent row: $(($recent | ForEach-Object { "$($_.name)$(if (-not $_.installed) { ' (not installed)' })" }) -join ', ')"
    $game = $recent[$moves]
    if (-not $game -or -not $game.installed) { throw "The Recent row's game $moves to the right isn't an installed one" }

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
    Start-Sleep -Milliseconds 500
    Save-Screen 'launch-screen'
    $booted = Wait-DashboardLog 'the game has booted' 60
    Say "booted: $(if ($booted) { $booted } else { 'no line in 60 s' }); foreground $(Front)"
    if (-not $booted) { throw 'The game never counted as booted' }
    $gone = Wait-DashboardLog 'Launch screen gone' 10
    Say "Launch screen: $(if ($gone) { $gone } else { 'not gone in 10 s' })"
    if (-not $gone) { $failures += 'the Launch screen did not go' }
    Start-Sleep -Milliseconds 500
    Save-Screen 'game'
    Say "in the game; foreground $(Front); game processes: $(@(Get-StandInGame).Count)"
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the game is not in front after the Launch screen: $(Front)" }

    # Guide over the game, and away again.
    Push-Button Guide
    $opened = Wait-DashboardLog 'Guide over the game' 10
    Start-Sleep -Milliseconds 900
    Save-Screen 'guide-over-game'
    Say "after Guide: $(if ($opened) { $opened } else { 'no Guide over the game line' }); foreground $(Front)"
    if (-not $opened) { $failures += 'the Guide did not open over the game' }

    Push-Button Guide
    $closed = Wait-DashboardLog 'Guide closed' 10
    Start-Sleep -Milliseconds 900
    Save-Screen 'guide-closed'
    Say "after Guide again: $(if ($closed) { $closed } else { 'no Guide closed line' }); foreground $(Front)"
    if (-not $closed) { $failures += 'the Guide did not close' }
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the game did not get the foreground back: $(Front)" }

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
