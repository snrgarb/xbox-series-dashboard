//! The hub's actions (docs/spec/game-hub.md, section 6): the main action, Manage, and the page toggle, in a row
//! under the cards. Between Manage and the toggle is the place of Queue (M4), which shows only for a game in the
//! Queue: when it is built it is an [`Action`] like Manage, and the toggle moves along.
use ui::{Color, Font, HAlign, Rect, TextStyle, Ui, VAlign};

use super::cards;
use crate::theme::{self, glyph, BODY, TEXT_DIM};

/// The row is 32 under the cards.
pub const TOP: f32 = cards::TOP + cards::HEIGHT + 32.0;
const HEIGHT: f32 = 80.0;
const GAP: f32 = 12.0;
const MAIN_WIDTH: f32 = 380.0;
const ICON_WIDTH: f32 = 80.0;
/// What is in an action starts this far in.
const PADDING: f32 = 28.0;
const REST: Color = Color::argb(0x33FFFFFF);
const RADIUS: f32 = 4.0;
const LABEL: TextStyle = TextStyle::new(Font::Display, 26.0, 700);
/// The toggle's icon, and the gap to its label.
const TOGGLE_ICON: f32 = 24.0;
const TOGGLE_GAP: f32 = 14.0;
const INFO: char = '\u{E946}';
/// The name chip over an icon action with focus: its top is this far above the action's.
const CHIP_UP: f32 = 48.0;
const CHIP: Color = Color::argb(0xCC1A1A1A);
const CHIP_EDGE: Color = Color::argb(0x80FFFFFF);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Play, Running or Install.
    Main,
    Manage,
    /// To the hub's other page.
    Toggle,
}

/// The actions, left to right.
pub const ALL: [Action; 3] = [Action::Main, Action::Manage, Action::Toggle];

/// What the main action is, by the game's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Main {
    Play,
    /// The game is running, or being launched: A does nothing.
    Running,
    /// The game is not installed.
    Install,
}

/// The toggle's label: the page it turns to.
fn toggle_label(details: bool) -> &'static str {
    if details {
        "GAME HUB"
    } else {
        "GAME DETAILS"
    }
}

/// Where each action is, in the order of [`ALL`]. The toggle is as wide as what is in it: `toggle` is its width.
pub fn rects(toggle: f32) -> [Rect; 3] {
    let manage = cards::LEFT + MAIN_WIDTH + GAP;
    [
        Rect::new(cards::LEFT, TOP, MAIN_WIDTH, HEIGHT),
        Rect::new(manage, TOP, ICON_WIDTH, HEIGHT),
        Rect::new(manage + ICON_WIDTH + GAP, TOP, toggle, HEIGHT),
    ]
}

/// Draws the row, with the focus ring round the action with focus. `source` is the store's name, which Install
/// carries under its label. Returns the toggle's width.
pub fn draw(ui: &mut Ui, main: Main, source: Option<&str>, details: bool, focus: Option<Action>) -> f32 {
    let label = toggle_label(details);
    let toggle = 2.0 * PADDING + TOGGLE_ICON + TOGGLE_GAP + ui.measure(&LABEL, label).width;
    let rects = rects(toggle.max(ICON_WIDTH));
    for (action, rect) in ALL.into_iter().zip(rects) {
        let fill = if focus == Some(action) { theme::ACCENT_LIGHT } else { REST };
        ui.fill(rect, fill, RADIUS);
        let (cx, cy) = rect.center();
        match action {
            Action::Main => {
                let (text, second) = match main {
                    Main::Play => ("PLAY", None),
                    Main::Running => ("RUNNING", None),
                    Main::Install => ("INSTALL", source),
                };
                let width = rect.w - 2.0 * PADDING;
                let line = ui.measure(&LABEL, text).height;
                // With the source under it, the two lines are centred as one block.
                let under = second.map_or(0.0, |_| ui.measure(&BODY, "Steam").height);
                let top = cy - (line + under) / 2.0;
                let x = rect.x + PADDING;
                ui.paragraph(Rect::new(x, top, width, line), text, &LABEL, HAlign::Left, VAlign::Top, 1);
                if let Some(second) = second {
                    let dim = BODY.color(TEXT_DIM);
                    ui.paragraph(Rect::new(x, top + line, width, under), second, &dim, HAlign::Left, VAlign::Top, 1);
                }
            }
            Action::Manage => ui.icon(cx, cy, glyph::SETTINGS, 28.0, Color::WHITE),
            Action::Toggle => {
                let icon = if details { glyph::HOME } else { INFO };
                ui.icon(rect.x + PADDING + TOGGLE_ICON / 2.0, cy, icon, TOGGLE_ICON, Color::WHITE);
                ui.label_middle(rect.x + PADDING + TOGGLE_ICON + TOGGLE_GAP, cy, label, &LABEL);
            }
        }
    }
    if let Some(at) = focus.and_then(|focus| ALL.iter().position(|&action| action == focus)) {
        theme::focus_ring_cornered(ui, rects[at], 14.0);
    }
    toggle
}

/// The name an icon action with focus says over itself: Manage's. The others carry their own labels. It lies over
/// the ring's top stroke and the cards' foot, so it is drawn after both, on a layer of its own.
pub fn name_chip(ui: &mut Ui, focus: Option<Action>, toggle: f32) {
    if focus != Some(Action::Manage) {
        return;
    }
    let action = rects(toggle)[1];
    let text = ui.measure(&BODY, "Manage");
    // 10 and 4 of padding inside a 1 wide border.
    let (width, height) = (text.width + 22.0, text.height + 10.0);
    let chip = Rect::new(action.center().0 - width / 2.0, action.y - CHIP_UP, width, height);
    ui.layer();
    ui.fill(chip, CHIP, 3.0);
    ui.ring(chip, CHIP_EDGE, 3.0, 1.0);
    ui.label_centered(chip.center().0, chip.center().1, "Manage", &BODY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_actions_are_where_the_spec_says() {
        let [main, manage, toggle] = rects(269.0);
        assert_eq!((main.x, main.right()), (80.0, 460.0));
        assert_eq!((manage.x, manage.right()), (472.0, 552.0));
        assert_eq!((toggle.x, toggle.right()), (564.0, 833.0));
        assert!((main.y - 802.4).abs() < 0.01 && main.h == 80.0);
    }
}
