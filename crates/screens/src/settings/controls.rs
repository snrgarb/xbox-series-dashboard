//! Settings' controls, as the Bundle draws them (docs/spec/settings.md, sections 4 and 5): the Box and the checkbox
//! box with its info text, the Row, the pane button, the dropdown box and its list, and the checkbox row; the
//! focus ring and glow they share; and the grid a pane of boxes is laid out and steered on.
//!
//! Each is a function that draws the control in a rectangle from what it is told, with or without focus: a pane
//! keeps the state (which control has focus, what is ticked) and draws the one with focus last, so its ring and
//! glow lie over its neighbours. Only the dropdown ([`Dropdown`]) and the grid ([`Grid`]) keep anything of their
//! own. Nothing here is scaled: every measure is the screen's, 1:1.
use ui::{Button, Color, HAlign, Rect, Ui, VAlign};

use crate::theme::{ACCENT_LIGHT, BODY, TEXT_DIM};

/// The page's fill.
pub const PAGE: Color = Color::rgb(0x1F1F1F);
/// The rail's dividers and the line between the rail and the content.
pub const LINE: Color = Color::rgb(0x303030);
const BOX: Color = Color::rgb(0x2D2D2D);
const FOCUSED: Color = Color::rgb(0x4A4A4A);
const BUTTON: Color = Color::rgb(0x333333);
/// A dropdown box or a checkbox row: its fill, its fill with focus or open, and its edge.
const FIELD: Color = Color::argb(0x14FFFFFF);
const FIELD_LIT: Color = Color::argb(0x26FFFFFF);
const FIELD_EDGE: Color = Color::argb(0x80FFFFFF);
const LIST: Color = Color::rgb(0x2E2E2E);
const LIST_EDGE: Color = Color::argb(0x33FFFFFF);

/// The content's left edge, and its right: the header's and every pane's.
pub const LEFT: f32 = 520.0;
pub const RIGHT: f32 = 1750.0;
/// The top of the pane row, where a pane of labels and fields starts. A pane of boxes starts at [`BOXES_TOP`].
pub const TOP: f32 = 190.0;
pub const BOXES_TOP: f32 = 210.0;
/// A Box, and the room between two.
pub const BOX_WIDTH: f32 = 437.0;
pub const BOX_HEIGHT: f32 = 131.0;
pub const BOX_GAP: f32 = 18.0;
/// A Row: as wide as two boxes, 8 under each.
pub const ROW_WIDTH: f32 = 892.0;
pub const ROW_HEIGHT: f32 = 66.0;
pub const ROW_PITCH: f32 = 74.0;
/// A dropdown box and a checkbox row, and a pane button's height.
pub const FIELD_WIDTH: f32 = 560.0;
pub const FIELD_HEIGHT: f32 = 58.0;
/// The info text beside a checkbox box wraps at this.
pub const INFO_WIDTH: f32 = 400.0;

/// The icon font's glyphs the controls use.
pub mod glyph {
    pub const CHECKBOX: char = '\u{E739}';
    pub const CHECKBOX_TICKED: char = '\u{E73A}';
    pub const TICK: char = '\u{E73E}';
    /// The plus in the tick's slot of the Row that adds one.
    pub const ADD: char = '\u{E710}';
    pub const CHEVRON_DOWN: char = '\u{E70D}';
    pub const CHEVRON_UP: char = '\u{E70E}';
}

/// The focus ring's outer corner radius, by what it is round (docs/spec/settings.md, 5.1).
pub const RING_BOX: f32 = 10.0;
pub const RING_BUTTON: f32 = 12.0;
pub const RING_FIELD: f32 = 14.0;

/// The focus ring round a control: a 4-wide stroke 4 clear of its edge, and the glow under it. It is Home's ring
/// at scale 1; only its corner differs by control (`radius`, its outer corner: [`RING_BOX`] and the others).
pub fn focus_ring(ui: &mut Ui, rect: Rect, radius: f32) {
    glow(ui, rect, 6.0);
    ui.ring(rect.grow(8.0), ACCENT_LIGHT, radius, 4.0);
}

/// The theme's glow, its middle `out` outside `rect`'s edge: 6 under a ring, 2 round the rail's focused item, -1
/// on an open dropdown box's own edge.
pub fn glow(ui: &mut Ui, rect: Rect, out: f32) {
    ui.glow(rect.grow(out + 5.5), ACCENT_LIGHT.fade(0.5), 17.5, 11.0, 17.0);
}

/// One line in a box, cut short with an ellipsis, its middle on the box's.
fn line(ui: &mut Ui, rect: Rect, text: &str, color: Color) {
    ui.paragraph(rect, text, &BODY.color(color), HAlign::Left, VAlign::Center, 1);
}

/// What a Box shows beside its name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Face<'a> {
    /// A glyph of the icon font: a box that opens a page, or acts.
    Icon(char),
    /// A checkbox at the right, and the text that shows beside the box while it has focus, in the second
    /// column's place. A turns the checkbox over.
    Checkbox { on: bool, info: &'a str },
}

/// A Box: 437 x 131 as the grid has it, square, its name on one line and never a second saying what it is set to.
pub fn settings_box(ui: &mut Ui, rect: Rect, name: &str, face: Face, focused: bool) {
    ui.fill(rect, if focused { FOCUSED } else { BOX }, 0.0);
    let (_, cy) = rect.center();
    match face {
        Face::Icon(glyph) => {
            ui.icon(rect.x + 50.0, cy, glyph, 34.0, Color::WHITE);
            line(ui, Rect::new(rect.x + 100.0, rect.y, rect.w - 124.0, rect.h), name, Color::WHITE);
        }
        Face::Checkbox { on, info } => {
            line(ui, Rect::new(rect.x + 30.0, rect.y, rect.w - 104.0, rect.h), name, Color::WHITE);
            let glyph = if on { glyph::CHECKBOX_TICKED } else { glyph::CHECKBOX };
            ui.icon(rect.right() - 24.0 - 17.0, cy, glyph, 34.0, Color::WHITE);
            if focused {
                let at = Rect::new(rect.right() + BOX_GAP + 30.0, rect.y + 8.0, INFO_WIDTH, 2000.0);
                ui.paragraph(at, info, &BODY, HAlign::Left, VAlign::Top, 0);
            }
        }
    }
    if focused {
        focus_ring(ui, rect, RING_BOX);
    }
}

/// How far below a checkbox box's top its info text ends: what a pane that scrolls keeps in view with the box.
pub fn info_height(ui: &mut Ui, info: &str) -> f32 {
    8.0 + ui.measure_wrapped(&BODY, info, INFO_WIDTH, 0).height
}

/// A Row: a choice in a list, the one set now ticked.
pub fn row(ui: &mut Ui, rect: Rect, name: &str, ticked: bool, focused: bool) {
    row_marked(ui, rect, name, ticked.then_some(glyph::TICK), focused);
}

/// A Row with this glyph in the tick's slot, or with the slot empty: the tick, or the plus of a Row that adds one.
pub fn row_marked(ui: &mut Ui, rect: Rect, name: &str, mark: Option<char>, focused: bool) {
    ui.fill(rect, if focused { FOCUSED } else { BOX }, 0.0);
    let (_, cy) = rect.center();
    if let Some(mark) = mark {
        ui.icon(rect.x + 45.0, cy, mark, 24.0, Color::WHITE);
    }
    line(ui, Rect::new(rect.x + 84.0, rect.y, rect.w - 108.0, rect.h), name, Color::WHITE);
    if focused {
        focus_ring(ui, rect, RING_BOX);
    }
}

/// How wide a pane button is: 180, or its label and 30 each side if that is more.
pub fn pane_button_width(ui: &mut Ui, label: &str) -> f32 {
    (ui.measure(&BODY, label).width + 60.0).max(180.0)
}

/// A pane button, 58 tall. One that does nothing now (`enabled` false) has its label dimmed, and takes focus all
/// the same.
pub fn pane_button(ui: &mut Ui, rect: Rect, label: &str, enabled: bool, focused: bool) {
    ui.fill(rect, if focused { FOCUSED } else { BUTTON }, 2.0);
    let (cx, cy) = rect.center();
    ui.label_centered(cx, cy, label, &BODY.color(Color::WHITE.fade(if enabled { 1.0 } else { 0.4 })));
    if focused {
        focus_ring(ui, rect, RING_BUTTON);
    }
}

/// A line of text over a control ("Dim after"). Returns its height.
pub fn label(ui: &mut Ui, x: f32, y: f32, text: &str) -> f32 {
    ui.label(x, y, text, &BODY).height
}

/// A description, a hint: dim text wrapped at `width`, never cut short. Returns its height.
pub fn description(ui: &mut Ui, x: f32, y: f32, width: f32, text: &str) -> f32 {
    let style = BODY.color(TEXT_DIM);
    ui.paragraph(Rect::new(x, y, width, 2000.0), text, &style, HAlign::Left, VAlign::Top, 0).height
}

/// The fill and edge of a dropdown box or a checkbox row, and its name or value: 18 in, 50 kept clear at the right.
fn field(ui: &mut Ui, rect: Rect, text: &str, lit: bool, edge: Color) {
    ui.fill(rect, if lit { FIELD_LIT } else { FIELD }, 4.0);
    ui.ring(rect, edge, 4.0, 2.0);
    line(ui, Rect::new(rect.x + 18.0, rect.y, rect.w - 86.0, rect.h), text, Color::WHITE);
}

/// A checkbox row: a checkbox drawn as a dropdown box is, its glyph in the chevron's place. A turns it over.
pub fn checkbox_row(ui: &mut Ui, rect: Rect, name: &str, on: bool, focused: bool) {
    field(ui, rect, name, focused, FIELD_EDGE);
    let glyph = if on { glyph::CHECKBOX_TICKED } else { glyph::CHECKBOX };
    ui.icon(rect.right() - 18.0 - 14.0, rect.center().1, glyph, 28.0, Color::WHITE);
    if focused {
        focus_ring(ui, rect, RING_FIELD);
    }
}

/// A choice's height in a dropdown's list, and from one choice to the next.
const CHOICE: f32 = 60.0;
const CHOICE_PITCH: f32 = 62.0;

/// An open dropdown's list: its choices, the one set, the one with focus, and how far it has scrolled.
#[derive(Debug, Clone, PartialEq)]
struct List {
    choices: Vec<String>,
    set: usize,
    focus: usize,
    scroll: f32,
}

/// A dropdown: a box showing what is set, and while it is open a list of the choices under it, in the page and
/// over whatever is below, which it does not move or dim (docs/spec/settings.md, 4.5 and 4.6).
///
/// The pane keeps one of these per dropdown and what is set. A on the box: [`Dropdown::open`] with the choices
/// as they are now. While it [`is_open`](Dropdown::is_open) the pane gives it every press
/// ([`Dropdown::press`]): A answers the choice, which the pane sets or refuses, and B closes it unchanged; either
/// way focus is back on the box. The pane draws the box in its place ([`Dropdown::draw`]) and the list after
/// everything else it draws ([`Dropdown::draw_list`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dropdown {
    open: Option<List>,
}

impl Dropdown {
    /// Opens the list on these choices, the one set now (`set`, its place among them) ticked and focused.
    pub fn open(&mut self, choices: Vec<String>, set: usize) {
        let set = set.min(choices.len().saturating_sub(1));
        self.open = (!choices.is_empty()).then_some(List { choices, set, focus: set, scroll: 0.0 });
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The choice focus is on in the open list: what the pane's focus line says.
    pub fn focused(&self) -> Option<&str> {
        self.open.as_ref().map(|list| list.choices[list.focus].as_str())
    }

    /// A press while the list is open. Up and Down move along the choices and stop at the ends; A closes the list
    /// and answers the choice's place; B closes it. Nothing else does anything.
    pub fn press(&mut self, button: Button) -> Option<usize> {
        let list = self.open.as_mut()?;
        match button {
            Button::Up => list.focus = list.focus.saturating_sub(1),
            Button::Down => list.focus = (list.focus + 1).min(list.choices.len() - 1),
            Button::A => return self.open.take().map(|list| list.focus),
            Button::B => self.open = None,
            _ => {}
        }
        None
    }

    /// The box, showing `value`: at rest, with focus, or with its list open.
    pub fn draw(&self, ui: &mut Ui, rect: Rect, value: &str, focused: bool) {
        let open = self.is_open();
        if open {
            // Its own edge turns green, the glow on that edge; no ring.
            glow(ui, rect, -1.0);
        }
        field(ui, rect, value, focused || open, if open { ACCENT_LIGHT } else { FIELD_EDGE });
        let chevron = if open { glyph::CHEVRON_UP } else { glyph::CHEVRON_DOWN };
        ui.icon(rect.right() - 18.0 - 8.0, rect.center().1, chevron, 16.0, Color::WHITE);
        if focused && !open {
            focus_ring(ui, rect, RING_FIELD);
        }
    }

    /// Where the open list's panel is under a box at `rect`: 8 under it and as wide, as tall as its choices need
    /// but never nearer the screen's foot than 32, nor shorter than three choices.
    fn panel(&self, rect: Rect, screen_height: f32) -> Option<Rect> {
        let list = self.open.as_ref()?;
        let top = rect.bottom() + 8.0;
        let all = 18.0 + CHOICE_PITCH * list.choices.len() as f32;
        Some(Rect::new(rect.x, top, rect.w, all.min((screen_height - top - 32.0).max(202.0))))
    }

    /// The open list, under the box at `rect`, on a layer of its own so that it covers the text under it. Drawn
    /// after everything else on the pane. Cut short, the choices scroll in it to keep the focused one in view,
    /// at once.
    pub fn draw_list(&mut self, ui: &mut Ui, rect: Rect) {
        let Some(panel) = self.panel(rect, ui.screen().h) else { return };
        let Some(list) = self.open.as_mut() else { return };
        ui.layer();
        ui.fill(panel, LIST, 4.0);
        ui.ring(panel, LIST_EDGE, 4.0, 1.0);
        // The choices scroll inside the panel's edge, the focused one kept 10 clear of its top and foot.
        let inside = panel.h - 2.0;
        let top = |choice: usize| 8.0 + CHOICE_PITCH * choice as f32;
        let most = (16.0 + CHOICE_PITCH * list.choices.len() as f32 - inside).max(0.0);
        list.scroll = show(list.scroll, top(list.focus) - 10.0, top(list.focus) + CHOICE + 10.0, inside);
        list.scroll = list.scroll.clamp(0.0, most);
        let scroll = list.scroll;
        // 16 past the panel's sides, so a ring's glow is not cut there.
        let view = Rect::new(panel.x - 16.0, panel.y + 1.0, panel.w + 32.0, inside);
        let list = &*list;
        ui.with_clip(view, |ui| {
            let choice = |i: usize| Rect::new(panel.x + 7.0, panel.y + 1.0 + top(i) - scroll, panel.w - 14.0, CHOICE);
            let focused = choice(list.focus);
            ui.fill(focused, FIELD_LIT, 4.0);
            focus_ring(ui, focused, RING_FIELD);
            for (i, name) in list.choices.iter().enumerate() {
                let at = choice(i);
                if at.bottom() < view.y || at.y > view.bottom() {
                    continue;
                }
                if i == list.set {
                    ui.icon(at.x + 14.0 + 10.0, at.center().1, glyph::TICK, 20.0, Color::WHITE);
                }
                line(ui, Rect::new(at.x + 52.0, at.y, at.w - 68.0, at.h), name, Color::WHITE);
            }
        });
    }
}

/// How far a view `height` tall, scrolled by `scroll`, must be scrolled to show what lies from `top` to `bottom`
/// in it: as little further as that takes, and where the two cannot both show, the top.
pub fn show(scroll: f32, top: f32, bottom: f32, height: f32) -> f32 {
    scroll.max(bottom - height).min(top)
}

/// Draws a pane with `body`, scrolled up by as little as shows whole what lies from `top` to `bottom` where the
/// pane rests, 32 clear of the screen's foot, and at once: nothing on Settings scrolls with an animation. `first`
/// is where the pane starts ([`TOP`] or [`BOXES_TOP`]); a pane that fits the screen never moves. Scrolled, it is
/// cut a ring and its glow's width over `first` and left of [`LEFT`], so what has focus is never cut where it
/// rests and the rest passes under the header's row.
pub fn scrolled(ui: &mut Ui, scroll: &mut f32, first: f32, top: f32, bottom: f32, body: impl FnOnce(&mut Ui)) {
    let foot = ui.screen().h - FOOT;
    *scroll = show(*scroll, top - first, bottom - first, foot - first).max(0.0);
    let view = Rect::new(LEFT - FOOT, first - FOOT, 4000.0, 4000.0);
    let by = *scroll;
    ui.with_clip(view, |ui| ui.with_offset(0.0, -by, body));
}

/// What a press on a pane of boxes came to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Move {
    /// Focus is on another box, or where it was: a row's end, the grid's top or foot.
    Stay,
    /// Left from the first column: back one level, as B.
    Back,
}

/// The grid a pane of boxes is laid out on, two wide from (520, 210), and where focus is on it. The rows may
/// differ in length; a checkbox box has a row to itself, the second column's place kept for its info text.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// How many boxes each row has.
    rows: Vec<usize>,
    row: usize,
    column: usize,
    /// How far the pane has scrolled up, where its rows run past the screen's foot.
    scroll: f32,
}

impl Grid {
    /// A grid of rows this long each, focus on its first box. Every row has a box.
    pub fn new(rows: &[usize]) -> Self {
        assert!(!rows.is_empty() && rows.iter().all(|&boxes| boxes > 0), "a pane of boxes has a box in every row");
        Self { rows: rows.to_vec(), row: 0, column: 0, scroll: 0.0 }
    }

    /// The row and column focus is on.
    pub fn focus(&self) -> (usize, usize) {
        (self.row, self.column)
    }

    /// Puts focus on the box at this row and column, or the nearest there is.
    pub fn set_focus(&mut self, row: usize, column: usize) {
        self.row = row.min(self.rows.len() - 1);
        self.column = column.min(self.rows[self.row] - 1);
    }

    /// A direction: Up and Down to the row above or below in the same column, or that row's last box if it is
    /// shorter; Right and Left along the row. All stop at the ends, but Left from the first column, which goes
    /// back a level (docs/spec/settings.md, 9.2).
    pub fn press(&mut self, button: Button) -> Move {
        match button {
            Button::Up if self.row > 0 => self.set_focus(self.row - 1, self.column),
            Button::Down => self.set_focus(self.row + 1, self.column),
            Button::Right => self.set_focus(self.row, self.column + 1),
            Button::Left if self.column == 0 => return Move::Back,
            Button::Left => self.column -= 1,
            _ => {}
        }
        Move::Stay
    }

    /// Where the box at this row and column is, before any scrolling.
    pub fn rect(row: usize, column: usize) -> Rect {
        let (across, down) = (BOX_WIDTH + BOX_GAP, BOX_HEIGHT + BOX_GAP);
        Rect::new(LEFT + across * column as f32, BOXES_TOP + down * row as f32, BOX_WIDTH, BOX_HEIGHT)
    }

    /// Draws the pane's boxes with `body`, scrolled so that the focused box shows whole with its ring, and what
    /// reaches `below` its top (its info text) with it: only as far as that takes, and at once. A pane that fits
    /// the screen never moves. `body` draws each box where [`Grid::rect`] says.
    pub fn scrolled(&mut self, ui: &mut Ui, below: f32, body: impl FnOnce(&mut Ui)) {
        let focused = Self::rect(self.row, self.column);
        let bottom = focused.y + below.max(focused.h);
        scrolled(ui, &mut self.scroll, BOXES_TOP, focused.y, bottom, body);
    }

    /// How far the pane has scrolled.
    pub fn scroll(&self) -> f32 {
        self.scroll
    }
}

/// How near the screen's foot a scrolled pane's focused box comes, and how far over the boxes a scrolled pane is
/// cut: room for a ring and its glow.
const FOOT: f32 = 32.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_steps_by_row_and_column_and_left_from_the_first_column_goes_back() {
        // Two boxes, one, then two: as Personalisation is with a box beside its first and third.
        let mut grid = Grid::new(&[2, 1, 2]);
        assert_eq!(grid.press(Button::Up), Move::Stay);
        assert_eq!(grid.focus(), (0, 0), "the top stops");
        grid.press(Button::Right);
        grid.press(Button::Right);
        assert_eq!(grid.focus(), (0, 1), "the row's end stops");
        // Down from the second column to a row of one: its last box. Down again keeps that column.
        grid.press(Button::Down);
        assert_eq!(grid.focus(), (1, 0));
        grid.press(Button::Down);
        grid.press(Button::Right);
        grid.press(Button::Down);
        assert_eq!(grid.focus(), (2, 1), "the foot stops");
        assert_eq!(grid.press(Button::Left), Move::Stay);
        assert_eq!(grid.focus(), (2, 0));
        assert_eq!(grid.press(Button::Left), Move::Back);
        assert_eq!(grid.focus(), (2, 0));
        assert_eq!(grid.press(Button::A), Move::Stay);
    }

    #[test]
    fn the_boxes_are_where_the_spec_says() {
        let first = Grid::rect(0, 0);
        assert_eq!((first.x, first.y, first.w, first.h), (520.0, 210.0, 437.0, 131.0));
        let other = Grid::rect(3, 1);
        assert_eq!((other.x, other.y), (975.0, 657.0));
    }

    #[test]
    fn a_view_scrolls_only_as_far_as_shows_what_has_focus() {
        // In view already: it stays, wherever it is scrolled to.
        assert_eq!(show(0.0, 100.0, 200.0, 870.0), 0.0);
        assert_eq!(show(50.0, 100.0, 200.0, 870.0), 50.0);
        // Past the foot: up until its foot is the view's.
        assert_eq!(show(0.0, 745.0, 926.0, 838.0), 88.0);
        // Above the top: back down until its top is the view's.
        assert_eq!(show(88.0, 0.0, 181.0, 838.0), 0.0);
        // Taller than the view: its top.
        assert_eq!(show(0.0, 100.0, 1100.0, 838.0), 100.0);
    }

    #[test]
    fn a_dropdown_opens_on_what_is_set_and_a_chooses_and_b_does_not() {
        let steps = || ["1 min", "2 min", "5 min"].map(String::from).to_vec();
        let mut dropdown = Dropdown::default();
        assert_eq!(dropdown.press(Button::A), None, "closed, it has no presses of its own");
        dropdown.open(steps(), 1);
        assert_eq!(dropdown.focused(), Some("2 min"));
        // Along the choices, stopping at the ends; Left and Right do nothing.
        assert_eq!(dropdown.press(Button::Down), None);
        assert_eq!(dropdown.press(Button::Down), None);
        assert_eq!(dropdown.press(Button::Right), None);
        assert_eq!(dropdown.focused(), Some("5 min"));
        assert_eq!(dropdown.press(Button::A), Some(2));
        assert!(!dropdown.is_open());

        dropdown.open(steps(), 2);
        dropdown.press(Button::Up);
        dropdown.press(Button::Up);
        dropdown.press(Button::Up);
        assert_eq!(dropdown.focused(), Some("1 min"));
        assert_eq!(dropdown.press(Button::B), None);
        assert!(!dropdown.is_open());
    }

    #[test]
    fn a_dropdowns_list_is_cut_short_of_the_screens_foot_and_never_under_three_choices() {
        let steps = |count: usize| (0..count).map(|i| i.to_string()).collect::<Vec<_>>();
        let mut dropdown = Dropdown::default();
        let field = |y: f32| Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        // Idle's Dim after: eleven choices fit under it.
        dropdown.open(steps(11), 1);
        let panel = dropdown.panel(field(238.58), 1080.0).unwrap();
        assert!((panel.y - 304.58).abs() < 0.01 && panel.h == 700.0, "{panel:?}");
        // Show things after's ten do not: cut 32 short of the foot.
        dropdown.open(steps(10), 0);
        let panel = dropdown.panel(field(393.16), 1080.0).unwrap();
        assert!((panel.h - 588.84).abs() < 0.01, "{panel:?}");
        // A box near the foot still has three choices' room.
        assert_eq!(dropdown.panel(field(900.0), 1080.0).unwrap().h, 202.0);
        // One choice: a list of one.
        dropdown.open(steps(1), 0);
        assert_eq!(dropdown.panel(field(393.16), 1080.0).unwrap().h, 80.0);
    }
}
