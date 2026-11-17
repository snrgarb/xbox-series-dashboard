//! The Idle screen (docs/spec/idle-screen.md): its two stages drawn over Home, over the Guide and over a game, and
//! what the press that wakes it does, which is nothing more. `SHOTS=bless cargo test -p screens --test idle`.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use model::{Game, Library};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::idle::{cards::Kind, News, Stage as IdleStage, Times, Woken};
use screens::{Dashboard, OverlayShows};
use ui::host::Offscreen;
use ui::Button::{self, Down, Guide, Right, A, Y};
use ui::{Host, PadEvent, Press};

fn target_dir() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

fn fonts() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        std::env::set_var("DASHBOARD_FONTS", fonts);
    });
}

/// Art for the first games, banded so a crop shows: a cover, a hero and a header each, as shots.rs draws them.
fn art_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("art");
        std::fs::create_dir_all(&dir).unwrap();
        for game in 0..13u32 {
            for (kind, w, h) in [("cover", 300, 450), ("hero", 960, 310), ("header", 460, 215)] {
                let hue = |shift: u32| (((game * 53 + shift * 85) % 200) + 40) as u8;
                let image = image::RgbImage::from_fn(w, h, |x, y| {
                    let band = (y * 8 / h) % 2 == 0;
                    let edge = x < 6 || y < 6 || x >= w - 6 || y >= h - 6;
                    let level = |c: u8| {
                        if edge {
                            255
                        } else if band {
                            c
                        } else {
                            c / 2
                        }
                    };
                    image::Rgb([level(hue(0)), level(hue(1)), level(hue(2))])
                });
                shots::save_shared(&image, &dir.join(format!("{game}-{kind}.png")));
            }
        }
        dir
    })
}

/// The fake library, the first three games with all their art and the fourth with a cover alone.
fn library() -> Library {
    let mut library = screens::fake_library();
    let art = |game: usize, kind: &str| Some(art_dir().join(format!("{game}-{kind}.png")));
    for (i, game) in library.games.iter_mut().enumerate() {
        let game: &mut Game = game;
        match i {
            0..=2 => (game.cover, game.hero, game.header) = (art(i, "cover"), art(i, "hero"), art(i, "header")),
            3 => game.cover = art(i, "cover"),
            _ => {}
        }
    }
    library
}

static DRAWING: Mutex<()> = Mutex::new(());

const HD: (u32, u32) = (1920, 1080);
const DIM: f32 = 3.0;
const SHOW: f32 = 6.0;

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    _drawing: MutexGuard<'static, ()>,
}

/// An achievement's icon, as Steam's are: square, here split across in two colours with a light edge.
fn icon() -> PathBuf {
    static ICON: OnceLock<PathBuf> = OnceLock::new();
    ICON.get_or_init(draw_icon).clone()
}

fn draw_icon() -> PathBuf {
    let path = art_dir().join("icon.png");
    let image = image::RgbImage::from_fn(64, 64, |x, y| {
        let edge = x < 3 || y < 3 || x >= 61 || y >= 61;
        if edge {
            image::Rgb([240, 240, 240])
        } else if x > y {
            image::Rgb([200, 140, 30])
        } else {
            image::Rgb([90, 40, 160])
        }
    });
    shots::save_shared(&image, &path);
    path
}

/// The library with achievements, as a snapshot of the library database carries them: its games given ids, Halyard
/// Point 2 of 6 with Legend nearest to hand, Hadrel II 3 of 4, and Halyard Point's Spartan (Rare, its icon fetched) the
/// last unlock, yesterday.
fn library_with_achievements() -> Library {
    use model::achievement::{Achievement, GameProgress, Totals, Unlock};
    let mut library = library();
    for (i, game) in library.games.iter_mut().enumerate() {
        game.id = i as i64 + 1;
    }
    let a = |name: &str, description: &str, rarity: f64, unlocked: Option<u64>| Achievement {
        key: name.to_uppercase(),
        name: name.into(),
        description: Some(description.into()),
        rarity: Some(rarity),
        unlocked,
        ..Achievement::default()
    };
    let at = library.games[0].last_played;
    library.achievements.totals = Some(Totals { score: 1_265, unlocked: 5, of: 10, games: 2 });
    library.achievements.games = vec![
        GameProgress { game: 1, unlocked: 2, of: 6, score: 60 },
        GameProgress { game: 5, unlocked: 3, of: 4, score: 45 },
    ];
    library.achievements.next = vec![
        Unlock { game: 1, achievement: a("Legend", "Finish the campaign on Legendary.", 8.2, None) },
        Unlock { game: 5, achievement: a("Night Bloom", "Grow every flower of the night.", 31.0, None) },
    ];
    let spartan = Achievement {
        icon_file: Some(icon()),
        ..a("Spartan", "Win a match without taking damage, against a full team.", 2.0, Some(at))
    };
    library.achievements.recent = vec![Unlock { game: 1, achievement: spartan }];
    library
}

/// A Dashboard with the Idle screen's times short, as the end-to-end test's: 3 s and 6 s. "Played yesterday" is
/// counted from the day after the fake library's last game was played.
fn dashboard() -> Dashboard {
    dashboard_of(library())
}

fn dashboard_of(library: Library) -> Dashboard {
    let mut dashboard = Dashboard::new(library);
    dashboard.set_clock(Some("9:41 AM".into()));
    // By the machine's own time zone, as the cards count, so it is the day after wherever the test runs.
    let last = screens::idle::cards::local_date(screens::fake_library().games[0].last_played);
    dashboard.set_date(last.succ_opt());
    dashboard.seed_idle(7);
    let secs = |n: f32| Some(Duration::from_secs_f32(n));
    let idle = Times { dim_after: secs(DIM), show_after: secs(SHOW) };
    dashboard.set_options(screens::Options { idle, ..dashboard.options() });
    dashboard
}

impl Stage {
    fn new() -> Self {
        Self::with(library())
    }

    fn with(library: Library) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, HD);
        Self { gpu, host, target, dashboard: dashboard_of(library), _drawing: drawing }
    }

    fn check(&mut self, name: &str) {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        // Twice: the panel asks for its art only once it has flown in, which the first one's settled frames see to.
        target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let image = image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap();
        compare(name, &image);
    }

    /// The Overlay alone over a screen of one colour: what is seen of it over a game.
    fn check_over_game(&mut self, name: &str) {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| {
            ui.fill(ui.screen(), ui::Color::rgb(0x2E7D5B), 0.0);
            dashboard.draw_overlay(ui);
        });
        let image = image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap();
        compare(name, &image);
    }
}

fn compare(name: &str, image: &image::RgbaImage) {
    let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
    image.save(&got).unwrap();
    shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// Time passes, a tenth of a second at a time, as the window's turns tell it.
fn wait(dashboard: &mut Dashboard, seconds: f32) {
    for _ in 0..(seconds * 10.0).round() as u32 {
        dashboard.advance(0.1);
    }
}

/// On to the card that matches, a card's turn at a time.
fn until_card(dashboard: &mut Dashboard, matches: impl Fn(&screens::idle::Card) -> bool) {
    for _ in 0..40 {
        if dashboard.idle_card().is_some_and(&matches) {
            return;
        }
        wait(dashboard, 20.0);
    }
    panic!("no such card came");
}

#[test]
fn it_dims_then_shows_things_and_a_press_only_wakes_it() {
    let mut dashboard = dashboard();
    press(&mut dashboard, &[Down, Right]);
    let focus = dashboard.focus_line();
    wait(&mut dashboard, DIM - 0.2);
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
    wait(&mut dashboard, 0.3);
    assert_eq!(dashboard.idle_stage(), IdleStage::Dimmed);
    assert_eq!(dashboard.overlay(), OverlayShows::Passive, "drawn on the Overlay, over whatever is in front");
    wait(&mut dashboard, SHOW - DIM);
    assert_eq!(dashboard.idle_stage(), IdleStage::ShowingThings);
    assert_eq!(dashboard.take_idle_news(), [News::Dimmed, News::ShowingThings]);
    assert!(dashboard.idle_card().is_some());

    // Right wakes it and does nothing more: focus is where it was.
    press(&mut dashboard, &[Right]);
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
    assert_eq!(dashboard.take_idle_news(), [News::Woken(Woken::Button(Right))]);
    assert_eq!(dashboard.focus_line(), focus);
    // The next press is a press.
    press(&mut dashboard, &[Right]);
    assert_ne!(dashboard.focus_line(), focus);
}

#[test]
fn the_guide_button_that_wakes_it_opens_nothing() {
    let mut dashboard = dashboard();
    wait(&mut dashboard, DIM + 0.1);
    for event in [PadEvent::GuideDown, PadEvent::Release(Guide), PadEvent::GuideTap] {
        dashboard.pad_event(event);
    }
    assert!(!dashboard.guide_open());
    assert_eq!(dashboard.take_idle_news(), [News::Dimmed, News::Woken(Woken::Guide)]);
    // Held, the same.
    wait(&mut dashboard, DIM + 0.1);
    for event in [PadEvent::GuideDown, PadEvent::GuideHeld, PadEvent::Release(Guide)] {
        dashboard.pad_event(event);
    }
    assert!(!dashboard.guide_open());
    // And the next tap opens the Guide.
    for event in [PadEvent::GuideDown, PadEvent::Release(Guide), PadEvent::GuideTap] {
        dashboard.pad_event(event);
    }
    assert!(dashboard.guide_open());
}

#[test]
fn y_on_a_game_card_opens_its_game_hub() {
    let mut dashboard = dashboard();
    wait(&mut dashboard, SHOW + 0.1);
    until_card(&mut dashboard, |card| card.kind == Kind::RecentlyPlayed);
    let game = dashboard.idle_card().unwrap().game.clone().unwrap();
    let id = dashboard.library().games.iter().find(|g| g.is(&game)).unwrap().source_id.clone();
    press(&mut dashboard, &[Y]);
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
    assert!(dashboard.focus_line().starts_with(&format!("GameHub: {id} ")), "{}", dashboard.focus_line());
}

#[test]
fn the_launch_screen_holds_it_off() {
    let mut dashboard = dashboard();
    dashboard.launch_started(2);
    wait(&mut dashboard, SHOW * 3.0);
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
    // Counted from zero once the game is up.
    dashboard.game_booted();
    wait(&mut dashboard, DIM - 0.2);
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
    wait(&mut dashboard, 0.3);
    assert_eq!(dashboard.idle_stage(), IdleStage::Dimmed);
}

/// Who's playing today? and Signing you in hold it off however long they show, and it counts from zero as Home
/// shows after (docs/spec/start-up.md).
#[test]
fn startup_holds_it_off_and_it_counts_from_zero_after() {
    let mut dashboard = dashboard();
    dashboard.start_up(false);
    assert_eq!(dashboard.startup_phase(), Some(screens::Phase::WhosPlaying));
    wait(&mut dashboard, SHOW * 3.0);
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.startup_phase(), Some(screens::Phase::SigningIn));
    // Three seconds, and the tenth of a second the steps' sum falls short by.
    wait(&mut dashboard, 3.1);
    assert!(!dashboard.starting_up());
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
    assert!(dashboard.take_idle_news().is_empty());
    wait(&mut dashboard, DIM - 0.2);
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
    wait(&mut dashboard, 0.3);
    assert_eq!(dashboard.idle_stage(), IdleStage::Dimmed);
}

/// Waking from sleep with the Idle screen up: it goes at once, not drawn over Who's playing today?, and stays away
/// while that asks.
#[test]
fn waking_from_sleep_takes_it_down_at_once_for_whos_playing_today() {
    let mut dashboard = dashboard();
    wait(&mut dashboard, SHOW + 0.1);
    assert_eq!(dashboard.idle_stage(), IdleStage::ShowingThings);
    assert!(dashboard.woke_from_sleep());
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
    wait(&mut dashboard, SHOW * 2.0);
    assert_eq!(dashboard.idle_stage(), IdleStage::Awake);
}

// --- Pictures ---

/// Dimmed over Home: the whole screen under 70% black.
#[test]
fn idle_dimmed() {
    let mut stage = Stage::new();
    press(&mut stage.dashboard, &[Down]);
    wait(&mut stage.dashboard, DIM + 0.1);
    stage.check("idle-dimmed");
}

/// Showing things, a game played yesterday: its hero art in the lower half, rising out of the panel.
#[test]
fn idle_showing_things_a_game() {
    let mut stage = Stage::new();
    press(&mut stage.dashboard, &[Down]);
    wait(&mut stage.dashboard, SHOW + 0.1);
    until_card(&mut stage.dashboard, |card| card.title == "Halyard Point");
    stage.check("idle-showing-game");
}

/// A game with only a cover: the cover blurred and darkened in the art's place, and the cover itself over it.
#[test]
fn idle_showing_things_a_game_with_a_cover_alone() {
    let mut stage = Stage::new();
    press(&mut stage.dashboard, &[Down]);
    wait(&mut stage.dashboard, SHOW + 0.1);
    until_card(&mut stage.dashboard, |card| card.title == "Starvault");
    stage.check("idle-showing-cover");
}

/// A tip: slate blue, the pad drawn with the buttons it is about filled.
#[test]
fn idle_showing_things_a_tip() {
    let mut stage = Stage::new();
    press(&mut stage.dashboard, &[Down]);
    wait(&mut stage.dashboard, SHOW + 0.1);
    until_card(&mut stage.dashboard, |card| card.title.starts_with("On the keyboard"));
    stage.check("idle-showing-tip");
}

/// A nearly-finished game: the locked achievement most players have, its Score with the Rare diamond, the game's
/// progress at the right and in the bar under it, the game's art.
#[test]
fn idle_showing_things_a_close_achievement() {
    let mut stage = Stage::with(library_with_achievements());
    press(&mut stage.dashboard, &[Down]);
    wait(&mut stage.dashboard, SHOW + 0.1);
    until_card(&mut stage.dashboard, |card| card.title == "Halyard Point - Legend");
    let card = stage.dashboard.idle_card().unwrap();
    assert_eq!(
        (card.kind, card.score, card.rare, card.label.as_deref()),
        (Kind::CloseAchievement, Some(30), true, Some("Close achievement"))
    );
    stage.check("idle-showing-close-achievement");
}

/// The last unlock: when at the top right, its Score at the left, its icon large over the game's art.
#[test]
fn idle_showing_things_a_recent_unlock() {
    let mut stage = Stage::with(library_with_achievements());
    press(&mut stage.dashboard, &[Down]);
    wait(&mut stage.dashboard, SHOW + 0.1);
    until_card(&mut stage.dashboard, |card| card.kind == Kind::RecentUnlock);
    let card = stage.dashboard.idle_card().unwrap();
    assert_eq!(card.title, "Halyard Point - Spartan");
    assert_eq!(card.header.as_deref(), Some("Unlocked yesterday"));
    stage.check("idle-showing-recent-unlock");
    // Y on it sees the game, as on any game's card.
    press(&mut stage.dashboard, &[Y]);
    assert_eq!(stage.dashboard.page(), Some("GameHub"));
}

/// Over the Guide, which it dims with everything else.
#[test]
fn idle_dimmed_over_the_guide() {
    let mut stage = Stage::new();
    press(&mut stage.dashboard, &[Guide]);
    wait(&mut stage.dashboard, DIM + 0.1);
    assert!(stage.dashboard.guide_open());
    stage.check("idle-dimmed-over-guide");
}

/// Over a game: the Overlay alone, the game showing through the dim.
#[test]
fn idle_showing_things_over_a_game() {
    let mut stage = Stage::new();
    stage.dashboard.launch_started(2);
    stage.dashboard.game_booted();
    wait(&mut stage.dashboard, SHOW + 0.1);
    until_card(&mut stage.dashboard, |card| card.title.starts_with("Press the Guide button"));
    stage.check_over_game("idle-showing-over-game");
    // Woken: the Overlay is wanted until the dim has faded, then for nothing.
    press(&mut stage.dashboard, &[A]);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Passive, "fading");
    stage.check_over_game("idle-woken-over-game");
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);
}
