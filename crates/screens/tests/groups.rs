//! Settings' Groups (docs/spec/filters.md, section 6): the page that lists every saved filter with a tick for
//! shown, the menu that moves one and deletes a custom one, New Group and a Group's own page, each change carried out
//! on a library database as the executable carries it out, and Home following the next snapshot at once. The
//! pictures are compared with the committed references (crates/shots): `SHOTS=bless cargo test -p screens --test
//! groups` accepts what is drawn now.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::{FilterChange, Played, Rule};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::keyboard::Typed;
use screens::pages::Open;
use screens::settings::Entry;
use screens::{hint_bar, Dashboard, Effect, Sound};
use ui::host::Offscreen;
use ui::Button::{self, Down, Guide, Left, Menu, Right, Up, A, B, X, Y};
use ui::{Host, Press};

/// When the tests' library came into its database.
const ADDED: u64 = 1_780_000_000;

/// The Dashboard over a library database, carrying out what it asks of the filters as the executable does: the
/// change, then the snapshot after.
struct Shell {
    database: library::Database,
    dashboard: Dashboard,
    /// What the last presses asked of the shell.
    effects: Vec<Effect>,
}

impl Shell {
    fn new() -> Self {
        let mut database = library::Database::in_memory().unwrap();
        database.sync(library::STEAM, &screens::fake_library(), ADDED).unwrap();
        let dashboard = Dashboard::new(database.snapshot().unwrap());
        Self { database, dashboard, effects: Vec::new() }
    }

    /// Personalisation > Groups, focus on its first Row.
    fn groups() -> Self {
        let mut shell = Self::new();
        shell.dashboard.open_page(Open::Settings(Entry::Personalisation));
        shell.press(&[A]);
        assert_eq!(shell.focus(), "Settings: Groups: Favourites; shown");
        shell.effects.clear();
        shell
    }

    fn press(&mut self, buttons: &[Button]) {
        self.effects.clear();
        for &button in buttons {
            self.dashboard.press(Press { button, repeat: false });
            self.carry_out();
        }
    }

    fn typed(&mut self, typed: Typed) {
        self.dashboard.typed(typed);
        self.carry_out();
    }

    fn carry_out(&mut self) {
        for effect in self.dashboard.take_effects() {
            if let Effect::Filter(change) = &effect {
                self.database.change_filter(change).unwrap();
                self.dashboard.set_library(self.database.snapshot().unwrap());
            }
            self.effects.push(effect);
        }
    }

    fn focus(&self) -> String {
        self.dashboard.focus_line()
    }

    fn changes(&self) -> Vec<FilterChange> {
        let filter = |effect: &Effect| match effect {
            Effect::Filter(change) => Some(change.clone()),
            _ => None,
        };
        self.effects.iter().filter_map(filter).collect()
    }

    fn sounds(&self) -> Vec<Sound> {
        let sound = |effect: &Effect| match effect {
            Effect::Sound(sound) => Some(*sound),
            _ => None,
        };
        self.effects.iter().filter_map(sound).collect()
    }

    fn id(&self, name: &str) -> i64 {
        self.dashboard.library().filters.iter().find(|f| f.name == name).expect(name).id
    }

    /// New Group, made and named through text entry, and back on the Groups page with focus on its Row.
    fn make(&mut self, name: &str) {
        self.press(&[Down; 9]);
        assert_eq!(self.focus(), "Settings: Groups: New Group");
        self.press(&[A]);
        assert!(self.focus().starts_with("Settings: Group: Rename; \"New Group\""), "{}", self.focus());
        self.press(&[A]);
        assert!(self.focus().starts_with("TextEntry: "), "{}", self.focus());
        for _ in 0.."New Group".len() {
            self.typed(Typed::Backspace);
        }
        self.typed(Typed::Text(name));
        // Menu keeps the text.
        self.press(&[Menu]);
        assert!(self.focus().starts_with(&format!("Settings: Group: Rename; {name:?}")), "{}", self.focus());
        self.press(&[B]);
        assert_eq!(self.focus(), format!("Settings: Groups: {name}; shown"));
    }
}

#[test]
fn the_groups_box_is_personalisations_first_and_opens_a_row_for_every_filter_then_new_group() {
    let mut shell = Shell::new();
    shell.dashboard.open_page(Open::Settings(Entry::Personalisation));
    assert_eq!(shell.focus(), "Settings: Personalisation: Groups");
    shell.press(&[A]);
    let mut rows = Vec::new();
    for _ in 0..8 {
        rows.push(shell.focus());
        shell.press(&[Down]);
    }
    assert_eq!(
        rows[..6],
        [
            "Settings: Groups: Favourites; shown",
            "Settings: Groups: Installed; not shown",
            "Settings: Groups: Most played; shown",
            "Settings: Groups: Recently added; shown",
            "Settings: Groups: Hidden; not shown",
            "Settings: Groups: New Group",
        ]
    );
    assert_eq!(rows[6], rows[5], "the foot stops");
    // Only a filter's Row has a menu, and so the hint.
    assert_eq!(shell.dashboard.hints(), hint_bar::NONE);
    shell.press(&[Up; 9]);
    assert_eq!(shell.focus(), "Settings: Groups: Favourites; shown", "the top stops");
    assert_eq!(shell.dashboard.hints(), hint_bar::MORE_OPTIONS);
    // Left, Right, X and Y do nothing; B is Personalisation, on the box that opened the page.
    shell.press(&[Left, Right, X, Y]);
    assert_eq!(shell.focus(), "Settings: Groups: Favourites; shown");
    assert!(shell.effects.is_empty());
    shell.press(&[B]);
    assert_eq!(shell.focus(), "Settings: Personalisation: Groups");
}

#[test]
fn a_turns_a_groups_tick_over_and_home_follows_at_once() {
    let mut shell = Shell::groups();
    assert_eq!(shell.dashboard.home_groups(), ["Most played", "Recently added"]);
    shell.press(&[Down, Down]);
    shell.press(&[A]);
    let most_played = shell.id("Most played");
    assert_eq!(shell.changes(), [FilterChange::Shown { filter: most_played, shown: false }]);
    assert_eq!(shell.sounds(), [Sound::Activate]);
    assert_eq!(shell.focus(), "Settings: Groups: Most played; not shown", "focus stays");
    assert_eq!(shell.dashboard.home_groups(), ["Recently added"]);
    // Installed, shown: it is a Group too, in its place.
    shell.press(&[Up, A]);
    assert_eq!(shell.dashboard.home_groups(), ["Installed", "Recently added"]);
    shell.press(&[A, Down, A]);
    assert_eq!(shell.dashboard.home_groups(), ["Most played", "Recently added"]);
}

/// The issue's own: a Group made with the pad, moved first, is Home's first Group and Full library's first tab.
#[test]
fn a_group_made_with_the_pad_and_moved_first_is_homes_first_group_and_the_first_tab() {
    let mut shell = Shell::groups();
    shell.make("Couch co-op");
    let id = shell.id("Couch co-op");
    assert_eq!(shell.dashboard.home_groups(), ["Most played", "Recently added", "Couch co-op"]);

    // Its menu: Move up, Edit, Delete; no Move down, it being last.
    shell.press(&[Menu]);
    assert_eq!(shell.focus(), "Settings: Groups: Menu(Move up) of Couch co-op");
    assert_eq!(shell.sounds(), [Sound::PromptOpen]);
    // A again and again moves it up; at the top Move up goes and focus drops to the next row.
    for _ in 0..5 {
        shell.press(&[A]);
        assert_eq!(shell.changes(), [FilterChange::Move { filter: id, by: -1 }]);
        assert_eq!(shell.sounds(), [Sound::GuideSelect]);
    }
    assert_eq!(shell.focus(), "Settings: Groups: Menu(Move down) of Couch co-op");
    shell.press(&[B]);
    assert_eq!(shell.focus(), "Settings: Groups: Couch co-op; shown", "on its Row, which has moved");
    assert_eq!(shell.sounds(), [Sound::PromptClose]);
    shell.press(&[Down]);
    assert_eq!(shell.focus(), "Settings: Groups: Favourites; shown");

    assert_eq!(shell.dashboard.home_groups(), ["Couch co-op", "Most played", "Recently added"]);
    let first_tab = shell.dashboard.library().groups().next().unwrap();
    assert_eq!((first_tab.id, first_tab.name.as_str()), (id, "Couch co-op"));
}

#[test]
fn a_built_in_groups_menu_moves_it_and_has_nothing_else() {
    let mut shell = Shell::groups();
    shell.press(&[Menu]);
    assert_eq!(shell.focus(), "Settings: Groups: Menu(Move down) of Favourites");
    shell.press(&[Down, Up]);
    assert!(shell.sounds().is_empty(), "nowhere to go");
    shell.press(&[Down, Down, Menu]);
    assert_eq!(shell.focus(), "Settings: Groups: Menu(Move down) of Favourites");
    // While the menu is up the hint bar has gone, and the page has a dialog over it.
    assert_eq!(shell.dashboard.hints(), hint_bar::NONE);
    shell.press(&[B, Down, Down]);
    shell.press(&[Menu, Down]);
    assert_eq!(shell.focus(), "Settings: Groups: Menu(Move down) of Most played");
    assert_eq!(shell.sounds(), [Sound::PromptOpen, Sound::GuideMove]);
    shell.press(&[A, B]);
    assert_eq!(shell.focus(), "Settings: Groups: Most played; shown");
    assert_eq!(shell.dashboard.home_groups(), ["Recently added", "Most played"]);
    shell.press(&[Up]);
    assert_eq!(shell.focus(), "Settings: Groups: Recently added; shown");
}

#[test]
fn delete_asks_first_cancel_goes_back_to_the_menu_and_delete_puts_focus_on_the_row_in_its_place() {
    let mut shell = Shell::groups();
    shell.make("Mine");
    shell.press(&[Menu, Down, Down]);
    assert_eq!(shell.focus(), "Settings: Groups: Menu(Delete) of Mine");
    shell.press(&[A]);
    assert_eq!(shell.focus(), "Settings: Groups: Prompt(Delete) of Mine");
    assert_eq!(shell.sounds(), [Sound::GuideSelect]);
    assert!(shell.changes().is_empty());
    // B and Cancel are both back to the menu, focus on Delete.
    shell.press(&[B]);
    assert_eq!(shell.focus(), "Settings: Groups: Menu(Delete) of Mine");
    shell.press(&[A, Down, A]);
    assert_eq!(shell.focus(), "Settings: Groups: Menu(Delete) of Mine");
    let id = shell.id("Mine");
    shell.press(&[A, A]);
    assert_eq!(shell.changes(), [FilterChange::Delete { filter: id }]);
    assert_eq!(shell.sounds(), [Sound::GuideSelect, Sound::PromptClose]);
    assert_eq!(shell.focus(), "Settings: Groups: New Group");
    assert!(shell.dashboard.library().filters.iter().all(|f| f.name != "Mine"));
    assert_eq!(shell.dashboard.home_groups(), ["Most played", "Recently added"]);
}

#[test]
fn the_guide_puts_the_menu_away_and_the_page_stays() {
    let mut shell = Shell::groups();
    shell.press(&[Menu]);
    shell.press(&[Guide]);
    assert!(shell.dashboard.guide_open());
    shell.press(&[B]);
    assert!(!shell.dashboard.guide_open());
    assert_eq!(shell.focus(), "Settings: Groups: Favourites; shown");
}

#[test]
fn a_groups_page_sets_what_it_picks_and_its_count_follows() {
    let mut shell = Shell::groups();
    shell.make("Ready");
    let all = shell.dashboard.library().games.len();
    shell.press(&[Menu, Down]);
    assert_eq!(shell.focus(), "Settings: Groups: Menu(Edit) of Ready");
    shell.press(&[A]);
    assert_eq!(shell.focus(), format!("Settings: Group: Installed; \"Ready\", {all} games, Installed: Any"));
    // Installed's list opens on Any; Installed chosen, saved at once, and the count is the installed games'.
    shell.press(&[A]);
    assert_eq!(shell.focus(), format!("Settings: Group: Installed > Any; \"Ready\", {all} games, Installed: Any"));
    shell.press(&[Down, A]);
    let id = shell.id("Ready");
    let installed = Rule { installed: Some(true), ..Rule::default() };
    let set = |rule: &Rule| FilterChange::Set {
        filter: id,
        name: "Ready".into(),
        rule: rule.clone(),
        sort: model::Sort::Name,
        descending: false,
    };
    assert_eq!(shell.changes(), [set(&installed)]);
    let ready = shell.dashboard.library().games.iter().filter(|game| game.installed).count();
    assert!(ready < all);
    assert_eq!(shell.focus(), format!("Settings: Group: Installed; \"Ready\", {ready} games, Installed: Installed"));
    // The checkbox row turns over in place; Played's list sets its clause.
    shell.press(&[Down, A]);
    let favourites = Rule { favourite: Some(true), ..installed.clone() };
    assert_eq!(shell.changes(), [set(&favourites)]);
    assert!(shell.focus().contains("Favourites only: on"), "{}", shell.focus());
    shell.press(&[A, Down, A, Down, Down, A]);
    let never = Rule { played: Some(Played::Never), ..installed.clone() };
    assert_eq!(shell.changes(), [set(&installed), set(&never)]);
    // B, or Left, back to the Groups page on its Row.
    shell.press(&[Left]);
    assert_eq!(shell.focus(), "Settings: Groups: Ready; shown");
}

#[test]
fn a_name_kept_empty_is_not_kept_and_b_drops_what_was_typed() {
    let mut shell = Shell::groups();
    shell.make("Kept");
    shell.press(&[Menu, Down, A, Up, A]);
    for _ in 0.."Kept".len() {
        shell.typed(Typed::Backspace);
    }
    shell.typed(Typed::Text("   "));
    shell.press(&[Menu]);
    assert!(shell.changes().is_empty());
    assert!(shell.focus().starts_with("Settings: Group: Rename; \"Kept\""), "{}", shell.focus());
    shell.press(&[A]);
    shell.typed(Typed::Text("s"));
    shell.press(&[B]);
    assert!(shell.changes().is_empty());
    assert!(shell.focus().starts_with("Settings: Group: Rename; \"Kept\""), "{}", shell.focus());
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
    /// The Groups page, focus on its first Row.
    fn groups() -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, HD);
        let mut shell = Shell::groups();
        shell.dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, shell, _drawing: drawing }
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

/// Personalisation, focus on Groups: its first box, alone on its row, the checkbox boxes under it.
#[test]
fn settings_personalisation_groups() {
    let mut stage = Stage::groups();
    stage.press(&[B]);
    stage.check("settings-personalisation-groups");
}

/// The Groups page: a Row a filter, ticked while it shows, New Group last; the hint bar for the menu.
#[test]
fn settings_groups() {
    let mut stage = Stage::groups();
    stage.press(&[Down, Down]);
    stage.check("settings-groups");
    stage.press(&[Down; 3]);
    stage.check("settings-groups-new-group");
}

/// A Group's menu over the dimmed page, and the question Delete asks in its place.
#[test]
fn settings_groups_menu_and_delete_prompt() {
    let mut stage = Stage::groups();
    stage.press(&[Down, Down, Menu]);
    stage.check("settings-groups-menu");
    stage.press(&[B]);
    stage.shell.make("Couch co-op");
    stage.press(&[Menu, Down, Down, A]);
    assert_eq!(stage.shell.focus(), "Settings: Groups: Prompt(Delete) of Couch co-op");
    stage.check("settings-groups-delete-prompt");
}

/// A Group's page as New Group opens it, focus on Rename; then Played's list open; then the page scrolled to Sort.
#[test]
fn settings_group_page() {
    let mut stage = Stage::groups();
    stage.press(&[Down; 9]);
    stage.press(&[A]);
    stage.check("settings-group");
    stage.press(&[Down, Down, Down, A]);
    assert!(stage.shell.focus().starts_with("Settings: Group: Played > Any"), "{}", stage.shell.focus());
    stage.check("settings-group-dropdown-open");
    stage.press(&[B, Down, Down, Down]);
    assert!(stage.shell.focus().starts_with("Settings: Group: Sort"), "{}", stage.shell.focus());
    stage.check("settings-group-scrolled");
}
