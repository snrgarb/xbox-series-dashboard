//! The Toast's banner (docs/spec/guide.md, 9.2 and 9.3): a green card low at the screen's middle, a tile at its left
//! and a line beside it that takes turns with "Press (Guide) to view". It grows from its middle as it fades in, and
//! goes by growing a little, then shrinking as it fades out. Where it is in all that is told by how long it has
//! been showing and going, so nothing here is remembered between frames.
use ui::{Color, Ease, Font, HAlign, Rect, TextStyle, Ui, VAlign};

use super::{Toast, ToastKind};
use crate::guide::marks;
use crate::home::profile_picture;
use crate::theme::{glyph, ACCENT};

/// The card is 128 high, its foot 64 above the screen's, and as wide as what is on it, to 640 at most.
const TOP: f32 = 888.0;
const HEIGHT: f32 = 128.0;
const MAX_WIDTH: f32 = 640.0;
/// 16 left of the tile, the tile's 96, 24 between it and the text, 32 right of the text.
const PAD: f32 = 16.0;
const TILE: f32 = 96.0;
const TILE_GAP: f32 = 24.0;
const PAD_RIGHT: f32 = 32.0;
const AROUND_TEXT: f32 = PAD + TILE + TILE_GAP + PAD_RIGHT;
const LINE: TextStyle = TextStyle::new(Font::Ui, 34.0, 400);
/// The lighter green of a tile that holds its kind's icon: white at 15%.
const TILE_FILL: Color = Color::argb(0x26FFFFFF);
/// The Guide button's mark in the prompt: 32 across, 12 either side, and under a margin of 2, which puts its
/// middle 1 below the text's.
const MARK: f32 = 32.0;
const MARK_GAP: f32 = 12.0;
const MARK_DOWN: f32 = 1.0;

/// Coming in: from 0.3 of its size to all of it, fading in, over 300 ms.
const IN_SECONDS: f32 = 0.3;
const GROW_FROM: f32 = 0.3;
/// A sliding kind's tile and text start a tile and its gap to the left, and slide to their place over 450 ms from
/// a second in.
const SLIDE_AT: f32 = 1.0;
const SLIDE_SECONDS: f32 = 0.45;
const SLIDE_BY: f32 = TILE + TILE_GAP;
/// Going: to 1.1 over 120 ms, then to 0.9 over 180 ms, fading out over 200 ms from 210 ms in.
const POP_TO: f32 = 1.1;
const POP_SECONDS: f32 = 0.12;
const SHRINK_TO: f32 = 0.9;
const SHRINK_SECONDS: f32 = 0.18;
const FADE_AT: f32 = 0.21;
const FADE_SECONDS: f32 = 0.2;
pub const OUT_SECONDS: f32 = FADE_AT + FADE_SECONDS;
/// The text's turns: the line for 2.2 s, 0.3 s fading to the prompt, the prompt for 2.2 s, 0.3 s fading back.
const TURN_SECONDS: f32 = 5.0;
const HOLD_SECONDS: f32 = 2.2;
const CROSS_SECONDS: f32 = 0.3;

/// How the banner stands at a moment.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pose {
    /// The card's size about its middle, 1 at rest.
    scale: f32,
    opacity: f32,
    /// How far left of their place the tile and the text are.
    shift: f32,
    /// The tile's own opacity.
    tile: f32,
    /// The line's opacity; the prompt has the rest.
    line: f32,
    /// Whether the next moment looks different: frames are wanted.
    moving: bool,
    /// Else how long until it does, if it will before the Toast goes.
    still_for: Option<f32>,
}

impl Pose {
    /// In, in place, on its line.
    const REST: Pose =
        Pose { scale: 1.0, opacity: 1.0, shift: 0.0, tile: 1.0, line: 1.0, moving: false, still_for: None };
}

/// WPF's CubicEase EaseInOut.
fn cubic_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// The banner `age` seconds after it began to appear and, if it is going, `going` seconds after it began to.
/// `slides`: its tile slides in. `prompt`: its line takes turns with the prompt. Under Reduce motion nothing grows
/// or slides, and the fades stay.
fn pose(slides: bool, prompt: bool, age: f32, going: Option<f32>, reduce_motion: bool) -> Pose {
    // It goes from wherever it had got to coming in.
    let came = Ease::CubicOut.at((age - going.unwrap_or(0.0)) / IN_SECONDS);
    let grown = 1.0 - (1.0 - GROW_FROM) * (1.0 - came);
    let (scale, opacity) = match going {
        None => (grown, came),
        Some(gone) => {
            let scale = if gone < POP_SECONDS {
                grown + (POP_TO - grown) * Ease::CubicOut.at(gone / POP_SECONDS)
            } else {
                POP_TO + (SHRINK_TO - POP_TO) * cubic_in_out((gone - POP_SECONDS) / SHRINK_SECONDS)
            };
            (scale, came * (1.0 - ((gone - FADE_AT) / FADE_SECONDS).clamp(0.0, 1.0)))
        }
    };
    let slid = if slides && !reduce_motion { Ease::CubicOut.at((age - SLIDE_AT) / SLIDE_SECONDS) } else { 1.0 };
    let turn = age % TURN_SECONDS;
    let line = if !prompt || turn < HOLD_SECONDS {
        1.0
    } else if turn < HOLD_SECONDS + CROSS_SECONDS {
        1.0 - (turn - HOLD_SECONDS) / CROSS_SECONDS
    } else if turn < TURN_SECONDS - CROSS_SECONDS {
        0.0
    } else {
        (turn - (TURN_SECONDS - CROSS_SECONDS)) / CROSS_SECONDS
    };

    // What moves, each from when to when; the turns of text come round every five seconds.
    let mut moves = vec![(0.0, IN_SECONDS)];
    if slides && !reduce_motion {
        moves.push((SLIDE_AT, SLIDE_AT + SLIDE_SECONDS));
    }
    if prompt {
        let round = age - turn;
        moves.push((round + HOLD_SECONDS, round + HOLD_SECONDS + CROSS_SECONDS));
        moves.push((round + TURN_SECONDS - CROSS_SECONDS, round + TURN_SECONDS));
    }
    let moving = going.is_some() || moves.iter().any(|&(from, to)| (from..to).contains(&age));
    let next = moves.iter().map(|&(from, _)| from - age).filter(|&wait| wait > 0.0).reduce(f32::min);
    Pose {
        scale: if reduce_motion { 1.0 } else { scale },
        opacity,
        shift: -SLIDE_BY * (1.0 - slid),
        tile: slid,
        line,
        moving,
        still_for: next.filter(|_| !moving),
    }
}

/// Draws the Toast, over everything drawn so far. A frame that is settling shows it at rest, and one that is going
/// not at all.
pub fn draw(ui: &mut Ui, toast: &Toast, age: f32, going: Option<f32>) {
    let slides = toast.kind == ToastKind::Greeting;
    let pose = if ui.store.settling() {
        if going.is_some() {
            return;
        }
        Pose::REST
    } else {
        pose(slides, toast.prompt, age, going, ui.store.reduce_motion)
    };
    if pose.moving {
        ui.store.keep_animating();
    } else if let Some(seconds) = pose.still_for {
        ui.store.wake_in(seconds);
    }
    if pose.opacity <= 0.0 {
        return;
    }

    // The line and the prompt share one place: the card is as wide as the longer, whichever shows.
    let line_width = ui.measure(&LINE, &toast.text).width;
    let press = ui.measure(&LINE, "Press").width;
    let to_view = ui.measure(&LINE, "to view").width;
    let prompt_width = press + MARK_GAP + MARK + MARK_GAP + to_view;
    let text_width = line_width.max(prompt_width).min(MAX_WIDTH - AROUND_TEXT);
    let width = AROUND_TEXT + text_width;
    let card = Rect::new((ui::DESIGN_WIDTH - width) / 2.0, TOP, width, HEIGHT);

    // Everything is laid out at its full size and drawn `scale` of it about the card's middle. Type comes in whole
    // sizes while it grows, so a Toast's coming and going set its line in a handful of sizes, not one a frame.
    let (cx, cy) = card.center();
    let scale = pose.scale;
    let across = |x: f32| cx + (x - cx) * scale;
    let at = |rect: Rect| Rect::new(across(rect.x), cy + (rect.y - cy) * scale, rect.w * scale, rect.h * scale);
    let size = if scale == 1.0 { LINE.size } else { (LINE.size * scale).round().max(1.0) };
    let style = LINE.size(size);

    ui.layer();
    ui.with_alpha(pose.opacity, |ui| {
        ui.fill(at(card), ACCENT, 6.0 * scale);
        // Cut to the card, so a sliding tile comes out from behind its left edge.
        ui.with_clip(at(card), |ui| {
            let tile = Rect::new(card.x + PAD + pose.shift, card.y + PAD, TILE, TILE);
            ui.with_alpha(pose.tile, |ui| match toast.kind {
                ToastKind::Notification => {
                    ui.fill(at(tile), TILE_FILL, 4.0 * scale);
                    ui.icon(across(tile.center().0), cy, glyph::NOTIFICATIONS, 44.0 * scale, Color::WHITE);
                }
                // The profile's picture, round and 80 across, bare on the card.
                ToastKind::Greeting => profile_picture(ui, at(tile.grow(-8.0))),
            });

            let text = tile.right() + TILE_GAP;
            ui.with_alpha(pose.line, |ui| {
                if line_width <= text_width {
                    ui.label_middle(across(text), cy, &toast.text, &style);
                } else {
                    let room = at(Rect::new(text, card.y, text_width, HEIGHT));
                    ui.paragraph(room, &toast.text, &style, HAlign::Left, VAlign::Center, 1);
                }
            });
            ui.with_alpha(1.0 - pose.line, |ui| {
                ui.label_middle(across(text), cy, "Press", &style);
                let mark = Rect::new(text + press + MARK_GAP, cy - MARK / 2.0 + MARK_DOWN, MARK, MARK);
                marks::xbox_mark(ui, at(mark), Color::WHITE);
                ui.label_middle(across(mark.right() + MARK_GAP), cy, "to view", &style);
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn it_grows_from_three_tenths_as_it_fades_in_over_300_ms() {
        let start = pose(false, true, 0.0, None, false);
        assert!(close(start.scale, 0.3) && close(start.opacity, 0.0) && start.moving);
        let half = pose(false, true, 0.15, None, false);
        assert!(close(half.scale, 0.3 + 0.7 * 0.875) && close(half.opacity, 0.875));
        let whole = pose(false, true, 0.3, None, false);
        assert_eq!(Pose { still_for: None, ..whole }, Pose::REST);
        // Nothing moves again until the line gives way to the prompt.
        assert!(close(whole.still_for.unwrap(), 1.9));
    }

    #[test]
    fn the_line_and_the_prompt_take_turns_over_five_seconds() {
        let line = |age: f32| pose(false, true, age, None, false).line;
        assert_eq!((line(1.0), line(2.19)), (1.0, 1.0));
        assert!(close(line(2.35), 0.5));
        assert_eq!((line(2.5), line(4.69)), (0.0, 0.0));
        assert!(close(line(4.85), 0.5));
        assert!(pose(false, true, 2.3, None, false).moving && pose(false, true, 4.8, None, false).moving);
        assert!(close(pose(false, true, 3.0, None, false).still_for.unwrap(), 1.7));
        // With no prompt there is only the line, and nothing more to draw once it is in.
        let alone = pose(false, false, 3.0, None, false);
        assert_eq!((alone.line, alone.moving, alone.still_for), (1.0, false, None));
    }

    #[test]
    fn a_sliding_kind_holds_its_tile_back_a_second_then_slides_it_in() {
        let held = pose(true, false, 0.5, None, false);
        assert_eq!((held.shift, held.tile, held.moving), (-120.0, 0.0, false));
        assert!(close(held.still_for.unwrap(), 0.5));
        let sliding = pose(true, false, 1.225, None, false);
        assert!(close(sliding.shift, -120.0 * 0.125) && close(sliding.tile, 0.875) && sliding.moving);
        let there = pose(true, false, 1.45, None, false);
        assert_eq!((there.shift, there.tile, there.moving), (0.0, 1.0, false));
        // Any other kind's is in its place from the start.
        assert_eq!(pose(false, false, 0.5, None, false).shift, 0.0);
    }

    #[test]
    fn it_goes_by_growing_a_little_then_shrinking_as_it_fades() {
        let going = |gone: f32| pose(false, false, 5.0 + gone, Some(gone), false);
        assert!(close(going(0.0).scale, 1.0) && close(going(0.0).opacity, 1.0) && going(0.0).moving);
        assert!(close(going(0.12).scale, 1.1) && close(going(0.12).opacity, 1.0));
        assert!(close(going(0.21).scale, 1.0) && close(going(0.21).opacity, 1.0));
        assert!(close(going(0.3).scale, 0.9));
        assert!(close(going(0.31).opacity, 0.5));
        assert!(close(going(OUT_SECONDS).opacity, 0.0));
        // Sent away part way in, it goes from where it had got to.
        let early = pose(false, false, 0.15, Some(0.0), false);
        assert!(close(early.scale, 0.3 + 0.7 * 0.875) && close(early.opacity, 0.875));
    }

    #[test]
    fn under_reduce_motion_it_only_fades() {
        let coming = pose(true, false, 0.15, None, true);
        assert_eq!((coming.scale, coming.shift, coming.tile), (1.0, 0.0, 1.0));
        assert!(close(coming.opacity, 0.875));
        let going = pose(true, false, 5.31, Some(0.31), true);
        assert!(going.scale == 1.0 && close(going.opacity, 0.5));
    }
}
