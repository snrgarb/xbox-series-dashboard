//! Theme settings, the rail's one category, and Personalisation (docs/spec/settings.md, 6.1 and 6.2). Only what
//! M3 owns is here: the boxes of later milestones (Boot video, Profiles, Who's playing today?, Idle, Discord) come
//! with their features, and those that are here close up in the grid meanwhile. Art is the port's own.
use super::art::Art;
use super::boxes::{Boxes, Item};
use super::groups::Groups;
use super::guide::Guide;
use super::Setting;

/// Theme settings' boxes.
pub fn theme_settings() -> Boxes {
    let personalisation =
        Item::Opens { icon: '\u{E771}', name: "Personalisation", pane: |_| Box::new(personalisation()) };
    let guide = Item::Opens { icon: '\u{E7FC}', name: "Guide", pane: |_| Box::new(Guide) };
    // The port's own (docs/spec/settings.md, 16.8): where the Bundle has Idle and Discord until they come back.
    let art = Item::Opens { icon: '\u{EB9F}', name: "Art", pane: |_| Box::new(Art::default()) };
    Boxes::new("Theme settings", vec![vec![personalisation, guide], vec![art]])
}

/// Personalisation's boxes: Groups (docs/spec/filters.md, 6.1), then a checkbox box a row, the second column's
/// place kept for the focused one's info. Groups is the Bundle's column 1 of row 1, beside Who's playing today?;
/// with the boxes of M5 not there yet the boxes close up, and it is first, alone on its row.
pub fn personalisation() -> Boxes {
    let check = |setting, name, info| vec![Item::Check { setting, name, info }];
    let groups = Item::Opens { icon: '\u{E8FD}', name: "Groups", pane: |around| Box::new(Groups::open(around)) };
    Boxes::new(
        "Personalisation",
        vec![
            vec![groups],
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
