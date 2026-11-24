//! The Achievement unlocked Toast (docs/spec/achievement-toast.md): what the Dashboard makes of the unlocks it is
//! given, told the time by the test, its row in the Notifications Flyout and what A on it opens; and the pill, both
//! forms, drawn offscreen and compared with the committed reference images, as toasts.rs does the banner.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::achievement::{Achievement, GameProgress, Unlock};
use render::gpu::{self, Gpu};
use render::offscreen::{read_back, wait, FORMAT};
use screens::guide::{Flyout, FlyoutFocus, Focus, Tab};
use screens::toasts::ToastKind;
use screens::{Dashboard, Effect, Sound};
use ui::host::Offscreen;
use ui::{Button, Host, Press};

/// The game whose achievements are unlocked: Halyard Point, row 1, which has achievements; Seal of Thorns, row 3,
/// which has none.
const GAME: i64 = 1;
const NO_ACHIEVEMENTS: i64 = 3;

/// A made-up icon: rings of two colours, so its crop to the circle and its zoom show.
fn icon() -> PathBuf {
    static ICON: OnceLock<PathBuf> = OnceLock::new();
    ICON.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("achievement-icon.png");
        let image = image::RgbImage::from_fn(64, 64, |x, y| {
            let d = ((x as f32 - 31.5).powi(2) + (y as f32 - 31.5).powi(2)).sqrt();
            if (d as u32 / 8).is_multiple_of(2) {
                image::Rgb([230, 170, 30])
            } else {
                image::Rgb([120, 40, 160])
            }
        });
        shots::save_shared(&image, &path);
        path
    })
    .clone()
}

/// The fake library with each game in a row of its own, 1 up, and Halyard Point's achievements counted.
fn library() -> model::Library {
    let mut library = screens::fake_library();
    for (i, game) in library.games.iter_mut().enumerate() {
        game.id = i as i64 + 1;
    }
    library.achievements.games.push(GameProgress { game: GAME, unlocked: 2, of: 6, score: 25 });
    library
}

fn unlock(game: i64, name: &str, rarity: Option<f64>, description: Option<&str>) -> Unlock {
    let achievement = Achievement {
        key: name.to_uppercase().replace(' ', "_"),
        name: name.into(),
        description: description.map(Into::into),
        rarity,
        unlocked: Some(1_700_000_000),
        icon_file: Some(icon()),
        ..Default::default()
    };
    Unlock { game, achievement }
}

fn night_route() -> Unlock {
    unlock(GAME, "Night Route", Some(18.2), Some("Finish a delivery after dark."))
}

fn no_detours() -> Unlock {
    unlock(GAME, "No Detours", Some(7.4), Some("Finish a chapter without leaving the road."))
}

fn press(dashboard: &mut Dashboard, button: Button) -> Vec<Effect> {
    dashboard.press(Press { button, repeat: false });
    dashboard.take_effects()
}

/// A turn of a loop that puts each frame on the screen at once.
fn pass(dashboard: &mut Dashboard, seconds: f32) {
    dashboard.toasts_presented(0.0);
    dashboard.advance(seconds);
    dashboard.toasts_presented(0.0);
}

fn showing(dashboard: &Dashboard) -> Option<(ToastKind, String)> {
    dashboard.toasts().showing().map(|toast| (toast.kind, toast.text.clone()))
}

#[test]
fn an_unlock_is_told_of_a_second_after_it_is_found_in_a_toast_and_a_notification() {
    let mut dashboard = Dashboard::new(library());
    dashboard.unlocked(vec![night_route()]);
    // It waits a second for others found with it: the loop is told to wake for it.
    assert_eq!((showing(&dashboard), dashboard.timer()), (None, Some(1.0)));
    pass(&mut dashboard, 0.5);
    assert_eq!(showing(&dashboard), None);
    pass(&mut dashboard, 0.5);
    assert_eq!(showing(&dashboard), Some((ToastKind::Achievement, "Achievement unlocked".into())));
    assert_eq!(dashboard.take_effects(), [Effect::Sound(Sound::Notification)]);
    let toast = dashboard.toasts().showing().unwrap();
    assert_eq!(toast.told_line(), " \"Night Route\", score 20");
    assert!(toast.prompt);
    // Listed, with its game, for A to open its page.
    let listed = dashboard.notifications().last().unwrap();
    assert_eq!(listed.text, "Achievement unlocked in Halyard Point: Night Route");
    assert_eq!(listed.game, Some(GAME));

    // It shows for 8.5 s, as the console's pill, and goes over 1.12 s.
    pass(&mut dashboard, 8.4);
    assert!(showing(&dashboard).is_some());
    pass(&mut dashboard, 0.2);
    assert!(showing(&dashboard).is_none() && dashboard.toasts().on_screen().is_some());
    pass(&mut dashboard, 1.0);
    assert!(dashboard.toasts().on_screen().is_some());
    pass(&mut dashboard, 0.2);
    assert!(dashboard.toasts().on_screen().is_none());
}

#[test]
fn a_rare_one_alone_is_the_rare_form_with_its_own_sound() {
    let mut dashboard = Dashboard::new(library());
    dashboard.unlocked(vec![no_detours()]);
    pass(&mut dashboard, 1.0);
    assert_eq!(showing(&dashboard), Some((ToastKind::RareAchievement, "Rare Achievement unlocked".into())));
    assert_eq!(dashboard.take_effects(), [Effect::Sound(Sound::RareAchievement)]);
    assert_eq!(dashboard.toasts().showing().unwrap().told_line(), " \"No Detours\", score 30, 7.4%");
    assert_eq!(
        dashboard.notifications().last().unwrap().text,
        "Rare Achievement unlocked in Halyard Point: No Detours"
    );
    // It goes at 8.45 s, as its sound's flourish starts.
    pass(&mut dashboard, 8.4);
    assert!(showing(&dashboard).is_some());
    pass(&mut dashboard, 0.1);
    assert!(showing(&dashboard).is_none());
}

#[test]
fn several_found_within_a_second_are_one_ordinary_toast_a_rare_one_among_them() {
    let mut dashboard = Dashboard::new(library());
    dashboard.unlocked(vec![night_route()]);
    pass(&mut dashboard, 0.6);
    dashboard.unlocked(vec![no_detours()]);
    pass(&mut dashboard, 0.4);
    assert_eq!(showing(&dashboard), Some((ToastKind::Achievement, "2 achievements unlocked".into())));
    assert_eq!(dashboard.take_effects(), [Effect::Sound(Sound::Notification)]);
    assert_eq!(dashboard.notifications().len(), 1);
    assert_eq!(dashboard.notifications()[0].text, "2 achievements unlocked in Halyard Point");
    // One found after the second is up is a Toast of its own, after this one's pill has gone and the gap.
    pass(&mut dashboard, 0.5);
    dashboard.unlocked(vec![unlock(GAME, "The Last Parcel", Some(45.0), None)]);
    pass(&mut dashboard, 1.0);
    assert_eq!(dashboard.toasts().waiting(), 1);
    pass(&mut dashboard, 7.1);
    pass(&mut dashboard, 1.12);
    assert_eq!(dashboard.toasts().on_screen().map(|t| t.kind), None);
    pass(&mut dashboard, 0.5);
    assert_eq!(showing(&dashboard), Some((ToastKind::Achievement, "Achievement unlocked".into())));
}

#[test]
fn with_the_guide_off_it_has_no_prompt() {
    let mut dashboard = Dashboard::new(library());
    dashboard.set_guide_enabled(false);
    dashboard.unlocked(vec![night_route()]);
    pass(&mut dashboard, 1.0);
    assert!(!dashboard.toasts().showing().unwrap().prompt);
}

#[test]
fn a_tap_of_the_guide_button_while_it_shows_opens_profile_and_system() {
    for unlocked in [night_route(), no_detours()] {
        let mut dashboard = Dashboard::new(library());
        dashboard.unlocked(vec![unlocked]);
        pass(&mut dashboard, 2.0);
        dashboard.take_effects();
        assert_eq!(press(&mut dashboard, Button::Guide), [Effect::Sound(Sound::GuideOpen)]);
        assert_eq!(dashboard.guide().focus(), (Tab::ProfileAndSystem, Focus::Entry(0)));
        // The pill goes, over the Guide.
        assert!(showing(&dashboard).is_none() && dashboard.toasts().on_screen().is_some());
    }
}

/// The Guide open on its Notifications Flyout, focus on the newest: after every Toast has gone, as while one shows the
/// Guide button opens it on what the Toast told of.
fn notifications(dashboard: &mut Dashboard) {
    for _ in 0..30 {
        pass(dashboard, 1.0);
    }
    assert!(dashboard.toasts().on_screen().is_none());
    press(dashboard, Button::Guide);
    dashboard.open_notifications();
    assert_eq!(dashboard.flyout(), Some((Flyout::Notifications, FlyoutFocus::Item(0))));
}

#[test]
fn a_on_its_notification_opens_the_games_achievements_page_and_b_goes_home() {
    let mut dashboard = Dashboard::new(library());
    dashboard.unlocked(vec![night_route()]);
    pass(&mut dashboard, 1.0);
    dashboard.notify("Download complete");
    notifications(&mut dashboard);
    // On a notification of anything else, A does nothing, in silence.
    assert_eq!(press(&mut dashboard, Button::A), []);
    assert!(dashboard.guide_open());
    // Down to the achievement's: the Guide closes onto its page, A sounding as the Guide's select, the Dashboard
    // asked in front of whatever the Guide opened over (an App, with no game running).
    press(&mut dashboard, Button::Down);
    assert_eq!(
        press(&mut dashboard, Button::A),
        [Effect::Sound(Sound::GuideSelect), Effect::GuideClosed, Effect::ShowHome]
    );
    assert!(!dashboard.guide_open());
    assert_eq!(dashboard.page(), Some("Achievements"));
    assert_eq!(dashboard.achievements_wanted(), Some(GAME));
    press(&mut dashboard, Button::B);
    assert_eq!(dashboard.page(), None);
}

#[test]
fn over_a_game_it_brings_the_dashboard_in_front_and_a_game_with_none_has_its_hub() {
    let mut dashboard = Dashboard::new(library());
    dashboard.launch_started(2);
    dashboard.game_booted();
    dashboard.unlocked(vec![unlock(NO_ACHIEVEMENTS, "Pirate Legend", Some(3.0), None)]);
    pass(&mut dashboard, 1.0);
    notifications(&mut dashboard);
    let effects = press(&mut dashboard, Button::A);
    assert!(effects.contains(&Effect::ShowHome), "{effects:?}");
    assert_eq!(dashboard.page(), Some("GameHub"));
}

// --- The pill, drawn ---

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
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn new() -> Self {
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        std::env::set_var("DASHBOARD_FONTS", fonts);
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, HD);
        let mut dashboard = Dashboard::new(library());
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    /// A Stage with these unlocks' Toast posted, its first frame not yet out.
    fn told(unlocks: Vec<Unlock>) -> Self {
        let mut stage = Self::new();
        stage.dashboard.unlocked(unlocks);
        stage.dashboard.advance(1.0);
        stage
    }

    fn image(&self, pixels: Vec<u8>) -> image::RgbaImage {
        image::RgbaImage::from_raw(self.target.size.0, self.target.size.1, pixels).unwrap()
    }

    /// Everything at rest: the pill with its first page in.
    fn still(&mut self) -> image::RgbaImage {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        self.image(pixels)
    }

    /// `seconds` after the last frame: the pill where its age puts it.
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

    /// Its first frame, and then the one `seconds` after it showed.
    fn at(&mut self, seconds: f32) -> image::RgbaImage {
        self.frame_after(0.0);
        self.frame_after(seconds)
    }
}

fn compare(name: &str, image: &image::RgbaImage) {
    let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
    image.save(&got).unwrap();
    shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
}

fn near(image: &image::RgbaImage, x: u32, y: u32, rgb: [u8; 3]) -> bool {
    let pixel = image.get_pixel(x, y).0;
    pixel[..3].iter().zip(rgb).all(|(&got, want)| got.abs_diff(want) <= 6)
}

#[test]
#[ignore = "screenshot"]
fn the_pill() {
    // An ordinary unlock at rest: the green pill, 778 by 128 with its foot 64 above the screen's, the icon in the
    // lighter circle at its left, the line over the Score badge, the Score and the name.
    let mut stage = Stage::told(vec![night_route()]);
    let ordinary = stage.still();
    compare("toast-achievement", &ordinary);
    // The pill's green just inside its right end and its middle's top and foot; Home just outside.
    assert!(near(&ordinary, 1340, 952, [0x10, 0x7C, 0x10]) && near(&ordinary, 960, 891, [0x10, 0x7C, 0x10]));
    assert!(near(&ordinary, 960, 1013, [0x10, 0x7C, 0x10]));
    assert!(!near(&ordinary, 960, 885, [0x10, 0x7C, 0x10]) && !near(&ordinary, 1352, 952, [0x10, 0x7C, 0x10]));

    // The description's page, pushed up in place of the first, and the prompt.
    drop(stage);
    let mut stage = Stage::told(vec![night_route()]);
    compare("toast-achievement-description", &stage.at(5.0));
    compare("toast-achievement-prompt", &stage.frame_after(2.5));
    // Going, 0.3 s in: the pill collapsing into its circle.
    stage.frame_after(1.0);
    compare("toast-achievement-going", &stage.frame_after(0.3));

    // Coming: the circle alone at the pill's centre with the Xbox mark, then the icon zooming out to its place.
    drop(stage);
    let mut stage = Stage::told(vec![night_route()]);
    compare("toast-achievement-coming", &stage.at(0.6));
    compare("toast-achievement-icon-zooming", &stage.frame_after(0.6));

    // The rare form at rest: blue, the diamond in the brighter circle, the rarity after the line.
    drop(stage);
    let mut stage = Stage::told(vec![no_detours()]);
    let rare = stage.still();
    compare("toast-achievement-rare", &rare);
    assert!(near(&rare, 1340, 952, [0x0F, 0x4E, 0x7E]));
    // The diamond flying out of the gem.
    drop(stage);
    let mut stage = Stage::told(vec![no_detours()]);
    compare("toast-achievement-rare-flying", &stage.at(1.2));

    // Several: the line alone in the middle, the trophy in the circle.
    drop(stage);
    let mut stage = Stage::told(vec![night_route(), no_detours()]);
    compare("toast-achievements-several", &stage.still());

    // Its row in the Notifications Flyout: the trophy in the bell's place.
    drop(stage);
    let mut stage = Stage::told(vec![no_detours()]);
    stage.dashboard.notify("Download complete");
    notifications(&mut stage.dashboard);
    compare("guide-notifications-achievement", &stage.still());
}
