//! The Launch screen: the launching game's art over everything, its name and three dots that pulse in turn.
use model::Game;
use ui::{Color, Crop, Ease, Gradient, HAlign, Rect, TextStyle, Ui, VAlign};

use crate::home::hero_art;

const FADE_SECONDS: f32 = 0.35;

#[derive(Default)]
pub struct LaunchScreen {
    /// The game being launched, while its Launch screen is up or fading.
    game: Option<Game>,
    up: bool,
}

impl LaunchScreen {
    pub fn show(&mut self, game: &Game) {
        self.game = Some(game.clone());
        self.up = true;
    }

    pub fn hide(&mut self) {
        self.up = false;
    }

    pub fn up(&self) -> bool {
        self.up
    }

    pub fn draw(&mut self, ui: &mut Ui) {
        let shown = ui.tween_from("launch screen", 0.0, if self.up { 1.0 } else { 0.0 }, FADE_SECONDS, Ease::CubicOut);
        if shown <= 0.0 {
            if !self.up {
                self.game = None;
            }
            return;
        }
        let Some(game) = &self.game else { return };
        let screen = ui.screen();
        ui.layer();
        ui.with_alpha(shown, |ui| {
            ui.gradient(screen, Color::rgb(0x12381C), Color::rgb(0x050F08), Gradient::Vertical, 0.0);
            if let Some((key, blurred)) = hero_art(game, ui.px()) {
                let rect = if blurred { screen.grow(120.0) } else { screen };
                ui.art(rect, &key, Crop::Top, 1.0, 0.0);
                if blurred {
                    ui.fill(screen, Color::argb(0x99000000), 0.0);
                }
            }
            ui.gradient(screen, Color::argb(0x4D000000), Color::argb(0xB3000000), Gradient::Vertical, 0.0);
            let title = TextStyle::new(ui::Font::Ui, 56.0, 600);
            let lines = Rect::new(210.0, 0.0, 1500.0, screen.h * 0.7);
            ui.paragraph(lines, &game.name, &title, HAlign::Center, VAlign::Bottom, 2);
            let time = ui.time() as f32;
            let dot = 14.0;
            for i in 0..3 {
                let phase = (time * 1.6 - i as f32 * 0.22).rem_euclid(1.0);
                let glow = 0.25 + 0.75 * (1.0 - (phase * 2.0 - 1.0).abs()).powi(2);
                let x = 960.0 + (i as f32 - 1.0) * dot * 2.4 - dot / 2.0;
                ui.fill(Rect::new(x, screen.h * 0.7 + 36.0, dot, dot), Color::WHITE.fade(glow), dot / 2.0);
            }
        });
    }
}
