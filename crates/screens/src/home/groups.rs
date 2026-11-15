//! The Groups under the wide tiles: each a header over a row of up to six games' cards and a See all card.
//! Authored in the rows' units: multiply by ROW_SCALE.
//!
//! Until there are saved filters (M3) the Groups are worked out from the library.
use std::cmp::Ordering;

use model::{Game, Library};
use ui::{Rect, Ui};

use super::{recent, tile, wide};
use crate::theme::{self, glyph, HEADING, MARGIN, ROW_SCALE as S};

/// How many games a Group shows before See all.
pub const CARDS: usize = 6;
/// How many covers See all shows, of the games after those.
const MORE: usize = 4;

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

pub struct Group {
    pub name: &'static str,
    /// The games on its cards, by their places in the library.
    pub games: Vec<usize>,
    /// The games after those, whose covers See all shows.
    pub more: Vec<usize>,
}

impl Group {
    /// Every card: the games', then See all, the Group's last.
    pub fn cards(&self) -> usize {
        self.games.len() + 1
    }
}

/// The Groups the library gives: what is installed, by name, and what has been played most. One with no games is
/// left out.
pub fn of(library: &Library) -> Vec<Group> {
    fn lower(game: &Game) -> impl Iterator<Item = char> + '_ {
        game.name.chars().flat_map(char::to_lowercase)
    }
    let by_name = |a: &Game, b: &Game| lower(a).cmp(lower(b));
    let by_playtime = |a: &Game, b: &Game| b.playtime_minutes.cmp(&a.playtime_minutes);
    let groups = [
        group(library, "Installed", |g| g.installed, by_name),
        group(library, "Most played", |g| g.playtime_minutes > 0, by_playtime),
    ];
    groups.into_iter().filter(|g| !g.games.is_empty()).collect()
}

/// A Group of the games `keep` takes, in `order`; games level in it stay in the library's order.
fn group(
    library: &Library,
    name: &'static str,
    keep: impl Fn(&Game) -> bool,
    order: impl Fn(&Game, &Game) -> Ordering,
) -> Group {
    let games = &library.games;
    let compare = |a: &usize, b: &usize| order(&games[*a], &games[*b]).then(a.cmp(b));
    let mut picked: Vec<usize> = (0..games.len()).filter(|&i| keep(&games[i])).collect();
    // Only the first few are shown: found without sorting the whole library, as this runs every frame.
    let shown = CARDS + MORE;
    if picked.len() > shown {
        picked.select_nth_unstable_by(shown, compare);
        picked.truncate(shown);
    }
    picked.sort_by(compare);
    let more = picked.split_off(picked.len().min(CARDS));
    Group { name, games: picked, more }
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

/// The Groups, with focus on this card of this Group if on any.
pub fn draw(ui: &mut Ui, library: &Library, groups: &[Group], focused: Option<(usize, usize)>) {
    for (g, group) in groups.iter().enumerate() {
        ui.label_middle(MARGIN, top(g) + HEADER * S / 2.0, group.name, &HEADING);
        for card in 0..group.cards() {
            let rect = card_rect(g, card);
            let has_focus = focused == Some((g, card));
            let title = match group.games.get(card).and_then(|&i| library.games.get(i)) {
                Some(game) => {
                    tile::game(ui, rect, game, rect.w, has_focus);
                    game.name.as_str()
                }
                None => {
                    let more = group.more.iter().filter_map(|&i| library.games.get(i));
                    tile::collage(ui, rect, more, glyph::LIBRARY);
                    "See all"
                }
            };
            tile::title_band(ui, rect, ("group", g, card), title, has_focus, rect.w);
            if has_focus {
                theme::focus_ring(ui, rect, S);
            }
        }
    }
}
