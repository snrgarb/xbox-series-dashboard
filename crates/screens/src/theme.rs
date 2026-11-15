//! The theme's colours, type and measures, as the Bundle's XAML has them (docs/spec/).
use ui::{Color, Font, Rect, TextStyle, Ui};

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
pub const PANEL: Color = Color::rgb(0x1C1C1C);

/// Body text: Segoe UI 26, the smallest the Xbox accessibility guidelines allow at ten feet.
pub const BODY: TextStyle = TextStyle::new(Font::Ui, 26.0, 400);
pub const BODY_SEMIBOLD: TextStyle = TextStyle::new(Font::Ui, 26.0, 600);
pub const HEADING: TextStyle = TextStyle::new(Font::Display, 32.0, 400);
pub const CLOCK: TextStyle = TextStyle::new(Font::Display, 22.0, 600);

/// Segoe Fluent Icons' glyphs.
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
    pub const HISTORY: char = '\u{E81C}';
    pub const TROPHY: char = '\u{E7C1}';
    pub const CHAT: char = '\u{E8BD}';
}

/// The focus ring round a rectangle, with its glow: a 4-wide stroke 4 clear of the edge, its outer corners 16,
/// and a soft band of the same green under it. `scale` is 1 on the top bar and [`ROW_SCALE`] in the rows.
pub fn focus_ring(ui: &mut Ui, rect: Rect, scale: f32) {
    // The theme's glow is a band 11 wide centred on the ring's centre line, at half opacity, blurred by 17.
    ui.glow(rect.grow(11.5 * scale), ACCENT_LIGHT.fade(0.5), 17.5 * scale, 11.0 * scale, 17.0 * scale);
    ui.ring(rect.grow(8.0 * scale), ACCENT_LIGHT, 16.0 * scale, 4.0 * scale);
}

/// The focus ring round a circle: 3 wide, `gap` clear of it.
pub fn focus_ring_round(ui: &mut Ui, rect: Rect, gap: f32) {
    let ring = rect.grow(gap + 3.0);
    ui.glow(ring.grow(4.0), ACCENT_LIGHT.fade(0.5), 999.0, 11.0, 17.0);
    ui.ring(ring, ACCENT_LIGHT, 999.0, 3.0);
}
