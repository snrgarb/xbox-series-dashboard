//! An App's tile (docs/spec/apps.md, 3; the Bundle's `AppTiles.xaml`), the one face of every App: My games & apps'
//! Apps, and Search's Apps row when it comes.
//!
//! White, the App's icon inset 18 and as large as fits, over a soft shadow of the icon's own shape (blur 18, 3 down
//! and right, 30% black). A Microsoft Store app's tile art instead fills the tile edge to edge over the accent, with
//! no shadow. With no icon, or one that can't be read, the generic app glyph as large: dark grey on white, white on
//! the accent. The face is cut to round corners of 6. While the tile is focused, the ring (4 wide, 8 out, outer
//! corners 14, with the glow) and a near-black band at its foot holding the name, which fades in and out with focus
//! as Home's game tiles' title does, Reduce motion or not.
use std::hash::Hash;

use model::App;
use ui::{ArtKey, Color, Ease, Font, HAlign, Kind, Rect, TextStyle, Ui, VAlign};

use crate::theme::{self, ACCENT};

const RADIUS: f32 = 6.0;
/// The icon stands this far in from the tile's edges, but a Store app's.
const INSET: f32 = 18.0;
/// The icon's shadow: `ImageShadow BlurRadius="18" ShadowDepth="3" ShadowOpacity="0.3"`, cast down and right as
/// WPF's default direction (315 degrees) casts it.
const SHADOW_BLUR: f32 = 18.0;
const SHADOW_DEPTH: f32 = 3.0;
const SHADOW_OPACITY: f32 = 0.3;
/// The glyph of an App with no icon (`&#xECAA;`), and its colour on white.
const NO_ICON: char = '\u{ECAA}';
const NO_ICON_GREY: Color = Color::rgb(0x333333);
/// The title band: `#E6000000`, its name 8 in, on at most two lines.
const BAND: Color = Color::argb(0xE6000000);
const BAND_MARGIN: f32 = 8.0;
const NAME: TextStyle = TextStyle::new(Font::Ui, 26.0, 400);
/// The ring's outer corners.
const RING: f32 = 14.0;

/// The App's picture, decoded at most 480 wide, as the Bundle decodes it: twice the widest a tile gets.
pub fn icon_art(app: &App) -> Option<ArtKey> {
    app.icon.as_ref().map(|path| ArtKey { path: path.clone(), kind: Kind::Thumb(480) })
}

/// How much of the title band shows: in over 150 ms after 50, out over 100, both linear.
fn band(ui: &mut Ui, id: impl Hash, focused: bool) -> f32 {
    // The first quarter of the value's way is the 50 ms wait; the way out crosses it unseen, after its 100 ms.
    let (to, seconds) = if focused { (1.0, 0.2) } else { (0.0, 0.1 / 0.75) };
    ((ui.tween((id, "app band"), to, seconds, Ease::Linear) - 0.25) / 0.75).max(0.0)
}

/// Draws the App's tile in `rect`, a square.
pub fn draw(ui: &mut Ui, rect: Rect, id: impl Hash + Copy, app: &App, focused: bool) {
    let band = band(ui, id, focused);
    let store = app.is_store();
    ui.fill(rect, if store { ACCENT } else { Color::WHITE }, RADIUS);
    let art = icon_art(app);
    let drawn = match &art {
        // Edge to edge, cut to the tile's corners: a Store app's art is the size of its tile.
        Some(art) if store => ui.art_fitted(rect, art, 1.0, 1.0).is_some(),
        Some(art) => {
            let inset = rect.grow(-INSET);
            // The shadow first, under the icon: its own shape, in black, blurred and faded as one picture.
            let (dx, dy) =
                (SHADOW_DEPTH * std::f32::consts::FRAC_1_SQRT_2, SHADOW_DEPTH * std::f32::consts::FRAC_1_SQRT_2);
            let cast = Rect::new(inset.x + dx, inset.y + dy, inset.w, inset.h);
            let ready = ui.group(cast.grow(SHADOW_BLUR), SHADOW_BLUR, SHADOW_OPACITY, |ui| {
                ui.art_fitted(cast, art, 1.0, 0.0).is_some()
            });
            ready && ui.art_fitted(inset, art, 1.0, 1.0).is_some()
        }
        None => false,
    };
    if !drawn {
        let (cx, cy) = rect.center();
        let colour = if store { Color::WHITE } else { NO_ICON_GREY };
        ui.icon(cx, cy, NO_ICON, rect.w - 2.0 * INSET, colour);
    }
    if band > 0.0 {
        let width = rect.w - 2.0 * BAND_MARGIN;
        let text = ui.measure_wrapped(&NAME, &app.name, width, 2);
        let height = text.height + 2.0 * BAND_MARGIN;
        let under = Rect::new(rect.x, rect.bottom() - height, rect.w, height);
        ui.with_alpha(band, |ui| {
            // Square at its top; the tile's corners at its foot.
            ui.with_clip(under, |ui| ui.fill(Rect { y: under.y - RADIUS, h: under.h + RADIUS, ..under }, BAND, RADIUS));
            let lines = Rect::new(rect.x + BAND_MARGIN, under.y + BAND_MARGIN, width, text.height);
            ui.paragraph(lines, &app.name, &NAME, HAlign::Left, VAlign::Top, 2);
        });
    }
    if focused {
        theme::focus_ring_cornered(ui, rect, RING);
    }
}
