//! The Utilities: the row of squares at the landing tab's foot (docs/spec/guide.md, section 2.8). The other tabs
//! have none.
use ui::{Color, Rect, Ui};

use super::focus_ring;
use crate::theme::{glyph, ACCENT_LIGHT, BODY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Utility {
    Notifications,
    Audio,
    Search,
    /// Opens Manage Queue, as Home's Downloads tile does (#193).
    Downloads,
}

impl Utility {
    /// The landing tab's, left to right.
    pub const LANDING: [Utility; 4] = [Utility::Notifications, Utility::Audio, Utility::Search, Utility::Downloads];

    fn glyph(self) -> char {
        match self {
            Utility::Notifications => glyph::NOTIFICATIONS,
            Utility::Audio => glyph::VOLUME,
            Utility::Search => glyph::SEARCH,
            Utility::Downloads => glyph::DOWNLOAD,
        }
    }
}

/// Each square is 87 across, 18 from the next, the first at x 93; y 942..1029. The row stands from the left as the
/// Bundle's does, so a fourth square ends at x 495, well inside the panel.
const AT: (f32, f32) = (93.0, 942.0);
const SIDE: f32 = 87.0;
const GAP: f32 = 18.0;

/// Where the square `place` along the row is.
pub fn rect(place: usize) -> Rect {
    Rect::new(AT.0 + place as f32 * (SIDE + GAP), AT.1, SIDE, SIDE)
}

/// The counts the squares' badges show: what Notifications lists, and how many games are in the Queue.
#[derive(Debug, Clone, Copy, Default)]
pub struct Counts {
    pub notifications: u32,
    pub queued: u32,
}

impl Counts {
    fn of(self, utility: Utility) -> u32 {
        match utility {
            Utility::Notifications => self.notifications,
            Utility::Downloads => self.queued,
            Utility::Audio | Utility::Search => 0,
        }
    }
}

/// Draws the row, each square with its badge where its count is more than 0.
pub fn draw(ui: &mut Ui, utilities: &[Utility], focused: Option<usize>, counts: Counts) {
    // The focused square last, so its neighbour's back does not cover its glow.
    let unfocused = (0..utilities.len()).filter(|&place| Some(place) != focused);
    for place in unfocused.chain(focused.filter(|&place| place < utilities.len())) {
        let (utility, square) = (utilities[place], rect(place));
        let has_focus = focused == Some(place);
        ui.fill(square, Color::argb(if has_focus { 0x26FFFFFF } else { 0x73000000 }), 4.0);
        if has_focus {
            focus_ring(ui, square);
        }
        let (cx, cy) = square.center();
        ui.icon(cx, cy, utility.glyph(), 33.0, Color::WHITE);
        let count = counts.of(utility);
        if count > 0 {
            badge(ui, square, count);
        }
    }
}

/// The count in a green pill at the square's top right, 6 in from both edges: 24 high, as wide as its figure and
/// 6 either side, and never narrower than it is high. The figure is the body type's, taller than the pill, and is
/// cut to it, as WPF cuts a child to a box of fixed height.
fn badge(ui: &mut Ui, square: Rect, count: u32) {
    let text = count.to_string();
    let width = (ui.measure(&BODY, &text).width + 12.0).max(24.0);
    let pill = Rect::new(square.right() - 6.0 - width, square.y + 6.0, width, 24.0);
    ui.fill(pill, ACCENT_LIGHT, 12.0);
    let (cx, cy) = pill.center();
    ui.with_clip(pill, |ui| ui.label_centered(cx, cy, &text, &BODY));
}
