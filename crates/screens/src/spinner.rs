//! The Spinner, the console's loading spinner: a white arc with a green tail that grows clockwise from a dot at the
//! top, its tail chasing its head round, resting as the dot before the next turn. docs/spec/home.md section 10.1 has
//! the Bundle's formula, which this is.
use ui::{Color, Rect, Ui};

use crate::theme;

/// One turn and its rest, in seconds.
pub const PERIOD: f64 = 1.2;
/// How long the dot is that the Spinner starts and rests as, in degrees.
const DOT: f64 = 6.0;

/// Where the Spinner's arcs are at one moment, in degrees clockwise from the top: white from `white_tail` to
/// `head`, green from `green_tail` to `white_tail`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arcs {
    pub head: f32,
    pub white_tail: f32,
    pub green_tail: f32,
}

/// The arcs `seconds` into the spinning, which repeats each [`PERIOD`].
pub fn arcs(seconds: f64) -> Arcs {
    let t = seconds.rem_euclid(PERIOD);
    // Each end eases round once over its own stretch of the turn: the head first, then where white turns green,
    // then the green's end, each later and steeper, so the arc grows long and shrinks back to the dot.
    let head = DOT + 360.0 * bezier(progress(t, 0.00, 0.80), 0.70, 0.20, 0.25, 0.95);
    let white_tail = (360.0 * bezier(progress(t, 0.15, 0.85), 0.75, 0.20, 0.15, 0.60)).min(head - DOT);
    let green_tail = (360.0 * bezier(progress(t, 0.25, 0.90), 0.70, 0.10, 0.15, 0.75)).min(white_tail);
    Arcs { head: head as f32, white_tail: white_tail as f32, green_tail: green_tail as f32 }
}

fn progress(t: f64, start: f64, end: f64) -> f64 {
    ((t - start) / (end - start)).clamp(0.0, 1.0)
}

/// CSS's `cubic-bezier(x1, y1, x2, y2)` at `x`.
fn bezier(x: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let curve = |u: f64, p1: f64, p2: f64| {
        let v = 1.0 - u;
        3.0 * p1 * u * v * v + 3.0 * p2 * u * u * v + u * u * u
    };
    // The curve's x only ever grows along it, so halving the range finds the point on it with this x.
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..40 {
        let middle = (low + high) / 2.0;
        if curve(middle, x1, x2) < x {
            low = middle;
        } else {
            high = middle;
        }
    }
    curve((low + high) / 2.0, y1, y2)
}

/// The Spinner `size` across, centred on (`cx`, `cy`), where it is at this moment. It turns for as long as it is
/// drawn, Reduce motion or not.
pub fn spinner(ui: &mut Ui, cx: f32, cy: f32, size: f32) {
    // Authored on a canvas of 100: a ring of radius 46 to its stroke's middle, the stroke 8, with round ends and no
    // track under it.
    let ring = Rect::new(cx - size / 2.0, cy - size / 2.0, size, size);
    let stroke = size * 0.08;
    let at = arcs(ui.time());
    if at.white_tail - at.green_tail > 0.01 {
        // A degree on under the white, so no seam shows where they meet.
        let end = (at.white_tail + 1.0).min(at.head);
        ui.arc(ring, at.green_tail, end, stroke, theme::ACCENT_LIGHT);
    }
    ui.arc(ring, at.white_tail, at.head, stroke, Color::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_starts_and_rests_as_a_dot_at_the_top() {
        assert_eq!(arcs(0.0), Arcs { head: 6.0, white_tail: 0.0, green_tail: 0.0 });
        // Round once, and at the top again: the same dot a turn on.
        for rest in [0.9, 1.0, 1.19] {
            assert_eq!(arcs(rest), Arcs { head: 366.0, white_tail: 360.0, green_tail: 360.0 });
        }
    }

    #[test]
    fn every_turn_is_the_same() {
        let (first, third) = (arcs(0.5), arcs(0.5 + PERIOD * 2.0));
        assert!((first.head - third.head).abs() < 0.01 && (first.green_tail - third.green_tail).abs() < 0.01);
    }

    #[test]
    fn the_tail_chases_the_head_and_never_passes_it() {
        let mut longest = 0.0f32;
        for step in 0..=1200 {
            let at = arcs(step as f64 / 1000.0);
            assert!(at.green_tail <= at.white_tail && at.white_tail <= at.head - 6.0 + 1e-3, "{step}: {at:?}");
            longest = longest.max(at.head - at.green_tail);
        }
        assert!(longest > 180.0 && longest < 360.0, "the arc grows long, never to a whole ring: {longest}");
    }

    #[test]
    fn mid_turn_the_head_leads_white_then_green() {
        let at = arcs(0.5);
        assert!(at.head > at.white_tail && at.white_tail > at.green_tail && at.green_tail > 0.0, "{at:?}");
    }
}
