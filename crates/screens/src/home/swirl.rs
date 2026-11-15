//! The green swirl behind Home while no game's art is up: broad bands of green and lime sweeping in from the top
//! right and the bottom left over dark green, turning and drifting slowly. docs/spec/home.md, section 8.5, has the
//! Bundle's drawing, which this is.
//!
//! As the Bundle does, each corner's bands are drawn once into a picture, one pixel to a unit of the 1920 x 1080
//! canvas they are authored on, and the picture is then only moved: a frame of the swirl is five quads.
use std::f64::consts::PI;
use std::path::PathBuf;

use tiny_skia as sk;
use ui::art::RgbaImage;
use ui::{ArtKey, Color, Gradient, Kind, Painter, Rect, Ui};

/// The canvas the swirl is authored on. It fills the screen whatever its shape: scaled until it covers it,
/// centred, and cut to it.
const WIDTH: f32 = 1920.0;
const HEIGHT: f32 = 1080.0;
/// The bands are drawn this much larger than authored, about the canvas's middle, so turning never shows their
/// ends.
const SCALE: f32 = 1.2;
/// The swirl is drawn again this often while it moves, as the Bundle's is: its fastest point covers some 30 pixels
/// a second at 1080p, a pixel a frame.
const STEP: f32 = 1.0 / 30.0;

/// One band: a stroke along two cubic curves, coloured by a gradient from the top left to the bottom right of the
/// curves' bounding box.
struct Band {
    /// The start, then each curve's two control points and its end.
    path: [(f32, f32); 7],
    width: f32,
    opacity: f32,
    /// Where along the gradient, and `0xRRGGBB`.
    stops: &'static [(f32, u32)],
}

/// The top right's bands, bottom first: in from the top, round and away off the right edge.
const TOP: [Band; 3] = [
    Band {
        path: [
            (520.0, -260.0),
            (600.0, 140.0),
            (920.0, 250.0),
            (1360.0, 250.0),
            (1760.0, 250.0),
            (1960.0, 420.0),
            (2200.0, 720.0),
        ],
        width: 300.0,
        opacity: 0.9,
        stops: &[(0.0, 0x14581A), (1.0, 0x2E8B22)],
    },
    Band {
        path: [
            (600.0, -260.0),
            (680.0, 90.0),
            (980.0, 180.0),
            (1380.0, 180.0),
            (1760.0, 180.0),
            (1980.0, 330.0),
            (2240.0, 620.0),
        ],
        width: 150.0,
        opacity: 1.0,
        stops: &[(0.0, 0x3E9A24), (0.6, 0x8FD13A), (1.0, 0x5CB02A)],
    },
    Band {
        path: [
            (650.0, -260.0),
            (730.0, 60.0),
            (1020.0, 130.0),
            (1400.0, 130.0),
            (1770.0, 130.0),
            (2000.0, 270.0),
            (2260.0, 560.0),
        ],
        width: 46.0,
        opacity: 0.85,
        stops: &[(0.0, 0x9BE03A), (1.0, 0xC6F25A)],
    },
];

/// The bottom left's bands: in from the left edge, round and away off the bottom.
const BOTTOM: [Band; 3] = [
    Band {
        path: [
            (-300.0, 520.0),
            (60.0, 640.0),
            (160.0, 880.0),
            (560.0, 940.0),
            (900.0, 990.0),
            (1060.0, 1060.0),
            (1200.0, 1400.0),
        ],
        width: 320.0,
        opacity: 0.9,
        stops: &[(0.0, 0x2E8B22), (1.0, 0x14581A)],
    },
    Band {
        path: [
            (-300.0, 600.0),
            (40.0, 720.0),
            (140.0, 960.0),
            (540.0, 1020.0),
            (860.0, 1070.0),
            (1000.0, 1140.0),
            (1120.0, 1460.0),
        ],
        width: 160.0,
        opacity: 1.0,
        stops: &[(0.0, 0x5CB02A), (0.4, 0xA6E040), (1.0, 0x3E9A24)],
    },
    Band {
        path: [
            (-300.0, 660.0),
            (20.0, 780.0),
            (120.0, 1020.0),
            (520.0, 1080.0),
            (830.0, 1130.0),
            (960.0, 1200.0),
            (1060.0, 1500.0),
        ],
        width: 50.0,
        opacity: 0.85,
        stops: &[(0.0, 0xC6F25A), (1.0, 0x9BE03A)],
    },
];

impl Band {
    /// The box around the curves themselves, not their stroke: x0, y0, x1, y1. The gradient runs across it.
    fn bounds(&self) -> [f32; 4] {
        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for curve in [&self.path[0..4], &self.path[3..7]] {
            for step in 0..=512 {
                let t = step as f32 / 512.0;
                let u = 1.0 - t;
                let weights = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                let x: f32 = curve.iter().zip(weights).map(|(point, weight)| point.0 * weight).sum();
                let y: f32 = curve.iter().zip(weights).map(|(point, weight)| point.1 * weight).sum();
                bounds = [bounds[0].min(x), bounds[1].min(y), bounds[2].max(x), bounds[3].max(y)];
            }
        }
        bounds
    }
}

/// The part of the canvas a set of bands' picture covers, in whole units: x, y, width, height. All of the bands
/// that can ever show: turned five degrees either way and drifted as far as they go, nothing further than 20 off
/// the canvas comes onto the screen.
fn extent(set: &[Band; 3]) -> (i32, i32, u32, u32) {
    let mut all = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for band in set {
        let [x0, y0, x1, y1] = band.bounds();
        let half = band.width / 2.0 + 1.0;
        all = [all[0].min(x0 - half), all[1].min(y0 - half), all[2].max(x1 + half), all[3].max(y1 + half)];
    }
    let (x0, y0) = (all[0].max(-20.0).floor(), all[1].max(-20.0).floor());
    let (x1, y1) = (all[2].min(WIDTH + 20.0).ceil(), all[3].min(HEIGHT + 20.0).ceil());
    (x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32)
}

/// A set of bands as a picture, clear where no band is.
fn paint(set: &[Band; 3]) -> RgbaImage {
    let (x, y, width, height) = extent(set);
    let mut pixmap = sk::Pixmap::new(width, height).expect("the swirl's picture has a size");
    let place = sk::Transform::from_translate(-x as f32, -y as f32);
    for band in set {
        let [start, a, b, c, d, e, end] = band.path;
        let mut path = sk::PathBuilder::new();
        path.move_to(start.0, start.1);
        path.cubic_to(a.0, a.1, b.0, b.1, c.0, c.1);
        path.cubic_to(d.0, d.1, e.0, e.1, end.0, end.1);
        let path = path.finish().expect("a band is a path");

        // The band's opacity is its colours': a stroke never lies over itself.
        let stop = |&(at, rgb): &(f32, u32)| {
            let channel = |shift: u32| ((rgb >> shift) & 0xff) as f32 / 255.0;
            let color = sk::Color::from_rgba(channel(16), channel(8), channel(0), band.opacity).expect("a colour");
            sk::GradientStop::new(at, color)
        };
        // From the box's top left to its bottom right, as WPF maps a brush to a box: stretched with it, so a
        // colour runs along the box's other diagonal, not square to the gradient's own line.
        let [x0, y0, x1, y1] = band.bounds();
        let shader = sk::LinearGradient::new(
            sk::Point::from_xy(0.0, 0.0),
            sk::Point::from_xy(1.0, 1.0),
            band.stops.iter().map(stop).collect(),
            sk::SpreadMode::Pad,
            sk::Transform::from_row(x1 - x0, 0.0, 0.0, y1 - y0, x0, y0),
        );
        let paint = sk::Paint {
            shader: shader.expect("a gradient"),
            anti_alias: true,
            force_hq_pipeline: true,
            ..Default::default()
        };
        // WPF's stroke: flat ends, mitred joins, which tiny-skia's are too.
        let stroke = sk::Stroke { width: band.width, ..Default::default() };
        pixmap.stroke_path(&path, &paint, &stroke, place, None);
    }

    let mut image = RgbaImage::new(width, height);
    for (to, from) in image.pixels_mut().zip(pixmap.pixels()) {
        let from = from.demultiply();
        to.0 = [from.red(), from.green(), from.blue(), from.alpha()];
    }
    bleed(&mut image);
    image
}

/// Gives the bands' colour to the clear pixels beside them and to the faintest of their edge, whose own colour
/// is lost with its alpha. The sampler mixes neighbours' colours whatever their alpha: without this it mixes black
/// into every band's edge.
fn bleed(image: &mut RgbaImage) {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut known: Vec<bool> = image.pixels().map(|pixel| pixel[3] >= 128).collect();
    for _ in 0..3 {
        let (before, was_known) = (image.clone(), known.clone());
        for y in 0..height {
            for x in 0..width {
                if was_known[y * width + x] {
                    continue;
                }
                let (mut sum, mut count) = ([0u32; 3], 0);
                for (nx, ny) in neighbours(x, y, width, height) {
                    if was_known[ny * width + nx] {
                        let pixel = before.get_pixel(nx as u32, ny as u32);
                        sum = [sum[0] + pixel[0] as u32, sum[1] + pixel[1] as u32, sum[2] + pixel[2] as u32];
                        count += 1;
                    }
                }
                // With no neighbour's colour to take yet, it waits for a later pass.
                let mean = |channel: usize| sum[channel].checked_div(count).map(|mean| mean as u8);
                if let (Some(red), Some(green), Some(blue)) = (mean(0), mean(1), mean(2)) {
                    let pixel = image.get_pixel_mut(x as u32, y as u32);
                    pixel.0 = [red, green, blue, pixel[3]];
                    known[y * width + x] = true;
                }
            }
        }
    }
}

/// The pixels around (x, y) that are in the picture.
fn neighbours(x: usize, y: usize, width: usize, height: usize) -> impl Iterator<Item = (usize, usize)> {
    let around = |at: usize, size: usize| at.saturating_sub(1)..=(at + 1).min(size - 1);
    around(y, height).flat_map(move |ny| around(x, width).map(move |nx| (nx, ny))).filter(move |&at| at != (x, y))
}

fn top_key() -> ArtKey {
    ArtKey { path: PathBuf::from("swirl/top"), kind: Kind::Painted(Painter(|| paint(&TOP))) }
}

fn bottom_key() -> ArtKey {
    ArtKey { path: PathBuf::from("swirl/bottom"), kind: Kind::Painted(Painter(|| paint(&BOTTOM))) }
}

/// Where the swirl's parts are: the turn of the whole, in degrees clockwise, and each set of bands' drift.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Pose {
    turn: f32,
    top: (f32, f32),
    bottom: (f32, f32),
}

impl Pose {
    /// The pose `seconds` into the swirl's movement: five swings there and back, each at its own pace.
    fn at(seconds: f64) -> Self {
        // From one end to the other in `one_way` seconds and back in as long, easing in and out of each end
        // (WPF's SineEase, EaseInOut, auto-reversed): a cosine.
        let swing = |from: f32, to: f32, one_way: f64| {
            let along = 0.5 - 0.5 * (PI * seconds / one_way).cos();
            from + (to - from) * along as f32
        };
        Self {
            turn: swing(-5.0, 5.0, 22.0),
            top: (swing(-60.0, 60.0, 13.0), swing(30.0, -30.0, 17.0)),
            bottom: (swing(50.0, -50.0, 15.0), swing(-40.0, 40.0, 19.0)),
        }
    }
}

#[derive(Default)]
pub struct Swirl {
    /// How long it has moved for, in seconds. None until it first moves: it then stands as authored, not turned
    /// and not drifted, which is not where its movement starts.
    moved: Option<f64>,
}

impl Swirl {
    /// The swirl's pictures are kept though it does not show, so that it is whole the moment it does.
    pub fn keep(&self, ui: &mut Ui) {
        for key in [top_key(), bottom_key()] {
            ui.art.get(&key);
        }
    }

    /// The swirl over the whole screen. Drawn `in_front`, it moves on and asks to be drawn again; left undrawn,
    /// behind something else, or under Reduce motion, it stays where it is.
    pub fn draw(&mut self, ui: &mut Ui, in_front: bool) {
        if in_front && !ui.store.reduce_motion {
            self.moved = Some(self.moved.map_or(0.0, |moved| moved + ui.store.dt() as f64));
            ui.store.wake_in(STEP);
        }
        let pose = self.moved.map_or(Pose::default(), Pose::at);

        // The canvas over the screen.
        let screen = ui.screen();
        let scale = (screen.w / WIDTH).max(screen.h / HEIGHT);
        let (left, top) = (screen.x + (screen.w - WIDTH * scale) / 2.0, screen.y + (screen.h - HEIGHT * scale) / 2.0);
        let on_screen =
            |rect: Rect| Rect::new(left + rect.x * scale, top + rect.y * scale, rect.w * scale, rect.h * scale);
        let (middle_x, middle_y) = (WIDTH / 2.0, HEIGHT / 2.0);
        // A box about the canvas's middle, this many canvas widths and heights across.
        let about_middle = |across: f32, down: f32| {
            Rect::new(middle_x - WIDTH * across / 2.0, middle_y - HEIGHT * down / 2.0, WIDTH * across, HEIGHT * down)
        };

        // Dark green, darker to the corners: a radial gradient whose radii are 0.75 of the canvas each way.
        let (inner, outer) = (Color::rgb(0x1E2C1C), Color::rgb(0x0C140B));
        ui.gradient(on_screen(about_middle(1.5, 1.5)), inner, outer, Gradient::Radial, 0.0);

        // Each set's picture where its drift, then the whole's scale and turn about the canvas's middle, put it.
        let turn = pose.turn.to_radians();
        let (sin, cos) = turn.sin_cos();
        for (key, set, drift) in [(top_key(), &TOP, pose.top), (bottom_key(), &BOTTOM, pose.bottom)] {
            let (x, y, width, height) = extent(set);
            let (width, height) = (width as f32, height as f32);
            let from_middle =
                (x as f32 + width / 2.0 + drift.0 - middle_x, y as f32 + height / 2.0 + drift.1 - middle_y);
            let x = middle_x + SCALE * (from_middle.0 * cos - from_middle.1 * sin);
            let y = middle_y + SCALE * (from_middle.0 * sin + from_middle.1 * cos);
            let (width, height) = (width * SCALE, height * SCALE);
            ui.art_turned(on_screen(Rect::new(x - width / 2.0, y - height / 2.0, width, height)), &key, 1.0, turn);
        }

        // A soft darkening to the middle: radii 0.45 of the canvas across and 0.55 down, and it does not turn.
        let (dark, clear) = (Color::argb(0x88101810), Color::argb(0x00101810));
        ui.gradient(on_screen(about_middle(0.9, 1.1)), dark, clear, Gradient::Radial, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    /// docs/spec/home.md, section 8.5: where each swing starts, where it is half way and at its far end.
    #[test]
    fn the_swirl_swings_between_its_ends() {
        let start = Pose::at(0.0);
        assert_eq!(start, Pose { turn: -5.0, top: (-60.0, 30.0), bottom: (50.0, -40.0) });
        assert!(close(Pose::at(11.0).turn, 0.0) && close(Pose::at(22.0).turn, 5.0) && close(Pose::at(44.0).turn, -5.0));
        assert!(close(Pose::at(13.0).top.0, 60.0) && close(Pose::at(17.0).top.1, -30.0));
        assert!(close(Pose::at(15.0).bottom.0, -50.0) && close(Pose::at(19.0).bottom.1, 40.0));
        // Easing in: a tenth of the way through in time is only a fortieth of the way there.
        assert!(close(Pose::at(2.2).turn, -5.0 + 10.0 * 0.02447));
    }

    #[test]
    fn a_bands_gradient_runs_across_its_curves_own_box() {
        // The curves' box, which their control points lie outside of: this band dips to 250 and no lower until
        // its second curve leaves the canvas.
        let [x0, y0, x1, y1] = TOP[0].bounds();
        assert!(
            close(x0, 520.0) && close(y0, -260.0) && close(x1, 2200.0) && close(y1, 720.0),
            "{:?}",
            TOP[0].bounds()
        );
        let [x0, _, _, y1] = BOTTOM[2].bounds();
        assert!(close(x0, -300.0) && close(y1, 1500.0));
    }

    #[test]
    fn the_pictures_hold_the_bands_colours_and_bleed_them_past_their_edges() {
        let top = paint(&TOP);
        let (x, y, width, height) = extent(&TOP);
        assert_eq!((x + width as i32, y), (1940, -20), "the picture stops 20 past the canvas");
        assert!(height < 1000, "and at the bands' own foot: {height}");
        // On the narrow lime band where it runs level, by x 1400: its centre is at y 130.
        let lime = top.get_pixel((1400 - x) as u32, (130 - y) as u32).0;
        assert!(lime[3] == 255 && lime[1] > 200 && lime[0] > 120 && lime[2] < 110, "{lime:?}");
        // Far below every band: clear. Just below the broad band's edge, which is at y 250 + 150: clear, but green.
        assert_eq!(top.get_pixel((1360 - x) as u32, (700 - y) as u32).0, [0, 0, 0, 0]);
        let beside = top.get_pixel((1360 - x) as u32, (401 - y) as u32).0;
        assert!(beside[3] == 0 && beside[1] > 80, "{beside:?}");
    }
}
