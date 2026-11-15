//! The nav down the page's left: the profile, the items with their counts, and the rule by which focus on an item
//! shows it (docs/spec/my-games.md, 2.4 to 2.6 and section 4).
use std::time::{Duration, Instant};

use ui::{Color, Rect, Ui};

use super::look::{self, glyph as page_glyph, DIVIDER, NAV_LABEL, SELECTED, TEXT, TEXT_DIM};
use crate::home::profile_picture;
use crate::theme::{glyph, ACCENT};

/// The nav's column, and the divider's two units right of it.
pub const WIDTH: f32 = 441.0;
const DIVIDER_WIDTH: f32 = 2.0;
/// The profile's picture, and the gamertag 14 right of it.
const PICTURE: Rect = Rect::new(75.0, 78.0, 54.0, 54.0);
/// The first item's top; each is 58 tall and the next 72 under it.
const TOP: f32 = 190.0;
const HEIGHT: f32 = 58.0;
const PITCH: f32 = 72.0;
/// The line between Full library and Manage: 18 under the last item above it, and Manage 32 under the line.
const LINE: (f32, f32) = (36.0, 383.0);
const LINE_GAP: (f32, f32) = (18.0, 32.0);
/// Inside an item: the icon's box from 93, 44 wide, its glyph at the box's left; the label after it; the count
/// ending at 398.
const ICON: f32 = 93.0;
const ICON_SIZE: f32 = 24.0;
const LABEL: f32 = 137.0;
const COUNT_RIGHT: f32 = 398.0;
/// The bar at the right edge of the item whose content is up.
const BAR: f32 = 6.0;

/// A nav item, and so what the page shows beside the nav.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {
    Games,
    /// M4: until then its item is there, counts none and shows a note.
    Apps,
    PlayHistory,
    FullLibrary,
    Manage,
}

impl Item {
    /// Top to bottom. Apps is always in the nav here: there is nothing that could fail to give Apps, which is
    /// what hides it in the Bundle.
    pub const ALL: [Item; 5] = [Item::Games, Item::Apps, Item::PlayHistory, Item::FullLibrary, Item::Manage];

    fn place(self) -> usize {
        Self::ALL.iter().position(|&item| item == self).unwrap_or(0)
    }

    /// The item above, if this is not the first.
    pub fn above(self) -> Option<Item> {
        self.place().checked_sub(1).map(|at| Self::ALL[at])
    }

    /// The item below, if this is not the last: the line is crossed without a stop.
    pub fn below(self) -> Option<Item> {
        Self::ALL.get(self.place() + 1).copied()
    }

    /// The header over its content, and its label in the nav.
    pub fn title(self) -> &'static str {
        match self {
            Item::Games => "Games",
            Item::Apps => "Apps",
            Item::PlayHistory => "Play history",
            Item::FullLibrary => "Full library",
            Item::Manage => "Manage",
        }
    }

    fn glyph(self) -> char {
        match self {
            Item::Games => glyph::GAMEPAD,
            Item::Apps => page_glyph::APPS,
            Item::PlayHistory => glyph::HISTORY,
            Item::FullLibrary => glyph::LIBRARY,
            Item::Manage => page_glyph::MANAGE,
        }
    }

    /// The item's box: the whole width of the column.
    fn rect(self) -> Rect {
        let under_line = if self == Item::Manage { LINE_GAP.0 + 2.0 + LINE_GAP.1 - (PITCH - HEIGHT) } else { 0.0 };
        Rect::new(0.0, TOP + self.place() as f32 * PITCH + under_line, WIDTH, HEIGHT)
    }

    /// Whether `self` is lower on the nav than `other`: the pane then flies up from below.
    pub fn is_below(self, other: Item) -> bool {
        self.place() > other.place()
    }
}

/// Focus on a nav item shows it, at once if focus came to the nav's items one at a time, and once it has rested on
/// the item for [`SETTLE`] if it is passing along them (docs/spec/my-games.md, 4.2).
pub const SETTLE: Duration = Duration::from_millis(150);

/// The settle wait. It is told the time, so that it is tested with a clock of the test's own.
#[derive(Debug, Default)]
pub struct Settle {
    /// When a nav item last took focus.
    last: Option<Instant>,
    /// The item waiting to be shown, and since when focus has been on it.
    pending: Option<(Item, Instant)>,
}

impl Settle {
    /// Focus came to `item` at `now`, with `showing` up. Gives the item to show at once, if there is one: an item
    /// passed in under the wait is shown later, by `due`, or never if focus moves on.
    pub fn focused(&mut self, item: Item, showing: Item, now: Instant) -> Option<Item> {
        let passing = self.last.is_some_and(|last| now.saturating_duration_since(last) < SETTLE);
        self.last = Some(now);
        self.pending = (passing && item != showing).then_some((item, now));
        (!passing && item != showing).then_some(item)
    }

    /// The item whose wait is over at `now`, once.
    pub fn due(&mut self, now: Instant) -> Option<Item> {
        let (item, since) = self.pending?;
        if now.saturating_duration_since(since) < SETTLE {
            return None;
        }
        self.pending = None;
        Some(item)
    }

    /// The item still waiting, to be shown now: Right or A on it does not wait.
    pub fn take(&mut self) -> Option<Item> {
        self.pending.take().map(|(item, _)| item)
    }

    /// Focus left the nav some other way: nothing is waiting any more.
    pub fn drop_pending(&mut self) {
        self.pending = None;
    }

    /// How long from `now` until the waiting item is due.
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.pending.map(|(_, since)| SETTLE.saturating_sub(now.saturating_duration_since(since)))
    }
}

/// What the nav is drawn from.
pub struct Face<'a> {
    pub user: &'a str,
    /// The item whose content is up: lit grey, with the bar.
    pub showing: Item,
    /// The item with focus, if focus is on the nav: filled with the accent.
    pub focused: Option<Item>,
    /// The number at an item's right, for those that count.
    pub count: &'a dyn Fn(Item) -> Option<usize>,
}

/// Draws the nav and the divider beside it. On a screen wider than the canvas the items' fills and the nav's
/// ground run out to the screen's left edge.
pub fn draw(ui: &mut Ui, face: &Face) {
    let screen = ui.screen();
    profile_picture(ui, PICTURE);
    ui.label_middle(PICTURE.right() + 14.0, PICTURE.center().1, face.user, &TEXT_DIM);

    let full = |rect: Rect| Rect::new(screen.x, rect.y, rect.right() - screen.x, rect.h);
    let showing = face.showing.rect();
    if face.focused != Some(face.showing) {
        ui.fill(full(showing), SELECTED, 0.0);
    }
    // The focused item is drawn over its neighbours, and its glow over the divider's place: the divider is drawn
    // after it.
    if let Some(focused) = face.focused {
        look::slab(ui, full(focused.rect()), 0.0);
    }
    // The bar is on what is showing, focused or not: on the accent fill it is the same green.
    ui.fill(Rect::new(showing.right() - BAR, showing.y, BAR, showing.h), ACCENT, 0.0);

    let line = Item::FullLibrary.rect().bottom() + LINE_GAP.0;
    ui.fill(Rect::new(LINE.0, line, LINE.1 - LINE.0, 2.0), DIVIDER, 0.0);
    ui.fill(Rect::new(WIDTH, 0.0, DIVIDER_WIDTH, screen.h), DIVIDER, 0.0);

    for item in Item::ALL {
        let rect = item.rect();
        let middle = rect.center().1;
        ui.icon(ICON + ICON_SIZE / 2.0, middle, item.glyph(), ICON_SIZE, Color::WHITE);
        ui.label_middle(LABEL, middle, item.title(), &NAV_LABEL);
        if let Some(count) = (face.count)(item) {
            ui.label_right(COUNT_RIGHT, middle, &count.to_string(), &TEXT);
        }
    }
    foot(ui);
}

/// **M4's hook, and the storage ring's.** The nav's foot, bottom left (x 75, its bottom 66 off the screen's): the
/// current download's block over the storage ring (docs/spec/my-games.md, 2.6). Neither is drawn yet: the block
/// is the Queue's, and the ring needs the drives' free space, which nothing reads (open question 9).
fn foot(_ui: &mut Ui) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/spec/my-games.md, 2.5, with Apps shown.
    #[test]
    fn the_items_stand_where_the_spec_says() {
        let tops: Vec<f32> = Item::ALL.iter().map(|item| item.rect().y).collect();
        assert_eq!(tops, [190.0, 262.0, 334.0, 406.0, 516.0]);
        assert_eq!(Item::FullLibrary.rect().bottom() + LINE_GAP.0, 482.0, "the line");
        assert!(Item::ALL.iter().all(|item| item.rect().h == 58.0 && item.rect().right() == 441.0));
    }

    #[test]
    fn the_nav_stops_at_its_ends_and_crosses_the_line() {
        assert_eq!((Item::Games.above(), Item::Manage.below()), (None, None));
        assert_eq!((Item::FullLibrary.below(), Item::Manage.above()), (Some(Item::Manage), Some(Item::FullLibrary)));
        assert!(Item::Manage.is_below(Item::Games) && !Item::Games.is_below(Item::Apps));
    }

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    /// docs/spec/my-games.md, 4.2: a single step opens at once.
    #[test]
    fn an_item_reached_in_one_step_shows_at_once() {
        let (mut settle, start) = (Settle::default(), Instant::now());
        assert_eq!(settle.focused(Item::Apps, Item::Games, start), Some(Item::Apps));
        assert_eq!(settle.wait(start), None);
        // And so does the next, a moment later than the wait.
        assert_eq!(settle.focused(Item::PlayHistory, Item::Apps, at(start, 150)), Some(Item::PlayHistory));
    }

    #[test]
    fn focus_passing_along_the_nav_shows_only_the_item_it_rests_on() {
        let (mut settle, start) = (Settle::default(), Instant::now());
        assert_eq!(settle.focused(Item::Apps, Item::Games, start), Some(Item::Apps));
        // A held direction repeats every 80 ms: the items passed over never show.
        assert_eq!(settle.focused(Item::PlayHistory, Item::Apps, at(start, 80)), None);
        assert_eq!(settle.due(at(start, 159)), None);
        assert_eq!(settle.focused(Item::FullLibrary, Item::Apps, at(start, 160)), None);
        assert_eq!(settle.wait(at(start, 200)), Some(Duration::from_millis(110)));
        assert_eq!(settle.due(at(start, 309)), None);
        assert_eq!(settle.due(at(start, 310)), Some(Item::FullLibrary));
        assert_eq!((settle.due(at(start, 400)), settle.wait(at(start, 400))), (None, None), "once");
    }

    #[test]
    fn back_on_the_item_showing_nothing_opens_and_what_waited_is_dropped() {
        let (mut settle, start) = (Settle::default(), Instant::now());
        settle.focused(Item::Apps, Item::Games, start);
        assert_eq!(settle.focused(Item::PlayHistory, Item::Apps, at(start, 50)), None);
        assert_eq!(settle.focused(Item::Apps, Item::Apps, at(start, 100)), None);
        assert_eq!(settle.due(at(start, 1000)), None);
    }

    #[test]
    fn right_or_a_takes_the_waiting_item_at_once_and_leaving_drops_it() {
        let (mut settle, start) = (Settle::default(), Instant::now());
        settle.focused(Item::Apps, Item::Games, start);
        settle.focused(Item::PlayHistory, Item::Apps, at(start, 50));
        assert_eq!(settle.take(), Some(Item::PlayHistory));
        assert_eq!(settle.take(), None);
        settle.focused(Item::FullLibrary, Item::PlayHistory, at(start, 100));
        settle.drop_pending();
        assert_eq!(settle.due(at(start, 1000)), None);
    }
}
