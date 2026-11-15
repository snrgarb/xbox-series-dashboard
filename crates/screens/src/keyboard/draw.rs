//! The Console keyboard drawn: the keys with their faces and pad badges, the one focus ring, and the text either
//! side of the caret that Search's box and text entry's field both show (docs/spec/search.md, section 7; the
//! Bundle's `ConsoleKeyboardControl.xaml`). Nothing here is scaled: every number is the Bundle's own.
use ui::{Color, Font, Rect, TextStyle, Ui};

use super::board::{Board, Key, Kind, KEYS};
use crate::theme::{ACCENT_LIGHT, BODY};

/// The board: 114, ten columns of 70 and 114 across, five rows of 52.
pub const WIDTH: f32 = 928.0;
pub const HEIGHT: f32 = 260.0;
/// It is centred across, 110 off the screen's foot, on Search and in text entry alike.
const FROM_FOOT: f32 = 110.0;
const SIDE_COLUMN: f32 = 114.0;
const COLUMN: f32 = 70.0;
const ROW: f32 = 52.0;
/// A key fills its cells less 2 all round, so keys are 4 apart.
const KEY_MARGIN: f32 = 2.0;
const KEY_RADIUS: f32 = 2.0;

/// The letters' grey, the side keys' a shade lighter, the focused key's, and the lightest while a key is held down.
const KEY: Color = Color::rgb(0x2F3238);
const SIDE_KEY: Color = Color::rgb(0x3A3D44);
const FOCUSED_KEY: Color = Color::rgb(0x555963);
const HELD_KEY: Color = Color::rgb(0x7A7E88);
/// The pad badges' grey; X's blue and Y's yellow.
const BADGE: Color = Color::rgb(0xB4B7BD);
const BLUE: Color = Color::rgb(0x3F8EF0);
const YELLOW: Color = Color::rgb(0xFFB900);
const BADGE_STROKE: f32 = 1.5;
/// A round badge is 24 across.
const ROUND: f32 = 24.0;
/// A glyph and the faces' text are 26; the badge letters 14 (X, Y) and 10.
const GLYPH: f32 = 26.0;
const LETTER: TextStyle = TextStyle::new(Font::Ui, 14.0, 400);
const NAME: TextStyle = TextStyle::new(Font::Ui, 10.0, 400).color(BADGE);

/// Segoe Fluent Icons' glyphs on the keys.
mod glyph {
    pub const BACKSPACE: char = '\u{E750}';
    pub const CHEVRON_LEFT: char = '\u{E76B}';
    pub const CHEVRON_RIGHT: char = '\u{E76C}';
    /// The arrow up: Shift.
    pub const SHIFT: char = '\u{E74A}';
    /// The arrow right: Enter.
    pub const ENTER: char = '\u{E72A}';
    /// Menu's three lines, in Enter's badge.
    pub const MENU: char = '\u{E700}';
}

/// The caret: 2 by 30, with 1 clear either side of it.
const CARET: (f32, f32) = (2.0, 30.0);
const CARET_GAP: f32 = 1.0;
/// A secret's character: a dot 12 across, one every 18.
const DOT: f32 = 12.0;
const DOT_STEP: f32 = 18.0;

impl Key {
    /// The key's cells, from the board's top left corner.
    fn cells(self) -> Rect {
        // Where a column starts: the side column, then the ten letters' columns, then the other side column.
        let x = |column: u8| match column {
            0 => 0.0,
            12 => WIDTH,
            column => SIDE_COLUMN + COLUMN * (column - 1) as f32,
        };
        let (left, right) = (x(self.column), x(self.column + self.columns));
        Rect::new(left, ROW * self.row as f32, right - left, ROW * self.rows as f32)
    }
}

impl Board {
    /// Where the board stands on this screen: centred on the canvas, 110 off the screen's foot, whatever the
    /// screen's shape (x 496..1424, y 710..970 at 1080p).
    pub fn rect(ui: &Ui) -> Rect {
        let x = (ui::DESIGN_WIDTH - WIDTH) / 2.0;
        Rect::new(x, ui.screen().bottom() - FROM_FOOT - HEIGHT, WIDTH, HEIGHT)
    }

    /// The face of a key on this screen: its cells less 2 all round.
    pub fn key_rect(ui: &Ui, key: Key) -> Rect {
        let board = Self::rect(ui);
        key.cells().offset(board.x, board.y).grow(-KEY_MARGIN)
    }

    /// Draws the keys where the board stands, with nothing behind them: the page, or text entry's dim, shows
    /// between. `focused` says a key has focus: the ring is on it, and the keys pad buttons hold down are lit.
    pub fn draw(&self, ui: &mut Ui, focused: bool) {
        for (index, &key) in KEYS.iter().enumerate() {
            let face = Self::key_rect(ui, key);
            let colour = if focused && self.is_held(index) {
                HELD_KEY
            } else if focused && index == self.focused_index() {
                FOCUSED_KEY
            } else if key.kind == Kind::Shift && self.shifted() {
                // Shift lights while it is on (#54): the focused key's grey, with no ring.
                FOCUSED_KEY
            } else if matches!(key.kind, Kind::Character | Kind::Space) {
                KEY
            } else {
                SIDE_KEY
            };
            ui.fill(face, colour, KEY_RADIUS);
            self.face(ui, key, face);
        }
        if focused {
            // One ring for the board, over every key: 3 thick with its inner edge on the key's edge, and the
            // theme's glow at 1:1, its band centred 2 outside the key.
            let face = Self::key_rect(ui, self.focused());
            ui.glow(face.grow(7.5), ACCENT_LIGHT.fade(0.5), 17.5, 11.0, 17.0);
            ui.ring(face.grow(3.0), ACCENT_LIGHT, 5.5, 3.0);
        }
    }

    /// What a key shows: its character, or its glyph and the pad button that presses it.
    fn face(&self, ui: &mut Ui, key: Key, face: Rect) {
        let (cx, cy) = face.center();
        match key.kind {
            Kind::Character => {
                let mut buffer = [0u8; 4];
                let reads = self.reads(key).unwrap_or(' ').encode_utf8(&mut buffer);
                ui.label_centered(cx, cy, reads, &BODY);
            }
            // The text, then 12 on the LT badge: 28 by 22, rounder at the top as a trigger is.
            Kind::Symbols => {
                let text = if self.shows_symbols() { "abc" } else { "#+=" };
                let width = ui.measure(&BODY, text).width;
                let x = cx - (width + 12.0 + 28.0) / 2.0;
                ui.label_middle(x, cy, text, &BODY);
                let badge = Rect::new(x + width + 12.0, cy - 11.0, 28.0, 22.0);
                // Corner radii 9, 9, 3, 3: the top half of one ring and the foot of another.
                let half = Rect::new(badge.x - 1.0, badge.y - 1.0, badge.w + 2.0, badge.h / 2.0 + 1.0);
                ui.with_clip(half, |ui| ui.ring(badge, BADGE, 9.0, BADGE_STROKE));
                ui.with_clip(half.offset(0.0, half.h), |ui| ui.ring(badge, BADGE, 3.0, BADGE_STROKE));
                ui.label_centered(badge.center().0, cy, "LT", &NAME);
            }
            // The glyph, then 12 on X in its blue ring.
            Kind::Backspace => {
                let x = cx - (GLYPH + 12.0 + ROUND) / 2.0;
                ui.icon(x + GLYPH / 2.0, cy, glyph::BACKSPACE, GLYPH, Color::WHITE);
                round_badge(ui, x + GLYPH + 12.0 + ROUND / 2.0, cy, BLUE);
                ui.label_centered(x + GLYPH + 12.0 + ROUND / 2.0, cy, "X", &LETTER.color(BLUE));
            }
            // Stacked: the chevron, and 16 under it the bumper's pill, 32 by 18.
            Kind::Left | Kind::Right => {
                let top = cy - (GLYPH + 16.0 + 18.0) / 2.0;
                let (chevron, name) = match key.kind {
                    Kind::Left => (glyph::CHEVRON_LEFT, "LB"),
                    _ => (glyph::CHEVRON_RIGHT, "RB"),
                };
                ui.icon(cx, top + GLYPH / 2.0, chevron, GLYPH, Color::WHITE);
                let pill = Rect::new(cx - 16.0, top + GLYPH + 16.0, 32.0, 18.0);
                ui.ring(pill, BADGE, 5.0, BADGE_STROKE);
                ui.label_centered(cx, pill.center().1, name, &NAME);
            }
            // Stacked: the arrow, and 14 under it a round badge: LS, or Menu's three lines.
            Kind::Shift | Kind::Enter => {
                let top = cy - (GLYPH + 14.0 + ROUND) / 2.0;
                let arrow = if key.kind == Kind::Shift { glyph::SHIFT } else { glyph::ENTER };
                ui.icon(cx, top + GLYPH / 2.0, arrow, GLYPH, Color::WHITE);
                let badge = top + GLYPH + 14.0 + ROUND / 2.0;
                round_badge(ui, cx, badge, BADGE);
                if key.kind == Kind::Shift {
                    ui.label_centered(cx, badge, "LS", &NAME);
                } else {
                    ui.icon(cx, badge, glyph::MENU, 11.0, BADGE);
                }
            }
            // The mark, an open box 24 across and 6 deep stroked 2 wide, pushed 10 down; 12 on, Y in its yellow
            // ring. The stroke makes the mark 25 by 7 for the layout, its left stroke centred on its left edge.
            Kind::Space => {
                let x = cx - (25.0 + 12.0 + ROUND) / 2.0;
                let top = cy - (7.0 + 10.0) / 2.0 + 10.0;
                ui.fill(Rect::new(x - 1.0, top, 2.0, 7.0), Color::WHITE, 0.0);
                ui.fill(Rect::new(x + 23.0, top, 2.0, 7.0), Color::WHITE, 0.0);
                ui.fill(Rect::new(x - 1.0, top + 5.0, 26.0, 2.0), Color::WHITE, 0.0);
                round_badge(ui, x + 25.0 + 12.0 + ROUND / 2.0, cy, YELLOW);
                ui.label_centered(x + 25.0 + 12.0 + ROUND / 2.0, cy, "Y", &LETTER.color(YELLOW));
            }
        }
    }

    /// The text in `area`, either side of the caret: Segoe UI 26, white, left-aligned and vertically centred, cut
    /// to the area. A text longer than the area scrolls so the caret stays in view (#54): the caret then stands
    /// at the area's right edge. `masked` shows a dot for each character, never the text. `caret` is false where
    /// the owner hides it (Search with the keyboard away).
    pub fn draw_text(&self, ui: &mut Ui, area: Rect, masked: bool, caret: bool) {
        let (before, after) = self.around_caret();
        let middle = area.center().1;
        // What is before the caret, as wide as it is drawn. A space at its end is as wide as any other: a mark
        // after it, measured with it and taken off again, keeps the space from being trimmed.
        let before_width = if masked {
            self.cursor() as f32 * DOT_STEP
        } else {
            ui.measure(&BODY, &format!("{before}|")).width - ui.measure(&BODY, "|").width
        };
        let caret_width = CARET_GAP + CARET.0 + CARET_GAP;
        let scroll = (before_width + caret_width - area.w).max(0.0);
        let x = area.x - scroll;
        ui.with_clip(area, |ui| {
            if masked {
                let dot = |ui: &mut Ui, x: f32| {
                    let at = Rect::new(x + (DOT_STEP - DOT) / 2.0, middle - DOT / 2.0, DOT, DOT);
                    ui.fill(at, Color::WHITE, DOT / 2.0);
                };
                for place in 0..self.len() {
                    let after_caret = if place >= self.cursor() { caret_width } else { 0.0 };
                    dot(ui, x + place as f32 * DOT_STEP + after_caret);
                }
            } else {
                ui.label_middle(x, middle, &before, &BODY);
                ui.label_middle(x + before_width + caret_width, middle, &after, &BODY);
            }
            if caret {
                let at = Rect::new(x + before_width + CARET_GAP, middle - CARET.1 / 2.0, CARET.0, CARET.1);
                ui.fill(at, Color::WHITE, 0.0);
            }
        });
    }
}

/// A round pad badge's ring, 24 across, centred here.
fn round_badge(ui: &mut Ui, cx: f32, cy: f32, colour: Color) {
    let ring = Rect::new(cx - ROUND / 2.0, cy - ROUND / 2.0, ROUND, ROUND);
    ui.ring(ring, colour, ROUND / 2.0, BADGE_STROKE);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The keys' faces from the board's corner at 496, 710, as docs/spec/search.md 7.1 lists them.
    #[test]
    fn the_keys_stand_where_the_bundle_has_them() {
        let face = |kind: Kind, row: u8, column: u8| {
            let key = KEYS.iter().find(|key| key.kind == kind && key.row == row && key.column == column);
            let face = key.expect("the key").cells().offset(496.0, 710.0).grow(-KEY_MARGIN);
            (face.x, face.y, face.w, face.h)
        };
        assert_eq!(face(Kind::Symbols, 0, 0), (498.0, 712.0, 110.0, 48.0));
        assert_eq!(face(Kind::Character, 0, 1), (612.0, 712.0, 66.0, 48.0));
        assert_eq!(face(Kind::Character, 0, 10), (1242.0, 712.0, 66.0, 48.0));
        assert_eq!(face(Kind::Backspace, 0, 11), (1312.0, 712.0, 110.0, 48.0));
        assert_eq!(face(Kind::Left, 1, 0), (498.0, 764.0, 110.0, 100.0));
        assert_eq!(face(Kind::Character, 1, 1), (612.0, 764.0, 66.0, 48.0));
        assert_eq!(face(Kind::Right, 1, 11), (1312.0, 764.0, 110.0, 100.0));
        assert_eq!(face(Kind::Character, 2, 4), (822.0, 816.0, 66.0, 48.0));
        assert_eq!(face(Kind::Shift, 3, 0), (498.0, 868.0, 110.0, 100.0));
        assert_eq!(face(Kind::Character, 3, 10), (1242.0, 868.0, 66.0, 48.0));
        assert_eq!(face(Kind::Enter, 3, 11), (1312.0, 868.0, 110.0, 100.0));
        assert_eq!(face(Kind::Space, 4, 1), (612.0, 920.0, 696.0, 48.0));
    }
}
