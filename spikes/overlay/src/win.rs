//! The Overlay window and its loop: a raw Win32 window, a wgpu surface on a DirectComposition visual, and a message
//! loop that draws while the Overlay is shown and sleeps while it is hidden.
use std::num::NonZeroIsize;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows_sys::Win32::Foundation::{FILETIME, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetPixel, ReleaseDC,
    SelectObject, CAPTUREBLT, CLR_INVALID, SRCCOPY,
};
use windows_sys::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_ESCAPE, VK_MENU,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use render::gpu::{self, Gpu};
use render::{Frame, Renderer};

use crate::args::{Args, Mode, SurfaceKind};
use crate::say;
use crate::scene::{self, Blend, Probe};

const SHOWN: Duration = Duration::from_secs(2);
const HIDDEN: Duration = Duration::from_secs(2);
/// No frame starts sooner than this after the last was presented. The vertical blank sets the pace (60 Hz on the TV);
/// this only holds a faster display, or a swap chain that doesn't wait, to about that.
const TICK: Duration = Duration::from_millis(12);
/// How long after the first frame `--selfcheck` reads the screen: time for the DWM to have composed it.
const SELFCHECK_AFTER: Duration = Duration::from_millis(500);

/// Whether the window is the click-through, never-activated kind. Read by the window procedure.
static PASSIVE: AtomicBool = AtomicBool::new(false);
/// How many times Windows activated the window.
static ACTIVATIONS: AtomicU32 = AtomicU32::new(0);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// wgpu's own warnings and errors, which say why a swap chain could not be made.
struct WgpuLog;

impl log::Log for WgpuLog {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "{:>9.1} ms  wgpu: {} {}: {}",
                crate::since_start(),
                record.level(),
                record.target(),
                record.args()
            );
        }
    }
    fn flush(&self) {}
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let passive = PASSIVE.load(Ordering::Relaxed);
    match message {
        WM_KEYDOWN if wparam as u16 == VK_ESCAPE => {
            say!("escape pressed");
            PostQuitMessage(0);
            0
        }
        WM_ACTIVATE => {
            let state = (wparam & 0xffff) as u32;
            if state != WA_INACTIVE {
                ACTIVATIONS.fetch_add(1, Ordering::Relaxed);
            }
            say!("WM_ACTIVATE {}", if state == WA_INACTIVE { "inactive" } else { "active" });
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        // A click that does reach a passive window neither activates it nor stops in it.
        WM_MOUSEACTIVATE if passive => MA_NOACTIVATE as LRESULT,
        WM_NCHITTEST if passive => HTTRANSPARENT as LRESULT,
        // Nothing is drawn through GDI: there is no redirection bitmap to erase.
        WM_ERASEBKGND => 1,
        WM_CLOSE => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

/// The extended styles, and why each is there.
fn ex_style(args: &Args) -> u32 {
    // NOREDIRECTIONBITMAP: the window has no GDI surface of its own, so the only thing the DWM composes for it is
    //   the DirectComposition visual, alpha and all. Without it an opaque redirection surface sits under the visual.
    // TOPMOST: above every ordinary window, a borderless game's included.
    // TOOLWINDOW: out of the taskbar and Alt+Tab.
    let mut ex = WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
    if args.mode == Mode::Passive {
        // NOACTIVATE: showing or clicking it never makes it the foreground window.
        // TRANSPARENT (with LAYERED): the mouse goes to whatever window is underneath, whatever is drawn here.
        ex |= WS_EX_NOACTIVATE | WS_EX_TRANSPARENT;
        if args.layered {
            ex |= WS_EX_LAYERED;
        }
    }
    ex
}

/// The window, hidden, covering the primary monitor. Returns it and its size in pixels.
unsafe fn create_window(args: &Args) -> (HWND, (u32, u32)) {
    // Real pixels: without this a scaled display gives a smaller, stretched window and GDI reads scaled coordinates.
    if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
        say!("SetProcessDpiAwarenessContext failed: sizes may be scaled");
    }
    let instance = GetModuleHandleW(null());
    let class = wide("XboxSeriesDashboardOverlaySpike");
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
    assert!(RegisterClassExW(&window_class) != 0, "RegisterClassExW failed");
    let (width, height) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
    let ex = ex_style(args);
    let hwnd = CreateWindowExW(
        ex,
        class.as_ptr(),
        wide("Overlay spike").as_ptr(),
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
    assert!(!hwnd.is_null(), "CreateWindowExW failed");
    if ex & WS_EX_LAYERED != 0 {
        // A layered window shows nothing until it is given its attributes: fully opaque, as far as layering goes.
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
    }
    say!("window {hwnd:?} created hidden, {width}x{height}, WS_POPUP, ex style {ex:#010x}");
    (hwnd, (width as u32, height as u32))
}

/// The spike's own composition tree, for `--visual`: a device, a target on the window, one visual as its root.
struct Composition {
    device: IDCompositionDevice,
    _target: IDCompositionTarget,
    visual: IDCompositionVisual,
}

impl Composition {
    unsafe fn new(hwnd: HWND) -> windows::core::Result<Self> {
        let device: IDCompositionDevice = DCompositionCreateDevice2(None::<&windows::core::IUnknown>)?;
        let target = device.CreateTargetForHwnd(windows::Win32::Foundation::HWND(hwnd), true)?;
        let visual = device.CreateVisual()?;
        target.SetRoot(&visual)?;
        Ok(Self { device, _target: target, visual })
    }

    /// Makes the swap chain wgpu has just set as the visual's content take effect.
    unsafe fn commit(&self) {
        if let Err(e) = self.device.Commit() {
            say!("IDCompositionDevice::Commit failed: {e}");
        }
    }
}

/// A window, for the log: its handle, process, class and title.
unsafe fn describe(hwnd: HWND) -> String {
    if hwnd.is_null() {
        return "none".into();
    }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    let mut class = [0u16; 128];
    let class_len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
    let mut title = [0u16; 128];
    let title_len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32).max(0) as usize;
    format!(
        "{hwnd:?} (pid {pid}, class \"{}\", title \"{}\")",
        String::from_utf16_lossy(&class[..class_len]),
        String::from_utf16_lossy(&title[..title_len])
    )
}

/// Takes the foreground from another program's window. Windows lets a program do that only after input it received,
/// so tap Alt first: as `shell`'s `take_foreground` does (xbox-series-theme docs/research/topmost-overlay.md §4).
unsafe fn take_foreground(hwnd: HWND, alt: bool) -> bool {
    if alt {
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_MENU, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        let taps = [key(0), key(KEYEVENTF_KEYUP)];
        let sent = SendInput(2, taps.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        if sent != 2 {
            say!("SendInput sent {sent} of 2 events");
        }
    }
    SetForegroundWindow(hwnd) != 0
}

/// The screen's colour at each point, read through GDI from the screen's device context. `None` where it can't be.
///
/// This is what `Graphics.CopyFromScreen` does, and under the DWM it reads the composed desktop: layered windows and
/// DirectComposition visuals included. It does not see a window that set `WDA_EXCLUDEFROMCAPTURE`, and what it returns
/// for content the DWM hands straight to the display (an independent-flip or overlay-plane game) is not documented:
/// it may be black or stale there, which says nothing about what the screen showed.
unsafe fn screen_colours(points: &[(i32, i32)]) -> Vec<Option<[u8; 3]>> {
    let screen = GetDC(null_mut());
    let memory = CreateCompatibleDC(screen);
    let bitmap = CreateCompatibleBitmap(screen, 1, 1);
    let previous = SelectObject(memory, bitmap);
    let colours = points
        .iter()
        .map(|&(x, y)| {
            if BitBlt(memory, 0, 0, 1, 1, screen, x, y, SRCCOPY | CAPTUREBLT) == 0 {
                return None;
            }
            let colour = GetPixel(memory, 0, 0);
            (colour != CLR_INVALID).then_some([colour as u8, (colour >> 8) as u8, (colour >> 16) as u8])
        })
        .collect();
    SelectObject(memory, previous);
    DeleteObject(bitmap);
    DeleteDC(memory);
    ReleaseDC(null_mut(), screen);
    colours
}

/// The processor time this process has used, kernel and user, in milliseconds.
unsafe fn cpu_ms() -> f64 {
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    if GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user) == 0 {
        return f64::NAN;
    }
    let ticks = |t: FILETIME| ((t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64) as f64;
    (ticks(kernel) + ticks(user)) / 10_000.0
}

/// Sleeps until `until` or until a message arrives, whichever is first.
unsafe fn wait_for_message_until(until: Instant) {
    let ms = until.saturating_duration_since(Instant::now()).as_millis().min(60_000) as u32;
    MsgWaitForMultipleObjects(0, null(), 0, ms, QS_ALLINPUT);
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Smallest, mean and largest of some milliseconds, for the log.
fn spread(values: &[f64]) -> String {
    if values.is_empty() {
        return "none".into();
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(0.0, f64::max);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    format!("{min:.2}/{mean:.2}/{max:.2} ms")
}

/// One stretch of the Overlay being shown: what each frame cost.
#[derive(Default)]
struct ShownStats {
    /// From one present returning to the next.
    intervals: Vec<f64>,
    /// Waiting for a back buffer, which is where the wait for the vertical blank happens.
    acquire: Vec<f64>,
    /// Encoding and submitting the frame.
    draw: Vec<f64>,
    /// The present call itself.
    present: Vec<f64>,
    /// Times no back buffer could be had.
    skipped: u32,
}

struct Overlay {
    args: Args,
    hwnd: HWND,
    size: (u32, u32),
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    composition: Option<Composition>,
    renderer: Renderer,
    frame: Frame,
    probes: [Probe; 3],

    shown: bool,
    /// The foreground window when the Overlay was last shown, to give the foreground back to.
    foreground_before: HWND,
    /// Frames presented since the process started.
    presented: u64,
    last_present: Option<Instant>,
    first_present: Option<Instant>,
    stats: ShownStats,
    behind: Vec<Option<[u8; 3]>>,
    selfchecked: bool,
}

impl Overlay {
    unsafe fn new(args: &Args) -> Self {
        let (hwnd, size) = create_window(args);

        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::DX12;
        // A swap chain made for composition, set as the content of a DirectComposition visual on the window: the
        // only kind DXGI gives an alpha mode other than "ignore". wgpu makes the visual itself with this option.
        descriptor.backend_options.dx12.presentation_system = match args.surface {
            SurfaceKind::Hwnd => wgpu::Dx12SwapchainKind::DxgiFromHwnd,
            SurfaceKind::WgpuVisual | SurfaceKind::OwnVisual => wgpu::Dx12SwapchainKind::DxgiFromVisual,
        };
        let instance = wgpu::Instance::new(descriptor);

        let composition = (args.surface == SurfaceKind::OwnVisual)
            .then(|| Composition::new(hwnd).expect("DirectComposition device, target and visual"));
        let target = match &composition {
            Some(composition) => wgpu::SurfaceTargetUnsafe::CompositionVisual(composition.visual.as_raw()),
            None => {
                let window = wgpu::rwh::Win32WindowHandle::new(NonZeroIsize::new(hwnd as isize).unwrap());
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(
                        wgpu::rwh::WindowsDisplayHandle::new(),
                    )),
                    raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(window),
                }
            }
        };
        // The window and the visual outlive the surface: `run` drops the surface before it destroys the window.
        let surface = instance.create_surface_unsafe(target).expect("create surface");
        say!("surface: {:?}", args.surface);

        // Hardware, then software (WARP), then anything: render's fallbacks. It logs the adapter.
        let gpu = gpu::init(instance, Some(&surface));

        let capabilities = surface.get_capabilities(&gpu.adapter);
        say!("capabilities: formats {:?}", capabilities.formats);
        say!("capabilities: alpha modes {:?}", capabilities.alpha_modes);
        say!("capabilities: present modes {:?}", capabilities.present_modes);
        say!("capabilities: usages {:?}", capabilities.usages);

        let mut config =
            surface.get_default_config(&gpu.adapter, size.0, size.1).expect("the adapter can't draw to the surface");
        // The renderer blends as encoded on a plain 8-bit target, as WPF does: --gamma is what it always does now.
        let blend = Blend::Gamma;
        let wanted = |format: &wgpu::TextureFormat| {
            format.is_srgb() == (blend == Blend::Linear)
                && matches!(
                    format.remove_srgb_suffix(),
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
        };
        match capabilities.formats.iter().copied().find(wanted) {
            Some(format) => config.format = format,
            None => say!("no 8-bit format for {blend:?} blending: using {:?}", config.format),
        }
        let premultiplied = capabilities.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied);
        config.alpha_mode =
            if premultiplied { wgpu::CompositeAlphaMode::PreMultiplied } else { capabilities.alpha_modes[0] };
        if !premultiplied {
            say!("alpha: PreMultiplied is NOT offered: the Overlay will be opaque");
        }
        config.present_mode = wgpu::PresentMode::Fifo;
        surface.configure(&gpu.device, &config);
        if let Some(composition) = &composition {
            composition.commit();
        }
        say!(
            "configured: {}x{} {:?}, alpha {:?}, present {:?}, blend {blend:?}",
            config.width,
            config.height,
            config.format,
            config.alpha_mode,
            config.present_mode
        );

        let renderer = Renderer::new(&gpu.device, &gpu.queue, config.format);
        let info = gpu.adapter.get_info();
        let lines = [
            "Overlay spike".to_string(),
            format!("Mode: {:?}", args.mode),
            format!("Surface: {:?}", args.surface),
            format!("{:?}, {:?}", config.format, config.alpha_mode),
            info.name.clone(),
            "Esc exits when this has the keyboard".to_string(),
        ];
        let frame = scene::frame(size, blend, &lines);

        Self {
            args: args.clone(),
            hwnd,
            size,
            gpu,
            surface,
            config,
            composition,
            renderer,
            frame,
            probes: scene::probes(size, blend),
            shown: false,
            foreground_before: null_mut(),
            presented: 0,
            last_present: None,
            first_present: None,
            stats: ShownStats::default(),
            behind: Vec::new(),
            selfchecked: false,
        }
    }

    fn probe_points(&self) -> Vec<(i32, i32)> {
        self.probes.iter().map(|p| (p.x, p.y)).collect()
    }

    unsafe fn show(&mut self) {
        if self.args.selfcheck && self.behind.is_empty() {
            self.behind = screen_colours(&self.probe_points());
        }
        self.foreground_before = GetForegroundWindow();
        say!("foreground before show: {}", describe(self.foreground_before));
        ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        // Above a game that is itself topmost: the last window made topmost is the upper one.
        SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        self.shown = true;
        self.last_present = None;
        self.stats = ShownStats::default();
        say!("shown ({:?})", self.args.mode);
        match self.args.mode {
            Mode::Guide => {
                let allowed = take_foreground(self.hwnd, self.args.alt);
                let now = GetForegroundWindow();
                say!(
                    "foreground after show: {} (SetForegroundWindow returned {allowed}, took foreground: {})",
                    describe(now),
                    now == self.hwnd
                );
            }
            Mode::Passive => self.report_passive("after show"),
        }
    }

    /// Passive: the foreground window is whatever it was, and Windows never activated the Overlay.
    unsafe fn report_passive(&self, when: &str) {
        let now = GetForegroundWindow();
        say!(
            "passive {when}: foreground unchanged: {}, activations: {} (foreground is {})",
            now == self.foreground_before && now != self.hwnd,
            ACTIVATIONS.load(Ordering::Relaxed),
            describe(now)
        );
    }

    unsafe fn hide(&mut self) {
        match self.args.mode {
            Mode::Guide => {
                say!("foreground before hide: {}", describe(GetForegroundWindow()));
                // Give the foreground back while it is still ours to give, then hide: hiding first lets Windows pick.
                if !self.foreground_before.is_null() && IsWindow(self.foreground_before) != 0 {
                    let gave = SetForegroundWindow(self.foreground_before) != 0;
                    say!("foreground given back to {}: {gave}", describe(self.foreground_before));
                }
            }
            Mode::Passive => self.report_passive("before hide"),
        }
        ShowWindow(self.hwnd, SW_HIDE);
        self.shown = false;
        say!("hidden");
    }

    /// Draws and presents one frame.
    unsafe fn draw(&mut self) {
        let started = Instant::now();
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.stats.skipped += 1;
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.stats.skipped += 1;
                self.surface.configure(&self.gpu.device, &self.config);
                if let Some(composition) = &self.composition {
                    composition.commit();
                }
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => panic!("surface validation error"),
        };
        let acquired = Instant::now();
        let view = texture.texture.create_view(&Default::default());
        self.renderer.render(
            &self.gpu.device,
            &self.gpu.queue,
            &view,
            self.size,
            &self.frame,
            wgpu::Color::TRANSPARENT,
        );
        let drawn = Instant::now();
        self.gpu.queue.present(texture);
        let presented = Instant::now();

        self.presented += 1;
        if self.first_present.is_none() {
            self.first_present = Some(presented);
            say!("first frame presented at {:.0} ms", crate::since_start());
        }
        if let Some(last) = self.last_present {
            self.stats.intervals.push(ms(presented - last));
        }
        self.last_present = Some(presented);
        self.stats.acquire.push(ms(acquired - started));
        self.stats.draw.push(ms(drawn - acquired));
        self.stats.present.push(ms(presented - drawn));
    }

    /// `--selfcheck`: what the screen shows at each probe, beside what was there before and what composing the back
    /// buffer over that should give.
    unsafe fn selfcheck(&mut self) {
        self.selfchecked = true;
        let now = screen_colours(&self.probe_points());
        let mut passed = 0;
        for ((probe, behind), now) in self.probes.iter().zip(&self.behind).zip(&now) {
            let stored = probe.stored.map(|c| (c * 255.0).round() as u8);
            let (Some(behind), Some(now)) = (behind, now) else {
                say!("selfcheck {}: at ({}, {}) the screen could not be read", probe.name, probe.x, probe.y);
                continue;
            };
            let (as_stored, linear) = scene::composed(probe.stored, *behind);
            let verdict = match (scene::close(*now, as_stored, 4), scene::close(*now, linear, 4)) {
                (true, true) => "matches both",
                (true, false) => "matches as-stored",
                (false, true) => "matches linear",
                (false, false) if scene::close(*now, *behind, 1) => "NO MATCH: unchanged, the Overlay is not on screen",
                (false, false) => "NO MATCH",
            };
            if !verdict.starts_with("NO") {
                passed += 1;
            }
            say!(
                "selfcheck {}: at ({}, {}) behind {behind:?} now {now:?}; buffer {stored:?} composed as stored \
                 {as_stored:?}, in linear light {linear:?}: {verdict}",
                probe.name,
                probe.x,
                probe.y
            );
        }
        say!("selfcheck: {passed} of {} points composed as expected", self.probes.len());
    }

    fn report_shown(&self, period: u32, lasted: Duration) {
        let stats = &self.stats;
        say!(
            "shown period {period}: {:.0} ms, {} frames presented, {} skipped, interval min/avg/max {}",
            ms(lasted),
            stats.acquire.len(),
            stats.skipped,
            spread(&stats.intervals)
        );
        say!(
            "shown period {period}: acquire min/avg/max {}, draw {}, present {}",
            spread(&stats.acquire),
            spread(&stats.draw),
            spread(&stats.present)
        );
    }
}

/// Dispatches what is in the queue. False once the loop should end.
unsafe fn pump() -> bool {
    let mut message: MSG = std::mem::zeroed();
    while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
        if message.message == WM_QUIT {
            return false;
        }
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    true
}

pub fn run(args: &Args) -> i32 {
    static LOGGER: WgpuLog = WgpuLog;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
    say!("arguments: {args:?}");
    unsafe {
        PASSIVE.store(args.mode == Mode::Passive, Ordering::Relaxed);
        // Sleeps of a few milliseconds that last a few milliseconds.
        timeBeginPeriod(1);
        let mut overlay = Overlay::new(args);

        let started = Instant::now();
        let run_until = args.run_seconds().map(|s| started + Duration::from_secs_f64(s));
        let mut period = 1;
        let mut period_started = started;
        // When the current shown or hidden stretch ends; never, without --cycle.
        let mut period_ends = (args.cycle > 0).then(|| started + SHOWN);
        // While hidden: frames presented and processor time at the start, and times the loop woke.
        let mut hidden_from = (0u64, 0.0f64);
        let mut wakes = 0u32;
        overlay.show();

        let reason = loop {
            if !pump() {
                break "quit";
            }
            let now = Instant::now();
            if run_until.is_some_and(|until| now >= until) {
                break "time is up";
            }
            if period_ends.is_some_and(|ends| now >= ends) {
                if overlay.shown {
                    overlay.report_shown(period, now - period_started);
                    overlay.hide();
                    hidden_from = (overlay.presented, cpu_ms());
                    wakes = 0;
                    period_ends = Some(now + HIDDEN);
                } else {
                    say!(
                        "hidden period {period}: {:.0} ms, {} frames presented, {wakes} wake-ups, {:.1} ms of processor time",
                        ms(now - period_started),
                        overlay.presented - hidden_from.0,
                        cpu_ms() - hidden_from.1
                    );
                    if period == args.cycle {
                        break "cycles done";
                    }
                    period += 1;
                    overlay.show();
                    period_ends = Some(Instant::now() + SHOWN);
                }
                period_started = Instant::now();
                continue;
            }
            if overlay.shown {
                if let Some(due) = overlay.last_present.map(|last| last + TICK).filter(|due| now < *due) {
                    wait_for_message_until(due);
                    continue;
                }
                overlay.draw();
                let settled = overlay.first_present.is_some_and(|first| first.elapsed() >= SELFCHECK_AFTER);
                if args.selfcheck && !overlay.selfchecked && settled {
                    overlay.selfcheck();
                }
            } else {
                // Hidden: nothing to draw, so nothing wakes this but a message or the end of the stretch.
                let until = [period_ends, run_until].into_iter().flatten().min().unwrap_or(now + HIDDEN);
                wait_for_message_until(until);
                wakes += 1;
            }
        };

        say!("ending: {reason}");
        if overlay.shown {
            if args.cycle == 0 {
                overlay.report_shown(period, period_started.elapsed());
            }
            overlay.hide();
        }
        say!("frames presented in all: {}, activations: {}", overlay.presented, ACTIVATIONS.load(Ordering::Relaxed));
        let Overlay { hwnd, surface, composition, renderer, gpu, .. } = overlay;
        drop(renderer);
        drop(surface);
        drop(composition);
        drop(gpu);
        DestroyWindow(hwnd);
        timeEndPeriod(1);
        say!("exit 0");
    }
    0
}
