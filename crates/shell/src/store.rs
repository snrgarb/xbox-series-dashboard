//! Asking a game's store to install or uninstall it (docs/spec/tile-menu.md, sections 10 and 13). Until the
//! Queue (M4) that is the store's own address for it, opened as Windows opens any address: Steam shows its own
//! window and does the rest. Only [`SystemAddresses`] opens anything, which the executable makes only when it runs
//! in its window; a test has a [`Recorded`], which keeps what it was asked.
use model::Game;

/// What a store is asked to do with a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    Install,
    Uninstall,
}

/// The address that asks the game's store for this: Steam's `steam://install/<appid>` and
/// `steam://uninstall/<appid>`. None for a game of a store that has no such address, or of no store.
pub fn address(game: &Game, ask: Ask) -> Option<String> {
    let verb = match ask {
        Ask::Install => "install",
        Ask::Uninstall => "uninstall",
    };
    (game.source == "steam" && game.appid != 0).then(|| format!("steam://{verb}/{}", game.appid))
}

/// Whoever opens an address: the seam between what the Dashboard asks of a store and Windows.
pub trait Addresses {
    /// Opens the address with whatever program is registered for it. False when nothing took it.
    fn open(&mut self, address: &str) -> bool;
}

/// Asks the game's store, through `addresses`, and says in the log what was asked: `store: asked Steam to install
/// <name>: steam://install/<appid>`. A game whose store can't be asked is said so and left alone.
pub fn ask(game: &Game, ask: Ask, addresses: &mut dyn Addresses) {
    let what = match ask {
        Ask::Install => "install",
        Ask::Uninstall => "uninstall",
    };
    match address(game, ask) {
        Some(address) if addresses.open(&address) => eprintln!("store: asked Steam to {what} {}: {address}", game.name),
        Some(address) => eprintln!("store: nothing took {address}: {} is as it was", game.name),
        None => eprintln!("store: {} is from no store that can {what} it", game.name),
    }
}

/// Keeps the addresses it was asked to open and opens none.
#[derive(Debug, Default)]
pub struct Recorded {
    pub opened: Vec<String>,
}

impl Addresses for Recorded {
    fn open(&mut self, address: &str) -> bool {
        self.opened.push(address.to_string());
        true
    }
}

/// Windows' own way of opening an address. Anywhere else it opens nothing.
#[derive(Debug, Default)]
pub struct SystemAddresses;

impl Addresses for SystemAddresses {
    #[cfg(windows)]
    fn open(&mut self, address: &str) -> bool {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY, SW_SHOWNORMAL};
        let (verb, address) = (crate::wide("open"), crate::wide(address));
        unsafe {
            // Home has the foreground, and with it the right to let the store's window take it.
            AllowSetForegroundWindow(ASFW_ANY);
            let null = std::ptr::null();
            let result =
                ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), address.as_ptr(), null, null, SW_SHOWNORMAL);
            result as isize > 32
        }
    }

    #[cfg(not(windows))]
    fn open(&mut self, _address: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(source: &str, appid: u32) -> Game {
        Game { appid, source: source.into(), name: "Quiet Orbit".into(), ..Game::default() }
    }

    #[test]
    fn steams_games_are_asked_for_by_their_appid() {
        let steam = game("steam", 9_100_040);
        assert_eq!(address(&steam, Ask::Install).as_deref(), Some("steam://install/9100040"));
        assert_eq!(address(&steam, Ask::Uninstall).as_deref(), Some("steam://uninstall/9100040"));
    }

    #[test]
    fn a_game_of_no_store_has_no_address() {
        assert_eq!(address(&game("", 9_100_040), Ask::Install), None);
        assert_eq!(address(&game("steam", 0), Ask::Uninstall), None);
    }

    #[test]
    fn asking_opens_the_games_address_and_nothing_for_a_game_of_no_store() {
        let mut addresses = Recorded::default();
        ask(&game("steam", 9_100_040), Ask::Install, &mut addresses);
        ask(&game("steam", 9_100_010), Ask::Uninstall, &mut addresses);
        ask(&game("", 5), Ask::Uninstall, &mut addresses);
        assert_eq!(addresses.opened, ["steam://install/9100040", "steam://uninstall/9100010"]);
    }
}
