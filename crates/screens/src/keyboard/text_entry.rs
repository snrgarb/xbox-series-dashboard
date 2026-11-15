//! Text entry: the Console keyboard under a caption and a field, over the page that asked for it, which dims
//! (docs/spec/search.md, section 10; the Bundle's `TextEntryControl.xaml` and `ExtensionCore.TextEntry.cs`).
//!
//! **Opening it.** A page answers a press with `PageAction::Open(Open::TextEntry(entry))`, the [`Entry`] saying
//! the caption, whether the text is a secret, and how long it may be. The text it starts from is asked of the page:
//! `Page::entry_text`. Text entry is then the top page and has the pad, every button of it, until it goes.
//!
//! **Hearing from it.** The page under it is told through `Page::entry`, at once, in the same press:
//! [`Event::Typed`] with the whole text each time it changes, [`Event::Kept`] with the text as Menu or the Enter
//! key keeps it, [`Event::Dropped`] with the text it started from as B drops it. After `Kept` and `Dropped` text
//! entry has gone and the page has the pad again, its focus where it was unless `entry` moved it.
use ui::{Button, Color, Font, Gradient, HAlign, Press, Rect, Repeat, TextStyle, Ui, VAlign};

use super::board::{Board, Outcome, Typed};
use crate::pages::{Around, Page, PageAction};

/// How text entry is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The line over the field: "Filter by name".
    pub caption: &'static str,
    /// A secret: the field shows a dot for each character and never the text.
    pub masked: bool,
    /// The most characters that may be typed, if there is a most.
    pub limit: Option<usize>,
}

impl Entry {
    pub const fn new(caption: &'static str) -> Self {
        Self { caption, masked: false, limit: None }
    }

    pub const fn masked(self) -> Self {
        Self { masked: true, ..self }
    }

    pub const fn limit(self, characters: usize) -> Self {
        Self { limit: Some(characters), ..self }
    }
}

/// What text entry tells the page under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The text changed: all of it, as it is now. Text entry stays up.
    Typed(String),
    /// Menu, or A on the Enter key: the text is kept, and text entry has gone.
    Kept(String),
    /// B (Escape on a keyboard): the text is dropped, and text entry has gone. This is the text it started from,
    /// for a page that followed the typing to put back.
    Dropped(String),
}

/// The dim: 35% black down to four tenths of the screen, then darkening to 78% at 62 hundredths and on to the
/// foot, so the page is seen above and the caption, field and keys read as on a dark page.
const DIM_TOP: Color = Color::argb(0x59000000);
const DIM_FOOT: Color = Color::argb(0xC8000000);
const DIM_STOPS: (f32, f32) = (0.4, 0.62);
/// The field: Search's pill, the keyboard's width, 54 tall, 36 over the keys; its text 28 in from each end.
const FIELD: Color = Color::rgb(0x2E3136);
const FIELD_HEIGHT: f32 = 54.0;
const FIELD_OVER_KEYS: f32 = 36.0;
const FIELD_INSET: f32 = 28.0;
/// The caption: Bahnschrift 30 SemiLight, 14 over the field, level with the field's text.
const CAPTION: TextStyle = TextStyle::new(Font::Display, 30.0, 350);
const CAPTION_OVER_FIELD: f32 = 14.0;

pub struct TextEntry {
    entry: Entry,
    board: Board,
    /// The text it opened over.
    start: String,
    /// What the page under it has not been told yet.
    says: Option<Event>,
}

impl TextEntry {
    /// Over `text`, the caret at its end; Shift on only if it is empty; focus on Q.
    pub fn open(entry: Entry, text: String) -> Self {
        Self { entry, board: Board::over(&text).limited(entry.limit), start: text, says: None }
    }

    fn answer(&mut self, outcome: Outcome) -> Option<PageAction> {
        match outcome {
            Outcome::Edited => self.says = Some(Event::Typed(self.board.text())),
            Outcome::Enter => {
                self.says = Some(Event::Kept(self.board.text()));
                return Some(PageAction::Close);
            }
            // Up from the top row goes nowhere: the field and the caption take no focus.
            Outcome::Taken | Outcome::LeftTop | Outcome::Ignored => {}
        }
        None
    }
}

impl Page for TextEntry {
    fn name(&self) -> &'static str {
        "TextEntry"
    }

    /// The focused key, by a name that is the same on every layer: `Q`, `Space`, `Enter`.
    fn focus(&self) -> String {
        self.board.focus()
    }

    /// The text with the caret in it, Shift and the layer: `"Ha|" lower letters`. A secret is stars.
    fn state(&self) -> String {
        self.board.state(self.entry.masked)
    }

    /// Every button is text entry's: what the keyboard has no use for is swallowed, and nothing reaches the page.
    fn press(&mut self, press: Press, _around: Around) -> Option<PageAction> {
        if press.button == Button::B {
            if press.repeat {
                return None;
            }
            self.says = Some(Event::Dropped(self.start.clone()));
            return Some(PageAction::Close);
        }
        let outcome = self.board.press(press);
        self.answer(outcome)
    }

    fn typed(&mut self, typed: Typed, _around: Around) -> bool {
        let outcome = self.board.typed(typed);
        self.answer(outcome);
        true
    }

    fn pad(&mut self, button: Button, down: bool) {
        if down {
            self.board.hold(button);
        } else {
            self.board.release(button);
        }
    }

    fn says(&mut self) -> Option<Event> {
        self.says.take()
    }

    fn repeat(&self) -> Repeat {
        super::REPEAT
    }

    /// The page under it is drawn first, and dimmed.
    fn covers(&self) -> bool {
        false
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        let screen = ui.screen();
        // The gradient's stops fall on whole pixels, so no line shows where its three parts meet.
        let px = ui.px();
        let stop = |share: f32| (screen.h * share * px).round() / px;
        let (from, to) = (stop(DIM_STOPS.0), stop(DIM_STOPS.1));
        ui.fill(Rect::new(screen.x, 0.0, screen.w, from), DIM_TOP, 0.0);
        ui.gradient(Rect::new(screen.x, from, screen.w, to - from), DIM_TOP, DIM_FOOT, Gradient::Vertical, 0.0);
        ui.fill(Rect::new(screen.x, to, screen.w, screen.h - to), DIM_FOOT, 0.0);

        let keys = Board::rect(ui);
        let field = Rect::new(keys.x, keys.y - FIELD_OVER_KEYS - FIELD_HEIGHT, keys.w, FIELD_HEIGHT);
        let text = Rect::new(field.x + FIELD_INSET, field.y, field.w - 2.0 * FIELD_INSET, field.h);
        let line = ui.measure(&CAPTION, self.entry.caption).height;
        let caption = Rect::new(text.x, field.y - CAPTION_OVER_FIELD - line, text.w, line);
        ui.paragraph(caption, self.entry.caption, &CAPTION, HAlign::Left, VAlign::Bottom, 1);
        ui.fill(field, FIELD, FIELD_HEIGHT / 2.0);
        // The caret is always shown: the keyboard is always up. It does not blink.
        self.board.draw_text(ui, text, self.entry.masked, true);
        self.board.draw(ui, true);
    }
}
