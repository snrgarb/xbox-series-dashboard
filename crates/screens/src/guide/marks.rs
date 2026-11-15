//! The marks the Bundle draws from vector paths that are other companies' trademarks: the Xbox mark, Discord's, the
//! stores'. They are not this project's to ship (docs/spec/guide.md, section 10), so each is a plain shape here, and
//! each function is the one place to draw the real thing once it comes from somewhere it may.
use ui::{Color, Rect, Ui};

/// The Xbox mark, filling `rect`: the landing tab's icon and the Guide button's hint. A plain disc stands in.
pub fn xbox_mark(ui: &mut Ui, rect: Rect, color: Color) {
    ui.fill(rect, color, 999.0);
}

/// Discord's mark, as wide as `rect` and three quarters as high, in its middle: the Discord tab's icon. A plain
/// rounded bar stands in.
pub fn discord_mark(ui: &mut Ui, rect: Rect, color: Color) {
    let height = rect.w * 0.75;
    ui.fill(Rect::new(rect.x, rect.y + (rect.h - height) / 2.0, rect.w, height), color, height / 3.0);
}

/// The store's badge on a game's cover in the list: its mark 18 across, 6 in from the cover's bottom left, on a
/// soft shadow, as Home's tiles carry it. A plain white disc stands in for the mark.
pub fn store_badge(ui: &mut Ui, cover: Rect) {
    let mark = Rect::new(cover.x + 6.0, cover.bottom() - 24.0, 18.0, 18.0);
    let shadow = mark.grow(2.7);
    ui.glow(shadow, Color::BLACK.fade(0.8), 999.0, shadow.w / 2.0, 5.4);
    ui.fill(mark, Color::WHITE, 999.0);
}
