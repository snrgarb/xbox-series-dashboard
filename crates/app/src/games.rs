//! What the Dashboard asks of a game's store beyond starting it: installing and uninstalling, which the Tile menu
//! offers (docs/spec/tile-menu.md, section 13). The game is named by its id in the library database, as the
//! effect names it; its source gives the store's address, which is opened through `shell::store::Addresses`, which
//! a test watches.
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
