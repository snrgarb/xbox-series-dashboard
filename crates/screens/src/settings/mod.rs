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
mod add;
mod added;
mod art;
mod boot_video;
pub mod boxes;
pub mod controls;
mod group;
pub mod groups;
mod guide;
pub mod idle;
pub mod options;
mod profiles;
mod theme_settings;

use ui::{Button, Color, Font, HAlign, Press, Rect, TextStyle, Ui, VAlign};

pub use self::boot_video::{ABOUT as BOOT_VIDEO_ABOUT, NONE_FOUND as NO_BOOT_VIDEOS};
pub use self::options::{BootVideo, Change, Found, Options, Setting, ART_KEY, BOOT_VIDEO};
use crate::hint_bar::{self, Hint};
use crate::keyboard;
use crate::pages::{Around, Dialog, Page, PageAction};
use crate::theme::ACCENT;
use crate::Effect;

/// How the page is opened: what shows, and so where focus lands and how many levels B climbs before the page
/// closes (docs/spec/settings.md, 8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// Theme settings' boxes, focus on the rail: Home's top bar button, the Guide's Settings.
    Rail,
    /// Home's Settings tile, which in the Bundle is "Playnite settings" and opens that category on its first box.
    /// That category is gone: the tile opens the one there is, Theme settings, on its first box.
    Tile,
    /// Personalisation's page, focus on Groups: the Tile menu's Customise Home.
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
    /// Back one level, and this for the Dashboard: a page that is done once it asks (Add a game's Add).
    BackWith(PageAction),
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

    /// What the focus log says of the pane beyond where its focus is: what a press changes without moving focus
    /// (whether the Group focus is on is shown).
    fn state(&self) -> String {
        String::new()
    }

    /// The hint bar's hints with focus where it is. Settings shows none (section 10.5) but on the Groups page.
    fn hints(&self, _around: Around) -> &'static [Hint] {
        hint_bar::NONE
    }

    /// A dialog the pane shows over the page (the menu of a Group), and of which kind, while one is up.
    fn dialog(&self) -> Option<Dialog> {
        None
    }

    /// Its dialog goes with no answer: the Guide has taken the screen.
    fn dismiss_dialog(&mut self) {}

    /// A newer snapshot of the library is in `around`: what the pane keeps of the old one (the Group focus is on)
    /// it finds again. A page to open over it, if the snapshot brought what it was waiting for (a Group just made,
    /// whose page opens).
    fn library_changed(&mut self, _around: Around) -> Option<Box<dyn Pane>> {
        None
    }

    /// The text that text entry, opened by this pane, starts from.
    fn entry_text(&self) -> String {
        String::new()
    }

    /// What text entry over the page had to say, while this pane shows.
    fn entry(&mut self, _event: keyboard::Event, _around: Around) {}

    /// What the pane asks of the shell that no press answered: a name text entry kept, to save.
    fn asks(&mut self) -> Vec<Effect> {
        Vec::new()
    }

    /// Whether what text entry had to say finished the pane, which then steps back as B would: the Picture page,
    /// once a picture's path is kept.
    fn closes(&mut self) -> bool {
        false
    }
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
            // Personalisation is Theme settings' first box, where focus is as its boxes are made; on its own boxes
            // focus is on Groups, the box that customises Home (docs/spec/settings.md, 16.3).
            Entry::Personalisation => Self::showing(Box::new(theme_settings::personalisation().with_focus(1, 0))),
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
            Step::BackWith(action) => {
                if self.panes.len() > 1 {
                    self.panes.pop();
                }
                return Some(action);
            }
        }
        None
    }

    fn state(&self) -> String {
        match self.panes.last() {
            Some(pane) if !self.on_rail => pane.state(),
            _ => String::new(),
        }
    }

    fn hints(&self, around: Around) -> &'static [Hint] {
        match self.panes.last() {
            Some(pane) if !self.on_rail && pane.dialog().is_none() => pane.hints(around),
            _ => hint_bar::NONE,
        }
    }

    fn dialog(&self) -> Option<Dialog> {
        self.panes.last().filter(|_| !self.on_rail).and_then(|pane| pane.dialog())
    }

    fn dismiss_dialog(&mut self) -> bool {
        if let Some(pane) = self.panes.last_mut() {
            pane.dismiss_dialog();
        }
        true
    }

    fn library_changed(&mut self, around: Around) {
        let last = self.panes.len().saturating_sub(1);
        let mut opened = None;
        for (at, pane) in self.panes.iter_mut().enumerate() {
            let over = pane.library_changed(around);
            if at == last {
                opened = over;
            }
        }
        if let Some(pane) = opened.filter(|_| !self.on_rail) {
            self.panes.push(pane);
        }
    }

    fn entry_text(&self) -> String {
        self.panes.last().map(|pane| pane.entry_text()).unwrap_or_default()
    }

    fn entry(&mut self, event: keyboard::Event, around: Around) {
        if let Some(pane) = self.panes.last_mut() {
            pane.entry(event, around);
        }
    }

    fn asks(&mut self) -> Vec<Effect> {
        let asks = self.panes.iter_mut().flat_map(|pane| pane.asks()).collect();
        // A pane that is done steps back, once what it asked has been taken.
        if self.panes.len() > 1 && self.panes.last_mut().is_some_and(|pane| pane.closes()) {
            self.panes.pop();
        }
        asks
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
