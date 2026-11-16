# Toasts: the greeting at start-up and a notification queued behind it (--toast), then, over the stand-in game, a
# Toast that shows without taking the foreground, goes after its five seconds, a second that waits behind the first,
# and a tap of the Guide button while a Toast shows, which opens the Guide on Notifications.
#
# A notification is posted to the running Dashboard by `standin toast <text>` (crates/model/src/post.rs), which the
# Dashboard takes only when started with --allow-toast-posts: the last thing here is that one started without it takes
# none. What the Dashboard did with it is read from its log (`toasts: posted`, `shown`, `going`, `gone`, each with the
# time since the Dashboard started, each said once the frame that shows it is on the screen) and, since the log saying
# so is not enough (#23), from the colour of the screen: the banner is the console's green, 128 high with its foot 64
# above the screen's, in the middle across.
#
# The sounds are tones `standin make-tones` writes; --log-sounds says each one played.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
# The Recent row's third game, as the smoke test launches: the Guide over it has the list the smoke test's points
# were measured on.
$moves = 2
$dashboardTitle = 'Xbox Series Dashboard'
$overlayTitle = 'Xbox Series Dashboard Overlay'
$stripTitle = 'Xbox Series Dashboard Toasts'
# The stand-in game's window: crates/standin/src/win.rs, BLUE.
$gameBlue = @(16, 32, 80)
# The banner's card: #107C10, y 888..1016 of the canvas, as wide as its text and never less than about 408, so
# x 760..1160 is always in it. These two points are in its top and bottom margins, clear of its tile and its text.
$green = @(16, 124, 16)
$cardPoints = @(
    @{ X = 960; Y = 894; What = "the card, under its top edge" }
    @{ X = 960; Y = 1010; What = "the card, over its bottom edge" }
)
# Beside the widest card (640: x 640..1280) and above it, in the strip's window: what is behind shows through.
$besidePoints = @(
    @{ X = 500; Y = 950; What = 'left of the card' }
    @{ X = 1420; Y = 950; What = 'right of the card' }
    @{ X = 960; Y = 860; What = 'above the card, in the strip' }
)
# The Guide over the game, at rest, as the smoke test has it; and the focus ring round the Notifications Utility, the
# first square at the panel's foot (x 93..180, y 942..1029): its 4-wide stroke stands 4 clear of the square.
$panel = @(21, 38, 17)
$dimmedGame = @(7, 14, 35)
$ring = @(44, 162, 67)
$guidePoints = @(
    @{ X = 270; Y = 900; What = "the Guide's panel"; Colour = $panel }
    @{ X = 1230; Y = 700; What = 'the game dimmed'; Colour = $dimmedGame }
    @{ X = 87; Y = 985; What = "the focus ring round the Notifications Utility"; Colour = $ring }
    @{ X = 960; Y = 894; What = 'the game dimmed where the Toast was'; Colour = $dimmedGame }
)

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }

# The time a log line ends with, in ms since the Dashboard started.
function At([string] $Line) { if ($Line -match ' at (\d+) ms$') { [int] $Matches[1] } else { -1 } }

# Reads the screen at each of $Points into its Seen, and returns the points that are not $Colour, or, with none
# given, not their own Colour. Callers wrap the call in @( ) to count what comes back.
function Find-WrongColours($Points, $Colour, [int] $Tolerance = 8) {
    foreach ($p in $Points) { $p.Seen = Get-DesignPixel $p.X $p.Y }
    $Points | Where-Object { -not (Test-Colour $_.Seen $(if ($Colour) { $Colour } else { $_.Colour }) $Tolerance) }
}

# Whether the banner is on screen, all the way in.
function Test-Banner { -not @(Find-WrongColours $cardPoints $green).Count }

# How many lines the log has up to and with the first that matches: what to look after for what follows it. 0 when
# none does.
function Find-LogLine([string] $Pattern) {
    $lines = @(Get-DashboardLog)
    for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -match $Pattern) { return $i + 1 } }
    0
}

# Posts a notification to the Dashboard, and returns the Dashboard's line saying it took it.
function Send-Toast([string] $Text) {
    $after = Get-DashboardLogLength
    $sent = Start-Process (Join-Path $Payload 'standin.exe') -ArgumentList 'toast', "`"$Text`"" -Wait -PassThru
    if ($sent.ExitCode -ne 0) { $script:failures += "standin toast '$Text' exited with $($sent.ExitCode)" }
    $posted = Wait-DashboardLog "^toasts: posted `"$([regex]::Escape($Text))`"" 5 -After $after
    Say "posted '$Text': $(if ($posted) { $posted } else { 'no posted line' })"
    if (-not $posted) { $script:failures += "the Dashboard did not take the notification '$Text'" }
    $posted
}

# Waits for the Toast with this line to show, and for the banner to be on screen. Returns its shown line.
function Wait-Toast([string] $Text, [string] $Kind, [int] $After = 0, [double] $Seconds = 15) {
    # To the line's end, its time: a line read while it is being written is not yet the line.
    $shown = Wait-DashboardLog "^toasts: shown `"$([regex]::Escape($Text))`" \($Kind\).* at \d+ ms`$" $Seconds -After $After
    if (-not $shown) {
        $script:failures += "no Toast '$Text' showed (log: $((Get-DashboardLog | Where-Object { $_ -match '^toasts:' } | Select-Object -Last 4) -join ' | '))"
        return $null
    }
    $onScreen = Wait-Until { Test-Banner } 4
    foreach ($p in $cardPoints) { Say "  the screen at ($($p.X), $($p.Y)), $($p.What): $(Format-Colour $p.Seen), the card is $(Format-Colour $green)" }
    if (-not $onScreen) { $script:failures += "the Toast '$Text' is in the log and not on the screen: $(($cardPoints | ForEach-Object { Format-Colour $_.Seen }) -join ', ')" }
    $shown
}

# A Toast's time on screen and the wait for the next, from the log's own times: five seconds, then half a second.
# Each line is said when the frame it tells of has been presented (shown: the Toast's first; going: the first of its
# going), so two of them are apart by what the screen had, give or take the log's own lateness. The VM draws with the
# processor, and one frame of Home there can take most of a second, which is how late the Dashboard can be to see a
# Toast's time is up: 4.5 to 6.5 s for the five seconds. The next Toast can't be before the last has gone (0.41 s) and
# is half a second after it began to go, give or take the same.
function Test-Timing([string] $Shown, [string] $Going, [string] $Next, [string] $What) {
    $showed = (At $Going) - (At $Shown)
    Say "$What`: shown for $showed ms"
    if ($showed -lt 4500 -or $showed -gt 6500) { $script:failures += "$What showed for $showed ms, not five seconds" }
    if ($Next) {
        $gap = (At $Next) - (At $Going)
        Say "$What`: the next showed $gap ms after it went"
        if ($gap -lt 410 -or $gap -gt 2000) { $script:failures += "the Toast after $What showed $gap ms after it went, not half a second" }
    }
}

# The strip's own count of the frames it presented while it was last shown, from the first `strip hidden` line after
# $After. A Toast moves for about a second of its five (in 0.3 s, the greeting's slide or a notification's two turns
# of text, out 0.41 s) and is drawn only while it moves: sixty-odd frames where the strip has the renderer, as it has
# over the game. Fifteen is a quarter of that: fewer is a strip kept waiting by Home's window, which drew 5 and 7.
function Test-Frames([int] $After, [string] $What, [int] $AtLeast = 15) {
    $hidden = Wait-DashboardLog '^toasts: strip hidden after' 10 -After $After
    if ($hidden -notmatch ' and (\d+) frames ') { $script:failures += "the strip did not hide after $What`: $hidden"; return }
    Say "$What`: $hidden"
    if ([int] $Matches[1] -lt $AtLeast) { $script:failures += "the strip drew $($Matches[1]) frames for $What, which is not a Toast seen to come and go: $hidden" }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam
    $recent = @($library.games | Where-Object last_played -gt 0 | Select-Object -First 8)
    $game = $recent[$moves]
    if (-not $game -or -not $game.installed) { throw "The Recent row's game $moves to the right isn't an installed one" }
    $env:DASHBOARD_NEEDS_USER_MS = '120000'
    $media = Join-Path $Out 'media'
    $toned = Start-Process (Join-Path $Payload 'standin.exe') -ArgumentList 'make-tones', "`"$media`"" -Wait -PassThru
    if ($toned.ExitCode -ne 0) { throw "standin make-tones exited with $($toned.ExitCode)" }

    # ------------------------------------------------------------------------------- at start-up, over Home
    $queued = 'Queued at start'
    $arguments = @('--media', "`"$media`"", '--log-sounds', '--log-focus')
    $dashboard = Start-Dashboard ($arguments + @('--allow-toast-posts', '--toast', "`"$queued`""))
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "first frame: $first"
    $strip = Wait-DashboardLog '^toasts: configured' 15
    Say "strip line: $(if ($strip) { $strip } else { "none: $((Get-DashboardLog | Where-Object { $_ -match '^toasts:|^overlay:|^wgpu:' }) -join ' | ')" })"
    if (-not $strip) { throw 'The Toast strip was not made' }

    # The greeting, with the notification --toast posted waiting behind it.
    # The greeting is the signed-in Profile's first name (docs/spec/profiles.md, 1): the real name's first word, else
    # the gamertag, as the `profile:` line has them.
    $profileLine = Wait-DashboardLog '^profile: ' 10
    $name = 'Player'
    if ($profileLine -match '^profile: "(.*)", real name "(.*)", ') {
        $name = if ($Matches[2].Trim()) { $Matches[2].Trim().Split(' ')[0] } else { $Matches[1] }
    }
    $greeting = Wait-Toast "Hi $name!" 'Greeting'
    Save-Screen 'greeting'
    Say "greeting: $(if ($greeting) { $greeting } else { 'none' }); foreground $(Front)"
    if ($greeting -and $greeting -notmatch ', 1 waiting at ') { $failures += "--toast's notification was not waiting behind the greeting: $greeting" }
    if ((Get-Foreground).Title -eq $stripTitle) { $failures += 'the Toast strip took the foreground: it must never be activated' }
    $sound = Wait-DashboardLog '^audio: played Notification\.wav$' 5
    if (-not $sound) { $failures += 'the greeting played no notification sound' }
    # This is while Home is at its busiest, its art coming in and its swirl turning, on a VM that draws Home with
    # the processor: the Toast must come and go in its own time all the same, and be seen to.
    $going = Wait-DashboardLog '^toasts: going' 15
    Say "the greeting: $(if ($going) { $going } else { 'did not go in 15 s' })"
    if (-not $going) { $failures += 'the greeting did not go after its time' }
    $early = Get-DashboardLog | Select-Object -First (Find-LogLine '^toasts: going') | Where-Object { $_ -match "^toasts: shown `"$queued`"" }
    if ($early) { $failures += "--toast's notification showed while the greeting was showing: $early" }
    Test-Frames (Find-LogLine '^toasts: strip shown') 'the greeting'
    $next = Wait-Toast $queued 'Notification'
    Save-Screen 'queued-at-start'
    Say "after the greeting: $(if ($next) { $next } else { 'nothing' })"
    if ($greeting -and $going -and $next) { Test-Timing $greeting $going $next 'the greeting' }
    # It goes in its turn, and the strip with it: Home shows where the card was. Looked for after its own shown line,
    # wherever the log has got to by now.
    $after = Find-LogLine "^toasts: shown `"$queued`""
    $goingNext = Wait-DashboardLog '^toasts: going' 15 -After $after
    $gone = Wait-DashboardLog '^toasts: gone' 10 -After $after
    $hidden = Wait-DashboardLog '^toasts: strip hidden' 5 -After $after
    if ($next -and $goingNext) { Test-Timing $next $goingNext $null "--toast's notification" }
    Test-Frames $after "--toast's notification"
    Start-Sleep -Milliseconds 300
    if (-not $gone -or -not $hidden) { $failures += "the last Toast did not go and the strip hide: gone '$gone', hidden '$hidden'" }
    if (Test-Banner) { $failures += 'the banner is still on screen after its Toast has gone' }
    Save-Screen 'home'

    # ------------------------------------------------------------------------------------- over the game
    if ((Get-Foreground).Title -ne $dashboardTitle) {
        $taken = Set-Foreground $dashboardTitle
        Say "note: the Dashboard was not in front after starting; put in front: $taken; foreground $(Front)"
        if (-not $taken) { $failures += "the Dashboard could not be put in front: $(Front)" }
    }
    # A press at a time, by what --log-focus says: Down to the Recent row, then along it.
    $steps = @('Down') + @(1..$moves | ForEach-Object { 'Right' })
    $at = 0
    foreach ($step in $steps) {
        $before = Get-DashboardLogLength
        Push-Button $step
        $want = "^focus: Recent\($at\)$"
        if (-not (Wait-DashboardLog $want 5 -After $before)) { throw "$step did not put focus on the Recent row's tile $at" }
        $at += 1
    }
    # A is the game's Game hub, after the double tap's 300 ms; A on its Play is the launch.
    $before = Get-DashboardLogLength
    Push-Button A
    if (-not (Wait-DashboardLog '^focus: GameHub: \d+ Hub Play$' 10 -After $before)) { throw "A did not open the game's hub (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    Push-Button A
    $up = Wait-DashboardLog 'Launch screen up' 10
    if (-not $up) { throw "A on Play did not start a launch (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    $booted = Wait-DashboardLog 'the game has booted' 60
    if (-not $booted) { throw "The game never counted as booted (launch log: $((Get-DashboardLog | Where-Object { $_ -match '^launch:' }) -join ' | '))" }
    $launchGone = Wait-DashboardLog 'Launch screen gone' 10
    if (-not $launchGone) { $failures += 'the Launch screen did not go' }
    Start-Sleep -Milliseconds 500
    Say "in the game; foreground $(Front)"
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the game is not in front after the Launch screen: $(Front)" }
    $notGame = @(Find-WrongColours ($cardPoints + $besidePoints) $gameBlue 6)
    foreach ($p in $notGame) { $failures += "the game does not show at ($($p.X), $($p.Y)) before any Toast: $(Format-Colour $p.Seen)" }

    # A Toast over the game: on the screen, the game around it, and the game still in front.
    $one = 'Over the game'
    $two = 'Second in line'
    $mark = Get-DashboardLogLength
    Send-Toast $one | Out-Null
    $shownOne = Wait-Toast $one 'Notification' -After $mark
    Save-Screen 'toast-over-game'
    Say "over the game: $(if ($shownOne) { $shownOne } else { 'no Toast' }); foreground $(Front)"
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the Toast took the foreground from the game: $(Front)" }
    foreach ($p in @(Find-WrongColours $besidePoints $gameBlue 6)) {
        $failures += "the game does not show $($p.What), at ($($p.X), $($p.Y)), while the Toast is up: $(Format-Colour $p.Seen)"
    }
    if (-not (Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^toasts: strip shown' })) { $failures += 'the Toast strip was not shown for the Toast over the game' }
    if (-not (Wait-DashboardLog '^audio: played Notification\.wav$' 5 -After $mark)) { $failures += 'the Toast over the game played no notification sound' }

    # A second waits behind the first: posted now, shown only when the first has had its five seconds and gone.
    Send-Toast $two | Out-Null
    $early = Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match "^toasts: shown `"$two`"" }
    if ($early) { $failures += "the second Toast showed while the first was showing: $early" }
    $goingOne = Wait-DashboardLog '^toasts: going' 10 -After $mark
    Say "the first: $(if ($goingOne) { $goingOne } else { 'did not go in 10 s' })"
    if (-not $goingOne) { $failures += 'the first Toast did not go after its time' }
    $shownTwo = Wait-Toast $two 'Notification' -After $mark
    Save-Screen 'second-toast'
    Say "the second: $(if ($shownTwo) { $shownTwo } else { 'no Toast' }); foreground $(Front)"
    if ($shownOne -and $goingOne -and $shownTwo) { Test-Timing $shownOne $goingOne $shownTwo 'the first Toast over the game' }
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the second Toast took the foreground from the game: $(Front)" }

    # A tap of the Guide button while it shows: the Guide opens over the game, on the Notifications Flyout, and the Toast goes.
    $mark = Get-DashboardLogLength
    Push-Button Guide
    $opened = Wait-DashboardLog 'Guide over the game' 10 -After $mark
    $focus = Wait-DashboardLog '^guide: ' 5 -After $mark
    $dropped = Wait-DashboardLog '^toasts: gone' 5 -After $mark
    Start-Sleep -Milliseconds 1500
    Save-Screen 'guide-from-toast'
    # It opens on the Notifications Flyout, which lists what the Toast told of. B closes the Flyout, focus on its
    # Utility: the Guide's panel is looked at there, and a notification that comes then has a Toast again (one that
    # comes while the Flyout is open is listed in it and has none).
    $markB = Get-DashboardLogLength
    Push-Button B
    $onUtility = Wait-DashboardLog '^guide: Landing Utility\(0\)$' 8 -After $markB
    if (-not $onUtility) { $failures += "B did not close the Notifications Flyout onto its Utility: $(Get-DashboardLog | Where-Object { $_ -match '^guide: ' } | Select-Object -Last 1)" }
    $atRest = Wait-Until { -not @(Find-WrongColours $guidePoints).Count } 6
    Save-Screen 'guide-flyout-closed'
    Say "after Guide: $(if ($opened) { $opened } else { 'no Guide over the game line' }); $focus; $dropped; foreground $(Front)"
    if (-not $opened) { $failures += 'a tap of the Guide button while a Toast showed did not open the Guide' }
    if ($focus -notmatch '^guide: Landing Utility\(0\), flyout Notifications ') { $failures += "the Guide did not open on the Notifications Flyout: $focus" }
    if (-not $dropped) { $failures += 'the Toast did not go as the Guide opened' }
    foreach ($p in $guidePoints) { Say "  Guide at ($($p.X), $($p.Y)), $($p.What): seen $(Format-Colour $p.Seen), expected $(Format-Colour $p.Colour)" }
    foreach ($p in @(Find-WrongColours $guidePoints)) {
        $failures += "the Guide from a Toast is not on screen at rest: at ($($p.X), $($p.Y)) expected $($p.What) $(Format-Colour $p.Colour), seen $(Format-Colour $p.Seen)"
    }
    if (-not $atRest) { Say 'note: the Guide took more than 6 s to come to rest' }
    if ((Get-Foreground).Title -ne $overlayTitle) { $failures += "the Guide did not take the foreground from the game: $(Front)" }

    # A notification that comes while the Guide is open shows over it, above the Overlay, which keeps the foreground.
    $three = 'Over the Guide'
    $markThree = Get-DashboardLogLength
    Send-Toast $three | Out-Null
    $shownThree = Wait-Toast $three 'Notification' -After $markThree
    Save-Screen 'toast-over-guide'
    Say "over the Guide: $(if ($shownThree) { $shownThree } else { 'no Toast' }); foreground $(Front)"
    if ((Get-Foreground).Title -ne $overlayTitle) { $failures += "the Toast over the Guide took the foreground from it: $(Front)" }

    # The Guide closes, the game is in front again, and the Toast stays its time over it.
    $mark = Get-DashboardLogLength
    Push-Button Guide
    $closed = Wait-DashboardLog 'Guide closed' 10 -After $mark
    Say "after Guide again: $(if ($closed) { $closed } else { 'no Guide closed line' }); foreground $(Front)"
    if (-not $closed) { $failures += 'the Guide did not close' }
    if ((Get-Foreground).Title -ne $game.name) { $failures += "the game did not get the foreground back: $(Front)" }
    $goneThree = Wait-DashboardLog '^toasts: gone' 10 -After $markThree
    $hidden = Wait-DashboardLog '^toasts: strip hidden' 5 -After $markThree
    Start-Sleep -Milliseconds 300
    if (-not $goneThree -or -not $hidden) { $failures += "the last Toast did not go and the strip hide: gone '$goneThree', hidden '$hidden'" }
    $away = Wait-Until { -not @(Find-WrongColours ($cardPoints + $besidePoints) $gameBlue 6).Count } 6
    Save-Screen 'game-again'
    foreach ($p in @(Find-WrongColours ($cardPoints + $besidePoints) $gameBlue 6)) {
        $failures += "something is still over the game at ($($p.X), $($p.Y)) after the last Toast: $(Format-Colour $p.Seen)"
    }

    # Every Toast played its sound, once: the greeting, --toast's, and the three posted.
    $sounds = @(Get-DashboardLog | Where-Object { $_ -eq 'audio: played Notification.wav' }).Count
    Say "notification sounds played: $sounds"
    if ($sounds -ne 5) { $failures += "$sounds notification sounds for 5 Toasts" }
    Get-DashboardLog | Where-Object { $_ -match '^toasts:' } | ForEach-Object { Say "  $_" }

    Stop-StandInGame
    $ended = Wait-DashboardLog 'the game has ended, Home back' 20
    if (-not $ended) { $failures += 'Home did not come back after the game ended' }
    if ($dashboard.HasExited) { $failures += "the Dashboard exited with $($dashboard.ExitCode)" }

    # ------------------------------------------------------------- a Dashboard not told to take posts takes none
    Stop-Dashboard
    Copy-Item $DashboardLog (Join-Path $Out 'dashboard-first.log') -Force
    $dashboard = Start-Dashboard $arguments
    if (-not (Wait-DashboardLog '^toasts: configured' 60)) { throw 'The Toast strip was not made at the second start' }
    Wait-Greeting | Out-Null
    $refused = 'Not allowed'
    $sent = Start-Process (Join-Path $Payload 'standin.exe') -ArgumentList 'toast', "`"$refused`"" -Wait -PassThru
    Start-Sleep -Seconds 2
    $taken = Get-DashboardLog | Where-Object { $_ -match "^toasts: (posted|shown) `"$refused`"" }
    Say "without --allow-toast-posts: standin toast exited with $($sent.ExitCode); the log: $(if ($taken) { $taken -join ' | ' } else { 'nothing of it' })"
    if ($sent.ExitCode -eq 0) { $failures += 'a Dashboard started without --allow-toast-posts took a posted notification' }
    if ($taken) { $failures += "a Dashboard started without --allow-toast-posts showed a posted notification: $($taken -join ' | ')" }
    if (Test-Banner) { $failures += 'a banner is on screen after a post that was not allowed' }
    Save-Screen 'post-not-allowed'
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
