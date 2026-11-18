//! The menu panel: a dark panel with a title and rows of an icon and a label, one of them focused. A menu offers
//! what can be done with something; a prompt asks a question, its title wrapping to be read whole and its rows the
//! answers. The Guide shows them over itself (docs/spec/guide.md, section 6.1) and the Tile menu is made of them
//! (docs/spec/tile-menu.md, sections 2 and 5): one drawing, so the two can't drift apart.
//!
//! A panel knows its rows and where focus is among them, takes the pad, and draws itself where it is told. What a
//! row stands for is the caller's own type; what is behind the panel (a dim, a blurred Guide), how it comes and
//! goes, and what its sounds are, are the caller's too: [`MenuPanel::choose_sound`] says which the Bundle plays.
use std::hash::Hash;

use model::Sound;
use ui::{Button, Color, Ease, Font, HAlign, Press, Rect, TextStyle, Ui, VAlign};

use crate::drawn::{self, Drawing};
use crate::theme::{self, BODY, TEXT_DIM};

/// The panel is this wide, whatever is in it; it is as tall as its title and rows.
pub const WIDTH: f32 = 450.0;
const PANEL: Color = Color::rgb(0x2B2B2B);
const RADIUS: f32 = 6.0;
/// Its shadow: black at half strength, blurred by 24, all round it.
const SHADOW: (Color, f32) = (Color::argb(0x80000000), 24.0);
/// How far outside the panel anything of it is drawn: the shadow, and a focused row's glow.
const REACH: f32 = 26.0;
/// The title: Bahnschrift 33, 28 in from the sides, 36 down, 24 over what follows. Its line is WPF's for the font.
const TITLE: TextStyle = TextStyle::new(Font::Display, 33.0, 400);
const TITLE_LINE: f32 = 39.6;
const SIDE: f32 = 28.0;
const ABOVE_TITLE: f32 = 36.0;
const UNDER_TITLE: f32 = 24.0;
/// A prompt's message: 14 closer to the title than the rows would be, 20 over them. Segoe UI 26's line is 34.58.
const MESSAGE_PULL: f32 = 14.0;
const UNDER_MESSAGE: f32 = 20.0;
const MESSAGE_LINE: f32 = 26.0 * 1.33;
/// A row, and the fill a focused one has in its middle.
const ROW: f32 = 72.0;
const FILL: f32 = 56.0;
const FOCUSED: Color = Color::rgb(0x404040);
/// The room over a row that starts a group of its own (the Tile menu's Customise).
const GAP: f32 = 21.0;
/// A row's mark in its icon's place (a launcher's, in "Install from"): this square, on the icon's centre.
const MARK: f32 = 26.0;

/// The dim under a panel that stands in the middle of the screen: black at 60%, there at once and gone at once.
pub const DIM: Color = Color::argb(0x99000000);
/// Such a panel flies up from this far under its place as it opens, and back down to it as it closes.
pub const FLY: f32 = 80.0;
const OPEN_SECONDS: f32 = 0.25;
const CLOSE_SECONDS: f32 = 0.15;

/// Where a panel that flies up into the middle of the screen is and how much of it shows, on its way in or,
/// `leaving`, out: how far under its place, and its opacity. `id` tells one flight from another: each panel that
/// takes the middle flies up afresh. Under Reduce motion it is in its place, or gone, at once. Out, it has gone
/// when it shows nothing or has reached [`FLY`].
pub fn flight(ui: &mut Ui, id: impl Hash + Copy, leaving: bool) -> (f32, f32) {
    let ids = (("menu panel fly", id), ("menu panel fade", id));
    let still = ui.store.reduce_motion;
    if leaving {
        let fly = ui.slide(ids.0, FLY, CLOSE_SECONDS, Ease::CubicIn);
        (fly, ui.tween(ids.1, 0.0, if still { 0.0 } else { CLOSE_SECONDS }, Ease::Linear))
    } else {
        let fly = ui.slide_from(ids.0, FLY, 0.0, OPEN_SECONDS, Ease::CubicOut);
        (fly, ui.tween_from(ids.1, 0.0, 1.0, if still { 0.0 } else { OPEN_SECONDS }, Ease::Linear))
    }
}

/// The icons the Bundle's menus and prompts use, in Segoe Fluent Icons.
pub mod icon {
    pub const PLAY: char = '\u{E768}';
    pub const PAUSE: char = '\u{E769}';
    /// Cancel an install: the cross.
    pub const CANCEL_INSTALL: char = '\u{E711}';
    pub const QUIT: char = '\u{E711}';
    pub const CANCEL: char = '\u{E72B}';
    pub const FAVOURITE: char = '\u{E734}';
    pub const UNFAVOURITE: char = '\u{E8D9}';
    pub const MANAGE: char = '\u{E7FC}';
    pub const INSTALL: char = '\u{E896}';
    pub const UNINSTALL: char = '\u{E74D}';
    pub const CUSTOMISE: char = '\u{E771}';
    pub const MORE_OPTIONS: char = '\u{E712}';
    pub const GAME_ACTION: char = '\u{E945}';
    pub const HIDE: char = '\u{ED1A}';
    pub const UNHIDE: char = '\u{E7B3}';
    pub const HDR: char = '\u{E793}';
    pub const EXTENSIONS: char = '\u{EA86}';
    pub const REMOVE_GAME: char = '\u{E738}';
    pub const MOVE_UP: char = '\u{E74A}';
    pub const MOVE_DOWN: char = '\u{E74B}';
    pub const EDIT: char = '\u{E70F}';
    pub const DELETE: char = '\u{E74D}';
    /// Refresh (Segoe Fluent Icons' Sync): fetch a game's art again.
    pub const REFRESH: char = '\u{E895}';
    /// Change art (Segoe Fluent Icons' Photo): choose a game's pictures.
    pub const CHANGE_ART: char = '\u{E91B}';
}

/// One row: what it shows, and what choosing it means to whoever made the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Row<T> {
    pub icon: char,
    pub label: String,
    pub value: T,
    /// The row starts a group of its own, 21 further down with no line drawn: the Tile menu's Customise.
    pub gap: bool,
    /// A drawing in the icon's place: a launcher's placeholder mark ("Install from", #249).
    pub mark: Option<Drawing>,
}

impl<T> Row<T> {
    pub fn new(icon: char, label: &str, value: T) -> Self {
        Self { icon, label: label.into(), value, gap: false, mark: None }
    }

    /// A row with a mark in its icon's place.
    pub fn marked(mark: Drawing, label: &str, value: T) -> Self {
        Self { mark: Some(mark), ..Self::new(' ', label, value) }
    }

    pub fn after_a_gap(self) -> Self {
        Self { gap: true, ..self }
    }
}

/// What a press did to a dialog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Answer<T> {
    /// Nothing: a button it has no use for, or a direction with nowhere to go.
    Nothing,
    /// Focus is on another option.
    Moved,
    /// A on an option.
    Chose(T),
    /// B: closed with no answer, which for a prompt is "no".
    Declined,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuPanel<T> {
    title: String,
    message: Option<String>,
    prompt: bool,
    rows: Vec<Row<T>>,
    focus: usize,
}

impl<T: Copy> MenuPanel<T> {
    /// A menu: its title on one line, cut short with an ellipsis. Focus starts on the first row.
    pub fn menu(title: &str, rows: Vec<Row<T>>) -> Self {
        Self { title: title.into(), message: None, prompt: false, rows, focus: 0 }
    }

    /// A prompt: its question as the title, wrapping; under it a message, if it has one; its answers as rows, the
    /// first of them, which has focus, the confirming one.
    pub fn prompt(title: &str, message: Option<&str>, rows: Vec<Row<T>>) -> Self {
        Self { title: title.into(), message: message.map(str::to_string), prompt: true, rows, focus: 0 }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// A prompt's message, changed: one that says something of the focused answer (the free space of a folder).
    pub fn set_message(&mut self, message: Option<&str>) {
        self.message = message.map(str::to_string);
    }

    pub fn is_prompt(&self) -> bool {
        self.prompt
    }

    pub fn rows(&self) -> &[Row<T>] {
        &self.rows
    }

    /// The focused row, from the top.
    pub fn focus(&self) -> usize {
        self.focus
    }

    /// Puts focus on a row: a panel coming back from behind another keeps the focus it is given.
    pub fn set_focus(&mut self, row: usize) {
        self.focus = row.min(self.rows.len().saturating_sub(1));
    }

    /// The sound of A on one of its rows, as the Bundle has it: a choice in a menu goes on with something, an answer
    /// to a prompt closes it. Opening either is `Sound::PromptOpen`, a move `Sound::GuideMove`, B
    /// `Sound::PromptClose`.
    pub fn choose_sound(&self) -> Sound {
        if self.prompt {
            Sound::PromptClose
        } else {
            Sound::GuideSelect
        }
    }

    /// The pad while the panel has it. Up and Down move along the rows and stop at the ends, and are all that
    /// repeats; A chooses the focused row; B declines. Nothing else does anything.
    pub fn press(&mut self, press: Press) -> Answer<T> {
        let last = self.rows.len().saturating_sub(1);
        let to = match press.button {
            Button::Up => self.focus.saturating_sub(1),
            Button::Down => (self.focus + 1).min(last),
            Button::A if !press.repeat => {
                return self.rows.get(self.focus).map_or(Answer::Nothing, |row| Answer::Chose(row.value))
            }
            Button::B if !press.repeat => return Answer::Declined,
            _ => return Answer::Nothing,
        };
        if to == self.focus {
            return Answer::Nothing;
        }
        self.focus = to;
        Answer::Moved
    }

    /// How many lines the title takes: one in a menu, as many as the question needs in a prompt.
    fn title_lines(&self, ui: &mut Ui) -> usize {
        if !self.prompt {
            return 1;
        }
        ui.measure_wrapped(&TITLE, &self.title, WIDTH - 2.0 * SIDE, 0).lines.max(1)
    }

    fn message_lines(&self, ui: &mut Ui) -> usize {
        let Some(message) = &self.message else { return 0 };
        ui.measure_wrapped(&BODY, message, WIDTH - 2.0 * SIDE, 0).lines.max(1)
    }

    /// How far under the panel's top its first row starts: the title, and a prompt's message.
    fn header(&self, ui: &mut Ui) -> f32 {
        let title = ABOVE_TITLE + self.title_lines(ui) as f32 * TITLE_LINE + UNDER_TITLE;
        match self.message_lines(ui) {
            0 => title,
            lines => title - MESSAGE_PULL + lines as f32 * MESSAGE_LINE + UNDER_MESSAGE,
        }
    }

    /// The panel's height: its header, its rows, and the gap over each row that starts a group.
    pub fn height(&self, ui: &mut Ui) -> f32 {
        let gaps = self.rows.iter().filter(|row| row.gap).count();
        self.header(ui) + self.rows.len() as f32 * ROW + gaps as f32 * GAP
    }

    /// Where the panel's top left is when it stands in the middle of the screen.
    pub fn centred(&self, ui: &mut Ui) -> (f32, f32) {
        let (screen, height) = (ui.screen(), self.height(ui));
        (screen.x + (screen.w - WIDTH) / 2.0, (screen.h - height) / 2.0)
    }

    /// Everything the panel draws when its top left is at `at`: the panel, its shadow and a focused row's ring and
    /// glow, which hang outside it. What a group it is drawn in must hold.
    pub fn bounds(&self, ui: &mut Ui, at: (f32, f32)) -> Rect {
        Rect::new(at.0, at.1, WIDTH, self.height(ui)).grow(REACH)
    }

    /// Draws the panel with its top left at `at`, in the layer being drawn: the caller starts one first if the
    /// panel is to cover text under it.
    pub fn draw(&self, ui: &mut Ui, at: (f32, f32)) {
        // As WPF's layout rounding: every edge on a whole pixel, each worked out from the panel's own place.
        let px = ui.px();
        let snap = |y: f32| (y * px).round() / px;
        let (x, top) = (snap(at.0), at.1);
        let rows_top = top + self.header(ui);
        let foot = snap(top + self.height(ui));
        let panel = Rect::new(x, snap(top), WIDTH, foot - snap(top));
        ui.shadow(panel, SHADOW.0, RADIUS, SHADOW.1);
        ui.fill(panel, PANEL, RADIUS);

        let lines = self.title_lines(ui);
        let title = Rect::new(x + SIDE, snap(top + ABOVE_TITLE), WIDTH - 2.0 * SIDE, lines as f32 * TITLE_LINE + 4.0);
        ui.paragraph(title, &self.title, &TITLE, HAlign::Left, VAlign::Top, if self.prompt { 0 } else { 1 });
        if let Some(message) = &self.message {
            let y = top + ABOVE_TITLE + lines as f32 * TITLE_LINE + UNDER_TITLE - MESSAGE_PULL;
            let height = self.message_lines(ui) as f32 * MESSAGE_LINE + 4.0;
            let message_box = Rect::new(x + SIDE, snap(y), WIDTH - 2.0 * SIDE, height);
            ui.paragraph(message_box, message, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Top, 0);
        }

        // Each row's top, a gap further down for one that starts a group.
        let mut y = rows_top;
        let mut tops = Vec::with_capacity(self.rows.len());
        for row in &self.rows {
            y += if row.gap { GAP } else { 0.0 };
            tops.push(snap(y));
            y += ROW;
        }
        // The focused row's fill is under what the row shows; its ring is over its neighbours' room: the row's
        // whole 72, standing 8 outside the panel on either side.
        let fill = tops.get(self.focus).map(|top| Rect::new(x, top + (ROW - FILL) / 2.0, WIDTH, FILL));
        if let Some(fill) = fill {
            ui.fill(fill, FOCUSED, 2.0);
        }
        for (row, top) in self.rows.iter().zip(&tops) {
            let middle = top + ROW / 2.0;
            match row.mark {
                Some(mark) => {
                    drawn::draw(ui, Rect::new(x + 44.0 - MARK / 2.0, middle - MARK / 2.0, MARK, MARK), mark, Color::WHITE);
                }
                None => {
                    ui.icon(x + 44.0, middle, row.icon, 22.0, Color::WHITE);
                }
            }
            ui.label_middle(x + 88.0, middle, &row.label, &BODY);
        }
        if let Some(fill) = fill {
            // WPF's corner of 7 is the stroke's centre line: 9 at its outer edge.
            theme::focus_ring_cornered(ui, fill, 9.0);
        }
    }

    /// Draws the panel centred on the screen by its own height, `fly` under its place, at `opacity`: a panel on
    /// its [`flight`]. Gives its place at rest.
    pub fn draw_flown(&self, ui: &mut Ui, fly: f32, opacity: f32) -> (f32, f32) {
        let at = self.centred(ui);
        if opacity >= 1.0 {
            self.draw(ui, (at.0, at.1 + fly));
        } else if opacity > 0.0 {
            self.draw_faded(ui, (at.0, at.1 + fly), opacity);
        }
        at
    }

    /// Draws the panel as one picture in a layer of its own, faded as a whole to `opacity`: a panel flying in, or
    /// the menu behind its flyout, whose shadow and rows must not show through its fading back.
    pub fn draw_faded(&self, ui: &mut Ui, at: (f32, f32), opacity: f32) {
        let bounds = self.bounds(ui, at);
        ui.group(bounds, 0.0, opacity, |ui| self.draw(ui, at));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(panel: &mut MenuPanel<u8>, button: Button) -> Answer<u8> {
        panel.press(Press { button, repeat: false })
    }

    fn menu() -> MenuPanel<u8> {
        let rows = vec![Row::new(icon::PLAY, "Play game", 1), Row::new(icon::FAVOURITE, "Add to Favourites", 2)];
        MenuPanel::menu(
            "Halyard Point",
            [rows, vec![Row::new(icon::CUSTOMISE, "Customise Home", 3).after_a_gap()]].concat(),
        )
    }

    #[test]
    fn focus_starts_on_the_first_row_moves_along_them_and_stops_at_the_ends() {
        let mut panel = menu();
        assert_eq!(panel.focus(), 0);
        assert_eq!(press(&mut panel, Button::Up), Answer::Nothing);
        assert_eq!(press(&mut panel, Button::Down), Answer::Moved);
        // The gap before a group is not a stop.
        assert_eq!(press(&mut panel, Button::Down), Answer::Moved);
        assert_eq!(panel.focus(), 2);
        assert_eq!(press(&mut panel, Button::Down), Answer::Nothing);
        assert_eq!(press(&mut panel, Button::Up), Answer::Moved);
        assert_eq!(panel.focus(), 1);
    }

    #[test]
    fn a_chooses_the_focused_row_b_declines_and_nothing_else_does_anything() {
        let mut panel = menu();
        press(&mut panel, Button::Down);
        assert_eq!(press(&mut panel, Button::A), Answer::Chose(2));
        assert_eq!(press(&mut panel, Button::B), Answer::Declined);
        let others = [Button::Left, Button::Right, Button::X, Button::Y, Button::Menu, Button::View];
        for button in others.into_iter().chain([Button::LeftBumper, Button::RightBumper, Button::Guide]) {
            assert_eq!(press(&mut panel, button), Answer::Nothing, "{button:?}");
        }
        assert_eq!(panel.focus(), 1);
    }

    #[test]
    fn only_a_direction_repeats() {
        let mut panel = menu();
        assert_eq!(panel.press(Press { button: Button::Down, repeat: true }), Answer::Moved);
        assert_eq!(panel.press(Press { button: Button::A, repeat: true }), Answer::Nothing);
        assert_eq!(panel.press(Press { button: Button::B, repeat: true }), Answer::Nothing);
    }

    #[test]
    fn a_menus_choice_sounds_as_a_selection_and_a_prompts_answer_as_its_closing() {
        assert_eq!(menu().choose_sound(), Sound::GuideSelect);
        let prompt = MenuPanel::prompt("Remove Halyard Point?", None, vec![Row::new(icon::REMOVE_GAME, "Remove", 1)]);
        assert_eq!(prompt.choose_sound(), Sound::PromptClose);
        assert!(prompt.is_prompt() && !menu().is_prompt());
    }

    #[test]
    fn focus_given_to_a_panel_stays_within_its_rows() {
        let mut panel = menu();
        panel.set_focus(1);
        assert_eq!(panel.focus(), 1);
        panel.set_focus(9);
        assert_eq!(panel.focus(), 2);
    }
}
