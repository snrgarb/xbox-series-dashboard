//! The Queue on the screens (docs/spec/home.md 3.4, game-hub.md 6, tile-menu.md 13): what reads Queued and
//! Installing..., Install asked once, the hub's Queue action, and the ways into Manage Queue.
use model::{InstallState, Library, Queue, Queued};
use screens::home::Focus;
use screens::{Dashboard, Effect};
use ui::Button::{self, Down, Left, Menu, Right, A, B};
use ui::Press;

/// The fake library as the library database gives it: the Recent row's sixth game (Baldur's Gate 3) is not
/// installed.
fn library() -> Library {
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &screens::fake_library(), 1_780_000_000).unwrap();
    database.snapshot().unwrap()
}

const UNINSTALLED: usize = 5;
/// Home's Downloads tile, the last of the wide tiles.
const DOWNLOADS: usize = 3;

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// What has been asked of the shell, but for the sounds.
fn asked(dashboard: &mut Dashboard) -> Vec<Effect> {
    dashboard.take_effects().into_iter().filter(|effect| !matches!(effect, Effect::Sound(_))).collect()
}

/// A press of A on a game's tile: its hub, at once.
fn a_alone(dashboard: &mut Dashboard) {
    press(dashboard, &[A]);
}

/// The game id of the Recent row's tile `tile`.
fn id_on(dashboard: &Dashboard, tile: usize) -> i64 {
    let row = dashboard.library().recent(screens::home::RECENT_GAMES);
    row[tile].id
}

fn queue(game: i64, state: InstallState) -> Queue {
    Queue::new(vec![Queued::new(game, state, 1)], Vec::new())
}

/// Home with focus on the Recent row's tile `tile`.
fn on_tile(tile: usize) -> Dashboard {
    let mut dashboard = Dashboard::new(library());
    press(&mut dashboard, &[Down]);
    press(&mut dashboard, &vec![Right; tile]);
    assert_eq!(dashboard.home_focus(), Focus::Recent(tile));
    dashboard.take_effects();
    dashboard
}

/// The hub of the Recent row's tile `tile`, just opened.
fn hub_on(tile: usize) -> Dashboard {
    let mut dashboard = on_tile(tile);
    a_alone(&mut dashboard);
    assert_eq!(dashboard.page(), Some("GameHub"));
    dashboard.take_effects();
    dashboard
}

#[test]
fn install_in_the_hub_asks_once_and_the_hub_closes() {
    let mut dashboard = hub_on(UNINSTALLED);
    let game = id_on(&dashboard, UNINSTALLED);
    assert!(dashboard.focus_line().ends_with("Hub Install"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), [Effect::Install(game)]);
    assert_eq!(dashboard.page(), None);
}

#[test]
fn the_hubs_main_action_reads_queued_then_installing_and_does_nothing() {
    let mut dashboard = hub_on(UNINSTALLED);
    let game = id_on(&dashboard, UNINSTALLED);
    dashboard.set_queue(queue(game, InstallState::Queued));
    // The focus line follows the Queue the moment the hub hears of it, with no press.
    assert!(dashboard.focus_line().ends_with("Hub Queued"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), []);
    assert_eq!(dashboard.page(), Some("GameHub"));
    dashboard.set_queue(queue(game, InstallState::Downloading { done: 1, total: 4 }));
    assert!(dashboard.focus_line().ends_with("Hub Installing"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), []);
}

#[test]
fn a_hub_opened_on_a_game_in_the_queue_reads_installing_from_its_first_focus_line() {
    let mut dashboard = on_tile(UNINSTALLED);
    let game = id_on(&dashboard, UNINSTALLED);
    dashboard.set_queue(queue(game, InstallState::Downloading { done: 1, total: 4 }));
    a_alone(&mut dashboard);
    assert_eq!(dashboard.page(), Some("GameHub"));
    assert!(dashboard.focus_line().ends_with("Hub Installing"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[B]);
    dashboard.set_queue(queue(game, InstallState::Queued));
    a_alone(&mut dashboard);
    assert!(dashboard.focus_line().ends_with("Hub Queued"), "{}", dashboard.focus_line());
}

#[test]
fn the_hubs_queue_action_shows_while_the_game_is_in_the_queue_and_opens_manage_queue_over_it() {
    let mut dashboard = hub_on(UNINSTALLED);
    let game = id_on(&dashboard, UNINSTALLED);
    // Not queued: Main, Manage, the toggle.
    press(&mut dashboard, &[Right, Right]);
    assert!(dashboard.focus_line().ends_with("Hub Toggle"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[Left, Left]);
    dashboard.set_queue(queue(game, InstallState::Queued));
    press(&mut dashboard, &[Right, Right]);
    assert!(dashboard.focus_line().ends_with("Hub Queue"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[Right]);
    assert!(dashboard.focus_line().ends_with("Hub Toggle"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[Left, A]);
    // Manage Queue over the hub, its row of the game: B comes back to the hub, on Queue.
    assert_eq!(dashboard.page(), Some("ManageQueue"));
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.page(), Some("GameHub"));
    assert!(dashboard.focus_line().ends_with("Hub Queue"), "{}", dashboard.focus_line());
    // The game leaves the Queue with focus on Queue: focus moves to Manage.
    dashboard.set_queue(Queue::default());
    press(&mut dashboard, &[Button::Y]);
    assert!(dashboard.focus_line().ends_with("Hub Manage"), "{}", dashboard.focus_line());
}

#[test]
fn install_on_a_game_already_in_the_queue_asks_nothing_more() {
    let mut dashboard = on_tile(UNINSTALLED);
    let game = id_on(&dashboard, UNINSTALLED);
    dashboard.set_queue(queue(game, InstallState::Queued));
    // #171: no Install in its menu while it is in the Queue (its store steering nothing here: no Pause or Cancel
    // either), so it cannot be asked for again.
    press(&mut dashboard, &[Menu]);
    assert!(dashboard.focus_line().contains("(Add to Favourites)"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[B]);
    assert_eq!(asked(&mut dashboard), []);
    assert_eq!(dashboard.page(), None);
    // Not queued, it asks.
    dashboard.set_queue(Queue::default());
    press(&mut dashboard, &[Menu]);
    assert!(dashboard.focus_line().contains("(Install)"), "{}", dashboard.focus_line());
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), [Effect::Install(game)]);
}

#[test]
fn homes_downloads_tile_opens_manage_queue_and_b_comes_back_to_it() {
    let mut dashboard = on_tile(0);
    press(&mut dashboard, &[Down, Right, Right, Right]);
    assert_eq!(dashboard.home_focus(), Focus::Wide(DOWNLOADS));
    press(&mut dashboard, &[A]);
    assert_eq!((dashboard.page(), dashboard.pages_up()), (Some("ManageQueue"), 1));
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.page(), None);
    assert_eq!(dashboard.home_focus(), Focus::Wide(DOWNLOADS));
}
