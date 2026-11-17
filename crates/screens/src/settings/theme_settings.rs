//! Theme settings, the rail's one category, and Personalisation (docs/spec/settings.md, 6.1 and 6.2). Games & apps
//! and Art are the port's own.
use super::about::{self, About};
use super::added::{self, Added};
use super::art::Art;
use super::boot_video::BootVideoPage;
use super::boxes::{Boxes, Item};
use super::discord::Discord;
use super::groups::Groups;
use super::guide::Guide;
use super::idle::Idle;
use super::launchers::{self, Launchers};
use super::profiles::Profiles;
use super::storage::{self, Storage};
use super::system;
use super::whos_playing::WhosPlayingPage;
use super::Setting;

/// Where Discord's box is on Theme settings: row, column.
pub const DISCORD: (usize, usize) = (1, 1);

/// Theme settings' boxes.
pub fn theme_settings() -> Boxes {
    let personalisation =
        Item::Opens { icon: '\u{E771}', name: "Personalisation", pane: |_| Box::new(personalisation()) };
    let idle = Item::Opens { icon: '\u{E708}', name: "Idle", pane: |_| Box::new(Idle::new()) };
    let guide = Item::Opens { icon: '\u{E7FC}', name: "Guide", pane: |_| Box::new(Guide::new()) };
    let discord = Item::Opens { icon: '\u{E8F2}', name: "Discord", pane: |_| Box::new(Discord::default()) };
    let added = Item::Opens { icon: '\u{E71D}', name: added::TITLE, pane: |around| Box::new(Added::open(around)) };
    // The Bundle's four at its places (6.1), then the port's own: Games & apps and Launchers beside it, where the
    // library's games come from (docs/spec/launchers.md), and Art under them (16.8).
    let launchers =
        Item::Opens { icon: '\u{E8F1}', name: launchers::TITLE, pane: |around| Box::new(Launchers::open(around)) };
    let art = Item::Opens { icon: '\u{EB9F}', name: "Art", pane: |around| Box::new(Art::open(around)) };
    // Storage beside Art, under Launchers: where installs go (docs/spec/storage.md), until Settings is regrouped
    // (#194).
    let storage =
        Item::Opens { icon: '\u{EDA2}', name: storage::TITLE, pane: |around| Box::new(Storage::open(around)) };
    // Shell mode's page, the port's own, on a row of its own under Art (docs/spec/shell-mode.md, 2), and About beside
    // it: which build this is (16.10).
    let system = Item::Opens { icon: '\u{E770}', name: system::TITLE, pane: |_| Box::new(system::System::default()) };
    let about = Item::Opens { icon: '\u{E946}', name: about::TITLE, pane: |around| Box::new(About::open(around)) };
    Boxes::new(
        "Theme settings",
        vec![
            vec![personalisation, idle],
            vec![guide, discord],
            vec![added, launchers],
            vec![art, storage],
            vec![system, about],
        ],
    )
}

/// Personalisation's boxes: Boot video (docs/spec/start-up.md) and Profiles (docs/spec/profiles.md) on the first
/// row, Who's playing today? under Boot video, all three at the Bundle's places; Groups (docs/spec/filters.md, 6.1)
/// in the Bundle's empty column 1 of row 1, beside it; then a checkbox box a row, the second column's place kept for
/// the focused one's info.
pub fn personalisation() -> Boxes {
    let check = |setting, name, info| vec![Item::Check { setting, name, info }];
    let groups = Item::Opens { icon: '\u{E8FD}', name: "Groups", pane: |around| Box::new(Groups::open(around)) };
    let profiles = Item::Opens { icon: '\u{E716}', name: "Profiles", pane: |around| Box::new(Profiles::open(around)) };
    let boot_video =
        Item::Opens { icon: '\u{E714}', name: "Boot video", pane: |around| Box::new(BootVideoPage::open(around)) };
    let whos_playing = Item::Opens {
        icon: '\u{E748}',
        name: "Who's playing today?",
        pane: |around| Box::new(WhosPlayingPage::open(around)),
    };
    Boxes::new(
        "Personalisation",
        vec![
            vec![boot_video, profiles],
            vec![whos_playing, groups],
            check(
                Setting::BackgroundDelay,
                "Background delay",
                "Waits a moment on a game before Home's background shows its art. Turn it off to change the \
                 background as you move along your recent games.",
            ),
            check(
                Setting::ReduceMotion,
                "Reduce motion",
                "Shows pages, rows and tiles in place and holds the background still. Turn it on if the Dashboard \
                 feels slow on this PC.",
            ),
            check(
                Setting::ShowPlayHistory,
                "Show Play history",
                "Ends Home's recent games with the Play history tile. Turn it off to show another recent game in \
                 its place; Play history stays in My games & apps.",
            ),
            check(
                Setting::ShowBatteryPercent,
                "Show battery percentages",
                "Shows how much charge is left beside the controller's battery, and this PC's on a laptop or \
                 handheld. Turn it off to show the batteries alone.",
            ),
        ],
    )
}
