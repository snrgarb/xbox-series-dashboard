//! Home's background: the swirl, the Recent row's focused game's art over it once focus has rested there, and the
//! scrim over both. It fills the screen and never scrolls. docs/spec/home.md, section 8, has the Bundle's rules,
//! which these are.
//!
//! Everything about when the background wants a frame is here and in the swirl: a delay's end, a fade, the swirl's
//! next step. A Home that is not drawn asks for none.
use std::path::PathBuf;

use model::Game;
use ui::{Art, ArtKey, Color, Crop, Ease, Gradient, Kind, Rect, Ui};

use super::swirl::Swirl;

/// How long focus rests on a Recent tile before the background follows it, so that art changes only for a game
/// the player has stopped on.
pub const DELAY_SECONDS: f64 = 1.5;
/// The same with Settings' Background delay off: only long enough that a held direction passes over tiles without
/// their art being read and flickered through.
pub const SETTLE_SECONDS: f64 = 0.15;
/// How long after focus leaves the Recent row's games the swirl comes back.
pub const LEAVE_SECONDS: f64 = 0.1;
/// The crossfade between one background and the next.
pub const FADE_SECONDS: f32 = 0.4;
/// The same under Reduce motion: still a fade, so art does not flash in, but a short one.
pub const FADE_SECONDS_REDUCED: f32 = 0.15;

/// How much of a blurred cover's light its darkening leaves: it lies under a sheet of `#99000000`.
const BLURRED_LIGHT: f32 = 1.0 - 0x99 as f32 / 255.0;

/// A game's art for the whole screen, and whether it is the blurred stand-in.
type Hero = (ArtKey, bool);

/// A game's art for the whole screen, and whether it is the blurred stand-in: its background art as it is, else
/// its cover, blurred, to be drawn overhanging the screen and darkened.
pub fn hero_art(game: &Game, px: f32) -> Option<Hero> {
    let full = |path: &PathBuf| ArtKey { path: path.clone(), kind: Kind::Full((1920.0 * px).ceil() as u32) };
    match (&game.hero, game.cover.as_ref().or(game.header.as_ref())) {
        (Some(hero), _) => Some((full(hero), false)),
        (None, Some(cover)) => Some((ArtKey { path: cover.clone(), kind: Kind::Blurred }, true)),
        (None, None) => None,
    }
}

pub struct Background {
    /// Settings' Background delay: whether focus has to rest on a tile for the long delay or only the short one.
    pub delay: bool,
    /// Whether Home is what the player is looking at: under the Guide the swirl holds still, as the Bundle's does
    /// whenever its window is not the one in front.
    pub in_front: bool,
    swirl: Swirl,
    /// The Recent row's game focus was on when last drawn.
    focused: Option<model::GameKey>,
    /// Focus has moved since: it may have gone and come back between two frames.
    moved: bool,
    /// What the background is to show: a game's art, or with none the swirl.
    wanted: Option<Hero>,
    /// What it is to show next, and when on the ui's clock.
    next: Option<(f64, Option<Hero>)>,
    /// What the layers last turned to: `wanted`, once its art was decoded.
    shown: Option<Hero>,
    /// Two layers of art over the swirl. The front one fades in, or out for the swirl, as the back one fades out,
    /// each from wherever it is, so a crossfade cut short by another carries on smoothly.
    layers: [Option<Hero>; 2],
    front: usize,
}

impl Default for Background {
    fn default() -> Self {
        Self {
            delay: true,
            in_front: true,
            swirl: Swirl::default(),
            focused: None,
            moved: false,
            wanted: None,
            next: None,
            shown: None,
            layers: [None, None],
            front: 0,
        }
    }
}

impl Background {
    /// Focus moved on Home: whatever the background was waiting to follow, the wait starts again.
    pub fn focus_moved(&mut self) {
        self.moved = true;
    }

    /// The swirl, then the art crossfading over it, then the scrim. `game` is the Recent row's game with focus;
    /// with focus anywhere else there is none.
    pub fn draw(&mut self, ui: &mut Ui, game: Option<&Game>) {
        self.follow(ui, game);

        // The art's layers, the back one first.
        let screen = ui.screen();
        let seconds = if ui.store.reduce_motion { FADE_SECONDS_REDUCED } else { FADE_SECONDS };
        let order = [1 - self.front, self.front];
        let shown = order.map(|layer| {
            let target = if layer == self.front && self.shown.is_some() { 1.0 } else { 0.0 };
            ui.tween(("home background", layer), target, seconds, Ease::Linear)
        });

        // The swirl shows, and moves, until art covers it whole.
        if self.shown.is_none() || shown[1] < 1.0 {
            self.swirl.draw(ui, self.in_front);
        } else {
            self.swirl.keep(ui);
        }
        for (layer, shown) in order.into_iter().zip(shown) {
            let Some((key, blurred)) = &self.layers[layer] else { continue };
            if shown <= 0.0 {
                continue;
            }
            if *blurred {
                // The blur's soft edges hang 120 off every side of the screen.
                ui.art_darkened(screen.grow(120.0), key, Crop::Top, shown, BLURRED_LIGHT);
            } else {
                ui.art(screen, key, Crop::Top, shown, 0.0);
            }
        }

        // The scrim: dark at the top for the top bar, darker at the foot for the rows.
        let stop = |share: f32| screen.h * share;
        ui.gradient(
            Rect::new(screen.x, 0.0, screen.w, stop(0.22)),
            Color::argb(0x99000000),
            Color::CLEAR,
            Gradient::Vertical,
            0.0,
        );
        ui.gradient(
            Rect::new(screen.x, stop(0.4), screen.w, stop(0.6)),
            Color::CLEAR,
            Color::argb(0xD9000000),
            Gradient::Vertical,
            0.0,
        );
    }

    /// Works out what the layers are to show: what focus asks for, once it has waited its time and its art is
    /// decoded.
    fn follow(&mut self, ui: &mut Ui, game: Option<&Game>) {
        let now = ui.store.time();
        let same = match (game, &self.focused) {
            (Some(game), Some(focused)) => game.is(focused),
            (None, None) => true,
            _ => false,
        };
        if std::mem::take(&mut self.moved) || !same {
            self.focused = game.map(Game::key);
            // Every move starts the wait again. A game with no art at all asks for nothing: what is up stays.
            self.next = match game {
                Some(game) => hero_art(game, ui.px()).map(|art| {
                    // Coming onto the row over the swirl, the art shows at once.
                    let wait = match (&self.wanted, self.delay) {
                        (None, _) => 0.0,
                        (Some(_), true) => DELAY_SECONDS,
                        (Some(_), false) => SETTLE_SECONDS,
                    };
                    (now + wait, Some(art))
                }),
                None => self.wanted.is_some().then_some((now + LEAVE_SECONDS, None)),
            };
        }
        if let Some((due, _)) = &self.next {
            // A picture of the screen at rest is of what it comes to.
            if now >= *due || ui.store.settling() {
                self.wanted = self.next.take().and_then(|(_, art)| art);
            } else {
                ui.store.wake_in((*due - now) as f32);
            }
        }

        // The art is read only now, once it is wanted, and what is up stays up until it is in. Art that focus has
        // left by then never shows; art that can't be read leaves what is up.
        if self.wanted != self.shown {
            let ready = self.wanted.as_ref().is_none_or(|(key, _)| matches!(ui.art.get(key), Art::Ready { .. }));
            if ready {
                self.front = 1 - self.front;
                if self.wanted.is_some() {
                    self.layers[self.front] = self.wanted.clone();
                }
                self.shown = self.wanted.clone();
            }
        }
    }
}
