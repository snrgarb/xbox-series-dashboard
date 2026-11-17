//! The Status area as the Guide has it, stacked at the screen's top right: the batteries on a line of their own
//! over the clock, large (docs/spec/guide.md, section 2.3). Home's top bar has the same parts in a row.
use model::discord::CallMark;
use ui::{Battery, Color, Font, Rect, TextStyle, Ui};

use crate::theme::{glyph, ACCENT_LIGHT, BODY, TEXT_DIM};

/// What the Status area and the Notifications badge show, beyond the clock and whether a pad is connected. The
/// Dashboard is told (`Dashboard::set_status`); where each value comes from is the shell's business.
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    /// The first pad's battery, in place of what the pad itself says (`Dashboard::set_pad`). With none known either
    /// way a connected pad shows as one on a cable does: its glyph.
    pub pad_battery: Option<Battery>,
    /// The PC's own battery; none on a PC without one.
    pub pc_battery: Option<PcBattery>,
    /// Settings' "show battery percentages": the figure beside each battery.
    pub show_percent: bool,
    /// How many notifications there are: the badge on the Notifications utility.
    pub notifications: u32,
    /// Whether Discord is connected: its tab stands between the landing tab and Profile & system.
    pub discord: bool,
    /// In a Discord Call, its mark, left of the batteries. The Dashboard sets it from what Discord says.
    pub discord_call: Option<CallMark>,
}

impl Default for Status {
    fn default() -> Self {
        let (pad_battery, pc_battery, discord_call) = (None, None, None);
        Self { pad_battery, pc_battery, show_percent: true, notifications: 0, discord: false, discord_call }
    }
}

/// The PC's own battery, on a laptop or a handheld.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PcBattery {
    /// 0 to 100, if Windows knows.
    pub percent: Option<u8>,
    pub charging: bool,
}

impl PcBattery {
    /// The icon font's upright battery: eleven glyphs from empty to full, another eleven with the bolt in them
    /// while charging, and one for a charge that is not known.
    fn glyph(self) -> char {
        let Some(percent) = self.percent else { return '\u{F608}' };
        let tenths = (percent.min(100) as u32 + 5) / 10;
        char::from_u32(if self.charging { 0xF5FD } else { 0xF5F2 } + tenths).unwrap_or('\u{F608}')
    }
}

/// The area's right edge is 150 from the screen's, its top 86 down.
const RIGHT_MARGIN: f32 = 150.0;
const TOP: f32 = 86.0;
/// The batteries' line: 28 high, a further 4 in from the right, its parts 12 apart.
const LINE: f32 = 28.0;
const LINE_INSET: f32 = 4.0;
const GAP: f32 = 12.0;
const CLOCK: TextStyle = TextStyle::new(Font::Display, 56.0, 600);

/// Draws the area against the screen's right edge. `pad` is whether a pad is connected.
pub fn draw(ui: &mut Ui, status: &Status, clock: &str, pad: bool) {
    let right = ui.screen().right() - RIGHT_MARGIN;
    let middle = TOP + LINE / 2.0;
    // Right to left: the pad, then the PC's battery.
    let mut x = right - LINE_INSET;
    if pad {
        x = pad_battery(ui, x, middle, status) - GAP;
    }
    if let Some(battery) = status.pc_battery {
        ui.icon(x - 13.0, middle, battery.glyph(), 26.0, Color::WHITE);
        x -= 26.0;
        if let Some(percent) = battery.percent.filter(|_| status.show_percent) {
            // Segoe UI's ink sits 2 under its line's middle: raised onto the battery's.
            let text = ui.label_right(x - 4.0, middle - 2.0, &format!("{percent}%"), &BODY);
            x -= 4.0 + text.width;
        }
        x -= GAP;
    }
    // In a Discord Call its mark, 12 left of the batteries.
    if let Some(mark) = status.discord_call {
        call_mark(ui, x, middle, mark);
    }
    // The clock, 10 under the line.
    let width = ui.measure(&CLOCK, clock).width;
    ui.label(right - width, TOP + LINE + 10.0, clock, &CLOCK);
}

/// The Discord mark of a Call, 30 by 24, ending at `right` with its middle at `middle`: Discord's mark, 24 by 18, in
/// grey, or in the accent while the user speaks; at its foot right a badge 14 across while muted or deafened (the
/// microphone or the headset, struck through), deafened over muted. Returns where it starts.
pub(crate) fn call_mark(ui: &mut Ui, right: f32, middle: f32, mark: CallMark) -> f32 {
    let area = Rect::new(right - 30.0, middle - 12.0, 30.0, 24.0);
    let colour = if mark.speaking { ACCENT_LIGHT } else { TEXT_DIM };
    // The stand-in draws as high as three quarters of its width, in the middle: 18 high, 3 down.
    super::marks::discord_mark(ui, Rect::new(area.x, area.y, 24.0, 24.0), colour);
    let badge = match (mark.deafened, mark.muted) {
        (true, _) => Some('\u{E7F6}'),
        (false, true) => Some('\u{E720}'),
        (false, false) => None,
    };
    if let Some(glyph) = badge {
        let disc = Rect::new(area.right() - 14.0, area.bottom() - 14.0, 14.0, 14.0);
        ui.fill(disc, Color::rgb(0x1A1A1A), 7.0);
        ui.ring(disc, Color::WHITE, 7.0, 1.0);
        let (cx, cy) = disc.center();
        ui.icon(cx, cy, glyph, 8.0, Color::WHITE);
        // The stroke from (3.5, 3.5) to (10.5, 10.5), 1.25 wide.
        let length = 7.0 * std::f32::consts::SQRT_2;
        let line = Rect::new(cx - length / 2.0, cy - 0.625, length, 1.25);
        ui.fill_turned(line, Color::WHITE, 0.6, std::f32::consts::FRAC_PI_4);
    }
    area.x
}

/// The pad's battery ending at `right`: its charge as a cell on its side, with the figure beside it where the pad
/// says a percentage (and Settings wants figures); at its level and with no figure where it says only a level; and
/// the pad's glyph for one on a cable or one that says nothing (docs/spec/home.md 2.4). Returns where it starts.
pub(crate) fn pad_battery(ui: &mut Ui, right: f32, middle: f32, status: &Status) -> f32 {
    let Some((battery, fill)) = status.pad_battery.and_then(|battery| Some((battery, battery.fill()?))) else {
        ui.icon(right - 12.0, middle, glyph::GAMEPAD, 24.0, Color::WHITE);
        return right - 24.0;
    };
    // The cell: a 30 x 16 outline, a bar in it as wide as the charge, and a nub at its right.
    ui.fill(Rect::new(right - 3.0, middle - 3.5, 3.0, 7.0), Color::WHITE, 1.5);
    let cell = Rect::new(right - 33.0, middle - 8.0, 30.0, 16.0);
    ui.ring(cell, Color::WHITE, 3.0, 2.0);
    let inside = cell.grow(-4.0);
    ui.fill(Rect::new(inside.x, inside.y, inside.w * fill, inside.h), Color::WHITE, 1.0);
    let Battery::Percent(percent) = battery else { return cell.x };
    if !status.show_percent {
        return cell.x;
    }
    let text = ui.label_right(cell.x - 8.0, middle - 2.0, &format!("{percent}%"), &BODY);
    cell.x - 8.0 - text.width
}
