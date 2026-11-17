//! The top bar: the profile at the left, the three round buttons in the middle, the clock and Notifications at the
//! right. Its measures are the screen's own, not the rows': nothing here is multiplied by ROW_SCALE.
use ui::{Color, Ease, Rect, Ui};

use crate::theme::{self, glyph, BODY, MARGIN};
use crate::{achievement, guide, profile};

const MIDDLE: f32 = 114.0;
const BUTTON: f32 = 60.0;
const PICTURE: f32 = 56.0;
/// The bar's foot: risen this far, it is above the screen.
pub const BOTTOM: f32 = 144.0;
const ICON_SCALE_SECONDS: f32 = 0.12;

/// The top bar's buttons that take focus, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopButton {
    Profile,
    MyGames,
    Search,
    Settings,
    Notifications,
}

impl TopButton {
    pub const ALL: [TopButton; 5] =
        [TopButton::Profile, TopButton::MyGames, TopButton::Search, TopButton::Settings, TopButton::Notifications];

    fn place(self) -> usize {
        Self::ALL.iter().position(|&b| b == self).unwrap_or(0)
    }

    /// The next button to the left, if this is not the first.
    pub fn left(self) -> Option<TopButton> {
        self.place().checked_sub(1).map(|i| Self::ALL[i])
    }

    /// The next button to the right, if this is not the last.
    pub fn right(self) -> Option<TopButton> {
        Self::ALL.get(self.place() + 1).copied()
    }

    /// Where the button is: the profile's picture, or a round button's 60 square.
    pub fn rect(self) -> Rect {
        let round = |left: f32| Rect::new(left, MIDDLE - BUTTON / 2.0, BUTTON, BUTTON);
        match self {
            TopButton::Profile => Rect::new(MARGIN, MIDDLE - PICTURE / 2.0, PICTURE, PICTURE),
            TopButton::MyGames => round(850.0),
            TopButton::Search => round(930.0),
            TopButton::Settings => round(1010.0),
            TopButton::Notifications => round(1745.0),
        }
    }

    /// The button whose middle is nearest `x` across.
    pub fn nearest(x: f32) -> TopButton {
        let distance = |b: &TopButton| (b.rect().center().0 - x).abs();
        Self::ALL.into_iter().min_by(|a, b| distance(a).total_cmp(&distance(b))).unwrap_or(TopButton::Profile)
    }

    /// The round button's glyph without focus and with it, and the label under it with focus.
    fn face(self) -> Option<(char, char, &'static str)> {
        match self {
            TopButton::Profile => None,
            TopButton::MyGames => Some((glyph::LIBRARY, glyph::LIBRARY, "My games & apps")),
            TopButton::Search => Some((glyph::SEARCH, glyph::SEARCH, "Search")),
            TopButton::Settings => Some((glyph::SETTINGS, glyph::SETTINGS_SOLID, "Settings")),
            TopButton::Notifications => Some((glyph::NOTIFICATIONS, glyph::NOTIFICATIONS, "Notifications")),
        }
    }
}

/// With a real name, the gamertag and the real name take turns in the name's place, as on the console: each for
/// 7.2 s, then a 0.3 s crossfade to the other, over and over (the Bundle's `NameSwap`).
const NAME_TURN: f32 = 15.0;
const NAME_HOLD: f32 = 7.2;
const NAME_FADE: f32 = 0.3;

/// How much of the gamertag shows at `t` seconds into the turns: 1, fading to 0 at 7.2..7.5, 0, back to 1 at
/// 14.7..15. The real name shows the rest.
pub fn gamertag_share(t: f32) -> f32 {
    let t = t.rem_euclid(NAME_TURN);
    let half = NAME_TURN / 2.0;
    if t < NAME_HOLD {
        1.0
    } else if t < half {
        1.0 - (t - NAME_HOLD) / NAME_FADE
    } else if t < half + NAME_HOLD {
        0.0
    } else {
        (t - half - NAME_HOLD) / NAME_FADE
    }
}

/// What the top bar shows that is not a button.
pub struct Status<'a> {
    /// The Profile signed in: its picture and gamertag, taking turns with its real name if it has one.
    pub profile: &'a model::Profile,
    pub clock: &'a str,
    /// Whether a pad is connected: it stands left of the clock.
    pub pad: bool,
    /// The pad's battery: a cell in the glyph's place where a charge is known. None, or a cable, for the glyph.
    pub pad_battery: Option<ui::Battery>,
    /// Settings' Show battery percentages: the figure beside the battery.
    pub show_percent: bool,
    /// The Score total under the gamertag, as the console's; none while there are no achievement totals.
    pub score: Option<u64>,
    /// In a Discord Call, its mark: 16 left of the pad, or of the clock without one.
    pub discord_call: Option<model::discord::CallMark>,
}

/// The Score line's G disc, and the gap after it (Main.xaml, XboxTopBarProfile).
const G: f32 = 19.0;
const G_GAP: f32 = 6.0;
/// A line of Segoe UI 26: the gamertag's, and the Score's under it.
const LINE: f32 = 34.58;

pub fn draw(ui: &mut Ui, status: &Status, focused: Option<TopButton>) {
    let Status { profile, clock, pad, pad_battery, show_percent, score, discord_call } = *status;
    let picture = TopButton::Profile.rect();
    profile::picture(ui, picture, &profile.picture);
    if focused == Some(TopButton::Profile) {
        // The ring stands 4 clear of the picture.
        theme::focus_ring_round(ui, picture, 4.0);
    }
    // Segoe UI sits 2 up on the icons' middle, Bahnschrift 2 down.
    let name_at = picture.right() + 16.0;
    // The turns run on the ui's clock, not under Reduce motion, when the gamertag stays. Frames are asked for only
    // while the two cross; at rest the bar asks to be woken for the next crossing.
    let share = if profile.has_real_name() && !ui.store.reduce_motion {
        let t = ui.store.time().rem_euclid(f64::from(NAME_TURN)) as f32;
        let share = gamertag_share(t);
        if share > 0.0 && share < 1.0 {
            ui.time();
        } else {
            let next = [NAME_HOLD, NAME_TURN / 2.0 + NAME_HOLD, NAME_TURN].into_iter().find(|&at| at > t);
            ui.store.wake_in(next.unwrap_or(NAME_TURN) - t);
        }
        share
    } else {
        1.0
    };
    if share > 0.0 {
        // With a Score the gamertag and the Score line under it stand together on the bar's middle, and fade
        // together.
        ui.with_alpha(share, |ui| match score {
            Some(score) => {
                ui.label_middle(name_at, MIDDLE - 2.0 - LINE / 2.0, &profile.gamertag, &BODY);
                let line = MIDDLE - 2.0 + LINE / 2.0;
                achievement::g_disc(ui, name_at + G / 2.0, line, G, Color::BLACK);
                ui.label_middle(name_at + G + G_GAP, line, &achievement::thousands(score), &BODY);
            }
            None => {
                ui.label_middle(name_at, MIDDLE - 2.0, &profile.gamertag, &BODY);
            }
        });
    }
    if share < 1.0 {
        ui.with_alpha(1.0 - share, |ui| ui.label_middle(name_at, MIDDLE - 2.0, &profile.real_name, &BODY));
    }

    for button in TopButton::ALL {
        let Some((glyph, glyph_focused, label)) = button.face() else { continue };
        let rect = button.rect();
        let (cx, cy) = rect.center();
        let has_focus = focused == Some(button);
        // The icon grows by a fifth with focus, eased, and goes back evenly; not at all under Reduce motion.
        let (to, ease) = if has_focus { (1.2, Ease::CubicOut) } else { (1.0, Ease::Linear) };
        let scale = ui.tween(("top bar icon", button), to, ICON_SCALE_SECONDS, ease);
        let scale = if ui.store.reduce_motion { 1.0 } else { scale };
        if has_focus {
            // The ring is the button's own edge, over a faint white disc.
            ui.fill(rect, Color::argb(0x33FFFFFF), 999.0);
            theme::focus_ring_round(ui, rect, -3.0);
            let width = ui.measure(&BODY, label).width;
            ui.label(cx - width / 2.0, rect.y + 70.0, label, &BODY);
        }
        ui.icon(cx, cy, if has_focus { glyph_focused } else { glyph }, 26.0 * scale, Color::WHITE);
    }
    // The status area ends 20 left of Notifications: the clock, and 16 left of it the pad.
    let clock = ui.label_right(1725.0, MIDDLE + 2.0, clock, &theme::CLOCK);
    let mut x = 1725.0 - clock.width;
    if pad {
        // On a cable the pad's glyph; wireless its battery, with its charge beside it unless Settings says not.
        let battery = guide::Status { pad_battery, show_percent, ..guide::Status::default() };
        x = guide::pad_battery(ui, x - 16.0, MIDDLE, &battery);
    }
    if let Some(mark) = discord_call {
        guide::call_mark(ui, x - 16.0, MIDDLE, mark);
    }
}

#[cfg(test)]
mod tests {
    use super::gamertag_share;

    /// The Bundle's swap (home.md 2.1): the gamertag for 7.2 s, out by 7.5, the real name until 14.7, back by 15.
    #[test]
    fn the_gamertag_and_the_real_name_take_turns() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(near(gamertag_share(0.0), 1.0));
        assert!(near(gamertag_share(7.2), 1.0));
        assert!(near(gamertag_share(7.35), 0.5));
        assert!(near(gamertag_share(7.5), 0.0));
        assert!(near(gamertag_share(14.7), 0.0));
        assert!(near(gamertag_share(14.85), 0.5));
        assert!(near(gamertag_share(15.0), 1.0));
        assert!(near(gamertag_share(22.5), 0.0), "the next turn");
    }
}
