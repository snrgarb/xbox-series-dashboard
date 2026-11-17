//! The marks the Bundle draws from vector paths that are other companies' trademarks: the Xbox mark, Discord's, the
//! stores'. They are not this project's to ship (docs/spec/guide.md, section 10): a store's is the project's own
//! drawing (`crate::drawn`), as are the Guide button's and chat's in the Xbox mark's and Discord's places.
use ui::{Color, Rect, Ui};

use crate::drawn::{self, Drawing};

/// The Guide button's mark, filling `rect`, where the Bundle draws the Xbox mark: the landing tab's icon, the Guide
/// button's hint and the Toasts' prompt. The project's own drawing of a home button (docs/spec/guide.md, 10.1).
pub fn guide_button(ui: &mut Ui, rect: Rect, color: Color) {
    drawn::draw(ui, rect, Drawing::GuideButton, color);
}

/// Discord's mark, filling `rect`: the Discord tab's icon, the account's, a Call's. The project's own drawing of
/// chat, never Discord's logo (docs/spec/guide.md, 10.1).
pub fn discord_mark(ui: &mut Ui, rect: Rect, color: Color) {
    drawn::draw(ui, rect, Drawing::Chat, color);
}

/// The store's badge on a game's cover in the list: its mark 18 across, 6 in from the cover's bottom left, on a
/// soft shadow, as Home's tiles carry it (docs/spec/home.md 3.3.1).
pub fn store_badge(ui: &mut Ui, cover: Rect, game: &model::Game) {
    let mark = Rect::new(cover.x + 6.0, cover.bottom() - 24.0, 18.0, 18.0);
    let shadow = mark.grow(2.7);
    ui.glow(shadow, Color::BLACK.fade(0.8), 999.0, shadow.w / 2.0, 5.4);
    crate::home::mark_drawn(ui, mark, crate::home::game_mark(game));
}
