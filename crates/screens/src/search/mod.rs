//! Search (docs/spec/search.md): the search box, the Console keyboard and the results.
//!
//! **A stand-in until #55:** the page opens and B closes it. Everything in this folder is that ticket's to replace;
//! what the rest of the Dashboard relies on is [`Search::open`] and the `Page` implementation.
use ui::{Button, Color, Press, Ui};

use crate::pages::{Around, Page, PageAction};
use crate::theme::HEADING;

/// The dark of the page's gradient, which the real page draws (docs/spec/search.md, 1.2).
const FILL: Color = Color::rgb(0x07140F);

/// Search opens empty every time, whatever opened it (docs/spec/search.md, 4.1): there is no way in to tell apart.
/// Where B goes is the stack's business: to the page under it, or to Home.
pub struct Search;

impl Search {
    pub fn open() -> Self {
        Self
    }
}

impl Page for Search {
    fn name(&self) -> &'static str {
        "Search"
    }

    fn focus(&self) -> String {
        // The real page opens with focus on the keyboard's first letter.
        "Open".into()
    }

    fn press(&mut self, press: Press, _around: Around) -> Option<PageAction> {
        (press.button == Button::B && !press.repeat).then_some(PageAction::Close)
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        ui.fill(ui.screen(), FILL, 0.0);
        ui.label(105.0, 200.0, "Search", &HEADING);
    }
}
