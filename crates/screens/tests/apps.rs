//! Apps, and games added by hand (docs/spec/apps.md): Settings' Games & apps, Add a game and Add an app worked with
//! the pad and text entry, each change carried out on a library database as the executable carries it out; the
//! Apps pane of My games & apps; an App kept out of everything that is a game's; and a game added by hand launched,
//! played and first in the Recent row after. Then the pictures, compared with the committed references
//! (crates/shots): `SHOTS=bless cargo test -p screens --test apps` accepts what is drawn now.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use model::by_hand::MANUAL;
use model::{Added, AppKind, ByHand, StoreApp};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::keyboard::Typed;
use screens::pages::Open;
use screens::{hint_bar, my_games, settings, Dashboard, Effect};
use ui::host::Offscreen;
use ui::Button::{self, Down, Left, Menu, Right, Up, A, B, Y};
use ui::{Host, Press};

/// When the tests' library came into its database.
const ADDED: u64 = 1_780_000_000;
const PROGRAM: &str = r"C:\Games\Tern\tern.exe";
const NOTEPAD: &str = r"C:\Windows\System32\notepad.exe";

/// The Dashboard over a library database, carrying out what it asks of the library as the executable does: the
/// change, then the snapshot after.
struct Shell {
    database: library::Database,
    dashboard: Dashboard,
    /// What the last presses asked of the shell, but for the sounds.
    effects: Vec<Effect>,
    /// What the database said of each change.
    done: Vec<String>,
}

impl Shell {
    fn new() -> Self {
        Self::on(library::Database::in_memory().unwrap())
    }

    fn on(mut database: library::Database) -> Self {
        database.sync("steam", &screens::fake_library(), ADDED).unwrap();
        let dashboard = Dashboard::new(database.snapshot().unwrap());
        Self { database, dashboard, effects: Vec::new(), done: Vec::new() }
    }

    /// Settings > Games & apps > Games & apps, focus on its first Row.
    fn games_and_apps(&mut self) {
        self.dashboard.open_page(Open::Settings(settings::Entry::Rail));
        self.press(&[Down, Down, Right, Down]);
        assert_eq!(self.focus(), "Settings: Games & apps: Games & apps");
        self.press(&[A]);
        assert!(self.focus().starts_with("Settings: GamesAndApps: Add a game; "), "{}", self.focus());
        self.effects.clear();
    }

    fn press(&mut self, buttons: &[Button]) {
        self.effects.clear();
        for &button in buttons {
            self.dashboard.press(Press { button, repeat: false });
            self.carry_out();
        }
    }

    /// Text entry, which is up, emptied of what it holds, then `text` typed and kept with Menu.
    fn type_in(&mut self, was: &str, text: &str) {
        assert!(self.focus().starts_with("TextEntry: "), "{}", self.focus());
        for _ in 0..was.chars().count() {
            self.dashboard.typed(Typed::Backspace);
        }
        self.dashboard.typed(Typed::Text(text));
        self.press(&[Menu]);
    }

    fn carry_out(&mut self) {
        for effect in self.dashboard.take_effects() {
            match &effect {
                Effect::ByHand(change) => {
                    let done = self.database.by_hand(change, ADDED + 100).unwrap_or_else(|e| format!("refused: {e:#}"));
                    self.done.push(done);
                    self.dashboard.set_library(self.database.snapshot().unwrap());
                }
                Effect::Sound(_) => continue,
                _ => {}
            }
            self.effects.push(effect);
        }
    }

    fn focus(&self) -> String {
        self.dashboard.focus_line()
    }

    /// Add a game or Add an app from Games & apps: its program typed, then its name, then Add.
    fn add(&mut self, row: usize, path: &str, name: &str) {
        self.press(&[Up, Up, Up]);
        self.press(&vec![Down; row]);
        self.press(&[A, A]);
        self.type_in("", path);
        self.press(&[Down]);
        let offered = self.dashboard_entry_offered();
        self.press(&[A]);
        self.type_in(&offered, name);
        self.press(&[Down, Down, A]);
    }

    /// What the name field holds now, by the page's state.
    fn dashboard_entry_offered(&self) -> String {
        let focus = self.focus();
        let name = focus.split("name \"").nth(1).and_then(|rest| rest.split('"').next()).unwrap_or_default();
        name.to_string()
    }

    fn game(&self, name: &str) -> Option<&model::Game> {
        let library = self.dashboard.library();
        library.games.iter().chain(&library.hidden).find(|game| game.name == name)
    }
}

// --- Settings: Games & apps ---

#[test]
fn games_and_apps_is_a_box_on_the_games_and_apps_category_with_the_two_that_add() {
    let mut shell = Shell::new();
    shell.games_and_apps();
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Add a game; 0 games, 0 apps");
    assert_eq!(shell.dashboard.hints(), hint_bar::NONE);
    shell.press(&[Down, Down]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Add an app; 0 games, 0 apps", "the foot stops");
    // Menu on a Row that adds is nothing.
    shell.press(&[Menu]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Add an app; 0 games, 0 apps");
    shell.press(&[A]);
    assert_eq!(shell.focus(), "Settings: AddApp: Path; path \"\", name \"\", picture \"\", not ready");
    shell.press(&[B]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Add an app; 0 games, 0 apps", "back where it was");
    shell.press(&[B]);
    assert_eq!(shell.focus(), "Settings: Games & apps: Games & apps");
}

/// The issue's own: a game added by typing its path is in the library as any game is, from the source `manual`.
#[test]
fn a_game_added_by_typing_its_path_is_a_game_of_the_manual_source() {
    let mut shell = Shell::new();
    shell.games_and_apps();
    shell.press(&[A]);
    assert_eq!(shell.focus(), "Settings: AddGame: Path; path \"\", name \"\", picture \"\", not ready");
    // Add is not there to press yet.
    shell.press(&[Down, Down, Down, A]);
    assert_eq!(shell.focus(), "Settings: AddGame: Add; path \"\", name \"\", picture \"\", not ready");
    assert!(shell.effects.is_empty());
    // The path, quoted as Explorer's Copy as path gives it: its name is offered.
    shell.press(&[Up, Up, Up, A]);
    shell.type_in("", &format!("\"{PROGRAM}\""));
    assert_eq!(
        shell.focus(),
        format!("Settings: AddGame: Path; path \"\"{PROGRAM}\"\", name \"tern\", picture \"\", ready")
    );
    shell.press(&[Down, A]);
    shell.type_in("tern", "Tern");
    shell.press(&[Down, Down]);
    assert_eq!(
        shell.focus(),
        format!("Settings: AddGame: Add; path \"\"{PROGRAM}\"\", name \"Tern\", picture \"\", ready")
    );
    shell.press(&[A]);
    let asked = ByHand::AddGame { path: PROGRAM.into(), name: "Tern".into(), art: None };
    assert_eq!(shell.effects, [Effect::ByHand(asked)]);
    assert_eq!(shell.done, ["added the game Tern"]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Add a game; 1 game, 0 apps", "back on the page");
    let game = shell.game("Tern").expect("in the library");
    assert_eq!((game.source.as_str(), game.source_id.as_str(), game.installed), (MANUAL, PROGRAM, true));
    shell.press(&[Down, Down]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Game Tern; 1 game, 0 apps");
    assert_eq!(shell.dashboard.hints(), hint_bar::MORE_OPTIONS);
}

#[test]
fn a_rows_menu_renames_and_removes_after_asking_and_focus_stays_in_its_place() {
    let mut shell = Shell::new();
    shell.games_and_apps();
    shell.add(0, PROGRAM, "Tern");
    shell.add(1, NOTEPAD, "Notepad");
    assert_eq!(shell.done, ["added the game Tern", "added the App Notepad"]);
    shell.press(&[Down, Down, Down]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: App Notepad; 1 game, 1 app");

    // Rename: text entry on its name, kept.
    shell.press(&[A]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Menu(Rename) of Notepad");
    shell.press(&[A]);
    shell.type_in("Notepad", "Notes");
    let notepad = shell.dashboard.library().apps[0].id;
    assert_eq!(shell.effects, [Effect::ByHand(ByHand::Rename { item: Added::App(notepad), name: "Notes".into() })]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: App Notes; 1 game, 1 app");
    // A name kept the same, or empty, is not asked for.
    shell.press(&[Menu, A]);
    shell.type_in("Notes", "Notes");
    shell.press(&[Menu, A]);
    shell.type_in("Notes", "  ");
    assert_eq!(shell.done.len(), 3);

    // Remove asks first; Cancel is back on the menu, B off it.
    shell.press(&[Menu, Down, A]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Prompt(Remove) of Notes");
    shell.press(&[Down, A]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Menu(Remove) of Notes");
    shell.press(&[B]);
    assert_eq!(shell.focus(), "Settings: GamesAndApps: App Notes; 1 game, 1 app");
    shell.press(&[Menu, Down, A, A]);
    assert_eq!(shell.effects, [Effect::ByHand(ByHand::Remove(Added::App(notepad)))]);
    assert_eq!(shell.done.last().unwrap(), "removed the App Notes");
    // Focus on the Row in its place: the game's, now last.
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Game Tern; 1 game, 0 apps");
    assert!(shell.dashboard.library().apps.is_empty());

    // The game, renamed and removed the same way.
    shell.press(&[A, A]);
    shell.type_in("Tern", "Tern Again");
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Game Tern Again; 1 game, 0 apps");
    assert!(shell.game("Tern Again").is_some());
    shell.press(&[Menu, Down, A, A]);
    assert_eq!(shell.done.last().unwrap(), "removed the game Tern Again");
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Add an app; 0 games, 0 apps");
    assert!(shell.game("Tern Again").is_none());
}

#[test]
fn what_was_added_is_there_after_the_database_is_opened_again() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("apps-persist");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("library.db");
    {
        let mut shell = Shell::on(library::Database::open(&path).unwrap());
        shell.games_and_apps();
        shell.add(0, PROGRAM, "Tern");
        shell.add(1, NOTEPAD, "Notepad");
        // From Add an app, where adding left focus, to the game's Row.
        shell.press(&[Down, A, A]);
        shell.type_in("Tern", "Tern Renamed");
        assert_eq!(shell.done.len(), 3, "{:?}", shell.done);
    }
    let mut shell = Shell::on(library::Database::open(&path).unwrap());
    let game = shell.game("Tern Renamed").expect("the game, renamed");
    assert_eq!((game.source.as_str(), game.source_id.as_str()), (MANUAL, PROGRAM));
    let apps = &shell.dashboard.library().apps;
    assert_eq!(apps.len(), 1);
    assert_eq!((apps[0].name.as_str(), apps[0].kind, apps[0].target.as_str()), ("Notepad", AppKind::Program, NOTEPAD));
    shell.games_and_apps();
    assert_eq!(shell.focus(), "Settings: GamesAndApps: Add a game; 1 game, 1 app");
}

#[test]
fn a_store_app_is_ticked_to_add_it_and_unticked_to_remove_it() {
    let mut shell = Shell::new();
    let calculator = StoreApp { name: "Calculator".into(), aumid: "Calc_8wekyb3d8bbwe!App".into(), logo: None };
    let photos = StoreApp { name: "Photos".into(), aumid: "Photos_8wekyb3d8bbwe!App".into(), logo: None };
    let mut library = shell.database.snapshot().unwrap();
    // Not listed yet: Add is the page's last control.
    shell.games_and_apps();
    shell.press(&[Down, A, Down, Down, Down, Down]);
    assert!(shell.focus().starts_with("Settings: AddApp: Add; "), "{}", shell.focus());
    shell.press(&[B]);
    // Listed: a Row each under Add.
    library.store_apps = Some(vec![calculator.clone(), photos]);
    shell.dashboard.set_library(library.clone());
    shell.press(&[A, Down, Down, Down, Down, Down]);
    assert!(shell.focus().starts_with("Settings: AddApp: Store(1); "), "{}", shell.focus());
    shell.press(&[Up, A]);
    assert_eq!(shell.effects, [Effect::ByHand(ByHand::AddStoreApp(calculator))]);
    assert!(shell.focus().starts_with("Settings: AddApp: Store(0); "), "focus stays: {}", shell.focus());
    let mut snapshot = shell.database.snapshot().unwrap();
    let app = snapshot.apps[0].clone();
    assert_eq!((app.name.as_str(), app.kind), ("Calculator", AppKind::Store));
    assert_eq!(app.start(), model::Start::Address(r"shell:AppsFolder\Calc_8wekyb3d8bbwe!App".into()));
    // Ticked now: A unticks it.
    snapshot.store_apps = library.store_apps.clone();
    shell.dashboard.set_library(snapshot);
    shell.press(&[A]);
    assert_eq!(shell.effects, [Effect::ByHand(ByHand::Remove(Added::App(app.id)))]);
    assert!(shell.database.snapshot().unwrap().apps.is_empty());
}

// --- My games & apps: Apps ---

/// Longer than the nav's settle wait: a step along the nav after it is a step on its own.
fn rest() {
    std::thread::sleep(Duration::from_millis(170));
}

/// My games & apps' Apps, focus on its item in the nav.
fn apps_pane(shell: &mut Shell) {
    shell.dashboard.open_page(Open::MyGames(my_games::Entry::Games));
    shell.press(&[Left]);
    rest();
    shell.press(&[Down]);
    assert!(shell.focus().starts_with("MyGames: Apps: Nav(Apps)"), "{}", shell.focus());
}

/// A shell with a game added by hand and seven Apps: five to a row, so two rows.
fn with_apps() -> Shell {
    let mut shell = Shell::new();
    let mut add = |change: ByHand| shell.database.by_hand(&change, ADDED + 100).unwrap();
    add(ByHand::AddGame { path: PROGRAM.into(), name: "Tern".into(), art: None });
    for name in ["Notepad", "Paint", "Terminal", "Recorder", "Clock", "Maps", "Mail"] {
        add(ByHand::AddApp { path: format!(r"C:\Apps\{name}.exe").into(), name: name.into(), icon: None });
    }
    let library = shell.database.snapshot().unwrap();
    shell.dashboard.set_library(library);
    shell
}

#[test]
fn the_apps_pane_is_a_grid_of_five_whose_a_starts_the_app() {
    let mut shell = with_apps();
    apps_pane(&mut shell);
    shell.press(&[Right]);
    assert_eq!(shell.focus(), "MyGames: Apps: Tile(0); \"Notepad\", 7 apps");
    assert_eq!(shell.dashboard.hints(), hint_bar::NONE, "no hints");
    shell.press(&[Right, Right, Right, Right, Right]);
    assert_eq!(shell.focus(), "MyGames: Apps: Tile(4); \"Clock\", 7 apps", "the row's end stops");
    // Down onto the shorter last row: its last tile.
    shell.press(&[Down]);
    assert_eq!(shell.focus(), "MyGames: Apps: Tile(6); \"Mail\", 7 apps");
    shell.press(&[Down, Left]);
    assert_eq!(shell.focus(), "MyGames: Apps: Tile(5); \"Maps\", 7 apps", "the foot stops");
    shell.press(&[Up, Up]);
    assert_eq!(shell.focus(), "MyGames: Apps: Tile(0); \"Notepad\", 7 apps", "the top stops");
    // A starts the App: nothing else is asked, nothing is up after.
    shell.press(&[Right, A]);
    let paint = shell.dashboard.library().apps[1].id;
    assert_eq!(shell.effects, [Effect::StartApp(paint)]);
    assert_eq!(shell.focus(), "MyGames: Apps: Tile(1); \"Paint\", 7 apps");
    // No Tile menu, and Y is nothing.
    shell.press(&[Menu, Y]);
    assert!(shell.effects.is_empty(), "{:?}", shell.effects);
    shell.press(&[Left, Left]);
    assert_eq!(shell.focus(), "MyGames: Apps: Nav(Apps); \"Notepad\", 7 apps", "the App last focused is said");
}

#[test]
fn with_no_apps_focus_stays_on_the_nav() {
    let mut shell = Shell::new();
    apps_pane(&mut shell);
    shell.press(&[Right, A]);
    assert_eq!(shell.focus(), "MyGames: Apps: Nav(Apps)");
    assert!(shell.effects.is_empty());
}

/// The issue's own: an App is not a game. It is in no Group, not in the Recent row, not in Search's Games row; it
/// starts with no Launch screen and no Play session.
#[test]
fn an_app_is_never_in_recent_a_group_or_searchs_games() {
    let mut shell = with_apps();
    let library = shell.dashboard.library().clone();
    let names = |games: &[model::Game]| games.iter().map(|game| game.name.clone()).collect::<Vec<_>>();
    for app in &library.apps {
        assert!(!names(&library.games).contains(&app.name), "{} is not a game", app.name);
        assert!(!names(&library.hidden).contains(&app.name));
    }
    // Every Group's games are games of the library.
    for filter in &library.filters {
        for &id in &filter.games {
            let game = library.game(id).expect("a Group's game is a game");
            assert!(library.apps.iter().all(|app| game.name != app.name), "{} in {}", game.name, filter.name);
        }
    }
    // Started, it changes nothing of the library: no Play session, no Recent.
    let recent = names(library.recent(10));
    apps_pane(&mut shell);
    shell.press(&[Right, A]);
    assert!(matches!(shell.effects[..], [Effect::StartApp(_)]));
    assert_eq!(names(shell.dashboard.library().recent(10)), recent);
    assert!(!shell.dashboard.launch_screen_showing());
    // Search: Notepad is no game, and Tern, added by hand, is.
    shell.press(&[B]);
    shell.press(&[Y]);
    assert!(shell.focus().starts_with("Search: "), "{}", shell.focus());
    shell.dashboard.typed(Typed::Text("Notepad"));
    assert!(shell.focus().ends_with("no results"), "{}", shell.focus());
    for _ in 0.."Notepad".len() {
        shell.dashboard.typed(Typed::Backspace);
    }
    shell.dashboard.typed(Typed::Text("Tern"));
    shell.press(&[B]);
    assert_eq!(shell.focus(), "Search: Games(0) Tern; \"Tern\" away, Games 1");
}

/// The issue's own: a game added by hand launches as any game: the Launch screen, a Play session, and first in the
/// Recent row after.
#[test]
fn a_game_added_by_hand_launches_is_played_and_is_first_in_recent() {
    let mut shell = with_apps();
    let game = shell.game("Tern").unwrap().clone();
    let place = shell.dashboard.library().games.iter().position(|g| g.id == game.id).unwrap();
    // From Search, its Game hub, and Play.
    shell.dashboard.open_page(Open::Search);
    shell.dashboard.typed(Typed::Text("Tern"));
    shell.press(&[B, A]);
    assert_eq!(shell.focus(), format!("GameHub: {PROGRAM} Hub Play"));
    shell.press(&[A]);
    assert_eq!(shell.effects, [Effect::Launch(place)]);
    // What the source starts: the program, as typed.
    let start = model::Source::start(&library::by_hand::Manual, &game);
    assert_eq!(start, Some(model::by_hand::program(Path::new(PROGRAM))));

    // The shell's part, as the executable does it.
    shell.dashboard.set_overlay(true);
    shell.dashboard.launch_started(place);
    assert!(shell.dashboard.launch_screen_showing(), "the Launch screen");
    shell.dashboard.game_booted();
    // Later than any game of the made-up library was played.
    let session = shell.database.session_started(game.id, 1_800_000_000).unwrap();
    shell.database.session_ended(session, 1_800_000_600).unwrap();
    let after = shell.database.snapshot().unwrap();
    shell.dashboard.set_library(after);
    shell.dashboard.game_ended();
    assert_eq!(shell.dashboard.library().recent(1)[0].name, "Tern");
    assert!(shell.focus().ends_with("Recent(0)"), "Home, on its tile: {}", shell.focus());
    assert_eq!(shell.database.sessions(game.id).unwrap().len(), 1);
}

// --- What needs a picture ---

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

/// Pictures for the Apps, drawn once: a program's icon, a round shape on a clear ground so its shadow shows, and a
/// Store app's tile art, a plain square on clear ground as a package's is.
fn pictures() -> (PathBuf, PathBuf) {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("apps-art");
        std::fs::create_dir_all(&dir).unwrap();
        let icon = image::RgbaImage::from_fn(256, 256, |x, y| {
            let (dx, dy) = (x as f32 - 127.5, y as f32 - 127.5);
            let d = (dx * dx + dy * dy).sqrt();
            match d {
                d if d < 70.0 => image::Rgba([250, 200, 40, 255]),
                d if d < 110.0 => image::Rgba([40, 110, 220, 255]),
                _ => image::Rgba([0, 0, 0, 0]),
            }
        });
        icon.save(dir.join("icon.png")).unwrap();
        let logo = image::RgbaImage::from_fn(300, 300, |x, y| {
            let inside = (90..210).contains(&x) && (90..210).contains(&y);
            if inside {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            }
        });
        logo.save(dir.join("logo.png")).unwrap();
        dir
    });
    (dir.join("icon.png"), dir.join("logo.png"))
}

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once on as many threads.
static DRAWING: Mutex<()> = Mutex::new(());

const HD: (u32, u32) = (1920, 1080);

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    shell: Shell,
    /// Held until the fields above, the device among them, are gone.
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn new(shell: Shell) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, HD);
        let mut shell = shell;
        shell.dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, shell, _drawing: drawing }
    }

    /// Apps of each face: a program with an icon, one with none, a Store app with its art, and one with none; and a
    /// game added by hand, with the Store apps listed for Add an app.
    fn with_apps() -> Self {
        let (icon, logo) = pictures();
        let mut shell = Shell::new();
        let mut add = |change: ByHand| shell.database.by_hand(&change, ADDED + 100).unwrap();
        add(ByHand::AddGame { path: PROGRAM.into(), name: "Tern".into(), art: None });
        add(ByHand::AddApp { path: r"C:\Tools\Orbit\orbit.exe".into(), name: "Orbit".into(), icon: Some(icon) });
        add(ByHand::AddApp { path: NOTEPAD.into(), name: "Notepad".into(), icon: None });
        let calculator = StoreApp { name: "Calculator".into(), aumid: "Calc!App".into(), logo: Some(logo) };
        let photos = StoreApp { name: "Photos and a name that runs on".into(), aumid: "Photos!App".into(), logo: None };
        add(ByHand::AddStoreApp(calculator.clone()));
        add(ByHand::AddStoreApp(photos.clone()));
        let clock = StoreApp { name: "Clock".into(), aumid: "Clock!App".into(), logo: None };
        let mut library = shell.database.snapshot().unwrap();
        library.store_apps = Some(vec![calculator, clock, photos]);
        shell.dashboard.set_library(library);
        Self::new(shell)
    }

    fn press(&mut self, buttons: &[Button]) {
        self.shell.press(buttons);
    }

    fn check(&mut self, name: &str) {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, shell, .. } = self;
        let pixels = target.still(gpu, host, |ui| shell.dashboard.draw_all(ui));
        let image = image::RgbaImage::from_raw(HD.0, HD.1, pixels).unwrap();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }
}

/// My games & apps' Apps with Apps in it, focus on the nav; then on a program's tile, a Store app's, and one with
/// a long name. The Bundle has no picture of this (docs/spec/apps.md, 2).
#[test]
#[ignore = "screenshot"]
fn my_games_apps() {
    let mut stage = Stage::with_apps();
    apps_pane(&mut stage.shell);
    stage.check("my-games-apps");
    stage.press(&[Right]);
    stage.check("my-games-apps-focused");
    stage.press(&[Right, Right]);
    stage.check("my-games-apps-focused-store");
    stage.press(&[Right]);
    stage.check("my-games-apps-focused-long-name");
}

/// The Games & apps category, focus on its Games & apps box; that page with a game and Apps; a Row's menu.
#[test]
#[ignore = "screenshot"]
fn settings_games_and_apps() {
    let mut stage = Stage::with_apps();
    stage.shell.dashboard.open_page(Open::Settings(settings::Entry::Rail));
    stage.press(&[Down, Down, Right, Down]);
    stage.check("settings-games-and-apps-boxes");
    stage.press(&[A, Down, Down, Down]);
    assert_eq!(stage.shell.focus(), "Settings: GamesAndApps: App Orbit; 1 game, 4 apps");
    stage.check("settings-games-and-apps");
    stage.press(&[A]);
    stage.check("settings-games-and-apps-menu");
}

/// Add a game empty, then filled in with focus on Add; Add an app with the Store apps under it.
#[test]
#[ignore = "screenshot"]
fn settings_add_pages() {
    let mut stage = Stage::with_apps();
    stage.shell.games_and_apps();
    stage.press(&[A]);
    stage.check("settings-add-a-game");
    stage.press(&[A]);
    stage.shell.type_in("", r"C:\Games\Lantern Keeper\Binaries\Win64\LanternKeeper-Win64-Shipping.exe");
    stage.press(&[Down, Down, A]);
    stage.shell.type_in("", r"C:\Art\lantern.png");
    stage.press(&[Down]);
    stage.check("settings-add-a-game-ready");
    stage.press(&[B, Down, A, Down, Down, Down, Down]);
    assert!(stage.shell.focus().starts_with("Settings: AddApp: Store(0); "), "{}", stage.shell.focus());
    stage.check("settings-add-an-app-store-apps");
}
