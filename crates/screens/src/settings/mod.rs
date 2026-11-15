//! Settings (docs/spec/settings.md): the rail down the left, and beside it a category's boxes and their pages.
//!
//! **A stand-in until #56:** the page opens, says where it was opened on, and B closes it. Everything in this
//! folder is that ticket's to replace; what the rest of the Dashboard relies on is [`Entry`], [`Settings::open`]
//! and the `Page` implementation.
use ui::{Button, Color, Press, Ui};

use crate::pages::{Around, Page, PageAction};
use crate::theme::{BODY, HEADING, TEXT_DIM};

/// How the page is opened: what shows, and so where focus lands and how many levels B climbs before the page
/// closes (docs/spec/settings.md, 8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// The first category's boxes, focus on the rail: Home's top bar button, the Guide's Settings.
    Rail,
    /// Home's Settings tile, which in the Bundle is "Playnite settings" and opens that category on its first box.
    /// Where it lands with that category gone is #56's to settle (docs/spec/settings.md, open question 2).
    Tile,
    /// Personalisation's page: the Tile menu's Customise Home.
    Personalisation,
    /// Discord's page, focus on Type: Home's Discord tile while there is no Discord tab in the Guide.
    Discord,
}

/// The page's fill.
const FILL: Color = Color::rgb(0x1F1F1F);

pub struct Settings {
    entry: Entry,
}

impl Settings {
    pub fn open(entry: Entry) -> Self {
        Self { entry }
    }
}

impl Page for Settings {
    fn name(&self) -> &'static str {
        "Settings"
    }

    fn focus(&self) -> String {
        format!("{:?}", self.entry)
    }

    fn press(&mut self, press: Press, _around: Around) -> Option<PageAction> {
        (press.button == Button::B && !press.repeat).then_some(PageAction::Close)
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        ui.fill(ui.screen(), FILL, 0.0);
        let shows = match self.entry {
            Entry::Rail | Entry::Tile => "Theme settings",
            Entry::Personalisation => "Personalisation",
            Entry::Discord => "Discord",
        };
        ui.label(75.0, 78.0, "Settings", &HEADING.size(30.0));
        ui.label(520.0, 72.0, shows, &BODY.color(TEXT_DIM));
    }
}
