//! My games & apps (docs/spec/my-games.md): the nav down the left, and beside it Games, Play history, Full library
//! or Manage.
//!
//! **A stand-in until #52:** the page opens, says where it was opened on, and B closes it. Everything in this
//! folder is that ticket's to replace; what the rest of the Dashboard relies on is [`Entry`], [`MyGames::open`] and
//! the `Page` implementation.
use ui::{Button, Color, Press, Ui};

use crate::pages::{Around, Page, PageAction};
use crate::theme::{BODY, HEADING, TEXT_DIM};

/// How the page is opened: which of its items shows, and so where focus lands (docs/spec/my-games.md, 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// Games, focus on the grid's first game: Home's top bar button, the Guide's My games & apps.
    Games,
    /// Play history, focus on its first row: Home's Play history tile.
    PlayHistory,
    /// Full library: See all on a Group, on the tab of that Group's saved filter (`model::Filter::id`). None is
    /// plain Full library, as the nav opens it.
    FullLibrary { filter: Option<i64> },
    /// Manage's Queue (M4): Home's Downloads tile.
    Queue,
}

/// The page's fill (`PageBrush`).
const FILL: Color = Color::rgb(0x1F1F1F);

pub struct MyGames {
    entry: Entry,
}

impl MyGames {
    pub fn open(entry: Entry) -> Self {
        Self { entry }
    }
}

impl Page for MyGames {
    fn name(&self) -> &'static str {
        "MyGames"
    }

    fn focus(&self) -> String {
        format!("{:?}", self.entry)
    }

    fn press(&mut self, press: Press, _around: Around) -> Option<PageAction> {
        (press.button == Button::B && !press.repeat).then_some(PageAction::Close)
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        ui.fill(ui.screen(), FILL, 0.0);
        let item = match self.entry {
            Entry::Games => "Games",
            Entry::PlayHistory => "Play history",
            Entry::FullLibrary { .. } => "Full library",
            Entry::Queue => "Manage",
        };
        ui.label(75.0, 78.0, "My games & apps", &BODY.color(TEXT_DIM));
        ui.label(520.0, 72.0, item, &HEADING);
    }
}
