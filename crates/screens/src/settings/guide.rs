//! Theme settings > Guide (docs/spec/settings.md, 6.8): Use the Guide. The Now Playing source under it comes with
//! the Audio flyout, in M5.
use ui::{Button, Press, Rect, Ui};

use super::controls::{self, FIELD_HEIGHT, FIELD_WIDTH, LEFT, TOP};
use super::{Change, Pane, Setting, Step};
use crate::pages::{Around, PageAction};
use crate::Effect;

const ABOUT: &str = "Opens the Guide with the Guide button and from Home. Turn it off to leave the Guide button to \
                     Steam or another app.";

/// The page has one control, which has focus whenever the page does.
pub struct Guide;

impl Pane for Guide {
    fn header(&self) -> String {
        "Guide".into()
    }

    fn focus(&self) -> String {
        "Guide: Use the Guide".into()
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        match press.button {
            Button::A => {
                let change = Change { setting: Setting::UseGuide, on: !around.options.use_guide };
                Step::Do(PageAction::Effect(Effect::Set(change)))
            }
            Button::B | Button::Left => Step::Back,
            _ => Step::Stay,
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        // The label, its description 4 under it, and the checkbox 18 under that.
        let mut y = TOP;
        y += controls::label(ui, LEFT, y, "Guide") + 4.0;
        y += controls::description(ui, LEFT, y, FIELD_WIDTH, ABOUT) + 18.0;
        let row = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        controls::checkbox_row(ui, row, "Use the Guide", around.options.use_guide, focused);
    }
}
