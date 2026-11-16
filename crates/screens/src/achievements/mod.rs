//! The Achievements page (docs/spec/achievements-page.md): the Game hub's view of one game's achievements, opened
//! from the hub's Your game stats card and shown in the hub's place, on the hub's own band; B goes back to the hub.
//! The Bundle names it (`CONTEXT.md`, "Achievements page") and never built it, so it is laid out here in the hub's
//! own parts: the hub's header, a card of the game's progress where the hub's first card stands, and the
//! achievements beside it, two to a row, unlocked first.
//!
//! No snapshot of the library carries a game's whole list: the page says which game it wants
//! (`Page::achievements_wanted`), and the shell gives the list (`Page::set_achievements`) as the page opens and again
//! as icons come in or the library changes. Until then the page has its progress card and no list.
mod draw;

use model::achievement::Achievement;
use model::{Game, GameKey, Library};
use ui::{Button, Press, Ui};

use crate::hint_bar::{self, Hint};
use crate::pages::{Around, Page, PageAction};
use crate::tile_menu;

/// How many achievements a row holds.
pub const COLUMNS: usize = 2;
/// How many rows show at once.
pub const ROWS: usize = 5;

/// The game a page is opened for, and the hub it is opened from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub game: GameKey,
    /// The game's row in the library database: what its achievements are kept by.
    pub id: i64,
    /// The hub's number for its picture of the screen behind it (`Page::behind`), which the page shows in the
    /// strips above and below the band as the hub did.
    pub behind: u64,
}

/// The page's achievements in the order it shows them: unlocked first, the newest first (one whose time is not
/// known after those whose time is), then the locked in the store's order, the secret ones last.
pub fn ordered(list: Vec<Achievement>) -> Vec<Achievement> {
    let mut list: Vec<(usize, Achievement)> = list.into_iter().enumerate().collect();
    list.sort_by_key(|(at, a)| match a.unlocked {
        Some(when) => (0, std::cmp::Reverse(when), *at),
        None => (1 + u8::from(a.hidden), std::cmp::Reverse(0), *at),
    });
    list.into_iter().map(|(_, a)| a).collect()
}

pub struct Achievements {
    entry: Entry,
    /// The game's achievements, as last given, in the page's order.
    list: Vec<Achievement>,
    /// The achievement with focus, by its key: it keeps focus as the list is given again.
    focus: Option<String>,
    /// The first row that shows.
    top_row: usize,
}

/// The game, wherever the library has it: among its games, or its hidden ones.
fn find<'a>(library: &'a Library, key: &GameKey) -> Option<&'a Game> {
    library.games.iter().chain(&library.hidden).find(|game| game.is(key))
}

impl Achievements {
    /// The page for a game that has achievements; none for one that has none, or is gone.
    pub fn open(entry: Entry, around: Around) -> Option<Self> {
        let game = find(around.library, &entry.game)?;
        around.library.achievements.of(game.id).filter(|_| game.id != 0)?;
        Some(Self { entry, list: Vec::new(), focus: None, top_row: 0 })
    }

    /// Where focus is in the list.
    fn at(&self) -> Option<usize> {
        let key = self.focus.as_deref()?;
        self.list.iter().position(|a| a.key == key)
    }

    /// Focus on the achievement at `at`, the rows scrolled by the least that shows its row.
    fn focus_on(&mut self, at: usize) {
        self.focus = self.list.get(at).map(|a| a.key.clone());
        let row = at / COLUMNS;
        if row < self.top_row {
            self.top_row = row;
        } else if row >= self.top_row + ROWS {
            self.top_row = row + 1 - ROWS;
        }
    }

    /// A direction: along a row, stopping at its ends; up and down a column, to the last achievement of a shorter
    /// last row.
    fn step(&mut self, button: Button) {
        let Some(at) = self.at() else { return };
        let last = self.list.len() - 1;
        let to = match button {
            Button::Left if at % COLUMNS > 0 => at - 1,
            Button::Right if at % COLUMNS < COLUMNS - 1 && at < last => at + 1,
            Button::Up if at >= COLUMNS => at - COLUMNS,
            Button::Down if (at / COLUMNS + 1) * COLUMNS <= last => (at + COLUMNS).min(last),
            _ => at,
        };
        self.focus_on(to);
    }
}

impl Page for Achievements {
    fn name(&self) -> &'static str {
        "Achievements"
    }

    /// The game by its id at its store, and the achievement with focus by its key: `9100010 ACH_ASH_RUNNER`; `9100010
    /// none` until the list is given.
    fn focus(&self) -> String {
        format!("{} {}", self.entry.game.source_id, self.focus.as_deref().unwrap_or("none"))
    }

    /// How many are unlocked, of how many: `2 of 6 unlocked`; nothing until the list is given.
    fn state(&self) -> String {
        if self.list.is_empty() {
            return String::new();
        }
        let unlocked = self.list.iter().filter(|a| a.is_unlocked()).count();
        format!("{unlocked} of {} unlocked", self.list.len())
    }

    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        let Some(game) = find(around.library, &self.entry.game) else {
            return (press.button == Button::B && !press.repeat).then_some(PageAction::Close);
        };
        match press.button {
            button if button.is_direction() => self.step(button),
            _ if press.repeat => {}
            Button::B => return Some(PageAction::Close),
            // The game's Tile menu, as on the hub whose place the page has.
            Button::Menu => return Some(tile_menu::open(game, tile_menu::Opener::GameHub)),
            // A on an achievement does nothing; nor do X, Y, View, the bumpers and the triggers.
            _ => {}
        }
        None
    }

    fn draw(&mut self, ui: &mut Ui, around: Around) {
        let Some(game) = find(around.library, &self.entry.game) else {
            ui.fill(ui.screen(), ui::Color::BLACK, 0.0);
            return;
        };
        let progress = around.library.achievements.of(game.id).copied();
        draw::draw(ui, game, self.entry.behind, progress, &self.list, self.at(), self.top_row);
    }

    fn hints(&self, _around: Around) -> &'static [Hint] {
        hint_bar::MORE_OPTIONS
    }

    fn behind(&self) -> Option<u64> {
        Some(self.entry.behind)
    }

    fn achievements_wanted(&self) -> Option<i64> {
        Some(self.entry.id)
    }

    /// The list as it is now. Focus stays on its achievement wherever the list has it now; on the first, the
    /// newest unlock, the first time, or if it has gone.
    fn set_achievements(&mut self, _game: i64, list: Vec<Achievement>) {
        let at = self.at();
        self.list = ordered(list);
        let to = self.at().or(at.map(|at| at.min(self.list.len().saturating_sub(1)))).unwrap_or(0);
        if self.list.is_empty() {
            (self.focus, self.top_row) = (None, 0);
        } else {
            self.focus_on(to);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::pages::Around;

    fn achievement(key: &str, unlocked: Option<u64>, hidden: bool) -> Achievement {
        Achievement { key: key.into(), name: key.into(), unlocked, hidden, ..Achievement::default() }
    }

    fn library() -> Library {
        let game = Game { id: 7, source_id: "9100010".into(), name: "Ashfall Courier".into(), ..Game::default() };
        let mut library = Library { games: vec![game], ..Library::default() };
        library.achievements.games.push(model::achievement::GameProgress { game: 7, unlocked: 2, of: 7, score: 25 });
        library
    }

    fn around(library: &Library) -> Around<'_> {
        Around {
            library,
            playing: None,
            options: Default::default(),
            now: Instant::now(),
            now_playing: crate::now_playing::none(),
            discord: crate::no_discord(),
        }
    }

    fn page(library: &Library) -> Achievements {
        let entry = Entry { game: library.games[0].key(), id: 7, behind: 0 };
        let mut page = Achievements::open(entry, around(library)).unwrap();
        page.set_achievements(
            7,
            vec![
                achievement("first", Some(100), false),
                achievement("secret", None, true),
                achievement("second", None, false),
                achievement("newest", Some(300), false),
                achievement("third", None, false),
                achievement("unknown time", Some(0), false),
                achievement("fourth", None, false),
            ],
        );
        page
    }

    fn go(page: &mut Achievements, library: &Library, buttons: &[Button]) -> String {
        for &button in buttons {
            assert_eq!(page.press(Press { button, repeat: false }, around(library)), None, "{button:?}");
        }
        page.focus()
    }

    #[test]
    fn unlocked_come_first_newest_first_then_the_locked_and_the_secret_last() {
        let library = library();
        let page = page(&library);
        let keys: Vec<&str> = page.list.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(keys, ["newest", "first", "unknown time", "second", "third", "fourth", "secret"]);
        assert_eq!(page.focus(), "9100010 newest");
        assert_eq!(page.state(), "3 of 7 unlocked");
    }

    #[test]
    fn focus_moves_over_the_rows_two_wide_and_stops_at_the_edges() {
        let library = library();
        let mut page = page(&library);
        assert_eq!(go(&mut page, &library, &[Button::Left, Button::Up]), "9100010 newest");
        assert_eq!(go(&mut page, &library, &[Button::Right]), "9100010 first");
        assert_eq!(go(&mut page, &library, &[Button::Right]), "9100010 first");
        assert_eq!(go(&mut page, &library, &[Button::Down]), "9100010 second");
        assert_eq!(go(&mut page, &library, &[Button::Down, Button::Down]), "9100010 secret");
        // The last row has one: Down from above its empty place goes to it, and Down from it nowhere.
        assert_eq!(go(&mut page, &library, &[Button::Down]), "9100010 secret");
        assert_eq!(go(&mut page, &library, &[Button::Up, Button::Right, Button::Down]), "9100010 secret");
    }

    #[test]
    fn focus_keeps_its_achievement_as_the_list_comes_again() {
        let library = library();
        let mut page = page(&library);
        go(&mut page, &library, &[Button::Down, Button::Right]);
        assert_eq!(page.focus(), "9100010 second");
        // Second is unlocked now: it is the newest, first in the list, and keeps focus.
        let mut list = page.list.clone();
        list.iter_mut().find(|a| a.key == "second").unwrap().unlocked = Some(400);
        page.set_achievements(7, list);
        assert_eq!(page.focus(), "9100010 second");
        assert_eq!(page.at(), Some(0));
    }

    #[test]
    fn rows_scroll_by_the_least_that_shows_focus() {
        let library = library();
        let mut page = page(&library);
        let many = (0..20).map(|i| achievement(&format!("a{i:02}"), None, false)).collect();
        page.set_achievements(7, many);
        assert_eq!(page.focus(), "9100010 a00");
        go(&mut page, &library, &[Button::Down; 5]);
        assert_eq!((page.focus().as_str(), page.top_row), ("9100010 a10", 1));
        go(&mut page, &library, &[Button::Up; 2]);
        assert_eq!((page.focus().as_str(), page.top_row), ("9100010 a06", 1));
        go(&mut page, &library, &[Button::Up; 3]);
        assert_eq!((page.focus().as_str(), page.top_row), ("9100010 a00", 0));
    }

    #[test]
    fn b_goes_back_and_menu_opens_the_tile_menu() {
        let library = library();
        let mut page = page(&library);
        let around = around(&library);
        assert_eq!(page.press(Press { button: Button::B, repeat: false }, around), Some(PageAction::Close));
        let menu = tile_menu::open(&library.games[0], tile_menu::Opener::GameHub);
        assert_eq!(page.press(Press { button: Button::Menu, repeat: false }, around), Some(menu));
        assert_eq!(page.press(Press { button: Button::A, repeat: false }, around), None);
    }

    #[test]
    fn a_game_with_no_achievements_has_no_page() {
        let mut library = library();
        library.achievements.games.clear();
        let entry = Entry { game: library.games[0].key(), id: 7, behind: 0 };
        assert!(Achievements::open(entry, around(&library)).is_none());
    }
}
