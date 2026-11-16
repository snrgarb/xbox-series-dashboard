//! Theme settings > Idle (docs/spec/settings.md, 6.7): the Idle screen's two times, each a dropdown of the same
//! steps, Show things after never shorter than Dim after (docs/spec/idle-screen.md).
use std::time::Duration;

use ui::{Button, Press, Rect, Ui};

use super::controls::{self, Dropdown, FIELD_HEIGHT, FIELD_WIDTH, LEFT, TOP};
use super::{Pane, Step};
use crate::idle::{describe, STEPS};
use crate::pages::{Around, PageAction};
use crate::Effect;

/// The page's two dropdowns, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Dim,
    Show,
}

pub struct Idle {
    focus: Focus,
    dim: Dropdown,
    show: Dropdown,
}

impl Idle {
    pub fn new() -> Self {
        Self { focus: Focus::Dim, dim: Dropdown::default(), show: Dropdown::default() }
    }
}

impl Default for Idle {
    fn default() -> Self {
        Self::new()
    }
}

/// A dropdown's list: the steps worded, and where the one set is among them (the first if it is none of them,
/// as a test's `--idle-after` time is not).
fn list(steps: &[Option<Duration>], set: Option<Duration>) -> (Vec<String>, usize) {
    let words = steps.iter().map(|&step| describe(step)).collect();
    (words, steps.iter().position(|&step| step == set).unwrap_or(0))
}

impl Pane for Idle {
    fn header(&self) -> String {
        "Idle".into()
    }

    /// `Idle: Dim after`, `Idle: Show things after`, and with a list open the choice focus is on:
    /// `Idle: Dim after, list on "2 min"`.
    fn focus(&self) -> String {
        let (name, open) = match self.focus {
            Focus::Dim => ("Dim after", self.dim.focused()),
            Focus::Show => ("Show things after", self.show.focused()),
        };
        match open {
            None => format!("Idle: {name}"),
            Some(choice) => format!("Idle: {name}, list on {choice:?}"),
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        let times = around.options.idle;
        let open = if self.dim.is_open() { Some(Focus::Dim) } else { self.show.is_open().then_some(Focus::Show) };
        if let Some(open) = open {
            if press.repeat && !press.button.is_direction() {
                return Step::Stay;
            }
            // A choice the coupling rule refuses, or the one set already, changes nothing and saves nothing.
            let chosen = match open {
                Focus::Dim => self.dim.press(press.button).and_then(|at| times.with_dim(STEPS[at])),
                Focus::Show => {
                    let steps = times.show_steps();
                    self.show.press(press.button).and_then(|at| steps.get(at)).and_then(|&step| times.with_show(step))
                }
            };
            return match chosen {
                Some(times) => Step::Do(PageAction::Effect(Effect::IdleTimes(times))),
                None => Step::Stay,
            };
        }
        match (press.button, self.focus) {
            (Button::Down, Focus::Dim) => self.focus = Focus::Show,
            (Button::Up, Focus::Show) => self.focus = Focus::Dim,
            (Button::A, Focus::Dim) if !press.repeat => {
                let (choices, set) = list(&STEPS, times.dim_after);
                self.dim.open(choices, set);
            }
            (Button::A, Focus::Show) if !press.repeat => {
                let (choices, set) = list(&times.show_steps(), times.show_after);
                self.show.open(choices, set);
            }
            (Button::B | Button::Left, _) => return Step::Back,
            _ => {}
        }
        Step::Stay
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        // Each label, its dropdown 14 under it, and the next label 48 under that; no descriptions, as the Bundle's.
        let times = around.options.idle;
        let mut y = TOP;
        y += controls::label(ui, LEFT, y, "Dim after") + 14.0;
        let dim = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        self.dim.draw(ui, dim, &describe(times.dim_after), focused && self.focus == Focus::Dim);
        y += FIELD_HEIGHT + 48.0;
        y += controls::label(ui, LEFT, y, "Show things after") + 14.0;
        let show = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        self.show.draw(ui, show, &describe(times.show_after), focused && self.focus == Focus::Show);
        // The open list last: it lies over what is under it and moves nothing.
        self.dim.draw_list(ui, dim);
        self.show.draw_list(ui, show);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idle::Times;
    use crate::settings::Options;
    use model::Library;

    fn around(library: &Library, idle: Times) -> Around<'_> {
        Around {
            library,
            playing: None,
            options: Options { idle, ..Options::default() },
            now: std::time::Instant::now(),
            now_playing: crate::now_playing::none(),
            discord: crate::no_discord(),
        }
    }

    fn press(pane: &mut Idle, button: Button, around: Around) -> Step {
        pane.press(Press { button, repeat: false }, around)
    }

    fn minutes(n: u64) -> Option<Duration> {
        Some(Duration::from_secs(n * 60))
    }

    #[test]
    fn dim_after_raised_past_show_things_after_raises_it_too() {
        let library = Library::default();
        let times = Times::default();
        let mut pane = Idle::new();
        assert_eq!(pane.focus(), "Idle: Dim after");
        press(&mut pane, Button::A, around(&library, times));
        assert_eq!(pane.focus(), "Idle: Dim after, list on \"2 min\"");
        // 2 min, 5 min, 10 min.
        press(&mut pane, Button::Down, around(&library, times));
        press(&mut pane, Button::Down, around(&library, times));
        let step = press(&mut pane, Button::A, around(&library, times));
        let raised = Times { dim_after: minutes(10), show_after: minutes(10) };
        assert!(matches!(step, Step::Do(PageAction::Effect(Effect::IdleTimes(times))) if times == raised));
        assert_eq!(pane.focus(), "Idle: Dim after");
    }

    #[test]
    fn show_things_after_offers_only_steps_not_shorter_than_dim_after() {
        let library = Library::default();
        let times = Times::default();
        let mut pane = Idle::new();
        press(&mut pane, Button::Down, around(&library, times));
        assert_eq!(pane.focus(), "Idle: Show things after");
        press(&mut pane, Button::A, around(&library, times));
        assert_eq!(pane.focus(), "Idle: Show things after, list on \"5 min\"");
        // Up twice: 2 min is the list's top, so it stops there; choosing it is allowed.
        for _ in 0..3 {
            press(&mut pane, Button::Up, around(&library, times));
        }
        assert_eq!(pane.focus(), "Idle: Show things after, list on \"2 min\"");
        let step = press(&mut pane, Button::A, around(&library, times));
        let set = Times { dim_after: minutes(2), show_after: minutes(2) };
        assert!(matches!(step, Step::Do(PageAction::Effect(Effect::IdleTimes(times))) if times == set));
        // The one set already saves nothing; B closes, Left goes back.
        press(&mut pane, Button::A, around(&library, set));
        assert!(matches!(press(&mut pane, Button::A, around(&library, set)), Step::Stay));
        assert!(matches!(press(&mut pane, Button::Left, around(&library, set)), Step::Back));
    }
}
