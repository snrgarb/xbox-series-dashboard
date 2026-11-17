//! Manage Queue (docs/spec/manage-queue.md): the page over the whole screen, the nav of My games & apps hidden, that
//! lists the Queue. Under its title, Recently installed's covers, then the Queue's rows under "Installing", the
//! current download first and those next up after it, each with its speed, how much is down of the whole and the
//! share; or, with nothing queued, an outlined box saying where they will show.
//!
//! It is opened three ways, each over what asked for it, so B is always a plain close that finds what is under it
//! as it was: Manage's Queue box (My games & apps, focus on the box), Home's Downloads tile (Home, on the tile), and
//! a Game hub's Queue action (the hub, on Queue).
//!
//! Its covers and rows are a game's as a Home tile is: A opens the game's Game hub at once, where a second A is its
//! Play; Menu and View open its Tile menu, which on a row is the queued game's menu,
//! with Pause or Resume and Cancel where its store can (`tile_menu::Opener::Queue`, #170).
pub mod figures;

use model::{Game, Library};
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use self::figures::Figures;
use crate::hint_bar::{self, Hint};
use crate::my_games::look::{BOX, HEADER, LINE, PAGE, SECTION, TEXT};
use crate::pages::{Around, Open, Page, PageAction};
use crate::theme::{self, glyph};
use crate::{game_hub, home, tile_menu};

/// What opens the page, from wherever: the hook Manage's Queue box, Home's Downloads tile and the hub's Queue action
/// all call.
pub fn open() -> Open {
    Open::ManageQueue
}

/// How many covers Recently installed holds.
pub const RECENTLY_INSTALLED: usize = 8;

/// The title's top left; the list starts under its line (Bahnschrift 42, 1.2 to the line).
const TITLE: (f32, f32) = (96.0, 72.0);
const LIST_TOP: f32 = 72.0 + 42.0 * 1.2;
/// The list's content, 24 inside the scroller's own edges (x 72..1848), which is what it is cut to.
const LEFT: f32 = 96.0;
const RIGHT: f32 = 1824.0;
const CLIP: (f32, f32) = (72.0, 1848.0);
/// A section's header (Bahnschrift 30, a 36 line), with what stands above and below it: Recently installed's,
/// and Installing's.
const SECTION_LINE: f32 = 36.0;
const RECENT_HEADER: (f32, f32) = (14.0, 14.0);
const INSTALLING_HEADER: (f32, f32) = (30.0, 30.0);
/// The covers: Home's tiles, 200 square and 16 apart, with 24 round them for their rings.
const COVER: f32 = 200.0;
const COVER_PITCH: f32 = 216.0;
const COVER_MARGIN: f32 = 24.0;
/// A row: 250 tall, 16 to the next; its cover the first 250 across; its text 36 right of that, 40 short of its end
/// and 38 down; the title 160 short of the speed's end.
const ROW: f32 = 250.0;
const ROW_PITCH: f32 = 266.0;
const ROW_TEXT: (f32, f32, f32) = (36.0, 40.0, 38.0);
const TITLE_SHORT: f32 = 160.0;
/// The amount's line is 34 under the state's; the bar 12 under it, 5 tall.
const AMOUNT_DOWN: f32 = 34.0;
const BAR_DOWN: f32 = 12.0;
const BAR: f32 = 5.0;
const TRACK: Color = Color::rgb(0x4A4A4A);
/// With nothing queued: the outlined box, 40 under what is above it, 330 tall; its glyph 64, the line 40 under it.
const EMPTY_DOWN: f32 = 40.0;
const EMPTY: f32 = 330.0;
const EMPTY_OUTLINE: Color = Color::rgb(0x5C5C5C);
const EMPTY_GLYPH: f32 = 64.0;
const EMPTY_GAP: f32 = 40.0;
pub const EMPTY_TEXT: &str = "When games and apps are installing, you'll see them here";
/// Under the list's end.
const END_PADDING: f32 = 80.0;
/// A row's title: Segoe UI Semibold, bold.
const ROW_TITLE: ui::TextStyle = TEXT.weight(700);

/// Manage Queue's Recently installed: the games the Queue saw finish installing, the latest first, then the rest
/// of those installed, the latest installed first by their install folders' dates, or by when they came into the
/// library where that is not known (the Bundle's `InstalledAt ?? Added`); none that is hidden or no longer
/// installed; at most [`RECENTLY_INSTALLED`].
pub fn recently_installed(library: &Library) -> Vec<&Game> {
    let shown = |game: &&Game| game.installed && !game.hidden;
    let finished: Vec<&Game> =
        library.queue.installed.iter().filter_map(|done| library.game(done.game)).filter(shown).collect();
    let mut rest: Vec<&Game> =
        library.games.iter().filter(shown).filter(|game| !finished.iter().any(|done| done.id == game.id)).collect();
    let when = |game: &Game| game.installed_at.unwrap_or(game.added);
    rest.sort_by(|a, b| when(b).cmp(&when(a)).then_with(|| a.name.cmp(&b.name)));
    finished.into_iter().chain(rest).take(RECENTLY_INSTALLED).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// Recently installed's cover.
    Cover(usize),
    /// The Queue's row.
    Row(usize),
    /// Nothing to focus: no cover and no row.
    Nothing,
}

/// Where everything stands down the list, from its top, for so many covers and rows.
struct Layout {
    covers: Option<f32>,
    rows: Option<f32>,
    empty: Option<f32>,
    headers: Vec<(f32, &'static str)>,
    height: f32,
}

impl Layout {
    fn of(covers: usize, rows: usize) -> Self {
        let mut y = 0.0;
        let mut headers = Vec::new();
        let mut layout = Layout { covers: None, rows: None, empty: None, headers: Vec::new(), height: 0.0 };
        if covers > 0 {
            headers.push((y + RECENT_HEADER.0, "Recently installed"));
            y += RECENT_HEADER.0 + SECTION_LINE + RECENT_HEADER.1;
            layout.covers = Some(y + COVER_MARGIN);
            y += 2.0 * COVER_MARGIN + COVER;
        }
        if rows > 0 {
            headers.push((y + INSTALLING_HEADER.0, "Installing"));
            y += INSTALLING_HEADER.0 + SECTION_LINE + INSTALLING_HEADER.1;
            layout.rows = Some(y);
            y += rows as f32 * ROW_PITCH;
        } else {
            layout.empty = Some(y + EMPTY_DOWN);
            y += EMPTY_DOWN + EMPTY;
        }
        layout.headers = headers;
        layout.height = y + END_PADDING;
        layout
    }

    fn row(&self, i: usize) -> Option<f32> {
        self.rows.map(|top| top + i as f32 * ROW_PITCH)
    }
}

pub struct ManageQueue {
    focus: Focus,
    /// The game focus is on, or last was: where it goes back to as the lists change.
    game: Option<i64>,
    /// The cover last focused: where Up from the first row goes.
    last_cover: usize,
    scroll: f32,
    /// What the focus log says, kept as the library was when last looked at: the focused game's name, how many
    /// rows and covers there are, and the focused row's state and share.
    name: String,
    counts: (usize, usize),
    row: Option<String>,
}

impl ManageQueue {
    pub fn open(around: Around) -> Self {
        let mut page = Self {
            focus: Focus::Nothing,
            game: None,
            last_cover: 0,
            scroll: 0.0,
            name: String::new(),
            counts: (0, 0),
            row: None,
        };
        page.keep(around.library);
        page
    }

    /// The library may have changed: focus stays on its game, as a cover or a row, whichever it now is; else the
    /// first cover, else the first row, else nothing.
    fn keep(&mut self, library: &Library) {
        let covers = recently_installed(library);
        let rows = &library.queue.entries;
        let row_of = |game: i64| rows.iter().position(|entry| entry.game == game);
        let cover_of = |game: i64| covers.iter().position(|cover| cover.id == game);
        let still = match self.focus {
            Focus::Row(i) => rows.get(i).filter(|entry| Some(entry.game) == self.game).map(|_| Focus::Row(i)),
            Focus::Cover(i) => covers.get(i).filter(|cover| Some(cover.id) == self.game).map(|_| Focus::Cover(i)),
            Focus::Nothing => None,
        };
        self.focus = still
            .or_else(|| self.game.and_then(row_of).map(Focus::Row))
            .or_else(|| self.game.and_then(cover_of).map(Focus::Cover))
            .or_else(|| (!covers.is_empty()).then_some(Focus::Cover(0)))
            .or_else(|| (!rows.is_empty()).then_some(Focus::Row(0)))
            .unwrap_or(Focus::Nothing);
        let game = match self.focus {
            Focus::Cover(i) => Some(covers[i].id),
            Focus::Row(i) => Some(rows[i].game),
            Focus::Nothing => None,
        };
        if let Focus::Cover(i) = self.focus {
            self.last_cover = i;
        }
        self.game = game.or(self.game);
        self.name = game.and_then(|game| library.game(game)).map(|game| game.name.clone()).unwrap_or_default();
        self.counts = (rows.len(), covers.len());
        self.row = match self.focus {
            Focus::Row(i) => {
                // Its state, a Workshop download's items, and its share: `Downloading Workshop content 3 of 12
                // items 25.0%`.
                let figures = Figures::of(&rows[i]);
                let words = [Some(figures.state.to_string()), figures.items, figures.percent];
                Some(words.into_iter().flatten().collect::<Vec<_>>().join(" "))
            }
            _ => None,
        };
    }

    /// The game focus is on, by its place in the library.
    fn place(&self, library: &Library) -> Option<usize> {
        if self.focus == Focus::Nothing {
            return None;
        }
        let game = self.game?;
        library.games.iter().position(|g| g.id == game)
    }

    /// How far the list is scrolled, from `scroll`, to show what has focus: a cover brings the list back to its
    /// top; a row comes into the view with the least movement, as WPF brings a focused element into view.
    fn scroll_to(&self, scroll: f32, layout: &Layout, view: f32) -> f32 {
        let scroll = match self.focus {
            Focus::Cover(_) | Focus::Nothing => 0.0,
            Focus::Row(i) => {
                let top = layout.row(i).unwrap_or(0.0);
                if top < scroll {
                    top
                } else {
                    scroll.max(top + ROW - view)
                }
            }
        };
        scroll.clamp(0.0, (layout.height - view).max(0.0))
    }
}

impl Page for ManageQueue {
    fn name(&self) -> &'static str {
        "ManageQueue"
    }

    /// `Cover(0) of Quiet Orbit`, `Row(1) of Saltwind Rally`, or `Nothing` with neither covers nor rows.
    fn focus(&self) -> String {
        match self.focus {
            Focus::Cover(i) => format!("Cover({i}) of {}", self.name),
            Focus::Row(i) => format!("Row({i}) of {}", self.name),
            Focus::Nothing => "Nothing".into(),
        }
    }

    /// How many games are installing and how many covers Recently installed has, then with focus on a row what
    /// its state and share read: `1 installing, 3 recently installed; Installing... 25.0%`. Its speed is not
    /// said: it changes every second.
    fn state(&self) -> String {
        let counts = format!("{} installing, {} recently installed", self.counts.0, self.counts.1);
        match &self.row {
            Some(row) => format!("{counts}; {row}"),
            None => counts,
        }
    }

    fn library_changed(&mut self, around: Around) {
        self.keep(around.library);
    }

    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        let library = around.library;
        self.keep(library);
        let once = !press.repeat;
        let covers = self.counts.1;
        let rows = library.queue.entries.len();
        match (press.button, self.focus) {
            (Button::B, _) if once => return Some(PageAction::Close),
            // Along the covers, stopping at the ends; down to the first row.
            (Button::Left, Focus::Cover(i)) if i > 0 => self.focus = Focus::Cover(i - 1),
            (Button::Right, Focus::Cover(i)) if i + 1 < covers => self.focus = Focus::Cover(i + 1),
            (Button::Down, Focus::Cover(_)) if rows > 0 => self.focus = Focus::Row(0),
            // Down the rows, stopping at the last; up from the first to the cover last focused.
            (Button::Down, Focus::Row(i)) if i + 1 < rows => self.focus = Focus::Row(i + 1),
            (Button::Up, Focus::Row(i)) if i > 0 => self.focus = Focus::Row(i - 1),
            (Button::Up, Focus::Row(0)) if covers > 0 => self.focus = Focus::Cover(self.last_cover.min(covers - 1)),
            // The game's Game hub, at once, as a Home tile's.
            (Button::A, Focus::Cover(_) | Focus::Row(_)) if once => {
                let place = self.place(library)?;
                return Some(PageAction::Open(Open::GameHub(game_hub::Entry::of(&library.games[place]))));
            }
            (Button::Menu | Button::View, Focus::Cover(_) | Focus::Row(_)) if once => {
                let place = self.place(library)?;
                let opener = match self.focus {
                    Focus::Row(_) => tile_menu::Opener::Queue,
                    _ => tile_menu::Opener::MyGames,
                };
                return Some(tile_menu::open(&library.games[place], opener));
            }
            _ => {}
        }
        // Where focus went names its game.
        let game = match self.focus {
            Focus::Cover(i) => recently_installed(library).get(i).map(|game| game.id),
            Focus::Row(i) => library.queue.entries.get(i).map(|entry| entry.game),
            Focus::Nothing => None,
        };
        self.game = game.or(self.game);
        self.keep(library);
        None
    }

    /// More options on a cover or a row, though View opens the menu there too, as the Bundle's bar says.
    fn hints(&self, _around: Around) -> &'static [Hint] {
        match self.focus {
            Focus::Nothing => hint_bar::NONE,
            _ => hint_bar::MORE_OPTIONS,
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around) {
        let library = around.library;
        self.keep(library);
        let screen = ui.screen();
        ui.fill(screen, PAGE, 0.0);
        ui.label(TITLE.0, TITLE.1, "Manage Queue", &HEADER);

        let covers = recently_installed(library);
        let entries = &library.queue.entries;
        let layout = Layout::of(covers.len(), entries.len());
        let view = screen.bottom() - LIST_TOP;
        self.scroll = self.scroll_to(self.scroll, &layout, view);
        let top = LIST_TOP - self.scroll;
        let clip = Rect::new(CLIP.0, LIST_TOP, CLIP.1 - CLIP.0, view);
        ui.with_clip(clip, |ui| {
            for &(y, header) in &layout.headers {
                ui.label(LEFT, top + y, header, &SECTION);
            }
            if let Some(y) = layout.covers {
                for (i, game) in covers.iter().enumerate() {
                    let rect = Rect::new(LEFT + i as f32 * COVER_PITCH, top + y, COVER, COVER);
                    let focused = self.focus == Focus::Cover(i);
                    home::tile_at(ui, rect, game, COVER, focused, 1.0);
                    let status = library.queue.status(game.id);
                    home::status_band_at(ui, rect, status, focused, 1.0);
                    home::title_band_at(ui, rect, ("recently installed", i), &game.name, status, focused, COVER, 1.0);
                }
            }
            if let Some(y) = layout.empty {
                empty(ui, Rect::new(LEFT, top + y, RIGHT - LEFT, EMPTY));
            }
            for (i, entry) in entries.iter().enumerate() {
                let Some(y) = layout.row(i) else { break };
                let rect = Rect::new(LEFT, top + y, RIGHT - LEFT, ROW);
                if rect.bottom() < LIST_TOP || rect.y > screen.bottom() {
                    continue;
                }
                row(ui, rect, library.game(entry.game), &Figures::of(entry));
            }
            // The ring over everything round it, so that a neighbour's back does not cover its glow.
            match self.focus {
                Focus::Cover(i) if layout.covers.is_some() => {
                    let y = top + layout.covers.unwrap_or(0.0);
                    theme::focus_ring(ui, Rect::new(LEFT + i as f32 * COVER_PITCH, y, COVER, COVER), 1.0);
                }
                Focus::Row(i) => {
                    if let Some(y) = layout.row(i) {
                        crate::my_games::look::ring(ui, Rect::new(LEFT, top + y, RIGHT - LEFT, ROW).grow(8.0));
                    }
                }
                _ => {}
            }
        });
    }
}

/// A game in the Queue: its cover at the row's left; its name, bold, with its speed at the right; its state; then
/// how much is down of the whole with the share at the right, over a thin white bar on a dim track filled to that
/// share. Queued, only the share and the empty track. Workshop content coming in (#188): the items in, and their
/// bytes where known, in the amount's place: "3 of 12 items" or "3 of 12 items, 82.00 MB of 1.20 GB".
fn row(ui: &mut Ui, rect: Rect, game: Option<&Game>, figures: &Figures) {
    ui.fill(rect, BOX, 0.0);
    let cover = Rect::new(rect.x, rect.y, ROW, ROW);
    ui.fill(cover, theme::TILE, 0.0);
    if let Some(key) = game.and_then(|game| home::square_art(game, ui.px())) {
        home::face(ui, cover, &key, 1.0, 0.0);
    }
    let (left, right) = (cover.right() + ROW_TEXT.0, rect.right() - ROW_TEXT.1);
    let line = |y: f32, short: f32| Rect::new(left, y, right - short - left, LINE);
    let right_aligned = |ui: &mut Ui, y: f32, text: &str| {
        let width = ui.measure(&TEXT, text).width;
        ui.label(right - width, y, text, &TEXT);
    };
    let first = rect.y + ROW_TEXT.2;
    if let Some(speed) = &figures.speed {
        right_aligned(ui, first, speed);
    }
    let name = game.map_or("", |game| game.name.as_str());
    ui.paragraph(line(first, TITLE_SHORT), name, &ROW_TITLE, HAlign::Left, VAlign::Top, 1);
    let second = first + LINE;
    ui.paragraph(line(second, 0.0), figures.state, &TEXT, HAlign::Left, VAlign::Top, 1);
    let third = second + LINE + AMOUNT_DOWN;
    let percent = figures.percent.as_deref().unwrap_or("");
    let percent_width = ui.measure(&TEXT, percent).width;
    right_aligned(ui, third, percent);
    let amount = match (&figures.items, &figures.amount) {
        (Some(items), Some(amount)) => Some(format!("{items}, {amount}")),
        (Some(items), None) => Some(items.clone()),
        (None, Some(amount)) => Some(format!("Network: {amount}")),
        (None, None) => None,
    };
    if let Some(amount) = amount {
        ui.paragraph(line(third, percent_width), &amount, &TEXT, HAlign::Left, VAlign::Top, 1);
    }
    let bar = Rect::new(left, third + LINE + BAR_DOWN, right - left, BAR);
    ui.fill(bar, TRACK, 0.0);
    if figures.fraction > 0.0 {
        ui.fill(Rect::new(bar.x, bar.y, bar.w * figures.fraction, bar.h), Color::WHITE, 0.0);
    }
}

/// Nothing queued: a box with a thin light grey outline and no fill, the download arrow over the line saying where
/// the Queue will show, both in its middle.
fn empty(ui: &mut Ui, rect: Rect) {
    ui.ring(rect, EMPTY_OUTLINE, 0.0, 2.0);
    let height = EMPTY_GLYPH + EMPTY_GAP + LINE;
    let top = rect.y + (rect.h - height) / 2.0;
    let middle = rect.center().0;
    ui.icon(middle, top + EMPTY_GLYPH / 2.0, glyph::DOWNLOAD, EMPTY_GLYPH, Color::WHITE);
    let width = ui.measure(&TEXT, EMPTY_TEXT).width;
    ui.label(middle - width / 2.0, top + EMPTY_GLYPH + EMPTY_GAP, EMPTY_TEXT, &TEXT);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/spec/manage-queue.md, 2: where the sections stand, from the list's top at 122.4.
    #[test]
    fn the_sections_stand_where_the_bundle_puts_them() {
        assert!((LIST_TOP - 122.4).abs() < 0.01);
        let both = Layout::of(3, 2);
        assert_eq!(both.headers, [(14.0, "Recently installed"), (312.0 + 30.0, "Installing")]);
        assert_eq!(both.covers, Some(88.0));
        assert_eq!((both.row(0), both.row(1)), (Some(408.0), Some(674.0)));
        assert_eq!(both.height, 408.0 + 2.0 * 266.0 + 80.0);
        // Nothing installed and nothing queued: the outlined box 40 under the title's line.
        let none = Layout::of(0, 0);
        assert!(none.headers.is_empty() && none.rows.is_none());
        assert_eq!((none.empty, none.height), (Some(40.0), 40.0 + 330.0 + 80.0));
        // Only rows: Installing 30 under the title's line.
        assert_eq!(Layout::of(0, 1).headers, [(30.0, "Installing")]);
    }
}
