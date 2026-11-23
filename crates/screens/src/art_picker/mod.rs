//! The art picker (docs/spec/art-picker.md, #213): a game's pictures chosen from SteamGridDB's candidates, a tab
//! per kind the Dashboard draws (`model::art_picker::Tab`), over the dimmed page that opened it. The Tile menu's
//! More options opens it ("Change art"), and so the Game hub's Manage, which is that menu.
//!
//! Each tab is a grid of whole pages, the cells at the proportions the picture is shown at: first "Automatic" (the
//! Dashboard's own pick) and the store's own picture where its client keeps one, then the candidates in
//! SteamGridDB's order, a page of them fetched as the grid's page needs it ([`Pager`]). LB and RB change tab, LT and
//! RT the page (or Left and Right past the grid's edge), A chooses, B leaves the picture as it was.
//!
//! The page asks the shell for what it needs ([`ArtPicker::art_asks`]: a page of the list, a thumbnail) and is told
//! what came ([`ArtPicker::art_answer`]); it fetches nothing itself.
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use model::art_picker::{Answer, Ask, Cell, Choice, Chosen, Failure, GameArt, Pager, Tab};
use model::{Game, GameKey, Library};
use ui::{ArtKey, Button, Color, Crop, Font, HAlign, Kind, Press, Rect, TextStyle, Ui, VAlign};

use crate::home;
use crate::menu_panel::DIM;
use crate::pages::{Around, Dialog, Open, Page, PageAction};
use crate::spinner::spinner;
use crate::theme::{self, BODY, TEXT_DIM};
use crate::Effect;

/// The band: Power's (docs/spec/guide.md, 6.2), the screen's whole width, y 134 to 942, `#2D2D2D`.
const BAND: (f32, f32) = (134.0, 808.0);
const BAND_FILL: Color = Color::rgb(0x2D2D2D);
/// The contents, as Power's: x 132 to 1691.
const LEFT: f32 = 132.0;
const RIGHT: f32 = 1691.0;
/// The tab strip across the band's top, the Guide's strip colour (guide.md, 2.2): 72 high.
const STRIP: f32 = 72.0;
const STRIP_FILL: Color = Color::rgb(0x1F1F1F);
/// A tab: 200 wide, its label centred; the selected one's bar, the Guide's, 6 high at the strip's foot.
const TAB_WIDTH: f32 = 200.0;
const TAB_DIM: Color = Color::rgb(0x8C8C8C);
const BAR: f32 = 6.0;
/// The line saying where the picture shows, its middle 38 under the strip; the page's count right of it.
const LINE_MIDDLE: f32 = 38.0;
/// The grid: from 86 under the strip, 568 high, over the contents' width; cells 16 apart.
const GRID_TOP: f32 = STRIP + 86.0;
const GRID_HEIGHT: f32 = 568.0;
const GAP: f32 = 16.0;
const CELL: Color = Color::rgb(0x3B3B3B);
const CELL_FOCUSED: Color = Color::rgb(0x4A4A4A);
/// The foot: a line of what to press, or what went wrong, its middle 44 above the band's foot; Retry right of it.
const FOOT_MIDDLE: f32 = 44.0;
const RETRY: (f32, f32) = (200.0, 52.0);
const RETRY_LABEL: TextStyle = TextStyle::new(Font::Ui, 26.0, 700);
/// The mark on the picture in use: a green disc 36 across, 10 in from the cell's top right corner, a tick on it.
const IN_USE: f32 = 36.0;
const TICK: char = '\u{E73E}';
/// The fixed entries' captions.
const SMALL: TextStyle = TextStyle::new(Font::Ui, 20.0, 400);

/// How the picker is opened: for which game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    game: GameKey,
}

/// The picker for `game`, as a page asks for it.
pub fn open(game: &Game) -> Open {
    Open::ArtPicker(Entry { game: game.key() })
}

/// One of the entries before the candidates.
#[derive(Debug, Clone, PartialEq)]
enum Fixed {
    Automatic,
    /// The store's own picture, from its files on the PC.
    Store(PathBuf),
}

/// Where focus is on a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// A cell of the page shown, from its top left along the rows.
    Cell(usize),
    Retry,
}

struct TabState {
    fixed: Vec<Fixed>,
    pager: Pager,
    /// The grid's page shown, from 0.
    page: usize,
    focus: Focus,
    /// The thumbnails that came, by candidate: None for one that could not be had.
    thumbs: HashMap<u64, Option<PathBuf>>,
    thumbs_asked: HashSet<u64>,
}

impl TabState {
    fn new(fixed: Vec<Fixed>, tab: Tab, service: Result<(), Failure>) -> Self {
        let pager = match service {
            Ok(()) => Pager::new(fixed.len(), tab.per_page()),
            Err(why) => Pager::without_service(fixed.len(), tab.per_page(), why),
        };
        Self { fixed, pager, page: 0, focus: Focus::Cell(0), thumbs: HashMap::new(), thumbs_asked: HashSet::new() }
    }

    fn cells(&self) -> usize {
        self.pager.cells(self.page).len()
    }

    /// Retry is there when a page failed that can be asked again, or the list came empty.
    fn retry_shown(&self) -> bool {
        let failed = matches!(self.pager.failure(), Some(Failure::Busy | Failure::Failed(_)));
        failed || self.empty()
    }

    fn empty(&self) -> bool {
        self.pager.failure().is_none() && self.pager.total() == Some(0)
    }
}

pub struct ArtPicker {
    game: GameKey,
    name: String,
    /// The game's id at SteamGridDB where the user matched it there (docs/spec/match-game.md).
    matched: Option<u64>,
    art: GameArt,
    tab: Tab,
    tabs: Vec<TabState>,
}

impl ArtPicker {
    /// The picker for the game as the library has it now; None for one that is not in it. Without art online or a
    /// SteamGridDB key it asks for nothing and says where to set them; its fixed entries can still be chosen.
    pub fn open(entry: Entry, around: Around) -> Option<Self> {
        let game = find(around.library, &entry.game)?;
        let service = if !around.options.art_online {
            Err(Failure::Offline)
        } else if !around.options.art_order.is_on(model::ArtSource::SteamGridDb) {
            Err(Failure::SourceOff)
        } else if !around.options.art_key {
            Err(Failure::NoKey)
        } else {
            Ok(())
        };
        let tabs = Tab::ALL
            .into_iter()
            .map(|tab| {
                let mut fixed = vec![Fixed::Automatic];
                if let Some(store) = game.art.store(tab) {
                    fixed.push(Fixed::Store(store.clone()));
                }
                TabState::new(fixed, tab, service.clone())
            })
            .collect();
        let matched = game.matched(model::ArtSource::SteamGridDb).and_then(|m| m.id.parse().ok());
        Some(Self { game: entry.game, name: game.name.clone(), matched, art: game.art.clone(), tab: Tab::Square, tabs })
    }

    fn current(&self) -> &TabState {
        &self.tabs[self.tab.index()]
    }

    fn current_mut(&mut self) -> &mut TabState {
        &mut self.tabs[self.tab.index()]
    }

    /// What the shell is to fetch now: the next page of the list the tab's page shown needs, and the thumbnails of
    /// its candidates not asked for yet. Each is asked once.
    pub fn art_asks(&mut self) -> Vec<Ask> {
        let (game, name, matched, tab) = (self.game.clone(), self.name.clone(), self.matched, self.tab);
        let state = self.current_mut();
        let mut asks = Vec::new();
        if let Some(page) = state.pager.want(state.page) {
            state.pager.asking(page);
            asks.push(Ask::Page { game, name, matched, tab, page });
        }
        let wanted: Vec<(u64, String)> = state
            .pager
            .cells(state.page)
            .into_iter()
            .filter_map(|cell| match cell {
                Cell::Candidate(c) if !state.thumbs_asked.contains(&c.id) => Some((c.id, c.thumb.clone())),
                _ => None,
            })
            .collect();
        for (id, url) in wanted {
            state.thumbs_asked.insert(id);
            asks.push(Ask::Thumb { id, url });
        }
        asks
    }

    /// What the shell fetched. An answer for another game is passed over.
    pub fn art_answer(&mut self, answer: Answer) {
        match answer {
            Answer::Page { game, tab, page, result } if game == self.game => {
                let state = &mut self.tabs[tab.index()];
                match result {
                    Ok(reply) => state.pager.took(reply),
                    Err(why) => state.pager.fail(page, why),
                }
                // Focus stays where it was, or on the last cell there is.
                let cells = state.cells();
                if let Focus::Cell(at) = state.focus {
                    if at >= cells {
                        state.focus = Focus::Cell(cells.saturating_sub(1));
                    }
                }
                if state.focus == Focus::Retry && !state.retry_shown() {
                    state.focus = Focus::Cell(0);
                }
            }
            Answer::Thumb { id, path } => {
                for state in &mut self.tabs {
                    if state.thumbs_asked.contains(&id) {
                        state.thumbs.insert(id, path.clone());
                    }
                }
            }
            Answer::Page { .. } | Answer::Results { .. } => {}
        }
    }

    /// What the focused cell is, as the focus log says it.
    fn cell_name(&self, state: &TabState) -> String {
        match state.focus {
            Focus::Retry => "Retry".into(),
            Focus::Cell(at) => match state.pager.cells(state.page).get(at) {
                Some(Cell::Fixed(i)) => match state.fixed.get(*i) {
                    Some(Fixed::Automatic) => "Automatic".into(),
                    Some(Fixed::Store(_)) => "Store".into(),
                    None => String::new(),
                },
                Some(Cell::Candidate(c)) => format!("Picture {}", c.id),
                Some(Cell::Coming) => format!("Coming {at}"),
                None => String::new(),
            },
        }
    }

    /// What is in use on the tab, as the focus log says it.
    fn in_use(&self, tab: Tab) -> String {
        match self.art.chosen(tab) {
            None => "Automatic".into(),
            Some(Chosen::Store) => "Store".into(),
            Some(Chosen::Picture(id)) => format!("Picture {id}"),
        }
    }

    /// The tab's page as the screen says it: "Page 2 of 12", or "Page 1" before the count is known.
    fn page_line(state: &TabState) -> String {
        if state.pager.counted() {
            format!("Page {} of {}", state.page + 1, state.pager.pages())
        } else {
            format!("Page {}", state.page + 1)
        }
    }

    /// To another page of the grid, focus on the same cell where the page has it, else its last.
    fn go_to_page(&mut self, page: usize, cell: Option<usize>) -> bool {
        let state = self.current_mut();
        if page == state.page || page >= state.pager.pages() {
            return false;
        }
        let at = match (cell, state.focus) {
            (Some(at), _) | (None, Focus::Cell(at)) => at,
            (None, Focus::Retry) => 0,
        };
        state.page = page;
        let cells = state.cells();
        state.focus = Focus::Cell(at.min(cells.saturating_sub(1)));
        true
    }

    /// A on the focused cell.
    fn choose(&mut self) -> Option<PageAction> {
        let tab = self.tab;
        let state = self.current();
        let Focus::Cell(at) = state.focus else {
            let state = self.current_mut();
            if state.empty() {
                // Asked again from the start: the list may have come since.
                let fixed = state.fixed.clone();
                *state = TabState::new(fixed, tab, Ok(()));
                state.focus = Focus::Retry;
            } else {
                state.pager.retry();
            }
            if !self.current().retry_shown() {
                self.current_mut().focus = Focus::Cell(0);
            }
            return None;
        };
        let (chosen, url) = match state.pager.cells(state.page).get(at)? {
            Cell::Fixed(i) => match state.fixed.get(*i)? {
                Fixed::Automatic => (None, None),
                Fixed::Store(_) => (Some(Chosen::Store), None),
            },
            Cell::Candidate(c) => (Some(Chosen::Picture(c.id)), Some(c.url.clone())),
            Cell::Coming => return None,
        };
        Some(PageAction::CloseWith(Effect::ChooseArt(Choice { game: self.game.clone(), tab, chosen, url })))
    }

    /// Where each cell of the tab's grid is.
    fn layout(tab: Tab, band: Rect) -> Vec<Rect> {
        let (rows, columns) = tab.grid();
        let width = RIGHT - LEFT;
        let mut h = (GRID_HEIGHT - GAP * (rows - 1) as f32) / rows as f32;
        let mut w = h * tab.aspect();
        if w * columns as f32 + GAP * (columns - 1) as f32 > width {
            w = (width - GAP * (columns - 1) as f32) / columns as f32;
            h = w / tab.aspect();
        }
        let across = w * columns as f32 + GAP * (columns - 1) as f32;
        let left = (LEFT + (width - across) / 2.0).round();
        let top = band.y + GRID_TOP;
        (0..rows * columns)
            .map(|at| {
                let (row, column) = ((at / columns) as f32, (at % columns) as f32);
                Rect::new((left + column * (w + GAP)).round(), (top + row * (h + GAP)).round(), w.round(), h.round())
            })
            .collect()
    }

    fn retry_rect(band: Rect) -> Rect {
        let middle = band.y + band.h - FOOT_MIDDLE;
        Rect::new(RIGHT - RETRY.0, middle - RETRY.1 / 2.0, RETRY.0, RETRY.1)
    }

    fn draw_cell(&self, ui: &mut Ui, state: &TabState, rect: Rect, cell: Cell, focused: bool) {
        let tab = self.tab;
        ui.fill(rect, if focused { CELL_FOCUSED } else { CELL }, 2.0);
        let in_use = match cell {
            Cell::Fixed(i) => match &state.fixed[i] {
                Fixed::Automatic => {
                    let (cx, cy) = rect.center();
                    ui.label_centered(cx, cy - 12.0, "Automatic", &BODY);
                    ui.label_centered(cx, cy + 20.0, "The Dashboard's pick", &SMALL.color(TEXT_DIM));
                    self.art.chosen(tab).is_none()
                }
                Fixed::Store(path) => {
                    picture(ui, tab, rect, path);
                    let band = Rect::new(rect.x, rect.y + rect.h - 40.0, rect.w, 40.0);
                    ui.fill(band, Color::argb(0xCC000000), 0.0);
                    ui.paragraph(band, "The store's own", &SMALL, HAlign::Center, VAlign::Center, 1);
                    self.art.chosen(tab) == Some(&Chosen::Store)
                }
            },
            Cell::Candidate(c) => {
                match state.thumbs.get(&c.id) {
                    Some(Some(path)) => picture(ui, tab, rect, path),
                    Some(None) => {
                        let size = format!("{} x {}", c.width, c.height);
                        ui.label_centered(rect.center().0, rect.center().1, &size, &SMALL.color(TEXT_DIM));
                    }
                    None => spinner(ui, rect.center().0, rect.center().1, 40.0),
                }
                self.art.chosen(tab) == Some(&Chosen::Picture(c.id))
            }
            Cell::Coming => {
                spinner(ui, rect.center().0, rect.center().1, 40.0);
                false
            }
        };
        if in_use {
            let disc = Rect::new(rect.x + rect.w - 10.0 - IN_USE, rect.y + 10.0, IN_USE, IN_USE);
            ui.fill(disc, theme::ACCENT_LIGHT, IN_USE / 2.0);
            ui.icon(disc.center().0, disc.center().1, TICK, 20.0, Color::WHITE);
        }
    }
}

/// A picture in a cell at the kind's proportions: cropped from the top as tiles and backgrounds are, and on a
/// square cell shown whole when it is not square (docs/spec/home.md 3.3).
fn picture(ui: &mut Ui, tab: Tab, rect: Rect, path: &std::path::Path) {
    let width = if tab == Tab::Background { 600 } else { 300 };
    let key = ArtKey { path: path.to_path_buf(), kind: Kind::Thumb(width) };
    match tab {
        Tab::Square => {
            home::face(ui, rect, &key, 1.0, 2.0);
        }
        _ => {
            ui.art(rect, &key, Crop::Top, 1.0, 2.0);
        }
    }
}

/// The game with this key, hidden or not.
fn find(library: &Library, key: &GameKey) -> Option<Game> {
    library.games.iter().chain(&library.hidden).find(|game| game.is(key)).cloned()
}

impl Page for ArtPicker {
    fn name(&self) -> &'static str {
        "ArtPicker"
    }

    /// The tab, its page and the focused cell: `Square: Page 2 of 6: Picture 40003`, `Cover: Page 1: Automatic`.
    fn focus(&self) -> String {
        let state = self.current();
        format!("{}: {}: {}", self.tab.label(), Self::page_line(state), self.cell_name(state))
    }

    /// What is in use on the tab, and whether a page is on its way or failed: `in use: Picture 40003; loading`.
    fn state(&self) -> String {
        let state = self.current();
        let mut line = format!("in use: {}", self.in_use(self.tab));
        if state.pager.waiting() {
            line.push_str("; loading");
        } else if let Some(why) = state.pager.failure() {
            line.push_str(&format!("; {}", failure_word(why)));
        } else if state.empty() {
            line.push_str("; none");
        }
        line
    }

    fn dialog(&self) -> Option<Dialog> {
        Some(Dialog::Menu)
    }

    fn art_asks(&mut self) -> Vec<Ask> {
        ArtPicker::art_asks(self)
    }

    fn art_answer(&mut self, answer: Answer) {
        ArtPicker::art_answer(self, answer);
    }

    fn is_art_picker(&self) -> bool {
        true
    }

    fn covers(&self) -> bool {
        false
    }

    fn press(&mut self, press: Press, _around: Around) -> Option<PageAction> {
        let (rows, columns) = self.tab.grid();
        let state = self.current();
        let (page, pages, cells, focus) = (state.page, state.pager.pages(), state.cells(), state.focus);
        let retry = state.retry_shown();
        match (press.button, focus) {
            (Button::B, _) if !press.repeat => return Some(PageAction::Close),
            (Button::A, _) if !press.repeat => return self.choose(),
            (Button::LeftBumper | Button::RightBumper, _) if !press.repeat => {
                let at = self.tab.index();
                let to = if press.button == Button::LeftBumper { at.checked_sub(1) } else { Some(at + 1) };
                if let Some(tab) = to.and_then(|to| Tab::ALL.get(to)) {
                    self.tab = *tab;
                }
            }
            (Button::LeftTrigger, _) if page > 0 => {
                self.go_to_page(page - 1, None);
            }
            (Button::RightTrigger, _) if page + 1 < pages => {
                self.go_to_page(page + 1, None);
            }
            (Button::Left, Focus::Cell(at)) if at % columns > 0 => self.current_mut().focus = Focus::Cell(at - 1),
            // Past the grid's left edge: the page before, on the same row's last cell.
            (Button::Left, Focus::Cell(at)) if page > 0 => {
                self.go_to_page(page - 1, Some(at - at % columns + columns - 1));
            }
            (Button::Right, Focus::Cell(at)) if at % columns + 1 < columns && at + 1 < cells => {
                self.current_mut().focus = Focus::Cell(at + 1);
            }
            // Past the grid's right edge, or its last cell: the next page, on the same row's first cell.
            (Button::Right, Focus::Cell(at)) if page + 1 < pages => {
                self.go_to_page(page + 1, Some(at - at % columns));
            }
            (Button::Up, Focus::Cell(at)) if at >= columns => self.current_mut().focus = Focus::Cell(at - columns),
            (Button::Up, Focus::Retry) => self.current_mut().focus = Focus::Cell(cells.saturating_sub(1)),
            (Button::Down, Focus::Cell(at)) if at + columns < cells => {
                self.current_mut().focus = Focus::Cell(at + columns);
            }
            (Button::Down, Focus::Cell(at)) if at / columns + 1 < rows && at / columns < (cells - 1) / columns => {
                self.current_mut().focus = Focus::Cell(cells - 1);
            }
            (Button::Down, Focus::Cell(_)) if retry => self.current_mut().focus = Focus::Retry,
            _ => {}
        }
        None
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        ui.fill(ui.screen(), DIM, 0.0);
        let screen = ui.screen();
        let band = Rect::new(screen.x, BAND.0, screen.w, BAND.1);
        ui.fill(band, BAND_FILL, 0.0);

        // The tabs.
        ui.fill(Rect::new(band.x, band.y, band.w, STRIP), STRIP_FILL, 0.0);
        for (at, tab) in Tab::ALL.into_iter().enumerate() {
            let x = LEFT + at as f32 * TAB_WIDTH;
            let selected = tab == self.tab;
            let style = if selected { theme::BODY_SEMIBOLD } else { BODY.color(TAB_DIM) };
            ui.label_centered(x + TAB_WIDTH / 2.0, band.y + STRIP / 2.0, tab.label(), &style);
            if selected {
                ui.fill(Rect::new(x, band.y + STRIP - BAR, TAB_WIDTH, BAR), theme::ACCENT_LIGHT, 2.0);
            }
        }
        let name = Rect::new(LEFT + 3.0 * TAB_WIDTH + 40.0, band.y, RIGHT - LEFT - 3.0 * TAB_WIDTH - 40.0, STRIP);
        ui.paragraph(name, &self.name, &BODY.color(TEXT_DIM), HAlign::Right, VAlign::Center, 1);

        let state = self.current();
        let line = band.y + STRIP + LINE_MIDDLE;
        ui.label_middle(LEFT, line, self.tab.shows(), &BODY.color(TEXT_DIM));
        let mut count = Self::page_line(state);
        if let Some(total) = state.pager.total().filter(|_| state.pager.failure().is_none()) {
            count.push_str(&format!(" \u{00B7} {total} from SteamGridDB"));
        }
        ui.label_right(RIGHT, line, &count, &BODY);

        let rects = Self::layout(self.tab, band);
        let cells = state.pager.cells(state.page);
        for (at, cell) in cells.iter().enumerate() {
            let focused = state.focus == Focus::Cell(at);
            self.draw_cell(ui, state, rects[at], *cell, focused);
        }
        // The first page coming, its count not yet known: the Spinner where the candidates will be.
        if state.pager.waiting() && !state.pager.counted() {
            let next = rects[cells.len().min(rects.len() - 1)];
            spinner(ui, next.center().0, next.center().1, 48.0);
        }

        // The foot: what went wrong, plainly, with Retry; else what to press.
        let foot = band.y + band.h - FOOT_MIDDLE;
        let said = match state.pager.failure() {
            Some(why) => why.says(),
            None if state.empty() => "SteamGridDB has no pictures of this kind for this game.".into(),
            None => "LB RB  kind of picture      LT RT  page      A  choose      B  leave it as it is".into(),
        };
        let words = Rect::new(LEFT, foot - 20.0, RIGHT - LEFT - RETRY.0 - 24.0, 40.0);
        ui.paragraph(words, &said, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Center, 1);
        if state.retry_shown() {
            let retry = Self::retry_rect(band);
            ui.fill(retry, if state.focus == Focus::Retry { CELL_FOCUSED } else { CELL }, 2.0);
            ui.label_centered(retry.center().0, retry.center().1, "Retry", &RETRY_LABEL);
        }

        // The ring last, over the cells beside it.
        let ring = match state.focus {
            Focus::Cell(at) if at < cells.len() => Some(rects[at]),
            Focus::Retry => Some(Self::retry_rect(band)),
            Focus::Cell(_) => None,
        };
        if let Some(ring) = ring {
            theme::focus_ring_cornered(ui, ring, 12.0);
        }
    }
}

/// A failure in one word, for the focus log.
fn failure_word(why: &Failure) -> &'static str {
    match why {
        Failure::NoKey | Failure::KeyRefused => "no key",
        Failure::Offline => "offline",
        Failure::SourceOff => "steamgriddb off",
        Failure::NoGame => "no game",
        Failure::Busy => "busy",
        Failure::Failed(_) => "failed",
    }
}

#[cfg(test)]
mod tests;
