//! Match game (docs/spec/match-game.md): the user says which of an online source's games this one is, as
//! Playnite's metadata download lets its user pick from what a search found. For a game whose name finds none or the
//! wrong one: Battle.net's "World of Warcraft: Forever" is "World of Warcraft" on SteamGridDB, and nothing at all by
//! its own name.
//!
//! The art picker's band (docs/spec/art-picker.md, 3), over the dimmed page that opened it: a tab per online art
//! source in Settings > Art's order (`model::ArtOrder`), a search field holding the game's name to start with, and
//! under it what the source's search found for those words. "Automatic" first: no match, the game found by its
//! name as before. LB and RB change source, A on the field (or Y anywhere) opens the Console keyboard to search for
//! other words, A on a result matches the game to it and closes, B leaves it as it was.
//!
//! The Tile menu's More options opens it ("Match game"), and so the Game hub's Manage, which is that menu. The
//! page asks the shell for each search ([`MatchGame::art_asks`]) and is told what came ([`MatchGame::art_answer`]);
//! it fetches nothing itself.
use model::art_match::{Hit, Match};
use model::art_picker::{Answer, Ask, Failure};
use model::{ArtSource, Game, GameKey, Library};
use ui::{Button, Color, Font, HAlign, Press, Rect, TextStyle, Ui, VAlign};

use crate::keyboard::{self, Entry as TextEntry};
use crate::menu_panel::DIM;
use crate::pages::{Around, Dialog, Open, Page, PageAction};
use crate::spinner::spinner;
use crate::theme::{self, BODY, TEXT_DIM};
use crate::Effect;

/// The page's name in the focus log.
pub const NAME: &str = "MatchGame";

/// The band, the tab strip and the line under it: the art picker's (docs/spec/art-picker.md, 3.1 and 3.2).
const BAND: (f32, f32) = (134.0, 808.0);
const BAND_FILL: Color = Color::rgb(0x2D2D2D);
const LEFT: f32 = 132.0;
const RIGHT: f32 = 1691.0;
const STRIP: f32 = 72.0;
const STRIP_FILL: Color = Color::rgb(0x1F1F1F);
const TAB_WIDTH: f32 = 200.0;
const TAB_DIM: Color = Color::rgb(0x8C8C8C);
const BAR: f32 = 6.0;
const LINE_MIDDLE: f32 = 38.0;
/// The search field: from 76 under the strip, 60 high, the contents' width; the glyph 24 in from its left.
const FIELD_TOP: f32 = STRIP + 76.0;
const FIELD_HEIGHT: f32 = 60.0;
const SEARCH: char = '\u{E721}';
/// The list: from 156 under the strip, rows 56 high and 4 apart, as many as fit above the foot: eight.
const LIST_TOP: f32 = STRIP + 156.0;
const ROW: f32 = 56.0;
const ROW_GAP: f32 = 4.0;
const ROWS: usize = 8;
const CELL: Color = Color::rgb(0x3B3B3B);
const CELL_FOCUSED: Color = Color::rgb(0x4A4A4A);
/// The foot: what to press, or what went wrong, its middle 44 above the band's foot.
const FOOT_MIDDLE: f32 = 44.0;
/// The mark on the match in use: the art picker's green disc, 36 across, with a tick.
const IN_USE: f32 = 36.0;
const TICK: char = '\u{E73E}';
const SMALL: TextStyle = TextStyle::new(Font::Ui, 22.0, 400);

/// Text entry's caption.
const WORDS: TextEntry = TextEntry::new("Search for the game").limit(100);

/// How the page is opened: for which game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    game: GameKey,
}

/// The page for `game`, as a page asks for it.
pub fn open(game: &Game) -> Open {
    Open::MatchGame(Entry { game: game.key() })
}

/// Where focus is on a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Field,
    /// A row of the list, from Automatic at 0.
    Row(usize),
}

struct TabState {
    source: ArtSource,
    words: String,
    /// What the search for `words` found; None while it has not come. A source that can't be asked has its reason
    /// here from the start.
    found: Option<Result<Vec<Hit>, Failure>>,
    asked: bool,
    focus: Focus,
    /// The first row shown, as the list scrolls.
    top: usize,
}

impl TabState {
    fn hits(&self) -> &[Hit] {
        match &self.found {
            Some(Ok(hits)) => hits,
            _ => &[],
        }
    }

    /// Rows in the list: Automatic and the hits.
    fn rows(&self) -> usize {
        1 + self.hits().len()
    }

    fn focus_row(&mut self, row: usize) {
        let row = row.min(self.rows() - 1);
        self.focus = Focus::Row(row);
        if row < self.top {
            self.top = row;
        } else if row >= self.top + ROWS {
            self.top = row + 1 - ROWS;
        }
    }
}

pub struct MatchGame {
    game: GameKey,
    name: String,
    matches: Vec<Match>,
    tab: usize,
    tabs: Vec<TabState>,
}

/// Why a source can't be searched now, if it can't: art online off, the source off among Settings > Art's
/// sources, or SteamGridDB with no key.
fn unasked(source: ArtSource, around: &Around) -> Option<Failure> {
    let options = &around.options;
    if !options.art_online {
        Some(Failure::Offline)
    } else if !options.art_order.is_on(source) {
        Some(Failure::SourceOff)
    } else if source == ArtSource::SteamGridDb && !options.art_key {
        Some(Failure::NoKey)
    } else {
        None
    }
}

/// What the page says of a failure, plainly.
fn says(source: ArtSource, why: &Failure) -> String {
    let name = source.name();
    match why {
        Failure::Offline => "Turn on Download art online in Settings > Games & apps > Art to search the stores.".into(),
        Failure::SourceOff => format!("Turn on {name} in Settings > Games & apps > Art's sources to search it."),
        Failure::NoKey | Failure::KeyRefused => {
            "Enter your SteamGridDB API key in Settings > Games & apps > Art to search it.".into()
        }
        Failure::Busy => format!("{name} is busy. Press Y and search again in a minute."),
        Failure::NoGame => format!("{name} found nothing for these words. Press Y to search for others."),
        Failure::Failed(why) => format!("The search couldn't be done: {why}. Press Y to search again."),
    }
}

/// A failure in one word, for the focus log.
fn failure_word(why: &Failure) -> &'static str {
    match why {
        Failure::NoKey | Failure::KeyRefused => "no key",
        Failure::Offline => "offline",
        Failure::SourceOff => "source off",
        Failure::NoGame => "none",
        Failure::Busy => "busy",
        Failure::Failed(_) => "failed",
    }
}

/// The game with this key, hidden or not.
fn find(library: &Library, key: &GameKey) -> Option<Game> {
    library.games.iter().chain(&library.hidden).find(|game| game.is(key)).cloned()
}

impl MatchGame {
    /// The page for the game as the library has it now; None for one that is not in it. Each source's search is
    /// for the game's name to start with, and asked as its tab is first shown.
    pub fn open(entry: Entry, around: Around) -> Option<Self> {
        let game = find(around.library, &entry.game)?;
        let tabs = around
            .options
            .art_order
            .0
            .iter()
            .map(|(source, _)| TabState {
                source: *source,
                words: game.name.clone(),
                found: unasked(*source, &around).map(Err),
                asked: false,
                focus: Focus::Row(0),
                top: 0,
            })
            .collect();
        Some(Self { game: entry.game, name: game.name.clone(), matches: game.matches.clone(), tab: 0, tabs })
    }

    fn current(&self) -> &TabState {
        &self.tabs[self.tab]
    }

    fn current_mut(&mut self) -> &mut TabState {
        &mut self.tabs[self.tab]
    }

    fn matched(&self, source: ArtSource) -> Option<&Match> {
        self.matches.iter().find(|m| m.source == source)
    }

    /// What the shell is to fetch now: the shown source's search for its words, once.
    pub fn art_asks(&mut self) -> Vec<Ask> {
        let game = self.game.clone();
        let state = self.current_mut();
        if state.asked || state.found.is_some() {
            return Vec::new();
        }
        state.asked = true;
        vec![Ask::Search { game, source: state.source, words: state.words.clone() }]
    }

    /// What the shell fetched. A search for another game, or for words since changed, is passed over; an empty
    /// list is "found nothing".
    pub fn art_answer(&mut self, answer: Answer) {
        let Answer::Results { game, source, words, result } = answer else { return };
        if game != self.game {
            return;
        }
        let Some(state) = self.tabs.iter_mut().find(|s| s.source == source && s.words == words && s.asked) else {
            return;
        };
        state.found = Some(match result {
            Ok(hits) if hits.is_empty() => Err(Failure::NoGame),
            other => other,
        });
        if let Focus::Row(row) = state.focus {
            state.focus_row(row);
        }
    }

    /// A on a row: the game matched to that hit, or Automatic; the page closes either way.
    fn choose(&self, row: usize) -> Option<PageAction> {
        let state = self.current();
        let source = state.source;
        let matched = match row {
            0 => None,
            n => Some(state.hits().get(n - 1)?.on(source)),
        };
        if matched.as_ref() == self.matched(source) {
            return Some(PageAction::Close);
        }
        Some(PageAction::CloseWith(Effect::MatchGame { game: self.game.clone(), source, matched }))
    }

    fn row_rect(band: Rect, shown: usize) -> Rect {
        let y = band.y + LIST_TOP + shown as f32 * (ROW + ROW_GAP);
        Rect::new(LEFT, y, RIGHT - LEFT, ROW)
    }

    fn field_rect(band: Rect) -> Rect {
        Rect::new(LEFT, band.y + FIELD_TOP, RIGHT - LEFT, FIELD_HEIGHT)
    }

    fn draw_row(&self, ui: &mut Ui, rect: Rect, row: usize, focused: bool) {
        let state = self.current();
        ui.fill(rect, if focused { CELL_FOCUSED } else { CELL }, 2.0);
        let middle = rect.center().1;
        let matched = self.matched(state.source);
        let right = rect.right() - 24.0 - IN_USE - 16.0;
        let in_use = match row {
            0 => {
                ui.label_middle(rect.x + 24.0, middle, "Automatic", &theme::BODY_SEMIBOLD);
                let by_name = format!("Found by its name: {}", self.name);
                let words = Rect::new(rect.x + 240.0, rect.y, right - rect.x - 240.0, rect.h);
                ui.paragraph(words, &by_name, &SMALL.color(TEXT_DIM), HAlign::Left, VAlign::Center, 1);
                matched.is_none()
            }
            n => {
                let Some(hit) = state.hits().get(n - 1) else { return };
                let mut detail = format!("{} {}", state.source.name(), hit.id);
                if let Some(year) = hit.year {
                    detail = format!("{year}   {detail}");
                }
                ui.label_right(right, middle, &detail, &SMALL.color(TEXT_DIM));
                let name = Rect::new(rect.x + 24.0, rect.y, right - rect.x - 24.0 - 360.0, rect.h);
                ui.paragraph(name, &hit.name, &BODY, HAlign::Left, VAlign::Center, 1);
                matched.is_some_and(|m| m.id == hit.id)
            }
        };
        if in_use {
            let disc = Rect::new(rect.right() - 24.0 - IN_USE, middle - IN_USE / 2.0, IN_USE, IN_USE);
            ui.fill(disc, theme::ACCENT_LIGHT, IN_USE / 2.0);
            ui.icon(disc.center().0, disc.center().1, TICK, 20.0, Color::WHITE);
        }
    }
}

impl Page for MatchGame {
    fn name(&self) -> &'static str {
        NAME
    }

    /// The source and the focused row: `SteamGridDB: Search`, `Steam: Automatic`, `SteamGridDB: Result 2 (5263)`.
    fn focus(&self) -> String {
        let state = self.current();
        let at = match state.focus {
            Focus::Field => "Search".to_string(),
            Focus::Row(0) => "Automatic".to_string(),
            Focus::Row(n) => match state.hits().get(n - 1) {
                Some(hit) => format!("Result {n} ({})", hit.id),
                None => format!("Result {n}"),
            },
        };
        format!("{}: {at}", state.source.name())
    }

    /// The match in use on the source, and how its search stands: `matched: 5263; loading`, `matched: Automatic`.
    fn state(&self) -> String {
        let state = self.current();
        let mut line = match self.matched(state.source) {
            Some(m) => format!("matched: {}", m.id),
            None => "matched: Automatic".to_string(),
        };
        match &state.found {
            None => line.push_str("; loading"),
            Some(Err(why)) => line.push_str(&format!("; {}", failure_word(why))),
            Some(Ok(hits)) => line.push_str(&format!("; {} found", hits.len())),
        }
        line
    }

    fn dialog(&self) -> Option<Dialog> {
        Some(Dialog::Menu)
    }

    fn covers(&self) -> bool {
        false
    }

    fn art_asks(&mut self) -> Vec<Ask> {
        MatchGame::art_asks(self)
    }

    fn art_answer(&mut self, answer: Answer) {
        MatchGame::art_answer(self, answer);
    }

    fn is_art_picker(&self) -> bool {
        true
    }

    fn entry_text(&self) -> String {
        self.current().words.clone()
    }

    /// The words kept: searched for on the source shown, focus on what is found.
    fn entry(&mut self, event: keyboard::Event, around: Around) {
        let keyboard::Event::Kept(words) = event else { return };
        let words = words.trim().to_string();
        if words.is_empty() {
            return;
        }
        let state = self.current_mut();
        state.found = unasked(state.source, &around).map(Err);
        state.words = words;
        state.asked = false;
        state.top = 0;
        state.focus = Focus::Row(0);
    }

    fn press(&mut self, press: Press, _around: Around) -> Option<PageAction> {
        let focus = self.current().focus;
        match (press.button, focus) {
            (Button::B, _) if !press.repeat => return Some(PageAction::Close),
            (Button::Y, _) | (Button::A, Focus::Field) if !press.repeat => {
                self.current_mut().focus = Focus::Field;
                return Some(PageAction::Open(Open::TextEntry(WORDS)));
            }
            (Button::A, Focus::Row(row)) if !press.repeat => return self.choose(row),
            (Button::LeftBumper, _) if !press.repeat && self.tab > 0 => self.tab -= 1,
            (Button::RightBumper, _) if !press.repeat && self.tab + 1 < self.tabs.len() => self.tab += 1,
            (Button::Up, Focus::Row(0)) => self.current_mut().focus = Focus::Field,
            (Button::Up, Focus::Row(row)) => self.current_mut().focus_row(row - 1),
            (Button::Down, Focus::Field) => self.current_mut().focus_row(0),
            (Button::Down, Focus::Row(row)) => self.current_mut().focus_row(row + 1),
            _ => {}
        }
        None
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        ui.fill(ui.screen(), DIM, 0.0);
        let screen = ui.screen();
        let band = Rect::new(screen.x, BAND.0, screen.w, BAND.1);
        ui.fill(band, BAND_FILL, 0.0);

        // The sources.
        ui.fill(Rect::new(band.x, band.y, band.w, STRIP), STRIP_FILL, 0.0);
        for (at, state) in self.tabs.iter().enumerate() {
            let x = LEFT + at as f32 * TAB_WIDTH;
            let selected = at == self.tab;
            let style = if selected { theme::BODY_SEMIBOLD } else { BODY.color(TAB_DIM) };
            ui.label_centered(x + TAB_WIDTH / 2.0, band.y + STRIP / 2.0, state.source.name(), &style);
            if selected {
                ui.fill(Rect::new(x, band.y + STRIP - BAR, TAB_WIDTH, BAR), theme::ACCENT_LIGHT, 2.0);
            }
        }
        let tabs = self.tabs.len() as f32 * TAB_WIDTH + 40.0;
        let name = Rect::new(LEFT + tabs, band.y, RIGHT - LEFT - tabs, STRIP);
        ui.paragraph(name, &self.name, &BODY.color(TEXT_DIM), HAlign::Right, VAlign::Center, 1);

        let state = self.current();
        let source = state.source;
        let line = band.y + STRIP + LINE_MIDDLE;
        ui.label_middle(LEFT, line, &format!("Which of {}'s games is this?", source.name()), &BODY.color(TEXT_DIM));
        let matched = match self.matched(source) {
            Some(m) => format!("Matched: {}", m.name),
            None => "Matched: Automatic".to_string(),
        };
        let right = Rect::new(LEFT + 760.0, line - 20.0, RIGHT - LEFT - 760.0, 40.0);
        ui.paragraph(right, &matched, &BODY, HAlign::Right, VAlign::Center, 1);

        // The search field.
        let field = Self::field_rect(band);
        ui.fill(field, if state.focus == Focus::Field { CELL_FOCUSED } else { CELL }, 2.0);
        ui.icon(field.x + 24.0 + 12.0, field.center().1, SEARCH, 24.0, Color::WHITE);
        let words = Rect::new(field.x + 72.0, field.y, field.w - 96.0, field.h);
        ui.paragraph(words, &state.words, &BODY, HAlign::Left, VAlign::Center, 1);

        // The list: Automatic, then what was found, scrolled to the focused row.
        let rows = state.rows();
        for (shown, row) in (state.top..rows).take(ROWS).enumerate() {
            let focused = state.focus == Focus::Row(row);
            self.draw_row(ui, Self::row_rect(band, shown), row, focused);
        }
        if state.found.is_none() {
            let next = Self::row_rect(band, 1);
            spinner(ui, next.center().0, next.center().1, 40.0);
        }

        // The foot: what went wrong, plainly; else what to press.
        let foot = band.y + band.h - FOOT_MIDDLE;
        let said = match &state.found {
            Some(Err(why)) => says(source, why),
            _ => "LB RB  where to look      Y  search      A  match      B  leave it as it is".into(),
        };
        let words = Rect::new(LEFT, foot - 20.0, RIGHT - LEFT, 40.0);
        ui.paragraph(words, &said, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Center, 1);

        let ring = match state.focus {
            Focus::Field => Some(field),
            Focus::Row(row) if row >= state.top && row < state.top + ROWS => {
                Some(Self::row_rect(band, row - state.top))
            }
            Focus::Row(_) => None,
        };
        if let Some(ring) = ring {
            theme::focus_ring_cornered(ui, ring, 12.0);
        }
    }
}

#[cfg(test)]
mod tests;
