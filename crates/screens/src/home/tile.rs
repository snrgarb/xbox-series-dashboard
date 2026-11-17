//! A tile's face, as the Recent row's tiles and the Groups' cards share it: a game's cover or its name, a collage
//! of covers for the tiles that open a list, and the title band a focused tile shows. Sizes are in the rows' units:
//! multiply by ROW_SCALE.
use model::Game;
use ui::{ArtKey, Color, Crop, Ease, HAlign, Kind, Rect, Ui, VAlign};

use crate::drawn::{self, Drawing};
use crate::theme::{self, BODY, ROW_SCALE as S};

pub const RADIUS: f32 = 6.0;

/// A game's cover for a portrait tile: its portrait cover, else its square art (nearer the shape than the rest),
/// else its header, else its wide art.
pub fn cover_art(game: &Game, px: f32) -> Option<ArtKey> {
    let path = game.cover.as_ref().or(game.square.as_ref()).or(game.header.as_ref()).or(game.hero.as_ref())?;
    Some(thumb(path, px))
}

/// A game's face for a square tile (Home's Recent row, docs/spec/home.md 3.3): its square art, else what
/// `cover_art` gives, cropped as before.
pub fn square_art(game: &Game, px: f32) -> Option<ArtKey> {
    match &game.square {
        Some(path) => Some(thumb(path, px)),
        None => cover_art(game, px),
    }
}

/// A game's face for a tile of this shape: `square_art` for one as wide as it is tall (within a tenth), else
/// `cover_art`.
pub fn art_for(game: &Game, rect: Rect, px: f32) -> Option<ArtKey> {
    if is_square(rect.w, rect.h) {
        square_art(game, px)
    } else {
        cover_art(game, px)
    }
}

/// As wide as it is tall, within a tenth: a square tile, or a square picture.
pub fn is_square(w: f32, h: f32) -> bool {
    (w - h).abs() <= 0.1 * w.max(h)
}

/// How a picture goes on a tile (docs/spec/home.md 3.3): filling it, cropped from the top, unless the tile is square
/// and the picture is not. Then it is shown whole, as large as fits and in the middle, over itself blurred and
/// darkened filling the tile, so that a square tile never shows a part of a portrait cover. Decided by the picture
/// as it is decoded, not by where it came from: a picture kept as square that is not is shown whole too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Fill,
    Whole,
}

/// How a picture of this size goes on a tile of this size.
pub fn fit(tile: (f32, f32), picture: (u32, u32)) -> Fit {
    if is_square(tile.0, tile.1) && !is_square(picture.0 as f32, picture.1 as f32) {
        Fit::Whole
    } else {
        Fit::Fill
    }
}

/// How much of the blurred picture's own colour is left under a picture shown whole.
const BEHIND_LIGHT: f32 = 0.55;

/// Draws a game's picture on a tile as [`fit`] says, once it is decoded; false until then.
pub fn face(ui: &mut Ui, rect: Rect, key: &ArtKey, alpha: f32, radius: f32) -> bool {
    let ui::Art::Ready { width, height, .. } = ui.art.get(key) else { return false };
    match fit((rect.w, rect.h), (width, height)) {
        Fit::Fill => ui.art(rect, key, Crop::Top, alpha, radius),
        Fit::Whole => {
            let behind = ArtKey { path: key.path.clone(), kind: Kind::Blurred };
            if ui.art(rect, &behind, Crop::Center, alpha, radius) {
                ui.fill(rect, Color::BLACK.fade(alpha * (1.0 - BEHIND_LIGHT)), radius);
            }
            ui.art_fitted(rect, key, alpha, 1.0).is_some()
        }
    }
}

/// As [`face`], for a picture that may come while the tile is on screen: `ui.art_arriving`'s fade, by `id`. Gives
/// how much of it shows.
pub fn face_arriving(ui: &mut Ui, id: impl std::hash::Hash, rect: Rect, key: Option<&ArtKey>, radius: f32) -> f32 {
    let target = if key.is_some() { 1.0 } else { 0.0 };
    let shown = ui.tween(("art arriving", id), target, ui::ART_FADE_SECONDS, Ease::Linear);
    match key {
        Some(key) if shown > 0.0 && face(ui, rect, key, shown, radius) => shown,
        _ => 0.0,
    }
}

/// A picture decoded 300 wide at 1080p, as the theme decodes a cover, and in proportion on a larger screen.
fn thumb(path: &std::path::Path, px: f32) -> ArtKey {
    let width = if px > 1.25 { 600 } else { 300 };
    ArtKey { path: path.to_path_buf(), kind: Kind::Thumb(width) }
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

impl Source {
    /// The project's own drawing that marks the store (docs/spec/home.md 3.3.1).
    pub fn drawing(self) -> Drawing {
        match self {
            Source::Steam => Drawing::Steam,
            Source::Epic => Drawing::Epic,
            Source::Gog => Drawing::Gog,
            Source::Ea => Drawing::Ea,
            Source::Ubisoft => Drawing::Ubisoft,
            Source::BattleNet => Drawing::BattleNet,
            Source::Xbox => Drawing::Xbox,
        }
    }
}

/// The drawing a game is marked with: its store's, a folder for a game added by hand, and a plain box for a
/// source with no drawing of its own.
pub fn game_mark(game: &Game) -> Drawing {
    match source(game) {
        Some(source) => source.drawing(),
        None if game.source == model::by_hand::MANUAL => Drawing::Manual,
        None => Drawing::Launcher,
    }
}

/// A source's mark, filling `mark`, in white: the project's own drawing of it, never the store's logo.
pub fn source_mark(ui: &mut Ui, mark: Rect, source: Source) {
    mark_drawn(ui, mark, source.drawing());
}

/// A mark, filling `mark`, in white.
pub fn mark_drawn(ui: &mut Ui, mark: Rect, drawing: Drawing) {
    drawn::draw(ui, mark, drawing, Color::WHITE);
}

/// The badge at the tile's bottom left: its mark, 20 across, on a soft shadow, at `scale` (the rows', or 1 on My
/// games & apps and Manage Queue, where nothing is scaled).
pub fn source_badge_at(ui: &mut Ui, tile: Rect, mark_of: Drawing, scale: f32) {
    let side = 20.0 * scale;
    let mark = Rect::new(tile.x + 10.0 * scale, tile.bottom() - 10.0 * scale - side, side, side);
    // The theme's shadow: 0.8 black, blurred by 6, straight under the mark.
    let shadow = mark.grow(3.0 * scale);
    ui.glow(shadow, Color::BLACK.fade(0.8), 999.0, shadow.w / 2.0, 6.0 * scale);
    mark_drawn(ui, mark, mark_of);
}

/// A game's face: its square art on a square tile (a Recent tile), else its cover filling the tile from the top, or
/// for want of either its name on the dark tile, set `name_width` wide so that it doesn't reflow as a tile grows;
/// and its source's badge, which a focused tile hides for its title.
pub fn game(ui: &mut Ui, rect: Rect, game: &Game, name_width: f32, focused: bool) {
    game_at(ui, rect, game, name_width, focused, S);
}

/// [`game`] at another scale than the rows': 1 on Manage Queue, where nothing is scaled.
pub fn game_at(ui: &mut Ui, rect: Rect, game: &Game, name_width: f32, focused: bool, scale: f32) {
    let s = scale;
    let radius = RADIUS * s;
    ui.fill(rect, theme::TILE, radius);
    // A cover downloaded while the Dashboard runs fades in over the name.
    let key = art_for(game, rect, ui.px());
    let cover = face_arriving(ui, (&game.source, &game.source_id, &game.name), rect, key.as_ref(), radius);
    if cover < 1.0 {
        let width = name_width - 20.0 * s;
        let lines = Rect::new(rect.x + (rect.w - width) / 2.0, rect.y + 10.0 * s, width, rect.h - 20.0 * s);
        ui.with_alpha(1.0 - cover, |ui| ui.paragraph(lines, &game.name, &BODY, HAlign::Center, VAlign::Center, 0));
    }
    if !focused {
        source_badge_at(ui, rect, game_mark(game), s);
    }
}

/// The face of a tile that opens a list of games: up to four of their covers, two by two, each filled from the
/// top (a square cell, the Play history tile's, has a game's square art); a game with no cover leaves its place
/// dark. With no games at all, `glyph`.
pub fn collage<'a>(ui: &mut Ui, rect: Rect, games: impl Iterator<Item = &'a Game>, glyph: char) {
    ui.fill(rect, theme::TILE, RADIUS * S);
    let (inset, margin) = (4.0 * S, 3.0 * S);
    let (w, h) = ((rect.w - 2.0 * inset) / 2.0, (rect.h - 2.0 * inset) / 2.0);
    let mut any = false;
    for (i, game) in games.take(4).enumerate() {
        any = true;
        let cell = Rect::new(rect.x + inset + (i % 2) as f32 * w, rect.y + inset + (i / 2) as f32 * h, w, h);
        let cell = cell.grow(-margin);
        if let Some(key) = art_for(game, cell, ui.px()) {
            face(ui, cell, &key, 1.0, 0.0);
        }
    }
    if !any {
        let (cx, cy) = rect.center();
        ui.icon(cx, cy, glyph, 40.0 * S, Color::WHITE);
    }
}

/// The caption at an unfocused tile's foot while its game is in the Queue: its state, Queued or Installing..., on
/// the title band's black, one line, cut with an ellipsis (the Bundle's HomeTiles.xaml StatusBand). A focused tile
/// shows it under its title instead (`title_band`), and none of it here at once, as the Bundle hides it.
pub fn status_band(ui: &mut Ui, rect: Rect, status: Option<&str>, focused: bool) {
    status_band_at(ui, rect, status, focused, S);
}

/// [`status_band`] at another scale than the rows'.
pub fn status_band_at(ui: &mut Ui, rect: Rect, status: Option<&str>, focused: bool, s: f32) {
    let Some(status) = status.filter(|_| !focused) else { return };
    let line = ui.measure_wrapped(&BODY, status, rect.w - 16.0 * s, 1).height;
    let height = line + 16.0 * s;
    ui.with_clip(Rect::new(rect.x, rect.bottom() - height, rect.w, height), |ui| {
        ui.fill(rect, theme::BAND, RADIUS * s);
        let text = Rect::new(rect.x + 8.0 * s, rect.bottom() - height + 8.0 * s, rect.w - 16.0 * s, line);
        ui.paragraph(text, status, &BODY, HAlign::Left, VAlign::Top, 1);
    });
}

/// The band along a focused tile's foot with its title on it, at most two lines, and under it the game's state in
/// the Queue if it is in it, one line: in over 150 ms after 50, out over 100. The text is laid out `width` wide
/// (the tile's width at its largest) and cut to the tile as it is.
pub fn title_band(
    ui: &mut Ui,
    rect: Rect,
    id: impl std::hash::Hash,
    title: &str,
    status: Option<&str>,
    focused: bool,
    width: f32,
) {
    title_band_at(ui, rect, id, title, status, focused, width, S);
}

/// [`title_band`] at another scale than the rows'.
#[allow(clippy::too_many_arguments)]
pub fn title_band_at(
    ui: &mut Ui,
    rect: Rect,
    id: impl std::hash::Hash,
    title: &str,
    status: Option<&str>,
    focused: bool,
    width: f32,
    s: f32,
) {
    // Linear both ways. The first quarter of the value's way is the 50 ms wait, with nothing showing; the way out
    // crosses the same quarter unseen, after its 100 ms.
    let (to, seconds) = if focused { (1.0, 0.2) } else { (0.0, 0.1 / 0.75) };
    let band = (ui.tween((id, "band"), to, seconds, Ease::Linear) - 0.25) / 0.75;
    if band <= 0.0 {
        return;
    }
    let width = width - 16.0 * s;
    let text = ui.measure_wrapped(&BODY, title, width, 2);
    let under = status.map_or(0.0, |status| ui.measure_wrapped(&BODY, status, width, 1).height);
    let height = text.height + under + 16.0 * s;
    ui.with_alpha(band, |ui| {
        // The tile's own shape, cut to the band: square across the top, the tile's corners at the foot.
        ui.with_clip(Rect::new(rect.x, rect.bottom() - height, rect.w, height), |ui| {
            ui.fill(rect, theme::BAND, RADIUS * s);
            let top = rect.bottom() - height + 8.0 * s;
            ui.paragraph(
                Rect::new(rect.x + 8.0 * s, top, width, text.height),
                title,
                &BODY,
                HAlign::Left,
                VAlign::Top,
                2,
            );
            if let Some(status) = status {
                let line = Rect::new(rect.x + 8.0 * s, top + text.height, width, under);
                ui.paragraph(line, status, &BODY, HAlign::Left, VAlign::Top, 1);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up game with the pictures named.
    fn game(cover: bool, header: bool, square: bool) -> Game {
        let path = |on: bool, name: &str| on.then(|| std::path::PathBuf::from(format!("art/1/{name}.png")));
        Game {
            name: "Made-up Game".into(),
            source: "steam".into(),
            source_id: "1".into(),
            cover: path(cover, "cover"),
            header: path(header, "header"),
            square: path(square, "square"),
            ..Game::default()
        }
    }

    fn picked(game: &Game, rect: Rect) -> Option<String> {
        art_for(game, rect, 1.0).map(|key| key.path.file_stem().unwrap().to_string_lossy().into_owned())
    }

    fn square_tile() -> Rect {
        Rect::new(0.0, 0.0, 245.0, 245.0)
    }

    fn portrait_card() -> Rect {
        Rect::new(0.0, 0.0, 194.0, 296.0)
    }

    #[test]
    fn a_square_tile_takes_the_square_picture_wherever_the_game_has_one() {
        assert_eq!(picked(&game(true, true, true), square_tile()).as_deref(), Some("square"));
        assert_eq!(picked(&game(false, false, true), square_tile()).as_deref(), Some("square"));
        // Without one: the cover, then the header.
        assert_eq!(picked(&game(true, true, false), square_tile()).as_deref(), Some("cover"));
        assert_eq!(picked(&game(false, true, false), square_tile()).as_deref(), Some("header"));
        assert_eq!(picked(&game(false, false, false), square_tile()), None);
        // A portrait card keeps the cover, and takes the square only for want of one.
        assert_eq!(picked(&game(true, true, true), portrait_card()).as_deref(), Some("cover"));
        assert_eq!(picked(&game(false, false, true), portrait_card()).as_deref(), Some("square"));
    }

    #[test]
    fn a_square_tile_never_crops_a_picture_that_is_not_square() {
        let tile = (245.0, 245.0);
        assert_eq!(fit(tile, (512, 512)), Fit::Fill);
        assert_eq!(fit(tile, (1024, 1000)), Fit::Fill);
        // A portrait cover, a header and a background are shown whole.
        assert_eq!(fit(tile, (600, 900)), Fit::Whole);
        assert_eq!(fit(tile, (300, 450)), Fit::Whole);
        assert_eq!(fit(tile, (920, 430)), Fit::Whole);
        assert_eq!(fit(tile, (3840, 1240)), Fit::Whole);
        // A picture kept as square that is not is judged by its own size.
        assert_eq!(fit(tile, (600, 800)), Fit::Whole);
        // A tile that is not square is filled whatever the picture, as the Bundle fills it.
        assert_eq!(fit((194.0, 296.0), (600, 900)), Fit::Fill);
        assert_eq!(fit((194.0, 296.0), (512, 512)), Fit::Fill);
        assert_eq!(fit((400.0, 211.0), (600, 900)), Fit::Fill);
    }
}
