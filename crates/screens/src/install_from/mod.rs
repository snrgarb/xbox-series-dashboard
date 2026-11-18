//! Install from: the question an Install asks when the library has the same game, not installed, from other
//! launchers that can install it now (docs/spec/install-from.md; #249). A prompt in the middle of the screen over the
//! dimmed page, as "Where do you want to install X?" is: a row a launcher, its placeholder mark and its name, in
//! Settings > Launchers' order, focus on the launcher of the game that was pressed. A goes on with the one chosen as
//! Install does for it alone (`tile_menu::install_from_store`: the folder's question, the store asked, the Queue); B
//! installs nothing.
//!
//! It opens in the place of whatever asked (the Tile menu's Install, the Game hub's), through
//! [`crate::tile_menu::install`], which asks it only where [`asks`] says there is more than one launcher to choose.
use std::sync::atomic::{AtomicU32, Ordering};

use model::{launcher, Game, GameKey, Library};
use ui::{Press, Ui};

use crate::menu_panel::{self, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, Open, Page, PageAction};
use crate::{home, tile_menu};

/// How the question is opened: for which game, the one pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    game: GameKey,
}

/// The question for `game`, in the place of what asked.
pub fn open(game: &Game) -> PageAction {
    PageAction::Replace(Open::InstallFrom(Entry { game: game.key() }))
}

/// Whether a copy can be installed as Install asks it today: not installed, in the library database, not in the
/// Queue already.
fn installable(game: &Game, library: &Library) -> bool {
    !game.installed && game.id != 0 && library.queue.get(game.id).is_none()
}

/// The launchers Install on `game` may install it from, a copy each, in Settings > Launchers' order: the game
/// pressed, if it can be installed at all; and each other launcher's copy of it (`Library::same_game_elsewhere`) that
/// is not installed or being installed, whose launcher is switched on, and whose store can install it now
/// (`Storage::installable`). One copy a launcher, the library's first. Empty where the game pressed cannot be
/// installed: a copy installed elsewhere is never offered, and does not keep this one's Install from going on.
pub fn choices<'a>(game: &'a Game, library: &'a Library) -> Vec<&'a Game> {
    if !installable(game, library) {
        return Vec::new();
    }
    let mut copies = vec![game];
    for other in library.same_game_elsewhere(game) {
        let new_launcher = copies.iter().all(|copy| copy.source != other.source);
        let can = library.storage.installable.contains(&other.id) && library.launchers.is_on(&other.source);
        if new_launcher && can && installable(other, library) {
            copies.push(other);
        }
    }
    let order = |game: &&Game| launcher::ALL.iter().position(|key| *key == game.source).unwrap_or(launcher::ALL.len());
    copies.sort_by_key(order);
    copies
}

/// Whether Install on `game` asks which launcher: there is more than one to choose.
pub fn asks(game: &Game, library: &Library) -> bool {
    choices(game, library).len() > 1
}

/// A launcher's name as the screens write it (the Game hub's, Settings > Launchers'), by its game's source.
fn store_name(game: &Game) -> String {
    home::source(game).map_or_else(|| game.source.clone(), |source| source.name().to_string())
}

static QUESTIONS: AtomicU32 = AtomicU32::new(0);

pub struct InstallFrom {
    name: String,
    /// Each launcher's copy, by its key, and the launcher's name: the rows, in order.
    copies: Vec<(GameKey, String)>,
    /// The row of the game pressed.
    pressed: usize,
    panel: MenuPanel<usize>,
    serial: u32,
}

impl InstallFrom {
    /// The question for the game as the library has it now. None for a game that has gone, or that no longer has
    /// more than one launcher to install it from.
    pub fn open(entry: Entry, around: Around) -> Option<Self> {
        let library = around.library;
        let game = library.games.iter().chain(&library.hidden).find(|game| game.is(&entry.game))?;
        let copies = choices(game, library);
        if copies.len() < 2 {
            return None;
        }
        let pressed = copies.iter().position(|copy| copy.is(&entry.game)).unwrap_or(0);
        let rows =
            copies.iter().enumerate().map(|(at, copy)| Row::marked(home::game_mark(copy), &store_name(copy), at));
        let mut panel = MenuPanel::prompt("Install from", Some(&game.name), rows.collect());
        panel.set_focus(pressed);
        Some(Self {
            name: game.name.clone(),
            copies: copies.iter().map(|copy| (copy.key(), store_name(copy))).collect(),
            pressed,
            panel,
            serial: QUESTIONS.fetch_add(1, Ordering::Relaxed),
        })
    }

    fn flight(&self, ui: &mut Ui, leaving: bool) -> (f32, f32) {
        menu_panel::flight(ui, ("install from", self.serial), leaving)
    }
}

impl Page for InstallFrom {
    fn name(&self) -> &'static str {
        "InstallFrom"
    }

    /// The focused launcher, by its place and name, then the game: `Launcher(1) Epic Games of Quiet Orbit`, and
    /// `, the one pressed` on the launcher of the game Install was pressed on.
    fn focus(&self) -> String {
        let at = self.panel.focus();
        let store = self.copies.get(at).map(|(_, name)| name.as_str()).unwrap_or_default();
        let pressed = if at == self.pressed { ", the one pressed" } else { "" };
        format!("Launcher({at}) {store}{pressed} of {}", self.name)
    }

    fn dialog(&self) -> Option<Dialog> {
        Some(Dialog::Prompt)
    }

    /// A goes on with the launcher chosen as its Install alone would, looked at again as it is chosen: its folder's
    /// question in this one's place, or the store asked. B installs nothing.
    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        match self.panel.press(press) {
            Answer::Chose(at) => {
                let (key, _) = self.copies.get(at)?;
                let library = around.library;
                let copy = library.games.iter().chain(&library.hidden).find(|game| game.is(key));
                Some(copy.map_or(PageAction::Close, |copy| tile_menu::install_from_store(copy, library)))
            }
            Answer::Declined => Some(PageAction::Close),
            Answer::Moved | Answer::Nothing => None,
        }
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        ui.fill(ui.screen(), DIM, 0.0);
        let (fly, shown) = self.flight(ui, false);
        self.panel.draw_flown(ui, fly, shown);
    }

    fn draw_leaving(&mut self, ui: &mut Ui, _around: Around) -> bool {
        let (fly, shown) = self.flight(ui, true);
        if shown <= 0.0 || fly >= FLY {
            return false;
        }
        self.panel.draw_flown(ui, fly, shown);
        true
    }

    fn covers(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use model::launcher::Launcher;
    use model::{InstallState, Queue, Queued};

    use super::*;

    fn game(id: i64, source: &str, name: &str) -> Game {
        Game { id, source: source.into(), source_id: format!("{source}-{id}"), name: name.into(), ..Game::default() }
    }

    /// Quiet Orbit on Steam, Epic, GOG and the EA app, none installed; Epic's and GOG's installable, the EA app's not.
    fn library() -> Library {
        let mut library = Library {
            games: vec![
                game(1, "steam", "Quiet Orbit"),
                game(2, "epic", "QUIET ORBIT™"),
                game(3, "ea", "Quiet Orbit"),
                game(4, "gog", "Quiet Orbit"),
                game(5, "steam", "Fathom Nine"),
                game(6, "epic", "Fathom Nine Deluxe Edition"),
            ],
            ..Library::default()
        };
        library.storage.installable = vec![1, 2, 4, 5, 6];
        library
    }

    fn stores(library: &Library, id: i64) -> Vec<String> {
        let game = library.game(id).unwrap();
        choices(game, library).iter().map(|copy| copy.source.clone()).collect()
    }

    #[test]
    fn the_launchers_are_the_pressed_ones_and_each_other_that_can_install_it_in_settings_order() {
        let library = library();
        assert_eq!(stores(&library, 2), ["steam", "epic", "gog"], "the EA app cannot install it from here");
        assert!(asks(library.game(2).unwrap(), &library));
        // An edition is another game: Fathom Nine is in one launcher only, and asks nothing.
        assert_eq!(stores(&library, 5), ["steam"]);
        assert!(!asks(library.game(5).unwrap(), &library));
    }

    #[test]
    fn a_copy_installed_queued_switched_off_or_not_installable_is_not_offered() {
        let mut installed = library();
        installed.games[1].installed = true;
        assert_eq!(stores(&installed, 1), ["steam", "gog"], "installed on Epic: still offered from the others");
        let mut queued = library();
        let entry = Queued { game: 4, state: InstallState::Queued, started: 1, speed: None, can: Default::default() };
        queued.queue = Queue::new(vec![entry], Vec::new());
        assert_eq!(stores(&queued, 1), ["steam", "epic"]);
        let mut off = library();
        off.launchers.list = vec![Launcher { key: "epic".into(), on: false, ..Launcher::default() }];
        assert_eq!(stores(&off, 1), ["steam", "gog"]);
        let mut unknown = library();
        unknown.storage.installable.clear();
        assert_eq!(stores(&unknown, 1), ["steam"], "not read yet: as before, no question");
    }

    #[test]
    fn a_game_that_cannot_be_installed_itself_asks_nothing() {
        let mut library = library();
        library.games[0].installed = true;
        assert!(stores(&library, 1).is_empty());
        library.games[0] = Game { id: 0, ..game(0, "steam", "Quiet Orbit") };
        assert!(choices(&library.games[0].clone(), &library).is_empty());
    }

    #[test]
    fn one_copy_a_launcher() {
        let mut library = library();
        library.games.push(game(7, "epic", "Quiet Orbit"));
        library.storage.installable.push(7);
        assert_eq!(stores(&library, 1), ["steam", "epic", "gog"]);
    }
}
