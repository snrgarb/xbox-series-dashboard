//! A pane of boxes: a category's, Personalisation's. Rows of boxes two wide, each opening a page in the boxes'
//! place or turning a checkbox over (docs/spec/settings.md, 4.1, 4.2 and 9.2); or, where a category has more than a
//! handful, columns of boxes each under a section title, as the console's Settings has them (6.1, 9.2).
use ui::{Button, Font, Press, Rect, TextStyle, Ui};

use super::controls::{self, Face, Grid, Move, BOXES_TOP, BOX_GAP, BOX_HEIGHT, BOX_WIDTH, LEFT};
use super::{Change, Pane, Setting, Step};
use crate::pages::{Around, PageAction};
use crate::Effect;

/// A section's title over its column: Bahnschrift 30, as the rail's title.
const SECTION: TextStyle = TextStyle::new(Font::Display, 30.0, 400);
/// A section's first box is this far under its title's top.
const SECTION_TITLE: f32 = 52.0;

/// A box on a pane of boxes.
#[derive(Clone, Copy)]
pub enum Item {
    /// A opens a page in the boxes' place: an icon and a name. The page is made from the world as it is then.
    Opens { icon: char, name: &'static str, pane: fn(Around) -> Box<dyn Pane> },
    /// A checkbox box: A turns the option over, at once and saved. It has a row to itself.
    Check { setting: Setting, name: &'static str },
}

impl Item {
    fn name(&self) -> &'static str {
        match *self {
            Item::Opens { name, .. } | Item::Check { name, .. } => name,
        }
    }
}

/// How the boxes are laid out, and where focus is on them.
enum Layout {
    /// Rows two wide from (520, 210): `Boxes::rows` are the rows.
    Rows(Grid),
    /// Columns side by side, each under its title: `Boxes::rows` are the columns. Focus is on box `at` of column
    /// `column`.
    Sections { titles: Vec<&'static str>, column: usize, at: usize },
}

/// A pane of boxes under a header. Rows of boxes scroll where they run past the screen's foot, to keep the focused
/// box in view.
pub struct Boxes {
    header: &'static str,
    rows: Vec<Vec<Item>>,
    layout: Layout,
}

impl Boxes {
    /// The pane with these rows of boxes, focus on the first.
    pub fn new(header: &'static str, rows: Vec<Vec<Item>>) -> Self {
        let lengths: Vec<usize> = rows.iter().map(Vec::len).collect();
        Self { header, rows, layout: Layout::Rows(Grid::new(&lengths)) }
    }

    /// The pane with these sections, a column each, its title over its boxes; focus on the first section's first.
    pub fn sections(header: &'static str, sections: Vec<(&'static str, Vec<Item>)>) -> Self {
        assert!(sections.iter().all(|(_, items)| !items.is_empty()), "a section has a box");
        let (titles, rows) = sections.into_iter().unzip();
        Self { header, rows, layout: Layout::Sections { titles, column: 0, at: 0 } }
    }

    /// The same with focus on the box at this row and column (in sections: this far down this section), or the
    /// nearest there is: a pane shown under the page one of its boxes opens.
    pub fn with_focus(mut self, row: usize, column: usize) -> Self {
        match &mut self.layout {
            Layout::Rows(grid) => grid.set_focus(row, column),
            Layout::Sections { column: at_column, at, .. } => {
                *at_column = column.min(self.rows.len() - 1);
                *at = row.min(self.rows[*at_column].len() - 1);
            }
        }
        self
    }

    /// How far the pane has scrolled up to show the focused box.
    pub fn scroll(&self) -> f32 {
        match &self.layout {
            Layout::Rows(grid) => grid.scroll(),
            Layout::Sections { .. } => 0.0,
        }
    }

    /// The focused box's place: row and column, or in sections how far down and which.
    fn at(&self) -> (usize, usize) {
        match &self.layout {
            Layout::Rows(grid) => grid.focus(),
            Layout::Sections { column, at, .. } => (*column, *at),
        }
    }

    fn focused(&self) -> Item {
        let (row, column) = self.at();
        self.rows[row][column]
    }

    /// Where a section's box is: its column at the grid's, under its title.
    fn section_rect(column: usize, at: usize) -> Rect {
        let (across, down) = (BOX_WIDTH + BOX_GAP, BOX_HEIGHT + BOX_GAP);
        Rect::new(LEFT + across * column as f32, BOXES_TOP + SECTION_TITLE + down * at as f32, BOX_WIDTH, BOX_HEIGHT)
    }
}

/// A direction in sections: Up and Down along the column, stopping at its ends; Right and Left to the column beside,
/// as far down as it goes; Left from the first column back a level, as on rows.
fn step_sections(rows: &[Vec<Item>], column: &mut usize, at: &mut usize, button: Button) -> Move {
    match button {
        Button::Up if *at > 0 => *at -= 1,
        Button::Down if *at + 1 < rows[*column].len() => *at += 1,
        Button::Right if *column + 1 < rows.len() => {
            *column += 1;
            *at = (*at).min(rows[*column].len() - 1);
        }
        Button::Left if *column == 0 => return Move::Back,
        Button::Left => {
            *column -= 1;
            *at = (*at).min(rows[*column].len() - 1);
        }
        _ => {}
    }
    Move::Stay
}

/// A box drawn with its face.
fn draw_item(ui: &mut Ui, around: Around, rect: Rect, item: Item, focused: bool) {
    let face = match item {
        Item::Opens { icon, .. } => Face::Icon(icon),
        Item::Check { setting, .. } => Face::Checkbox { on: around.options.get(setting) },
    };
    controls::settings_box(ui, rect, item.name(), face, focused);
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
            (button, _) => {
                let moved = match &mut self.layout {
                    Layout::Rows(grid) => grid.press(button),
                    Layout::Sections { column, at, .. } => step_sections(&self.rows, column, at, button),
                };
                match moved {
                    Move::Back => Step::Back,
                    Move::Stay => Step::Stay,
                }
            }
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let at = self.at();
        let focused_item = self.focused();
        let rows = &self.rows;
        match &mut self.layout {
            Layout::Sections { titles, .. } => {
                for (column, (title, items)) in titles.iter().zip(rows).enumerate() {
                    let x = Self::section_rect(column, 0).x;
                    ui.label(x, BOXES_TOP, title, &SECTION);
                    for (down, &item) in items.iter().enumerate() {
                        if !focused || (column, down) != at {
                            draw_item(ui, around, Self::section_rect(column, down), item, false);
                        }
                    }
                }
                // The focused box last: its ring and glow lie over its neighbours.
                if focused {
                    draw_item(ui, around, Self::section_rect(at.0, at.1), focused_item, true);
                }
            }
            Layout::Rows(grid) => {
                grid.scrolled(ui, |ui| {
                    for (row, items) in rows.iter().enumerate() {
                        for (column, &item) in items.iter().enumerate() {
                            if !focused || (row, column) != at {
                                draw_item(ui, around, Grid::rect(row, column), item, false);
                            }
                        }
                    }
                    if focused {
                        draw_item(ui, around, Grid::rect(at.0, at.1), focused_item, true);
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_step_along_a_column_and_across_to_the_one_beside_as_far_down_as_it_goes() {
        let item = Item::Check { setting: Setting::ReduceMotion, name: "x" };
        let rows = vec![vec![item; 4], vec![item; 2]];
        let (mut column, mut at) = (0, 0);
        let mut step = |button| {
            let moved = step_sections(&rows, &mut column, &mut at, button);
            (moved, column, at)
        };
        assert_eq!(step(Button::Up), (Move::Stay, 0, 0), "the top stops");
        assert_eq!(step(Button::Down), (Move::Stay, 0, 1));
        assert_eq!(step(Button::Down), (Move::Stay, 0, 2));
        assert_eq!(step(Button::Right), (Move::Stay, 1, 1), "the shorter column's foot");
        assert_eq!(step(Button::Right), (Move::Stay, 1, 1), "the last column stops");
        assert_eq!(step(Button::Down), (Move::Stay, 1, 1), "the foot stops");
        assert_eq!(step(Button::Left), (Move::Stay, 0, 1));
        assert_eq!(step(Button::Left), (Move::Back, 0, 1), "Left from the first column goes back");
    }

    #[test]
    fn a_sections_boxes_are_under_its_title_at_the_grids_columns() {
        let first = Boxes::section_rect(0, 0);
        assert_eq!((first.x, first.y, first.w, first.h), (520.0, 262.0, 437.0, 131.0));
        let other = Boxes::section_rect(1, 2);
        assert_eq!((other.x, other.y), (975.0, 560.0));
    }
}
