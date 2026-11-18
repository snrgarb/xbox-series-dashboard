//! Pictures the Dashboard draws itself from vector paths, in one colour: the launchers' marks (docs/spec/home.md,
//! 3.3.1), the Xbox sphere the Guide and the Toasts carry and Discord's mark (docs/spec/guide.md, 10.1), and the few
//! the Dashboard has of its own. The stores', the console's and Discord's are their owners' marks, as the Bundle ships
//! them: its own paths for Steam, the Xbox sphere and Discord, Simple Icons' for the other stores
//! (`crate::path_data::marks`; NOTICE.md says whose each is). A drawing is laid out on a 24 x 24 grid, as an icon font's glyph is, painted with tiny-skia once per
//! size it is asked at (a picture a little larger than it is shown, so it is never stretched up), in white, and drawn
//! in whatever colour it is asked for.
use tiny_skia as sk;
use ui::art::RgbaImage;
use ui::{ArtKey, Color, Kind, Painter, Rect, Ui};

use crate::path_data::{marks, parse};

/// Every drawing, by what it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Drawing {
    /// Steam's mark.
    Steam,
    /// Epic Games' mark.
    Epic,
    /// GOG's mark.
    Gog,
    /// Battle.net's mark.
    BattleNet,
    /// EA's mark, for the EA app.
    Ea,
    /// Ubisoft's mark, for Ubisoft Connect.
    Ubisoft,
    /// The Xbox sphere: the Xbox and Microsoft Store launcher's mark, and the Guide button's wherever the Bundle
    /// shows it (the Guide's landing tab and hint, the Toasts' prompt and the achievement's circle).
    Xbox,
    /// A game added by hand: a folder with a plus cut out of it.
    Manual,
    /// An App: a window, its title bar solid.
    App,
    /// A launcher of no drawing of its own: a box, as a cube.
    Launcher,
    /// Discord's mark: the Discord tile and tab, the account, a server or person with no picture, a Call.
    Discord,
}

impl Drawing {
    pub const ALL: [Drawing; 11] = [
        Drawing::Steam,
        Drawing::Epic,
        Drawing::Gog,
        Drawing::BattleNet,
        Drawing::Ea,
        Drawing::Ubisoft,
        Drawing::Xbox,
        Drawing::Manual,
        Drawing::App,
        Drawing::Launcher,
        Drawing::Discord,
    ];

    /// Its name, which is the painted picture's.
    pub fn name(self) -> &'static str {
        match self {
            Drawing::Steam => "steam",
            Drawing::Epic => "epic",
            Drawing::Gog => "gog",
            Drawing::BattleNet => "battlenet",
            Drawing::Ea => "ea",
            Drawing::Ubisoft => "ubisoft",
            Drawing::Xbox => "xbox",
            Drawing::Manual => "manual",
            Drawing::App => "app",
            Drawing::Launcher => "launcher",
            Drawing::Discord => "discord",
        }
    }

    /// A store's or the console's mark: its path data on the 24 grid.
    fn mark(self) -> Option<&'static str> {
        Some(match self {
            Drawing::Steam => marks::STEAM,
            Drawing::Epic => marks::EPIC_GAMES,
            Drawing::Gog => marks::GOG,
            Drawing::BattleNet => marks::BATTLE_NET,
            Drawing::Ea => marks::EA,
            Drawing::Ubisoft => marks::UBISOFT,
            Drawing::Xbox => marks::XBOX,
            Drawing::Discord => marks::DISCORD,
            _ => return None,
        })
    }
}

/// The sizes a drawing is painted at, in pixels across: the smallest at least as large as it is shown.
const SIZES: [u32; 4] = [32, 64, 128, 256];

/// Draws `drawing` filling `rect` (a square: the grid is), in `color`, once it is painted; false until then.
pub fn draw(ui: &mut Ui, rect: Rect, drawing: Drawing, color: Color) -> bool {
    let pixels = rect.w.max(rect.h) * ui.px();
    let size = SIZES.iter().position(|&size| size as f32 >= pixels).unwrap_or(SIZES.len() - 1);
    let path = format!("drawn/{}/{}", drawing.name(), SIZES[size]);
    let key = ArtKey { path: path.into(), kind: Kind::Painted(painter(drawing, size)) };
    ui.art_tinted(rect, &key, color)
}

/// A painter is a function of nothing: one for each drawing at each size.
fn paint_one<const DRAWING: u8, const SIZE: usize>() -> RgbaImage {
    paint(Drawing::ALL[DRAWING as usize], SIZES[SIZE])
}

fn painter(drawing: Drawing, size: usize) -> Painter {
    macro_rules! table {
        ($($d:literal)*) => {
            match (drawing as u8, size) {
                $(
                    ($d, 0) => Painter(paint_one::<$d, 0>),
                    ($d, 1) => Painter(paint_one::<$d, 1>),
                    ($d, 2) => Painter(paint_one::<$d, 2>),
                    ($d, _) => Painter(paint_one::<$d, 3>),
                )*
                _ => unreachable!("every drawing is in the table"),
            }
        };
    }
    table!(0 1 2 3 4 5 6 7 8 9 10)
}

/// The drawing, `size` pixels square: white, its shape in the alpha.
pub fn paint(drawing: Drawing, size: u32) -> RgbaImage {
    let mut canvas = Canvas {
        pixmap: sk::Pixmap::new(size, size).expect("a drawing has a size"),
        at: sk::Transform::from_scale(size as f32 / 24.0, size as f32 / 24.0),
    };
    shape(&mut canvas, drawing);
    let mut image = RgbaImage::new(size, size);
    for (to, from) in image.pixels_mut().zip(canvas.pixmap.pixels()) {
        // White throughout, so the sampler mixes no black into an edge.
        to.0 = [255, 255, 255, from.demultiply().alpha()];
    }
    image
}

/// Each drawing on the 24 grid.
fn shape(c: &mut Canvas, drawing: Drawing) {
    if let Some(data) = drawing.mark() {
        c.fill(parse(data));
        return;
    }
    match drawing {
        Drawing::Manual => {
            c.fill(rounded(1.5, 3.0, 9.0, 5.0, 1.5));
            c.fill(rounded(1.5, 5.5, 21.0, 15.5, 2.2));
            c.cut(rect(10.9, 9.0, 2.2, 9.0));
            c.cut(rect(7.5, 12.4, 9.0, 2.2));
        }
        Drawing::App => {
            c.fill(rounded(1.5, 2.5, 21.0, 19.0, 3.0));
            c.cut(rounded(3.8, 8.0, 16.4, 11.2, 1.2));
        }
        Drawing::Launcher => {
            let corners = [(12.0, 0.8), (21.7, 6.4), (21.7, 17.6), (12.0, 23.2), (2.3, 17.6), (2.3, 6.4)];
            c.fill(polygon(&corners));
            c.cut_stroke(polyline(&[(2.3, 6.4), (12.0, 12.0), (21.7, 6.4)]), 1.7);
            c.cut_stroke(polyline(&[(12.0, 12.0), (12.0, 23.2)]), 1.7);
        }
        _ => unreachable!("a mark is drawn from its path data"),
    }
}

/// What a drawing is painted on: white shapes, and shapes cut out of them.
struct Canvas {
    pixmap: sk::Pixmap,
    at: sk::Transform,
}

impl Canvas {
    fn paint(clear: bool) -> sk::Paint<'static> {
        let mut paint = sk::Paint { anti_alias: true, ..Default::default() };
        paint.set_color(sk::Color::WHITE);
        if clear {
            paint.blend_mode = sk::BlendMode::Clear;
        }
        paint
    }

    fn stroke_of(width: f32) -> sk::Stroke {
        sk::Stroke { width, line_cap: sk::LineCap::Round, line_join: sk::LineJoin::Round, ..Default::default() }
    }

    fn fill(&mut self, path: sk::Path) {
        self.pixmap.fill_path(&path, &Self::paint(false), sk::FillRule::Winding, self.at, None);
    }

    fn cut(&mut self, path: sk::Path) {
        self.pixmap.fill_path(&path, &Self::paint(true), sk::FillRule::Winding, self.at, None);
    }

    fn cut_stroke(&mut self, path: sk::Path, width: f32) {
        self.pixmap.stroke_path(&path, &Self::paint(true), &Self::stroke_of(width), self.at, None);
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> sk::Path {
    sk::PathBuilder::from_rect(sk::Rect::from_xywh(x, y, w, h).expect("a rectangle"))
}

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> sk::Path {
    let (right, bottom) = (x + w, y + h);
    let mut path = sk::PathBuilder::new();
    path.move_to(x + r, y);
    path.line_to(right - r, y);
    path.quad_to(right, y, right, y + r);
    path.line_to(right, bottom - r);
    path.quad_to(right, bottom, right - r, bottom);
    path.line_to(x + r, bottom);
    path.quad_to(x, bottom, x, bottom - r);
    path.line_to(x, y + r);
    path.quad_to(x, y, x + r, y);
    path.close();
    path.finish().expect("a rounded rectangle")
}

fn polyline(points: &[(f32, f32)]) -> sk::Path {
    let mut path = sk::PathBuilder::new();
    path.move_to(points[0].0, points[0].1);
    for &(x, y) in &points[1..] {
        path.line_to(x, y);
    }
    path.finish().expect("a line")
}

fn polygon(points: &[(f32, f32)]) -> sk::Path {
    let mut path = sk::PathBuilder::new();
    path.move_to(points[0].0, points[0].1);
    for &(x, y) in &points[1..] {
        path.line_to(x, y);
    }
    path.close();
    path.finish().expect("a polygon")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_drawing_paints_at_every_size_and_leaves_something_and_some_clear() {
        for drawing in Drawing::ALL {
            for (index, &size) in SIZES.iter().enumerate() {
                let Painter(paint) = painter(drawing, index);
                let image = paint();
                assert_eq!(image.dimensions(), (size, size));
                let ink = image.pixels().filter(|p| p.0[3] > 128).count() as f32 / (size * size) as f32;
                assert!((0.15..0.85).contains(&ink), "{drawing:?} at {size}: {ink} of it inked");
            }
        }
    }

    /// The Bundle's XAML fills a path even-odd, as WPF's path markup does; Simple Icons draws for SVG's nonzero, which
    /// the Dashboard fills with. Every mark is the same either way.
    #[test]
    fn every_mark_fills_the_same_even_odd_as_nonzero() {
        let fill = |data: &str, rule: sk::FillRule| {
            let mut pixmap = sk::Pixmap::new(96, 96).unwrap();
            let paint = Canvas::paint(false);
            pixmap.fill_path(&parse(data), &paint, rule, sk::Transform::from_scale(4.0, 4.0), None);
            pixmap
        };
        for drawing in Drawing::ALL {
            let Some(data) = drawing.mark() else { continue };
            let (winding, even_odd) = (fill(data, sk::FillRule::Winding), fill(data, sk::FillRule::EvenOdd));
            let differ = winding.pixels().iter().zip(even_odd.pixels()).filter(|(a, b)| a.alpha() != b.alpha()).count();
            assert_eq!(differ, 0, "{drawing:?}");
        }
    }

    #[test]
    fn the_drawings_are_unlike_each_other() {
        let painted: Vec<RgbaImage> = Drawing::ALL.iter().map(|&drawing| paint(drawing, 32)).collect();
        for (i, a) in painted.iter().enumerate() {
            for (j, b) in painted.iter().enumerate().skip(i + 1) {
                let differ = a.pixels().zip(b.pixels()).filter(|(a, b)| a.0[3].abs_diff(b.0[3]) > 128).count();
                assert!(
                    differ > 32 * 32 / 8,
                    "{:?} and {:?} differ in {differ} pixels",
                    Drawing::ALL[i],
                    Drawing::ALL[j]
                );
            }
        }
    }
}
