//! General > Guide (docs/spec/settings.md, 6.8): Use the Guide, and under it the Now Playing source, which
//! app the Audio Flyout's Now Playing card follows (docs/spec/now-playing.md).
use ui::{Button, Press, Rect, Ui};

use super::controls::{self, Dropdown, FIELD_HEIGHT, FIELD_WIDTH, LEFT, TOP};
use super::{Change, Pane, Setting, Step};
use crate::pages::{Around, PageAction};
use crate::Effect;

const ABOUT: &str = "Opens the Guide with the Guide button and from Home. Turn it off to leave the Guide button to \
                     Steam or another app.";
const SOURCE_ABOUT: &str =
    "The app the Audio flyout's Now Playing follows while it plays or is paused, else whatever Windows shows.";

/// The page's two controls, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    UseGuide,
    Source,
}

pub struct Guide {
    focus: Focus,
    source: Dropdown,
}

impl Guide {
    pub fn new() -> Self {
        Self { focus: Focus::UseGuide, source: Dropdown::default() }
    }
}

impl Default for Guide {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for Guide {
    fn header(&self) -> String {
        "Guide".into()
    }

    /// `Guide: Use the Guide`, `Guide: Now Playing source`, and with its list open the choice focus is on:
    /// `Guide: Now Playing source, list on "Spotify"`.
    fn focus(&self) -> String {
        match (self.focus, self.source.focused()) {
            (Focus::UseGuide, _) => "Guide: Use the Guide".into(),
            (Focus::Source, None) => "Guide: Now Playing source".into(),
            (Focus::Source, Some(choice)) => format!("Guide: Now Playing source, list on {choice:?}"),
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        if self.source.is_open() {
            if press.repeat && !press.button.is_direction() {
                return Step::Stay;
            }
            let choices = around.now_playing.choices();
            return match self.source.press(press.button).and_then(|at| choices.get(at)) {
                Some(choice) => Step::Do(PageAction::Effect(Effect::NowPlayingSource(choice.clone()))),
                None => Step::Stay,
            };
        }
        match (press.button, self.focus) {
            (Button::Down, Focus::UseGuide) => self.focus = Focus::Source,
            (Button::Up, Focus::Source) => self.focus = Focus::UseGuide,
            (Button::A, Focus::UseGuide) if !press.repeat => {
                let change = Change { setting: Setting::UseGuide, on: !around.options.use_guide };
                return Step::Do(PageAction::Effect(Effect::Set(change)));
            }
            // The list opens on the source set now: Any, Spotify, then every app seen since the start.
            (Button::A, Focus::Source) if !press.repeat => {
                let choices = around.now_playing.choices();
                let set = choices.iter().position(|choice| choice == around.now_playing.source()).unwrap_or(0);
                self.source.open(choices, set);
            }
            (Button::B | Button::Left, _) => return Step::Back,
            _ => {}
        }
        Step::Stay
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        // The label, its description 4 under it, and the checkbox 18 under that; the source's label 40 under the
        // checkbox, and the same again for its dropdown.
        let mut y = TOP;
        y += controls::label(ui, LEFT, y, "Guide") + 4.0;
        y += controls::description(ui, LEFT, y, FIELD_WIDTH, ABOUT) + 18.0;
        let row = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        let on_row = focused && self.focus == Focus::UseGuide;
        controls::checkbox_row(ui, row, "Use the Guide", around.options.use_guide, on_row);
        y += FIELD_HEIGHT + 40.0;
        y += controls::label(ui, LEFT, y, "Now Playing source") + 4.0;
        y += controls::description(ui, LEFT, y, FIELD_WIDTH, SOURCE_ABOUT) + 18.0;
        let dropdown = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        let on_source = focused && self.focus == Focus::Source;
        self.source.draw(ui, dropdown, around.now_playing.source(), on_source);
        self.source.draw_list(ui, dropdown);
    }
}
