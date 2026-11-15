//! The Recent row: the games last played, each tile growing with focus, and the Play history tile at its end.
//! Authored in the rows' units: multiply by ROW_SCALE.
use model::{Game, Library};
use ui::{Rect, Ui};

use super::tile;
use crate::theme::{self, glyph, MARGIN, ROW_SCALE as S};

/// How many games the row holds, before the Play history tile.
pub const GAMES: usize = 7;
/// How many covers the Play history tile shows.
const HISTORY: usize = 4;

const TILE: f32 = 153.0;
const GROWTH: f32 = 92.0;
const GAP: f32 = 30.0;
/// The top of the row's box, in design units.
pub const TOP: f32 = 533.0;
/// The row's box, as tall as a focused tile; tiles stand on its bottom.
pub const HEIGHT: f32 = TILE + GROWTH;
/// A tile closes on its size by 63% every 50 ms.
const GROWTH_TAU: f32 = 0.05;

/// What the row shows. A game's place in the row is its place in the library.
pub struct Row<'a> {
    /// The games played last, and when those are too few the first never played.
    pub games: &'a [Game],
    /// The next games played after those: the Play history tile's covers.
    pub history: &'a [Game],
}

impl<'a> Row<'a> {
    pub fn of(library: &'a Library) -> Self {
        let (games, rest) = library.games.split_at(library.games.len().min(GAMES));
        let played = rest.iter().take_while(|g| g.last_played > 0).count();
        Self { games, history: &rest[..played.min(HISTORY)] }
    }

    /// Every tile: the games', then Play history's.
    pub fn tiles(&self) -> usize {
        self.games.len() + 1
    }
}

/// The row's tiles, each with its share of the growth: standing on one line from the left margin. What no tile
/// has taken of the growth widens the gaps, so the row is as wide whatever has focus.
fn rects(count: usize, share: &dyn Fn(usize) -> f32) -> Vec<Rect> {
    let total: f32 = (0..count).map(share).sum();
    let gap = GAP + if count > 1 { GROWTH * (1.0 - total).max(0.0) / (count - 1) as f32 } else { 0.0 };
    let bottom = TOP + HEIGHT * S;
    let mut x = 0.0;
    (0..count)
        .map(|i| {
            let side = TILE + GROWTH * share(i);
            let rect = Rect::new(MARGIN + x * S, bottom - side * S, side * S, side * S);
            x += side + gap;
            rect
        })
        .collect()
}

/// The tiles at rest with focus on `focused`, or on none of them.
pub fn rects_at_rest(count: usize, focused: Option<usize>) -> Vec<Rect> {
    rects(count, &|i| if focused == Some(i) { 1.0 } else { 0.0 })
}

pub fn draw(ui: &mut Ui, row: &Row, focused: Option<usize>) {
    // Each tile's share of the growth closes on 1 with focus and 0 without.
    let shares: Vec<f32> = (0..row.tiles())
        .map(|i| ui.approach(("recent tile", i), if focused == Some(i) { 1.0 } else { 0.0 }, GROWTH_TAU))
        .collect();
    let rects = rects(row.tiles(), &|i| shares[i]);
    for (i, &rect) in rects.iter().enumerate() {
        let title = match row.games.get(i) {
            Some(game) => {
                tile::game(ui, rect, game, TILE * S, focused == Some(i));
                game.name.as_str()
            }
            None => {
                tile::collage(ui, rect, row.history.iter(), glyph::HISTORY);
                "Play history"
            }
        };
        tile::title_band(ui, rect, ("recent", i), title, focused == Some(i), HEIGHT * S);
    }
    if let Some(&rect) = focused.and_then(|i| rects.get(i)) {
        theme::focus_ring(ui, rect, S);
    }
}
