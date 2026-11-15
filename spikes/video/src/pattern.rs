//! The test clip's picture and sound: colour bars that slide one bar a second, the frame's number in large blocky
//! digits, and a 440 Hz beep at the start of every second with a white square in the corner while it sounds. A
//! screenshot shows which frame is up; the square and the beep together show whether sound and picture agree.

/// How long the beep, and its square, last at the start of each second.
pub const BEEP_SECONDS: f64 = 0.15;
const BEEP_HZ: f64 = 440.0;
const BARS: [[u8; 3]; 8] = [
    [235, 235, 235],
    [235, 235, 16],
    [16, 235, 235],
    [16, 235, 16],
    [235, 16, 235],
    [235, 16, 16],
    [16, 16, 235],
    [40, 40, 40],
];
/// The digits 0-9, three blocks wide and five high: a row per three bits, the top row first.
const DIGITS: [u16; 10] = [
    0b111_101_101_101_111,
    0b010_110_010_010_111,
    0b111_001_111_100_111,
    0b111_001_111_001_111,
    0b101_101_111_001_001,
    0b111_100_111_001_111,
    0b111_100_111_101_111,
    0b111_001_010_010_010,
    0b111_101_111_101_111,
    0b111_101_111_001_111,
];
const COUNTER_DIGITS: u32 = 4;

/// Studio-range BT.709 Y, Cb, Cr of an sRGB colour.
pub fn yuv(rgb: [u8; 3]) -> [u8; 3] {
    let [r, g, b] = rgb.map(|c| c as f64 / 255.0);
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let cb = (b - y) / 1.8556;
    let cr = (r - y) / 1.5748;
    [(16.0 + 219.0 * y).round() as u8, (128.0 + 224.0 * cb).round() as u8, (128.0 + 224.0 * cr).round() as u8]
}

/// One NV12 frame being drawn: a plane of Y, then a half-height plane of Cb, Cr pairs. Both sides are even.
struct Nv12<'a> {
    width: usize,
    height: usize,
    pixels: &'a mut [u8],
}

impl Nv12<'_> {
    /// Fills a rectangle, clipped to the frame and widened to whole chroma pairs.
    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, rgb: [u8; 3]) {
        let [luma, cb, cr] = yuv(rgb);
        let (x0, y0) = (x.min(self.width) & !1, y.min(self.height) & !1);
        let (x1, y1) = ((x + w).min(self.width).next_multiple_of(2), (y + h).min(self.height).next_multiple_of(2));
        for row in y0..y1 {
            self.pixels[row * self.width + x0..row * self.width + x1].fill(luma);
        }
        let chroma = self.width * self.height;
        for row in y0 / 2..y1 / 2 {
            let from = chroma + row * self.width;
            for pair in self.pixels[from + x0..from + x1].as_chunks_mut::<2>().0 {
                *pair = [cb, cr];
            }
        }
    }
}

/// The bytes one NV12 frame takes.
pub fn frame_len(width: u32, height: u32) -> usize {
    width as usize * height as usize * 3 / 2
}

/// Draws frame `number` of a `fps` frames a second clip into `pixels`, which is [`frame_len`] long.
pub fn draw_frame(width: u32, height: u32, number: u32, fps: u32, pixels: &mut [u8]) {
    let mut frame = Nv12 { width: width as usize, height: height as usize, pixels };
    let (w, h) = (frame.width, frame.height);
    assert!(w % 2 == 0 && h % 2 == 0 && frame.pixels.len() == frame_len(width, height));

    // The bars, moving left by one bar's width every second.
    let bar = w.div_ceil(BARS.len()).next_multiple_of(2);
    let shift = (number as usize * bar / fps as usize) & !1;
    let mut x = 0;
    while x < w {
        let at = x + shift;
        let width = (bar - at % bar).min(w - x);
        frame.fill(x, 0, width, h, BARS[at / bar % BARS.len()]);
        x += width;
    }

    // The frame's number on a black panel in the middle.
    let block = (h / 12).min(w / 20).max(2) & !1;
    let (panel_w, panel_h) = ((COUNTER_DIGITS as usize * 4 + 1) * block, 7 * block);
    let (panel_x, panel_y) = (w.saturating_sub(panel_w) / 2, h.saturating_sub(panel_h) / 2);
    frame.fill(panel_x, panel_y, panel_w, panel_h, [16, 16, 16]);
    for place in 0..COUNTER_DIGITS {
        let digit = number / 10u32.pow(COUNTER_DIGITS - 1 - place) % 10;
        let digit_x = panel_x + block + place as usize * 4 * block;
        for cell in 0..15 {
            if DIGITS[digit as usize] >> (14 - cell) & 1 == 1 {
                frame.fill(digit_x + cell % 3 * block, panel_y + block + cell / 3 * block, block, block, [235; 3]);
            }
        }
    }

    // The square that goes with the beep.
    if (number % fps) as f64 / (fps as f64) < BEEP_SECONDS {
        frame.fill(block, block, 2 * block, 2 * block, [255; 3]);
    }
}

/// `count` frames of the clip's sound from frame `first`, as interleaved 16-bit samples, added to `out`.
pub fn tone(first: u64, count: usize, rate: u32, channels: u16, out: &mut Vec<i16>) {
    let rate = rate as f64;
    for index in first..first + count as u64 {
        let second = (index % rate as u64) as f64 / rate;
        // Five milliseconds of fade at each end, so the beep doesn't click.
        let envelope = (second / 0.005).min((BEEP_SECONDS - second) / 0.005).clamp(0.0, 1.0);
        let sample = (std::f64::consts::TAU * BEEP_HZ * second).sin() * envelope * 0.25 * i16::MAX as f64;
        out.extend(std::iter::repeat_n(sample as i16, channels as usize));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: (u32, u32) = (480, 240);

    fn frame(number: u32) -> Vec<u8> {
        let mut pixels = vec![0; frame_len(SIZE.0, SIZE.1)];
        draw_frame(SIZE.0, SIZE.1, number, 30, &mut pixels);
        pixels
    }

    fn luma(pixels: &[u8], x: u32, y: u32) -> u8 {
        pixels[(y * SIZE.0 + x) as usize]
    }

    #[test]
    fn white_black_and_red_in_studio_range() {
        assert_eq!(yuv([255, 255, 255]), [235, 128, 128]);
        assert_eq!(yuv([0, 0, 0]), [16, 128, 128]);
        assert_eq!(yuv([255, 0, 0]), [63, 102, 240]);
    }

    #[test]
    fn the_counter_shows_the_frame_number() {
        // Blocks are 20 pixels; the panel is 17 by 7 blocks, centred, with a block of margin: the last digit's
        // cells start at x 330, y 70.
        let cell = |pixels: &[u8], column: u32, row: u32| luma(pixels, 340 + column * 20, 80 + row * 20) > 128;
        let seven = frame(7);
        assert!(cell(&seven, 0, 0) && cell(&seven, 1, 0) && cell(&seven, 2, 0));
        assert!(!cell(&seven, 0, 1) && cell(&seven, 2, 1) && cell(&seven, 1, 4) && !cell(&seven, 2, 4));
        let eight = frame(8);
        assert!(cell(&eight, 0, 1) && cell(&eight, 0, 4) && cell(&eight, 2, 4) && !cell(&eight, 1, 1));
    }

    #[test]
    fn the_bars_move_and_fill_the_frame() {
        let (first, later) = (frame(0), frame(15));
        // Frame 0 starts on the white bar; half a second on, half a bar has gone by and yellow is at the right of it.
        assert_eq!(luma(&first, 4, 200), yuv(BARS[0])[0]);
        assert_ne!(first[..SIZE.0 as usize], later[..SIZE.0 as usize]);
        // No pixel is left undrawn, luma or chroma.
        assert!(later.iter().all(|&byte| byte != 0));
    }

    #[test]
    fn the_square_shows_only_while_the_beep_sounds() {
        // White over the yellow bar that is there in both frames.
        assert_eq!(luma(&frame(30), 30, 30), 235);
        assert_eq!(luma(&frame(40), 30, 30), yuv([235, 235, 16])[0]);
    }

    #[test]
    fn a_beep_at_the_start_of_each_second_then_silence() {
        let mut samples = Vec::new();
        tone(0, 96_000, 48_000, 2, &mut samples);
        assert_eq!(samples.len(), 192_000);
        let loud = |from: usize, to: usize| samples[2 * from..2 * to].iter().any(|s| s.unsigned_abs() > 4000);
        assert!(loud(480, 6_000) && loud(48_480, 54_000));
        assert!(samples[2 * 7_300..2 * 48_000].iter().all(|&s| s == 0));
        // Both channels carry the same sample.
        assert!(samples.chunks(2).all(|pair| pair[0] == pair[1]));
    }
}
