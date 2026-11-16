//! A tab's entries: which they are, where each is, and what each looks like (docs/spec/guide.md, sections 2.4 to
//! 2.7).
use model::discord::Discord;
use model::Library;
use ui::{Color, Crop, HAlign, Rect, Ui, VAlign};

use super::panel::{PANEL, STRIP};
use super::{discord, focus_ring, marks, Around, Tab};
use crate::home::square_art;
use crate::theme::{glyph, ACCENT_LIGHT, BODY, TEXT_DIM};
use crate::{achievement, profile};

/// How many games the landing tab lists, the running one among them.
const GAMES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Home,
    MyGames,
    /// The game that is running, by its place in the library.
    Running(usize),
    /// A game played before, by its place in the library.
    Recent(usize),
    Profile,
    SwitchProfile,
    /// One of the last achievements unlocked, by its place among the library's last unlocks, newest first.
    RecentAchievement(usize),
    Settings,
    Power,
    /// One of the Discord tab's.
    Discord(discord::Item),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub kind: Kind,
    /// What it says. The Profile entry says the user's name, which is known when it is drawn.
    pub title: String,
    /// Whether a divider is drawn above it.
    pub divider: bool,
    /// A disabled entry is greyed out; it still takes focus, and A does nothing on it.
    pub enabled: bool,
    /// Whether the achievements' totals stand under it, with their divider: Switch profile's, while there are any.
    pub totals: bool,
}

impl Entry {
    pub(super) fn new(kind: Kind, title: &str) -> Self {
        Self { kind, title: title.into(), divider: false, enabled: true, totals: false }
    }

    pub(super) fn under_a_divider(self) -> Self {
        Self { divider: true, ..self }
    }

    /// Whether the Menu button opens a menu on it: the running game's, and a Shortcut's (Remove shortcut).
    pub fn has_menu(&self) -> bool {
        matches!(self.kind, Kind::Running(_) | Kind::Discord(discord::Item::Shortcut(_)))
    }

    /// The box's height, and the room under it.
    fn height(&self) -> (f32, f32) {
        match self.kind {
            Kind::Home | Kind::MyGames => (105.0, 3.0),
            Kind::Running(_) | Kind::Recent(_) => (96.0, 12.0),
            Kind::Profile | Kind::RecentAchievement(_) => (96.0, 3.0),
            Kind::SwitchProfile | Kind::Settings | Kind::Power => (75.0, 3.0),
            Kind::Discord(item) => discord::height(item),
        }
    }
}

/// The achievements' totals under Switch profile (E/OverlayWindow.xaml, AchievementsSection): a divider 14 under
/// the entry's room and 26 over the line, the line of Segoe UI 26 (34.58), and 12 under it.
const TOTALS: f32 = 14.0 + 1.0 + 26.0 + 34.58 + 12.0;

/// The tab's entries, top to bottom.
pub fn of(tab: Tab, around: Around, discord: &Discord) -> Vec<Entry> {
    match tab {
        Tab::Landing => landing(around.library, around.playing),
        Tab::Discord => discord::entries(discord),
        Tab::ProfileAndSystem => profile_and_system(around),
    }
}

/// The profile card, Switch profile, the achievements' totals under it and the last unlocks as entries of their
/// own while there are any totals, then Settings and Power (C/ExtensionCore.cs, ShowProfileTab).
fn profile_and_system(around: Around) -> Vec<Entry> {
    let achievements = &around.library.achievements;
    let totals = achievements.totals.is_some();
    let mut entries = vec![
        Entry::new(Kind::Profile, ""),
        // Switching profile would put Start-up in front of a running game.
        Entry { enabled: around.playing.is_none(), totals, ..Entry::new(Kind::SwitchProfile, "Switch profile") },
    ];
    if totals {
        let recent = achievements.recent.iter().take(model::achievement::GUIDE_RECENT);
        entries.extend(
            recent.enumerate().map(|(i, unlock)| Entry::new(Kind::RecentAchievement(i), &unlock.achievement.name)),
        );
    }
    entries.push(Entry::new(Kind::Settings, "Settings").under_a_divider());
    entries.push(Entry::new(Kind::Power, "Power").under_a_divider());
    entries
}

/// Home, My games & apps, then up to four games under a divider: the running game first, then the games played,
/// most recently first.
fn landing(library: &Library, playing: Option<usize>) -> Vec<Entry> {
    let mut entries = vec![Entry::new(Kind::Home, "Home"), Entry::new(Kind::MyGames, "My games & apps")];
    let running = playing.filter(|&game| game < library.games.len());
    let played = (0..library.recent(usize::MAX).len()).filter(|&game| Some(game) != running);
    let games = running.map(Kind::Running).into_iter().chain(played.map(Kind::Recent)).take(GAMES);
    for (i, kind) in games.enumerate() {
        let (Kind::Running(game) | Kind::Recent(game)) = kind else { continue };
        let entry = Entry::new(kind, &library.games[game].name);
        entries.push(if i == 0 { entry.under_a_divider() } else { entry });
    }
    entries
}

/// The content starts 30 under the tab strip.
const CONTENT_TOP: f32 = PANEL.y + STRIP + 30.0;
/// The heading's top, and its line: WPF's for Bahnschrift at 33.
pub const HEADING_TOP: f32 = CONTENT_TOP - 4.0;
const HEADING_LINE: f32 = 39.6;
/// The room the absent Utilities' row still takes at the foot of a tab without any, and above the row on the
/// landing tab.
const FOOT: f32 = 28.0;
const ABOVE_UTILITIES: f32 = 12.0;

/// Where the first entry's box starts on a tab at rest: the landing tab's Home sits where the other tabs have
/// their heading's foot.
fn first_top(tab: Tab) -> f32 {
    match tab {
        Tab::Landing => CONTENT_TOP + 18.0,
        _ => HEADING_TOP + HEADING_LINE + 12.0,
    }
}

/// The part of the panel the entries show in: they scroll under its top and its foot.
pub fn viewport(tab: Tab) -> (f32, f32) {
    let foot = match tab {
        Tab::Landing => super::utilities::rect(0).y - ABOVE_UTILITIES,
        _ => PANEL.bottom() - FOOT,
    };
    // The viewport is 24 larger than its content at the top, so a focused entry's glow is not cut there.
    (first_top(tab) - if tab == Tab::Landing { 42.0 } else { 24.0 }, foot)
}

/// Where an entry is, before any scrolling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// Its divider's top, if it has one.
    pub divider: Option<f32>,
    /// The entry's whole extent down the list: its divider and the room under its box included, and the
    /// achievements' totals under it.
    pub extent: (f32, f32),
    pub rect: Rect,
    /// Where the achievements' totals under it start, if they stand under it.
    pub totals: Option<f32>,
}

/// Lays the entries out down the panel. The Discord tab's Shortcuts stand in rows of three, each tile 9 in from
/// its column of 178.
pub fn layout(tab: Tab, entries: &[Entry]) -> Vec<Placed> {
    let mut y = first_top(tab);
    // The row of tiles being filled: its top (its divider's included) and its tiles' top.
    let mut row = (0.0, 0.0);
    let place = |entry: &Entry| {
        if let Kind::Discord(discord::Item::Shortcut(at)) = entry.kind {
            let column = at % discord::PER_ROW;
            let divider = entry.divider.then_some(y + 14.0);
            if column == 0 {
                row = (y, y + if entry.divider { 41.0 } else { 0.0 });
                y = row.1 + discord::TILE + discord::TILE_BELOW;
            }
            let x = PANEL.x + column as f32 * discord::TILE_COLUMN + 9.0;
            let rect = Rect::new(x, row.1, discord::TILE, discord::TILE);
            return Placed { divider, extent: (row.0, y), rect, totals: None };
        }
        let top = y;
        // A divider is 1 high, 14 under what is above it and 26 over the box.
        let divider = entry.divider.then_some(y + 14.0);
        if entry.divider {
            y += 41.0;
        }
        let (height, below) = entry.height();
        let rect = Rect::new(PANEL.x + 9.0, y, PANEL.w - 18.0, height);
        y += height + below;
        let totals = entry.totals.then_some(y);
        if entry.totals {
            y += TOTALS;
        }
        Placed { divider, extent: (top, y), rect, totals }
    };
    entries.iter().map(place).collect()
}

/// How far the list is scrolled once the entry over `extent` has focus: the least movement from `scroll` that
/// brings it into the viewport whole, and all the way back for the list's first entry.
pub fn scroll_to(scroll: f32, first: bool, extent: (f32, f32), viewport: (f32, f32)) -> f32 {
    if first {
        return 0.0;
    }
    let (top, bottom) = (extent.0 - scroll, extent.1 - scroll);
    if top < viewport.0 || bottom - top > viewport.1 - viewport.0 {
        scroll + top - viewport.0
    } else if bottom > viewport.1 {
        scroll + bottom - viewport.1
    } else {
        scroll
    }
}

/// What an entry's face needs from outside the Guide.
pub struct Faces<'a> {
    pub library: &'a Library,
    /// The Profile signed in: the profile entry's picture and gamertag.
    pub profile: &'a model::Profile,
    /// Discord as it last said: the Discord tab's entries' faces.
    pub discord: &'a Discord,
}

/// Draws one entry at `placed`.
pub fn draw(ui: &mut Ui, entry: &Entry, placed: &Placed, focused: bool, faces: &Faces) {
    if let Some(y) = placed.divider {
        ui.fill(Rect::new(PANEL.x + 30.0, y, PANEL.w - 60.0, 1.0), Color::argb(0x33FFFFFF), 0.0);
    }
    let rect = placed.rect;
    if let Kind::Discord(item) = entry.kind {
        discord::draw(ui, item, entry, rect, focused, faces.discord);
        return;
    }
    let (_, cy) = rect.center();
    if focused {
        ui.fill(rect, Color::argb(0x26FFFFFF), 4.0);
    }
    // What stands at the entry's left, and where its text starts.
    let mut text_x = rect.x + 112.0;
    let mut lines = 2;
    let mut detail = None;
    let alpha = if entry.enabled { 1.0 } else { 0.35 };
    match entry.kind {
        Kind::Home | Kind::MyGames => {
            let glyph = if entry.kind == Kind::Home { glyph::HOME } else { glyph::LIBRARY };
            ui.icon(rect.x + 51.0, cy, glyph, 30.0, Color::WHITE);
        }
        Kind::Running(game) | Kind::Recent(game) => {
            cover(ui, Rect::new(rect.x + 6.0, cy - 45.0, 90.0, 90.0), faces.library, game);
            if matches!(entry.kind, Kind::Running(_)) {
                detail = Some("Running".to_string());
            }
        }
        // Its icon on a rounded square, its name over its game's in grey.
        Kind::RecentAchievement(at) => {
            if let Some(unlock) = faces.library.achievements.recent.get(at) {
                achievement::icon(ui, Rect::new(rect.x + 18.0, cy - 30.0, 60.0, 60.0), &unlock.achievement, 4.0);
                let library = faces.library;
                let game = library.games.iter().chain(&library.hidden).find(|game| game.id == unlock.game);
                detail = game.map(|game| game.name.clone());
            }
            (text_x, lines) = (rect.x + 96.0, 1);
        }
        Kind::Profile => {
            let picture = Rect::new(rect.x + 18.0, cy - 30.0, 60.0, 60.0);
            ui.fill(picture, Color::rgb(0x4A4A4A), 999.0);
            profile::picture(ui, picture, &faces.profile.picture);
            (text_x, lines) = (rect.x + 96.0, 1);
        }
        Kind::SwitchProfile | Kind::Settings | Kind::Power => {
            let glyph = match entry.kind {
                Kind::SwitchProfile => glyph::SWITCH_PROFILE,
                Kind::Settings => glyph::SETTINGS,
                _ => glyph::POWER,
            };
            ui.icon(rect.x + 36.0, cy, glyph, 26.0, Color::WHITE.fade(alpha));
            text_x = rect.x + 69.0;
        }
        Kind::Discord(_) => {}
    }
    // The ring is the entry's last part but its text: its glow lies over the fill and the cover.
    if focused {
        focus_ring(ui, rect);
    }

    // The title, at most two lines, and under it the second line some kinds have: together in the box's middle.
    let title = if entry.kind == Kind::Profile { &faces.profile.gamertag } else { &entry.title };
    let width = rect.right() - 16.0 - text_x;
    // A title that fits is set with no limit of lines: with one, the lines after the first start with the space
    // they were broken at.
    if lines > 1 && ui.measure_wrapped(&BODY, title, width, 0).lines <= lines as usize {
        lines = 0;
    }
    let size = ui.measure_wrapped(&BODY, title, width, lines);
    // The title takes at most 68 of the stack, as in the Bundle: two lines and "Running" are a little taller than
    // the box, and are cut to it.
    let taken = size.height.min(68.0);
    let height = taken + if detail.is_some() { size.line_height } else { 0.0 };
    let top = cy - height / 2.0;
    ui.with_alpha(alpha, |ui| {
        ui.with_clip(rect, |ui| {
            ui.paragraph(Rect::new(text_x, top, width, size.height), title, &BODY, HAlign::Left, VAlign::Top, lines);
            if let Some(detail) = &detail {
                let line = Rect::new(text_x, top + taken, width, size.line_height);
                ui.paragraph(line, detail, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Top, 1);
            }
        });
    });
    if let Some(top) = placed.totals {
        totals(ui, top, faces.library);
    }
}

/// The achievements' totals from `top`: a divider, then the trophy, the Score total, "12,345 G", and how many are
/// unlocked, "56 achievements", dimmer (E/OverlayWindow.xaml, AchievementsSection). Not an entry: focus never
/// rests on it.
fn totals(ui: &mut Ui, top: f32, library: &Library) {
    let Some(totals) = library.achievements.totals else { return };
    ui.fill(Rect::new(PANEL.x + 30.0, top + 14.0, PANEL.w - 60.0, 1.0), Color::argb(0x33FFFFFF), 0.0);
    let line = top + 41.0;
    let middle = line + 34.58 / 2.0;
    // The trophy in a box 36 wide, 33 in; the Score 12 after it, the count 24 after that.
    let left = PANEL.x + 33.0;
    ui.icon(left + 18.0, middle, glyph::TROPHY, 26.0, Color::WHITE);
    let score = format!("{} G", achievement::thousands(totals.score));
    let width = ui.label_middle(left + 48.0, middle - 2.0, &score, &BODY).width;
    let count = match totals.unlocked {
        1 => "1 achievement".to_string(),
        n => format!("{} achievements", achievement::thousands(u64::from(n))),
    };
    ui.label_middle(left + 48.0 + width + 24.0, middle - 2.0, &count, &BODY.color(TEXT_DIM));
}

/// A game's cover in the list: square, filled from the cover's top. Without one, the console's own tile for a game
/// with no art: a pad on the accent.
fn cover(ui: &mut Ui, rect: Rect, library: &Library, game: usize) {
    let Some(game) = library.games.get(game) else { return };
    ui.fill(rect, ACCENT_LIGHT, 0.0);
    let (cx, cy) = rect.center();
    let drawn = square_art(game, ui.px()).is_some_and(|key| ui.art(rect, &key, Crop::Top, 1.0, 0.0));
    if !drawn {
        ui.icon(cx, cy, glyph::GAMEPAD, 36.0, Color::WHITE);
    }
    marks::store_badge(ui, rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_landing_tab_lays_out_as_the_bundle_measures() {
        let library = crate::fake_library();
        let entries = of(
            Tab::Landing,
            Around {
                library: &library,
                playing: None,
                discord: false,
                audio: &model::Audio::default(),
                notifications: &[],
                now_playing: None,
            },
            &Discord::default(),
        );
        let placed = layout(Tab::Landing, &entries);
        let tops: Vec<(f32, f32)> = placed.iter().map(|p| (p.rect.y, p.rect.bottom())).collect();
        assert_eq!(
            tops,
            [(183.0, 288.0), (291.0, 396.0), (440.0, 536.0), (548.0, 644.0), (656.0, 752.0), (764.0, 860.0)]
        );
        assert_eq!(placed[2].divider, Some(413.0));
        assert_eq!((placed[0].rect.x, placed[0].rect.right()), (87.0, 605.0));
        // Four games end clear of the Utilities: the landing tab never scrolls.
        assert!(placed[5].extent.1 <= viewport(Tab::Landing).1);
    }

    #[test]
    fn profile_and_system_lays_out_under_its_heading() {
        let library = crate::fake_library();
        let entries = of(
            Tab::ProfileAndSystem,
            Around {
                library: &library,
                playing: None,
                discord: false,
                audio: &model::Audio::default(),
                notifications: &[],
                now_playing: None,
            },
            &Discord::default(),
        );
        let placed = layout(Tab::ProfileAndSystem, &entries);
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        assert!(near(placed[0].rect.y, 212.6) && near(placed[0].rect.h, 96.0));
        assert!(near(placed[1].rect.y, 311.6) && near(placed[1].rect.h, 75.0));
        // The Bundle's pictures have the dividers at y 403 and, with nothing between Switch profile and Settings,
        // 119 further down.
        assert!(near(placed[2].divider.unwrap(), 403.6) && near(placed[3].divider.unwrap(), 522.6));
    }

    #[test]
    fn the_achievements_stand_under_switch_profile_while_there_are_totals() {
        use model::achievement::{Achievement, Totals, Unlock};
        let mut library = crate::fake_library();
        let audio = model::Audio::default();
        fn around<'a>(library: &'a Library, audio: &'a model::Audio) -> Around<'a> {
            Around { library, playing: None, discord: false, audio, notifications: &[], now_playing: None }
        }
        let unlock =
            |name: &str| Unlock { game: 1, achievement: Achievement { name: name.into(), ..Default::default() } };
        library.achievements.recent = ["Five", "Four", "Three", "Two", "One"].map(unlock).to_vec();
        // No totals: no section and no unlocks, as the Bundle has none without them.
        assert_eq!(of(Tab::ProfileAndSystem, around(&library, &audio), &Discord::default()).len(), 4);

        library.achievements.totals = Some(Totals { score: 12_345, unlocked: 56, of: 80, games: 3 });
        let entries = of(Tab::ProfileAndSystem, around(&library, &audio), &Discord::default());
        let kinds: Vec<Kind> = entries.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [
                Kind::Profile,
                Kind::SwitchProfile,
                Kind::RecentAchievement(0),
                Kind::RecentAchievement(1),
                Kind::RecentAchievement(2),
                Kind::Settings,
                Kind::Power
            ]
        );
        assert_eq!(entries[2].title, "Five");
        assert!(entries[1].totals && !entries[2].divider);
        let placed = layout(Tab::ProfileAndSystem, &entries);
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        // The totals' divider where the Bundle's picture has it, y 403, the first unlock under their line.
        assert!(near(placed[1].totals.unwrap() + 14.0, 403.6));
        assert!(near(placed[2].rect.y, 477.18) && near(placed[2].rect.h, 96.0));
        assert!(near(placed[4].rect.bottom(), 477.18 + 2.0 * 99.0 + 96.0));
        assert!(near(placed[5].divider.unwrap(), 477.18 + 3.0 * 99.0 + 14.0));
    }

    #[test]
    fn the_list_scrolls_by_the_least_that_shows_the_focused_entry() {
        let viewport = (100.0, 500.0);
        assert_eq!(scroll_to(0.0, false, (200.0, 300.0), viewport), 0.0, "in view: stays");
        assert_eq!(scroll_to(0.0, false, (450.0, 560.0), viewport), 60.0, "under the foot: up by what is cut");
        assert_eq!(scroll_to(60.0, false, (130.0, 230.0), viewport), 30.0, "over the top: down to its top");
        assert_eq!(scroll_to(60.0, true, (124.0, 230.0), viewport), 0.0, "the first entry: all the way back");
    }
}
