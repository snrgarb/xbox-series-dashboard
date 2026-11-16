//! What the Achievements page looks like (docs/spec/achievements-page.md, section 3): the hub's band and header, a
//! card of the game's progress in the place of the hub's first card, and the achievements beside it, two to a row.
use model::achievement::{Achievement, GameProgress};
use model::Game;
use ui::{Color, Font, HAlign, Rect, TextStyle, Ui, VAlign};

use super::{COLUMNS, ROWS};
use crate::achievement::{self, thousands};
use crate::game_hub::{self, backdrop, cards, text};
use crate::pages;
use crate::theme::{self, BODY, TEXT_DIM};

/// The progress card: as wide as the hub's Your game stats, where the hub's first card stands.
const CARD_WIDTH: f32 = 416.0;
/// The list: from 16 past the card to the content's right edge, 1840.
const LIST_LEFT: f32 = cards::LEFT + CARD_WIDTH + GAP;
const LIST_RIGHT: f32 = 1840.0;
const GAP: f32 = 16.0;
/// An achievement's box: half the list's width less the gap, 120 tall.
const TILE_HEIGHT: f32 = 120.0;
/// Its icon, 88 square, 16 in.
const ICON: f32 = 88.0;
const INSET: f32 = 16.0;
/// Its text, from 16 past the icon to 20 short of the box's right.
const TEXT_LEFT: f32 = INSET + ICON + 16.0;
const TEXT_RIGHT: f32 = 20.0;
/// The lines under the name: the description and the state, in the hub's facts' type.
const SMALL: TextStyle = TextStyle::new(Font::Ui, 20.0, 400).color(TEXT_DIM);
const SMALL_BOLD: TextStyle = TextStyle::new(Font::Ui, 20.0, 700);
/// The Score's G disc beside an achievement's Score.
const G: f32 = 22.0;

/// What a secret achievement still locked shows in place of its name and its description, as the console's.
pub const SECRET_NAME: &str = "Secret achievement";
pub const SECRET_DESCRIPTION: &str = "Keep playing to unlock this secret achievement.";

/// An achievement's box, at `at` in the list with `top_row` the first row shown; None for one not on screen.
pub fn tile(at: usize, top_row: usize) -> Option<Rect> {
    let row = (at / COLUMNS).checked_sub(top_row).filter(|&row| row < ROWS)?;
    let width = (LIST_RIGHT - LIST_LEFT - GAP * (COLUMNS - 1) as f32) / COLUMNS as f32;
    let x = LIST_LEFT + (at % COLUMNS) as f32 * (width + GAP);
    Some(Rect::new(x, cards::TOP + row as f32 * (TILE_HEIGHT + GAP), width, TILE_HEIGHT))
}

/// The page: `behind` is the hub's picture of the screen it was opened over; `at` where focus is in `list`.
pub fn draw(
    ui: &mut Ui,
    game: &Game,
    behind: u64,
    progress: Option<GameProgress>,
    list: &[Achievement],
    at: Option<usize>,
    top_row: usize,
) {
    let behind = pages::behind(ui, behind);
    backdrop::draw(ui, game, behind);
    game_hub::header(ui, &game.name, "Achievements");
    progress_card(ui, progress, list);
    for (i, achievement) in list.iter().enumerate() {
        if let Some(rect) = tile(i, top_row) {
            tile_at(ui, rect, achievement);
        }
    }
    if let Some(rect) = at.and_then(|at| tile(at, top_row)) {
        theme::focus_ring_cornered(ui, rect, 14.0);
    }
}

/// The game's progress, in the hub's card: its Achievements row, then the Score of those unlocked of the Score of
/// them all, how many Rare ones are unlocked, and when the last was. What the list is still to say is left out.
fn progress_card(ui: &mut Ui, progress: Option<GameProgress>, list: &[Achievement]) {
    let rect = Rect::new(cards::LEFT, cards::TOP, CARD_WIDTH, cards::HEIGHT);
    let inner = cards::titled(ui, rect, "Your progress");
    let mut y = inner.y;
    if let Some(progress) = progress {
        y += cards::achievements_row(ui, inner.x, y, inner.w, &progress);
    }
    if list.is_empty() {
        return;
    }
    let unlocked = list.iter().filter(|a| a.is_unlocked());
    let score: u64 = unlocked.clone().map(|a| u64::from(a.score())).sum();
    let of: u64 = list.iter().map(|a| u64::from(a.score())).sum();
    let mut pairs = vec![("Score", format!("{} of {}", thousands(score), thousands(of)))];
    if list.iter().any(|a| a.is_rare()) {
        let rare = unlocked.clone().filter(|a| a.is_rare()).count();
        let of = list.iter().filter(|a| a.is_rare()).count();
        pairs.push(("Rare", format!("{rare} of {of} unlocked")));
    }
    if let Some(last) = unlocked.filter_map(|a| a.unlocked).filter(|&at| at > 0).max() {
        pairs.push(("Last unlocked", text::date(last)));
    }
    for (label, value) in pairs {
        cards::pair(ui, inner.x, y, inner.w, label, &value);
        y += cards::PAIR_HEIGHT;
    }
}

/// One achievement's box: its icon; its name, with its Score at the right; its description; and whether it is
/// unlocked (and when), with how rare it is at the right. A secret one still locked keeps its name, description
/// and icon to itself.
fn tile_at(ui: &mut Ui, rect: Rect, a: &Achievement) {
    ui.fill(rect, Color::argb(0x4D2A2A2A), 4.0);
    achievement::icon(ui, Rect::new(rect.x + INSET, rect.y + INSET, ICON, ICON), a, 4.0);
    let secret = a.hidden && !a.is_unlocked();
    let (name, description) =
        if secret { (SECRET_NAME, Some(SECRET_DESCRIPTION)) } else { (a.name.as_str(), a.description.as_deref()) };
    let left = rect.x + TEXT_LEFT;
    let right = rect.right() - TEXT_RIGHT;

    // The Score at the top right: the G disc and the number.
    let score = thousands(u64::from(a.score()));
    let score_width = ui.measure(&BODY, &score).width;
    let name_line = rect.y + 12.0 + BODY.size * 0.665;
    ui.label_right(right, name_line - 2.0, &score, &BODY);
    let g_x = right - score_width - 6.0 - G / 2.0;
    achievement::g_disc(ui, g_x, name_line, G, Color::BLACK);
    let name_width = g_x - G / 2.0 - 16.0 - left;
    ui.paragraph(Rect::new(left, rect.y + 12.0, name_width, 36.0), name, &BODY, HAlign::Left, VAlign::Top, 1);
    if let Some(description) = description {
        let line = Rect::new(left, rect.y + 50.0, right - left, 28.0);
        ui.paragraph(line, description, &SMALL, HAlign::Left, VAlign::Top, 1);
    }

    // At the foot: unlocked, and when, or locked; at the right how rare it is, Rare in bold.
    let foot = rect.y + 82.0;
    let state = match a.unlocked {
        Some(0) => "Unlocked".to_string(),
        Some(when) => format!("Unlocked {}", text::date(when)),
        None => "Locked".to_string(),
    };
    let state_style = if a.is_unlocked() { SMALL.color(Color::WHITE) } else { SMALL };
    let rarity = a.rarity.map(|r| format!("{} of players", achievement::rarity(r)));
    // A little over its measure: a paragraph cuts a line that only just fits.
    let rarity_width = rarity.as_deref().map_or(0.0, |r| ui.measure(&SMALL, r).width.ceil() + 2.0);
    let rare_width = if a.is_rare() { ui.measure(&SMALL_BOLD, "Rare").width + 10.0 } else { 0.0 };
    let state_room = Rect::new(left, foot, (right - left - rarity_width - rare_width - 16.0).max(0.0), 28.0);
    ui.paragraph(state_room, &state, &state_style, HAlign::Left, VAlign::Top, 1);
    if let Some(rarity) = rarity {
        let at = Rect::new(right - rarity_width, foot, rarity_width, 28.0);
        ui.paragraph(at, &rarity, &SMALL, HAlign::Right, VAlign::Top, 1);
        if a.is_rare() {
            let rare = Rect::new(at.x - rare_width, foot, rare_width, 28.0);
            ui.paragraph(rare, "Rare", &SMALL_BOLD, HAlign::Left, VAlign::Top, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_two_wide_beside_the_card_and_five_rows_tall() {
        let first = tile(0, 0).unwrap();
        assert_eq!((first.x, first.y, first.w, first.h), (512.0, 270.4, 656.0, 120.0));
        let second = tile(1, 0).unwrap();
        assert_eq!((second.x, second.right()), (1184.0, 1840.0));
        let last = tile(9, 0).unwrap();
        assert!((last.bottom() - 934.4).abs() < 0.01, "{last:?}");
        assert_eq!(tile(10, 0), None);
        assert_eq!(tile(10, 1).map(|r| r.y), Some(first.y + 4.0 * 136.0));
        assert_eq!(tile(1, 1), None);
    }
}
