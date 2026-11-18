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
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR,
    CoreWebView2EnvironmentOptions, CreateCoreWebView2ControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, ExecuteScriptCompletedHandler, NewWindowRequestedEventHandler,
    NotificationReceivedEventHandler, PermissionRequestedEventHandler, ProcessFailedEventHandler,
    SetPermissionStateCompletedHandler, SourceChangedEventHandler, WebMessageReceivedEventHandler,
};
use windows::core::{s, w, Interface, HRESULT, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::act::ActOp;
use crate::{redact, say, Area, Notified, PageOp, Report, Tell, WebKey};

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
    pub fn open(loader: &Loader, data: &Path, base: &str) -> Result<Host, String> {
        let hwnd = window()?;
        Self::open_in(hwnd, loader, data, base).inspect_err(|_| unsafe {
            let _ = DestroyWindow(hwnd);
        })
    }

    fn open_in(hwnd: HWND, loader: &Loader, data: &Path, base: &str) -> Result<Host, String> {
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
        host.events(&authority(base));
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

    /// Where the page went (scheme, host and path only), new windows refused, permissions refused (notifications
    /// alone are granted, to the page's own address: `reading` takes them), a crash.
    fn events(&self, authority: &str) {
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
            let authority = authority.to_string();
            // Notifications are granted only where this runtime lets `reading` take them (ICoreWebView2_24).
            let taken = self.webview.cast::<ICoreWebView2_24>().is_ok();
            let _ = self.webview.add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                        let _ = args.PermissionKind(&mut kind);
                        // Discord's own address only: the microphone (and autoplay) for voice, notifications for
                        // `reading`; the camera (no video) and every other permission are refused
                        // (`crate::permission_granted`).
                        let from = authority_of(|p| args.Uri(p));
                        let grant = crate::permission_granted(kind.0, &from, &authority, taken);
                        let state = if grant {
                            COREWEBVIEW2_PERMISSION_STATE_ALLOW
                        } else {
                            COREWEBVIEW2_PERMISSION_STATE_DENY
                        };
                        let _ = args.SetState(state);
                        say(&format!(
                            "discord web: permission: {} {}",
                            crate::permission_name(kind.0),
                            if grant { "granted" } else { "refused" }
                        ));
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

    /// Over `area` of the screen, on top, never activated; the page zoomed so Discord's QR code is large and its QR
    /// panel kept (`crate::zoom`). Only the zoom is changed: nothing of Discord's page is cropped or restyled.
    pub fn show(&self, area: Area) {
        unsafe {
            let mut raster = 1.0f64;
            if let Ok(c3) = self.controller.cast::<ICoreWebView2Controller3>() {
                let _ = c3.RasterizationScale(&mut raster);
            }
            let zoom = crate::zoom(area, raster);
            let _ = self.controller.SetZoomFactor(zoom);
            say(&format!(
                "discord web: page zoom {zoom:.2} at scale {raster:.2}, {:.0}x{:.0} CSS pixels",
                area.width as f64 / (zoom * raster),
                area.height as f64 / (zoom * raster)
            ));
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
            // Hidden, the page is looked at as Discord lays it out unzoomed.
            let _ = self.controller.SetZoomFactor(1.0);
        }
        say("discord web: the sign-in page is put away");
    }

    /// Reading Discord's Direct Messages (docs/discord-web.md, "Reading"): the page script (`crate::direct_script`)
    /// installed to run in every new document before the page's own, what it posts taken as `Report::Page` (only from
    /// the page's own address), and the page's web notifications taken and marked handled, so Windows shows none.
    /// Called before the first navigation. A failure is logged and the web view goes on without it: the state then
    /// stays Loading, and the health says nothing.
    pub fn reading(&self, tell: Tell, base: &str) {
        let authority = authority(base);
        match self.add_script(&crate::direct_script()) {
            Ok(()) => say("discord web: the page script is installed"),
            Err(e) => say(&format!("discord web: the page script could not be installed: {e}")),
        }
        let mut token = 0i64;
        unsafe {
            let page = tell.clone();
            let added = self.webview.add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    if authority_of(|p| args.Source(p)) != authority {
                        return Ok(());
                    }
                    let mut json = PWSTR::null();
                    if args.WebMessageAsJson(&mut json).is_ok() && !json.is_null() {
                        page(Report::Page(CoTaskMemPWSTR::from(json).to_string()));
                    }
                    Ok(())
                })),
                &mut token,
            );
            if let Err(e) = added {
                say(&format!("discord web: WebMessageReceived refused: {e}"));
            }
            let w24 = self.webview.cast::<ICoreWebView2_24>();
            self.allow_ahead(base, w24.is_ok());
            let Ok(w24) = w24 else {
                say("discord web: this runtime has no NotificationReceived (ICoreWebView2_24): notifications stay refused");
                return;
            };
            let added = w24.add_NotificationReceived(
                &NotificationReceivedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let _ = args.SetHandled(true);
                    if let Ok(note) = args.Notification() {
                        let text = |get: &dyn Fn(*mut PWSTR) -> windows::core::Result<()>| {
                            let mut p = PWSTR::null();
                            match get(&mut p) {
                                Ok(()) if !p.is_null() => CoTaskMemPWSTR::from(p).to_string(),
                                _ => String::new(),
                            }
                        };
                        tell(Report::Notified(Notified {
                            title: text(&|p| note.Title(p)),
                            body: text(&|p| note.Body(p)),
                            tag: text(&|p| note.Tag(p)),
                        }));
                    }
                    Ok(())
                })),
                &mut token,
            );
            if let Err(e) = added {
                say(&format!("discord web: NotificationReceived refused: {e}"));
            }
        }
    }

    /// Allowed ahead for Discord's origin, so the page never waits on a prompt: notifications, the microphone and
    /// autoplay (voice); nothing else, and never the camera.
    fn allow_ahead(&self, base: &str, notifications: bool) {
        let profile = unsafe { self.webview.cast::<ICoreWebView2_13>().and_then(|w| w.Profile()) };
        let Ok(profile) = profile.and_then(|p| p.cast::<ICoreWebView2Profile4>()) else {
            say("discord web: this runtime cannot allow permissions ahead: only as asked");
            return;
        };
        let origin = HSTRING::from(origin(base));
        for (kind, name) in [
            (COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS, "notifications"),
            (COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, "microphone"),
            (COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY, "autoplay"),
        ] {
            // Notifications only where this runtime lets `reading` take them: else Windows would show them.
            if kind == COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS && !notifications {
                continue;
            }
            let handler = SetPermissionStateCompletedHandler::create(Box::new(move |result| {
                if let Err(e) = result {
                    say(&format!("discord web: {name} could not be allowed ahead: {e}"));
                }
                Ok(())
            }));
            if let Err(e) =
                unsafe { profile.SetPermissionState(kind, &origin, COREWEBVIEW2_PERMISSION_STATE_ALLOW, &handler) }
            {
                say(&format!("discord web: {name} refused ahead: {e}"));
            }
        }
    }

    /// Something asked of the page script (`window.__xsdDirect`). An id that is not digits is never put in a script.
    pub fn page(&self, op: &PageOp) {
        let (name, js) = match op {
            PageOp::Open(id) if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) => {
                ("open", format!("window.__xsdDirect ? window.__xsdDirect.open(\"{id}\") : false"))
            }
            PageOp::Open(_) => return,
            PageOp::Close => ("close", "window.__xsdDirect ? window.__xsdDirect.close() : false".to_string()),
            PageOp::Older => ("older", "window.__xsdDirect ? window.__xsdDirect.older() : false".to_string()),
            PageOp::Resend => ("resend", "window.__xsdDirect ? window.__xsdDirect.resend() : false".to_string()),
            PageOp::Browse(Some(id)) if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) => {
                ("browse", format!("window.__xsdDirect ? window.__xsdDirect.browse(\"{id}\") : false"))
            }
            PageOp::Browse(Some(_)) => return,
            PageOp::Browse(None) => {
                ("browse", "window.__xsdDirect ? window.__xsdDirect.browse(null) : false".to_string())
            }
        };
        match self.eval(&js) {
            Ok(Value::Bool(true)) => {}
            Ok(_) => say(&format!("discord web: the page script did not take {name}")),
            Err(e) => say(&format!("discord web: the page script's {name} failed: {e}")),
        }
    }

    /// An action for the page script (`js/act.js`; docs/discord-web.md, "Acting"). False when the page did not take it:
    /// the page script is not there, it is busy, or the ask is not well formed. Nothing of the ask is logged but its kind.
    pub fn act(&self, op: &ActOp) -> bool {
        let Some(js) = op.script() else {
            say(&format!("discord web: {} was not asked of the page: not well formed", op.kind()));
            return false;
        };
        match self.eval(&js) {
            Ok(Value::Bool(true)) => true,
            Ok(_) => {
                say(&format!("discord web: the page script did not take {}", op.kind()));
                false
            }
            Err(e) => {
                say(&format!("discord web: the page script's {} failed: {e}", op.kind()));
                false
            }
        }
    }

    /// WebView2's own `AddScriptToExecuteOnDocumentCreated`: the script runs in every new document of the main world
    /// before the page's scripts. Waited for.
    fn add_script(&self, js: &str) -> Result<(), String> {
        let webview = self.webview.clone();
        let js = HSTRING::from(js);
        let (tx, rx) = mpsc::channel();
        AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                webview.AddScriptToExecuteOnDocumentCreated(&js, &handler).map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |result, _id| {
                let _ = tx.send(result);
                Ok(())
            }),
        )
        .map_err(|e| format!("AddScriptToExecuteOnDocumentCreated: {e:?}"))?;
        rx.recv().map_err(|e| e.to_string())?.map_err(|e| format!("AddScriptToExecuteOnDocumentCreated: {e}"))
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
        self.key_with(key, if key == WebKey::ShiftTab { 8 } else { 0 });
    }

    /// A key with modifiers (Alt 1, Ctrl 2, Meta 4, Shift 8: DevTools' bit set).
    fn key_with(&self, key: WebKey, modifiers: i32) {
        let (name, code, vk, text) = parts(key);
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

    /// A message for the page's focused message box, as typing would put it there: emptied of a draft first if `clear`
    /// (select all, delete), then each line inserted, a line break by Shift+Enter (a plain Enter would send). Enter
    /// itself is pressed later, by [`Host::enter`], once the box has had a moment to take the text (the spike's lesson).
    /// Never logged, never kept.
    pub fn type_message(&self, text: &str, clear: bool) {
        if clear {
            let ctrl = json!({ "key": "a", "code": "KeyA", "windowsVirtualKeyCode": 65, "nativeVirtualKeyCode": 65, "modifiers": 2 });
            for kind in ["rawKeyDown", "keyUp"] {
                let mut event = ctrl.clone();
                event["type"] = json!(kind);
                self.cdp_send("Input.dispatchKeyEvent", event);
            }
            self.key(WebKey::Backspace);
        }
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                self.key_with(WebKey::Enter, 8);
            }
            let line = line.trim_end_matches('\r');
            if !line.is_empty() {
                self.cdp_send("Input.insertText", json!({ "text": line }));
            }
        }
    }

    /// The one Enter that sends: rawKeyDown, char, keyUp.
    pub fn enter(&self) {
        self.key(WebKey::Enter);
    }

    /// Takes focus off the field the page put it in by itself (Discord focuses the e-mail field), so the sign-in page
    /// opens on the QR code with no field waiting for the keyboard.
    pub fn unfocus(&self) {
        self.cdp_send(
            "Runtime.evaluate",
            json!({ "expression": "{ const a = document.activeElement; if (a && a !== document.body && a.blur) a.blur(); }" }),
        );
    }

    /// The pad's first step into the form, after `unfocus`: the form's first field, where Tab would have begun.
    pub fn first_field(&self) {
        self.cdp_send(
            "Runtime.evaluate",
            json!({ "expression": "{ const f = document.querySelector('form input:not([type=hidden]), input:not([type=hidden])'); if (f) f.focus(); }" }),
        );
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

/// `host:port` of an address, lower case and without a user: what two addresses of one site share.
fn authority(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    rest[..end].rsplit('@').next().unwrap_or("").to_ascii_lowercase()
}

/// The address's origin (`https://discord.com`).
fn origin(url: &str) -> String {
    let scheme = url.split_once("://").map_or("https", |(scheme, _)| scheme);
    format!("{scheme}://{}", authority(url))
}

fn authority_of(get: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>) -> String {
    let mut uri = PWSTR::null();
    match get(&mut uri) {
        Ok(()) if !uri.is_null() => authority(&CoTaskMemPWSTR::from(uri).to_string()),
        _ => String::new(),
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
