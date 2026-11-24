//! Toasts (docs/spec/guide.md, section 9): the queue and what the Guide button does while one shows, told the time
//! by the test, and the banner drawn offscreen and compared with the committed reference images, as shots.rs does.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use render::gpu::{self, Gpu};
use render::offscreen::{read_back, wait, FORMAT};
use screens::guide::{Focus, Tab};
use screens::toasts::ToastKind;
use screens::{Dashboard, Effect, Sound};
use ui::host::Offscreen;
use ui::{Button, Host, PadEvent, Press};

const NOTIFICATION: Effect = Effect::Sound(Sound::Notification);
const GUIDE_OPEN: Effect = Effect::Sound(Sound::GuideOpen);

fn dashboard() -> Dashboard {
    Dashboard::new(screens::fake_library())
}

/// A cover for a game of the Queue's Toasts: tall, with bands across it, so a crop to its middle square shows.
fn cover() -> PathBuf {
    static COVER: OnceLock<PathBuf> = OnceLock::new();
    COVER
        .get_or_init(|| {
            let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("toast-cover.png");
            let image = image::RgbImage::from_fn(300, 450, |_, y| match (y * 9 / 450) % 2 {
                0 => image::Rgb([200, 60, 40]),
                _ => image::Rgb([40, 60, 200]),
            });
            shots::save_shared(&image, &path);
            path
        })
        .clone()
}

/// The fake library with each game in a database's row, 1 up, and the first two and Barlin's Gate 3 with a cover: what the Queue's
/// Toasts name games by.
fn library_with_ids() -> model::Library {
    let mut library = screens::fake_library();
    for (i, game) in library.games.iter_mut().enumerate() {
        game.id = i as i64 + 1;
        game.cover = (i < 2 || i == BALDURS_GATE.0).then(cover);
    }
    library
}

/// Places and rows of two of its games: Halyard Point, installed, and Barlin's Gate 3, not.
const HALO: (usize, i64) = (0, 1);
const BALDURS_GATE: (usize, i64) = (5, 6);

fn press(dashboard: &mut Dashboard, button: Button) -> Vec<Effect> {
    dashboard.press(Press { button, repeat: false });
    dashboard.take_effects()
}

/// The line of the Toast showing.
fn showing(dashboard: &Dashboard) -> Option<String> {
    dashboard.toasts().showing().map(|toast| toast.text.clone())
}

/// A turn of a loop that puts each frame on the screen at once: what there is now, the time passing, what there is
/// then. True when what is drawn may have changed.
fn pass(dashboard: &mut Dashboard, seconds: f32) -> bool {
    dashboard.toasts_presented(0.0);
    let changed = dashboard.advance(seconds);
    dashboard.toasts_presented(0.0);
    changed
}

#[test]
fn a_notification_is_listed_and_shows_in_a_toast_with_its_sound() {
    let mut dashboard = dashboard();
    assert_eq!(dashboard.notify("Download complete"), 1);
    assert_eq!(dashboard.take_effects(), [NOTIFICATION]);
    assert_eq!(showing(&dashboard).as_deref(), Some("Download complete"));
    let toast = dashboard.toasts().showing().unwrap();
    assert_eq!((toast.kind, toast.prompt), (ToastKind::Notification, true));

    // It goes after five seconds, in silence, and stays in the list.
    assert!(!pass(&mut dashboard, 4.9));
    assert!(pass(&mut dashboard, 0.2));
    assert_eq!((showing(&dashboard).as_deref(), dashboard.take_effects()), (None, vec![]));
    assert!(pass(&mut dashboard, 0.5));
    assert!(dashboard.toasts().on_screen().is_none());
    assert_eq!(dashboard.toasts().wake(), None);
    let listed: Vec<(u64, &str)> = dashboard.notifications().iter().map(|n| (n.id, n.text.as_str())).collect();
    assert_eq!(listed, [(1, "Download complete")]);
}

#[test]
fn a_second_waits_behind_the_first_and_shows_half_a_second_after_it_goes() {
    let mut dashboard = dashboard();
    dashboard.notify("one");
    dashboard.notify("two");
    // One sound: the second has not shown.
    assert_eq!(dashboard.take_effects(), [NOTIFICATION]);
    assert_eq!((showing(&dashboard).as_deref(), dashboard.toasts().waiting()), (Some("one"), 1));
    pass(&mut dashboard, 5.0);
    pass(&mut dashboard, 0.45);
    assert_eq!((showing(&dashboard).as_deref(), dashboard.take_effects()), (None, vec![]));
    assert!(pass(&mut dashboard, 0.1));
    assert_eq!((showing(&dashboard).as_deref(), dashboard.take_effects()), (Some("two"), vec![NOTIFICATION]));
    assert_eq!(dashboard.notifications().len(), 2);
}

#[test]
fn a_tap_of_the_guide_button_while_a_toast_shows_opens_the_guide_on_notifications() {
    let mut dashboard = dashboard();
    dashboard.notify("one");
    dashboard.notify("two");
    dashboard.take_effects();
    pass(&mut dashboard, 1.0);
    dashboard.pad_event(PadEvent::GuideTap);
    assert_eq!(dashboard.take_effects(), [GUIDE_OPEN]);
    assert!(dashboard.guide_open());
    // Until there is a Notifications Flyout (#41): focus on the Utility that will open it.
    assert_eq!(dashboard.guide().focus(), (Tab::Landing, Focus::Utility(0)));
    // The Toast goes and the one waiting is forgotten: the Guide has them now.
    assert_eq!((showing(&dashboard).as_deref(), dashboard.toasts().waiting()), (None, 0));
    pass(&mut dashboard, 1.0);
    assert!(dashboard.toasts().on_screen().is_none());
    assert_eq!((dashboard.toasts().shown(), dashboard.take_effects()), (1, vec![]));
}

#[test]
fn a_tap_as_the_toast_goes_still_opens_the_guide_on_it_and_one_after_does_not() {
    // The tap is told 300 ms after the press: a Toast that began to go meanwhile was showing at the press.
    let mut going = dashboard();
    going.notify("one");
    pass(&mut going, 5.2);
    assert!(going.toasts().showing().is_none() && going.toasts().on_screen().is_some());
    press(&mut going, Button::Guide);
    assert_eq!(going.guide().focus(), (Tab::Landing, Focus::Utility(0)));

    let mut gone = dashboard();
    gone.notify("one");
    pass(&mut gone, 5.0);
    pass(&mut gone, 0.5);
    press(&mut gone, Button::Guide);
    assert_eq!(gone.guide().focus(), (Tab::Landing, Focus::Entry(0)));
}

#[test]
fn the_greeting_has_no_prompt_and_a_tap_during_it_opens_profile_and_system() {
    let mut dashboard = dashboard();
    dashboard.greet();
    assert_eq!(dashboard.take_effects(), [NOTIFICATION]);
    // The fake library has no Profiles: its user, Player One, is the gamertag, and with no real name the greeting
    // says the whole of it (docs/spec/profiles.md, 1).
    let toast = dashboard.toasts().showing().unwrap();
    assert_eq!((toast.text.as_str(), toast.kind, toast.prompt), ("Hi Player One!", ToastKind::Greeting, false));
    // It is no notification.
    assert!(dashboard.notifications().is_empty());
    // One posted meanwhile waits behind it.
    dashboard.notify("one");
    assert_eq!((showing(&dashboard).as_deref(), dashboard.toasts().waiting()), (Some("Hi Player One!"), 1));

    assert_eq!(press(&mut dashboard, Button::Guide), [GUIDE_OPEN]);
    assert_eq!(dashboard.guide().focus(), (Tab::ProfileAndSystem, Focus::Entry(0)));
    assert_eq!(dashboard.toasts().waiting(), 0);
}

#[test]
fn opening_the_guide_any_other_way_drops_the_toasts() {
    let mut dashboard = dashboard();
    dashboard.notify("one");
    dashboard.notify("two");
    // Home starts with focus on the profile, and A there opens the Guide.
    press(&mut dashboard, Button::A);
    assert!(dashboard.guide_open());
    assert_eq!((showing(&dashboard).as_deref(), dashboard.toasts().waiting()), (None, 0));
}

#[test]
fn a_notification_that_comes_while_the_guide_is_open_shows_over_it() {
    let mut dashboard = dashboard();
    press(&mut dashboard, Button::Guide);
    dashboard.notify("one");
    assert_eq!((showing(&dashboard).as_deref(), dashboard.take_effects()), (Some("one"), vec![NOTIFICATION]));
    // The Guide button closes the Guide, as it does with no Toast, and the Toast stays its time.
    press(&mut dashboard, Button::Guide);
    assert!(!dashboard.guide_open());
    assert_eq!(showing(&dashboard).as_deref(), Some("one"));
}

#[test]
fn with_the_guide_off_a_toast_has_no_prompt_and_the_guide_button_does_nothing() {
    let mut dashboard = dashboard();
    dashboard.set_guide_enabled(false);
    dashboard.notify("one");
    assert!(!dashboard.toasts().showing().unwrap().prompt);
    dashboard.take_effects();
    dashboard.pad_event(PadEvent::GuideTap);
    assert!(!dashboard.guide_open());
    assert_eq!((showing(&dashboard).as_deref(), dashboard.take_effects()), (Some("one"), vec![]));
}

#[test]
fn queue_toasts_are_worded_as_the_bundles_and_a_tap_on_queued_opens_manage_queue() {
    let mut dashboard = Dashboard::new(library_with_ids());
    dashboard.tell(ToastKind::Queued, BALDURS_GATE.1);
    dashboard.tell(ToastKind::Queued, HALO.1);
    dashboard.tell(ToastKind::Uninstalled, HALO.1);
    assert_eq!(dashboard.take_effects(), [NOTIFICATION]);
    let toast = dashboard.toasts().showing().unwrap().clone();
    assert_eq!((toast.text.as_str(), toast.kind, toast.prompt), ("Queued", ToastKind::Queued, true));
    assert_eq!(toast.game, Some(BALDURS_GATE.1));
    assert_eq!(toast.art.as_deref().map(|game| game.name.as_str()), Some("Barlin's Gate 3"));
    // None for a game the library does not have, nor as a notification.
    dashboard.tell(ToastKind::Installed, 999);
    dashboard.tell(ToastKind::Notification, HALO.1);
    assert_eq!((dashboard.toasts().waiting(), dashboard.notifications().len()), (2, 0));

    // The Guide button opens Manage Queue, not the Guide; the Toast goes, the other Queued one with it, and the
    // tap sounds as A does.
    pass(&mut dashboard, 1.0);
    assert_eq!(press(&mut dashboard, Button::Guide), [Effect::Sound(Sound::Activate)]);
    assert!(!dashboard.guide_open());
    assert_eq!(dashboard.page(), Some("ManageQueue"));
    assert_eq!((showing(&dashboard), dashboard.toasts().waiting()), (None, 1));
    // The Uninstalled one shows after, worded with the game's name, and a tap on it opens the Guide.
    pass(&mut dashboard, 0.5);
    pass(&mut dashboard, 0.5);
    assert_eq!(showing(&dashboard).as_deref(), Some("Halyard Point was uninstalled"));
    assert_eq!(dashboard.toasts().showing().unwrap().kind, ToastKind::Uninstalled);
    dashboard.take_effects();
    press(&mut dashboard, Button::Guide);
    assert!(dashboard.guide_open());
}

#[test]
fn a_tap_on_ready_to_play_starts_the_game() {
    let mut dashboard = Dashboard::new(library_with_ids());
    dashboard.tell(ToastKind::Installed, HALO.1);
    let toast = dashboard.toasts().showing().unwrap();
    assert_eq!((toast.text.as_str(), toast.kind, toast.prompt), ("Ready to play", ToastKind::Installed, true));
    dashboard.take_effects();
    pass(&mut dashboard, 1.0);
    let effects = press(&mut dashboard, Button::Guide);
    assert_eq!(effects, [Effect::Sound(Sound::Activate), Effect::Launch(HALO.0)]);
    assert!(!dashboard.guide_open());
    assert_eq!(showing(&dashboard), None);
}

#[test]
fn the_notifications_badge_counts_what_the_dashboard_holds() {
    // Seen in the pictures below; here, that the list is what is counted.
    let mut dashboard = dashboard();
    for text in ["one", "two", "three"] {
        dashboard.notify(text);
    }
    assert_eq!(dashboard.notifications().len(), 3);
    assert_eq!(dashboard.notifications().last().map(|n| n.id), Some(3));
}

// --- The banner, drawn ---

fn target_dir() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

const HD: (u32, u32) = (1920, 1080);

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once on as many threads.
static DRAWING: Mutex<()> = Mutex::new(());

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    /// Held until the fields above, the device among them, are gone.
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn new(size: (u32, u32)) -> Self {
        Self::with(size, screens::fake_library())
    }

    fn with(size: (u32, u32), library: model::Library) -> Self {
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        std::env::set_var("DASHBOARD_FONTS", fonts);
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, size);
        let mut dashboard = Dashboard::new(library);
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn image(&self, pixels: Vec<u8>) -> image::RgbaImage {
        image::RgbaImage::from_raw(self.target.size.0, self.target.size.1, pixels).unwrap()
    }

    /// Everything at rest: Home, the Overlay's picture over it, the Toast over both.
    fn still(&mut self) -> image::RgbaImage {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        self.image(pixels)
    }

    /// The same as it is now, `seconds` after the last frame: the Toast where its age puts it.
    fn frame_after(&mut self, seconds: f32) -> image::RgbaImage {
        self.dashboard.toasts_presented(0.0);
        self.dashboard.advance(seconds);
        let Self { gpu, host, target, dashboard, .. } = self;
        loop {
            let index = host.draw(gpu, &target.view, target.size, 0.0, wgpu::Color::BLACK, |ui| dashboard.draw_all(ui));
            wait(gpu, Some(index));
            if !host.art.wait(gpu, &mut host.renderer) {
                break;
            }
        }
        let pixels = read_back(&self.gpu, &self.target.texture, self.target.size);
        self.image(pixels)
    }
}

fn compare(name: &str, image: &image::RgbaImage) {
    let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
    image.save(&got).unwrap();
    shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
}

/// The card's green, and whether a pixel is it.
const GREEN: [u8; 3] = [0x10, 0x7C, 0x10];

fn is_green(image: &image::RgbaImage, x: u32, y: u32) -> bool {
    let pixel = image.get_pixel(x, y).0;
    pixel[..3].iter().zip(GREEN).all(|(&got, want)| got.abs_diff(want) <= 6)
}

#[test]
#[ignore = "screenshot"]
fn the_banner() {
    // The greeting over Home, as the Bundle's picture of it (60-toast-greeting.jpg): the card 128 high with its
    // foot 64 above the screen's, the picture round at its left, no prompt.
    let mut stage = Stage::new(HD);
    stage.dashboard.greet();
    let greeting = stage.still();
    compare("toast-greeting", &greeting);
    // Just inside the card's edges, top and bottom, and Home just outside them.
    assert!(is_green(&greeting, 960, 890) && is_green(&greeting, 960, 1013));
    assert!(!is_green(&greeting, 960, 885) && !is_green(&greeting, 960, 1019));

    // The greeting's tile is held back for its first second: the line alone, at the card's left.
    drop(stage);
    let mut stage = Stage::new(HD);
    stage.dashboard.greet();
    compare("toast-greeting-before-slide", &stage.frame_after(0.6));

    // A notification over Home, on its line.
    drop(stage);
    let mut stage = Stage::new(HD);
    stage.dashboard.notify("Screenshot saved");
    compare("toast-notification", &stage.still());
    // Three seconds in, on its prompt.
    compare("toast-prompt", &stage.frame_after(3.0));
    // Half way between the two, each at half.
    drop(stage);
    let mut stage = Stage::new(HD);
    stage.dashboard.notify("Screenshot saved");
    compare("toast-crossfade", &stage.frame_after(2.35));

    // Coming in, half way through its 300 ms: smaller, about the same middle, and part faded.
    drop(stage);
    let mut stage = Stage::new(HD);
    stage.dashboard.notify("Screenshot saved");
    compare("toast-coming-in", &stage.frame_after(0.15));
    // Going, from when its five seconds are seen to be up: at its largest 120 ms in, and gone 410 ms in.
    stage.frame_after(4.86);
    compare("toast-going", &stage.frame_after(0.12));
    let gone = stage.frame_after(0.3);
    assert!(!is_green(&gone, 960, 950));

    // A line too long for the card ends in an ellipsis, the card at its widest: 640.
    drop(stage);
    let mut stage = Stage::new(HD);
    stage.dashboard.notify("The quick brown fox jumps over the lazy dog and keeps on running");
    let long = stage.still();
    compare("toast-long", &long);
    assert!(is_green(&long, 642, 1000) && is_green(&long, 1277, 1000));
    assert!(!is_green(&long, 637, 1000) && !is_green(&long, 1282, 1000));

    // Over the Guide over a game: the Toast is above the Overlay, and the Utility's badge counts the notification.
    drop(stage);
    let mut stage = Stage::new(HD);
    stage.dashboard.launch_started(0);
    stage.dashboard.game_booted();
    stage.dashboard.press(Press { button: Button::Guide, repeat: false });
    stage.dashboard.notify("Screenshot saved");
    compare("toast-over-guide", &stage.still());

    // The Queue's Toasts: a game's cover, its middle square, bare on the card and rounded 4.
    drop(stage);
    let mut stage = Stage::with(HD, library_with_ids());
    stage.dashboard.tell(ToastKind::Queued, BALDURS_GATE.1);
    compare("toast-queued", &stage.still());
    // Ready to play, on its prompt: "Press (Guide) to start".
    drop(stage);
    let mut stage = Stage::with(HD, library_with_ids());
    stage.dashboard.tell(ToastKind::Installed, HALO.1);
    compare("toast-ready-to-play-prompt", &stage.frame_after(3.0));
    // A game with no cover (Seal of Thorns): its kind's icon on the lighter square, as a notification's bell is.
    drop(stage);
    let mut stage = Stage::with(HD, library_with_ids());
    stage.dashboard.tell(ToastKind::Uninstalled, 3);
    compare("toast-uninstalled", &stage.still());

    // On a screen of another shape the card is in the middle across, and as far from the foot of the canvas.
    drop(stage);
    let mut stage = Stage::new((2560, 1080));
    stage.dashboard.notify("Screenshot saved");
    compare("toast-ultrawide", &stage.still());
}

/// The strip's window is the foot of the screen: a frame drawn for that part alone is that part of the whole frame.
#[test]
fn a_frame_for_the_strip_is_the_foot_of_the_whole_screens() {
    let mut stage = Stage::new(HD);
    stage.dashboard.notify("Screenshot saved");
    stage.dashboard.toasts_presented(0.0);
    stage.dashboard.advance(1.0);
    let Stage { gpu, host, target, dashboard, .. } = &mut stage;
    let draw = |host: &mut Host, target: &Offscreen, part, dashboard: &mut Dashboard| {
        // Twice: the first frame finds what text is wanted.
        for _ in 0..2 {
            let clear = wgpu::Color::TRANSPARENT;
            let index =
                host.draw_part(gpu, &target.view, target.size, part, 0.0, clear, |ui| dashboard.draw_toasts(ui));
            wait(gpu, Some(index));
        }
        read_back(gpu, &target.texture, target.size)
    };
    let whole = draw(host, target, (HD, 0), dashboard);
    let (top, height) = (840, 240);
    let strip = Offscreen::new(gpu, (HD.0, height));
    let part = draw(host, &strip, (HD, top), dashboard);
    let row = HD.0 as usize * 4;
    assert_eq!(part.len(), row * height as usize);
    // To a rounding: the same shapes are placed in a target of another height.
    let apart = part.iter().zip(&whole[row * top as usize..]).filter(|(a, b)| a.abs_diff(**b) > 4).count();
    assert_eq!(apart, 0, "the strip's picture is not the foot of the whole screen's");
    // And it has the Toast in it, on nothing: green in the card, clear beside it.
    let pixel = |x: usize, y: usize| &part[y * row + x * 4..][..4];
    assert_eq!(pixel(700, 60), [0, 0, 0, 0]);
    assert_eq!(pixel(960, 60)[3], 255);
}
