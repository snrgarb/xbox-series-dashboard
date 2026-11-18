//! The Dashboard's core: every screen and what is open, with no window and nothing of Windows in it. The executable
//! feeds it the pad's presses and what the shell saw happen (a game started, it ended), draws it each frame, and
//! carries out the effects it asks for. Tests do the same with no window at all.
//!
//! It declares two pictures: Home's window's (`draw`) and the Overlay's (`draw_overlay`), the window above every
//! other that shows the Launch screen and the Guide, over a running game and over Home alike. `overlay` says what
//! the Overlay is wanted for, which the executable follows by showing and hiding it.
//! Toasts are a third (`draw_toasts`), for the strip above both.
pub mod achievement;
pub mod achievements;
pub mod agreement;
mod app_tile;
pub mod art_picker;
pub mod date;
pub mod discord_sign_in;
pub mod drawn;
pub mod game_hub;
pub mod guide;
pub mod hint_bar;
pub mod home;
pub mod idle;
pub mod install_from;
pub mod install_where;
pub mod keyboard;
pub mod launch;
pub mod manage_queue;
pub mod menu_panel;
pub mod my_games;
pub mod now_playing;
pub mod pages;
mod path_data;
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

/// The world a page is told of, made from fields so that a field of the Dashboard can be borrowed mutably beside it.
macro_rules! around_of {
    ($s:expr) => {
        pages::Around {
            library: &$s.library,
            playing: $s.playing,
            options: $s.options,
            now: $s.now,
            now_playing: &$s.now_playing,
            discord: &$s.discord,
        }
    };
}

use crate::guide::{Around, DialogFocus, Face, Flyout, FlyoutFocus, Guide, GuideAction, PowerChoice, Status, Tab};
use crate::home::{Focus, Home, HomeAction, TopButton};
use crate::launch::{Backdrop, LaunchScreen};
use crate::pages::{Open, PageAction};
pub use crate::settings::{Options, Setting, WhilePlaying};
use crate::startup::Startup;
pub use crate::startup::{BootVideo, Found, Phase};
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
    /// Settings > Idle changed its times: the Dashboard counts by them already, from now; they are for whoever
    /// keeps the settings to save (`idle::Times::saved`).
    IdleTimes(idle::Times),
    /// Mark the game with this id in the library database as a favourite, or as one no longer. The shell answers
    /// with a new snapshot of the library (`set_library`).
    Favourite { game: i64, on: bool },
    /// Hide the game with this id from every list but the Hidden filter's, or show it again. A new snapshot
    /// follows, as for a favourite.
    Hidden { game: i64, on: bool },
    /// Ask its store to install the game with this id in the library database: the store's own window does it.
    Install(i64),
    /// Ask its store to install the game with this id into this folder, one of its store's (`Library::storage`):
    /// the answer to "Where do you want to install X?" (docs/spec/storage.md).
    InstallTo { game: i64, folder: std::path::PathBuf },
    /// Ask its store to uninstall the game with this id. The player has said yes to "Uninstall X?".
    Uninstall(i64),
    /// Ask its store to pause, resume or cancel the install under way of the game with this id: a queued game's
    /// menu in the Queue (docs/spec/manage-queue.md, section 8). The Queue follows what the store then does.
    Steer { game: i64, steer: model::Steer },
    /// The user's answer to the licence agreement's prompt asked with this number (`Dashboard::ask_agreement`,
    /// #230): for whoever waits on it. Only the user's own A on Accept is `Reply::Accept`.
    Agreement { id: u64, reply: model::Reply },
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
    /// Settings > Art's sources: the online art sources' order, and which are on, as the user has just set them
    /// (`model::ArtOrder`). In force on the page at once; whoever keeps the settings saves it, and the art lookup
    /// and the Art picker follow it.
    ArtOrder(model::ArtOrder),
    /// Settings > Launchers' Game Pass region (#277): the region whose PC Game Pass catalogue is listed, None for
    /// Windows' own. In force on the page at once; whoever keeps the settings saves it and reads Xbox again.
    GamePassRegion(Option<model::game_pass::Region>),
    /// Settings > Art: remove the art downloaded into the data folder (the user has said yes). A store's own art is
    /// not touched.
    ClearArt,
    /// Settings > Art's "Refresh all art and details" (none), or the Tile menu's "Refresh art" for one game:
    /// forget what was downloaded for it and fetch it again in the background (docs/spec/settings.md, 16.8). The
    /// snapshots that follow say how a refresh of all goes (`Library::art_refresh`).
    RefreshArt(Option<model::GameKey>),
    /// The art picker's choice of a game's picture of a kind (docs/spec/art-picker.md): to keep as the user's own
    /// in the library database, the full picture downloaded first for a SteamGridDB one; `chosen: None` puts the
    /// Dashboard's own pick back. A new snapshot follows.
    ChooseArt(model::art_picker::Choice),
    /// Settings > Launchers switched the launcher with this key (`Game::source`) on or off (docs/spec/launchers.md).
    /// A new snapshot follows, in which its games are back, or in no list at all.
    Launcher { key: String, on: bool },
    /// Settings > Storage chose the folder a store's installs go to unless the user says otherwise (the store by its
    /// key, `Game::source`). A new snapshot follows with it as the store's default (`Library::storage`).
    DefaultFolder { store: String, folder: std::path::PathBuf },
    /// Read the drives and the stores' install folders again, off the UI thread: Settings > Storage has opened. A
    /// new snapshot follows once they are read.
    LookAtStorage,
    /// Settings > Launchers' Update now: read every launcher switched on again, and fetch what is missing or out of
    /// date. The snapshots that follow say how it goes (`Library::launchers`).
    UpdateLaunchers,
    /// Settings' Profiles pages added, renamed or removed a Profile, or changed its picture; or a Profile signed in
    /// (`Dashboard::sign_in`). A new snapshot follows, as for a favourite, and every place a Profile shows follows
    /// it (docs/spec/profiles.md).
    Profile(ProfileChange),
    /// Settings > General > Boot video chose this one, or Off (docs/spec/start-up.md). The Dashboard has it
    /// already: this is for whoever keeps the settings to save, and it plays from the next start.
    BootVideo(BootVideo),
    /// Settings > Discord: a Client ID or Secret typed, a Paste, Connect or Disconnect (docs/discord.md). The
    /// connection answers with how it is now (`Dashboard::set_discord`). Its `Debug` never shows what was typed.
    Discord(model::discord::Ask),
    /// Settings > Account > Discord messages and its sign-in page (#197, docs/discord-web.md): show or put away
    /// Discord's sign-in page, a key from the pad, text the Console keyboard kept, Sign out. The session answers with
    /// how it stands (`Options::discord_web`). Its `Debug` never shows the text.
    DiscordWeb(model::discord_web::WebAsk),
    /// What the Guide's Direct Messages ask of the web session (docs/spec/party-chats.md): open a conversation, older
    /// messages, send, react, mark read. Each is shown done only once the session's stores say so; the session does
    /// one at a time, paced. Its `Debug` never shows a name, an id or a message.
    Direct(model::discord_dms::DirectAsk),
    /// Settings > Account > Direct Messages > While playing chose this: the Dashboard has it already
    /// (`Options::while_playing`); whoever keeps the settings saves it as `settings::DISCORD_WHILE_PLAYING`.
    DiscordWhilePlaying(settings::WhilePlaying),
    /// Settings > System: turn shell mode on (the user has said yes) or off (docs/spec/shell-mode.md). The
    /// executable answers with how it is then, in `Options::shell`.
    ShellMode(bool),
    /// Settings > About's Check for updates (docs/spec/updates.md): ask the update address for the newest release,
    /// and download and check a newer one. The executable answers with how it goes, in `Options::updates`.
    CheckForUpdates,
    /// Settings > About's Install, with an update ready: put it in place and start it. The executable answers as
    /// above; once the new one has started this one exits.
    InstallUpdate,
    /// Settings > Helpers (docs/spec/helpers.md): fetch, check and keep a helper on the user's Yes, its licence and
    /// source in front of them, or remove it. The executable answers with how each stands, in `Library::helpers`.
    Helper(model::helper::HelperAsk),
    /// Settings > Set up this PC (docs/spec/set-up.md): install a launcher's app on the user's Yes, the vendor, the
    /// address and the size in front of them, or remove it with its own uninstaller. The executable answers with how
    /// each stands, in `Library::clients`.
    SetUp(model::setup::SetUpAsk),
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
    /// Home's window behind the game while one is up: its Launch screen's picture, fading to Home when it ends.
    backdrop: Backdrop,
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
    /// The Discord connection as the shell last told it (docs/discord.md): what Settings > Discord shows.
    discord: model::discord::Discord,
    /// Discord's Direct Messages as the web session last said (docs/spec/party-chats.md).
    direct: model::discord_dms::Direct,
    /// Text entry over a conversation's reply box: the keyboard, and the conversation it is for (the Guide is not a
    /// page, so text entry is kept here and drawn over it).
    writing: Option<(keyboard::TextEntry, model::discord_dms::Id)>,
    /// The Dashboard's clock: where the time it is told (`advance`, `toasts_presented`) has brought it. Nothing in
    /// the Dashboard reads the system's.
    now: Instant,
    /// Start-up, while it shows (docs/spec/start-up.md): full screen over everything, with the pad and the Guide
    /// button to itself.
    startup: Option<Startup>,
    /// The Idle screen (docs/spec/idle-screen.md): its stage, counted on the Dashboard's clock.
    idle: idle::Idle,
    /// What the Idle screen's cards count "Played yesterday" from, in place of the machine's date: a test's.
    date: Option<chrono::NaiveDate>,
    /// Start-up just over: its swirl, lifting off Home, until it has gone.
    leaving: Option<Startup>,
    /// A Start-up has begun and its greeting has not been posted: Toasts wait for it, and it is posted once
    /// Start-up is over and Toasts have somewhere to show.
    greeting_owed: bool,
    /// The greeting has been posted since the Dashboard started.
    greeted: bool,
    /// Toasts have somewhere to show (`toasts_ready`).
    toasts_ready: bool,
    /// Toasts that came while Start-up showed or its greeting was owed: they follow the greeting.
    held_toasts: Vec<Toast>,
    /// Achievements unlocked, waiting a second for others found with them: one Toast tells of them all.
    unlocks: toasts::achievement::Gather,
    effects: Vec<Effect>,
}

impl Dashboard {
    pub fn new(library: Library) -> Self {
        let now = Instant::now();
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
            backdrop: Backdrop::default(),
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
            discord: model::discord::Discord::default(),
            direct: model::discord_dms::Direct::default(),
            writing: None,
            now,
            startup: None,
            idle: idle::Idle::new(now),
            date: None,
            leaving: None,
            greeting_owed: false,
            greeted: false,
            toasts_ready: false,
            held_toasts: Vec::new(),
            unlocks: Default::default(),
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
        // Nor what each store can be asked of its games: each keeps what it had.
        for entry in library.queue.entries.iter_mut().filter(|entry| entry.can == model::Can::NOTHING) {
            entry.can = self.library.queue.get(entry.game).map_or(model::Can::NOTHING, |was| was.can);
        }
        // Nor which games stopped and why (the follower keeps them, not the database): they stay, but for a game
        // installing again or installed now.
        if library.queue.stopped.is_empty() {
            let gone = |game: i64| {
                library.queue.get(game).is_some() || library.games.iter().any(|g| g.id == game && g.installed)
            };
            let stopped = self.library.queue.stopped.iter().filter(|stopped| !gone(stopped.game)).cloned().collect();
            library.queue.stopped = stopped;
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
            discord: &self.discord,
        };
        self.pages.library_changed(around);
    }

    /// How a refresh of every game's art goes (Settings > Art, `Library::art_refresh`), with no new snapshot: a page
    /// that shows it is told, as of any change to the library. Whether it changed.
    /// How the helpers stand (Settings > Helpers), without a new snapshot: whether that changed anything.
    pub fn set_helpers(&mut self, helpers: Vec<model::helper::Helper>) -> bool {
        if self.library.helpers == helpers {
            return false;
        }
        self.library.helpers = helpers;
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
            discord: &self.discord,
        };
        self.pages.library_changed(around);
        true
    }

    /// How the launchers' apps stand (Settings > Set up this PC), without a new snapshot: whether that changed
    /// anything.
    pub fn set_clients(&mut self, clients: Vec<model::setup::Client>) -> bool {
        if self.library.clients == clients {
            return false;
        }
        self.library.clients = clients;
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
            discord: &self.discord,
        };
        self.pages.library_changed(around);
        true
    }

    pub fn set_art_refresh(&mut self, refresh: model::ArtRefresh) -> bool {
        if self.library.art_refresh == refresh {
            return false;
        }
        self.library.art_refresh = refresh;
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
            discord: &self.discord,
        };
        self.pages.library_changed(around);
        true
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
            discord: &self.discord,
        };
        self.pages.library_changed(around);
        shown
    }

    /// The names of Home's Groups, in order, as Home shows them now: what `--log-focus` says of them
    /// (`groups: ...`), since a Group's place is all Home's focus says of it.
    pub fn home_groups(&self) -> Vec<String> {
        home::groups::of(&self.library).iter().map(|group| group.name.to_string()).collect()
    }

    /// What `--log-focus` says of the game's art behind Home as it was last drawn (`home background: 1920x620 art
    /// drawn over 0,0 3840x2160, uv 0.213,0.000,0.787,1.000`): its decoded size, its box on the output in pixels
    /// and the part of it that fills the box (#247). None with the swirl alone.
    pub fn home_background_line(&self) -> Option<String> {
        self.home.background_line().map(str::to_string)
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

    /// The Discord connection as it is now (docs/discord.md): Settings > Discord shows it, the Guide has its
    /// Discord tab while it has one (docs/spec/discord-tab.md), and Home's Discord tile opens that tab then.
    pub fn set_discord(&mut self, discord: &model::discord::Discord) {
        let tab = discord.has_tab() || self.direct.state != model::discord_dms::DirectState::Off;
        if self.discord != *discord || self.status.discord != tab {
            self.discord = discord.clone();
            self.status.discord = tab;
            self.guide.set_discord(discord);
            self.refresh_guide();
        }
    }

    /// Discord's Direct Messages as the web session now says (docs/spec/party-chats.md): the Discord tab has them
    /// as its section, the tab is there while Direct Messages are on whether or not the voice connection is, and an
    /// open All Direct Messages or conversation follows. Whatever the Guide asks of the session in answer is in
    /// `take_effects` as `Effect::Direct`.
    pub fn set_direct(&mut self, direct: &model::discord_dms::Direct) {
        if self.direct == *direct {
            return;
        }
        self.direct = direct.clone();
        // Settings' status line and focus log follow it.
        self.options.direct_state = direct.state;
        self.tick_page();
        self.status.discord = self.discord.has_tab() || direct.state != model::discord_dms::DirectState::Off;
        self.guide.set_direct(direct);
        self.refresh_guide();
        self.take_direct();
    }

    /// Direct Messages as the Dashboard was last told them.
    pub fn direct(&self) -> &model::discord_dms::Direct {
        &self.direct
    }

    /// Something the web session tells once, as it happens. A captcha or a refusal is a prompt over the Guide, which
    /// opens for it; the Toasts for a message, a mention and a ring are ticket #202's, and are not built yet.
    pub fn direct_event(&mut self, event: &model::discord_dms::DirectEvent) {
        use model::discord_dms::DirectEvent;
        match event {
            DirectEvent::Captcha | DirectEvent::Refused(_) => {
                if !self.open_guide_for_a_prompt() {
                    return;
                }
                match event {
                    DirectEvent::Refused(reason) => self.guide.ask_refused(reason.as_str()),
                    _ => self.guide.ask_captcha(),
                }
                self.effects.push(Effect::Sound(Sound::PromptOpen));
            }
            // Toasts (#202): the hook is here.
            DirectEvent::Sent { channel, nonce, message } => self.guide.sent(channel, *nonce, message),
            DirectEvent::NotSent { channel, nonce } => self.guide.not_sent(channel, *nonce),
            DirectEvent::Message { .. }
            | DirectEvent::Mention { .. }
            | DirectEvent::Ring(_)
            | DirectEvent::RingEnded(_) => {}
        }
    }

    /// What time it is for the times of messages: the Unix milliseconds and the zone's offset east of UTC in
    /// seconds, or None for the machine's own. A test's, so a picture is of one moment.
    pub fn set_time(&mut self, time: Option<(i64, i32)>) {
        let when = time.map_or(guide::When::default(), |(now, offset)| guide::When::at(now, offset));
        self.guide.set_when(when);
    }

    /// What the Guide asked of the web session, and the text entry it wants, as effects.
    fn take_direct(&mut self) {
        for ask in self.guide.take_asks() {
            self.effects.push(Effect::Direct(ask));
        }
        if let Some(write) = self.guide.take_write() {
            let entry = keyboard::Entry::new("Message").private().limit(model::discord_dms::MESSAGE_LIMIT);
            let text = keyboard::TextEntry::open(entry, write.text).captioned(write.caption);
            self.writing = Some((text, write.channel));
            self.guide.set_writing(true);
        }
    }

    /// A press while text entry is up over a conversation: the keyboard has every button. What it types is the
    /// draft at once; Menu or Enter sends it; B leaves it as typed.
    fn write_press(&mut self, press: Press) {
        let around = around_of!(self);
        let Some((entry, channel)) = &mut self.writing else { return };
        let channel = channel.clone();
        let action = pages::Page::press(entry, press, around);
        self.write_heard(&channel, action);
    }

    fn write_heard(&mut self, channel: &model::discord_dms::Id, action: Option<PageAction>) {
        let Some((entry, _)) = &mut self.writing else { return };
        let mut says = Vec::new();
        while let Some(event) = pages::Page::says(entry) {
            says.push(event);
        }
        // What is typed so far, before anything it says is acted on.
        let typed = entry.text();
        for event in says {
            match event {
                keyboard::Event::Typed(text) => self.guide.set_draft(channel, &text),
                keyboard::Event::Kept(text) => {
                    self.guide.set_draft(channel, &text);
                    self.guide.send_draft(channel);
                }
                // B: the draft is kept as typed, not put back (decision 6).
                keyboard::Event::Dropped(_) => self.guide.set_draft(channel, &typed),
            }
        }
        if action.is_some() {
            self.writing = None;
            self.guide.set_writing(false);
        }
        self.take_direct();
    }

    /// The Discord connection as the Dashboard was last told it.
    pub fn discord(&self) -> &model::discord::Discord {
        &self.discord
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

    /// The first pad's battery as the Status areas show it: what the shell's `Status` says, else what the pad says.
    fn pad_battery(&self) -> Option<Battery> {
        self.status.pad_battery.or(self.pad.battery)
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
        // New idle times count from now.
        if options.idle != self.idle.times() {
            self.idle.set_times(options.idle, self.now);
        }
        self.options = options;
        // A page whose focus log says how the executable's things stand follows them at once.
        self.tick_page();
        self.home.set_background_delay(options.background_delay);
        self.home.set_play_history(options.show_play_history, &self.library);
        self.home.set_recent_launcher(options.show_recent_launcher);
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
            discord: &self.discord,
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
        // The key that wakes the Idle screen types nothing.
        if self.idle.typed(self.now) {
            return true;
        }
        if self.writing.is_some() && self.guide.is_open() {
            let around = around_of!(self);
            let channel = self.writing.as_ref().map(|(_, channel)| channel.clone());
            let taken = match &mut self.writing {
                Some((entry, _)) => pages::Page::typed(entry, typed, around),
                None => false,
            };
            if let Some(channel) = channel {
                self.write_heard(&channel, None);
            }
            return taken;
        }
        if !self.page_has_pad() {
            return false;
        }
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
            discord: &self.discord,
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
        // While the Idle screen is up the first input only wakes it, and nothing of that press does more.
        let now = self.now;
        let swallowed = match event {
            PadEvent::Press(press) => self.idle_press(press),
            PadEvent::Release(Button::Guide) => self.idle.guide_release(),
            PadEvent::Release(button) => self.idle.release(button),
            PadEvent::GuideDown => self.idle.guide_down(now),
            PadEvent::GuideTap => self.idle.guide_tap(),
            PadEvent::GuideHeld => self.idle.guide_held(),
        };
        if swallowed {
            return;
        }
        match event {
            // Nothing yet: whether it is a tap or a hold is told later.
            PadEvent::GuideDown => {}
            PadEvent::Press(press) => {
                let was_open = self.guide.is_open();
                let page = self.page_has_pad().then(|| self.pages.depth());
                self.press(press);
                // The page that took the press is told the button is down, for what it draws while it is.
                if !press.repeat && page == Some(self.pages.depth()) {
                    self.pages.pad(press.button, true);
                }
                if let Some((entry, _)) = &mut self.writing {
                    if !press.repeat {
                        pages::Page::pad(entry, press.button, true);
                    }
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
                if let Some((entry, _)) = &mut self.writing {
                    pages::Page::pad(entry, button, false);
                }
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
    /// time the two have brought it to. Besides Toasts, the quit of a game is timed by it (docs/spec/guide.md, 7.4):
    /// "Force quit?" once the game has had `QUIT_TIMEOUT` to close, and the Guide closing `CLOSE_AFTER_QUIT` after a
    /// game quit from it has stopped.
    /// Whether anything came of that is `take_timed`'s to say.
    pub fn advance(&mut self, seconds: f32) -> bool {
        let before = (self.toasts.shown(), self.toasts.showing().is_some(), self.toasts.on_screen().is_some());
        if self.toasts.advance(seconds) {
            self.effects.push(Effect::Sound(self.toast_sound()));
        }
        self.pass(seconds);
        // Unlocks whose second is up are told of here, not as a frame is presented: their Toast is not on the
        // screen yet.
        if let Some(unlocks) = self.unlocks.take() {
            self.tell_unlocks(&unlocks);
        }
        before != (self.toasts.shown(), self.toasts.showing().is_some(), self.toasts.on_screen().is_some())
    }

    /// The time the quit of a game, Start-up and the Idle screen go by.
    fn pass(&mut self, seconds: f32) {
        // To the microsecond: steps a test adds up land where it says, not a hair short.
        self.now += Duration::from_micros((f64::from(seconds) * 1e6).round().max(0.0) as u64);
        if self.startup.as_mut().is_some_and(|startup| startup.advance(seconds)) {
            self.end_startup();
        }
        self.timed |= self.idle_tick();
        // Messages going out that are not seen sent become Not sent; the Direct Messages section's spinner comes
        // after a second.
        if self.guide.is_open() {
            self.guide.pass(seconds);
            self.timed = true;
        } else {
            self.guide.pass(seconds);
        }
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
        self.unlocks.pass(seconds);
    }

    /// Whether the time told since this was last asked changed what is shown: "Force quit?" came up, the Guide
    /// closed after a quit.
    pub fn take_timed(&mut self) -> bool {
        std::mem::take(&mut self.timed)
    }

    /// How soon `advance` has something to do, in seconds: None when nothing is being timed.
    pub fn timer(&self) -> Option<f32> {
        let quit = self.quit.filter(|quit| !quit.asked).map(|quit| quit.left);
        // The Now Playing card's time, while the Audio Flyout shows a track playing.
        let second = self.audio_shown().then(|| self.now_playing.next_second(self.now)).flatten();
        let second = second.map(|left| left.as_secs_f32());
        // Signing you in's three seconds.
        let signing_in = self.startup.as_ref().and_then(Startup::timer);
        let unlocks = self.unlocks.timer();
        let direct = self.guide.timer();
        [quit, self.close_in, second, signing_in, unlocks, direct]
            .into_iter()
            .flatten()
            .reduce(f32::min)
            .map(|left| left.max(0.0))
    }

    /// How soon the Idle screen has something to do, in seconds: dim, show things, turn a card, end a wake's grace.
    /// Apart from `timer`, which is what the screens time, as this is nearly always counting.
    pub fn idle_timer(&self) -> Option<f32> {
        self.idle.due(self.now).map(|left| left.as_secs_f32())
    }

    /// The top page is told of a press: Search follows a library that changed while its Game hub was over it.
    fn tick_page(&mut self) {
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
            discord: &self.discord,
        };
        self.pages.tick(around);
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
        if self.idle_press(press) {
            return;
        }
        if self.writing.is_some() && self.guide.is_open() {
            // Text entry takes every button but the Guide's, which closes the Guide (the draft is kept as typed).
            if press.button != Button::Guide {
                self.write_press(press);
                return;
            }
            self.writing = None;
            self.guide.set_writing(false);
        }
        let (was_open, guide) = (self.guide.is_open(), self.guide_place());
        // Focus moved if the same screen has the pad as before and says its focus is somewhere else. A page
        // opening or closing is no move: B is silent, and so is Y.
        let (depth, focus) = (self.pages.depth(), self.focus_place());
        let dialog = self.guide.dialog();
        let page_dialog = self.pages.dialog();
        let asked = self.effects.len();
        let activated = self.steer(press);
        self.take_direct();
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
        self.tick_page();
    }

    /// Does what the top page asked for.
    fn page_asks(&mut self, action: PageAction) {
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
            discord: &self.discord,
        };
        match action {
            PageAction::Close => self.close_page(),
            PageAction::Open(open) => {
                self.pages.open_over(open, around);
            }
            PageAction::Effect(Effect::Set(change)) => self.set(change),
            // Settings > Idle: in force at once, counting from now, and saved.
            PageAction::Effect(Effect::IdleTimes(times)) => {
                self.set_options(Options { idle: times, ..self.options });
                self.effects.push(Effect::IdleTimes(times));
            }
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
            // The Game Pass region: shown at once, saved, and the catalogue read again.
            PageAction::Effect(Effect::GamePassRegion(region)) => {
                self.options.game_pass_region = region;
                self.effects.push(Effect::GamePassRegion(region));
            }
            // Discord's Toasts while a game runs: in force at once, and saved.
            PageAction::Effect(Effect::DiscordWhilePlaying(choice)) => {
                self.options.while_playing = choice;
                self.effects.push(Effect::DiscordWhilePlaying(choice));
            }
            // The art sources' order: shown at once, and saved.
            PageAction::Effect(Effect::ArtOrder(order)) => {
                self.options.art_order = order;
                self.effects.push(Effect::ArtOrder(order));
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
                    discord: &self.discord,
                };
                if !self.pages.open_over(open, around) {
                    self.close_page();
                }
            }
        }
    }

    /// Where the Guide's focus is, and the volume: a press that changed any of it moved something.
    fn guide_place(&self) -> ((Tab, guide::Focus), Option<(Flyout, FlyoutFocus)>, u8, Option<String>) {
        (self.guide.focus(), self.guide.flyout(self.guide_around()), self.audio.volume, self.guide.direct_flyout_line())
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
    fn steer(&mut self, press: Press) -> bool {
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
                // A tap while a Queue Toast is up does what it says: Manage Queue, or the game started.
                if let Some(toast) = self.toasts.showing().cloned() {
                    if let Some(done) = self.do_what_toast_says(&toast) {
                        return done;
                    }
                }
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
                discord: &self.discord,
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
        // A held A is one A: it opens nothing again.
        if press.repeat && press.button == Button::A {
            return false;
        }
        match self.home.press(press.button, &self.library) {
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
                        discord: &self.discord,
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
                discord: &self.discord,
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

    /// A store's install asks the user to accept a licence agreement (#230): its prompt over whatever is up, which
    /// answers with `Effect::Agreement` and this `entry`'s number.
    pub fn ask_agreement(&mut self, entry: agreement::Entry) {
        let around = pages::Around {
            library: &self.library,
            playing: self.playing,
            options: self.options,
            now: self.now,
            now_playing: &self.now_playing,
            discord: &self.discord,
        };
        if self.pages.open_over(agreement::open(entry), around) {
            self.effects.push(Effect::Sound(Sound::PromptOpen));
        }
    }

    /// The licence agreements' prompts that are up, by their numbers: one asked and no longer up went with no
    /// answer (a page from the Guide took the screen, a game ended), which is a Decline.
    pub fn agreements_up(&self) -> Vec<u64> {
        self.pages.agreements()
    }

    /// An install stopped, or a game left the Queue without installing (#230): a Toast with the game's cover and
    /// why, in a few words. A tap opens Manage Queue, where its row says the rest.
    pub fn tell_stopped(&mut self, stopped: &model::Stopped) {
        let Some(found) = self.library.games.iter().find(|found| found.id == stopped.game) else { return };
        let art = Some(Box::new(found.clone()));
        let toast = Toast::new(ToastKind::Stopped, stopped.headline(), self.options.use_guide);
        self.toast(Toast { game: Some(stopped.game), art, ..toast });
    }

    /// A game its store would not start now (`Source::cant_start`): a Toast with its cover and what to do, in the
    /// store's own words, in place of the Launch screen.
    pub fn tell_cant_start(&mut self, game: i64, why: String) {
        let Some(found) = self.library.games.iter().find(|found| found.id == game) else { return };
        let art = Some(Box::new(found.clone()));
        let toast = Toast::new(ToastKind::Notification, why, self.options.use_guide);
        self.toast(Toast { game: Some(game), art, ..toast });
    }

    /// Whether Discord's sign-in page shows now (#197): its page has the pad and nothing is over it, not text entry,
    /// not the Guide. The executable shows the web view then, and puts it away otherwise.
    pub fn discord_sign_in_shown(&self) -> bool {
        self.page() == Some(discord_sign_in::NAME) && !self.guide.is_open() && !self.starting_up()
    }

    /// Whether Discord's sign-in page is up, shown or under text entry (#197): the session's web view is wanted.
    pub fn discord_sign_in_open(&self) -> bool {
        self.pages.has(discord_sign_in::NAME)
    }

    /// Discord's session has signed in: its sign-in page goes, if it has the pad, back to Discord messages.
    pub fn discord_signed_in(&mut self) {
        if self.page() == Some(discord_sign_in::NAME) {
            self.close_page();
            self.tick_page();
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

    /// The game, by its id in the library database, whose whole list of achievements an Achievements page that is
    /// up shows (docs/spec/achievements-page.md). Whoever has the database reads it and gives it with
    /// [`Dashboard::set_achievements`] as the page opens, and again as its icons come in or the library changes.
    pub fn achievements_wanted(&self) -> Option<i64> {
        self.pages.achievements_wanted()
    }

    /// A game's whole list of achievements, in its store's order, for the pages that show it.
    pub fn set_achievements(&mut self, game: i64, list: Vec<model::achievement::Achievement>) {
        self.pages.set_achievements(game, list);
    }

    /// What an art picker that is up wants fetched now (docs/spec/art-picker.md): pages of SteamGridDB's candidates
    /// and their thumbnails, each asked once. Whoever has the network fetches them off the UI thread and tells what
    /// came with [`Dashboard::art_answer`]. Empty with no picker up.
    pub fn art_asks(&mut self) -> Vec<model::art_picker::Ask> {
        self.pages.art_asks()
    }

    /// What was fetched for the art picker.
    pub fn art_answer(&mut self, answer: model::art_picker::Answer) {
        self.pages.art_answer(answer);
    }

    /// Whether an art picker is up.
    pub fn art_picker_up(&self) -> bool {
        self.pages.art_picker_up()
    }

    /// What is on top of Home's window, the Guide aside, and where its focus is: Home's focus as it is written
    /// (`Recent(2)`), or a page's name and what it says of its own (`Settings: Rail: General`). A press that
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
    /// A menu or a prompt has its title after it (`, dialog Menu { prompt: true, row: 0 } "Force quit Hadrel II?"`),
    /// and whatever the line is, it ends `, quitting` while the running game is being quit, and
    /// `, quitting, then Cerulea` when a game is to start after it.
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
            // What it shows: `, flyout Discord Item(0) "Servers" "Stand-in Server" of 3`.
            Some((Flyout::Discord, at)) => {
                format!(", flyout Discord {at:?} {}", self.guide.discord_flyout_line().unwrap_or_default())
            }
            Some((which, at)) => format!(", flyout {which:?} {at:?}"),
            None => String::new(),
        };
        // On the Discord tab, the entry focus is on and what Discord says (`Entry(4) Mute; in "General" with ...`).
        let discord = self.guide.discord_line().unwrap_or_default();
        // An open All Direct Messages or conversation (`, flyout Conversation ReplyBox; draft 12 chars`).
        let direct = self.guide.direct_flyout_line().unwrap_or_default();
        let tag = self.guide.dialog_tag().map(|tag| format!(" ({tag})")).unwrap_or_default();
        // On Profile & system, the entry by name (`Entry(6) Power`): its place moves with the achievements.
        let named = self.guide.profile_line().unwrap_or_default();
        match self.guide.dialog() {
            Some(dialog) => format!("{tab:?} {focus:?}{named}{panel}, dialog {dialog:?}{title}{tag}{quitting}"),
            None => format!("{tab:?} {focus:?}{named}{discord}{panel}{flyout}{direct}{quitting}"),
        }
    }

    /// `--log-focus`'s line, which tests on the VM judge by: where focus is (`Recent(2)`, `Settings: Rail`), and
    /// after it what the page says of its state beyond focus, if it says any (`TextEntry: A; "Ha|" lower letters`).
    /// The Game hub on top, if one is: its number, new each time a hub opens, and its game. The shell times a
    /// hub's opening by it (#214).
    pub fn game_hub_up(&self) -> Option<(u64, &model::Game)> {
        self.pages.top()?.game_hub(&self.library)
    }

    pub fn focus_line(&self) -> String {
        // Text entry over a conversation: its state says how long the message is, and nothing of what it says.
        if let (Some((entry, _)), true) = (&self.writing, self.guide.is_open()) {
            let state = pages::Page::state(entry);
            return format!("{}: {}; {state}", pages::Page::name(entry), pages::Page::focus(entry));
        }
        let place = self.focus_place();
        if self.startup.is_some() {
            return place;
        }
        match self.pages.top().map(|page| page.state()) {
            Some(state) if !state.is_empty() => format!("{place}; {state}"),
            // Settings > Discord: how the connection is, which the page has from the shell, not of its own.
            _ if place.starts_with("Settings: Discord: ") => format!("{place}; {}", self.discord.state_line()),
            // Settings > About: where updates stand, which the page has from the shell, not of its own, so the line
            // follows a check or an install at once rather than at the page's next draw.
            _ if place.starts_with("Settings: About: ") => format!("{place}; {}", self.options.updates.state()),
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
            GuideAction::SwitchProfile => self.switch_profile(),
            GuideAction::MyGames => self.guide_opens(Open::MyGames(my_games::Entry::Games)),
            GuideAction::Settings => self.guide_opens(Open::Settings(settings::Entry::Rail)),
            GuideAction::Search => self.guide_opens(Open::Search),
            // As Search: the Guide closes onto the page, in front of a running game too.
            GuideAction::ManageQueue => self.guide_opens(manage_queue::open()),
            GuideAction::Achievements(game) => self.open_achievements(game),
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
            // "Sign in to Discord in Settings > Account > Direct Messages": the Guide closes onto that page.
            GuideAction::DirectSettings => self.guide_opens(Open::Settings(settings::Entry::DirectMessages)),
            // Asked of Discord: the tab shows it done once Discord says so.
            GuideAction::Discord(ask) => {
                if let Some(ask) = ask.ask(&self.discord) {
                    self.effects.push(Effect::Discord(ask));
                }
            }
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

    /// A tap of the Guide button while a Queue Toast shows, as the Bundle's (docs/spec/guide.md, 9.6): "Queued"
    /// opens Manage Queue in front of whatever is up, those waiting of its kind let go; "Ready to play" starts its
    /// game. Either way the Toast goes, and the tap sounds as A does. None for any other Toast: the Guide opens.
    fn do_what_toast_says(&mut self, toast: &Toast) -> Option<bool> {
        match toast.kind {
            ToastKind::Queued | ToastKind::Stopped => {
                self.toasts.drop_waiting(toast.kind);
                self.toasts.send_away();
                self.open_page(Open::ManageQueue);
                self.show_dashboard();
                Some(true)
            }
            ToastKind::Installed => {
                let game = toast.game?;
                let place = self.library.games.iter().position(|found| found.id == game)?;
                self.toasts.send_away();
                self.play(place);
                Some(true)
            }
            _ => None,
        }
    }

    /// The Guide, just opened by a tap while a Toast of this kind was on screen, shows what the Toast was about
    /// (docs/spec/guide.md, 9.5). An Uninstalled Toast's is the Guide as it opens.
    fn show_what_toast_told(&mut self, toast: ToastKind) {
        match toast {
            ToastKind::Queued | ToastKind::Installed | ToastKind::Uninstalled | ToastKind::Stopped => {}
            ToastKind::Notification => self.open_notifications(),
            // An achievement's opens it where the Profile's totals and last unlocks are, Rare or not.
            ToastKind::Greeting | ToastKind::Achievement | ToastKind::RareAchievement => {
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
            self.toast(Toast::new(ToastKind::Notification, text, self.options.use_guide));
        }
        id
    }

    /// Every notification posted since the Dashboard started, oldest first: what the Notifications Flyout lists,
    /// newest first.
    pub fn notifications(&self) -> &[Notification] {
        self.toasts.notifications()
    }

    /// Greets the Profile signed in: "Hi <first name>!" in a Toast with its picture and no prompt, ahead of any
    /// Toast that waited for it.
    pub fn greet(&mut self) {
        self.greeting_owed = false;
        self.greeted = true;
        let profile = self.profile();
        let text = format!("Hi {}!", profile.first_name());
        let picture = Some(profile.picture);
        self.toast(Toast { picture, ..Toast::new(ToastKind::Greeting, text, false) });
        for toast in std::mem::take(&mut self.held_toasts) {
            self.toast(toast);
        }
    }

    /// Toasts have somewhere to show from now on (the executable has made the Toast strip, or knows there is none):
    /// the Profile signed in is greeted, if it has not been, once Start-up is over (docs/spec/start-up.md, 5).
    pub fn toasts_ready(&mut self) {
        self.toasts_ready = true;
        self.greet_if_due();
    }

    /// The greeting, if it is due: Toasts have somewhere to show, Start-up is over, and since it began, or since
    /// the Dashboard started with none, nobody has been greeted.
    fn greet_if_due(&mut self) {
        if self.toasts_ready && self.startup.is_none() && (self.greeting_owed || !self.greeted) {
            self.greet();
        }
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
    /// remember it as the one signed in last: what Signing you in does. A Profile the library has not got changes
    /// nothing.
    pub fn sign_in(&mut self, profile: i64) {
        if self.library.profiles.get(profile).is_none() {
            return;
        }
        self.library.profiles.signed_in = Some(profile);
        self.refresh_guide();
        self.effects.push(Effect::Profile(ProfileChange::SignIn(profile)));
    }

    /// Tells of a game in the Queue in a Toast with its cover (docs/spec/guide.md, 9.6), as the Bundle words them:
    /// `Queued` as the Dashboard asks for its install, "Ready to play" once it is installed, "<Name> was
    /// uninstalled" once it is gone. Nothing for a game the library does not have, or for any other kind.
    pub fn tell(&mut self, kind: ToastKind, game: i64) {
        let Some(found) = self.library.games.iter().find(|found| found.id == game) else { return };
        let text = match kind {
            ToastKind::Queued => "Queued".to_string(),
            ToastKind::Installed => "Ready to play".to_string(),
            ToastKind::Uninstalled => format!("{} was uninstalled", found.name),
            ToastKind::Notification
            | ToastKind::Greeting
            | ToastKind::Achievement
            | ToastKind::RareAchievement
            | ToastKind::Stopped => return,
        };
        let art = Some(Box::new(found.clone()));
        self.toast(Toast { game: Some(game), art, ..Toast::new(kind, text, self.options.use_guide) });
    }

    fn toast(&mut self, toast: Toast) {
        // During Start-up, and until its greeting has shown, a Toast waits (the Bundle holds them back so).
        if self.startup.is_some() || self.greeting_owed {
            self.held_toasts.push(toast);
            return;
        }
        if self.toasts.post(toast) {
            self.effects.push(Effect::Sound(self.toast_sound()));
        }
    }

    /// The sound of the Toast that has just begun to show: a Rare achievement's own, else the notification's.
    fn toast_sound(&self) -> Sound {
        match self.toasts.showing().map(|toast| toast.kind) {
            Some(ToastKind::RareAchievement) => Sound::RareAchievement,
            _ => Sound::Notification,
        }
    }

    /// Achievements newly unlocked in the game watched (docs/achievements.md), found together: told of in one
    /// Achievement unlocked Toast a second after the first, with any others found by then (docs/spec/
    /// achievement-toast.md). The totals and the last unlocks come with the library's next snapshot.
    pub fn unlocked(&mut self, unlocks: Vec<model::achievement::Unlock>) {
        self.unlocks.add(unlocks);
    }

    /// The Toast for unlocks found together, waiting its turn as a notification's does, and its notification in
    /// the list, which A on in the Notifications Flyout opens the game's Achievements page from. It shows whether
    /// or not the Notifications Flyout is open, as the Bundle's does.
    fn tell_unlocks(&mut self, unlocks: &[model::achievement::Unlock]) {
        if unlocks.is_empty() {
            return;
        }
        let toast = toasts::achievement::toast(unlocks, self.options.use_guide);
        let text = toasts::achievement::notification(&toast, &self.library);
        self.toasts.record_of(&text, toast.game);
        self.refresh_guide();
        self.toast(toast);
    }

    /// A on an Achievement unlocked notification in the Guide: the Guide closes onto the game's Achievements page,
    /// over Home (B goes back to Home), in front of a running game as the Guide's pages are. A game with no
    /// achievements now has its Game hub instead; one the library has lost, nothing.
    fn open_achievements(&mut self, game: i64) {
        let library = &self.library;
        let Some(found) = library.games.iter().chain(&library.hidden).find(|found| found.id == game) else { return };
        let page = achievements::Entry { game: found.key(), id: found.id, behind: game_hub::serial() };
        let hub = game_hub::Entry::of(found);
        let has = library.achievements.of(found.id).is_some();
        self.guide_opens(if has { Open::Achievements(page) } else { Open::GameHub(hub) });
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
            self.writing = None;
            self.take_direct();
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
        } else if guide || self.launch.showing() || (self.has_overlay && self.idle.shown()) {
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

    /// Whether Home has nothing to show: a game is running in front of it, or its Launch screen has faded in over
    /// it. Home's window then shows the game's backdrop instead, and is drawn only when that changes.
    pub fn home_covered(&self) -> bool {
        self.playing.is_some() && !self.in_front && (!self.launch.up() || self.launch.covers())
    }

    /// Whether Home's window shows the backdrop, whole or fading to Home: false once Home is all there is.
    pub fn backdrop_showing(&self) -> bool {
        self.backdrop.showing() || self.home_covered()
    }

    /// Whether the backdrop is fading to Home, the game having ended.
    pub fn backdrop_fading(&self) -> bool {
        self.backdrop.fading()
    }

    /// Whether the backdrop was whole when last drawn: the game's art was in, or it has none to wait for.
    pub fn backdrop_whole(&self) -> bool {
        self.backdrop.whole()
    }

    /// The game whose backdrop covers Home's window now.
    fn backdrop_game(&self) -> Option<&model::Game> {
        self.playing.filter(|_| self.home_covered()).and_then(|game| self.library.games.get(game))
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
        // What showed where the game was, its backdrop, fades to Home (docs/spec/launch-screen.md).
        self.backdrop.fade();
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
        // The pad's own word on its battery, unless the shell has said otherwise. One on a cable shows its glyph.
        let battery = self.pad_battery();
        let show_percent = self.status.show_percent && self.options.show_battery_percent;
        // The badge counts the notifications the Dashboard itself holds, unless the shell has told of more.
        let notifications = self.status.notifications.max(self.toasts.notifications().len() as u32);
        let status = Status {
            pad_battery: battery,
            show_percent,
            notifications,
            discord_call: self.discord.voice.mark(),
            ..self.status.clone()
        };
        let profile = self.profile();
        let face = Face { profile: &profile, clock: &clock, pad: self.pad.connected, status: &status };
        self.guide.draw(ui, around, &face);
        if self.guide.is_open() {
            let around = around_of!(self);
            if let Some((entry, _)) = &mut self.writing {
                ui.layer();
                pages::Page::draw(entry, ui, around);
            }
        }
    }

    /// Home's window: Home or the page over it, and the hint bar; the Guide over them where there is no Overlay to
    /// draw it on; and the Toast, where Toasts have no window of their own. While a game is up, its backdrop in Home's
    /// place, fading to Home when the game has ended.
    pub fn draw(&mut self, ui: &mut Ui) {
        if self.draw_startup(ui) {
            return;
        }
        let covering = self.backdrop_game().cloned();
        if covering.is_none() {
            self.draw_home(ui);
        }
        self.backdrop.draw(ui, covering.as_ref(), self.slow_renderer);
        self.draw_leaving(ui);
        if !self.toast_strip {
            self.draw_toasts(ui);
        }
    }

    fn draw_home(&mut self, ui: &mut Ui) {
        ui.store.reduce_motion = self.options.reduce_motion;
        let clock = self.clock();
        // The pad's battery, as the Guide's Status area has it; one on a cable shows its glyph.
        let pad_battery = self.pad_battery();
        let profile = self.profile();
        let status = home::Status {
            profile: &profile,
            clock: &clock,
            pad: self.pad.connected,
            pad_battery,
            show_percent: self.status.show_percent && self.options.show_battery_percent,
            score: self.library.achievements.totals.map(|totals| totals.score),
            discord_call: self.discord.voice.mark(),
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
            discord: &self.discord,
        };
        let (home, library) = (&mut self.home, &self.library);
        self.pages.draw(ui, around, &mut |ui| home.draw(ui, library, &status));
        // The hint bar, once for whatever has the pad, over all of it.
        hint_bar::draw(ui, self.hints());
        if !self.guide_overlaid {
            self.draw_guide(ui);
        }
        // With no Overlay the Idle screen dims Home's window, over everything in it but the Toast.
        if !self.has_overlay {
            self.draw_idle(ui);
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
        // Over all of it: the Launch screen, the Guide, its dialogs; under the Toasts' strip.
        if self.has_overlay {
            self.draw_idle(ui);
        }
    }

    /// All in one frame, the Overlay's picture over Home's window's and the Toast over both: what an offscreen
    /// frame shows.
    pub fn draw_all(&mut self, ui: &mut Ui) {
        if self.draw_startup(ui) {
            return;
        }
        self.draw_home(ui);
        self.draw_leaving(ui);
        self.draw_overlay(ui);
        self.draw_toasts(ui);
    }

    // --- Start-up (docs/spec/start-up.md) ---

    /// Start-up begins, as the Dashboard starts: with the Boot video if one is to play (`video`: it was chosen and
    /// found, and the executable has it opening), else at once with what follows it, Who's playing today? or
    /// Signing you in. Over everything until it ends, with the pad and the Guide button its own. The executable
    /// does not call it with `--no-startup`, and the Dashboard then starts at Home.
    pub fn start_up(&mut self, video: bool) {
        self.greeting_owed = true;
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

    /// Where Start-up is, while it shows.
    pub fn startup_phase(&self) -> Option<Phase> {
        self.startup.as_ref().map(Startup::phase)
    }

    /// The Profile Start-up shows, while it shows one: the one focused on Who's playing today?, or signing in.
    pub fn startup_profile(&self) -> Option<&Profile> {
        self.startup.as_ref().and_then(Startup::profile)
    }

    /// How many Profiles Who's playing today? shows, while it shows.
    pub fn startup_profiles(&self) -> usize {
        self.startup.as_ref().map_or(0, |startup| startup.profiles().len())
    }

    /// Whether the Boot video is what shows: the executable plays it while this says so, and stops it when it
    /// stops saying so (a press skipped it).
    pub fn boot_video_playing(&self) -> bool {
        self.startup_phase() == Some(Phase::BootVideo)
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

    /// The Guide's Switch profile: the Guide goes at once and Who's playing today? asks, without the Boot video,
    /// whatever Settings says of asking (the Bundle's `SwitchProfile`).
    fn switch_profile(&mut self) {
        self.ask_whos_playing();
    }

    /// Windows woke from sleep: Who's playing today? asks again, without the Boot video, if Settings has it ask
    /// (the Bundle's `OnResumedFromSleep`); over a running game the Dashboard comes in front of it first. Nothing
    /// while Start-up already shows. Waking the Idle screen is not waking from sleep, and never asks. True when it
    /// asked.
    pub fn woke_from_sleep(&mut self) -> bool {
        if !self.options.ask_whos_playing || self.startup.is_some() {
            return false;
        }
        self.show_dashboard();
        self.ask_whos_playing();
        true
    }

    /// Who's playing today? over everything, asked afresh: the Guide goes at once, with a dialog page, and the
    /// Toast on screen; Toasts wait until the greeting has shown (the Bundle's `CoverForStartup`).
    fn ask_whos_playing(&mut self) {
        self.pages.close_dialog();
        if self.guide.is_open() {
            self.effects.push(Effect::GuideClosed);
        }
        self.guide.close_at_once();
        self.close_in = None;
        self.toasts.drop_all();
        self.leaving = None;
        self.greeting_owed = true;
        let (profiles, last) = self.whos_playing_profiles();
        self.startup = Some(Startup::after_video(profiles, last, true));
        // Woken from sleep, the Idle screen may be up: it goes at once, and waits until Start-up is over.
        self.idle.wait(true, self.now);
    }

    /// What comes after the Boot video, or at once with it Off: Who's playing today?, or with Settings having it
    /// not ask, Signing you in for the Profile signed in last.
    fn after_boot_video(&mut self) {
        let (profiles, last) = self.whos_playing_profiles();
        let ask = self.options.ask_whos_playing;
        match &mut self.startup {
            Some(startup) => startup.go_on(profiles, last, ask),
            None => self.startup = Some(Startup::after_video(profiles, last, ask)),
        }
        let signing_in = self.startup.as_ref().filter(|s| s.phase() == Phase::SigningIn).and_then(Startup::profile);
        if let Some(id) = signing_in.map(|profile| profile.id) {
            self.signing_in(id);
        }
    }

    /// Who's playing today?'s Profiles, and which of them was signed in last: the library's, or where it has none
    /// (a library from no database) the Profile shown, alone.
    fn whos_playing_profiles(&self) -> (Vec<Profile>, usize) {
        let profiles = &self.library.profiles;
        if profiles.list.is_empty() {
            return (vec![self.profile()], 0);
        }
        let last = profiles.signed_in.and_then(|id| profiles.list.iter().position(|p| p.id == id)).unwrap_or(0);
        (profiles.list.clone(), last)
    }

    /// Signing you in shows this Profile: it is signed in at once, everywhere, and remembered, with the console's
    /// sign-in sound (the Bundle's `StartupSequence.SignIn` and `OnStartupSignedIn`).
    fn signing_in(&mut self, profile: i64) {
        self.effects.push(Effect::Sound(Sound::Login));
        self.sign_in(profile);
    }

    /// Signing you in has shown its time: Start-up is over. Home shows, the swirl lifting off it, and the Profile is
    /// greeted, or will be as soon as Toasts have somewhere to show.
    fn end_startup(&mut self) {
        self.leaving = self.startup.take();
        // The Idle screen counts from zero from here (unless a Launch screen still holds it off).
        let now = self.now;
        self.idle.wait(self.launch.up(), now);
        self.timed = true;
        self.greet_if_due();
    }

    /// A press while Start-up shows. Any button skips the Boot video, once its picture has been up a moment, in
    /// silence, as in the Bundle. On Who's playing today? Left and Right move, with the Guide's move sound, and A
    /// signs the focused Profile in, with the Guide's select sound, then the sign-in sound (the Bundle's
    /// `Navigate`, `Enter` and `Login`). Nothing else is told of it.
    fn startup_press(&mut self, press: Press) {
        let Some(startup) = &mut self.startup else { return };
        if startup.phase() == Phase::BootVideo {
            if startup.takes_presses() && !press.repeat {
                self.after_boot_video();
            }
            return;
        }
        match startup.press(press) {
            startup::Answer::Nothing => {}
            startup::Answer::Moved => self.effects.push(Effect::Sound(Sound::GuideMove)),
            startup::Answer::Chose(profile) => {
                self.effects.push(Effect::Sound(Sound::GuideSelect));
                self.signing_in(profile.id);
            }
        }
    }

    /// The pad while Start-up shows: a button going down is a press, the Guide button's release or hold too. What
    /// is still to come of a Guide button press Start-up took (its tap, its release after a hold) does nothing
    /// once Home shows: the Guide does not open behind Start-up.
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
            PadEvent::GuideDown => {}
            // A on Switch profile closed the Guide: the foreground comes back as it is let go.
            PadEvent::Release(button) => {
                if self.release_awaited == Some(button) {
                    self.release_awaited = None;
                }
            }
        }
    }

    /// Start-up's picture, if it shows. Home is drawn under it once, unseen, as the video's picture first shows,
    /// so its art is decoded behind the video.
    fn draw_startup(&mut self, ui: &mut Ui) -> bool {
        let Some(warm) = self.startup.as_mut().map(Startup::warm_home) else { return false };
        ui.store.reduce_motion = self.options.reduce_motion;
        if warm {
            ui.with_alpha(0.0, |ui| self.draw_home(ui));
        }
        if let Some(startup) = &mut self.startup {
            startup.draw(ui);
        }
        true
    }

    // --- The Idle screen (docs/spec/idle-screen.md) ---

    /// Where the Idle screen is.
    pub fn idle_stage(&self) -> idle::Stage {
        self.idle.stage()
    }

    /// The card the Idle screen's panel shows, while it is Showing things.
    pub fn idle_card(&self) -> Option<&idle::Card> {
        (self.idle.stage() == idle::Stage::ShowingThings).then(|| self.idle.card().map(|(_, card)| card)).flatten()
    }

    /// The Idle screen's times as they are in force: Settings', or `--idle-after`'s.
    pub fn idle_times(&self) -> idle::Times {
        self.idle.times()
    }

    /// What the Idle screen did since this was last asked, for the log (`idle: dimmed`, `idle: woken by A`).
    pub fn take_idle_news(&mut self) -> Vec<idle::News> {
        self.idle.take_news()
    }

    /// The shell's clock saw input anywhere in Windows at `at`: the keyboard, the mouse, a pad. Told each turn,
    /// before `advance` and before the presses the input made, so that one of those is the wake press.
    pub fn input_seen(&mut self, at: Instant) {
        self.idle.input_at(at, self.now);
    }

    /// The Idle screen's cards are shuffled from here: the window's, from the time it starts.
    pub fn seed_idle(&mut self, seed: u64) {
        self.idle.seed(seed);
    }

    /// The date the Idle screen's cards count from ("Played yesterday"), in place of the machine's: a test's.
    pub fn set_date(&mut self, date: Option<chrono::NaiveDate>) {
        self.date = date;
    }

    /// The Idle screen at the Dashboard's clock: it waits while Start-up or a Launch screen shows, else moves on if
    /// its time has come. True when what it shows changed.
    fn idle_tick(&mut self) -> bool {
        let now = self.now;
        self.idle.wait(self.startup.is_some() || self.launch.up(), now);
        let (library, use_guide) = (&self.library, self.options.use_guide);
        let today = self.date.unwrap_or_else(idle::cards::today);
        self.idle.tick(now, || idle::cards::pool(library, use_guide, today))
    }

    /// A press as the Idle screen has it: true when it was the wake press, or part of it, and does nothing more. Y's
    /// wake press on a game's card opens the game's Game hub, unless a game is running (the Bundle's "See game").
    fn idle_press(&mut self, press: Press) -> bool {
        match self.idle.press(press, self.now) {
            idle::Pressed::Pass => false,
            idle::Pressed::Swallowed => true,
            idle::Pressed::Woke(card) => {
                let game = card.and_then(|card| card.game).filter(|_| press.button == Button::Y);
                let game = game.and_then(|key| self.library.games.iter().find(|game| game.is(&key)));
                if let Some(entry) = game.map(game_hub::Entry::of).filter(|_| self.playing.is_none()) {
                    self.close_guide();
                    self.close_pages();
                    self.open_page(Open::GameHub(entry));
                }
                self.timed = true;
                true
            }
        }
    }

    /// The dim and the panel, over whatever is drawn already.
    fn draw_idle(&mut self, ui: &mut Ui) {
        let clock = self.clock();
        idle::draw(ui, &mut self.idle, &self.library, &clock);
    }

    /// The swirl lifting off Home as Start-up ends, over everything Home's window shows, until it has gone.
    fn draw_leaving(&mut self, ui: &mut Ui) {
        let Some(leaving) = &mut self.leaving else { return };
        let alpha = ui.tween_from("startup leaving", 1.0, 0.0, startup::FADE_OUT, ui::Ease::Linear);
        if alpha <= 0.0 {
            self.leaving = None;
            return;
        }
        ui.layer();
        leaving.draw_leaving(ui, alpha);
    }
}

/// Fourteen games with no art, so Home always has something to show: ten played, most recently first, then four
/// never played; some not installed. In no database, so with no saved filters: whoever shows it gives it them.
/// Discord not connected, never set up: for a page drawn or pressed outside the Dashboard (a test's).
pub fn no_discord() -> &'static model::discord::Discord {
    static NONE: std::sync::OnceLock<model::discord::Discord> = std::sync::OnceLock::new();
    NONE.get_or_init(model::discord::Discord::default)
}

pub fn fake_library() -> Library {
    // Name, minutes played, installed.
    let games = [
        ("Halyard Point", 600, true),
        ("Fernway Rally 5", 1250, true),
        ("Seal of Thorns", 90, true),
        ("Starvault", 2400, true),
        ("Hadrel II", 310, true),
        ("Barlin's Gate 3", 45, false),
        ("Hearthwick: Silverthorn", 780, true),
        ("Meridian Gliding Simulator", 1500, true),
        ("Coldspire 2079", 20, false),
        ("Starbeck Valley", 3600, true),
        ("Aeon of Embers IV", 0, true),
        ("Cerulea", 0, false),
        ("Oda and the Wake of the Moths", 0, true),
        ("Pinwheelers 2", 0, false),
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
