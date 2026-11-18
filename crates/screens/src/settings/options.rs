//! The options Settings holds, as the Dashboard has them: typed, each with its default (docs/spec/settings.md,
//! section 7). They are kept in the library database's `settings` table, a key and a value each, which the core
//! never sees: whoever owns the database reads them into an [`Options`] at start-up ([`Options::read`]) and writes
//! each [`Change`] the Dashboard asks for (`Effect::Set`). An option is in the table only once it has been set, so
//! one that was never touched follows its default, whatever that comes to be.

pub use crate::startup::{BootVideo, Found};

/// An option that is on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Setting {
    /// Home's background waits a second and a half on a game before showing its art; off, 150 ms.
    BackgroundDelay,
    /// Pages, rows and tiles are shown in place, and the background holds still.
    ReduceMotion,
    /// The Recent row ends with the Play history tile; off, with one game more.
    ShowPlayHistory,
    /// The Recent row's tiles carry their launcher's mark at their foot; off, none of them does.
    ShowRecentLauncher,
    /// The figure beside the pad's battery and the PC's.
    ShowBatteryPercent,
    /// The Guide opens with the Guide button and from Home; off, the button is left to Steam or another program.
    UseGuide,
    /// The running game is paused (its processes suspended) while the Guide is open over it; off, it runs on under
    /// the Guide (Settings > General > Guide, #270; docs/spec/guide.md 7.2).
    PauseGame,
    /// Art is downloaded for the games that have none on the PC: the stores' own online, and SteamGridDB's with a
    /// key (Settings > Art). Off, nothing is asked of any service.
    DownloadArtOnline,
    /// Who's playing today? asks on every start and on waking from sleep; off, the Profile signed in last signs in
    /// by itself (Settings > Account > Who's playing today?, docs/spec/start-up.md).
    AskWhosPlaying,
    /// Settings > About's Check for updates every day (docs/spec/updates.md): off by default, while the
    /// repository's releases are not public.
    DailyUpdateCheck,
    /// Settings > Launchers' Show Game Pass games (#277): the PC Game Pass catalogue's games are listed with Xbox's.
    ShowGamePass,
}

impl Setting {
    pub const ALL: [Setting; 11] = [
        Setting::BackgroundDelay,
        Setting::ReduceMotion,
        Setting::ShowPlayHistory,
        Setting::ShowRecentLauncher,
        Setting::ShowBatteryPercent,
        Setting::UseGuide,
        Setting::PauseGame,
        Setting::DownloadArtOnline,
        Setting::AskWhosPlaying,
        Setting::DailyUpdateCheck,
        Setting::ShowGamePass,
    ];

    /// The option's key in the settings table. Never changed: a database has it.
    pub fn key(self) -> &'static str {
        match self {
            Setting::BackgroundDelay => "home.background_delay",
            Setting::ReduceMotion => "motion.reduce",
            Setting::ShowPlayHistory => "home.play_history",
            Setting::ShowRecentLauncher => "home.recent_launcher",
            Setting::ShowBatteryPercent => "status.battery_percent",
            Setting::UseGuide => "guide.enabled",
            Setting::PauseGame => "guide.pause_game",
            // `library::art::ONLINE_SETTING`.
            Setting::DownloadArtOnline => "art.online",
            Setting::AskWhosPlaying => "startup.ask_whos_playing",
            Setting::DailyUpdateCheck => "updates.daily_check",
            Setting::ShowGamePass => model::game_pass::SHOWN_KEY,
        }
    }

    /// What the option is until it is set.
    pub fn default(self) -> bool {
        !matches!(self, Setting::ReduceMotion | Setting::DailyUpdateCheck)
    }
}

/// An option set to a value: what a checkbox asks for as it is turned over. The Dashboard has taken it already;
/// it is for whoever keeps the settings to save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    pub setting: Setting,
    pub on: bool,
}

impl Change {
    pub fn key(self) -> &'static str {
        self.setting.key()
    }

    /// The value as the settings table holds it.
    pub fn value(self) -> &'static str {
        if self.on {
            "true"
        } else {
            "false"
        }
    }
}

/// The settings row that holds the Boot video chosen (Settings > General > Boot video): a
/// [`BootVideo::key`], `Off` among them. None until one is chosen, which is the Bundle's Xbox Series X.
pub const BOOT_VIDEO: &str = "startup.boot_video";

/// The settings row that holds the user's SteamGridDB key (`library::art::KEY_SETTING`). The core is told only
/// whether there is one: what it is never comes back from the database to the screens.
pub const ART_KEY: &str = "art.steamgriddb_key";

/// The settings row that holds when updates were last checked, by hand or of the Dashboard's own accord: seconds
/// since 1970. The executable's alone: the daily check is due a day after it (docs/spec/updates.md).
pub const LAST_UPDATE_CHECK: &str = "updates.last_check";

/// Every option's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub background_delay: bool,
    pub reduce_motion: bool,
    pub show_play_history: bool,
    /// The launcher's mark on the Recent row's tiles: on by default (#248).
    pub show_recent_launcher: bool,
    pub show_battery_percent: bool,
    pub use_guide: bool,
    /// The running game is paused while the Guide is open over it: on by default, as the Bundle has it (#270).
    pub pause_game: bool,
    /// Art is downloaded for the games that have none on the PC (Settings > Art): on by default.
    pub art_online: bool,
    /// A SteamGridDB key has been entered (Settings > Art): with art online on, SteamGridDB is asked too.
    pub art_key: bool,
    /// The online art sources' order, and which are on (Settings > Art's sources): kept as the row
    /// `model::art_order::KEY`. Playnite's order, all on, until it is set.
    pub art_order: model::ArtOrder,
    /// Who's playing today? asks on every start and on waking from sleep: on by default, as the Bundle's.
    pub ask_whos_playing: bool,
    /// Settings > About checks for updates once a day: off by default.
    pub daily_update_check: bool,
    /// The PC Game Pass catalogue's games are listed (Settings > Launchers, #277): on by default.
    pub show_game_pass: bool,
    /// The region whose PC Game Pass catalogue is listed, as chosen (`model::game_pass::REGION_KEY`); None is
    /// Windows' own.
    pub game_pass_region: Option<model::game_pass::Region>,
    /// Windows' own region, as the executable read it as it started: not an option, but what "Windows' region"
    /// stands for in the list. None where it could not be read.
    pub windows_region: Option<model::game_pass::Region>,
    /// Whether the account has PC Game Pass, as the executable judged it as it started (#173 follow-up): not an
    /// option, but Settings > Launchers' status line.
    pub game_pass_subscription: model::game_pass::Subscription,
    /// The Boot video chosen (docs/spec/start-up.md), which plays as the Dashboard starts if it is found.
    pub boot_video: BootVideo,
    /// Which boot videos the media folder has: not an option, but what Settings' Boot video lists. The executable
    /// looks as it starts (`Dashboard::set_boot_videos`).
    pub boot_videos: Found,
    /// Settings > Idle's two times (docs/spec/idle-screen.md): kept as the rows `idle::DIM_KEY` and `idle::SHOW_KEY`.
    pub idle: crate::idle::Times,
    /// Shell mode as the executable last found it (Settings > System, docs/spec/shell-mode.md): not an option kept
    /// in the settings table, but the PC's own, which the executable reads and changes.
    pub shell: model::shell_mode::ShellMode,
    /// Which build this is (Settings > About): not an option, but the executable's own, which it says as it starts.
    pub build: super::about::Build,
    /// Where updates stand (Settings > About): not an option, but what the executable last said of its checks.
    pub updates: model::update::Updates,
}

impl Default for Options {
    fn default() -> Self {
        let of = Setting::default;
        Self {
            background_delay: of(Setting::BackgroundDelay),
            reduce_motion: of(Setting::ReduceMotion),
            show_play_history: of(Setting::ShowPlayHistory),
            show_recent_launcher: of(Setting::ShowRecentLauncher),
            show_battery_percent: of(Setting::ShowBatteryPercent),
            use_guide: of(Setting::UseGuide),
            pause_game: of(Setting::PauseGame),
            art_online: of(Setting::DownloadArtOnline),
            art_key: false,
            art_order: model::ArtOrder::default(),
            ask_whos_playing: of(Setting::AskWhosPlaying),
            daily_update_check: of(Setting::DailyUpdateCheck),
            show_game_pass: of(Setting::ShowGamePass),
            game_pass_region: None,
            windows_region: None,
            game_pass_subscription: model::game_pass::Subscription::Unknown,
            boot_video: BootVideo::DEFAULT,
            boot_videos: Found::NONE,
            idle: crate::idle::Times::default(),
            shell: model::shell_mode::ShellMode::Unknown,
            build: super::about::Build::default(),
            updates: model::update::Updates::Unchecked,
        }
    }
}

impl Options {
    /// The options as a settings table has them: `stored` gives a key's value, or none for one never set. A value
    /// that says neither on nor off leaves the default.
    pub fn read(stored: impl Fn(&str) -> Option<String>) -> Self {
        let mut options = Self::default();
        for setting in Setting::ALL {
            if let Some(on) = stored(setting.key()).as_deref().and_then(flag) {
                options.set(setting, on);
            }
        }
        options.art_key = stored(ART_KEY).is_some_and(|key| !key.trim().is_empty());
        options.art_order = model::ArtOrder::read(stored(model::art_order::KEY).as_deref());
        if let Some(video) = stored(BOOT_VIDEO).as_deref().and_then(BootVideo::from_key) {
            options.boot_video = video;
        }
        options.idle = crate::idle::Times::read(&stored);
        options.game_pass_region =
            stored(model::game_pass::REGION_KEY).as_deref().and_then(model::game_pass::Region::parse);
        options
    }

    pub fn get(&self, setting: Setting) -> bool {
        match setting {
            Setting::BackgroundDelay => self.background_delay,
            Setting::ReduceMotion => self.reduce_motion,
            Setting::ShowPlayHistory => self.show_play_history,
            Setting::ShowRecentLauncher => self.show_recent_launcher,
            Setting::ShowBatteryPercent => self.show_battery_percent,
            Setting::UseGuide => self.use_guide,
            Setting::PauseGame => self.pause_game,
            Setting::DownloadArtOnline => self.art_online,
            Setting::AskWhosPlaying => self.ask_whos_playing,
            Setting::DailyUpdateCheck => self.daily_update_check,
            Setting::ShowGamePass => self.show_game_pass,
        }
    }

    pub fn set(&mut self, setting: Setting, on: bool) {
        *match setting {
            Setting::BackgroundDelay => &mut self.background_delay,
            Setting::ReduceMotion => &mut self.reduce_motion,
            Setting::ShowPlayHistory => &mut self.show_play_history,
            Setting::ShowRecentLauncher => &mut self.show_recent_launcher,
            Setting::ShowBatteryPercent => &mut self.show_battery_percent,
            Setting::UseGuide => &mut self.use_guide,
            Setting::PauseGame => &mut self.pause_game,
            Setting::DownloadArtOnline => &mut self.art_online,
            Setting::AskWhosPlaying => &mut self.ask_whos_playing,
            Setting::DailyUpdateCheck => &mut self.daily_update_check,
            Setting::ShowGamePass => &mut self.show_game_pass,
        } = on;
    }
}

/// On or off, as the settings table holds it, written by this program or by hand.
fn flag(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => Some(true),
        "false" | "0" | "off" | "no" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_bundles() {
        let options = Options::default();
        assert!(options.background_delay && options.show_play_history && options.show_battery_percent);
        // The port's own (#248): the Recent row's launcher marks show until they are turned off.
        assert!(options.show_recent_launcher);
        assert!(options.use_guide && !options.reduce_motion && options.ask_whos_playing);
        // The game is paused under the Guide until that is turned off (#270).
        assert!(options.pause_game);
        // The port's own: art online is on until it is turned off.
        assert!(options.art_online && !options.art_key);
        assert_eq!(options.art_order, model::ArtOrder::default(), "Steam, Xbox, PlayStation, SteamGridDB, all on");
        // The port's own: no update check of its own accord until it is turned on.
        assert!(!options.daily_update_check);
        // The port's own (#277): the Game Pass catalogue's games listed, for Windows' own region.
        assert!(options.show_game_pass && options.game_pass_region.is_none());
    }

    #[test]
    fn the_game_pass_region_is_a_market_code_and_none_is_windows() {
        let read = |value: &'static str| {
            Options::read(move |key| (key == model::game_pass::REGION_KEY).then(|| value.to_string())).game_pass_region
        };
        assert_eq!(read("gb").map(|r| r.to_string()), Some("GB".into()));
        assert_eq!(read("Great Britain"), None);
    }

    #[test]
    fn an_option_is_its_default_until_the_table_says_on_or_off() {
        let stored = |key: &str| match key {
            "home.background_delay" => Some("false".to_string()),
            "motion.reduce" => Some(" On ".to_string()),
            "home.play_history" => Some("perhaps".to_string()),
            "home.recent_launcher" => Some("false".to_string()),
            _ => None,
        };
        let options = Options::read(stored);
        let expected =
            Options { background_delay: false, reduce_motion: true, show_recent_launcher: false, ..Options::default() };
        assert_eq!(options, expected);
        assert_eq!(options.boot_video, BootVideo::XboxSeriesX, "the Bundle's, until one is chosen");
    }

    #[test]
    fn the_boot_video_chosen_is_read_and_one_the_table_garbles_is_the_default() {
        let read = |value: &str| {
            let value = value.to_string();
            Options::read(move |key| (key == BOOT_VIDEO).then(|| value.clone())).boot_video
        };
        assert_eq!(read("Off"), BootVideo::Off);
        assert_eq!(read("XboxOne"), BootVideo::XboxOne);
        assert_eq!(read("the best one"), BootVideo::XboxSeriesX);
    }

    #[test]
    fn a_change_is_read_back_as_it_was_written() {
        for setting in Setting::ALL {
            for on in [true, false] {
                let change = Change { setting, on };
                let read = Options::read(|key| (key == change.key()).then(|| change.value().to_string()));
                assert_eq!(read.get(setting), on, "{setting:?}");
                let mut set = Options::default();
                set.set(setting, on);
                assert_eq!(read, set);
            }
        }
    }
}
