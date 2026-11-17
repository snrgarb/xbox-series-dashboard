# The desktop never shows round a game (#164, docs/spec/launch-screen.md): from Play until Home is back, what is on
# screen is the Launch screen, the game, the game's backdrop (its Launch screen without the Spinner, in Home's window,
# behind the game) or Home, and never the desktop, the taskbar or another program's window. One stand-in game, started
# from Home five times, its game-args.txt rewritten before each:
#
#   1. Slow to start (no window for 5 s), then its launcher's window for 3 s, then no window for 2 s, then its own
#      window full screen; it exits by itself 20 s later. Through the start, the run, the exit and the fade to Home.
#   2. Windowed, 85% of the screen in a frame: the backdrop round it. Then quit from the Guide.
#   3. Full screen, and it crashes 6 s after its window is up (the process ends where it is, nothing closed).
#   4. A hand-off: 5 s after its window is up it starts itself again as another process and exits; the second one's
#      window comes 1 s later, and it exits by itself 12 s after. The backdrop holds across the gap.
#
# Throughout, the screen is sampled over and over at points round its edges and along the taskbar's row, two ways:
#   - the window there, by z-order (`WindowFromPoint`, the window's top-level owner): the desktop (Progman, WorkerW),
#     the taskbar (Shell_TrayWnd, Shell_SecondaryTrayWnd) or another program's window is a failure; the Dashboard's
#     windows and the stand-in game's are what should be there. The Launch screen and Toasts are click-through, so
#     this looks through them to what is under.
#   - the pixel there: the desktop's own colour at that point, read before the Dashboard started (or 0.44 of it, under
#     the Guide's dim), and not the Launch screen's, the game's or Home's, at two points at once, or at a point of the
#     taskbar's row, is a failure.
# The taskbar's row is judged throughout, the windowed game's run included: the Dashboard marks the game's window
# full screen for the shell, so the taskbar goes under the backdrop while the game is active (docs/spec/launch-screen.md,
# 3).
#
# Judged too by the Dashboard's `launch:` lines: `backdrop up` and `game window seen` (in either order: a game quick to
# show its window is seen before the backdrop's first frame is up), then `game gone, backdrop showing`, `fading to Home`,
# `Home back`, in that order; the backdrop alone for no more than 1.5 s between the game's window going and Home being
# back (a look at the desktop every 0.2 s, the game's process gone, the 0.3 s fade); and focus back on the game's tile
# (--log-focus).
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
$gameBlue = @(16, 32, 80)
# The Guide's dim over a game: black at 0x8F.
$dimmed = 1 - 0x8F / 255

Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class BackdropDesk {
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    // The top-level window at a point of the screen in real pixels, as "<pid> <class>"; "0 none" for none.
    public static string At(int x, int y) {
        var p = new POINT { X = x, Y = y };
        IntPtr h = WindowFromPoint(p);
        if (h == IntPtr.Zero) return "0 none";
        IntPtr root = GetAncestor(h, 2);  // GA_ROOT
        if (root != IntPtr.Zero) h = root;
        uint pid; GetWindowThreadProcessId(h, out pid);
        var s = new StringBuilder(128); GetClassName(h, s, 128);
        return pid + " " + s.ToString();
    }
}
'@

# Where the screen is sampled, on the Dashboard's 1920x1080 canvas. Round the edges, outside the windowed game's frame
# (85% of the screen in its middle leaves 144 across and 81 down on each side) and clear of the Launch screen's Spinner
# and name, and three along the taskbar's row.
$points = @(
    @{ X = 40; Y = 40; Name = 'top left' }
    @{ X = 1880; Y = 40; Name = 'top right' }
    @{ X = 960; Y = 50; Name = 'top middle' }
    @{ X = 40; Y = 620; Name = 'left' }
    @{ X = 1880; Y = 620; Name = 'right' }
)
$taskbar = @(
    @{ X = 120; Y = 1068; Name = 'taskbar left'; Taskbar = $true }
    @{ X = 960; Y = 1068; Name = 'taskbar middle'; Taskbar = $true }
    @{ X = 1800; Y = 1068; Name = 'taskbar right'; Taskbar = $true }
)
$all = $points + $taskbar

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }
function Fail([string] $What) { $script:failures += $What; Say "WRONG: $What" }

# A point of the canvas in real pixels of the screen, as Get-DesignPixel maps it.
function Get-ScreenPoint([double] $X, [double] $Y) {
    $b = Get-ScreenBounds
    $scale = [Math]::Min($b.Width / 1920, $b.Height / 1080)
    $left = ($b.Width - 1920 * $scale) / 2
    @([int] [Math]::Floor($b.X + $left + ($X + 0.5) * $scale), [int] [Math]::Floor($b.Y + ($Y + 0.5) * $scale))
}

# What each process whose window is found is: 'ours' (the Dashboard), 'game' (a stand-in game), or its name.
$script:Owners = @{}
function Get-Owner([int] $Id) {
    if ($script:Owners.ContainsKey($Id)) { return $script:Owners[$Id] }
    $process = Get-Process -Id $Id -ErrorAction SilentlyContinue
    $owner = if (-not $process) { 'gone' }
             elseif ($Id -eq $script:DashboardPid) { 'ours' }
             elseif ($process.ProcessName -eq 'game' -and (Test-OwnPath $(try { $process.Path } catch { $null }))) { 'game' }
             else { $process.ProcessName }
    $script:Owners[$Id] = $owner
    $owner
}

# What a point shows now: the window there by z-order, and what its pixel is like.
function Read-Point($Point) {
    $at = Get-ScreenPoint $Point.X $Point.Y
    $window = [BackdropDesk]::At($at[0], $at[1]) -split ' ', 2
    $owner = Get-Owner ([int] $window[0])
    $class = $window[1]
    $pixel = Get-DesignPixel $Point.X $Point.Y
    $looks = if ($Point.Splash -and (Test-Colour $pixel $Point.Splash 10)) { 'splash' }
             elseif (Test-Colour $pixel $gameBlue 10) { 'game' }
             elseif ($Point.Home -and (Test-Colour $pixel $Point.Home 10)) { 'home' }
             elseif ((Test-Colour $pixel $Point.Desk 6) -or (Test-Colour $pixel $Point.DeskDimmed 6)) { 'desktop' }
             else { 'other' }
    $desktopWindow = $class -in @('Progman', 'WorkerW', 'Shell_TrayWnd', 'Shell_SecondaryTrayWnd')
    $window = if ($desktopWindow) { "the desktop ($class)" }
              elseif ($owner -in @('ours', 'game')) { $owner }
              elseif ($class -eq 'none') { 'nothing' }
              else { "$owner's window ($class)" }
    # A window whose process has just ended (the game's, as it exits) is said, and is no failure by itself: what is
    # under it shows at the next sample.
    $bad = $desktopWindow -or -not ($owner -in @('ours', 'game', 'gone') -or $class -eq 'none')
    [pscustomobject]@{ Point = $Point; Pixel = $pixel; Looks = $looks; Window = $window; Bad = $bad }
}

# Samples every point over and over, for at most $Seconds, until a Dashboard line matching $Until (a regular
# expression, after $Mark) is logged; with no $Until, for $Seconds. A sample that finds the desktop is a failure: a
# window there not the Dashboard's or the game's, two points the desktop's colour, or a point of the taskbar's row the
# desktop's colour or the taskbar's window. Returns the line that ended it, or $null; says how the samples looked.
function Watch-Screen([string] $Phase, [double] $Seconds, [string] $Until = '', [int] $Mark = 0) {
    $judged = @($all | Where-Object { $_.Usable })
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $samples = 0; $seen = @{}; $bad = 0; $line = $null
    do {
        $samples++
        $read = @($judged | ForEach-Object { Read-Point $_ })
        foreach ($r in $read) { $seen[$r.Looks] = 1 + [int] $seen[$r.Looks] }
        $wrong = @($read | Where-Object { $_.Bad })
        $desktop = @($read | Where-Object { $_.Looks -eq 'desktop' })
        $desktopTaskbar = @($desktop | Where-Object { $_.Point.Taskbar })
        if ($wrong.Count -or $desktop.Count -ge 2 -or $desktopTaskbar.Count) {
            $bad++
            if ($bad -le 3) {
                $what = ($read | ForEach-Object { "$($_.Point.Name): $($_.Window), $(Format-Colour $_.Pixel) $($_.Looks)" }) -join '; '
                Fail "${Phase}: the desktop showed at $([int] $watch.Elapsed.TotalMilliseconds) ms (sample $samples): $what"
                Save-Screen "desktop-$Phase-$bad"
            }
        }
        if ($Until -and $samples % 4 -eq 0) {
            $line = Get-DashboardLog | Select-Object -Skip $Mark | Where-Object { $_ -match $Until } | Select-Object -First 1
            if ($line) { break }
        }
        Start-Sleep -Milliseconds 40
    } while ($watch.Elapsed.TotalSeconds -lt $Seconds)
    $looked = ($seen.Keys | Sort-Object | ForEach-Object { "$_ $($seen[$_])" }) -join ', '
    Say "${Phase}: $samples samples of $($judged.Count) points in $($watch.Elapsed.TotalSeconds.ToString('0.0')) s ($looked); the desktop seen in $bad"
    if ($Until -and -not $line) { Fail "${Phase}: no line '$Until' in $Seconds s" }
    $line
}

function Expect-Log([string] $Pattern, [int] $Mark, [string] $What, [double] $Seconds = 10) {
    $line = Wait-DashboardLog $Pattern $Seconds $Mark
    Say "${What}: $(if ($line) { $line } else { "NO line '$Pattern'" })"
    if (-not $line) { Fail "$What was not logged ('$Pattern')" }
    $line
}

# The `launch:` lines after $Mark, in order.
function Get-LaunchLines([int] $Mark) { @(Get-DashboardLog | Select-Object -Skip $Mark | Where-Object { $_ -match '^launch: ' }) }

# Whether the `launch:` lines after $Mark have each of $Patterns, in that order.
function Test-Order([int] $Mark, [string[]] $Patterns, [string] $What) {
    $lines = Get-LaunchLines $Mark
    $at = 0
    foreach ($pattern in $Patterns) {
        $found = $false
        for (; $at -lt $lines.Count; $at++) { if ($lines[$at] -match $pattern) { $found = $true; $at++; break } }
        if (-not $found) { Fail "${What}: no '$pattern' after the lines before it, in: $($lines -join ' | ')"; return }
    }
    Say "${What}: in order"
}

# The game's arguments for its next start, after its title (its window's, which is the game's name), as the fake wrote
# them.
function Set-GameArgs([string[]] $Arguments) {
    $Arguments = @('--title', $script:Game.name) + $Arguments
    [IO.File]::WriteAllLines((Join-Path $script:Game.install_dir 'game-args.txt'), [string[]] $Arguments)
    Say "game-args.txt: $($Arguments -join ' ')"
}

# A on the game's tile (focus is on it), its Game hub, then A on Play: the launch. Returns the log's mark before it.
function Start-Game {
    $mark = Get-DashboardLogLength
    Push-Button A
    if (-not (Wait-DashboardLog "^focus: GameHub: $($script:Game.source_id) Hub Play`$" 10 $mark)) { throw "A did not open the game's hub (log: $((Get-DashboardLog | Select-Object -Last 3) -join ' | '))" }
    $mark = Get-DashboardLogLength
    Push-Button A
    if (-not (Wait-DashboardLog 'Launch screen up' 10 $mark)) { throw "A on Play did not start a launch" }
    $mark
}

# After the game: Home back, in front, focus on the game's tile; the lines in order.
function Expect-HomeBack([int] $Mark, [string] $What) {
    # The backdrop's first frame and the game's first window, in either order, each before the rest.
    $rest = @('^launch: game gone, backdrop showing at', '^launch: fading to Home at', '^launch: Home back at \d+ ms$')
    Test-Order $Mark (@('^launch: backdrop up at \d+ ms$') + $rest) "${What} (the backdrop)"
    Test-Order $Mark (@('^launch: game window seen at') + $rest) "${What} (the game's window)"
    $gone = Get-LaunchLines $Mark | Where-Object { $_ -match '^launch: game gone, backdrop showing at (\d+) ms' } | Select-Object -Last 1
    $goneAt = if ($gone -match 'at (\d+) ms') { [int] $Matches[1] }
    $back = Get-LaunchLines $Mark | Where-Object { $_ -match '^launch: Home back at (\d+) ms' } | Select-Object -Last 1
    $backAt = if ($back -match 'at (\d+) ms') { [int] $Matches[1] }
    if ($goneAt -and $backAt) {
        Say "${What}: the backdrop alone for $($backAt - $goneAt) ms between the game's window going and Home being back"
        if ($backAt - $goneAt -gt 1500) { Fail "${What}: the backdrop held alone for $($backAt - $goneAt) ms, more than 1500" }
    }
    $taken = Get-LaunchLines $Mark | Where-Object { $_ -match "^launch: Home's window takes the foreground" } | Select-Object -Last 1
    Say "${What}: $(if ($taken) { $taken } else { "no line of Home's window taking the foreground" })"
    $focus = Wait-DashboardLog '^focus: Recent\(0\)$' 5 $Mark
    if (-not $focus) { Fail "${What}: focus is not back on the game's tile" }
    $ok = Wait-Until { (Get-Foreground).Title -eq $dashboardTitle } 5
    Say "${What}: foreground $(Front)"
    if (-not $ok) { Fail "${What}: the Dashboard is not in front: $(Front)" }
    if (@(Get-StandInGame).Count) { Fail "${What}: a stand-in game still runs" }
    Start-Sleep -Milliseconds 1000
    Save-Screen "$What-home"
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam
    $script:Game = @($library.games | Where-Object { $_.last_played -gt 0 })[0]
    if (-not $script:Game -or -not $script:Game.installed) { throw "The Recent row's first game is not an installed one" }
    Say "the game: $($script:Game.name) ($($script:Game.source_id))"
    # Neither the launcher nor the gaps are questions put to the user, however slow the machine.
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    # The desktop as it is before the Dashboard: what must never be seen again until the end.
    foreach ($p in $all) {
        $p.Desk = Get-DesignPixel $p.X $p.Y
        $p.DeskDimmed = [pscustomobject]@{ R = [int] ($p.Desk.R * $dimmed); G = [int] ($p.Desk.G * $dimmed); B = [int] ($p.Desk.B * $dimmed) }
        $at = Get-ScreenPoint $p.X $p.Y
        Say "the desktop at ($($p.X), $($p.Y)), $($p.Name): $(Format-Colour $p.Desk), window $([BackdropDesk]::At($at[0], $at[1]))"
    }
    Save-Screen '00-desktop'

    $dashboard = Start-Dashboard @('--log-focus')
    $script:DashboardPid = $dashboard.Id
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 2
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    if (-not (Wait-DashboardLog '^overlay: configured' 15)) { throw 'The Overlay was not made' }
    Push-Button Down
    if (-not (Wait-DashboardLog '^focus: Recent\(0\)' 10)) { throw "Down did not put focus on the Recent row's first tile" }
    # Home as it is with focus on the game: its background waits 1.5 s for the game's art.
    Start-Sleep -Seconds 3
    foreach ($p in $all) { $p.Home = Get-DesignPixel $p.X $p.Y }
    Save-Screen '01-home'

    # ---- 1. A slow start, a launcher, a gap, full screen, and an exit of its own.
    Set-GameArgs @('--start-ms', '5000', '--splash-ms', '3000', '--gap-ms', '2000', '--seconds', '20')
    $mark = Start-Game
    Expect-Log '^launch: backdrop up at \d+ ms$' $mark 'the backdrop drawn under the Launch screen' 10 | Out-Null
    # Nothing of the game's for 5 s: the Launch screen is what is on screen.
    Start-Sleep -Milliseconds 800
    Save-Screen '02-launch-screen'
    foreach ($p in $all) {
        $p.Splash = Get-DesignPixel $p.X $p.Y
        # A point is judged only where the desktop, the Launch screen, the game and Home can be told apart.
        $same = @()
        if (Test-Colour $p.Desk $p.Splash 10) { $same += 'the Launch screen' }
        if (Test-Colour $p.Desk $gameBlue 10) { $same += 'the game' }
        if (Test-Colour $p.Desk $p.Home 10) { $same += 'Home' }
        $p.Usable = -not $same.Count
        Say "($($p.X), $($p.Y)), $($p.Name): desktop $(Format-Colour $p.Desk), Home $(Format-Colour $p.Home), Launch screen $(Format-Colour $p.Splash)$(if ($same.Count) { "; its colour is not judged: the desktop is the colour of $($same -join ' and ') there" })"
    }
    if (@($points | Where-Object Usable).Count -lt 3) { Fail "only $(@($points | Where-Object Usable).Count) of the points round the edges tell the desktop from the Dashboard by colour" }
    Watch-Screen 'start' 60 'Launch screen gone' $mark | Out-Null
    Expect-Log '^launch: game window seen at' $mark 'the launcher' | Out-Null
    Expect-Log '^launch: game gone, Launch screen showing at' $mark 'the launcher gone, the game not yet up' | Out-Null
    Expect-Log "^launch: Home's window behind .* as the game's backdrop" $mark 'Home behind the game' | Out-Null
    $ok = Wait-Until { (Get-Foreground).Title -eq $script:Game.name } 5
    Say "the game started: foreground $(Front)"
    if (-not $ok) { Fail "the game is not in front: $(Front)" }
    Save-Screen '03-game'
    Watch-Screen 'running' 4 | Out-Null
    Watch-Screen 'exit' 40 '^launch: Home back at' $mark | Out-Null
    Expect-HomeBack $mark '04-exit'

    # ---- 2. Windowed: the backdrop round it. Then Quit from the Guide.
    Set-GameArgs @('--windowed', '85', '--seconds', '120')
    $mark = Start-Game
    Watch-Screen 'start-windowed' 60 'Launch screen gone' $mark | Out-Null
    $ok = Wait-Until { (Get-Foreground).Title -eq $script:Game.name } 5
    Say "the windowed game started: foreground $(Front)"
    if (-not $ok) { Fail "the windowed game is not in front: $(Front)" }
    Start-Sleep -Milliseconds 800
    Save-Screen '05-windowed'
    # Round it, the backdrop: the Launch screen's colours, not only not the desktop's.
    foreach ($p in $points) {
        $seen = Get-DesignPixel $p.X $p.Y
        Say "round the windowed game at ($($p.X), $($p.Y)), $($p.Name): $(Format-Colour $seen), the Launch screen is $(Format-Colour $p.Splash)"
        if (-not (Test-Colour $seen $p.Splash 10)) { Fail "the backdrop is not round the windowed game at ($($p.X), $($p.Y)): $(Format-Colour $seen)" }
    }
    Expect-Log '^launch: the game''s window marked full screen' $mark 'the windowed game marked full screen for the shell' 1 | Out-Null
    Watch-Screen 'windowed' 5 | Out-Null
    # The Guide, the running game's menu, Quit.
    $guideMark = Get-DashboardLogLength
    Push-Button Guide
    if (-not (Wait-DashboardLog '^guide: Landing Entry\(0\)$' 10 $guideMark)) { throw 'The Guide did not open over the game' }
    for ($i = 0; $i -lt 2; $i++) {
        $step = Get-DashboardLogLength
        Push-Button Down
        Wait-DashboardLog "^guide: Landing Entry\($($i + 1)\)" 5 $step | Out-Null
    }
    if (-not (Wait-DashboardLog '^guide: Landing Entry\(2\)' 1 $guideMark)) { throw "Focus did not reach the running game's entry" }
    $step = Get-DashboardLogLength
    Push-Button Start
    if (-not (Wait-DashboardLog '^guide: Landing Entry\(2\), dialog Menu' 8 $step)) { throw "Start did not open the running game's menu" }
    Start-Sleep -Milliseconds 1500
    Save-Screen '06-guide-menu'
    $quitMark = Get-DashboardLogLength
    Push-Button A
    Expect-Log '^launch: asked the game to quit' $quitMark 'Quit' | Out-Null
    Watch-Screen 'quit' 40 '^launch: Home back at' $quitMark | Out-Null
    if (-not (Wait-DashboardLog '^guide: closed$' 10 $quitMark)) { Fail 'the Guide did not close after the game quit from it had stopped' }
    Expect-HomeBack $mark '07-quit'

    # ---- 3. A crash.
    Set-GameArgs @('--crash-ms', '6000', '--seconds', '120')
    $mark = Start-Game
    Watch-Screen 'start-crash' 60 'Launch screen gone' $mark | Out-Null
    Watch-Screen 'crash' 40 '^launch: Home back at' $mark | Out-Null
    $crashed = Get-Content (Join-Path $Out 'standin.log') -ErrorAction SilentlyContinue | Where-Object { $_ -match 'game: crashing' }
    if (-not $crashed) { Fail 'the stand-in game does not say it crashed' }
    Expect-HomeBack $mark '08-crash'

    # ---- 4. A hand-off to another process, with a second's gap.
    Set-GameArgs @('--hand-off-ms', '5000', '--gap-ms', '1000', '--seconds', '12')
    $mark = Start-Game
    Watch-Screen 'start-hand-off' 60 'Launch screen gone' $mark | Out-Null
    Watch-Screen 'hand-off' 60 '^launch: Home back at' $mark | Out-Null
    $handed = Get-Content (Join-Path $Out 'standin.log') -ErrorAction SilentlyContinue | Where-Object { $_ -match 'game: handed off to pid' }
    if (-not $handed) { Fail 'the stand-in game does not say it handed off' }
    Test-Order $mark @('^launch: game gone, backdrop showing at', '^launch: game window seen at',
        "^launch: Home's window behind .* as the game's backdrop", '^launch: game gone, backdrop showing at', '^launch: fading to Home at') 'the hand-off'
    $ended = @(Get-LaunchLines $mark | Where-Object { $_ -match '^launch: the game has ended' })
    if ($ended.Count -ne 1) { Fail "the launch ended $($ended.Count) times across the hand-off, not once" }
    Expect-HomeBack $mark '09-hand-off'

    if ($dashboard.HasExited) { Fail "the Dashboard exited with $($dashboard.ExitCode)" }
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
