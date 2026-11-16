//! The theme's colours, type and measures, as the Bundle's XAML has them (docs/spec/).
use ui::{Color, Font, Gradient, Rect, TextStyle, Ui};

/// The Recent row, the wide tiles and the Groups are authored on a panel 1526 wide and shown 1700 wide: everything
/// in them (sizes, radii, the focus ring, type) is multiplied by this. The top bar is not.
pub const ROW_SCALE: f32 = 1700.0 / 1526.0;
/// The rows' left edge, and the top bar's.
pub const MARGIN: f32 = 105.0;
/// The rows' right edge.
pub const ROW_RIGHT: f32 = 1805.0;

/// The focus ring and the light accent.
pub const ACCENT_LIGHT: Color = Color::rgb(0x2CA243);
/// The brand green: the profile picture's disc.
pub const ACCENT: Color = Color::rgb(0x107C10);
pub const TILE: Color = Color::rgb(0x1A1A1A);
pub const BAND: Color = Color::argb(0xE6000000);
pub const TEXT_DIM: Color = Color::argb(0xB3FFFFFF);

/// Body text: Segoe UI 26, the smallest the Xbox accessibility guidelines allow at ten feet.
pub const BODY: TextStyle = TextStyle::new(Font::Ui, 26.0, 400);
pub const BODY_SEMIBOLD: TextStyle = TextStyle::new(Font::Ui, 26.0, 600);
pub const HEADING: TextStyle = TextStyle::new(Font::Display, 32.0, 400);
pub const CLOCK: TextStyle = TextStyle::new(Font::Display, 22.0, 600);

/// Segoe Fluent Icons' glyphs, and one that is not: the trophy, which the icon font lacks and Segoe UI Symbol has.
pub mod glyph {
    pub const CONTACT: char = '\u{E77B}';
    pub const LIBRARY: char = '\u{E8F1}';
    pub const SEARCH: char = '\u{E721}';
    pub const SETTINGS: char = '\u{E713}';
    /// The gear filled in: the Settings button with focus.
    pub const SETTINGS_SOLID: char = '\u{F8B0}';
    pub const NOTIFICATIONS: char = '\u{E91C}';
    /// A pad, connected.
    pub const GAMEPAD: char = '\u{E7FC}';
    pub const DOWNLOAD: char = '\u{E896}';
    /// The play triangle: a game ready to play.
    pub const PLAY: char = '\u{E768}';
    /// The bin: a game uninstalled.
    pub const DELETE: char = '\u{E74D}';
    pub const HISTORY: char = '\u{E81C}';
    /// Not in the icon font: `Ui::icon` draws it from Segoe UI Symbol, as the Bundle's body font does.
    pub const TROPHY: char = '\u{1F3C6}';
    pub const CHAT: char = '\u{E8BD}';
    pub const HOME: char = '\u{E80F}';
    /// The speaker: the Audio utility.
    pub const VOLUME: char = '\u{E767}';
    pub const SWITCH_PROFILE: char = '\u{E748}';
    pub const POWER: char = '\u{E7E8}';
}

/// The focus ring round a rectangle, with its glow: a 4-wide stroke 4 clear of the edge, its outer corners 16,
/// and a soft band of the same green under it. `scale` is 1 on the top bar and [`ROW_SCALE`] in the rows.
pub fn focus_ring(ui: &mut Ui, rect: Rect, scale: f32) {
    // The theme's glow is a band 11 wide centred on the ring's centre line, at half opacity, blurred by 17.
    ui.glow(rect.grow(11.5 * scale), ACCENT_LIGHT.fade(0.5), 17.5 * scale, 11.0 * scale, 17.0 * scale);
    ui.ring(rect.grow(8.0 * scale), ACCENT_LIGHT, 16.0 * scale, 4.0 * scale);
}

/// The focus ring of a dialog's option, with its glow: the same 4-wide stroke 4 clear of the box, its outer corners
/// `outer` (WPF's `CornerRadius` is the stroke's centre line: 2 less).
pub fn focus_ring_cornered(ui: &mut Ui, rect: Rect, outer: f32) {
    ui.glow(rect.grow(11.5), ACCENT_LIGHT.fade(0.5), outer + 1.5, 11.0, 17.0);
    ui.ring(rect.grow(8.0), ACCENT_LIGHT, outer, 4.0);
}

/// The hero scrim, over a game's art on the whole screen: dark at the top for the top bar, clear through the
/// middle, darker at the foot for the rows (docs/spec/home.md section 8.4).
pub fn hero_scrim(ui: &mut Ui) {
    let screen = ui.screen();
    let stop = |share: f32| screen.h * share;
    let top = Rect::new(screen.x, 0.0, screen.w, stop(0.22));
    ui.gradient(top, Color::argb(0x99000000), Color::CLEAR, Gradient::Vertical, 0.0);
    let foot = Rect::new(screen.x, stop(0.4), screen.w, stop(0.6));
    ui.gradient(foot, Color::CLEAR, Color::argb(0xD9000000), Gradient::Vertical, 0.0);
}

/// The focus ring round a circle: 3 wide, `gap` clear of it.
pub fn focus_ring_round(ui: &mut Ui, rect: Rect, gap: f32) {
    let ring = rect.grow(gap + 3.0);
    ui.glow(ring.grow(4.0), ACCENT_LIGHT.fade(0.5), 999.0, 11.0, 17.0);
    ui.ring(ring, ACCENT_LIGHT, 999.0, 3.0);
}
