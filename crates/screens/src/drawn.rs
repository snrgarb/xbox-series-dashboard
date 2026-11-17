//! Pictures the Dashboard draws itself from vector paths, in one colour: the launchers' marks (docs/spec/home.md,
//! 3.3.1). Each is the project's own drawing of an idea its launcher brings to mind, never a company's logo or a
//! tracing of one. A drawing is laid out on a 24 x 24 grid, as an icon font's glyph is, painted with tiny-skia once
//! per size it is asked at (a picture a little larger than it is shown, so it is never stretched up), in white, and
//! drawn in whatever colour it is asked for.
use tiny_skia as sk;
use ui::art::RgbaImage;
use ui::{ArtKey, Color, Kind, Painter, Rect, Ui};

/// Every drawing, by what it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Drawing {
    /// Steam: a disc with three wisps of steam rising through it.
    Steam,
    /// The Epic Games launcher: a shield with a star cut out of it.
    Epic,
    /// GOG GALAXY: a ringed planet.
    Gog,
    /// Battle.net: two swords crossed.
    BattleNet,
    /// The EA app: a lightning bolt, for Electronic Arts.
    Ea,
    /// Ubisoft Connect: two rings linked, for Connect.
    Ubisoft,
    /// Xbox and the Microsoft Store: a pad.
    Xbox,
    /// A game added by hand: a folder with a plus cut out of it.
    Manual,
    /// An App: a window, its title bar solid.
    App,
    /// A launcher of no drawing of its own: a box, as a cube.
    Launcher,
}

impl Drawing {
    pub const ALL: [Drawing; 10] = [
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
        }
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
    table!(0 1 2 3 4 5 6 7 8 9)
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
    match drawing {
        Drawing::Steam => {
            c.fill(circle(12.0, 12.0, 11.5));
            for (x, bottom, top) in [(7.2, 17.0, 7.5), (12.0, 19.5, 4.5), (16.8, 17.0, 7.5)] {
                c.cut_stroke(wave(x, bottom, top, 1.5), 2.1);
            }
        }
        Drawing::Epic => {
            let mut shield = sk::PathBuilder::new();
            shield.move_to(5.5, 1.5);
            shield.line_to(18.5, 1.5);
            shield.quad_to(21.0, 1.5, 21.0, 4.0);
            shield.line_to(21.0, 12.0);
            shield.cubic_to(21.0, 17.5, 16.5, 20.5, 12.0, 23.0);
            shield.cubic_to(7.5, 20.5, 3.0, 17.5, 3.0, 12.0);
            shield.line_to(3.0, 4.0);
            shield.quad_to(3.0, 1.5, 5.5, 1.5);
            shield.close();
            c.fill(shield.finish().expect("a shield"));
            c.cut(star(12.0, 11.0, 6.0, 2.5));
        }
        Drawing::Gog => {
            let (turn, rx, ry) = (-0.4, 11.6, 4.0);
            c.fill(circle(12.0, 12.0, 6.6));
            // The ring passes behind the planet above and in front of it below: a gap either side of the front.
            c.cut_stroke(ellipse(12.0, 12.0, rx, ry, turn, 0.0, std::f32::consts::PI), 4.2);
            c.stroke(ellipse(12.0, 12.0, rx, ry, turn, 0.0, std::f32::consts::TAU), 1.8);
        }
        Drawing::BattleNet => {
            let turned = |degrees: f32| {
                let turn = sk::Transform::from_rotate_at(degrees, 12.0, 12.0);
                sword().into_iter().map(move |part| part.transform(turn).expect("a sword"))
            };
            turned(45.0).for_each(|part| c.fill(part));
            // The front sword with a margin cut round it, so the two read as two.
            turned(-45.0).for_each(|part| c.cut_stroke(part, 2.0));
            turned(-45.0).for_each(|part| c.fill(part));
        }
        Drawing::Ea => {
            c.fill(polygon(&[(15.5, 0.5), (4.0, 14.0), (11.0, 14.0), (8.5, 23.5), (20.0, 9.5), (13.0, 9.5)]));
        }
        Drawing::Ubisoft => {
            let (left, right, r, width) = (7.6, 16.4, 6.3, 2.5);
            let half = std::f32::consts::PI;
            c.stroke(ellipse(left, 12.0, r, r, 0.0, 0.0, 2.0 * half), width);
            // The right ring over the left above, the left over the right below: linked.
            c.cut_stroke(ellipse(right, 12.0, r, r, 0.0, half, 2.0 * half), width + 2.2);
            c.stroke(ellipse(right, 12.0, r, r, 0.0, 0.0, 2.0 * half), width);
            c.cut_stroke(ellipse(left, 12.0, r, r, 0.0, -0.5, 1.2), width + 2.2);
            c.stroke(ellipse(left, 12.0, r, r, 0.0, -0.5, 1.2), width);
        }
        Drawing::Xbox => {
            c.fill(rounded(1.5, 5.0, 21.0, 10.5, 5.0));
            c.fill(circle(6.0, 15.0, 4.6));
            c.fill(circle(18.0, 15.0, 4.6));
            // A cross for the pad's directions, two holes for its buttons.
            c.cut(rect(4.1, 9.3, 5.8, 1.9));
            c.cut(rect(6.05, 7.35, 1.9, 5.8));
            c.cut(circle(15.6, 11.6, 1.35));
            c.cut(circle(18.4, 8.9, 1.35));
        }
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
    }
}

/// A sword standing on its point, the length of the grid: blade, guard, grip and pommel, each a path of its own so
/// that where they meet is filled once.
fn sword() -> [sk::Path; 4] {
    [
        polygon(&[(12.0, -0.5), (13.4, 1.8), (13.4, 15.5), (10.6, 15.5), (10.6, 1.8)]),
        rect(7.6, 15.3, 8.8, 2.2),
        rect(11.1, 17.4, 1.8, 4.6),
        circle(12.0, 22.6, 1.6),
    ]
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

    fn stroke(&mut self, path: sk::Path, width: f32) {
        self.pixmap.stroke_path(&path, &Self::paint(false), &Self::stroke_of(width), self.at, None);
    }

    fn cut_stroke(&mut self, path: sk::Path, width: f32) {
        self.pixmap.stroke_path(&path, &Self::paint(true), &Self::stroke_of(width), self.at, None);
    }
}

fn circle(cx: f32, cy: f32, r: f32) -> sk::Path {
    sk::PathBuilder::from_circle(cx, cy, r).expect("a circle")
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

/// A five-pointed star, a point up.
fn star(cx: f32, cy: f32, outer: f32, inner: f32) -> sk::Path {
    let points: Vec<(f32, f32)> = (0..10)
        .map(|i| {
            let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let r = if i % 2 == 0 { outer } else { inner };
            (cx + r * angle.cos(), cy + r * angle.sin())
        })
        .collect();
    polygon(&points)
}

/// Part of an ellipse about (`cx`, `cy`), turned `turn` radians, from angle `from` to `to` (0 is its right end,
/// a quarter turn its bottom).
fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32, turn: f32, from: f32, to: f32) -> sk::Path {
    let steps = 48;
    let (sin, cos) = turn.sin_cos();
    let points: Vec<(f32, f32)> = (0..=steps)
        .map(|i| {
            let angle = from + (to - from) * i as f32 / steps as f32;
            let (x, y) = (rx * angle.cos(), ry * angle.sin());
            (cx + x * cos - y * sin, cy + x * sin + y * cos)
        })
        .collect();
    polyline(&points)
}

/// A wisp rising from `bottom` to `top` at `x`, swaying `sway` either side.
fn wave(x: f32, bottom: f32, top: f32, sway: f32) -> sk::Path {
    let steps = 24;
    let points: Vec<(f32, f32)> = (0..=steps)
        .map(|i| {
            let along = i as f32 / steps as f32;
            (x + sway * (along * std::f32::consts::TAU).sin(), bottom + (top - bottom) * along)
        })
        .collect();
    polyline(&points)
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
