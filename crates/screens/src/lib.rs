//! The Dashboard's core: every screen and what is open, with no window and nothing of Windows in it. The executable
//! feeds it the pad's presses and what the shell saw happen (a game started, it ended), draws it each frame, and
//! carries out the effects it asks for. Tests do the same with no window at all.
//!
//! It declares two pictures: Home's window's (`draw`) and the Overlay's (`draw_overlay`), the window above every
//! other that shows the Launch screen and the Guide, over a running game and over Home alike. `overlay` says what
//! the Overlay is wanted for, which the executable follows by showing and hiding it.
//! Toasts are a third (`draw_toasts`), for the strip above both.
mod app_tile;
pub mod game_hub;
pub mod guide;
pub mod hint_bar;
pub mod home;
pub mod keyboard;
pub mod launch;
pub mod manage_queue;
pub mod menu_panel;
pub mod my_games;
pub mod now_playing;
pub mod pages;
pub mod profile;
pub mod search;
pub mod settings;
pub mod spinner;
pub mod startup;
pub mod theme;
pub mod tile_menu;
pub mod toasts;
pub mod wide_dialog;

use std::time::{Duration, Instant};

use model::{Audio, Flow, Library, Media, MediaCommand, Profile};
pub use model::{FilterChange, PowerCommand, ProfileChange, Sound};
use ui::{Battery, Button, PadEvent, PadStatus, Press, Repeat, Ui};

use crate::guide::{Around, DialogFocus, Face, Flyout, FlyoutFocus, Guide, GuideAction, PowerChoice, Status, Tab};
use crate::home::{Focus, Home, HomeAction, TopButton};
use crate::launch::LaunchScreen;
use crate::pages::{Open, PageAction};
pub use crate::settings::{Options, Setting};
pub use crate::startup::{BootVideo, Found};
use crate::startup::{Phase, Startup};
pub use crate::toasts::Notification;
use crate::toasts::{Toast, ToastKind, Toasts};

/// What the Dashboard asks of the shell.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Start the game at this place in the library. The shell answers with `launch_started` or `game_ended`.
    Launch(usize),
    /// Ask the running game to close.
    QuitGame,
    /// The running game was asked to close and has not: end it. The shell answers `quit_given_up` when it finds
    /// nothing to end.
    ForceQuitGame,
    /// The Guide closed: the window that had the foreground gets it back, unless one of the two below says who does.
    GuideClosed,
    /// The Dashboard in front of the running game, which runs on behind it: Home or a page chosen in the Guide.
    ShowHome,
    /// The running game in front again, whatever was in front when the Guide opened.
    ShowGame,
    /// Leave to the desktop: Power's Exit to desktop.
    Exit,
    /// Play this sound, if there is a file for it.
    Sound(Sound),
    /// Turn the machine off, restart it or put it to sleep: chosen in Power (docs/spec/guide.md, 6.2). Whoever
    /// carries effects out decides whether that is for real; nothing in this crate can do it.
    Power(PowerCommand),
    /// Audio's Volume moved: the system's master volume to this, 0 to 100. The Dashboard shows the new level
    /// already, and takes the system's word for it at the next `set_audio`.
    Volume(u8),
    /// Audio's Mute: the system's master mute on or off.
    Mute(bool),
    /// A device chosen in one of Audio's dropdowns, by its place among `audio()`'s of that flow: make it Windows'
    /// default.
    DefaultDevice(Flow, usize),
    /// One of the Now Playing card's controls: ask the app with this id, Windows' own for it (`MediaSession::app_id`),
    /// to do it. What it does shows only as the shell's next reading of its session (`set_media`) says.
    Media(MediaCommand, String),
    /// Settings > Guide's Now Playing source changed to this: the Dashboard follows it already, and it is for
    /// whoever keeps the settings to save (`now_playing::SOURCE_KEY`).
    NowPlayingSource(String),
    /// Settings changed an option. The Dashboard has taken the change already: this is for whoever keeps the
    /// settings to save it.
    Set(settings::Change),
    /// Mark the game with this id in the library database as a favourite, or as one no longer. The shell answers
    /// with a new snapshot of the library (`set_library`).
    Favourite { game: i64, on: bool },
    /// Hide the game with this id from every list but the Hidden filter's, or show it again. A new snapshot
    /// follows, as for a favourite.
    Hidden { game: i64, on: bool },
    /// Ask its store to install the game with this id in the library database: the store's own window does it.
    Install(i64),
    /// Ask its store to uninstall the game with this id. The player has said yes to "Uninstall X?".
    Uninstall(i64),
    /// Settings' Groups page changed a saved filter: shown or not, moved, made, changed, deleted
    /// (docs/spec/filters.md, section 6). A new snapshot follows, as for a favourite, and the pages over Home are
    /// told of it.
    Filter(FilterChange),
    /// Start the App with this id in the library database (`Library::apps`): as it is, whatever is running, with no
    /// Launch screen and no Play session (docs/spec/apps.md).
    StartApp(i64),
    /// Settings' Games & apps pages added, renamed or removed a game added by hand or an App. A new snapshot
    /// follows, as for a favourite, and the pages over Home are told of it.
    ByHand(model::ByHand),
    /// Settings > Art: the user's SteamGridDB key, to keep (none: forget it). The Dashboard takes note that there
    /// is one, never what it is; whoever keeps the settings saves it and looks up art with it.
    ArtKey(Option<Secret>),
    /// Settings > Art: remove the art downloaded into the data folder (the user has said yes). A store's own art is
    /// not touched.
    ClearArt,
    /// Settings' Profiles pages added, renamed or removed a Profile, or changed its picture; or a Profile signed in
    /// (`Dashboard::sign_in`). A new snapshot follows, as for a favourite, and every place a Profile shows follows
    /// it (docs/spec/profiles.md).
    Profile(ProfileChange),
    /// Settings > Personalisation > Boot video chose this one, or Off (docs/spec/start-up.md). The Dashboard has it
    /// already: this is for whoever keeps the settings to save, and it plays from the next start.
    BootVideo(BootVideo),
}

/// A secret the user typed: shown nowhere, in no log and no `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(..)")
    }
}

/// A held direction's pace in the Guide (docs/spec/guide.md, 3.1). Anywhere else it is `Repeat::default()`.
pub const GUIDE_REPEAT: Repeat = Repeat::millis(700, 80);

/// How long the Guide waits for the button that closed it to be let go before it hands the foreground back all the
/// same (docs/spec/guide.md, 3.5). Timed by whoever has a clock, who then says `release_wait_over`.
pub const RELEASE_FALLBACK: std::time::Duration = std::time::Duration::from_secs(1);

/// How long a game asked to quit has to close by itself before "Force quit?" is asked, in seconds; and as long
/// again after a force quit before it is asked anew (docs/spec/guide.md, 7.4).
pub const QUIT_TIMEOUT: f32 = 10.0;

/// How long after a game quit from the Guide has stopped the Guide closes, in seconds.
pub const CLOSE_AFTER_QUIT: f32 = 0.5;

/// The running game being quit (docs/spec/guide.md, 7.4).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Quit {
    /// The seconds it still has to close by itself.
    left: f32,
    /// Whether "Force quit?" is up: nothing is counted meanwhile.
    asked: bool,
    /// The game to start once it has stopped, by its place in the library: "Quit and start".
    then: Option<usize>,
}

/// What the Overlay is wanted for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayShows {
    /// Nothing: it is hidden, and nothing is drawn on it.
    Nothing,
    /// Something to look at that takes no input, so whatever is in front keeps the foreground: the Launch screen,
    /// or the Guide sliding away.
    Passive,
    /// The Guide, over a running game or over Home: it takes the foreground from whatever has it, and keeps it
    /// until the Guide has closed and the button that closed it is let go.
    Guide,
}

pub struct Dashboard {
    library: Library,
    /// What the clock reads, in place of the time: a screenshot test's is the same every run.
    clock: Option<String>,
    /// Whether a pad is connected and what powers it, which the Status area shows.
    pad: PadStatus,
    /// What the Guide's Status area shows beyond those.
    status: Status,
    /// Settings' options (docs/spec/settings.md, section 7). Use the Guide among them: off, the Guide button does
    /// nothing.
    options: Options,
    /// The Guide button's press has done what it does, so what is still to come of it (its release after a hold,
    /// its tap after the release that closed the Guide) does nothing more.
    guide_button_spent: bool,
    home: Home,
    /// The pages over Home, the top one taking the pad (docs/ui.md, "Screens and what is over what").
    pages: pages::Stack,
    /// The page over Home was opened by A on the top bar's Search or Settings button, which gets focus back when
    /// Home shows again.
    back_to_button: bool,
    guide: Guide,
    /// Whether the Guide, as it was last opened, is on the Overlay, as it is wherever there is one: over a game and
    /// over Home alike, so there is one Guide. Without an Overlay it is drawn in Home's window, at Home only.
    guide_overlaid: bool,
    /// The pad's button whose press closed the Guide, until it is let go: the foreground goes back only then, so
    /// the window that gets it never sees the press (docs/spec/guide.md, 3.5).
    release_awaited: Option<Button>,
    launch: LaunchScreen,
    /// Whether there is an Overlay to draw on.
    has_overlay: bool,
    /// The game being launched or running, by its place in the library.
    playing: Option<usize>,
    /// The Toasts showing and waiting, and the notifications list.
    toasts: Toasts,
    /// Whether Toasts have a window of their own: if not they are drawn in Home's.
    toast_strip: bool,
    /// Whether frames come slowly: Home then gives way to a Toast.
    slow_renderer: bool,
    /// Whether the Dashboard is in front of the running game: Home or a page was chosen in the Guide over it. The
    /// game runs on behind, first in the Guide's list, where A on it brings it back.
    in_front: bool,
    /// The quit of the running game that is under way.
    quit: Option<Quit>,
    /// The seconds until the Guide closes by itself: the game quit from it has stopped.
    close_in: Option<f32>,
    /// Whether the time told has changed what the Guide shows, since `take_timed` last asked.
    timed: bool,
    /// The buttons pressed while a dialog page (the Tile menu) was up and not let go since: held on after it has
    /// closed, they do nothing to what was under it (docs/spec/tile-menu.md, section 6).
    held: Vec<Button>,
    /// The system's sound as the shell last read it, which the Audio Flyout shows.
    audio: Audio,
    /// Windows' media sessions as the shell last read them, and Settings' Now Playing source.
    now_playing: now_playing::NowPlaying,
    /// The Now Playing card's time, in whole seconds, as last drawn: a turn of it is something to draw.
    now_playing_second: Option<u64>,
    /// The Dashboard's clock: where the time it is told (`advance`, `toasts_presented`) has brought it. Nothing in
    /// the Dashboard reads the system's.
    now: Instant,
    /// How long from `now` until the double tap's wait is over, if one is waited out (`wait_out`).
    waiting: Option<Duration>,
    /// Start-up, while it shows (docs/spec/start-up.md): full screen over everything, with the pad and the Guide
    /// button to itself.
    startup: Option<Startup>,
    effects: Vec<Effect>,
}

impl Dashboard {
    pub fn new(library: Library) -> Self {
        Self {
            library,
            clock: None,
            pad: PadStatus::default(),
            status: Status::default(),
            options: Options::default(),
            guide_button_spent: false,
            home: Home::default(),
            pages: pages::Stack::default(),
            back_to_button: false,
            guide: Guide::default(),
            guide_overlaid: false,
            release_awaited: None,
            launch: LaunchScreen::default(),
            has_overlay: true,
            playing: None,
            toasts: Toasts::default(),
            toast_strip: false,
            slow_renderer: false,
            in_front: false,
            quit: None,
            close_in: None,
            timed: false,
            held: Vec::new(),
            audio: Audio::default(),
            now_playing: now_playing::NowPlaying::default(),
            now_playing_second: None,
            now: Instant::now(),
            waiting: None,
            startup: None,
            effects: Vec::new(),
        }
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    /// A newer snapshot of the library takes the old one's place. Focus stays on the game it was on and the game
    /// being played stays the one being played, wherever the snapshot has them now.
    pub fn set_library(&mut self, mut library: Library) {
        // A snapshot read from the database has no speeds (the Queue's follower works them out): a download keeps
        // the one it had until the follower next says.
        for entry in library.queue.entries.iter_mut().filter(|entry| entry.speed.is_none() && entry.state.downloading())
        {
            let had = self.library.queue.get(entry.game).filter(|was| was.state.downloading());
            entry.speed = had.and_then(|was| was.speed);
        }
        let place = |at: usize| {
            let game = self.library.games.get(at)?;
            library.games.iter().position(|g| g.is(&game.key()))
        };
        // Only a Recent tile follows its game: a Group's card keeps its place in the Group, whatever is there now.
        let on_row = matches!(self.home.focus(), home::Focus::Recent(_));
        let was = self.home.focused_game(&self.library).filter(|_| on_row);
        let focused = was.and_then(place);
        self.playing = self.playing.and_then(place);
        if let Some(quit) = &mut self.quit {
            quit.then = quit.then.and_then(place);
        }
        self.library = library;
        // A focused game that has left the Recent row leaves focus on the tile where it was, or the row's last.
        let last = self.library.recent(self.home.recent_games()).len().saturating_sub(1);
        if let Some(was) = was {
            let game = focused.filter(|&game| game <= last).unwrap_or(was.min(last));
            self.home.focus_recent(game, &self.library);
        }
        // The Guide's lists name games by their place: made again, focus staying where it is in them.
        self.refresh_guide();
        // A page that keeps something of the library (Settings' Groups, the row focus is on) finds it again.
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
        };
        self.pages.library_changed(around);
    }

    /// The Queue moved on (`library::queue`): it takes the old one's place, the games staying as they are. A page
    /// that shows it (the Game hub's Queue action, Manage Queue) is told, as of any change to the library. Whether
    /// the screen may show the change: a download's bytes and speed are shown only on a page (Manage Queue, My
    /// games & apps' current download), never on Home, which need not be drawn again for them.
    pub fn set_queue(&mut self, queue: model::Queue) -> bool {
        if self.library.queue == queue {
            return false;
        }
        let order = |queue: &model::Queue| -> Vec<(i64, &'static str)> {
            queue.entries.iter().map(|entry| (entry.game, entry.state.status())).collect()
        };
        let shown = !self.pages.is_empty()
            || order(&self.library.queue) != order(&queue)
            || self.library.queue.installed != queue.installed;
        self.library.queue = queue;
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
        };
        self.pages.library_changed(around);
        shown
    }

    /// The names of Home's Groups, in order, as Home shows them now: what `--log-focus` says of them
    /// (`groups: ...`), since a Group's place is all Home's focus says of it.
    pub fn home_groups(&self) -> Vec<String> {
        home::groups::of(&self.library).iter().map(|group| group.name.to_string()).collect()
    }

    fn refresh_guide(&mut self) {
        if self.guide.is_open() {
            let around = Around {
                library: &self.library,
                playing: self.playing,
                discord: self.status.discord,
                audio: &self.audio,
                notifications: self.toasts.notifications(),
                now_playing: self.now_playing.card(self.now),
            };
            self.guide.refresh(around);
        }
    }

    /// The batteries, the notifications' count and whether Discord is connected, as the shell last read them.
    pub fn set_status(&mut self, status: Status) {
        self.status = status;
        self.refresh_guide();
    }

    /// The system's sound, as the shell has just read it. An open Audio Flyout follows.
    pub fn set_audio(&mut self, audio: Audio) {
        self.audio = audio;
        self.refresh_guide();
    }

    /// The system's sound as the Dashboard has it: `Effect::DefaultDevice` names a device by its place here.
    pub fn audio(&self) -> &Audio {
        &self.audio
    }

    /// Whether the Audio Flyout is open: the shell reads the system's sound as it opens and keeps reading while
    /// it is, so that it follows what is changed elsewhere. Its Now Playing card follows the media sessions then.
    pub fn audio_shown(&self) -> bool {
        self.flyout().is_some_and(|(which, _)| which == Flyout::Audio)
    }

    /// Windows' media sessions, as the shell has just read them (docs/spec/now-playing.md): an open Audio Flyout's
    /// Now Playing card follows, and every app among them is a Now Playing source to choose from now on.
    pub fn set_media(&mut self, media: Media) {
        self.now_playing.set_media(media, self.now);
        self.now_playing_second = self.now_playing_second();
        self.refresh_guide();
    }

    /// Settings' Now Playing source as the settings table kept it, at start-up: none is Spotify.
    pub fn set_now_playing_source(&mut self, saved: Option<&str>) {
        let media = self.now_playing.media().clone();
        self.now_playing = now_playing::NowPlaying::new(saved);
        self.now_playing.set_media(media, self.now);
        self.refresh_guide();
    }

    /// Now Playing: the sessions, the source and its choices.
    pub fn now_playing(&self) -> &now_playing::NowPlaying {
        &self.now_playing
    }

    /// What the Audio Flyout's Now Playing card shows, for `--log-focus` (`now playing: ...`): `nothing`, or the
    /// track, the app, whether it plays and its controls. None while the Flyout is not open.
    pub fn now_playing_line(&self) -> Option<String> {
        self.audio_shown().then(|| self.now_playing.line())
    }

    /// The card's time in whole seconds now, while the Audio Flyout shows a card with a length.
    fn now_playing_second(&self) -> Option<u64> {
        let card = self.now_playing.card(self.now).filter(|_| self.audio_shown())?;
        (!card.session.length.is_zero()).then_some(card.position.as_secs())
    }

    /// The Flyout open over the Guide's panel, and where focus is in it.
    pub fn flyout(&self) -> Option<(Flyout, FlyoutFocus)> {
        self.guide.flyout(self.guide_around()).filter(|_| self.guide.is_open())
    }

    /// Notifications, in the open Guide: its Flyout opens, where the showing tab has the Utility (the landing
    /// tab, which the Guide opens on). What the Guide shows when it was opened while a Toast told of a
    /// notification (docs/spec/guide.md, 9.5).
    pub fn open_notifications(&mut self) {
        if self.guide.is_open() {
            self.guide.open_flyout(Flyout::Notifications);
        }
    }

    fn guide_around(&self) -> Around<'_> {
        Around {
            library: &self.library,
            playing: self.playing,
            discord: self.status.discord,
            audio: &self.audio,
            notifications: self.toasts.notifications(),
            now_playing: self.now_playing.card(self.now),
        }
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
        self.options.use_guide = enabled;
    }

    /// Settings' options, as they are now.
    pub fn options(&self) -> Options {
        self.options
    }

    /// Every option at once, as the settings table has them at start-up. In force from the next press and frame.
    pub fn set_options(&mut self, options: Options) {
        self.options = options;
        self.home.set_background_delay(options.background_delay);
        self.home.set_play_history(options.show_play_history, &self.library);
    }

    /// Settings turned an option over: it is in force at once, and whoever keeps the settings is asked to save it.
    fn set(&mut self, change: settings::Change) {
        let mut options = self.options;
        options.set(change.setting, change.on);
        self.set_options(options);
        self.effects.push(Effect::Set(change));
    }

    /// What the pages ask beyond a press's answer, for the shell: a Settings key kept is noted as there being one.
    fn take_asked(&mut self) {
        for effect in self.pages.asked() {
            if let Effect::ArtKey(key) = &effect {
                self.options.art_key = key.is_some();
            }
            self.effects.push(effect);
        }
    }

    fn around(&self) -> pages::Around<'_> {
        pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
        }
    }

    /// How a held direction repeats now, for whoever times the pad: in the Guide and in a dialog at their pace
    /// (docs/spec/tile-menu.md, section 6), anywhere else at Home's.
    pub fn repeat(&self) -> Repeat {
        if self.guide.is_open() || self.pages.dialog().is_some() {
            GUIDE_REPEAT
        } else if self.page_has_pad() {
            self.pages.repeat().unwrap_or_default()
        } else {
            Repeat::default()
        }
    }

    /// Whether a page over Home is what a press goes to now.
    fn page_has_pad(&self) -> bool {
        self.startup.is_none()
            && !self.guide.is_open()
            && !self.launch.up()
            && self.playing.is_none()
            && !self.pages.is_empty()
    }

    /// What a physical keyboard typed that is text's: letters, Space, Backspace. It goes to the page that has the
    /// pad, if that takes text now (text entry; Search). False if nothing took it: Backspace is then B, as it is
    /// everywhere else, and anything else nothing.
    pub fn typed(&mut self, typed: keyboard::Typed) -> bool {
        if !self.page_has_pad() {
            return false;
        }
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
        };
        let taken = self.pages.typed(typed, around);
        self.take_asked();
        taken
    }

    /// What a pad says happened. A button going down is a `press`. The Guide button is a tap or a hold
    /// (docs/spec/guide.md, 7.1): a tap closes an open Guide as the button is let go, and opens a closed one when
    /// the tap is told, 300 ms after the button went down; a hold opens Power, with no panel.
    pub fn pad_event(&mut self, event: PadEvent) {
        if self.startup.is_some() {
            self.startup_pad_event(event);
            return;
        }
        let guide = Press { button: Button::Guide, repeat: false };
        match event {
            PadEvent::Press(press) => {
                let was_open = self.guide.is_open();
                let page = self.page_has_pad().then(|| self.pages.depth());
                self.press(press);
                // The page that took the press is told the button is down, for what it draws while it is.
                if !press.repeat && page == Some(self.pages.depth()) {
                    self.pages.pad(press.button, true);
                }
                // Closed by this press: the Overlay keeps the foreground until the button is up again.
                if was_open && !self.guide.is_open() && self.guide_overlaid {
                    self.release_awaited = Some(press.button);
                }
            }
            PadEvent::Release(Button::Guide) => {
                // Let go after a hold: Power has been asked for, and that is all the press does.
                if !std::mem::take(&mut self.guide_button_spent) && self.guide.is_open() {
                    self.press(guide);
                    self.guide_button_spent = true;
                }
            }
            PadEvent::Release(button) => {
                self.pages.pad(button, false);
                self.held.retain(|&held| held != button);
                if self.release_awaited == Some(button) {
                    self.release_awaited = None;
                }
            }
            PadEvent::GuideTap => {
                if !std::mem::take(&mut self.guide_button_spent) && !self.guide.is_open() {
                    self.press(guide);
                }
            }
            PadEvent::GuideHeld => {
                self.guide_button_spent = true;
                self.open_power();
                self.settle_quit();
            }
        }
    }

    /// Time has passed: `seconds` since the Dashboard was last told the time, by this or by `toasts_presented`.
    /// Toasts are timed by it, since the Dashboard has no clock: one that begins to show asks for its sound. True
    /// when what `draw_toasts` draws may have changed.
    ///
    /// It is the Dashboard's one clock (docs/ui.md, "The pad's timing"), with `toasts_presented`: a press is at the
    /// time the two have brought it to. Besides Toasts, what it times is the double tap of A on a game's tile, whose
    /// Game hub opens once it is over with no second A (docs/spec/game-hub.md, 9.1), and Play history's the same.
    /// The quit of a game is timed by it too (docs/spec/guide.md, 7.4): "Force quit?" once the game has had
    /// `QUIT_TIMEOUT` to close, and the Guide closing `CLOSE_AFTER_QUIT` after a game quit from it has stopped.
    /// Whether anything came of that is `take_timed`'s to say.
    pub fn advance(&mut self, seconds: f32) -> bool {
        let before = (self.toasts.shown(), self.toasts.showing().is_some(), self.toasts.on_screen().is_some());
        if self.toasts.advance(seconds) {
            self.effects.push(Effect::Sound(Sound::Notification));
        }
        self.pass(seconds);
        before != (self.toasts.shown(), self.toasts.showing().is_some(), self.toasts.on_screen().is_some())
    }

    /// The time the double tap and the quit of a game go by.
    fn pass(&mut self, seconds: f32) {
        // To the microsecond: steps a test adds up land where it says, not a hair short.
        self.now += Duration::from_micros((f64::from(seconds) * 1e6).round().max(0.0) as u64);
        if let Some(startup) = &mut self.startup {
            startup.advance(seconds);
        }
        self.timed |= self.wait_out();
        // The Now Playing card's time turned a second: it is drawn again.
        let second = self.now_playing_second();
        if second != self.now_playing_second {
            self.now_playing_second = second;
            self.timed = true;
        }
        let due = self.quit.as_mut().filter(|quit| !quit.asked).is_some_and(|quit| {
            quit.left -= seconds;
            quit.left <= 0.0
        });
        if due {
            self.offer_force_quit();
            self.timed = true;
        }
        if let Some(left) = &mut self.close_in {
            *left -= seconds;
            if *left <= 0.0 {
                self.close_in = None;
                self.timed |= self.guide.is_open();
                self.close_guide();
            }
        }
    }

    /// Whether the time told since this was last asked changed what is shown: "Force quit?" came up, the Guide
    /// closed after a quit, or a Game hub opened at the end of the double tap's wait.
    pub fn take_timed(&mut self) -> bool {
        std::mem::take(&mut self.timed)
    }

    /// How soon `advance` has something to do, in seconds: None when nothing is being timed.
    pub fn timer(&self) -> Option<f32> {
        let quit = self.quit.filter(|quit| !quit.asked).map(|quit| quit.left);
        let tap = self.waiting.map(|left| left.as_secs_f32());
        // The Now Playing card's time, while the Audio Flyout shows a track playing.
        let second = self.audio_shown().then(|| self.now_playing.next_second(self.now)).flatten();
        let second = second.map(|left| left.as_secs_f32());
        [quit, self.close_in, tap, second].into_iter().flatten().reduce(f32::min).map(|left| left.max(0.0))
    }

    /// The double tap's wait, at `now`: the Game hub of the game whose tile A chose on Home once its time has
    /// passed with no second A, or what the top page's own wait brings (Play history's, for the same double tap).
    /// What its time brings while something else has the pad is dropped. Keeps how long is left in `waiting`, and
    /// is true when something opened.
    fn wait_out(&mut self) -> bool {
        let now = self.now;
        let free = !self.guide.is_open() && !self.launch.up() && (self.playing.is_none() || self.in_front);
        self.waiting = None;
        if !self.pages.is_empty() {
            // A page over Home has the pad: a tile's A is forgotten.
            self.home.tap(now, false);
            let around = pages::Around {
                library: &self.library,
                playing: self.playing,
                options: self.options,
                now: self.now,
                now_playing: &self.now_playing,
            };
            return match self.pages.tick(around) {
                pages::Tick::Wait(left) if free => {
                    self.waiting = Some(left);
                    false
                }
                pages::Tick::Act(action) if free => {
                    self.page_asks(action);
                    true
                }
                _ => false,
            };
        }
        match self.home.tap(now, free) {
            home::Tap::None => false,
            home::Tap::Waiting(left) => {
                self.waiting = Some(left);
                false
            }
            // A game that has left the library since has no hub to show.
            home::Tap::Hub(game) if game.in_library(&self.library) => {
                self.open_page(Open::GameHub(game));
                true
            }
            home::Tap::Hub(_) => false,
        }
    }

    /// Whether the running game has been asked to quit and has not stopped yet, nor been given up on: it is not
    /// frozen under the Guide meanwhile, so it is free to close.
    pub fn quitting(&self) -> bool {
        self.quit.is_some()
    }

    /// Whether the Dashboard is in front of the running game.
    pub fn in_front_of_game(&self) -> bool {
        self.playing.is_some() && self.in_front
    }

    /// The shell found nothing of the game to end: asking again would never help, so the quit is over and the game
    /// left to itself.
    pub fn quit_given_up(&mut self) {
        self.quit = None;
    }

    /// Quits the running game: asked to close, as its window's close button would, with no confirmation. Nothing
    /// while it is being quit already.
    fn quit_game(&mut self) {
        if self.playing.is_some() && self.quit.is_none() {
            self.quit = Some(Quit { left: QUIT_TIMEOUT, asked: false, then: None });
            self.effects.push(Effect::QuitGame);
        }
    }

    /// The game has not closed by itself in time: "Force quit {X}?", over the Guide, which opens for it if it is
    /// closed; with the Guide setting off, over the dim alone.
    fn offer_force_quit(&mut self) {
        let Some(name) = self.playing.and_then(|game| self.library.games.get(game)).map(|game| game.name.clone())
        else {
            self.quit = None;
            return;
        };
        if !self.open_guide_for_a_prompt() {
            self.quit = None;
            return;
        }
        self.guide.ask_force_quit(&name);
        if let Some(quit) = &mut self.quit {
            quit.asked = true;
        }
        self.effects.push(Effect::Sound(Sound::PromptOpen));
    }

    /// A prompt needs the Guide: opened if it is closed, with its panel only if the Guide setting is on. False when
    /// there is nowhere to show it: over a game with no Overlay.
    fn open_guide_for_a_prompt(&mut self) -> bool {
        if !self.guide.is_open() {
            self.open_guide();
            if !self.options.use_guide {
                self.guide.without_panel();
            }
        }
        self.guide.is_open()
    }

    /// "Force quit?" has gone unanswered with "Force quit": by Cancel, B, the Guide closing or another dialog taking
    /// its place. Declined, the game is left running and the quit is over, with whatever was to start after it.
    fn settle_quit(&mut self) {
        let prompt = matches!(self.guide.dialog(), Some(DialogFocus::Menu { prompt: true, .. }));
        if self.quit.is_some_and(|quit| quit.asked) && !prompt {
            self.quit = None;
        }
    }

    /// A on a game: started, or with another game running asked about first, "Quit {X} and start {Y}?"
    /// (docs/spec/guide.md, 6.4), over the Guide, which opens for it. On the running game itself, back to it. Two
    /// games are never started at once: while one is quitting or starting, nothing.
    fn play(&mut self, game: usize) {
        if self.playing == Some(game) {
            self.show_game();
            return;
        }
        if self.quit.is_some() || self.launch.up() {
            return;
        }
        let Some(running) = self.playing else {
            self.effects.push(Effect::Launch(game));
            return;
        };
        let name = |game: usize| self.library.games.get(game).map_or("the game", |game| game.name.as_str()).to_string();
        let (running, name) = (name(running), name(game));
        if self.open_guide_for_a_prompt() {
            self.guide.ask_quit_and_start(&running, game, &name);
        }
    }

    /// The running game in front again.
    fn show_game(&mut self) {
        if self.playing.is_some() {
            self.in_front = false;
            self.effects.push(Effect::ShowGame);
        }
    }

    /// The Dashboard in front of the running game, if one is running: Home or a page was chosen in the Guide over
    /// it. The shell says so itself when the game it was to go back to has no window.
    pub fn show_dashboard(&mut self) {
        if self.playing.is_some() {
            self.in_front = true;
            self.effects.push(Effect::ShowHome);
        }
    }

    /// Settings' Background delay. See [`Home::set_background_delay`].
    pub fn set_background_delay(&mut self, on: bool) {
        self.options.background_delay = on;
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
    ///
    /// A dialog over the Guide has sounds of its own: one for its coming up, after A's if A brought it; a move's;
    /// A's on an option, which in a prompt is the sound of its closing; and B's, the closing's. A tap of the Guide
    /// button closes whatever is up in silence.
    ///
    /// A page that is a dialog (the Tile menu) sounds as a dialog does (docs/spec/tile-menu.md, section 8): its
    /// opening, after the sound of what opened it; a move along its rows; A on a row, which in a prompt is the
    /// sound of its closing; B at every level. A move with nowhere to go is silent there too.
    pub fn press(&mut self, press: Press) {
        if self.startup.is_some() {
            self.startup_press(press);
            return;
        }
        let now = self.now;
        let (was_open, guide) = (self.guide.is_open(), self.guide_place());
        // Focus moved if the same screen has the pad as before and says its focus is somewhere else. A page
        // opening or closing is no move: B is silent, and so is Y.
        let (depth, focus) = (self.pages.depth(), self.focus_place());
        let dialog = self.guide.dialog();
        let page_dialog = self.pages.dialog();
        let asked = self.effects.len();
        let activated = self.steer(press, now);
        self.settle_quit();
        let select = activated.then_some(Sound::GuideSelect);
        // A dialog a page shows over itself (Settings' menu of a Group) coming up or going is no move either.
        let same_level = self.pages.depth() == depth && self.pages.dialog().is_some() == page_dialog.is_some();
        let moved = same_level && self.focus_place() != focus;
        // A dialog page that was not up before the press came up by it.
        let opened = (page_dialog.is_none() && self.pages.dialog().is_some()).then_some(Sound::PromptOpen);
        let sounds = match (dialog, self.guide.dialog()) {
            (None, Some(_)) => [select, Some(Sound::PromptOpen)],
            (Some(was), Some(now)) => [(was != now).then_some(Sound::GuideMove), None],
            (Some(was), None) => {
                let chose = match was {
                    DialogFocus::Menu { prompt: true, .. } => Sound::PromptClose,
                    _ => Sound::GuideSelect,
                };
                match press.button {
                    Button::A => [Some(chose), None],
                    Button::B => [Some(Sound::PromptClose), None],
                    _ => [None, None],
                }
            }
            (None, None) if was_open => {
                // B closing a Flyout or a list changes where focus is, and is silent all the same.
                let moved = press.button != Button::B && self.guide.is_open() && self.guide_place() != guide;
                [select.or(moved.then_some(Sound::GuideMove)), None]
            }
            (None, None) if self.guide.is_open() => [Some(Sound::GuideOpen), None],
            (None, None) => match (page_dialog, press.button) {
                (Some(kind), Button::A) if !press.repeat => [Some(kind.chosen()), None],
                (Some(_), Button::B) if !press.repeat => [Some(Sound::PromptClose), None],
                (Some(_), _) => [moved.then_some(Sound::GuideMove), None],
                (None, _) if activated => [Some(Sound::Activate), opened],
                (None, _) => [moved.then_some(Sound::Move), opened],
            },
        };
        for (at, sound) in sounds.into_iter().flatten().enumerate() {
            self.effects.insert(asked + at, Effect::Sound(sound));
        }
        // What the press set waiting: a tile's A, the double tap's time.
        self.wait_out();
    }

    /// Does what the top page asked for.
    fn page_asks(&mut self, action: PageAction) {
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
        };
        match action {
            PageAction::Close => self.close_page(),
            PageAction::Open(open) => {
                self.pages.open_over(open, around);
            }
            PageAction::Effect(Effect::Set(change)) => self.set(change),
            // Settings > Guide's Now Playing source: followed at once, and saved; one that was set already, or is
            // not among the choices, changes nothing.
            PageAction::Effect(Effect::NowPlayingSource(source)) => {
                if let Some(source) = self.now_playing.choose(&source) {
                    self.effects.push(Effect::NowPlayingSource(source));
                }
            }
            // Plays from the next start: the Boot video page ticks it at once.
            PageAction::Effect(Effect::BootVideo(video)) => {
                self.options.boot_video = video;
                self.effects.push(Effect::BootVideo(video));
            }
            // Whether there is a key is the Dashboard's to show; the key itself goes on to be kept.
            PageAction::Effect(Effect::ArtKey(key)) => {
                self.options.art_key = key.is_some();
                self.effects.push(Effect::ArtKey(key));
            }
            // A page's Play is Home's: with another game running it is asked about first.
            PageAction::Effect(Effect::Launch(game)) => self.play(game),
            // And a page's Quit is the Guide's, with its ten seconds and "Force quit?".
            PageAction::Effect(Effect::QuitGame) => self.quit_game(),
            PageAction::Effect(effect) => self.effects.push(effect),
            PageAction::CloseWith(effect) => {
                self.close_page();
                self.effects.push(effect);
            }
            // What was under the page is under the new one: B there comes back to it.
            PageAction::Replace(open) => {
                self.pages.close();
                let around = pages::Around {
                    library: &self.library,
                    playing: self.playing,
                    options: self.options,
                    now: self.now,
                    now_playing: &self.now_playing,
                };
                if !self.pages.open_over(open, around) {
                    self.close_page();
                }
            }
        }
    }

    /// Where the Guide's focus is, and the volume: a press that changed any of it moved something.
    fn guide_place(&self) -> ((Tab, guide::Focus), Option<(Flyout, FlyoutFocus)>, u8) {
        (self.guide.focus(), self.guide.flyout(self.guide_around()), self.audio.volume)
    }

    /// Power, by a hold of the Guide button (docs/spec/guide.md, 7.1): over the dim alone, wherever the Dashboard
    /// is. An open Guide's panel slides away under it. Not with the Guide setting off, nor while a game starts, nor
    /// over a game with no Overlay to show it on.
    fn open_power(&mut self) {
        let nowhere = self.playing.is_some() && !self.has_overlay && !self.in_front;
        if !self.options.use_guide || nowhere || (self.launch.up() && !self.guide.is_open()) {
            return;
        }
        if !self.guide.is_open() {
            // A dialog page closes with no answer as the Guide comes.
            self.pages.close_dialog();
            // As the Guide opens: on the Overlay where there is one, which keeps the foreground if it still has it.
            self.guide_overlaid = self.has_overlay;
            self.release_awaited = None;
        }
        self.guide.open_power(Around {
            library: &self.library,
            playing: self.playing,
            discord: self.status.discord,
            audio: &self.audio,
            notifications: self.toasts.notifications(),
            now_playing: self.now_playing.card(self.now),
        });
        self.effects.push(Effect::Sound(Sound::PromptOpen));
    }

    /// Gives the press to whatever takes input now. True when it was A and it chose something.
    fn steer(&mut self, press: Press, now: Instant) -> bool {
        if self.guide.is_open() {
            let around = Around {
                library: &self.library,
                playing: self.playing,
                discord: self.status.discord,
                audio: &self.audio,
                notifications: self.toasts.notifications(),
                now_playing: self.now_playing.card(self.now),
            };
            let action = self.guide.press(press, around);
            if let Some(action) = action {
                self.guide_action(action);
            }
            // Everything but closing with B or the Guide button is a choice, and sounds as one.
            // The volume moving is a move, and sounds as one.
            return action.is_some_and(|action| !matches!(action, GuideAction::Close | GuideAction::Volume(_)));
        }
        // While a game starts, the Dashboard takes no input.
        if self.launch.up() {
            return false;
        }
        if press.button == Button::Guide {
            if !press.repeat && self.options.use_guide {
                // A tap while a Toast is on screen opens the Guide on what the Toast is about. Opening drops it.
                let toast = self.toasts.on_screen().map(|toast| toast.kind);
                self.open_guide();
                if let Some(toast) = toast.filter(|_| self.guide.is_open()) {
                    self.show_what_toast_told(toast);
                }
            }
            return false;
        }
        // Behind a running game there is nothing to steer.
        if self.playing.is_some() && !self.in_front {
            return false;
        }
        if self.pages.dialog().is_some() {
            // The dialog gets every press, and each button pressed while it is up is remembered until let go.
            if !self.held.contains(&press.button) {
                self.held.push(press.button);
            }
        } else if press.repeat && self.held.contains(&press.button) {
            // Still down from before the dialog closed: nothing under it hears of it.
            return false;
        } else {
            // Pressed afresh: it was let go, whether or not anyone said so (a key has no release).
            self.held.retain(|&held| held != press.button);
        }
        if !self.pages.is_empty() {
            let around = pages::Around {
                library: &self.library,
                playing: self.playing,
                options: self.options,
                now: self.now,
                now_playing: &self.now_playing,
            };
            let action = self.pages.press(press, around);
            // What the page under text entry asks for as its text is kept: a Group's new name, to save.
            self.take_asked();
            match action {
                Some(action) => self.page_asks(action),
                None => return false,
            }
            return press.button == Button::A;
        }
        // A held A is one A, not a double tap.
        if press.repeat && press.button == Button::A {
            return false;
        }
        match self.home.press(press.button, &self.library, now) {
            // The first A of a double tap, or the only one: the tile is chosen, and sounds so.
            Some(HomeAction::Chosen) => {}
            // The second: the pair is one choice, which has sounded. With another game running it is asked about
            // first.
            Some(HomeAction::Launch(game)) => {
                self.play(game);
                return false;
            }
            // The profile opens the Guide where the profile is.
            Some(HomeAction::OpenGuide) if !press.repeat && self.options.use_guide => {
                self.open_guide_on(Tab::ProfileAndSystem);
            }
            Some(HomeAction::Open(open)) if !press.repeat => {
                let on_button = matches!(self.home.focus(), Focus::TopBar(TopButton::Search | TopButton::Settings));
                self.pages.open(
                    open,
                    pages::Around {
                        library: &self.library,
                        playing: self.playing,
                        options: self.options,
                        now: self.now,
                        now_playing: &self.now_playing,
                    },
                );
                self.back_to_button = on_button && press.button == Button::A;
            }
            // The Guide on its Discord tab while Discord is connected (M6); with no such tab, or the Guide off,
            // Settings on Discord.
            Some(HomeAction::Discord) if !press.repeat => {
                if self.status.discord && self.options.use_guide {
                    self.open_guide_on(Tab::Discord);
                } else {
                    self.open_page(Open::Settings(settings::Entry::Discord));
                }
            }
            _ => return false,
        }
        press.button == Button::A
    }

    /// Opens a page over Home, in place of any that is up: from the Guide, or by whoever drives the Dashboard.
    /// B on it goes to Home, its rows taking focus where they last had it.
    pub fn open_page(&mut self, open: Open) {
        self.pages.open(
            open,
            pages::Around {
                library: &self.library,
                playing: self.playing,
                options: self.options,
                now: self.now,
                now_playing: &self.now_playing,
            },
        );
        self.back_to_button = false;
    }

    /// The top page goes, at once. With none left Home shows again, at the rest it was left at.
    fn close_page(&mut self) {
        self.pages.close();
        if self.pages.is_empty() {
            self.home.uncovered(std::mem::take(&mut self.back_to_button), &self.library);
        }
    }

    /// Every page goes: Home, its rows taking focus where they last had it.
    fn close_pages(&mut self) {
        if self.pages.clear() {
            self.back_to_button = false;
            self.home.uncovered(false, &self.library);
        }
    }

    /// The name of the page that has the pad, if one is up over Home.
    pub fn page(&self) -> Option<&'static str> {
        self.pages.top().map(|page| page.name())
    }

    /// How many pages are up over Home.
    pub fn pages_up(&self) -> usize {
        self.pages.depth()
    }

    /// What is on top of Home's window, the Guide aside, and where its focus is: Home's focus as it is written
    /// (`Recent(2)`), or a page's name and what it says of its own (`Settings: Rail: Theme settings`). A press that
    /// changes this moved focus.
    fn focus_place(&self) -> String {
        if let Some(startup) = &self.startup {
            return startup.focus();
        }
        match self.pages.top() {
            Some(page) => format!("{}: {}", page.name(), page.focus()),
            None => format!("{:?}", self.home.focus()),
        }
    }

    /// The Guide's own line beside `focus_line`'s, written the same way and logged as `guide: ...` when it
    /// changes: `closed`, or the tab and focus (`ProfileAndSystem Entry(3)`), then `, no panel` when Power came by
    /// a hold, then the dialog up over it and its focus (`, dialog Power(Close)`).
    ///
    /// A menu or a prompt has its title after it (`, dialog Menu { prompt: true, row: 0 } "Force quit Hades II?"`),
    /// and whatever the line is, it ends `, quitting` while the running game is being quit, and
    /// `, quitting, then Celeste` when a game is to start after it.
    pub fn guide_line(&self) -> String {
        let quitting = match self.quit {
            Some(Quit { then: Some(next), .. }) => {
                format!(", quitting, then {}", self.library.games.get(next).map_or("?", |game| game.name.as_str()))
            }
            Some(_) => ", quitting".into(),
            None => String::new(),
        };
        if !self.guide.is_open() {
            return format!("closed{quitting}");
        }
        let (tab, focus) = self.guide.focus();
        let panel = if self.guide.panel_shown() { "" } else { ", no panel" };
        let title = self.guide.dialog_title().map(|title| format!(" {title:?}")).unwrap_or_default();
        let flyout = match self.guide.flyout(self.guide_around()) {
            Some((Flyout::Audio, at)) => {
                let muted = if self.audio.muted { ", muted" } else { "" };
                format!(", flyout Audio {at:?}, volume {}{muted}", self.audio.volume)
            }
            Some((which, at)) => format!(", flyout {which:?} {at:?}"),
            None => String::new(),
        };
        match self.guide.dialog() {
            Some(dialog) => format!("{tab:?} {focus:?}{panel}, dialog {dialog:?}{title}{quitting}"),
            None => format!("{tab:?} {focus:?}{panel}{flyout}{quitting}"),
        }
    }

    /// `--log-focus`'s line, which tests on the VM judge by: where focus is (`Recent(2)`, `Settings: Rail`), and
    /// after it what the page says of its state beyond focus, if it says any (`TextEntry: A; "Ha|" lower letters`).
    pub fn focus_line(&self) -> String {
        let place = self.focus_place();
        if self.startup.is_some() {
            return place;
        }
        match self.pages.top().map(|page| page.state()) {
            Some(state) if !state.is_empty() => format!("{place}; {state}"),
            _ => place,
        }
    }

    /// The hints the hint bar shows now: those of whatever has the pad, and none under the Guide.
    pub fn hints(&self) -> &'static [hint_bar::Hint] {
        if self.guide.is_open() {
            hint_bar::NONE
        } else if self.pages.is_empty() {
            self.home.hints(&self.library)
        } else {
            self.pages.hints(self.around())
        }
    }

    fn guide_action(&mut self, action: GuideAction) {
        match action {
            GuideAction::Close => self.close_guide(),
            // To the game's own window, whatever the Guide opened over.
            GuideAction::Resume => {
                self.close_guide();
                self.show_game();
            }
            // Over a running game, the Dashboard comes in front of it, as the Bundle brings Playnite.
            GuideAction::Home => {
                self.close_pages();
                self.close_guide();
                self.show_dashboard();
            }
            GuideAction::MyGames => self.guide_opens(Open::MyGames(my_games::Entry::Games)),
            GuideAction::Settings => self.guide_opens(Open::Settings(settings::Entry::Rail)),
            GuideAction::Search => self.guide_opens(Open::Search),
            // The Guide stays open over a game being quit, and closes a moment after the game has stopped.
            GuideAction::QuitGame => self.quit_game(),
            // The game that was running is quit, and the one chosen starts when it has stopped.
            GuideAction::QuitAndStart(game) => {
                self.quit_game();
                if let Some(quit) = &mut self.quit {
                    quit.then = Some(game);
                }
            }
            // Ended, not asked: and asked about again if it is somehow still there after as long again.
            GuideAction::ForceQuit => {
                if let Some(quit) = &mut self.quit {
                    (quit.left, quit.asked) = (QUIT_TIMEOUT, false);
                    self.effects.push(Effect::ForceQuitGame);
                }
            }
            GuideAction::Launch(game) if self.playing.is_none() && !self.launch.up() => {
                self.home.focus_recent(game, &self.library);
                self.close_guide();
                self.effects.push(Effect::Launch(game));
            }
            // With a game running the Guide stays, under the question.
            GuideAction::Launch(game) => self.play(game),
            GuideAction::Asked => {}
            // The level shows at once; the shell's next reading says what the system made of it.
            GuideAction::Volume(level) => {
                self.audio.volume = level;
                self.effects.push(Effect::Volume(level));
            }
            GuideAction::Mute(muted) => {
                self.audio.muted = muted;
                self.effects.push(Effect::Mute(muted));
            }
            GuideAction::Device(flow, device) => {
                match flow {
                    Flow::Output => self.audio.output = Some(device),
                    Flow::Input => self.audio.input = Some(device),
                }
                self.effects.push(Effect::DefaultDevice(flow, device));
            }
            // The app the card shows is asked; the card shows nothing changed until its session says so.
            GuideAction::Media(command) => {
                if let Some(card) = self.now_playing.card(self.now) {
                    self.effects.push(Effect::Media(command, card.session.app_id.clone()));
                }
            }
            // The Guide closes first; then what was chosen is the shell's to do.
            GuideAction::Power(choice) => {
                self.close_guide();
                match choice {
                    PowerChoice::Machine(command) => self.effects.push(Effect::Power(command)),
                    // Leaving has a sound of its own, which whoever leaves lets play out first.
                    PowerChoice::Exit => self.effects.extend([Effect::Sound(Sound::PowerOff), Effect::Exit]),
                }
            }
        }
    }

    /// The Guide closes onto a page, which takes the place of any that is up: in front of a running game, too.
    fn guide_opens(&mut self, open: Open) {
        self.close_guide();
        self.open_page(open);
        self.show_dashboard();
    }

    /// Opens the Guide, whatever the Guide setting says: that is for the Guide button.
    pub fn open_guide(&mut self) {
        self.open_guide_on(Tab::Landing);
    }

    /// Opens the Guide on a tab.
    pub fn open_guide_on(&mut self, tab: Tab) {
        // Over a game the Guide is on the Overlay; with no Overlay there is nowhere to show it.
        if self.playing.is_some() && !self.has_overlay && !self.in_front {
            return;
        }
        // A dialog page closes with no answer as the Guide comes (docs/spec/tile-menu.md, section 6).
        self.pages.close_dialog();
        let around = Around {
            library: &self.library,
            playing: self.playing,
            discord: self.status.discord,
            audio: &self.audio,
            notifications: self.toasts.notifications(),
            now_playing: self.now_playing.card(self.now),
        };
        self.guide.open_on(tab, around);
        self.guide_overlaid = self.has_overlay;
        // Opened again before the foreground went back: the Overlay still has it, and keeps it.
        self.release_awaited = None;
        // What the Toasts told of is in the Guide now.
        self.toasts.drop_all();
    }

    /// The Guide, just opened by a tap while a Toast of this kind was on screen, shows what the Toast was about
    /// (docs/spec/guide.md, 9.5).
    fn show_what_toast_told(&mut self, toast: ToastKind) {
        match toast {
            ToastKind::Notification => self.open_notifications(),
            ToastKind::Greeting => {
                let around = Around {
                    library: &self.library,
                    playing: self.playing,
                    discord: self.status.discord,
                    audio: &self.audio,
                    notifications: self.toasts.notifications(),
                    now_playing: self.now_playing.card(self.now),
                };
                self.guide.show_tab(guide::Tab::ProfileAndSystem, around);
            }
        }
    }

    /// Whether the Guide's Notifications Flyout is open: a notification that comes meanwhile is listed there and
    /// makes no Toast.
    fn notifications_open(&self) -> bool {
        matches!(self.flyout(), Some((Flyout::Notifications, _)))
    }

    /// Adds a notification to the list the Notifications Flyout shows, which follows if it is open, with no Toast:
    /// one that was there before the Dashboard started. Gives its id in the list.
    pub fn record(&mut self, text: &str) -> u64 {
        let id = self.toasts.record(text);
        self.refresh_guide();
        id
    }

    /// Posts a notification: it joins the list the Notifications Flyout shows, and tells of itself in a Toast.
    /// Gives its id in the list.
    pub fn notify(&mut self, text: &str) -> u64 {
        let id = self.record(text);
        if !self.notifications_open() {
            // With the Guide off the Guide button does nothing, and the Toast does not say to press it.
            let prompt = self.options.use_guide;
            self.toast(Toast { kind: ToastKind::Notification, text: text.to_string(), prompt, picture: None });
        }
        id
    }

    /// Every notification posted since the Dashboard started, oldest first: what the Notifications Flyout lists,
    /// newest first.
    pub fn notifications(&self) -> &[Notification] {
        self.toasts.notifications()
    }

    /// Greets the Profile signed in, as the Dashboard starts: "Hi <first name>!" in a Toast with its picture and
    /// no prompt. (Who's playing today?, #127, signs one in first and greets it so.)
    pub fn greet(&mut self) {
        let profile = self.profile();
        let text = format!("Hi {}!", profile.first_name());
        let picture = Some(profile.picture);
        self.toast(Toast { kind: ToastKind::Greeting, text, prompt: false, picture });
    }

    /// The Profile signed in, which every place a Profile shows draws (docs/spec/profiles.md): the one the library
    /// database remembers, or with no database the store's user under the first plain picture.
    pub fn profile(&self) -> Profile {
        profile::signed_in(&self.library)
    }

    /// What the log says of the Profile signed in, as Home's top bar, the Guide and the greeting show it:
    /// `"Alpha", real name "Ann Smith", shape 3`. The executable says it (`profile: ...`) whenever it changes.
    pub fn profile_line(&self) -> String {
        let profile = self.profile();
        format!("{:?}, real name {:?}, {}", profile.gamertag, profile.real_name, profile.picture.describe())
    }

    /// Signs this Profile in: every place a Profile shows has it at once, and the library database is asked to
    /// remember it as the one signed in last. The hook Who's playing today? and Switch profile sign in by (#127);
    /// nothing calls it yet. A Profile the library has not got changes nothing.
    pub fn sign_in(&mut self, profile: i64) {
        if self.library.profiles.get(profile).is_none() {
            return;
        }
        self.library.profiles.signed_in = Some(profile);
        self.refresh_guide();
        self.effects.push(Effect::Profile(ProfileChange::SignIn(profile)));
    }

    fn toast(&mut self, toast: Toast) {
        if self.toasts.post(toast) {
            self.effects.push(Effect::Sound(Sound::Notification));
        }
    }

    /// The Toasts' picture as it is now has been put on the screen, or taken off it: said by whoever shows it,
    /// after each frame and after hiding it. The queue goes by it (`toasts::Toasts::presented`).
    /// `seconds` is the time since the Dashboard was last told the time, by this or by `advance`: a frame is out
    /// well into a turn of the loop.
    pub fn toasts_presented(&mut self, seconds: f32) {
        self.toasts.tick(seconds);
        self.pass(seconds);
        self.toasts.presented();
    }

    /// Says whether the renderer is a slow one: the processor, where there is no GPU, which takes long over a
    /// frame of Home with the swirl moving. On one, the swirl holds still while a Toast is on screen or coming, as
    /// it does under the Guide, so the Toast's own frames are not kept waiting (docs/spec/guide.md, 9.4).
    pub fn set_slow_renderer(&mut self, slow: bool) {
        self.slow_renderer = slow;
    }

    /// Whether Home's window gives way to the Toasts: on a slow renderer, while one is on screen or coming.
    pub fn home_gives_way(&self) -> bool {
        self.slow_renderer && self.toasts.busy()
    }

    /// The Toasts: which is on screen, how many wait, how many have shown.
    pub fn toasts(&self) -> &Toasts {
        &self.toasts
    }

    /// Says whether Toasts have a window of their own, above every other: the shell's strip. Without one, as until
    /// told otherwise, they are drawn in Home's window, and so are not seen over a game.
    pub fn set_toast_strip(&mut self, there: bool) {
        self.toast_strip = there;
    }

    /// The Toast on screen, alone: what the strip shows. Drawn over nothing, as the Overlay is.
    pub fn draw_toasts(&mut self, ui: &mut Ui) {
        ui.store.reduce_motion = self.options.reduce_motion;
        self.toasts.draw(ui);
    }

    pub fn close_guide(&mut self) {
        if self.guide.is_open() {
            self.guide.close();
            self.close_in = None;
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

    /// Whether the Guide is open over a game: the game is then held still under it (docs/spec/guide.md, 7.2).
    pub fn guide_over_game(&self) -> bool {
        self.guide_overlaid && self.guide.is_open() && self.playing.is_some()
    }

    /// Whether the Guide has closed and is keeping the foreground until the button that closed it is let go.
    pub fn awaiting_release(&self) -> bool {
        self.release_awaited.is_some()
    }

    /// The button that closed the Guide has been down `RELEASE_FALLBACK` since: the foreground goes back now.
    pub fn release_wait_over(&mut self) {
        self.release_awaited = None;
    }

    /// Says whether there is an Overlay to draw on, as there is unless told otherwise. Without one no Launch screen
    /// shows and the Guide does not open over a game: neither can be drawn in Home's window, which is behind the
    /// game. At Home the Guide is then drawn in Home's window.
    pub fn set_overlay(&mut self, there: bool) {
        self.has_overlay = there;
        if !there {
            // Whatever it was showing is gone with it, but for the Guide at Home, which Home's window can show.
            self.launch = LaunchScreen::default();
            if self.guide_overlaid && self.playing.is_some() {
                self.guide = Guide::default();
            }
            self.guide_overlaid = false;
            self.release_awaited = None;
        }
    }

    /// What the Overlay is wanted for now.
    pub fn overlay(&self) -> OverlayShows {
        let guide = self.guide_overlaid && self.guide.showing();
        if self.guide_overlaid && (self.guide.is_open() || self.release_awaited.is_some()) {
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
        self.playing.is_some() && !self.in_front && (!self.launch.up() || self.launch.covers())
    }

    /// The shell has asked the store for the game: its Launch screen comes up.
    pub fn launch_started(&mut self, game: usize) {
        if let Some(found) = self.library.games.get(game) {
            if self.has_overlay {
                self.launch.show(found);
            }
            self.playing = Some(game);
            (self.in_front, self.quit, self.close_in) = (false, None, None);
            self.refresh_guide();
        }
    }

    /// The Launch screen goes: the game's own window is up, or something else wants the screen.
    pub fn game_booted(&mut self) {
        self.launch.hide();
    }

    /// The game ended, or never started: Home again, focus on its tile.
    ///
    /// An open Guide stays open, as the Bundle's does (docs/spec/guide.md, 3.4): its list is made again without the
    /// game, a menu or a prompt about the game goes in silence, and Power stays up. Closing it then lands on Home.
    /// Only a game that was quit from the Guide takes the Guide with it: `CLOSE_AFTER_QUIT` later, or at once when
    /// another game was to start after it, which is then asked of the shell.
    pub fn game_ended(&mut self) {
        // A page that was up goes with it.
        self.close_pages();
        if let Some(game) = self.playing.take() {
            self.home.focus_recent(game, &self.library);
        }
        self.launch.hide();
        self.in_front = false;
        let quit = self.quit.take();
        // A prompt that was up on its own goes with the dim it showed over.
        let alone = self.guide.is_open() && !self.guide.panel_shown() && self.guide.dialog_title().is_some();
        self.refresh_guide();
        if alone {
            self.close_guide();
        }
        if let Some(next) = quit.and_then(|quit| quit.then).filter(|&next| next < self.library.games.len()) {
            self.home.focus_recent(next, &self.library);
            self.close_guide();
            self.effects.push(Effect::Launch(next));
        } else if quit.is_some() && self.guide.is_open() {
            self.close_in = Some(CLOSE_AFTER_QUIT);
        }
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
        let around = Around {
            library: &self.library,
            playing: self.playing,
            discord: self.status.discord,
            audio: &self.audio,
            notifications: self.toasts.notifications(),
            now_playing: self.now_playing.card(self.now),
        };
        // The pad's own word on its charge, unless the shell has said otherwise. One on a cable shows its glyph.
        let charge = match self.pad.battery {
            Some(Battery::Charge(charge)) => Some((charge.clamp(0.0, 1.0) * 100.0).round() as u8),
            _ => None,
        };
        let show_percent = self.status.show_percent && self.options.show_battery_percent;
        // The badge counts the notifications the Dashboard itself holds, unless the shell has told of more.
        let notifications = self.status.notifications.max(self.toasts.notifications().len() as u32);
        let status = Status {
            pad_battery: self.status.pad_battery.or(charge),
            show_percent,
            notifications,
            ..self.status.clone()
        };
        let profile = self.profile();
        let face = Face { profile: &profile, clock: &clock, pad: self.pad.connected, status: &status };
        self.guide.draw(ui, around, &face);
    }

    /// Home's window: Home or the page over it, and the hint bar; the Guide over them where there is no Overlay to
    /// draw it on; and the Toast, where Toasts have no window of their own.
    pub fn draw(&mut self, ui: &mut Ui) {
        if self.draw_startup(ui) {
            return;
        }
        self.draw_home(ui);
        if !self.toast_strip {
            self.draw_toasts(ui);
        }
    }

    fn draw_home(&mut self, ui: &mut Ui) {
        ui.store.reduce_motion = self.options.reduce_motion;
        let clock = self.clock();
        // A wireless pad's charge, as the Guide's Status area has it; one on a cable shows its glyph.
        let pad_battery = self.status.pad_battery.or(match self.pad.battery {
            Some(Battery::Charge(charge)) => Some((charge.clamp(0.0, 1.0) * 100.0).round() as u8),
            _ => None,
        });
        let profile = self.profile();
        let status = home::Status {
            profile: &profile,
            clock: &clock,
            pad: self.pad.connected,
            pad_battery,
            show_percent: self.status.show_percent && self.options.show_battery_percent,
        };
        // A page that fills the screen is there in Home's place, top bar and all: nothing of Home is drawn, but for
        // the picture a page takes of the screen it was opened over. Under a page that leaves it showing (the Tile
        // menu) Home goes on as it was.
        // On a slow renderer it holds still while it gives way to a Toast.
        self.home.set_in_front(!self.guide.is_open() && !self.pages.hides_home() && !self.home_gives_way());
        if !self.pages.hides_home() {
            self.home.draw(ui, &self.library, &status);
        }
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
        };
        let (home, library) = (&mut self.home, &self.library);
        self.pages.draw(ui, around, &mut |ui| home.draw(ui, library, &status));
        // The hint bar, once for whatever has the pad, over all of it.
        hint_bar::draw(ui, self.hints());
        if !self.guide_overlaid {
            self.draw_guide(ui);
        }
    }

    /// The Overlay: the Launch screen, and the Guide. Drawn over nothing, so where it draws nothing the game shows,
    /// or Home.
    pub fn draw_overlay(&mut self, ui: &mut Ui) {
        // On a slow renderer the Launch screen comes up whole: a fade there is two faint frames and a long wait for
        // the third, with the game's launcher in plain sight meanwhile.
        ui.store.reduce_motion = self.options.reduce_motion;
        self.launch.draw(ui, self.slow_renderer);
        if self.guide_overlaid {
            self.draw_guide(ui);
        }
    }

    /// All in one frame, the Overlay's picture over Home's window's and the Toast over both: what an offscreen
    /// frame shows.
    pub fn draw_all(&mut self, ui: &mut Ui) {
        if self.draw_startup(ui) {
            return;
        }
        self.draw_home(ui);
        self.draw_overlay(ui);
        self.draw_toasts(ui);
    }

    // --- Start-up (docs/spec/start-up.md) ---

    /// Start-up begins, as the Dashboard starts: with the Boot video if one is to play (`video`: it was chosen and
    /// found, and the executable has it opening), else at once with what follows it. Over everything until it ends,
    /// with the pad and the Guide button its own.
    pub fn start_up(&mut self, video: bool) {
        if video {
            self.startup = Some(Startup::boot_video());
        } else {
            self.after_boot_video();
        }
    }

    /// Whether Start-up shows: nothing else is drawn, and every press is its.
    pub fn starting_up(&self) -> bool {
        self.startup.is_some()
    }

    /// Whether the Boot video is what shows: the executable plays it while this says so, and stops it when it
    /// stops saying so (a press skipped it).
    pub fn boot_video_playing(&self) -> bool {
        self.startup.as_ref().is_some_and(|startup| startup.phase() == Phase::BootVideo)
    }

    /// The video's first frame is in this texture, `aspect` wide to high: it shows from the next frame on, and a
    /// press skips it a moment after (`startup::SKIP_AFTER`).
    pub fn boot_video_picture(&mut self, tex: ui::TexId, aspect: f32) {
        if let Some(startup) = self.startup.as_mut().filter(|startup| startup.phase() == Phase::BootVideo) {
            startup.set_picture(tex, aspect);
        }
    }

    /// The Boot video has ended, or could not play, or ran too long: Start-up goes on.
    pub fn boot_video_ended(&mut self) {
        if self.boot_video_playing() {
            self.after_boot_video();
        }
    }

    /// What comes after the Boot video, or at once with it Off.
    ///
    /// **The hook for #127**: Who's playing today? and Signing you in go here, as `startup::Phase`s of their own
    /// that end Start-up as Signing you in has shown, the Profile signed in (`sign_in`) and greeted. Until they come,
    /// Start-up ends with the video: Home shows, and the executable greets the Profile signed in last
    /// (`greet`) once Toasts have somewhere to show.
    fn after_boot_video(&mut self) {
        self.startup = None;
    }

    /// A press while Start-up shows. Any button skips the Boot video, once its picture has been up a moment; no
    /// sound, as in the Bundle. Nothing else is told of it.
    fn startup_press(&mut self, press: Press) {
        let Some(startup) = &self.startup else { return };
        if startup.phase() == Phase::BootVideo && startup.takes_presses() && !press.repeat {
            self.after_boot_video();
        }
    }

    /// The pad while Start-up shows: a button going down is a press, the Guide button's release or hold too. What
    /// is still to come of a Guide button press Start-up took (its tap, its release after a hold) does nothing
    /// once Home shows: the Guide does not open behind the video.
    fn startup_pad_event(&mut self, event: PadEvent) {
        let guide = Press { button: Button::Guide, repeat: false };
        match event {
            PadEvent::Press(press) => self.startup_press(press),
            // A hold: its release follows, and no tap.
            PadEvent::GuideHeld => {
                self.guide_button_spent = true;
                self.startup_press(guide);
            }
            // The release of a hold Start-up took ends that press; a tap's has its tap still to come.
            PadEvent::Release(Button::Guide) => {
                if !std::mem::take(&mut self.guide_button_spent) {
                    self.guide_button_spent = true;
                    self.startup_press(guide);
                }
            }
            PadEvent::GuideTap => self.guide_button_spent = false,
            PadEvent::Release(_) => {}
        }
    }

    /// Start-up's picture, if it shows. Home is drawn under it once, unseen, as the picture first shows, so its
    /// art is decoded behind the video.
    fn draw_startup(&mut self, ui: &mut Ui) -> bool {
        let Some(warm) = self.startup.as_mut().map(Startup::warm_home) else { return false };
        if warm {
            ui.with_alpha(0.0, |ui| self.draw_home(ui));
        }
        if let Some(startup) = &self.startup {
            startup.draw(ui);
        }
        true
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
        source_id: (1000 + i).to_string(),
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
        let (focused, playing) = (dashboard.library().games[2].key(), dashboard.library().games[4].key());

        // The focused game has been played since: it is the Recent row's first now.
        let mut next = fake_library();
        let mut game = next.games.remove(2);
        game.last_played = next.games[0].last_played + 60;
        next.games.insert(0, game);
        next.user = Some("Someone Else".into());
        assert_eq!(dashboard.profile().gamertag, "Player One");
        dashboard.set_library(next);

        assert_eq!(dashboard.home_focus(), Focus::Recent(0));
        assert_eq!(dashboard.focused_game().map(|g| dashboard.library().games[g].key()), Some(focused));
        assert_eq!(dashboard.playing().map(|g| dashboard.library().games[g].key()), Some(playing));
        assert_eq!(dashboard.profile().gamertag, "Someone Else");
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
