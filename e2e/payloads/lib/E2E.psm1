# What the end-to-end tests do on the test VM's desktop: start the Dashboard and read what it logs, give it a fake Steam
# to read and launch from, press a virtual Xbox pad, and look at the screen and the foreground. Imported by each test's
# run.ps1, which `e2e/run.sh` puts beside this folder as lib\, with dashboard.exe and standin.exe beside run.ps1.
# Everything a test writes goes to $env:E2E_OUT.
$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue'

$script:Out = $env:E2E_OUT
$script:Payload = Split-Path $PSScriptRoot -Parent
$script:DashboardLog = Join-Path $script:Out 'dashboard.log'
$script:Dashboard = $null
$script:Pad = $null

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class E2EDesk {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    public static uint WindowPid(IntPtr h) { uint pid; GetWindowThreadProcessId(h, out pid); return pid; }
    // Windows gives the foreground only to a process that sent the last input: a tap of Alt from this process is that
    // input.
    public static void TakeForeground(IntPtr h) {
        keybd_event(0x12, 0, 0, UIntPtr.Zero);
        keybd_event(0x12, 0, 2, UIntPtr.Zero);
        SetForegroundWindow(h);
    }
    public static uint ForegroundPid() { uint pid; GetWindowThreadProcessId(GetForegroundWindow(), out pid); return pid; }
    public static string ForegroundTitle() { var s = new StringBuilder(256); GetWindowText(GetForegroundWindow(), s, 256); return s.ToString(); }
}
'@
[E2EDesk]::SetProcessDPIAware() | Out-Null

# ---------------------------------------------------------------------------------------------- what a test reports

# A line in steps.txt, the test's account of what it did and saw, which `e2e/run.sh` prints after the run. Written to
# the host, not the output stream, so a function that says something still returns only what it returns.
# Each line has the clock's time and, after it, the seconds since the test began by a stopwatch: the VM's time sync
# can move the clock by hours mid-test, and the stopwatch says how long things really took.
$script:Began = [Diagnostics.Stopwatch]::StartNew()
function Say([string] $Text) {
    $line = "[$(Get-Date -Format HH:mm:ss.fff) +$($script:Began.Elapsed.TotalSeconds.ToString('0.0'))s] $Text"
    Add-Content (Join-Path $script:Out 'steps.txt') $line -Encoding UTF8
    Write-Host $line
}

# The whole primary screen, as <Name>.png in the test's output.
function Save-Screen([string] $Name) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try {
        $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
        $bmp.Save((Join-Path $script:Out "$Name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $g.Dispose(); $bmp.Dispose() }
}

# The colour of one pixel of the primary screen, as R, G and B, 0 to 255: what the compositor shows there, the
# Overlay included (docs/research/overlay-window.md, "Reading the screen back"). $X and $Y are real pixels.
function Get-ScreenPixel([int] $X, [int] $Y) {
    $origin = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Location
    $bmp = New-Object System.Drawing.Bitmap 1, 1
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try {
        $g.CopyFromScreen($origin.X + $X, $origin.Y + $Y, 0, 0, (New-Object System.Drawing.Size 1, 1))
        $c = $bmp.GetPixel(0, 0)
    } finally { $g.Dispose(); $bmp.Dispose() }
    [pscustomobject]@{ R = [int] $c.R; G = [int] $c.G; B = [int] $c.B }
}

# The same at a point of the Dashboard's 1920x1080 canvas, which is scaled to fit the screen whole, centred across and
# at the top (docs/ui.md, Coordinates): on a 1920x1080 screen the same pixel.
function Get-DesignPixel([double] $X, [double] $Y) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $scale = [Math]::Min($b.Width / 1920, $b.Height / 1080)
    $left = ($b.Width - 1920 * $scale) / 2
    Get-ScreenPixel ([int] [Math]::Round($left + $X * $scale)) ([int] [Math]::Round($Y * $scale))
}

# A colour as "(r, g, b)", for a line in steps.txt. Takes a pixel or three numbers.
function Format-Colour($Colour) {
    if ($Colour -is [array]) { return "($($Colour[0]), $($Colour[1]), $($Colour[2]))" }
    "($($Colour.R), $($Colour.G), $($Colour.B))"
}

# Whether a pixel is the colour $Rgb (three numbers, or another pixel), each channel within $Tolerance.
function Test-Colour($Pixel, $Rgb, [int] $Tolerance = 8) {
    if ($Rgb -isnot [array]) { $Rgb = @($Rgb.R, $Rgb.G, $Rgb.B) }
    ([Math]::Abs($Pixel.R - $Rgb[0]) -le $Tolerance) -and ([Math]::Abs($Pixel.G - $Rgb[1]) -le $Tolerance) -and
        ([Math]::Abs($Pixel.B - $Rgb[2]) -le $Tolerance)
}

# Whether a wait of $Seconds that began with $Watch and has slept $SleptMs in all should go on. Time is the
# stopwatch's, never the clock's, which the VM's time sync can move mid-test; and the wait's own sleeps must add up to
# $Seconds too, so a stopwatch that jumped forward with the clock (a 60 s wait once came back at once on a run where
# the clock moved five hours) cannot end a wait early. Says so when the two disagree by more than a minute.
function Test-StillWaiting($Watch, [double] $SleptMs, [double] $Seconds) {
    $elapsed = $Watch.Elapsed.TotalSeconds
    if ($elapsed -lt $Seconds) { return $true }
    if ($SleptMs / 1000 -lt $Seconds) {
        if ($elapsed - $SleptMs / 1000 -gt 60) { Say "note: the stopwatch says $([int] $elapsed) s into a wait that has slept $([int] ($SleptMs / 1000)) s: waiting by the sleeps" }
        return $true
    }
    return $false
}

# Runs $Condition every quarter second until it gives something true, which is returned, or $null after $Seconds.
function Wait-Until([scriptblock] $Condition, [double] $Seconds = 10) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $slept = 0
    do {
        $got = & $Condition
        if ($got) { return $got }
        Start-Sleep -Milliseconds 250
        $slept += 250
    } while (Test-StillWaiting $watch $slept $Seconds)
    return $null
}

# ------------------------------------------------------------------------------------------------------ the desktop

# The window in front: its process's id and name, and its title.
function Get-Foreground {
    $pid_ = [E2EDesk]::ForegroundPid()
    $name = (Get-Process -Id $pid_ -ErrorAction SilentlyContinue).ProcessName
    [pscustomobject]@{ Pid = $pid_; Process = $name; Title = [E2EDesk]::ForegroundTitle() }
}

# Puts the visible window titled $Title in front, as a user clicking it would. $true if it is in front after.
function Set-Foreground([string] $Title) {
    $window = [E2EDesk]::FindWindow([NullString]::Value, $Title)  # $null would pass "", a class no window has
    if ($window -eq [IntPtr]::Zero -or -not [E2EDesk]::IsWindowVisible($window)) { return $false }
    [E2EDesk]::TakeForeground($window)
    Start-Sleep -Milliseconds 300
    return (Get-Foreground).Title -eq $Title
}

# Windows' own full-screen prompts after sign-in, as "Protect your files and memories by backing up your PC" (its
# "finish setting up your device" screen, drawn by WebExperienceHostApp), turned off for this user and closed if up:
# over the Dashboard they take the foreground.
function Close-WindowsPrompts {
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement'
    New-Item $key -Force | Out-Null
    Set-ItemProperty $key ScoobeSystemSettingEnabled 0 -Type DWord
    $prompts = @(Get-Process WebExperienceHostApp, UserOOBEBroker -ErrorAction SilentlyContinue)
    if ($prompts.Count) {
        $prompts | Stop-Process -Force -ErrorAction SilentlyContinue
        Say "closed Windows' prompt ($(($prompts | ForEach-Object ProcessName | Sort-Object -Unique) -join ', '))"
        Start-Sleep -Seconds 1
    }
}

# ---------------------------------------------------------------------------------------------------------- the pad

# A virtual wired Xbox 360 pad on ViGEmBus, as Windows and XInput see a real one, the Guide button included.
function Connect-Pad {
    Add-Type -Path (Join-Path $PSScriptRoot 'Nefarius.ViGEm.Client.dll')
    $client = New-Object Nefarius.ViGEm.Client.ViGEmClient
    $script:Pad = $client.CreateXbox360Controller()
    $script:Pad.Connect()
    # Above the Dashboard, which on a VM with no GPU draws in software and can take every core: a press whose release
    # comes late is a held one, and the Dashboard repeats a direction held 0.4 s.
    try { (Get-Process -Id $PID).PriorityClass = 'High' } catch { Say "could not raise the test's priority: $_" }
    Say "virtual pad connected"
}

# Presses and releases a button: A, B, X, Y, Start, Back, Guide, Up, Down, Left, Right, LeftShoulder, RightShoulder.
# The Dashboard reads the pad once a frame, so a press must outlast a frame; it repeats a direction held 400 ms.
function Push-Button([string] $Button, [int] $HoldMs = 120) {
    $b = [Nefarius.ViGEm.Client.Targets.Xbox360.Xbox360Button]::$Button
    if (-not $b) { throw "No button $Button" }
    $held = [Diagnostics.Stopwatch]::StartNew()
    $script:Pad.SetButtonState($b, $true)
    Start-Sleep -Milliseconds $HoldMs
    $script:Pad.SetButtonState($b, $false)
    $held.Stop()
    # A hold that ran long says so: it may have counted twice.
    $late = if ($held.ElapsedMilliseconds -gt $HoldMs + 200) { ", held $($held.ElapsedMilliseconds) ms" }
    Say "pressed $Button for $HoldMs ms$late"
}

function Disconnect-Pad { if ($script:Pad) { try { $script:Pad.Disconnect() } catch {} } }

# ---------------------------------------------------------------------------------------------------- the Dashboard

# Starts dashboard.exe from the payload, with no console window of its own and what it logs (stderr) going to
# dashboard.log in the test's output. Returns its process. It is not put in front: Get-Foreground says whether it is.
function Start-Dashboard([string[]] $Arguments = @()) {
    $exe = Join-Path $script:Payload 'dashboard.exe'
    if (-not (Test-Path $exe)) { throw "No dashboard.exe in $script:Payload" }
    $start = @{ FilePath = $exe; WorkingDirectory = $script:Payload; RedirectStandardError = $script:DashboardLog
                NoNewWindow = $true; PassThru = $true }
    if ($Arguments.Count) { $start.ArgumentList = $Arguments }  # Start-Process refuses an empty list
    $script:Dashboard = Start-Process @start
    $script:Dashboard.Handle | Out-Null  # held, so ExitCode can be read once it has exited
    Say "started the Dashboard, pid $($script:Dashboard.Id)$(if ($Arguments.Count) { ": $($Arguments -join ' ')" })"
    $script:Dashboard
}

# The lines the Dashboard has logged so far. Read while it writes them.
function Get-DashboardLog {
    if (-not (Test-Path $script:DashboardLog)) { return @() }
    $share = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
    $file = [IO.File]::Open($script:DashboardLog, [IO.FileMode]::Open, [IO.FileAccess]::Read, $share)
    try {
        $reader = New-Object IO.StreamReader $file, ([Text.Encoding]::UTF8)
        $text = $reader.ReadToEnd()
    } finally { $file.Dispose() }
    return @($text -split "\r?\n" | Where-Object { $_ -ne '' })
}

# How many lines the Dashboard has logged: a mark for Wait-DashboardLog's -After.
function Get-DashboardLogLength { @(Get-DashboardLog).Count }

# Waits for a logged line matching $Pattern (a regular expression), among those after the first $After; returns the
# line, or $null after $Seconds. Throws if the Dashboard exits meanwhile without having logged it.
function Wait-DashboardLog([string] $Pattern, [double] $Seconds = 30, [int] $After = 0) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $slept = 0
    do {
        $exited = $script:Dashboard -and $script:Dashboard.HasExited
        $hit = Get-DashboardLog | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -First 1
        if ($hit) { return $hit }
        if ($exited) { throw "The Dashboard exited with $($script:Dashboard.ExitCode) before logging '$Pattern'" }
        Start-Sleep -Milliseconds 100
        $slept += 100
    } while (Test-StillWaiting $watch $slept $Seconds)
    return $null
}

# Waits for the greeting, the "Hi <user>!" Toast the Dashboard shows once it has started, to come and go: it is on the
# screen for five seconds, low in the middle (x 736..1184, y 881..1023 of the canvas at its largest), and a tap of the
# Guide button while it shows opens the Guide on Profile & system. A test that looks at Home or presses the Guide
# button waits it out first. Returns whether it came and went.
function Wait-Greeting([double] $Seconds = 30) {
    $shown = Wait-DashboardLog '^toasts: shown .* \(Greeting\)' $Seconds
    if (-not $shown) { Say 'note: no greeting showed'; return $false }
    $gone = Wait-DashboardLog '^toasts: gone' 20
    Say "the greeting: $shown; $(if ($gone) { $gone } else { 'still there after 20 s' })"
    [bool] $gone
}

function Stop-Dashboard {
    if ($script:Dashboard -and -not $script:Dashboard.HasExited) {
        Stop-Process -Id $script:Dashboard.Id -Force -ErrorAction SilentlyContinue
        $script:Dashboard.WaitForExit(5000) | Out-Null
    }
    Get-Process dashboard -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

# ------------------------------------------------------------------------------------------------- Steam and games

# Gives this user a Steam to find, with no Steam installed: a fake install at $Root written by `standin make-steam`
# (fourteen made-up games with generated art, eight installed, six played: crates/source-steam/src/fixture.rs), Steam's
# SteamPath registry value naming it, and standin.exe registered as this user's steam:// handler, so a game the
# Dashboard asks Steam for is started by the stand-in Steam: the game's folder's game.exe, a copy of standin.exe that
# shows a full-screen window titled with the game's name.
# With -FullShelf twelve more are installed, twenty in all, which is rows enough for My games & apps to scroll.
#
# Returns the Library as the Dashboard's own reader read it back (also fake-steam.json in the test's output): user, and
# games, most recently played first, each with source_id (its appid), name, installed, install_dir, last_played, cover, hero, header.
function Initialize-FakeSteam([string] $Root = 'C:\e2e\steam', [switch] $FullShelf) {
    $standin = Join-Path $script:Payload 'standin.exe'
    if (-not (Test-Path $standin)) { throw "No standin.exe in $script:Payload" }
    $json = Join-Path $script:Out 'fake-steam.json'
    $arguments = @('make-steam', "`"$Root`"", '--json', "`"$json`"") + $(if ($FullShelf) { '--full-shelf' })
    $made = Start-Process $standin -ArgumentList $arguments -Wait -PassThru
    if ($made.ExitCode -ne 0) {
        $why = Get-Content (Join-Path (Split-Path $Root -Parent) 'standin-error.txt') -Raw -ErrorAction SilentlyContinue
        throw "standin make-steam exited with $($made.ExitCode): $why"
    }

    New-Item 'HKCU:\Software\Valve\Steam' -Force | Out-Null
    # As Steam writes it: lower case, forward slashes.
    Set-ItemProperty 'HKCU:\Software\Valve\Steam' SteamPath ($Root.ToLower() -replace '\\', '/')
    $class = 'HKCU:\Software\Classes\steam'
    New-Item "$class\shell\open\command" -Force | Out-Null
    Set-ItemProperty $class '(default)' 'URL:steam protocol'
    Set-ItemProperty $class 'URL Protocol' ''
    Set-ItemProperty "$class\shell\open\command" '(default)' "`"$standin`" steam-url `"%1`""

    $library = Get-Content $json -Raw -Encoding UTF8 | ConvertFrom-Json
    $games = @($library.games)
    Say "fake Steam at ${Root}: user $($library.user), $($games.Count) games, $(@($games | Where-Object installed).Count) installed, $(@($games | Where-Object last_played -gt 0).Count) played"
    $library
}

# Writes a fake store at $Root with `standin <Command> $Root` (make-epic, make-gog, make-ea, make-ubisoft,
# make-battlenet) and returns the Library as the Dashboard's own reader read it back, which is also $Json in the
# test's output.
function New-FakeStore([string] $Command, [string] $Root, [string] $Json) {
    $standin = Join-Path $script:Payload 'standin.exe'
    if (-not (Test-Path $standin)) { throw "No standin.exe in $script:Payload" }
    $json = Join-Path $script:Out $Json
    $made = Start-Process $standin -ArgumentList @($Command, "`"$Root`"", '--json', "`"$json`"") -Wait -PassThru
    if ($made.ExitCode -ne 0) {
        $why = Get-Content (Join-Path (Split-Path $Root -Parent) 'standin-error.txt') -Raw -ErrorAction SilentlyContinue
        throw "standin $Command exited with $($made.ExitCode): $why"
    }
    Get-Content $json -Raw -Encoding UTF8 | ConvertFrom-Json
}

# Gives the Dashboard an Epic Games Launcher to read, with none installed: a fake at $Root written by `standin
# make-epic` (three made-up games, two installed and one owned only: crates/source-epic/src/fixture.rs), which the
# Dashboard reads when started with `--epic $Root`; and standin.exe registered as this user's com.epicgames.launcher://
# handler with $Root, the stand-in launcher, so a game the Dashboard asks the launcher for is started by it: the game's
# Binaries\Win64\game.exe, a copy of standin.exe showing a window titled with the game's name.
#
# Returns the Library as the Dashboard's own reader read it back (also fake-epic.json in the test's output): games by
# name, each with source_id (its AppName), name, installed, install_dir.
function Initialize-FakeEpic([string] $Root = 'C:\e2e\epic') {
    $library = New-FakeStore 'make-epic' $Root 'fake-epic.json'
    $standin = Join-Path $script:Payload 'standin.exe'
    $class = 'HKCU:\Software\Classes\com.epicgames.launcher'
    New-Item "$class\shell\open\command" -Force | Out-Null
    Set-ItemProperty $class '(default)' 'URL:com.epicgames.launcher protocol'
    Set-ItemProperty $class 'URL Protocol' ''
    Set-ItemProperty "$class\shell\open\command" '(default)' "`"$standin`" epic-url `"$Root`" `"%1`""
    $games = @($library.games)
    Say "fake Epic at ${Root}: $($games.Count) games, $(@($games | Where-Object installed).Count) installed"
    $library
}

# Gives the Dashboard a GOG to read, with none installed: a fake at $Root written by `standin make-gog` (three made-up
# games, two installed and one owned in a made-up Galaxy database, with generated WebP art in its picture cache:
# crates/source-gog/src/fixture.rs), which the Dashboard reads when started with `--gog $Root`. A GOG game starts as
# its own program, here its folder's game.exe, a copy of standin.exe: nothing is registered.
#
# Returns the Library as the Dashboard's own reader read it back (also fake-gog.json in the test's output): games most
# recently played first, each with source_id (its product id), name, installed, install_dir.
function Initialize-FakeGog([string] $Root = 'C:\e2e\gog') {
    $library = New-FakeStore 'make-gog' $Root 'fake-gog.json'
    $games = @($library.games)
    Say "fake GOG at ${Root}: $($games.Count) games, $(@($games | Where-Object installed).Count) installed"
    $library
}

# Gives the Dashboard an EA app to read, with none installed: a fake at $Root written by `standin make-ea` (made-up
# games in a registry.json of Uninstall keys, each installed one's folder with its installerdata.xml:
# crates/source-ea/src/fixture.rs), which the Dashboard reads when started with `--ea $Root`. An EA game starts as its
# own program, its folder's game.exe, a copy of standin.exe: nothing is registered.
#
# Returns the Library as the Dashboard's own reader read it back (also fake-ea.json in the test's output): games by
# name, each with source_id (its content id), name, installed, install_dir.
function Initialize-FakeEa([string] $Root = 'C:\e2e\ea') {
    $library = New-FakeStore 'make-ea' $Root 'fake-ea.json'
    $games = @($library.games)
    Say "fake EA app at ${Root}: $($games.Count) games, $(@($games | Where-Object installed).Count) installed"
    $library
}

# Gives the Dashboard a Ubisoft Connect to read, with none installed: a fake at $Root written by `standin make-ubisoft`
# (made-up games in a registry.json of Installs keys and the client's cache, installed and owned:
# crates/source-ubisoft/src/fixture.rs), which the Dashboard reads when started with `--ubisoft $Root`; and standin.exe
# registered as this user's uplay:// handler with $Root, the stand-in client, which starts the game a launch link
# names: its folder's game.exe, a copy of standin.exe.
#
# Returns the Library as the Dashboard's own reader read it back (also fake-ubisoft.json in the test's output): games
# by name, each with source_id (its uplay_id), name, installed, install_dir.
function Initialize-FakeUbisoft([string] $Root = 'C:\e2e\ubisoft') {
    $library = New-FakeStore 'make-ubisoft' $Root 'fake-ubisoft.json'
    $standin = Join-Path $script:Payload 'standin.exe'
    $class = 'HKCU:\Software\Classes\uplay'
    New-Item "$class\shell\open\command" -Force | Out-Null
    Set-ItemProperty $class '(default)' 'URL:uplay protocol'
    Set-ItemProperty $class 'URL Protocol' ''
    Set-ItemProperty "$class\shell\open\command" '(default)' "`"$standin`" uplay-url `"$Root`" `"%1`""
    $games = @($library.games)
    Say "fake Ubisoft Connect at ${Root}: $($games.Count) games, $(@($games | Where-Object installed).Count) installed"
    $library
}

# Gives the Dashboard a Battle.net to read, with none installed: a fake at $Root written by `standin make-battlenet`
# (made-up games in the agent's product.db and aggregate.json, and a registry.json of Uninstall keys:
# crates/source-battlenet/src/fixture.rs), which the Dashboard reads when started with `--battlenet $Root`. Its client,
# $Root\Battle.net.exe, is a copy of standin.exe, the stand-in Battle.net, which starts the game `--exec="launch <code>"`
# names: its folder's game.exe. Nothing is registered.
#
# Returns the Library as the Dashboard's own reader read it back (also fake-battlenet.json in the test's output):
# games by name, each with source_id (its product code), name, installed, install_dir.
function Initialize-FakeBattleNet([string] $Root = 'C:\e2e\battlenet') {
    $library = New-FakeStore 'make-battlenet' $Root 'fake-battlenet.json'
    $games = @($library.games)
    Say "fake Battle.net at ${Root}: $($games.Count) games, $(@($games | Where-Object installed).Count) installed"
    $library
}

# The stand-in games running now: the game.exe processes, each with its Path.
function Get-StandInGame {
    @(Get-Process game -ErrorAction SilentlyContinue)
}

# Ends every stand-in game at once, as a game that crashes or is killed ends; the stand-in Steam then says it stopped.
function Stop-StandInGame {
    Get-StandInGame | Stop-Process -Force -ErrorAction SilentlyContinue
}

# The stand-in game's pulse: a square of its window, x 1700..1820 and y 900..1020 of a 1920x1080 screen, that is one of
# two colours and changes four times a second while the game runs (crates/standin/src/win.rs, PULSE_COLOURS). A frozen
# game paints nothing, so its pulse stands still. Under the Guide's dim, black at 56%, each colour is 0.44 of itself.
$script:PulseColours = @(@(200, 200, 40), @(40, 200, 200))

# Whether a pixel is one of the pulse's two colours, as they are under the Guide's dim with $Dimmed.
function Test-PulseColour($Pixel, [bool] $Dimmed = $false) {
    foreach ($colour in $script:PulseColours) {
        $expected = if ($Dimmed) { @($colour | ForEach-Object { [int] [Math]::Round($_ * (1 - 0x8F / 255)) }) } else { $colour }
        if (Test-Colour $Pixel $expected 8) { return $true }
    }
    return $false
}

# Looks at the middle of the pulse every 100 ms for $Seconds and returns each different colour it showed: two for a
# game that is running, one for a game that is frozen. Callers wrap the call in @( ) to count what comes back.
function Watch-Pulse([double] $Seconds = 3) {
    $seen = @()
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $slept = 0
    do {
        $pixel = Get-DesignPixel 1760 960
        if (-not @($seen | Where-Object { Test-Colour $pixel $_ 8 }).Count) { $seen += $pixel }
        Start-Sleep -Milliseconds 100
        $slept += 100
    } while (Test-StillWaiting $watch $slept $Seconds)
    $seen
}

# How many times the stand-in games have said, in standin.log, that they stood still: a game says so at the first
# beat of its pulse after it was frozen for more than a second and a half.
function Get-StoodStill {
    @(Get-Content (Join-Path $script:Out 'standin.log') -ErrorAction SilentlyContinue | Where-Object { $_ -match 'game: stood still for \d+ ms' })
}

# Whether every thread of the process $Id is suspended, as all are of one that is frozen. $false for a process that
# has ended.
function Test-Suspended([int] $Id) {
    $process = Get-Process -Id $Id -ErrorAction SilentlyContinue
    if (-not $process) { return $false }
    $threads = @($process.Threads)
    if (-not $threads.Count) { return $false }
    -not @($threads | Where-Object { $_.ThreadState -ne 'Wait' -or $_.WaitReason -ne 'Suspended' }).Count
}

Export-ModuleMember -Function * -Variable Out, Payload, DashboardLog
