//! Home: the top bar, the Recent row, the wide tiles, and behind them the focused game's art. docs/spec/home.md has
//! the Bundle's measures, which these are.
use std::path::PathBuf;

use model::{Game, Library};
use ui::{ArtKey, Button, Color, Crop, Ease, Gradient, HAlign, Kind, Rect, Ui, VAlign};

use crate::theme::{self, glyph, BODY, BODY_SEMIBOLD, MARGIN, ROW_SCALE as S};

/// How many games the Recent row holds.
pub const RECENT_GAMES: usize = 8;

// The Recent row, in the units it is authored in: multiply by ROW_SCALE.
const TILE: f32 = 153.0;
const GROWTH: f32 = 92.0;
const GAP: f32 = 30.0;
/// The top of the row's box, which is as tall as a focused tile; tiles stand on its bottom.
const ROW_TOP: f32 = 533.0;
const TILE_RADIUS: f32 = 6.0;
/// A tile closes on its size by 63% every 50 ms.
const GROWTH_TAU: f32 = 0.05;

const WIDE_WIDTH: f32 = 359.0;
const WIDE_HEIGHT: f32 = 190.0;
const WIDE_GAP: f32 = 30.0;
/// The wide tiles' top, under the Recent row's box and a gap of 24.
const WIDE_TOP: f32 = ROW_TOP + (TILE + GROWTH + 24.0) * S;

const TOP_BAR_MIDDLE: f32 = 114.0;
const HERO_FADE_SECONDS: f32 = 0.4;
const HERO_FADE_SECONDS_REDUCED: f32 = 0.15;

struct WideTile {
    title: &'static str,
    from: Color,
    to: Color,
    glyph: char,
}

const WIDE_TILES: [WideTile; 4] = [
    WideTile { title: "Discord", from: Color::rgb(0x7983F5), to: Color::rgb(0x4752C4), glyph: glyph::CHAT },
    WideTile { title: "Settings", from: Color::rgb(0x5F6B7A), to: Color::rgb(0x2B323C), glyph: glyph::SETTINGS },
    WideTile { title: "Achievements", from: Color::rgb(0x3FA34D), to: Color::rgb(0x0E5A1C), glyph: glyph::TROPHY },
    WideTile { title: "Downloads", from: Color::rgb(0x3A8EE6), to: Color::rgb(0x1A4C8F), glyph: glyph::DOWNLOAD },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    Recent(usize),
    Wide(usize),
}

/// What a press on Home asks the Dashboard to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HomeAction {
    /// Launch the game at this place in the library.
    Launch(usize),
}

/// A game's cover for a tile: its portrait cover, else its header, else its wide art.
pub fn cover_art(game: &Game, px: f32) -> Option<ArtKey> {
    let path = game.cover.as_ref().or(game.header.as_ref()).or(game.hero.as_ref())?;
    // Decoded 300 wide at 1080p, as the theme decodes it, and in proportion on a larger screen.
    let width = if px > 1.25 { 600 } else { 300 };
    Some(ArtKey { path: path.clone(), kind: Kind::Thumb(width) })
}

/// A game's art for the whole screen, and whether it is the blurred stand-in: its background art as it is, else
/// its cover, blurred, to be drawn overhanging the screen and darkened.
pub fn hero_art(game: &Game, px: f32) -> Option<(ArtKey, bool)> {
    let full = |path: &PathBuf| ArtKey { path: path.clone(), kind: Kind::Full((1920.0 * px).ceil() as u32) };
    match (&game.hero, game.cover.as_ref().or(game.header.as_ref())) {
        (Some(hero), _) => Some((full(hero), false)),
        (None, Some(cover)) => Some((ArtKey { path: cover.clone(), kind: Kind::Blurred }, true)),
        (None, None) => None,
    }
}

/// The profile's picture: until Profiles have pictures, the green disc with the contact glyph, or where there is
/// no icon font the gamertag's initial.
pub fn profile_picture(ui: &mut Ui, rect: Rect, user: &str) {
    ui.fill(rect, theme::ACCENT, 999.0);
    let (cx, cy) = rect.center();
    if ui.has_font(ui::Font::Icons) {
        ui.icon(cx, cy, glyph::CONTACT, 26.0, Color::WHITE);
    } else {
        let initial: String = user.chars().take(1).flat_map(char::to_uppercase).collect();
        ui.label_centered(cx, cy - 1.0, &initial, &BODY_SEMIBOLD);
    }
}

pub struct Home {
    focus: Focus,
    /// The Recent tile Down last left, which Up from the wide tiles goes back to.
    left_recent: usize,
    /// The background art showing, and the one it is taking over from.
    hero: Option<(ArtKey, bool)>,
    hero_before: Option<(ArtKey, bool)>,
}

impl Default for Home {
    fn default() -> Self {
        Self { focus: Focus::Recent(0), left_recent: 0, hero: None, hero_before: None }
    }
}

impl Home {
    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// Puts focus on the Recent row's tile for the game at this place in the library, if it has one.
    pub fn focus_recent(&mut self, game: usize, library: &Library) {
        if game < library.recent(RECENT_GAMES).len() {
            self.focus = Focus::Recent(game);
        }
    }

    /// The game focus is on, by its place in the library.
    pub fn focused_game(&self, library: &Library) -> Option<usize> {
        match self.focus {
            Focus::Recent(i) if i < library.recent(RECENT_GAMES).len() => Some(i),
            _ => None,
        }
    }

    pub fn press(&mut self, button: Button, library: &Library) -> Option<HomeAction> {
        let count = library.recent(RECENT_GAMES).len();
        match (self.focus, button) {
            (Focus::Recent(i), Button::Left) if i > 0 => self.focus = Focus::Recent(i - 1),
            (Focus::Recent(i), Button::Right) if i + 1 < count => self.focus = Focus::Recent(i + 1),
            (Focus::Recent(i), Button::Down) => {
                // The wide tile whose middle is nearest this tile's.
                let tiles = recent_rects(count, &|k| if k == i { 1.0 } else { 0.0 });
                let middle = tiles.get(i).map_or(MARGIN, |r| r.center().0);
                let nearest = (0..WIDE_TILES.len())
                    .min_by(|&a, &b| {
                        let distance = |k: usize| (wide_rect(k).center().0 - middle).abs();
                        distance(a).total_cmp(&distance(b))
                    })
                    .unwrap_or(0);
                self.left_recent = i;
                self.focus = Focus::Wide(nearest);
            }
            (Focus::Recent(i), Button::A) if i < count => return Some(HomeAction::Launch(i)),
            (Focus::Wide(i), Button::Left) if i > 0 => self.focus = Focus::Wide(i - 1),
            (Focus::Wide(i), Button::Right) if i + 1 < WIDE_TILES.len() => self.focus = Focus::Wide(i + 1),
            (Focus::Wide(_), Button::Up) if count > 0 => self.focus = Focus::Recent(self.left_recent.min(count - 1)),
            _ => {}
        }
        None
    }

    pub fn draw(&mut self, ui: &mut Ui, library: &Library, user: &str, clock: &str) {
        let games = library.recent(RECENT_GAMES);
        self.background(ui, games);
        top_bar(ui, user, clock);

        // The Recent row. Each tile's share of the growth closes on 1 with focus and 0 without; what no tile has
        // taken widens the gaps, so the row is as wide whatever has focus.
        let focused = match self.focus {
            Focus::Recent(i) => Some(i),
            Focus::Wide(_) => None,
        };
        let shares: Vec<f32> = (0..games.len())
            .map(|i| ui.approach(("recent tile", i), if focused == Some(i) { 1.0 } else { 0.0 }, GROWTH_TAU))
            .collect();
        let rects = recent_rects(games.len(), &|i| shares[i]);
        for (i, (game, &rect)) in games.iter().zip(&rects).enumerate() {
            game_tile(ui, rect, game, ("recent", i), focused == Some(i));
        }
        if let Some(&rect) = focused.and_then(|i| rects.get(i)) {
            theme::focus_ring(ui, rect, S);
        }

        // The wide tiles.
        for (i, tile) in WIDE_TILES.iter().enumerate() {
            let rect = wide_rect(i);
            ui.gradient(rect, tile.from, tile.to, Gradient::Diagonal, TILE_RADIUS * S);
            let (cx, cy) = rect.center();
            ui.icon(cx, cy - 12.0 * S, tile.glyph, 40.0 * S, Color::WHITE);
            let line = ui.measure(&BODY_SEMIBOLD, tile.title).height;
            let caption = Rect::new(rect.x + 18.0 * S, rect.bottom() - 14.0 * S - line, rect.w - 36.0 * S, line);
            ui.paragraph(caption, tile.title, &BODY_SEMIBOLD, HAlign::Left, VAlign::Top, 1);
            if self.focus == Focus::Wide(i) {
                theme::focus_ring(ui, rect, S);
            }
        }
    }

    /// The base, then the focused Recent game's art crossfading over the last one's, then the scrim.
    fn background(&mut self, ui: &mut Ui, games: &[Game]) {
        let screen = ui.screen();
        ui.gradient(screen, Color::rgb(0x1E2C1C), Color::rgb(0x0C140B), Gradient::Radial, 0.0);

        // A game with no art leaves the last game's up; a game's art takes over once it is decoded.
        let wanted = match self.focus {
            Focus::Recent(i) => games.get(i).and_then(|g| hero_art(g, ui.px())).or(self.hero.clone()),
            Focus::Wide(_) => None,
        };
        if wanted != self.hero {
            let ready = wanted.as_ref().is_none_or(|(key, _)| matches!(ui.art.get(key), ui::Art::Ready { .. }));
            if ready {
                self.hero_before = self.hero.take();
                self.hero = wanted;
            }
        }
        let seconds = if ui.store.reduce_motion { HERO_FADE_SECONDS_REDUCED } else { HERO_FADE_SECONDS };
        let layer = |ui: &mut Ui, art: &Option<(ArtKey, bool)>, to: f32| {
            let Some((key, blurred)) = art else { return 0.0 };
            let shown = ui.tween_from(("hero", key), 0.0, to, seconds, Ease::Linear);
            if *blurred {
                // The blur's soft edges hang 120 off every side of the screen.
                ui.art(screen.grow(120.0), key, Crop::Top, shown, 0.0);
                ui.fill(screen, Color::argb(0x99000000).fade(shown), 0.0);
            } else {
                ui.art(screen, key, Crop::Top, shown, 0.0);
            }
            shown
        };
        if self.hero_before != self.hero && layer(ui, &self.hero_before.clone(), 0.0) <= 0.0 {
            self.hero_before = None;
        }
        layer(ui, &self.hero.clone(), 1.0);

        // The scrim: dark at the top for the top bar, darker at the foot for the rows.
        let stop = |share: f32| screen.h * share;
        ui.gradient(
            Rect::new(screen.x, 0.0, screen.w, stop(0.22)),
            Color::argb(0x99000000),
            Color::CLEAR,
            Gradient::Vertical,
            0.0,
        );
        ui.gradient(
            Rect::new(screen.x, stop(0.4), screen.w, stop(0.6)),
            Color::CLEAR,
            Color::argb(0xD9000000),
            Gradient::Vertical,
            0.0,
        );
    }
}

/// The Recent row's tiles, each with its share of the growth: standing on one line from the left margin.
fn recent_rects(count: usize, share: &dyn Fn(usize) -> f32) -> Vec<Rect> {
    let total: f32 = (0..count).map(share).sum();
    let gap = GAP + if count > 1 { GROWTH * (1.0 - total).max(0.0) / (count - 1) as f32 } else { 0.0 };
    let bottom = ROW_TOP + (TILE + GROWTH) * S;
    let mut x = 0.0;
    (0..count)
        .map(|i| {
            let side = TILE + GROWTH * share(i);
            let rect = Rect::new(MARGIN + x * S, bottom - side * S, side * S, side * S);
            x += side + gap;
            rect
        })
        .collect()
}

fn wide_rect(i: usize) -> Rect {
    Rect::new(MARGIN + i as f32 * (WIDE_WIDTH + WIDE_GAP) * S, WIDE_TOP, WIDE_WIDTH * S, WIDE_HEIGHT * S)
}

/// A game's tile: its cover filling it from the top, or its name on the dark tile; with focus, its name on a band
/// along the foot.
fn game_tile(ui: &mut Ui, rect: Rect, game: &Game, id: (&str, usize), focused: bool) {
    let radius = TILE_RADIUS * S;
    ui.fill(rect, theme::TILE, radius);
    let has_cover = cover_art(game, ui.px()).is_some_and(|key| ui.art(rect, &key, Crop::Top, 1.0, radius));
    if !has_cover {
        // Laid out once, for the tile at rest, so it doesn't reflow as the tile grows.
        let width = (TILE - 20.0) * S;
        let lines = Rect::new(rect.x + (rect.w - width) / 2.0, rect.y + 10.0 * S, width, rect.h - 20.0 * S);
        ui.paragraph(lines, &game.name, &BODY, HAlign::Center, VAlign::Center, 0);
    }
    let band = ui.tween((id, "band"), if focused { 1.0 } else { 0.0 }, if focused { 0.15 } else { 0.1 }, Ease::Linear);
    if band > 0.0 && has_cover {
        // Laid out for the tile at its full size, and cut to the tile as it is.
        let width = (TILE + GROWTH - 16.0) * S;
        let text = ui.measure_wrapped(&BODY, &game.name, width, 2);
        let height = text.height + 16.0 * S;
        ui.with_alpha(band, |ui| {
            // The tile's own shape, cut to the band: square across the top, the tile's corners at the foot.
            ui.with_clip(Rect::new(rect.x, rect.bottom() - height, rect.w, height), |ui| {
                ui.fill(rect, theme::BAND, radius);
                let lines = Rect::new(rect.x + 8.0 * S, rect.bottom() - height + 8.0 * S, width, text.height);
                ui.paragraph(lines, &game.name, &BODY, HAlign::Left, VAlign::Top, 2);
            });
        });
    }
}

/// The top bar: the profile at the left, the three round buttons in the middle, the clock and Notifications at the
/// right.
fn top_bar(ui: &mut Ui, user: &str, clock: &str) {
    let picture = Rect::new(MARGIN, TOP_BAR_MIDDLE - 28.0, 56.0, 56.0);
    profile_picture(ui, picture, user);
    // Segoe UI sits 2 up on the icons' middle, Bahnschrift 2 down.
    ui.label_middle(picture.right() + 16.0, TOP_BAR_MIDDLE - 2.0, user, &BODY);
    for (i, glyph) in [glyph::LIBRARY, glyph::SEARCH, glyph::SETTINGS].into_iter().enumerate() {
        ui.icon(880.0 + 80.0 * i as f32, TOP_BAR_MIDDLE, glyph, 26.0, Color::WHITE);
    }
    ui.icon(1775.0, TOP_BAR_MIDDLE, glyph::NOTIFICATIONS, 26.0, Color::WHITE);
    ui.label_right(1725.0, TOP_BAR_MIDDLE + 2.0, clock, &theme::CLOCK);
}
