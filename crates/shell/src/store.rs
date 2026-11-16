//! Asking a game's store to install or uninstall it (docs/spec/tile-menu.md, sections 10 and 13). Until the
//! Queue (M4) that is what the game's source gives (`model::Source::ask`: the store's own address for it, or a
//! program to run), opened as Windows opens it: the store shows its own window and does the rest. Only
//! [`SystemAddresses`] opens anything, which the executable makes only when it runs in its window; a test has a
//! [`Recorded`], which keeps what it was asked.
use model::{Game, Source, Start};

pub use model::Ask;

/// Whoever opens what a store is asked by: the seam between what the Dashboard asks of a store and Windows.
pub trait Addresses {
    /// Opens an address with whatever program is registered for it, or runs a program. False when nothing took it.
    fn open(&mut self, start: &Start) -> bool;
}

/// Asks the game's store, by way of its source (none for a game of no source the Dashboard has) and `addresses`,
/// and says in the log what was asked: `store: asked Steam to install <name>: steam://install/<appid>`. A game
/// whose store can't be asked is said so and left alone.
pub fn ask(game: &Game, source: Option<&dyn Source>, ask: Ask, addresses: &mut dyn Addresses) {
    let what = ask.verb();
    match source.and_then(|source| Some((source.name(), source.ask(game, ask)?))) {
        Some((store, address)) if addresses.open(&address) => {
            eprintln!("store: asked {store} to {what} {}: {address}", game.name)
        }
        Some((_, address)) => eprintln!("store: nothing took {address}: {} is as it was", game.name),
        None => eprintln!("store: {} is from no store that can {what} it", game.name),
    }
}

/// Keeps what it was asked to open, as the log says it, and opens nothing.
#[derive(Debug, Default)]
pub struct Recorded {
    pub opened: Vec<String>,
}

impl Addresses for Recorded {
    fn open(&mut self, start: &Start) -> bool {
        self.opened.push(start.to_string());
        true
    }
}

/// Windows' own way of opening an address. Anywhere else it opens nothing.
#[derive(Debug, Default)]
pub struct SystemAddresses;

impl Addresses for SystemAddresses {
    #[cfg(windows)]
    fn open(&mut self, start: &Start) -> bool {
        // Home has the foreground, and with it the right to let the store's window take it.
        crate::shell_open(start) > 32
    }

    #[cfg(not(windows))]
    fn open(&mut self, _start: &Start) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use model::Found;

    use super::*;

    /// A store whose addresses are `fake://<verb>/<id>`, and which has none for a game with no id.
    struct Fake;

    impl Source for Fake {
        fn key(&self) -> &'static str {
            "fake"
        }

        fn name(&self) -> &'static str {
            "Fake Store"
        }

        fn read(&self) -> anyhow::Result<Option<Found>> {
            Ok(None)
        }

        fn start(&self, _: &Game) -> Option<Start> {
            None
        }

        fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
            (!game.source_id.is_empty()).then(|| Start::Address(format!("fake://{}/{}", ask.verb(), game.source_id)))
        }
    }

    fn game(source_id: &str) -> Game {
        Game { source: "fake".into(), source_id: source_id.into(), name: "Quiet Orbit".into(), ..Game::default() }
    }

    #[test]
    fn asking_opens_the_address_the_games_source_gives_and_nothing_where_it_has_none() {
        let mut addresses = Recorded::default();
        ask(&game("9100040"), Some(&Fake), Ask::Install, &mut addresses);
        ask(&game("9100010"), Some(&Fake), Ask::Uninstall, &mut addresses);
        ask(&game(""), Some(&Fake), Ask::Uninstall, &mut addresses);
        ask(&game("5"), None, Ask::Uninstall, &mut addresses);
        assert_eq!(addresses.opened, ["fake://install/9100040", "fake://uninstall/9100010"]);
    }
}
