//! Home's background: the base, the focused Recent game's art over it, and the scrim over both. It fills the
//! screen and never scrolls.
use std::path::PathBuf;

use model::Game;
use ui::{ArtKey, Color, Crop, Ease, Gradient, Kind, Rect, Ui};

const HERO_FADE_SECONDS: f32 = 0.4;
const HERO_FADE_SECONDS_REDUCED: f32 = 0.15;

/// A game's art for the whole screen, and whether it is the blurred stand-in: its background art as it is, else
/// its cover, blurred, to be drawn overhanging the screen and darkened.
pub fn hero_art(game: &Game, px: f32) -> Option<(ArtKey, bool)> {
    let full = |path: &PathBuf| ArtKey { path: path.clone(), kind: Kind::Full((1920.0 * px).ceil() as u32) };
    match (&game.hero, game.cover.as_ref().or(game.header.as_ref())) {
        (Some(hero), _) => Some((full(hero), false)),
        (None, Some(cover)) => Some((ArtKey { path: cover.clone(), kind: Kind::Blurred }, true)),
        (None, None) => None,
    }
}

#[derive(Default)]
pub struct Background {
    /// The background art showing, and the one it is taking over from.
    hero: Option<(ArtKey, bool)>,
    hero_before: Option<(ArtKey, bool)>,
}

impl Background {
    /// The base, then `game`'s art crossfading over the last one's, then the scrim. `game` is the Recent row's
    /// game with focus; with focus anywhere else there is none, and the base shows.
    pub fn draw(&mut self, ui: &mut Ui, game: Option<&Game>) {
        let screen = ui.screen();
        ui.gradient(screen, Color::rgb(0x1E2C1C), Color::rgb(0x0C140B), Gradient::Radial, 0.0);

        // A game with no art leaves the last game's up; a game's art takes over once it is decoded.
        let wanted = game.and_then(|g| hero_art(g, ui.px()).or(self.hero.clone()));
        if wanted != self.hero {
            let ready = wanted.as_ref().is_none_or(|(key, _)| matches!(ui.art.get(key), ui::Art::Ready { .. }));
            if ready {
                self.hero_before = self.hero.take();
                self.hero = wanted;
            }
        }
        let seconds = if ui.store.reduce_motion { HERO_FADE_SECONDS_REDUCED } else { HERO_FADE_SECONDS };
        let layer = |ui: &mut Ui, art: &Option<(ArtKey, bool)>, to: f32| {
            let Some((key, blurred)) = art else { return 0.0 };
            let shown = ui.tween_from(("hero", key), 0.0, to, seconds, Ease::Linear);
            if *blurred {
                // The blur's soft edges hang 120 off every side of the screen.
                ui.art(screen.grow(120.0), key, Crop::Top, shown, 0.0);
                ui.fill(screen, Color::argb(0x99000000).fade(shown), 0.0);
            } else {
                ui.art(screen, key, Crop::Top, shown, 0.0);
            }
            shown
        };
        if self.hero_before != self.hero && layer(ui, &self.hero_before.clone(), 0.0) <= 0.0 {
            self.hero_before = None;
        }
        layer(ui, &self.hero.clone(), 1.0);

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
}
