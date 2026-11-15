//! Where the picture goes on the screen.

/// The largest rectangle of the picture's shape that fits the screen, centred, as `[x, y, width, height]` in whole
/// pixels: bars above and below a wider picture, left and right of a narrower one. `picture` is in the video's own
/// pixels, each `pixel_aspect.0 : pixel_aspect.1` wide to high.
pub fn letterbox(screen: (u32, u32), picture: (u32, u32), pixel_aspect: (u32, u32)) -> [f32; 4] {
    let (sw, sh) = (screen.0 as f64, screen.1 as f64);
    let wide = picture.0 as f64 * pixel_aspect.0.max(1) as f64;
    let high = picture.1 as f64 * pixel_aspect.1.max(1) as f64;
    if wide <= 0.0 || high <= 0.0 {
        return [0.0, 0.0, sw as f32, sh as f32];
    }
    let scale = (sw / wide).min(sh / high);
    let (w, h) = ((wide * scale).round(), (high * scale).round());
    [((sw - w) / 2.0).floor() as f32, ((sh - h) / 2.0).floor() as f32, w as f32, h as f32]
}

#[cfg(test)]
mod tests {
    use super::letterbox;

    #[test]
    fn the_same_shape_fills_the_screen() {
        assert_eq!(letterbox((1920, 1080), (1280, 720), (1, 1)), [0.0, 0.0, 1920.0, 1080.0]);
        assert_eq!(letterbox((1920, 1080), (3840, 2160), (1, 1)), [0.0, 0.0, 1920.0, 1080.0]);
    }

    #[test]
    fn a_wider_picture_gets_bars_above_and_below() {
        assert_eq!(letterbox((1920, 1080), (1920, 800), (1, 1)), [0.0, 140.0, 1920.0, 800.0]);
    }

    #[test]
    fn a_narrower_picture_gets_bars_left_and_right() {
        assert_eq!(letterbox((1920, 1080), (640, 480), (1, 1)), [240.0, 0.0, 1440.0, 1080.0]);
    }

    #[test]
    fn pixels_that_are_not_square_change_the_shape() {
        // 720x480 with 32:27 pixels is 16:9.
        assert_eq!(letterbox((1920, 1080), (720, 480), (32, 27)), [0.0, 0.0, 1920.0, 1080.0]);
    }
}
