//! A tile's face, as the Recent row's tiles and the Groups' cards share it: a game's cover or its name, a collage
//! of covers for the tiles that open a list, and the title band a focused tile shows. Sizes are in the rows' units:
//! multiply by ROW_SCALE.
use model::Game;
use ui::{ArtKey, Color, Crop, Ease, HAlign, Kind, Rect, Ui, VAlign};

use crate::theme::{self, BODY, ROW_SCALE as S};

pub const RADIUS: f32 = 6.0;

/// A game's cover for a tile: its portrait cover, else its header, else its wide art.
pub fn cover_art(game: &Game, px: f32) -> Option<ArtKey> {
    let path = game.cover.as_ref().or(game.header.as_ref()).or(game.hero.as_ref())?;
    // Decoded 300 wide at 1080p, as the theme decodes it, and in proportion on a larger screen.
    let width = if px > 1.25 { 600 } else { 300 };
    Some(ArtKey { path: path.clone(), kind: Kind::Thumb(width) })
}

/// A game's face: its cover filling the tile from the top, or for want of one its name on the dark tile, set
/// `name_width` wide so that it doesn't reflow as a tile grows.
pub fn game(ui: &mut Ui, rect: Rect, game: &Game, name_width: f32) {
    let radius = RADIUS * S;
    ui.fill(rect, theme::TILE, radius);
    let has_cover = cover_art(game, ui.px()).is_some_and(|key| ui.art(rect, &key, Crop::Top, 1.0, radius));
    if !has_cover {
        let width = name_width - 20.0 * S;
        let lines = Rect::new(rect.x + (rect.w - width) / 2.0, rect.y + 10.0 * S, width, rect.h - 20.0 * S);
        ui.paragraph(lines, &game.name, &BODY, HAlign::Center, VAlign::Center, 0);
    }
}

/// The face of a tile that opens a list of games: up to four of their covers, two by two, each filled from the
/// top; a game with no cover leaves its place dark. With no games at all, `glyph`.
pub fn collage<'a>(ui: &mut Ui, rect: Rect, games: impl Iterator<Item = &'a Game>, glyph: char) {
    ui.fill(rect, theme::TILE, RADIUS * S);
    let (inset, margin) = (4.0 * S, 3.0 * S);
    let (w, h) = ((rect.w - 2.0 * inset) / 2.0, (rect.h - 2.0 * inset) / 2.0);
    let mut any = false;
    for (i, game) in games.take(4).enumerate() {
        any = true;
        let cell = Rect::new(rect.x + inset + (i % 2) as f32 * w, rect.y + inset + (i / 2) as f32 * h, w, h);
        if let Some(key) = cover_art(game, ui.px()) {
            ui.art(cell.grow(-margin), &key, Crop::Top, 1.0, 0.0);
        }
    }
    if !any {
        let (cx, cy) = rect.center();
        ui.icon(cx, cy, glyph, 40.0 * S, Color::WHITE);
    }
}

/// The band along a focused tile's foot with its title on it, at most two lines: in over 150 ms after 50, out
/// over 100. The text is laid out `width` wide (the tile's width at its largest) and cut to the tile as it is.
pub fn title_band(ui: &mut Ui, rect: Rect, id: impl std::hash::Hash, title: &str, focused: bool, width: f32) {
    // Linear both ways. The first quarter of the value's way is the 50 ms wait, with nothing showing; the way out
    // crosses the same quarter unseen, after its 100 ms.
    let (to, seconds) = if focused { (1.0, 0.2) } else { (0.0, 0.1 / 0.75) };
    let band = (ui.tween((id, "band"), to, seconds, Ease::Linear) - 0.25) / 0.75;
    if band <= 0.0 {
        return;
    }
    let width = width - 16.0 * S;
    let text = ui.measure_wrapped(&BODY, title, width, 2);
    let height = text.height + 16.0 * S;
    ui.with_alpha(band, |ui| {
        // The tile's own shape, cut to the band: square across the top, the tile's corners at the foot.
        ui.with_clip(Rect::new(rect.x, rect.bottom() - height, rect.w, height), |ui| {
            ui.fill(rect, theme::BAND, RADIUS * S);
            let lines = Rect::new(rect.x + 8.0 * S, rect.bottom() - height + 8.0 * S, width, text.height);
            ui.paragraph(lines, title, &BODY, HAlign::Left, VAlign::Top, 2);
        });
    });
}
