//! Search drawn (docs/spec/search.md, sections 1 to 3; the Bundle's `SearchControl.xaml`): the page's gradient, the
//! search box, the rows of results, and the keyboard at the foot while it is up. Nothing here is scaled: every
//! number is the Bundle's own, on the 1920 x 1080 canvas.
use std::path::PathBuf;

use model::{Game, Library};
use ui::art::RgbaImage;
use ui::{ArtKey, Color, Crop, Ease, Font, HAlign, Kind, Painter, Rect, TextStyle, Ui, VAlign};

use super::{Focus, Search};
use crate::home::cover_art;
use crate::keyboard::Board;
use crate::theme::{self, ACCENT_LIGHT, BODY};

/// The page's gradient, from the top left corner to a point 0.6 of the way across the foot
/// (`StartPoint="0,0" EndPoint="0.6,1"`), its colours square to that line on the screen: dark green, a darker green
/// by 0.45, black by 0.8 and on. The Bundle's picture 15 has it so: black along the top from three quarters across,
/// the darker green at the foot of the left edge.
const STOPS: [(f32, u32); 3] = [(0.0, 0x123A2C), (0.45, 0x07140F), (0.8, 0x000000)];
/// What shows until the gradient's picture is ready: its middle colour.
const GROUND: Color = Color::rgb(0x07140F);

/// The search box: 960 by 54, 77 from the top, centred; its text 28 in from each end.
const BOX: Rect = Rect::new(480.0, 77.0, 960.0, 54.0);
const BOX_INSET: f32 = 28.0;
const BOX_FILL: Color = Color::rgb(0x2E3136);
const BOX_FOCUSED: Color = Color::rgb(0x44474E);
/// The placeholder starts 4 into the text's room, in the page's dim grey.
const PLACEHOLDER: &str = "Search for games and apps";
const DIM: Color = Color::rgb(0xB4B7BD);

/// The results start 36 under the box, and stop 20 over the keyboard while it is up.
const RESULTS_TOP: f32 = 167.0;
const OVER_KEYS: f32 = 20.0;
/// A row: its heading (Bahnschrift SemiLight 30, a line of 36), 20 under it the tiles, 30 under them the next row.
const HEADING: TextStyle = TextStyle::new(Font::Display, 30.0, 350);
const TILES_DOWN: f32 = 56.0;
const GAME_ROW: f32 = 458.0;
/// The row's left edge, the tiles' and the heading's.
const LEFT: f32 = 74.0;
/// A tile: 240 wide, 15 between; a square cover over a caption 132 tall, rounded by 6 all round.
const TILE: f32 = 240.0;
const PITCH: f32 = 255.0;
const COVER: f32 = 240.0;
const CAPTION: f32 = 132.0;
const RADIUS: f32 = 6.0;
const COVER_GROUND: Color = Color::rgb(0x1A1A1A);
const CAPTION_FILL: Color = Color::rgb(0x3A3D44);
/// The caption's text is 18 in from the sides and 14 down; the title two lines at most.
const CAPTION_SIDE: f32 = 18.0;
const CAPTION_TOP: f32 = 14.0;
const TITLE_HEIGHT: f32 = 72.0;
/// The row shows whole tiles only: as many as fit the page less 74 each side, seven at most, and is cut 9 left of
/// the first and 15 right of the last.
pub const TILES_SHOWN: usize = 7;
const CLIP_LEFT: f32 = 9.0;
/// A row slides along over 300 ms.
const SLIDE: f32 = 0.3;

fn gradient_key() -> ArtKey {
    ArtKey { path: PathBuf::from("search/gradient"), kind: Kind::Painted(Painter(paint_gradient)) }
}

/// The gradient, a pixel to every four of the canvas: the colours change slowly enough that nothing shows of it.
fn paint_gradient() -> RgbaImage {
    let (width, height) = (480u32, 270u32);
    let (end_x, end_y) = (0.6 * ui::DESIGN_WIDTH, ui::DESIGN_HEIGHT);
    let channel = |rgb: u32, shift: u32| ((rgb >> shift) & 0xff) as f32;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let at_x = (x as f32 + 0.5) * ui::DESIGN_WIDTH / width as f32;
            let at_y = (y as f32 + 0.5) * ui::DESIGN_HEIGHT / height as f32;
            let t = (at_x * end_x + at_y * end_y) / (end_x * end_x + end_y * end_y);
            let colour = |shift: u32| {
                let last = channel(STOPS[STOPS.len() - 1].1, shift);
                let value = STOPS.windows(2).find(|pair| t <= pair[1].0).map_or(last, |pair| {
                    let ((from, a), (to, b)) = (pair[0], pair[1]);
                    let along = ((t - from) / (to - from)).clamp(0.0, 1.0);
                    channel(a, shift) + (channel(b, shift) - channel(a, shift)) * along
                });
                value.round() as u8
            };
            pixels.extend_from_slice(&[colour(16), colour(8), colour(0), 255]);
        }
    }
    RgbaImage::from_raw(width, height, pixels).expect("as many pixels as the picture has")
}

/// The first tile's left edge, and so the row's.
fn tile_x(at: usize, slid: f32) -> f32 {
    LEFT + PITCH * (at as f32 - slid)
}

pub(super) fn page(search: &mut Search, ui: &mut Ui, library: &Library) {
    let screen = ui.screen();
    ui.fill(screen, GROUND, 0.0);
    ui.art(screen, &gradient_key(), Crop::Center, 1.0, 0.0);

    search_box(search, ui);

    let bottom = if search.up { Board::rect(ui).y - OVER_KEYS } else { screen.bottom() };
    let viewport = Rect::new(screen.x, RESULTS_TOP, screen.w, bottom - RESULTS_TOP);
    scroll_to_focus(search, viewport.h);
    ui.with_clip(viewport, |ui| {
        let mut top = RESULTS_TOP - search.scroll;
        for at in 0..search.rows.len() {
            row(search, ui, library, at, top);
            top += GAME_ROW;
        }
    });

    if search.up {
        search.board.draw(ui, search.focus == Focus::Keys);
    }
}

/// The smallest scroll that has the focused tile wholly in view, at once: WPF's bringing it into view.
fn scroll_to_focus(search: &mut Search, height: f32) {
    let content = GAME_ROW * search.rows.len() as f32 + 24.0;
    if let Focus::Tile { row, .. } = search.focus {
        let top = GAME_ROW * row as f32 + TILES_DOWN;
        let bottom = top + COVER + CAPTION;
        if top < search.scroll {
            search.scroll = top;
        } else if bottom > search.scroll + height {
            search.scroll = bottom - height;
        }
    }
    search.scroll = search.scroll.min(content - height).max(0.0);
}

fn search_box(search: &Search, ui: &mut Ui) {
    let focused = search.focus == Focus::Box;
    if focused {
        // The theme's rectangular glow, though the ring round it is a pill (docs/spec/search.md, open question 6).
        ui.glow(BOX.grow(11.5), ACCENT_LIGHT.fade(0.5), 17.5, 11.0, 17.0);
    }
    ui.fill(BOX, if focused { BOX_FOCUSED } else { BOX_FILL }, BOX.h / 2.0);
    if focused {
        let ring = BOX.grow(8.0);
        ui.ring(ring, ACCENT_LIGHT, ring.h / 2.0, 4.0);
    }
    let text = Rect::new(BOX.x + BOX_INSET, BOX.y, BOX.w - 2.0 * BOX_INSET, BOX.h);
    if search.board.is_empty() {
        let room = Rect::new(text.x + 4.0, text.y, text.w - 4.0, text.h);
        let style = BODY.color(DIM);
        ui.paragraph(room, PLACEHOLDER, &style, HAlign::Left, VAlign::Center, 1);
    }
    // The caret only while the keyboard is up.
    search.board.draw_text(ui, text, false, search.up);
}

fn row(search: &Search, ui: &mut Ui, library: &Library, at: usize, top: f32) {
    let row = &search.rows[at];
    ui.label(LEFT, top, row.kind.heading(), &HEADING);
    let slid = ui.slide(("search row", search.generation, at), row.first as f32, SLIDE, Ease::CubicOut);
    let tiles_top = top + TILES_DOWN;
    // Cut 9 left of the first tile shown and 15 right of the last, and 24 above and below: room for the ring.
    let clip =
        Rect::new(LEFT - CLIP_LEFT, tiles_top - 24.0, PITCH * TILES_SHOWN as f32 + CLIP_LEFT, COVER + CAPTION + 48.0);
    let focused = match search.focus {
        Focus::Tile { row, at: tile } if row == at => Some(tile),
        _ => None,
    };
    ui.with_clip(clip, |ui| {
        // The focused tile last, so its glow lies over its neighbours.
        let order = (0..row.games.len()).filter(|&i| Some(i) != focused).chain(focused);
        for i in order {
            let x = tile_x(i, slid);
            if x + TILE < clip.x || x > clip.right() {
                continue;
            }
            let Some(game) = library.games.get(row.games[i]) else { continue };
            tile(ui, Rect::new(x, tiles_top, TILE, COVER + CAPTION), game, Some(i) == focused);
        }
    });
}

/// A game's tile: its cover, square, over a grey caption with its name. It never grows, and has no badges.
fn tile(ui: &mut Ui, rect: Rect, game: &Game, focused: bool) {
    ui.fill(rect, CAPTION_FILL, RADIUS);
    let cover = Rect::new(rect.x, rect.y, rect.w, COVER);
    // Rounded at the tile's top corners only: drawn a radius taller, and cut at the caption.
    let rounded = Rect::new(cover.x, cover.y, cover.w, cover.h + RADIUS);
    ui.with_clip(cover, |ui| {
        ui.fill(rounded, COVER_GROUND, RADIUS);
        let drawn = cover_art(game, ui.px()).is_some_and(|key| ui.art(rounded, &key, Crop::Top, 1.0, RADIUS));
        if !drawn {
            ui.paragraph(cover.grow(-12.0), &game.name, &BODY, HAlign::Center, VAlign::Center, 0);
        }
    });
    let title =
        Rect::new(rect.x + CAPTION_SIDE, cover.bottom() + CAPTION_TOP, rect.w - 2.0 * CAPTION_SIDE, TITLE_HEIGHT);
    ui.paragraph(title, &game.name, &BODY, HAlign::Left, VAlign::Top, 2);
    if focused {
        theme::focus_ring(ui, rect, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_whole_tiles_fit_the_row() {
        let room = ui::DESIGN_WIDTH - 2.0 * LEFT;
        assert_eq!(((room + PITCH - TILE) / PITCH).floor() as usize, TILES_SHOWN);
    }

    #[test]
    fn the_gradient_is_green_at_the_top_left_and_black_towards_the_bottom_right() {
        let image = paint_gradient();
        assert_eq!(image.get_pixel(0, 0).0[..3], [0x12, 0x3A, 0x2C]);
        assert_eq!(image.get_pixel(479, 269).0[..3], [0, 0, 0]);
    }
}
