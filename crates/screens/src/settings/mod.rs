//! Settings (docs/spec/settings.md): the rail down the left, and beside it a category's boxes and their pages.
//!
//! This file is the shell: the rail, the header, and the panes that are up, the last showing. A category's boxes
//! are a pane, and so is every page a box opens in the boxes' place: each a struct that implements [`Pane`], keeps
//! its own focus, and is drawn from [`controls`]. B climbs one level at a time, a page to the box that opened it,
//! the category's boxes to the rail, the rail out of Settings; an entry point that opens deep climbs the same way
//! (section 8.2). Each category's boxes keep where focus was while the page is up; the page remembers nothing
//! between visits: it opens as its [`Entry`] says.
//!
//! **Adding a page:** a struct that implements [`Pane`], and a box that opens it on the category it belongs to
//! ([`categories`], [`boxes::Item::Opens`]). Nothing here changes.
pub mod about;
mod add;
mod added;
mod art;
mod boot_video;
pub mod boxes;
pub mod categories;
pub mod controls;
mod discord;
pub mod discord_web;
mod group;
pub mod groups;
mod guide;
pub mod helpers;
pub mod idle;
pub mod launchers;
pub mod options;
mod profiles;
pub mod set_up;
pub mod storage;
pub mod system;
mod whos_playing;

use ui::{Button, Color, Font, HAlign, Press, Rect, TextStyle, Ui, VAlign};

pub use self::boot_video::NONE_FOUND as NO_BOOT_VIDEOS;
use self::boxes::Boxes;
pub use self::categories::Category;
pub use self::options::{BootVideo, Change, Found, Options, Setting, ART_KEY, BOOT_VIDEO, LAST_UPDATE_CHECK};
use crate::hint_bar::{self, Hint};
use crate::keyboard;
use crate::pages::{Around, Dialog, Page, PageAction};
use crate::theme::ACCENT;
use crate::Effect;

/// How the page is opened: what shows, and so where focus lands and how many levels B climbs before the page
/// closes (docs/spec/settings.md, 8.2 and 16.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// General's boxes, focus on the rail: Home's top bar button, the Guide's Settings.
    Rail,
    /// Home's Settings tile, which in the Bundle is "Playnite settings" and opens that category on its first box.
    /// That category is gone: the tile opens the first there is, General, on its first box.
    Tile,
    /// Games & apps' boxes, focus on Groups: the Tile menu's Customise Home.
    CustomiseHome,
    /// Home's Discord tile while there is no Discord tab in the Guide: Account > Discord, focus on the Client ID's
    /// Type; B goes back to Account's boxes with focus on Discord.
    Discord,
    /// Manage Queue's row of a game stopped for want of space (#230): Games & apps > Storage; B goes straight back
    /// to the row, as the page was opened from there (`Settings::returns_at`); a page opened from Storage, closed,
    /// comes back to Storage first.
    Storage,
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

    /// Asked after each press and as the options change under it (`Dashboard::set_options`): for a pane whose focus
    /// log says how something the executable tells stands (Discord messages' session).
    fn tick(&mut self, _around: Around) {}

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
/// The first rail item's top, and the pitch from one to the next: 58 tall, 14 between.
const RAIL_TOP: f32 = 190.0;
const RAIL_PITCH: f32 = 72.0;
const SELECTED: Color = Color::rgb(0x262626);
const TITLE: TextStyle = TextStyle::new(Font::Display, 30.0, 400);
const ITEM: TextStyle = TextStyle::new(Font::Display, 24.0, 350);
const HEADER: TextStyle = TextStyle::new(Font::Display, 42.0, 350);
/// The header's category, before the page's name: bold, as the console's (docs/spec/settings.md, 3).
const HEADER_CATEGORY: TextStyle = TextStyle::new(Font::Display, 42.0, 700);
/// Between the category and the page's name in the header.
const HEADER_GAP: f32 = 20.0;

pub struct Settings {
    /// Focus is on the rail's item; else on the pane that shows.
    on_rail: bool,
    /// The category showing, the rail's focused or selected item: its place in [`Category::ALL`].
    category: usize,
    /// Every category's boxes, each keeping where its focus was while the page is up (16.3).
    categories: Vec<Boxes>,
    /// Each page opened over the category's boxes, from the one before it. The last shows.
    pages: Vec<Box<dyn Pane>>,
    /// Opened from a place that is not the Bundle's own entry (the Queue's Stopped row, #230): B on the page it
    /// opened on closes Settings, back to where the user came from, focus where they left it. The Bundle's entries
    /// climb page, boxes, rail, out (docs/spec/settings.md, 8.2): `None`.
    returns_at: Option<usize>,
}

impl Settings {
    /// Settings as `entry` says, from what is `around`: Storage's page reads the library as it opens.
    pub fn open(entry: Entry, around: Around) -> Self {
        match entry {
            Entry::Storage => {
                let (row, column) = categories::STORAGE;
                let mut settings = Self::on(Category::GamesAndApps);
                let boxes = Category::GamesAndApps.boxes().with_focus(row, column);
                settings.categories[Category::GamesAndApps.index()] = boxes;
                settings.pages.push(Box::new(storage::Storage::open(around)));
                settings.returns_at = Some(settings.pages.len());
                settings
            }
            Entry::Rail => Self { on_rail: true, ..Self::on(Category::General) },
            Entry::Tile => Self::on(Category::General),
            Entry::Discord => {
                let (row, column) = categories::DISCORD;
                let mut settings = Self::on(Category::Account);
                settings.categories[Category::Account.index()] = Category::Account.boxes().with_focus(row, column);
                settings.pages.push(Box::new(discord::Discord::default()));
                settings
            }
            // Groups, the box that customises Home (docs/spec/settings.md, 16.3).
            Entry::CustomiseHome => {
                let (row, column) = categories::GROUPS;
                let mut settings = Self::on(Category::GamesAndApps);
                let boxes = Category::GamesAndApps.boxes().with_focus(row, column);
                settings.categories[Category::GamesAndApps.index()] = boxes;
                settings
            }
        }
    }

    /// Settings on this category's boxes, focus on its first.
    fn on(category: Category) -> Self {
        let categories = Category::ALL.iter().map(|category| category.boxes()).collect();
        Self { on_rail: false, category: category.index(), categories, pages: Vec::new(), returns_at: None }
    }

    /// Settings with this page showing, as if a box of this category had opened it: B goes back to its boxes.
    pub fn showing(category: Category, pane: Box<dyn Pane>) -> Self {
        let mut settings = Self::on(category);
        settings.pages.push(pane);
        settings
    }

    /// The pane that shows: the last page, or the category's boxes.
    fn pane(&self) -> &dyn Pane {
        match self.pages.last() {
            Some(page) => page.as_ref(),
            None => &self.categories[self.category],
        }
    }

    fn pane_mut(&mut self) -> &mut dyn Pane {
        match self.pages.last_mut() {
            Some(page) => page.as_mut(),
            None => &mut self.categories[self.category],
        }
    }

    fn rail(&self, ui: &mut Ui) {
        ui.label(RAIL_LEFT, 78.0, "Settings", &TITLE);
        let item = |at: usize| Rect::new(0.0, RAIL_TOP + RAIL_PITCH * at as f32, RAIL, 58.0);
        let showing = item(self.category);
        if self.on_rail {
            // The darker green over the whole item, and the glow hugging it; no ring.
            controls::glow(ui, showing, 2.0);
            ui.fill(showing, ACCENT, 0.0);
        } else {
            // The category showing: lit, with a bar on its right edge, against the line.
            ui.fill(showing, SELECTED, 0.0);
            ui.fill(Rect::new(RAIL - 6.0, showing.y, 6.0, showing.h), ACCENT, 0.0);
        }
        for (at, category) in Category::ALL.iter().enumerate() {
            let item = item(at);
            let name = Rect::new(RAIL_LEFT, item.y, RAIL - 43.0 - RAIL_LEFT, item.h);
            ui.paragraph(name, category.name(), &ITEM, HAlign::Left, VAlign::Center, 1);
        }
        // The line is over the focused item's glow, which shows again beyond it.
        ui.fill(Rect::new(RAIL, 0.0, DIVIDER, ui.screen().h), controls::LINE, 0.0);
    }

    /// The header: the category in bold, then the page's name where a page shows, as the console's.
    fn header(&self, ui: &mut Ui) {
        let category = Category::ALL[self.category].name();
        let top = 72.0;
        let header = Rect::new(controls::LEFT, top, controls::RIGHT - controls::LEFT, 60.0);
        let bold = ui.paragraph(header, category, &HEADER_CATEGORY, HAlign::Left, VAlign::Top, 1).width;
        if let Some(page) = self.pages.last() {
            let x = controls::LEFT + bold + HEADER_GAP;
            let rest = Rect::new(x, top, (controls::RIGHT - x).max(0.0), 60.0);
            ui.paragraph(rest, &page.header(), &HEADER, HAlign::Left, VAlign::Top, 1);
        }
    }
}

impl Page for Settings {
    fn name(&self) -> &'static str {
        "Settings"
    }

    fn tick(&mut self, around: Around) {
        self.pane_mut().tick(around);
    }

    fn focus(&self) -> String {
        if self.on_rail {
            format!("Rail: {}", Category::ALL[self.category].name())
        } else {
            self.pane().focus()
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        if press.repeat && !press.button.is_direction() {
            return None;
        }
        if self.on_rail {
            match press.button {
                // Into the category, on the box focus was on when it was left, its first the first time.
                Button::Right | Button::A => self.on_rail = false,
                Button::B => return Some(PageAction::Close),
                // Along the rail, stopping at its ends: the category shows at once.
                Button::Up if self.category > 0 => self.category -= 1,
                Button::Down if self.category + 1 < Category::ALL.len() => self.category += 1,
                // Everything else is swallowed.
                _ => {}
            }
            return None;
        }
        match self.pane_mut().press(press, around) {
            Step::Stay => {}
            // Opened from elsewhere on this page: back there.
            Step::Back if self.returns_at == Some(self.pages.len()) => return Some(PageAction::Close),
            Step::Back if !self.pages.is_empty() => {
                self.pages.pop();
            }
            // From the category's boxes to the rail, the boxes keeping where focus was.
            Step::Back => self.on_rail = true,
            Step::Open(pane) => self.pages.push(pane),
            Step::Do(action) => return Some(action),
            Step::BackWith(action) => {
                self.pages.pop();
                return Some(action);
            }
        }
        None
    }

    fn state(&self) -> String {
        if self.on_rail {
            String::new()
        } else {
            self.pane().state()
        }
    }

    fn hints(&self, around: Around) -> &'static [Hint] {
        let pane = self.pane();
        if !self.on_rail && pane.dialog().is_none() {
            pane.hints(around)
        } else {
            hint_bar::NONE
        }
    }

    fn dialog(&self) -> Option<Dialog> {
        if self.on_rail {
            None
        } else {
            self.pane().dialog()
        }
    }

    fn dismiss_dialog(&mut self) -> bool {
        self.pane_mut().dismiss_dialog();
        true
    }

    fn library_changed(&mut self, around: Around) {
        let mut opened = self.categories[self.category].library_changed(around);
        for page in &mut self.pages {
            opened = page.library_changed(around);
        }
        if let Some(pane) = opened.filter(|_| !self.on_rail) {
            self.pages.push(pane);
        }
    }

    fn entry_text(&self) -> String {
        self.pane().entry_text()
    }

    fn entry(&mut self, event: keyboard::Event, around: Around) {
        self.pane_mut().entry(event, around);
    }

    fn asks(&mut self) -> Vec<Effect> {
        let mut asks = self.categories[self.category].asks();
        asks.extend(self.pages.iter_mut().flat_map(|pane| pane.asks()));
        // A page that is done steps back, once what it asked has been taken.
        if self.pages.last_mut().is_some_and(|pane| pane.closes()) {
            self.pages.pop();
        }
        asks
    }

    fn draw(&mut self, ui: &mut Ui, around: Around) {
        ui.fill(ui.screen(), controls::PAGE, 0.0);
        self.rail(ui);
        self.header(ui);
        let focused = !self.on_rail;
        self.pane_mut().draw(ui, around, focused);
    }
}
