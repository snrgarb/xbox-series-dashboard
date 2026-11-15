//! The hint bar: which button does what where focus is, in the screen's bottom right corner (docs/spec/home.md,
//! section 10.4). Whatever has the pad says which hints it wants (`Home::hints`, `Page::hints`); the Dashboard draws
//! the bar once, over all of it, so no page draws its own. It is there or not at once, and never scrolls.
use ui::{Color, Font, Rect, TextStyle, Ui};

use crate::theme::BODY;

/// The pad button a hint is for, drawn in a ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintButton {
    /// Menu: its three lines, white in a white ring.
    Menu,
    /// Y: the letter in a ring, both in the pad's yellow.
    Y,
}

/// One hint: a button and what it does here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hint {
    pub button: HintButton,
    pub label: &'static str,
}

const MENU: Hint = Hint { button: HintButton::Menu, label: "More options" };
const Y: Hint = Hint { button: HintButton::Y, label: "Search" };

/// The bar's three states in the Bundle. Nothing to hint at: there is no bar.
pub const NONE: &[Hint] = &[];
/// What has focus has a menu.
pub const MORE_OPTIONS: &[Hint] = &[MENU];
/// It has a menu, and Y opens Search from here: a Group's game card on Home, a game in Games' grid. Y is never
/// hinted at alone.
pub const MORE_OPTIONS_AND_SEARCH: &[Hint] = &[MENU, Y];

/// The bar is 63 high, 60 in from the screen's right and 52 up from its foot.
const HEIGHT: f32 = 63.0;
const FROM_RIGHT: f32 = 60.0;
const FROM_FOOT: f32 = 52.0;
const PADDING: (f32, f32) = (16.0, 20.0);
const BACK: Color = Color::argb(0xD9030303);
const RADIUS: f32 = 6.0;
/// Each hint is a ring 36 across, then its label 12 on; the next hint is 26 after the label.
const RING: f32 = 36.0;
const LABEL_GAP: f32 = 12.0;
const HINT_GAP: f32 = 26.0;
/// The label sits 3 over the bar's middle and the Y 1 over its ring's, so the lower case reads centred on the ring.
const LABEL_RISE: f32 = 3.0;
const LETTER_RISE: f32 = 1.0;
const YELLOW: Color = Color::rgb(0xFFB900);
const LETTER: TextStyle = TextStyle::new(Font::Ui, 19.0, 400).color(YELLOW);
const MENU_GLYPH: char = '\u{E700}';

/// Where the bar is with these hints on this screen: anchored to the screen's corner, whatever its shape. None with
/// no hints.
pub fn rect(ui: &mut Ui, hints: &[Hint]) -> Option<Rect> {
    if hints.is_empty() {
        return None;
    }
    let labels: f32 = hints.iter().map(|hint| ui.measure(&BODY, hint.label).width).sum();
    let count = hints.len() as f32;
    let width = PADDING.0 + count * (RING + LABEL_GAP) + labels + (count - 1.0) * HINT_GAP + PADDING.1;
    let screen = ui.screen();
    Some(Rect::new(screen.right() - FROM_RIGHT - width, screen.bottom() - FROM_FOOT - HEIGHT, width, HEIGHT))
}

/// Draws the bar over everything drawn so far, on a layer of its own: it covers the page's text. Nothing with no
/// hints.
pub fn draw(ui: &mut Ui, hints: &[Hint]) {
    let Some(bar) = rect(ui, hints) else { return };
    ui.layer();
    ui.fill(bar, BACK, RADIUS);
    let middle = bar.center().1;
    let mut x = bar.x + PADDING.0;
    for hint in hints {
        let ring = Rect::new(x, middle - RING / 2.0, RING, RING);
        let (cx, cy) = ring.center();
        match hint.button {
            HintButton::Menu => {
                ui.ring(ring, Color::WHITE, 999.0, 2.0);
                ui.icon(cx, cy, MENU_GLYPH, 17.0, Color::WHITE);
            }
            HintButton::Y => {
                ui.ring(ring, YELLOW, 999.0, 2.0);
                ui.label_centered(cx, cy - LETTER_RISE, "Y", &LETTER);
            }
        }
        let label = ui.label_middle(ring.right() + LABEL_GAP, middle - LABEL_RISE, hint.label, &BODY);
        x = ring.right() + LABEL_GAP + label.width + HINT_GAP;
    }
}
