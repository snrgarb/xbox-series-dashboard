//! The marks the Bundle draws from vector paths that are other companies' trademarks. The Xbox sphere and the
//! stores' are drawn as the Bundle draws them (`crate::drawn`, NOTICE.md); Discord's is still the project's own
//! drawing of chat (docs/spec/guide.md, section 10).
use ui::{Color, Rect, Ui};

use crate::drawn::{self, Drawing};

/// The Xbox sphere, filling `rect`, where the Bundle draws it for the Guide button: the landing tab's icon, the
/// Guide button's hint and the Toasts' prompt and circle (docs/spec/guide.md, 10.1).
pub fn xbox(ui: &mut Ui, rect: Rect, color: Color) {
    drawn::draw(ui, rect, Drawing::Xbox, color);
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
