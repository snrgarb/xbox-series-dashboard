//! The hub's actions (docs/spec/game-hub.md, section 6): the main action, Manage, and the page toggle, in a row
//! under the cards. Between Manage and the toggle is Queue, which shows only for a game in the Queue, the toggle
//! moving along for it.
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
    /// Play, Running, Install, Queued or Installing...
    Main,
    Manage,
    /// To Manage Queue: only while the game is in the Queue.
    Queue,
    /// To the hub's other page.
    Toggle,
}

/// The actions, left to right, for a game in the Queue or not.
pub fn shown(queued: bool) -> &'static [Action] {
    if queued {
        &[Action::Main, Action::Manage, Action::Queue, Action::Toggle]
    } else {
        &[Action::Main, Action::Manage, Action::Toggle]
    }
}

/// What the main action is, by the game's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Main {
    Play,
    /// The game is running, or being launched: A does nothing.
    Running,
    /// The game is not installed.
    Install,
    /// The game is in the Queue, waiting its turn (or paused): A does nothing.
    Queued,
    /// The game is in the Queue, downloading: A does nothing.
    Installing,
}

impl Main {
    /// What it reads: the state, in capitals.
    fn label(self) -> &'static str {
        match self {
            Main::Play => "PLAY",
            Main::Running => "RUNNING",
            Main::Install => "INSTALL",
            Main::Queued => "QUEUED",
            Main::Installing => "INSTALLING...",
        }
    }
}

/// The toggle's label: the page it turns to.
fn toggle_label(details: bool) -> &'static str {
    if details {
        "GAME HUB"
    } else {
        "GAME DETAILS"
    }
}

/// Where each action is, in the order of [`shown`], with Queue or without. The toggle is as wide as what is in it:
/// `toggle` is its width.
pub fn rects(toggle: f32, queued: bool) -> Vec<Rect> {
    let mut x = cards::LEFT;
    shown(queued)
        .iter()
        .map(|action| {
            let width = match action {
                Action::Main => MAIN_WIDTH,
                Action::Manage | Action::Queue => ICON_WIDTH,
                Action::Toggle => toggle,
            };
            let rect = Rect::new(x, TOP, width, HEIGHT);
            x += width + GAP;
            rect
        })
        .collect()
}

/// Draws the row, with the focus ring round the action with focus, and Queue while the game is `queued`. `source`
/// is the store's name, which Install carries under its label. Returns the toggle's width.
pub fn draw(ui: &mut Ui, main: Main, source: Option<&str>, details: bool, queued: bool, focus: Option<Action>) -> f32 {
    let label = toggle_label(details);
    let toggle = 2.0 * PADDING + TOGGLE_ICON + TOGGLE_GAP + ui.measure(&LABEL, label).width;
    let rects = rects(toggle.max(ICON_WIDTH), queued);
    for (&action, &rect) in shown(queued).iter().zip(&rects) {
        let fill = if focus == Some(action) { theme::ACCENT_LIGHT } else { REST };
        ui.fill(rect, fill, RADIUS);
        let (cx, cy) = rect.center();
        match action {
            Action::Main => {
                let text = main.label();
                let second = source.filter(|_| main == Main::Install);
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
            Action::Queue => ui.icon(cx, cy, glyph::DOWNLOAD, 28.0, Color::WHITE),
            Action::Toggle => {
                let icon = if details { glyph::HOME } else { INFO };
                ui.icon(rect.x + PADDING + TOGGLE_ICON / 2.0, cy, icon, TOGGLE_ICON, Color::WHITE);
                ui.label_middle(rect.x + PADDING + TOGGLE_ICON + TOGGLE_GAP, cy, label, &LABEL);
            }
        }
    }
    if let Some(at) = focus.and_then(|focus| shown(queued).iter().position(|&action| action == focus)) {
        theme::focus_ring_cornered(ui, rects[at], 14.0);
    }
    toggle
}

/// The name an icon action with focus says over itself: Manage's or Queue's. The others carry their own labels. It
/// lies over the ring's top stroke and the cards' foot, so it is drawn after both, on a layer of its own.
pub fn name_chip(ui: &mut Ui, focus: Option<Action>, toggle: f32, queued: bool) {
    let name = match focus {
        Some(Action::Manage) => "Manage",
        Some(Action::Queue) => "Queue",
        _ => return,
    };
    let Some(at) = shown(queued).iter().position(|&action| Some(action) == focus) else { return };
    let action = rects(toggle, queued)[at];
    let text = ui.measure(&BODY, name);
    // 10 and 4 of padding inside a 1 wide border.
    let (width, height) = (text.width + 22.0, text.height + 10.0);
    let chip = Rect::new(action.center().0 - width / 2.0, action.y - CHIP_UP, width, height);
    ui.layer();
    ui.fill(chip, CHIP, 3.0);
    ui.ring(chip, CHIP_EDGE, 3.0, 1.0);
    ui.label_centered(chip.center().0, chip.center().1, name, &BODY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_actions_are_where_the_spec_says() {
        let [main, manage, toggle] = rects(269.0, false)[..] else { panic!() };
        assert_eq!((main.x, main.right()), (80.0, 460.0));
        assert_eq!((manage.x, manage.right()), (472.0, 552.0));
        assert_eq!((toggle.x, toggle.right()), (564.0, 833.0));
        assert!((main.y - 802.4).abs() < 0.01 && main.h == 80.0);
        // Queue between Manage and the toggle, which moves 92 along for it.
        let [_, _, queue, moved] = rects(269.0, true)[..] else { panic!() };
        assert_eq!((queue.x, queue.right()), (564.0, 644.0));
        assert_eq!(moved.x - toggle.x, 92.0);
    }
}
