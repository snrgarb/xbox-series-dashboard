//! Full library: every game, installed or not, in the grid, under a tab for each Group and each launcher, and the
//! Sort and Filter row (docs/spec/my-games.md, sections 7 and 18; docs/spec/filters.md, sections 4.2 and 8).
//!
//! - **The tabs** are the saved filters that are Home's Groups, in the same order, and each launcher's whole
//!   library, installed or not, whether it is a Group or not (`Library::tabs`). The lit one is the filter the
//!   grid shows; none is lit as the nav opens the page, and the grid is then every game by name. The tabs take no
//!   focus themselves: focus "on the tabs" shows nothing, and Left and Right there step the lit tab, as LB and RB do
//!   from anywhere on the page.
//! - **Sort** opens a list of the sorts over the grid. One chosen there orders every tab until the page closes;
//!   until then a tab is in its filter's own order.
//! - **The name box** shows the name the grid is narrowed by. It is never typed into: A or Y on it opens text entry
//!   over the page, and the grid narrows on every key.
//!
//! The pane is told the library each time and keeps none of it: what it remembers is the lit tab's filter by its
//! id, the sort chosen, the name, and where focus is.
use std::cmp::Reverse;
use std::collections::HashMap;

use model::{Filter, Game, Library, Sort};
use ui::{Button, Color, Ease, Gradient, Press, Rect, Ui};

use super::games::{sort_key, EMPTY};
use super::grid::{self, Grid, Step};
use super::look::{self, glyph as page_glyph, BUTTON, PAGE, TEXT, TEXT_DIM};
use super::{open_game_hub, open_tile_menu, Look, Outcome, Pane, FLY, FLY_SECONDS};
use crate::hint_bar::{self, Hint};
use crate::keyboard::{self, Event};
use crate::pages::{Around, Open, PageAction};
use crate::theme::glyph;

/// The tabs' strip: from the content's left edge to its right, 56 tall, at the content row's top. A tab is its
/// name with 22 either side, and the next is 8 on. The strip is cut 24 outside itself, for the lit tab's glow.
const TABS: Rect = Rect::new(520.0, 190.0, 1230.0, 56.0);
const TAB_SIDE: f32 = 22.0;
const TAB_GAP: f32 = 8.0;
const TABS_ROOM: f32 = 24.0;
/// The Sort and Filter row: 72 under the content row's top, 60 tall, 4 inside the content's edges.
const SORT: Rect = Rect::new(524.0, 262.0, 400.0, 60.0);
/// The name box, 24 right of Sort.
const NAME: Rect = Rect::new(948.0, 262.0, 400.0, 60.0);
const ROW_RIGHT: f32 = 1746.0;
/// What is in a box of the row is 22 in from its sides; in the Filter button, 26, its glyph 14 before its label.
const BOX_SIDE: f32 = 22.0;
const FILTER_SIDE: f32 = 26.0;
const FILTER_GAP: f32 = 14.0;
/// The count ends 24 left of Filter.
const COUNT_GAP: f32 = 24.0;
const CHEVRON_SIZE: f32 = 18.0;
const GLYPH_SIZE: f32 = 20.0;
/// The name in its box ends 14 before the glyph.
const NAME_GAP: f32 = 14.0;
/// The Sort list: 8 under the Sort box and as wide, 8 over its first option and under its last, each option 60
/// tall. An option's name is 62 in; the tick stands in the middle of what is right of its first 18, at the left of
/// a box 44 wide, which is where the Bundle's picture has it: after the name, not before it.
const LIST: Color = Color::rgb(0x2B2B2B);
const LIST_UNDER: f32 = 8.0;
const LIST_PADDING: f32 = 8.0;
const OPTION: f32 = 60.0;
const OPTION_NAME: f32 = 62.0;
const TICK: f32 = 18.0 + (SORT.w - 18.0 - 44.0) / 2.0 + GLYPH_SIZE / 2.0;
/// Over the grid's top, from where it is cut: the page's colour, clear 12 lower.
const FADE: f32 = 12.0;
/// What text entry says over the field.
const CAPTION: &str = "Filter by name";

/// A sort's name in the Sort box and its list.
fn sort_name(sort: Sort) -> &'static str {
    match sort {
        Sort::Name => "A\u{2013}Z",
        Sort::LastPlayed => "Last played",
        Sort::Added => "Recently added",
        Sort::Playtime => "Playtime",
    }
}

/// Orders games as the library database orders a filter's (docs/spec/filters.md, 3.2), each sort in its own
/// direction: names upward, the rest newest or most first, a game that has none of it last, ties by name.
fn sort_games(games: &mut [&Game], sort: Sort) {
    let name = |game: &Game| (sort_key(&game.name), game.name.clone(), game.id);
    match sort {
        Sort::Name => games.sort_by_cached_key(|game| name(game)),
        Sort::LastPlayed => games.sort_by_cached_key(|game| (Reverse(game.last_played), name(game))),
        Sort::Added => games.sort_by_cached_key(|game| (Reverse(game.added), name(game))),
        Sort::Playtime => games.sort_by_cached_key(|game| (Reverse(game.playtime_minutes), name(game))),
    }
}

/// What a view `width` wide, scrolled by `scroll`, must be scrolled by to show what lies from `left` to `right` in
/// it: as little further as that takes.
fn show(scroll: f32, left: f32, right: f32, width: f32) -> f32 {
    scroll.max(right - width).min(left)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// On the tabs, which shows nothing: the lit tab is lit with focus or without.
    Tabs,
    Sort,
    /// The name box, while it shows.
    Name,
    Filter,
    /// An option of the open Sort list, by its place in [`Sort::ALL`].
    SortList(usize),
    /// The grid's focused tile.
    Grid,
}

pub struct FullLibrary {
    /// The lit tab's saved filter, by its id. None: no tab is lit, and the grid is every game.
    tab: Option<i64>,
    /// The sort chosen on the dropdown, which orders every tab until the page closes.
    chosen: Option<Sort>,
    /// The name the grid is narrowed by.
    name: String,
    /// The name box is open: it shows though it holds no name.
    name_open: bool,
    /// Text entry is up over the page, and this was the name as it opened.
    typing: Option<String>,
    focus: Focus,
    /// What in the Sort and Filter row last had focus: where Down from the tabs and Up from the grid go.
    row: Focus,
    grid: Grid,
    /// The tabs switched: which time, counted, and how far right of its rest the grid set out (left, if
    /// negative).
    flight: Option<(u32, f32)>,
    /// How far the tabs' strip is scrolled, when the tabs are wider than it.
    tabs_scroll: f32,
}

impl FullLibrary {
    /// On the tab of this saved filter, as See all opens it, or with no tab lit, as the nav does. Focus is the
    /// grid's first game's.
    pub fn open(filter: Option<i64>) -> Self {
        Self {
            tab: filter,
            chosen: None,
            name: String::new(),
            name_open: false,
            typing: None,
            focus: Focus::Grid,
            row: Focus::Sort,
            grid: Grid::full_library(),
            flight: None,
            tabs_scroll: 0.0,
        }
    }

    /// The lit tab's place among the tabs, and its filter.
    fn lit<'a>(&self, library: &'a Library) -> Option<(usize, &'a Filter)> {
        let id = self.tab?;
        library.tabs().enumerate().find(|(_, filter)| filter.id == id)
    }

    /// What the grid is ordered by, and whether downward: the sort chosen, else the lit tab's own, else by name.
    fn order(&self, library: &Library) -> (Sort, bool) {
        match (self.chosen, self.lit(library)) {
            (Some(sort), _) => (sort, sort.default_descending()),
            (None, Some((_, filter))) => (filter.sort, filter.descending),
            (None, None) => (Sort::Name, false),
        }
    }

    /// The option of the Sort list the grid is in, if it is in one of them: a filter may have a sort the other way
    /// up, which is none of them.
    fn ticked(&self, library: &Library) -> Option<usize> {
        let (sort, descending) = self.order(library);
        Sort::ALL.iter().position(|&option| option == sort && option.default_descending() == descending)
    }

    /// The games the grid shows: the lit tab's in its filter's own order, or every game that is not hidden by
    /// name; in the sort chosen, if one was; and of those the ones whose name has the name in it, whatever its
    /// case.
    fn games<'a>(&self, library: &'a Library) -> Vec<&'a Game> {
        let mut games: Vec<&Game> = match self.lit(library) {
            Some((_, filter)) => {
                let by_id: HashMap<i64, &Game> =
                    library.games.iter().chain(&library.hidden).map(|game| (game.id, game)).collect();
                filter.games.iter().filter_map(|id| by_id.get(id).copied()).collect()
            }
            None => {
                let mut all: Vec<&Game> = library.games.iter().collect();
                sort_games(&mut all, Sort::Name);
                all
            }
        };
        if let Some(sort) = self.chosen {
            sort_games(&mut games, sort);
        }
        let wanted = self.name.to_lowercase();
        if !wanted.is_empty() {
            games.retain(|game| game.name.to_lowercase().contains(&wanted));
        }
        games
    }

    /// The name box shows while it is open or holds a name.
    fn name_shows(&self) -> bool {
        self.name_open || !self.name.is_empty()
    }

    /// Focus goes to `to`. The name box left empty closes as focus leaves it.
    fn focus_to(&mut self, to: Focus) {
        if self.focus == Focus::Name && to != Focus::Name && self.name.is_empty() {
            self.name_open = false;
        }
        if matches!(to, Focus::Sort | Focus::Name | Focus::Filter) {
            self.row = to;
        }
        self.focus = to;
    }

    /// To the Sort and Filter row, on what last had focus there: Sort, if that was the name box and it has gone.
    fn focus_row(&mut self) {
        let to = if self.row == Focus::Name && !self.name_shows() { Focus::Sort } else { self.row };
        self.focus_to(to);
    }

    /// Y anywhere on the page, or A on Filter: the name box opens and takes focus. Nothing is narrowed yet.
    fn open_name_box(&mut self) {
        self.name_open = true;
        self.focus_to(Focus::Name);
    }

    /// The library may have changed, or the name: a lit tab whose filter is no longer a tab goes dark, the grid's
    /// focus stays on its game, and focus that was on a game of a grid that is now empty goes to the row. Gives
    /// the games the grid shows.
    fn keep<'a>(&mut self, around: Around<'a>) -> Vec<&'a Game> {
        if self.lit(around.library).is_none() {
            self.tab = None;
        }
        let games = self.games(around.library);
        if !self.grid.keep(&games) && self.focus == Focus::Grid {
            if self.name.is_empty() {
                self.focus_to(Focus::Sort);
            } else {
                self.open_name_box();
            }
        }
        games
    }

    /// Lights the tab `step` along from the lit one, its games flying in from that side. With none lit, a step
    /// right lights the first. False where there is no tab that way.
    fn switch(&mut self, step: isize, library: &Library) -> bool {
        let tabs: Vec<i64> = library.tabs().map(|filter| filter.id).collect();
        let to = match self.lit(library) {
            Some((at, _)) => at.checked_add_signed(step),
            None => (step > 0).then_some(0),
        };
        let Some(&tab) = to.and_then(|to| tabs.get(to)) else { return false };
        self.tab = Some(tab);
        // Another tab's games: the grid starts again at its first.
        self.grid.forget();
        let flights = self.flight.map_or(0, |(count, _)| count);
        self.flight = Some((flights + 1, if step > 0 { FLY } else { -FLY }));
        true
    }

    /// A or Y on the name box: text entry over the page, from the name as it is.
    fn type_name(&mut self) -> Outcome {
        self.typing = Some(self.name.clone());
        Outcome::Action(PageAction::Open(Open::TextEntry(keyboard::Entry::new(CAPTION))))
    }

    /// A press with the Sort list open, which takes every button: Up and Down move through it and stop at its
    /// ends, and A sorts the grid by the option, closes the list and puts focus back on the Sort box.
    fn press_list(&mut self, at: usize, press: Press) {
        match press.button {
            Button::Up => self.focus = Focus::SortList(at.saturating_sub(1)),
            Button::Down => self.focus = Focus::SortList((at + 1).min(Sort::ALL.len() - 1)),
            Button::A if !press.repeat => {
                self.chosen = Some(Sort::ALL[at]);
                // The grid in its new order: Down from the row starts again at its first game.
                self.grid.forget();
                self.focus = Focus::Sort;
            }
            _ => {}
        }
    }

    /// The Filter button: as wide as its glyph and label need, its right edge the row's.
    fn filter_button(ui: &mut Ui) -> Rect {
        let width = 2.0 * FILTER_SIDE + GLYPH_SIZE + FILTER_GAP + ui.measure(&TEXT, "Filter").width;
        Rect::new(ROW_RIGHT - width, SORT.y, width, SORT.h)
    }

    /// The tabs, the lit one filled with the accent and glowing. They scroll, at once, the least that shows the
    /// lit one whole.
    fn draw_tabs(&mut self, ui: &mut Ui, library: &Library) {
        let lit = self.lit(library).map(|(at, _)| at);
        let mut left = 0.0;
        let tabs: Vec<(f32, f32, &str)> = library
            .tabs()
            .map(|filter| {
                let width = ui.measure(&TEXT, &filter.name).width + 2.0 * TAB_SIDE;
                let tab = (left, width, filter.name.as_str());
                left += width + TAB_GAP;
                tab
            })
            .collect();
        let all = (left - TAB_GAP).max(0.0);
        if let Some(&(left, width, _)) = lit.and_then(|at| tabs.get(at)) {
            self.tabs_scroll = show(self.tabs_scroll, left, left + width, TABS.w);
        }
        self.tabs_scroll = self.tabs_scroll.clamp(0.0, (all - TABS.w).max(0.0));
        let scroll = self.tabs_scroll;
        ui.with_clip(TABS.grow(TABS_ROOM), |ui| {
            for (at, &(left, width, name)) in tabs.iter().enumerate() {
                let tab = Rect::new(TABS.x + left - scroll, TABS.y, width, TABS.h);
                if lit == Some(at) {
                    look::slab(ui, tab, 2.0);
                }
                ui.label_middle(tab.x + TAB_SIDE, tab.center().1, name, &TEXT);
            }
        });
    }

    /// The Sort box, the name box while it shows, the count and the Filter button.
    fn draw_row(&self, ui: &mut Ui, library: &Library, count: usize, focused: Option<Focus>) {
        let middle = SORT.center().1;
        let button = |ui: &mut Ui, rect: Rect, focus: Focus| {
            if focused == Some(focus) {
                look::slab(ui, rect, 2.0);
            } else {
                ui.fill(rect, BUTTON, 2.0);
            }
        };

        button(ui, SORT, Focus::Sort);
        let label = match self.ticked(library) {
            Some(option) => format!("Sorted by {}", sort_name(Sort::ALL[option])),
            None => "Sort".to_string(),
        };
        ui.label_middle(SORT.x + BOX_SIDE, middle, &label, &TEXT);
        ui.icon(SORT.right() - BOX_SIDE - CHEVRON_SIZE / 2.0, middle, page_glyph::CHEVRON, CHEVRON_SIZE, Color::WHITE);

        if self.name_shows() {
            // The box stays dark with focus, so that the name stays legible: a ring on its own edge instead.
            ui.fill(NAME, BUTTON, 2.0);
            let glyph_left = NAME.right() - BOX_SIDE - GLYPH_SIZE;
            if self.name.is_empty() {
                ui.label_middle(NAME.x + BOX_SIDE, middle, CAPTION, &TEXT_DIM);
            } else {
                let text = Rect::new(NAME.x + BOX_SIDE, NAME.y, glyph_left - NAME_GAP - NAME.x - BOX_SIDE, NAME.h);
                ui.with_clip(text, |ui| ui.label_middle(text.x, middle, &self.name, &TEXT));
            }
            ui.icon(glyph_left + GLYPH_SIZE / 2.0, middle, glyph::SEARCH, GLYPH_SIZE, Color::WHITE);
            // Not while text entry is over the page: focus is on its keyboard then (the Bundle's picture 13).
            if focused == Some(Focus::Name) && self.typing.is_none() {
                look::ring_inside(ui, NAME);
            }
        }

        let filter = Self::filter_button(ui);
        button(ui, filter, Focus::Filter);
        let glyph_middle = filter.x + FILTER_SIDE + GLYPH_SIZE / 2.0;
        ui.icon(glyph_middle, middle, page_glyph::FILTER, GLYPH_SIZE, Color::WHITE);
        ui.label_middle(filter.x + FILTER_SIDE + GLYPH_SIZE + FILTER_GAP, middle, "Filter", &TEXT);
        ui.label_right(filter.x - COUNT_GAP, middle, &format!("{count} games"), &TEXT_DIM);
    }

    /// The open Sort list, over the grid, on a layer of its own so that it covers the names on the tiles under it.
    /// It has no shadow and no edge, and the focused option's glow is cut at its sides.
    fn draw_list(&self, ui: &mut Ui, library: &Library, focused: usize) {
        let height = 2.0 * LIST_PADDING + OPTION * Sort::ALL.len() as f32;
        let list = Rect::new(SORT.x, SORT.bottom() + LIST_UNDER, SORT.w, height);
        let ticked = self.ticked(library);
        ui.layer();
        ui.fill(list, LIST, 0.0);
        ui.with_clip(list, |ui| {
            let option = |at: usize| Rect::new(list.x, list.y + LIST_PADDING + OPTION * at as f32, list.w, OPTION);
            look::slab(ui, option(focused), 0.0);
            for (at, &sort) in Sort::ALL.iter().enumerate() {
                let (rect, middle) = (option(at), option(at).center().1);
                if ticked == Some(at) {
                    ui.icon(rect.x + TICK, middle, page_glyph::TICK, GLYPH_SIZE, Color::WHITE);
                }
                ui.label_middle(rect.x + OPTION_NAME, middle, sort_name(sort), &TEXT);
            }
        });
    }
}

impl Pane for FullLibrary {
    /// The games the grid shows, under the tab and the name: the nav's count follows both.
    fn nav_count(&self, around: Around) -> Option<usize> {
        Some(self.games(around.library).len())
    }

    /// From the nav, or as the page opens: the grid's game focus last left, else its first. With a name that finds
    /// nothing, the name box, where the name is changed; with no games at all, Sort: the row is always there.
    fn enter(&mut self, around: Around) -> bool {
        self.focus = Focus::Grid;
        self.keep(around);
        true
    }

    /// The row is always there to hold focus, whatever became of the grid.
    fn holds_focus(&mut self, around: Around) -> bool {
        self.keep(around);
        true
    }

    fn focus(&self) -> Option<String> {
        Some(match self.focus {
            Focus::Grid => format!("Tile({})", self.grid.focus()),
            Focus::SortList(option) => format!("SortList({:?})", Sort::ALL[option]),
            focus => format!("{focus:?}"),
        })
    }

    /// `tab 2/4 "Most played", sort Playtime, name "ha", 3 games`: the lit tab's place among the tabs (0 with
    /// none lit) and its name, what the grid is ordered by (`reversed` where that is none of the Sort list's), the
    /// name it is narrowed by, and how many games it shows.
    fn state(&self, around: Around) -> String {
        let library = around.library;
        let (at, name) = self.lit(library).map_or((0, ""), |(at, filter)| (at + 1, filter.name.as_str()));
        let (sort, _) = self.order(library);
        let reversed = if self.ticked(library).is_none() { " reversed" } else { "" };
        let (tabs, games) = (library.tabs().count(), self.games(library).len());
        format!("tab {at}/{tabs} {name:?}, sort {sort:?}{reversed}, name {:?}, {games} games", self.name)
    }

    fn press(&mut self, press: Press, around: Around) -> Outcome {
        let games = self.keep(around);
        let library = around.library;
        let once = !press.repeat;
        match (self.focus, press.button) {
            // The open list keeps every button from the page behind it.
            (Focus::SortList(at), _) => self.press_list(at, press),
            (Focus::Name, Button::A | Button::Y) if once => return self.type_name(),
            // Y anywhere else on the page opens the name box.
            (_, Button::Y) if once => self.open_name_box(),

            // The tabs: Left and Right step the lit one; Left with none before it is the nav.
            (Focus::Tabs, Button::Left) => {
                if !self.switch(-1, library) {
                    return Outcome::Nav;
                }
            }
            // With no tab lit Right is nothing: LB and RB are what light one then.
            (Focus::Tabs, Button::Right) => {
                if self.tab.is_some() {
                    self.switch(1, library);
                }
            }
            (Focus::Tabs, Button::Down) => self.focus_row(),

            // The row.
            (Focus::Sort, Button::Left) => return Outcome::Nav,
            (Focus::Sort, Button::Right) => {
                self.focus_to(if self.name_shows() { Focus::Name } else { Focus::Filter });
            }
            (Focus::Sort, Button::A) if once => self.focus = Focus::SortList(self.ticked(library).unwrap_or(0)),
            (Focus::Name, Button::Left) => self.focus_to(Focus::Sort),
            (Focus::Name, Button::Right) => self.focus_to(Focus::Filter),
            (Focus::Filter, Button::Left) => {
                self.focus_to(if self.name_shows() { Focus::Name } else { Focus::Sort });
            }
            (Focus::Filter, Button::A) if once => self.open_name_box(),
            (Focus::Sort | Focus::Name | Focus::Filter, Button::Up) => self.focus_to(Focus::Tabs),
            // The game focus last left the grid from, else its first; with no games focus stays.
            (Focus::Sort | Focus::Name | Focus::Filter, Button::Down) if !games.is_empty() => {
                self.focus_to(Focus::Grid);
            }

            // The grid: as Games', but that Up from its first row is the row, and nothing is under its last.
            (Focus::Grid, button) if button.is_direction() => match self.grid.step(button, &games) {
                Step::Off(Button::Left) => return Outcome::Nav,
                Step::Off(Button::Up) => self.focus_row(),
                Step::Moved | Step::Stopped | Step::Off(_) => {}
            },
            (Focus::Grid, Button::A) if once => {
                return open_game_hub(games[self.grid.focus()]).map_or(Outcome::Stay, Outcome::Action);
            }
            (Focus::Grid, Button::Menu | Button::X) if once => {
                return open_tile_menu(games[self.grid.focus()]).map_or(Outcome::Stay, Outcome::Action);
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// B, the first that applies (docs/spec/my-games.md, 3.5): the Sort list closes; in the name box the name is
    /// cleared and the box closes, focus on Filter; elsewhere a name is cleared, focus where it was. Otherwise the
    /// page closes.
    fn back(&mut self, around: Around) -> bool {
        match self.focus {
            Focus::SortList(_) => self.focus = Focus::Sort,
            Focus::Name => {
                self.name.clear();
                self.name_open = false;
                self.focus_to(Focus::Filter);
            }
            _ if !self.name.is_empty() => {
                self.name.clear();
                self.name_open = false;
            }
            _ => return false,
        }
        self.keep(around);
        true
    }

    /// LB and RB step the lit tab from anywhere on the page, the nav included, but not under the open Sort list.
    /// They stop at the first tab and the last.
    fn bumper(&mut self, button: Button, around: Around) {
        if matches!(self.focus, Focus::SortList(_)) {
            return;
        }
        self.keep(around);
        self.switch(if button == Button::LeftBumper { -1 } else { 1 }, around.library);
        self.keep(around);
    }

    /// The page moves to another item: the name goes and the grid forgets its game. The tab and the sort chosen
    /// stay for as long as the page is up.
    fn leave(&mut self) {
        self.name.clear();
        (self.name_open, self.typing) = (false, None);
        (self.focus, self.row) = (Focus::Grid, Focus::Sort);
        self.grid.forget();
    }

    fn hints(&self, _around: Around) -> &'static [Hint] {
        // Y opens the name box here, which is not hinted at.
        if self.focus == Focus::Grid {
            hint_bar::MORE_OPTIONS
        } else {
            hint_bar::NONE
        }
    }

    fn entry_text(&self) -> String {
        self.name.clone()
    }

    /// The grid narrows on every key. Kept, a new name that finds something puts focus on the first game found;
    /// otherwise, and dropped, focus is on the name box still, open, the name dropped put back as it was.
    fn entry(&mut self, event: Event, around: Around) {
        let before = self.typing.clone().unwrap_or_default();
        let (name, kept) = match event {
            Event::Typed(name) => (name, None),
            Event::Kept(name) => (name, Some(true)),
            Event::Dropped(start) => (start, Some(false)),
        };
        if name != self.name {
            self.name = name;
            self.grid.forget();
        }
        if let Some(kept) = kept {
            self.typing = None;
            let found = !self.games(around.library).is_empty();
            if kept && !self.name.is_empty() && self.name != before && found {
                self.grid.forget();
                self.focus_to(Focus::Grid);
            } else {
                self.open_name_box();
            }
        }
        self.keep(around);
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, look: Look) {
        let games = self.keep(around);
        let library = around.library;
        let focused = look.focused.then_some(self.focus);
        // The tabs and the row fade in with the pane as it comes from the nav; the grid only slides.
        ui.with_alpha(look.fade, |ui| {
            self.draw_tabs(ui, library);
            self.draw_row(ui, library, games.len(), focused);
        });

        // Across the tabs only the grid moves: in from the side its tab is on, without a fade.
        let across = match self.flight {
            Some((flight, from)) => ui.slide_from(("full library tab", flight), from, 0.0, FLY_SECONDS, Ease::CubicOut),
            None => 0.0,
        };
        let running = around.playing.and_then(|at| library.games.get(at)).map(grid::key);
        ui.with_offset(across, 0.0, |ui| {
            self.grid.draw(ui, &games, focused == Some(Focus::Grid), running, &library.queue, EMPTY);
        });
        // Tiles scrolled up pass under this and are cut where it starts, so they never draw over the row.
        let fade = Rect::new(grid::LIST.0, grid::FULL_LIBRARY_CUT, grid::LIST.1, FADE);
        ui.gradient(fade, PAGE, PAGE.fade(0.0), Gradient::Vertical, 0.0);

        if let Some(Focus::SortList(option)) = focused {
            self.draw_list(ui, library, option);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: i64, name: &str, last_played: u64, playtime_minutes: u32, added: u64) -> Game {
        Game { id, name: name.into(), last_played, playtime_minutes, added, ..Game::default() }
    }

    fn names(games: &[&Game]) -> Vec<String> {
        games.iter().map(|game| game.name.clone()).collect()
    }

    /// docs/spec/filters.md, 3.2: each sort in its own direction, a game without the field last, ties by name.
    #[test]
    fn each_sort_orders_as_the_library_does() {
        let all = [
            game(1, "The Zephyr", 0, 0, 50),
            game(2, "moss", 300, 10, 50),
            game(3, "Anchor", 0, 0, 70),
            game(4, "An Orchard", 900, 10, 60),
        ];
        let sorted = |sort| {
            let mut games: Vec<&Game> = all.iter().collect();
            sort_games(&mut games, sort);
            names(&games)
        };
        assert_eq!(sorted(Sort::Name), ["Anchor", "moss", "An Orchard", "The Zephyr"]);
        assert_eq!(sorted(Sort::LastPlayed), ["An Orchard", "moss", "Anchor", "The Zephyr"]);
        assert_eq!(sorted(Sort::Added), ["Anchor", "An Orchard", "moss", "The Zephyr"]);
        assert_eq!(sorted(Sort::Playtime), ["moss", "An Orchard", "Anchor", "The Zephyr"]);
    }

    /// docs/spec/my-games.md, 7.2: the strip moves the least that shows the lit tab whole.
    #[test]
    fn the_tabs_scroll_only_as_far_as_the_lit_one_needs() {
        assert_eq!(show(0.0, 100.0, 300.0, 1230.0), 0.0, "in view: nothing moves");
        assert_eq!(show(0.0, 1200.0, 1400.0, 1230.0), 170.0, "off the right: its right edge comes to the strip's");
        assert_eq!(show(170.0, 60.0, 200.0, 1230.0), 60.0, "off the left: its left edge comes to the strip's");
    }
}
