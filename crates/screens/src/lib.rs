//! The Dashboard's core: every screen and what is open, with no window and nothing of Windows in it. The executable
//! feeds it the pad's presses and what the shell saw happen (a game started, it ended), draws it each frame, and
//! carries out the effects it asks for. Tests do the same with no window at all.
pub mod guide;
pub mod home;
pub mod launch;
pub mod theme;

use model::Library;
use ui::{Button, Press, Ui};

use crate::guide::{Guide, GuideAction};
use crate::home::{Home, HomeAction};
use crate::launch::LaunchScreen;

/// What the Dashboard asks of the shell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    /// Start the game at this place in the library. The shell answers with `launch_started` or `game_ended`.
    Launch(usize),
    /// Ask the running game to close.
    QuitGame,
    /// The Guide closed: over a game, the game gets the foreground back.
    GuideClosed,
    /// Leave to the desktop.
    Exit,
}

pub struct Dashboard {
    library: Library,
    user: String,
    /// What the clock reads, in place of the time: a screenshot test's is the same every run.
    clock: Option<String>,
    home: Home,
    guide: Guide,
    launch: LaunchScreen,
    /// The game being launched or running, by its place in the library.
    playing: Option<usize>,
    effects: Vec<Effect>,
}

impl Dashboard {
    pub fn new(library: Library) -> Self {
        let user = library.user.clone().unwrap_or_else(|| "Player".into());
        Self {
            library,
            user,
            clock: None,
            home: Home::default(),
            guide: Guide::default(),
            launch: LaunchScreen::default(),
            playing: None,
            effects: Vec::new(),
        }
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn set_clock(&mut self, text: Option<String>) {
        self.clock = text;
    }

    /// What the shell is asked to do since the last call.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub fn press(&mut self, press: Press) {
        if self.guide.is_open() {
            if let Some(action) = self.guide.press(press.button) {
                self.guide_action(action);
            }
            return;
        }
        // While a game starts, the Dashboard takes no input.
        if self.launch.up() {
            return;
        }
        if press.button == Button::Guide {
            if !press.repeat {
                self.open_guide();
            }
            return;
        }
        // Behind a running game there is nothing to steer.
        if self.playing.is_some() {
            return;
        }
        match self.home.press(press.button, &self.library) {
            Some(HomeAction::Launch(game)) if !press.repeat => self.effects.push(Effect::Launch(game)),
            _ => {}
        }
    }

    fn guide_action(&mut self, action: GuideAction) {
        match action {
            GuideAction::Close => self.close_guide(),
            GuideAction::QuitGame => {
                self.effects.push(Effect::QuitGame);
                self.close_guide();
            }
            GuideAction::Launch(game) => {
                self.home.focus_recent(game, &self.library);
                self.close_guide();
                self.effects.push(Effect::Launch(game));
            }
            GuideAction::Exit => self.effects.push(Effect::Exit),
        }
    }

    pub fn open_guide(&mut self) {
        self.guide.open(&self.library, self.playing);
    }

    pub fn close_guide(&mut self) {
        if self.guide.is_open() {
            self.guide.close();
            self.effects.push(Effect::GuideClosed);
        }
    }

    pub fn guide_open(&self) -> bool {
        self.guide.is_open()
    }

    /// How wide the Guide's panel was when last drawn, from the screen's left edge, in design units.
    pub fn guide_panel_width(&self) -> f32 {
        self.guide.panel_width()
    }

    /// The shell has asked the store for the game: its Launch screen comes up.
    pub fn launch_started(&mut self, game: usize) {
        if let Some(found) = self.library.games.get(game) {
            self.launch.show(found);
            self.playing = Some(game);
        }
    }

    /// The game's own window is up: the Launch screen goes.
    pub fn game_booted(&mut self) {
        self.launch.hide();
    }

    /// The game ended, or never started: Home again, focus on its tile.
    pub fn game_ended(&mut self) {
        if let Some(game) = self.playing.take() {
            self.home.focus_recent(game, &self.library);
        }
        self.launch.hide();
        self.guide.close();
    }

    /// The game focus is on, by its place in the library.
    pub fn focused_game(&self) -> Option<usize> {
        self.home.focused_game(&self.library)
    }

    pub fn home_focus(&self) -> home::Focus {
        self.home.focus()
    }

    pub fn playing(&self) -> Option<usize> {
        self.playing
    }

    pub fn draw(&mut self, ui: &mut Ui) {
        let clock = self.clock.clone().unwrap_or_else(|| chrono::Local::now().format("%-I:%M %p").to_string());
        self.home.draw(ui, &self.library, &self.user, &clock);
        self.launch.draw(ui);
        self.guide.draw(ui, &self.library, &self.user);
    }
}

/// Ten games with no art, so Home always has something to show.
pub fn fake_library() -> Library {
    let names = [
        "Halo Infinite",
        "Forza Horizon 5",
        "Sea of Thieves",
        "Starfield",
        "Hades II",
        "Baldur's Gate 3",
        "Hollow Knight: Silksong",
        "Microsoft Flight Simulator",
        "Cyberpunk 2077",
        "Stardew Valley",
    ];
    let games = names.iter().enumerate().map(|(i, name)| model::Game {
        appid: 1000 + i as u32,
        name: name.to_string(),
        last_played: 1_790_000_000 - i as u64 * 86_400,
        playtime_minutes: 600 - i as u32 * 50,
        installed: true,
        install_dir: None,
        cover: None,
        hero: None,
        header: None,
    });
    Library { user: Some("Player One".into()), games: games.collect() }
}
