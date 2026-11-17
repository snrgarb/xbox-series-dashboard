//! A game's tile in the grid of Games and of Full library (docs/spec/my-games.md, 5.3). It is not Home's tile: it
//! does not grow, its title band is a gradient, and its state badge marks a game that is not installed.
use std::hash::Hash;

use model::Game;
use ui::{Color, Ease, Gradient, HAlign, Rect, Ui, VAlign};

use super::look::{glyph as page_glyph, TEXT};
use crate::home::{art_for, face_arriving, source, source_badge_at};
use crate::theme::{self, glyph, BAND};

const RADIUS: f32 = 6.0;
/// The state badge, 6 in from the tile's top left.
const BADGE: f32 = 24.0;
const BADGE_BACK: Color = Color::argb(0xE62B2B2B);
/// The title band: its text 12 in from the sides, 36 under the band's top and 10 over the tile's foot.
const BAND_SIDE: f32 = 12.0;
const BAND_TOP: f32 = 36.0;
const BAND_FOOT: f32 = 10.0;

pub struct Tile<'a> {
    pub game: &'a Game,
    pub focused: bool,
    /// The game is the one being launched or running: the play mark, top left.
    pub running: bool,
    /// The game is in the Queue: "Installing...", queued or not, as the Bundle's list reads it (Playnite has one
    /// flag for both), in place of the source's badge and the state badge.
    pub installing: bool,
}

/// What an installing tile reads, at its foot or under its title.
const INSTALLING: &str = model::queue::INSTALLING;
/// The Installing band's text: 28 under the band's top, 12 in from the sides, 10 over the tile's foot.
const INSTALLING_TOP: f32 = 28.0;

/// How much of the title band shows: in over 150 ms after 50, out over 100, both linear; there and gone at once
/// under Reduce motion, unlike Home's.
fn band(ui: &mut Ui, id: impl Hash, focused: bool) -> f32 {
    if ui.store.reduce_motion {
        return if focused { 1.0 } else { 0.0 };
    }
    // The first quarter of the value's way is the 50 ms wait; the way out crosses it unseen, after its 100 ms.
    let (to, seconds) = if focused { (1.0, 0.2) } else { (0.0, 0.1 / 0.75) };
    ((ui.tween((id, "band"), to, seconds, Ease::Linear) - 0.25) / 0.75).max(0.0)
}

pub fn draw(ui: &mut Ui, rect: Rect, id: impl Hash, tile: &Tile) {
    let Tile { game, focused, running, installing } = *tile;
    let band = band(ui, id, focused);
    ui.fill(rect, theme::TILE, RADIUS);
    // A cover downloaded while the Dashboard runs fades in over the name, as on Home's tiles.
    let key = art_for(game, rect, ui.px());
    let cover = face_arriving(ui, (&game.source, &game.source_id, &game.name), rect, key.as_ref(), RADIUS);
    if cover < 1.0 {
        // The Bundle's tile has no fallback of its own (Playnite supplies a default cover): here it is the game's
        // name, as on Home's tiles, which gives way to the title band.
        let lines = rect.grow(-10.0);
        let name = (1.0 - band) * (1.0 - cover);
        ui.with_alpha(name, |ui| ui.paragraph(lines, &game.name, &TEXT, HAlign::Center, VAlign::Center, 0));
    }
    // The source's badge goes at once as focus comes, and the title takes its corner; an installing game's caption
    // takes it the rest of the time.
    if let Some(source) = source(game).filter(|_| !focused && !installing) {
        source_badge_at(ui, rect, source, 1.0);
    }
    let mark = match (running, game.installed) {
        (true, _) => Some((page_glyph::PLAY, 11.0)),
        (false, false) if !installing => Some((glyph::DOWNLOAD, 14.0)),
        (false, _) => None,
    };
    if let Some((mark, size)) = mark {
        let badge = Rect::new(rect.x + 6.0, rect.y + 6.0, BADGE, BADGE);
        ui.fill(badge, BADGE_BACK, 3.0);
        ui.icon(badge.center().0, badge.center().1, mark, size, Color::WHITE);
    }
    let width = rect.w - 2.0 * BAND_SIDE;
    // Installing...: at the foot while the tile has no focus, gone at once as focus comes.
    if installing && !focused {
        let line = ui.measure_wrapped(&TEXT, INSTALLING, width, 1).height;
        let under = Rect::new(
            rect.x,
            rect.bottom() - INSTALLING_TOP - line - BAND_FOOT,
            rect.w,
            INSTALLING_TOP + line + BAND_FOOT,
        );
        ui.gradient(under, Color::CLEAR, BAND, Gradient::Vertical, RADIUS);
        let text = Rect::new(rect.x + BAND_SIDE, under.y + INSTALLING_TOP, width, line);
        ui.paragraph(text, INSTALLING, &TEXT, HAlign::Left, VAlign::Top, 1);
    }
    if band > 0.0 {
        let text = ui.measure_wrapped(&TEXT, &game.name, width, 2);
        let line = if installing { ui.measure_wrapped(&TEXT, INSTALLING, width, 1).height } else { 0.0 };
        let height = BAND_TOP + text.height + line + BAND_FOOT;
        let under = Rect::new(rect.x, rect.bottom() - height, rect.w, height);
        ui.with_alpha(band, |ui| {
            // Clear at its top, so only the foot's corners show the tile's rounding.
            ui.gradient(under, Color::CLEAR, BAND, Gradient::Vertical, RADIUS);
            let lines = Rect::new(rect.x + BAND_SIDE, under.y + BAND_TOP, width, text.height);
            ui.paragraph(lines, &game.name, &TEXT, HAlign::Left, VAlign::Top, 2);
            if installing {
                let under_title = Rect::new(lines.x, lines.bottom(), width, line);
                ui.paragraph(under_title, INSTALLING, &TEXT, HAlign::Left, VAlign::Top, 1);
            }
        });
    }
    if focused {
        theme::focus_ring(ui, rect, 1.0);
    }
}
