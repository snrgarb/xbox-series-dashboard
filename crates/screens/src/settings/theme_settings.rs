//! Theme settings, the rail's one category, and Personalisation (docs/spec/settings.md, 6.1 and 6.2). Only what
//! M3 owns is here: the boxes of later milestones (Boot video, Profiles, Who's playing today?, Idle, Discord) come
//! with their features, and those that are here close up in the grid meanwhile.
use super::boxes::{Boxes, Item};
use super::guide::Guide;
use super::Setting;

/// Theme settings' boxes.
pub fn theme_settings() -> Boxes {
    let personalisation =
        Item::Opens { icon: '\u{E771}', name: "Personalisation", pane: || Box::new(personalisation()) };
    let guide = Item::Opens { icon: '\u{E7FC}', name: "Guide", pane: || Box::new(Guide) };
    Boxes::new("Theme settings", vec![vec![personalisation, guide]])
}

/// Personalisation's boxes: a checkbox box a row, the second column's place kept for the focused one's info.
pub fn personalisation() -> Boxes {
    let check = |setting, name, info| vec![Item::Check { setting, name, info }];
    Boxes::new(
        "Personalisation",
        vec![
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
