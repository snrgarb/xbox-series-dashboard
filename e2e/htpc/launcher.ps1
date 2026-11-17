# Runs one test on the TV PC's desktop, in safe mode, from the folder e2e/htpc/run.sh copied it to: started by a
# one-off interactive scheduled task (run.cmd), so it is in the signed-in user's session, where the screen, the
# foreground and the pad are.
#
# First it looks whether the TV is in use, and if it is it runs nothing: a game running (a program Steam started), a
# window other than the desktop's or the live Dashboard's covering the whole screen in front (a game, a film, the lock
# screen), with the live Dashboard in front a full-screen window of another program behind it, or a Dashboard other
# than the installed one, or Playnite, running. The live Dashboard as the shell (shell.ps1) is not the TV in use: with
# nothing else going on it is stopped for the test, the watchdog first, and `dashboard.exe --shell` is started again
# afterwards, whatever the test did (and by cleanup.ps1, if this launcher was ended first). Then it runs the payload's
# run.ps1 with E2E_SAFE=1 and E2E_OUT=<folder>\out. Whatever happens, Windows' Task View is not left in front, and
# out\exitcode.txt is written last: 3 when the TV was in use (out\refused.txt says why), else the payload's code.
$ErrorActionPreference = 'Continue'
$folder = $PSScriptRoot
$out = Join-Path $folder 'out'
New-Item $out -ItemType Directory -Force | Out-Null
$code = 1
. (Join-Path $folder 'shell.ps1')
# There while the live shell is stopped by this run (shell.ps1).
$stoppedMarker = Join-Path $folder 'shell-stopped.txt'

function Note([string] $Text) {
    Add-Content (Join-Path $out 'launcher.txt') "[$(Get-Date -Format HH:mm:ss.fff)] $Text" -Encoding UTF8
}

Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class XsdLauncher {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct MONITORINFO { public int Size; public RECT Monitor, Work; public uint Flags; }
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern IntPtr MonitorFromWindow(IntPtr h, uint flags);
    [DllImport("user32.dll")] static extern bool GetMonitorInfo(IntPtr m, ref MONITORINFO info);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongW")] static extern int GetWindowLong(IntPtr h, int index);
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h, int attribute, out int value, int size);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    public static uint Pid(IntPtr h) { uint pid; GetWindowThreadProcessId(h, out pid); return pid; }
    public static string Title(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
    public static string Class(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
    // Whether the window covers the whole of the monitor it is on.
    public static bool CoversItsMonitor(IntPtr h) {
        RECT r;
        if (!GetWindowRect(h, out r)) return false;
        var info = new MONITORINFO(); info.Size = Marshal.SizeOf(typeof(MONITORINFO));
        if (!GetMonitorInfo(MonitorFromWindow(h, 2), ref info)) return false;
        return r.Left <= info.Monitor.Left && r.Top <= info.Monitor.Top && r.Right >= info.Monitor.Right && r.Bottom >= info.Monitor.Bottom;
    }
    // Every top-level window shown over its whole monitor: visible, not minimised, not cloaked (a suspended Store
    // app's), and not a tool or click-through window (WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT: an overlay's).
    public static IntPtr[] FullScreenWindows() {
        var found = new List<IntPtr>();
        EnumWindows((h, l) => {
            int cloaked = 0;
            bool isCloaked = DwmGetWindowAttribute(h, 14, out cloaked, 4) == 0 && cloaked != 0;
            if (IsWindowVisible(h) && !IsIconic(h) && (GetWindowLong(h, -20) & 0xA0) == 0 && !isCloaked && CoversItsMonitor(h)) found.Add(h);
            return true;
        }, IntPtr.Zero);
        return found.ToArray();
    }
    public static void Escape() { keybd_event(0x1B, 0, 0, UIntPtr.Zero); keybd_event(0x1B, 0, 2, UIntPtr.Zero); }
}
'@
[XsdLauncher]::SetProcessDPIAware() | Out-Null

# A window, as the log says it.
function Describe([IntPtr] $Window) {
    $process = (Get-Process -Id ([XsdLauncher]::Pid($Window)) -ErrorAction SilentlyContinue).ProcessName
    "$process '$([XsdLauncher]::Title($Window))' (class $([XsdLauncher]::Class($Window)))"
}

# What Steam started that is a game: its children, but for Steam's own helpers.
function Get-SteamGames {
    $all = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)
    $steam = @($all | Where-Object { $_.Name -ieq 'steam.exe' } | ForEach-Object { $_.ProcessId })
    @($all | Where-Object { $_.ParentProcessId -in $steam -and $_.Name -notmatch '^(steam.*|gameoverlayui.*|.*crashhandler.*)\.exe$' })
}

# Why the TV is in use, or $null when it is not. Sets $script:live to the live shell to stop for the test, if any.
function Get-InUse {
    $script:live = @()
    $live = @(Get-LiveShell)
    $livePids = @($live | ForEach-Object { $_.ProcessId })
    $others = @(Get-Process dashboard, Playnite* -ErrorAction SilentlyContinue | Where-Object { $_.Id -notin $livePids })
    if ($others.Count) { return "running: $(($others | ForEach-Object { "$($_.ProcessName) (pid $($_.Id)) $($_.Path)" }) -join ', ')" }
    if ($live.Count) {
        Note "the live shell: $(($live | ForEach-Object { "pid $($_.ProcessId): $($_.CommandLine)" }) -join '; ')"
        # The installed Dashboard with no watchdog was started by hand, not as the shell: the user's, so the TV in use.
        if (-not @($live | Where-Object { $_.Watchdog }).Count) {
            return "the installed Dashboard is running, but not as the shell (pid $($livePids -join ', '))"
        }
    } else {
        Note 'the live shell is not running'
    }
    $games = @(Get-SteamGames)
    if ($games.Count) { return "a game is running (Steam started it): $(($games | ForEach-Object { "$($_.Name) (pid $($_.ProcessId))" }) -join ', ')" }
    $front = [XsdLauncher]::GetForegroundWindow()
    if ($front -ne [IntPtr]::Zero) {
        $frontPid = [XsdLauncher]::Pid($front)
        $process = (Get-Process -Id $frontPid -ErrorAction SilentlyContinue).ProcessName
        $class = [XsdLauncher]::Class($front)
        $what = Describe $front
        Note "in front: $what"
        if ($frontPid -in $livePids) {
            # The live Dashboard in front: Home, or its Guide over a game. A full-screen window of another program
            # behind it is that game, or a film.
            foreach ($window in [XsdLauncher]::FullScreenWindows()) {
                $owner = [XsdLauncher]::Pid($window)
                if ($owner -in $livePids -or (Get-Process -Id $owner -ErrorAction SilentlyContinue).ProcessName -eq 'explorer') { continue }
                return "the live Dashboard is in front of a full-screen window: $(Describe $window)"
            }
            Note 'the live Dashboard is in front, and no other full-screen window is up'
        } elseif ($process -eq 'explorer' -or $class -in @('Progman', 'WorkerW', 'Shell_TrayWnd')) {
            # The desktop and the taskbar are explorer's, and so is Task View: none of them is the TV in use.
        } elseif ([XsdLauncher]::CoversItsMonitor($front)) {
            return "a full-screen window is in front: $what"
        }
    }
    $script:live = $live
    $null
}

# Starts the live shell again if this run stopped it; the marker stays for cleanup.ps1 if that fails.
function Restore-LiveShell {
    if (-not (Test-Path $stoppedMarker)) { return }
    if (Start-LiveShell ${function:Note}) { Remove-Item $stoppedMarker -Force -ErrorAction SilentlyContinue }
    else { Note 'the live shell is left for cleanup.ps1 to start again' }
}

try {
    $busy = Get-InUse
    if ($busy) {
        $code = 3
        Set-Content (Join-Path $out 'refused.txt') "the TV is in use, so no test ran: $busy" -Encoding UTF8
        Note "refused: $busy"
        return
    }
    if ($script:live.Count) {
        Note 'nothing but the live shell is going on: stopping it for the test'
        # Written first, so a run ended at any point from here starts it again.
        Set-Content $stoppedMarker $LiveShellExe -Encoding UTF8
        if (-not (Stop-LiveShell ${function:Note})) {
            $code = 3
            Set-Content (Join-Path $out 'refused.txt') 'the live Dashboard (the shell) could not be stopped, so no test ran' -Encoding UTF8
            Note 'refused: the live shell could not be stopped'
            return
        }
    }
    $env:E2E_SAFE = '1'
    $env:E2E_OUT = $out
    Note 'running run.ps1 in safe mode'
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $folder 'run.ps1') > (Join-Path $out 'console.txt') 2>&1
    $code = $LASTEXITCODE
    Note "run.ps1 exited with $code"
} catch {
    Note "error: $_"
    $code = 1
} finally {
    try {
        # A press that reached Windows rather than the Dashboard can leave Task View up (Win+Tab): put away.
        if ([XsdLauncher]::Title([XsdLauncher]::GetForegroundWindow()) -eq 'Task View') {
            [XsdLauncher]::Escape()
            Start-Sleep -Seconds 1
            Note 'Task View was in front: Esc'
        }
        Note "in front at the end: $([XsdLauncher]::Title([XsdLauncher]::GetForegroundWindow()))"
    } catch { Note "error at the end: $_" }
    try { Restore-LiveShell } catch { Note "error starting the live shell again: $_" }
    # Last: e2e/htpc/run.sh waits for it.
    Set-Content (Join-Path $out 'exitcode.txt') "$code" -Encoding Ascii
}
