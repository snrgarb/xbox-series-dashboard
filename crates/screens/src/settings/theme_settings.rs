//! Theme settings, the rail's one category, and Personalisation (docs/spec/settings.md, 6.1 and 6.2). The boxes of
//! features still to come (Boot video, Who's playing today?, Idle, Discord) come with them, and those that are here
//! close up in the grid meanwhile. Games & apps and Art are the port's own.
use super::added::{self, Added};
use super::art::Art;
use super::boxes::{Boxes, Item};
use super::groups::Groups;
use super::guide::Guide;
use super::profiles::Profiles;
use super::Setting;

/// Theme settings' boxes.
pub fn theme_settings() -> Boxes {
    let personalisation =
        Item::Opens { icon: '\u{E771}', name: "Personalisation", pane: |_| Box::new(personalisation()) };
    let guide = Item::Opens { icon: '\u{E7FC}', name: "Guide", pane: |_| Box::new(Guide) };
    let added = Item::Opens { icon: '\u{E71D}', name: added::TITLE, pane: |around| Box::new(Added::open(around)) };
    // The port's own (docs/spec/settings.md, 16.8): beside Games & apps, where the Bundle has Discord.
    let art = Item::Opens { icon: '\u{EB9F}', name: "Art", pane: |_| Box::new(Art::default()) };
    Boxes::new("Theme settings", vec![vec![personalisation, guide], vec![added, art]])
}

/// Personalisation's boxes: Groups (docs/spec/filters.md, 6.1) and Profiles (docs/spec/profiles.md) on the first
/// row, then a checkbox box a row, the second column's place kept for the focused one's info. Profiles is in the
/// Bundle's place, column 1 of row 0; Groups is the Bundle's empty column 1 of row 1, and with Boot video and Who's
/// playing today? not there yet it is first, beside Profiles.
pub fn personalisation() -> Boxes {
    let check = |setting, name, info| vec![Item::Check { setting, name, info }];
    let groups = Item::Opens { icon: '\u{E8FD}', name: "Groups", pane: |around| Box::new(Groups::open(around)) };
    let profiles = Item::Opens { icon: '\u{E716}', name: "Profiles", pane: |around| Box::new(Profiles::open(around)) };
    Boxes::new(
        "Personalisation",
        vec![
            vec![groups, profiles],
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
