//! Whether the account on this PC has PC Game Pass, judged with no sign-in of its own and nothing kept
//! (docs/research/game-pass.md 7). Every route that would say "subscribed now" needs a Microsoft account ticket the
//! Dashboard does not have; every route open to an unpackaged program without one says nothing of a subscription:
//!
//! - `StoreContext` answers only for the calling packaged app's own products and add-ons; an unpackaged process has
//!   no "current app".
//! - `AppInstallManager`'s checks (`GetIsAppAllowedToInstallAsync`, `GetIsApplicableAsync`) answer policy and
//!   device, not licence; its free-entitlement calls claim, which is a change, never a look.
//! - Gaming Services' registry keeps the same values for a bought game and a Game Pass one; the Xbox app's own
//!   files and the licence store (`ClipSVC`) are protected, and reading them would be decrypting licences.
//!
//! So the judgement is "can't tell", said with why, until the probe (`e2e/htpc/probes/gamepass.ps1`) shows a route
//! that answers. `DASHBOARD_GAMEPASS_SUBSCRIPTION` (`active`, `none`, `unknown`) stands in for it, for the tests.
use model::game_pass::Subscription;

pub const STAND_IN: &str = "DASHBOARD_GAMEPASS_SUBSCRIPTION";

/// What is judged, and how, for the log.
pub fn judged() -> (Subscription, String) {
    if let Some(given) = std::env::var(STAND_IN).ok().and_then(|v| Subscription::parse(&v)) {
        return (given, format!("as {STAND_IN} says"));
    }
    (
        Subscription::Unknown,
        "no route tells without a sign-in: StoreContext answers only a packaged app's own products, and nothing on \
         this PC that can be read keeps a subscription (docs/research/game-pass.md 7)"
            .into(),
    )
}
