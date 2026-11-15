//! What the Overlay draws, and what that should leave in the back buffer and on the screen.
//!
//! A 60% black dim over everything, a translucent panel down the left with a few lines of text (the Guide's stand-in)
//! and a small opaque green banner near the bottom centre (a Toast's).
//!
//! # Is what the renderer writes premultiplied, as the DWM wants?
//!
//! The swap chain is `DXGI_ALPHA_MODE_PREMULTIPLIED`: the DWM composes `screen = buffer.rgb + (1 - buffer.a) * behind`.
//!
//! - **Alpha: yes.** `quad.wgsl` returns `(rgb * a, a)` and the pipeline blends with `PREMULTIPLIED_ALPHA_BLENDING`
//!   (`One, OneMinusSrcAlpha` on colour and on alpha), so over a target cleared to `(0, 0, 0, 0)` the alpha channel
//!   ends as the coverage `a_src + a_dst * (1 - a_src)`. glyphon's text pipeline is `ALPHA_BLENDING` (`SrcAlpha,
//!   OneMinusSrcAlpha` on colour; `One, OneMinusSrcAlpha` on alpha), which over a premultiplied target leaves the same
//!   thing: premultiplied colour and coverage alpha.
//! - **Colour: yes in the blend's own space, and that is the catch.** On an sRGB target ([`Blend::Linear`], what the
//!   Dashboard uses) the blend runs in linear light and the buffer stores `encode(rgb_linear * a)`. A DWM that
//!   composes the stored bytes as they are (an SDR desktop) wants `encode(rgb_linear) * a`. The two agree where
//!   `a` is 0 or 1 and where the colour is black, so the dim, the opaque banner and opaque text are exact, and the
//!   near-black panel at 85% is within a count or two. They disagree for a light colour at a low alpha: white at 10%
//!   is stored as 89 where 26 is meant, a colour above its alpha, which the DWM adds as a glow. On a non-sRGB target
//!   ([`Blend::Gamma`], `--gamma`) the blend runs on the encoded values, the stored colour is exactly `encoded * a`,
//!   and the result is what WPF and the Bundle's alpha values mean. A DWM that composes in linear light (HDR or
//!   Advanced Colour on) would make the sRGB target the exact one instead.
//!
//! `--selfcheck` logs what each of the two ways of composing predicts beside what the screen shows, so a run says which
//! one the DWM used.
use render::{rgba, Font, Frame, HAlign, Layer, Quad, Rect, Rgba, Text, TextKey};

pub const PANEL_WIDTH: f32 = 540.0;
const DIM: (u32, f32) = (0x000000, 0.6);
const PANEL: (u32, f32) = (0x1C1C1C, 0.85);
const BANNER: (u32, f32) = (0x107C10, 1.0);

/// The space the target blends in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    /// An sRGB target: colours are linear, the GPU encodes on the way out.
    Linear,
    /// A plain 8-bit target: colours are the encoded values, blended as they are.
    Gamma,
}

fn encode(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn decode(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `0xRRGGBB` at an alpha, straight, in the space `blend` blends in.
fn colour((hex, a): (u32, f32), blend: Blend) -> Rgba {
    match blend {
        // What an sRGB target would be given: linear light, encoded by the GPU on the way out.
        Blend::Linear => {
            let [r, g, b, a] = rgba(hex, a);
            [decode(r), decode(g), decode(b), a]
        }
        Blend::Gamma => rgba(hex, a),
    }
}

fn panel(size: (u32, u32)) -> Rect {
    Rect::new(0.0, 0.0, PANEL_WIDTH, size.1 as f32)
}

fn banner(size: (u32, u32)) -> Rect {
    let (w, h) = (420.0, 72.0);
    Rect::new(((size.0 as f32 - w) / 2.0).round(), size.1 as f32 - h - 48.0, w, h)
}

/// The Overlay's one frame. `lines` go in the panel, the first as its title.
pub fn frame(size: (u32, u32), blend: Blend, lines: &[String]) -> Frame {
    let screen = Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32);
    let (panel, banner) = (panel(size), banner(size));
    let white = [255, 255, 255, 255];
    let mut layer = Layer::default();
    layer.quads.push(Quad::fill(screen, colour(DIM, blend), 0.0));
    layer.quads.push(Quad::fill(panel, colour(PANEL, blend), 16.0));
    layer.quads.push(Quad::fill(banner, colour(BANNER, blend), 8.0));
    let key = |text: &str, size: u32, weight: u16, width: u32, align: HAlign| TextKey {
        text: text.to_string(),
        size,
        weight,
        font: Font::Ui,
        width,
        max_lines: 1,
        align,
    };
    let mut y = 56.0;
    for (i, line) in lines.iter().enumerate() {
        let title = i == 0;
        let key = key(line, if title { 320 } else { 200 }, if title { 600 } else { 400 }, 0, HAlign::Left);
        layer.texts.push(Text { key, x: 40.0, y, clip: panel, color: white });
        y += if title { 60.0 } else { 34.0 };
    }
    let key = key("Toast stand-in", 200, 600, banner.w as u32, HAlign::Center);
    layer.texts.push(Text { key, x: banner.x, y: banner.y + 22.0, clip: banner, color: white });
    Frame { layers: vec![layer] }
}

/// A pixel to look at, clear of any text.
#[derive(Debug, Clone, Copy)]
pub struct Probe {
    pub name: &'static str,
    pub x: i32,
    pub y: i32,
    /// What the back buffer should hold there: premultiplied RGB as stored, and alpha, each 0 to 1.
    pub stored: [f32; 4],
}

/// One pixel inside the panel, one in the dimmed area, one in the banner.
pub fn probes(size: (u32, u32), blend: Blend) -> [Probe; 3] {
    // Layers bottom to top, straight alpha in the blend's space, laid over a transparent target as the GPU does.
    let stored = |layers: &[(u32, f32)]| {
        let mut out = [0.0f32; 4];
        for &layer in layers {
            let [r, g, b, a] = colour(layer, blend);
            for (o, c) in out.iter_mut().zip([r, g, b]) {
                *o = c * a + *o * (1.0 - a);
            }
            out[3] = a + out[3] * (1.0 - a);
        }
        if blend == Blend::Linear {
            for c in &mut out[..3] {
                *c = encode(*c);
            }
        }
        out
    };
    let (w, h) = (size.0 as f32, size.1 as f32);
    let banner = banner(size);
    [
        Probe { name: "panel", x: (PANEL_WIDTH / 2.0) as i32, y: (h * 0.75) as i32, stored: stored(&[DIM, PANEL]) },
        Probe { name: "dim", x: (w * 0.75) as i32, y: (h * 0.4) as i32, stored: stored(&[DIM]) },
        Probe {
            name: "banner",
            x: (banner.x + 30.0) as i32,
            y: (banner.y + banner.h / 2.0) as i32,
            stored: stored(&[DIM, BANNER]),
        },
    ]
}

/// What the screen should show where the back buffer holds `stored` over `behind`, composed the two ways a DWM might:
/// on the stored bytes as they are, and in linear light.
pub fn composed(stored: [f32; 4], behind: [u8; 3]) -> ([u8; 3], [u8; 3]) {
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    let through = 1.0 - stored[3];
    let mut as_stored = [0; 3];
    let mut linear = [0; 3];
    for i in 0..3 {
        let b = behind[i] as f32 / 255.0;
        as_stored[i] = byte(stored[i] + through * b);
        linear[i] = byte(encode(decode(stored[i]) + through * decode(b)));
    }
    (as_stored, linear)
}

/// Whether two colours are the same to within `by` counts a channel.
pub fn close(a: [u8; 3], b: [u8; 3], by: u8) -> bool {
    a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= by)
}

#[cfg(test)]
mod tests {
    use super::*;
    use render::gpu;
    use render::offscreen::read_back;

    const SIZE: (u32, u32) = (1280, 720);

    #[test]
    fn the_dim_stores_black_at_60_percent_and_lets_40_percent_through() {
        for blend in [Blend::Linear, Blend::Gamma] {
            let dim = probes(SIZE, blend)[1];
            assert_eq!(dim.stored, [0.0, 0.0, 0.0, 0.6]);
            let (as_stored, linear) = composed(dim.stored, [255, 255, 255]);
            assert_eq!(as_stored, [102, 102, 102]);
            // The same 40% of the light, which is a lighter grey once encoded.
            assert_eq!(linear, [170, 170, 170]);
        }
    }

    #[test]
    fn the_banner_is_opaque_whatever_is_behind() {
        let banner = probes(SIZE, Blend::Gamma)[2];
        assert_eq!(banner.stored[3], 1.0);
        let (as_stored, linear) = composed(banner.stored, [200, 30, 90]);
        assert_eq!(as_stored, [0x10, 0x7C, 0x10]);
        assert_eq!(linear, [0x10, 0x7C, 0x10]);
    }

    #[test]
    fn the_probes_are_where_they_say() {
        for size in [(1280, 720), (1920, 1080), (3840, 2160)] {
            let [panel, dim, banner] = probes(size, Blend::Linear);
            assert!(panel.x < PANEL_WIDTH as i32);
            assert!(dim.x > PANEL_WIDTH as i32 && dim.y < banner.y - 40);
            assert!((banner.x - size.0 as i32 / 2).abs() < 210);
        }
    }

    /// The renderer, clearing to transparent, leaves in the target what `probes` says: so what `--selfcheck` expects
    /// of the screen starts from what the back buffer really holds. Drawn offscreen, as the screenshot tests are.
    #[test]
    fn the_renderer_writes_what_the_probes_expect() {
        let gpu = gpu::init(gpu::instance(None), None);
        let lines = ["Overlay spike".to_string(), "a line of text".to_string()];
        // The renderer blends as encoded, on a plain 8-bit target, and nothing else.
        for (blend, format) in [(Blend::Gamma, wgpu::TextureFormat::Rgba8Unorm)] {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("overlay"),
                size: wgpu::Extent3d { width: SIZE.0, height: SIZE.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let mut renderer = render::Renderer::new(&gpu.device, &gpu.queue, format);
            let frame = frame(SIZE, blend, &lines);
            renderer.render(&gpu.device, &gpu.queue, &view, SIZE, &frame, wgpu::Color::TRANSPARENT);
            let pixels = read_back(&gpu, &texture, SIZE);
            for probe in probes(SIZE, blend) {
                let at = 4 * (probe.y as usize * SIZE.0 as usize + probe.x as usize);
                let want = probe.stored.map(|c| (c * 255.0).round() as u8);
                for channel in 0..4 {
                    let got = pixels[at + channel];
                    assert!(
                        got.abs_diff(want[channel]) <= 1,
                        "{blend:?} {}: drew {:?}, expected {want:?}",
                        probe.name,
                        &pixels[at..at + 4]
                    );
                }
            }
        }
    }
}
