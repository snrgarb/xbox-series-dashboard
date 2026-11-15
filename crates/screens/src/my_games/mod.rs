//! My games & apps (docs/spec/my-games.md): the nav down the left, and beside it Games, Apps, Play history, Full
//! library or Manage.
//!
//! This file is the page's shell: which item is showing, whether focus is on the nav or in the item's content, how
//! the pad moves between the two, and the header and the fly-in they share. Each item's content is a [`Pane`] in a
//! file of its own, which keeps its own focus:
//!
//! | Item | Pane |
//! |---|---|
//! | Games | `games` (the grid is `grid`, its tile `tile`: both are Full library's too) |
//! | Apps | [`Note`], until M4 |
//! | Play history | `history` |
//! | Full library | [`full_library`], an empty pane until #53 |
//! | Manage | `manage` |
//!
//! **Hooks for other tickets**, each one function that does nothing yet: [`full_library`] and
//! [`full_library_count`] (#53), `manage::open_manage_queue`
//! and `nav::foot` (M4), [`progress`] (the library updating).
mod games;
mod grid;
mod history;
mod look;
mod manage;
mod nav;
mod tile;

use std::time::Instant;

use model::{Game, Library};
use ui::{Button, Ease, Press, Ui};

use self::games::Games;
use self::history::History;
use self::look::{HEADER, PAGE, TEXT_DIM};
use self::manage::Manage;
use self::nav::{Item, Settle};
use crate::game_hub;
use crate::hint_bar::{self, Hint};
use crate::pages::{Around, Open, Page, PageAction, Tick};

/// How the page is opened: which of its items shows, and so where focus lands (docs/spec/my-games.md, 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// Games, focus on the grid's first game: Home's top bar button, the Guide's My games & apps.
    Games,
    /// Play history, focus on its first row: Home's Play history tile.
    PlayHistory,
    /// Full library: See all on a Group, on the tab of that Group's saved filter (`model::Filter::id`). None is
    /// plain Full library, as the nav opens it.
    FullLibrary { filter: Option<i64> },
    /// Manage's Queue (M4): Home's Downloads tile. Until there is a Manage Queue it is Manage, focus on the Queue
    /// box.
    Queue,
}

/// The content's left edge and its right; the header's top, and the content row's 118 under it.
const CONTENT_LEFT: f32 = 520.0;
const CONTENT_RIGHT: f32 = 1750.0;
const HEADER_TOP: f32 = 72.0;
/// The header's count is 16 under the header's top.
const COUNT_TOP: f32 = 88.0;
/// A pane opened by focus moving along the nav flies in from this far, over this long.
const FLY: f32 = 50.0;
const FLY_SECONDS: f32 = 0.12;

/// A on a game opens its Game hub over this page, so that B there comes back with focus on the game.
fn open_game_hub(game: &Game) -> Option<PageAction> {
    Some(PageAction::Open(Open::GameHub(game_hub::Entry::of(game))))
}

/// Menu on a game (and X on a grid tile) opens its Tile menu, with "Customise games & apps" in place of Home's
/// "Customise Home".
fn open_tile_menu(game: &Game) -> Option<PageAction> {
    Some(crate::tile_menu::open(game, crate::tile_menu::Opener::MyGames))
}

/// **#53's hook.** Full library's content, opened on the tab of this saved filter (`model::Filter::id`), or as the
/// nav opens it: the tabs, the Sort and Filter row and the grid under them. A file of its own that implements
/// [`Pane`] takes this function's one line; nothing else in this folder changes. Until then the pane is empty and
/// takes no focus, and says in the focus log which filter it was handed.
fn full_library(filter: Option<i64>) -> Box<dyn Pane> {
    Box::new(Note {
        text: "",
        log_name: Some(format!("FullLibrary {{ filter: {filter:?} }}")),
        count: full_library_count,
    })
}

/// **#53's hook.** The count at Full library's nav item: the games its tab and name filter show. Until there are
/// tabs, every game that is not hidden.
fn full_library_count(library: &Library) -> Option<usize> {
    Some(library.games.len())
}

/// **The library updating: a hook.** While the library is being read the Bundle repeats the top bar's status text
/// and its Spinner here, right-aligned to x 1815 in a strip y 84..144, 12 between them (docs/spec/my-games.md,
/// 11.3). The page is not told that the library is updating, so nothing is drawn.
fn progress(_ui: &mut Ui, _around: Around) {}

/// How a pane is to draw itself this frame.
#[derive(Debug, Clone, Copy)]
struct Look {
    /// Focus is in the pane, not on the nav.
    focused: bool,
    /// How far the pane has faded in on its way from the nav: everything in it but a grid of games takes this
    /// alpha.
    fade: f32,
}

/// What a press in a pane's content came to.
enum Outcome {
    /// Nothing for the page to do: focus moved inside the pane, or the press did nothing.
    Stay,
    /// Left from the content's edge: focus goes to the nav item of what is showing.
    Nav,
    /// Another item is to show, by way of its nav item, and take focus if it has anything to give it to: SEE FULL
    /// LIBRARY.
    Show(Item),
    /// Something for the Dashboard.
    Action(PageAction),
}

/// An item's content, beside the nav and under the header. A pane lives as long as the page, showing or not, and
/// keeps its own focus and what it remembers; it is told the library each time and keeps none of it.
trait Pane {
    /// The pane's own name in the focus log, in place of its item's.
    fn log_name(&self) -> Option<String> {
        None
    }

    /// The number at the item's right in the nav.
    fn nav_count(&self, _around: Around) -> Option<usize> {
        None
    }

    /// The count at the header's right.
    fn count(&self, _around: Around) -> Option<String> {
        None
    }

    /// Focus comes into the content, from the nav or as the page opens: where it was when it last left, or the
    /// content's first place. False if there is nothing to give it to, and focus stays on the nav.
    fn enter(&mut self, _around: Around) -> bool {
        false
    }

    /// Focus is in the content and the library may have changed under it: put it where it should be now. False if
    /// nothing is left to hold it, and it goes to the nav.
    fn holds_focus(&mut self, _around: Around) -> bool {
        false
    }

    /// Where focus is in the content, for the focus log. None for a pane that never takes focus.
    fn focus(&self) -> Option<String> {
        None
    }

    /// What time has brought the pane while it shows ([`Page::tick`]).
    fn tick(&mut self, _around: Around) -> Tick {
        Tick::Nothing
    }

    /// A press with focus in the content, but for B and the bumpers.
    fn press(&mut self, _press: Press, _around: Around) -> Outcome {
        Outcome::Stay
    }

    /// B while the pane is showing, focus in it or on the nav. True if the pane had a use for it (Full library
    /// dropping its name filter); otherwise the page closes.
    fn back(&mut self, _around: Around) -> bool {
        false
    }

    /// LB or RB while the pane is showing, focus in it or on the nav: Full library's tabs.
    fn bumper(&mut self, _button: Button, _around: Around) {}

    /// The page has moved on to another item: what is remembered only while showing is forgotten.
    fn leave(&mut self) {}

    /// The hint bar's hints with focus where it is in the content.
    fn hints(&self, _around: Around) -> &'static [Hint] {
        hint_bar::NONE
    }

    /// Draws the content. The page has drawn the header, and moved everything by the fly-in's offset.
    fn draw(&mut self, ui: &mut Ui, around: Around, look: Look);
}

/// A pane that is a line of dim text at the content row's top, and takes no focus: Apps until M4, and Full library
/// until #53, with no line at all.
struct Note {
    text: &'static str,
    log_name: Option<String>,
    count: fn(&Library) -> Option<usize>,
}

impl Pane for Note {
    fn log_name(&self) -> Option<String> {
        self.log_name.clone()
    }

    fn nav_count(&self, around: Around) -> Option<usize> {
        (self.count)(around.library)
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, look: Look) {
        ui.with_alpha(look.fade, |ui| ui.label(548.0, 190.0, self.text, &TEXT_DIM));
    }
}

/// Apps until M4: the item is in the nav, counts none, and its pane says where Apps will come from. The Bundle's
/// line names Playnite's software tools, which are not this Dashboard's.
fn apps() -> Note {
    Note { text: "Apps you add show up here.", log_name: None, count: |_| Some(0) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// On a nav item: the one showing, or one on its way to showing.
    Nav(Item),
    /// In the content of the item showing.
    Content,
}

pub struct MyGames {
    /// The item whose content is up.
    showing: Item,
    focus: Focus,
    /// Whether focus has been given to the content the page opened on, or to the nav for want of any: the page is
    /// opened without the library, and finds out at its first press or frame.
    arrived: bool,
    settle: Settle,
    /// The fly-in the pane showing came by, if it came by focus moving along the nav: which one, counted, and how
    /// far below its rest it set out (above, if negative).
    flight: Option<(u32, f32)>,
    games: Games,
    apps: Note,
    history: History,
    full_library: Box<dyn Pane>,
    manage: Manage,
}

impl MyGames {
    pub fn open(entry: Entry) -> Self {
        let (showing, filter) = match entry {
            Entry::Games => (Item::Games, None),
            Entry::PlayHistory => (Item::PlayHistory, None),
            Entry::FullLibrary { filter } => (Item::FullLibrary, filter),
            Entry::Queue => (Item::Manage, None),
        };
        let mut page = Self {
            showing,
            // Every way in gives focus to the content, not the nav.
            focus: Focus::Content,
            arrived: false,
            settle: Settle::default(),
            flight: None,
            games: Games::new(),
            apps: apps(),
            history: History::default(),
            full_library: full_library(filter),
            manage: Manage,
        };
        // A pane that never takes focus leaves it on the nav from the start.
        if page.pane(showing).focus().is_none() {
            (page.focus, page.arrived) = (Focus::Nav(showing), true);
        }
        page
    }

    fn pane(&self, item: Item) -> &dyn Pane {
        match item {
            Item::Games => &self.games,
            Item::Apps => &self.apps,
            Item::PlayHistory => &self.history,
            Item::FullLibrary => &*self.full_library,
            Item::Manage => &self.manage,
        }
    }

    fn pane_mut(&mut self, item: Item) -> &mut dyn Pane {
        match item {
            Item::Games => &mut self.games,
            Item::Apps => &mut self.apps,
            Item::PlayHistory => &mut self.history,
            Item::FullLibrary => &mut *self.full_library,
            Item::Manage => &mut self.manage,
        }
    }

    /// Shows `item` in place of what is up, because focus moved along the nav: it flies in, from below if it is
    /// lower on the nav than what it replaces.
    fn show(&mut self, item: Item) {
        if item == self.showing {
            return;
        }
        self.pane_mut(self.showing).leave();
        let flights = self.flight.map_or(0, |(count, _)| count);
        self.flight = Some((flights + 1, if item.is_below(self.showing) { FLY } else { -FLY }));
        self.showing = item;
    }

    /// Focus is where it can be: in the content the page opened on if that has anything to focus, and still on
    /// something if the library changed under it; otherwise on the nav item of what is showing.
    fn keep(&mut self, around: Around) {
        let showing = self.showing;
        let held = match (self.focus, std::mem::replace(&mut self.arrived, true)) {
            (Focus::Nav(_), _) => return,
            (Focus::Content, false) => self.pane_mut(showing).enter(around),
            (Focus::Content, true) => self.pane_mut(showing).holds_focus(around),
        };
        if !held {
            self.focus = Focus::Nav(showing);
        }
    }

    /// Focus goes into the content of `item`, which is showing, if it has anything to give it to.
    fn enter(&mut self, item: Item, around: Around) {
        if self.pane_mut(item).enter(around) {
            self.focus = Focus::Content;
        }
    }

    fn press_nav(&mut self, item: Item, press: Press, around: Around, now: Instant) {
        match press.button {
            // Along the nav, stopping at its ends. The item lights at once; its content follows by the settle
            // wait.
            Button::Up | Button::Down => {
                let next = if press.button == Button::Up { item.above() } else { item.below() };
                let Some(next) = next else { return };
                self.focus = Focus::Nav(next);
                if let Some(open) = self.settle.focused(next, self.showing, now) {
                    self.show(open);
                }
            }
            // Into the item's content, the item showing at once if it was still waiting to: never into what was
            // showing before it. With nothing there to focus, focus stays.
            Button::Right | Button::A if !press.repeat => {
                self.settle.drop_pending();
                self.show(item);
                self.enter(item, around);
            }
            // Left is nothing; nor are Y, X, Menu and View: no game has the pad.
            _ => {}
        }
    }

    fn user<'a>(around: Around<'a>) -> &'a str {
        around.library.user.as_deref().unwrap_or("Player")
    }
}

impl Page for MyGames {
    fn name(&self) -> &'static str {
        "MyGames"
    }

    /// The item showing, then where focus is: `Games: Tile(3)`, `Games: SeeFullLibrary`, `PlayHistory: Row(0)`,
    /// `Manage: Queue`, or on the nav `Games: Nav(Apps)`, which is focus on Apps while Games is still up (the
    /// settle wait; it is `Apps: Nav(Apps)` at once when Apps was reached in one step).
    fn focus(&self) -> String {
        let pane = self.pane(self.showing);
        let showing = pane.log_name().unwrap_or_else(|| format!("{:?}", self.showing));
        match self.focus {
            Focus::Nav(item) => format!("{showing}: Nav({item:?})"),
            Focus::Content => format!("{showing}: {}", pane.focus().unwrap_or_default()),
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        let now = Instant::now();
        self.keep(around);
        // An item whose wait ended with no frame drawn since is showing by now.
        if let Some(item) = self.settle.due(now) {
            self.show(item);
        }
        let showing = self.showing;
        match (press.button, self.focus) {
            (Button::B, _) => {
                if press.repeat || self.pane_mut(showing).back(around) {
                    return None;
                }
                // Home, at once. An item still waiting to show does not.
                self.settle.drop_pending();
                return Some(PageAction::Close);
            }
            (Button::LeftBumper | Button::RightBumper, _) => {
                if !press.repeat {
                    self.pane_mut(showing).bumper(press.button, around);
                }
            }
            (_, Focus::Nav(item)) => self.press_nav(item, press, around, now),
            (_, Focus::Content) => match self.pane_mut(showing).press(press, around) {
                Outcome::Stay => {}
                Outcome::Nav => {
                    self.focus = Focus::Nav(showing);
                    self.settle.focused(showing, showing, now);
                }
                Outcome::Show(item) => {
                    // By way of the item's place on the nav, so its pane flies in as from there.
                    self.focus = Focus::Nav(item);
                    self.settle.focused(item, item, now);
                    self.show(item);
                    self.enter(item, around);
                }
                Outcome::Action(action) => return Some(action),
            },
        }
        None
    }

    fn tick(&mut self, around: Around) -> Tick {
        let showing = self.showing;
        self.pane_mut(showing).tick(around)
    }

    fn hints(&self, around: Around) -> &'static [Hint] {
        match self.focus {
            Focus::Nav(_) => hint_bar::NONE,
            Focus::Content => self.pane(self.showing).hints(around),
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around) {
        self.keep(around);
        // The item focus rests on shows once its wait is over: a frame is asked for then. A frame that is to show
        // the page at rest does not wait.
        let now = Instant::now();
        let due = if ui.store.settling() { self.settle.take() } else { self.settle.due(now) };
        if let Some(item) = due {
            self.show(item);
        } else if let Some(wait) = self.settle.wait(now) {
            ui.store.wake_in(wait.as_secs_f32());
        }

        ui.fill(ui.screen(), PAGE, 0.0);
        let showing = self.showing;
        let focused = match self.focus {
            Focus::Nav(item) => Some(item),
            Focus::Content => None,
        };
        let count = |item: Item| self.pane(item).nav_count(around);
        nav::draw(ui, &nav::Face { user: Self::user(around), showing, focused, count: &count });

        // The pane that came by the nav slides the 50 to its rest and fades in as it does; the grid of games
        // slides with it at its full strength. Under Reduce motion it is simply there.
        let (rise, fade) = match self.flight {
            Some((flight, from)) if !ui.store.reduce_motion => (
                ui.slide_from(("my games fly", flight), from, 0.0, FLY_SECONDS, Ease::CubicOut),
                ui.tween_from(("my games fade", flight), 0.0, 1.0, FLY_SECONDS, Ease::QuadOut),
            ),
            _ => (0.0, 1.0),
        };
        let look = Look { focused: focused.is_none(), fade };
        ui.with_offset(0.0, rise, |ui| {
            ui.with_alpha(fade, |ui| {
                ui.label(CONTENT_LEFT, HEADER_TOP, showing.title(), &HEADER);
                if let Some(count) = self.pane(showing).count(around) {
                    let width = ui.measure(&TEXT_DIM, &count).width;
                    ui.label(CONTENT_RIGHT - width, COUNT_TOP, &count, &TEXT_DIM);
                }
            });
            self.pane_mut(showing).draw(ui, around, look);
        });
        progress(ui, around);
    }
}
