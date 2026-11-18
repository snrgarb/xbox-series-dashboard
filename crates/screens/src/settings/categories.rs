//! The rail's four categories and their boxes (docs/spec/settings.md, 6.1), and Personalisation's (6.2). The
//! categories are generic, as the console's Settings has them (#194): General, Account, Games & apps and System.
//! The Bundle had one, Theme settings, holding every box.
use super::about::{self, About};
use super::added::{self, Added};
use super::art::Art;
use super::boot_video::BootVideoPage;
use super::boxes::{Boxes, Item};
use super::discord::Discord;
use super::discord_web::DiscordWeb;
use super::groups::Groups;
use super::guide::Guide;
use super::helpers::{self, Helpers};
use super::idle::Idle;
use super::launchers::{self, Launchers};
use super::profiles::Profiles;
use super::set_up::{self, SetUp};
use super::storage::{self, Storage};
use super::system;
use super::whos_playing::WhosPlayingPage;
use super::Setting;

/// A category on the rail, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// How the Dashboard looks and behaves: Personalisation, Idle, Guide, Boot video.
    General,
    /// Who plays, and the accounts they connect: Profiles, Who's playing today?, Discord.
    Account,
    /// Where the games come from and where they go: Launchers, Games & apps, Groups, Art; Storage, Helpers, Set up
    /// this PC.
    GamesAndApps,
    /// The Dashboard on this PC: System (shell mode) and About (the version and its updates).
    System,
}

impl Category {
    /// Every category, in the rail's order.
    pub const ALL: [Category; 4] = [Category::General, Category::Account, Category::GamesAndApps, Category::System];

    /// The rail's name for it, and its header.
    pub fn name(self) -> &'static str {
        match self {
            Category::General => "General",
            Category::Account => "Account",
            Category::GamesAndApps => "Games & apps",
            Category::System => "System",
        }
    }

    /// Where it is on the rail.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&category| category == self).unwrap_or(0)
    }

    /// Its boxes, focus on the first.
    pub fn boxes(self) -> Boxes {
        match self {
            Category::General => general(),
            Category::Account => account(),
            Category::GamesAndApps => games_and_apps(),
            Category::System => system(),
        }
    }
}

/// Where Discord's box is on Account: row, column.
pub const DISCORD: (usize, usize) = (1, 0);
/// Where Groups' box is on Games & apps: its place in its section, and the section.
pub const GROUPS: (usize, usize) = (2, 0);
/// Where Storage's box is on Games & apps: first in Installs.
pub const STORAGE: (usize, usize) = (0, 1);

/// General: Personalisation and Idle, the Bundle's first row; Guide under Personalisation, and Boot video beside it.
fn general() -> Boxes {
    let personalisation =
        Item::Opens { icon: '\u{E771}', name: "Personalisation", pane: |_| Box::new(personalisation()) };
    let idle = Item::Opens { icon: '\u{E708}', name: "Idle", pane: |_| Box::new(Idle::new()) };
    let guide = Item::Opens { icon: '\u{E7FC}', name: "Guide", pane: |_| Box::new(Guide::new()) };
    let boot_video =
        Item::Opens { icon: '\u{E714}', name: "Boot video", pane: |around| Box::new(BootVideoPage::open(around)) };
    Boxes::new(Category::General.name(), vec![vec![personalisation, idle], vec![guide, boot_video]])
}

/// Account: Profiles and Who's playing today?, then Discord under Profiles and Discord messages beside it (#197).
fn account() -> Boxes {
    let profiles = Item::Opens { icon: '\u{E716}', name: "Profiles", pane: |around| Box::new(Profiles::open(around)) };
    let whos_playing = Item::Opens {
        icon: '\u{E748}',
        name: "Who's playing today?",
        pane: |around| Box::new(WhosPlayingPage::open(around)),
    };
    let discord = Item::Opens { icon: '\u{E8F2}', name: "Discord", pane: |_| Box::new(Discord::default()) };
    let messages =
        Item::Opens { icon: '\u{E8BD}', name: super::discord_web::TITLE, pane: |_| Box::new(DiscordWeb::default()) };
    Boxes::new(Category::Account.name(), vec![vec![profiles, whos_playing], vec![discord, messages]])
}

/// Games & apps: more than a handful of boxes, so in two columns under section titles, as the console's pages with
/// many boxes are. Library: where games come from (Launchers, and Games & apps, added by hand), how Home groups them
/// and their art. Installs: where installs go, the helper programs some stores install through, and the launchers'
/// own apps (Set up this PC).
fn games_and_apps() -> Boxes {
    let launchers =
        Item::Opens { icon: '\u{E8F1}', name: launchers::TITLE, pane: |around| Box::new(Launchers::open(around)) };
    let added = Item::Opens { icon: '\u{E71D}', name: added::TITLE, pane: |around| Box::new(Added::open(around)) };
    let groups = Item::Opens { icon: '\u{E8FD}', name: "Groups", pane: |around| Box::new(Groups::open(around)) };
    let art = Item::Opens { icon: '\u{EB9F}', name: "Art", pane: |around| Box::new(Art::open(around)) };
    let storage =
        Item::Opens { icon: '\u{EDA2}', name: storage::TITLE, pane: |around| Box::new(Storage::open(around)) };
    let helpers =
        Item::Opens { icon: '\u{E896}', name: helpers::TITLE, pane: |around| Box::new(Helpers::open(around)) };
    let set_up = Item::Opens { icon: '\u{E977}', name: set_up::TITLE, pane: |around| Box::new(SetUp::open(around)) };
    Boxes::sections(
        Category::GamesAndApps.name(),
        vec![("Library", vec![launchers, added, groups, art]), ("Installs", vec![storage, helpers, set_up])],
    )
}

/// System: shell mode's page (docs/spec/shell-mode.md, 2), and About beside it: which build this is and its updates
/// (16.10).
fn system() -> Boxes {
    let system = Item::Opens { icon: '\u{E770}', name: system::TITLE, pane: |_| Box::new(system::System::default()) };
    let about = Item::Opens { icon: '\u{E946}', name: about::TITLE, pane: |around| Box::new(About::open(around)) };
    Boxes::new(Category::System.name(), vec![vec![system, about]])
}

/// Personalisation's boxes: the five checkbox boxes, a row each, with no text beside them (#261). Its boxes that
/// opened pages are in General (Boot video), Account (Profiles, Who's playing today?) and Games & apps (Groups) now.
pub fn personalisation() -> Boxes {
    let check = |setting, name| vec![Item::Check { setting, name }];
    Boxes::new(
        "Personalisation",
        vec![
            check(Setting::BackgroundDelay, "Background delay"),
            check(Setting::ReduceMotion, "Reduce motion"),
            check(Setting::ShowPlayHistory, "Show Play history"),
            check(Setting::ShowRecentLauncher, "Show launcher icons"),
            check(Setting::ShowBatteryPercent, "Show battery percentages"),
        ],
    )
}
