//! The Launch screen: the launching game's art over the whole screen, as the Bundle's, with the game's name and the
//! Spinner. It is drawn on the Overlay, above every window, so what Steam and the game put up while the game starts
//! stays behind it; it fades in as a launch starts and away once the game has booted. The same picture without the
//! Spinner is Home's window's while the game runs (`Backdrop`), behind the game, and fades to Home when it ends.
use model::Game;
use ui::{Color, Crop, Ease, Gradient, HAlign, Rect, Ui, VAlign};

use crate::home::hero_art;
use crate::spinner::spinner;
use crate::theme;

/// The Bundle's fade, in and out: 300 ms, WPF's CubicEase EaseOut.
const FADE_SECONDS: f32 = 0.3;
/// The Spinner's middle, and how far across it is.
const SPINNER_Y: f32 = 540.0;
const SPINNER: f32 = 64.0;
/// The name's box under the Spinner: the middle three sevenths of the canvas, as the Bundle's status column.
const NAME: Rect = Rect::new(1920.0 * 2.0 / 7.0, SPINNER_Y + SPINNER / 2.0 + 28.0, 1920.0 * 3.0 / 7.0, 160.0);

#[derive(Default)]
pub struct LaunchScreen {
    /// The game being launched, while its Launch screen is up or fading.
    game: Option<Game>,
    up: bool,
    /// How much of it showed when it was last drawn, 0 to 1.
    shown: f32,
}

impl LaunchScreen {
    pub fn show(&mut self, game: &Game) {
        self.game = Some(game.clone());
        self.up = true;
        self.shown = 0.0;
    }

    pub fn hide(&mut self) {
        self.up = false;
    }

    /// Up, as far as input goes: not yet told to go.
    pub fn up(&self) -> bool {
        self.up
    }

    /// Whether there is anything of it to draw: up, or still fading away.
    pub fn showing(&self) -> bool {
        self.game.is_some()
    }

    /// Whether it has faded all the way in: nothing under it shows.
    pub fn covers(&self) -> bool {
        self.up && self.shown >= 1.0
    }

    /// `whole`: with no fade, in and away.
    pub fn draw(&mut self, ui: &mut Ui, whole: bool) {
        if self.game.is_none() {
            return;
        }
        let seconds = if whole { 0.0 } else { FADE_SECONDS };
        let shown = ui.tween_from("launch screen", 0.0, if self.up { 1.0 } else { 0.0 }, seconds, Ease::CubicOut);
        self.shown = shown;
        if shown <= 0.0 && !self.up {
            self.game = None;
        }
        let Some(game) = &self.game else { return };
        ui.layer();
        ui.with_alpha(shown, |ui| picture(ui, game, true));
    }
}

/// The Launch screen's picture over the whole screen: the game's art as Home's hero chooses it, the scrim, the shade
/// and the game's name, with the Spinner when `turning`.
fn picture(ui: &mut Ui, game: &Game, turning: bool) {
    let screen = ui.screen();
    // The art as Home's hero chooses it: background art as it is, else the cover blurred and darkened, else black.
    ui.fill(screen, Color::BLACK, 0.0);
    if let Some((key, blurred)) = hero_art(game, ui.px()) {
        let rect = if blurred { screen.grow(120.0) } else { screen };
        if ui.art(rect, &key, Crop::Top, 1.0, 0.0) && blurred {
            ui.fill(screen, Color::argb(0x99000000), 0.0);
        }
    }
    theme::hero_scrim(ui);
    // A soft shade behind the Spinner and the name, for bright art: the Bundle's is centred a little under the middle,
    // 45% of the screen wide and 60% high each way, with a stop this gradient's two leave out.
    let shade = Rect::new(screen.x + screen.w * 0.05, screen.h * -0.05, screen.w * 0.9, screen.h * 1.2);
    ui.gradient(shade, Color::argb(0xB3000000), Color::CLEAR, Gradient::Radial, 0.0);

    if turning {
        spinner(ui, 960.0, SPINNER_Y, SPINNER);
    }
    ui.paragraph(NAME, &game.name, &theme::HEADING, HAlign::Center, VAlign::Top, 2);
}

/// Home's window while a game is up: the game's Launch screen without its Spinner, opaque over the whole of it, so
/// that round a game that does not fill the screen, and between the game's window going and Home coming back, it is
/// what shows and never the desktop (docs/spec/launch-screen.md). Nothing in it moves, so it is drawn once and left.
/// Once the game has ended it fades to Home underneath, in the Launch screen's own fade: the Bundle's only fade between
/// a game's picture and what is under it.
#[derive(Default)]
pub struct Backdrop {
    /// The game whose picture it is, while it covers Home's window or fades away from it.
    game: Option<Game>,
    /// Fading to Home: the game has ended.
    fading: bool,
}

impl Backdrop {
    /// The game has ended: the backdrop fades to Home. Anything else that uncovers Home (Home chosen in the Guide over
    /// the game, Alt+Tab to the Dashboard) cuts to it.
    pub fn fade(&mut self) {
        self.fading = self.game.is_some();
    }

    /// Whether there is anything of it to draw: it covers Home's window, or is fading away.
    pub fn showing(&self) -> bool {
        self.game.is_some()
    }

    /// Whether it is fading to Home.
    pub fn fading(&self) -> bool {
        self.fading
    }

    /// Draws it over whatever is drawn already. `covering`: the game it covers Home's window for now, which it does at
    /// once and whole; none, and it fades away if told to (`fade`), else goes at once. `whole`: no fade, as on a slow
    /// renderer, where the Launch screen comes and goes whole too.
    pub fn draw(&mut self, ui: &mut Ui, covering: Option<&Game>, whole: bool) {
        match covering {
            Some(game) => {
                if self.game.as_ref().is_none_or(|shown| shown.key() != game.key()) {
                    self.game = Some(game.clone());
                }
                self.fading = false;
            }
            None if !self.fading => self.game = None,
            None => {}
        }
        let Some(game) = &self.game else { return };
        let (target, seconds) = match covering {
            Some(_) => (1.0, 0.0),
            None => (0.0, if whole { 0.0 } else { FADE_SECONDS }),
        };
        let shown = ui.tween_from("backdrop", 1.0, target, seconds, Ease::CubicOut);
        if shown <= 0.0 {
            (self.game, self.fading) = (None, false);
            return;
        }
        ui.layer();
        ui.with_alpha(shown, |ui| picture(ui, game, false));
    }
}
