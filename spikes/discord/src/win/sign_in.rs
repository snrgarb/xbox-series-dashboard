//! `--sign-in`: discord.com/login full screen, for the USER to sign in with the pad (or a real keyboard and mouse).
//! The spike never types, reads or keeps a credential: the pad's presses become key events in the page, the
//! keyboard's keys become `Input.insertText`, and nothing typed is logged. The log says where the page went (host and
//! path) and what Discord showed (login form, captcha, two-factor, QR code, a new-location check), and when the app
//! page is reached; the step then waits for the `--quit` file or `--minutes`. The session stays in `--data`.
//!
//! The pad, keyboard closed: D-pad up/down Shift+Tab/Tab, left/right the arrow keys, LB/RB arrow up/down, A Enter,
//! B Escape, X Space, View Backspace, Y the keyboard; left stick the mouse pointer, right trigger its left button,
//! right stick the wheel. Keyboard open: D-pad over the keys, A types, X Backspace, Y space, LB the next layer,
//! View Tab, Menu Enter, B closes it.
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use windows::core::{w, GUID, HRESULT, HSTRING, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, CLSCTX_LOCAL_SERVER};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{GetDesktopWindow, SW_SHOWNORMAL};

use super::host::{self, pump, yes, Host, Key, Loader, Show, CLOSED};
use super::system;
use crate::args::{Keyboard, Options};
use crate::keyboard::{Board, LAYER_NAMES};
use crate::pad::{self, Presses};
use crate::say;

#[derive(Default)]
struct Ui {
    keyboard: bool,
    board: Board,
}

pub fn run(options: &Options, loader: &Loader) -> i32 {
    let host = match Host::open(loader, &options.data, Show::Full, options.browser_args.as_deref()) {
        Ok(host) => host,
        Err(e) => {
            say!("sign-in: no web view: {e}");
            return 8;
        }
    };
    if options.quit.exists() {
        let _ = std::fs::remove_file(&options.quit);
        say!("sign-in: an old quit file was removed");
    }
    say!(
        "sign-in: keyboard {:?}; ends when {} appears, the window is closed, or after {} min",
        options.keyboard,
        options.quit.display(),
        options.minutes
    );

    let area = host.area;
    let height = area.bottom - area.top;
    let bar = height / 22;
    let keys_height = height * 3 / 10;
    let ui = Rc::new(RefCell::new(Ui::default()));
    {
        let ui = ui.clone();
        let kind = options.keyboard;
        host::set_painter(Box::new(move |hdc, rect| {
            // Never while the pad's handling holds it: the next paint draws it.
            if let Ok(ui) = ui.try_borrow() {
                paint(hdc, rect, &ui, bar, keys_height, kind);
            }
        }));
    }
    let layout = |host: &Host, keyboard: bool| {
        let bottom = area.bottom - bar - if keyboard { keys_height } else { 0 };
        host.set_bounds(RECT { bottom, ..area });
        unsafe {
            let _ = InvalidateRect(Some(host.hwnd), None, true);
        }
    };
    layout(&host, false);
    host.navigate("https://discord.com/login");
    host.focus();

    let started = Instant::now();
    let limit = Duration::from_secs(options.minutes * 60);
    let mut presses = Presses::default();
    let mut pad_slot: Option<Option<u32>> = None;
    let mut clicking = false;
    let mut last_look = Instant::now() - Duration::from_secs(10);
    let mut shown = String::new();
    let mut app_reached = false;
    let mut was_editable = false;
    let mut tip = None;
    loop {
        if CLOSED.with(|c| c.get()) {
            say!("sign-in: the window was closed");
            break;
        }
        if options.quit.exists() {
            say!("sign-in: the quit file is there");
            let _ = std::fs::remove_file(&options.quit);
            break;
        }
        if started.elapsed() >= limit {
            say!("sign-in: {} min are up", options.minutes);
            break;
        }
        pump(Duration::from_millis(16));

        // The pad.
        let (slot, state) = read_pad();
        if pad_slot != Some(slot) {
            match slot {
                Some(n) => say!("pad: one is connected (XInput slot {n})"),
                None => say!("pad: none connected"),
            }
            pad_slot = Some(slot);
        }
        if let Some(state) = state {
            let g = state.Gamepad;
            let pressed = presses.update(g.wButtons.0, Instant::now());
            let mut ui_now = ui.borrow_mut();
            if ui_now.keyboard {
                let mut toggle = false;
                for (bit, dx, dy) in [(pad::UP, 0, -1), (pad::DOWN, 0, 1), (pad::LEFT, -1, 0), (pad::RIGHT, 1, 0)] {
                    if pressed & bit != 0 {
                        ui_now.board.step(dx, dy);
                    }
                }
                if pressed & pad::A != 0 {
                    host.cdp_send("Input.insertText", serde_json::json!({ "text": ui_now.board.key().to_string() }));
                }
                if pressed & pad::X != 0 {
                    host.key(Key::Backspace, false);
                }
                if pressed & pad::Y != 0 {
                    host.cdp_send("Input.insertText", serde_json::json!({ "text": " " }));
                }
                if pressed & pad::LB != 0 {
                    ui_now.board.next_layer();
                }
                if pressed & pad::VIEW != 0 {
                    host.key(Key::Tab, false);
                }
                if pressed & pad::MENU != 0 {
                    host.key(Key::Enter, false);
                }
                if pressed & pad::B != 0 {
                    toggle = true;
                }
                if pressed != 0 {
                    unsafe {
                        let _ = InvalidateRect(Some(host.hwnd), None, false);
                    }
                }
                if toggle {
                    ui_now.keyboard = false;
                    drop(ui_now);
                    layout(&host, false);
                    say!("keyboard: closed with B");
                }
            } else {
                let keys = [
                    (pad::UP, Key::Tab, true),
                    (pad::DOWN, Key::Tab, false),
                    (pad::LEFT, Key::Left, false),
                    (pad::RIGHT, Key::Right, false),
                    (pad::LB, Key::Up, false),
                    (pad::RB, Key::Down, false),
                    (pad::A, Key::Enter, false),
                    (pad::B, Key::Escape, false),
                    (pad::X, Key::Space, false),
                    (pad::VIEW, Key::Backspace, false),
                ];
                for (bit, key, shift) in keys {
                    if pressed & bit != 0 {
                        host.key(key, shift);
                    }
                }
                drop(ui_now);
                if pressed & pad::Y != 0 {
                    keyboard(options.keyboard, &host, &ui, &layout, &mut tip, "Y");
                }
            }
            // The pointer, its button and the wheel, for what keys cannot reach (a captcha's checkbox).
            let (dx, dy) = pad::pointer(g.sThumbLX, g.sThumbLY, height as f32 / 70.0);
            if dx != 0 || dy != 0 {
                mouse(dx, dy, 0, MOUSEEVENTF_MOVE);
            }
            let wheel = pad::scroll(g.sThumbRY) / 3;
            if wheel != 0 {
                mouse(0, 0, wheel, MOUSEEVENTF_WHEEL);
            }
            let trigger = g.bRightTrigger > 64;
            if trigger != clicking {
                mouse(0, 0, 0, if trigger { MOUSEEVENTF_LEFTDOWN } else { MOUSEEVENTF_LEFTUP });
                clicking = trigger;
            }
        }

        // What the page shows, every two seconds.
        if last_look.elapsed() >= Duration::from_secs(2) {
            last_look = Instant::now();
            if let Ok(s) = host.state() {
                let flag = |k: &str| yes(s[k] == true);
                let now = format!(
                    "route {}, login form {}, password field {}, captcha {} ({} hCaptcha frames), two-factor {}, QR code {}, new-location check {}, app {}",
                    s["route"].as_str().unwrap_or("?"),
                    flag("login_form"),
                    flag("password_field"),
                    flag("captcha"),
                    s["captcha_frames"],
                    flag("two_factor"),
                    flag("qr"),
                    flag("new_location"),
                    flag("app")
                );
                if now != shown {
                    say!("page: {now}");
                    shown = now;
                }
                if s["app"] == true && !app_reached {
                    app_reached = true;
                    say!(
                        "sign-in: the app page is reached after {} s: signed in; the session is kept in {}",
                        started.elapsed().as_secs(),
                        options.data.display()
                    );
                }
                let editable = s["editable"] == true;
                if editable != was_editable {
                    say!(
                        "focus: {}",
                        if editable {
                            format!("a text field ({})", s["editable_kind"].as_str().unwrap_or("?"))
                        } else {
                            "not a text field".into()
                        }
                    );
                    if editable && options.keyboard == Keyboard::Pad && !ui.borrow().keyboard {
                        keyboard(options.keyboard, &host, &ui, &layout, &mut tip, "a text field took focus");
                    }
                    was_editable = editable;
                }
            }
        }
    }
    if clicking {
        mouse(0, 0, 0, MOUSEEVENTF_LEFTUP);
    }
    say!("sign-in: closing; signed in {}; the session stays in {}", yes(app_reached), options.data.display());
    host.close();
    super::probe::folder(&options.data);
    0
}

/// The first connected pad, and its state.
fn read_pad() -> (Option<u32>, Option<XINPUT_STATE>) {
    for slot in 0..4 {
        let mut state = XINPUT_STATE::default();
        if unsafe { XInputGetState(slot, &mut state) } == 0 {
            return (Some(slot), Some(state));
        }
    }
    (None, None)
}

fn mouse(dx: i32, dy: i32, wheel: i32, flags: MOUSE_EVENT_FLAGS) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT { dx, dy, mouseData: wheel as u32, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        thread_local!(static SAID: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });
        if !SAID.with(|s| s.replace(true)) {
            say!("pointer: SendInput refused ({})", windows::core::Error::from_thread());
        }
    }
}

/// Windows' touch keyboard's own toggle (undocumented, as every program that shows it uses it).
#[windows::core::interface("37c994e7-432b-4834-a2f7-dce1f13b834b")]
unsafe trait ITipInvocation: windows::core::IUnknown {
    fn toggle(&self, wnd: HWND) -> HRESULT;
}
const TIP: GUID = GUID::from_u128(0x4ce576fa_83dc_4f88_951c_9d0782b4e376);

/// Brings up (or puts away) the keyboard chosen.
fn keyboard(
    kind: Keyboard,
    host: &Host,
    ui: &Rc<RefCell<Ui>>,
    layout: &dyn Fn(&Host, bool),
    tip: &mut Option<ITipInvocation>,
    why: &str,
) {
    match kind {
        Keyboard::Pad => {
            let open = {
                let mut ui = ui.borrow_mut();
                ui.keyboard = !ui.keyboard;
                ui.keyboard
            };
            layout(host, open);
            say!("keyboard: the spike's pad keyboard {} ({why})", if open { "opened" } else { "closed" });
        }
        Keyboard::TabTip => {
            let counts = system::processes(&["TabTip.exe", "TextInputHost.exe"]);
            if counts[0] == 0 {
                let path =
                    std::env::var("CommonProgramFiles").unwrap_or_else(|_| r"C:\Program Files\Common Files".into());
                start(&Path::new(&path).join(r"microsoft shared\ink\TabTip.exe").to_string_lossy());
                pump(Duration::from_millis(800));
            }
            if tip.is_none() {
                match unsafe {
                    CoCreateInstance::<_, ITipInvocation>(&TIP, None, CLSCTX_INPROC_SERVER | CLSCTX_LOCAL_SERVER)
                } {
                    Ok(t) => *tip = Some(t),
                    Err(e) => say!("keyboard: the touch keyboard's toggle cannot be had: {e}"),
                }
            }
            if let Some(t) = tip {
                let hr = unsafe { t.toggle(GetDesktopWindow()) };
                say!(
                    "keyboard: touch keyboard toggled ({why}): {hr:?}; TabTip.exe was running {}, TextInputHost.exe {}",
                    yes(counts[0] > 0),
                    yes(counts[1] > 0)
                );
            }
            host.focus();
        }
        Keyboard::Osk => {
            start("osk.exe");
            say!("keyboard: osk.exe asked for ({why}); press its keys with the left stick's pointer and the right trigger");
        }
        Keyboard::None => say!("keyboard: none (--keyboard none): a real keyboard only"),
    }
}

fn start(path: &str) {
    let result =
        unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(path), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    let code = result.0 as isize;
    let file = Path::new(path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    say!("start: {file}: {}", if code > 32 { "started".to_string() } else { format!("refused ({code})") });
}

/// The help bar along the bottom, and the pad keyboard above it when it is open.
fn paint(hdc: HDC, rect: RECT, ui: &Ui, bar: i32, keys_height: i32, kind: Keyboard) {
    let width = rect.right - rect.left;
    let bottom = rect.bottom;
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        let font = CreateFontW(
            bar * 9 / 20,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI"),
        );
        let old = SelectObject(hdc, font.into());
        let band = RECT { left: 0, top: bottom - bar, right: width, bottom };
        fill(hdc, band, 0x202020);
        let help = if ui.keyboard {
            format!(
                "Keyboard [{}]   D-pad move   A type   X delete   Y space   LB layer   View Tab   Menu Enter   B close",
                LAYER_NAMES[ui.board.layer]
            )
        } else {
            let y = match kind {
                Keyboard::Pad => "Y keyboard",
                Keyboard::TabTip => "Y touch keyboard",
                Keyboard::Osk => "Y on-screen keyboard",
                Keyboard::None => "",
            };
            format!("D-pad Tab / Shift+Tab, arrows   A Enter   B Esc   X Space   View Backspace   {y}   L-stick pointer   RT click   R-stick scroll")
        };
        text(hdc, &help, band, 0xE0E0E0);
        if ui.keyboard {
            let top = bottom - bar - keys_height;
            fill(hdc, RECT { left: 0, top, right: width, bottom: bottom - bar }, 0x101010);
            let rows = ui.board.rows();
            let key_w = width * 6 / 10 / 10;
            let key_h = keys_height / (rows.len() as i32 + 1);
            let left = (width - key_w * 10) / 2;
            let gap = key_h / 10;
            for (r, row) in rows.iter().enumerate() {
                for (c, ch) in row.iter().enumerate() {
                    let x = left + c as i32 * key_w;
                    let y = top + key_h / 2 + r as i32 * key_h;
                    let key = RECT { left: x + gap, top: y + gap, right: x + key_w - gap, bottom: y + key_h - gap };
                    let focused = r == ui.board.row && c == ui.board.col;
                    // The Dashboard's focus green, #2CA243 (COLORREF is BGR).
                    fill(hdc, key, if focused { 0x43A22C } else { 0x3A3A3A });
                    text(hdc, &ch.to_string(), key, 0xFFFFFF);
                }
            }
        }
        SelectObject(hdc, old);
        let _ = DeleteObject(font.into());
    }
}

fn fill(hdc: HDC, rect: RECT, bgr: u32) {
    unsafe {
        let brush = CreateSolidBrush(COLORREF(bgr));
        FillRect(hdc, &rect, brush);
        let _ = DeleteObject(brush.into());
    }
}

fn text(hdc: HDC, s: &str, mut rect: RECT, bgr: u32) {
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    unsafe {
        SetTextColor(hdc, COLORREF(bgr));
        DrawTextW(hdc, &mut wide, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
    }
}
