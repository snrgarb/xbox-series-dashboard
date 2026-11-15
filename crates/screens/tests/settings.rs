//! Settings (docs/spec/settings.md): where its entry points land and how B climbs back, how its panes are walked,
//! that each option it holds changes Home at once and is as it was left when the core starts again, and what the
//! page and its controls look like. The pictures are compared with the committed references (crates/shots):
//! `SHOTS=bless cargo test -p screens --test settings` accepts what is drawn now.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::Library;
use render::gpu::{self, Gpu};
use render::offscreen::{read_back, wait, FORMAT};
use screens::home::{Focus, TopButton};
use screens::pages::{Around, Open, Page};
use screens::settings::boxes::{Boxes, Item};
use screens::settings::controls::{self, Dropdown, FIELD_HEIGHT, FIELD_WIDTH, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH};
use screens::settings::{Change, Entry, Options, Pane, Setting, Settings, Step};
use screens::{Dashboard, Effect};
use ui::host::Offscreen;
use ui::Button::{self, Down, Guide, Left, LeftBumper, Menu, Right, RightBumper, Up, A, B, X, Y};
use ui::{Battery, Host, PadStatus, Press, Rect, Ui};

/// When the tests' library came into its database.
const ADDED: u64 = 1_780_000_000;

/// A library database in memory with the fake library in it: where the settings are kept.
fn database() -> library::Database {
    let mut database = library::Database::in_memory().unwrap();
    database.sync(library::STEAM, &screens::fake_library(), ADDED).unwrap();
    database
}

/// The Dashboard as the executable starts it: the database's snapshot, and the options its settings table has.
fn start(database: &library::Database) -> Dashboard {
    let mut dashboard = Dashboard::new(database.snapshot().unwrap());
    dashboard.set_options(Options::read(|key| database.setting(key).unwrap()));
    dashboard
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// The options Settings changed since the last call, each saved as the executable saves it.
fn save(dashboard: &mut Dashboard, database: &library::Database) -> Vec<Change> {
    let changes: Vec<Change> = dashboard
        .take_effects()
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::Set(change) => Some(change),
            _ => None,
        })
        .collect();
    for change in &changes {
        database.set_setting(change.key(), change.value()).unwrap();
    }
    changes
}

/// Settings from the top bar's button, focus on the rail.
fn settings(database: &library::Database) -> Dashboard {
    let mut dashboard = start(database);
    press(&mut dashboard, &[Right, Right, Right, A]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: Theme settings");
    dashboard.take_effects();
    dashboard
}

/// Personalisation, focus on the checkbox box this far down it: under Groups, its first box.
fn personalisation(database: &library::Database, down: usize) -> Dashboard {
    let mut dashboard = settings(database);
    press(&mut dashboard, &[Right, A]);
    press(&mut dashboard, &vec![Down; down + 1]);
    dashboard.take_effects();
    dashboard
}

#[test]
fn the_rail_shows_theme_settings_and_right_or_a_goes_into_its_boxes() {
    let database = database();
    for into in [Right, A] {
        let mut dashboard = settings(&database);
        // One item: nowhere to go along the rail, and nothing to its left.
        press(&mut dashboard, &[Up, Down, Left]);
        assert_eq!(dashboard.focus_line(), "Settings: Rail: Theme settings");
        press(&mut dashboard, &[into]);
        assert_eq!(dashboard.focus_line(), "Settings: Theme settings: Personalisation");
        // B is the rail again, and B there is Home, on the button that opened the page.
        press(&mut dashboard, &[B]);
        assert_eq!(dashboard.focus_line(), "Settings: Rail: Theme settings");
        press(&mut dashboard, &[B]);
        assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::TopBar(TopButton::Settings)));
    }
}

#[test]
fn a_pane_of_boxes_is_walked_by_row_and_column_and_left_from_the_first_column_goes_back() {
    let database = database();
    let mut dashboard = settings(&database);
    press(&mut dashboard, &[Right]);
    // Theme settings: Personalisation, then Guide beside it. The row's end, the top and the foot stop.
    press(&mut dashboard, &[Right, Right, Up, Down]);
    assert_eq!(dashboard.focus_line(), "Settings: Theme settings: Guide");
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Theme settings: Personalisation");
    // Left from the first column is back a level, as B: the rail.
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: Theme settings");

    // Personalisation: Groups, then a checkbox box a row.
    press(&mut dashboard, &[A, A]);
    let boxes = ["Groups", "Background delay", "Reduce motion", "Show Play history", "Show battery percentages"];
    for name in boxes {
        assert_eq!(dashboard.focus_line(), format!("Settings: Personalisation: {name}"));
        press(&mut dashboard, &[Right, Down]);
    }
    assert_eq!(dashboard.focus_line(), "Settings: Personalisation: Show battery percentages", "the foot stops");
    press(&mut dashboard, &[Up, Up, Up, Up, Up]);
    assert_eq!(dashboard.focus_line(), "Settings: Personalisation: Groups", "the top stops");
    // Left steps back to Theme settings' boxes, focus on the box that opened the page.
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Theme settings: Personalisation");
}

#[test]
fn b_climbs_one_level_at_a_time_and_a_page_gives_focus_back_to_the_box_that_opened_it() {
    let database = database();
    let mut dashboard = settings(&database);
    press(&mut dashboard, &[Right, Right, A]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Use the Guide");
    // The Guide page has the one control: nowhere to move, and Right does nothing.
    press(&mut dashboard, &[Up, Down, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Use the Guide");
    for back in [B, Left] {
        press(&mut dashboard, &[back]);
        assert_eq!(dashboard.focus_line(), "Settings: Theme settings: Guide");
        press(&mut dashboard, &[A]);
    }
    // Page, boxes, rail, out.
    press(&mut dashboard, &[B, B]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: Theme settings");
    // Into the category again: its first box, wherever focus was when it was left.
    press(&mut dashboard, &[Right]);
    assert_eq!(dashboard.focus_line(), "Settings: Theme settings: Personalisation");
    press(&mut dashboard, &[B, B]);
    assert_eq!(dashboard.page(), None);
    assert!(save(&mut dashboard, &database).is_empty(), "walking the page changes nothing");
}

#[test]
fn customise_home_opens_personalisation_and_b_climbs_back_through_every_level() {
    let database = database();
    let mut dashboard = start(&database);
    press(&mut dashboard, &[Down, Right]);
    dashboard.open_page(Open::Settings(Entry::Personalisation));
    assert_eq!(dashboard.focus_line(), "Settings: Personalisation: Groups");
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.focus_line(), "Settings: Theme settings: Personalisation");
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: Theme settings");
    press(&mut dashboard, &[B]);
    assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::Recent(1)), "back on the tile");
}

#[test]
fn the_settings_tile_and_the_discord_tile_open_theme_settings_on_its_first_box() {
    let database = database();
    for tile in [0, 1] {
        let mut dashboard = start(&database);
        press(&mut dashboard, &[Down, Down]);
        press(&mut dashboard, &vec![Right; tile]);
        assert_eq!(dashboard.home_focus(), Focus::Wide(tile));
        press(&mut dashboard, &[A]);
        assert_eq!(dashboard.focus_line(), "Settings: Theme settings: Personalisation");
        press(&mut dashboard, &[B]);
        assert_eq!(dashboard.focus_line(), "Settings: Rail: Theme settings");
        press(&mut dashboard, &[B]);
        assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::Wide(tile)));
    }
}

#[test]
fn every_other_button_is_swallowed_and_nothing_behind_the_page_acts() {
    let database = database();
    let mut dashboard = personalisation(&database, 1);
    let home = dashboard.home_focus();
    press(&mut dashboard, &[Y, X, Menu, LeftBumper, RightBumper]);
    assert_eq!(dashboard.focus_line(), "Settings: Personalisation: Reduce motion");
    assert_eq!((dashboard.pages_up(), dashboard.home_focus()), (1, home));
    assert!(dashboard.take_effects().is_empty());
    // A held A turns a checkbox over once, not again and again.
    dashboard.press(Press { button: A, repeat: true });
    assert!(dashboard.take_effects().is_empty() && !dashboard.options().reduce_motion);
}

#[test]
fn a_checkbox_turns_over_in_place_at_once_and_asks_for_it_to_be_saved() {
    let database = database();
    let mut dashboard = personalisation(&database, 0);
    assert!(dashboard.options().background_delay);
    press(&mut dashboard, &[A]);
    assert!(!dashboard.options().background_delay);
    assert_eq!(dashboard.focus_line(), "Settings: Personalisation: Background delay", "focus stays");
    let change = Change { setting: Setting::BackgroundDelay, on: false };
    assert_eq!(dashboard.take_effects(), [Effect::Sound(screens::Sound::Activate), Effect::Set(change)]);
    // And back again.
    press(&mut dashboard, &[A]);
    assert!(dashboard.options().background_delay);
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::BackgroundDelay, on: true }]);
}

#[test]
fn show_play_history_off_gives_the_recent_row_a_game_in_the_tiles_place() {
    let database = database();
    let mut dashboard = start(&database);
    // On, as it starts: seven games, then the Play history tile.
    press(&mut dashboard, &[Down]);
    press(&mut dashboard, &[Right; 9]);
    assert_eq!((dashboard.home_focus(), dashboard.focused_game()), (Focus::Recent(7), None));

    // Off, with focus left on the tile: it is on the game that took its place, at once.
    dashboard.open_page(Open::Settings(Entry::Personalisation));
    press(&mut dashboard, &[Down, Down, Down, A, B, B, B]);
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::ShowPlayHistory, on: false }]);
    assert_eq!(dashboard.page(), None);
    press(&mut dashboard, &[Right; 9]);
    assert_eq!((dashboard.home_focus(), dashboard.focused_game()), (Focus::Recent(7), Some(7)), "eight games");
    press(&mut dashboard, &[A]);
    assert!(dashboard.take_effects().contains(&Effect::Launch(7)), "A on the eighth tile plays its game");

    // On again: the tile is back, and A on it is Play history.
    dashboard.open_page(Open::Settings(Entry::Personalisation));
    press(&mut dashboard, &[Down, Down, Down, A, B, B, B]);
    assert_eq!((dashboard.home_focus(), dashboard.focused_game()), (Focus::Recent(6), Some(6)));
    press(&mut dashboard, &[Right, A]);
    assert_eq!(dashboard.focus_line(), "MyGames: PlayHistory: Row(0)");
}

#[test]
fn use_the_guide_off_leaves_the_guide_button_and_homes_profile_alone() {
    let database = database();
    let mut dashboard = settings(&database);
    press(&mut dashboard, &[Right, Right, A, A]);
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::UseGuide, on: false }]);
    // At once, on the page itself: the Guide button opens nothing.
    press(&mut dashboard, &[Guide]);
    assert!(!dashboard.guide_open());
    // Nor does the profile on Home.
    press(&mut dashboard, &[B, B, B, Left, Left, Left, A]);
    assert_eq!(dashboard.home_focus(), Focus::TopBar(TopButton::Profile));
    assert!(!dashboard.guide_open());

    // On again: both do.
    press(&mut dashboard, &[Right, Right, Right, A, Right, Right, A, A]);
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::UseGuide, on: true }]);
    press(&mut dashboard, &[Guide]);
    assert!(dashboard.guide_open());
}

#[test]
fn every_option_is_as_it_was_left_when_the_core_starts_again() {
    let database = database();
    let mut dashboard = start(&database);
    assert_eq!(dashboard.options(), Options::default(), "nothing is set until it is changed");
    assert!(Setting::ALL.iter().all(|setting| database.setting(setting.key()).unwrap().is_none()));

    // Every checkbox on Personalisation turned over, and Use the Guide.
    dashboard.open_page(Open::Settings(Entry::Personalisation));
    press(&mut dashboard, &[Down, A, Down, A, Down, A, Down, A, B, Right, A, A]);
    assert_eq!(save(&mut dashboard, &database).len(), 5);
    let left = Options {
        background_delay: false,
        reduce_motion: true,
        show_play_history: false,
        show_battery_percent: false,
        use_guide: false,
    };
    assert_eq!(dashboard.options(), left);

    // The core starts again on the same database: the options are as they were left, and Home is as they say.
    let mut again = start(&database);
    assert_eq!(again.options(), left);
    press(&mut again, &[Down]);
    press(&mut again, &[Right; 9]);
    assert_eq!(again.focused_game(), Some(7), "the Recent row has its eighth game");
    press(&mut again, &[Guide]);
    assert!(!again.guide_open(), "and the Guide button is left alone");

    // One turned back, and a third start.
    again.open_page(Open::Settings(Entry::Personalisation));
    press(&mut again, &[Down, Down, A]);
    assert_eq!(save(&mut again, &database), [Change { setting: Setting::ReduceMotion, on: false }]);
    assert_eq!(start(&database).options(), Options { reduce_motion: false, ..left });
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

/// The fake library, its first game with plain red background art and its second with plain blue: which game's
/// art Home's background shows is told by its colour.
fn library_with_plain_art() -> Library {
    let mut library = screens::fake_library();
    for (game, colour) in [[200, 40, 40], [40, 40, 200]].into_iter().enumerate() {
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("settings-hero-{game}.png"));
        image::RgbImage::from_pixel(96, 31, image::Rgb(colour)).save(&path).unwrap();
        library.games[game].hero = Some(path);
    }
    let mut database = library::Database::in_memory().unwrap();
    database.sync(library::STEAM, &library, ADDED).unwrap();
    database.snapshot().unwrap()
}

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once on as many threads.
static DRAWING: Mutex<()> = Mutex::new(());

const HD: (u32, u32) = (1920, 1080);

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    /// Held until the fields above, the device among them, are gone.
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn new(library: Library) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, HD);
        let mut dashboard = Dashboard::new(library);
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn press(&mut self, buttons: &[Button]) {
        press(&mut self.dashboard, buttons);
    }

    /// The screen at rest, as an image.
    fn still(&mut self) -> image::RgbaImage {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        image::RgbaImage::from_raw(HD.0, HD.1, pixels).unwrap()
    }

    fn check(&mut self, name: &str) {
        let image = self.still();
        compare(name, &image);
    }

    /// Frames a 25th of a second apart for this long, art decoded as it is asked for; then the screen as it is.
    fn after(&mut self, seconds: f32) -> image::RgbaImage {
        let Self { gpu, host, target, dashboard, .. } = self;
        for _ in 0..(seconds * 25.0).round() as u32 {
            let mut dt = 0.04;
            loop {
                let draw = |ui: &mut Ui| dashboard.draw_all(ui);
                let index = host.draw(gpu, &target.view, target.size, dt, wgpu::Color::BLACK, draw);
                wait(gpu, Some(index));
                if !host.art.wait(gpu, &mut host.renderer) {
                    break;
                }
                while host.art.wait(gpu, &mut host.renderer) {}
                dt = 0.0;
            }
        }
        let pixels = read_back(gpu, &target.texture, target.size);
        image::RgbaImage::from_raw(HD.0, HD.1, pixels).unwrap()
    }

    /// A Settings page of the test's own in the real shell, at rest: what a pane built from the controls looks
    /// like.
    fn check_pane(&mut self, name: &str, pane: Box<dyn Pane>) {
        let mut page = Settings::showing(pane);
        let library = Library::default();
        let around = Around { library: &library, playing: None, options: Options::default() };
        let Self { gpu, host, target, .. } = self;
        let pixels = target.still(gpu, host, |ui| page.draw(ui, around));
        compare(name, &image::RgbaImage::from_raw(HD.0, HD.1, pixels).unwrap());
    }
}

fn compare(name: &str, image: &image::RgbaImage) {
    let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
    image.save(&got).unwrap();
    shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
}

/// Settings from the top bar's button, on a stage.
fn settings_stage() -> Stage {
    let mut stage = Stage::new(database().snapshot().unwrap());
    stage.press(&[Right, Right, Right, A]);
    stage
}

/// The Bundle's picture 28: focus on the rail's Theme settings, its boxes beside it with no ring.
#[test]
fn settings_theme_settings() {
    settings_stage().check("settings-theme-settings");
}

/// The Bundle's picture 29: focus on Theme settings' first box.
#[test]
fn settings_theme_settings_box_focused() {
    let mut stage = settings_stage();
    stage.press(&[Right]);
    stage.check("settings-theme-settings-box");
}

/// The Bundle's picture 31: Personalisation, focus on Background delay, its info text beside it.
#[test]
fn settings_personalisation_checkbox_focused() {
    let mut stage = settings_stage();
    stage.press(&[Right, A, Down]);
    stage.check("settings-personalisation-checkbox");
    // Another box, turned over: its info moves with focus, and nothing else does.
    stage.press(&[Down, Down, A]);
    stage.check("settings-personalisation-play-history-off");
}

/// The Bundle's picture 35, without the Now Playing source: the Guide page, focus on Use the Guide.
#[test]
fn settings_guide() {
    let mut stage = settings_stage();
    stage.press(&[Right, Right, A]);
    stage.check("settings-guide");
}

/// Idle's page as the Bundle's pictures 33 and 34 have it, built from the controls: a label over each of two
/// dropdowns. The feature is M5's; the page is here for what the controls look like, and to show they are
/// enough to build one.
struct Idle {
    focus: usize,
    dim: Dropdown,
}

const STEPS: [&str; 11] =
    ["1 min", "2 min", "5 min", "10 min", "15 min", "30 min", "1 h", "2 h", "3 h", "4 h", "Never"];

impl Pane for Idle {
    fn header(&self) -> String {
        "Idle".into()
    }

    fn focus(&self) -> String {
        match self.dim.focused() {
            Some(choice) => format!("Idle: Dim after > {choice}"),
            None => format!("Idle: {}", ["Dim after", "Show things after"][self.focus]),
        }
    }

    fn press(&mut self, press: Press, _around: Around) -> Step {
        if self.dim.is_open() {
            self.dim.press(press.button);
            return Step::Stay;
        }
        match press.button {
            Up => self.focus = 0,
            Down => self.focus = 1,
            A if self.focus == 0 => self.dim.open(STEPS.map(String::from).to_vec(), 1),
            B | Left => return Step::Back,
            _ => {}
        }
        Step::Stay
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, focused: bool) {
        let mut y = controls::TOP;
        y += controls::label(ui, LEFT, y, "Dim after") + 14.0;
        let dim = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        self.dim.draw(ui, dim, "2 min", focused && self.focus == 0);
        y += FIELD_HEIGHT + 48.0;
        y += controls::label(ui, LEFT, y, "Show things after") + 14.0;
        let show = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        Dropdown::default().draw(ui, show, "5 min", focused && self.focus == 1);
        // The open list last: it lies over Show things after and moves nothing.
        self.dim.draw_list(ui, dim);
    }
}

fn around(library: &Library) -> Around<'_> {
    Around { library, playing: None, options: Options::default() }
}

/// The Bundle's pictures 33 and 34: a dropdown box with focus, and its list open on the choice that is set.
#[test]
fn settings_dropdown_closed_and_open() {
    let mut stage = settings_stage();
    stage.check_pane("settings-dropdown", Box::new(Idle { focus: 0, dim: Dropdown::default() }));

    let library = Library::default();
    let mut idle = Idle { focus: 0, dim: Dropdown::default() };
    idle.press(Press { button: A, repeat: false }, around(&library));
    assert_eq!(idle.focus(), "Idle: Dim after > 2 min", "the list opens on the choice that is set");
    stage.check_pane("settings-dropdown-open", Box::new(idle));

    // The list's foot: the choices have scrolled if they had to, and the last has focus.
    let mut idle = Idle { focus: 0, dim: Dropdown::default() };
    for button in [A, Down, Down, Down, Down, Down, Down, Down, Down, Down, Down] {
        idle.press(Press { button, repeat: false }, around(&library));
    }
    assert_eq!(idle.focus(), "Idle: Dim after > Never");
    stage.check_pane("settings-dropdown-open-last", Box::new(idle));
}

/// The controls no page of M3 has yet: Rows, as Boot video's page has them (the Bundle's picture 32), and pane
/// buttons, as Discord's (36).
struct RowsAndButtons;

impl Pane for RowsAndButtons {
    fn header(&self) -> String {
        "Boot video".into()
    }

    fn focus(&self) -> String {
        "Boot video: Xbox 360 (2005)".into()
    }

    fn press(&mut self, _press: Press, _around: Around) -> Step {
        Step::Stay
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, focused: bool) {
        let mut y = controls::BOXES_TOP;
        y +=
            controls::description(ui, LEFT, y, ROW_WIDTH, "Plays as the Dashboard starts. Any button skips it.") + 26.0;
        let long = "A console whose name is far too long for its row and so is cut short with an ellipsis at its end";
        let rows = ["Off", "Xbox (2001)", "Xbox 360 (2005)", long];
        // The focused Row last, over its neighbours.
        for (i, name) in rows.iter().enumerate().filter(|&(i, _)| i != 2) {
            controls::row(ui, Rect::new(LEFT, y + ROW_PITCH * i as f32, ROW_WIDTH, ROW_HEIGHT), name, i == 1, false);
        }
        controls::row(ui, Rect::new(LEFT, y + ROW_PITCH * 2.0, ROW_WIDTH, ROW_HEIGHT), rows[2], false, focused);
        y += ROW_PITCH * rows.len() as f32 + 40.0;
        let mut x = LEFT;
        for (label, enabled, focused) in
            [("Type", true, false), ("Paste", true, true), ("Disconnect now", false, false)]
        {
            let width = controls::pane_button_width(ui, label);
            controls::pane_button(ui, Rect::new(x, y, width, FIELD_HEIGHT), label, enabled, focused);
            x += width + 20.0;
        }
    }
}

#[test]
fn settings_rows_and_pane_buttons() {
    settings_stage().check_pane("settings-rows-and-buttons", Box::new(RowsAndButtons));
}

/// Personalisation as it will be with the boxes of later milestones over its checkbox boxes: seven rows, which
/// run off a 1080 screen. Nothing moves until focus is on a box that does not show whole; then the pane has
/// scrolled just far enough to show it with its info text, and its first rows are cut under the header's row.
#[test]
fn a_pane_of_boxes_that_runs_off_the_screen_scrolls_to_the_focused_box() {
    let opens = |icon, name| Item::Opens { icon, name, pane: |_| Box::new(RowsAndButtons) };
    let check = |name| vec![Item::Check { setting: Setting::ReduceMotion, name, info: INFO }];
    const INFO: &str = "Shows how much charge is left beside the controller's battery, and this PC's on a laptop or \
                        handheld. Turn it off to show the batteries alone.";
    let mut boxes = Boxes::new(
        "Personalisation",
        vec![
            vec![opens('\u{E714}', "Boot video"), opens('\u{E716}', "Profiles")],
            vec![opens('\u{E748}', "Who's playing today?"), opens('\u{E71D}', "Groups")],
            check("Background delay"),
            check("Reduce motion"),
            check("Show Play history"),
            check("Show battery percentages"),
            check("One more than the Bundle has"),
        ],
    );
    let library = Library::default();
    let mut stage = settings_stage();
    let draw = |stage: &mut Stage, boxes: &mut Boxes| {
        let Stage { gpu, host, target, .. } = stage;
        target.still(gpu, host, |ui| boxes.draw(ui, around(&library), true));
    };
    // Down to the fifth row, the last that shows whole with its info text: nothing has moved.
    for _ in 0..4 {
        boxes.press(Press { button: Down, repeat: false }, around(&library));
        draw(&mut stage, &mut boxes);
        assert_eq!(boxes.scroll(), 0.0, "{}", boxes.focus());
    }
    // The sixth is cut by the screen's foot where it rests: the pane has risen to show it and its info, 32 clear.
    boxes.press(Press { button: Down, repeat: false }, around(&library));
    draw(&mut stage, &mut boxes);
    assert_eq!(boxes.focus(), "Personalisation: Show battery percentages");
    let sixth = boxes.scroll();
    assert!(sixth > 6.0 && sixth < 149.0, "{sixth}");
    boxes.press(Press { button: Down, repeat: false }, around(&library));
    draw(&mut stage, &mut boxes);
    assert!((boxes.scroll() - sixth - 149.0).abs() < 0.01, "a row further: {}", boxes.scroll());
    // Up again does not move it while the focused box shows; at the top the first row is back in its place.
    boxes.press(Press { button: Up, repeat: false }, around(&library));
    draw(&mut stage, &mut boxes);
    assert_eq!(boxes.scroll(), sixth + 149.0);
    for _ in 0..5 {
        boxes.press(Press { button: Up, repeat: false }, around(&library));
        draw(&mut stage, &mut boxes);
    }
    assert_eq!((boxes.focus().as_str(), boxes.scroll()), ("Personalisation: Boot video", 0.0));

    for _ in 0..6 {
        boxes.press(Press { button: Down, repeat: false }, around(&library));
    }
    stage.check_pane("settings-boxes-scrolled", Box::new(boxes));
}

/// The colour of Home's background in the clear between the top bar and the rows.
fn background(image: &image::RgbaImage) -> [u8; 3] {
    let pixel = image.get_pixel(960, 300).0;
    [pixel[0], pixel[1], pixel[2]]
}

#[test]
fn background_delay_off_shows_a_games_art_almost_at_once() {
    let red = |colour: [u8; 3]| colour[0] > 2 * colour[2].max(30);
    let blue = |colour: [u8; 3]| colour[2] > 2 * colour[0].max(30);
    // On, as it starts: a second after focus moved along the Recent row the background is still the game's it
    // left, and a second later the one's it is on.
    let mut stage = Stage::new(library_with_plain_art());
    stage.press(&[Down]);
    assert!(red(background(&stage.after(1.0))));
    stage.press(&[Right]);
    let waiting = background(&stage.after(1.0));
    assert!(red(waiting), "the art changed before the delay was over: {waiting:?}");
    let shown = background(&stage.after(1.5));
    assert!(blue(shown), "{shown:?}");

    // Turned off in Settings, and back on Home: a second after a move the background is the game's focus is on.
    stage.dashboard.open_page(Open::Settings(Entry::Personalisation));
    stage.press(&[Down, A, B, B, B]);
    assert!(!stage.dashboard.options().background_delay && stage.dashboard.page().is_none());
    assert!(blue(background(&stage.after(1.0))));
    stage.press(&[Left]);
    let off = background(&stage.after(1.0));
    assert!(red(off), "the art had not changed a second after focus moved: {off:?}");
}

#[test]
fn reduce_motion_is_in_force_from_the_next_frame() {
    let mut stage = Stage::new(database().snapshot().unwrap());
    stage.after(0.1);
    assert!(!stage.host.store.reduce_motion);
    stage.dashboard.open_page(Open::Settings(Entry::Personalisation));
    stage.press(&[Down, Down, A]);
    stage.after(0.04);
    assert!(stage.host.store.reduce_motion, "what slides is told to stand in place");
    // The Guide then shows whole in its first frame, where it would slide in.
    stage.press(&[B, B, B, Guide]);
    let first = stage.after(0.04);
    let at_rest = stage.still();
    assert!(shots::difference(&first, &at_rest).share < 0.001, "the Guide was still on its way in");
    // And off again.
    stage.press(&[B]);
    stage.dashboard.open_page(Open::Settings(Entry::Personalisation));
    stage.press(&[Down, Down, A]);
    stage.after(0.04);
    assert!(!stage.host.store.reduce_motion);
}

#[test]
fn show_battery_percentages_off_leaves_the_pads_battery_alone_on_home() {
    // The top bar's status area, left of the clock: how much of it is white.
    let white = |image: &image::RgbaImage| {
        let bar = image::imageops::crop_imm(image, 1300, 90, 425, 50).to_image();
        bar.pixels().filter(|pixel| pixel.0[..3].iter().all(|&channel| channel > 200)).count()
    };
    let mut stage = Stage::new(database().snapshot().unwrap());
    stage.dashboard.set_pad(PadStatus { connected: true, battery: Some(Battery::Charge(0.57)) });
    let home = stage.still();
    // Kept to look at: the top bar with a wireless pad's battery and its charge.
    home.save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("home-pad-battery.png")).unwrap();
    let with = white(&home);

    stage.dashboard.open_page(Open::Settings(Entry::Personalisation));
    stage.press(&[Down, Down, Down, Down, A, B, B, B]);
    assert!(!stage.dashboard.options().show_battery_percent && stage.dashboard.page().is_none());
    let without = white(&stage.still());
    assert!(without > 100, "the battery's cell is still there: {without}");
    assert!(with > without + 60, "the figure went with the setting: {with} with it, {without} without");
}
