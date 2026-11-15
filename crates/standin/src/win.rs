//! The stand-in game's window and the stand-in Steam, on Windows.
use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, InvalidateRect,
    SelectObject, SetBkMode, SetTextColor, DT_CENTER, DT_SINGLELINE, DT_VCENTER, PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::{
    RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_SZ,
};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::{install_dir, note, GameArgs};

const VK_ESCAPE: WPARAM = 0x1B;
/// The timer that ends the launcher's window, and the one that ends the game.
const SPLASH_OVER: usize = 1;
const TIME_UP: usize = 2;
/// The timer the game's pulse beats to.
const PULSE: usize = 3;
/// How often the pulse beats: the square in the game's window changes colour each time.
const PULSE_MS: u32 = 250;
/// A beat this long after the one before means the game stood still meanwhile: it was frozen.
const STOOD_STILL_MS: u64 = 1500;
/// The pulse's two colours, 0x00BBGGRR: (200, 200, 40) and (40, 200, 200). Far enough apart to tell under the
/// Guide's dim.
const PULSE_COLOURS: [u32; 2] = [0x0028_C8C8, 0x00C8_C828];
/// 0x00BBGGRR: the dark blue of the theme's stand-in game, (16, 32, 80).
const BLUE: u32 = 0x0050_2010;
const WHITE: u32 = 0x00FF_FFFF;

/// Set once the game was asked to end, by its window closing, Esc or its time running out, rather than its
/// launcher's window going away on its own.
static ENDED: AtomicBool = AtomicBool::new(false);
/// The game's own window, once it is up: the one that has the pulse.
static GAME: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
/// How many times the pulse has beaten, and when it last did, in ms since Windows started.
static BEATS: AtomicU64 = AtomicU64::new(0);
static LAST_BEAT: AtomicU64 = AtomicU64::new(0);

/// Where the pulse is in a window this size: on a 1920x1080 screen the square x 1700..1820, y 900..1020, clear of
/// everything the Guide draws over the game but its dim.
fn pulse_rect(client: &RECT) -> RECT {
    let (w, h) = (client.right - client.left, client.bottom - client.top);
    RECT { left: w * 1700 / 1920, top: h * 900 / 1080, right: w * 1820 / 1920, bottom: h * 1020 / 1080 }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn game(args: &GameArgs) -> i32 {
    unsafe {
        SetProcessDPIAware();
        let instance = GetModuleHandleW(null());
        let class = wide("StandInGame");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = instance;
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.hbrBackground = CreateSolidBrush(BLUE);
        wc.lpszClassName = class.as_ptr();
        if RegisterClassW(&wc) == 0 {
            note("game: could not register its window class");
            return 1;
        }
        let (width, height) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        let open = |title: &str, x: i32, y: i32, w: i32, h: i32| {
            let title = wide(title);
            let style = WS_POPUP | WS_VISIBLE;
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                style,
                x,
                y,
                w,
                h,
                null_mut(),
                null_mut(),
                instance,
                null(),
            )
        };

        if args.splash_ms > 0 {
            let title = format!("{} launcher", args.title);
            let splash = open(&title, (width - 400) / 2, (height - 200) / 2, 400, 200);
            if splash.is_null() {
                note("game: could not open its launcher's window");
                return 1;
            }
            SetTimer(splash, SPLASH_OVER, args.splash_ms, None);
            note(&format!("game: \"{title}\" up for {} ms", args.splash_ms));
            pump();
            if ENDED.load(Ordering::Relaxed) {
                note("game: closed at its launcher");
                return 0;
            }
        }

        let window = open(&args.title, 0, 0, width, height);
        if window.is_null() {
            note("game: could not open its window");
            return 1;
        }
        SetTimer(window, TIME_UP, args.seconds.saturating_mul(1000).min(USER_TIMER_MAXIMUM), None);
        // Something that moves, so a test can see the game running, and see it stand still while it is frozen.
        GAME.store(window, Ordering::Relaxed);
        LAST_BEAT.store(GetTickCount64(), Ordering::Relaxed);
        SetTimer(window, PULSE, PULSE_MS, None);
        SetForegroundWindow(window);
        note(&format!("game: \"{}\" up, {width}x{height}", args.title));
        pump();
        note(&format!("game: \"{}\" closed", args.title));
    }
    0
}

/// Runs the thread's messages until a window of ours is destroyed.
unsafe fn pump() {
    let mut message: MSG = std::mem::zeroed();
    while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_TIMER if wparam == SPLASH_OVER => {
            DestroyWindow(hwnd);
        }
        WM_TIMER if wparam == PULSE => {
            let now = GetTickCount64();
            let since = now.saturating_sub(LAST_BEAT.swap(now, Ordering::Relaxed));
            let beats = BEATS.fetch_add(1, Ordering::Relaxed) + 1;
            if since >= STOOD_STILL_MS {
                note(&format!("game: stood still for {since} ms, until beat {beats}"));
            }
            let mut client: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut client);
            InvalidateRect(hwnd, &pulse_rect(&client), 0);
        }
        WM_TIMER | WM_CLOSE => {
            ENDED.store(true, Ordering::Relaxed);
            DestroyWindow(hwnd);
        }
        WM_KEYDOWN if wparam == VK_ESCAPE => {
            ENDED.store(true, Ordering::Relaxed);
            DestroyWindow(hwnd);
        }
        WM_DESTROY => PostQuitMessage(0),
        WM_PAINT => {
            // The window's title in its middle, a twelfth of the window high.
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let mut title = [0u16; 256];
            let length = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
            let font = CreateFontW(rect.bottom / 12, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0, wide("Segoe UI").as_ptr());
            let before = SelectObject(dc, font);
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, WHITE);
            DrawTextW(dc, title.as_ptr(), length, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            SelectObject(dc, before);
            DeleteObject(font);
            if hwnd == GAME.load(Ordering::Relaxed) {
                // The pulse: a square that is one colour on even beats and another on odd ones.
                let brush = CreateSolidBrush(PULSE_COLOURS[(BEATS.load(Ordering::Relaxed) % 2) as usize]);
                FillRect(dc, &pulse_rect(&rect), brush);
                DeleteObject(brush);
            }
            EndPaint(hwnd, &paint);
        }
        _ => return DefWindowProcW(hwnd, message, wparam, lparam),
    }
    0
}

/// Posts a notification to the running Dashboard: its text, to the Toast strip's window. 0 when the Dashboard took
/// it.
pub fn toast(text: &str) -> i32 {
    let class = wide(model::post::WINDOW_CLASS);
    let data = COPYDATASTRUCT {
        dwData: model::post::COPYDATA_ID,
        cbData: text.len() as u32,
        lpData: text.as_ptr() as *mut c_void,
    };
    let mut taken = 0usize;
    let sent = unsafe {
        let window = FindWindowW(class.as_ptr(), null());
        if window.is_null() {
            note("toast: no Dashboard with a Toast strip is running");
            return 1;
        }
        // Waited for, but not for ever: a Dashboard that has hung answers nothing.
        let data = &data as *const COPYDATASTRUCT as LPARAM;
        SendMessageTimeoutW(window, WM_COPYDATA, 0, data, SMTO_ABORTIFHUNG, 5000, &mut taken)
    };
    if sent == 0 || taken == 0 {
        note(&format!("toast: the Dashboard did not take {text:?}"));
        return 1;
    }
    note(&format!("toast: posted {text:?}"));
    0
}

/// Starts `appid`'s game as Steam would and waits for it, Steam's `Running` value saying so meanwhile.
pub fn steam_url(appid: u32) -> i32 {
    let Some(root) = steam_path() else {
        note("steam-url: no HKCU\\Software\\Valve\\Steam\\SteamPath");
        return 1;
    };
    let Some(dir) = install_dir(&root, appid) else {
        note(&format!("steam-url: {appid} isn't installed in {}", root.display()));
        return 1;
    };
    let exe = dir.join("game.exe");
    let mut game = match std::process::Command::new(&exe).current_dir(&dir).spawn() {
        Ok(game) => game,
        Err(e) => {
            note(&format!("steam-url: could not start {}: {e}", exe.display()));
            return 1;
        }
    };
    set_running(appid, 1);
    note(&format!("steam-url: {appid} running, {} pid {}", exe.display(), game.id()));
    let status = game.wait();
    set_running(appid, 0);
    note(&format!("steam-url: {appid} ended ({})", status.map_or_else(|e| e.to_string(), |s| s.to_string())));
    0
}

/// `HKCU\Software\Valve\Steam\SteamPath`, which Steam writes with forward slashes.
fn steam_path() -> Option<PathBuf> {
    let (key, value) = (wide("Software\\Valve\\Steam"), wide("SteamPath"));
    let mut buffer = [0u16; 1024];
    let mut size = std::mem::size_of_val(&buffer) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr() as *mut c_void,
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    let text = &buffer[..(size as usize / 2).min(buffer.len())];
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    Some(PathBuf::from(String::from_utf16_lossy(&text[..end]).replace('/', "\\")))
}

/// `HKCU\Software\Valve\Steam\Apps\<appid>\Running`, the key made if it isn't there.
fn set_running(appid: u32, running: u32) {
    let (key, value) = (wide(&format!("Software\\Valve\\Steam\\Apps\\{appid}")), wide("Running"));
    let data = &running as *const u32 as *const c_void;
    let status = unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), REG_DWORD, data, 4) };
    if status != 0 {
        note(&format!("steam-url: could not set {appid}'s Running to {running} (error {status})"));
    }
}
