//! Theme settings > Art (docs/spec/settings.md, 16.8): the user's own SteamGridDB key, which art for the games a
//! store gives none for is looked up with, and "Clear downloaded art". The Bundle has no such page (Playnite's
//! metadata sources did this); it is laid out as the Bundle's Discord page is (section 6.9): a label, its
//! description, a value with its pane buttons, then the next group 40 under.
//!
//! The key is typed on the Console keyboard, shown as dots, and goes to the shell as `Effect::ArtKey` the moment it
//! is kept. Nothing here ever has it again: the page shows dots while there is one, "Not set" while there is not.
use std::sync::atomic::{AtomicU32, Ordering};

use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, FIELD_HEIGHT, LEFT, ROW_WIDTH, TOP};
use super::{Pane, Step};
use crate::keyboard::{self, Event};
use crate::menu_panel::{self, icon, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, Open, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::{Effect, Secret};

const KEY_LABEL: &str = "SteamGridDB key";
const KEY_ABOUT: &str =
    "Art for games their store has none for comes from SteamGridDB, with your own API key: make one \
                         at steamgriddb.com, under Preferences and API. Only a game's store id or name is sent.";
const ART_LABEL: &str = "Downloaded art";
const ART_ABOUT: &str =
    "Art downloaded for your games is kept on this PC. Clearing it removes only that: a store's own \
                         art stays.";
const CLEAR_ART: &str = "Clear downloaded art";
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Focus {
    #[default]
    Type,
    Clear,
    ClearArt,
}

impl Focus {
    fn name(self) -> &'static str {
        match self {
            Focus::Type => "Type",
            Focus::Clear => "Clear",
            Focus::ClearArt => CLEAR_ART,
        }
    }
}

/// The answers to "Clear downloaded art?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Clear,
    Cancel,
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
}

impl Art {
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
            Confirm::Cancel => Step::Stay,
        }
    }
}

impl Pane for Art {
    fn header(&self) -> String {
        "Art".into()
    }

    /// `Art: Type`, `Art: Clear`, `Art: Clear downloaded art`; with the question up, `Art: Prompt(Clear) of Clear
    /// downloaded art`.
    fn focus(&self) -> String {
        match &self.prompt {
            Some(prompt) => {
                let row = prompt.rows().get(prompt.focus()).map_or("", |row| row.label.as_str());
                format!("Art: Prompt({row}) of {CLEAR_ART}")
            }
            None => format!("Art: {}", self.focus.name()),
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        if self.prompt.is_some() {
            return self.prompt_press(press);
        }
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        use Focus::*;
        match (press.button, self.focus) {
            (Button::Right, Type) => self.focus = Clear,
            (Button::Left, Clear) => self.focus = Type,
            (Button::Down, Type | Clear) => self.focus = ClearArt,
            (Button::Up, ClearArt) => self.focus = Type,
            // Left from the first column is B, as on Discord's page.
            (Button::B, _) | (Button::Left, Type | ClearArt) => return Step::Back,
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
                self.flying = FLIGHTS.fetch_add(1, Ordering::Relaxed);
                self.leaving = None;
            }
            _ => {}
        }
        Step::Stay
    }

    fn dialog(&self) -> Option<Dialog> {
        self.prompt.as_ref().map(|_| Dialog::Prompt)
    }

    fn dismiss_dialog(&mut self) {
        self.prompt = None;
        self.leaving = None;
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
        let has = |focus| focused && self.focus == focus;
        // The key: its label, the description 4 under it, and its row 30 under that.
        let mut y = TOP;
        y += controls::label(ui, LEFT, y, KEY_LABEL) + 4.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, KEY_ABOUT) + 30.0;
        let (value, colour) = if around.options.art_key { (DOTS, Color::WHITE) } else { (NOT_SET, TEXT_DIM) };
        let value_box = Rect::new(LEFT, y, VALUE_WIDTH, FIELD_HEIGHT);
        ui.paragraph(value_box, value, &BODY.color(colour), HAlign::Left, VAlign::Center, 1);
        let type_box = Rect::new(TYPE_X, y, BUTTON_WIDTH, FIELD_HEIGHT);
        let clear_box = Rect::new(CLEAR_X, y, BUTTON_WIDTH, FIELD_HEIGHT);
        y += FIELD_HEIGHT + 40.0;
        // What was downloaded: its label, description, and the button 18 under.
        y += controls::label(ui, LEFT, y, ART_LABEL) + 4.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, ART_ABOUT) + 18.0;
        let clear_art_box = Rect::new(LEFT, y, controls::pane_button_width(ui, CLEAR_ART), FIELD_HEIGHT);
        // The focused button last, its ring over its neighbours.
        let buttons = [
            (Focus::Type, type_box, "Type", true),
            (Focus::Clear, clear_box, "Clear", around.options.art_key),
            (Focus::ClearArt, clear_art_box, CLEAR_ART, true),
        ];
        for (focus, rect, label, enabled) in buttons.iter().filter(|b| !has(b.0)) {
            controls::pane_button(ui, *rect, label, *enabled, has(*focus));
        }
        for (_, rect, label, enabled) in buttons.iter().filter(|b| has(b.0)) {
            controls::pane_button(ui, *rect, label, *enabled, true);
        }
        // The question over the dimmed page; or on its way out, the dim gone at once.
        if let Some(prompt) = &self.prompt {
            ui.layer();
            ui.fill(ui.screen(), DIM, 0.0);
            let (fly, shown) = menu_panel::flight(ui, ("art prompt", self.flying), false);
            prompt.draw_flown(ui, fly, shown);
        } else if let Some((prompt, flying)) = &self.leaving {
            ui.layer();
            let (fly, shown) = menu_panel::flight(ui, ("art prompt", *flying), true);
            if shown <= 0.0 || fly >= FLY {
                self.leaving = None;
            } else {
                prompt.draw_flown(ui, fly, shown);
            }
        }
    }
}
