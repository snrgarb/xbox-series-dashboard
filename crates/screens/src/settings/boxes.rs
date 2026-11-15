//! A pane of boxes: Theme settings, Personalisation. Rows of boxes two wide, each opening a page in the boxes'
//! place or turning a checkbox over (docs/spec/settings.md, 4.1, 4.2 and 9.2).
use ui::{Button, Press, Ui};

use super::controls::{self, Face, Grid, Move};
use super::{Change, Pane, Setting, Step};
use crate::pages::{Around, PageAction};
use crate::Effect;

/// A box on a pane of boxes.
#[derive(Clone, Copy)]
pub enum Item {
    /// A opens a page in the boxes' place: an icon and a name. The page is made from the world as it is then.
    Opens { icon: char, name: &'static str, pane: fn(Around) -> Box<dyn Pane> },
    /// A checkbox box: A turns the option over, at once and saved. Its info text shows beside it while it has
    /// focus, so it has a row to itself.
    Check { setting: Setting, name: &'static str, info: &'static str },
}

impl Item {
    fn name(&self) -> &'static str {
        match *self {
            Item::Opens { name, .. } | Item::Check { name, .. } => name,
        }
    }
}

/// A pane of boxes under a header. It scrolls where its rows run past the screen's foot, to keep the focused box
/// and its info text in view.
pub struct Boxes {
    header: &'static str,
    rows: Vec<Vec<Item>>,
    grid: Grid,
}

impl Boxes {
    /// The pane with these rows of boxes, focus on the first.
    pub fn new(header: &'static str, rows: Vec<Vec<Item>>) -> Self {
        let lengths: Vec<usize> = rows.iter().map(Vec::len).collect();
        Self { header, rows, grid: Grid::new(&lengths) }
    }

    /// The same with focus on the box at this row and column: a pane shown under the page one of its boxes opens.
    pub fn with_focus(mut self, row: usize, column: usize) -> Self {
        self.grid.set_focus(row, column);
        self
    }

    /// How far the pane has scrolled up to show the focused box.
    pub fn scroll(&self) -> f32 {
        self.grid.scroll()
    }

    fn focused(&self) -> Item {
        let (row, column) = self.grid.focus();
        self.rows[row][column]
    }
}

impl Pane for Boxes {
    fn header(&self) -> String {
        self.header.into()
    }

    fn focus(&self) -> String {
        format!("{}: {}", self.header, self.focused().name())
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        match (press.button, self.focused()) {
            (Button::B, _) => Step::Back,
            (Button::A, Item::Opens { pane, .. }) => Step::Open(pane(around)),
            (Button::A, Item::Check { setting, .. }) => {
                let change = Change { setting, on: !around.options.get(setting) };
                Step::Do(PageAction::Effect(Effect::Set(change)))
            }
            (button, _) => match self.grid.press(button) {
                Move::Back => Step::Back,
                Move::Stay => Step::Stay,
            },
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let at = self.grid.focus();
        // A focused checkbox box's info text may reach below it: the pane scrolls to show that too.
        let below = match self.focused() {
            Item::Check { info, .. } if focused => controls::info_height(ui, info),
            _ => 0.0,
        };
        let rows = &self.rows;
        self.grid.scrolled(ui, below, |ui| {
            let draw = |ui: &mut Ui, row: usize, column: usize, focused: bool| {
                let item = rows[row][column];
                let face = match item {
                    Item::Opens { icon, .. } => Face::Icon(icon),
                    Item::Check { setting, info, .. } => Face::Checkbox { on: around.options.get(setting), info },
                };
                controls::settings_box(ui, Grid::rect(row, column), item.name(), face, focused);
            };
            for (row, items) in rows.iter().enumerate() {
                for column in 0..items.len() {
                    if !focused || (row, column) != at {
                        draw(ui, row, column, false);
                    }
                }
            }
            // The focused box last: its ring and glow lie over its neighbours.
            if focused {
                draw(ui, at.0, at.1, true);
            }
        });
    }
}
