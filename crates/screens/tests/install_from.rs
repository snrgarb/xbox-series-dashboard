//! "Install from", the question an Install asks of a game the library has, not installed, from other launchers that
//! can install it (docs/spec/install-from.md, #249): where focus goes and what a press asks for, with no picture; then
//! drawn offscreen and set against the committed reference (`SHOTS=bless cargo test -p screens --test install_from`
//! accepts what is drawn now). The other launchers' copies are made up here, as their sources would read them.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use model::storage::{Place, StoreFolders};
use model::{Game, Library, Storage};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::{Dashboard, Effect, Sound};
use ui::host::Offscreen;
use ui::Button::{self, Down, Menu, Right, Up, A, B};
use ui::{Host, Press};

const NOW: u64 = 1_780_000_000;

/// A game of another store's, owned and not installed, as its source reads it.
fn copy(source_id: &str, name: &str) -> Library {
    let game = Game { source_id: source_id.into(), name: name.into(), ..Game::default() };
    Library { games: vec![game], ..Library::default() }
}

/// The fake library as the library database gives it, its games Steam's; Barlin's Gate 3, not installed, also on
/// Epic (its name in capitals with a trademark sign), GOG and the EA app; and which of them their stores can install
/// as the executable reads it: every one but the EA app's.
fn library() -> Library {
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &screens::fake_library(), NOW).unwrap();
    database.sync("epic", &copy("BarlinsGate", "BARLIN'S GATE™ 3"), NOW).unwrap();
    database.sync("gog", &copy("1207658930", "Barlin's Gate 3"), NOW).unwrap();
    database.sync("ea", &copy("Origin.OFR.50.0001", "Barlin's Gate 3"), NOW).unwrap();
    let mut library = database.snapshot().unwrap();
    let installable = library.games.iter().filter(|game| model::names::same_name(&game.name, "Barlin's Gate 3"));
    let installable = installable.filter(|game| game.source != "ea").map(|game| game.id).collect();
    library.storage = Storage { installable, ..Storage::default() };
    library
}

fn id(library: &Library, source: &str) -> i64 {
    let barlins = |game: &&Game| model::names::normalised_name(&game.name) == "barlins gate 3";
    library.games.iter().filter(barlins).find(|game| game.source == source).unwrap().id
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

fn pressed(dashboard: &mut Dashboard, buttons: &[Button]) -> Vec<Effect> {
    press(dashboard, buttons);
    dashboard.take_effects().into_iter().filter(|effect| !matches!(effect, Effect::Sound(_))).collect()
}

/// The Tile menu of the Recent row's sixth game, Steam's Barlin's Gate 3, which is not installed.
fn install_menu(library: Library) -> Dashboard {
    let mut dashboard = Dashboard::new(library);
    press(&mut dashboard, &[Down, Right, Right, Right, Right, Right, Menu]);
    assert_eq!(dashboard.focus_line(), "TileMenu: Menu(Install) of Barlin's Gate 3");
    dashboard.take_effects();
    dashboard
}

#[test]
fn install_asks_which_launcher_focus_on_the_one_pressed_and_a_installs_from_the_one_chosen() {
    let mut dashboard = install_menu(library());
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.focus_line(), "InstallFrom: Launcher(0) Steam, the one pressed of Barlin's Gate 3");
    // Install was a menu's row that goes on: the menu's choice sound, then the question in its place.
    assert_eq!(dashboard.take_effects(), [Effect::Sound(Sound::GuideSelect)]);
    press(&mut dashboard, &[Up]);
    assert_eq!(dashboard.take_effects(), [], "the top stops, silent");
    press(&mut dashboard, &[Down]);
    assert_eq!(dashboard.take_effects(), [Effect::Sound(Sound::GuideMove)]);
    assert_eq!(dashboard.focus_line(), "InstallFrom: Launcher(1) Epic Games of Barlin's Gate 3");
    press(&mut dashboard, &[Down]);
    assert_eq!(dashboard.focus_line(), "InstallFrom: Launcher(2) GOG of Barlin's Gate 3");
    press(&mut dashboard, &[Down]);
    assert_eq!(dashboard.focus_line(), "InstallFrom: Launcher(2) GOG of Barlin's Gate 3", "no EA app: it can't");
    dashboard.take_effects();
    press(&mut dashboard, &[Up, A]);
    let epic = id(dashboard.library(), "epic");
    let sounds = [Effect::Sound(Sound::GuideMove), Effect::Sound(Sound::PromptClose)];
    assert_eq!(dashboard.take_effects(), [&sounds[..], &[Effect::Install(epic)]].concat());
    assert_eq!(dashboard.page(), None);
}

#[test]
fn a_on_the_one_pressed_installs_it_as_before_and_b_installs_nothing() {
    let mut dashboard = install_menu(library());
    let steam = id(dashboard.library(), "steam");
    assert_eq!(pressed(&mut dashboard, &[A, A]), [Effect::Install(steam)]);
    let mut dashboard = install_menu(library());
    assert_eq!(pressed(&mut dashboard, &[A, Down, B]), []);
    assert_eq!(dashboard.page(), None);
}

#[test]
fn the_launcher_chosen_asks_its_folder_where_it_has_more_than_one() {
    let mut library = library();
    let place = |path: &str| Place { path: path.into(), drive: None };
    library.storage.stores = vec![StoreFolders {
        store: "gog".into(),
        name: "GOG".into(),
        folders: vec![place(r"C:\GOG Games"), place(r"D:\GOG Games")],
        choose: true,
        default: 1,
    }];
    let mut dashboard = install_menu(library);
    press(&mut dashboard, &[A, Down, Down, A]);
    assert_eq!(dashboard.focus_line(), r"InstallWhere: Folder(1) D:\GOG Games, the default of Barlin's Gate 3");
    let gog = id(dashboard.library(), "gog");
    let install = Effect::InstallTo { game: gog, folder: PathBuf::from(r"D:\GOG Games") };
    assert_eq!(pressed(&mut dashboard, &[A]), [install]);
}

#[test]
fn from_the_game_hub_too() {
    let mut dashboard = install_menu(library());
    // The menu's Manage game: the hub, whose main action is Install.
    press(&mut dashboard, &[Down, Down]);
    assert_eq!(dashboard.focus_line(), "TileMenu: Menu(Manage game) of Barlin's Gate 3");
    press(&mut dashboard, &[A]);
    assert!(dashboard.focus_line().starts_with("GameHub: "), "{}", dashboard.focus_line());
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.focus_line(), "InstallFrom: Launcher(0) Steam, the one pressed of Barlin's Gate 3");
}

#[test]
fn a_game_in_one_launcher_only_or_installed_elsewhere_with_no_other_asks_nothing() {
    // Only Steam's can be installed: no question.
    let mut alone = library();
    alone.storage.installable = vec![id(&alone, "steam")];
    let mut dashboard = install_menu(alone);
    let steam = id(dashboard.library(), "steam");
    assert_eq!(pressed(&mut dashboard, &[A]), [Effect::Install(steam)]);
    assert_eq!(dashboard.page(), None);
    // Installed on Epic and GOG: Steam's Install is still offered, and asks nothing.
    let mut installed = library();
    for game in installed.games.iter_mut().filter(|game| game.source == "epic" || game.source == "gog") {
        game.installed = true;
    }
    let mut dashboard = install_menu(installed);
    assert_eq!(pressed(&mut dashboard, &[A]), [Effect::Install(steam)]);
}

// ------------------------------------------------------------------------------------------------------ pictures

fn target_dir() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once.
static DRAWING: Mutex<()> = Mutex::new(());

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn new(dashboard: Dashboard) -> Self {
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var("DASHBOARD_FONTS", fonts);
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, (1920, 1080));
        let mut dashboard = dashboard;
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

#[test]
fn install_from() {
    let mut dashboard = install_menu(library());
    press(&mut dashboard, &[A]);
    let mut stage = Stage::new(dashboard);
    stage.check("install-from");
    press(&mut stage.dashboard, &[Down]);
    stage.check("install-from-other-launcher");
}
