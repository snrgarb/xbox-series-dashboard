//! `dashboard.exe --shell`: what Winlogon starts in place of explorer under shell mode (docs/spec/shell-mode.md, 5).
//! The Boot shell's job (the Bundle's bootshell/Program.cs), never touching the GPU, the library or the stores:
//!
//! 1. holds `Local\XboxSeriesDashboard.Shell` for the session, which the sentinel looks for;
//! 2. a black window over the screen first, then `ShellDesktopSwitchEvent`, so Welcome gives way to the black;
//! 3. the two events, made unset;
//! 4. waits for Windows' default audio device to settle (6 s at most), then starts `dashboard.exe --from-shell`;
//! 5. then every 100 ms: the events, the Dashboard's processes (the watchdog), the pad, explorer at the Boot video's
//!    end and whenever it has died, until Windows ends the session.
//!
//! Everything it does is a line in `<data>\shell\logs\shell-<stamp>.log`. Whatever goes wrong, it ends with a shell
//! window: a panic takes the black down and starts explorer.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, LRESULT, RECT, WAIT_OBJECT_0, WPARAM,
};
use windows_sys::Win32::Graphics::Dwm::DwmFlush;
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, DrawTextW, EndPaint, GetStockObject, InvalidateRect, SelectObject, SetBkMode,
    SetTextColor, UpdateWindow, BLACK_BRUSH, CLEARTYPE_QUALITY, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
    FW_NORMAL, HFONT, PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, ResetEvent, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
};
use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
    GetShellWindow, GetSystemMetrics, GetWindowThreadProcessId, PostMessageW, RegisterClassW, SetCursor, SetTimer,
    SetWindowPos, ShowWindow, TranslateMessage, ASFW_ANY, HWND_TOPMOST, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_SHUTTINGDOWN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SW_HIDE, SW_SHOWNOACTIVATE, WM_APP, WM_CLOSE, WM_ENDSESSION, WM_PAINT, WM_QUERYENDSESSION, WM_SETCURSOR, WM_TIMER,
    WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::dashboard::start_held;
use crate::log::Log;
use crate::processes::Processes;
use crate::setup::{self, WINLOGON};
use crate::system::{Place, System, ThisPc};
use crate::watch::{AudioSettle, Orders, PadPresses, PadWatch, ShellWatch, SignInSchedule, Watchdog};
use crate::{contract, dashboard};

const POLL: Duration = Duration::from_millis(100);
const PAD_EVERY: Duration = Duration::from_millis(30);
/// The black's own limit: down if the main loop hasn't renewed it for this long, so a hung --shell process never
/// keeps a topmost black over the desktop the safety net brings up.
const BLACK_OWN_LIMIT: Duration = Duration::from_secs(60);

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// `dashboard_args`: what the Dashboard is started with, `--from-shell` and the logging switches passed on
/// (`contract::from_shell_args`).
pub fn run(data: Option<PathBuf>, dashboard_args: Vec<String>) -> i32 {
    let name = wide(contract::SHELL_MUTEX);
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if mutex.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        eprintln!("shell: another --shell process runs in this session");
        return contract::NOT_A_DASHBOARD as i32;
    }
    let mut log = Log::new("shell");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Boot::run(&mut log, data, dashboard_args)));
    // Only a panic comes here: never leave the screen black with no shell.
    let why = match result {
        Ok(never) => match never {},
        Err(panic) => panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default(),
    };
    log.line(&format!("ERROR: {why}"));
    if let Some(black) = BLACK.get() {
        black.up.store(false, Ordering::Relaxed);
        apply_black();
        log.line("black down: error");
    }
    start_explorer(&mut log, &windows_folder().join("explorer.exe"));
    unsafe { CloseHandle(mutex) };
    1
}

fn windows_folder() -> PathBuf {
    std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
}

/// Starts explorer, only while no shell window is up, so never a second one.
fn start_explorer(log: &mut Log, explorer: &Path) {
    let shell = unsafe { GetShellWindow() };
    if !shell.is_null() {
        let mut owner = 0;
        unsafe { GetWindowThreadProcessId(shell, &mut owner) };
        log.line(&format!("explorer not started: a shell is up already, pid {owner}"));
        return;
    }
    match dashboard::start(explorer, &[]) {
        Ok(pid) => log.line(&format!("explorer started, pid {pid}: {}", explorer.display())),
        Err(why) => log.line(&format!("ERROR: explorer didn't start: {why}")),
    }
}

struct Events {
    first_frame: HANDLE,
    ended: HANDLE,
}

impl Events {
    fn make(log: &mut Log) -> Self {
        let make = |name: &str| unsafe {
            let name = wide(name);
            let handle = CreateEventW(std::ptr::null(), 1, 0, name.as_ptr());
            // One left over is unset.
            ResetEvent(handle);
            handle
        };
        let events = Events { first_frame: make(contract::FIRST_FRAME), ended: make(contract::BOOT_VIDEO_ENDED) };
        log.line(&format!("events made: {}, {}", contract::FIRST_FRAME, contract::BOOT_VIDEO_ENDED));
        events
    }

    fn set(handle: HANDLE) -> bool {
        unsafe { WaitForSingleObject(handle, 0) == WAIT_OBJECT_0 }
    }

    fn reset(&self) {
        unsafe {
            ResetEvent(self.first_frame);
            ResetEvent(self.ended);
        }
    }
}

struct Boot {
    began: Instant,
    exe: PathBuf,
    /// What the Dashboard is started with: `--from-shell`, and the logging switches `--shell` was given.
    args: Vec<String>,
    explorer: PathBuf,
    events: Events,
    processes: Processes,
    dog: Watchdog,
    first_seen: bool,
    ended_seen: bool,
}

impl Boot {
    fn now(&self) -> Duration {
        self.began.elapsed()
    }

    fn run(log: &mut Log, data: Option<PathBuf>, args: Vec<String>) -> std::convert::Infallible {
        let began = Instant::now();
        let running = std::env::current_exe().unwrap_or_default();
        // The Dashboard is the file beside this one under its own name: after an update renamed this one aside, the
        // new build starts (docs/research/shell.md, 4.1).
        let exe = running.with_file_name("dashboard.exe");
        log.line(&format!("--shell started, pid {}, from {}", std::process::id(), running.display()));
        unsafe {
            AllowSetForegroundWindow(ASFW_ANY);
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        show_black(began);
        log.line("black up");
        switch_desktop(log);
        let events = Events::make(log);

        // The Dashboard waits for the audio device, the events being made first, so a Start-up can set them.
        let mut settle = AudioSettle::default();
        let mut audio = DefaultAudio::default();
        loop {
            renew_black(began);
            let why = settle.start(audio.id(log), began.elapsed());
            for line in settle.take_said() {
                log.line(&line);
            }
            if let Some(why) = why {
                log.line(&format!("the Dashboard: {why}"));
                break;
            }
            std::thread::sleep(POLL);
        }

        let mut boot = Boot {
            began,
            exe: exe.clone(),
            args,
            explorer: windows_folder().join("explorer.exe"),
            events,
            processes: Processes::new(exe),
            dog: Watchdog::default(),
            first_seen: false,
            ended_seen: false,
        };
        boot.obey(log, Orders { launch: true, ..Orders::default() });

        // The log's file opens after, so the Dashboard never waits on the disk.
        if let Some(data) = &data {
            log.open(&data.join("shell").join("logs"));
        }
        say_the_pc(log);

        let pad = pad_thread();
        let mut schedule = SignInSchedule::default();
        let mut shell_watch = ShellWatch::default();
        let mut ending_said = false;
        loop {
            renew_black(began);
            for line in take_black_said() {
                log.line(&line);
            }
            if session_ending() {
                if !std::mem::replace(&mut ending_said, true) {
                    log.line("Windows is ending the session: watching stops");
                }
                std::thread::sleep(POLL);
                continue;
            }
            if std::mem::take(&mut ending_said) {
                log.line("Windows called the end of the session off: watching again");
            }
            let now = boot.now();
            if !boot.first_seen && Events::set(boot.events.first_frame) {
                boot.first_seen = true;
                log.line("signal: FirstFrame");
                schedule.first_frame(now);
                boot.dog.first_frame(now);
            }
            if !boot.ended_seen && Events::set(boot.events.ended) {
                boot.ended_seen = true;
                log.line("signal: BootVideoEnded");
                schedule.boot_video_ended();
            }
            let (appeared, exited) = boot.processes.poll();
            // Windows tells the Dashboard the session is ending too, and it may be gone first: hear it before
            // judging its exit.
            if session_ending() {
                continue;
            }
            for (pid, what) in appeared {
                boot.dog.appeared(pid, &what);
            }
            // One it didn't start is a Dashboard once it has the Dashboard's window; this exe's jobs never do.
            for pid in boot.processes.windowed(&boot.dog.windowless()) {
                boot.dog.has_window(pid);
            }
            for (pid, code) in exited {
                let orders = boot.dog.exited(pid, code, now);
                boot.obey(log, orders);
            }
            while let Ok((presses, said)) = pad.try_recv() {
                for line in said {
                    log.line(&line);
                }
                if presses.hold {
                    let orders = boot.dog.held(boot.now());
                    boot.obey(log, orders);
                }
                if presses.guide {
                    let orders = boot.dog.guide_pressed();
                    boot.obey(log, orders);
                }
            }
            let orders = boot.dog.tick(boot.now());
            boot.obey(log, orders);
            for line in boot.dog.take_said() {
                log.line(&line);
            }
            set_black(boot.dog.black(), boot.dog.line(), log);
            if let Some(why) = schedule.start_explorer(boot.now()) {
                log.line(&format!("explorer: {why}"));
                start_explorer(log, &boot.explorer);
            }
            let shell_up = !unsafe { GetShellWindow() }.is_null();
            if shell_watch.start_explorer(shell_up, boot.now()) {
                log.line(&format!(
                    "explorer: no shell window for {} s; starting it again",
                    crate::watch::SHELL_GRACE.as_secs()
                ));
                start_explorer(log, &boot.explorer);
            }
            std::thread::sleep(POLL);
        }
    }

    /// Carries out what the watchdog ordered.
    fn obey(&mut self, log: &mut Log, orders: Orders) {
        let mut launch = orders.launch;
        while launch {
            launch = false;
            self.events.reset();
            (self.first_seen, self.ended_seen) = (false, false);
            log.line("events reset");
            set_black(true, self.dog.line(), log);
            let args: Vec<&str> = self.args.iter().map(String::as_str).collect();
            match start_held(&self.exe, &args) {
                Ok((pid, handle)) => {
                    self.processes.adopt(pid, handle);
                    self.dog.launched(pid, self.now());
                    log.line(&format!("started pid {pid}: \"{}\" {}", self.exe.display(), self.args.join(" ")));
                }
                Err(why) => {
                    log.line(&format!("ERROR: \"{}\" didn't start: {why}", self.exe.display()));
                    let orders = self.dog.launch_failed(self.now());
                    launch = orders.launch;
                }
            }
            for line in self.dog.take_said() {
                log.line(&line);
            }
        }
        if !orders.close.is_empty() {
            let asked = self.processes.close(&orders.close);
            log.line(&format!("WM_CLOSE to {asked} window(s) of pid {:?}", orders.close));
        }
        if !orders.end.is_empty() {
            self.processes.end(&orders.end);
            log.line(&format!("ended pid {:?}", orders.end));
        }
        if orders.start_desktop {
            start_explorer(log, &self.explorer);
        }
        for line in self.dog.take_said() {
            log.line(&line);
        }
    }
}

/// Tells Winlogon the shell's desktop is up, so its Welcome screen gives way to the black.
fn switch_desktop(log: &mut Log) {
    let name = wide("ShellDesktopSwitchEvent");
    unsafe {
        let handle = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
        if handle.is_null() {
            log.line(&format!("ERROR: couldn't open ShellDesktopSwitchEvent: {}", std::io::Error::last_os_error()));
            return;
        }
        let set = SetEvent(handle) != 0;
        let error = std::io::Error::last_os_error();
        CloseHandle(handle);
        log.line(&if set {
            "ShellDesktopSwitchEvent set".to_string()
        } else {
            format!("ERROR: couldn't set ShellDesktopSwitchEvent: {error}")
        });
    }
}

/// What the log keeps of the PC at each sign-in: Windows' build, and the machine's shell and policy, read only.
fn say_the_pc(log: &mut Log) {
    let pc = ThisPc { user: String::new(), scratch: PathBuf::new() };
    let read = |key: &str, name: &str| {
        pc.read(&Place::machine(key, name)).ok().flatten().map_or("none".to_string(), |value| value.to_string())
    };
    let version = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    log.line(&format!(
        "Windows build {}.{} {}",
        read(version, "CurrentBuild"),
        read(version, "UBR"),
        read(version, "DisplayVersion")
    ));
    log.line(&format!(
        "HKLM Shell {}; OverrideShellProgram {}; HKCU Shell {}",
        read(WINLOGON, "Shell"),
        read(setup::POLICY, "OverrideShellProgram"),
        pc.read(&setup::shell_place()).ok().flatten().map_or("none".to_string(), |value| value.to_string())
    ));
}

/// Reads the pads on a thread of their own, every 30 ms: what they asked, and what was said of it.
fn pad_thread() -> mpsc::Receiver<(PadPresses, Vec<String>)> {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("shell pad".into())
        .spawn(move || {
            let began = Instant::now();
            let pads = input::RawPads::new();
            let mut watch = PadWatch::default();
            loop {
                let mut presses = PadPresses::default();
                for (pad, buttons) in pads.read().into_iter().enumerate() {
                    let read = watch.read(pad, buttons, began.elapsed());
                    presses.guide |= read.guide;
                    presses.hold |= read.hold;
                }
                let said = watch.take_said();
                if (presses.guide || presses.hold || !said.is_empty()) && sender.send((presses, said)).is_err() {
                    return;
                }
                std::thread::sleep(PAD_EVERY);
            }
        })
        .expect("the pad thread starts");
    receiver
}

/// Windows' default audio device (eRender, eConsole), through Core Audio. One that can't be read counts as none:
/// audio least ready is what the wait is for, and the 6 s limit still starts the Dashboard.
#[derive(Default)]
struct DefaultAudio {
    failed: Option<String>,
}

impl DefaultAudio {
    fn id(&mut self, log: &mut Log) -> Option<String> {
        use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
        };
        let read = || -> windows::core::Result<Option<String>> {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let device = match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                    Ok(device) => device,
                    // E_NOTFOUND: no audio device.
                    Err(e) if e.code().0 as u32 == 0x8007_0490 => return Ok(None),
                    Err(e) => return Err(e),
                };
                let id = device.GetId()?;
                let text = id.to_string().unwrap_or_default();
                CoTaskMemFree(Some(id.0 as *const core::ffi::c_void));
                Ok(Some(text))
            }
        };
        match read() {
            Ok(id) => id,
            Err(e) => {
                let message = e.message();
                if self.failed.as_deref() != Some(&message) {
                    log.line(&format!("ERROR: audio: couldn't read the default audio device: {message}"));
                    self.failed = Some(message);
                }
                None
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------- the black

/// The black window's state, shared between its own thread, which owns the window, and the main loop.
struct BlackState {
    began: Instant,
    up: AtomicBool,
    line: Mutex<Option<String>>,
    /// When the main loop last renewed the black, in ms since the start.
    renewed: AtomicU64,
    ending: AtomicBool,
    window: AtomicIsize,
    said: Mutex<Vec<String>>,
}

static BLACK: OnceLock<BlackState> = OnceLock::new();
const WM_APPLY: u32 = WM_APP + 1;
const CLASS: &str = "XboxSeriesDashboard.Shell.Black";

/// Shows the black on a thread of its own, and waits until the desktop has composed it.
fn show_black(began: Instant) {
    let _ = BLACK.set(BlackState {
        began,
        up: AtomicBool::new(true),
        line: Mutex::new(None),
        renewed: AtomicU64::new(0),
        ending: AtomicBool::new(false),
        window: AtomicIsize::new(0),
        said: Mutex::new(Vec::new()),
    });
    let (ready, shown) = mpsc::channel();
    std::thread::Builder::new()
        .name("shell black".into())
        .spawn(move || unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = wide(CLASS);
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(black_proc),
                hInstance: instance,
                hbrBackground: GetStockObject(BLACK_BRUSH),
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&window_class);
            let window = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if let Some(state) = BLACK.get() {
                state.window.store(window as isize, Ordering::Relaxed);
            }
            ShowWindow(window, SW_SHOWNOACTIVATE);
            UpdateWindow(window);
            DwmFlush();
            SetTimer(window, 1, 250, None);
            let _ = ready.send(());
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        })
        .expect("the black's thread starts");
    let _ = shown.recv_timeout(Duration::from_secs(5));
}

fn renew_black(began: Instant) {
    if let Some(state) = BLACK.get() {
        state.renewed.store(began.elapsed().as_millis() as u64, Ordering::Relaxed);
    }
}

fn session_ending() -> bool {
    BLACK.get().is_some_and(|state| state.ending.load(Ordering::Relaxed))
        || unsafe { GetSystemMetrics(SM_SHUTTINGDOWN) } != 0
}

fn take_black_said() -> Vec<String> {
    BLACK.get().map_or(Vec::new(), |state| std::mem::take(&mut *state.said.lock().unwrap()))
}

/// Puts the black and its line up or down as the watchdog has them, saying each change.
fn set_black(up: bool, line: Option<&str>, log: &mut Log) {
    let Some(state) = BLACK.get() else { return };
    let was = state.up.swap(up, Ordering::Relaxed);
    let line = line.map(String::from);
    let line_changed = {
        let mut shown = state.line.lock().unwrap();
        let changed = *shown != line;
        *shown = line.clone();
        changed
    };
    if was != up {
        log.line(if up { "black up" } else { "black down" });
    }
    if line_changed {
        log.line(&line.map_or("line down".into(), |line| format!("line up: {line}")));
    }
    if was != up || line_changed {
        apply_black();
    }
}

/// Has the black's thread put the window as the state says.
fn apply_black() {
    if let Some(state) = BLACK.get() {
        let window = state.window.load(Ordering::Relaxed) as HWND;
        if !window.is_null() {
            unsafe { PostMessageW(window, WM_APPLY, 0, 0) };
        }
    }
}

unsafe extern "system" fn black_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(state) = BLACK.get() else { return DefWindowProcW(window, message, wparam, lparam) };
    match message {
        WM_SETCURSOR => {
            SetCursor(std::ptr::null_mut());
            1
        }
        // It can't be closed, only taken down.
        WM_CLOSE => 0,
        WM_PAINT => {
            paint(window, state);
            0
        }
        WM_QUERYENDSESSION => {
            state.ending.store(true, Ordering::Relaxed);
            1
        }
        // wParam 0: the end was called off.
        WM_ENDSESSION => {
            state.ending.store(wparam != 0, Ordering::Relaxed);
            0
        }
        WM_TIMER => {
            let renewed = Duration::from_millis(state.renewed.load(Ordering::Relaxed));
            if state.up.load(Ordering::Relaxed) && state.began.elapsed().saturating_sub(renewed) > BLACK_OWN_LIMIT {
                state.up.store(false, Ordering::Relaxed);
                ShowWindow(window, SW_HIDE);
                state.said.lock().unwrap().push(format!(
                    "black down by its own limit: the main loop hasn't renewed it for {} s",
                    BLACK_OWN_LIMIT.as_secs()
                ));
            }
            0
        }
        WM_APPLY => {
            if state.up.load(Ordering::Relaxed) {
                SetWindowPos(
                    window,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                InvalidateRect(window, std::ptr::null(), 1);
                UpdateWindow(window);
                DwmFlush();
            } else {
                ShowWindow(window, SW_HIDE);
            }
            0
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

static FONT: AtomicIsize = AtomicIsize::new(0);

unsafe fn paint(window: HWND, state: &BlackState) {
    let mut paint: PAINTSTRUCT = std::mem::zeroed();
    let dc = BeginPaint(window, &mut paint);
    if let Some(line) = state.line.lock().unwrap().clone() {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(window, &mut rect);
        let mut font = FONT.load(Ordering::Relaxed) as HFONT;
        if font.is_null() {
            let face = wide("Segoe UI");
            font = CreateFontW(
                -(rect.bottom - rect.top) / 30,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                0,
                0,
                0,
                CLEARTYPE_QUALITY as u32,
                0,
                face.as_ptr(),
            );
            FONT.store(font as isize, Ordering::Relaxed);
        }
        let old = SelectObject(dc, font);
        SetTextColor(dc, 0x00FF_FFFF);
        SetBkMode(dc, TRANSPARENT as i32);
        let text: Vec<u16> = line.encode_utf16().collect();
        DrawTextW(
            dc,
            text.as_ptr(),
            text.len() as i32,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(dc, old);
    }
    EndPaint(window, &paint);
}
