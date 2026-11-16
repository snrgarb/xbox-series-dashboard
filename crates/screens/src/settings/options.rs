//! The options Settings holds, as the Dashboard has them: typed, each with its default (docs/spec/settings.md,
//! section 7). They are kept in the library database's `settings` table, a key and a value each, which the core
//! never sees: whoever owns the database reads them into an [`Options`] at start-up ([`Options::read`]) and writes
//! each [`Change`] the Dashboard asks for (`Effect::Set`). An option is in the table only once it has been set, so
//! one that was never touched follows its default, whatever that comes to be.

/// An option that is on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Setting {
    /// Home's background waits a second and a half on a game before showing its art; off, 150 ms.
    BackgroundDelay,
    /// Pages, rows and tiles are shown in place, and the background holds still.
    ReduceMotion,
    /// The Recent row ends with the Play history tile; off, with one game more.
    ShowPlayHistory,
    /// The figure beside the pad's battery and the PC's.
    ShowBatteryPercent,
    /// The Guide opens with the Guide button and from Home; off, the button is left to Steam or another program.
    UseGuide,
}

impl Setting {
    pub const ALL: [Setting; 5] = [
        Setting::BackgroundDelay,
        Setting::ReduceMotion,
        Setting::ShowPlayHistory,
        Setting::ShowBatteryPercent,
        Setting::UseGuide,
    ];

    /// The option's key in the settings table. Never changed: a database has it.
    pub fn key(self) -> &'static str {
        match self {
            Setting::BackgroundDelay => "home.background_delay",
            Setting::ReduceMotion => "motion.reduce",
            Setting::ShowPlayHistory => "home.play_history",
            Setting::ShowBatteryPercent => "status.battery_percent",
            Setting::UseGuide => "guide.enabled",
        }
    }

    /// What the option is until it is set.
    pub fn default(self) -> bool {
        self != Setting::ReduceMotion
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

/// The settings row that holds the user's SteamGridDB key (`library::art::KEY_SETTING`). The core is told only
/// whether there is one: what it is never comes back from the database to the screens.
pub const ART_KEY: &str = "art.steamgriddb_key";

/// Every option's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub background_delay: bool,
    pub reduce_motion: bool,
    pub show_play_history: bool,
    pub show_battery_percent: bool,
    pub use_guide: bool,
    /// A SteamGridDB key has been entered (Settings > Art): art is looked up for the games a store gives none for.
    pub art_key: bool,
}

impl Default for Options {
    fn default() -> Self {
        let of = Setting::default;
        Self {
            background_delay: of(Setting::BackgroundDelay),
            reduce_motion: of(Setting::ReduceMotion),
            show_play_history: of(Setting::ShowPlayHistory),
            show_battery_percent: of(Setting::ShowBatteryPercent),
            use_guide: of(Setting::UseGuide),
            art_key: false,
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
        options
    }

    pub fn get(&self, setting: Setting) -> bool {
        match setting {
            Setting::BackgroundDelay => self.background_delay,
            Setting::ReduceMotion => self.reduce_motion,
            Setting::ShowPlayHistory => self.show_play_history,
            Setting::ShowBatteryPercent => self.show_battery_percent,
            Setting::UseGuide => self.use_guide,
        }
    }

    pub fn set(&mut self, setting: Setting, on: bool) {
        *match setting {
            Setting::BackgroundDelay => &mut self.background_delay,
            Setting::ReduceMotion => &mut self.reduce_motion,
            Setting::ShowPlayHistory => &mut self.show_play_history,
            Setting::ShowBatteryPercent => &mut self.show_battery_percent,
            Setting::UseGuide => &mut self.use_guide,
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
        assert!(options.use_guide && !options.reduce_motion);
    }

    #[test]
    fn an_option_is_its_default_until_the_table_says_on_or_off() {
        let stored = |key: &str| match key {
            "home.background_delay" => Some("false".to_string()),
            "motion.reduce" => Some(" On ".to_string()),
            "home.play_history" => Some("perhaps".to_string()),
            _ => None,
        };
        let options = Options::read(stored);
        let expected = Options { background_delay: false, reduce_motion: true, ..Options::default() };
        assert_eq!(options, expected);
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
