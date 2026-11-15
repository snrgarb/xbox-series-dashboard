//! Settings (docs/spec/settings.md): the rail down the left, and beside it a category's boxes and their pages.
//!
//! This file is the shell: the rail, the header, and the panes that are up, the last showing. A category's boxes
//! are a pane, and so is every page a box opens in the boxes' place: each a struct that implements [`Pane`], keeps
//! its own focus, and is drawn from [`controls`]. B climbs one level at a time, a page to the box that opened it,
//! the category's boxes to the rail, the rail out of Settings; an entry point that opens deep climbs the same way
//! (section 8.2). The page remembers nothing between visits: it opens as its [`Entry`] says.
//!
//! **Adding a page:** a struct that implements [`Pane`], and a box that opens it on the pane it belongs to
//! ([`boxes::Item::Opens`]). Nothing here changes.
pub mod boxes;
pub mod controls;
mod guide;
pub mod options;
mod theme_settings;

use ui::{Button, Color, Font, HAlign, Press, Rect, TextStyle, Ui, VAlign};

pub use self::options::{Change, Options, Setting};
use crate::pages::{Around, Page, PageAction};
use crate::theme::ACCENT;

/// How the page is opened: what shows, and so where focus lands and how many levels B climbs before the page
/// closes (docs/spec/settings.md, 8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// Theme settings' boxes, focus on the rail: Home's top bar button, the Guide's Settings.
    Rail,
    /// Home's Settings tile, which in the Bundle is "Playnite settings" and opens that category on its first box.
    /// That category is gone: the tile opens the one there is, Theme settings, on its first box.
    Tile,
    /// Personalisation's page, focus on its first box: the Tile menu's Customise Home.
    Personalisation,
    /// Home's Discord tile while there is no Discord tab in the Guide. Discord's page comes with M6; until then
    /// this is as [`Entry::Tile`].
    Discord,
}

/// What a press on a pane came to.
pub enum Step {
    /// Focus moved on the pane, or nothing happened.
    Stay,
    /// Back one level: B, or Left from a pane's first column.
    Back,
    /// A page in this pane's place. B on it comes back to this pane as it is, focus on the box that opened it.
    Open(Box<dyn Pane>),
    /// Something for the Dashboard: an option changed (`Effect::Set`), or another page over Settings.
    Do(PageAction),
}

/// What shows right of the rail: a category's boxes, or a page a box opened in their place.
pub trait Pane {
    /// The header over the pane: the only sign of how deep the page is.
    fn header(&self) -> String;

    /// Where focus is, for the focus log: the pane's name, then the control ("Personalisation: Reduce motion").
    fn focus(&self) -> String;

    /// A press while the pane shows. A held direction comes with `press.repeat`; nothing else is repeated.
    fn press(&mut self, press: Press, around: Around) -> Step;

    /// Draws the pane in the pane row: boxes from [`controls::BOXES_TOP`], labels and fields from
    /// [`controls::TOP`], both from [`controls::LEFT`]. `focused` is false while focus is on the rail: nothing on
    /// the pane then has a ring.
    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool);
}

/// The rail's width, and the line's between it and the content.
const RAIL: f32 = 441.0;
const DIVIDER: f32 = 2.0;
/// The rail's text starts here, and its items' names end 43 short of its edge.
const RAIL_LEFT: f32 = 75.0;
/// The rail's one category, as the Bundle names it.
const CATEGORY: &str = "Theme settings";
const SELECTED: Color = Color::rgb(0x262626);
const TITLE: TextStyle = TextStyle::new(Font::Display, 30.0, 400);
const ITEM: TextStyle = TextStyle::new(Font::Display, 24.0, 350);
const HEADER: TextStyle = TextStyle::new(Font::Display, 42.0, 350);

pub struct Settings {
    /// Focus is on the rail's item; else on the last pane.
    on_rail: bool,
    /// The category's boxes, then each page opened from the one before it. The last shows.
    panes: Vec<Box<dyn Pane>>,
}

impl Settings {
    pub fn open(entry: Entry) -> Self {
        let boxes: Box<dyn Pane> = Box::new(theme_settings::theme_settings());
        match entry {
            Entry::Rail => Self { on_rail: true, panes: vec![boxes] },
            Entry::Tile | Entry::Discord => Self { on_rail: false, panes: vec![boxes] },
            // Personalisation is Theme settings' first box, where focus is as its boxes are made.
            Entry::Personalisation => Self::showing(Box::new(theme_settings::personalisation())),
        }
    }

    /// Settings with this page showing, as if a box of Theme settings had opened it: B goes back to those boxes.
    pub fn showing(pane: Box<dyn Pane>) -> Self {
        Self { on_rail: false, panes: vec![Box::new(theme_settings::theme_settings()), pane] }
    }

    fn rail(&self, ui: &mut Ui) {
        ui.label(RAIL_LEFT, 78.0, "Settings", &TITLE);
        let item = Rect::new(0.0, 190.0, RAIL, 58.0);
        if self.on_rail {
            // The darker green over the whole item, and the glow hugging it; no ring.
            controls::glow(ui, item, 2.0);
            ui.fill(item, ACCENT, 0.0);
        } else {
            // The category showing: lit, with a bar on its right edge, against the line.
            ui.fill(item, SELECTED, 0.0);
            ui.fill(Rect::new(RAIL - 6.0, item.y, 6.0, item.h), ACCENT, 0.0);
        }
        let name = Rect::new(RAIL_LEFT, item.y, RAIL - 43.0 - RAIL_LEFT, item.h);
        ui.paragraph(name, CATEGORY, &ITEM, HAlign::Left, VAlign::Center, 1);
        // The line is over the focused item's glow, which shows again beyond it.
        ui.fill(Rect::new(RAIL, 0.0, DIVIDER, ui.screen().h), controls::LINE, 0.0);
    }
}

impl Page for Settings {
    fn name(&self) -> &'static str {
        "Settings"
    }

    fn focus(&self) -> String {
        match self.panes.last() {
            Some(pane) if !self.on_rail => pane.focus(),
            _ => format!("Rail: {CATEGORY}"),
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        if press.repeat && !press.button.is_direction() {
            return None;
        }
        if self.on_rail {
            match press.button {
                // Into the category, on its first box always.
                Button::Right | Button::A => self.on_rail = false,
                Button::B => return Some(PageAction::Close),
                // One item: Up and Down have nowhere to go. Everything else is swallowed.
                _ => {}
            }
            return None;
        }
        match self.panes.last_mut()?.press(press, around) {
            Step::Stay => {}
            Step::Back if self.panes.len() > 1 => {
                self.panes.pop();
            }
            // From the category's boxes to the rail, which forgets where focus was on them.
            Step::Back => {
                self.panes = vec![Box::new(theme_settings::theme_settings())];
                self.on_rail = true;
            }
            Step::Open(pane) => self.panes.push(pane),
            Step::Do(action) => return Some(action),
        }
        None
    }

    fn draw(&mut self, ui: &mut Ui, around: Around) {
        ui.fill(ui.screen(), controls::PAGE, 0.0);
        self.rail(ui);
        let Some(pane) = self.panes.last_mut() else { return };
        let header = Rect::new(controls::LEFT, 72.0, controls::RIGHT - controls::LEFT, 60.0);
        ui.paragraph(header, &pane.header(), &HEADER, HAlign::Left, VAlign::Top, 1);
        pane.draw(ui, around, !self.on_rail);
    }
}
