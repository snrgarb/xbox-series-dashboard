//! The pages over Home, and the stack they are in (docs/ui.md, "Screens and what is over what").
//!
//! A page is a screen that takes Home's place: My games & apps, Search, Settings. Each is a module of its own that
//! keeps its own state and implements [`Page`]; this file knows nothing of what is in one. The top page takes the
//! pad. One under it keeps its state, so B on the top one finds it as it was. Opening and closing are instant: a
//! page is there in the next frame and gone in the one after B, with no transition (docs/spec/my-games.md, 3.2).
//!
//! A page that does not fill the screen is drawn over the one under it. One that is a **dialog** (the Tile menu)
//! has sounds of its own, holds what was pressed while it was up back from the page under it until it is let go,
//! and may still be drawn for a moment after it has closed, on its way out.
//!
//! **Adding a page:** a module with a struct that implements [`Page`], a variant of [`Open`] that carries how it is
//! opened, and that variant's line in [`Open::page`]. Nothing else here changes, and nothing in `Dashboard`.
use model::{Library, Sound};
use ui::{Button, Press, Repeat, Ui};

use crate::hint_bar::{self, Hint};
use crate::keyboard::{self, Typed};
use crate::{my_games, search, settings, tile_menu, Effect};

/// What a page is told of the world each time it is pressed or drawn. It keeps none of it.
#[derive(Clone, Copy)]
pub struct Around<'a> {
    pub library: &'a Library,
    /// The game being launched or running, by its place in the library.
    pub playing: Option<usize>,
    /// Settings' options as they are now.
    pub options: settings::Options,
}

/// A page to open, and how: each page's own `Entry` says where on it to open and where focus lands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Open {
    MyGames(my_games::Entry),
    Search,
    Settings(settings::Entry),
    /// Text entry over the page that asks for it (`keyboard::text_entry`), which hears of it through
    /// [`Page::entry`].
    TextEntry(keyboard::Entry),
    /// The stand-in page that opens text entry until Full library and Settings do (`keyboard::stand_in`).
    KeyboardStandIn,
    /// A game's Tile menu, over whatever asked for it (`tile_menu::open`).
    TileMenu(tile_menu::Entry),
}

impl Open {
    /// The page, made anew: a page remembers nothing from the last time it was up. `under` is the page it opens
    /// over, if it is opened over one. It is made from the world as it is now, and one that has nothing to open on
    /// (the Tile menu of a game that has gone) is not made.
    fn page(self, under: Option<&dyn Page>, around: Around) -> Option<Box<dyn Page>> {
        Some(match self {
            Open::MyGames(entry) => Box::new(my_games::MyGames::open(entry)),
            Open::Search => Box::new(search::Search::open()),
            Open::Settings(entry) => Box::new(settings::Settings::open(entry)),
            Open::TextEntry(entry) => {
                let text = under.map(|page| page.entry_text()).unwrap_or_default();
                Box::new(keyboard::TextEntry::open(entry, text))
            }
            Open::KeyboardStandIn => Box::new(keyboard::stand_in::StandIn::open()),
            Open::TileMenu(entry) => Box::new(tile_menu::TileMenu::open(entry, around)?),
        })
    }
}

/// What a press on a page asks the Dashboard to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageAction {
    /// B with nothing of the page's own left to step back from: the page goes, and what opened it has the pad
    /// again with focus where it was.
    Close,
    /// Another page over this one, which stays under it as it is: B there comes back here.
    Open(Open),
    /// Something for the shell: a launch, say.
    Effect(Effect),
    /// The page goes, and then this is asked of the shell: a row of a menu that closes it and acts.
    CloseWith(Effect),
    /// The page goes and this one opens in its place, over what was under it: B there comes back to that.
    Replace(Open),
}

/// What kind of dialog a page is while it is one, which is what its sounds are told by (docs/spec/tile-menu.md,
/// section 8): opening is `PromptOpen`, a move `GuideMove`, B `PromptClose`, and A what [`Dialog::chosen`] says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    /// Rows of things to do.
    Menu,
    /// A question and its answers.
    Prompt,
}

impl Dialog {
    /// The sound of A on a row: a choice in a menu goes on with something, an answer to a prompt closes it.
    pub fn chosen(self) -> Sound {
        match self {
            Dialog::Menu => Sound::GuideSelect,
            Dialog::Prompt => Sound::PromptClose,
        }
    }
}

/// A page over Home. It owns its state and its focus, as every screen does (docs/ui.md): `press` moves them, `draw`
/// declares the frame from them.
pub trait Page {
    /// The page's name in the focus log (`--log-focus`), which tests on the VM judge by: one word, never changed.
    fn name(&self) -> &'static str;

    /// Where focus is on the page, for the focus log, and for the move sound: a press that changes what this says
    /// moved focus. Say enough to tell every place focus can be from every other.
    fn focus(&self) -> String;

    /// A press of the pad while the page is on top. A held direction comes with `press.repeat`; ignore a repeat for
    /// anything but movement. B that the page has no use of its own for answers [`PageAction::Close`].
    fn press(&mut self, press: Press, around: Around) -> Option<PageAction>;

    /// Declares the frame. A page that covers the screen fills it first: nothing of Home is drawn under it.
    fn draw(&mut self, ui: &mut Ui, around: Around);

    /// The hint bar's hints with focus where it is. The Dashboard draws the bar.
    fn hints(&self, _around: Around) -> &'static [Hint] {
        hint_bar::NONE
    }

    /// Whether the page fills the screen, as My games & apps, Search and Settings do. One that does not (something
    /// drawn over the page under it) has that page, or Home, drawn first.
    fn covers(&self) -> bool {
        true
    }

    /// What the focus log says of the page beyond where its focus is, after it: what a press can change without
    /// moving focus, such as the text typed. A change here alone is not a move, and makes no sound.
    fn state(&self) -> String {
        String::new()
    }

    /// A held direction's pace while the page has the pad.
    fn repeat(&self) -> Repeat {
        Repeat::default()
    }

    /// What a physical keyboard typed that is text's (letters, Space, Backspace), while the page is on top. True
    /// if the page takes text now; false, and Backspace is B as it is everywhere else and the rest is nothing.
    fn typed(&mut self, _typed: Typed, _around: Around) -> bool {
        false
    }

    /// A button of the pad that `press` was just given is down (`down`), or was let go: for what is drawn only
    /// while a button is held, as a key of the Console keyboard is. A keyboard's key and a test's `press` tell
    /// of neither.
    fn pad(&mut self, _button: Button, _down: bool) {}

    /// The text that text entry starts from when this page opens it (`Open::TextEntry`), with focus where it is.
    fn entry_text(&self) -> String {
        String::new()
    }

    /// What text entry over this page has to say: the text as it changes, kept, or dropped
    /// (`keyboard::text_entry`). Told in the press that did it, before text entry goes.
    fn entry(&mut self, _event: keyboard::Event, _around: Around) {}

    /// Text entry's own: what it has for the page under it since it was last asked.
    fn says(&mut self) -> Option<keyboard::Event> {
        None
    }

    /// Whether the page is a dialog over what is under it, and of which kind with focus where it is. A dialog's
    /// sounds are a dialog's, its held direction repeats at a dialog's pace, a button pressed while it is up does
    /// nothing under it until it is let go, and it closes when the Guide opens.
    fn dialog(&self) -> Option<Dialog> {
        None
    }

    /// Draws what still shows of the page once it has closed: a dialog on its way out. False when nothing does,
    /// and it is then asked no more. The page under it has the pad already.
    fn draw_leaving(&mut self, _ui: &mut Ui, _around: Around) -> bool {
        false
    }
}

/// The pages that are up, the last on top. Home is under them all and is not one of them.
#[derive(Default)]
pub struct Stack {
    pages: Vec<Box<dyn Page>>,
    /// A dialog that has closed and is still on its way out. It takes no input.
    leaving: Option<Box<dyn Page>>,
}

impl Stack {
    /// The page that has the pad.
    pub fn top(&self) -> Option<&dyn Page> {
        self.pages.last().map(|page| &**page)
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// How many pages are up.
    pub fn depth(&self) -> usize {
        self.pages.len()
    }

    /// Opens a page from outside the stack (Home, the Guide): it takes the place of whatever pages are up, so My
    /// games & apps, Search and Settings are never up together and B on it goes to Home. False, with nothing
    /// changed, when the page has nothing to open on.
    pub fn open(&mut self, open: Open, around: Around) -> bool {
        let Some(page) = open.page(None, around) else { return false };
        self.pages.clear();
        self.push(page);
        true
    }

    /// Opens a page the top page asked for, over it. False when the page has nothing to open on.
    pub fn open_over(&mut self, open: Open, around: Around) -> bool {
        let Some(page) = open.page(self.top(), around) else { return false };
        self.push(page);
        true
    }

    fn push(&mut self, page: Box<dyn Page>) {
        // A dialog opened while another is on its way out takes over from it.
        if page.dialog().is_some() {
            self.leaving = None;
        }
        self.pages.push(page);
    }

    /// The top page goes. A dialog is kept for as long as something of it still shows.
    pub fn close(&mut self) {
        if let Some(page) = self.pages.pop() {
            if page.dialog().is_some() {
                self.leaving = Some(page);
            }
        }
    }

    /// The kind of dialog that has the pad, if the top page is one.
    pub fn dialog(&self) -> Option<Dialog> {
        self.top().and_then(|page| page.dialog())
    }

    /// A dialog on top goes, with no answer: something else has taken the screen. True if there was one.
    pub fn close_dialog(&mut self) -> bool {
        let any = self.dialog().is_some();
        while self.dialog().is_some() {
            self.close();
        }
        any
    }

    /// Every page goes. True if there was one.
    pub fn clear(&mut self) -> bool {
        let any = !self.pages.is_empty();
        self.pages.clear();
        any
    }

    /// Gives the press to the top page.
    pub fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        let action = self.pages.last_mut()?.press(press, around);
        self.relay(around);
        action
    }

    /// Gives the top page what a physical keyboard typed. False if it takes no text, or no page is up.
    pub fn typed(&mut self, typed: Typed, around: Around) -> bool {
        let taken = self.pages.last_mut().is_some_and(|page| page.typed(typed, around));
        self.relay(around);
        taken
    }

    /// Tells the top page a button of the pad it was given is down, or was let go.
    pub fn pad(&mut self, button: Button, down: bool) {
        if let Some(page) = self.pages.last_mut() {
            page.pad(button, down);
        }
    }

    /// A held direction's pace on the top page, if one is up.
    pub fn repeat(&self) -> Option<Repeat> {
        self.top().map(|page| page.repeat())
    }

    /// What the top page has for the page under it (text entry's text) goes to that page.
    fn relay(&mut self, around: Around) {
        if let Some((top, under)) = self.pages.split_last_mut() {
            if let (Some(event), Some(under)) = (top.says(), under.last_mut()) {
                under.entry(event, around);
            }
        }
    }

    /// Whether a page fills the screen: Home is not drawn, its top bar neither.
    pub fn hides_home(&self) -> bool {
        self.pages.iter().any(|page| page.covers())
    }

    /// The top page's hints, or none with no page up.
    pub fn hints(&self, around: Around) -> &'static [Hint] {
        self.top().map_or(hint_bar::NONE, |page| page.hints(around))
    }

    /// Draws what shows of the pages: the topmost that fills the screen and those over it. Each but the first thing
    /// in the frame is on a layer of its own, so it covers the text of what is under it.
    pub fn draw(&mut self, ui: &mut Ui, around: Around) {
        let first = self.pages.iter().rposition(|page| page.covers()).unwrap_or(0);
        let over_home = !self.hides_home();
        for (at, page) in self.pages.iter_mut().enumerate().skip(first) {
            if at > first || over_home {
                ui.layer();
            }
            page.draw(ui, around);
        }
        if let Some(page) = &mut self.leaving {
            ui.layer();
            if !page.draw_leaving(ui, around) {
                self.leaving = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ui::Button;

    use super::*;

    /// A page that counts the presses it was given. A asks for Search over it; B closes it.
    struct Counter {
        presses: u32,
        covers: bool,
    }

    impl Page for Counter {
        fn name(&self) -> &'static str {
            "Counter"
        }

        fn focus(&self) -> String {
            self.presses.to_string()
        }

        fn press(&mut self, press: Press, _around: Around) -> Option<PageAction> {
            self.presses += 1;
            match press.button {
                Button::A => Some(PageAction::Open(Open::Search)),
                Button::B => Some(PageAction::Close),
                _ => None,
            }
        }

        fn draw(&mut self, _ui: &mut Ui, _around: Around) {}

        fn covers(&self) -> bool {
            self.covers
        }
    }

    fn press(stack: &mut Stack, button: Button) -> Option<PageAction> {
        let library = Library::default();
        stack.press(Press { button, repeat: false }, around(&library))
    }

    fn around(library: &Library) -> Around<'_> {
        Around { library, playing: None, options: settings::Options::default() }
    }

    fn focus(stack: &Stack) -> Option<(&'static str, String)> {
        stack.top().map(|page| (page.name(), page.focus()))
    }

    #[test]
    fn the_top_page_takes_the_pad_and_the_one_under_it_keeps_its_state() {
        let mut stack = Stack::default();
        assert_eq!(press(&mut stack, Button::Down), None, "with no page up there is no one to press");
        stack.pages.push(Box::new(Counter { presses: 0, covers: true }));
        press(&mut stack, Button::Down);
        press(&mut stack, Button::Down);
        // It asks for Search over it, as the Dashboard would open it.
        assert_eq!(press(&mut stack, Button::A), Some(PageAction::Open(Open::Search)));
        stack.open_over(Open::Search, around(&Library::default()));
        assert_eq!((stack.depth(), focus(&stack)), (2, Some(("Search", "Open".into()))));
        // Search has the pad: the page under it hears nothing, and is as it was when Search has gone.
        press(&mut stack, Button::Down);
        assert_eq!(press(&mut stack, Button::B), Some(PageAction::Close));
        stack.close();
        assert_eq!((stack.depth(), focus(&stack)), (1, Some(("Counter", "3".into()))));
    }

    #[test]
    fn a_page_opened_from_outside_takes_the_place_of_every_page_that_is_up() {
        let mut stack = Stack::default();
        let library = Library::default();
        stack.open(Open::MyGames(my_games::Entry::Games), around(&library));
        stack.open_over(Open::Search, around(&Library::default()));
        stack.open(Open::Settings(settings::Entry::Rail), around(&library));
        assert_eq!((stack.depth(), focus(&stack)), (1, Some(("Settings", "Rail: Theme settings".into()))));
        assert!(stack.clear() && stack.is_empty() && !stack.clear());
    }

    #[test]
    fn home_is_hidden_by_a_page_that_fills_the_screen_and_not_by_one_that_does_not() {
        let mut stack = Stack::default();
        assert!(!stack.hides_home());
        stack.pages.push(Box::new(Counter { presses: 0, covers: false }));
        assert!(!stack.hides_home());
        stack.open_over(Open::Search, around(&Library::default()));
        assert!(stack.hides_home());
    }
}
