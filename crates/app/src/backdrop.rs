//! When Home's window is drawn while a game is up, and what the log says of it (docs/spec/launch-screen.md). Under a
//! game Home's window is the game's backdrop, the Launch screen's picture without the Spinner: nothing in it moves, so
//! it is drawn once it covers, again only when its art comes in or the window changes size, and never while it sits
//! behind the game. Once the game has ended it fades to Home, and the log says when Home is all there is.
use crate::since_start;

#[derive(Default)]
pub struct Frames {
    /// The backdrop has been drawn whole since Home's window was last uncovered: no more frames until it changes.
    drawn: bool,
    /// Art was still being decoded when it was last drawn: it is drawn again when art comes in.
    waits_art: bool,
    /// Whether this launch's backdrop has been said to be up.
    said: bool,
    /// Fading to Home: the game has ended, and Home's being back is still to be said.
    fading: bool,
}

impl Frames {
    /// Whether Home's window wants a frame for the backdrop: it covers Home's window (`covered`), and has not been
    /// drawn whole since it began to.
    pub fn due(&mut self, covered: bool) -> bool {
        if !covered {
            self.drawn = false;
        }
        covered && !self.drawn
    }

    /// Whether a frame Home's window was asked for is to be left undrawn: the backdrop covers it, and is drawn.
    pub fn skip(&self, covered: bool) -> bool {
        covered && self.drawn
    }

    /// A frame of Home's window was presented: `covered`, it was the backdrop; `showing`, the backdrop was in it,
    /// whole or fading. `moving`: something in it is still moving. `whole`: the backdrop's art was in (or it has
    /// none), as the picture itself said when it was drawn. `size`: the frame's, in pixels. Each frame of the
    /// backdrop is said, with its size and whether its art was in: there are few.
    pub fn presented(&mut self, covered: bool, showing: bool, moving: bool, whole: bool, size: (u32, u32)) {
        if covered {
            self.drawn = !moving;
            self.waits_art = !whole;
            if !self.said {
                self.said = true;
                eprintln!("launch: backdrop up at {:.0} ms", since_start());
            }
            let art = if whole { "its art in" } else { "its art still to come" };
            eprintln!("launch: backdrop drawn, {}x{}, {art}, at {:.0} ms", size.0, size.1, since_start());
        }
        if self.fading && !showing {
            self.fading = false;
            eprintln!("launch: Home back at {:.0} ms", since_start());
        }
    }

    /// Art came in: a backdrop drawn without its art is drawn again.
    pub fn art_came(&mut self) {
        if self.waits_art {
            self.drawn = false;
        }
    }

    /// The backdrop is drawn again, as what was drawn may be gone: Home's window changed size or scale, was shown
    /// again or activated. Said when it covers.
    pub fn again(&mut self, covered: bool, why: &str) {
        if covered {
            eprintln!("launch: backdrop to be drawn again: {why} at {:.0} ms", since_start());
        }
        self.drawn = false;
    }

    /// A launch began: its backdrop is said once it is up.
    pub fn launch_started(&mut self) {
        self.said = false;
    }

    /// The game has ended, and Home's window has been told: `fading`, its backdrop fades to Home, which is said once
    /// it has; else Home is back now.
    pub fn game_ended(&mut self, fading: bool) {
        self.fading = fading;
        if fading {
            eprintln!("launch: fading to Home at {:.0} ms", since_start());
        } else {
            eprintln!("launch: Home back at {:.0} ms", since_start());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backdrop_is_drawn_once_it_covers_and_then_left() {
        let mut frames = Frames::default();
        assert!(!frames.due(false), "Home draws as ever");
        assert!(frames.due(true), "a frame for the backdrop");
        assert!(!frames.skip(true));
        frames.presented(true, true, false, true, HD);
        assert!(!frames.due(true) && frames.skip(true), "drawn: left alone behind the game");
        frames.again(true, "resized");
        assert!(frames.due(true), "drawn again at its new size");
    }

    const HD: (u32, u32) = (1920, 1080);

    #[test]
    fn a_backdrop_drawn_before_its_art_is_drawn_again_when_art_comes() {
        let mut frames = Frames::default();
        frames.presented(true, true, false, false, HD);
        assert!(!frames.due(true), "no frame until something comes");
        frames.art_came();
        assert!(frames.due(true));
        frames.presented(true, true, false, true, HD);
        frames.art_came();
        assert!(!frames.due(true), "art that is not its own draws nothing");
    }

    #[test]
    fn uncovered_and_covered_again_it_is_drawn_again() {
        let mut frames = Frames::default();
        frames.presented(true, true, false, true, HD);
        assert!(!frames.due(false));
        assert!(frames.due(true));
    }

    #[test]
    fn a_fade_to_home_ends_when_the_backdrop_has_gone() {
        let mut frames = Frames::default();
        frames.presented(true, true, false, true, HD);
        frames.game_ended(true);
        frames.presented(false, true, true, true, HD);
        assert!(frames.fading, "mid-fade");
        frames.presented(false, false, false, true, HD);
        assert!(!frames.fading, "Home back");
    }
}
