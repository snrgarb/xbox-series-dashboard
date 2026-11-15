//! The Dashboard's core: every screen and what is open, with no window and nothing of Windows in it. The executable
//! feeds it the pad's presses and what the shell saw happen (a game started, it ended), draws it each frame, and
//! carries out the effects it asks for. Tests do the same with no window at all.
//!
//! It declares two pictures: Home's window's (`draw`) and the Overlay's (`draw_overlay`), the window above every
//! other that shows the Launch screen and, over a running game, the Guide. `overlay` says what the Overlay is wanted
//! for, which the executable follows by showing and hiding it.
pub mod guide;
pub mod home;
pub mod launch;
pub mod spinner;
pub mod theme;

use model::Library;
pub use model::Sound;
use ui::{Battery, Button, PadEvent, PadStatus, Press, Repeat, Ui};

use crate::guide::{Around, Face, Guide, GuideAction, Status};
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
    /// Play this sound, if there is a file for it.
    Sound(Sound),
    /// The Guide button was held: Power (docs/spec/guide.md, 7.1).
    PowerHold,
}

/// A held direction's pace in the Guide (docs/spec/guide.md, 3.1). Anywhere else it is `Repeat::default()`.
pub const GUIDE_REPEAT: Repeat = Repeat::millis(700, 80);

/// What the Overlay is wanted for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayShows {
    /// Nothing: it is hidden, and nothing is drawn on it.
    Nothing,
    /// Something to look at that takes no input, so whatever is in front keeps the foreground: the Launch screen,
    /// or the Guide sliding away.
    Passive,
    /// The Guide over a running game: it takes the foreground from the game.
    Guide,
}

pub struct Dashboard {
    library: Library,
    user: String,
    /// What the clock reads, in place of the time: a screenshot test's is the same every run.
    clock: Option<String>,
    /// Whether a pad is connected and what powers it, which the Status area shows.
    pad: PadStatus,
    /// What the Guide's Status area shows beyond those.
    status: Status,
    /// The Guide setting: off, the Guide button does nothing.
    guide_enabled: bool,
    /// The Guide button's press has done what it does, so what is still to come of it (its release after a hold,
    /// its tap after the release that closed the Guide) does nothing more.
    guide_button_spent: bool,
    home: Home,
    guide: Guide,
    /// Whether the Guide, as it was last opened, is over a game: drawn on the Overlay, not in Home's window.
    guide_overlaid: bool,
    launch: LaunchScreen,
    /// Whether there is an Overlay to draw on.
    has_overlay: bool,
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
            pad: PadStatus::default(),
            status: Status::default(),
            guide_enabled: true,
            guide_button_spent: false,
            home: Home::default(),
            guide: Guide::default(),
            guide_overlaid: false,
            launch: LaunchScreen::default(),
            has_overlay: true,
            playing: None,
            effects: Vec::new(),
        }
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    /// A newer snapshot of the library takes the old one's place. Focus stays on the game it was on and the game
    /// being played stays the one being played, wherever the snapshot has them now.
    pub fn set_library(&mut self, library: Library) {
        let place = |at: usize| {
            let game = self.library.games.get(at)?;
            library.games.iter().position(|g| (g.id, g.appid) == (game.id, game.appid))
        };
        // Only a Recent tile follows its game: a Group's card keeps its place in the Group, whatever is there now.
        let on_row = matches!(self.home.focus(), home::Focus::Recent(_));
        let was = self.home.focused_game(&self.library).filter(|_| on_row);
        let focused = was.and_then(place);
        self.playing = self.playing.and_then(place);
        self.user = library.user.clone().unwrap_or_else(|| "Player".into());
        self.library = library;
        // A focused game that has left the Recent row leaves focus on the tile where it was, or the row's last.
        let last = self.library.recent(home::RECENT_GAMES).len().saturating_sub(1);
        if let Some(was) = was {
            let game = focused.filter(|&game| game <= last).unwrap_or(was.min(last));
            self.home.focus_recent(game, &self.library);
        }
        // The Guide's lists name games by their place: made again, focus staying where it is in them.
        self.refresh_guide();
    }

    fn refresh_guide(&mut self) {
        if self.guide.is_open() {
            let around = Around { library: &self.library, playing: self.playing, discord: self.status.discord };
            self.guide.refresh(around);
        }
    }

    /// The batteries, the notifications' count and whether Discord is connected, as the shell last read them.
    pub fn set_status(&mut self, status: Status) {
        self.status = status;
        self.refresh_guide();
    }

    pub fn set_clock(&mut self, text: Option<String>) {
        self.clock = text;
    }

    /// A pad was connected, or the last one went.
    pub fn set_pad_connected(&mut self, connected: bool) {
        self.pad.connected = connected;
        if !connected {
            self.pad.battery = None;
        }
    }

    /// The pad as `input` reports it. True when that is news.
    pub fn set_pad(&mut self, pad: PadStatus) -> bool {
        let news = self.pad != pad;
        self.pad = pad;
        news
    }

    pub fn pad(&self) -> PadStatus {
        self.pad
    }

    /// The Guide setting. Off, the Guide button does nothing, a hold too, and nor does the profile on Home's top
    /// bar. A Guide that is open stays open until it is closed.
    pub fn set_guide_enabled(&mut self, enabled: bool) {
        self.guide_enabled = enabled;
    }

    /// How a held direction repeats now, for whoever times the pad.
    pub fn repeat(&self) -> Repeat {
        if self.guide.is_open() {
            GUIDE_REPEAT
        } else {
            Repeat::default()
        }
    }

    /// What a pad says happened. A button going down is a `press`. The Guide button is a tap or a hold
    /// (docs/spec/guide.md, 7.1): a tap closes an open Guide as the button is let go, and opens a closed one when
    /// the tap is told, 300 ms after the button went down; a hold asks for Power and opens nothing.
    pub fn pad_event(&mut self, event: PadEvent) {
        let guide = Press { button: Button::Guide, repeat: false };
        match event {
            PadEvent::Press(press) => self.press(press),
            PadEvent::Release(Button::Guide) => {
                // Let go after a hold: Power has been asked for, and that is all the press does.
                if !std::mem::take(&mut self.guide_button_spent) && self.guide.is_open() {
                    self.press(guide);
                    self.guide_button_spent = true;
                }
            }
            PadEvent::Release(_) => {}
            PadEvent::GuideTap => {
                if !std::mem::take(&mut self.guide_button_spent) && !self.guide.is_open() {
                    self.press(guide);
                }
            }
            PadEvent::GuideHeld => {
                self.guide_button_spent = true;
                if self.guide_enabled {
                    self.effects.push(Effect::PowerHold);
                }
            }
        }
    }

    /// Settings' Background delay. See [`Home::set_background_delay`].
    pub fn set_background_delay(&mut self, on: bool) {
        self.home.set_background_delay(on);
    }

    /// What the shell is asked to do since the last call.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// A press of the pad, and the sound of what it did. A press of the Guide button is a whole tap of it, at once:
    /// the keyboard's, and a test's. A pad's is told by `pad_event`. The sound is asked for before anything else the press
    /// asks of the shell, and is decided here for every screen, by what the press changed: a move that went
    /// nowhere, B, and the Guide closing are silent, as they are in the Bundle.
    pub fn press(&mut self, press: Press) {
        let (was_open, guide, home) = (self.guide.is_open(), self.guide.focus(), self.home.focus());
        let asked = self.effects.len();
        let activated = self.steer(press);
        let sound = if was_open {
            if activated {
                Some(Sound::GuideSelect)
            } else {
                (self.guide.is_open() && self.guide.focus() != guide).then_some(Sound::GuideMove)
            }
        } else if self.guide.is_open() {
            Some(Sound::GuideOpen)
        } else if activated {
            Some(Sound::Activate)
        } else {
            (self.home.focus() != home).then_some(Sound::Move)
        };
        if let Some(sound) = sound {
            self.effects.insert(asked, Effect::Sound(sound));
        }
    }

    /// Gives the press to whatever takes input now. True when it was A and it chose something.
    fn steer(&mut self, press: Press) -> bool {
        if self.guide.is_open() {
            let around = Around { library: &self.library, playing: self.playing, discord: self.status.discord };
            let action = self.guide.press(press, around);
            if let Some(action) = action {
                self.guide_action(action);
            }
            // Everything but closing with B or the Guide button is a choice, and sounds as one.
            return action.is_some_and(|action| action != GuideAction::Close);
        }
        // While a game starts, the Dashboard takes no input.
        if self.launch.up() {
            return false;
        }
        if press.button == Button::Guide {
            if !press.repeat && self.guide_enabled {
                self.open_guide();
            }
            return false;
        }
        // Behind a running game there is nothing to steer.
        if self.playing.is_some() {
            return false;
        }
        match self.home.press(press.button, &self.library) {
            Some(HomeAction::Launch(game)) if !press.repeat => self.effects.push(Effect::Launch(game)),
            Some(HomeAction::OpenGuide) if !press.repeat && self.guide_enabled => self.open_guide(),
            _ => return false,
        }
        press.button == Button::A
    }

    fn guide_action(&mut self, action: GuideAction) {
        match action {
            GuideAction::Close | GuideAction::Home | GuideAction::Resume => self.close_guide(),
            // The Guide stays open over a game being quit, and goes when the game has ended.
            GuideAction::QuitGame => self.effects.push(Effect::QuitGame),
            GuideAction::Launch(game) => {
                self.home.focus_recent(game, &self.library);
                self.close_guide();
                self.effects.push(Effect::Launch(game));
            }
            GuideAction::Exit => self.effects.push(Effect::Exit),
        }
    }

    /// Opens the Guide, whatever the Guide setting says: that is for the Guide button.
    pub fn open_guide(&mut self) {
        // Over a game the Guide is on the Overlay; with no Overlay there is nowhere to show it.
        if self.playing.is_some() && !self.has_overlay {
            return;
        }
        self.guide.open(Around { library: &self.library, playing: self.playing, discord: self.status.discord });
        self.guide_overlaid = self.playing.is_some();
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

    /// The Guide, for what it shows and where its focus is.
    pub fn guide(&self) -> &Guide {
        &self.guide
    }

    /// Whether the Guide is open over a game.
    pub fn guide_over_game(&self) -> bool {
        self.guide_overlaid && self.guide.is_open()
    }

    /// Says whether there is an Overlay to draw on, as there is unless told otherwise. Without one no Launch screen
    /// shows and the Guide does not open over a game: neither can be drawn in Home's window, which is behind the
    /// game.
    pub fn set_overlay(&mut self, there: bool) {
        self.has_overlay = there;
        if !there {
            // Whatever it was showing is gone with it.
            self.launch = LaunchScreen::default();
            if self.guide_overlaid {
                self.guide = Guide::default();
                self.guide_overlaid = false;
            }
        }
    }

    /// What the Overlay is wanted for now.
    pub fn overlay(&self) -> OverlayShows {
        let guide = self.guide_overlaid && self.guide.showing();
        if guide && self.guide.is_open() {
            OverlayShows::Guide
        } else if guide || self.launch.showing() {
            OverlayShows::Passive
        } else {
            OverlayShows::Nothing
        }
    }

    /// Whether the Overlay's picture is the Launch screen all the way in and nothing else: only its Spinner moves.
    pub fn overlay_at_rest(&self) -> bool {
        self.launch.covers() && !(self.guide_overlaid && self.guide.showing())
    }

    /// Whether the Launch screen is up, or still fading away.
    pub fn launch_screen_showing(&self) -> bool {
        self.launch.showing()
    }

    /// Whether Home's window has nothing to show: a game is running in front of it, or its Launch screen has faded
    /// in over it.
    pub fn home_covered(&self) -> bool {
        self.playing.is_some() && (!self.launch.up() || self.launch.covers())
    }

    /// The shell has asked the store for the game: its Launch screen comes up.
    pub fn launch_started(&mut self, game: usize) {
        if let Some(found) = self.library.games.get(game) {
            if self.has_overlay {
                self.launch.show(found);
            }
            self.playing = Some(game);
            self.refresh_guide();
        }
    }

    /// The Launch screen goes: the game's own window is up, or something else wants the screen.
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

    /// What the clock reads.
    fn clock(&self) -> String {
        self.clock.clone().unwrap_or_else(|| chrono::Local::now().format("%-I:%M %p").to_string())
    }

    /// The Guide alone, over whatever is drawn already: the same in Home's window and on the Overlay.
    pub fn draw_guide(&mut self, ui: &mut Ui) {
        let clock = self.clock();
        let around = Around { library: &self.library, playing: self.playing, discord: self.status.discord };
        // The pad's own word on its charge, unless the shell has said otherwise. One on a cable shows its glyph.
        let charge = match self.pad.battery {
            Some(Battery::Charge(charge)) => Some((charge.clamp(0.0, 1.0) * 100.0).round() as u8),
            _ => None,
        };
        let status = Status { pad_battery: self.status.pad_battery.or(charge), ..self.status.clone() };
        let face = Face { user: &self.user, clock: &clock, pad: self.pad.connected, status: &status };
        self.guide.draw(ui, around, &face);
    }

    /// Home's window: the page, and the Guide over it.
    pub fn draw(&mut self, ui: &mut Ui) {
        let clock = self.clock();
        let status = home::Status { user: &self.user, clock: &clock, pad: self.pad.connected };
        self.home.set_in_front(!self.guide.is_open());
        self.home.draw(ui, &self.library, &status);
        if !self.guide_overlaid {
            self.draw_guide(ui);
        }
    }

    /// The Overlay: the Launch screen, and the Guide over a game. Drawn over nothing, so where it draws nothing
    /// the game shows.
    pub fn draw_overlay(&mut self, ui: &mut Ui) {
        self.launch.draw(ui);
        if self.guide_overlaid {
            self.draw_guide(ui);
        }
    }

    /// Both in one frame, the Overlay's picture over Home's window's: what an offscreen frame shows.
    pub fn draw_all(&mut self, ui: &mut Ui) {
        self.draw(ui);
        self.draw_overlay(ui);
    }
}

/// Fourteen games with no art, so Home always has something to show: ten played, most recently first, then four
/// never played; some not installed. In no database, so with no saved filters: whoever shows it gives it them.
pub fn fake_library() -> Library {
    // Name, minutes played, installed.
    let games = [
        ("Halo Infinite", 600, true),
        ("Forza Horizon 5", 1250, true),
        ("Sea of Thieves", 90, true),
        ("Starfield", 2400, true),
        ("Hades II", 310, true),
        ("Baldur's Gate 3", 45, false),
        ("Hollow Knight: Silksong", 780, true),
        ("Microsoft Flight Simulator", 1500, true),
        ("Cyberpunk 2077", 20, false),
        ("Stardew Valley", 3600, true),
        ("Age of Empires IV", 0, true),
        ("Celeste", 0, false),
        ("Ori and the Will of the Wisps", 0, true),
        ("Psychonauts 2", 0, false),
    ];
    let games = games.iter().enumerate().map(|(i, &(name, playtime_minutes, installed))| model::Game {
        id: 0,
        appid: 1000 + i as u32,
        name: name.to_string(),
        last_played: if playtime_minutes > 0 { 1_790_000_000 - i as u64 * 86_400 } else { 0 },
        playtime_minutes,
        installed,
        install_dir: None,
        cover: None,
        hero: None,
        header: None,
        ..model::Game::default()
    });
    Library { user: Some("Player One".into()), games: games.collect(), ..Library::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::Focus;

    fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
        for &button in buttons {
            dashboard.press(Press { button, repeat: false });
        }
    }

    #[test]
    fn a_new_snapshot_keeps_focus_and_the_game_playing_on_the_same_games() {
        let mut dashboard = Dashboard::new(fake_library());
        press(&mut dashboard, &[Button::Down, Button::Right, Button::Right]);
        dashboard.launch_started(4);
        let (focused, playing) = (dashboard.library().games[2].appid, dashboard.library().games[4].appid);

        // The focused game has been played since: it is the Recent row's first now.
        let mut next = fake_library();
        let mut game = next.games.remove(2);
        game.last_played = next.games[0].last_played + 60;
        next.games.insert(0, game);
        next.user = Some("Someone Else".into());
        dashboard.set_library(next);

        assert_eq!(dashboard.home_focus(), Focus::Recent(0));
        assert_eq!(dashboard.focused_game().map(|g| dashboard.library().games[g].appid), Some(focused));
        assert_eq!(dashboard.playing().map(|g| dashboard.library().games[g].appid), Some(playing));
        assert_eq!(dashboard.user, "Someone Else");
    }

    #[test]
    fn a_new_snapshot_without_the_focused_game_leaves_focus_in_its_place() {
        let mut dashboard = Dashboard::new(fake_library());
        press(&mut dashboard, &[Button::Down, Button::Right, Button::Right, Button::Right]);
        let mut next = fake_library();
        next.games.remove(3);
        dashboard.set_library(next);
        assert_eq!(dashboard.home_focus(), Focus::Recent(3));

        // A Recent row shorter than where focus was: its last tile.
        let mut next = fake_library();
        next.games.truncate(2);
        dashboard.set_library(next);
        assert_eq!(dashboard.home_focus(), Focus::Recent(1));
    }

    #[test]
    fn a_new_snapshot_leaves_focus_on_a_wide_tile() {
        let mut dashboard = Dashboard::new(fake_library());
        press(&mut dashboard, &[Button::Down, Button::Down]);
        let focus = dashboard.home_focus();
        assert!(matches!(focus, Focus::Wide(_)));
        let mut next = fake_library();
        next.games.reverse();
        dashboard.set_library(next);
        assert_eq!(dashboard.home_focus(), focus);
    }
}
