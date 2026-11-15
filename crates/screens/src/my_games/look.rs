//! The page's colours, type and its two looks of focus (docs/spec/my-games.md, 2.1 and 12.3): a ring round what
//! holds a picture or a row, and the accent filling what is a button. Both carry the glow. Nothing on this page is
//! scaled: every measure is the Bundle's own.
use ui::{Color, Font, Rect, TextStyle, Ui};

use crate::theme::{ACCENT, ACCENT_LIGHT};

/// The page's fill (`PageBrush`).
pub const PAGE: Color = Color::rgb(0x1F1F1F);
/// The divider beside the nav, and the line across it (`DividerBrush`).
pub const DIVIDER: Color = Color::rgb(0x303030);
/// The nav item whose content is up (`SelectedBrush`).
pub const SELECTED: Color = Color::rgb(0x262626);
/// A Play history row's back (`RowBrush`).
pub const ROW: Color = Color::rgb(0x292929);
/// Dim text: opaque here, where Home's is 70% white (`DimBrush`).
pub const DIM: Color = Color::rgb(0x8F8F8F);
/// A flat button at rest.
pub const BUTTON: Color = Color::rgb(0x333333);
/// A box at rest, and with focus.
pub const BOX: Color = Color::rgb(0x2D2D2D);
pub const BOX_FOCUSED: Color = Color::rgb(0x4A4A4A);

/// `Text`: Segoe UI 26.
pub const TEXT: TextStyle = TextStyle::new(Font::Ui, 26.0, 400);
pub const TEXT_DIM: TextStyle = TEXT.color(DIM);
pub const TEXT_SEMIBOLD: TextStyle = TEXT.weight(600);
/// The page's header: Bahnschrift SemiLight 42.
pub const HEADER: TextStyle = TextStyle::new(Font::Display, 42.0, 350);
/// `NavLabel`: Bahnschrift SemiLight 26.
pub const NAV_LABEL: TextStyle = TextStyle::new(Font::Display, 26.0, 350);
/// A Play history section's header: Bahnschrift 30.
pub const SECTION: TextStyle = TextStyle::new(Font::Display, 30.0, 400);
/// The line an empty grid shows: Segoe UI 32.
pub const NO_GAMES: TextStyle = TextStyle::new(Font::Ui, 32.0, 400);

/// A line of Segoe UI 26 is this tall.
pub const LINE: f32 = 26.0 * 1.33;

/// The icons this page has that the theme's list lacks.
pub mod glyph {
    pub const APPS: char = '\u{E71D}';
    pub const MANAGE: char = '\u{E821}';
    /// A filled triangle: the running game's mark.
    pub const PLAY: char = '\u{F5B0}';
    /// Full library's: the Sort box's chevron, Filter's funnel, and the tick on the sort the grid is in.
    pub const CHEVRON: char = '\u{E70D}';
    pub const FILTER: char = '\u{E71C}';
    pub const TICK: char = '\u{E73E}';
}

/// `XboxFocusGlow` round a band whose middle line is `line` and whose corners there are `radius`: 11 wide, half
/// opaque, blurred by 17 (docs/spec/home.md, 4.3).
fn glow(ui: &mut Ui, line: Rect, radius: f32) {
    ui.glow(line.grow(5.5), ACCENT_LIGHT.fade(0.5), radius + 5.5, 11.0, 17.0);
}

/// A control that fills with the accent when it has focus (a nav item, SEE FULL LIBRARY): the fill, and the glow
/// round it, whose middle line is 2 outside the fill's edge and rounded by 12 though the slab is square.
pub fn slab(ui: &mut Ui, rect: Rect, radius: f32) {
    glow(ui, rect.grow(2.0), 12.0);
    ui.fill(rect, ACCENT, radius);
}

/// A focus ring 4 wide whose outer edge is `outer`, its middle line rounded by 8, with the glow: a Play history
/// row's and a box's.
pub fn ring(ui: &mut Ui, outer: Rect) {
    glow(ui, outer.grow(-2.0), 8.0);
    ui.ring(outer, ACCENT_LIGHT, 10.0, 4.0);
}

/// A focus ring 4 wide on a box's own edge, inside it, with the glow on its middle line: the name box's, which
/// stays dark so that the name stays legible, and whose ring would be painted over by the grid if it stood out.
pub fn ring_inside(ui: &mut Ui, rect: Rect) {
    glow(ui, rect.grow(-2.0), 12.0);
    ui.ring(rect, ACCENT_LIGHT, 4.0, 4.0);
}
