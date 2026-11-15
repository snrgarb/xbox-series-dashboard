//! The Guide, as the proof of concept has it: a panel from the left over the dimmed screen, with a short list.
//! Over Home it is drawn in Home's window; over a running game on the Overlay, where its dim is over the game.
//! M2 replaces it with the real one.
use model::Library;
use ui::{Button, Color, Crop, Ease, Rect, Ui};

use crate::home::{cover_art, RECENT_GAMES};
use crate::theme::{self, BODY, BODY_SEMIBOLD};

const WIDTH: f32 = 540.0;
const ENTRY: f32 = 105.0;
const TOP: f32 = 168.0;
const SLIDE_SECONDS: f32 = 0.2;

/// What selecting an entry does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GuideAction {
    /// Close the Guide: back to Home, or to the running game.
    Close,
    QuitGame,
    /// Launch the game at this place in the library.
    Launch(usize),
    Exit,
}

struct Entry {
    title: String,
    detail: Option<String>,
    /// The game, by its place in the library, whose cover it shows.
    game: Option<usize>,
    action: GuideAction,
}

#[derive(Default)]
pub struct Guide {
    entries: Vec<Entry>,
    focus: usize,
    open: bool,
    /// How far in the panel was when last drawn, 0 to 1.
    shown: f32,
}

impl Guide {
    /// Opens on the landing list. Over a game (`playing`, its place in the library): the game, to go back to it,
    /// and Quit. Over Home: Home, the Recent row's first games, and the way out to the desktop.
    pub fn open(&mut self, library: &Library, playing: Option<usize>) {
        let entry = |title: &str, detail: Option<&str>, game, action| Entry {
            title: title.into(),
            detail: detail.map(Into::into),
            game,
            action,
        };
        self.entries = match playing {
            Some(playing) => {
                let name = library.games.get(playing).map_or("Game", |g| g.name.as_str());
                vec![
                    entry(name, Some("Running"), Some(playing), GuideAction::Close),
                    entry("Quit", None, None, GuideAction::QuitGame),
                ]
            }
            None => {
                let mut entries = vec![entry("Home", None, None, GuideAction::Close)];
                entries.extend(library.recent(RECENT_GAMES).iter().take(4).enumerate().map(|(i, game)| {
                    let detail = (!game.installed).then_some("Not installed");
                    entry(&game.name, detail, Some(i), GuideAction::Launch(i))
                }));
                entries.push(entry("Exit to desktop", None, None, GuideAction::Exit));
                entries
            }
        };
        self.focus = 0;
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Which entry focus is on, from the top.
    pub fn focus(&self) -> usize {
        self.focus
    }

    /// Whether there is anything of it to draw: open, or still sliding away.
    pub fn showing(&self) -> bool {
        self.open || self.shown > 0.0
    }

    /// The pad while the Guide is open. A's action is returned for the Dashboard to carry out.
    pub fn press(&mut self, button: Button) -> Option<GuideAction> {
        let last = self.entries.len().saturating_sub(1);
        match button {
            Button::Up => self.focus = self.focus.saturating_sub(1),
            Button::Down => self.focus = (self.focus + 1).min(last),
            Button::A => return self.entries.get(self.focus).map(|e| e.action),
            Button::B | Button::Guide => return Some(GuideAction::Close),
            _ => {}
        }
        None
    }

    pub fn draw(&mut self, ui: &mut Ui, library: &Library, user: &str) {
        self.shown = ui.slide_from("guide", 0.0, if self.open { 1.0 } else { 0.0 }, SLIDE_SECONDS, Ease::CubicOut);
        let shown = self.shown;
        if shown <= 0.0 {
            return;
        }
        let screen = ui.screen();
        ui.layer();
        ui.fill(screen, Color::BLACK.fade(0.6 * shown), 0.0);
        let panel = Rect::new(screen.x + (shown - 1.0) * WIDTH, 0.0, WIDTH, screen.h);
        ui.with_offset(panel.x, 0.0, |ui| {
            ui.fill(Rect::new(0.0, 0.0, WIDTH, screen.h), theme::PANEL, 0.0);

            // The profile, as on Home's top bar.
            let picture = Rect::new(42.0, 64.0, 56.0, 56.0);
            crate::home::profile_picture(ui, picture);
            let (_, cy) = picture.center();
            ui.label_middle(picture.right() + 18.0, cy - 2.0, user, &BODY_SEMIBOLD);
            ui.fill(Rect::new(30.0, 144.0, WIDTH - 60.0, 1.0), Color::WHITE.fade(0.2), 0.0);

            for (i, entry) in self.entries.iter().enumerate() {
                let rect = Rect::new(9.0, TOP + i as f32 * (ENTRY + 3.0), WIDTH - 18.0, ENTRY);
                if i == self.focus {
                    ui.fill(rect, Color::WHITE.fade(0.08), 4.0);
                    ui.ring(rect.grow(4.0), Color::WHITE, 10.0, 4.0);
                }
                let mut text_x = rect.x + 33.0;
                if let Some(game) = entry.game.and_then(|g| library.games.get(g)) {
                    let cover = Rect::new(rect.x + 8.0, rect.y + 7.5, 90.0, 90.0);
                    ui.fill(cover, Color::rgb(0x3B3B3B), 4.0);
                    if let Some(key) = cover_art(game, ui.px()) {
                        ui.art(cover, &key, Crop::Top, 1.0, 4.0);
                    }
                    text_x = rect.x + 112.0;
                }
                let line = ui.measure(&BODY, &entry.title).height;
                let lines = if entry.detail.is_some() { 2.0 } else { 1.0 };
                let mut y = rect.y + (rect.h - lines * line) / 2.0;
                ui.with_clip(Rect::new(rect.x, rect.y, rect.w - 16.0, rect.h), |ui| {
                    ui.label(text_x, y, &entry.title, &BODY);
                    if let Some(detail) = &entry.detail {
                        y += line;
                        ui.label(text_x, y, detail, &BODY.color(theme::TEXT_DIM));
                    }
                });
            }
            let hint = BODY.color(theme::TEXT_DIM);
            let line = ui.measure(&hint, "A").height;
            ui.label(42.0, screen.h - 60.0 - line, "A  Select      B  Close", &hint);
        });
    }
}
