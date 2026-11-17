//! Manage Queue (docs/spec/manage-queue.md): where focus goes on it, the three ways in and B back out of each, what
//! A, Menu and View do; then the page drawn offscreen and compared with the committed references (`SHOTS=bless
//! cargo test -p screens` accepts what is drawn now). No picture of the Bundle's shows this page: it is drawn from
//! the Bundle's XAML (docs/spec/manage-queue.md says where each measure comes from).
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::{Can, Game, InstallState, Installed, Library, Queue, Queued, Steer};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::home::Focus;
use screens::my_games::Entry;
use screens::pages::Open;
use screens::{hint_bar, Dashboard, Effect};
use ui::host::Offscreen;
use ui::Button::{self, Down, Left, Menu, Right, Up, View, A, B};
use ui::{Host, Press};

/// When the library database first saw every game.
const ADDED: u64 = 1_780_000_000;

/// Art for the games, drawn once: a cover each, in its own colours with bands across it, so a crop shows.
fn art_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("manage-queue-art");
        std::fs::create_dir_all(&dir).unwrap();
        for game in 0..GAMES.len() as u32 {
            let hue = |shift: u32| (((game * 71 + shift * 89) % 200) + 40) as u8;
            let image = image::RgbImage::from_fn(300, 450, |x, y| {
                let edge = x < 6 || y < 6 || x >= 294 || y >= 444;
                let level = |c: u8| match (edge, (y * 8 / 450) % 2 == 0) {
                    (true, _) => 255,
                    (false, true) => c,
                    (false, false) => c / 2,
                };
                image::Rgb([level(hue(0)), level(hue(1)), level(hue(2))])
            });
            shots::save_shared(&image, &dir.join(format!("{game}.png")));
        }
        dir
    })
}

/// Made-up games: name, installed, days since last played (none: never). The three not installed were played, so
/// that the Recent row has them and their hubs are a press away.
const GAMES: [(&str, bool, Option<u64>); 8] = [
    ("Ashfall Courier", true, Some(5)),
    ("Brass Meridian", true, Some(6)),
    ("Copperhead Station", true, None),
    ("Dune Cartographer", false, Some(1)),
    ("Ember Choir", false, Some(2)),
    ("Fathom Nine", false, Some(3)),
    ("Glasswing Atlas", true, None),
    ("Hollow Pines Motel and the Very Long Name of Its Second Season", true, None),
];

/// The games as the library database gives them, with only those `installed` picks installed.
fn library_where(installed: impl Fn(&str, bool) -> bool) -> Library {
    let games = GAMES.iter().enumerate().map(|(i, &(name, was, played))| Game {
        source_id: (3000 + i).to_string(),
        name: name.into(),
        installed: installed(name, was),
        last_played: played.map_or(0, |days| ADDED + 100 * 86_400 - days * 86_400),
        playtime_minutes: if played.is_some() { 60 } else { 0 },
        cover: Some(art_dir().join(format!("{i}.png"))),
        ..Game::default()
    });
    let found = Library { user: Some("Player One".into()), games: games.collect(), ..Library::default() };
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &found, ADDED).unwrap();
    database.snapshot().unwrap()
}

fn library() -> Library {
    library_where(|_, installed| installed)
}

fn id(library: &Library, name: &str) -> i64 {
    library.games.iter().find(|game| game.name == name).unwrap().id
}

const MB: u64 = 1 << 20;
const GB: u64 = 1 << 30;

/// Dune Cartographer downloading, 67.80 MB of 3.55 GB at 106 Mbps.
fn downloading(library: &Library) -> Queued {
    let state = InstallState::Downloading { done: 71_094_272, total: 3_811_768_320 };
    Queued { speed: Some(106_000_000), can: STEERS, ..Queued::new(id(library, "Dune Cartographer"), state, 10) }
}

/// A store that can pause, resume and cancel its installs, as the stand-in Steam can.
const STEERS: Can = Can { pause: true, cancel: true, ..Can::NOTHING };

/// The current download, then Ember Choir paused part way, then Fathom Nine waiting its turn.
fn several(library: &Library) -> Queue {
    let paused = InstallState::Paused { done: 512 * MB, total: 2 * GB };
    let paused = Queued { can: STEERS, ..Queued::new(id(library, "Ember Choir"), paused, 20) };
    let waiting = Queued { can: STEERS, ..Queued::new(id(library, "Fathom Nine"), InstallState::Queued, 30) };
    Queue::new(vec![downloading(library), paused, waiting], Vec::new())
}

fn with_queue(mut library: Library, queue: impl Fn(&Library) -> Queue) -> Library {
    library.queue = queue(&library);
    library
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// Where focus is after the presses, the page's name and state left off.
fn after(dashboard: &mut Dashboard, buttons: &[Button]) -> String {
    press(dashboard, buttons);
    let line = dashboard.focus_line();
    let line = line.strip_prefix("ManageQueue: ").unwrap_or_else(|| panic!("Manage Queue is not up: {line}"));
    line.split("; ").next().unwrap().to_string()
}

fn on_page(library: Library) -> Dashboard {
    let mut dashboard = Dashboard::new(library);
    dashboard.open_page(screens::manage_queue::open());
    dashboard.take_effects();
    dashboard
}

/// What has been asked of the shell, but for the sounds.
fn asked(dashboard: &mut Dashboard) -> Vec<Effect> {
    dashboard.take_effects().into_iter().filter(|effect| !matches!(effect, Effect::Sound(_))).collect()
}

#[test]
fn recently_installed_is_what_the_queue_saw_finish_then_the_rest_installed() {
    let mut library = library();
    let (glasswing, ashfall) = (id(&library, "Glasswing Atlas"), id(&library, "Ashfall Courier"));
    library.queue.installed =
        vec![Installed { game: glasswing, finished: 9 }, Installed { game: ashfall, finished: 8 }];
    let names: Vec<&str> =
        screens::manage_queue::recently_installed(&library).iter().map(|game| game.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Glasswing Atlas",
            "Ashfall Courier",
            "Brass Meridian",
            "Copperhead Station",
            "Hollow Pines Motel and the Very Long Name of Its Second Season"
        ]
    );
    // One no longer installed, or hidden, is not there.
    let mut library = library_where(|name, installed| installed && name != "Glasswing Atlas");
    library.queue.installed = vec![Installed { game: id(&library, "Glasswing Atlas"), finished: 9 }];
    assert_eq!(screens::manage_queue::recently_installed(&library).len(), 4);
}

#[test]
fn focus_goes_along_the_covers_down_the_rows_and_up_to_the_cover_it_left() {
    let mut page = on_page(with_queue(library(), several));
    assert_eq!(page.focus_line(), "ManageQueue: Cover(0) of Ashfall Courier; 3 installing, 5 recently installed");
    assert_eq!(page.hints(), hint_bar::MORE_OPTIONS);
    // Along the covers, stopping at their ends.
    assert_eq!(after(&mut page, &[Left, Up]), "Cover(0) of Ashfall Courier");
    assert_eq!(after(&mut page, &[Right, Right]), "Cover(2) of Copperhead Station");
    assert_eq!(
        after(&mut page, &[Right, Right, Right, Right]),
        "Cover(4) of Hollow Pines Motel and the Very Long Name of Its Second Season"
    );
    assert_eq!(after(&mut page, &[Left, Left]), "Cover(2) of Copperhead Station");
    // Down to the first row, the current download, and down the rows, stopping at the last. A row's state and
    // share are said after the counts.
    assert_eq!(after(&mut page, &[Down]), "Row(0) of Dune Cartographer");
    assert!(
        page.focus_line().ends_with("; 3 installing, 5 recently installed; Installing... 1.9%"),
        "{}",
        page.focus_line()
    );
    assert_eq!(after(&mut page, &[Left, Right]), "Row(0) of Dune Cartographer");
    assert_eq!(after(&mut page, &[Down, Down, Down]), "Row(2) of Fathom Nine");
    assert!(page.focus_line().ends_with("; Queued 0.0%"), "{}", page.focus_line());
    assert_eq!(page.hints(), hint_bar::MORE_OPTIONS);
    // Up the rows, and from the first to the cover focus left.
    assert_eq!(after(&mut page, &[Up, Up]), "Row(0) of Dune Cartographer");
    assert_eq!(after(&mut page, &[Up]), "Cover(2) of Copperhead Station");
}

#[test]
fn with_nothing_installed_or_queued_nothing_has_focus_and_b_still_goes() {
    let mut page = on_page(library_where(|_, _| false));
    assert_eq!(page.focus_line(), "ManageQueue: Nothing; 0 installing, 0 recently installed");
    assert_eq!(page.hints(), hint_bar::NONE);
    press(&mut page, &[A, Menu, View, Down, Right]);
    assert_eq!((asked(&mut page), page.page()), (vec![], Some("ManageQueue")));
    press(&mut page, &[B]);
    assert_eq!(page.page(), None);
}

#[test]
fn only_rows_focus_the_first_row_and_up_goes_nowhere() {
    let mut page = on_page(with_queue(library_where(|_, _| false), several));
    assert_eq!(after(&mut page, &[]), "Row(0) of Dune Cartographer");
    assert_eq!(after(&mut page, &[Up]), "Row(0) of Dune Cartographer");
}

#[test]
fn a_game_that_finishes_keeps_focus_on_its_cover_and_one_dropped_gives_it_to_the_first_cover() {
    let mut library = with_queue(library(), several);
    let mut page = on_page(library.clone());
    assert_eq!(after(&mut page, &[Down, Down]), "Row(1) of Ember Choir");
    // Ember Choir finishes: it leaves the Queue for Recently installed, and focus goes with it.
    let ember = id(&library, "Ember Choir");
    library.games.iter_mut().filter(|game| game.id == ember).for_each(|game| game.installed = true);
    library.queue.entries.retain(|entry| entry.game != ember);
    library.queue.installed = vec![Installed { game: ember, finished: 40 }];
    page.set_library(library.clone());
    assert_eq!(after(&mut page, &[]), "Cover(0) of Ember Choir");
    assert!(page.focus_line().ends_with("; 2 installing, 6 recently installed"), "{}", page.focus_line());
    // Down and on to Fathom Nine, which Steam then drops: the first cover.
    assert_eq!(after(&mut page, &[Down, Down]), "Row(1) of Fathom Nine");
    page.set_queue(Queue::new(vec![downloading(&library)], library.queue.installed.clone()));
    assert_eq!(after(&mut page, &[]), "Cover(0) of Ember Choir");
}

#[test]
fn menu_and_view_open_the_games_tile_menu_and_b_there_comes_back() {
    let mut page = on_page(with_queue(library(), several));
    press(&mut page, &[Down, Down]);
    for button in [Menu, View] {
        press(&mut page, &[button]);
        assert!(page.focus_line().starts_with("TileMenu: "), "{}", page.focus_line());
        assert!(page.focus_line().ends_with(" of Ember Choir"), "{}", page.focus_line());
        assert_eq!(after(&mut page, &[B]), "Row(1) of Ember Choir");
    }
    // On a cover too, with My games & apps' Customise row.
    press(&mut page, &[Up, Up, View]);
    assert!(page.focus_line().ends_with(" of Ashfall Courier"), "{}", page.focus_line());
}

/// #170: a row's menu is the queued game's, with Pause or Resume and Cancel after Install; each asks the game's
/// store through the shell, Cancel once the player has said yes.
#[test]
fn a_rows_menu_pauses_resumes_and_cancels_its_install() {
    let library = with_queue(library(), several);
    let (dune, ember) = (id(&library, "Dune Cartographer"), id(&library, "Ember Choir"));
    let mut page = on_page(library);
    press(&mut page, &[Down, View]);
    assert_eq!(page.focus_line(), "TileMenu: Menu(Install) of Dune Cartographer");
    press(&mut page, &[Down]);
    assert_eq!(page.focus_line(), "TileMenu: Menu(Pause installation) of Dune Cartographer");
    press(&mut page, &[Down]);
    assert_eq!(page.focus_line(), "TileMenu: Menu(Cancel installation) of Dune Cartographer");
    press(&mut page, &[Up]);
    page.take_effects();
    press(&mut page, &[A]);
    assert_eq!(asked(&mut page), [Effect::Steer { game: dune, steer: Steer::Pause }]);
    assert_eq!(after(&mut page, &[]), "Row(0) of Dune Cartographer");

    // The paused one resumes.
    press(&mut page, &[Down, Menu, Down]);
    assert_eq!(page.focus_line(), "TileMenu: Menu(Resume installation) of Ember Choir");
    press(&mut page, &[A]);
    assert_eq!(asked(&mut page), [Effect::Steer { game: ember, steer: Steer::Resume }]);

    // Cancel asks first; Keep installing goes back to the menu, on Cancel installation.
    press(&mut page, &[View, Down, Down]);
    press(&mut page, &[A]);
    assert_eq!(page.focus_line(), "TileMenu: Prompt(Cancel installation) of Ember Choir");
    assert!(asked(&mut page).is_empty());
    press(&mut page, &[Down, A]);
    assert_eq!(page.focus_line(), "TileMenu: Menu(Cancel installation) of Ember Choir");
    press(&mut page, &[A, A]);
    assert_eq!(asked(&mut page), [Effect::Steer { game: ember, steer: Steer::Cancel }]);
    assert_eq!(after(&mut page, &[]), "Row(1) of Ember Choir");
}

/// Glasswing Atlas installed, three of its twelve Workshop items in (#188), before Dune Cartographer's download.
fn workshop(library: &Library) -> Queue {
    let extras = InstallState::Extras { items_done: 3, items: 12, done: 0, total: 0 };
    let fetching = Queued { can: STEERS, ..Queued::new(id(library, "Glasswing Atlas"), extras, 5) };
    Queue::new(vec![fetching, downloading(library)], Vec::new())
}

/// #188: a game installed whose Workshop items are coming in reads so, with its items, its share by them and no
/// speed; its menu is an installed game's, Play first and no Pause or Cancel, whatever its store can.
#[test]
fn a_workshop_row_says_what_it_is_and_its_game_is_played_as_an_installed_one() {
    let mut page = on_page(with_queue(library(), workshop));
    press(&mut page, &[Down]);
    let line = page.focus_line();
    assert!(line.ends_with("; Downloading Workshop content 3 of 12 items 25.0%"), "{line}");
    assert!(line.starts_with("ManageQueue: Row(0) of Glasswing Atlas"), "{line}");
    press(&mut page, &[View]);
    assert_eq!(page.focus_line(), "TileMenu: Menu(Play game) of Glasswing Atlas");
    press(&mut page, &[Down]);
    assert_ne!(page.focus_line(), "TileMenu: Menu(Pause installation) of Glasswing Atlas");
}

/// A store that cannot steer its installs (Steam with its debugger off): the row's menu is the Bundle's.
#[test]
fn a_rows_menu_has_no_pause_or_cancel_where_its_store_cannot() {
    let library = with_queue(library(), |library| {
        Queue::new(vec![Queued { can: Can::NOTHING, ..downloading(library) }], Vec::new())
    });
    let mut page = on_page(library);
    press(&mut page, &[Down, View, Down]);
    assert_eq!(page.focus_line(), "TileMenu: Menu(Add to Favourites) of Dune Cartographer");
}

#[test]
fn a_opens_the_games_hub_after_the_double_taps_time_and_a_second_a_plays_it() {
    let library = with_queue(library(), several);
    let mut page = on_page(library.clone());
    press(&mut page, &[A]);
    assert_eq!(page.page(), Some("ManageQueue"));
    page.advance(screens::home::DOUBLE_TAP.as_secs_f32());
    assert_eq!(page.page(), Some("GameHub"));
    // B on the hub: the page as it was.
    assert_eq!(after(&mut page, &[B]), "Cover(0) of Ashfall Courier");
    press(&mut page, &[A, A]);
    let place = library.games.iter().position(|game| game.name == "Ashfall Courier").unwrap();
    assert_eq!(asked(&mut page), [Effect::Launch(place)]);
    // A row's game, being installed, has its hub too.
    press(&mut page, &[Down, A]);
    page.advance(screens::home::DOUBLE_TAP.as_secs_f32());
    assert!(page.focus_line().starts_with("GameHub: 3003 Hub Installing"), "{}", page.focus_line());
    assert_eq!(after(&mut page, &[B]), "Row(0) of Dune Cartographer");
}

/// The three ways in, and B back to each with focus where it was.
#[test]
fn b_goes_back_to_where_manage_queue_was_opened_from() {
    let library = with_queue(library(), several);
    // Manage's Queue box.
    let mut dashboard = Dashboard::new(library.clone());
    dashboard.open_page(Open::MyGames(Entry::Manage));
    press(&mut dashboard, &[A]);
    assert_eq!(after(&mut dashboard, &[Right]), "Cover(1) of Brass Meridian");
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.focus_line(), "MyGames: Manage: Queue");

    // Home's Downloads tile.
    let mut dashboard = Dashboard::new(library.clone());
    press(&mut dashboard, &[Down, Down, Right, Right, Right]);
    assert_eq!(dashboard.home_focus(), Focus::Wide(3));
    press(&mut dashboard, &[A]);
    assert_eq!((dashboard.page(), dashboard.pages_up()), (Some("ManageQueue"), 1));
    press(&mut dashboard, &[Down, B]);
    assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::Wide(3)));

    // A Game hub's Queue action: the hub of the Recent row's first game, Dune Cartographer, being installed.
    let mut dashboard = Dashboard::new(library);
    press(&mut dashboard, &[Down, A]);
    dashboard.advance(screens::home::DOUBLE_TAP.as_secs_f32());
    press(&mut dashboard, &[Right, Right]);
    assert_eq!(dashboard.focus_line(), "GameHub: 3003 Hub Queue");
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.page(), Some("ManageQueue"));
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.focus_line(), "GameHub: 3003 Hub Queue");
}

// --- What it looks like ---

fn target_dir() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

/// The test fonts, before anything loads a font.
fn fonts() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        std::env::set_var("DASHBOARD_FONTS", fonts);
    });
}

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once.
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
    fn new(library: Library, open: Open) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, (1920, 1080));
        let mut dashboard = Dashboard::new(library);
        dashboard.set_clock(Some("9:41 AM".into()));
        dashboard.open_page(open);
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn press(&mut self, buttons: &[Button]) {
        press(&mut self.dashboard, buttons);
    }

    /// The screen at rest, compared with its reference.
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

/// Nothing installed and nothing queued: the outlined box alone, nothing focused, no hint bar.
#[test]
fn manage_queue_empty() {
    Stage::new(library_where(|_, _| false), screens::manage_queue::open()).check("manage_queue_empty");
}

/// One download, focus on its row: its speed, amount and share, its bar a fiftieth full; Recently installed above.
#[test]
fn manage_queue_one_download() {
    let mut stage = Stage::new(
        with_queue(library(), |library| Queue::new(vec![downloading(library)], Vec::new())),
        screens::manage_queue::open(),
    );
    stage.press(&[Down]);
    stage.check("manage_queue_one_download");
}

/// A game installed whose Workshop items are coming in (#188), focus on its row: Downloading Workshop content, 3 of 12
/// items, 25.0% and its bar a quarter full, no speed; the download under it as before.
#[test]
fn manage_queue_workshop() {
    let mut stage = Stage::new(with_queue(library(), workshop), screens::manage_queue::open());
    stage.press(&[Down]);
    stage.check("manage_queue_workshop");
}

/// The same with the bytes of every item known: the items and their bytes, the share by the bytes.
#[test]
fn manage_queue_workshop_bytes() {
    let sized = |library: &Library| {
        let extras = InstallState::Extras { items_done: 3, items: 12, done: 82 * MB, total: 1229 * MB };
        Queue::new(vec![Queued::new(id(library, "Glasswing Atlas"), extras, 5)], Vec::new())
    };
    let mut stage = Stage::new(with_queue(library(), sized), screens::manage_queue::open());
    stage.press(&[Down]);
    stage.check("manage_queue_workshop_bytes");
}

/// The current download, one paused (Paused, with its amount and share, #170) and one waiting (Queued, 0.0%), focus
/// on the first cover.
#[test]
fn manage_queue_several() {
    Stage::new(with_queue(library(), several), screens::manage_queue::open()).check("manage_queue_several");
}

/// Down to the last row: the list scrolled the least that shows it.
#[test]
fn manage_queue_several_scrolled() {
    let mut stage = Stage::new(with_queue(library(), several), screens::manage_queue::open());
    stage.press(&[Down, Down, Down]);
    stage.check("manage_queue_several_scrolled");
}

/// The menu of the current download's row (#170): Pause installation and Cancel installation after Install.
#[test]
fn manage_queue_menu_pause() {
    let mut stage = Stage::new(with_queue(library(), several), screens::manage_queue::open());
    stage.press(&[Down, View, Down]);
    stage.check("manage_queue_menu_pause");
}

/// The paused row focused, its menu up: Resume installation in Pause's place.
#[test]
fn manage_queue_menu_resume() {
    let mut stage = Stage::new(with_queue(library(), several), screens::manage_queue::open());
    stage.press(&[Down, Down, View, Down]);
    stage.check("manage_queue_menu_resume");
}

/// Cancel installation asks first.
#[test]
fn manage_queue_cancel_prompt() {
    let mut stage = Stage::new(with_queue(library(), several), screens::manage_queue::open());
    stage.press(&[Down, View, Down, Down, A]);
    stage.check("manage_queue_cancel_prompt");
}

/// Nothing queued, a game just finished first among Recently installed; focus on the long-named cover, its title on
/// two lines.
#[test]
fn manage_queue_recently_installed_only() {
    let mut library = library();
    let glasswing = id(&library, "Glasswing Atlas");
    library.queue.installed = vec![Installed { game: glasswing, finished: 9 }];
    let mut stage = Stage::new(library, screens::manage_queue::open());
    stage.press(&[Right, Right, Right, Right]);
    stage.check("manage_queue_recently_installed_only");
}

/// Manage while the Queue holds something: its box counts it, the nav's Manage has its Queue line, and the current
/// download stands at the nav's foot.
#[test]
fn my_games_manage_queued() {
    Stage::new(with_queue(library(), several), Open::MyGames(Entry::Manage)).check("my_games_manage_queued");
}
