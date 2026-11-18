//! Account > Who's playing today? (docs/spec/settings.md, 6.4; docs/spec/start-up.md, section 6): two Rows, Ask
//! every time and Sign in the last profile, the one in force ticked. A sets a Row at once and saved; the tick moves,
//! focus stays. It is the Bundle's page (E/PersonalisationPane.xaml:289-294,
//! E/PersonalisationPane.xaml.cs:194-200, 367-392) without the Bundle's line over the Rows (#261).
use ui::{Button, Press, Rect, Ui};

use super::controls::{self, BOXES_TOP, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH};
use super::options::{Change, Setting};
use super::{Pane, Step};
use crate::pages::{Around, PageAction};
use crate::Effect;

/// The Rows, top down: what each says, and whether choosing it has Who's playing today? ask.
const ROWS: [(&str, bool); 2] = [("Ask every time", true), ("Sign in the last profile", false)];

pub struct WhosPlayingPage {
    focus: usize,
    /// The Row ticked, as this page last set it: what the focus log says of the focused Row.
    ticked: usize,
}

impl WhosPlayingPage {
    /// The page as the option stands, focus on the Row ticked.
    pub fn open(around: Around) -> Self {
        let ticked = Self::ticked(around);
        Self { focus: ticked, ticked }
    }

    fn ticked(around: Around) -> usize {
        usize::from(!around.options.ask_whos_playing)
    }
}

impl Pane for WhosPlayingPage {
    fn header(&self) -> String {
        "Who's playing today?".into()
    }

    fn focus(&self) -> String {
        format!("Who's playing today?: {}", ROWS[self.focus].0)
    }

    /// Whether the focused Row is the one ticked, as Boot video's says it:
    /// `focus: Settings: Who's playing today?: Ask every time; ticked`.
    fn state(&self) -> String {
        if self.focus == self.ticked {
            "ticked".into()
        } else {
            String::new()
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        match press.button {
            Button::B => Step::Back,
            Button::Up => {
                self.focus = self.focus.saturating_sub(1);
                Step::Stay
            }
            Button::Down => {
                self.focus = (self.focus + 1).min(ROWS.len() - 1);
                Step::Stay
            }
            Button::A if !press.repeat => {
                let ask = ROWS[self.focus].1;
                if ask == around.options.ask_whos_playing {
                    return Step::Stay;
                }
                self.ticked = self.focus;
                let change = Change { setting: Setting::AskWhosPlaying, on: ask };
                Step::Do(PageAction::Effect(Effect::Set(change)))
            }
            _ => Step::Stay,
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let top = BOXES_TOP;
        let ticked = Self::ticked(around);
        let row = |i: usize| Rect::new(LEFT, top + ROW_PITCH * i as f32, ROW_WIDTH, ROW_HEIGHT);
        for (i, (name, _)) in ROWS.iter().enumerate() {
            if !focused || i != self.focus {
                controls::row(ui, row(i), name, i == ticked, false);
            }
        }
        if focused {
            controls::row(ui, row(self.focus), ROWS[self.focus].0, self.focus == ticked, true);
        }
    }
}
