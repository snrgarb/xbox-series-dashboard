//! The web view: WebView2Loader.dll loaded at run time, a window of our own that is hidden but for the sign-in page,
//! the environment on the Profile's session folder, its controller and web view, and the calls the session makes:
//! navigate, ExecuteScript, DevTools methods. All on the one thread that made the window (WebView2's rule). As the
//! spike (#196) found it worked on the TV PC; docs/research/discord-web-spike.md, 8.
use std::cell::Cell;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR, CoreWebView2EnvironmentOptions,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    ExecuteScriptCompletedHandler, NewWindowRequestedEventHandler, PermissionRequestedEventHandler,
    ProcessFailedEventHandler, SourceChangedEventHandler,
};
use windows::core::{s, w, Interface, HRESULT, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::{redact, say, Area, WebKey};

type CreateEnvironment = unsafe extern "system" fn(PCWSTR, PCWSTR, *mut c_void, *mut c_void) -> HRESULT;
type RuntimeVersion = unsafe extern "system" fn(PCWSTR, *mut PWSTR) -> HRESULT;

/// WebView2Loader.dll, from beside dashboard.exe. Loaded by hand rather than linked, so the Dashboard starts, and
/// says there is no runtime, on a PC without it.
pub struct Loader {
    module: HMODULE,
    create: CreateEnvironment,
    version: RuntimeVersion,
    pub path: PathBuf,
}

// The loader's two functions may be called from any thread.
unsafe impl Send for Loader {}

/// Each Loader is one LoadLibrary, let go with it: nothing is left loaded by an attempt that failed.
impl Drop for Loader {
    fn drop(&mut self) {
        let _ = unsafe { FreeLibrary(self.module) };
    }
}

impl Loader {
    pub fn load() -> Result<Loader, String> {
        let exe = std::env::current_exe().map_err(|e| format!("where is dashboard.exe? {e}"))?;
        let dir = exe.parent().unwrap_or(Path::new("."));
        let path = dir.join("WebView2Loader.dll");
        if !path.is_file() {
            return Err(format!("WebView2Loader.dll is not beside the exe, in {}", dir.display()));
        }
        unsafe {
            let module = LoadLibraryW(&HSTRING::from(path.as_path())).map_err(|e| format!("LoadLibrary: {e}"))?;
            let create = GetProcAddress(module, s!("CreateCoreWebView2EnvironmentWithOptions"))
                .ok_or("WebView2Loader.dll has no CreateCoreWebView2EnvironmentWithOptions")?;
            let version = GetProcAddress(module, s!("GetAvailableCoreWebView2BrowserVersionString"))
                .ok_or("WebView2Loader.dll has no GetAvailableCoreWebView2BrowserVersionString")?;
            Ok(Loader {
                module,
                create: std::mem::transmute::<unsafe extern "system" fn() -> isize, CreateEnvironment>(create),
                version: std::mem::transmute::<unsafe extern "system" fn() -> isize, RuntimeVersion>(version),
                path,
            })
        }
    }

    /// The Evergreen runtime the loader finds; `Err` with none installed.
    pub fn runtime_version(&self) -> Result<String, String> {
        let mut version = PWSTR::null();
        let hr = unsafe { (self.version)(PCWSTR::null(), &mut version) };
        if version.is_null() {
            return Err(if hr.is_err() { format!("{hr:?} {}", hr.message()) } else { "no runtime found".into() });
        }
        Ok(CoTaskMemPWSTR::from(version).to_string())
    }
}

thread_local! {
    /// The window was asked to close (Alt+F4 while it had the foreground).
    static CLOSED: Cell<bool> = const { Cell::new(false) };
}

pub fn closed() -> bool {
    CLOSED.with(Cell::get)
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            FillRect(hdc, &rect, HBRUSH(GetStockObject(BLACK_BRUSH).0));
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CLOSE => {
            CLOSED.with(|c| c.set(true));
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// The hidden window's size: Discord lays itself out at 1600 x 900 CSS pixels.
const HIDDEN: (i32, i32) = (1600, 900);

/// A window of its own, never activated (the Dashboard keeps the foreground and the pad), on top of the Dashboard's
/// while shown, and never shown otherwise.
fn window() -> Result<HWND, String> {
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandle: {e}"))?;
        let class = w!("DashboardDiscordWeb");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class,
            w!("Discord"),
            WS_POPUP | WS_CLIPCHILDREN,
            0,
            0,
            HIDDEN.0,
            HIDDEN.1,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("CreateWindowEx: {e}"))
    }
}

pub struct Host {
    hwnd: HWND,
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
}

impl Host {
    /// The web view in a window of its own. On any failure the window is destroyed before the error comes back:
    /// an attempt that fails leaves nothing behind.
    pub fn open(loader: &Loader, data: &Path) -> Result<Host, String> {
        let hwnd = window()?;
        Self::open_in(hwnd, loader, data).inspect_err(|_| unsafe {
            let _ = DestroyWindow(hwnd);
        })
    }

    fn open_in(hwnd: HWND, loader: &Loader, data: &Path) -> Result<Host, String> {
        std::fs::create_dir_all(data).map_err(|e| format!("cannot make the session's folder: {e}"))?;
        let started = Instant::now();
        let options: ICoreWebView2EnvironmentOptions = CoreWebView2EnvironmentOptions::default().into();
        let folder = HSTRING::from(data);
        let create = loader.create;
        let (tx, rx) = mpsc::channel();
        CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                create(PCWSTR::null(), PCWSTR(folder.as_ptr()), options.as_raw(), handler.as_raw())
                    .ok()
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |result, env| {
                let _ = tx.send(result.and_then(|()| env.ok_or_else(|| E_POINTER.into())));
                Ok(())
            }),
        )
        .map_err(|e| format!("environment: {e:?}"))?;
        let env: ICoreWebView2Environment =
            rx.recv().map_err(|e| format!("environment: {e}"))?.map_err(|e| format!("environment: {e}"))?;

        let (tx, rx) = mpsc::channel();
        CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                env.CreateCoreWebView2Controller(hwnd, &handler).map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |result, controller| {
                let _ = tx.send(result.and_then(|()| controller.ok_or_else(|| E_POINTER.into())));
                Ok(())
            }),
        )
        .map_err(|e| format!("controller: {e:?}"))?;
        let controller: ICoreWebView2Controller =
            rx.recv().map_err(|e| format!("controller: {e}"))?.map_err(|e| format!("controller: {e}"))?;
        let webview = unsafe {
            controller
                .SetBounds(RECT { left: 0, top: 0, right: HIDDEN.0, bottom: HIDDEN.1 })
                .map_err(|e| format!("SetBounds: {e}"))?;
            controller.SetIsVisible(false).map_err(|e| format!("SetIsVisible: {e}"))?;
            controller.CoreWebView2().map_err(|e| format!("CoreWebView2: {e}"))?
        };
        say(&format!("discord web: the web view is open, hidden, in {} ms", started.elapsed().as_millis()));
        let host = Host { hwnd, controller, webview };
        host.settings();
        host.events();
        Ok(host)
    }

    /// No password saved by WebView2, no autofill, no DevTools window, no context menu, no zoom by keys.
    fn settings(&self) {
        unsafe {
            let Ok(settings) = self.webview.Settings() else { return };
            let _ = settings.SetAreDevToolsEnabled(false);
            let _ = settings.SetAreDefaultContextMenusEnabled(false);
            let _ = settings.SetIsStatusBarEnabled(false);
            let _ = settings.SetIsZoomControlEnabled(false);
            if let Ok(s4) = settings.cast::<ICoreWebView2Settings4>() {
                let _ = s4.SetIsPasswordAutosaveEnabled(false);
                let _ = s4.SetIsGeneralAutofillEnabled(false);
            }
        }
    }

    /// Where the page went (scheme, host and path only), new windows refused, permissions refused, a crash.
    fn events(&self) {
        let mut token = 0i64;
        unsafe {
            let last = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
            let _ = self.webview.add_SourceChanged(
                &SourceChangedEventHandler::create(Box::new(move |sender, _| {
                    if let Some(sender) = sender {
                        let now = address(|p| sender.Source(p));
                        if *last.borrow() != now {
                            say(&format!("discord web: route {now}"));
                            *last.borrow_mut() = now;
                        }
                    }
                    Ok(())
                })),
                &mut token,
            );
            let _ = self.webview.add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        let _ = args.SetHandled(true);
                        say(&format!("discord web: a new window refused: {}", address(|p| args.Uri(p))));
                    }
                    Ok(())
                })),
                &mut token,
            );
            let _ = self.webview.add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                        let _ = args.PermissionKind(&mut kind);
                        let _ = args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY);
                        say(&format!("discord web: a permission asked and refused: kind {}", kind.0));
                    }
                    Ok(())
                })),
                &mut token,
            );
            let _ = self.webview.add_ProcessFailed(
                &ProcessFailedEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                        let _ = args.ProcessFailedKind(&mut kind);
                        say(&format!("discord web: a browser process failed: kind {}", kind.0));
                    }
                    Ok(())
                })),
                &mut token,
            );
        }
    }

    pub fn navigate(&self, url: &str) {
        say(&format!("discord web: navigate {}", redact::address(url)));
        if let Err(e) = unsafe { self.webview.Navigate(&HSTRING::from(url)) } {
            say(&format!("discord web: navigate refused: {e}"));
        }
    }

    /// Over `area` of the screen, on top, never activated.
    pub fn show(&self, area: Area) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                area.x,
                area.y,
                area.width,
                area.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = self.controller.SetBounds(RECT { left: 0, top: 0, right: area.width, bottom: area.height });
            let _ = self.controller.SetIsVisible(true);
        }
        say(&format!(
            "discord web: the sign-in page is shown, {}x{} at {},{}",
            area.width, area.height, area.x, area.y
        ));
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            let _ = self.controller.SetIsVisible(false);
        }
        say("discord web: the sign-in page is put away");
    }

    /// ExecuteScript: the script's value, as JSON.
    fn eval(&self, js: &str) -> Result<Value, String> {
        let webview = self.webview.clone();
        let js = HSTRING::from(js);
        let (tx, rx) = mpsc::channel();
        ExecuteScriptCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                webview.ExecuteScript(&js, &handler).map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |result, json| {
                let _ = tx.send(result.map(|()| json));
                Ok(())
            }),
        )
        .map_err(|e| format!("ExecuteScript: {e:?}"))?;
        let json = rx.recv().map_err(|e| e.to_string())?.map_err(|e| format!("ExecuteScript: {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("ExecuteScript's answer: {e}"))
    }

    /// A DevTools protocol method, not waited for (input events, sent in order). A failure is logged by name only:
    /// the parameters may hold what was typed.
    pub fn cdp_send(&self, method: &'static str, params: Value) {
        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, _| {
            if let Err(e) = result {
                say(&format!("discord web: {method} failed: {e}"));
            }
            Ok(())
        }));
        let (name, params) = (HSTRING::from(method), HSTRING::from(params.to_string()));
        if let Err(e) = unsafe { self.webview.CallDevToolsProtocolMethod(&name, &params, &handler) } {
            say(&format!("discord web: {method} refused: {e}"));
        }
    }

    /// How the page stands (`js/state.js`): flags only.
    pub fn state(&self) -> Result<Value, String> {
        self.eval(STATE)
    }

    /// A key pressed and let go, by DevTools' `Input.dispatchKeyEvent`: trusted input, whether or not the window has
    /// the foreground. Enter is the full sequence the spike found a form needs: rawKeyDown, char, keyUp.
    pub fn key(&self, key: WebKey) {
        let (name, code, vk, text) = parts(key);
        let modifiers = if key == WebKey::ShiftTab { 8 } else { 0 };
        let base = json!({ "key": name, "code": code, "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk, "modifiers": modifiers });
        let with = |kind: &str| {
            let mut event = base.clone();
            event["type"] = json!(kind);
            event
        };
        self.cdp_send("Input.dispatchKeyEvent", with("rawKeyDown"));
        if let Some(text) = text {
            let mut event = with("char");
            event["text"] = json!(text);
            event["unmodifiedText"] = json!(text);
            self.cdp_send("Input.dispatchKeyEvent", event);
        }
        self.cdp_send("Input.dispatchKeyEvent", with("keyUp"));
    }

    /// What the Console keyboard kept, in place of the focused field's text, as typing would put it there. Never
    /// logged, never kept: it goes to the page and is dropped.
    pub fn insert(&self, text: &str) {
        self.cdp_send(
            "Runtime.evaluate",
            json!({ "expression": "{ const a = document.activeElement; if (a && a.select) a.select(); }" }),
        );
        if text.is_empty() {
            self.key(WebKey::Backspace);
        } else {
            self.cdp_send("Input.insertText", json!({ "text": text }));
        }
    }

    pub fn close(self) {
        unsafe {
            let _ = self.controller.Close();
            let _ = DestroyWindow(self.hwnd);
        }
        pump(Duration::from_millis(500));
    }
}

const STATE: &str = include_str!("../../js/state.js");

fn parts(key: WebKey) -> (&'static str, &'static str, i32, Option<&'static str>) {
    match key {
        WebKey::Tab | WebKey::ShiftTab => ("Tab", "Tab", 9, None),
        WebKey::Enter => ("Enter", "Enter", 13, Some("\r")),
        WebKey::Escape => ("Escape", "Escape", 27, None),
        WebKey::Space => (" ", "Space", 32, Some(" ")),
        WebKey::Backspace => ("Backspace", "Backspace", 8, None),
        WebKey::Left => ("ArrowLeft", "ArrowLeft", 37, None),
        WebKey::Up => ("ArrowUp", "ArrowUp", 38, None),
        WebKey::Right => ("ArrowRight", "ArrowRight", 39, None),
        WebKey::Down => ("ArrowDown", "ArrowDown", 40, None),
    }
}

fn address(get: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>) -> String {
    let mut uri = PWSTR::null();
    match get(&mut uri) {
        Ok(()) if !uri.is_null() => redact::address(&CoTaskMemPWSTR::from(uri).to_string()),
        _ => "(unknown)".into(),
    }
}

/// Runs this thread's messages for up to `duration`, back as soon as a message comes (a command's poke among them):
/// WebView2's callbacks and events come through them.
pub fn pump(duration: Duration) {
    let end = Instant::now() + duration;
    let mut msg = MSG::default();
    loop {
        let mut any = false;
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                any = true;
                if msg.hwnd.is_invalid() && msg.message == WM_APP {
                    continue;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let now = Instant::now();
        if any || now >= end {
            return;
        }
        let wait = (end - now).as_millis().min(u128::from(INFINITE - 1)) as u32;
        unsafe { MsgWaitForMultipleObjects(None, false, wait, QS_ALLINPUT) };
    }
}
