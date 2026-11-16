//! The Guide's panel and the tab strip along its top (docs/spec/guide.md, sections 1.2 and 2.2).
use ui::{Color, Gradient, Rect, Ui};

use super::marks;
use super::Tab;
use crate::profile;
use crate::theme::ACCENT_LIGHT;

/// The panel: x 78..614, y 33..1045.
pub const PANEL: Rect = Rect::new(78.0, 33.0, 536.0, 1012.0);
const RADIUS: f32 = 12.0;
/// The tab strip is the panel's top 102.
pub const STRIP: f32 = 102.0;
/// A tab's box, and where the first starts.
const TAB: (f32, f32) = (84.0, STRIP);
const FIRST_TAB: f32 = 99.0;

/// The panel's colours down its height: grey through the top half, then turning a dark green.
const PANEL_STOPS: [(f32, u32); 5] =
    [(0.0, 0x1F1F1F), (0.5, 0x1E201C), (0.71, 0x172116), (0.86, 0x152611), (1.0, 0x132A0E)];
/// The strip's, down its own height: the panel's grey, darkening to a band along its foot.
const STRIP_STOPS: [(f32, u32); 6] =
    [(0.0, 0x1F1F1F), (0.52, 0x1F1F1F), (0.6, 0x1B1B1B), (0.74, 0x191919), (0.95, 0x171717), (1.0, 0x171717)];

/// The colour `share` of the way down a gradient of `stops`.
fn at(stops: &[(f32, u32)], share: f32) -> Color {
    let after = stops.iter().position(|&(offset, _)| offset >= share).unwrap_or(stops.len() - 1).max(1);
    let ((from, a), (to, b)) = (stops[after - 1], stops[after]);
    let (a, b) = (Color::rgb(a), Color::rgb(b));
    let t = ((share - from) / (to - from).max(f32::EPSILON)).clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color { r: mix(a.r, b.r), g: mix(a.g, b.g), b: mix(a.b, b.b), a: 1.0 }
}

/// A gradient of many stops over `over`, drawn between `top` and `bottom` of it: a two-colour quad for each
/// stretch between stops. The quads meet on whole pixels, so no seam shows between them.
fn stretch(ui: &mut Ui, over: Rect, stops: &[(f32, u32)], top: f32, bottom: f32) {
    let px = ui.px();
    let mut cuts = vec![top];
    cuts.extend(stops.iter().map(|&(offset, _)| over.y + offset * over.h).filter(|&y| y > top && y < bottom));
    cuts.push(bottom);
    cuts.iter_mut().for_each(|y| *y = (*y * px).round() / px);
    let colour = |y: f32| at(stops, (y - over.y) / over.h);
    for pair in cuts.windows(2) {
        let part = Rect::new(over.x, pair[0], over.w, pair[1] - pair[0]);
        ui.gradient(part, colour(pair[0]), colour(pair[1]), Gradient::Vertical, 0.0);
    }
}

/// The panel's background and the strip's: opaque, with neither border nor shadow.
pub fn background(ui: &mut Ui) {
    // The whole panel with its round corners, in its first and last colours: over the corners' 12 the colours
    // hardly change, so this is what shows there. Everything between is drawn over it, stop by stop.
    let (first, last) = (at(&PANEL_STOPS, 0.0), at(&PANEL_STOPS, 1.0));
    ui.gradient(PANEL, first, last, Gradient::Vertical, RADIUS);
    let strip = Rect::new(PANEL.x, PANEL.y, PANEL.w, STRIP);
    stretch(ui, strip, &STRIP_STOPS, PANEL.y + RADIUS, strip.bottom());
    stretch(ui, PANEL, &PANEL_STOPS, strip.bottom(), PANEL.bottom() - RADIUS);
}

/// The tabs' icons, and the green bar under the one showing. Tabs take no focus: the bumpers change them, and the
/// bar moves at once.
pub fn tabs(ui: &mut Ui, tabs: &[Tab], showing: Tab, picture: &model::Picture) {
    const UNSELECTED: Color = Color::rgb(0x8C8C8C);
    for (i, &tab) in tabs.iter().enumerate() {
        let bounds = Rect::new(FIRST_TAB + i as f32 * TAB.0, PANEL.y, TAB.0, TAB.1);
        let (cx, cy) = bounds.center();
        let selected = tab == showing;
        let colour = if selected { Color::WHITE } else { UNSELECTED };
        let square = |side: f32| Rect::new(cx - side / 2.0, cy - side / 2.0, side, side);
        match tab {
            Tab::Landing => marks::xbox_mark(ui, square(33.0), colour),
            Tab::Discord => marks::discord_mark(ui, square(36.0), colour),
            Tab::ProfileAndSystem => {
                // The disc shows where there is no picture; the picture is the same selected or not.
                ui.fill(square(54.0), Color::rgb(0x4A4A4A), 999.0);
                profile::picture(ui, square(54.0), picture);
            }
        }
        if selected {
            // 6 high at the tab's foot, hanging 4 below it.
            ui.fill(Rect::new(bounds.x, bounds.bottom() - 2.0, bounds.w, 6.0), ACCENT_LIGHT, 2.0);
        }
    }
}
