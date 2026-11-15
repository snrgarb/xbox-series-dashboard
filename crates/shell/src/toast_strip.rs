//! The Toast strip: a second window above every other, the Overlay's included, along the foot of the screen, where
//! Toasts are drawn (docs/spec/guide.md, 9.1). It is made as the Overlay is (a raw window with no redirection
//! bitmap, its picture a composition swap chain) but is only ever passive: clicked through and never activated,
//! whatever the Overlay is doing under it, so a Toast takes neither the foreground nor the pad from a game, nor
//! from the Guide.
//!
//! It is also where another program posts a notification to (`model::post`): the window takes the message while
//! hidden, and `take_posts` hands the texts on.
use render::gpu::Gpu;

/// How much of a screen 1080 high the strip covers, from the foot: room for the banner, 128 high and 64 up, grown
/// by a tenth as it goes.
pub const STRIP_HEIGHT: u32 = 240;

#[cfg(windows)]
pub use win::ToastStrip;

/// Anywhere but Windows there is no strip: `new` makes none, so nothing else here is ever called.
#[cfg(not(windows))]
pub struct ToastStrip(());

#[cfg(not(windows))]
impl ToastStrip {
    pub fn new(_: &Gpu, _: wgpu::TextureFormat) -> Option<Self> {
        None
    }

    pub fn size(&self) -> (u32, u32) {
        (1, 1)
    }

    pub fn screen(&self) -> (u32, u32) {
        (1, 1)
    }

    pub fn top(&self) -> u32 {
        0
    }

    pub fn shown(&self) -> bool {
        false
    }

    pub fn lost(&self) -> bool {
        false
    }

    pub fn set(&mut self, _: bool) {}

    pub fn keep_above(&self) {}

    pub fn frame(&mut self, _: &Gpu) -> Option<wgpu::SurfaceTexture> {
        None
    }

    pub fn present(&mut self, _: &Gpu, _: wgpu::SurfaceTexture) {}

    pub fn take_posts(&mut self) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(windows)]
mod win {
    use std::ptr::{null, null_mut};
    use std::sync::Mutex;
    use std::time::Instant;

    use windows::core::Interface;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    use super::{Gpu, STRIP_HEIGHT};
    use crate::overlay::win::{configure, Composition};
    use crate::wide;

    /// The texts posted to the window since they were last taken.
    static POSTS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    /// No more than this many wait to be taken: a program that posts without end fills nothing.
    const MAX_POSTS: usize = 32;
    /// How far up the windows above it are looked through for one of the Dashboard's own.
    const LOOK_ABOVE: usize = 64;

    fn log(message: &str) {
        eprintln!("toasts: {message} at {:.0} ms", crate::since_start());
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match message {
            // A notification posted by another program. Answers 1 when it was taken.
            WM_COPYDATA => {
                let data = lparam as *const COPYDATASTRUCT;
                if data.is_null() || (*data).dwData != model::post::COPYDATA_ID || (*data).lpData.is_null() {
                    return 0;
                }
                // Four bytes a character at most: what is beyond that could not be shown anyway.
                let length = ((*data).cbData as usize).min(model::post::MAX_CHARS * 4);
                let bytes = std::slice::from_raw_parts((*data).lpData as *const u8, length);
                let (Some(text), Ok(mut posts)) = (model::post::clean(bytes), POSTS.lock()) else { return 0 };
                if posts.len() >= MAX_POSTS {
                    return 0;
                }
                posts.push(text);
                1
            }
            // A click that does reach it neither activates it nor stops in it.
            WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
            WM_NCHITTEST => HTTRANSPARENT as LRESULT,
            // Nothing is drawn through GDI: there is no redirection bitmap to erase.
            WM_ERASEBKGND => 1,
            WM_CLOSE => 0,
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    /// Where the strip is, in pixels.
    #[derive(Clone, Copy)]
    struct Place {
        /// The window's size.
        size: (u32, u32),
        /// The size of the screen it is the foot of.
        screen: (u32, u32),
        /// How far down that screen its top is.
        top: u32,
    }

    /// The window, hidden, along the foot of the primary monitor. Returns it and where it is.
    unsafe fn create_window() -> Option<(HWND, Place)> {
        let instance = GetModuleHandleW(null());
        let class = wide(model::post::WINDOW_CLASS);
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
        let (width, screen_height) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        if width <= 0 || screen_height <= 0 {
            log(&format!("the primary monitor is {width}x{screen_height}"));
            return None;
        }
        let height = ((screen_height as f64 * STRIP_HEIGHT as f64 / 1080.0).round() as i32).clamp(1, screen_height);
        let top = screen_height - height;
        // The Overlay's styles as it is while passive (overlay.rs says what each is for), and they stay: this
        // window is never anything else.
        let ex = WS_EX_NOREDIRECTIONBITMAP
            | WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE
            | WS_EX_TRANSPARENT
            | WS_EX_LAYERED;
        let hwnd = CreateWindowExW(
            ex,
            class.as_ptr(),
            wide("Xbox Series Dashboard Toasts").as_ptr(),
            WS_POPUP,
            0,
            top,
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
        // A layered window shows nothing until it is given its attributes: fully opaque, as far as layering goes.
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        log(&format!("window {hwnd:?} made hidden, {width}x{height} at y {top}, ex style {ex:#010x}"));
        let size = (width as u32, height as u32);
        Some((hwnd, Place { size, screen: (width as u32, screen_height as u32), top: top as u32 }))
    }

    pub struct ToastStrip {
        // Dropped in this order: the surface before the visual it presents to, both before the window goes.
        surface: Option<wgpu::Surface<'static>>,
        composition: Option<Composition>,
        hwnd: HWND,
        place: Place,
        config: wgpu::SurfaceConfiguration,
        shown: bool,
        /// When it was last shown, and how many frames it has presented since.
        shown_at: Instant,
        presented: u64,
        /// The surface can't be drawn to any more: said once, then it stays hidden.
        lost: bool,
    }

    impl ToastStrip {
        /// Makes the strip, hidden, on `gpu`'s device, to be drawn in `format`: Home's window's, so one renderer
        /// draws all three. None, having said why, where it can't be made: Toasts are then drawn in Home's window.
        pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Option<Self> {
            unsafe {
                let (hwnd, place) = create_window()?;
                let made = Self::on_window(gpu, format, hwnd, place);
                if made.is_none() {
                    DestroyWindow(hwnd);
                }
                made
            }
        }

        unsafe fn on_window(gpu: &Gpu, format: wgpu::TextureFormat, hwnd: HWND, place: Place) -> Option<Self> {
            let composition = match Composition::new(hwnd) {
                Ok(composition) => composition,
                Err(e) => {
                    log(&format!("no DirectComposition device, target and visual: {e}"));
                    return None;
                }
            };
            let target = wgpu::SurfaceTargetUnsafe::CompositionVisual(composition.visual.as_raw());
            let surface = match gpu.instance.create_surface_unsafe(target) {
                Ok(surface) => surface,
                Err(e) => {
                    log(&format!("no surface on the visual: {e}"));
                    return None;
                }
            };
            let capabilities = surface.get_capabilities(&gpu.adapter);
            let alpha = capabilities.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied);
            if !capabilities.formats.contains(&format) || !alpha {
                log(&format!(
                    "the surface can't be {format:?} with premultiplied alpha (formats {:?}, alpha modes {:?})",
                    capabilities.formats, capabilities.alpha_modes
                ));
                return None;
            }
            let Some(mut config) = surface.get_default_config(&gpu.adapter, place.size.0, place.size.1) else {
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
            log(&format!("configured: {}x{} {:?}", config.width, config.height, config.format));
            Some(Self {
                surface: Some(surface),
                composition: Some(composition),
                hwnd,
                place,
                config,
                shown: false,
                shown_at: Instant::now(),
                presented: 0,
                lost: false,
            })
        }

        /// The window's size in pixels.
        pub fn size(&self) -> (u32, u32) {
            self.place.size
        }

        /// The size of the screen it is the foot of, in pixels.
        pub fn screen(&self) -> (u32, u32) {
            self.place.screen
        }

        /// How far down that screen its top is, in pixels.
        pub fn top(&self) -> u32 {
            self.place.top
        }

        pub fn shown(&self) -> bool {
            self.shown
        }

        /// Whether it has stopped working, as it says in the log when it does: it is hidden and stays so.
        pub fn lost(&self) -> bool {
            self.lost
        }

        /// Shows it, above every other window and without activating it, or hides it.
        pub fn set(&mut self, shown: bool) {
            let shown = shown && !self.lost;
            if shown == self.shown {
                return;
            }
            self.shown = shown;
            unsafe {
                if shown {
                    ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                    self.shown_at = Instant::now();
                    self.presented = 0;
                    self.raise();
                    log("strip shown");
                } else {
                    ShowWindow(self.hwnd, SW_HIDE);
                    let seconds = self.shown_at.elapsed().as_secs_f64();
                    log(&format!("strip hidden after {seconds:.1} s and {} frames", self.presented));
                }
            }
        }

        unsafe fn raise(&self) {
            SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }

        /// Puts it back above the Dashboard's other windows if one of them has come to be above it, as the Overlay
        /// does when it is shown, raised or given the foreground: of two topmost windows the one made so last is
        /// the upper. Does nothing while hidden, or while nothing of the Dashboard's is above it.
        pub fn keep_above(&self) {
            if !self.shown {
                return;
            }
            unsafe {
                let mut above = self.hwnd;
                for _ in 0..LOOK_ABOVE {
                    above = GetWindow(above, GW_HWNDPREV);
                    if above.is_null() {
                        return;
                    }
                    let mut process = 0;
                    GetWindowThreadProcessId(above, &mut process);
                    if process == std::process::id() && IsWindowVisible(above) != 0 {
                        self.raise();
                        return;
                    }
                }
            }
        }

        /// The next frame to draw on, cleared by whoever draws it. None while hidden, and when there is none to
        /// be had this time round.
        pub fn frame(&mut self, gpu: &Gpu) -> Option<wgpu::SurfaceTexture> {
            if !self.shown {
                return None;
            }
            let (surface, composition) = (self.surface.as_ref()?, self.composition.as_ref()?);
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
            log(&format!("{why}: the strip is lost"));
            self.set(false);
            self.lost = true;
        }

        /// Presents a frame drawn on.
        pub fn present(&mut self, gpu: &Gpu, frame: wgpu::SurfaceTexture) {
            gpu.queue.present(frame);
            self.presented += 1;
            if self.presented == 1 {
                let after = self.shown_at.elapsed().as_secs_f64() * 1000.0;
                log(&format!("first frame presented {after:.0} ms after the strip was shown"));
            }
        }

        /// The notifications other programs have posted since the last call, oldest first.
        pub fn take_posts(&mut self) -> Vec<String> {
            POSTS.lock().map(|mut posts| std::mem::take(&mut *posts)).unwrap_or_default()
        }
    }

    impl Drop for ToastStrip {
        fn drop(&mut self) {
            self.set(false);
            self.surface = None;
            self.composition = None;
            unsafe { DestroyWindow(self.hwnd) };
        }
    }
}
