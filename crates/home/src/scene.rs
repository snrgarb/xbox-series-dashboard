//! Home's state and layout: the top bar, the Recent row, the wide tiles and the background that follows focus.
//! Laid out in design units on a 1920x1080 canvas, as the theme is, and scaled to the output by its width.
use model::Library;

use crate::art::{Job, Slot};
use crate::render::{rgba, Frame, Quad, Rect, TexId, Text, TextKey, TextSys};

/// The theme lays its rows out 1526 wide and scales them to the 1700 between the side margins.
const K: f32 = 1700.0 / 1526.0;
const MARGIN: f32 = 105.0;
const ROW_WIDTH: f32 = 1700.0;
const TILE: f32 = 153.0 * K;
const FOCUSED_TILE: f32 = 245.0 * K;
const GAP: f32 = 30.0 * K;
/// Measured off the console's Home (docs/reference/startup/4-home.jpg).
const ROW_BOTTOM: f32 = 743.0;
const WIDE_TOP: f32 = ROW_BOTTOM + 24.0 * K;
const WIDE_HEIGHT: f32 = 190.0 * K;
const TILE_RADIUS: f32 = 6.0 * K;
const RING_OFFSET: f32 = 8.0 * K;
const RING_THICKNESS: f32 = 4.0 * K;
const RING_RADIUS: f32 = 14.0 * K;
const TEXT_SIZE: f32 = 26.0;
const TILE_COLOR: u32 = 0x3B3B3B;
pub const RECENT_COUNT: usize = 8;

const MOVE_SECONDS: f32 = 0.18;
const BACKGROUND_SECONDS: f32 = 0.45;
const ART_SECONDS: f32 = 0.2;
const LAUNCH_SECONDS: f32 = 0.35;
const GUIDE_SECONDS: f32 = 0.2;
/// The Guide's panel and its entries, as the extension's Overlay lays them out.
const GUIDE_WIDTH: f32 = 540.0;
const GUIDE_ENTRY: f32 = 105.0;
const GUIDE_TOP: f32 = 168.0;

/// What selecting a Guide entry does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GuideAction {
    /// Close the Guide: back to Home, or to the running game.
    Close,
    QuitGame,
    /// Launch the Recent row's game at this place.
    Launch(usize),
    Exit,
}

pub struct GuideEntry {
    pub title: String,
    pub detail: Option<String>,
    /// The Recent row game whose art it shows.
    pub game: Option<usize>,
    pub action: GuideAction,
}

#[derive(Default)]
struct Guide {
    entries: Vec<GuideEntry>,
    focus: usize,
    open: bool,
    t: f32,
}

const WIDE_TILES: [(&str, u32, u32); 4] = [
    ("Discord", 0x5865F2, 0x3C45A5),
    ("Playnite settings", 0x4A4A4A, 0x2E2E2E),
    ("Achievements", 0x1E7A34, 0x0F4A1E),
    ("Downloads", 0x4A4A4A, 0x2E2E2E),
];

#[derive(Clone, Copy, PartialEq)]
enum Art {
    None,
    Pending,
    Ready(TexId),
}

impl Art {
    fn ready(self) -> Option<TexId> {
        match self {
            Art::Ready(tex) => Some(tex),
            _ => None,
        }
    }
}

struct Tile {
    name: String,
    art: Art,
    hero: Art,
    /// How focused it is, 0 to 1: its size follows.
    focus: f32,
    focus_from: f32,
    /// Its art's fade-in.
    art_shown: f32,
}

pub struct Scene {
    user: String,
    tiles: Vec<Tile>,
    focus: usize,
    move_t: f32,
    /// Where the focus ring left from, in design units.
    ring_from: Option<Rect>,
    background: Option<TexId>,
    background_from: Option<TexId>,
    background_t: f32,
    /// The Launch screen: the focused game's art alone, while it starts.
    launching: bool,
    launch_t: f32,
    time: f32,
    guide: Guide,
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

impl Scene {
    /// The scene and the art it needs, the visible screen's first.
    pub fn new(library: &Library, focus: usize, output: (u32, u32)) -> (Self, Vec<Job>) {
        let games = library.recent(RECENT_COUNT);
        let focus = focus.min(games.len().saturating_sub(1));
        let side = (FOCUSED_TILE * output.0 as f32 / 1920.0).ceil() as u32;
        let mut jobs = Vec::new();
        let mut heroes = Vec::new();
        let mut tiles = Vec::new();
        for (i, game) in games.iter().enumerate() {
            let pending = |path: &Option<_>| if path.is_some() { Art::Pending } else { Art::None };
            // A square tile from the portrait cover, else the header, else the hero.
            let art = game.cover.as_ref().or(game.header.as_ref()).or(game.hero.as_ref());
            if let Some(path) = art {
                jobs.push(Job { slot: Slot::Tile(i), path: path.clone(), target: (side, side) });
            }
            if let Some(path) = &game.hero {
                let job = Job { slot: Slot::Hero(i), path: path.clone(), target: output };
                if i == focus { heroes.insert(0, job) } else { heroes.push(job) }
            }
            tiles.push(Tile {
                name: game.name.clone(),
                art: pending(&art.cloned()),
                hero: pending(&game.hero),
                focus: if i == focus { 1.0 } else { 0.0 },
                focus_from: 0.0,
                art_shown: 0.0,
            });
        }
        jobs.extend(heroes);
        let scene = Self {
            user: library.user.clone().unwrap_or_else(|| "Player".into()),
            tiles,
            focus,
            move_t: 1.0,
            ring_from: None,
            background: None,
            background_from: None,
            background_t: 1.0,
            launching: false,
            launch_t: 0.0,
            time: 0.0,
            guide: Guide::default(),
        };
        (scene, jobs)
    }

    pub fn set_art(&mut self, slot: Slot, tex: Option<TexId>) {
        let art = tex.map_or(Art::None, Art::Ready);
        match slot {
            Slot::Tile(i) => self.tiles[i].art = art,
            Slot::Hero(i) => self.tiles[i].hero = art,
        }
    }

    /// Whether art the screen shows now is still loading: any tile's, or the focused game's hero.
    pub fn visible_art_pending(&self) -> bool {
        self.tiles.iter().any(|t| t.art == Art::Pending)
            || self.tiles.get(self.focus).is_some_and(|t| t.hero == Art::Pending)
    }

    pub fn art_pending(&self) -> bool {
        self.tiles.iter().any(|t| t.art == Art::Pending || t.hero == Art::Pending)
    }

    pub fn focus(&self) -> usize {
        self.focus
    }

    /// Moves focus along the Recent row; false at either end.
    pub fn move_focus(&mut self, by: i32) -> bool {
        let to = (self.focus as i32 + by).clamp(0, self.tiles.len() as i32 - 1).max(0) as usize;
        if to == self.focus {
            return false;
        }
        self.ring_from = Some(self.ring());
        for tile in &mut self.tiles {
            tile.focus_from = tile.focus;
        }
        self.focus = to;
        self.move_t = 0.0;
        true
    }

    /// Opens the Guide on these entries, focus on the first.
    pub fn open_guide(&mut self, entries: Vec<GuideEntry>) {
        self.guide.entries = entries;
        self.guide.focus = 0;
        self.guide.open = true;
    }

    pub fn close_guide(&mut self) {
        self.guide.open = false;
    }

    pub fn guide_open(&self) -> bool {
        self.guide.open
    }

    pub fn guide_move(&mut self, by: i32) {
        let last = self.guide.entries.len().saturating_sub(1) as i32;
        self.guide.focus = (self.guide.focus as i32 + by).clamp(0, last) as usize;
    }

    pub fn guide_action(&self) -> Option<GuideAction> {
        self.guide.entries.get(self.guide.focus).map(|e| e.action)
    }

    /// How much of the output's width the Guide's panel covers now, in pixels: 0 once it has slid away.
    pub fn guide_panel(&self, output: (u32, u32)) -> u32 {
        (GUIDE_WIDTH * ease(self.guide.t) * output.0 as f32 / 1920.0).round() as u32
    }

    /// Turns Home into the Launch screen, or back.
    pub fn set_launching(&mut self, on: bool) {
        self.launching = on;
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        let guide_step = dt / GUIDE_SECONDS;
        self.guide.t = (self.guide.t + if self.guide.open { guide_step } else { -guide_step }).clamp(0.0, 1.0);
        let step = dt / LAUNCH_SECONDS;
        self.launch_t = (self.launch_t + if self.launching { step } else { -step }).clamp(0.0, 1.0);
        self.move_t = (self.move_t + dt / MOVE_SECONDS).min(1.0);
        let e = ease(self.move_t);
        for (i, tile) in self.tiles.iter_mut().enumerate() {
            let to = if i == self.focus { 1.0 } else { 0.0 };
            tile.focus = tile.focus_from + (to - tile.focus_from) * e;
            if tile.art.ready().is_some() {
                tile.art_shown = (tile.art_shown + dt / ART_SECONDS).min(1.0);
            }
        }
        let wanted = self.tiles.get(self.focus).and_then(|t| t.hero.ready());
        if wanted != self.background {
            self.background_from = self.background;
            self.background = wanted;
            self.background_t = 0.0;
        }
        self.background_t = (self.background_t + dt / BACKGROUND_SECONDS).min(1.0);
    }

    /// Ends every animation where it is going.
    pub fn settle(&mut self) {
        self.update(10.0);
    }

    /// The tiles of the Recent row in design units: a row from the left margin, standing on one line.
    fn tile_rects(&self) -> Vec<Rect> {
        let mut x = MARGIN;
        self.tiles
            .iter()
            .map(|tile| {
                let side = TILE + (FOCUSED_TILE - TILE) * tile.focus;
                let rect = Rect::new(x, ROW_BOTTOM - side, side, side);
                x += side + GAP;
                rect
            })
            .collect()
    }

    /// The focus ring now, in design units: sliding from where it left to the focused tile.
    fn ring(&self) -> Rect {
        let Some(&to) = self.tile_rects().get(self.focus) else { return Rect::new(0.0, 0.0, 0.0, 0.0) };
        let to = to.grow(RING_OFFSET);
        self.ring_from.map_or(to, |from| from.lerp(to, ease(self.move_t)))
    }

    pub fn build(&self, text: &mut TextSys, output: (u32, u32)) -> Frame {
        let (width, height) = (output.0 as f32, output.1 as f32);
        let s = width / 1920.0;
        // The top bar hangs from the top, the rows stand on the bottom: the same on any shape of screen.
        let top = |r: Rect| Rect::new(r.x * s, r.y * s, r.w * s, r.h * s);
        let bottom = |r: Rect| Rect::new(r.x * s, height - (1080.0 - r.y) * s, r.w * s, r.h * s);
        let screen = Rect::new(0.0, 0.0, width, height);
        let key = |string: &str, semibold: bool, wrap: f32, centered: bool| TextKey {
            text: string.into(),
            size: (TEXT_SIZE * s * 10.0).round() as u32,
            semibold,
            wrap: (wrap * s) as u32,
            centered,
        };
        // Home's own text fades out as the Launch screen comes up over it.
        let launch = ease(self.launch_t);
        let white = |alpha: f32| [255, 255, 255, ((alpha * (1.0 - launch)).clamp(0.0, 1.0) * 255.0) as u8];
        let mut frame = Frame::default();

        // Background: the focused game's hero cross-fading over the last, darkened so the text reads.
        frame.quads.push(Quad::gradient(screen, rgba(0x12381C, 1.0), rgba(0x050F08, 1.0), 0.0));
        let e = ease(self.background_t);
        match (self.background_from, self.background) {
            (from, Some(to)) => {
                frame.quads.extend(from.map(|from| Quad::image(screen, from, 1.0, 0.0)));
                frame.quads.push(Quad::image(screen, to, e, 0.0));
            }
            (Some(from), None) => frame.quads.push(Quad::image(screen, from, 1.0 - e, 0.0)),
            (None, None) => {}
        }
        frame.quads.push(Quad::gradient(screen, rgba(0, 0.6), rgba(0, 0.3), 0.0));
        let lower = Rect::new(0.0, height * 0.35, width, height * 0.65);
        frame.quads.push(Quad::gradient(lower, rgba(0, 0.0), rgba(0, 0.75), 0.0));

        // Top bar: the profile at the left, the clock at the right.
        let picture = top(Rect::new(MARGIN, 86.0, 56.0, 56.0));
        frame.quads.push(Quad::gradient(picture, rgba(0x2CA243, 1.0), rgba(0x107C10, 1.0), picture.w / 2.0));
        let bar_middle = picture.y + picture.h / 2.0;
        let mut line = |frame: &mut Frame, key: TextKey, place: &dyn Fn(f32) -> f32| {
            let (w, line_height, _) = text.measure(&key);
            frame.texts.push(Text { key, x: place(w), y: bar_middle - line_height / 2.0, clip: screen, color: white(1.0) });
        };
        let initial: String = self.user.chars().take(1).flat_map(char::to_uppercase).collect();
        line(&mut frame, key(&initial, true, 0.0, false), &|w| picture.x + (picture.w - w) / 2.0);
        line(&mut frame, key(&self.user, false, 0.0, false), &|_| picture.x + picture.w + 16.0 * s);
        let clock = chrono::Local::now().format("%-I:%M %p").to_string();
        line(&mut frame, key(&clock, true, 0.0, false), &|w| width - MARGIN * s - w);

        // The Recent row.
        let pad = 8.0 * K * s;
        for (tile, rect) in self.tiles.iter().zip(self.tile_rects()) {
            let rect = bottom(rect);
            let radius = TILE_RADIUS * s;
            frame.quads.push(Quad::fill(rect, rgba(TILE_COLOR, 1.0), radius));
            if let Some(tex) = tile.art.ready() {
                frame.quads.push(Quad::image(rect, tex, tile.art_shown, radius));
            }
            // Without art, or until it shows, the name in the tile's middle.
            if tile.art_shown < 1.0 {
                let key = key(&tile.name, false, TILE - 20.0, true);
                let (_, line_height, lines) = text.measure(&key);
                let x = rect.x + (rect.w - key.wrap as f32) / 2.0;
                let y = rect.y + ((rect.h - lines as f32 * line_height) / 2.0).max(pad);
                frame.texts.push(Text { key, x, y, clip: rect.grow(-pad), color: white(1.0 - tile.art_shown) });
            }
            // Over art, the focused tile's name on a band along its bottom, at most two lines.
            let shown = tile.focus * tile.art_shown;
            if shown > 0.01 {
                let key = key(&tile.name, false, FOCUSED_TILE - 16.0 * K, false);
                let (_, line_height, lines) = text.measure(&key);
                let text_height = lines.min(2) as f32 * line_height;
                // Blending is in linear light, where black takes more alpha to darken than it does in WPF: the band
                // is laid twice, so it is dark by the first line and still fades out smoothly above it.
                let band = Rect::new(rect.x, rect.y + rect.h * 0.35, rect.w, rect.h * 0.65);
                for _ in 0..2 {
                    frame.quads.push(Quad::gradient(band, rgba(0, 0.0), rgba(0, 0.95 * shown), radius));
                }
                let y = rect.y + rect.h - pad - text_height;
                let clip = Rect::new(rect.x + pad, y, rect.w - 2.0 * pad, text_height);
                frame.texts.push(Text { key, x: rect.x + pad, y, clip, color: white(shown) });
            }
        }
        if !self.tiles.is_empty() {
            let ring = bottom(self.ring());
            let line = 1.5 * s;
            frame.quads.push(Quad::ring(ring.grow(line), rgba(0, 0.4), (RING_RADIUS + 1.5) * s, line));
            frame.quads.push(Quad::ring(ring, rgba(0xFFFFFF, 1.0), RING_RADIUS * s, RING_THICKNESS * s));
        }

        // The wide tiles.
        let wide = (ROW_WIDTH - 3.0 * GAP) / 4.0;
        for (i, (label, from, to)) in WIDE_TILES.into_iter().enumerate() {
            let rect = bottom(Rect::new(MARGIN + i as f32 * (wide + GAP), WIDE_TOP, wide, WIDE_HEIGHT));
            frame.quads.push(Quad::gradient(rect, rgba(from, 1.0), rgba(to, 1.0), TILE_RADIUS * s));
            let key = key(label, false, 0.0, false);
            let (_, line_height, _) = text.measure(&key);
            let inset = 16.0 * s;
            let y = rect.y + rect.h - inset - line_height;
            frame.texts.push(Text { key, x: rect.x + inset, y, clip: rect, color: white(1.0) });
        }

        // The Guide: a panel sliding in from the left over a dimmed Home. Text is drawn over every quad, so Home's
        // own is dimmed with it and cut off at the panel's edge.
        let guide = ease(self.guide.t);
        if guide > 0.0 {
            let panel = Rect::new((guide - 1.0) * GUIDE_WIDTH * s, 0.0, GUIDE_WIDTH * s, height);
            let edge = panel.x + panel.w;
            for t in &mut frame.texts {
                let right = t.clip.x + t.clip.w;
                t.clip.x = t.clip.x.max(edge);
                t.clip.w = (right - t.clip.x).max(0.0);
                t.color[3] = (t.color[3] as f32 * (1.0 - 0.6 * guide)) as u8;
            }
            frame.quads.push(Quad::fill(screen, rgba(0, 0.8 * guide), 0.0));
            frame.quads.push(Quad::fill(panel, rgba(0x1C1C1C, 1.0), 0.0));
            let light = |alpha: f32| [255, 255, 255, (alpha * 255.0) as u8];
            let line = |string: &str, semibold: bool, wrap: f32| TextKey {
                text: string.into(),
                size: (TEXT_SIZE * s * 10.0).round() as u32,
                semibold,
                wrap: (wrap * s) as u32,
                centered: false,
            };

            // The profile, as on Home's top bar.
            let picture = Rect::new(panel.x + 42.0 * s, 64.0 * s, 56.0 * s, 56.0 * s);
            frame.quads.push(Quad::gradient(picture, rgba(0x2CA243, 1.0), rgba(0x107C10, 1.0), picture.w / 2.0));
            let initial: String = self.user.chars().take(1).flat_map(char::to_uppercase).collect();
            let key = line(&initial, true, 0.0);
            let (w, line_height, _) = text.measure(&key);
            let y = picture.y + (picture.h - line_height) / 2.0;
            frame.texts.push(Text { key, x: picture.x + (picture.w - w) / 2.0, y, clip: panel, color: light(1.0) });
            let key = line(&self.user, true, 0.0);
            frame.texts.push(Text { key, x: picture.x + picture.w + 18.0 * s, y, clip: panel, color: light(1.0) });
            let rule = Rect::new(panel.x + 30.0 * s, 144.0 * s, panel.w - 60.0 * s, (1.0 * s).max(1.0));
            frame.quads.push(Quad::fill(rule, rgba(0xFFFFFF, 0.2), 0.0));

            for (i, entry) in self.guide.entries.iter().enumerate() {
                let y = (GUIDE_TOP + i as f32 * (GUIDE_ENTRY + 3.0)) * s;
                let rect = Rect::new(panel.x + 9.0 * s, y, panel.w - 18.0 * s, GUIDE_ENTRY * s);
                if i == self.guide.focus {
                    frame.quads.push(Quad::fill(rect, rgba(0xFFFFFF, 0.03), 4.0 * s));
                    frame.quads.push(Quad::ring(rect.grow(4.0 * s), rgba(0xFFFFFF, 1.0), 10.0 * s, 4.0 * s));
                }
                let mut text_x = rect.x + 33.0 * s;
                if let Some(tile) = entry.game.and_then(|g| self.tiles.get(g)) {
                    let cover = Rect::new(rect.x + 8.0 * s, rect.y + 7.5 * s, 90.0 * s, 90.0 * s);
                    frame.quads.push(Quad::fill(cover, rgba(TILE_COLOR, 1.0), 4.0 * s));
                    if let Some(tex) = tile.art.ready() {
                        frame.quads.push(Quad::image(cover, tex, 1.0, 4.0 * s));
                    }
                    text_x = rect.x + 112.0 * s;
                }
                let key = line(&entry.title, false, 0.0);
                let (_, line_height, _) = text.measure(&key);
                let lines = if entry.detail.is_some() { 2.0 } else { 1.0 };
                let mut text_y = rect.y + (rect.h - lines * line_height) / 2.0;
                let clip = Rect::new(rect.x, rect.y, rect.w - 16.0 * s, rect.h);
                frame.texts.push(Text { key, x: text_x, y: text_y, clip, color: light(1.0) });
                if let Some(detail) = &entry.detail {
                    text_y += line_height;
                    frame.texts.push(Text { key: line(detail, false, 0.0), x: text_x, y: text_y, clip, color: light(0.7) });
                }
            }
            let key = line("A  Select      B  Close", false, 0.0);
            let (_, line_height, _) = text.measure(&key);
            let y = height - 60.0 * s - line_height;
            frame.texts.push(Text { key, x: panel.x + 42.0 * s, y, clip: panel, color: light(0.7) });
        }

        // The Launch screen: the game's art over all of Home, its name and three dots that pulse in turn.
        if launch > 0.0 {
            match self.background {
                Some(tex) => frame.quads.push(Quad::image(screen, tex, launch, 0.0)),
                None => frame.quads.push(Quad::gradient(screen, rgba(0x12381C, launch), rgba(0x050F08, launch), 0.0)),
            }
            frame.quads.push(Quad::gradient(screen, rgba(0, 0.35 * launch), rgba(0, 0.8 * launch), 0.0));
            let name = self.tiles.get(self.focus).map_or("", |t| t.name.as_str());
            let key = TextKey {
                text: name.into(),
                size: (56.0 * s * 10.0).round() as u32,
                semibold: true,
                wrap: (1500.0 * s) as u32,
                centered: true,
            };
            let (_, line_height, lines) = text.measure(&key);
            let y = height * 0.7 - lines as f32 * line_height;
            let color = [255, 255, 255, (launch * 255.0) as u8];
            frame.texts.push(Text { key: key.clone(), x: (width - key.wrap as f32) / 2.0, y, clip: screen, color });
            let dot = 14.0 * s;
            for i in 0..3 {
                let phase = (self.time * 1.6 - i as f32 * 0.22).rem_euclid(1.0);
                let glow = 0.25 + 0.75 * (1.0 - (phase * 2.0 - 1.0).abs()).powi(2);
                let x = width / 2.0 + (i as f32 - 1.0) * dot * 2.4 - dot / 2.0;
                let rect = Rect::new(x, height * 0.7 + 36.0 * s, dot, dot);
                frame.quads.push(Quad::fill(rect, rgba(0xFFFFFF, glow * launch), dot / 2.0));
            }
        }
        frame
    }
}
