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
use screens::settings::controls::{self, FIELD_HEIGHT, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH};
use screens::settings::{
    BootVideo, Category, Change, Entry, Found, Options, Pane, Setting, Settings, Step, BOOT_VIDEO,
};
use screens::{Dashboard, Effect};
use ui::host::Offscreen;
use ui::Button::{self, Down, Guide, Left, LeftBumper, Menu, Right, RightBumper, Up, A, B, X, Y};
use ui::{Battery, Host, PadStatus, Press, Rect, Ui};

/// When the tests' library came into its database.
const ADDED: u64 = 1_780_000_000;

/// A library database in memory with the fake library in it: where the settings are kept.
fn database() -> library::Database {
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &screens::fake_library(), ADDED).unwrap();
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
            // Settings > Idle's times are their own rows, saved as the executable saves them.
            Effect::IdleTimes(times) => {
                for (key, value) in times.saved() {
                    database.set_setting(key, &value).unwrap();
                }
                None
            }
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
    assert_eq!(dashboard.focus_line(), "Settings: Rail: General");
    dashboard.take_effects();
    dashboard
}

/// Personalisation, General's first box, focus on the checkbox box this far down it.
fn personalisation(database: &library::Database, down: usize) -> Dashboard {
    let mut dashboard = settings(database);
    press(&mut dashboard, &[Right, A]);
    press(&mut dashboard, &vec![Down; down]);
    dashboard.take_effects();
    dashboard
}

#[test]
fn the_rail_lists_four_categories_each_showing_as_it_has_focus_and_right_or_a_goes_into_it() {
    let database = database();
    for into in [Right, A] {
        let mut dashboard = settings(&database);
        // General, Account, Games & apps, System: the ends stop, and nothing is left of the rail.
        press(&mut dashboard, &[Up, Left]);
        assert_eq!(dashboard.focus_line(), "Settings: Rail: General");
        for category in ["Account", "Games & apps", "System", "System"] {
            press(&mut dashboard, &[Down]);
            assert_eq!(dashboard.focus_line(), format!("Settings: Rail: {category}"));
        }
        press(&mut dashboard, &[Up, Up, Up]);
        assert_eq!(dashboard.focus_line(), "Settings: Rail: General");
        press(&mut dashboard, &[into]);
        assert_eq!(dashboard.focus_line(), "Settings: General: Personalisation");
        // B is the rail again, and B there is Home, on the button that opened the page.
        press(&mut dashboard, &[B]);
        assert_eq!(dashboard.focus_line(), "Settings: Rail: General");
        press(&mut dashboard, &[B]);
        assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::TopBar(TopButton::Settings)));
    }
}

#[test]
fn each_category_keeps_where_focus_was_while_the_page_is_up() {
    let database = database();
    let mut dashboard = settings(&database);
    // General on Idle, then Account on Who's playing today?, each left for the rail.
    press(&mut dashboard, &[Right, Right, B, Down, Right, Right, B]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: Account");
    press(&mut dashboard, &[Up, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Idle");
    press(&mut dashboard, &[B, Down, A]);
    assert_eq!(dashboard.focus_line(), "Settings: Account: Who's playing today?");
    // A visit to a page and back keeps it too.
    press(&mut dashboard, &[A, B, B, Up, Up, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Idle");
    // Not between visits: Settings opens as its entry says, on each category's first box.
    press(&mut dashboard, &[B, B, A, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Personalisation");
    press(&mut dashboard, &[B, Down, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: Account: Profiles");
}

#[test]
fn a_pane_of_boxes_is_walked_by_row_and_column_and_left_from_the_first_column_goes_back() {
    let database = database();
    let mut dashboard = settings(&database);
    press(&mut dashboard, &[Right]);
    // General: Personalisation and Idle beside it, the Bundle's first row; Guide under Personalisation with Boot
    // video beside it. The row's end, the top and the foot stop.
    press(&mut dashboard, &[Right, Right, Up]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Idle");
    press(&mut dashboard, &[Down, Right, Down]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Boot video");
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Guide");
    press(&mut dashboard, &[Up]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Personalisation");
    // Left from the first column is back a level, as B: the rail.
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: General");

    // Account: Profiles and Who's playing today?, Discord under Profiles and Discord messages beside it (#197).
    press(&mut dashboard, &[Down, Right, Right, Down]);
    assert_eq!(dashboard.focus_line(), "Settings: Account: Discord messages");
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Account: Discord");
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: Account");

    // Games & apps, in two sections: Library (Launchers, Games & apps, Groups, Art) and Installs (Storage, Helpers,
    // Set up this PC).
    // Down and Up go along a section, stopping at its ends; Right and Left to the section beside, as far down as it
    // goes.
    press(&mut dashboard, &[Down, Right]);
    for name in ["Launchers", "Games & apps", "Groups", "Art", "Art"] {
        assert_eq!(dashboard.focus_line(), format!("Settings: Games & apps: {name}"));
        press(&mut dashboard, &[Down]);
    }
    press(&mut dashboard, &[Right]);
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Set up this PC", "the shorter section's foot");
    press(&mut dashboard, &[Right, Down]);
    assert_eq!(
        dashboard.focus_line(),
        "Settings: Games & apps: Set up this PC",
        "the last section, and its foot, stop"
    );
    press(&mut dashboard, &[Up, Up, Up]);
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Storage");
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Launchers");
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: Games & apps");

    // System: System and About beside it.
    press(&mut dashboard, &[Down, Right, Right, Right, Down]);
    assert_eq!(dashboard.focus_line(), "Settings: System: About");
    press(&mut dashboard, &[Left, Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: System");

    // Personalisation: a checkbox box a row.
    press(&mut dashboard, &[Up, Up, Up, Right, A]);
    let boxes =
        ["Background delay", "Reduce motion", "Show Play history", "Show launcher icons", "Show battery percentages"];
    for name in boxes {
        assert_eq!(dashboard.focus_line(), format!("Settings: Personalisation: {name}"));
        press(&mut dashboard, &[Right, Down]);
    }
    assert_eq!(dashboard.focus_line(), "Settings: Personalisation: Show battery percentages", "the foot stops");
    press(&mut dashboard, &[Up; 5]);
    assert_eq!(dashboard.focus_line(), "Settings: Personalisation: Background delay", "the top stops");
    // Left steps back to General's boxes, focus on the box that opened the page.
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Personalisation");
}

#[test]
fn b_climbs_one_level_at_a_time_and_a_page_gives_focus_back_to_the_box_that_opened_it() {
    let database = database();
    let mut dashboard = settings(&database);
    press(&mut dashboard, &[Right, Down, A]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Use the Guide");
    // The Guide page has three controls: Use the Guide, Pause the game while the Guide is open, and the Now
    // Playing source under them. Right does nothing.
    press(&mut dashboard, &[Up, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Use the Guide");
    press(&mut dashboard, &[Down, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Pause the game while the Guide is open");
    press(&mut dashboard, &[Down, Down, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Now Playing source");
    press(&mut dashboard, &[Up, Up]);
    for back in [B, Left] {
        press(&mut dashboard, &[back]);
        assert_eq!(dashboard.focus_line(), "Settings: General: Guide");
        press(&mut dashboard, &[A]);
    }
    // Page, boxes, rail, out.
    press(&mut dashboard, &[B, B]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: General");
    // Into the category again: the box focus was on when it was left.
    press(&mut dashboard, &[Right]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Guide");
    press(&mut dashboard, &[B, B]);
    assert_eq!(dashboard.page(), None);
    assert!(save(&mut dashboard, &database).is_empty(), "walking the page changes nothing");
}

#[test]
fn customise_home_opens_games_and_apps_on_groups_and_b_climbs_back_through_every_level() {
    let database = database();
    let mut dashboard = start(&database);
    press(&mut dashboard, &[Down, Right]);
    dashboard.open_page(Open::Settings(Entry::CustomiseHome));
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Groups");
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.focus_line(), "Settings: Rail: Games & apps");
    press(&mut dashboard, &[B]);
    assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::Recent(1)), "back on the tile");
}

/// #230, #235: Storage opened from Settings' own boxes climbs back through them as every page does; opened from
/// Manage Queue's Stopped row, B goes straight back to the row.
#[test]
fn storage_from_its_box_climbs_back_and_from_the_queue_goes_straight_back() {
    let database = database();
    let mut dashboard = start(&database);
    dashboard.open_page(Open::Settings(Entry::CustomiseHome));
    for _ in 0..6 {
        if dashboard.focus_line() == "Settings: Games & apps: Storage" {
            break;
        }
        press(&mut dashboard, &[Up, Right]);
    }
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Storage");
    press(&mut dashboard, &[A]);
    assert!(dashboard.focus_line().starts_with("Settings: Storage: "), "{}", dashboard.focus_line());
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Storage");
    press(&mut dashboard, &[B, B]);
    assert_eq!(dashboard.page(), None);

    // Opened as the Queue's Stopped row opens it: one B and it is gone, back to what was under it (the row
    // itself: tests/manage_queue.rs, a_stopped_row_says_why_and_one_with_no_room_opens_storage).
    let mut dashboard = start(&database);
    dashboard.open_page(Open::Settings(Entry::Storage));
    assert!(dashboard.focus_line().starts_with("Settings: Storage: "), "{}", dashboard.focus_line());
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.page(), None);
}

#[test]
fn the_settings_tile_opens_general_on_its_first_box_and_the_discord_tile_account_on_discord() {
    let database = database();
    let discord = "Settings: Discord: Client ID Type; client id not set, secret not set, \"Not connected\"";
    for (tile, opens, rail) in [(0, discord, "Account"), (1, "Settings: General: Personalisation", "General")] {
        let mut dashboard = start(&database);
        press(&mut dashboard, &[Down, Down]);
        press(&mut dashboard, &vec![Right; tile]);
        assert_eq!(dashboard.home_focus(), Focus::Wide(tile));
        press(&mut dashboard, &[A]);
        assert_eq!(dashboard.focus_line(), opens);
        if tile == 0 {
            // Discord's page, then Account's boxes with focus on Discord.
            press(&mut dashboard, &[B]);
            assert_eq!(dashboard.focus_line(), "Settings: Account: Discord");
        }
        press(&mut dashboard, &[B]);
        assert_eq!(dashboard.focus_line(), format!("Settings: Rail: {rail}"));
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
    dashboard.open_page(Open::Settings(Entry::Tile));
    press(&mut dashboard, &[A, Down, Down, A, B, B, B]);
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::ShowPlayHistory, on: false }]);
    assert_eq!(dashboard.page(), None);
    press(&mut dashboard, &[Right; 9]);
    assert_eq!((dashboard.home_focus(), dashboard.focused_game()), (Focus::Recent(7), Some(7)), "eight games");
    // A is the game's hub, and A again its Play.
    press(&mut dashboard, &[A, A]);
    assert!(dashboard.take_effects().contains(&Effect::Launch(7)), "A twice on the eighth tile plays its game");

    // On again: the tile is back, and A on it is Play history.
    dashboard.open_page(Open::Settings(Entry::Tile));
    press(&mut dashboard, &[A, Down, Down, A, B, B, B]);
    assert_eq!((dashboard.home_focus(), dashboard.focused_game()), (Focus::Recent(6), Some(6)));
    press(&mut dashboard, &[Right, A]);
    assert_eq!(dashboard.focus_line(), "MyGames: PlayHistory: Row(0)");
}

/// Show launcher icons (#248): on as it starts; turned over in place, saved, and read back on the next
/// start. What it hides is `home_recent_row_without_launcher_marks`' picture.
#[test]
fn show_launcher_icons_is_turned_off_saved_and_read_back() {
    let database = database();
    let mut dashboard = personalisation(&database, 3);
    assert_eq!(dashboard.focus_line(), "Settings: Personalisation: Show launcher icons");
    assert!(dashboard.options().show_recent_launcher, "on until it is turned off");
    press(&mut dashboard, &[A]);
    assert!(!dashboard.options().show_recent_launcher);
    assert_eq!(dashboard.focus_line(), "Settings: Personalisation: Show launcher icons", "focus stays");
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::ShowRecentLauncher, on: false }]);
    assert_eq!(database.setting("home.recent_launcher").unwrap().as_deref(), Some("false"));
    assert!(!start(&database).options().show_recent_launcher, "kept across a restart");
}

#[test]
fn use_the_guide_off_leaves_the_guide_button_and_homes_profile_alone() {
    let database = database();
    let mut dashboard = settings(&database);
    press(&mut dashboard, &[Right, Down, A, A]);
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::UseGuide, on: false }]);
    // At once, on the page itself: the Guide button opens nothing.
    press(&mut dashboard, &[Guide]);
    assert!(!dashboard.guide_open());
    // Nor does the profile on Home.
    press(&mut dashboard, &[B, B, B, Left, Left, Left, A]);
    assert_eq!(dashboard.home_focus(), Focus::TopBar(TopButton::Profile));
    assert!(!dashboard.guide_open());

    // On again: both do.
    press(&mut dashboard, &[Right, Right, Right, A, Right, Down, A, A]);
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::UseGuide, on: true }]);
    press(&mut dashboard, &[Guide]);
    assert!(dashboard.guide_open());
}

/// Pause the game while the Guide is open (#270): on as it starts; turned over in place, saved, and read back on
/// the next start. What it does to a game is the executable's (`app::window`'s `Hold`).
#[test]
fn pause_the_game_is_turned_off_saved_and_read_back() {
    let database = database();
    let mut dashboard = settings(&database);
    assert!(dashboard.options().pause_game, "on until it is turned off");
    press(&mut dashboard, &[Right, Down, A, Down]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Pause the game while the Guide is open");
    press(&mut dashboard, &[A]);
    assert!(!dashboard.options().pause_game);
    assert!(dashboard.options().use_guide, "Use the Guide is left as it was");
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Pause the game while the Guide is open", "focus stays");
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::PauseGame, on: false }]);
    assert_eq!(database.setting("guide.pause_game").unwrap().as_deref(), Some("false"));
    assert!(!start(&database).options().pause_game, "kept across a restart");
    // On again.
    press(&mut dashboard, &[A]);
    assert_eq!(save(&mut dashboard, &database), [Change { setting: Setting::PauseGame, on: true }]);
    assert!(start(&database).options().pause_game);
}

/// Pause the game off, focus on it: its box unticked under Use the Guide's, the Now Playing source under both.
#[test]
fn settings_guide_pause_the_game_off() {
    let mut stage = settings_stage();
    stage.press(&[Right, Down, A, Down, A]);
    stage.check("settings-guide-pause-game-off");
}

/// Settings > Guide's Now Playing source, as the Dashboard is told of it and of the apps with a session.
fn now_playing_apps(dashboard: &mut Dashboard, apps: &[&str]) {
    let sessions = apps
        .iter()
        .map(|app| model::MediaSession { app_id: (*app).into(), title: "t".into(), ..Default::default() })
        .collect();
    dashboard.set_media(model::Media { sessions, current: None });
}

/// The Now Playing sources Settings asked to save since the last call.
fn sources(dashboard: &mut Dashboard) -> Vec<String> {
    let effects = dashboard.take_effects().into_iter();
    effects
        .filter_map(|effect| if let Effect::NowPlayingSource(source) = effect { Some(source) } else { None })
        .collect()
}

#[test]
fn the_now_playing_source_lists_any_spotify_and_every_app_seen_and_a_choice_is_saved() {
    let database = database();
    let mut dashboard = settings(&database);
    now_playing_apps(&mut dashboard, &["chrome.exe", "Spotify.exe"]);
    now_playing_apps(&mut dashboard, &["vlc.exe"]);
    press(&mut dashboard, &[Right, Down, A, Down, Down]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Now Playing source");
    // The list opens on the source set now, Spotify until one is chosen.
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.focus_line(), r#"Settings: Guide: Now Playing source, list on "Spotify""#);
    press(&mut dashboard, &[Down, Down]);
    assert_eq!(dashboard.focus_line(), r#"Settings: Guide: Now Playing source, list on "VLC""#);
    press(&mut dashboard, &[Up, Up, Up]);
    assert_eq!(dashboard.focus_line(), r#"Settings: Guide: Now Playing source, list on "Any""#);
    press(&mut dashboard, &[Down, Down, A]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Now Playing source");
    assert_eq!(sources(&mut dashboard), ["Chrome"]);
    assert_eq!(dashboard.now_playing().source(), "Chrome");
    // The same again saves nothing; B closes the list as it was.
    press(&mut dashboard, &[A, A]);
    assert!(sources(&mut dashboard).is_empty());
    press(&mut dashboard, &[A, Down, B]);
    assert_eq!(dashboard.focus_line(), "Settings: Guide: Now Playing source");
    assert_eq!(dashboard.now_playing().source(), "Chrome");

    // As it was left, the next time: a choice before its app has a session.
    let mut again = start(&database);
    again.set_now_playing_source(Some("Chrome"));
    assert_eq!(again.now_playing().choices(), ["Any", "Spotify", "Chrome"]);
}

#[test]
fn every_option_is_as_it_was_left_when_the_core_starts_again() {
    let database = database();
    let mut dashboard = start(&database);
    assert_eq!(dashboard.options(), Options::default(), "nothing is set until it is changed");
    assert!(Setting::ALL.iter().all(|setting| database.setting(setting.key()).unwrap().is_none()));

    // Every checkbox on Personalisation turned over, and the Guide page's two.
    dashboard.open_page(Open::Settings(Entry::Tile));
    press(&mut dashboard, &[A, A, Down, A, Down, A, Down, A, Down, A, B, Down, A, A, Down, A]);
    // And Idle's Dim after, from 2 min to 10 min, which raises Show things after to 10 min with it.
    press(&mut dashboard, &[B, Up, Right, A, A, Down, Down, A]);
    assert_eq!(save(&mut dashboard, &database).len(), 7);
    let ten = Some(std::time::Duration::from_secs(600));
    let left = Options {
        background_delay: false,
        reduce_motion: true,
        show_play_history: false,
        show_recent_launcher: false,
        show_battery_percent: false,
        use_guide: false,
        pause_game: false,
        art_online: true,
        art_key: false,
        art_order: model::ArtOrder::default(),
        ask_whos_playing: true,
        boot_video: BootVideo::XboxSeriesX,
        boot_videos: Found::NONE,
        idle: screens::idle::Times { dim_after: ten, show_after: ten },
        shell: model::shell_mode::ShellMode::Unknown,
        build: screens::settings::about::Build::default(),
        daily_update_check: false,
        updates: model::update::Updates::Unchecked,
        show_game_pass: true,
        discord_messages: false,
        discord_web: model::discord_web::Web::default(),
        game_pass_region: None,
        windows_region: None,
        game_pass_subscription: model::game_pass::Subscription::Unknown,
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
    again.open_page(Open::Settings(Entry::Tile));
    press(&mut again, &[A, Down, A]);
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
        shots::save_shared(&image::RgbImage::from_pixel(96, 31, image::Rgb(colour)), &path);
        library.games[game].hero = Some(path);
    }
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &library, ADDED).unwrap();
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
        let mut page = Settings::showing(Category::General, pane);
        let library = Library::default();
        let around = Around {
            library: &library,
            playing: None,
            options: Options::default(),
            now: std::time::Instant::now(),
            now_playing: screens::now_playing::none(),
            discord: screens::no_discord(),
        };
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

/// The Bundle's picture 28, with the console's four categories: focus on the rail's General, its boxes beside it
/// with no ring.
#[test]
fn settings_general() {
    settings_stage().check("settings-general");
}

/// The Bundle's picture 29: focus on General's first box.
#[test]
fn settings_general_box_focused() {
    let mut stage = settings_stage();
    stage.press(&[Right]);
    stage.check("settings-general-box");
}

/// Each other category, focus on its first box.
#[test]
fn settings_account_games_and_apps_and_system() {
    let mut stage = settings_stage();
    for (category, first) in [("account", "Account: Profiles"), ("games-and-apps", "Games & apps: Launchers")] {
        stage.press(&[Down, Right]);
        assert_eq!(stage.dashboard.focus_line(), format!("Settings: {first}"));
        stage.check(&format!("settings-category-{category}"));
        stage.press(&[B]);
    }
    stage.press(&[Down, Right]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: System");
    stage.check("settings-category-system");
}

/// A page's header: the category in bold, then the page.
#[test]
fn settings_page_header_names_the_category_then_the_page() {
    let mut stage = settings_stage();
    stage.press(&[Right, Right, A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Idle: Dim after");
    stage.check("settings-idle-header");
}

/// The Bundle's picture 31: Personalisation, focus on Background delay; with no info text beside it since #261.
#[test]
fn settings_personalisation_checkbox_focused() {
    let mut stage = settings_stage();
    stage.press(&[Right, A]);
    stage.check("settings-personalisation-checkbox");
    // Another box, turned over: the ring moves with focus, and nothing else does.
    stage.press(&[Down, Down, A]);
    stage.check("settings-personalisation-play-history-off");
}

/// The media folder with these boot videos in it, as the executable finds them.
fn found(videos: &[BootVideo]) -> Found {
    videos.iter().fold(Found::NONE, |found, video| found.with(*video))
}

/// General's Boot video box focused, beside Guide.
#[test]
fn settings_general_boot_video_box() {
    let mut stage = settings_stage();
    stage.press(&[Right, Down, Right]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: General: Boot video");
    stage.check("settings-general-boot-video");
}

/// The Boot video page with every video found, the Bundle's default ticked and focused; then focus on another
/// Row, the tick where it was.
#[test]
fn settings_boot_video() {
    let mut stage = settings_stage();
    stage.dashboard.set_options(Options { boot_videos: found(&BootVideo::ALL), ..Options::default() });
    stage.press(&[Right, Down, Right, A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Boot video: Xbox Series X (2020); ticked");
    stage.check("settings-boot-video");
    stage.press(&[Up, Up, Up]);
    stage.check("settings-boot-video-focus-moved");
}

/// The page with no video found: the line says so, and Off is the one Row.
#[test]
fn settings_boot_video_none_found() {
    let mut stage = settings_stage();
    stage.press(&[Right, Down, Right, A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Boot video: Off; ticked");
    stage.check("settings-boot-video-none-found");
}

/// The Boot video page, as the executable starts the Dashboard: these videos found, this one chosen.
fn boot_video_page(database: &library::Database, videos: &[BootVideo]) -> Dashboard {
    let mut dashboard = settings(database);
    let options = dashboard.options();
    dashboard.set_options(Options { boot_videos: found(videos), ..options });
    press(&mut dashboard, &[Right, Down, Right, A]);
    dashboard.take_effects();
    dashboard
}

#[test]
fn boot_video_lists_off_and_the_videos_found_in_the_bundles_order_focus_on_the_one_ticked() {
    let database = database();
    let mut dashboard = boot_video_page(&database, &[BootVideo::XboxSeriesX, BootVideo::Xbox, BootVideo::XboxOne]);
    // The Bundle's default, Xbox Series X, is ticked until a choice is made, and focus opens on it.
    assert_eq!(dashboard.focus_line(), "Settings: Boot video: Xbox Series X (2020); ticked");
    let mut rows = Vec::new();
    press(&mut dashboard, &[Up; 5]);
    for _ in 0..5 {
        rows.push(dashboard.focus_line());
        press(&mut dashboard, &[Down]);
    }
    assert_eq!(
        rows,
        [
            "Settings: Boot video: Off",
            "Settings: Boot video: Xbox (2001)",
            "Settings: Boot video: Xbox One (2013)",
            "Settings: Boot video: Xbox Series X (2020); ticked",
            "Settings: Boot video: Xbox Series X (2020); ticked",
        ],
        "the top and the foot stop"
    );
    // Left, Right and the rest do nothing; B is General, on Boot video.
    dashboard.take_effects();
    press(&mut dashboard, &[Left, Right, X, Y, Menu]);
    assert_eq!(dashboard.focus_line(), "Settings: Boot video: Xbox Series X (2020); ticked");
    assert!(dashboard.take_effects().is_empty());
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.focus_line(), "Settings: General: Boot video");
}

#[test]
fn a_sets_the_boot_video_at_once_focus_stays_and_it_is_saved_and_read_back() {
    let database = database();
    let mut dashboard = boot_video_page(&database, &[BootVideo::XboxSeriesX, BootVideo::Xbox360]);
    // A on the Row already ticked does nothing.
    press(&mut dashboard, &[A]);
    assert!(dashboard.take_effects().is_empty());
    press(&mut dashboard, &[Up, Up, A]);
    assert_eq!(dashboard.focus_line(), "Settings: Boot video: Off; ticked");
    assert_eq!(dashboard.options().boot_video, BootVideo::Off);
    let effects = dashboard.take_effects();
    assert_eq!(effects.last(), Some(&Effect::BootVideo(BootVideo::Off)), "{effects:?}");
    // Saved as the executable saves it, it is the choice the next time.
    database.set_setting(BOOT_VIDEO, BootVideo::Off.key()).unwrap();
    assert_eq!(start(&database).options().boot_video, BootVideo::Off);
    press(&mut dashboard, &[Down, A]);
    assert_eq!(dashboard.take_effects().last(), Some(&Effect::BootVideo(BootVideo::Xbox360)));
    // A held A sets nothing again.
    dashboard.press(Press { button: A, repeat: true });
    assert!(dashboard.take_effects().is_empty());
}

#[test]
fn a_choice_whose_video_is_gone_ticks_off_and_is_kept_until_another_is_made() {
    let database = database();
    database.set_setting(BOOT_VIDEO, BootVideo::XboxOneX.key()).unwrap();
    let mut dashboard = settings(&database);
    let options = dashboard.options();
    assert_eq!(options.boot_video, BootVideo::XboxOneX);
    dashboard.set_options(Options { boot_videos: found(&[BootVideo::Xbox]), ..options });
    press(&mut dashboard, &[Right, Down, Right, A]);
    assert_eq!(dashboard.focus_line(), "Settings: Boot video: Off; ticked", "Off is what plays");
    // A on it makes Off the choice.
    dashboard.take_effects();
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.take_effects().last(), Some(&Effect::BootVideo(BootVideo::Off)));
}

/// The Bundle's picture 35: the Guide page, focus on Use the Guide, the Now Playing source Spotify under it.
#[test]
fn settings_guide() {
    let mut stage = settings_stage();
    stage.press(&[Right, Down, A]);
    stage.check("settings-guide");
}

/// The Now Playing source's list open, focus on the source set now.
#[test]
fn settings_guide_now_playing_source_list() {
    let mut stage = settings_stage();
    now_playing_apps(&mut stage.dashboard, &["chrome.exe", "vlc.exe"]);
    stage.press(&[Right, Down, A, Down, Down, A]);
    stage.check("settings-guide-now-playing-source-list");
}

use screens::settings::idle::Idle;

fn around(library: &Library) -> Around<'_> {
    Around {
        library,
        playing: None,
        options: Options::default(),
        now: std::time::Instant::now(),
        now_playing: screens::now_playing::none(),
        discord: screens::no_discord(),
    }
}

/// The Bundle's pictures 33 and 34: a dropdown box with focus, and its list open on the choice that is set.
#[test]
fn settings_dropdown_closed_and_open() {
    let mut stage = settings_stage();
    stage.check_pane("settings-dropdown", Box::new(Idle::new()));

    let library = Library::default();
    let mut idle = Idle::new();
    idle.press(Press { button: A, repeat: false }, around(&library));
    assert_eq!(idle.focus(), "Idle: Dim after, list on \"2 min\"", "the list opens on the choice that is set");
    stage.check_pane("settings-dropdown-open", Box::new(idle));

    // The list's foot: the choices have scrolled if they had to, and the last has focus.
    let mut idle = Idle::new();
    for button in [A, Down, Down, Down, Down, Down, Down, Down, Down, Down, Down] {
        idle.press(Press { button, repeat: false }, around(&library));
    }
    assert_eq!(idle.focus(), "Idle: Dim after, list on \"Never\"");
    stage.check_pane("settings-dropdown-open-last", Box::new(idle));
}

/// The controls no page of M3 has yet: Rows, as Boot video's page has them (the Bundle's picture 32, without its line
/// over the Rows since #261), and pane buttons, as Discord's (36).
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
/// scrolled just far enough to show it, and its first rows are cut under the header's row.
#[test]
fn a_pane_of_boxes_that_runs_off_the_screen_scrolls_to_the_focused_box() {
    let opens = |icon, name| Item::Opens { icon, name, pane: |_| Box::new(RowsAndButtons) };
    let check = |name| vec![Item::Check { setting: Setting::ReduceMotion, name }];
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
    // Down to the fifth row, the last that shows whole: nothing has moved.
    for _ in 0..4 {
        boxes.press(Press { button: Down, repeat: false }, around(&library));
        draw(&mut stage, &mut boxes);
        assert_eq!(boxes.scroll(), 0.0, "{}", boxes.focus());
    }
    // The sixth is cut by the screen's foot where it rests: the pane has risen to show it, 32 clear.
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
    stage.dashboard.open_page(Open::Settings(Entry::Tile));
    stage.press(&[A, A, B, B, B]);
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
    stage.dashboard.open_page(Open::Settings(Entry::Tile));
    stage.press(&[A, Down, A]);
    stage.after(0.04);
    assert!(stage.host.store.reduce_motion, "what slides is told to stand in place");
    // The Guide then shows whole in its first frame, where it would slide in.
    stage.press(&[B, B, B, Guide]);
    let first = stage.after(0.04);
    let at_rest = stage.still();
    assert!(shots::difference(&first, &at_rest).share < 0.001, "the Guide was still on its way in");
    // And off again.
    stage.press(&[B]);
    stage.dashboard.open_page(Open::Settings(Entry::Tile));
    stage.press(&[A, Down, A]);
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
    stage.dashboard.set_pad(PadStatus { connected: true, battery: Some(Battery::Percent(57)) });
    let home = stage.still();
    // Kept to look at: the top bar with a wireless pad's battery and its charge.
    home.save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("home-pad-battery.png")).unwrap();
    let with = white(&home);

    stage.dashboard.open_page(Open::Settings(Entry::Tile));
    stage.press(&[A, Down, Down, Down, Down, A, B, B, B]);
    assert!(!stage.dashboard.options().show_battery_percent && stage.dashboard.page().is_none());
    let without = white(&stage.still());
    assert!(without > 100, "the battery's cell is still there: {without}");
    assert!(with > without + 60, "the figure went with the setting: {with} with it, {without} without");
}

/// Games & apps > Art (docs/spec/settings.md, 16.8): no key, a key typed and kept, and the question before the
/// downloaded art goes. The key is never on screen, in the focus log or in an effect's `Debug`.
#[test]
fn settings_art() {
    const KEY: &str = "f00dfeedcafe1234";
    let mut stage = settings_stage();
    stage.press(&[Down, Down, Right, Down, Down, Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Games & apps: Art");
    stage.check("settings-games-and-apps-art-box");
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Type");
    stage.check("settings-art");

    // Type opens the Console keyboard, the key shown as dots; Menu keeps it.
    stage.press(&[A]);
    assert!(stage.dashboard.typed(screens::keyboard::Typed::Text(KEY)));
    assert!(!stage.dashboard.focus_line().contains(KEY), "{}", stage.dashboard.focus_line());
    stage.press(&[Menu]);
    let effects = stage.dashboard.take_effects();
    let kept: Vec<_> = effects.iter().filter(|e| matches!(e, Effect::ArtKey(_))).collect();
    assert_eq!(kept, [&Effect::ArtKey(Some(screens::Secret(KEY.into())))]);
    assert!(!format!("{effects:?}").contains(KEY));
    assert!(stage.dashboard.options().art_key);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Type");
    stage.press(&[Right]);
    stage.check("settings-art-key-set");

    // Clear forgets it; with none, Clear does nothing.
    stage.press(&[A]);
    assert_eq!(asked(&mut stage), [Effect::ArtKey(None)]);
    assert!(!stage.dashboard.options().art_key);
    stage.press(&[A]);
    assert!(asked(&mut stage).is_empty());

    // Clear downloaded art asks first: Cancel, or B, does nothing; Clear clears.
    stage.press(&[Down, A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Prompt(Clear) of Clear downloaded art");
    stage.check("settings-art-clear-prompt");
    stage.press(&[B]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Clear downloaded art");
    stage.press(&[A, Down, A]);
    assert!(!asked(&mut stage).contains(&Effect::ClearArt));
    stage.press(&[A, A]);
    assert!(asked(&mut stage).contains(&Effect::ClearArt));
    // Left from the first column, as B, goes back to Games & apps' boxes on Art.
    stage.press(&[Up, Left]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Games & apps: Art");
}

/// Settings > Art's sources (#246; docs/spec/settings.md, 16.8): the four online sources in their order under
/// Download art online, Playnite's at first, each numbered by its place among those on (#262); A turns one off and
/// on, its menu moves it up or down as Groups' does, focus following it, each change asked as `Effect::ArtOrder` and
/// in force at once; the page scrolls to them.
#[test]
fn settings_art_sources() {
    use model::{ArtOrder, ArtSource};
    let mut stage = settings_stage();
    stage.press(&[Down, Down, Right, Down, Down, Down, A, Down, Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Download art online");
    stage.press(&[Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Source Steam, 1 of 4, on, number 1");
    stage.press(&[Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Source Xbox, 2 of 4, on, number 2");
    stage.check("settings-art-sources");

    // A turns PlayStation off, and on again.
    stage.press(&[Down, A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Source PlayStation, 3 of 4, off");
    let off = ArtOrder::default().switched(ArtSource::PlayStation);
    assert_eq!(asked(&mut stage), [Effect::ArtOrder(off)]);
    assert_eq!(stage.dashboard.options().art_order.text(), "steam,xbox,-playstation,steamgriddb");
    // Off, it has no number, and the ones after it number on from the one before it (#262).
    stage.press(&[Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Source SteamGridDB, 4 of 4, on, number 3");
    stage.check("settings-art-sources-off");
    stage.press(&[Up, A]);
    assert_eq!(asked(&mut stage), [Effect::ArtOrder(ArtOrder::default())]);

    // SteamGridDB's menu: Move up alone, at the foot. Up twice, the menu staying, focus going with it.
    stage.press(&[Down, Menu]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Menu(Move up) of SteamGridDB");
    stage.check("settings-art-sources-menu");
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Menu(Move up) of SteamGridDB");
    stage.press(&[A]);
    let moved = ArtOrder::default().moved(ArtSource::SteamGridDb, -2);
    assert_eq!(stage.dashboard.options().art_order, moved);
    assert_eq!(moved.text(), "steam,steamgriddb,xbox,playstation");
    stage.press(&[B]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Source SteamGridDB, 2 of 4, on, number 2");
    let orders: Vec<_> = asked(&mut stage).into_iter().filter(|e| matches!(e, Effect::ArtOrder(_))).collect();
    assert_eq!(orders.last(), Some(&Effect::ArtOrder(moved)));
    // Up from the first source is Download art online; Left is back.
    stage.press(&[Up, Up]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Download art online");
    stage.press(&[Down, Left]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Games & apps: Art");
}

/// Settings > Art's Refresh all art and details (#190; docs/spec/settings.md, 16.8): beside Clear downloaded art,
/// asked first, and the line under the two saying how it goes: at rest, working (where A does nothing), paused and
/// done, as the executable says (`Dashboard::set_art_refresh`), the focus log saying it after the `;`.
#[test]
fn settings_art_refresh() {
    use model::ArtRefresh;
    let mut stage = settings_stage();
    stage.press(&[Down, Down, Right, Down, Down, Down, A, Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Clear downloaded art");
    stage.press(&[Right]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Refresh all art and details");
    stage.check("settings-art-refresh");
    // Up from either of the two is Type; Down from either is Download art online.
    stage.press(&[Up]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Type");
    stage.press(&[Down, Right, Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Download art online");
    stage.press(&[Up, Right]);

    // Asked first: Cancel, or B, does nothing.
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Prompt(Refresh) of Refresh all art and details");
    stage.check("settings-art-refresh-prompt");
    stage.press(&[B]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Refresh all art and details");
    stage.press(&[A, Down, A]);
    assert!(asked(&mut stage).iter().all(|e| !matches!(e, Effect::RefreshArt(_))));
    stage.press(&[A, A]);
    assert!(asked(&mut stage).contains(&Effect::RefreshArt(None)));

    // Working: the line says how far it has got, beside a spinner, and Refresh does nothing.
    assert!(stage.dashboard.set_art_refresh(ArtRefresh::Working { done: 3, of: 14 }));
    assert!(!stage.dashboard.set_art_refresh(ArtRefresh::Working { done: 3, of: 14 }));
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Refresh all art and details; refreshing 3 of 14");
    stage.check("settings-art-refreshing");
    stage.press(&[A]);
    assert!(asked(&mut stage).is_empty());
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Refresh all art and details; refreshing 3 of 14");

    // Paused, then done.
    stage.dashboard.set_art_refresh(ArtRefresh::Paused);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Refresh all art and details; refresh paused");
    stage.dashboard.set_art_refresh(ArtRefresh::Done { pictures: 9 });
    assert_eq!(stage.dashboard.focus_line(), "Settings: Art: Refresh all art and details; refreshed: 9 pictures");
    stage.check("settings-art-refreshed");
    // Done, it can be asked for again.
    stage.press(&[A]);
    assert_eq!(
        stage.dashboard.focus_line(),
        "Settings: Art: Prompt(Refresh) of Refresh all art and details; refreshed: 9 pictures"
    );
}

/// System > System (docs/spec/shell-mode.md, 2): the question before turning shell mode on, none before
/// turning it off, nothing while it is refused, and the page following where the executable says it stands.
#[test]
fn settings_system() {
    use model::shell_mode::{Refusal, ShellMode};
    let mut stage = settings_stage();
    let shell = |stage: &mut Stage, shell| {
        let options = stage.dashboard.options();
        stage.dashboard.set_options(Options { shell, ..options });
    };
    shell(&mut stage, ShellMode::Off);
    stage.press(&[Down, Down, Down, Right]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: System");
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: Start with Windows as the shell");
    stage.check("settings-system-off");

    // On asks first: Cancel, or B, does nothing; Turn on asks the executable.
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: Prompt(Turn on) of Start with Windows as the shell");
    stage.check("settings-system-prompt");
    stage.press(&[B]);
    assert!(asked(&mut stage).is_empty());
    stage.press(&[A, Down, A]);
    assert!(asked(&mut stage).is_empty());
    stage.press(&[A, A]);
    assert_eq!(asked(&mut stage), [Effect::ShellMode(true)]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: Start with Windows as the shell");

    // On, A turns it off at once.
    shell(&mut stage, ShellMode::On);
    stage.check("settings-system-on");
    stage.press(&[A]);
    assert_eq!(asked(&mut stage), [Effect::ShellMode(false)]);

    // Refused: the line says why, and A does nothing.
    shell(&mut stage, ShellMode::Refused(Refusal::BundleBootShell));
    stage.check("settings-system-refused");
    stage.press(&[A]);
    assert!(asked(&mut stage).is_empty());
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: Start with Windows as the shell");
    stage.press(&[B]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: System");
}

/// System > About (docs/spec/settings.md, 16.10; docs/spec/updates.md) on a stage, with a made-up build so
/// that the picture never changes with a release or a commit, and updates where `updates` stand.
fn about_stage(updates: model::update::Updates) -> Stage {
    use screens::settings::about::Build;
    let mut stage = settings_stage();
    let options = stage.dashboard.options();
    stage.dashboard.set_options(Options { build: Build { version: "1.2.3", commit: "0123abc" }, updates, ..options });
    stage.press(&[Down, Down, Down, Right, Right]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: About");
    stage.press(&[A]);
    stage
}

/// About as it opens: the version, updates not checked yet, focus on Check for updates; its three controls walked,
/// and B goes back to its box.
#[test]
fn settings_about() {
    use model::update::Updates;
    let mut stage = about_stage(Updates::Unchecked);
    assert_eq!(stage.dashboard.focus_line(), "Settings: About: Version 1.2.3 (0123abc); Check for updates; unchecked");
    stage.check("settings-about");
    stage.press(&[Right]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: About: Version 1.2.3 (0123abc); Install; unchecked");
    stage.press(&[Down]);
    assert_eq!(
        stage.dashboard.focus_line(),
        "Settings: About: Version 1.2.3 (0123abc); Check for updates every day; unchecked"
    );
    stage.press(&[Up, B]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: About");
    stage.press(&[A, Left]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: System: About");
}

/// Check for updates asks the executable; Install asks only with an update ready and no game running; the daily
/// check is an option, off until it is turned on, and saved.
#[test]
fn about_asks_for_a_check_installs_only_what_is_ready_and_turns_the_daily_check_over() {
    use model::update::{Updates, Version};
    let mut stage = about_stage(Updates::Unchecked);
    asked(&mut stage);
    stage.press(&[A]);
    assert!(matches!(asked(&mut stage)[..], [Effect::CheckForUpdates]));
    // Nothing more while one is under way.
    let options = stage.dashboard.options();
    stage.dashboard.set_options(Options { updates: Updates::Checking, ..options });
    stage.press(&[A]);
    assert!(asked(&mut stage).is_empty());
    // Install, with nothing ready: nothing; with one ready: asked.
    stage.press(&[Right]);
    stage.dashboard.set_options(Options { updates: Updates::UpToDate(Version::new(1, 2, 3)), ..options });
    stage.press(&[A]);
    assert!(asked(&mut stage).is_empty());
    stage.dashboard.set_options(Options { updates: Updates::Ready(Version::new(1, 3, 0)), ..options });
    stage.press(&[A]);
    assert!(matches!(asked(&mut stage)[..], [Effect::InstallUpdate]));
    // The daily check: off, then on and saved.
    assert!(!stage.dashboard.options().daily_update_check);
    stage.press(&[Down, A]);
    let change = Change { setting: Setting::DailyUpdateCheck, on: true };
    assert!(matches!(asked(&mut stage)[..], [Effect::Set(c)] if c == change));
    assert!(stage.dashboard.options().daily_update_check);
    // The shell says where updates stand: the focus line follows at once, before the page is drawn again.
    let refused = Updates::BadDownload(Version::new(1, 3, 0));
    stage.dashboard.set_options(Options { updates: refused, ..stage.dashboard.options() });
    assert_eq!(
        stage.dashboard.focus_line(),
        "Settings: About: Version 1.2.3 (0123abc); Check for updates every day; bad-download 1.3.0"
    );
}

/// About with a version ready to install, focus on Install, the daily check on.
#[test]
fn settings_about_ready() {
    use model::update::{Updates, Version};
    let mut stage = about_stage(Updates::Ready(Version::new(1, 3, 0)));
    stage.press(&[Down, A, Up, Right]);
    assert!(stage.dashboard.options().daily_update_check);
    assert_eq!(stage.dashboard.focus_line(), "Settings: About: Version 1.2.3 (0123abc); Install; ready 1.3.0");
    stage.check("settings-about-ready");
}

/// About when the update address answered 404, as GitHub's does while the repository is private: said plainly on
/// the page, and no Toast.
#[test]
fn settings_about_not_public() {
    use model::update::Updates;
    let mut stage = about_stage(Updates::NotPublic);
    assert!(stage.dashboard.notifications().is_empty());
    stage.check("settings-about-not-public");
}

/// About when a download did not match its manifest's SHA-256.
#[test]
fn settings_about_bad_download() {
    use model::update::{Updates, Version};
    let mut stage = about_stage(Updates::BadDownload(Version::new(1, 3, 0)));
    stage.check("settings-about-bad-download");
}

/// About while a check is under way: Check for updates dimmed, the Spinner beside the buttons.
#[test]
fn settings_about_checking() {
    use model::update::Updates;
    let mut stage = about_stage(Updates::Checking);
    stage.check("settings-about-checking");
}

/// Account > Discord (docs/spec/settings.md, 6.9 and 9.5; docs/discord.md): the Bundle's picture 36, its
/// three rows of two walked, Type and Paste asked of the shell, the Secret never on screen or in the log, and the
/// page following the connection as the shell tells it.
#[test]
fn settings_discord() {
    use model::discord::{Ask, Discord, Field};
    const ID: &str = "1290000000000000001";
    const SECRET: &str = "s3cr3t-s3cr3t-s3cr3t-s3cr3t-0001";
    let mut stage = settings_stage();
    stage.press(&[Down, Right, Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Account: Discord");
    stage.press(&[A]);
    let line = |rest: &str| format!("Settings: Discord: {rest}; client id not set, secret not set, \"Not connected\"");
    assert_eq!(stage.dashboard.focus_line(), line("Client ID Type"));
    stage.check("settings-discord");

    // Three rows of two: Up and Down keep the column, Left and Right stay in the row; dimmed buttons take focus.
    for (button, at) in [
        (Right, "Client ID Paste"),
        (Right, "Client ID Paste"),
        (Down, "Client Secret Paste"),
        (Down, "Disconnect"),
        (Down, "Disconnect"),
        (Left, "Connect"),
        (Up, "Client Secret Type"),
        (Up, "Client ID Type"),
        (Up, "Client ID Type"),
    ] {
        stage.press(&[button]);
        assert_eq!(stage.dashboard.focus_line(), line(at), "after {button:?}");
    }
    // Connect and Disconnect do nothing with nothing set up.
    stage.press(&[Down, Down, A, Right, A]);
    assert!(asked(&mut stage).is_empty());

    // Paste asks the shell, which reads the clipboard.
    stage.press(&[Up, Up, A, Down, A]);
    let pastes = [Ask::Paste(Field::ClientId), Ask::Paste(Field::ClientSecret)].map(Effect::Discord);
    assert_eq!(asked(&mut stage), pastes);

    // Type: the Client ID from empty; the Secret as dots, never in the log or an effect's Debug.
    stage.press(&[Up, Left, A]);
    assert!(stage.dashboard.typed(screens::keyboard::Typed::Text(ID)));
    stage.press(&[Menu]);
    assert_eq!(asked(&mut stage), [Effect::Discord(Ask::ClientId(ID.into()))]);
    stage.press(&[Down, A]);
    assert!(stage.dashboard.typed(screens::keyboard::Typed::Text(SECRET)));
    assert!(!stage.dashboard.focus_line().contains(SECRET), "{}", stage.dashboard.focus_line());
    stage.check("settings-discord-secret-entry");
    stage.press(&[Menu]);
    let effects = asked(&mut stage);
    assert_eq!(effects, [Effect::Discord(Ask::ClientSecret(SECRET.into()))]);
    assert!(!format!("{effects:?}").contains(SECRET));
    // An empty Secret kept leaves the saved one alone: nothing is asked.
    stage.press(&[A, Menu]);
    assert!(asked(&mut stage).is_empty());

    // The shell answers with how the connection is: the page follows it, the Client ID shown, the Secret not.
    let saved = Discord { client_id: Some(ID.into()), has_secret: true, ..Discord::default() };
    stage.dashboard.set_discord(&saved);
    stage.press(&[Down]);
    let line =
        |rest: &str, status: &str| format!("Settings: Discord: {rest}; client id set, secret saved, \"{status}\"");
    assert_eq!(stage.dashboard.focus_line(), line("Connect", "Not connected"));
    stage.press(&[A]);
    assert_eq!(asked(&mut stage), [Effect::Discord(Ask::Connect)]);
    // Type on the Client ID starts from the one saved.
    stage.press(&[Up, Up, A]);
    assert!(stage.dashboard.focus_line().contains(&format!("\"{ID}|\"")), "{}", stage.dashboard.focus_line());
    stage.press(&[B]);
    let connected = Discord {
        has_tokens: true,
        connected: true,
        status: "Connected as E2E Tester".into(),
        user: Some("E2E Tester".into()),
        ..saved
    };
    stage.dashboard.set_discord(&connected);
    stage.press(&[Down, Down, Right]);
    assert_eq!(stage.dashboard.focus_line(), line("Disconnect", "Connected as E2E Tester"));
    stage.check("settings-discord-connected");
    stage.press(&[A]);
    assert_eq!(asked(&mut stage), [Effect::Discord(Ask::Disconnect)]);

    // Left from the first column, as B, goes back to Account's boxes on Discord.
    stage.press(&[Left, Left]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Account: Discord");
}

/// What the Dashboard asked of the shell since last time, but its sounds.
/// Account > Discord messages (#197; docs/spec/settings.md 16.12, docs/discord-web.md): off by default with the terms
/// said under the switch; on, Sign in opens Discord's sign-in page, where the pad sends keys and A on a field opens
/// the Console keyboard, whose text goes to the session alone; signed in, the page closes itself; Sign out asks the
/// session.
#[test]
fn settings_discord_messages() {
    use model::discord_web::{Web, WebAsk, WebField, WebKey, WebState};
    let mut stage = settings_stage();
    stage.press(&[Down, Right, Right, Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Account: Discord messages");
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Discord messages: Use Discord messages; off, off");
    stage.check("settings-discord-messages-off");
    // Off, there is nothing under the switch to go to.
    stage.press(&[Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Discord messages: Use Discord messages; off, off");

    // On: saved, and the session tells how it stands.
    stage.press(&[A]);
    let on = screens::settings::Change { setting: screens::settings::Setting::DiscordMessages, on: true };
    assert_eq!(asked(&mut stage), [Effect::Set(on)]);
    let set = |stage: &mut Stage, web: Web| {
        stage.dashboard.set_options(screens::Options { discord_web: web, ..stage.dashboard.options() })
    };
    set(&mut stage, Web { state: WebState::SignedOut, ..Web::default() });
    stage.press(&[Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Discord messages: Sign in; on, signed out");
    stage.check("settings-discord-messages-signed-out");

    // Sign in: the sign-in page over Settings, shown while it has the pad.
    stage.press(&[A]);
    assert_eq!(stage.dashboard.page(), Some("DiscordSignIn"));
    assert!(stage.dashboard.discord_sign_in_shown() && stage.dashboard.discord_sign_in_open());
    stage.check("discord-sign-in");
    stage.press(&[Down, Up, X]);
    let keys = [WebKey::Tab, WebKey::ShiftTab, WebKey::Space].map(|key| Effect::DiscordWeb(WebAsk::Key(key)));
    assert_eq!(asked(&mut stage), keys);
    // A on a password field: the Console keyboard, the web view put away under it, the text to the session alone.
    set(&mut stage, Web { state: WebState::SigningIn, field: Some(WebField::Password), kept: false });
    stage.press(&[A]);
    assert_eq!(stage.dashboard.page(), Some("TextEntry"));
    assert!(!stage.dashboard.discord_sign_in_shown() && stage.dashboard.discord_sign_in_open());
    assert!(stage.dashboard.typed(screens::keyboard::Typed::Text("hunter2")));
    assert!(!stage.dashboard.focus_line().contains("hunter2"), "{}", stage.dashboard.focus_line());
    stage.press(&[Menu]);
    let effects = asked(&mut stage);
    assert_eq!(effects, [Effect::DiscordWeb(WebAsk::Text("hunter2".into()))]);
    assert!(!format!("{effects:?}").contains("hunter2"));
    assert!(stage.dashboard.discord_sign_in_shown());

    // Signed in: the page closes itself, back on Discord messages.
    set(&mut stage, Web { state: WebState::SignedIn, field: None, kept: true });
    stage.dashboard.discord_signed_in();
    assert!(!stage.dashboard.discord_sign_in_open());
    assert_eq!(stage.dashboard.focus_line(), "Settings: Discord messages: Sign in; on, signed in, kept");
    stage.check("settings-discord-messages-signed-in");
    // Sign in does nothing now; Sign out asks the session.
    stage.press(&[A]);
    assert_eq!(stage.dashboard.page(), Some("Settings"));
    stage.press(&[Right, A]);
    assert_eq!(asked(&mut stage), [Effect::DiscordWeb(WebAsk::SignOut)]);
}

fn asked(stage: &mut Stage) -> Vec<Effect> {
    stage.dashboard.take_effects().into_iter().filter(|e| !matches!(e, Effect::Sound(_))).collect()
}

// --- Games & apps > Launchers (docs/spec/launchers.md) ---

/// Now, in Unix seconds: the page says how long ago each launcher was read.
fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

/// The fake library's first five games Steam's and the rest Epic's, both read five minutes ago; GOG, the EA app,
/// Ubisoft Connect and Battle.net not on this PC; Xbox never read.
fn launchers_database() -> library::Database {
    let fake = screens::fake_library();
    let (steam, epic) = fake.games.split_at(5);
    let read = library::sources::Read {
        found: vec![
            ("steam", Library { games: steam.to_vec(), ..Library::default() }),
            ("epic", Library { games: epic.to_vec(), ..Library::default() }),
        ],
        failed: Vec::new(),
        absent: vec!["gog", "ea", "ubisoft", "battlenet"],
    };
    let mut database = library::Database::in_memory().unwrap();
    database.sync_all(&read, unix_now() - 300).unwrap();
    database
}

/// Games & apps, focus on Launchers' box.
fn to_launchers(dashboard: &mut Dashboard) {
    dashboard.open_page(Open::Settings(Entry::Rail));
    press(dashboard, &[Down, Down, Right]);
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Launchers");
}

/// Carries out what the page asked of the shell, as the executable does, the snapshot after handed back. The
/// update is the test's to say.
fn carry_out(dashboard: &mut Dashboard, database: &library::Database) -> Vec<Effect> {
    let asked: Vec<Effect> = dashboard.take_effects().into_iter().filter(|e| !matches!(e, Effect::Sound(_))).collect();
    for effect in &asked {
        if let Effect::Launcher { key, on } = effect {
            database.set_launcher(key, *on).unwrap();
            dashboard.set_library(database.snapshot().unwrap());
        }
    }
    asked
}

fn with_update(database: &library::Database, update: model::launcher::Update) -> Library {
    let mut library = database.snapshot().unwrap();
    library.launchers.update = update;
    library
}

fn names_of(dashboard: &Dashboard, source: &str) -> Vec<String> {
    dashboard.library().games.iter().filter(|g| g.source == source).map(|g| g.name.clone()).collect()
}

#[test]
fn launchers_are_walked_by_those_that_can_be_switched_and_one_off_leaves_every_list() {
    let database = launchers_database();
    let mut dashboard = start(&database);
    to_launchers(&mut dashboard);
    press(&mut dashboard, &[A]);
    let line = |row: &str, count: &str| format!("Settings: Launchers: {row}; {count}; ready");
    assert_eq!(dashboard.focus_line(), line("Steam; on, 5 games", "14 games from 2 launchers"));
    // Down passes over the four not on this PC, to Xbox, Update now and the Game Pass controls; the foot stops.
    press(&mut dashboard, &[Down, Down]);
    assert_eq!(dashboard.focus_line(), line("Xbox; on, 0 games", "14 games from 2 launchers"));
    press(&mut dashboard, &[Down]);
    assert_eq!(dashboard.focus_line(), "Settings: Launchers: Update now; 14 games from 2 launchers; ready");
    press(&mut dashboard, &[Down, Down, Down]);
    assert_eq!(
        dashboard.focus_line(),
        "Settings: Launchers: Game Pass region; 14 games from 2 launchers; ready; Game Pass on, Windows'"
    );
    press(&mut dashboard, &[Up, Up, Up, Up]);
    assert_eq!(dashboard.focus_line(), line("Epic Games; on, 9 games", "14 games from 2 launchers"));

    // Epic off: its games leave the library every page shows, and the count; on again, they are back.
    let epic = names_of(&dashboard, "epic");
    assert_eq!(epic.len(), 9);
    press(&mut dashboard, &[A]);
    assert_eq!(carry_out(&mut dashboard, &database), [Effect::Launcher { key: "epic".into(), on: false }]);
    assert_eq!(dashboard.focus_line(), line("Epic Games; off, 9 games", "5 games from 1 launcher"));
    assert!(dashboard.library().games.iter().all(|g| g.source == "steam"));
    press(&mut dashboard, &[A]);
    assert_eq!(carry_out(&mut dashboard, &database), [Effect::Launcher { key: "epic".into(), on: true }]);
    assert_eq!(dashboard.focus_line(), line("Epic Games; on, 9 games", "14 games from 2 launchers"));
    assert_eq!(names_of(&dashboard, "epic"), epic);
    // Left, as B, is Games & apps' boxes on Launchers.
    press(&mut dashboard, &[Left]);
    assert_eq!(dashboard.focus_line(), "Settings: Games & apps: Launchers");
}

#[test]
fn a_launcher_a_game_is_playing_from_stays_on_and_the_page_says_why() {
    let database = launchers_database();
    let mut dashboard = start(&database);
    let playing = dashboard.library().games.iter().position(|g| g.source == "steam").unwrap();
    let name = dashboard.library().games[playing].name.clone();
    dashboard.launch_started(playing);
    dashboard.game_booted();
    dashboard.show_dashboard();
    to_launchers(&mut dashboard);
    press(&mut dashboard, &[A]);
    dashboard.take_effects();
    press(&mut dashboard, &[A]);
    assert_eq!(carry_out(&mut dashboard, &database), []);
    let refused = format!(
        "Settings: Launchers: Steam; on, 5 games; 14 games from 2 launchers; refused: Quit {name} before switching \
         Steam off."
    );
    assert_eq!(dashboard.focus_line(), refused);
    // Another launcher is switched as ever, and moving on forgets why.
    press(&mut dashboard, &[Down]);
    assert!(dashboard.focus_line().ends_with("; ready"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[A]);
    assert_eq!(carry_out(&mut dashboard, &database), [Effect::Launcher { key: "epic".into(), on: false }]);
}

#[test]
fn update_now_is_asked_once_while_it_works_and_says_what_it_changed() {
    let database = launchers_database();
    let mut dashboard = start(&database);
    to_launchers(&mut dashboard);
    press(&mut dashboard, &[A, Down, Down, Down]);
    dashboard.take_effects();
    press(&mut dashboard, &[A]);
    assert_eq!(carry_out(&mut dashboard, &database), [Effect::UpdateLaunchers]);
    dashboard.set_library(with_update(&database, model::launcher::Update::Working));
    assert_eq!(dashboard.focus_line(), "Settings: Launchers: Update now; 14 games from 2 launchers; updating");
    press(&mut dashboard, &[A]);
    assert_eq!(carry_out(&mut dashboard, &database), [], "pressed again while it works: nothing");
    let done = model::launcher::Update::Done("1 new game, play time for 1".into());
    dashboard.set_library(with_update(&database, done));
    assert_eq!(
        dashboard.focus_line(),
        "Settings: Launchers: Update now; 14 games from 2 launchers; updated: 1 new game, play time for 1"
    );
    press(&mut dashboard, &[A]);
    assert_eq!(carry_out(&mut dashboard, &database), [Effect::UpdateLaunchers], "and again once it is done");
}

/// The page at rest, focus on Steam; Epic off, focus on Update now while it works; done; a launcher refused.
#[test]
fn settings_launchers() {
    let database = launchers_database();
    let mut stage = Stage::new(database.snapshot().unwrap());
    to_launchers(&mut stage.dashboard);
    stage.check("settings-games-and-apps-launchers-box");
    stage.press(&[A]);
    stage.check("settings-launchers");

    stage.press(&[Down, A]);
    carry_out(&mut stage.dashboard, &database);
    stage.press(&[Down, Down]);
    stage.dashboard.set_library(with_update(&database, model::launcher::Update::Working));
    let line = stage.dashboard.focus_line();
    assert!(line.starts_with("Settings: Launchers: Update now; 5 games from 1 launcher"), "{line}");
    stage.check("settings-launchers-updating");
    let done = model::launcher::Update::Done("1 new game, play time for 1".into());
    stage.dashboard.set_library(with_update(&database, done));
    stage.check("settings-launchers-updated");

    // Steam switched off while one of its games plays: refused, and why.
    drop(stage);
    let database = launchers_database();
    let mut stage = Stage::new(database.snapshot().unwrap());
    let playing = stage.dashboard.library().games.iter().position(|g| g.source == "steam").unwrap();
    stage.dashboard.launch_started(playing);
    stage.dashboard.game_booted();
    stage.dashboard.show_dashboard();
    to_launchers(&mut stage.dashboard);
    stage.press(&[A, A]);
    assert!(stage.dashboard.focus_line().contains("; refused: Quit "), "{}", stage.dashboard.focus_line());
    stage.check("settings-launchers-refused");
}

/// Show Game Pass games asks for the setting turned over; the region's list opens on Windows' region and a market
/// chosen there is asked for and shown at once (#277).
#[test]
fn the_game_pass_controls_ask_for_the_switch_and_the_region() {
    let database = launchers_database();
    let mut dashboard = start(&database);
    let gb = model::game_pass::Region::parse("GB");
    dashboard.set_options(screens::Options { windows_region: gb, ..dashboard.options() });
    to_launchers(&mut dashboard);
    press(&mut dashboard, &[A, Down, Down, Down, Down]);
    assert_eq!(
        dashboard.focus_line(),
        "Settings: Launchers: Show Game Pass games; 14 games from 2 launchers; ready; Game Pass on, GB (Windows')"
    );
    dashboard.take_effects();
    press(&mut dashboard, &[A]);
    let off = screens::settings::Change { setting: screens::settings::Setting::ShowGamePass, on: false };
    assert_eq!(carry_out(&mut dashboard, &database), [Effect::Set(off)]);
    assert!(dashboard.focus_line().ends_with("; Game Pass off, GB (Windows')"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[Down, A]);
    assert!(
        dashboard
            .focus_line()
            .starts_with("Settings: Launchers: Game Pass region, list on \"Windows' region (United Kingdom)\"; "),
        "{}",
        dashboard.focus_line()
    );
    // Down to Albania, the first market, and chosen.
    press(&mut dashboard, &[Down, A]);
    let albania = model::game_pass::Region::parse("AL");
    assert_eq!(carry_out(&mut dashboard, &database), [Effect::GamePassRegion(albania)]);
    assert!(dashboard.focus_line().ends_with("; Game Pass off, AL"), "{}", dashboard.focus_line());
    // Opened again on it; Up is Windows' again.
    press(&mut dashboard, &[A, Up, A]);
    assert_eq!(carry_out(&mut dashboard, &database), [Effect::GamePassRegion(None)]);
    // B closes the list unchanged.
    press(&mut dashboard, &[A, Down, B]);
    assert_eq!(carry_out(&mut dashboard, &database), []);
    assert!(
        dashboard.focus_line().starts_with("Settings: Launchers: Game Pass region; "),
        "{}",
        dashboard.focus_line()
    );
}

/// The Game Pass controls, focus on the region with its list open on the United Kingdom (#277).
#[test]
fn settings_launchers_game_pass() {
    let database = launchers_database();
    let mut stage = Stage::new(database.snapshot().unwrap());
    let gb = model::game_pass::Region::parse("GB");
    let options = stage.dashboard.options();
    stage.dashboard.set_options(screens::Options { windows_region: gb, game_pass_region: gb, ..options });
    to_launchers(&mut stage.dashboard);
    stage.press(&[A, Down, Down, Down, Down, Down, A]);
    let line = stage.dashboard.focus_line();
    assert!(line.starts_with("Settings: Launchers: Game Pass region, list on \"United Kingdom\""), "{line}");
    stage.check("settings-launchers-game-pass");
}

// --- Games & apps > Helpers (docs/spec/helpers.md) ---

/// The two helpers as the table has them: legendary as `legendary` stands, gogdl added.
fn helpers_shown(legendary: model::helper::HelperState) -> Vec<model::helper::Helper> {
    use model::helper::{Helper, HelperState};
    let helper = |key: &str, store: &str, version: &str, source: &str, state| Helper {
        key: key.into(),
        store: store.into(),
        version: version.into(),
        licence: "GPL-3.0".into(),
        source: source.into(),
        state,
        account: Default::default(),
    };
    vec![
        helper("legendary", "Epic Games", "0.21.1", "https://github.com/legendary-gl/legendary", legendary),
        helper(
            "gogdl",
            "GOG",
            "1.4.0",
            "https://github.com/Heroic-Games-Launcher/heroic-gogdl",
            HelperState::Added { version: "1.4.0".into() },
        ),
    ]
}

/// Helpers, under Art: each helper with its licence and source and how it stands; Add asks first, the licence and
/// source in the question, and only Yes asks the executable; Remove asks too; the page follows the snapshot.
#[test]
fn settings_helpers() {
    use model::helper::{HelperAsk, HelperState};
    let mut stage = settings_stage();
    stage.dashboard.set_helpers(helpers_shown(HelperState::NotAdded));
    stage.press(&[Down, Down, Right, Right, Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Games & apps: Helpers");
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Helpers: legendary; Add; not added");
    stage.check("settings-helpers");

    // Add asks; B, or No, fetches nothing.
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Helpers: legendary; Add; not added; asking Add legendary?");
    stage.check("settings-helpers-question");
    stage.press(&[B]);
    assert!(asked(&mut stage).is_empty());
    stage.press(&[A, Down, A]);
    assert!(asked(&mut stage).is_empty());
    stage.press(&[A, A]);
    assert_eq!(asked(&mut stage), [Effect::Helper(HelperAsk::Add("legendary".into()))]);

    // Fetching: no button, a spinner; then refused, Add again with why.
    stage.dashboard.set_helpers(helpers_shown(HelperState::Fetching));
    assert_eq!(stage.dashboard.focus_line(), "Settings: Helpers: legendary; busy; fetching");
    stage.press(&[A]);
    assert!(asked(&mut stage).is_empty());
    let refused = "The download wasn't the file expected, so it wasn't kept.".to_string();
    stage.dashboard.set_helpers(helpers_shown(HelperState::Failed(refused)));
    assert_eq!(stage.dashboard.focus_line(), "Settings: Helpers: legendary; Add; failed");
    stage.check("settings-helpers-refused");

    // gogdl, added: Remove asks, and Remove removes.
    stage.press(&[Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Helpers: gogdl; Remove; added 1.4.0");
    stage.press(&[A, A]);
    assert_eq!(asked(&mut stage), [Effect::Helper(HelperAsk::Remove("gogdl".into()))]);
    stage.press(&[B]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Games & apps: Helpers");
}

// --- Games & apps > Set up this PC (docs/spec/set-up.md) ---

/// The launchers' apps as the executable shows them: Steam installed, Battle.net as `battlenet`, the rest not.
fn clients_shown(battlenet: model::setup::ClientState) -> Vec<model::setup::Client> {
    use model::setup::{Client, ClientState};
    let client = |key: &str, name: &str, vendor: &str, address: &str, size, state| Client {
        key: key.into(),
        name: name.into(),
        vendor: vendor.into(),
        version: "1.0".into(),
        address: address.into(),
        size,
        window: key == "battlenet",
        uninstall_window: key == "battlenet",
        removing: "Its games stay, but start and update only through it.".into(),
        state,
    };
    vec![
        client("steam", "Steam", "Valve", "https://cdn.example/SteamSetup.exe", 2_380_800, ClientState::Installed),
        client(
            "epic",
            "Epic Games Launcher",
            "Epic Games",
            "https://dl.example/Epic.msi",
            264_237_056,
            ClientState::NotInstalled,
        ),
        client("gog", "GOG Galaxy", "GOG", "https://dl.example/galaxy.exe", 342_291_968, ClientState::NotInstalled),
        client("ea", "EA app", "Electronic Arts", "https://dl.example/EA.exe", 2_141_208, ClientState::NotInstalled),
        client(
            "ubisoft",
            "Ubisoft Connect",
            "Ubisoft",
            "https://dl.example/Ubi.exe",
            263_096_496,
            ClientState::NotInstalled,
        ),
        client("battlenet", "Battle.net", "Blizzard", "https://dl.example/Battle.net-Setup.exe", 4_700_880, battlenet),
    ]
}

/// Set up this PC, under Helpers: a Row an app with how it stands; Install asks first, the vendor, the address and
/// the size in the question, and only Install asks the executable; Remove asks too, with what happens to the games;
/// a step under way shows on its Row, and A on another app then waits.
#[test]
fn settings_set_up() {
    use model::setup::{ClientState, SetUpAsk};
    let mut stage = settings_stage();
    stage.dashboard.set_clients(clients_shown(ClientState::NotInstalled));
    stage.press(&[Down, Down, Right, Right, Down, Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Games & apps: Set up this PC");
    stage.check("settings-set-up-box");
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Set up this PC: Steam; installed");

    // Battle.net, at the foot: Install asks; Cancel, or B, asks nothing of the executable.
    stage.press(&[Down, Down, Down, Down, Down, Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Set up this PC: Battle.net; not installed");
    stage.check("settings-set-up");
    stage.press(&[A]);
    assert_eq!(
        stage.dashboard.focus_line(),
        "Settings: Set up this PC: Battle.net; not installed; asking Install Battle.net?"
    );
    stage.check("settings-set-up-question");
    stage.press(&[B]);
    stage.press(&[A, Down, A]);
    assert!(asked(&mut stage).is_empty());
    stage.press(&[A, A]);
    assert_eq!(asked(&mut stage), [Effect::SetUp(SetUpAsk::Install("battlenet".into()))]);

    // Under way: its Row says so, and A on another app waits for it.
    stage.dashboard.set_clients(clients_shown(ClientState::Downloading { done: 2_350_440 }));
    assert_eq!(stage.dashboard.focus_line(), "Settings: Set up this PC: Battle.net; downloading");
    stage.dashboard.set_clients(clients_shown(ClientState::Installing));
    assert_eq!(stage.dashboard.focus_line(), "Settings: Set up this PC: Battle.net; installing");
    stage.check("settings-set-up-installing");
    stage.press(&[Up, A]);
    assert_eq!(
        stage.dashboard.focus_line(),
        "Settings: Set up this PC: Ubisoft Connect; not installed; refused: Wait until Battle.net is done."
    );
    assert!(asked(&mut stage).is_empty());

    // Installed: Remove asks, with what happens to its games, and Remove asks the executable.
    stage.dashboard.set_clients(clients_shown(ClientState::Installed));
    stage.press(&[Down]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Set up this PC: Battle.net; installed");
    stage.press(&[A]);
    assert_eq!(
        stage.dashboard.focus_line(),
        "Settings: Set up this PC: Battle.net; installed; asking Remove Battle.net?"
    );
    stage.check("settings-set-up-remove-question");
    stage.press(&[A]);
    assert_eq!(asked(&mut stage), [Effect::SetUp(SetUpAsk::Remove("battlenet".into()))]);

    // A failure says why on the line, and the Row how it stands.
    let why = "It wasn't installed: its installer ended with 1. The Dashboard's log says more.".to_string();
    stage.dashboard.set_clients(clients_shown(ClientState::Failed { why: why.clone(), installed: false }));
    assert_eq!(
        stage.dashboard.focus_line(),
        format!("Settings: Set up this PC: Battle.net; not installed; failed: {why}")
    );
    stage.press(&[B]);
    assert_eq!(stage.dashboard.focus_line(), "Settings: Games & apps: Set up this PC");
}
