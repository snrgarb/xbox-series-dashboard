//! Settings > Storage, the question "Where do you want to install X?" and Manage's storage ring
//! (docs/spec/storage.md, #179): where focus goes and what a press asks for, with no picture; then each drawn
//! offscreen and set against the committed references (`SHOTS=bless cargo test -p screens --test storage` accepts
//! what is drawn now). The drives and folders are made up here, as the shell's `model::Drives` would give them.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use model::storage::{Drive, Place, StoreFolders};
use model::{Library, Storage};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::my_games::Entry;
use screens::pages::Open;
use screens::settings;
use screens::{Dashboard, Effect, Sound};
use ui::host::Offscreen;
use ui::Button::{self, Down, Left, Menu, Right, Up, A, B};
use ui::{Host, Press};

const GB: u64 = 1024 * 1024 * 1024;

fn drive(root: &str, label: &str, free: u64, total: u64) -> Drive {
    Drive { root: root.into(), label: label.into(), free, total }
}

fn system() -> Drive {
    drive("C:\\", "", 354 * GB + 6 * GB / 10, 802 * GB)
}

fn games() -> Drive {
    drive("D:\\", "Games", 1210 * GB, 1863 * GB)
}

/// Two drives; Steam with two libraries, which the user may choose between, its own on C: the default; Battle.net
/// with one folder of its own choosing.
fn storage() -> Storage {
    let place = |path: &str, drive: Drive| Place { path: path.into(), drive: Some(drive) };
    let steam = StoreFolders {
        store: "steam".into(),
        name: "Steam".into(),
        folders: vec![place(r"D:\SteamLibrary", games()), place(r"C:\Program Files (x86)\Steam", system())],
        choose: true,
        default: 1,
    };
    let battlenet = StoreFolders {
        store: "battlenet".into(),
        name: "Battle.net".into(),
        folders: vec![place(r"C:\Program Files (x86)", system())],
        choose: false,
        default: 0,
    };
    Storage {
        drives: vec![system(), games()],
        stores: vec![steam, battlenet],
        ring: Some(system()),
        ..Storage::default()
    }
}

/// The fake library as the library database gives it, its games Steam's, with Storage as the shell would carry it.
fn library(storage: Storage) -> Library {
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &screens::fake_library(), 1_780_000_000).unwrap();
    Library { storage, ..database.snapshot().unwrap() }
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

/// Settings > Storage, opened from the Games & apps category's Installs section.
fn storage_page(library: Library) -> Dashboard {
    let mut dashboard = Dashboard::new(library);
    dashboard.open_page(Open::Settings(settings::Entry::Tile));
    press(&mut dashboard, &[B, Down, Down, Right, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Storage");
    press(&mut dashboard, &[A]);
    dashboard
}

/// The Tile menu of the Recent row's sixth game, Barlin's Gate 3, which is not installed.
fn install_menu(library: Library) -> Dashboard {
    let mut dashboard = Dashboard::new(library);
    press(&mut dashboard, &[Down, Right, Right, Right, Right, Right, Menu]);
    assert_eq!(dashboard.focus_line(), "TileMenu: Menu(Install) of Barlin's Gate 3");
    dashboard.take_effects();
    dashboard
}

// ------------------------------------------------------------------------------------------------ Settings > Storage

#[test]
fn storage_lists_each_stores_folders_focus_on_the_first_stores_ticked_one_and_asks_for_the_drives_again() {
    let mut dashboard = storage_page(library(storage()));
    assert_eq!(dashboard.focus_line(), r"Settings: Storage: Steam C:\Program Files (x86)\Steam; the default; 2 drives");
    assert!(dashboard.take_effects().contains(&Effect::LookAtStorage), "the drives are read again as it opens");
    // Down through every store's folders, the foot stopping; Up back to the top, which stops too.
    press(&mut dashboard, &[Down]);
    assert_eq!(dashboard.focus_line(), r"Settings: Storage: Battle.net C:\Program Files (x86); the default; 2 drives");
    press(&mut dashboard, &[Down, Up, Up, Up]);
    assert_eq!(dashboard.focus_line(), r"Settings: Storage: Steam D:\SteamLibrary; not the default; 2 drives");
    // B is Games & apps again, on Storage.
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Storage");
}

#[test]
fn a_on_a_folder_of_a_store_that_can_be_told_makes_it_the_default_and_nothing_elsewhere() {
    let mut dashboard = storage_page(library(storage()));
    dashboard.take_effects();
    press(&mut dashboard, &[Up]);
    let chose = Effect::DefaultFolder { store: "steam".into(), folder: PathBuf::from(r"D:\SteamLibrary") };
    assert_eq!(pressed(&mut dashboard, &[A]), [chose]);
    assert_eq!(dashboard.focus_line(), r"Settings: Storage: Steam D:\SteamLibrary; the default; 2 drives");
    // The ticked one again, and a store that chooses for itself: nothing.
    assert_eq!(pressed(&mut dashboard, &[A]), []);
    press(&mut dashboard, &[Down, Down]);
    assert_eq!(pressed(&mut dashboard, &[A]), []);
}

#[test]
fn with_no_store_that_names_its_folders_focus_has_nowhere_to_go() {
    let mut dashboard = storage_page(library(Storage { drives: vec![system()], ..Storage::default() }));
    assert_eq!(dashboard.focus_line(), "Settings: Storage: Nothing; 1 drive");
    press(&mut dashboard, &[Down, Up, A]);
    assert_eq!(dashboard.focus_line(), "Settings: Storage: Nothing; 1 drive");
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Storage");
}

#[test]
fn a_new_snapshot_keeps_focus_on_the_same_folder() {
    let mut dashboard = storage_page(library(storage()));
    press(&mut dashboard, &[Down]);
    let mut moved = storage();
    moved.stores.swap(0, 1);
    dashboard.set_library(library(moved));
    assert_eq!(dashboard.focus_line(), r"Settings: Storage: Battle.net C:\Program Files (x86); the default; 2 drives");
}

// ------------------------------------------------------------------------------------------------- which folder

#[test]
fn install_asks_which_folder_focus_on_the_default_and_a_installs_there() {
    let mut dashboard = install_menu(library(storage()));
    let game = dashboard.library().games[5].id;
    press(&mut dashboard, &[A]);
    assert_eq!(
        dashboard.focus_line(),
        r"InstallWhere: Folder(1) C:\Program Files (x86)\Steam, the default of Barlin's Gate 3"
    );
    // Install was a menu's row that goes on: the menu's choice sound, then the question in its place.
    assert_eq!(dashboard.take_effects(), [Effect::Sound(Sound::GuideSelect)]);
    press(&mut dashboard, &[Down]);
    assert_eq!(
        dashboard.focus_line(),
        r"InstallWhere: Folder(1) C:\Program Files (x86)\Steam, the default of Barlin's Gate 3",
        "the foot stops"
    );
    assert_eq!(dashboard.take_effects(), [], "a move that goes nowhere is silent");
    press(&mut dashboard, &[Up]);
    assert_eq!(dashboard.take_effects(), [Effect::Sound(Sound::GuideMove)]);
    assert_eq!(dashboard.focus_line(), r"InstallWhere: Folder(0) D:\SteamLibrary of Barlin's Gate 3");
    press(&mut dashboard, &[A]);
    let install = Effect::InstallTo { game, folder: PathBuf::from(r"D:\SteamLibrary") };
    assert_eq!(dashboard.take_effects(), [Effect::Sound(Sound::PromptClose), install]);
    assert_eq!(dashboard.page(), None);
}

#[test]
fn b_on_the_question_installs_nothing() {
    let mut dashboard = install_menu(library(storage()));
    press(&mut dashboard, &[A]);
    dashboard.take_effects();
    assert_eq!(pressed(&mut dashboard, &[B]), []);
    assert_eq!(dashboard.page(), None);
}

#[test]
fn with_one_folder_or_a_store_that_cannot_be_told_install_asks_nothing() {
    let mut one = storage();
    one.stores[0].folders.truncate(1);
    one.stores[0].default = 0;
    let mut fixed = storage();
    fixed.stores[0].choose = false;
    for storage in [one, fixed, Storage::default()] {
        let mut dashboard = install_menu(library(storage));
        let game = dashboard.library().games[5].id;
        assert_eq!(pressed(&mut dashboard, &[A]), [Effect::Install(game)]);
        assert_eq!(dashboard.page(), None);
    }
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
#[ignore = "screenshot"]
fn settings_storage() {
    let mut stage = Stage::new(storage_page(library(storage())));
    stage.check("settings-storage");
    press(&mut stage.dashboard, &[Up]);
    stage.check("settings-storage-other-folder");
}

#[test]
#[ignore = "screenshot"]
fn settings_storage_with_nothing_to_show() {
    Stage::new(storage_page(library(Storage::default()))).check("settings-storage-nothing");
}

#[test]
#[ignore = "screenshot"]
fn install_where() {
    let mut dashboard = install_menu(library(storage()));
    press(&mut dashboard, &[A]);
    let mut stage = Stage::new(dashboard);
    stage.check("install-where");
    press(&mut stage.dashboard, &[Up]);
    stage.check("install-where-other-folder");
}

/// Manage on My games & apps, the storage ring at the nav's foot: "All storage", the drive's free space, and the
/// ring of how much is used (my-games.md 2.6).
#[test]
#[ignore = "screenshot"]
fn my_games_storage_ring() {
    let mut dashboard = Dashboard::new(library(storage()));
    dashboard.open_page(Open::MyGames(Entry::Manage));
    Stage::new(dashboard).check("my_games_storage_ring");
}
