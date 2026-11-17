//! Games & apps > Games & apps (docs/spec/apps.md, section 4): Add a game and Add an app, each opening its page
//! in this one's place, then a Row for every game added by hand and every App. A or Menu on one of those opens its
//! menu, the Tile menu's panel over the dimmed page: Rename, which opens text entry on its name, and Remove, which
//! asks "Remove X?" first.
//!
//! Every change is asked of the shell as an `Effect::ByHand` and carried out on the library database; the page shows
//! what the next snapshot says (`Pane::library_changed`), focus staying on the Row it was on wherever that has gone.
use std::sync::atomic::{AtomicU32, Ordering};

use model::by_hand::MANUAL;
use model::{Added as Item, ByHand, Library};
use ui::{Button, Press, Rect, Ui};

use super::add::{Add, What};
use super::controls::{self, glyph, BOXES_TOP, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH};
use super::{Pane, Step};
use crate::hint_bar::{self, Hint};
use crate::keyboard::{self, Event};
use crate::menu_panel::{self, icon, Answer, MenuPanel, Row as MenuRow, DIM, FLY};
use crate::pages::{Around, Dialog, Open, PageAction};
use crate::Effect;

/// The box on the Games & apps category, and the page's header.
pub const TITLE: &str = "Games & apps";
/// The line over the Rows.
const ABOUT: &str = "Add games that no store brings in, and apps to start from My games & apps. Press A on one to \
                     rename or remove it.";
/// The first Row's top: under the line's two lines and 26 more.
const ROWS_TOP: f32 = BOXES_TOP + 2.0 * 34.58 + 26.0;
/// The name text entry asks for.
pub const NAME: keyboard::Entry = keyboard::Entry::new("Name").limit(60);
/// The glyphs of a game's Row and an App's.
const GAME: char = '\u{E7FC}';
const APP: char = '\u{E71D}';

/// A Row of the page, top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    AddGame,
    AddApp,
    Game { id: i64, name: String },
    App { id: i64, name: String },
}

impl Line {
    fn item(&self) -> Option<(Item, &str)> {
        match self {
            Line::Game { id, name } => Some((Item::Game(*id), name)),
            Line::App { id, name } => Some((Item::App(*id), name)),
            Line::AddGame | Line::AddApp => None,
        }
    }

    /// The Row's name, and its glyph.
    fn face(&self) -> (&str, char) {
        match self {
            Line::AddGame => ("Add a game", glyph::ADD),
            Line::AddApp => ("Add an app", glyph::ADD),
            Line::Game { name, .. } => (name, GAME),
            Line::App { name, .. } => (name, APP),
        }
    }

    /// What the focus log says of it: `Add a game`, `Game Moss`, `App Notepad`.
    fn said(&self) -> String {
        match self {
            Line::AddGame | Line::AddApp => self.face().0.to_string(),
            Line::Game { name, .. } => format!("Game {name}"),
            Line::App { name, .. } => format!("App {name}"),
        }
    }
}

/// The Rows for the library as it is: the two that add, the games added by hand by name (hidden ones too: this is
/// where they are removed), then the Apps in the order they were added.
fn lines(library: &Library) -> Vec<Line> {
    let mut games: Vec<_> = library.games.iter().chain(&library.hidden).filter(|game| game.source == MANUAL).collect();
    games.sort_by_cached_key(|game| (crate::my_games::sort_key(&game.name), game.id));
    let games = games.into_iter().map(|game| Line::Game { id: game.id, name: game.name.clone() });
    let apps = library.apps.iter().map(|app| Line::App { id: app.id, name: app.name.clone() });
    [Line::AddGame, Line::AddApp].into_iter().chain(games).chain(apps).collect()
}

/// What a row of the menu does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Rename,
    Remove,
}

/// The answers to "Remove X?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Remove,
    Cancel,
}

/// The panel in the middle of the screen: the menu, or the question in its place.
#[derive(Clone)]
enum Middle {
    Menu(MenuPanel<Choice>),
    Prompt(MenuPanel<Confirm>),
}

impl Middle {
    fn draw(&self, ui: &mut Ui, fly: f32, opacity: f32) {
        match self {
            Middle::Menu(panel) => panel.draw_flown(ui, fly, opacity),
            Middle::Prompt(panel) => panel.draw_flown(ui, fly, opacity),
        };
    }
}

/// A Row's menu while it is up.
struct Menu {
    item: Item,
    name: String,
    panel: MenuPanel<Choice>,
    prompt: Option<MenuPanel<Confirm>>,
}

impl Menu {
    fn middle(&self) -> Middle {
        match &self.prompt {
            Some(prompt) => Middle::Prompt(prompt.clone()),
            None => Middle::Menu(self.panel.clone()),
        }
    }
}

/// Each panel that takes the middle flies up afresh: a number for every one.
static FLIGHTS: AtomicU32 = AtomicU32::new(0);

fn flight() -> u32 {
    FLIGHTS.fetch_add(1, Ordering::Relaxed)
}

pub struct Added {
    lines: Vec<Line>,
    /// The Row focus is on.
    at: usize,
    menu: Option<Menu>,
    /// The panel in the middle now, and its flight.
    flying: u32,
    /// A panel on its way out, and its flight.
    leaving: Option<(Middle, u32)>,
    scroll: f32,
    /// A new name text entry kept, to be asked of the shell.
    asks: Vec<Effect>,
}

impl Added {
    /// The page as the library has it now, focus on Add a game.
    pub fn open(around: Around) -> Self {
        Self {
            lines: lines(around.library),
            at: 0,
            menu: None,
            flying: 0,
            leaving: None,
            scroll: 0.0,
            asks: Vec::new(),
        }
    }

    fn focused(&self) -> &Line {
        &self.lines[self.at.min(self.lines.len() - 1)]
    }

    fn open_menu(&mut self) {
        let Some((item, name)) = self.focused().item() else { return };
        let rows = vec![
            MenuRow::new(icon::EDIT, "Rename", Choice::Rename),
            MenuRow::new(icon::REMOVE_GAME, "Remove", Choice::Remove),
        ];
        let panel = MenuPanel::menu(name, rows);
        self.menu = Some(Menu { item, name: name.to_string(), panel, prompt: None });
        self.flying = flight();
        self.leaving = None;
    }

    fn close_menu(&mut self) {
        if let Some(menu) = self.menu.take() {
            self.leaving = Some((menu.middle(), self.flying));
        }
    }

    /// A press while the menu is up: every button is the menu's.
    fn menu_press(&mut self, press: Press) -> Step {
        let Some(menu) = self.menu.as_mut() else { return Step::Stay };
        let item = menu.item;
        if let Some(prompt) = menu.prompt.as_mut() {
            match prompt.press(press) {
                Answer::Chose(Confirm::Remove) => {
                    self.close_menu();
                    return Step::Do(PageAction::Effect(Effect::ByHand(ByHand::Remove(item))));
                }
                // Back to the menu, which flies up again, focus on Remove.
                Answer::Chose(Confirm::Cancel) | Answer::Declined => {
                    menu.prompt = None;
                    self.flying = flight();
                }
                Answer::Moved | Answer::Nothing => {}
            }
            return Step::Stay;
        }
        match menu.panel.press(press) {
            // The menu goes as text entry comes; the name is kept or dropped there.
            Answer::Chose(Choice::Rename) => {
                self.menu = None;
                Step::Do(PageAction::Open(Open::TextEntry(NAME)))
            }
            Answer::Chose(Choice::Remove) => {
                let answers = vec![
                    MenuRow::new(icon::REMOVE_GAME, "Remove", Confirm::Remove),
                    MenuRow::new(icon::CANCEL, "Cancel", Confirm::Cancel),
                ];
                menu.prompt = Some(MenuPanel::prompt(&format!("Remove {}?", menu.name), None, answers));
                self.flying = flight();
                Step::Stay
            }
            Answer::Declined => {
                self.close_menu();
                Step::Stay
            }
            Answer::Moved | Answer::Nothing => Step::Stay,
        }
    }

    fn row(i: usize) -> Rect {
        Rect::new(LEFT, ROWS_TOP + ROW_PITCH * i as f32, ROW_WIDTH, ROW_HEIGHT)
    }
}

impl Pane for Added {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `GamesAndApps: Add a game`, `GamesAndApps: App Notepad`; with the menu up `GamesAndApps: Menu(Rename) of
    /// Notepad`, and with its question `GamesAndApps: Prompt(Remove) of Notepad`.
    fn focus(&self) -> String {
        fn row<T: Copy>(panel: &MenuPanel<T>) -> &str {
            panel.rows().get(panel.focus()).map_or("", |row| row.label.as_str())
        }
        match &self.menu {
            Some(menu) => match &menu.prompt {
                Some(prompt) => format!("GamesAndApps: Prompt({}) of {}", row(prompt), menu.name),
                None => format!("GamesAndApps: Menu({}) of {}", row(&menu.panel), menu.name),
            },
            None => format!("GamesAndApps: {}", self.focused().said()),
        }
    }

    /// How many of each there are: `1 game, 2 apps`. Nothing while the menu is up.
    fn state(&self) -> String {
        if self.menu.is_some() {
            return String::new();
        }
        let games = self.lines.iter().filter(|line| matches!(line, Line::Game { .. })).count();
        let apps = self.lines.iter().filter(|line| matches!(line, Line::App { .. })).count();
        let plural = |n: usize, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") };
        format!("{}, {}", plural(games, "game"), plural(apps, "app"))
    }

    fn press(&mut self, press: Press, _around: Around) -> Step {
        if self.menu.is_some() {
            return self.menu_press(press);
        }
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        match press.button {
            Button::Up => self.at = self.at.saturating_sub(1),
            Button::Down => self.at = (self.at + 1).min(self.lines.len() - 1),
            Button::B | Button::Left => return Step::Back,
            Button::A => match self.focused() {
                Line::AddGame => return Step::Open(Box::new(Add::open(What::Game))),
                Line::AddApp => return Step::Open(Box::new(Add::open(What::App))),
                _ => self.open_menu(),
            },
            Button::Menu => self.open_menu(),
            _ => {}
        }
        Step::Stay
    }

    fn hints(&self, _around: Around) -> &'static [Hint] {
        match self.focused().item() {
            Some(_) => hint_bar::MORE_OPTIONS,
            None => hint_bar::NONE,
        }
    }

    fn dialog(&self) -> Option<Dialog> {
        let menu = self.menu.as_ref()?;
        Some(if menu.prompt.is_some() { Dialog::Prompt } else { Dialog::Menu })
    }

    fn dismiss_dialog(&mut self) {
        self.menu = None;
        self.leaving = None;
    }

    /// Focus stays on its Row wherever it is now, else on the Row in its place; a menu whose Row has gone goes.
    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        let was = self.focused().clone();
        self.lines = lines(around.library);
        let same = |line: &Line| match (line, &was) {
            (Line::Game { id, .. }, Line::Game { id: was, .. }) | (Line::App { id, .. }, Line::App { id: was, .. }) => {
                id == was
            }
            (line, was) => line == was,
        };
        if let Some(at) = self.lines.iter().position(same) {
            self.at = at;
        }
        self.at = self.at.min(self.lines.len() - 1);
        if let Some(menu) = &mut self.menu {
            match self.lines.iter().find_map(|line| line.item().filter(|(item, _)| *item == menu.item)) {
                Some((_, name)) => menu.name = name.to_string(),
                None => self.menu = None,
            }
        }
        None
    }

    fn entry_text(&self) -> String {
        self.focused().item().map(|(_, name)| name.to_string()).unwrap_or_default()
    }

    /// A name kept is saved; one that is empty or all spaces, or the same, is not.
    fn entry(&mut self, event: Event, _around: Around) {
        let Event::Kept(text) = event else { return };
        let Some((item, name)) = self.focused().item() else { return };
        let new = text.trim();
        if !new.is_empty() && new != name {
            self.asks.push(Effect::ByHand(ByHand::Rename { item, name: new.to_string() }));
        }
    }

    fn asks(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.asks)
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, focused: bool) {
        let at = self.at.min(self.lines.len() - 1);
        let focused_row = Self::row(at);
        let lines = &self.lines;
        controls::scrolled(ui, &mut self.scroll, BOXES_TOP, focused_row.y, focused_row.bottom(), |ui| {
            controls::description(ui, LEFT, BOXES_TOP, ROW_WIDTH, ABOUT);
            let draw = |ui: &mut Ui, i: usize, focused: bool| {
                let (name, mark) = lines[i].face();
                controls::row_marked(ui, Self::row(i), name, Some(mark), focused);
            };
            for i in (0..lines.len()).filter(|&i| i != at || !focused) {
                draw(ui, i, false);
            }
            if focused {
                draw(ui, at, true);
            }
        });
        if let Some(menu) = &self.menu {
            ui.layer();
            ui.fill(ui.screen(), DIM, 0.0);
            let (fly, shown) = menu_panel::flight(ui, ("added menu", self.flying), false);
            menu.middle().draw(ui, fly, shown);
        } else if let Some((middle, flying)) = &self.leaving {
            ui.layer();
            let (fly, shown) = menu_panel::flight(ui, ("added menu", *flying), true);
            let gone = shown <= 0.0 || fly >= FLY;
            if !gone {
                middle.draw(ui, fly, shown);
            }
            if gone {
                self.leaving = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use model::{App, AppKind, Game};

    use super::*;

    #[test]
    fn the_rows_are_the_two_that_add_then_the_games_by_name_then_the_apps_as_added() {
        let game = |id: i64, name: &str, source: &str| Game {
            id,
            name: name.into(),
            source: source.into(),
            ..Game::default()
        };
        let app = |id: i64, name: &str| App {
            id,
            name: name.into(),
            kind: AppKind::Program,
            target: String::new(),
            icon: None,
        };
        let library = Library {
            games: vec![game(1, "Tern", MANUAL), game(2, "Steam's", "steam"), game(3, "The Anchor", MANUAL)],
            hidden: vec![game(4, "Moss", MANUAL)],
            apps: vec![app(7, "Zed"), app(5, "Notepad")],
            ..Library::default()
        };
        let said: Vec<String> = lines(&library).iter().map(Line::said).collect();
        assert_eq!(
            said,
            ["Add a game", "Add an app", "Game The Anchor", "Game Moss", "Game Tern", "App Zed", "App Notepad"]
        );
    }
}
