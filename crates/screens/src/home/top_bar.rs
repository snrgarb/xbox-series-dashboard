//! The top bar: the profile at the left, the three round buttons in the middle, the clock and Notifications at the
//! right. Its measures are the screen's own, not the rows': nothing here is multiplied by ROW_SCALE.
use ui::{Color, Ease, Rect, Ui};

use crate::theme::{self, glyph, BODY, BODY_SEMIBOLD, MARGIN};

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

/// The profile's picture: until Profiles have pictures, the green disc with the contact glyph, or where there is
/// no icon font the gamertag's initial.
pub fn profile_picture(ui: &mut Ui, rect: Rect, user: &str) {
    ui.fill(rect, theme::ACCENT, 999.0);
    let (cx, cy) = rect.center();
    if ui.has_font(ui::Font::Icons) {
        ui.icon(cx, cy, glyph::CONTACT, 26.0, Color::WHITE);
    } else {
        let initial: String = user.chars().take(1).flat_map(char::to_uppercase).collect();
        ui.label_centered(cx, cy - 1.0, &initial, &BODY_SEMIBOLD);
    }
}

pub fn draw(ui: &mut Ui, user: &str, clock: &str, focused: Option<TopButton>) {
    let picture = TopButton::Profile.rect();
    profile_picture(ui, picture, user);
    if focused == Some(TopButton::Profile) {
        // The ring stands 4 clear of the picture.
        theme::focus_ring_round(ui, picture, 4.0);
    }
    // Segoe UI sits 2 up on the icons' middle, Bahnschrift 2 down.
    ui.label_middle(picture.right() + 16.0, MIDDLE - 2.0, user, &BODY);

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
    ui.label_right(1725.0, MIDDLE + 2.0, clock, &theme::CLOCK);
}
