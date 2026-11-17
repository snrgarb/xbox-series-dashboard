//! A licence agreement (#230; docs/spec/manage-queue.md, section 10): the Dashboard's own prompt for an agreement a
//! store asks the user to accept before it installs a game, in the console's manner: a wide dark panel in the
//! middle of the dimmed screen, the game's name over "Licence agreement", the agreement's own text in a box that
//! scrolls with the pad, the position shown under it, and **Accept** and **Decline** side by side at its foot, focus
//! starting on Decline. Where the text could not be had it says so, and offers **Open in Steam** and Decline only.
//!
//! It is a legal acceptance: only the user's own A on Accept answers [`Reply::Accept`]. B, Decline, and the page
//! going with no answer (whoever drives the Dashboard looks with [`crate::Dashboard::agreements_up`]) are all a
//! Decline, and nothing is remembered: the next install asks again. It is opened over whatever is up by whoever
//! drives the Dashboard, as the store's installer asks ([`crate::Dashboard::ask_agreement`]), and answers with
//! [`Effect::Agreement`]. It is not a dialog of the page stack's: the Guide opening leaves it up, unanswered.
use model::{Agreement, Reply};
use ui::{Button, Color, Font, HAlign, Press, Rect, TextStyle, Ui, VAlign};

use crate::hint_bar::{self, Hint};
use crate::menu_panel::DIM;
use crate::pages::{Around, Open, Page, PageAction};
use crate::theme::{self, BODY, TEXT_DIM};
use crate::Effect;

/// What the prompt is opened with: the question's number (whoever asked waits on it), the game's name, and the
/// agreement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: u64,
    pub game: String,
    pub agreement: Agreement,
}

/// The prompt, over whatever is up.
pub fn open(entry: Entry) -> Open {
    Open::Agreement(entry)
}

/// The panel: 1200 by 880 in the middle of the screen, the menu panel's grey and corners.
const PANEL: (f32, f32) = (1200.0, 880.0);
const FILL: Color = Color::rgb(0x2B2B2B);
const RADIUS: f32 = 6.0;
const SHADOW: (Color, f32) = (Color::argb(0x80000000), 24.0);
/// Its insides: 48 in from the sides, the title 40 down.
const SIDE: f32 = 48.0;
const TITLE_DOWN: f32 = 40.0;
const TITLE: TextStyle = TextStyle::new(Font::Display, 33.0, 400);
const TITLE_LINE: f32 = 39.6;
/// "Licence agreement" under the title.
const SUBTITLE_DOWN: f32 = 8.0;
/// The text's box: 24 under the subtitle, down to 32 over the buttons; the bar 24 right of it, 4 wide.
const TEXT_DOWN: f32 = 150.0;
const TEXT_UP: f32 = 152.0;
const BAR_GAP: f32 = 24.0;
const BAR: f32 = 4.0;
const TRACK: Color = Color::rgb(0x4A4A4A);
/// A line of the text, and the room an empty line makes between paragraphs.
const LINE: f32 = 26.0 * 1.33;
const PARAGRAPH_GAP: f32 = LINE * 0.5;
/// Up and Down move it three lines; a bumper or a trigger a box's height, less a line.
const STEP_LINES: f32 = 3.0;
/// The buttons: 280 by 56, 24 apart, 48 over the panel's foot.
const BUTTON: (f32, f32) = (280.0, 56.0);
const BUTTON_GAP: f32 = 24.0;
const BUTTON_UP: f32 = 48.0 + 56.0;
const BUTTON_FILL: Color = Color::rgb(0x3A3A3A);
const BUTTON_FOCUSED: Color = Color::rgb(0x505050);

/// The answers it offers, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Accept,
    OpenInSteam,
    Decline,
}

impl Choice {
    fn label(self) -> &'static str {
        match self {
            Choice::Accept => "Accept",
            Choice::OpenInSteam => "Open in Steam",
            Choice::Decline => "Decline",
        }
    }

    fn reply(self) -> Reply {
        match self {
            Choice::Accept => Reply::Accept,
            Choice::OpenInSteam => Reply::OpenInStore,
            Choice::Decline => Reply::Decline,
        }
    }

    /// As the focus log names it.
    fn word(self) -> &'static str {
        match self {
            Choice::Accept => "Accept",
            Choice::OpenInSteam => "OpenInSteam",
            Choice::Decline => "Decline",
        }
    }
}

pub struct AgreementPrompt {
    entry: Entry,
    choices: Vec<Choice>,
    focus: usize,
    /// How far the text is scrolled, in design units.
    scroll: f32,
    /// The text's blocks (a line of it, wrapped; or the room of an empty one), and each one's height as last drawn.
    blocks: Vec<String>,
    heights: Vec<f32>,
    /// The box's height as last drawn.
    view: f32,
    /// The pad's scrolling before the text was first measured, in lines and in boxes-full: done at its first draw,
    /// so that a press as the prompt comes up is not lost.
    early: (f32, f32),
}

impl AgreementPrompt {
    pub fn open(entry: Entry) -> Self {
        let choices = match &entry.agreement.text {
            Ok(_) => vec![Choice::Accept, Choice::Decline],
            Err(_) => vec![Choice::OpenInSteam, Choice::Decline],
        };
        let text = match &entry.agreement.text {
            Ok(text) => text.clone(),
            Err(why) => format!(
                "The licence agreement couldn't be loaded: {why}.\n\nIt can't be accepted here without being read. \
                 Open in Steam leaves this install to Steam's own window, where the agreement is shown; Decline \
                 installs nothing."
            ),
        };
        let blocks: Vec<String> = text.lines().map(|line| line.trim().to_string()).collect();
        let focus = choices.len() - 1;
        Self { entry, choices, focus, scroll: 0.0, heights: Vec::new(), blocks, view: 0.0, early: (0.0, 0.0) }
    }

    /// The text's whole height, as last measured.
    fn total(&self) -> f32 {
        self.heights.iter().sum()
    }

    fn most(&self) -> f32 {
        (self.total() - self.view).max(0.0)
    }

    /// Whether the text has been measured: it is, at its first draw. Before that how many pages it has is not
    /// known, and is not said.
    fn measured(&self) -> bool {
        self.view > 0.0 && self.heights.len() == self.blocks.len()
    }

    /// Which box-full of the text shows, and how many there are: "page 2 of 7". A text that fits is one page,
    /// with no bar, and the pad's scrolling does nothing.
    fn pages(&self) -> (usize, usize) {
        if !self.measured() {
            return (1, 1);
        }
        let of = ((self.total() / self.view).ceil() as usize).max(1);
        let at = if self.scroll >= self.most() - 0.5 { of } else { (self.scroll / self.view).floor() as usize + 1 };
        (at.min(of), of)
    }

    /// A box-full less a line: what a bumper or trigger scrolls.
    fn page(&self) -> f32 {
        (self.view - LINE).max(LINE)
    }

    /// Scrolls by `lines` and `pages`, within the text's ends; before the text is measured, kept for its first draw.
    fn scroll_by(&mut self, lines: f32, pages: f32) {
        if !self.measured() {
            self.early.0 += lines;
            self.early.1 += pages;
            return;
        }
        let by = lines * LINE + pages * self.page();
        self.scroll = (self.scroll + by).clamp(0.0, self.most());
    }

    fn rect(ui: &Ui) -> Rect {
        let screen = ui.screen();
        Rect::new(screen.x + (screen.w - PANEL.0) / 2.0, (screen.h - PANEL.1) / 2.0, PANEL.0, PANEL.1)
    }
}

impl Page for AgreementPrompt {
    fn name(&self) -> &'static str {
        "Agreement"
    }

    /// `Decline of Saltwind Rally`.
    fn focus(&self) -> String {
        format!("{} of {}", self.choices[self.focus].word(), self.entry.game)
    }

    /// Which agreement and where the text is: `agreement 1 of 2, page 3 of 7`; `agreement 1 of 1, not read` where
    /// it could not be had. Before its first draw, how many pages there are is not known: `agreement 1 of 2, page
    /// 1`, so that the first press after it is drawn says the count.
    fn state(&self) -> String {
        let which = format!("agreement {} of {}", self.entry.agreement.at, self.entry.agreement.of);
        match self.entry.agreement.text {
            Ok(_) if !self.measured() => format!("{which}, page 1"),
            Ok(_) => {
                let (at, of) = self.pages();
                format!("{which}, page {at} of {of}")
            }
            Err(_) => format!("{which}, not read"),
        }
    }

    fn agreement(&self) -> Option<u64> {
        Some(self.entry.id)
    }

    fn press(&mut self, press: Press, _around: Around) -> Option<PageAction> {
        let once = !press.repeat;
        match press.button {
            Button::Up => self.scroll_by(-STEP_LINES, 0.0),
            Button::Down => self.scroll_by(STEP_LINES, 0.0),
            Button::LeftBumper | Button::LeftTrigger => self.scroll_by(0.0, -1.0),
            Button::RightBumper | Button::RightTrigger => self.scroll_by(0.0, 1.0),
            Button::Left if self.focus > 0 => self.focus -= 1,
            Button::Right if self.focus + 1 < self.choices.len() => self.focus += 1,
            Button::A if once => {
                let reply = self.choices[self.focus].reply();
                return Some(PageAction::CloseWith(Effect::Agreement { id: self.entry.id, reply }));
            }
            Button::B if once => {
                return Some(PageAction::CloseWith(Effect::Agreement { id: self.entry.id, reply: Reply::Decline }));
            }
            _ => {}
        }
        None
    }

    fn hints(&self, _around: Around) -> &'static [Hint] {
        hint_bar::NONE
    }

    fn covers(&self) -> bool {
        false
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        ui.fill(ui.screen(), DIM, 0.0);
        let panel = Self::rect(ui);
        ui.shadow(panel, SHADOW.0, RADIUS, SHADOW.1);
        ui.fill(panel, FILL, RADIUS);
        let inner = panel.w - 2.0 * SIDE;
        let title = Rect::new(panel.x + SIDE, panel.y + TITLE_DOWN, inner, TITLE_LINE + 4.0);
        ui.paragraph(title, &self.entry.game, &TITLE, HAlign::Left, VAlign::Top, 1);
        let agreement = &self.entry.agreement;
        let subtitle = match agreement.of {
            1 => "Licence agreement".to_string(),
            of => format!("Licence agreement {} of {of}", agreement.at),
        };
        ui.label(panel.x + SIDE, title.y + TITLE_LINE + SUBTITLE_DOWN, &subtitle, &BODY.color(TEXT_DIM));

        // The text, a block at a time, only those that show.
        let text = Rect::new(panel.x + SIDE, panel.y + TEXT_DOWN, inner - BAR_GAP - BAR, panel.h - TEXT_DOWN - TEXT_UP);
        self.view = text.h;
        if self.heights.len() != self.blocks.len() {
            let width = text.w;
            self.heights = self
                .blocks
                .iter()
                .map(|block| {
                    if block.is_empty() {
                        PARAGRAPH_GAP
                    } else {
                        ui.measure_wrapped(&BODY, block, width, 0).lines.max(1) as f32 * LINE
                    }
                })
                .collect();
        }
        // What was pressed before the text was measured, done now it is.
        let (lines, pages) = std::mem::take(&mut self.early);
        self.scroll = (self.scroll + lines * LINE + pages * self.page()).clamp(0.0, self.most());
        let style = if agreement.text.is_ok() { BODY } else { BODY.color(TEXT_DIM) };
        ui.with_clip(text, |ui| {
            let mut y = text.y - self.scroll;
            for (block, height) in self.blocks.iter().zip(&self.heights) {
                if y + height >= text.y && y <= text.bottom() && !block.is_empty() {
                    let at = Rect::new(text.x, y, text.w, height + 4.0);
                    ui.paragraph(at, block, &style, HAlign::Left, VAlign::Top, 0);
                }
                y += height;
            }
        });
        // The bar: the track the box's height, the thumb as much of it as shows, where it shows.
        let total = self.total();
        let track = Rect::new(text.right() + BAR_GAP, text.y, BAR, text.h);
        if total > text.h {
            ui.fill(track, TRACK, BAR / 2.0);
            let thumb = (text.h / total * text.h).max(24.0);
            let at = if self.most() > 0.0 { self.scroll / self.most() * (text.h - thumb) } else { 0.0 };
            ui.fill(Rect::new(track.x, track.y + at, BAR, thumb), Color::WHITE, BAR / 2.0);
        }

        // The buttons at the foot, and where the text is beside them.
        let top = panel.bottom() - BUTTON_UP;
        let mut ring = None;
        for (i, choice) in self.choices.iter().enumerate() {
            let rect = Rect::new(panel.x + SIDE + i as f32 * (BUTTON.0 + BUTTON_GAP), top, BUTTON.0, BUTTON.1);
            let focused = i == self.focus;
            ui.fill(rect, if focused { BUTTON_FOCUSED } else { BUTTON_FILL }, 2.0);
            let width = ui.measure(&BODY, choice.label()).width;
            ui.label_middle(rect.center().0 - width / 2.0, rect.center().1, choice.label(), &BODY);
            if focused {
                ring = Some(rect);
            }
        }
        if agreement.text.is_ok() {
            let (at, of) = self.pages();
            let position = format!("Page {at} of {of}");
            ui.label_right(panel.right() - SIDE, top + BUTTON.1 / 2.0, &position, &BODY.color(TEXT_DIM));
        }
        if let Some(ring) = ring {
            theme::focus_ring_cornered(ui, ring, 9.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: Result<&str, &str>) -> Entry {
        let text = text.map(str::to_string).map_err(str::to_string);
        Entry { id: 7, game: "Saltwind Rally".into(), agreement: Agreement { at: 1, of: 2, text } }
    }

    fn around(library: &model::Library) -> Around<'_> {
        Around {
            library,
            playing: None,
            options: crate::settings::Options::default(),
            now: std::time::Instant::now(),
            now_playing: crate::now_playing::none(),
            discord: crate::no_discord(),
        }
    }

    fn press(page: &mut AgreementPrompt, button: Button) -> Option<PageAction> {
        let library = model::Library::default();
        page.press(Press { button, repeat: false }, around(&library))
    }

    #[test]
    fn focus_starts_on_decline_and_only_a_on_accept_accepts() {
        let mut page = AgreementPrompt::open(entry(Ok("Terms.")));
        assert_eq!(page.focus(), "Decline of Saltwind Rally");
        assert_eq!(page.state(), "agreement 1 of 2, page 1", "not drawn: its count not known");
        // Drawn, it fits: one page, and Up and Down move nothing.
        page.view = 500.0;
        page.heights = vec![LINE];
        assert_eq!(page.state(), "agreement 1 of 2, page 1 of 1");
        for button in [Button::Down, Button::RightBumper, Button::RightTrigger, Button::Up] {
            assert_eq!(press(&mut page, button), None);
            assert_eq!((page.scroll, page.state().as_str()), (0.0, "agreement 1 of 2, page 1 of 1"));
        }
        // B, and A on Decline, decline.
        let decline = Some(PageAction::CloseWith(Effect::Agreement { id: 7, reply: Reply::Decline }));
        assert_eq!(press(&mut page, Button::B), decline);
        assert_eq!(press(&mut page, Button::A), decline);
        assert_eq!(press(&mut page, Button::Right), None);
        assert_eq!(page.focus(), "Decline of Saltwind Rally", "Decline is the last");
        assert_eq!(press(&mut page, Button::Left), None);
        assert_eq!(page.focus(), "Accept of Saltwind Rally");
        // A held is not an answer.
        let library = model::Library::default();
        assert_eq!(page.press(Press { button: Button::A, repeat: true }, around(&library)), None);
        assert_eq!(
            press(&mut page, Button::A),
            Some(PageAction::CloseWith(Effect::Agreement { id: 7, reply: Reply::Accept }))
        );
        assert_eq!(page.agreement(), Some(7));
    }

    /// A bumper pressed as the prompt comes up, before its text is first measured, scrolls once it is.
    #[test]
    fn scrolling_before_the_first_draw_is_done_at_it() {
        let mut page = AgreementPrompt::open(entry(Ok("a\n\nb")));
        press(&mut page, Button::RightBumper);
        press(&mut page, Button::Down);
        assert_eq!((page.scroll, page.state().as_str()), (0.0, "agreement 1 of 2, page 1"));
        // Measured as a draw measures it, then what was pressed done.
        page.view = 500.0;
        page.heights = vec![2000.0, PARAGRAPH_GAP, 3000.0 - PARAGRAPH_GAP];
        let (lines, pages) = std::mem::take(&mut page.early);
        page.scroll_by(lines, pages);
        assert!((page.scroll - (500.0 - LINE + STEP_LINES * LINE)).abs() < 0.01, "{}", page.scroll);
        assert_eq!(page.state(), "agreement 1 of 2, page 2 of 10");
    }

    #[test]
    fn with_no_text_there_is_no_accept() {
        let mut page = AgreementPrompt::open(entry(Err("Steam's store could not be reached")));
        assert_eq!(page.focus(), "Decline of Saltwind Rally");
        assert_eq!(page.state(), "agreement 1 of 2, not read");
        press(&mut page, Button::Left);
        assert_eq!(page.focus(), "OpenInSteam of Saltwind Rally");
        assert_eq!(press(&mut page, Button::Left), None);
        assert_eq!(page.focus(), "OpenInSteam of Saltwind Rally");
        assert_eq!(
            press(&mut page, Button::A),
            Some(PageAction::CloseWith(Effect::Agreement { id: 7, reply: Reply::OpenInStore }))
        );
        assert!(!page.choices.contains(&Choice::Accept));
    }

    #[test]
    fn the_text_scrolls_within_its_ends_and_says_where_it_is() {
        let mut page = AgreementPrompt::open(entry(Ok("a\n\nb")));
        // As if drawn: ten boxes of text, a height a block.
        page.view = 500.0;
        page.heights = vec![2000.0, PARAGRAPH_GAP, 3000.0 - PARAGRAPH_GAP];
        assert_eq!(page.blocks.len(), 3);
        assert_eq!(page.pages(), (1, 10));
        press(&mut page, Button::Up);
        assert_eq!(page.scroll, 0.0);
        press(&mut page, Button::Down);
        assert!((page.scroll - 3.0 * LINE).abs() < 0.01);
        // A box's height less a line each: three lines and two boxes on is into the third.
        press(&mut page, Button::RightBumper);
        press(&mut page, Button::RightTrigger);
        assert_eq!(page.pages().0, 3);
        for _ in 0..40 {
            press(&mut page, Button::RightBumper);
        }
        assert_eq!((page.scroll, page.pages()), (4500.0, (10, 10)));
        press(&mut page, Button::LeftTrigger);
        assert!(page.scroll < 4500.0);
        assert_eq!(page.state(), "agreement 1 of 2, page 9 of 10");
    }
}
