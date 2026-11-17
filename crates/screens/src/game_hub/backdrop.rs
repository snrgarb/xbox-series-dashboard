//! What the hub lies on (docs/spec/game-hub.md, section 2): the game's art, blurred, over the whole screen and
//! darkened; the screen the hub was opened over in a strip above and a strip below; and the band between them.
use std::path::PathBuf;

use model::Game;
use ui::art::RgbaImage;
use ui::{ArtKey, Color, Crop, Kind, Painter, Rect, Snapshot, Ui};

/// The band leaves this much of the screen above it and below it.
pub const STRIP: f32 = 88.0;
const BAND: Color = Color::argb(0xB3000000);
/// What darkens the screen behind in the strips.
const STRIP_SHADE: Color = Color::argb(0x99000000);
/// The painted pictures' size: the art's own, a fifteenth of the screen.
const SMALL: (u32, u32) = (128, 72);

/// The game's art for behind the band, and how far past the screen it is drawn: its background art, blurred by 2
/// at a fifteenth of the screen; else its cover, blurred by 4 and drawn 1.15 times the screen's size so that the
/// blur's soft edges fall off it.
pub(crate) fn art(game: &Game) -> Option<(ArtKey, f32)> {
    let soft = |path: &PathBuf, radius| ArtKey { path: path.clone(), kind: Kind::Soft(radius) };
    match (&game.hero, game.cover.as_ref().or(game.header.as_ref())) {
        (Some(hero), _) => Some((soft(hero, 2), 1.0)),
        (None, Some(cover)) => Some((soft(cover, 4), 1.15)),
        (None, None) => None,
    }
}

/// A colour between the stops of a gradient, `at` along it.
fn between(stops: &[(f32, [f32; 4])], at: f32) -> [u8; 4] {
    let after = stops.iter().position(|&(stop, _)| stop >= at).unwrap_or(stops.len() - 1).max(1);
    let ((from, a), (to, b)) = (stops[after - 1], stops[after]);
    let along = ((at - from) / (to - from)).clamp(0.0, 1.0);
    std::array::from_fn(|channel| (a[channel] + (b[channel] - a[channel]) * along).round() as u8)
}

/// A picture of a gradient: `along` says how far along it the point (x, y) of a box 1 by 1 is.
fn painted(stops: &[(f32, [f32; 4])], along: impl Fn(f32, f32) -> f32) -> RgbaImage {
    RgbaImage::from_fn(SMALL.0, SMALL.1, |x, y| {
        let (x, y) = ((x as f32 + 0.5) / SMALL.0 as f32, (y as f32 + 0.5) / SMALL.1 as f32);
        between(stops, along(x, y)).into()
    })
}

/// The darkening over the art: black from the screen's top left corner to its bottom right, 80% there, 40% a
/// little past the middle and 60% at the end. The renderer's gradients have two stops, so this one is a picture.
fn darkening() -> RgbaImage {
    let black = |alpha: f32| [0.0, 0.0, 0.0, alpha];
    painted(&[(0.0, black(204.0)), (0.55, black(102.0)), (1.0, black(153.0))], |x, y| (x + y) / 2.0)
}

/// The theme's own background, which shows for a game with no art at all: green about a point near the screen's
/// bottom left corner, black by its top right.
fn green() -> RgbaImage {
    let stops = [(0.0, [11.0, 74.0, 11.0, 255.0]), (0.55, [4.0, 26.0, 4.0, 255.0]), (1.0, [0.0, 0.0, 0.0, 255.0])];
    painted(&stops, |x, y| ((x - 0.1) / 1.1).hypot((y - 1.0) / 1.3))
}

fn picture(name: &str, paint: fn() -> RgbaImage) -> ArtKey {
    ArtKey { path: PathBuf::from(name), kind: Kind::Painted(Painter(paint)) }
}

/// The band's place on this screen.
pub fn band(ui: &Ui) -> Rect {
    let screen = ui.screen();
    Rect::new(screen.x, STRIP, screen.w, screen.h - 2.0 * STRIP)
}

/// Fills the screen: the art, or the green for want of any; the strips, which show `behind`, the screen the hub
/// was opened over, where there is a picture of it and the art where there is none; the band.
pub fn draw(ui: &mut Ui, game: &Game, behind: Option<Snapshot>) {
    let screen = ui.screen();
    ui.fill(screen, Color::BLACK, 0.0);
    match art(game) {
        Some((key, scale)) => {
            let (cx, cy) = screen.center();
            let (w, h) = (screen.w * scale, screen.h * scale);
            // Nothing shows until the art is read, and then it is there: it does not fade in.
            if ui.art(Rect::new(cx - w / 2.0, cy - h / 2.0, w, h), &key, Crop::Center, 1.0, 0.0) {
                ui.art_turned(screen, &picture("game hub/darkening", darkening), 1.0, 0.0);
            }
        }
        None => {
            ui.art_turned(screen, &picture("game hub/green", green), 1.0, 0.0);
        }
    }
    let band = band(ui);
    if let Some(behind) = behind {
        for y in [screen.y, band.bottom()] {
            let strip = Rect::new(screen.x, y, screen.w, STRIP);
            ui.with_clip(strip, |ui| ui.image(screen, behind.tex, behind.size, Crop::Center, 1.0, 0.0));
            ui.fill(strip, STRIP_SHADE, 0.0);
        }
    }
    ui.fill(band, BAND, 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_darkening_runs_corner_to_corner_through_its_three_stops() {
        let picture = darkening();
        let alpha = |x: u32, y: u32| picture.get_pixel(x, y)[3] as i32;
        assert!((alpha(0, 0) - 203).abs() <= 1, "80% at the top left: {}", alpha(0, 0));
        assert!((alpha(127, 71) - 152).abs() <= 1, "60% at the bottom right: {}", alpha(127, 71));
        // 55% of the way: the lightest there is, on the diagonal that crosses the middle a little past it.
        assert!((alpha(70, 40) - 102).abs() <= 2, "40% past the middle: {}", alpha(70, 40));
        assert!((alpha(127, 0) - alpha(0, 71)).abs() <= 1, "the same along the other diagonal");
        assert!(picture.pixels().all(|pixel| pixel.0[..3] == [0, 0, 0]));
    }

    #[test]
    fn the_green_is_brightest_by_the_bottom_left_corner() {
        let picture = green();
        let corner = picture.get_pixel(12, 71).0;
        assert!(corner[0] == 11 && corner[1] >= 73 && corner[2] == 11 && corner[3] == 255, "{corner:?}");
        assert_eq!(picture.get_pixel(127, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn the_art_is_the_background_or_the_cover_overhanging() {
        let mut game = Game::default();
        assert!(art(&game).is_none());
        game.header = Some("header.jpg".into());
        assert_eq!(art(&game), Some((ArtKey { path: "header.jpg".into(), kind: Kind::Soft(4) }, 1.15)));
        game.cover = Some("cover.jpg".into());
        assert_eq!(art(&game).unwrap().0.path, PathBuf::from("cover.jpg"));
        game.hero = Some("hero.jpg".into());
        assert_eq!(art(&game), Some((ArtKey { path: "hero.jpg".into(), kind: Kind::Soft(2) }, 1.0)));
    }
}
