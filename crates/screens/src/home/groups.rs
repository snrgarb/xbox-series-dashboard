//! The Groups under the wide tiles: each a header over a row of up to six games' cards and a See all card.
//! Authored in the rows' units: multiply by ROW_SCALE.
//!
//! The Groups are the library's saved filters that are shown and have a game, in the user's order
//! (docs/spec/filters.md): which games are in one, and in what order, the library database has worked out.
use model::{Game, Library};
use ui::{Rect, Ui};

use super::{recent, tile, wide};
use crate::theme::{self, glyph, HEADING, MARGIN, ROW_SCALE as S};

/// How many games a Group shows before See all.
pub const CARDS: usize = 6;
/// How many covers See all shows, of the games after those.
const MORE: usize = 4;

/// The built-in filter that is the Group of last resort, by its key, and its name where the library has no such
/// filter (one that is in no database).
const FAVOURITES: (&str, &str) = ("favourites", "Favourites");

const CARD_WIDTH: f32 = 194.0;
pub const CARD_HEIGHT: f32 = 296.0;
const CARD_GAP: f32 = 28.0;
/// The gap over each Group: under the wide tiles, or the Group before.
const GAP_ABOVE: f32 = 40.0;
/// The header's line: the theme's 28.73 type on its 1.2 line.
const HEADER: f32 = 28.73 * 1.2;
/// How far under a Group's top its cards start: the header and 16.
pub const CARDS_TOP: f32 = HEADER + 16.0;
/// From one Group's top to the next's.
const PITCH: f32 = CARDS_TOP + CARD_HEIGHT + GAP_ABOVE;

pub struct Group<'a> {
    pub name: &'a str,
    /// The saved filter it is, by its id: what See all opens Full library on. None where the library has no such
    /// filter.
    pub filter: Option<i64>,
    /// The games on its cards.
    pub games: Vec<&'a Game>,
    /// The games after those, whose covers See all shows.
    pub more: Vec<&'a Game>,
}

impl Group<'_> {
    /// Every card: the games', then See all, the Group's last.
    pub fn cards(&self) -> usize {
        self.games.len() + 1
    }
}

/// The Groups the library gives: its filters that are shown and have a game, in order, each with the first of its
/// games. With none at all, Favourites alone and empty, a header over a See all card, so that the page under the
/// wide tiles is never blank and Down from them has somewhere to go.
pub fn of(library: &Library) -> Vec<Group<'_>> {
    let mut groups = Vec::new();
    for filter in library.filters.iter().filter(|f| f.shown) {
        // Only the first few are shown, and this runs every frame: no more of the filter's games are looked up.
        let mut games: Vec<&Game> = filter.games.iter().filter_map(|&id| library.game(id)).take(CARDS + MORE).collect();
        if games.is_empty() {
            continue;
        }
        let more = games.split_off(games.len().min(CARDS));
        groups.push(Group { name: &filter.name, filter: Some(filter.id), games, more });
    }
    if groups.is_empty() {
        let favourites = library.filters.iter().find(|f| f.key.as_deref() == Some(FAVOURITES.0));
        let name = favourites.map_or(FAVOURITES.1, |f| f.name.as_str());
        groups.push(Group { name, filter: favourites.map(|f| f.id), games: Vec::new(), more: Vec::new() });
    }
    groups
}

/// A Group's top with the page at rest on the Recent row, in design units. The first is under the Recent row's
/// box, the wide tiles and a gap.
pub fn top(group: usize) -> f32 {
    recent::TOP + (recent::HEIGHT + wide::ROW_HEIGHT + GAP_ABOVE + group as f32 * PITCH) * S
}

pub fn card_rect(group: usize, card: usize) -> Rect {
    let x = MARGIN + card as f32 * (CARD_WIDTH + CARD_GAP) * S;
    Rect::new(x, top(group) + CARDS_TOP * S, CARD_WIDTH * S, CARD_HEIGHT * S)
}

/// The Groups, with focus on this card of this Group if on any; `queue` says which games are being installed.
pub fn draw(ui: &mut Ui, groups: &[Group], queue: &model::Queue, focused: Option<(usize, usize)>) {
    for (g, group) in groups.iter().enumerate() {
        ui.label_middle(MARGIN, top(g) + HEADER * S / 2.0, group.name, &HEADING);
        for card in 0..group.cards() {
            let rect = card_rect(g, card);
            let has_focus = focused == Some((g, card));
            let (title, status) = match group.games.get(card) {
                Some(game) => {
                    tile::game(ui, rect, game, rect.w, has_focus);
                    (game.name.as_str(), queue.status(game.id))
                }
                None => {
                    tile::collage(ui, rect, group.more.iter().copied(), glyph::LIBRARY);
                    ("See all", None)
                }
            };
            tile::status_band(ui, rect, status, has_focus);
            tile::title_band(ui, rect, ("group", g, card), title, status, has_focus, rect.w);
            if has_focus {
                theme::focus_ring(ui, rect, S);
            }
        }
    }
}
