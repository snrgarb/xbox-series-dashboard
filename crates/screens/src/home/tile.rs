//! A tile's face, as the Recent row's tiles and the Groups' cards share it: a game's cover or its name, a collage
//! of covers for the tiles that open a list, and the title band a focused tile shows. Sizes are in the rows' units:
//! multiply by ROW_SCALE.
use model::Game;
use ui::{ArtKey, Color, Crop, Ease, HAlign, Kind, Rect, Ui, VAlign};

use crate::theme::{self, BODY, ROW_SCALE as S};

pub const RADIUS: f32 = 6.0;

/// A game's cover for a tile: its portrait cover, else its header, else its wide art.
pub fn cover_art(game: &Game, px: f32) -> Option<ArtKey> {
    let path = game.cover.as_ref().or(game.header.as_ref()).or(game.hero.as_ref())?;
    // Decoded 300 wide at 1080p, as the theme decodes it, and in proportion on a larger screen.
    let width = if px > 1.25 { 600 } else { 300 };
    Some(ArtKey { path: path.clone(), kind: Kind::Thumb(width) })
}

/// Where a game comes from: the store whose mark its tile carries. A source the library reads (docs/sources.md)
/// has its variant here, by its key, for its mark and the name the Game hub writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Steam,
    Epic,
    Gog,
    Ea,
    Ubisoft,
    BattleNet,
    Xbox,
}

impl Source {
    /// The store's name, as the Game hub writes it.
    pub fn name(self) -> &'static str {
        match self {
            Source::Steam => "Steam",
            Source::Epic => "Epic Games",
            Source::Gog => "GOG",
            Source::Ea => "EA app",
            Source::Ubisoft => "Ubisoft Connect",
            Source::BattleNet => "Battle.net",
            Source::Xbox => "Xbox",
        }
    }
}

/// The game's source, by `Game::source`; none for a store the screens have no mark for. A made-up game, of no
/// source, carries Steam's, as every game did before there were other sources.
pub fn source(game: &Game) -> Option<Source> {
    match game.source.as_str() {
        "steam" | "" => Some(Source::Steam),
        "epic" => Some(Source::Epic),
        "gog" => Some(Source::Gog),
        "ea" => Some(Source::Ea),
        "ubisoft" => Some(Source::Ubisoft),
        "battlenet" => Some(Source::BattleNet),
        "xbox" => Some(Source::Xbox),
        _ => None,
    }
}

/// A source's mark, filling `mark`. The marks are the stores' own logos, which are not this project's to ship:
/// until they are loaded from somewhere they may be, a plain white shape stands in for each, one unlike the
/// others'. This is the one place to draw the real thing: each store's own function below.
pub fn source_mark(ui: &mut Ui, mark: Rect, source: Source) {
    match source {
        Source::Steam => steam_mark(ui, mark),
        Source::Epic => epic_mark(ui, mark),
        Source::Gog => gog_mark(ui, mark),
        Source::Ea => ea_mark(ui, mark),
        Source::Ubisoft => ubisoft_mark(ui, mark),
        Source::BattleNet => battlenet_mark(ui, mark),
        Source::Xbox => xbox_mark(ui, mark),
    }
}

/// Steam's stand-in: a white disc.
fn steam_mark(ui: &mut Ui, mark: Rect) {
    ui.fill(mark, Color::WHITE, 999.0);
}

/// Epic's stand-in: a white square, its corners a little rounded, a little inside the disc's bounds so that it
/// looks no bigger.
fn epic_mark(ui: &mut Ui, mark: Rect) {
    ui.fill(mark.grow(-mark.w * 0.08), Color::WHITE, mark.w * 0.12);
}

/// GOG's stand-in: a white ring.
fn gog_mark(ui: &mut Ui, mark: Rect) {
    ui.ring(mark, Color::WHITE, 999.0, mark.w * 0.2);
}

/// The EA app's stand-in: the top half of a white disc, its flat side down.
fn ea_mark(ui: &mut Ui, mark: Rect) {
    let top = Rect::new(mark.x, mark.y + mark.h * 0.15, mark.w, mark.h * 0.5);
    let disc = Rect::new(mark.x, top.y, mark.w, mark.h);
    ui.with_clip(top, |ui| ui.fill(disc, Color::WHITE, 999.0));
}

/// Ubisoft Connect's stand-in: four small white squares, two by two, apart.
fn ubisoft_mark(ui: &mut Ui, mark: Rect) {
    let side = mark.w * 0.36;
    let gap = mark.w * 0.12;
    let left = mark.x + (mark.w - 2.0 * side - gap) / 2.0;
    let top = mark.y + (mark.h - 2.0 * side - gap) / 2.0;
    for (column, row) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
        let cell = Rect::new(left + column * (side + gap), top + row * (side + gap), side, side);
        ui.fill(cell, Color::WHITE, side * 0.2);
    }
}

/// Battle.net's stand-in: a white bar across the middle, half as tall as it is wide.
fn battlenet_mark(ui: &mut Ui, mark: Rect) {
    let bar = Rect::new(mark.x, mark.y + mark.h * 0.25, mark.w, mark.h * 0.5);
    ui.fill(bar, Color::WHITE, mark.h * 0.25);
}

/// Xbox's stand-in: a white plus, two bars a third as thick as the mark is wide, crossed.
fn xbox_mark(ui: &mut Ui, mark: Rect) {
    let thick = mark.w * 0.34;
    let across = Rect::new(mark.x, mark.y + (mark.h - thick) / 2.0, mark.w, thick);
    let down = Rect::new(mark.x + (mark.w - thick) / 2.0, mark.y, thick, mark.h);
    ui.fill(across, Color::WHITE, thick * 0.2);
    ui.fill(down, Color::WHITE, thick * 0.2);
}

/// The source's badge at the tile's bottom left: its mark, 20 across, on a soft shadow.
pub fn source_badge(ui: &mut Ui, tile: Rect, source: Source) {
    source_badge_at(ui, tile, source, S);
}

/// The source's badge at another scale than the rows': 1 on My games & apps, where nothing is scaled.
pub fn source_badge_at(ui: &mut Ui, tile: Rect, source: Source, scale: f32) {
    let side = 20.0 * scale;
    let mark = Rect::new(tile.x + 10.0 * scale, tile.bottom() - 10.0 * scale - side, side, side);
    // The theme's shadow: 0.8 black, blurred by 6, straight under the mark.
    let shadow = mark.grow(3.0 * scale);
    ui.glow(shadow, Color::BLACK.fade(0.8), 999.0, shadow.w / 2.0, 6.0 * scale);
    source_mark(ui, mark, source);
}

/// A game's face: its cover filling the tile from the top, or for want of one its name on the dark tile, set
/// `name_width` wide so that it doesn't reflow as a tile grows; and its source's badge, which a focused tile
/// hides for its title.
pub fn game(ui: &mut Ui, rect: Rect, game: &Game, name_width: f32, focused: bool) {
    let radius = RADIUS * S;
    ui.fill(rect, theme::TILE, radius);
    let has_cover = cover_art(game, ui.px()).is_some_and(|key| ui.art(rect, &key, Crop::Top, 1.0, radius));
    if !has_cover {
        let width = name_width - 20.0 * S;
        let lines = Rect::new(rect.x + (rect.w - width) / 2.0, rect.y + 10.0 * S, width, rect.h - 20.0 * S);
        ui.paragraph(lines, &game.name, &BODY, HAlign::Center, VAlign::Center, 0);
    }
    if let Some(source) = source(game).filter(|_| !focused) {
        source_badge(ui, rect, source);
    }
}

/// The face of a tile that opens a list of games: up to four of their covers, two by two, each filled from the
/// top; a game with no cover leaves its place dark. With no games at all, `glyph`.
pub fn collage<'a>(ui: &mut Ui, rect: Rect, games: impl Iterator<Item = &'a Game>, glyph: char) {
    ui.fill(rect, theme::TILE, RADIUS * S);
    let (inset, margin) = (4.0 * S, 3.0 * S);
    let (w, h) = ((rect.w - 2.0 * inset) / 2.0, (rect.h - 2.0 * inset) / 2.0);
    let mut any = false;
    for (i, game) in games.take(4).enumerate() {
        any = true;
        let cell = Rect::new(rect.x + inset + (i % 2) as f32 * w, rect.y + inset + (i / 2) as f32 * h, w, h);
        if let Some(key) = cover_art(game, ui.px()) {
            ui.art(cell.grow(-margin), &key, Crop::Top, 1.0, 0.0);
        }
    }
    if !any {
        let (cx, cy) = rect.center();
        ui.icon(cx, cy, glyph, 40.0 * S, Color::WHITE);
    }
}

/// The band along a focused tile's foot with its title on it, at most two lines: in over 150 ms after 50, out
/// over 100. The text is laid out `width` wide (the tile's width at its largest) and cut to the tile as it is.
pub fn title_band(ui: &mut Ui, rect: Rect, id: impl std::hash::Hash, title: &str, focused: bool, width: f32) {
    // Linear both ways. The first quarter of the value's way is the 50 ms wait, with nothing showing; the way out
    // crosses the same quarter unseen, after its 100 ms.
    let (to, seconds) = if focused { (1.0, 0.2) } else { (0.0, 0.1 / 0.75) };
    let band = (ui.tween((id, "band"), to, seconds, Ease::Linear) - 0.25) / 0.75;
    if band <= 0.0 {
        return;
    }
    let width = width - 16.0 * S;
    let text = ui.measure_wrapped(&BODY, title, width, 2);
    let height = text.height + 16.0 * S;
    ui.with_alpha(band, |ui| {
        // The tile's own shape, cut to the band: square across the top, the tile's corners at the foot.
        ui.with_clip(Rect::new(rect.x, rect.bottom() - height, rect.w, height), |ui| {
            ui.fill(rect, theme::BAND, RADIUS * S);
            let lines = Rect::new(rect.x + 8.0 * S, rect.bottom() - height + 8.0 * S, width, text.height);
            ui.paragraph(lines, title, &BODY, HAlign::Left, VAlign::Top, 2);
        });
    });
}
