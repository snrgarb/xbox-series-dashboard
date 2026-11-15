//! What the Dashboard asks of a game's store beyond starting it: installing and uninstalling, which the Tile menu
//! offers (docs/spec/tile-menu.md, section 13). The game is named by its id in the library database, as the
//! effect names it, and the store is reached through `shell::store::Addresses`, which a test watches.
use model::Library;
use shell::store::{self, Addresses, Ask};

/// Asks the store of the game with this id, hidden or not, to install or uninstall it. A game that is in no
/// database (id 0) or no longer in the library is said so and nothing is asked.
pub fn ask_store(library: &Library, game: i64, ask: Ask, addresses: &mut dyn Addresses) {
    match library.game(game).filter(|_| game != 0) {
        Some(game) => store::ask(game, ask, addresses),
        None => eprintln!("store: no game {game} in the library: nothing asked"),
    }
}

#[cfg(test)]
mod tests {
    use model::Game;
    use shell::store::Recorded;

    use super::*;

    fn library() -> Library {
        let game = |id: i64, appid: u32, source: &str| Game { id, appid, source: source.into(), ..Game::default() };
        Library {
            games: vec![game(1, 9_100_010, "steam"), game(2, 77, "")],
            hidden: vec![game(3, 9_100_040, "steam")],
            ..Library::default()
        }
    }

    #[test]
    fn install_and_uninstall_open_steams_address_for_the_game() {
        let mut addresses = Recorded::default();
        ask_store(&library(), 3, Ask::Install, &mut addresses);
        ask_store(&library(), 1, Ask::Uninstall, &mut addresses);
        assert_eq!(addresses.opened, ["steam://install/9100040", "steam://uninstall/9100010"]);
    }

    #[test]
    fn nothing_is_asked_for_a_game_that_is_gone_or_of_no_store() {
        let mut addresses = Recorded::default();
        // No such game; a game in no database, which id 0 must never find; a game from no store.
        ask_store(&library(), 9, Ask::Uninstall, &mut addresses);
        let unsaved =
            Library { games: vec![Game { appid: 5, source: "steam".into(), ..Game::default() }], ..library() };
        ask_store(&unsaved, 0, Ask::Uninstall, &mut addresses);
        ask_store(&library(), 2, Ask::Install, &mut addresses);
        assert!(addresses.opened.is_empty(), "{:?}", addresses.opened);
    }
}
