//! What the Dashboard asks of a game's store beyond starting it: installing and uninstalling, which the Tile menu
//! offers (docs/spec/tile-menu.md, section 13), and pausing, resuming and cancelling an install, which a queued
//! game's menu in the Queue offers (docs/spec/manage-queue.md, section 8). The game is named by its id in the
//! library database, as the effect names it; its source's installer, or the address its source gives, is what is
//! asked, through `shell::store::Addresses`, which a test watches.
use std::sync::Arc;

use model::{Library, Source};
use shell::store::{self, Addresses, Ask};

/// Asks the store of the game with this id, hidden or not, to install or uninstall it. A game that is in no
/// database (id 0) or no longer in the library is said so and nothing is asked. Whether the store was asked.
pub fn ask_store(
    library: &Library,
    sources: &[Arc<dyn Source>],
    game: i64,
    ask: Ask,
    addresses: &mut dyn Addresses,
) -> bool {
    match library.game(game).filter(|_| game != 0) {
        Some(game) => store::ask(game, crate::sources::of(sources, game).map(|source| &**source), ask, addresses),
        None => {
            eprintln!("store: no game {game} in the library: nothing asked");
            false
        }
    }
}

/// Asks the store of the game with this id to pause, resume or cancel its install under way. Whether it was asked.
pub fn steer(
    library: &Library,
    sources: &[Arc<dyn Source>],
    game: i64,
    steer: model::Steer,
    addresses: &mut dyn Addresses,
) -> bool {
    match library.game(game).filter(|_| game != 0) {
        Some(game) => store::steer(game, crate::sources::of(sources, game).map(|source| &**source), steer, addresses),
        None => {
            eprintln!("store: no game {game} in the library: nothing asked");
            false
        }
    }
}

/// Starts the App with this id, as it is: no Launch screen and no Play session, whatever is running
/// (docs/spec/apps.md). The log says `apps: started Notepad: C:\Windows\System32\notepad.exe`.
pub fn start_app(library: &Library, app: i64, addresses: &mut dyn Addresses) {
    let Some(app) = library.apps.iter().find(|found| found.id == app) else {
        eprintln!("apps: no App {app} in the library: nothing started");
        return;
    };
    let start = app.start();
    match addresses.open(&start) {
        Ok(()) => eprintln!("apps: started {}: {start}", app.name),
        Err(why) => eprintln!("apps: nothing took {start} ({why}): {} isn't started", app.name),
    }
}

#[cfg(test)]
mod tests {
    use model::Game;
    use shell::store::Recorded;

    use super::*;

    fn library() -> Library {
        let game = |id: i64, appid: &str, source: &str| Game {
            id,
            source_id: appid.into(),
            source: source.into(),
            ..Game::default()
        };
        Library {
            games: vec![game(1, "9100010", "steam"), game(2, "77", "")],
            hidden: vec![game(3, "9100040", "steam")],
            ..Library::default()
        }
    }

    fn sources() -> Vec<Arc<dyn Source>> {
        crate::sources::all(&crate::Args::default())
    }

    #[test]
    fn install_and_uninstall_open_steams_address_for_the_game() {
        let mut addresses = Recorded::default();
        ask_store(&library(), &sources(), 3, Ask::Install, &mut addresses);
        ask_store(&library(), &sources(), 1, Ask::Uninstall, &mut addresses);
        assert_eq!(addresses.opened, ["steam://install/9100040", "steam://uninstall/9100010"]);
    }

    #[test]
    fn the_real_steam_is_not_asked_to_pause_and_the_stand_in_is() {
        let mut addresses = Recorded::default();
        assert!(!steer(&library(), &sources(), 3, model::Steer::Pause, &mut addresses));
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("steam.exe"), "").unwrap();
        std::fs::write(dir.path().join(source_steam::fixture::STANDIN_CLIENT), "").unwrap();
        let standin = crate::sources::all(&crate::Args { steam: Some(dir.path().to_path_buf()), ..Default::default() });
        assert!(steer(&library(), &standin, 3, model::Steer::Cancel, &mut addresses));
        let exe = dir.path().join("steam.exe");
        assert_eq!(addresses.opened, [format!("{} -silent +standin_cancel 9100040", exe.display())]);
    }

    #[test]
    fn an_app_is_opened_as_it_is_and_one_not_there_is_not() {
        let app = |id: i64, kind, target: &str| model::App {
            id,
            name: format!("App {id}"),
            kind,
            target: target.into(),
            icon: None,
        };
        let library = Library {
            apps: vec![
                app(1, model::AppKind::Program, r"C:\Windows\System32\notepad.exe"),
                app(2, model::AppKind::Store, "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"),
            ],
            ..library()
        };
        let mut addresses = Recorded::default();
        for id in [2, 1, 3] {
            start_app(&library, id, &mut addresses);
        }
        assert_eq!(
            addresses.opened,
            [r"shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", r"C:\Windows\System32\notepad.exe"]
        );
    }

    #[test]
    fn nothing_is_asked_for_a_game_that_is_gone_or_of_no_store() {
        let mut addresses = Recorded::default();
        // No such game; a game in no database, which id 0 must never find; a game from no store; one from a store
        // the Dashboard does not have.
        ask_store(&library(), &sources(), 9, Ask::Uninstall, &mut addresses);
        let unsaved = Library {
            games: vec![Game { source_id: "5".into(), source: "steam".into(), ..Game::default() }],
            ..library()
        };
        ask_store(&unsaved, &sources(), 0, Ask::Uninstall, &mut addresses);
        ask_store(&library(), &sources(), 2, Ask::Install, &mut addresses);
        let elsewhere = Library {
            games: vec![Game { id: 4, source_id: "5".into(), source: "nowhere".into(), ..Game::default() }],
            ..library()
        };
        ask_store(&elsewhere, &sources(), 4, Ask::Install, &mut addresses);
        assert!(addresses.opened.is_empty(), "{:?}", addresses.opened);
    }
}
