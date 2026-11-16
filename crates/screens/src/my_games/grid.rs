//! The grid of games: five to a row, three rows and the top of a fourth in view, scrolled a row at a time
//! (docs/spec/my-games.md, 5.2 and 5.5). Games' grid, and Full library's, which is the same list lifted under its
//! Sort and Filter row (7.1).
//!
//! In the Bundle this is Playnite's own list, whose scrolling and edges are not in the Bundle's source. They are
//! decided here (the spec's section 16): the grid moves by whole rows, the least that brings the focused row into
//! one of the three row slots, so the row after it shows its top at the foot; every edge stops, nothing wraps,
//! and Down onto a shorter last row lands on its last tile.
use model::Game;
use ui::{Button, Ease, Rect, Ui};

use super::look::NO_GAMES;
use super::nav;
use super::tile::{self, Tile};

pub const COLUMNS: usize = 5;
/// The rows that are whole in view.
const ROWS: usize = 3;
/// The list's box across, which an empty grid's line is centred in.
pub const LIST: (f32, f32) = (516.5, 1237.0);
/// The list's height: three and a half cells, each as tall as a fifth of its width, a whole number of units.
pub const HEIGHT: f32 = 865.0;
const CELL: f32 = HEIGHT / 3.5;
/// A tile stands 7.5 inside its cell.
const GAP: f32 = 15.0;
/// Across, the tiles as the Bundle's pictures have them: Playnite's list sets its cells 2 in from each side.
const LEFT: f32 = 526.0;
const PITCH: f32 = 246.6;
const WIDTH: f32 = 231.6;
/// The grid slides to its row over this long, as Home's page does to a rest.
const SCROLL_SECONDS: f32 = 0.3;
/// Full library's grid is cut 4 under its Sort and Filter row, and its first row's tiles are 24 under the cut.
pub const FULL_LIBRARY_CUT: f32 = 326.0;
const FULL_LIBRARY_PADDING: f32 = 24.0;

/// What tells one game from another across snapshots of the library, with or without a database's ids.
pub type Key = model::GameKey;

pub fn key(game: &Game) -> Key {
    game.key()
}

/// What a direction did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Focus is on another tile.
    Moved,
    /// An edge that stops: Right at a row's end.
    Stopped,
    /// The grid's own edge that way, for whoever holds the grid to say what is beyond: Left from the first
    /// column, Up from the first row, Down from the last.
    Off(Button),
}

pub struct Grid {
    /// What its animated values are known by.
    id: &'static str,
    /// The list's top: a quarter cell and half a gap over the first row's tiles.
    top: f32,
    /// Where the grid is cut, top and bottom: the list's own box on Games; on Full library its top is under the
    /// Sort and Filter row and its foot is under the screen's.
    cut: (f32, f32),
    focus: usize,
    /// The game focus is on, by which it is found again when the library changes.
    key: Option<Key>,
    /// The row in the first of the three slots.
    first_row: usize,
    /// How many times the grid has started again at its top: it is there at once then, not slid to.
    fresh: u32,
}

impl Grid {
    /// Games' grid: the list from y 126, its first row's tiles at 195.29.
    pub fn games() -> Self {
        Self { id: "games grid", top: 126.0, cut: (126.0, 126.0 + HEIGHT), focus: 0, key: None, first_row: 0, fresh: 0 }
    }

    /// Full library's grid (docs/spec/my-games.md, 7.1): the list lifted so that its first row's tiles are at 350,
    /// 24 under where it is cut at 326. Three rows show whole and no fourth: the list's own box runs on under the
    /// screen's foot.
    pub fn full_library() -> Self {
        let top = FULL_LIBRARY_CUT + FULL_LIBRARY_PADDING - CELL / 4.0 - GAP / 2.0;
        let cut = (FULL_LIBRARY_CUT, top + HEIGHT);
        Self { id: "full library grid", top, cut, focus: 0, key: None, first_row: 0, fresh: 0 }
    }

    /// The tile focus is on, or was when it left for the nav.
    pub fn focus(&self) -> usize {
        self.focus
    }

    /// Focus on the first game and the grid at its top, at once: what was remembered is forgotten.
    pub fn forget(&mut self) {
        (self.focus, self.key, self.first_row) = (0, None, 0);
        self.fresh = self.fresh.wrapping_add(1);
    }

    /// The library may have changed: focus stays on its game wherever that is now, and falls back to the first if
    /// it has gone. False with no games.
    pub fn keep(&mut self, games: &[&Game]) -> bool {
        if games.is_empty() {
            self.forget();
            return false;
        }
        if let Some(was) = self.key.take().filter(|was| !games.get(self.focus).is_some_and(|game| game.is(was))) {
            self.focus = games.iter().position(|game| game.is(&was)).unwrap_or(0);
        }
        self.focus = self.focus.min(games.len() - 1);
        self.key = Some(key(games[self.focus]));
        self.follow(games.len());
        true
    }

    /// The least scroll that has the focused row in one of the three slots, and no slot left empty at the end.
    fn follow(&mut self, count: usize) {
        let row = self.focus / COLUMNS;
        let last = count.saturating_sub(1) / COLUMNS;
        self.first_row = self.first_row.clamp(row.saturating_sub(ROWS - 1), row).min(last.saturating_sub(ROWS - 1));
    }

    /// A direction with focus on a tile, among `games`.
    pub fn step(&mut self, button: Button, games: &[&Game]) -> Step {
        let count = games.len();
        if count == 0 {
            return Step::Stopped;
        }
        let (row, column) = (self.focus / COLUMNS, self.focus % COLUMNS);
        let last_row = (count - 1) / COLUMNS;
        let to = match button {
            Button::Left if column == 0 => return Step::Off(button),
            Button::Left => self.focus - 1,
            Button::Right if column + 1 == COLUMNS || self.focus + 1 == count => return Step::Stopped,
            Button::Right => self.focus + 1,
            Button::Up if row == 0 => return Step::Off(button),
            Button::Up => self.focus - COLUMNS,
            Button::Down if row == last_row => return Step::Off(button),
            // Onto a shorter last row: its last tile.
            Button::Down => (self.focus + COLUMNS).min(count - 1),
            _ => return Step::Stopped,
        };
        self.focus = to;
        self.key = Some(key(games[to]));
        self.follow(count);
        Step::Moved
    }

    /// Where tile `index` is with the grid scrolled by `scroll`.
    fn rect(&self, index: usize, scroll: f32) -> Rect {
        let (row, column) = (index / COLUMNS, index % COLUMNS);
        let y = self.top + CELL / 4.0 + GAP / 2.0 + row as f32 * CELL - scroll;
        Rect::new(LEFT + column as f32 * PITCH, y, WIDTH, CELL - GAP)
    }

    /// The list's box: what the grid is cut to, top and bottom. Across it is left open, for the glow.
    fn clip(&self, ui: &Ui) -> Rect {
        let left = nav::WIDTH + 2.0;
        Rect::new(left, self.cut.0, ui.screen().right() - left, self.cut.1 - self.cut.0)
    }

    /// The foot of the last row's tiles as the grid is drawn now, if that row is wholly in view: what is under the
    /// grid's end hangs from it and shows only then.
    pub fn end(&self, ui: &mut Ui, count: usize) -> Option<f32> {
        let last = self.rect(count.checked_sub(1)?, self.scroll(ui));
        let clip = self.clip(ui);
        (last.y >= clip.y && last.bottom() <= clip.bottom()).then_some(last.bottom())
    }

    fn scroll(&self, ui: &mut Ui) -> f32 {
        ui.slide((self.id, "scroll", self.fresh), self.first_row as f32 * CELL, SCROLL_SECONDS, Ease::CubicOut)
    }

    /// Draws the grid. `focused` says focus is on its tile; `running` is the game being launched or running. With
    /// no games, `empty` is its one line, centred in the list's box. `queue` says which games are being installed.
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        games: &[&Game],
        focused: bool,
        running: Option<Key>,
        queue: &model::Queue,
        empty: &str,
    ) {
        if !self.keep(games) {
            ui.label_centered(LIST.0 + LIST.1 / 2.0, (self.cut.0 + self.cut.1) / 2.0, empty, &NO_GAMES);
            return;
        }
        let scroll = self.scroll(ui);
        let clip = self.clip(ui);
        let id = self.id;
        let face = |ui: &mut Ui, index: usize, focused: bool| {
            let game = games[index];
            let running = running.as_ref().is_some_and(|running| game.is(running));
            let installing = game.id != 0 && queue.get(game.id).is_some();
            let tile = Tile { game, focused, running, installing };
            tile::draw(ui, self.rect(index, scroll), (id, key(game)), &tile);
        };
        // The rows that show any of themselves, the one going out at the top as the grid slides included.
        let first = ((scroll - CELL / 4.0) / CELL).floor().max(0.0) as usize;
        let shown = (first * COLUMNS)..games.len().min((first + ROWS + 2) * COLUMNS);
        ui.with_clip(clip, |ui| {
            for index in shown.clone().filter(|&index| !(focused && index == self.focus)) {
                face(ui, index, false);
            }
            // The focused tile last: its ring and glow lie over its neighbours.
            if focused && shown.contains(&self.focus) {
                face(ui, self.focus, true);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn games(count: usize) -> Vec<Game> {
        (0..count).map(|i| Game { id: i as i64 + 1, source_id: (100 + i).to_string(), ..Game::default() }).collect()
    }

    fn walk(grid: &mut Grid, games: &[&Game], buttons: &[Button]) -> Step {
        buttons.iter().map(|&button| grid.step(button, games)).last().unwrap_or(Step::Stopped)
    }

    /// docs/spec/my-games.md, 5.2.
    #[test]
    fn the_tiles_stand_where_the_spec_says() {
        let grid = Grid::games();
        let close = |a: f32, b: f32| (a - b).abs() < 0.01;
        let first = grid.rect(0, 0.0);
        assert!(close(first.x, 526.0) && close(first.y, 195.29) && close(first.w, 231.6) && close(first.h, 232.14));
        assert!(close(grid.rect(4, 0.0).right(), 1744.0));
        let tops: Vec<f32> = (0..4).map(|row| grid.rect(row * COLUMNS, 0.0).y).collect();
        assert!(close(tops[1], 442.43) && close(tops[2], 689.57) && close(tops[3], 936.71), "{tops:?}");
    }

    #[test]
    fn every_edge_stops_and_says_which_it_is() {
        let games = games(12);
        let games: Vec<&Game> = games.iter().collect();
        let mut grid = Grid::games();
        assert!(grid.keep(&games));
        assert_eq!(grid.step(Button::Left, &games), Step::Off(Button::Left));
        assert_eq!(grid.step(Button::Up, &games), Step::Off(Button::Up));
        assert_eq!(walk(&mut grid, &games, &[Button::Right; 4]), Step::Moved);
        assert_eq!((grid.step(Button::Right, &games), grid.focus()), (Step::Stopped, 4), "nothing wraps");
        // Down onto the shorter last row is its last tile; Right there stops; Down is off the grid.
        assert_eq!(walk(&mut grid, &games, &[Button::Down, Button::Down]), Step::Moved);
        assert_eq!(grid.focus(), 11);
        assert_eq!(grid.step(Button::Right, &games), Step::Stopped);
        assert_eq!(grid.step(Button::Down, &games), Step::Off(Button::Down));
        // Up from there is the tile straight above, not the column Down came from.
        assert_eq!((grid.step(Button::Up, &games), grid.focus()), (Step::Moved, 6));
        assert_eq!(grid.step(Button::A, &games), Step::Stopped);
    }

    #[test]
    fn the_grid_scrolls_a_row_at_a_time_and_only_as_far_as_it_must() {
        let games = games(28);
        let games: Vec<&Game> = games.iter().collect();
        let mut grid = Grid::games();
        grid.keep(&games);
        let mut rows = Vec::new();
        for button in [[Button::Down; 5].as_slice(), &[Button::Up; 5]].concat() {
            grid.step(button, &games);
            rows.push((grid.focus() / COLUMNS, grid.first_row));
        }
        // Down: the focused row stays in a slot until it would leave the third. The sixth row is the last.
        assert_eq!(rows[..5], [(1, 0), (2, 0), (3, 1), (4, 2), (5, 3)]);
        // Up: nothing moves until focus would leave the first slot.
        assert_eq!(rows[5..], [(4, 3), (3, 3), (2, 2), (1, 1), (0, 0)]);
    }

    #[test]
    fn focus_follows_its_game_through_a_new_library_and_falls_back_to_the_first() {
        let all = games(12);
        let mut grid = Grid::games();
        let before: Vec<&Game> = all.iter().collect();
        grid.keep(&before);
        walk(&mut grid, &before, &[Button::Down, Button::Right, Button::Right]);
        assert_eq!(grid.focus(), 7);
        // Two games before it have gone: it is the sixth now.
        let after: Vec<&Game> = all.iter().skip(2).collect();
        assert!(grid.keep(&after));
        assert_eq!((grid.focus(), key(after[grid.focus()])), (5, key(&all[7])));
        // It has gone itself: the first.
        let without: Vec<&Game> = all.iter().filter(|game| key(game) != key(&all[7])).collect();
        grid.keep(&without);
        assert_eq!(grid.focus(), 0);
        assert!(!grid.keep(&[]));
    }

    #[test]
    fn a_shorter_library_brings_the_grid_back_up() {
        let all = games(30);
        let long: Vec<&Game> = all.iter().collect();
        let mut grid = Grid::games();
        grid.keep(&long);
        walk(&mut grid, &long, &[Button::Down; 5]);
        assert_eq!(grid.first_row, 3);
        // The focused game is in the second row of a library of two rows.
        let short: Vec<&Game> = all.iter().skip(20).collect();
        grid.keep(&short);
        assert_eq!((grid.focus() / COLUMNS, grid.first_row), (1, 0));
    }
}
