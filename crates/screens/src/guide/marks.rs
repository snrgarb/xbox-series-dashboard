//! The marks the Bundle draws from vector paths that are other companies' trademarks: the Xbox sphere, Discord's
//! and the stores', drawn as the Bundle draws them (`crate::drawn`, NOTICE.md; docs/spec/guide.md, section 10).
use ui::{Color, Rect, Ui};

use crate::drawn::{self, Drawing};

/// The Xbox sphere, filling `rect`, where the Bundle draws it for the Guide button: the landing tab's icon, the
/// Guide button's hint and the Toasts' prompt and circle (docs/spec/guide.md, 10.1).
pub fn xbox(ui: &mut Ui, rect: Rect, color: Color) {
    drawn::draw(ui, rect, Drawing::Xbox, color);
}

/// Discord's mark, filling `rect` across (it is 24 wide and 18.3 high, on the square's middle): the Discord tile and
/// tab, the account's, a server's or a person's with no picture, a Call's (docs/spec/guide.md, 10.1).
pub fn discord_mark(ui: &mut Ui, rect: Rect, color: Color) {
    drawn::draw(ui, rect, Drawing::Discord, color);
}

/// The store's badge on a game's cover in the list: its mark 18 across, 6 in from the cover's bottom left, on a
/// soft shadow, as Home's tiles carry it (docs/spec/home.md 3.3.1).
pub fn store_badge(ui: &mut Ui, cover: Rect, game: &model::Game) {
    let mark = Rect::new(cover.x + 6.0, cover.bottom() - 24.0, 18.0, 18.0);
    let shadow = mark.grow(2.7);
    ui.glow(shadow, Color::BLACK.fade(0.8), 999.0, shadow.w / 2.0, 5.4);
    crate::home::mark_drawn(ui, mark, crate::home::game_mark(game));
}
