//! A tip's pad: an Xbox One or Series pad from the front in white line, as the console's tip has it and the Bundle
//! draws it (`E/IdleCardControl.xaml`, its Canvas "Controller"; docs/spec/idle-screen.md, 4). The body is the
//! user's own tracing, carried over as the XAML's path data by their request (#251), as are the bumpers and the
//! D-pad; the sticks and buttons are the XAML's circles. Every line is 3.2 wide, centred on the path, round-joined.
//!
//! Painted with tiny-skia, as the Dashboard's other drawings are (`crate::drawn`): once all outlined, and once for
//! each part filled, as a part is when the tip is about it, each a picture a little larger than it is shown. The
//! filled part goes over the outlined pad, as the Bundle fills a part's own shape in place; its letter or mark is
//! in the panel's colour, the letters drawn as text over it and the marks cut out of it.
use tiny_skia as sk;
use ui::art::RgbaImage;
use ui::{ArtKey, Button, Color, Font, Kind, Painter, Rect, TextStyle, Ui};

use crate::path_data::parse;

/// The pad's box, 442 by 312: the Bundle's Canvas.
pub const SIZE: (f32, f32) = (442.0, 312.0);
/// Every line of it, as the XAML's StrokeThickness.
const LINE: f32 = 3.2;
/// The Guide button's line, the Bundle's for its mark.
const GUIDE_LINE: f32 = 1.8;
/// A face button's letter: 22 bold, in the panel's colour.
const LETTER: f32 = 22.0;

/// The body, traced (the XAML's Path "Body").
const BODY: &str = "M 181.1,13.8 C 175.1,13.8 168.7,14.5 162.8,13.6 C 154.9,12.3 148.6,5.9 141.3,3.2 \
    C 131.2,-0.6 100,10.9 90,15.7 C 83.3,19 72.7,24.4 70,31.8 C 69.3,33.6 69.6,35.5 68.1,37 \
    C 62.7,42.6 57.1,47.3 53.1,54 C 47.4,63.7 39.6,89.1 35.3,100.8 C 31.3,111.5 28.1,122.4 24.4,133.1 \
    C 13.3,165.3 4.8,198.8 3.6,233 C 3,248.5 3,264.4 9.9,278.7 C 14.8,289 22.8,297.8 32.8,303.3 \
    C 37.6,305.9 44.4,309.6 50,308.5 C 54.1,307.7 58.2,303.7 61.3,301 C 68.5,294.8 74.9,287.7 80.7,280.2 \
    C 94.1,263.2 109.3,235.8 130,227.5 C 132.4,226.6 135.2,225.5 137.7,225.3 C 146,224.7 154.5,225.2 162.8,225.2 \
    L 279.2,225.2 C 287.5,225.2 296,224.7 304.3,225.3 C 306.8,225.5 309.6,226.6 312,227.5 \
    C 332.7,235.8 347.9,263.2 361.3,280.2 C 367.1,287.7 373.5,294.8 380.7,301 C 383.8,303.7 387.9,307.7 392,308.5 \
    C 397.6,309.6 404.4,305.9 409.2,303.3 C 419.2,297.8 427.2,289 432.1,278.7 C 439,264.4 439,248.5 438.4,233 \
    C 437.2,198.8 428.7,165.3 417.6,133.1 C 413.9,122.4 410.7,111.5 406.7,100.8 C 402.4,89.1 394.6,63.7 388.9,54 \
    C 384.9,47.3 379.3,42.6 373.9,37 C 372.4,35.5 372.7,33.6 372,31.8 C 369.3,24.4 358.7,19 352,15.7 \
    C 342,10.9 310.8,-0.6 300.7,3.2 C 293.4,5.9 287.1,12.3 279.2,13.6 C 273.3,14.5 266.9,13.8 260.9,13.8 Z";
const LEFT_BUMPER: &str = "M 66.5,36.5 C 78,22 100,9 120,4 C 128,2 136,0.5 141,1.5 C 147,2.5 155,8 162,12.5 \
    L 163.5,14 L 150.5,32.5 C 147,27 142,20 136,18.5 C 125,16 100,19 82,29 C 76,32 71,35 66.5,36.5 Z";
const RIGHT_BUMPER: &str = "M 375.5,36.5 C 364,22 342,9 322,4 C 314,2 306,0.5 301,1.5 C 295,2.5 287,8 280,12.5 \
    L 278.5,14 L 291.5,32.5 C 295,27 300,20 306,18.5 C 317,16 342,19 360,29 C 366,32 371,35 375.5,36.5 Z";
const D_PAD: &str = "M 155.2,127.3 L 169.2,127.3 A 3,3 0 0 1 172.2,130.3 L 172.2,147.6 A 2,2 0 0 0 174.2,149.6 \
    L 191.5,149.6 A 3,3 0 0 1 194.5,152.6 L 194.5,166.6 A 3,3 0 0 1 191.5,169.6 L 174.2,169.6 \
    A 2,2 0 0 0 172.2,171.6 L 172.2,188.9 A 3,3 0 0 1 169.2,191.9 L 155.2,191.9 A 3,3 0 0 1 152.2,188.9 \
    L 152.2,171.6 A 2,2 0 0 0 150.2,169.6 L 132.9,169.6 A 3,3 0 0 1 129.9,166.6 L 129.9,152.6 \
    A 3,3 0 0 1 132.9,149.6 L 150.2,149.6 A 2,2 0 0 0 152.2,147.6 L 152.2,130.3 A 3,3 0 0 1 155.2,127.3 Z";
/// View's mark, lit: a square's outline behind, 1.4 wide, and a square filled in front.
const VIEW_BACK: &str = "M 186.4,89.9 L 182.8,89.9 L 182.8,82.9 L 189.8,82.9 L 189.8,86.9";
const VIEW_FRONT: &str = "M 186.4,86.9 L 193.4,86.9 L 193.4,93.9 L 186.4,93.9 Z";
/// Menu's mark, lit: three bars.
const MENU_MARK: &str = "M 248.1,84.4 L 259.1,84.4 L 259.1,86.2 L 248.1,86.2 Z M 248.1,87.5 L 259.1,87.5 \
    L 259.1,89.3 L 248.1,89.3 Z M 248.1,90.6 L 259.1,90.6 L 259.1,92.4 L 248.1,92.4 Z";
/// The Guide button: its middle, the Bundle's sphere's radius outlined (Kenney's sphere scaled by 0.725) and its
/// disc's when lit, and the sphere's radius cut out of the disc (scaled by 0.56).
const GUIDE: (f32, f32) = (221.0, 43.3);
const GUIDE_RING: f32 = 24.0 * 0.725;
const GUIDE_DISC: f32 = 19.5;
const GUIDE_LIT_RING: f32 = 24.0 * 0.56;

/// A part of the pad a tip can be about, named as the Bundle's (its ControllerButton; the D-pad's four as one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Part {
    LeftBumper,
    RightBumper,
    LeftStick,
    RightStick,
    DPad,
    View,
    Menu,
    X,
    Y,
    B,
    A,
    Guide,
}

impl Part {
    pub const ALL: [Part; 12] = [
        Part::LeftBumper,
        Part::RightBumper,
        Part::LeftStick,
        Part::RightStick,
        Part::DPad,
        Part::View,
        Part::Menu,
        Part::X,
        Part::Y,
        Part::B,
        Part::A,
        Part::Guide,
    ];

    /// The part a button lights: the D-pad's four as one. The triggers are behind the pad, so not drawn, as in the
    /// Bundle.
    pub fn of(button: Button) -> Option<Part> {
        Some(match button {
            Button::Up | Button::Down | Button::Left | Button::Right => Part::DPad,
            Button::A => Part::A,
            Button::B => Part::B,
            Button::X => Part::X,
            Button::Y => Part::Y,
            Button::Menu => Part::Menu,
            Button::View => Part::View,
            Button::LeftBumper => Part::LeftBumper,
            Button::RightBumper => Part::RightBumper,
            Button::LeftStick => Part::LeftStick,
            Button::Guide => Part::Guide,
            Button::LeftTrigger | Button::RightTrigger => return None,
        })
    }

    /// A face button's letter and middle.
    fn letter(self) -> Option<(&'static str, (f32, f32))> {
        match self {
            Part::X => Some(("X", (306.6, 88.4))),
            Part::Y => Some(("Y", (337.2, 58.1))),
            Part::B => Some(("B", (367.6, 88.4))),
            Part::A => Some(("A", (337.2, 117.7))),
            _ => None,
        }
    }

    /// Its outline, as the XAML's Path tagged Shape. The Guide button has none: its mark is drawn alone.
    fn shape(self) -> Option<sk::Path> {
        let circle = |cx: f32, cy: f32, r: f32| sk::PathBuilder::from_circle(cx, cy, r);
        match self {
            Part::LeftBumper => Some(parse(LEFT_BUMPER)),
            Part::RightBumper => Some(parse(RIGHT_BUMPER)),
            Part::LeftStick => circle(105.0, 88.4, 24.5),
            Part::RightStick => circle(281.0, 159.6, 24.5),
            Part::DPad => Some(parse(D_PAD)),
            Part::View => circle(188.1, 88.4, 9.5),
            Part::Menu => circle(253.6, 88.4, 9.5),
            Part::X | Part::Y | Part::B | Part::A => {
                let (_, (cx, cy)) = self.letter().expect("a face button has a letter");
                circle(cx, cy, 13.8)
            }
            Part::Guide => None,
        }
    }

    /// Where its filled picture goes, in the pad's units: its shape and its line, whole numbers so the picture's
    /// pixels fall on the screen's.
    fn frame(self) -> Frame {
        let (left, top, right, bottom) = match self.shape() {
            Some(shape) => {
                let b = shape.bounds();
                (b.left(), b.top(), b.right(), b.bottom())
            }
            None => (GUIDE.0 - GUIDE_DISC, GUIDE.1 - GUIDE_DISC, GUIDE.0 + GUIDE_DISC, GUIDE.1 + GUIDE_DISC),
        };
        let margin = LINE;
        let (x, y) = ((left - margin).floor(), (top - margin).floor());
        Frame { x, y, w: (right + margin).ceil() - x, h: (bottom + margin).ceil() - y }
    }
}

/// A picture's place in the pad's units.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// The outlined pad's picture: the box and the lines' overhang.
const OUTLINE_FRAME: Frame = Frame { x: -4.0, y: -4.0, w: SIZE.0 + 8.0, h: SIZE.1 + 8.0 };

/// What is painted: the pad outlined, or one part filled.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Layer {
    Outline,
    Lit(Part),
}

impl Layer {
    fn index(self) -> u8 {
        match self {
            Layer::Outline => 0,
            Layer::Lit(part) => 1 + part as u8,
        }
    }

    fn of(index: u8) -> Layer {
        match index {
            0 => Layer::Outline,
            n => Layer::Lit(Part::ALL[n as usize - 1]),
        }
    }

    fn frame(self) -> Frame {
        match self {
            Layer::Outline => OUTLINE_FRAME,
            Layer::Lit(part) => part.frame(),
        }
    }
}

/// The scales a picture is painted at: the smallest at least the screen's.
const SCALES: [f32; 3] = [1.0, 2.0, 3.0];

/// Draws the pad with its top left at `at` (442 by 312), the parts `buttons` names filled, their letters in
/// `panel`'s colour.
pub fn draw(ui: &mut Ui, at: (f32, f32), buttons: &[Button], panel: Color) {
    let scale = SCALES.iter().position(|&s| s >= ui.px() - 0.01).unwrap_or(SCALES.len() - 1);
    let place = |ui: &mut Ui, layer: Layer| {
        let frame = layer.frame();
        let key = ArtKey {
            path: format!("idle/pad/{}/{}", layer.index(), SCALES[scale]).into(),
            kind: Kind::Painted(painter(layer.index(), scale)),
        };
        ui.art_tinted(Rect::new(at.0 + frame.x, at.1 + frame.y, frame.w, frame.h), &key, Color::WHITE)
    };
    place(ui, Layer::Outline);
    for part in Part::ALL {
        if !buttons.iter().filter_map(|&b| Part::of(b)).any(|lit| lit == part) {
            continue;
        }
        // The letter once the filled button is there to carry it.
        if place(ui, Layer::Lit(part)) {
            if let Some((letter, (cx, cy))) = part.letter() {
                let style = TextStyle::new(Font::Ui, LETTER, 700).color(panel);
                ui.label_centered(at.0 + cx, at.1 + cy, letter, &style);
            }
        }
    }
}

/// A painter is a function of nothing: one for each layer at each scale.
fn paint_one<const LAYER: u8, const SCALE: usize>() -> RgbaImage {
    paint(Layer::of(LAYER), SCALES[SCALE])
}

fn painter(layer: u8, scale: usize) -> Painter {
    macro_rules! table {
        ($($l:literal)*) => {
            match (layer, scale) {
                $(
                    ($l, 0) => Painter(paint_one::<$l, 0>),
                    ($l, 1) => Painter(paint_one::<$l, 1>),
                    ($l, _) => Painter(paint_one::<$l, 2>),
                )*
                _ => unreachable!("every layer is in the table"),
            }
        };
    }
    table!(0 1 2 3 4 5 6 7 8 9 10 11 12)
}

/// The layer at `scale` pixels to the pad's unit: white, its lines in the alpha.
fn paint(layer: Layer, scale: f32) -> RgbaImage {
    let frame = layer.frame();
    let (w, h) = ((frame.w * scale).round() as u32, (frame.h * scale).round() as u32);
    let mut canvas = Canvas {
        pixmap: sk::Pixmap::new(w, h).expect("a layer has a size"),
        at: sk::Transform::from_translate(-frame.x, -frame.y).post_scale(scale, scale),
    };
    match layer {
        Layer::Outline => {
            canvas.stroke(&parse(BODY), LINE);
            for part in Part::ALL {
                match part.shape() {
                    Some(shape) => canvas.stroke(&shape, LINE),
                    None => guide_mark(&mut canvas, false),
                }
            }
        }
        Layer::Lit(part) => {
            if let Some(shape) = part.shape() {
                canvas.fill(&shape);
                canvas.stroke(&shape, LINE);
            }
            match part {
                Part::View => {
                    let back = sk::Stroke { width: 1.4, ..Default::default() };
                    canvas.pixmap.stroke_path(&parse(VIEW_BACK), &Canvas::paint(true), &back, canvas.at, None);
                    canvas.cut(&parse(VIEW_FRONT));
                }
                Part::Menu => canvas.cut(&parse(MENU_MARK)),
                Part::Guide => guide_mark(&mut canvas, true),
                _ => {}
            }
        }
    }
    let mut image = RgbaImage::new(w, h);
    for (to, from) in image.pixels_mut().zip(canvas.pixmap.pixels()) {
        // White throughout, so the sampler mixes no black into an edge.
        to.0 = [255, 255, 255, from.demultiply().alpha()];
    }
    image
}

/// **The Guide button's mark: a hook.** The Bundle outlines the Xbox sphere there (Kenney's Input Prompts, its top
/// filled) and, lit, cuts it out of a white disc; the port never draws the Xbox logo, so a plain ring stands in at the
/// sphere's size: outlined, or cut out of the disc.
fn guide_mark(canvas: &mut Canvas, lit: bool) {
    let circle = |r: f32| sk::PathBuilder::from_circle(GUIDE.0, GUIDE.1, r).expect("a circle");
    if lit {
        canvas.fill(&circle(GUIDE_DISC));
        canvas.cut_stroke(&circle(GUIDE_LIT_RING), GUIDE_LINE);
    } else {
        canvas.stroke(&circle(GUIDE_RING), GUIDE_LINE);
    }
}

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

    fn round(width: f32) -> sk::Stroke {
        sk::Stroke { width, line_join: sk::LineJoin::Round, ..Default::default() }
    }

    fn fill(&mut self, path: &sk::Path) {
        self.pixmap.fill_path(path, &Self::paint(false), sk::FillRule::Winding, self.at, None);
    }

    fn cut(&mut self, path: &sk::Path) {
        self.pixmap.fill_path(path, &Self::paint(true), sk::FillRule::Winding, self.at, None);
    }

    fn stroke(&mut self, path: &sk::Path, width: f32) {
        self.pixmap.stroke_path(path, &Self::paint(false), &Self::round(width), self.at, None);
    }

    fn cut_stroke(&mut self, path: &sk::Path, width: f32) {
        self.pixmap.stroke_path(path, &Self::paint(true), &Self::round(width), self.at, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The alpha at the pad's point (`x`, `y`) in `layer` painted at 1:1.
    fn alpha(image: &RgbaImage, layer: Layer, x: f32, y: f32) -> u8 {
        let frame = layer.frame();
        image.get_pixel((x - frame.x) as u32, (y - frame.y) as u32).0[3]
    }

    #[test]
    fn the_outline_follows_the_traced_body_and_leaves_the_buttons_open() {
        let outline = paint(Layer::Outline, 1.0);
        assert_eq!(outline.dimensions(), (450, 320));
        // On the body's line: its left grip's outer edge, and its bottom between the grips.
        assert!(alpha(&outline, Layer::Outline, 3.6, 233.0) > 200);
        assert!(alpha(&outline, Layer::Outline, 221.0, 225.2) > 200);
        // Inside the body, and inside A's ring: clear.
        assert_eq!(alpha(&outline, Layer::Outline, 221.0, 150.0), 0);
        assert_eq!(alpha(&outline, Layer::Outline, 337.2, 117.7), 0);
        // On A's ring.
        assert!(alpha(&outline, Layer::Outline, 337.2 + 13.8, 117.7) > 200);
    }

    #[test]
    fn a_lit_part_is_filled_its_mark_cut_out() {
        let a = Layer::Lit(Part::A);
        assert!(alpha(&paint(a, 1.0), a, 337.2, 117.7) > 250);
        let view = Layer::Lit(Part::View);
        let painted = paint(view, 1.0);
        // The front square cut out of the disc, the disc whole beside it.
        assert_eq!(alpha(&painted, view, 190.0, 90.5), 0);
        assert!(alpha(&painted, view, 188.1, 96.0) > 250);
        let guide = Layer::Lit(Part::Guide);
        let painted = paint(guide, 1.0);
        assert!(alpha(&painted, guide, 221.0, 43.3) > 250);
        assert!(alpha(&painted, guide, 221.0 + GUIDE_LIT_RING, 43.3) < 60);
    }

    #[test]
    fn buttons_light_their_parts_and_every_layer_has_a_painter() {
        assert_eq!(Part::of(Button::Left), Some(Part::DPad));
        assert_eq!(Part::of(Button::LeftTrigger), None);
        for index in 0..=Part::ALL.len() as u8 {
            assert_eq!(Layer::of(index).index(), index);
            let _ = painter(index, 0);
        }
        // Twice the size at twice the scale.
        assert_eq!(paint(Layer::Outline, 2.0).dimensions(), (900, 640));
    }
}
