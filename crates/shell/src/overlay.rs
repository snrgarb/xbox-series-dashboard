//! The Overlay: a window above every other, a running game's included, that the Dashboard draws on with alpha, so
//! the game shows wherever nothing is drawn. It is a raw Win32 window, not winit's, because the style that lets it be
//! see-through can only be given when a window is made; its picture is a swap chain made for composition, on a
//! DirectComposition visual. docs/research/overlay-window.md has what was measured and why each style is there.
//!
//! It is shown in one of two ways. Passive, it is clicked through and never activated, so whatever is in front
//! keeps the foreground and the pad: the Launch screen, later Toasts. As the Guide it takes the foreground, from a
//! game or from Home's window, and gives it back when it stops being the Guide. Hidden, nothing is drawn or
//! presented and it is out of the compositor's scene.
//!
//! Each time it is shown it covers the monitor of the window that has the foreground then, as the Bundle's does
//! (OverlayWindow.xaml.cs, ShowOverForeground): the Guide opens on the screen the game is on.
//!
//! What it cannot do is what the Bundle cannot, and is said in the log when it shows (docs/spec/guide.md, 7.2): a
//! game in true exclusive fullscreen is not covered, and one that runs elevated keeps the foreground.
//!
//! It is made on the thread that runs the window's message loop, which dispatches its messages with Home's.
use render::gpu::Gpu;

/// How the Overlay is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Clicked through and never activated.
    Passive,
    /// Activatable, and given the foreground.
    Guide,
}

/// A key pressed while the Overlay had the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Enter,
    Back,
    Escape,
}

#[cfg(windows)]
pub use win::Overlay;

/// A window that came to the front while the Guide was up keeps the foreground when the Guide closes only if it
/// covers this much of its monitor, as a game's does (the Launch screen's rule for a game's window). A small one, such
/// as Steam's "Controller Connected" toast (`SDL_app`, #123), does not: the foreground goes back where it was.
const KEEPS_FOREGROUND: f64 = 0.6;

/// Whether a window `window` big (width, height), on a monitor `monitor` big, is one that keeps the foreground it
/// took while the Guide was up.
#[cfg_attr(not(windows), allow(dead_code))]
fn keeps_foreground(window: (i32, i32), monitor: (i32, i32)) -> bool {
    let area = |(w, h): (i32, i32)| w.max(0) as f64 * h.max(0) as f64;
    area(monitor) > 0.0 && area(window) >= area(monitor) * KEEPS_FOREGROUND
}

/// Anywhere but Windows there is no Overlay: `new` makes none, so nothing else here is ever called.
#[cfg(not(windows))]
pub struct Overlay(());

#[cfg(not(windows))]
impl Overlay {
    pub fn new(_: &Gpu, _: wgpu::TextureFormat) -> Option<Self> {
        None
    }

    pub fn size(&self) -> (u32, u32) {
        (1, 1)
    }

    pub fn mode(&self) -> Option<Mode> {
        None
    }

    pub fn lost(&self) -> bool {
        false
    }

    pub fn set(&mut self, _: Option<Mode>) {}

    pub fn give_back_to(&mut self, _: *mut std::ffi::c_void) {}

    pub fn raise(&self) {}

    pub fn frame(&mut self, _: &Gpu) -> Option<wgpu::SurfaceTexture> {
        None
    }

    pub fn present(&mut self, _: &Gpu, _: wgpu::SurfaceTexture) {}

    pub fn take_keys(&mut self) -> Vec<(Key, bool)> {
        Vec::new()
    }
}

#[cfg(windows)]
pub(crate) mod win {
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use std::time::Instant;

    use windows::core::Interface;
    use windows::Win32::Graphics::DirectComposition::{
        DCompositionCreateDevice2, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
    };
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        VK_BACK, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP,
    };
    use windows_sys::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    use super::{keeps_foreground, Gpu, Key, Mode};
    use crate::{describe, take_foreground, wide};

    /// Whether the window is being clicked through. Read by the window procedure.
    static PASSIVE: AtomicBool = AtomicBool::new(true);
    /// Keys pressed while the window had the keyboard, and whether each was a held key repeating.
    static KEYS: Mutex<Vec<(Key, bool)>> = Mutex::new(Vec::new());

    fn log(message: &str) {
        eprintln!("overlay: {message} at {:.0} ms", crate::since_start());
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let passive = PASSIVE.load(Ordering::Relaxed);
        match message {
            WM_KEYDOWN => {
                let key = match wparam as u16 {
                    VK_LEFT => Key::Left,
                    VK_RIGHT => Key::Right,
                    VK_UP => Key::Up,
                    VK_DOWN => Key::Down,
                    VK_RETURN => Key::Enter,
                    VK_BACK => Key::Back,
                    VK_ESCAPE => Key::Escape,
                    _ => return DefWindowProcW(hwnd, message, wparam, lparam),
                };
                // Bit 30: the key was already down, so this is it repeating.
                let repeat = lparam & (1 << 30) != 0;
                if let Ok(mut keys) = KEYS.lock() {
                    keys.push((key, repeat));
                }
                0
            }
            WM_ACTIVATE => {
                let active = (wparam & 0xffff) as u32 != WA_INACTIVE;
                log(&format!("WM_ACTIVATE {}", if active { "active" } else { "inactive" }));
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            // A click that does reach a passive window neither activates it nor stops in it.
            WM_MOUSEACTIVATE if passive => MA_NOACTIVATE as LRESULT,
            WM_NCHITTEST if passive => HTTRANSPARENT as LRESULT,
            // Nothing is drawn through GDI: there is no redirection bitmap to erase.
            WM_ERASEBKGND => 1,
            // Alt+F4 with the Guide up is not a way to be rid of the Overlay: it comes as SC_CLOSE, which would be
            // a WM_CLOSE.
            WM_SYSCOMMAND if wparam & 0xFFF0 == SC_CLOSE as WPARAM => 0,
            // Another program asking the Dashboard to close (taskkill): the Dashboard closes, not the Overlay alone.
            WM_CLOSE => {
                crate::ask_to_close("the Overlay");
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    /// The window, hidden and passive, covering the primary monitor until it is first shown. Returns it and its size
    /// in pixels.
    unsafe fn create_window() -> Option<(HWND, (u32, u32))> {
        let instance = GetModuleHandleW(null());
        let class = wide("XboxSeriesDashboardOverlay");
        let window_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
            hIconSm: null_mut(),
        };
        if RegisterClassExW(&window_class) == 0 {
            log("RegisterClassExW failed");
            return None;
        }
        // Real pixels: winit has already made the process aware of each monitor's scaling.
        let (width, height) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        if width <= 0 || height <= 0 {
            log(&format!("the primary monitor is {width}x{height}"));
            return None;
        }
        // NOREDIRECTIONBITMAP: the window has no GDI surface of its own, so all the compositor has of it is the
        //   visual, alpha and all. It can only be given here, as the window is made.
        // TOPMOST: above every ordinary window, a borderless game's included.
        // TOOLWINDOW: out of the taskbar and Alt+Tab.
        // NOACTIVATE: showing or clicking it never makes it the foreground window.
        // TRANSPARENT (with LAYERED): the mouse goes to whatever window is underneath, whatever is drawn here.
        // The last two are what `set` takes away while it is the Guide. LAYERED stays, as the window was made with
        // it; DASHBOARD_OVERLAY_NO_LAYERED=1 makes the window without it, which the spike found works as well.
        let mut ex =
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT;
        if std::env::var_os("DASHBOARD_OVERLAY_NO_LAYERED").is_none() {
            ex |= WS_EX_LAYERED;
        }
        let hwnd = CreateWindowExW(
            ex,
            class.as_ptr(),
            wide("Xbox Series Dashboard Overlay").as_ptr(),
            WS_POPUP,
            0,
            0,
            width,
            height,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            log("CreateWindowExW failed");
            return None;
        }
        if ex & WS_EX_LAYERED != 0 {
            // A layered window shows nothing until it is given its attributes: fully opaque, as far as layering goes.
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        }
        // taskkill's request to close reaches it from any program, as Home's window's does.
        crate::hear_close_from_anyone(hwnd);
        log(&format!("window {hwnd:?} made hidden, {width}x{height}, WS_POPUP, ex style {ex:#010x}"));
        Some((hwnd, (width as u32, height as u32)))
    }

    /// The window's composition tree: a device, a target on the window, one visual as its root. wgpu can make
    /// these itself, but only for every window of an instance, Home's included; made here, the Overlay alone is
    /// composed and Home's window keeps its ordinary swap chain on the same device.
    pub(crate) struct Composition {
        device: IDCompositionDevice,
        _target: IDCompositionTarget,
        pub(crate) visual: IDCompositionVisual,
    }

    impl Composition {
        pub(crate) unsafe fn new(hwnd: HWND) -> windows::core::Result<Self> {
            let device: IDCompositionDevice = DCompositionCreateDevice2(None::<&windows::core::IUnknown>)?;
            let target = device.CreateTargetForHwnd(windows::Win32::Foundation::HWND(hwnd), true)?;
            let visual = device.CreateVisual()?;
            target.SetRoot(&visual)?;
            Ok(Self { device, _target: target, visual })
        }

        /// Makes the swap chain wgpu has just set as the visual's content take effect: wgpu sets it and leaves the
        /// commit to whoever owns the visual.
        pub(crate) unsafe fn commit(&self) {
            if let Err(e) = self.device.Commit() {
                log(&format!("IDCompositionDevice::Commit failed: {e}"));
            }
        }
    }

    pub struct Overlay {
        // Each is there until the Overlay is dropped, which takes them in this order: the surface before the visual
        // it presents to, and both before the window is destroyed.
        surface: Option<wgpu::Surface<'static>>,
        composition: Option<Composition>,
        hwnd: HWND,
        /// The window's size in pixels: that of the monitor it was last shown over.
        size: (u32, u32),
        /// The swap chain's own size, which follows the window's when the next frame is asked for.
        config: wgpu::SurfaceConfiguration,
        mode: Option<Mode>,
        /// The foreground window when the Overlay became the Guide, to give the foreground back to.
        foreground_before: HWND,
        /// When it was last shown, and how many frames it has presented since.
        shown_at: Instant,
        presented: u64,
        /// The surface can't be drawn to any more: said once, then it stays hidden.
        lost: bool,
    }

    impl Overlay {
        /// Makes the Overlay, hidden, on `gpu`'s device, to be drawn in `format` (Home's window's, so one renderer
        /// draws both). None, having said why, where it can't be made: the Dashboard then runs without one.
        pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Option<Self> {
            let began = Instant::now();
            unsafe {
                let (hwnd, size) = create_window()?;
                let made = Self::on_window(gpu, format, hwnd, size);
                if made.is_none() {
                    // Not a window to leave about: an Overlay that was made destroys its own when dropped.
                    DestroyWindow(hwnd);
                } else {
                    log(&format!("made in {:.0} ms", began.elapsed().as_secs_f64() * 1000.0));
                }
                made
            }
        }

        unsafe fn on_window(gpu: &Gpu, format: wgpu::TextureFormat, hwnd: HWND, size: (u32, u32)) -> Option<Self> {
            let composition = match Composition::new(hwnd) {
                Ok(composition) => composition,
                Err(e) => {
                    log(&format!("no DirectComposition device, target and visual: {e}"));
                    return None;
                }
            };
            // The visual outlives the surface: see `Overlay`'s fields.
            let target = wgpu::SurfaceTargetUnsafe::CompositionVisual(composition.visual.as_raw());
            let surface = match gpu.instance.create_surface_unsafe(target) {
                Ok(surface) => surface,
                Err(e) => {
                    log(&format!("no surface on the visual: {e}"));
                    return None;
                }
            };

            let capabilities = surface.get_capabilities(&gpu.adapter);
            log(&format!("capabilities: formats {:?}", capabilities.formats));
            log(&format!("capabilities: alpha modes {:?}", capabilities.alpha_modes));
            log(&format!("capabilities: present modes {:?}", capabilities.present_modes));
            if !capabilities.formats.contains(&format) {
                log(&format!("the surface can't be {format:?}, Home's window's format"));
                return None;
            }
            if !capabilities.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
                log("PreMultiplied is not offered: the Overlay would be opaque");
                return None;
            }
            let Some(mut config) = surface.get_default_config(&gpu.adapter, size.0, size.1) else {
                log("the adapter can't draw to the surface");
                return None;
            };
            config.format = format;
            config.alpha_mode = wgpu::CompositeAlphaMode::PreMultiplied;
            config.present_mode = wgpu::PresentMode::Fifo;
            if let Some(error) = configure(gpu, &surface, &config) {
                log(&format!("the swap chain could not be made: {error}"));
                return None;
            }
            composition.commit();
            log(&format!(
                "configured: {}x{} {:?}, alpha {:?}, present {:?}",
                config.width, config.height, config.format, config.alpha_mode, config.present_mode
            ));
            Some(Self {
                surface: Some(surface),
                composition: Some(composition),
                hwnd,
                size,
                config,
                mode: None,
                foreground_before: null_mut(),
                shown_at: Instant::now(),
                presented: 0,
                lost: false,
            })
        }

        /// The window's size in pixels: that of the monitor it was last shown over.
        pub fn size(&self) -> (u32, u32) {
            self.size
        }

        /// How it is shown; None while hidden.
        pub fn mode(&self) -> Option<Mode> {
            self.mode
        }

        /// Whether it has stopped working, as it says in the log when it does: it is hidden and stays so.
        pub fn lost(&self) -> bool {
            self.lost
        }

        /// Shows it as `mode`, or hides it. Shown from hidden, it goes over the monitor of the window that has the
        /// foreground. Becoming the Guide takes the foreground from whatever has it; ceasing to be gives it back to
        /// that window.
        pub fn set(&mut self, mode: Option<Mode>) {
            let mode = mode.filter(|_| !self.lost);
            if mode == self.mode {
                return;
            }
            let before = self.mode;
            unsafe {
                if before == Some(Mode::Guide) {
                    self.give_foreground_back();
                }
                match mode {
                    None => {
                        ShowWindow(self.hwnd, SW_HIDE);
                        let shown = self.shown_at.elapsed().as_secs_f64();
                        log(&format!("hidden after {shown:.1} s and {} frames", self.presented));
                    }
                    Some(mode) => {
                        self.click_through(mode == Mode::Passive);
                        if mode == Mode::Guide {
                            self.foreground_before = GetForegroundWindow();
                        }
                        if before.is_none() {
                            self.cover_foreground_monitor();
                            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                            self.shown_at = Instant::now();
                            self.presented = 0;
                        }
                        self.raise();
                        log(&format!("shown ({mode:?})"));
                        if mode == Mode::Guide {
                            self.say_if_exclusive();
                            let allowed = take_foreground(self.hwnd);
                            let now = GetForegroundWindow();
                            log(&format!(
                                "foreground was {}, is {} (SetForegroundWindow returned {allowed}, took it: {})",
                                describe(self.foreground_before),
                                describe(now),
                                now == self.hwnd
                            ));
                            if now != self.hwnd {
                                // The Bundle's limit, accepted: the Alt tap does not reach a program that runs
                                // elevated, so Windows keeps the foreground where it is.
                                log("Windows did not give the Overlay the foreground, as it won't from a game that \
                                     runs elevated: the Guide shows above it and follows the pad, and the game still \
                                     hears the pad too (docs/spec/guide.md, 7.2)");
                            }
                        }
                    }
                }
            }
            self.mode = mode;
        }

        /// While it is the Guide: the foreground goes to this window when it stops being the Guide, and not to
        /// the one that had it when the Guide opened. Home's, for Home chosen over a game and for a game that
        /// ended under the Guide; the game's, for the running game chosen with something else in front.
        // An HWND is a handle Windows looks up, never a pointer this code follows.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub fn give_back_to(&mut self, window: *mut std::ffi::c_void) {
            if self.mode == Some(Mode::Guide) && !window.is_null() {
                self.foreground_before = window;
                log(&format!("the foreground will go to {}", unsafe { describe(window) }));
            }
        }

        /// Passive or not: the two styles a passive window has and the Guide's doesn't.
        unsafe fn click_through(&self, passive: bool) {
            PASSIVE.store(passive, Ordering::Relaxed);
            let ex = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            let styles = (WS_EX_NOACTIVATE | WS_EX_TRANSPARENT) as isize;
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, if passive { ex | styles } else { ex & !styles });
        }

        /// Moves the window over the monitor of the window in front, the primary one when nothing is in front. In
        /// real pixels: the process is aware of each monitor's scaling.
        unsafe fn cover_foreground_monitor(&mut self) {
            let front = GetForegroundWindow();
            let nearest = if front.is_null() { MONITOR_DEFAULTTOPRIMARY } else { MONITOR_DEFAULTTONEAREST };
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(MonitorFromWindow(front, nearest), &mut info) == 0 {
                log("Windows did not say which monitor the window in front is on: the Overlay stays where it was");
                return;
            }
            let area = info.rcMonitor;
            let (width, height) = (area.right - area.left, area.bottom - area.top);
            if width <= 0 || height <= 0 {
                return;
            }
            SetWindowPos(self.hwnd, HWND_TOPMOST, area.left, area.top, width, height, SWP_NOACTIVATE);
            self.size = (width as u32, height as u32);
            log(&format!(
                "over the monitor at ({}, {}), {width}x{height}, which {} is on",
                area.left,
                area.top,
                describe(front)
            ));
        }

        /// Says so when Windows reports a Direct3D program with the screen to itself: a game in true exclusive
        /// fullscreen, which no other window is drawn over. The Bundle's limit, accepted (docs/spec/guide.md, 7.2).
        unsafe fn say_if_exclusive(&self) {
            let mut state = 0;
            if SHQueryUserNotificationState(&mut state) == 0 && state == QUNS_RUNNING_D3D_FULL_SCREEN {
                log("Windows says a Direct3D program has the screen to itself (exclusive fullscreen): the Guide may \
                     not show over it, and the game may leave fullscreen or minimise as it loses the foreground \
                     (docs/spec/guide.md, 7.2)");
            }
        }

        /// Gives the foreground back while it is still the Overlay's to give: hiding first lets Windows pick. If
        /// another window as big as a game's has taken it meanwhile (a game that started under the Guide), it is
        /// left there; a small one (a store's toast, #123) is not, and the foreground goes back all the same.
        unsafe fn give_foreground_back(&mut self) {
            let to = std::mem::replace(&mut self.foreground_before, null_mut());
            let front = GetForegroundWindow();
            if front != self.hwnd && front != to {
                if !front.is_null() && keeps_foreground(Self::window_size(front), Self::monitor_size(front)) {
                    log(&format!("the foreground has gone to {} meanwhile, and stays there", describe(front)));
                    return;
                }
                log(&format!(
                    "the foreground has gone to {} meanwhile, a window smaller than a game's: it goes back all the \
                     same",
                    describe(front)
                ));
            }
            if to.is_null() || IsWindow(to) == 0 {
                log("the window that had the foreground is gone: Windows picks the next");
                return;
            }
            if IsIconic(to) != 0 {
                ShowWindow(to, SW_RESTORE);
            }
            let gave = SetForegroundWindow(to) != 0;
            log(&format!("foreground given back to {}: {gave}", describe(to)));
            if !gave {
                log("Windows refused to hand the foreground back, as it does to a game that runs elevated \
                     (docs/spec/guide.md, 7.2)");
            }
        }

        /// A window's outer size, (0, 0) when Windows won't say.
        unsafe fn window_size(window: HWND) -> (i32, i32) {
            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(window, &mut rect) == 0 {
                return (0, 0);
            }
            (rect.right - rect.left, rect.bottom - rect.top)
        }

        /// The size of the monitor a window is on, (0, 0) when Windows won't say.
        unsafe fn monitor_size(window: HWND) -> (i32, i32) {
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
                return (0, 0);
            }
            let area = info.rcMonitor;
            (area.right - area.left, area.bottom - area.top)
        }

        /// Puts it above every other topmost window, as a game that is itself topmost may be: the last window
        /// made topmost is the upper one. Does nothing while hidden.
        pub fn raise(&self) {
            unsafe {
                if IsWindowVisible(self.hwnd) != 0 {
                    SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
        }

        /// The next frame to draw on, cleared by whoever draws it. None while hidden, and when there is none to
        /// be had this time round.
        pub fn frame(&mut self, gpu: &Gpu) -> Option<wgpu::SurfaceTexture> {
            self.mode?;
            let (surface, composition) = (self.surface.as_ref()?, self.composition.as_ref()?);
            // Shown over a monitor of another size since the last frame: the swap chain is made that size.
            if (self.config.width, self.config.height) != self.size {
                let mut config = self.config.clone();
                (config.width, config.height) = self.size;
                if let Some(error) = configure(gpu, surface, &config) {
                    let size = format!("{}x{}", config.width, config.height);
                    self.give_up(&format!("the swap chain could not be made {size}: {error}"));
                    return None;
                }
                unsafe { composition.commit() };
                log(&format!("configured: {}x{}, for the monitor it is over now", config.width, config.height));
                self.config = config;
            }
            match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                    Some(frame)
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => None,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    match configure(gpu, surface, &self.config) {
                        Some(error) => self.give_up(&format!("the swap chain could not be made again: {error}")),
                        None => unsafe { composition.commit() },
                    }
                    None
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    self.give_up("the surface gave a validation error");
                    None
                }
            }
        }

        /// Stops for good, hidden so a stale picture doesn't stay over the game.
        fn give_up(&mut self, why: &str) {
            log(&format!("{why}: the Overlay is lost"));
            self.set(None);
            self.lost = true;
        }

        /// Presents a frame drawn on.
        pub fn present(&mut self, gpu: &Gpu, frame: wgpu::SurfaceTexture) {
            gpu.queue.present(frame);
            self.presented += 1;
            if self.presented == 1 {
                let after = self.shown_at.elapsed().as_secs_f64() * 1000.0;
                log(&format!("first frame presented {after:.0} ms after it was shown"));
            }
        }

        /// The keys pressed since the last call while it had the keyboard, each with whether it was a repeat.
        pub fn take_keys(&mut self) -> Vec<(Key, bool)> {
            KEYS.lock().map(|mut keys| std::mem::take(&mut *keys)).unwrap_or_default()
        }
    }

    impl Drop for Overlay {
        fn drop(&mut self) {
            self.set(None);
            self.surface = None;
            self.composition = None;
            unsafe { DestroyWindow(self.hwnd) };
        }
    }

    /// Configures the surface, returning what went wrong rather than letting wgpu panic over it.
    pub(crate) fn configure(
        gpu: &Gpu,
        surface: &wgpu::Surface<'_>,
        config: &wgpu::SurfaceConfiguration,
    ) -> Option<String> {
        let filters = [wgpu::ErrorFilter::OutOfMemory, wgpu::ErrorFilter::Internal, wgpu::ErrorFilter::Validation];
        let scopes = filters.map(|filter| gpu.device.push_error_scope(filter));
        surface.configure(&gpu.device, config);
        // Popped in the reverse of the order they were pushed in.
        let mut errors = Vec::new();
        for scope in scopes.into_iter().rev() {
            if let Some(error) = pollster::block_on(scope.pop()) {
                errors.push(error.to_string());
            }
        }
        (!errors.is_empty()).then(|| errors.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::keeps_foreground;

    /// #123: a game's window keeps the foreground it took under the Guide; Steam's toast in a corner does not.
    #[test]
    fn only_a_window_as_big_as_a_games_keeps_the_foreground() {
        let screen = (3840, 2160);
        assert!(keeps_foreground(screen, screen), "a fullscreen game");
        assert!(keeps_foreground((3200, 1800), screen), "a big window");
        assert!(!keeps_foreground((480, 140), screen), "a toast");
        assert!(!keeps_foreground((1920, 1080), screen), "a quarter of the screen");
        assert!(!keeps_foreground((0, 0), screen) && !keeps_foreground(screen, (0, 0)), "sizes Windows didn't give");
    }
}
