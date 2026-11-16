//! Games & apps > Add a game, and Add an app (docs/spec/apps.md, section 4.2): the program's path, its name and its
//! picture, each a field A opens text entry on (the Console keyboard, or a keyboard's keys); then Add, which is
//! there to press once a path and a name are. Add an app goes on with the Microsoft Store apps installed on this PC,
//! a Row each, ticked while it is an App: A ticks one (it is added) or unticks it (it is removed).
//!
//! No file picker: a pad has no use for one. A path typed is taken as it is: the shell refuses one with no program
//! there, and says so in a notification.
use model::by_hand::{self, typed_path};
use model::{Added as Item, App, ByHand, StoreApp};
use ui::{Button, Press, Rect, Ui};

use super::controls::{self, FIELD_HEIGHT, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH, TOP};
use super::{Pane, Step};
use crate::keyboard::{self, Event};
use crate::pages::{Around, Open, PageAction};
use crate::theme::BODY;
use crate::Effect;

/// From one control's name to the next: the name's line, 4, the control, and 40.
const PITCH: f32 = 34.58 + 4.0 + FIELD_HEIGHT + 40.0;
const UNDER_NAME: f32 = 34.58 + 4.0;
/// The Store apps' heading is this far under Add, its line under it, and their Rows 26 under that.
const STORE_UNDER: f32 = 56.0;

/// What the page adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    Game,
    App,
}

/// A control of the page, top to bottom; the Store apps' Rows follow Add.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Path,
    Name,
    Picture,
    Add,
    Store(usize),
}

pub struct Add {
    what: What,
    path: String,
    name: String,
    picture: String,
    /// The control focus is on, as its place: 0 to 3, then the Store apps' Rows.
    focus: usize,
    scroll: f32,
}

impl Add {
    pub fn open(what: What) -> Self {
        Self { what, path: String::new(), name: String::new(), picture: String::new(), focus: 0, scroll: 0.0 }
    }

    /// The Microsoft Store apps the list shows: Add an app's only, and only once they have been listed.
    fn store<'a>(&self, around: Around<'a>) -> &'a [StoreApp] {
        match self.what {
            What::App => around.library.store_apps.as_deref().unwrap_or_default(),
            What::Game => &[],
        }
    }

    fn controls(&self, around: Around) -> Vec<Control> {
        let fields = [Control::Path, Control::Name, Control::Picture, Control::Add];
        fields.into_iter().chain((0..self.store(around).len()).map(Control::Store)).collect()
    }

    fn focused(&self, around: Around) -> Control {
        let controls = self.controls(around);
        controls[self.focus.min(controls.len() - 1)]
    }

    /// The caption over text entry for a field.
    fn caption(&self, control: Control) -> keyboard::Entry {
        match (control, self.what) {
            (Control::Path, What::Game) => keyboard::Entry::new("The game's program").limit(260),
            (Control::Path, What::App) => keyboard::Entry::new("The app's program").limit(260),
            (Control::Picture, What::Game) => keyboard::Entry::new("Art for its tiles").limit(260),
            (Control::Picture, What::App) => keyboard::Entry::new("Its icon").limit(260),
            _ => super::added::NAME,
        }
    }

    /// The name over each field.
    fn label(&self, control: Control) -> &'static str {
        match (control, self.what) {
            (Control::Path, _) => "Program",
            (Control::Name, _) => "Name",
            (Control::Picture, What::Game) => "Art (optional)",
            (Control::Picture, What::App) => "Icon (optional)",
            _ => "",
        }
    }

    fn placeholder(&self, control: Control) -> &'static str {
        match control {
            Control::Path => r"Its full path, as C:\Games\Tern\tern.exe",
            Control::Name => "Its name in the library",
            _ => "A picture's full path",
        }
    }

    fn text(&self, control: Control) -> &str {
        match control {
            Control::Path => &self.path,
            Control::Name => &self.name,
            Control::Picture => &self.picture,
            _ => "",
        }
    }

    /// What Add asks for, once there is a path and a name.
    fn change(&self) -> Option<ByHand> {
        let path = typed_path(&self.path)?;
        let name = self.name.trim();
        if name.is_empty() {
            return None;
        }
        let name = name.to_string();
        let picture = typed_path(&self.picture);
        Some(match self.what {
            What::Game => ByHand::AddGame { path, name, art: picture },
            What::App => ByHand::AddApp { path, name, icon: picture },
        })
    }

    /// The App a Store app is now, if it is one.
    fn ticked<'a>(app: &StoreApp, around: Around<'a>) -> Option<&'a App> {
        around.library.apps.iter().find(|added| added.is_store() && added.target == app.aumid)
    }

    fn name_top(k: usize) -> f32 {
        TOP + PITCH * k as f32
    }

    /// Where control k is, before any scrolling: a field under its name, Add, a Store app's Row.
    fn rect(&self, ui: &mut Ui, k: usize, control: Control) -> Rect {
        match control {
            Control::Add => Rect::new(LEFT, Self::name_top(3), controls::pane_button_width(ui, "Add"), FIELD_HEIGHT),
            Control::Store(i) => Rect::new(LEFT, Self::store_top() + ROW_PITCH * i as f32, ROW_WIDTH, ROW_HEIGHT),
            _ => Rect::new(LEFT, Self::name_top(k) + UNDER_NAME, ROW_WIDTH, FIELD_HEIGHT),
        }
    }

    /// The Store apps' heading's top, and their first Row's.
    fn store_heading() -> f32 {
        Self::name_top(3) + FIELD_HEIGHT + STORE_UNDER
    }

    fn store_top() -> f32 {
        Self::store_heading() + 34.58 + 4.0 + 2.0 * 34.58 + 26.0
    }
}

impl Pane for Add {
    fn header(&self) -> String {
        match self.what {
            What::Game => "Add a game".into(),
            What::App => "Add an app".into(),
        }
    }

    /// `AddGame: Path`, `AddApp: Add`, `AddApp: Store(Calculator)`. The focus log has no library: a Store app's Row
    /// is said by its place, and the state says which app it is.
    fn focus(&self) -> String {
        let page = match self.what {
            What::Game => "AddGame",
            What::App => "AddApp",
        };
        let control = match self.focus {
            0 => "Path".to_string(),
            1 => "Name".to_string(),
            2 => "Picture".to_string(),
            3 => "Add".to_string(),
            i => format!("Store({})", i - 4),
        };
        format!("{page}: {control}")
    }

    /// What is typed so far, and whether Add would add it: `path "C:\Tools\tern.exe", name "Tern", picture "",
    /// ready`.
    fn state(&self) -> String {
        let ready = if self.change().is_some() { "ready" } else { "not ready" };
        format!("path \"{}\", name \"{}\", picture \"{}\", {ready}", self.path, self.name, self.picture)
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        let last = self.controls(around).len() - 1;
        match press.button {
            Button::Up => self.focus = self.focus.min(last).saturating_sub(1),
            Button::Down => self.focus = (self.focus + 1).min(last),
            Button::B | Button::Left => return Step::Back,
            Button::A => match self.focused(around) {
                Control::Add => {
                    // Not there to press until there is a path and a name.
                    if let Some(change) = self.change() {
                        return Step::BackWith(PageAction::Effect(Effect::ByHand(change)));
                    }
                }
                Control::Store(i) => {
                    let app = &self.store(around)[i];
                    let change = match Self::ticked(app, around) {
                        Some(added) => ByHand::Remove(Item::App(added.id)),
                        None => ByHand::AddStoreApp(app.clone()),
                    };
                    return Step::Do(PageAction::Effect(Effect::ByHand(change)));
                }
                field => return Step::Do(PageAction::Open(Open::TextEntry(self.caption(field)))),
            },
            _ => {}
        }
        Step::Stay
    }

    fn entry_text(&self) -> String {
        match self.focus {
            0 => self.path.clone(),
            1 => self.name.clone(),
            2 => self.picture.clone(),
            _ => String::new(),
        }
    }

    /// A text kept goes in its field. A path kept with no name yet offers its program's name.
    fn entry(&mut self, event: Event, _around: Around) {
        let Event::Kept(text) = event else { return };
        match self.focus {
            0 => {
                self.path = text.trim().to_string();
                if self.name.trim().is_empty() {
                    self.name = typed_path(&self.path).map(|path| by_hand::name_of(&path)).unwrap_or_default();
                }
            }
            1 => self.name = text.trim().to_string(),
            2 => self.picture = text.trim().to_string(),
            _ => {}
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let controls = self.controls(around);
        let at = self.focus.min(controls.len() - 1);
        let rects: Vec<Rect> = controls.iter().enumerate().map(|(k, &control)| self.rect(ui, k, control)).collect();
        let lit = rects[at];
        let top = match controls[at] {
            Control::Store(_) => lit.y,
            _ => Self::name_top(at.min(3)),
        };
        let store = self.store(around);
        let mut scroll = self.scroll;
        let this = &*self;
        controls::scrolled(ui, &mut scroll, TOP, top, lit.bottom(), |ui| {
            for (k, &control) in controls.iter().enumerate() {
                let (rect, on) = (rects[k], focused && k == at);
                match control {
                    Control::Add => controls::pane_button(ui, rect, "Add", this.change().is_some(), on),
                    Control::Store(i) => {
                        let ticked = Self::ticked(&store[i], around).is_some();
                        controls::row(ui, rect, &store[i].name, ticked, on);
                    }
                    field => {
                        controls::label(ui, LEFT, Self::name_top(k), this.label(field));
                        controls::text_field(ui, rect, this.text(field), this.placeholder(field), on);
                    }
                }
            }
            if this.what == What::App {
                let heading = Self::store_heading();
                ui.label(LEFT, heading, "Microsoft Store apps", &BODY);
                let line = match &around.library.store_apps {
                    None => "The Microsoft Store apps on this PC show up here once Windows has listed them.",
                    Some(apps) if apps.is_empty() => "There are no Microsoft Store apps on this PC.",
                    Some(_) => "Tick the ones to start from My games & apps.",
                };
                controls::description(ui, LEFT, heading + 34.58 + 4.0, ROW_WIDTH, line);
            }
        });
        self.scroll = scroll;
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn add_asks_once_there_is_a_path_and_a_name_and_a_path_offers_its_name() {
        let mut page = Add::open(What::Game);
        assert_eq!(page.change(), None);
        let library = model::Library::default();
        let options = crate::settings::Options::default();
        let around = Around { library: &library, playing: None, options, now: std::time::Instant::now() };
        page.entry(Event::Kept(" \"C:\\Games\\Tern\\tern.exe\" ".into()), around);
        assert_eq!(page.name, "tern");
        assert_eq!(
            page.change(),
            Some(ByHand::AddGame { path: PathBuf::from(r"C:\Games\Tern\tern.exe"), name: "tern".into(), art: None })
        );
        // A name typed is kept; another path does not change it.
        page.focus = 1;
        page.entry(Event::Kept("Tern ".into()), around);
        page.focus = 0;
        page.entry(Event::Kept(r"C:\Games\Tern\tern2.exe".into()), around);
        page.focus = 2;
        page.entry(Event::Kept(r"C:\Art\tern.png".into()), around);
        assert_eq!(
            page.change(),
            Some(ByHand::AddGame {
                path: PathBuf::from(r"C:\Games\Tern\tern2.exe"),
                name: "Tern".into(),
                art: Some(PathBuf::from(r"C:\Art\tern.png"))
            })
        );
        // A name of nothing: not ready.
        page.focus = 1;
        page.entry(Event::Kept("   ".into()), around);
        assert_eq!(page.change(), None);
        // Dropped changes nothing.
        page.entry(Event::Dropped("x".into()), around);
        assert!(page.name.is_empty());
    }
}
