//! The pages over Home, and the stack they are in (docs/ui.md, "Screens and what is over what").
//!
//! A page is a screen that takes Home's place: My games & apps, Search, Settings. Each is a module of its own that
//! keeps its own state and implements [`Page`]; this file knows nothing of what is in one. The top page takes the
//! pad. One under it keeps its state, so B on the top one finds it as it was. Opening and closing are instant: a
//! page is there in the next frame and gone in the one after B, with no transition (docs/spec/my-games.md, 3.2).
//!
//! **Adding a page:** a module with a struct that implements [`Page`], a variant of [`Open`] that carries how it is
//! opened, and that variant's line in [`Open::page`]. Nothing else here changes, and nothing in `Dashboard`.
use model::Library;
use ui::{Press, Ui};

use crate::hint_bar::{self, Hint};
use crate::{my_games, search, settings, Effect};

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
}

impl Open {
    /// The page, made anew: a page remembers nothing from the last time it was up.
    fn page(self) -> Box<dyn Page> {
        match self {
            Open::MyGames(entry) => Box::new(my_games::MyGames::open(entry)),
            Open::Search => Box::new(search::Search::open()),
            Open::Settings(entry) => Box::new(settings::Settings::open(entry)),
        }
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
}

/// The pages that are up, the last on top. Home is under them all and is not one of them.
#[derive(Default)]
pub struct Stack {
    pages: Vec<Box<dyn Page>>,
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
    /// games & apps, Search and Settings are never up together and B on it goes to Home.
    pub fn open(&mut self, open: Open) {
        self.pages.clear();
        self.pages.push(open.page());
    }

    /// Opens a page the top page asked for, over it.
    pub fn open_over(&mut self, open: Open) {
        self.pages.push(open.page());
    }

    /// The top page goes.
    pub fn close(&mut self) {
        self.pages.pop();
    }

    /// Every page goes. True if there was one.
    pub fn clear(&mut self) -> bool {
        let any = !self.pages.is_empty();
        self.pages.clear();
        any
    }

    /// Gives the press to the top page.
    pub fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        self.pages.last_mut()?.press(press, around)
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
        let around = Around { library: &library, playing: None, options: settings::Options::default() };
        stack.press(Press { button, repeat: false }, around)
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
        stack.open_over(Open::Search);
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
        stack.open(Open::MyGames(my_games::Entry::Games));
        stack.open_over(Open::Search);
        stack.open(Open::Settings(settings::Entry::Rail));
        assert_eq!((stack.depth(), focus(&stack)), (1, Some(("Settings", "Rail: Theme settings".into()))));
        assert!(stack.clear() && stack.is_empty() && !stack.clear());
    }

    #[test]
    fn home_is_hidden_by_a_page_that_fills_the_screen_and_not_by_one_that_does_not() {
        let mut stack = Stack::default();
        assert!(!stack.hides_home());
        stack.pages.push(Box::new(Counter { presses: 0, covers: false }));
        assert!(!stack.hides_home());
        stack.open_over(Open::Search);
        assert!(stack.hides_home());
    }
}
