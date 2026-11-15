//! The wide tiles under the Recent row: four, the row's width across. Authored in the rows' units: multiply by
//! ROW_SCALE.
use ui::{Color, Gradient, HAlign, Rect, Ui, VAlign};

use super::{recent, tile};
use crate::theme::{self, glyph, BODY, BODY_SEMIBOLD, MARGIN, ROW_SCALE as S};

const WIDTH: f32 = 359.0;
const HEIGHT: f32 = 190.0;
const GAP: f32 = 30.0;
/// The gap between the Recent row's box and the tiles.
const GAP_ABOVE: f32 = 24.0;
/// What the tiles take of the page's height, with the gap over them.
pub const ROW_HEIGHT: f32 = GAP_ABOVE + HEIGHT;

struct WideTile {
    title: &'static str,
    /// A second line under the title, dimmer.
    subtitle: Option<&'static str>,
    from: Color,
    to: Color,
    glyph: char,
}

const TILES: [WideTile; 4] = [
    WideTile {
        title: "Discord",
        subtitle: None,
        from: Color::rgb(0x7983F5),
        to: Color::rgb(0x4752C4),
        glyph: glyph::CHAT,
    },
    WideTile {
        title: "Settings",
        subtitle: None,
        from: Color::rgb(0x5F6B7A),
        to: Color::rgb(0x2B323C),
        glyph: glyph::SETTINGS,
    },
    // Its subtitle is the Score, once there are achievements to count (M6). Until then it is the Bundle's line for
    // none recorded; its other, "Needs PlayniteAchievements", names a dependency there is none of here.
    WideTile {
        title: "Achievements",
        subtitle: Some("Nothing recorded yet"),
        from: Color::rgb(0x3FA34D),
        to: Color::rgb(0x0E5A1C),
        glyph: glyph::TROPHY,
    },
    // Its subtitle counts the Queue, once there is one.
    WideTile {
        title: "Downloads",
        subtitle: Some("Nothing queued"),
        from: Color::rgb(0x3A8EE6),
        to: Color::rgb(0x1A4C8F),
        glyph: glyph::DOWNLOAD,
    },
];

pub const COUNT: usize = TILES.len();

pub fn rect(i: usize) -> Rect {
    let top = recent::TOP + (recent::HEIGHT + GAP_ABOVE) * S;
    Rect::new(MARGIN + i as f32 * (WIDTH + GAP) * S, top, WIDTH * S, HEIGHT * S)
}

/// The tile whose middle is nearest `x` across.
pub fn nearest(x: f32) -> usize {
    let distance = |i: &usize| (rect(*i).center().0 - x).abs();
    (0..COUNT).min_by(|a, b| distance(a).total_cmp(&distance(b))).unwrap_or(0)
}

pub fn draw(ui: &mut Ui, focused: Option<usize>) {
    for (i, tile) in TILES.iter().enumerate() {
        let rect = rect(i);
        ui.gradient(rect, tile.from, tile.to, Gradient::Diagonal, tile::RADIUS * S);
        let (cx, cy) = rect.center();
        ui.icon(cx, cy - 12.0 * S, tile.glyph, 40.0 * S, Color::WHITE);
        // The caption stands on the tile's foot: the title, and under it the subtitle if there is one.
        let line = ui.measure(&BODY_SEMIBOLD, tile.title).height;
        let mut caption = Rect::new(rect.x + 18.0 * S, rect.bottom() - 14.0 * S - line, rect.w - 36.0 * S, line);
        if let Some(subtitle) = tile.subtitle {
            ui.paragraph(caption, subtitle, &BODY.color(theme::TEXT_DIM), HAlign::Left, VAlign::Top, 1);
            caption.y -= line;
        }
        ui.paragraph(caption, tile.title, &BODY_SEMIBOLD, HAlign::Left, VAlign::Top, 1);
        if focused == Some(i) {
            theme::focus_ring(ui, rect, S);
        }
    }
}
