//! Search (docs/spec/search.md, sections 1 to 6): the search box at the top, the results under it, and the Console
//! keyboard at the foot while it is up. Typing on the keys finds games by name as each key goes in; B puts the
//! keyboard away and then leaves; A on a result opens its Game hub at once.
//!
//! This file is the page's state: the keyboard, which rows show, where focus is and where it goes. `find.rs` says
//! what is found, `draw.rs` draws it.
//!
//! **The focus log.** `Search: <focus>; <text> <keyboard>, <rows>`, where focus is `Box` (the search box),
//! `Key(Q)` (a key, by the keyboard's names: `Space`, `Shift`, `Quote`), or a tile by its row, its place in the row
//! and its game's name: `Games(1) Halo Infinite`, `RecentlyAdded(0) Alan Wake 2`. The text is the keyboard's with
//! the caret in it, Shift and its layer while the keyboard is up (`"Ha|" lower letters`), and with the keyboard
//! away the text alone and `away` (`"Ha" away`). The rows are each row's name and how many it holds
//! (`RecentlyAdded 12`, `Games 2`), or `no results`. So: `Search: Key(A); "Ha|" lower letters, Games 2`.
mod draw;
pub mod find;

use model::Library;
use ui::{Button, Press, Repeat, Ui};

use self::find::Kind;
use crate::game_hub;
use crate::keyboard::{self, Board, Outcome, Typed};
use crate::pages::{Around, Open, Page, PageAction};

/// Where focus is on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// The search box.
    Box,
    /// A tile: its row, top to bottom, and its place along it.
    Tile { row: usize, at: usize },
    /// A key of the keyboard: whichever the keyboard says.
    Keys,
}

/// A row of results as it shows now.
struct Row {
    kind: Kind,
    /// Its games' places in the library they were found in.
    games: Vec<usize>,
    /// Which games they are, whatever library comes next: a row is only replaced when these change.
    ids: Vec<model::GameKey>,
    /// Their names, for the focus log.
    names: Vec<String>,
    /// The tile focus was last on in this row: where Up and Down onto it land.
    remembered: Option<usize>,
    /// The first tile in view: the row rests a whole number of tiles along.
    first: usize,
}

/// What the rows were last worked out from: the text, and which library (a new snapshot is a new list of games).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    text: String,
    library: (usize, usize, usize),
}

impl Seen {
    fn of(library: &Library, text: String) -> Self {
        Self { text, library: (library.games.as_ptr() as usize, library.games.len(), library.hidden.len()) }
    }
}

/// Search opens empty every time, whatever opened it (docs/spec/search.md, 4.1): nothing typed, Shift on, the
/// letters, the keyboard up with focus on Q, Recently added under the box. Where B goes is the stack's business: to
/// the page under it, or to Home.
pub struct Search {
    board: Board,
    /// Whether the keyboard is up.
    up: bool,
    focus: Focus,
    rows: Vec<Row>,
    seen: Option<Seen>,
    /// Counts the rows' replacements: a row that is new starts at its first tile, with no slide left over.
    generation: u64,
    /// How far the results are scrolled up, to keep the focused tile in view.
    scroll: f32,
}

impl Search {
    pub fn open(library: &Library) -> Self {
        let mut search = Self {
            board: Board::new(),
            up: true,
            focus: Focus::Keys,
            rows: Vec::new(),
            seen: None,
            generation: 0,
            scroll: 0.0,
        };
        search.follow(library);
        search
    }

    /// Works the rows out again if the text or the library changed since they last were, and replaces a row only if
    /// what it shows changed (docs/spec/search.md, 3.7). With a row replaced and focus on a tile, focus is placed
    /// again: after typing, on the keys if the keyboard is up, else the first tile, else the box; after the library
    /// changed (a game hub was up over the page), on the same game if a row still holds it (5.3).
    fn follow(&mut self, library: &Library) {
        let seen = Seen::of(library, self.board.text());
        if self.seen.as_ref() == Some(&seen) {
            return;
        }
        let typed = self.seen.as_ref().is_none_or(|was| was.text != seen.text);
        let rows = find::rows(library, &seen.text);
        self.seen = Some(seen);
        let id = |&at: &usize| library.games[at].key();
        let same = rows.len() == self.rows.len()
            && rows.iter().zip(&self.rows).all(|((kind, games), row)| {
                *kind == row.kind && games.len() == row.ids.len() && games.iter().map(id).eq(row.ids.iter().cloned())
            });
        if same {
            // The same games, perhaps at other places in the new library.
            for ((_, games), row) in rows.into_iter().zip(&mut self.rows) {
                row.names = games.iter().map(|&at| library.games[at].name.clone()).collect();
                row.games = games;
            }
            return;
        }
        let was = match self.focus {
            Focus::Tile { row, at } => self.rows.get(row).and_then(|row| row.ids.get(at)).cloned(),
            _ => None,
        };
        self.rows = rows
            .into_iter()
            .map(|(kind, games)| Row {
                kind,
                ids: games.iter().map(id).collect(),
                names: games.iter().map(|&at| library.games[at].name.clone()).collect(),
                games,
                remembered: None,
                first: 0,
            })
            .collect();
        self.generation += 1;
        if !matches!(self.focus, Focus::Tile { .. }) {
            return;
        }
        let same_game = was.filter(|_| !typed).and_then(|was| {
            self.rows.iter().enumerate().find_map(|(row, r)| r.ids.iter().position(|id| *id == was).map(|at| (row, at)))
        });
        match same_game {
            Some((row, at)) => self.focus_tile(row, at),
            None if self.up => self.focus_keys(),
            None => self.focus_first(),
        }
    }

    /// Focus onto a tile, the row slid to show it whole.
    fn focus_tile(&mut self, row: usize, at: usize) {
        if self.focus == Focus::Keys {
            self.board.let_go();
        }
        self.focus = Focus::Tile { row, at };
        let r = &mut self.rows[row];
        r.remembered = Some(at);
        let shown = draw::TILES_SHOWN;
        if at < r.first {
            r.first = at;
        } else if at >= r.first + shown {
            r.first = at + 1 - shown;
        }
    }

    /// Focus onto a row from above or below: the tile it last had, else its first.
    fn focus_row(&mut self, row: usize) {
        let at = self.rows[row].remembered.unwrap_or(0).min(self.rows[row].games.len().saturating_sub(1));
        self.focus_tile(row, at);
    }

    /// Focus onto the keys, on the key it was last on.
    fn focus_keys(&mut self) {
        if self.focus != Focus::Keys {
            self.board.enter();
        }
        self.focus = Focus::Keys;
    }

    fn focus_box(&mut self) {
        if self.focus == Focus::Keys {
            self.board.let_go();
        }
        self.focus = Focus::Box;
    }

    /// The first row's first tile, or the box when no row has a tile.
    fn focus_first(&mut self) {
        match self.rows.first().filter(|row| !row.games.is_empty()) {
            Some(_) => self.focus_tile(0, 0),
            None => self.focus_box(),
        }
    }

    /// The rows that have a tile to take focus.
    fn rows_with_tiles(&self) -> Vec<usize> {
        self.rows.iter().enumerate().filter(|(_, row)| !row.games.is_empty()).map(|(at, _)| at).collect()
    }

    /// B, Menu or the Enter key with the keyboard up: it goes, the text stays, and focus is on the first tile.
    fn put_away(&mut self) {
        self.up = false;
        self.focus_first();
        self.board.let_go();
    }

    /// What the keyboard did.
    fn answer(&mut self, outcome: Outcome, library: &Library) {
        match outcome {
            Outcome::Edited => self.follow(library),
            Outcome::Enter => self.put_away(),
            // Up from the keys' top row: the last row of results, or the box.
            Outcome::LeftTop => match self.rows_with_tiles().last().copied() {
                Some(row) => self.focus_row(row),
                None => self.focus_box(),
            },
            Outcome::Taken | Outcome::Ignored => {}
        }
    }

    /// The game on a tile, as the library has it now.
    fn game(&self, row: usize, at: usize) -> Option<usize> {
        self.rows.get(row)?.games.get(at).copied()
    }
}

impl Page for Search {
    fn name(&self) -> &'static str {
        "Search"
    }

    fn focus(&self) -> String {
        match self.focus {
            Focus::Box => "Box".into(),
            Focus::Keys => format!("Key({})", self.board.focus()),
            Focus::Tile { row, at } => {
                let row = &self.rows[row];
                format!("{}({at}) {}", row.kind.name(), row.names[at])
            }
        }
    }

    fn state(&self) -> String {
        let text = if self.up { self.board.state(false) } else { format!("{:?} away", self.board.text()) };
        let rows: Vec<String> =
            self.rows.iter().map(|row| format!("{} {}", row.kind.name(), row.games.len())).collect();
        let rows = if rows.is_empty() { "no results".to_string() } else { rows.join(", ") };
        format!("{text}, {rows}")
    }

    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        let library = around.library;
        self.follow(library);
        let button = press.button;
        if button == Button::B {
            if press.repeat {
                return None;
            }
            // Two steps: the keyboard goes, then the page.
            if self.up {
                self.put_away();
                return None;
            }
            return Some(PageAction::Close);
        }
        if self.focus == Focus::Keys {
            let outcome = self.board.press(press);
            self.answer(outcome, library);
            return None;
        }
        // The pad's shortcuts press their keys wherever focus is while the keyboard is up; LT only on the keys.
        let shortcut = matches!(
            button,
            Button::X | Button::Y | Button::LeftBumper | Button::RightBumper | Button::LeftStick | Button::Menu
        );
        if shortcut {
            if self.up && !press.repeat {
                let outcome = self.board.shortcut(button);
                self.answer(outcome, library);
            }
            return None;
        }
        if press.repeat && !button.is_direction() {
            return None;
        }
        match (self.focus, button) {
            // The keyboard up again as it was, focus on its key.
            (Focus::Box, Button::A) => {
                self.up = true;
                self.focus_keys();
            }
            (Focus::Box, Button::Down) => match self.rows_with_tiles().first().copied() {
                Some(row) => self.focus_row(row),
                None if self.up => self.focus_keys(),
                None => {}
            },
            (Focus::Tile { row, at }, Button::Left) if at > 0 => self.focus_tile(row, at - 1),
            (Focus::Tile { row, at }, Button::Right) if at + 1 < self.rows[row].games.len() => {
                self.focus_tile(row, at + 1)
            }
            (Focus::Tile { row, .. }, Button::Up) => {
                match self.rows_with_tiles().into_iter().rfind(|&above| above < row) {
                    Some(above) => self.focus_row(above),
                    None => self.focus_box(),
                }
            }
            (Focus::Tile { row, .. }, Button::Down) => {
                match self.rows_with_tiles().into_iter().find(|&below| below > row) {
                    Some(below) => self.focus_row(below),
                    None if self.up => self.focus_keys(),
                    None => {}
                }
            }
            // The game's hub, at once, as on Home.
            (Focus::Tile { row, at }, Button::A) => {
                let game = &library.games[self.game(row, at)?];
                return Some(PageAction::Open(Open::GameHub(game_hub::Entry::of(game))));
            }
            _ => {}
        }
        None
    }

    /// Asked after each press while the page is on top: the rows follow a library that changed while something was
    /// over the page (its Game hub), so the focus log says where focus is as soon as the page is back.
    fn tick(&mut self, around: Around) {
        self.follow(around.library);
    }

    /// A physical keyboard's text goes in whether the keyboard is up or away.
    fn typed(&mut self, typed: Typed, around: Around) -> bool {
        let outcome = self.board.typed(typed);
        self.answer(outcome, around.library);
        true
    }

    fn pad(&mut self, button: Button, down: bool) {
        if down && self.up {
            self.board.hold(button);
        } else if !down {
            self.board.release(button);
        }
    }

    /// The keys' own pace on the keys, as in text entry; elsewhere the page's.
    fn repeat(&self) -> Repeat {
        if self.focus == Focus::Keys {
            keyboard::REPEAT
        } else {
            Repeat::default()
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around) {
        self.follow(around.library);
        draw::page(self, ui, around.library);
    }
}

#[cfg(test)]
mod tests {
    use model::Game;
    use ui::Button::{Down, Left, LeftStick, Menu, Right, Up, A, B, X, Y};

    use super::*;

    fn library() -> Library {
        let names = ["Hades", "Halo Infinite", "Celeste", "Shadow of the Colossus"];
        let games = names.iter().enumerate().map(|(i, name)| Game {
            id: i as i64 + 1,
            source_id: (i + 10).to_string(),
            name: name.to_string(),
            added: 100 - i as u64,
            ..Game::default()
        });
        Library { games: games.collect(), ..Library::default() }
    }

    fn press(search: &mut Search, library: &Library, buttons: &[Button]) -> Option<PageAction> {
        let around = Around {
            library,
            playing: None,
            options: crate::settings::Options::default(),
            now: std::time::Instant::now(),
            now_playing: crate::now_playing::none(),
            discord: crate::no_discord(),
        };
        let mut last = None;
        for &button in buttons {
            last = search.press(Press { button, repeat: false }, around);
        }
        last
    }

    fn line(search: &Search) -> String {
        format!("{}; {}", search.focus(), search.state())
    }

    #[test]
    fn it_opens_on_q_with_recently_added() {
        let search = Search::open(&library());
        assert_eq!(line(&search), "Key(Q); \"|\" upper letters, RecentlyAdded 4");
    }

    #[test]
    fn typing_finds_and_focus_stays_on_the_keys() {
        let library = library();
        let mut search = Search::open(&library);
        // "H", then "a": A on Q's row is not H; walk there. H is row 2, column 6.
        press(&mut search, &library, &[Down, Right, Right, Right, Right, Right, A]);
        assert_eq!(line(&search), "Key(H); \"H|\" lower letters, Games 3");
        press(&mut search, &library, &[Left, Left, Left, Left, Left, A]);
        assert_eq!(line(&search), "Key(A); \"Ha|\" lower letters, Games 3");
        // The shortcut X is Backspace, Y a space.
        press(&mut search, &library, &[X, X, X]);
        assert_eq!(line(&search), "Key(A); \"|\" lower letters, RecentlyAdded 4");
        press(&mut search, &library, &[LeftStick, A, A, A]);
        assert_eq!(line(&search), "Key(A); \"Aaa|\" lower letters, no results");
    }

    #[test]
    fn b_puts_the_keyboard_away_then_leaves() {
        let library = library();
        let mut search = Search::open(&library);
        assert_eq!(press(&mut search, &library, &[B]), None);
        assert_eq!(line(&search), "RecentlyAdded(0) Hades; \"\" away, RecentlyAdded 4");
        assert_eq!(press(&mut search, &library, &[B]), Some(PageAction::Close));
    }

    #[test]
    fn menu_and_the_enter_key_put_it_away_too_and_a_on_the_box_brings_it_back_on_its_key() {
        let library = library();
        let mut search = Search::open(&library);
        press(&mut search, &library, &[Right, Menu]);
        assert_eq!(line(&search), "RecentlyAdded(0) Hades; \"\" away, RecentlyAdded 4");
        // With the keyboard away the shortcuts do nothing.
        press(&mut search, &library, &[Y, X, Menu]);
        assert_eq!(line(&search), "RecentlyAdded(0) Hades; \"\" away, RecentlyAdded 4");
        press(&mut search, &library, &[Up, A]);
        assert_eq!(line(&search), "Key(W); \"|\" upper letters, RecentlyAdded 4");
    }

    #[test]
    fn up_from_the_keys_is_the_last_row_and_up_again_the_box() {
        let library = library();
        let mut search = Search::open(&library);
        press(&mut search, &library, &[Up, Up]);
        assert_eq!(search.focus(), "RecentlyAdded(0) Hades");
        press(&mut search, &library, &[Right, Right, Up]);
        assert_eq!(search.focus(), "Box");
        // Down is the tile the row last had; Down again the keys, on the key they were left on.
        press(&mut search, &library, &[Down]);
        assert_eq!(search.focus(), "RecentlyAdded(2) Celeste");
        press(&mut search, &library, &[Right, Right, Down]);
        assert_eq!(search.focus(), "Key(1)");
    }

    #[test]
    fn a_on_a_result_opens_its_hub() {
        let library = library();
        let mut search = Search::open(&library);
        let opened = press(&mut search, &library, &[B, Right, A]);
        let hub = game_hub::Entry::of(&library.games[1]);
        assert_eq!(opened, Some(PageAction::Open(Open::GameHub(hub))));
    }
}
