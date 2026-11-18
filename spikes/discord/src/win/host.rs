//! The web view host: WebView2Loader.dll loaded at run time, one window of our own (full screen, or hidden one of two
//! ways), the environment on `--data`, its controller and web view, the events logged, and the calls the steps make:
//! navigate, ExecuteScript, DevTools methods. Everything here runs on the one thread that made the window (WebView2's
//! rule); the waits pump that thread's messages.
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR,
    CoreWebView2EnvironmentOptions, CreateCoreWebView2ControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, DevToolsProtocolEventReceivedEventHandler,
    ExecuteScriptCompletedHandler, NavigationCompletedEventHandler, NavigationStartingEventHandler,
    NewWindowRequestedEventHandler, NotificationReceivedEventHandler, PermissionRequestedEventHandler,
    ProcessFailedEventHandler, SetPermissionStateCompletedHandler, SourceChangedEventHandler,
};
use windows::core::{s, w, Interface, BOOL, HRESULT, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::args::Hide;
use crate::{redact, say};

type CreateEnvironment = unsafe extern "system" fn(PCWSTR, PCWSTR, *mut c_void, *mut c_void) -> HRESULT;
type RuntimeVersion = unsafe extern "system" fn(PCWSTR, *mut PWSTR) -> HRESULT;

/// WebView2Loader.dll, from beside the exe. Loaded by hand rather than linked, so the exe starts (and says what is
/// missing) on a PC without it.
pub struct Loader {
    create: CreateEnvironment,
    version: RuntimeVersion,
    pub path: PathBuf,
}

impl Loader {
    pub fn load() -> Result<Loader, String> {
        let exe = std::env::current_exe().map_err(|e| format!("where is this exe? {e}"))?;
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
                create: std::mem::transmute::<unsafe extern "system" fn() -> isize, CreateEnvironment>(create),
                version: std::mem::transmute::<unsafe extern "system" fn() -> isize, RuntimeVersion>(version),
                path,
            })
        }
    }

    /// The Evergreen runtime the loader finds (no browser folder given, so never a fixed version).
    pub fn runtime_version(&self) -> Result<String, String> {
        let mut version = PWSTR::null();
        let hr = unsafe { (self.version)(PCWSTR::null(), &mut version) };
        if version.is_null() {
            return Err(if hr.is_err() { format!("{hr:?} {}", hr.message()) } else { "no runtime found".into() });
        }
        Ok(CoTaskMemPWSTR::from(version).to_string())
    }
}

pub enum Show {
    /// Over the whole primary screen, topmost, given the foreground if Windows lets it.
    Full,
    Hidden(Hide),
}

thread_local! {
    /// What the window draws round the web view (the sign-in step's help bar and keyboard).
    static PAINTER: RefCell<Option<Painter>> = const { RefCell::new(None) };
    /// The window was asked to close (Alt+F4).
    pub static CLOSED: Cell<bool> = const { Cell::new(false) };
    /// The voice steps: the microphone (and autoplay) is granted to the page; otherwise refused.
    pub static GRANT_MICROPHONE: Cell<bool> = const { Cell::new(false) };
    /// Discord's web notifications taken this run.
    pub static NOTIFICATIONS: Cell<u32> = const { Cell::new(0) };
}

/// The voice steps call this before `Host::open`: the microphone is granted when the page asks.
pub fn grant_microphone() {
    GRANT_MICROPHONE.with(|g| g.set(true));
}

pub fn notifications() -> u32 {
    NOTIFICATIONS.with(Cell::get)
}

fn permission_name(kind: COREWEBVIEW2_PERMISSION_KIND) -> &'static str {
    match kind {
        COREWEBVIEW2_PERMISSION_KIND_MICROPHONE => "microphone",
        COREWEBVIEW2_PERMISSION_KIND_CAMERA => "camera",
        COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS => "notifications",
        COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY => "autoplay",
        COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ => "clipboard read",
        COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION => "geolocation",
        COREWEBVIEW2_PERMISSION_KIND_WINDOW_MANAGEMENT => "window management",
        _ => "other",
    }
}

pub type Painter = Box<dyn Fn(HDC, RECT)>;

pub fn set_painter(painter: Painter) {
    PAINTER.with(|p| *p.borrow_mut() = Some(painter));
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            FillRect(hdc, &rect, HBRUSH(GetStockObject(BLACK_BRUSH).0));
            PAINTER.with(|p| {
                if let Ok(p) = p.try_borrow() {
                    if let Some(paint) = p.as_ref() {
                        paint(hdc, rect);
                    }
                }
            });
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

/// A hidden window's size, in pixels, and the web view's scale in it (1, whatever the screen's), so Discord lays
/// itself out at 1600 x 900 CSS pixels: wide enough for the DM list beside a conversation.
const HIDDEN: (i32, i32) = (1600, 900);

fn window(show: &Show) -> Result<(HWND, RECT), String> {
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandle: {e}"))?;
        let class = w!("DiscordSpikeHost");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let screen = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        let tool = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let (ex, style, at, size) = match show {
            Show::Full => (WS_EX_TOPMOST, WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN, (0, 0), screen),
            Show::Hidden(Hide::Controller | Hide::VisibleUnshown) => (tool, WS_POPUP | WS_CLIPCHILDREN, (0, 0), HIDDEN),
            Show::Hidden(Hide::Offscreen) => (tool, WS_POPUP | WS_CLIPCHILDREN, (-20000, -20000), HIDDEN),
        };
        let hwnd = CreateWindowExW(
            ex,
            class,
            w!("discord-spike"),
            style,
            at.0,
            at.1,
            size.0,
            size.1,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("CreateWindowEx: {e}"))?;
        match show {
            Show::Full => {
                let front = SetForegroundWindow(hwnd).as_bool();
                say!("window: full screen {}x{} px, topmost; foreground given: {}", size.0, size.1, yes(front));
            }
            Show::Hidden(Hide::Offscreen) => {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                say!("window: shown off every screen at -20000,-20000, {}x{} px, never activated", size.0, size.1);
            }
            Show::Hidden(Hide::Controller) => {
                say!("window: never shown, {}x{} px; the web view is told it is not visible", size.0, size.1)
            }
            Show::Hidden(Hide::VisibleUnshown) => {
                say!("window: never shown, {}x{} px; the web view is told it IS visible", size.0, size.1)
            }
        }
        Ok((hwnd, RECT { left: 0, top: 0, right: size.0, bottom: size.1 }))
    }
}

pub fn yes(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

pub struct Host {
    pub hwnd: HWND,
    pub area: RECT,
    pub env: ICoreWebView2Environment,
    pub controller: ICoreWebView2Controller,
    pub webview: ICoreWebView2,
}

impl Host {
    pub fn open(loader: &Loader, data: &Path, show: Show, browser_args: Option<&str>) -> Result<Host, String> {
        let hidden = matches!(show, Show::Hidden(_));
        let controller_hidden = matches!(show, Show::Hidden(Hide::Controller));
        let (hwnd, area) = window(&show)?;
        std::fs::create_dir_all(data).map_err(|e| format!("cannot make {}: {e}", data.display()))?;

        let started = Instant::now();
        let options = CoreWebView2EnvironmentOptions::default();
        if let Some(args) = browser_args {
            unsafe { options.set_additional_browser_arguments(args.to_string()) };
            say!("environment: extra browser switches: {args}");
        }
        let options: ICoreWebView2EnvironmentOptions = options.into();
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
        let env = rx.recv().map_err(|e| format!("environment: {e}"))?.map_err(|e| format!("environment: {e}"))?;
        let mut version = PWSTR::null();
        let version = match unsafe { env.BrowserVersionString(&mut version) } {
            Ok(()) => CoTaskMemPWSTR::from(version).to_string(),
            Err(e) => format!("(not known: {e})"),
        };
        say!("environment: made in {} ms, browser {version}, user data folder {}", ms(started), data.display());

        let started = Instant::now();
        let (tx, rx) = mpsc::channel();
        let made_by = env.clone();
        CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                made_by.CreateCoreWebView2Controller(hwnd, &handler).map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |result, controller| {
                let _ = tx.send(result.and_then(|()| controller.ok_or_else(|| E_POINTER.into())));
                Ok(())
            }),
        )
        .map_err(|e| format!("controller: {e:?}"))?;
        let controller = rx.recv().map_err(|e| format!("controller: {e}"))?.map_err(|e| format!("controller: {e}"))?;
        say!("controller: made in {} ms", ms(started));

        let webview = unsafe {
            if hidden {
                if let Ok(c3) = controller.cast::<ICoreWebView2Controller3>() {
                    let _ = c3.SetShouldDetectMonitorScaleChanges(false);
                    let _ = c3.SetRasterizationScale(1.0);
                }
            }
            controller.SetBounds(area).map_err(|e| format!("SetBounds: {e}"))?;
            controller.SetIsVisible(!controller_hidden).map_err(|e| format!("SetIsVisible: {e}"))?;
            if hidden {
                let mut visible = BOOL(0);
                let _ = controller.IsVisible(&mut visible);
                say!("controller: IsVisible reads {}", yes(visible.as_bool()));
            }
            controller.CoreWebView2().map_err(|e| format!("CoreWebView2: {e}"))?
        };
        let host = Host { hwnd, area, env, controller, webview };
        host.settings();
        host.events();
        host.notifications();
        host.preset_permissions();
        Ok(host)
    }

    /// Discord's web notifications (a message, a call), taken by us: counted, their lengths logged, marked handled
    /// so Windows shows nothing. Never their words or who sent them.
    fn notifications(&self) {
        let Ok(w24) = self.webview.cast::<ICoreWebView2_24>() else {
            say!("notifications: this runtime has no NotificationReceived (ICoreWebView2_24): Windows would show them");
            return;
        };
        let mut token = 0i64;
        let added = unsafe {
            w24.add_NotificationReceived(
                &NotificationReceivedEventHandler::create(Box::new(|_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let n = NOTIFICATIONS.with(|c| {
                        c.set(c.get() + 1);
                        c.get()
                    });
                    let _ = args.SetHandled(true);
                    match args.Notification() {
                        Ok(note) => {
                            let length = |get: &dyn Fn(*mut PWSTR) -> windows::core::Result<()>| {
                                let mut p = PWSTR::null();
                                match get(&mut p) {
                                    Ok(()) if !p.is_null() => CoTaskMemPWSTR::from(p).to_string().chars().count(),
                                    _ => 0,
                                }
                            };
                            let title = length(&|p| note.Title(p));
                            let body = length(&|p| note.Body(p));
                            let tag = length(&|p| note.Tag(p));
                            let (mut interaction, mut silent) = (BOOL(0), BOOL(0));
                            let _ = note.RequiresInteraction(&mut interaction);
                            let _ = note.IsSilent(&mut silent);
                            say!(
                                "notification: {n} (title {title} chars, body {body} chars, tag {tag} chars, requires interaction {}, silent {}): handled, Windows shows nothing",
                                yes(interaction.as_bool()),
                                yes(silent.as_bool())
                            );
                        }
                        Err(e) => say!("notification: {n} (unreadable: {e}): handled"),
                    }
                    Ok(())
                })),
                &mut token,
            )
        };
        match added {
            Ok(()) => say!("notifications: NotificationReceived taken: each is counted and marked handled"),
            Err(e) => say!("notifications: NotificationReceived refused: {e}"),
        }
    }

    /// The permissions Discord's page gets without asking, set on the profile for https://discord.com, so it never
    /// waits on a prompt: notifications always (for NotificationReceived to have something to take), the
    /// microphone in the voice steps. Never kept past what the session already keeps.
    fn preset_permissions(&self) {
        let mut kinds = vec![(COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS, "notifications")];
        if GRANT_MICROPHONE.with(Cell::get) {
            kinds.push((COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, "microphone"));
            kinds.push((COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY, "autoplay"));
        }
        let profile = unsafe { self.webview.cast::<ICoreWebView2_13>().and_then(|w| w.Profile()) };
        let Ok(profile) = profile.and_then(|p| p.cast::<ICoreWebView2Profile4>()) else {
            say!("permissions: this runtime cannot set them ahead (no ICoreWebView2Profile4): only as asked");
            return;
        };
        for (kind, name) in kinds {
            let handler = SetPermissionStateCompletedHandler::create(Box::new(move |result| {
                match result {
                    Ok(()) => say!("permissions: {name} allowed ahead for https://discord.com"),
                    Err(e) => say!("permissions: {name} could not be allowed ahead: {e}"),
                }
                Ok(())
            }));
            let origin = HSTRING::from("https://discord.com");
            if let Err(e) =
                unsafe { profile.SetPermissionState(kind, &origin, COREWEBVIEW2_PERMISSION_STATE_ALLOW, &handler) }
            {
                say!("permissions: {name} refused ahead: {e}");
            }
        }
        pump(Duration::from_millis(300));
    }

    /// Whether Chromium says the page is playing sound now (`ICoreWebView2_8::IsDocumentPlayingAudio`).
    pub fn playing_audio(&self) -> Option<bool> {
        let w8 = self.webview.cast::<ICoreWebView2_8>().ok()?;
        let mut playing = BOOL(0);
        unsafe { w8.IsDocumentPlayingAudio(&mut playing) }.ok()?;
        Some(playing.as_bool())
    }

    /// No password saved by WebView2, no autofill, no DevTools window, no context menu, no zoom by keys.
    fn settings(&self) {
        unsafe {
            let Ok(settings) = self.webview.Settings() else { return };
            let _ = settings.SetAreDevToolsEnabled(false);
            let _ = settings.SetAreDefaultContextMenusEnabled(false);
            let _ = settings.SetIsStatusBarEnabled(false);
            let _ = settings.SetIsZoomControlEnabled(false);
            match settings.cast::<ICoreWebView2Settings4>() {
                Ok(s4) => {
                    let _ = s4.SetIsPasswordAutosaveEnabled(false);
                    let _ = s4.SetIsGeneralAutofillEnabled(false);
                    say!("settings: password saving and autofill off");
                }
                Err(e) => say!("settings: password saving could not be turned off ({e})"),
            }
        }
    }

    /// Every event that says where the page went or what went wrong, logged as host and path only.
    fn events(&self) {
        let mut token = 0i64;
        unsafe {
            let _ = self.webview.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        say!("navigation: start {}", address(|p| args.Uri(p)));
                    }
                    Ok(())
                })),
                &mut token,
            );
            let _ = self.webview.add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(|sender, args| {
                    let (Some(sender), Some(args)) = (sender, args) else { return Ok(()) };
                    let mut ok = BOOL(0);
                    let _ = args.IsSuccess(&mut ok);
                    let mut status = 0;
                    if let Ok(args2) = args.cast::<ICoreWebView2NavigationCompletedEventArgs2>() {
                        let _ = args2.HttpStatusCode(&mut status);
                    }
                    let mut error = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                    let _ = args.WebErrorStatus(&mut error);
                    say!(
                        "navigation: done {} ok {} http {status}{}",
                        address(|p| sender.Source(p)),
                        yes(ok.as_bool()),
                        if ok.as_bool() { String::new() } else { format!(" web error {}", error.0) }
                    );
                    Ok(())
                })),
                &mut token,
            );
            let last = Rc::new(RefCell::new(String::new()));
            let _ = self.webview.add_SourceChanged(
                &SourceChangedEventHandler::create(Box::new(move |sender, _| {
                    if let Some(sender) = sender {
                        let now = address(|p| sender.Source(p));
                        if *last.borrow() != now {
                            say!("route: {now}");
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
                        say!("new window refused: {}", address(|p| args.Uri(p)));
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
                        let mut user = BOOL(0);
                        let _ = args.IsUserInitiated(&mut user);
                        // Granted: notifications (so NotificationReceived takes them), and in the voice steps the
                        // microphone and autoplay. Never the camera (D5, no video) or anything else.
                        let grant = kind == COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS
                            || (GRANT_MICROPHONE.with(Cell::get)
                                && (kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                                    || kind == COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY));
                        let state = if grant {
                            COREWEBVIEW2_PERMISSION_STATE_ALLOW
                        } else {
                            COREWEBVIEW2_PERMISSION_STATE_DENY
                        };
                        let _ = args.SetState(state);
                        // Asked again next run, so each run's log shows the request.
                        if let Ok(args3) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() {
                            let _ = args3.SetSavesInProfile(false);
                        }
                        say!(
                            "permission: kind {} ({}) {}, asked by a user gesture {}",
                            kind.0,
                            permission_name(kind),
                            if grant { "granted" } else { "refused" },
                            yes(user.as_bool())
                        );
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
                        say!("process failed: kind {}", kind.0);
                    }
                    Ok(())
                })),
                &mut token,
            );
        }
    }

    pub fn navigate(&self, url: &str) {
        say!("navigate: {}", redact::address(url));
        if let Err(e) = unsafe { self.webview.Navigate(&HSTRING::from(url)) } {
            say!("navigate: refused: {e}");
        }
    }

    pub fn set_bounds(&self, rect: RECT) {
        let _ = unsafe { self.controller.SetBounds(rect) };
    }

    /// Tells the controller it is (or is not) visible; whether it now says so.
    pub fn set_visible(&self, visible: bool) -> bool {
        let mut now = BOOL(0);
        unsafe {
            let _ = self.controller.SetIsVisible(visible);
            let _ = self.controller.IsVisible(&mut now);
        }
        now.as_bool() == visible
    }

    pub fn is_visible(&self) -> bool {
        let mut now = BOOL(0);
        let _ = unsafe { self.controller.IsVisible(&mut now) };
        now.as_bool()
    }

    pub fn focus(&self) {
        let _ = unsafe { self.controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) };
    }

    /// ExecuteScript: the script's value, as JSON.
    pub fn eval(&self, js: &str) -> Result<Value, String> {
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

    /// WebView2's own `AddScriptToExecuteOnDocumentCreated`: the script runs in every new document of the main
    /// world before the page's scripts. Waited for.
    pub fn add_script(&self, js: &str) -> Result<(), String> {
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

    /// A DevTools protocol event: `handler` gets each one's parameters (its domain must be enabled too).
    pub fn on_event(&self, name: &str, handler: Box<dyn Fn(Value)>) -> Result<(), String> {
        unsafe {
            let receiver = self
                .webview
                .GetDevToolsProtocolEventReceiver(&HSTRING::from(name))
                .map_err(|e| format!("{name}: {e}"))?;
            let mut token = 0i64;
            receiver
                .add_DevToolsProtocolEventReceived(
                    &DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                        if let Some(args) = args {
                            let mut p = PWSTR::null();
                            if args.ParameterObjectAsJson(&mut p).is_ok() && !p.is_null() {
                                let json = CoTaskMemPWSTR::from(p).to_string();
                                if let Ok(v) = serde_json::from_str(&json) {
                                    handler(v);
                                }
                            }
                        }
                        Ok(())
                    })),
                    &mut token,
                )
                .map_err(|e| format!("{name}: {e}"))
        }
    }

    /// A DevTools protocol method, waited for: its answer.
    pub fn cdp(&self, method: &str, params: Value) -> Result<Value, String> {
        let webview = self.webview.clone();
        let (name, params) = (HSTRING::from(method), HSTRING::from(params.to_string()));
        let (tx, rx) = mpsc::channel();
        CallDevToolsProtocolMethodCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                webview.CallDevToolsProtocolMethod(&name, &params, &handler).map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |result, json| {
                let _ = tx.send(result.map(|()| json));
                Ok(())
            }),
        )
        .map_err(|e| format!("{method}: {e:?}"))?;
        let json = rx.recv().map_err(|e| e.to_string())?.map_err(|e| format!("{method}: {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("{method}'s answer: {e}"))
    }

    /// A DevTools protocol method, not waited for (input events, sent in order); a failure is logged.
    pub fn cdp_send(&self, method: &'static str, params: Value) {
        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, _| {
            if let Err(e) = result {
                say!("devtools: {method} failed: {e}");
            }
            Ok(())
        }));
        let (name, params) = (HSTRING::from(method), HSTRING::from(params.to_string()));
        if let Err(e) = unsafe { self.webview.CallDevToolsProtocolMethod(&name, &params, &handler) } {
            say!("devtools: {method} refused: {e}");
        }
    }

    /// `Runtime.evaluate`, awaiting a promise: the value it settles to.
    pub fn evaluate(&self, js: &str) -> Result<Value, String> {
        self.evaluate_as(js, false)
    }

    /// `Runtime.evaluate` as if the user had just pressed something (`userGesture`): what an AudioContext's
    /// `resume()` needs under Chromium's autoplay rule.
    pub fn evaluate_as(&self, js: &str, gesture: bool) -> Result<Value, String> {
        let answer = self.cdp(
            "Runtime.evaluate",
            json!({ "expression": js, "awaitPromise": true, "returnByValue": true, "userGesture": gesture }),
        )?;
        if let Some(details) = answer.get("exceptionDetails") {
            let text = details
                .pointer("/exception/description")
                .or_else(|| details.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("an exception");
            return Err(format!("the script threw: {}", text.lines().next().unwrap_or("")));
        }
        Ok(answer.pointer("/result/value").cloned().unwrap_or(Value::Null))
    }

    /// A key pressed and let go, by DevTools' `Input.dispatchKeyEvent`: trusted input, sent to the page whether or
    /// not the window has the foreground.
    pub fn key(&self, key: Key, shift: bool) {
        let (name, code, vk, text) = key.parts();
        let modifiers = if shift { 8 } else { 0 };
        let mut down = json!({
            "type": if text.is_some() { "keyDown" } else { "rawKeyDown" },
            "key": name, "code": code, "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk, "modifiers": modifiers,
        });
        if let Some(text) = text {
            down["text"] = json!(text);
            down["unmodifiedText"] = json!(text);
        }
        self.cdp_send("Input.dispatchKeyEvent", down);
        self.cdp_send(
            "Input.dispatchKeyEvent",
            json!({ "type": "keyUp", "key": name, "code": code, "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk, "modifiers": modifiers }),
        );
    }

    /// A left click at a point of the page, in CSS pixels, by DevTools' `Input.dispatchMouseEvent`.
    pub fn click(&self, x: f64, y: f64) {
        self.cdp_send("Input.dispatchMouseEvent", json!({ "type": "mouseMoved", "x": x, "y": y }));
        for kind in ["mousePressed", "mouseReleased"] {
            self.cdp_send(
                "Input.dispatchMouseEvent",
                json!({ "type": kind, "x": x, "y": y, "button": "left", "buttons": 1, "clickCount": 1 }),
            );
        }
    }

    /// What the page shows now (`js/state.js`).
    pub fn state(&self) -> Result<Value, String> {
        self.eval(STATE)
    }

    /// Waits for Discord's app page, loaded and signed in; `Err` with the step's exit code if the login page shows
    /// or nothing comes in time.
    pub fn wait_for_app(&self, limit: Duration) -> Result<Value, i32> {
        let started = Instant::now();
        while started.elapsed() < limit {
            pump(Duration::from_millis(1000));
            let Ok(state) = self.state() else { continue };
            if state["app"].as_bool() == Some(true) {
                say!("page: the app is shown after {} ms", ms(started));
                return Ok(state);
            }
            if state["login_form"].as_bool() == Some(true) || state["route"].as_str() == Some("login") {
                say!("page: Discord shows its login page: this session is not signed in (run --sign-in)");
                return Err(3);
            }
        }
        say!("page: the app did not show in {} s", limit.as_secs());
        Err(8)
    }

    pub fn close(self) {
        unsafe {
            let _ = self.controller.Close();
            let _ = DestroyWindow(self.hwnd);
        }
        pump(Duration::from_millis(500));
    }
}

pub const STATE: &str = include_str!("../js/state.js");

#[derive(Clone, Copy)]
pub enum Key {
    Tab,
    Enter,
    Escape,
    Space,
    Backspace,
    Left,
    Right,
    Up,
    Down,
}

impl Key {
    fn parts(self) -> (&'static str, &'static str, i32, Option<&'static str>) {
        match self {
            Key::Tab => ("Tab", "Tab", 9, None),
            Key::Enter => ("Enter", "Enter", 13, Some("\r")),
            Key::Escape => ("Escape", "Escape", 27, None),
            Key::Space => (" ", "Space", 32, Some(" ")),
            Key::Backspace => ("Backspace", "Backspace", 8, None),
            Key::Left => ("ArrowLeft", "ArrowLeft", 37, None),
            Key::Up => ("ArrowUp", "ArrowUp", 38, None),
            Key::Right => ("ArrowRight", "ArrowRight", 39, None),
            Key::Down => ("ArrowDown", "ArrowDown", 40, None),
        }
    }
}

fn address(get: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>) -> String {
    let mut uri = PWSTR::null();
    match get(&mut uri) {
        Ok(()) if !uri.is_null() => redact::address(&CoTaskMemPWSTR::from(uri).to_string()),
        _ => "(unknown)".into(),
    }
}

pub fn ms(since: Instant) -> u128 {
    since.elapsed().as_millis()
}

/// Runs this thread's messages for a while: WebView2's callbacks and events come through them.
pub fn pump(duration: Duration) {
    let end = Instant::now() + duration;
    let mut msg = MSG::default();
    loop {
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let now = Instant::now();
        if now >= end {
            return;
        }
        let wait = (end - now).as_millis().min(u128::from(INFINITE - 1)) as u32;
        unsafe { MsgWaitForMultipleObjects(None, false, wait, QS_ALLINPUT) };
    }
}
