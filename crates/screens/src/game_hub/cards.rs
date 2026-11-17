//! The hub's row of cards (docs/spec/game-hub.md, section 5): the cover, and whatever the library can tell of the
//! game. A card with nothing to show is left out, and those after it close up to the left.
use model::achievement::GameProgress;
use model::Game;
use ui::{ArtKey, Color, Crop, Font, HAlign, Kind, Rect, TextStyle, Ui, VAlign, DESIGN_WIDTH};

use super::text;
use crate::theme::{self, BODY};
use crate::{achievement, home};

/// The cards' top: under the title's line (56.4), the facts' (54), and 32 of margins, from the content's top at 128.
pub const TOP: f32 = 270.4;
pub const HEIGHT: f32 = 500.0;
/// The first card's left edge, and the content's.
pub const LEFT: f32 = 80.0;
const GAP: f32 = 16.0;
/// A row of cards wider than the screen ends this far from the screen's right edge when it is lined up by its end.
const END: f32 = 64.0;
const FILL: Color = Color::argb(0x4D2A2A2A);
const RADIUS: f32 = 4.0;
/// A card's padding, across and down.
const PADDING: (f32, f32) = (26.0, 22.0);
const TITLE: TextStyle = TextStyle::new(Font::Ui, 20.0, 700);
/// A card's title with the 12 under it.
const TITLE_HEIGHT: f32 = 38.6;
/// A label and its value: two lines of 34.58, 2 between them and 10 after.
pub const PAIR_HEIGHT: f32 = 81.16;
const VALUE_DOWN: f32 = 36.58;
/// A value stops this far short of its column's end.
const VALUE_MARGIN: f32 = 16.0;
/// The chip the source's mark sits on, at the cover's bottom left.
const CHIP: Color = Color::argb(0xCC1A1A1A);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Card {
    Cover,
    /// Your game stats.
    Stats,
    Info,
    /// The Game details page's wide card: the game's art, where the Bundle plays a trailer over it.
    Trailer,
}

impl Card {
    pub fn width(self) -> f32 {
        match self {
            Card::Cover => 375.0,
            Card::Stats => 416.0,
            Card::Info => 436.0,
            Card::Trailer => 889.0,
        }
    }
}

/// The rows of Your game stats that have something to say: how long the game was played, how many times (the Play
/// sessions the Dashboard recorded), and when last.
///
/// The Bundle's Completion row is not here: it is a Playnite notion with no store behind it. The Achievements row
/// comes last, under these, for a game that has achievements ([`achievements_row`]).
fn stats(game: &Game) -> Vec<(&'static str, String)> {
    let mut rows = Vec::new();
    if game.playtime_minutes > 0 {
        rows.push(("Time played", text::playtime(game.playtime_minutes)));
    }
    if game.play_count > 0 {
        rows.push(("Times played", game.play_count.to_string()));
    }
    if game.last_played > 0 {
        rows.push(("Last played", text::date(game.last_played)));
    }
    rows
}

/// Info's pairs that have something to say, in the Bundle's order. Its Genres and Series need names Steam's files
/// have only numbers for, and Your score is Playnite's: none of the three is here.
fn info(game: &Game) -> Vec<(&'static str, String)> {
    let mut pairs = Vec::new();
    if !game.developers.is_empty() {
        pairs.push(("Developer", game.developers.join(", ")));
    }
    pairs.extend(game.critic_score.map(|score| ("Critic score", score.to_string())));
    pairs.extend(game.community_score.map(|score| ("Community score", score.to_string())));
    pairs
}

/// The cards that show for this game, left to right, on the Game hub page or on Game details, `progress` how far
/// it has got with its achievements if it has any. The Bundle's About and Description are in neither: no file of
/// Steam's on this machine holds a game's description.
pub fn showing(game: &Game, details: bool, progress: Option<&GameProgress>) -> Vec<Card> {
    let mut cards = vec![Card::Cover];
    if details {
        cards.push(Card::Trailer);
    } else if !stats(game).is_empty() || progress.is_some() {
        cards.push(Card::Stats);
    }
    if !info(game).is_empty() {
        cards.push(Card::Info);
    }
    cards
}

/// Where each of `cards` is. A row that fits the screen starts at the content's left edge. One that does not has
/// two places and jumps between them: there too, but for focus on Info, when its end is lined up instead.
pub fn rects(cards: &[Card], focus: Option<Card>) -> Vec<Rect> {
    let strip = LEFT + END + cards.iter().map(|card| card.width() + GAP).sum::<f32>();
    let by_its_end = strip > DESIGN_WIDTH && focus == Some(Card::Info);
    let mut x = if by_its_end { LEFT + DESIGN_WIDTH - strip } else { LEFT };
    let place = |card: &Card| {
        let rect = Rect::new(x, TOP, card.width(), HEIGHT);
        x += card.width() + GAP;
        rect
    };
    cards.iter().map(place).collect()
}

/// The game's cover for a card: twice a tile's width, as the card is larger than one.
pub(crate) fn cover_key(game: &Game, px: f32) -> Option<ArtKey> {
    let key = home::cover_art(game, px)?;
    Some(ArtKey { kind: Kind::Thumb(if px > 1.25 { 1200 } else { 600 }), ..key })
}

/// Draws the cards, and the focus ring round the one with focus, over all of them.
pub fn draw(ui: &mut Ui, game: &Game, cards: &[Card], focus: Option<Card>, progress: Option<&GameProgress>) {
    let rects = rects(cards, focus);
    // A row wider than the screen is cut at the screen's edges, and 24 above and below the cards for the ring.
    let screen = ui.screen();
    let clip = Rect::new(screen.x, TOP - 24.0, screen.w, HEIGHT + 48.0);
    ui.with_clip(clip, |ui| {
        for (&card, &rect) in cards.iter().zip(&rects) {
            match card {
                Card::Cover => cover(ui, rect, game),
                Card::Trailer => trailer(ui, rect, game),
                Card::Stats => {
                    let inner = titled(ui, rect, "Your game stats");
                    let rows = stats(game);
                    for (row, (label, value)) in rows.iter().enumerate() {
                        pair(ui, inner.x, inner.y + row as f32 * PAIR_HEIGHT, inner.w, label, value);
                    }
                    if let Some(progress) = progress {
                        achievements_row(ui, inner.x, inner.y + rows.len() as f32 * PAIR_HEIGHT, inner.w, progress);
                    }
                }
                Card::Info => {
                    let inner = titled(ui, rect, "Info");
                    let column = inner.w / 2.0;
                    for (at, (label, value)) in info(game).iter().enumerate() {
                        let (x, y) = (inner.x + (at % 2) as f32 * column, inner.y + (at / 2) as f32 * PAIR_HEIGHT);
                        pair(ui, x, y, column, label, value);
                    }
                }
            }
        }
        if let Some(at) = focus.and_then(|focus| cards.iter().position(|&card| card == focus)) {
            theme::focus_ring_cornered(ui, rects[at], 14.0);
        }
    });
}

/// The Achievements row (GameDetails.xaml, StatsAchievements): "Achievements"; under it the count unlocked, "2 / 6",
/// and at the right the share, "33%", dimmer; then the bar, `width` long. Returns how tall it is: two lines of
/// 34.58, the 2 and 10 round the second, and the bar's 6.
pub fn achievements_row(ui: &mut Ui, x: f32, y: f32, width: f32, progress: &GameProgress) -> f32 {
    line(ui, x, y, width, "Achievements", &BODY);
    let count = format!("{} / {}", progress.unlocked, progress.of);
    line(ui, x, y + VALUE_DOWN, width, &count, &BODY);
    let percent = format!("{}%", achievement::percent(progress.unlocked, progress.of));
    let room = Rect::new(x, y + VALUE_DOWN, width, BODY.size * 1.5);
    ui.paragraph(room, &percent, &BODY.color(theme::TEXT_DIM), HAlign::Right, VAlign::Top, 1);
    let bar_top = y + VALUE_DOWN + ROW_LINE + 10.0;
    let share = if progress.of == 0 { 0.0 } else { progress.unlocked as f32 / progress.of as f32 };
    achievement::bar(ui, Rect::new(x, bar_top, width, 6.0), share);
    bar_top + 6.0 - y
}

/// A row's line: Segoe UI 26's, 34.58.
const ROW_LINE: f32 = 34.58;

/// A card's fill and title. Returns where its rows go.
pub fn titled(ui: &mut Ui, rect: Rect, title: &str) -> Rect {
    ui.fill(rect, FILL, RADIUS);
    let inner = Rect::new(rect.x + PADDING.0, rect.y + PADDING.1, rect.w - 2.0 * PADDING.0, rect.h - 2.0 * PADDING.1);
    line(ui, inner.x, inner.y, inner.w, title, &TITLE);
    Rect::new(inner.x, inner.y + TITLE_HEIGHT, inner.w, inner.h - TITLE_HEIGHT)
}

/// One line from (`x`, `y`), ending in an ellipsis where it would pass `width`.
fn line(ui: &mut Ui, x: f32, y: f32, width: f32, text: &str, style: &TextStyle) {
    let height = style.size * 1.5;
    ui.paragraph(Rect::new(x, y, width, height), text, style, HAlign::Left, VAlign::Top, 1);
}

/// A label with its value under it, in a column `width` wide.
pub fn pair(ui: &mut Ui, x: f32, y: f32, width: f32, label: &str, value: &str) {
    line(ui, x, y, width, label, &BODY);
    line(ui, x, y + VALUE_DOWN, width - VALUE_MARGIN, value, &BODY);
}

/// The cover card: the cover filling it from the top, as a tile's does, or for want of one the game's name on the
/// dark tile; and its source's mark on a chip at the bottom left, which stays while the card has focus.
fn cover(ui: &mut Ui, rect: Rect, game: &Game) {
    ui.fill(rect, theme::TILE, RADIUS);
    let has_cover = cover_key(game, ui.px()).is_some_and(|key| ui.art(rect, &key, Crop::Top, 1.0, RADIUS));
    if !has_cover {
        ui.paragraph(rect.grow(-20.0), &game.name, &BODY, HAlign::Center, VAlign::Center, 0);
    }
    // The mark is 20 across, 8 and 4 in from the chip's edges; the chip 10 in from the card's.
    let chip = Rect::new(rect.x + 10.0, rect.bottom() - 10.0 - 28.0, 36.0, 28.0);
    ui.fill(chip, CHIP, 3.0);
    home::mark_drawn(ui, Rect::new(chip.x + 8.0, chip.y + 4.0, 20.0, 20.0), home::game_mark(game));
}

/// The Game details page's wide card: the game's background art about its middle, over its cover; with neither,
/// the card's fill alone. The Bundle plays the game's trailer over them where another add-on has one.
fn trailer(ui: &mut Ui, rect: Rect, game: &Game) {
    ui.fill(rect, FILL, RADIUS);
    let width = if ui.px() > 1.25 { 1800 } else { 900 };
    let art = |path: &std::path::PathBuf| ArtKey { path: path.clone(), kind: Kind::Thumb(width) };
    let shown = game.hero.as_ref().is_some_and(|hero| ui.art(rect, &art(hero), Crop::Center, 1.0, 0.0));
    if !shown {
        if let Some(cover) = game.cover.as_ref().or(game.header.as_ref()) {
            ui.art(rect, &art(cover), Crop::Center, 1.0, 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn told() -> Game {
        Game {
            name: "Brass Meridian".into(),
            playtime_minutes: 600,
            play_count: 3,
            last_played: 1_789_992_000,
            developers: vec!["Tin Lantern".into(), "Second Kettle".into()],
            critic_score: Some(87),
            community_score: Some(0),
            ..Game::default()
        }
    }

    #[test]
    fn a_card_with_nothing_to_show_is_left_out() {
        let game = told();
        assert_eq!(showing(&game, false, None), [Card::Cover, Card::Stats, Card::Info]);
        assert_eq!(showing(&game, true, None), [Card::Cover, Card::Trailer, Card::Info]);
        // A game the library knows only the name of: the cover alone, and on Game details its art.
        let bare = Game { name: "Bare".into(), ..Game::default() };
        assert_eq!(showing(&bare, false, None), [Card::Cover]);
        assert_eq!(showing(&bare, true, None), [Card::Cover, Card::Trailer]);
        // Never played, but Steam says who made it.
        let unplayed = Game { playtime_minutes: 0, play_count: 0, last_played: 0, ..told() };
        assert_eq!(showing(&unplayed, false, None), [Card::Cover, Card::Info]);
        // Never played, with achievements: Your game stats has their row alone.
        let progress = GameProgress { game: 1, unlocked: 0, of: 6, score: 0 };
        assert_eq!(showing(&unplayed, false, Some(&progress)), [Card::Cover, Card::Stats, Card::Info]);
    }

    #[test]
    fn the_rows_are_those_with_something_to_say() {
        let game = told();
        let stats = stats(&game);
        assert_eq!(stats.iter().map(|row| row.0).collect::<Vec<_>>(), ["Time played", "Times played", "Last played"]);
        assert_eq!((stats[0].1.as_str(), stats[1].1.as_str(), stats[2].1.as_str()), ("10h 0m", "3", "9/21/2026"));
        let info = info(&game);
        // A score of 0 is a score.
        let expected = [("Developer", "Tin Lantern, Second Kettle"), ("Critic score", "87"), ("Community score", "0")];
        assert_eq!(info.iter().map(|(label, value)| (*label, value.as_str())).collect::<Vec<_>>(), expected);
        let played_once = Game { play_count: 0, critic_score: None, ..game };
        assert_eq!(super::stats(&played_once).len(), 2);
        assert_eq!(super::info(&played_once).len(), 2);
    }

    #[test]
    fn the_cards_are_where_the_spec_says() {
        let hub = rects(&[Card::Cover, Card::Stats, Card::Info], None);
        let across = |rects: &[Rect]| rects.iter().map(|r| (r.x, r.right())).collect::<Vec<_>>();
        assert_eq!(across(&hub), [(80.0, 455.0), (471.0, 887.0), (903.0, 1339.0)]);
        assert!(hub.iter().all(|r| (r.y, r.h) == (270.4, 500.0)));
        // Game details fits the screen without the Bundle's Description, so focus on Info moves nothing.
        let details = [Card::Cover, Card::Trailer, Card::Info];
        assert_eq!(across(&rects(&details, Some(Card::Info))), [(80.0, 455.0), (471.0, 1360.0), (1376.0, 1812.0)]);
        // A row that does not fit lines its end up 64 and a gap from the screen's edge while focus is on Info.
        let wide = [Card::Cover, Card::Trailer, Card::Stats, Card::Info];
        assert_eq!(rects(&wide, Some(Card::Trailer))[0].x, 80.0);
        assert_eq!(rects(&wide, Some(Card::Info))[3].right(), 1840.0);
    }
}
