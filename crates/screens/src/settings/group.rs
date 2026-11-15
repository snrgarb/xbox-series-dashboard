//! A custom Group's page (docs/spec/filters.md, 6.3): its name with how many games it holds, Rename, and the
//! dropdowns and the checkbox row that say which games it picks and how it sorts them. Each control saves as it
//! changes: an `Effect::Filter` with the whole of the filter as it is to be, and the next snapshot says what it
//! holds now.
use model::{Filter, FilterChange, Library, Played, Rule, Sort};
use ui::{Button, Press, Rect, Ui};

use super::controls::{self, Dropdown, FIELD_HEIGHT, FIELD_WIDTH, LEFT, TOP};
use super::{Pane, Step};
use crate::keyboard::{self, Event};
use crate::pages::{Around, Open, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::Effect;

/// Text entry for a Group's name: at most 40 characters.
const NAME: keyboard::Entry = keyboard::Entry::new("Group name").limit(40);
/// From one control's name to the next: the name's line, 4, the control, and 40.
const PITCH: f32 = 34.58 + 4.0 + FIELD_HEIGHT + 40.0;
const UNDER_NAME: f32 = 34.58 + 4.0;

/// The days "In the last ..." can be, and how many hours "... or more" played.
const DAYS: [(u32, &str); 4] =
    [(7, "In the last 7 days"), (30, "In the last 30 days"), (90, "In the last 90 days"), (365, "In the last year")];
const HOURS: [(u32, &str); 4] =
    [(1, "1 hour or more"), (10, "10 hours or more"), (50, "50 hours or more"), (100, "100 hours or more")];
const SORTS: [(Sort, &str); 4] = [
    (Sort::Name, "A\u{2013}Z"),
    (Sort::LastPlayed, "Last played"),
    (Sort::Added, "Recently added"),
    (Sort::Playtime, "Playtime"),
];

/// A control of the page, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Rename,
    Installed,
    Favourites,
    Played,
    Added,
    TimePlayed,
    Store,
    Sort,
}

impl Control {
    /// Its name over it; Rename's is the Group's own name.
    fn name(self) -> &'static str {
        match self {
            Control::Rename => "Rename",
            Control::Installed => "Installed",
            Control::Favourites => "Favourites",
            Control::Played => "Played",
            Control::Added => "Added",
            Control::TimePlayed => "Time played",
            Control::Store => "Store",
            Control::Sort => "Sort",
        }
    }
}

/// The sources the library has games from, by name.
fn sources(library: &Library) -> Vec<String> {
    let mut sources: Vec<String> =
        library.games.iter().chain(&library.hidden).map(|game| game.source.clone()).collect();
    sources.sort();
    sources.dedup();
    sources.retain(|source| !source.is_empty());
    sources
}

/// How a store is named on the page: its source id with a capital ("steam" is "Steam").
fn store_name(source: &str) -> String {
    let mut chars = source.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

pub struct Group {
    id: i64,
    name: String,
    /// How many games it holds, as the last snapshot said.
    count: usize,
    rule: Rule,
    sort: Sort,
    descending: bool,
    /// The sources the library has games from, by name.
    sources: Vec<String>,
    focus: usize,
    list: Dropdown,
    scroll: f32,
    /// A new name text entry kept, to be asked of the shell.
    asks: Vec<Effect>,
}

impl Group {
    /// The page of this filter, focus on Rename for one just made and on Installed otherwise.
    pub fn open(filter: &Filter, made: bool, library: &Library) -> Self {
        let mut page = Self {
            id: filter.id,
            name: String::new(),
            count: 0,
            rule: Rule::default(),
            sort: Sort::Name,
            descending: false,
            sources: sources(library),
            focus: if made { 0 } else { 1 },
            list: Dropdown::default(),
            scroll: 0.0,
            asks: Vec::new(),
        };
        page.take(filter);
        page
    }

    fn take(&mut self, filter: &Filter) {
        self.name = filter.name.clone();
        self.count = filter.games.len();
        self.rule = filter.rule.clone();
        self.sort = filter.sort;
        self.descending = filter.descending;
    }

    /// The page's controls: Store only while the library has games from more than one store.
    fn controls(&self) -> Vec<Control> {
        use Control::*;
        let all = [Rename, Installed, Favourites, Played, Added, TimePlayed, Store, Sort];
        all.into_iter().filter(|&control| control != Store || self.sources.len() > 1).collect()
    }

    fn focused(&self) -> Control {
        let controls = self.controls();
        controls[self.focus.min(controls.len() - 1)]
    }

    /// A dropdown's choices, and the place of the one set now.
    fn choices(&self, control: Control) -> (Vec<String>, usize) {
        let rule = &self.rule;
        let days = |set: Option<u32>| set.and_then(|d| DAYS.iter().position(|&(days, _)| days == d)).map(|i| i + 1);
        let (choices, set): (Vec<String>, usize) = match control {
            Control::Installed => (
                ["Any", "Installed", "Not installed"].map(String::from).to_vec(),
                match rule.installed {
                    None => 0,
                    Some(true) => 1,
                    Some(false) => 2,
                },
            ),
            Control::Played => {
                let mut choices: Vec<String> = ["Any", "Played", "Never played"].map(String::from).to_vec();
                choices.extend(DAYS.iter().map(|(_, name)| name.to_string()));
                let set = match (days(rule.played_within_days), rule.played) {
                    (Some(i), _) => 2 + i,
                    (None, Some(Played::Played)) => 1,
                    (None, Some(Played::Never)) => 2,
                    (None, None) => 0,
                };
                (choices, set)
            }
            Control::Added => {
                let choices = std::iter::once("Any time").chain(DAYS.iter().map(|(_, name)| *name));
                (choices.map(String::from).collect(), days(rule.added_within_days).unwrap_or(0))
            }
            Control::TimePlayed => {
                let choices = std::iter::once("Any").chain(HOURS.iter().map(|(_, name)| *name));
                let set = rule.min_playtime_hours.and_then(|h| HOURS.iter().position(|&(hours, _)| hours == h));
                (choices.map(String::from).collect(), set.map_or(0, |i| i + 1))
            }
            Control::Store => {
                let choices = std::iter::once("Any".to_string()).chain(self.sources.iter().map(|s| store_name(s)));
                let set = rule.sources.first().and_then(|source| self.sources.iter().position(|s| s == source));
                (choices.collect(), set.map_or(0, |i| i + 1))
            }
            Control::Sort => (
                SORTS.iter().map(|(_, name)| name.to_string()).collect(),
                Sort::ALL.iter().position(|&s| s == self.sort).unwrap_or(0),
            ),
            Control::Rename | Control::Favourites => (Vec::new(), 0),
        };
        (choices, set)
    }

    /// What a control shows: a dropdown's choice set now, the checkbox's name.
    fn value(&self, control: Control) -> String {
        let (choices, set) = self.choices(control);
        choices.get(set).cloned().unwrap_or_default()
    }

    /// The filter with choice `at` of `control`'s list set.
    fn choose(&mut self, control: Control, at: usize) {
        let rule = &mut self.rule;
        let day = |at: usize| at.checked_sub(1).and_then(|i| DAYS.get(i)).map(|&(days, _)| days);
        match control {
            Control::Installed => rule.installed = [None, Some(true), Some(false)].get(at).copied().flatten(),
            Control::Played => {
                (rule.played, rule.played_within_days) = match at {
                    0 => (None, None),
                    1 => (Some(Played::Played), None),
                    2 => (Some(Played::Never), None),
                    at => (None, day(at - 2)),
                }
            }
            Control::Added => rule.added_within_days = day(at),
            Control::TimePlayed => {
                rule.min_playtime_hours = at.checked_sub(1).and_then(|i| HOURS.get(i)).map(|&(hours, _)| hours);
            }
            Control::Store => {
                rule.sources = at.checked_sub(1).and_then(|i| self.sources.get(i)).cloned().into_iter().collect();
            }
            Control::Sort => {
                self.sort = Sort::ALL.get(at).copied().unwrap_or(Sort::Name);
                self.descending = self.sort.default_descending();
            }
            Control::Rename | Control::Favourites => {}
        }
    }

    /// The filter as the page has it now, to be saved.
    fn change(&self) -> Effect {
        Effect::Filter(FilterChange::Set {
            filter: self.id,
            name: self.name.clone(),
            rule: self.rule.clone(),
            sort: self.sort,
            descending: self.descending,
        })
    }

    fn save(&self) -> Step {
        Step::Do(PageAction::Effect(self.change()))
    }

    /// Where control k's name is, and the control under it, before any scrolling.
    fn name_top(k: usize) -> f32 {
        TOP + PITCH * k as f32
    }

    fn control_rect(&self, ui: &mut Ui, k: usize, control: Control) -> Rect {
        let width = match control {
            Control::Rename => controls::pane_button_width(ui, "Rename"),
            _ => FIELD_WIDTH,
        };
        Rect::new(LEFT, Self::name_top(k) + UNDER_NAME, width, FIELD_HEIGHT)
    }

    fn games(&self) -> String {
        match self.count {
            1 => " \u{b7} 1 game".into(),
            n => format!(" \u{b7} {n} games"),
        }
    }
}

impl Pane for Group {
    fn header(&self) -> String {
        self.name.clone()
    }

    /// `Group: Installed`, and with its list open `Group: Installed > Not installed`.
    fn focus(&self) -> String {
        let control = self.focused().name();
        match self.list.focused() {
            Some(choice) => format!("Group: {control} > {choice}"),
            None => format!("Group: {control}"),
        }
    }

    /// Its name, how many games it holds, and what the focused control is set to: `"Couch co-op", 4 games,
    /// Installed: Any`.
    fn state(&self) -> String {
        let control = self.focused();
        let set = match control {
            Control::Rename => String::new(),
            Control::Favourites => {
                format!(", Favourites only: {}", if self.rule.favourite == Some(true) { "on" } else { "off" })
            }
            control => format!(", {}: {}", control.name(), self.value(control)),
        };
        format!("{:?}, {} games{set}", self.name, self.count)
    }

    fn press(&mut self, press: Press, _around: Around) -> Step {
        let control = self.focused();
        if self.list.is_open() {
            if let Some(at) = self.list.press(press.button) {
                self.choose(control, at);
                return self.save();
            }
            return Step::Stay;
        }
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        let last = self.controls().len() - 1;
        match press.button {
            Button::Up => self.focus = self.focus.min(last).saturating_sub(1),
            Button::Down => self.focus = (self.focus + 1).min(last),
            Button::B | Button::Left => return Step::Back,
            Button::A => match control {
                Control::Rename => return Step::Do(PageAction::Open(Open::TextEntry(NAME))),
                Control::Favourites => {
                    self.rule.favourite = if self.rule.favourite == Some(true) { None } else { Some(true) };
                    return self.save();
                }
                control => {
                    let (choices, set) = self.choices(control);
                    self.list.open(choices, set);
                }
            },
            _ => {}
        }
        Step::Stay
    }

    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        self.sources = sources(around.library);
        if let Some(filter) = around.library.filters.iter().find(|filter| filter.id == self.id) {
            self.take(filter);
        }
        None
    }

    fn entry_text(&self) -> String {
        self.name.clone()
    }

    /// A name kept is saved; one that is empty or all spaces is not.
    fn entry(&mut self, event: Event, _around: Around) {
        if let Event::Kept(text) = event {
            let name = text.trim();
            if !name.is_empty() && name != self.name {
                self.name = name.to_string();
                self.asks.push(self.change());
            }
        }
    }

    fn asks(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.asks)
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, focused: bool) {
        let controls = self.controls();
        let at = self.focus.min(controls.len() - 1);
        let rects: Vec<Rect> =
            controls.iter().enumerate().map(|(k, &control)| self.control_rect(ui, k, control)).collect();
        let focused_rect = rects[at];
        let below = match self.list.is_open() {
            // An open list shows whole under its box, or as much of it as three choices.
            true => 8.0 + (18.0 + 62.0 * self.choices(controls[at]).0.len() as f32).min(202.0),
            false => 0.0,
        };
        let top = Self::name_top(at);
        let mut scroll = self.scroll;
        let this = &*self;
        controls::scrolled(ui, &mut scroll, TOP, top, focused_rect.bottom() + below, |ui| {
            for (k, &control) in controls.iter().enumerate() {
                let y = Self::name_top(k);
                let rect = rects[k];
                let lit = focused && k == at;
                match control {
                    Control::Rename => {
                        let name = ui.label(LEFT, y, &this.name, &BODY);
                        ui.label(LEFT + name.width, y, &this.games(), &BODY.color(TEXT_DIM));
                        controls::pane_button(ui, rect, "Rename", true, lit);
                    }
                    Control::Favourites => {
                        controls::label(ui, LEFT, y, control.name());
                        let on = this.rule.favourite == Some(true);
                        controls::checkbox_row(ui, rect, "Favourites only", on, lit);
                    }
                    control => {
                        controls::label(ui, LEFT, y, control.name());
                        let closed = Dropdown::default();
                        let list = if k == at { &this.list } else { &closed };
                        list.draw(ui, rect, &this.value(control), lit);
                    }
                }
            }
        });
        self.scroll = scroll;
        // The open list over everything under its box, cut short of the screen's foot where it is now.
        let open = Rect { y: focused_rect.y - self.scroll, ..focused_rect };
        self.list.draw_list(ui, open);
    }
}

#[cfg(test)]
mod tests {
    use model::FilterKind;

    use super::*;

    fn page() -> Group {
        let filter = Filter {
            id: 9,
            kind: FilterKind::Custom,
            key: None,
            name: "Couch co-op".into(),
            shown: true,
            rule: Rule::default(),
            sort: Sort::Name,
            descending: false,
            games: vec![1, 2],
        };
        let game = |source: &str| model::Game { source: source.into(), ..model::Game::default() };
        let library = Library { games: vec![game("steam"), game("epic"), game("steam")], ..Library::default() };
        Group::open(&filter, false, &library)
    }

    #[test]
    fn each_choice_sets_its_clause_and_reads_back_as_set() {
        let mut page = page();
        for control in
            [Control::Installed, Control::Played, Control::Added, Control::TimePlayed, Control::Store, Control::Sort]
        {
            let (choices, set) = page.choices(control);
            assert_eq!(set, 0, "{control:?} starts on its first choice");
            for (at, choice) in choices.iter().enumerate() {
                page.choose(control, at);
                assert_eq!(page.choices(control).1, at, "{control:?} {choice}");
            }
        }
        // The last of each, set: as the rule keeps them.
        assert_eq!(page.rule.installed, Some(false));
        assert_eq!((page.rule.played, page.rule.played_within_days), (None, Some(365)));
        assert_eq!(page.rule.added_within_days, Some(365));
        assert_eq!(page.rule.min_playtime_hours, Some(100));
        assert_eq!(page.rule.sources, ["steam"]);
        assert_eq!((page.sort, page.descending), (Sort::Playtime, true));
        assert_eq!(page.value(Control::Store), "Steam");
        page.choose(Control::Played, 2);
        assert_eq!((page.rule.played, page.rule.played_within_days), (Some(Played::Never), None));
    }

    #[test]
    fn the_store_is_offered_only_with_games_from_more_than_one() {
        let mut page = page();
        assert!(page.controls().contains(&Control::Store));
        page.sources.pop();
        assert!(!page.controls().contains(&Control::Store));
        assert_eq!(page.controls().len(), 7);
    }
}
