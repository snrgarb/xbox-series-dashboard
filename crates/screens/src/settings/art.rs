//! Games & apps > Art (docs/spec/settings.md, 16.8): the user's own SteamGridDB key, which art no store has for a
//! game is looked up with, "Clear downloaded art", "Refresh all art and details" (#190), which forgets everything
//! downloaded and fetches it again in the background, and "Download art online", which turns every download of art
//! off and on. The Bundle has no such page (Playnite's metadata sources did this); it is laid out as the Bundle's Discord
//! page is (section 6.9): a label, its control 14 under, then the next group 40 under; only the key keeps a line, the
//! one place a key is made (#261).
//!
//! The key is typed on the Console keyboard, shown as dots, and goes to the shell as `Effect::ArtKey` the moment it
//! is kept. Nothing here ever has it again: the page shows dots while there is one, "Not set" while there is not.
//!
//! Last, "Art sources" (#246): the four online sources in the order they are asked, each a Row ticked while it is
//! on and numbered at its right end by its place among the sources that are on (#262). A turns a source on or off;
//! Menu opens its menu, the Tile menu's panel over the dimmed page, which moves it up or down, as Settings > Groups
//! moves a Group (`groups.rs`). Each change goes to the shell as `Effect::ArtOrder`.
//! The page scrolls to keep what has focus on screen.
use std::sync::atomic::{AtomicU32, Ordering};

use model::{ArtOrder, ArtRefresh, ArtSource};
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, glyph, FIELD_HEIGHT, FIELD_WIDTH, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH, TOP};
use super::{Change, Pane, Setting, Step};
use crate::hint_bar::{self, Hint};
use crate::keyboard::{self, Event};
use crate::menu_panel::{self, icon, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, Open, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::{Effect, Secret};

const KEY_LABEL: &str = "SteamGridDB key";
/// Where a key is made: the one line the page cannot do without.
const KEY_ABOUT: &str = "Make one at steamgriddb.com, under Preferences and API.";
const ART_LABEL: &str = "Downloaded art";
const CLEAR_ART: &str = "Clear downloaded art";
const REFRESH: &str = "Refresh all art and details";
/// The line under the two buttons, as a refresh goes: none at rest.
const REFRESH_PAUSED: &str = "Refresh paused: it goes on once Download art online is on.";
const ONLINE_LABEL: &str = "Art online";
const SOURCES_LABEL: &str = "Art sources";
const ONLINE: &str = "Download art online";
/// What the value reads while a key is kept: as many dots whatever its length.
pub const DOTS: &str = "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}";
const NOT_SET: &str = "Not set";
/// The key on the Console keyboard: shown as dots, and long enough for any key the service gives.
const ENTRY: keyboard::Entry = keyboard::Entry::new(KEY_LABEL).masked().limit(64);
/// The value's column, then the two buttons, as Discord's Client ID row (6.9).
const VALUE_WIDTH: f32 = 420.0;
const TYPE_X: f32 = 940.0;
const CLEAR_X: f32 = 1140.0;
const BUTTON_WIDTH: f32 = 180.0;
/// Clear downloaded art and Refresh side by side, as Discord's Connect and Disconnect (6.9), and the line that says
/// how a refresh goes under them, as Discord's status line.
const BUTTONS_APART: f32 = 20.0;
const LINE_UNDER: f32 = 24.0;
/// A source's number on its Row: right-aligned in a box this wide, as far in from the Row's right end as its tick is
/// from the left (the tick's middle 45 in, 12 either side of it).
const NUMBER_WIDTH: f32 = 60.0;
const NUMBER_IN: f32 = 33.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Focus {
    #[default]
    Type,
    Clear,
    ClearArt,
    Refresh,
    Online,
    /// A source's Row, by the source: focus stays on it wherever it moves.
    Source(ArtSource),
}

impl Focus {
    fn name(self) -> &'static str {
        match self {
            Focus::Type => "Type",
            Focus::Clear => "Clear",
            Focus::ClearArt => CLEAR_ART,
            Focus::Refresh => REFRESH,
            Focus::Online => ONLINE,
            Focus::Source(source) => source.name(),
        }
    }
}

/// What a row of a source's menu does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Up,
    Down,
}

/// A source's menu, as Groups' (filters.md 6.2): Move up but for the first, Move down but for the last.
fn source_rows(at: usize, count: usize) -> Vec<Row<Choice>> {
    let mut rows = Vec::new();
    if at > 0 {
        rows.push(Row::new(icon::MOVE_UP, "Move up", Choice::Up));
    }
    if at + 1 < count {
        rows.push(Row::new(icon::MOVE_DOWN, "Move down", Choice::Down));
    }
    rows
}

/// A source's menu while it is up.
struct Menu {
    source: ArtSource,
    panel: MenuPanel<Choice>,
}

/// Where each control is, before any scrolling: what drawing and scrolling both go by.
struct Layout {
    key_label: f32,
    key_about: f32,
    key_row: f32,
    art_label: f32,
    buttons: f32,
    line: f32,
    line_height: f32,
    online_label: f32,
    online: f32,
    sources_label: f32,
    rows: f32,
}

impl Layout {
    /// The key: its label, where a key is made 4 under it, and its row 30 under that; 40 on, what was downloaded:
    /// its label, the buttons 14 under and, once a refresh has been asked for, the line 24 under them; 40 on, Art
    /// online: its label and the checkbox 14 under; 40 on, the sources: label, and the Rows 14 under.
    fn of(ui: &mut Ui, refresh: ArtRefresh) -> Self {
        let label = ui.measure(&BODY, KEY_LABEL).height;
        let key_label = TOP;
        let key_about = key_label + label + 4.0;
        let key_row = key_about + ui.measure_wrapped(&BODY, KEY_ABOUT, ROW_WIDTH, 0).height + 30.0;
        let art_label = key_row + FIELD_HEIGHT + 40.0;
        let buttons = art_label + label + 14.0;
        let (line, line_height) = match refresh_line(refresh) {
            Some((said, _)) => {
                (buttons + FIELD_HEIGHT + LINE_UNDER, ui.measure_wrapped(&BODY, &said, ROW_WIDTH, 1).height)
            }
            None => (buttons + FIELD_HEIGHT, 0.0),
        };
        let online_label = line + line_height + 40.0;
        let online = online_label + label + 14.0;
        let sources_label = online + FIELD_HEIGHT + 40.0;
        let rows = sources_label + label + 14.0;
        Self {
            key_label,
            key_about,
            key_row,
            art_label,
            buttons,
            line,
            line_height,
            online_label,
            online,
            sources_label,
            rows,
        }
    }

    /// Source Row i.
    fn row(&self, i: usize) -> Rect {
        Rect::new(LEFT, self.rows + ROW_PITCH * i as f32, ROW_WIDTH, ROW_HEIGHT)
    }

    /// What must show of the control with focus: from its top to its foot.
    fn span(&self, focus: Focus, order: &ArtOrder) -> (f32, f32) {
        match focus {
            Focus::Type | Focus::Clear => (self.key_label, self.key_row + FIELD_HEIGHT),
            Focus::ClearArt | Focus::Refresh => (self.buttons, self.buttons + FIELD_HEIGHT),
            Focus::Online => (self.online, self.online + FIELD_HEIGHT),
            Focus::Source(source) => {
                let row = self.row(order.place(source));
                (row.y, row.bottom())
            }
        }
    }
}

/// The answers to "Clear downloaded art?" and "Refresh all art and details?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Clear,
    Refresh,
    Cancel,
}

/// What the line under Clear downloaded art and Refresh says of a refresh, and its colour; nothing while none has
/// been asked for.
fn refresh_line(refresh: ArtRefresh) -> Option<(String, Color)> {
    let said = match refresh {
        ArtRefresh::Idle => return None,
        ArtRefresh::Working { done, of } => format!("Refreshing: {done} of {of} {}...", games(of)),
        ArtRefresh::Paused => REFRESH_PAUSED.into(),
        ArtRefresh::Done { pictures: 1 } => "Refreshed: 1 picture downloaded.".into(),
        ArtRefresh::Done { pictures } => format!("Refreshed: {pictures} pictures downloaded."),
    };
    Some((said, Color::WHITE))
}

fn games(n: usize) -> &'static str {
    if n == 1 {
        "game"
    } else {
        "games"
    }
}

/// How a refresh goes, as the focus log says it after the page's focus: none while none was asked for.
fn refresh_state(refresh: ArtRefresh) -> Option<String> {
    match refresh {
        ArtRefresh::Idle => None,
        ArtRefresh::Working { done, of } => Some(format!("refreshing {done} of {of}")),
        ArtRefresh::Paused => Some("refresh paused".into()),
        ArtRefresh::Done { pictures } => Some(format!("refreshed: {pictures} pictures")),
    }
}

/// Each prompt flies up afresh: a number for every one.
static FLIGHTS: AtomicU32 = AtomicU32::new(0);

#[derive(Default)]
pub struct Art {
    focus: Focus,
    prompt: Option<MenuPanel<Confirm>>,
    flying: u32,
    /// The prompt on its way out, and its flight.
    leaving: Option<(MenuPanel<Confirm>, u32)>,
    /// A key text entry kept, to be asked of the shell.
    asks: Vec<Effect>,
    /// The question's button: Clear downloaded art or Refresh, which its focus line names.
    asking: Focus,
    /// How a refresh goes, as the library last said: for the focus line.
    refresh: ArtRefresh,
    /// The sources' order as last shown: for the focus line.
    order: ArtOrder,
    /// A source's menu, while it is up; and one on its way out, with its flight.
    menu: Option<Menu>,
    menu_leaving: Option<(MenuPanel<Choice>, u32)>,
    /// How far the page has scrolled up to show what has focus.
    scroll: f32,
}

impl Art {
    /// The page as it opens, a refresh under way said at once.
    pub fn open(around: Around) -> Self {
        Self { refresh: around.library.art_refresh, order: around.options.art_order, ..Self::default() }
    }

    /// The menu for the source focus is on.
    fn open_menu(&mut self, order: &ArtOrder) {
        let Focus::Source(source) = self.focus else { return };
        let panel = MenuPanel::menu(source.name(), source_rows(order.place(source), order.0.len()));
        self.menu = Some(Menu { source, panel });
        self.flying = FLIGHTS.fetch_add(1, Ordering::Relaxed);
        self.menu_leaving = None;
    }

    fn close_menu(&mut self) {
        if let Some(menu) = self.menu.take() {
            self.menu_leaving = Some((menu.panel, self.flying));
        }
    }

    /// A press while a source's menu is up: every button is the menu's. A move is asked of the shell at once, and
    /// the menu stays up with its rows as they are for the source's new place, focus on the same row if it is
    /// still there.
    fn menu_press(&mut self, press: Press, order: ArtOrder) -> Step {
        let Some(menu) = self.menu.as_mut() else { return Step::Stay };
        let by = match menu.panel.press(press) {
            Answer::Chose(Choice::Up) => -1,
            Answer::Chose(Choice::Down) => 1,
            Answer::Declined => {
                self.close_menu();
                return Step::Stay;
            }
            Answer::Moved | Answer::Nothing => return Step::Stay,
        };
        let was = menu.panel.rows().get(menu.panel.focus()).map(|row| row.value);
        let moved = order.moved(menu.source, by);
        let rows = source_rows(moved.place(menu.source), moved.0.len());
        let focus = rows.iter().position(|row| Some(row.value) == was).unwrap_or(0);
        menu.panel = MenuPanel::menu(menu.source.name(), rows);
        menu.panel.set_focus(focus);
        self.order = moved;
        Step::Do(PageAction::Effect(Effect::ArtOrder(moved)))
    }

    fn prompt_press(&mut self, press: Press) -> Step {
        let Some(prompt) = self.prompt.as_mut() else { return Step::Stay };
        let chose = match prompt.press(press) {
            Answer::Chose(confirm) => Some(confirm),
            Answer::Declined => Some(Confirm::Cancel),
            Answer::Moved | Answer::Nothing => None,
        };
        let Some(confirm) = chose else { return Step::Stay };
        if let Some(prompt) = self.prompt.take() {
            self.leaving = Some((prompt, self.flying));
        }
        match confirm {
            Confirm::Clear => Step::Do(PageAction::Effect(Effect::ClearArt)),
            Confirm::Refresh => Step::Do(PageAction::Effect(Effect::RefreshArt(None))),
            Confirm::Cancel => Step::Stay,
        }
    }
}

impl Pane for Art {
    fn header(&self) -> String {
        "Art".into()
    }

    /// `Art: Type`, `Art: Clear`, `Art: Clear downloaded art`, `Art: Refresh all art and details`, `Art: Download
    /// art online`; with a question up, `Art: Prompt(Clear) of Clear downloaded art`. On a source's Row, its place
    /// among the Rows, and whether it is on with the number its Row shows: `Art: Source Xbox, 2 of 4, on, number 2`,
    /// `Art: Source PlayStation, 3 of 4, off`; with its menu up, `Art: Menu(Move up) of Xbox`.
    fn focus(&self) -> String {
        if let Some(menu) = &self.menu {
            let row = menu.panel.rows().get(menu.panel.focus()).map_or("", |row| row.label.as_str());
            return format!("Art: Menu({row}) of {}", menu.source.name());
        }
        if let (Focus::Source(source), None) = (self.focus, &self.prompt) {
            let (at, of) = (self.order.place(source) + 1, self.order.0.len());
            let number = match self.order.number(source) {
                Some(number) => format!("on, number {number}"),
                None => "off".into(),
            };
            return format!("Art: Source {}, {at} of {of}, {number}", source.name());
        }
        match &self.prompt {
            Some(prompt) => {
                let row = prompt.rows().get(prompt.focus()).map_or("", |row| row.label.as_str());
                format!("Art: Prompt({row}) of {}", self.asking.name())
            }
            None => format!("Art: {}", self.focus.name()),
        }
    }

    /// Once a refresh has been asked for, how it goes: `refreshing 3 of 14`, `refreshed: 9 pictures`, `refresh
    /// paused`; nothing before.
    fn state(&self) -> String {
        refresh_state(self.refresh).unwrap_or_default()
    }

    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        self.refresh = around.library.art_refresh;
        None
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        let order = around.options.art_order;
        self.order = order;
        if self.prompt.is_some() {
            return self.prompt_press(press);
        }
        if self.menu.is_some() {
            return self.menu_press(press, order);
        }
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        use Focus::*;
        let first = order.0[0].0;
        match (press.button, self.focus) {
            (Button::Down, Online) => self.focus = Source(first),
            (Button::Up, Source(source)) if order.place(source) == 0 => self.focus = Online,
            (Button::Up, Source(source)) => self.focus = Source(order.0[order.place(source) - 1].0),
            (Button::Down, Source(source)) => {
                let at = (order.place(source) + 1).min(order.0.len() - 1);
                self.focus = Source(order.0[at].0);
            }
            (Button::A, Source(source)) => {
                let switched = order.switched(source);
                self.order = switched;
                return Step::Do(PageAction::Effect(Effect::ArtOrder(switched)));
            }
            (Button::Menu, Source(_)) => self.open_menu(&order),
            (Button::Left, Source(_)) => return Step::Back,
            (Button::Right, Type) => self.focus = Clear,
            (Button::Left, Clear) => self.focus = Type,
            (Button::Right, ClearArt) => self.focus = Refresh,
            (Button::Left, Refresh) => self.focus = ClearArt,
            // Down onto a row of two is onto its first, and Up from a row of two onto Type, as before Refresh.
            (Button::Down, Type | Clear) => self.focus = ClearArt,
            (Button::Down, ClearArt | Refresh) => self.focus = Online,
            (Button::Up, ClearArt | Refresh) => self.focus = Type,
            (Button::Up, Online) => self.focus = ClearArt,
            // Left from the first column is B, as on Discord's page.
            (Button::B, _) | (Button::Left, Type | ClearArt | Online) => return Step::Back,
            // Pressed while a refresh is under way it does nothing, as Update now does.
            (Button::A, Refresh) if matches!(around.library.art_refresh, ArtRefresh::Working { .. }) => {}
            (Button::A, Refresh) => {
                let answers = vec![
                    Row::new(icon::REFRESH, "Refresh", Confirm::Refresh),
                    Row::new(icon::CANCEL, "Cancel", Confirm::Cancel),
                ];
                let message = "Everything downloaded for your games is removed and fetched again, in the background. \
                               Your games, play time, Groups and choices stay.";
                self.prompt = Some(MenuPanel::prompt(&format!("{REFRESH}?"), Some(message), answers));
                self.asking = Refresh;
                self.flying = FLIGHTS.fetch_add(1, Ordering::Relaxed);
                self.leaving = None;
            }
            (Button::A, Online) => {
                let change = Change { setting: Setting::DownloadArtOnline, on: !around.options.art_online };
                return Step::Do(PageAction::Effect(Effect::Set(change)));
            }
            (Button::A, Type) => return Step::Do(PageAction::Open(Open::TextEntry(ENTRY))),
            (Button::A, Clear) if around.options.art_key => {
                return Step::Do(PageAction::Effect(Effect::ArtKey(None)));
            }
            (Button::A, ClearArt) => {
                let answers = vec![
                    Row::new(icon::DELETE, "Clear", Confirm::Clear),
                    Row::new(icon::CANCEL, "Cancel", Confirm::Cancel),
                ];
                let message = "The art downloaded for your games is removed from this PC. A store's own art stays.";
                self.prompt = Some(MenuPanel::prompt(&format!("{CLEAR_ART}?"), Some(message), answers));
                self.asking = ClearArt;
                self.flying = FLIGHTS.fetch_add(1, Ordering::Relaxed);
                self.leaving = None;
            }
            _ => {}
        }
        Step::Stay
    }

    fn dialog(&self) -> Option<Dialog> {
        if self.menu.is_some() {
            return Some(Dialog::Menu);
        }
        self.prompt.as_ref().map(|_| Dialog::Prompt)
    }

    fn dismiss_dialog(&mut self) {
        self.prompt = None;
        self.leaving = None;
        self.menu = None;
        self.menu_leaving = None;
    }

    fn hints(&self, _around: Around) -> &'static [Hint] {
        match self.focus {
            Focus::Source(_) => hint_bar::MORE_OPTIONS,
            _ => hint_bar::NONE,
        }
    }

    /// Text entry starts empty: the key kept is never shown, nor typed over.
    fn entry_text(&self) -> String {
        String::new()
    }

    /// A key kept is asked of the shell; an empty one leaves the key that is kept as it is.
    fn entry(&mut self, event: Event, _around: Around) {
        if let Event::Kept(text) = event {
            let key = text.trim();
            if !key.is_empty() {
                self.asks.push(Effect::ArtKey(Some(Secret(key.to_string()))));
            }
        }
    }

    fn asks(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.asks)
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        self.refresh = around.library.art_refresh;
        self.order = around.options.art_order;
        let order = self.order;
        let layout = Layout::of(ui, around.library.art_refresh);
        let (top, bottom) = layout.span(self.focus, &order);
        let focus = self.focus;
        let has = |which| focused && focus == which;
        controls::scrolled(ui, &mut self.scroll, TOP, top, bottom, |ui| {
            controls::label(ui, LEFT, layout.key_label, KEY_LABEL);
            controls::description(ui, LEFT, layout.key_about, ROW_WIDTH, KEY_ABOUT);
            let y = layout.key_row;
            let (value, colour) = if around.options.art_key { (DOTS, Color::WHITE) } else { (NOT_SET, TEXT_DIM) };
            let value_box = Rect::new(LEFT, y, VALUE_WIDTH, FIELD_HEIGHT);
            ui.paragraph(value_box, value, &BODY.color(colour), HAlign::Left, VAlign::Center, 1);
            let type_box = Rect::new(TYPE_X, y, BUTTON_WIDTH, FIELD_HEIGHT);
            let clear_box = Rect::new(CLEAR_X, y, BUTTON_WIDTH, FIELD_HEIGHT);
            // What was downloaded: its label and the two buttons.
            controls::label(ui, LEFT, layout.art_label, ART_LABEL);
            let y = layout.buttons;
            let clear_art_box = Rect::new(LEFT, y, controls::pane_button_width(ui, CLEAR_ART), FIELD_HEIGHT);
            let refresh_x = clear_art_box.right() + BUTTONS_APART;
            let refresh_box = Rect::new(refresh_x, y, controls::pane_button_width(ui, REFRESH), FIELD_HEIGHT);
            // How a refresh goes, one line under the two once one has been asked for: a spinner before it while it
            // works.
            if let Some((said, colour)) = refresh_line(around.library.art_refresh) {
                let (y, height) = (layout.line, layout.line_height);
                let mut line = Rect::new(LEFT, y, ROW_WIDTH, height);
                if let ArtRefresh::Working { .. } = around.library.art_refresh {
                    crate::spinner::spinner(ui, LEFT + 16.0, y + height / 2.0, 32.0);
                    line = Rect::new(LEFT + 48.0, y, ROW_WIDTH - 48.0, height);
                }
                ui.paragraph(line, &said, &BODY.color(colour), HAlign::Left, VAlign::Center, 1);
            }
            // Art online: its label and the checkbox.
            controls::label(ui, LEFT, layout.online_label, ONLINE_LABEL);
            let online = Rect::new(LEFT, layout.online, FIELD_WIDTH, FIELD_HEIGHT);
            controls::checkbox_row(ui, online, ONLINE, around.options.art_online, has(Focus::Online));
            // The sources: label, and a Row each in their order, ticked while on.
            controls::label(ui, LEFT, layout.sources_label, SOURCES_LABEL);
            let source_row = |ui: &mut Ui, i: usize, focused: bool| {
                let (source, on) = order.0[i];
                let row = layout.row(i);
                controls::row_marked(ui, row, source.name(), on.then_some(glyph::TICK), focused);
                // Its number in the order at the Row's right end; none while it is off.
                if let Some(number) = order.number(source) {
                    let number_box = Rect::new(row.right() - NUMBER_IN - NUMBER_WIDTH, row.y, NUMBER_WIDTH, row.h);
                    ui.paragraph(number_box, &number.to_string(), &BODY, HAlign::Right, VAlign::Center, 1);
                }
            };
            let at = match focus {
                Focus::Source(source) => Some(order.place(source)),
                _ => None,
            };
            for i in (0..order.0.len()).filter(|&i| !(focused && Some(i) == at)) {
                source_row(ui, i, false);
            }
            // The focused one last, its ring over its neighbours; it keeps its ring under its menu.
            if let (true, Some(at)) = (focused, at) {
                source_row(ui, at, true);
            }
            // The focused button last, its ring over its neighbours.
            let buttons = [
                (Focus::Type, type_box, "Type", true),
                (Focus::Clear, clear_box, "Clear", around.options.art_key),
                (Focus::ClearArt, clear_art_box, CLEAR_ART, true),
                (
                    Focus::Refresh,
                    refresh_box,
                    REFRESH,
                    !matches!(around.library.art_refresh, ArtRefresh::Working { .. }),
                ),
            ];
            for (which, rect, label, enabled) in buttons.iter().filter(|b| !has(b.0)) {
                controls::pane_button(ui, *rect, label, *enabled, has(*which));
            }
            for (_, rect, label, enabled) in buttons.iter().filter(|b| has(b.0)) {
                controls::pane_button(ui, *rect, label, *enabled, true);
            }
        });
        // The question or the menu over the dimmed page; or on its way out, the dim gone at once.
        if let Some(prompt) = &self.prompt {
            ui.layer();
            ui.fill(ui.screen(), DIM, 0.0);
            let (fly, shown) = menu_panel::flight(ui, ("art prompt", self.flying), false);
            prompt.draw_flown(ui, fly, shown);
        } else if let Some(menu) = &self.menu {
            ui.layer();
            ui.fill(ui.screen(), DIM, 0.0);
            let (fly, shown) = menu_panel::flight(ui, ("art prompt", self.flying), false);
            menu.panel.draw_flown(ui, fly, shown);
        } else if let Some((prompt, flying)) = &self.leaving {
            ui.layer();
            let (fly, shown) = menu_panel::flight(ui, ("art prompt", *flying), true);
            if shown <= 0.0 || fly >= FLY {
                self.leaving = None;
            } else {
                prompt.draw_flown(ui, fly, shown);
            }
        } else if let Some((panel, flying)) = &self.menu_leaving {
            ui.layer();
            let (fly, shown) = menu_panel::flight(ui, ("art prompt", *flying), true);
            if shown <= 0.0 || fly >= FLY {
                self.menu_leaving = None;
            } else {
                panel.draw_flown(ui, fly, shown);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(at: usize, count: usize) -> Vec<String> {
        source_rows(at, count).into_iter().map(|row| row.label).collect()
    }

    #[test]
    fn a_sources_menu_moves_it_up_or_down_as_far_as_it_can_go() {
        assert_eq!(labels(0, 4), ["Move down"]);
        assert_eq!(labels(1, 4), ["Move up", "Move down"]);
        assert_eq!(labels(3, 4), ["Move up"]);
    }
}
