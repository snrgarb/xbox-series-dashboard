//! The UI layer. A screen keeps its own state (where focus is, what is open) and, every frame, declares what it
//! looks like to a [`Ui`]: quads, images and text in design units on a 1920-wide canvas, with animated values asked
//! for by id. Nothing is retained between frames but those values and the art cache. See docs/ui.md.
pub mod anim;
pub mod art;
pub mod host;
pub mod input;
pub mod snap;

use std::hash::Hash;

pub use anim::{Ease, Store};
pub use art::{Art, ArtCache, ArtKey, Kind, Painter};
pub use host::Host;
pub use input::{Battery, Button, PadEvent, PadStatus, Press, Repeat};
pub use render::{Font, Gradient, HAlign, Rect, TexId};
use render::{Frame, Layer, Measured, Quad, Rgba, Text, TextKey, TextSys, WHITE};
pub use snap::Snapshot;

/// Every screen is laid out on a canvas this size, scaled to fit the output whole. On an output of another shape the
/// canvas sits centred across and at the top, and [`Ui::screen`] is the larger rectangle a background fills.
pub const DESIGN_WIDTH: f32 = 1920.0;
pub const DESIGN_HEIGHT: f32 = 1080.0;

/// Pixels per design unit on an output of this size.
pub fn scale_for(output: (u32, u32)) -> f32 {
    (output.0 as f32 / DESIGN_WIDTH).min(output.1 as f32 / DESIGN_HEIGHT)
}

/// An sRGB colour with straight alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color::rgb(0xFFFFFF);
    pub const BLACK: Color = Color::rgb(0x000000);
    pub const CLEAR: Color = Color::argb(0x00000000);

    /// `0xRRGGBB`, opaque.
    pub const fn rgb(hex: u32) -> Self {
        Self { r: (hex >> 16) as u8, g: (hex >> 8) as u8, b: hex as u8, a: 1.0 }
    }

    /// `0xAARRGGBB`, as XAML writes a colour.
    pub const fn argb(hex: u32) -> Self {
        Self { r: (hex >> 16) as u8, g: (hex >> 8) as u8, b: hex as u8, a: (hex >> 24) as f32 / 255.0 }
    }

    /// The same colour, its alpha multiplied.
    pub fn fade(self, by: f32) -> Self {
        Self { a: self.a * by, ..self }
    }

    fn quad(self, alpha: f32) -> Rgba {
        [self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0, self.a * alpha]
    }

    fn text(self, alpha: f32) -> [u8; 4] {
        [self.r, self.g, self.b, ((self.a * alpha).clamp(0.0, 1.0) * 255.0).round() as u8]
    }
}

/// How a piece of text is set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub font: Font,
    /// In design units.
    pub size: f32,
    /// 300 light, 400 regular, 600 semibold, 700 bold.
    pub weight: u16,
    pub color: Color,
}

impl TextStyle {
    pub const fn new(font: Font, size: f32, weight: u16) -> Self {
        Self { font, size, weight, color: Color::WHITE }
    }
    pub const fn color(self, color: Color) -> Self {
        Self { color, ..self }
    }
    pub const fn size(self, size: f32) -> Self {
        Self { size, ..self }
    }
    pub const fn weight(self, weight: u16) -> Self {
        Self { weight, ..self }
    }
}

/// Where a box's text sits in it, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

/// Which part of an image that doesn't fit its box is kept: the box is always filled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Crop {
    /// The middle across, the top down: the theme's rule for covers and backgrounds.
    Top,
    Center,
}

/// A laid-out text's size in design units.
#[derive(Debug, Clone, Copy, Default)]
pub struct TextSize {
    pub width: f32,
    pub height: f32,
    pub line_height: f32,
    pub lines: usize,
}

/// One frame being declared.
pub struct Ui<'a> {
    frame: &'a mut Frame,
    text: &'a mut TextSys,
    pub store: &'a mut Store,
    pub art: &'a mut ArtCache,
    /// Pixels per design unit.
    px: f32,
    /// The output's size in pixels.
    output: (u32, u32),
    width: f32,
    height: f32,
    alpha: f32,
    offset: (f32, f32),
    clip: Option<Rect>,
}

impl<'a> Ui<'a> {
    /// Starts declaring a frame for an output of `output` pixels. `frame` is added to, so one frame can be declared
    /// in parts (the page, then the Guide over it).
    pub fn new(
        frame: &'a mut Frame,
        text: &'a mut TextSys,
        store: &'a mut Store,
        art: &'a mut ArtCache,
        output: (u32, u32),
    ) -> Self {
        let px = scale_for(output);
        let width = output.0 as f32 / px;
        if frame.layers.is_empty() {
            frame.layers.push(Layer::default());
        }
        Self {
            frame,
            text,
            store,
            art,
            px,
            output,
            width,
            height: output.1 as f32 / px,
            alpha: 1.0,
            offset: ((width - DESIGN_WIDTH) / 2.0, 0.0),
            clip: None,
        }
    }

    /// Starts declaring a frame for the part of an output of `whole` pixels that begins `top` pixels down it: a
    /// window that covers a strip of the screen. Everything is laid out as on the whole, and what falls outside the
    /// part is not seen.
    pub fn part(
        frame: &'a mut Frame,
        text: &'a mut TextSys,
        store: &'a mut Store,
        art: &'a mut ArtCache,
        whole: (u32, u32),
        top: u32,
    ) -> Self {
        let mut ui = Self::new(frame, text, store, art, whole);
        ui.offset.1 -= top as f32 / ui.px;
        ui
    }

    /// The whole output in design units: the canvas, and whatever of the output lies beside or below it. With no
    /// offset in force its `x` is 0 on a 16:9 output and negative on a wider one.
    pub fn screen(&self) -> Rect {
        let margin = (self.width - DESIGN_WIDTH) / 2.0;
        Rect::new(-margin, 0.0, self.width, self.height)
    }

    /// Pixels per design unit.
    pub fn px(&self) -> f32 {
        self.px
    }

    pub fn has_font(&self, font: Font) -> bool {
        self.text.has(font)
    }

    // --- Layers and the stacks that everything drawn goes through ---

    /// Starts a new layer: what is drawn from here on covers everything before it, its text included.
    pub fn layer(&mut self) {
        if self.frame.layers.last().is_some_and(|l| !l.quads.is_empty() || !l.texts.is_empty()) {
            self.frame.layers.push(Layer::default());
        }
    }

    /// Draws `body` as a layer of its own that is one picture, over everything before it: blurred by `blur` as
    /// WPF's `BlurEffect` blurs (a Gaussian that reaches `blur`; 0 for none), and faded as a whole to `opacity`
    /// times the alpha in force, as WPF fades an element: its parts do not show through each other. `bounds` is
    /// everything `body` draws and `blur` more round it, and moves with the offset in force; nothing of the layer
    /// shows outside it. What is drawn after covers it. `body` starts no layer of its own.
    pub fn group<R>(&mut self, bounds: Rect, blur: f32, opacity: f32, body: impl FnOnce(&mut Self) -> R) -> R {
        self.layer();
        let group = render::Group { bounds: self.place(bounds), blur: blur * self.px, opacity: opacity * self.alpha };
        self.top().group = Some(group);
        let layers = self.frame.layers.len();
        // The picture is drawn whole and faded after.
        let alpha = std::mem::replace(&mut self.alpha, 1.0);
        let result = body(self);
        self.alpha = alpha;
        debug_assert_eq!(self.frame.layers.len(), layers, "a group is one layer");
        // Even if it drew nothing, what follows is not part of it.
        self.frame.layers.push(Layer::default());
        result
    }

    /// A picture of what `body` draws on a screen of its own, this one's size: taken the first frame `key` is asked
    /// for, and kept, the same picture, for as long as some frame asks for it again (here, or with
    /// [`Self::snapshot_taken`]); `body` is not run again while it is kept. It is `shrink` times smaller each way
    /// than the screen and blurred there by `blur` of its own pixels, as WPF's `BlurEffect` blurs: draw it with
    /// [`Self::image`], stretched back over the screen. Nothing `body` draws is in this frame. `body` starts from
    /// no offset, clip or fade, and draws with the same animated values and art as the frame.
    ///
    /// This is how a panel shows what it was opened over, as it was then, without that being drawn again. None when
    /// no picture can be taken in this frame (a second new one in the same frame): ask again in the next.
    pub fn snapshot(&mut self, key: impl Hash, shrink: u32, blur: f32, body: impl FnOnce(&mut Ui)) -> Option<Snapshot> {
        let (id, now) = (anim::id(key), self.store.frame());
        if let Some(taken) = self.store.snaps.taken(id, now) {
            return Some(taken);
        }
        let tex = self.store.snaps.take_spare()?;
        let mut frame = Frame::default();
        body(&mut Ui::new(&mut frame, &mut *self.text, &mut *self.store, &mut *self.art, self.output));
        let size = ((self.output.0 / shrink.max(1)).max(1), (self.output.1 / shrink.max(1)).max(1));
        let job = snap::Job { tex, frame, output: self.output, shrink, blur };
        Some(self.store.snaps.take(id, now, Snapshot { tex, size }, job))
    }

    /// The picture kept under `key`, if one is: asking keeps it for another frame.
    pub fn snapshot_taken(&mut self, key: impl Hash) -> Option<Snapshot> {
        let now = self.store.frame();
        self.store.snaps.taken(anim::id(key), now)
    }

    /// A soft shadow round the rounded rectangle `rect`, as WPF's `DropShadowEffect` with no offset casts it:
    /// `blur` is its `BlurRadius`. It is darkest under the rectangle, so draw what casts it over it.
    pub fn shadow(&mut self, rect: Rect, color: Color, radius: f32, blur: f32) {
        // Measured on the Bundle's picture of the Tile menu (radius 24, half black): a quarter black at the panel's
        // edge, a sixth 5 out, a twelfth 10 out and gone by 18, which is the shader's soft edge one and a half
        // times the radius wide.
        self.push(Quad { feather: blur * 1.5, ..Quad::fill(rect, color.quad(self.alpha), radius) });
    }

    /// Draws `body` with every alpha multiplied by `alpha`.
    pub fn with_alpha<R>(&mut self, alpha: f32, body: impl FnOnce(&mut Self) -> R) -> R {
        let was = self.alpha;
        self.alpha *= alpha;
        let result = body(self);
        self.alpha = was;
        result
    }

    /// Draws `body` moved by this much.
    pub fn with_offset<R>(&mut self, dx: f32, dy: f32, body: impl FnOnce(&mut Self) -> R) -> R {
        let was = self.offset;
        self.offset = (was.0 + dx, was.1 + dy);
        let result = body(self);
        self.offset = was;
        result
    }

    /// Draws `body` cut to `clip` (which moves with the offset in force).
    pub fn with_clip<R>(&mut self, clip: Rect, body: impl FnOnce(&mut Self) -> R) -> R {
        let was = self.clip;
        let clip = clip.offset(self.offset.0, self.offset.1);
        self.clip = Some(was.map_or(clip, |w| w.intersect(clip)));
        let result = body(self);
        self.clip = was;
        result
    }

    /// The alpha everything is being multiplied by.
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    fn place(&self, rect: Rect) -> Rect {
        rect.offset(self.offset.0, self.offset.1).scale(self.px)
    }

    fn clip_px(&self) -> Option<Rect> {
        self.clip.map(|c| c.scale(self.px))
    }

    fn top(&mut self) -> &mut Layer {
        self.frame.layers.last_mut().expect("a frame has a layer")
    }

    // --- Quads ---

    fn push(&mut self, mut quad: Quad) {
        if quad.c0[3] <= 0.0 && quad.c1[3] <= 0.0 {
            return;
        }
        quad.rect = self.place(quad.rect);
        quad.radius *= self.px;
        quad.ring *= self.px;
        quad.feather *= self.px;
        quad.clip = self.clip_px();
        self.top().quads.push(quad);
    }

    pub fn fill(&mut self, rect: Rect, color: Color, radius: f32) {
        self.push(Quad::fill(rect, color.quad(self.alpha), radius));
    }

    pub fn gradient(&mut self, rect: Rect, from: Color, to: Color, gradient: Gradient, radius: f32) {
        self.push(Quad::gradient(rect, from.quad(self.alpha), to.quad(self.alpha), gradient, radius));
    }

    /// A stroke `thickness` wide just inside `rect`'s edge; `radius` is its outer corner radius.
    pub fn ring(&mut self, rect: Rect, color: Color, radius: f32, thickness: f32) {
        self.push(Quad::ring(rect, color.quad(self.alpha), radius, thickness));
    }

    /// A soft band: a ring `thickness` wide whose edges fade over `feather`.
    pub fn glow(&mut self, rect: Rect, color: Color, radius: f32, thickness: f32, feather: f32) {
        self.push(Quad { feather, ..Quad::ring(rect, color.quad(self.alpha), radius, thickness) });
    }

    /// An arc of the circle `rect` holds: a stroke `thickness` wide just inside it, from `from` to `to` degrees
    /// clockwise from the top, its ends round.
    pub fn arc(&mut self, rect: Rect, from: f32, to: f32, thickness: f32, color: Color) {
        self.push(Quad::arc(rect, color.quad(self.alpha), thickness, from.to_radians(), to.to_radians()));
    }

    /// A texture filling `rect`, cropped to its shape.
    pub fn image(&mut self, rect: Rect, tex: TexId, size: (u32, u32), crop: Crop, alpha: f32, radius: f32) {
        let (iw, ih) = (size.0.max(1) as f32, size.1.max(1) as f32);
        // The part of the image, in its own 0-1 space, with the box's shape.
        let (mut u, mut v) = (1.0, 1.0);
        if iw / ih > rect.w / rect.h {
            u = (rect.w / rect.h) * ih / iw;
        } else {
            v = (rect.h / rect.w) * iw / ih;
        }
        let u0 = (1.0 - u) / 2.0;
        let v0 = if crop == Crop::Top { 0.0 } else { (1.0 - v) / 2.0 };
        self.push(Quad::image(rect, tex, [u0, v0, u0 + u, v0 + v], alpha * self.alpha, radius));
    }

    /// As `art`, under a black sheet: `light` of the picture's own colour is left, 1 for all of it. The darkening
    /// is part of the picture, so the two fade as one.
    pub fn art_darkened(&mut self, rect: Rect, key: &ArtKey, crop: Crop, alpha: f32, light: f32) -> bool {
        let before = self.top().quads.len();
        let drawn = self.art(rect, key, crop, alpha, 0.0);
        if let Some(quad) = self.top().quads.get_mut(before) {
            for colour in [&mut quad.c0, &mut quad.c1] {
                colour[..3].iter_mut().for_each(|channel| *channel *= light);
            }
        }
        drawn
    }

    /// A piece of art, all of it, as large as fits in `rect` and in its middle, as WPF's `Stretch="Uniform"` draws
    /// an image; `light` of its own colour is left, as `art_darkened` leaves it (0: its shape in black, for a shadow).
    /// Where it was drawn, once it is decoded; None until then, or if it can't be.
    pub fn art_fitted(&mut self, rect: Rect, key: &ArtKey, alpha: f32, light: f32) -> Option<Rect> {
        let Art::Ready { tex, width, height } = self.art.get(key) else { return None };
        let (iw, ih) = (width.max(1) as f32, height.max(1) as f32);
        let scale = (rect.w / iw).min(rect.h / ih);
        let (w, h) = (iw * scale, ih * scale);
        let fitted = Rect::new(rect.x + (rect.w - w) / 2.0, rect.y + (rect.h - h) / 2.0, w, h);
        let mut quad = Quad::image(fitted, tex, [0.0, 0.0, 1.0, 1.0], alpha * self.alpha, 0.0);
        for colour in [&mut quad.c0, &mut quad.c1] {
            colour[..3].iter_mut().for_each(|channel| *channel *= light);
        }
        self.push(quad);
        Some(fitted)
    }

    /// A piece of art, all of it, stretched over `rect`, which is turned `turn` radians clockwise about its
    /// middle. False until it is decoded.
    pub fn art_turned(&mut self, rect: Rect, key: &ArtKey, alpha: f32, turn: f32) -> bool {
        match self.art.get(key) {
            Art::Ready { tex, .. } => {
                let quad = Quad::image(rect, tex, [0.0, 0.0, 1.0, 1.0], alpha * self.alpha, 0.0);
                self.push(Quad { turn, ..quad });
                true
            }
            _ => false,
        }
    }

    /// As `art`, for what may have no picture at first and be given one while it is on screen (art downloaded while
    /// the Dashboard runs, docs/sources.md "Art"): `key` is None while there is none. Under `id`, a picture that
    /// comes after the first frame fades in over [`ART_FADE_SECONDS`]; one there from the first is drawn at once,
    /// as soon as it is decoded. Gives how much of it shows, 0 while there is none, so that what stands in for it (a
    /// tile's name) can be drawn under it until it is whole.
    pub fn art_arriving(&mut self, id: impl Hash, rect: Rect, key: Option<&ArtKey>, crop: Crop, radius: f32) -> f32 {
        let target = if key.is_some() { 1.0 } else { 0.0 };
        let shown = self.tween(("art arriving", id), target, ART_FADE_SECONDS, Ease::Linear);
        match key {
            Some(key) if shown > 0.0 && self.art(rect, key, crop, shown, radius) => shown,
            _ => 0.0,
        }
    }

    /// A piece of art filling `rect`, once it is decoded; false until then, or if it can't be.
    pub fn art(&mut self, rect: Rect, key: &ArtKey, crop: Crop, alpha: f32, radius: f32) -> bool {
        match self.art.get(key) {
            Art::Ready { tex, width, height } => {
                self.image(rect, tex, (width, height), crop, alpha, radius);
                true
            }
            _ => false,
        }
    }

    // --- Text ---

    fn key(&self, style: &TextStyle, text: &str, width: f32, max_lines: u8, align: HAlign) -> TextKey {
        TextKey {
            text: text.into(),
            size: (style.size * self.px * 10.0).round() as u32,
            weight: style.weight,
            font: style.font,
            width: (width * self.px).round() as u32,
            max_lines,
            align,
        }
    }

    fn sized(&self, m: Measured) -> TextSize {
        TextSize {
            width: m.width / self.px,
            height: m.height() / self.px,
            line_height: m.line_height / self.px,
            lines: m.lines,
        }
    }

    /// One line's size.
    pub fn measure(&mut self, style: &TextStyle, text: &str) -> TextSize {
        let key = self.key(style, text, 0.0, 0, HAlign::Left);
        let measured = self.text.measure(&key);
        self.sized(measured)
    }

    /// The size of text wrapped to `width`, at most `max_lines` lines (0 = any number).
    pub fn measure_wrapped(&mut self, style: &TextStyle, text: &str, width: f32, max_lines: u8) -> TextSize {
        let key = self.key(style, text, width, max_lines, HAlign::Left);
        let measured = self.text.measure(&key);
        self.sized(measured)
    }

    fn push_text(&mut self, key: TextKey, x: f32, y: f32, clip: Rect, color: Color) {
        let color = color.text(self.alpha);
        if color[3] == 0 || key.text.is_empty() {
            return;
        }
        let at = self.place(Rect::new(x, y, 0.0, 0.0));
        let mut clip = self.place(clip);
        if let Some(outer) = self.clip_px() {
            clip = clip.intersect(outer);
        }
        self.top().texts.push(Text { key, x: at.x, y: at.y, clip, color });
    }

    /// One line with its top left at (`x`, `y`). Returns its size.
    pub fn label(&mut self, x: f32, y: f32, text: &str, style: &TextStyle) -> TextSize {
        let size = self.measure(style, text);
        let key = self.key(style, text, 0.0, 0, HAlign::Left);
        self.push_text(key, x, y, self.screen_unoffset(), style.color);
        size
    }

    /// One line whose middle is at (`cx`, `cy`).
    pub fn label_centered(&mut self, cx: f32, cy: f32, text: &str, style: &TextStyle) -> TextSize {
        let size = self.measure(style, text);
        self.label(cx - size.width / 2.0, cy - size.height / 2.0, text, style)
    }

    /// One line ending at `right`, its vertical middle at `cy`.
    pub fn label_right(&mut self, right: f32, cy: f32, text: &str, style: &TextStyle) -> TextSize {
        let size = self.measure(style, text);
        self.label(right - size.width, cy - size.height / 2.0, text, style)
    }

    /// One line starting at `x`, its vertical middle at `cy`.
    pub fn label_middle(&mut self, x: f32, cy: f32, text: &str, style: &TextStyle) -> TextSize {
        let size = self.measure(style, text);
        self.label(x, cy - size.height / 2.0, text, style)
    }

    /// Text set in a box: wrapped to its width, at most `max_lines` lines (0 = as many as fit its height), the last
    /// ending in an ellipsis, and never drawn outside it.
    pub fn paragraph(
        &mut self,
        rect: Rect,
        text: &str,
        style: &TextStyle,
        align: HAlign,
        valign: VAlign,
        max_lines: u8,
    ) -> TextSize {
        let key = self.key(style, text, rect.w, max_lines, align);
        let measured = self.text.measure(&key);
        let size = self.sized(measured);
        let y = match valign {
            VAlign::Top => rect.y,
            VAlign::Center => rect.y + (rect.h - size.height).max(0.0) / 2.0,
            VAlign::Bottom => rect.y + (rect.h - size.height).max(0.0),
        };
        self.push_text(key, rect.x, y, rect, style.color);
        size
    }

    /// The whole canvas, in the coordinates in force: what a label that isn't clipped is clipped to.
    fn screen_unoffset(&self) -> Rect {
        Rect::new(-self.offset.0, -self.offset.1, self.width, self.height).grow(4096.0)
    }

    /// A glyph centred on (`cx`, `cy`), `size` design units tall: from the icon font if it is one of its own (they
    /// are all in Unicode's private use area), else from the symbol font, which is where the Bundle's body font
    /// finds a symbol such as the trophy. Where the font is missing (anywhere but Windows) a small outlined square
    /// stands in, so a screenshot still shows something is there.
    pub fn icon(&mut self, cx: f32, cy: f32, glyph: char, size: f32, color: Color) {
        let font = if ('\u{E000}'..='\u{F8FF}').contains(&glyph) { Font::Icons } else { Font::Symbols };
        if self.text.has(font) {
            let style = TextStyle { font, size, weight: 400, color };
            let mut buffer = [0u8; 4];
            self.label_centered(cx, cy, glyph.encode_utf8(&mut buffer), &style);
        } else {
            let side = size * 0.8;
            let rect = Rect::new(cx - side / 2.0, cy - side / 2.0, side, side);
            self.ring(rect, color, side * 0.2, (size * 0.08).max(1.5));
        }
    }

    // --- Animation ---

    /// See [`Store::tween`].
    pub fn tween(&mut self, key: impl Hash, target: f32, seconds: f32, ease: Ease) -> f32 {
        self.store.tween(anim::id(key), target, seconds, ease)
    }

    /// See [`Store::tween_from`].
    pub fn tween_from(&mut self, key: impl Hash, from: f32, target: f32, seconds: f32, ease: Ease) -> f32 {
        self.store.tween_from(anim::id(key), from, target, seconds, ease)
    }

    /// See [`Store::slide`].
    pub fn slide(&mut self, key: impl Hash, target: f32, seconds: f32, ease: Ease) -> f32 {
        self.store.slide(anim::id(key), target, seconds, ease)
    }

    /// See [`Store::slide_from`].
    pub fn slide_from(&mut self, key: impl Hash, from: f32, target: f32, seconds: f32, ease: Ease) -> f32 {
        self.store.slide_from(anim::id(key), from, target, seconds, ease)
    }

    /// See [`Store::approach`].
    pub fn approach(&mut self, key: impl Hash, target: f32, tau: f32) -> f32 {
        self.store.approach(anim::id(key), target, tau)
    }

    /// Seconds since the first frame, for what turns endlessly. Asking keeps frames coming.
    pub fn time(&mut self) -> f64 {
        self.store.keep_animating();
        self.store.time()
    }
}

/// A solid white texture's id, for a quad with no image.
pub const NO_TEXTURE: TexId = WHITE;

/// How long a picture that comes late takes to fade in (`Ui::art_arriving`).
pub const ART_FADE_SECONDS: f32 = 0.25;
