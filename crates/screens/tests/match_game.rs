//! Match game (docs/spec/match-game.md): opened from the Tile menu's More options, what it asks the shell for and
//! what A and B do; then drawn offscreen and set against the committed references
//! (`SHOTS=bless cargo test -p screens --test match_game` accepts what is drawn now).
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use model::art_match::{Hit, Match};
use model::art_picker::{Answer, Ask};
use model::ArtSource;
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::keyboard::Typed;
use screens::{Dashboard, Effect, Options};
use ui::host::Offscreen;
use ui::Button::{self, Down, Menu, Right, RightBumper, A, B, Y};
use ui::{Host, Press};

/// The fake library as the library database gives it: its games a store's, so their art can be looked up.
fn library() -> model::Library {
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &screens::fake_library(), 1_780_000_000).unwrap();
    database.snapshot().unwrap()
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// Home with art online and a SteamGridDB key, the Recent row's fifth game's Match game opened from its Tile menu.
fn opened() -> Dashboard {
    let mut dashboard = Dashboard::new(library());
    dashboard.set_options(Options { art_key: true, art_online: true, ..dashboard.options() });
    press(&mut dashboard, &[Down, Right, Right, Right, Right, Menu]);
    for _ in 0..8 {
        if dashboard.focus_line().contains("(More options)") {
            break;
        }
        press(&mut dashboard, &[Down]);
    }
    press(&mut dashboard, &[A, Down, Down, Down]);
    assert_eq!(dashboard.focus_line(), "TileMenu: MoreOptions(Match game) of Hadrel II");
    press(&mut dashboard, &[A]);
    dashboard.take_effects();
    dashboard
}

fn hits() -> Vec<Hit> {
    vec![
        Hit { id: "5263".into(), name: "Hadrel II".into(), year: Some(2004) },
        Hit { id: "5264".into(), name: "Hadrel II: The Burning Orchard".into(), year: Some(2007) },
        Hit { id: "5270".into(), name: "Hadrel II Classic".into(), year: Some(2019) },
        Hit { id: "38100".into(), name: "Hadrel".into(), year: None },
    ]
}

/// Answers every search asked, with `hits`; gives what was asked.
fn serve(dashboard: &mut Dashboard, hits: &[Hit]) -> Vec<(ArtSource, String)> {
    let mut asked = Vec::new();
    for ask in dashboard.art_asks() {
        let Ask::Search { game, source, words } = ask else { panic!("only searches: {ask:?}") };
        asked.push((source, words.clone()));
        dashboard.art_answer(Answer::Results { game, source, words, result: Ok(hits.to_vec()) });
    }
    asked
}

// ---------------------------------------------------------------------------------------------------- behaviour

#[test]
fn match_game_opens_in_the_menus_place_and_b_goes_back_to_home() {
    let mut dashboard = opened();
    assert_eq!(dashboard.page(), Some("MatchGame"));
    assert_eq!(dashboard.pages_up(), 1, "in the menu's place");
    assert_eq!(dashboard.focus_line(), "MatchGame: Steam: Automatic; matched: Automatic; loading");
    assert_eq!(serve(&mut dashboard, &[]), [(ArtSource::Steam, "Hadrel II".to_string())]);
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.page(), None);
    assert_eq!(dashboard.focus_line(), "Recent(4)");
    assert!(!dashboard.take_effects().iter().any(|e| matches!(e, Effect::MatchGame { .. })), "left as it was");
}

#[test]
fn a_result_found_for_other_words_matches_the_game_on_that_source() {
    let mut dashboard = opened();
    serve(&mut dashboard, &[]);
    press(&mut dashboard, &[RightBumper, RightBumper, RightBumper]);
    serve(&mut dashboard, &[]);
    assert_eq!(dashboard.focus_line(), "MatchGame: SteamGridDB: Automatic; matched: Automatic; none");
    // Y: text entry over the page, on the words; typed and kept, they are searched for.
    press(&mut dashboard, &[Y]);
    assert_eq!(dashboard.page(), Some("TextEntry"));
    dashboard.typed(Typed::Text("x"));
    press(&mut dashboard, &[Menu]);
    assert_eq!(dashboard.page(), Some("MatchGame"));
    assert_eq!(serve(&mut dashboard, &hits()), [(ArtSource::SteamGridDb, "Hadrel IIx".to_string())]);
    press(&mut dashboard, &[Down, Down]);
    assert_eq!(dashboard.focus_line(), "MatchGame: SteamGridDB: Result 2 (5264); matched: Automatic; 4 found");
    press(&mut dashboard, &[A]);
    let game = dashboard.library().games[4].key();
    let matched =
        Match { source: ArtSource::SteamGridDb, id: "5264".into(), name: "Hadrel II: The Burning Orchard".into() };
    let effects = dashboard.take_effects();
    assert!(
        effects.contains(&Effect::MatchGame { game, source: ArtSource::SteamGridDb, matched: Some(matched) }),
        "{effects:?}"
    );
    assert_eq!(dashboard.page(), None);
}

// ---------------------------------------------------------------------------------------------------- pictures

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once.
static DRAWING: Mutex<()> = Mutex::new(());

fn target_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")
}

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn opened() -> Self {
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var("DASHBOARD_FONTS", fonts);
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, (1920, 1080));
        let mut dashboard = opened();
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn check(&mut self, name: &str) {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let image = image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }
}

/// SteamGridDB's tab: what its search found for the game's name, focus on the second result.
#[test]
fn shot_the_results_of_a_search() {
    let mut stage = Stage::opened();
    serve(&mut stage.dashboard, &[]);
    press(&mut stage.dashboard, &[RightBumper, RightBumper, RightBumper]);
    serve(&mut stage.dashboard, &hits());
    press(&mut stage.dashboard, &[Down, Down]);
    stage.check("match-game-results");
}

/// A source turned off among Settings > Art's sources: nothing asked, and said where to turn it on.
#[test]
fn shot_a_source_that_is_off() {
    let mut stage = Stage::opened();
    let order = model::ArtOrder::default().switched(ArtSource::Steam);
    let options = Options { art_order: order, ..stage.dashboard.options() };
    stage.dashboard = opened_with(options);
    assert!(stage.dashboard.art_asks().is_empty());
    stage.check("match-game-off");
}

fn opened_with(options: Options) -> Dashboard {
    let mut dashboard = Dashboard::new(library());
    dashboard.set_options(options);
    press(&mut dashboard, &[Down, Right, Right, Right, Right, Menu]);
    for _ in 0..8 {
        if dashboard.focus_line().contains("(More options)") {
            break;
        }
        press(&mut dashboard, &[Down]);
    }
    press(&mut dashboard, &[A, Down, Down, Down, A]);
    dashboard.set_clock(Some("9:41 AM".into()));
    dashboard
}
