//! The hints beside the panel: which button does what where focus is (docs/spec/guide.md, section 3.6). There are
//! no A or B hints in the Guide.
use ui::{Color, Rect, Ui};

use super::marks;
use crate::theme::BODY;

/// The column starts here from the screen's left; each row is 45 high.
const AT: (f32, f32) = (675.0, 168.0);
const ROW: f32 = 45.0;
const GLYPH: f32 = 27.0;
/// The three lines in the Menu button's ring.
const MENU: char = '\u{E700}';

/// Draws the hints: the Guide button's always, and the Menu button's while focus is on something with a menu.
pub fn draw(ui: &mut Ui, has_menu: bool) {
    let mut rows = vec![(false, "Hold for power options")];
    if has_menu {
        rows.push((true, "More options"));
    }
    for (i, (menu, text)) in rows.into_iter().enumerate() {
        let middle = AT.1 + i as f32 * ROW + ROW / 2.0;
        let glyph = Rect::new(AT.0, middle - GLYPH / 2.0, GLYPH, GLYPH);
        if menu {
            ui.ring(glyph, Color::WHITE, 999.0, 2.0);
            ui.icon(glyph.center().0, middle, MENU, 12.0, Color::WHITE);
        } else {
            marks::guide_button(ui, glyph, Color::WHITE);
        }
        ui.label_middle(glyph.right() + 14.0, middle, text, &BODY);
    }
}
