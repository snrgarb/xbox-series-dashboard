//! What every screen that shows achievements draws alike (docs/achievements.md): a Score written as the Bundle
//! writes it, the console's G disc, an achievement's icon with the Bundle's stand-in for one not fetched, and the
//! hub's progress bar.
use model::achievement::Achievement;
use ui::{ArtKey, Color, Crop, Font, Kind, Painter, Rect, TextStyle, Ui};

use crate::theme::{glyph, ACCENT_LIGHT};

/// A number with thousands separators, as WPF's `{0:N0}` writes it in English: "12,345".
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// How far a game has got, as a whole percentage rounded as WPF's `{0:0}` rounds it (halves away from 0).
pub fn percent(unlocked: u32, of: u32) -> u32 {
    if of == 0 {
        return 0;
    }
    (f64::from(unlocked) * 100.0 / f64::from(of)).round() as u32
}

/// How rare an achievement is, as a share of players: "7.4%", "82.5%", "0.8%", one decimal.
pub fn rarity(rarity: f64) -> String {
    format!("{rarity:.1}%")
}

/// The console's Score mark: a white disc `size` across with a black bold G in it (Home's top bar's is 19 with a
/// G of 13), its middle at (`cx`, `cy`). `knocked` is the G's colour: black, or the colour of what it lies on.
pub fn g_disc(ui: &mut Ui, cx: f32, cy: f32, size: f32, knocked: Color) {
    ui.fill(Rect::new(cx - size / 2.0, cy - size / 2.0, size, size), Color::WHITE, 999.0);
    let style = TextStyle::new(Font::Ui, size * 13.0 / 19.0, 700).color(knocked);
    ui.label_centered(cx, cy, "G", &style);
}

/// An achievement's icon in `rect`: its own, unlocked or locked as it is, once fetched; until then, or for a secret
/// one still locked, the Bundle's stand-in, a grey square with the trophy (OverlayWindow.xaml, AchievementIcon).
pub fn icon(ui: &mut Ui, rect: Rect, achievement: &Achievement, radius: f32) {
    ui.fill(rect, Color::rgb(0x4A4A4A), radius);
    let file = if achievement.is_unlocked() {
        achievement.icon_file.as_ref()
    } else if achievement.hidden {
        None
    } else {
        achievement.icon_locked_file.as_ref()
    };
    let width = (rect.w * ui.px()).ceil().max(1.0) as u32;
    let drawn = file.is_some_and(|path| {
        let key = ArtKey { path: path.clone(), kind: Kind::Thumb(width.next_power_of_two().min(256)) };
        ui.art(rect, &key, Crop::Center, 1.0, radius)
    });
    if !drawn {
        let (cx, cy) = rect.center();
        ui.icon(cx, cy, glyph::TROPHY, rect.h * 26.0 / 60.0, Color::argb(0xB3FFFFFF));
    }
}

/// The hub's progress bar: `share` of a 6-tall track filled in the accent from the left, both rounded by 3
/// (GameDetails.xaml, XboxHubProgressBar).
pub fn bar(ui: &mut Ui, rect: Rect, share: f32) {
    ui.fill(rect, Color::argb(0x33FFFFFF), rect.h / 2.0);
    let done = rect.w * share.clamp(0.0, 1.0);
    if done > 0.0 {
        ui.fill(Rect::new(rect.x, rect.y, done, rect.h), ACCENT_LIGHT, rect.h / 2.0);
    }
}

/// The Rare mark, 44 x 38: an outlined diamond with its facets, white, a 3 line with round joins
/// (IdleCardControl.xaml's `Rare` path), drawn once at twice its size.
pub fn rare_diamond(ui: &mut Ui, x: f32, cy: f32) {
    let key = ArtKey { path: "achievement/rare".into(), kind: Kind::Painted(Painter(paint_diamond)) };
    ui.art(Rect::new(x, cy - 19.0, 44.0, 38.0), &key, Crop::Center, 1.0, 0.0);
}

fn paint_diamond() -> ui::art::RgbaImage {
    use tiny_skia as sk;
    const SCALE: f32 = 2.0;
    let mut pixmap = sk::Pixmap::new(88, 76).expect("the diamond has a size");
    let mut path = sk::PathBuilder::new();
    let points = |path: &mut sk::PathBuilder, points: &[(f32, f32)]| {
        path.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            path.line_to(x, y);
        }
    };
    points(&mut path, &[(10.0, 2.0), (34.0, 2.0), (42.0, 13.0), (22.0, 36.0), (2.0, 13.0)]);
    path.close();
    points(&mut path, &[(2.0, 13.0), (42.0, 13.0)]);
    points(&mut path, &[(15.0, 2.0), (12.0, 13.0), (22.0, 36.0), (32.0, 13.0), (29.0, 2.0)]);
    let path = path.finish().expect("the diamond is a path");
    let mut paint = sk::Paint { anti_alias: true, ..Default::default() };
    paint.set_color(sk::Color::WHITE);
    let stroke = sk::Stroke { width: 3.0, line_join: sk::LineJoin::Round, ..Default::default() };
    pixmap.stroke_path(&path, &paint, &stroke, sk::Transform::from_scale(SCALE, SCALE), None);
    let mut image = ui::art::RgbaImage::new(88, 76);
    for (to, from) in image.pixels_mut().zip(pixmap.pixels()) {
        let from = from.demultiply();
        // White throughout, so the sampler mixes no black into the line's edge.
        to.0 = [255, 255, 255, from.alpha()];
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_have_thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(95), "95");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(12_345), "12,345");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn a_percentage_is_whole_and_rounds_halves_up() {
        assert_eq!(percent(2, 6), 33);
        assert_eq!(percent(3, 5), 60);
        assert_eq!(percent(1, 8), 13);
        assert_eq!(percent(0, 0), 0);
        assert_eq!(percent(6, 6), 100);
    }
}
