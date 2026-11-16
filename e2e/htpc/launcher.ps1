# Runs one test on the TV PC's desktop, in safe mode, from the folder e2e/htpc/run.sh copied it to: started by a
# one-off interactive scheduled task (run.cmd), so it is in the signed-in user's session, where the screen, the
# foreground and the pad are.
#
# First it looks whether the TV is in use, and if it is it runs nothing: a window other than the desktop's covering
# the whole screen in front (a game, a film, the lock screen), or the Dashboard or Playnite running. Then it runs the
# payload's run.ps1 with E2E_SAFE=1 and E2E_OUT=<folder>\out. Whatever happens, Windows' Task View is not left in front,
# and out\exitcode.txt is written last: 3 when the TV was in use (out\refused.txt says why), else the payload's code.
$ErrorActionPreference = 'Continue'
$folder = $PSScriptRoot
$out = Join-Path $folder 'out'
New-Item $out -ItemType Directory -Force | Out-Null
$code = 1

function Note([string] $Text) {
    Add-Content (Join-Path $out 'launcher.txt') "[$(Get-Date -Format HH:mm:ss.fff)] $Text" -Encoding UTF8
}

Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class XsdLauncher {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct MONITORINFO { public int Size; public RECT Monitor, Work; public uint Flags; }
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern IntPtr MonitorFromWindow(IntPtr h, uint flags);
    [DllImport("user32.dll")] static extern bool GetMonitorInfo(IntPtr m, ref MONITORINFO info);
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
    public static void Escape() { keybd_event(0x1B, 0, 0, UIntPtr.Zero); keybd_event(0x1B, 0, 2, UIntPtr.Zero); }
}
'@
[XsdLauncher]::SetProcessDPIAware() | Out-Null

# Why the TV is in use, or $null when it is not.
function Get-InUse {
    $running = @(Get-Process dashboard, Playnite* -ErrorAction SilentlyContinue)
    if ($running.Count) { return "running: $(($running | ForEach-Object { "$($_.ProcessName) (pid $($_.Id))" }) -join ', ')" }
    $front = [XsdLauncher]::GetForegroundWindow()
    if ($front -eq [IntPtr]::Zero) { return $null }
    $process = (Get-Process -Id ([XsdLauncher]::Pid($front)) -ErrorAction SilentlyContinue).ProcessName
    $class = [XsdLauncher]::Class($front)
    $what = "$process '$([XsdLauncher]::Title($front))' (class $class)"
    Note "in front: $what"
    # The desktop and the taskbar are explorer's, and so is Task View: none of them is the TV in use.
    if ($process -eq 'explorer' -or $class -in @('Progman', 'WorkerW', 'Shell_TrayWnd')) { return $null }
    if ([XsdLauncher]::CoversItsMonitor($front)) { return "a full-screen window is in front: $what" }
    $null
}

try {
    $busy = Get-InUse
    if ($busy) {
        $code = 3
        Set-Content (Join-Path $out 'refused.txt') "the TV is in use, so no test ran: $busy" -Encoding UTF8
        Note "refused: $busy"
        return
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
    # Last: e2e/htpc/run.sh waits for it.
    Set-Content (Join-Path $out 'exitcode.txt') "$code" -Encoding Ascii
}
