//! The wide dialog: a band across the whole screen with a question, its options as tiles three to a row, and a
//! Close button under them (docs/spec/guide.md, section 6.2). Power is the one there is. Like the menu panel it
//! knows its options and where focus is, takes the pad and draws itself; what is behind it and what its sounds are
//! are the caller's.
use std::hash::Hash;

use ui::{Button, Color, Ease, Font, Press, Rect, TextStyle, Ui};

use crate::menu_panel::Answer;
use crate::theme::{self, BODY};

/// The band: the screen's whole width, y 134 to 942.
const BAND: (f32, f32) = (134.0, 808.0);
const BAND_FILL: Color = Color::rgb(0x2D2D2D);
/// Its contents: 132 from the left, 229 short of the right, 78 down the band.
const LEFT: f32 = 132.0;
const RIGHT: f32 = 229.0;
const TOP: f32 = 78.0;
/// The question: Bahnschrift 54, whose line is WPF's for the font. The tiles are 79 under it.
const TITLE: TextStyle = TextStyle::new(Font::Display, 54.0, 400);
const TITLE_LINE: f32 = 64.8;
const UNDER_TITLE: f32 = 79.0;
/// The tiles: three to a row over the contents' width, each its cell less 16 at the right and 18 below.
const COLUMNS: usize = 3;
const TILE_HEIGHT: f32 = 129.0;
const TILE_GAP: (f32, f32) = (16.0, 18.0);
const TILE: Color = Color::rgb(0x3B3B3B);
const FOCUSED: Color = Color::rgb(0x4A4A4A);
/// Close: 80 under the tiles' cells, 6 left of the contents' edge.
const CLOSE: (f32, f32) = (312.0, 104.0);
const ABOVE_CLOSE: f32 = 80.0;
const CLOSE_LABEL: TextStyle = TextStyle::new(Font::Ui, 26.0, 700);
/// The contents come in from this far to the left, 40 ms after the band, over 120 ms.
const TRAVEL: f32 = -200.0;
const WAIT_SECONDS: f32 = 0.04;
const SETTLE_SECONDS: f32 = 0.12;

/// One tile: what it shows, and what choosing it means to whoever made the dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile<T> {
    pub icon: char,
    pub label: String,
    pub value: T,
}

impl<T> Tile<T> {
    pub fn new(icon: char, label: &str, value: T) -> Self {
        Self { icon, label: label.into(), value }
    }
}

/// Where focus is in the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// A tile, counted along the rows from the top left.
    Tile(usize),
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WideDialog<T> {
    title: String,
    tiles: Vec<Tile<T>>,
    focus: Focus,
}

impl<T: Copy> WideDialog<T> {
    /// Focus starts on Close: nothing is carried out by an A too many.
    pub fn new(title: &str, tiles: Vec<Tile<T>>) -> Self {
        Self { title: title.into(), tiles, focus: Focus::Close }
    }

    pub fn tiles(&self) -> &[Tile<T>] {
        &self.tiles
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// The pad while the dialog has it. Left and Right move along a row of tiles and stop at its ends. Up is the
    /// tile above. Down is the tile below, or the last tile where the row below is too short to have one; from the
    /// last row it is Close. Up from Close is the last row's first tile. Only a direction repeats. A chooses the
    /// focused tile, or nothing (`Chose(None)`) on Close; B declines.
    pub fn press(&mut self, press: Press) -> Answer<Option<T>> {
        let count = self.tiles.len();
        let to = match (press.button, self.focus) {
            (Button::A, focus) if !press.repeat => {
                return match focus {
                    Focus::Tile(at) => {
                        self.tiles.get(at).map_or(Answer::Nothing, |tile| Answer::Chose(Some(tile.value)))
                    }
                    Focus::Close => Answer::Chose(None),
                };
            }
            (Button::B, _) if !press.repeat => return Answer::Declined,
            (Button::Left, Focus::Tile(at)) if at % COLUMNS > 0 => Focus::Tile(at - 1),
            (Button::Right, Focus::Tile(at)) if at % COLUMNS + 1 < COLUMNS && at + 1 < count => Focus::Tile(at + 1),
            (Button::Up, Focus::Tile(at)) if at >= COLUMNS => Focus::Tile(at - COLUMNS),
            (Button::Up, Focus::Close) if count > 0 => Focus::Tile((count - 1) / COLUMNS * COLUMNS),
            (Button::Down, Focus::Tile(at)) if at + COLUMNS < count => Focus::Tile(at + COLUMNS),
            (Button::Down, Focus::Tile(at)) if at / COLUMNS == (count - 1) / COLUMNS => Focus::Close,
            (Button::Down, Focus::Tile(_)) => Focus::Tile(count - 1),
            _ => return Answer::Nothing,
        };
        self.focus = to;
        Answer::Moved
    }

    /// Draws the dialog in the layer being drawn, and a layer of its own over that while its contents come in.
    /// `opening` names this showing of it: the contents come in once for each, and not at all under Reduce motion.
    pub fn draw(&self, ui: &mut Ui, opening: impl Hash) {
        let screen = ui.screen();
        let band = Rect::new(screen.x, BAND.0, screen.w, BAND.1);
        ui.fill(band, BAND_FILL, 0.0);

        let whole = WAIT_SECONDS + SETTLE_SECONDS;
        let passed = ui.slide_from(("wide dialog", opening), 0.0, 1.0, whole, Ease::Linear) * whole;
        let settled = ((passed - WAIT_SECONDS) / SETTLE_SECONDS).clamp(0.0, 1.0);
        if settled >= 1.0 {
            self.contents(ui, band);
        } else {
            let offset = TRAVEL * (1.0 - Ease::CubicOut.at(settled));
            ui.group(band, 0.0, settled, |ui| ui.with_offset(offset, 0.0, |ui| self.contents(ui, band)));
        }
    }

    fn contents(&self, ui: &mut Ui, band: Rect) {
        // As WPF's layout rounding: every edge on a whole pixel.
        let px = ui.px();
        let snap = |at: f32| (at * px).round() / px;
        let (left, top) = (band.x + LEFT, band.y + TOP);
        let width = band.w - LEFT - RIGHT;
        ui.label(left, top, &self.title, &TITLE);

        let tiles_top = snap(top + TITLE_LINE) + UNDER_TITLE;
        let cell = (width / COLUMNS as f32, TILE_HEIGHT + TILE_GAP.1);
        let place = |at: usize| {
            let (column, row) = ((at % COLUMNS) as f32, (at / COLUMNS) as f32);
            let (x, right) = (snap(left + column * cell.0), snap(left + (column + 1.0) * cell.0 - TILE_GAP.0));
            Rect::new(x, tiles_top + row * cell.1, right - x, TILE_HEIGHT)
        };
        let rows = self.tiles.len().div_ceil(COLUMNS) as f32;
        let close = Rect::new(left - 6.0, tiles_top + rows * cell.1 + ABOVE_CLOSE, CLOSE.0, CLOSE.1);

        for (at, tile) in self.tiles.iter().enumerate() {
            let rect = place(at);
            let focused = self.focus == Focus::Tile(at);
            ui.fill(rect, if focused { FOCUSED } else { TILE }, 2.0);
            let middle = rect.center().1;
            // The icon in a box 36 wide, 48 in; the label 42 after the box.
            ui.icon(rect.x + 66.0, middle, tile.icon, 30.0, Color::WHITE);
            ui.label_middle(rect.x + 126.0, middle, &tile.label, &BODY);
        }
        ui.fill(close, if self.focus == Focus::Close { FOCUSED } else { TILE }, 2.0);
        ui.label_centered(close.center().0, close.center().1, "CLOSE", &CLOSE_LABEL);

        // The ring last, over the tiles beside it. WPF's corner of 10 is the stroke's centre line: 12 outside.
        let focused = match self.focus {
            Focus::Tile(at) if at < self.tiles.len() => place(at),
            _ => close,
        };
        theme::focus_ring_cornered(ui, focused, 12.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(tiles: usize) -> WideDialog<usize> {
        WideDialog::new("What?", (0..tiles).map(|at| Tile::new('x', "Tile", at)).collect())
    }

    fn press(dialog: &mut WideDialog<usize>, buttons: &[Button]) -> Focus {
        for &button in buttons {
            dialog.press(Press { button, repeat: false });
        }
        dialog.focus()
    }

    #[test]
    fn focus_starts_on_close_which_goes_nowhere_but_up_to_the_last_rows_first_tile() {
        let mut power = dialog(4);
        assert_eq!(power.focus(), Focus::Close);
        for button in [Button::Left, Button::Right, Button::Down] {
            assert_eq!(power.press(Press { button, repeat: false }), Answer::Nothing, "{button:?}");
        }
        assert_eq!(press(&mut power, &[Button::Up]), Focus::Tile(3));
        // Five tiles: the second row has two, and its first is where Close leads.
        assert_eq!(press(&mut dialog(5), &[Button::Up]), Focus::Tile(3));
        assert_eq!(press(&mut dialog(3), &[Button::Up]), Focus::Tile(0));
    }

    #[test]
    fn left_and_right_stay_in_the_row_and_stop_at_its_ends() {
        let mut power = dialog(4);
        assert_eq!(press(&mut power, &[Button::Up, Button::Up]), Focus::Tile(0));
        assert_eq!(power.press(Press { button: Button::Left, repeat: false }), Answer::Nothing);
        assert_eq!(press(&mut power, &[Button::Right, Button::Right]), Focus::Tile(2));
        // The row's end: not on to the next row's first tile.
        assert_eq!(power.press(Press { button: Button::Right, repeat: false }), Answer::Nothing);
        assert_eq!(press(&mut power, &[Button::Left]), Focus::Tile(1));
        // The second row's one tile has no neighbours.
        let mut power = dialog(4);
        press(&mut power, &[Button::Up]);
        assert_eq!(power.press(Press { button: Button::Right, repeat: false }), Answer::Nothing);
        assert_eq!(power.press(Press { button: Button::Left, repeat: false }), Answer::Nothing);
    }

    #[test]
    fn down_is_the_tile_below_or_the_last_tile_and_from_the_last_row_close() {
        let mut power = dialog(4);
        press(&mut power, &[Button::Up, Button::Up]);
        assert_eq!(press(&mut power, &[Button::Down]), Focus::Tile(3));
        assert_eq!(press(&mut power, &[Button::Down]), Focus::Close);
        // From the first row's last tile, with none under it: the last tile.
        press(&mut power, &[Button::Up, Button::Up, Button::Right, Button::Right]);
        assert_eq!(power.focus(), Focus::Tile(2));
        assert_eq!(press(&mut power, &[Button::Down]), Focus::Tile(3));
        // Up from the top row stays.
        press(&mut power, &[Button::Up]);
        assert_eq!(power.press(Press { button: Button::Up, repeat: false }), Answer::Nothing);
    }

    #[test]
    fn a_chooses_the_focused_tile_or_close_and_b_declines() {
        let mut power = dialog(4);
        assert_eq!(power.press(Press { button: Button::A, repeat: false }), Answer::Chose(None));
        press(&mut power, &[Button::Up]);
        assert_eq!(power.press(Press { button: Button::A, repeat: false }), Answer::Chose(Some(3)));
        assert_eq!(power.press(Press { button: Button::B, repeat: false }), Answer::Declined);
        assert_eq!(power.press(Press { button: Button::A, repeat: true }), Answer::Nothing);
        assert_eq!(power.press(Press { button: Button::B, repeat: true }), Answer::Nothing);
        assert_eq!(power.press(Press { button: Button::Down, repeat: true }), Answer::Moved);
        for button in [Button::X, Button::Y, Button::Menu, Button::View, Button::LeftBumper, Button::RightBumper] {
            assert_eq!(power.press(Press { button, repeat: false }), Answer::Nothing, "{button:?}");
        }
    }
}
