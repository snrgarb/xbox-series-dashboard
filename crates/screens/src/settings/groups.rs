//! Games & apps > Groups (docs/spec/filters.md, 6.2): a Row for every saved filter in its order, ticked while it
//! shows on Home and in Full library, and New Group last. A turns a tick over; Menu opens the Group's menu, the Tile
//! menu's panel over the dimmed page, which moves the Group up or down, opens a custom Group's page, or deletes it
//! after asking.
//!
//! Every change is asked of the shell as an `Effect::Filter` and is carried out on the library database; the page
//! shows what the next snapshot says (`Pane::library_changed`), focus staying on the Group it was on wherever that
//! has gone.
use std::sync::atomic::{AtomicU32, Ordering};

use model::{Filter, FilterChange, FilterKind};
use ui::{Button, Press, Rect, Ui};

use super::controls::{self, glyph, BOXES_TOP, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH};
use super::group::Group;
use super::{Pane, Step};
use crate::hint_bar::{self, Hint};
use crate::menu_panel::{self, icon, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, PageAction};
use crate::Effect;

/// The first Row's top: the page's, with no line over the Rows (#261).
const ROWS_TOP: f32 = BOXES_TOP;
/// The last Row's name, and the name a Group made by it starts with.
pub const NEW_GROUP: &str = "New Group";

/// What a row of a Group's menu does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Up,
    Down,
    Edit,
    Delete,
}

/// The answers to "Delete X?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Delete,
    Cancel,
}

/// The menu's rows for the filter at `at` of `count` (filters.md, 6.2): Move up but for the first, Move down but for
/// the last, and Edit and Delete for a custom one. A row that does not apply is absent.
fn rows(filter: &Filter, at: usize, count: usize) -> Vec<Row<Choice>> {
    let mut rows = Vec::new();
    if at > 0 {
        rows.push(Row::new(icon::MOVE_UP, "Move up", Choice::Up));
    }
    if at + 1 < count {
        rows.push(Row::new(icon::MOVE_DOWN, "Move down", Choice::Down));
    }
    if filter.kind == FilterKind::Custom {
        rows.push(Row::new(icon::EDIT, "Edit", Choice::Edit));
        rows.push(Row::new(icon::DELETE, "Delete", Choice::Delete));
    }
    rows
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

/// A Group's menu while it is up.
struct Menu {
    filter: i64,
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

/// Where focus is: a filter's Row by its id, or New Group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Filter(i64),
    NewGroup,
}

pub struct Groups {
    /// Every filter, in order, as the last snapshot had them.
    filters: Vec<Filter>,
    focus: Focus,
    /// Where focus was among the Rows: where it goes when its filter has gone.
    at: usize,
    menu: Option<Menu>,
    /// The panel in the middle now, and its flight.
    flying: u32,
    /// A panel on its way out, and its flight.
    leaving: Option<(Middle, u32)>,
    /// New Group was chosen: the ids there were then, so the one made is known when the snapshot has it.
    making: Option<Vec<i64>>,
    scroll: f32,
}

impl Groups {
    /// The page as the library has the filters now, focus on the first Row.
    pub fn open(around: Around) -> Self {
        let filters = around.library.filters.clone();
        let focus = filters.first().map_or(Focus::NewGroup, |filter| Focus::Filter(filter.id));
        Self { filters, focus, at: 0, menu: None, flying: 0, leaving: None, making: None, scroll: 0.0 }
    }

    /// The Row focus is on, from the top: New Group is the last.
    fn place(&self) -> usize {
        match self.focus {
            Focus::Filter(id) => self.filters.iter().position(|filter| filter.id == id).unwrap_or(self.at),
            Focus::NewGroup => self.filters.len(),
        }
    }

    fn set_place(&mut self, at: usize) {
        self.at = at.min(self.filters.len());
        self.focus = self.filters.get(self.at).map_or(Focus::NewGroup, |filter| Focus::Filter(filter.id));
    }

    fn filter(&self, id: i64) -> Option<&Filter> {
        self.filters.iter().find(|filter| filter.id == id)
    }

    fn focused(&self) -> Option<&Filter> {
        match self.focus {
            Focus::Filter(id) => self.filter(id),
            Focus::NewGroup => None,
        }
    }

    /// The menu for the Group focus is on.
    fn open_menu(&mut self) {
        let at = self.place();
        let Some(filter) = self.filters.get(at) else { return };
        let panel = MenuPanel::menu(&filter.name, rows(filter, at, self.filters.len()));
        if panel.rows().is_empty() {
            // The one filter there is, and not a custom one: there is nothing to do with it.
            return;
        }
        self.menu = Some(Menu { filter: filter.id, name: filter.name.clone(), panel, prompt: None });
        self.flying = flight();
        self.leaving = None;
    }

    /// The menu goes, flying out.
    fn close_menu(&mut self) {
        if let Some(menu) = self.menu.take() {
            self.leaving = Some((menu.middle(), self.flying));
        }
    }

    /// A press while the menu is up: every button is the menu's.
    fn menu_press(&mut self, press: Press, around: Around) -> Step {
        let Some(menu) = self.menu.as_mut() else { return Step::Stay };
        let filter = menu.filter;
        if let Some(prompt) = menu.prompt.as_mut() {
            match prompt.press(press) {
                Answer::Chose(Confirm::Delete) => {
                    self.close_menu();
                    return Step::Do(PageAction::Effect(Effect::Filter(FilterChange::Delete { filter })));
                }
                // Back to the menu, which flies up again, focus on Delete.
                Answer::Chose(Confirm::Cancel) | Answer::Declined => {
                    menu.prompt = None;
                    self.flying = flight();
                }
                Answer::Moved | Answer::Nothing => {}
            }
            return Step::Stay;
        }
        match menu.panel.press(press) {
            Answer::Chose(Choice::Up) => Step::Do(move_by(filter, -1)),
            Answer::Chose(Choice::Down) => Step::Do(move_by(filter, 1)),
            Answer::Chose(Choice::Edit) => {
                self.menu = None;
                match self.filter(filter) {
                    Some(filter) => Step::Open(Box::new(Group::open(filter, false, around.library))),
                    None => Step::Stay,
                }
            }
            Answer::Chose(Choice::Delete) => {
                let answers = vec![
                    Row::new(icon::DELETE, "Delete", Confirm::Delete),
                    Row::new(icon::CANCEL, "Cancel", Confirm::Cancel),
                ];
                menu.prompt = Some(MenuPanel::prompt(&format!("Delete {}?", menu.name), None, answers));
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

    /// Where Row i is before any scrolling.
    fn row(i: usize) -> Rect {
        Rect::new(LEFT, ROWS_TOP + ROW_PITCH * i as f32, ROW_WIDTH, ROW_HEIGHT)
    }

    /// How far the page has scrolled up to show the focused Row.
    pub fn scroll(&self) -> f32 {
        self.scroll
    }
}

fn move_by(filter: i64, by: i32) -> PageAction {
    PageAction::Effect(Effect::Filter(FilterChange::Move { filter, by }))
}

impl Pane for Groups {
    fn header(&self) -> String {
        "Groups".into()
    }

    /// `Groups: Most played`, `Groups: New Group`; with the menu up, `Groups: Menu(Move up) of Most played`, and
    /// with its question, `Groups: Prompt(Delete) of Couch co-op`.
    fn focus(&self) -> String {
        fn row<T: Copy>(panel: &MenuPanel<T>) -> &str {
            panel.rows().get(panel.focus()).map_or("", |row| row.label.as_str())
        }
        if let Some(menu) = &self.menu {
            return match &menu.prompt {
                Some(prompt) => format!("Groups: Prompt({}) of {}", row(prompt), menu.name),
                None => format!("Groups: Menu({}) of {}", row(&menu.panel), menu.name),
            };
        }
        match self.focused() {
            Some(filter) => format!("Groups: {}", filter.name),
            None => format!("Groups: {NEW_GROUP}"),
        }
    }

    /// Whether the Group focus is on shows: `shown` or `not shown`.
    fn state(&self) -> String {
        match self.focused() {
            Some(filter) if self.menu.is_none() => (if filter.shown { "shown" } else { "not shown" }).into(),
            _ => String::new(),
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        if self.menu.is_some() {
            return self.menu_press(press, around);
        }
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        let at = self.place();
        match press.button {
            Button::Up => self.set_place(at.saturating_sub(1)),
            Button::Down => self.set_place(at + 1),
            Button::B => return Step::Back,
            Button::A => match self.focused() {
                Some(filter) => {
                    let change = FilterChange::Shown { filter: filter.id, shown: !filter.shown };
                    return Step::Do(PageAction::Effect(Effect::Filter(change)));
                }
                None => {
                    self.making = Some(self.filters.iter().map(|filter| filter.id).collect());
                    let change = FilterChange::Add { name: NEW_GROUP.into() };
                    return Step::Do(PageAction::Effect(Effect::Filter(change)));
                }
            },
            Button::Menu => self.open_menu(),
            // Left and Right do nothing, as on Personalisation's other pages.
            _ => {}
        }
        Step::Stay
    }

    fn hints(&self, _around: Around) -> &'static [Hint] {
        match self.focus {
            Focus::Filter(_) => hint_bar::MORE_OPTIONS,
            Focus::NewGroup => hint_bar::NONE,
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

    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        self.filters = around.library.filters.clone();
        // Focus stays on its Group wherever it is now; on the Row in its place if it has gone.
        let at = self.place();
        self.set_place(at);
        if let Some(menu) = &mut self.menu {
            match self.filters.iter().position(|filter| filter.id == menu.filter) {
                // Its rows as they are now, focus on the same row if it is still there, else on the one in its
                // place: Move up gone at the top, focus drops to the next row.
                Some(at) => {
                    let filter = &self.filters[at];
                    let was = menu.panel.rows().get(menu.panel.focus()).map(|row| row.value);
                    let rows = rows(filter, at, self.filters.len());
                    let focus = rows.iter().position(|row| Some(row.value) == was).unwrap_or(menu.panel.focus());
                    menu.name = filter.name.clone();
                    menu.panel = MenuPanel::menu(&filter.name, rows);
                    menu.panel.set_focus(focus);
                    if menu.panel.rows().is_empty() {
                        self.menu = None;
                    }
                }
                None => self.menu = None,
            }
        }
        // A Group just made: focus on its Row, and its page opens over this one, focus on Rename.
        let before = self.making.take()?;
        let made = self
            .filters
            .iter()
            .rev()
            .find(|filter| filter.kind == FilterKind::Custom && !before.contains(&filter.id))?;
        let (id, page) = (made.id, Group::open(made, true, around.library));
        self.focus = Focus::Filter(id);
        self.at = self.place();
        Some(Box::new(page))
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, focused: bool) {
        let at = self.place();
        let rows = self.filters.len() + 1;
        let focused_row = Self::row(at);
        let filters = &self.filters;
        controls::scrolled(ui, &mut self.scroll, BOXES_TOP, focused_row.y, focused_row.bottom(), |ui| {
            let draw = |ui: &mut Ui, i: usize, focused: bool| match filters.get(i) {
                Some(filter) => {
                    controls::row_marked(ui, Self::row(i), &filter.name, filter.shown.then_some(glyph::TICK), focused)
                }
                None => controls::row_marked(ui, Self::row(i), NEW_GROUP, Some(glyph::ADD), focused),
            };
            for i in (0..rows).filter(|&i| i != at || !focused) {
                draw(ui, i, false);
            }
            // The focused Row last, its ring over its neighbours. It keeps its ring under the menu, as a tile keeps
            // its own under the Tile menu.
            if focused {
                draw(ui, at, true);
            }
        });
        // The menu over the dimmed page, the rail with it; or a panel on its way out, the dim gone at once.
        if let Some(menu) = &self.menu {
            ui.layer();
            ui.fill(ui.screen(), DIM, 0.0);
            let (fly, shown) = menu_panel::flight(ui, ("groups menu", self.flying), false);
            menu.middle().draw(ui, fly, shown);
        } else if let Some((middle, flying)) = &self.leaving {
            ui.layer();
            let (fly, shown) = menu_panel::flight(ui, ("groups menu", *flying), true);
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
    use model::{Rule, Sort};

    use super::*;

    fn filter(id: i64, kind: FilterKind) -> Filter {
        Filter {
            id,
            kind,
            key: None,
            name: format!("Filter {id}"),
            shown: true,
            rule: Rule::default(),
            sort: Sort::Name,
            descending: false,
            games: Vec::new(),
        }
    }

    fn labels(filter: &Filter, at: usize, count: usize) -> Vec<String> {
        rows(filter, at, count).into_iter().map(|row| row.label).collect()
    }

    #[test]
    fn a_groups_menu_has_the_rows_that_apply_and_no_others() {
        let builtin = filter(1, FilterKind::Builtin);
        assert_eq!(labels(&builtin, 0, 3), ["Move down"]);
        assert_eq!(labels(&builtin, 1, 3), ["Move up", "Move down"]);
        assert_eq!(labels(&builtin, 2, 3), ["Move up"]);
        assert_eq!(labels(&filter(2, FilterKind::Collection), 1, 3), ["Move up", "Move down"]);
        assert_eq!(labels(&filter(3, FilterKind::Custom), 2, 3), ["Move up", "Edit", "Delete"]);
        assert!(labels(&builtin, 0, 1).is_empty());
    }
}
