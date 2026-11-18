//! The Battle.net products the Dashboard knows how to ask the Agent for (#172; docs/sources.md "Battle.net"), where
//! the client's own cache does not say (#276: `crate::cache` reads the client's catalogue and the account's
//! licences, and the games owned go over this table's rows with the client's own uid, code, launch code and name,
//! verified; this table is the fallback for a PC whose client has no such cache, or is signed out).
//! `Battle.net.config`'s `Games` names only those installed or played (galaxy_blizzard_plugin's README). So, as
//! Lutris does, a table: each product's NGDP code (its `product.db` code, and what the patch server knows it
//! by), the Agent's uid for it, the code `--exec="launch ..."` takes, and its name. **Ownership is not known**: a
//! game of the table is "available to install" while the client is installed, and only the Agent's answer to an
//! install says whether the account may have it (a refusal is said, and the client is opened on the game instead).
//!
//! **The uid is what the client knows the install by, and a table's uid may be stale** (#172): the TV PC's first
//! real install went in under `libra`, the uid Playnite's and NonSteamLaunchers' tables give Tony Hawk's Pro Skater
//! 3 + 4; the Agent took it (it takes any uid with a good patch URL), and the client, which keeps that game under
//! `lbra` (its `Battle.net.config` `Games` key, its log's product state), showed Install and would not start it.
//! So a row's uid is used for an install through the Agent only where it is **verified**: by the row itself
//! (`verified`, what confirmed it), or on the PC (`crate::BattleNet::resolve`: `Battle.net.config`'s `Games` names the
//! uid, or names the product code, which is then the uid). Otherwise the client is opened on the game.
//!
//! Every code checked against Blizzard's public patch server (`http://us.patch.battle.net:1119/<code>/versions`,
//! read 2026-11-18: each has a live `us` version) and its product configuration on Blizzard's CDN
//! (`tpr/configs/data/<ProductConfig>`): every one names itself (`product`, `title_info.title_id`) by its code, gives
//! the client's folder name (`form.game_dir.dirname`) and writes `--uid=%uid%` into its Uninstall key, so none says
//! what the uid is, **except Hearthstone's**, which lists `valid_uids: ["hs_beta"]`. The other uids are the tables'
//! (Lutris' `lutris/util/battlenet/definitions.py` @`eb1cedf`, from galaxy_blizzard_plugin, which read real
//! `product.db` files years ago; Playnite's `BattleNetLibrary` @`886468f`; NonSteamLaunchers @`02dfcd0`).
//! The older games keep the names of their first Agent (`prometheus`, `heroes`, `hs_beta`, `diablo3_enus`, `s2_enus`),
//! names an old table may well have right; for the games of the last few years (`title_info` with no
//! `products_in_title`, the product's code its only name) the current client keys by the code itself, as `lbra`
//! shows, and a table's codename for one (`libra`, `arkansas`) is a guess that fails. Launch codes are the tables'.
use std::path::Path;

/// A product the Agent can be asked to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    /// The NGDP code: `product.db`'s product code, the game's `source_id`, and the patch server's product.
    pub code: String,
    /// The Agent's uid: its install paths, `--uid=` and `--game=`.
    pub uid: String,
    /// What `Battle.net.exe --exec="launch <it>"` takes (Overwatch's `Pro`, Diablo IV's `Fen`), where known.
    pub launch: Option<String>,
    pub name: String,
    /// What confirmed `uid` as the one the current client uses, as the log says it; None where only a table gives it.
    pub verified: Option<String>,
}

/// What confirmed Tony Hawk's Pro Skater 3 + 4's uid.
const LBRA_SEEN: &str = "the TV PC's Battle.net client, 2026-11-18: its Battle.net.config Games key and its product \
                         state are lbra";
/// What confirmed Hearthstone's.
const HSB_SEEN: &str = "Blizzard's product configuration for hsb: valid_uids [hs_beta]";

/// What confirms a uid the client's own cache gives (#276; `crate::cache`): its catalogue's retail type's uid,
/// installed as the game's code. Each such uid on the TV PC was one its client's log names (36 of 36).
pub const CACHE_SEEN: &str = "the client's own catalogue in its cache (CachedData.db's catalog_cache)";

/// (NGDP code, Agent uid, launch code, name). The uid verified where [`verified`] says so.
const BUILT_IN: &[(&str, &str, &str, &str)] = &[
    ("anbs", "anbs", "ANBS", "Diablo Immortal"),
    ("aqua", "aqua", "AQUA", "Avowed"),
    ("aris", "aris", "ARIS", "DOOM: The Dark Ages"),
    ("ark", "ark", "ARK", "The Outer Worlds 2"),
    ("auks", "auks", "AUKS", "Call of Duty"),
    ("d3", "diablo3_enus", "D3", "Diablo III"),
    ("fenris", "fenris", "Fen", "Diablo IV"),
    ("fore", "fore", "FORE", "Call of Duty: Vanguard"),
    ("gryphon", "gryphon", "GRY", "Warcraft Rumble"),
    ("hero", "heroes", "Hero", "Heroes of the Storm"),
    ("hsb", "hs_beta", "WTCG", "Hearthstone"),
    ("lazr", "lazarus", "LAZR", "Call of Duty: Modern Warfare 2 Campaign Remastered"),
    ("lbra", "lbra", "LBRA", "Tony Hawk's Pro Skater 3 + 4"),
    ("odin", "odin", "ODIN", "Call of Duty: Modern Warfare"),
    ("osi", "osi", "OSI", "Diablo II: Resurrected"),
    ("pro", "prometheus", "Pro", "Overwatch 2"),
    ("rtro", "rtro", "RTRO", "Blizzard Arcade Collection"),
    ("s1", "s1", "S1", "StarCraft: Remastered"),
    ("s2", "s2_enus", "S2", "StarCraft II"),
    ("scor", "scorpio", "SCOR", "Sea of Thieves"),
    ("viper", "viper", "VIPR", "Call of Duty: Black Ops 4"),
    ("w1r", "w1r", "W1R", "Warcraft I: Remastered"),
    ("w2r", "w2r", "W2R", "Warcraft II: Remastered"),
    ("w3", "w3", "W3", "Warcraft III: Reforged"),
    ("wlby", "wlby", "WLBY", "Crash Bandicoot 4: It's About Time"),
    ("wow", "wow", "WoW", "World of Warcraft"),
    ("zeus", "zeus", "ZEUS", "Call of Duty: Black Ops Cold War"),
];

/// What confirmed a row's uid, where anything has.
fn verified(code: &str) -> Option<&'static str> {
    match code {
        "lbra" => Some(LBRA_SEEN),
        "hsb" => Some(HSB_SEEN),
        _ => None,
    }
}

/// The built-in table.
pub fn built_in() -> Vec<Product> {
    BUILT_IN
        .iter()
        .map(|&(code, uid, launch, name)| Product {
            code: code.into(),
            uid: uid.into(),
            launch: Some(launch.into()),
            name: name.into(),
            verified: verified(code).map(String::from),
        })
        .collect()
}

/// A fake's table, `catalogue.json` (`[{"uid", "code", "name", "launch"?, "verified"?}]`): what it reads in place of
/// the built-in one.
pub fn of_fake(root: &Path) -> Vec<Product> {
    let Ok(text) = std::fs::read_to_string(root.join(crate::fixture::CATALOGUE)) else { return Vec::new() };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str(&text) else { return Vec::new() };
    items
        .iter()
        .filter_map(|item| {
            let text = |key: &str| item.get(key).and_then(serde_json::Value::as_str).map(str::to_string);
            Some(Product {
                code: text("code")?,
                uid: text("uid")?,
                launch: text("launch"),
                name: text("name")?,
                verified: text("verified"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_one_product_by_code_and_by_uid() {
        let table = built_in();
        let thps = table.iter().find(|p| p.code == "lbra").expect("Tony Hawk's Pro Skater 3 + 4");
        // `lbra`, as the client keeps it: `libra`, the tables' uid, is what the TV PC's first install went in under.
        assert_eq!((thps.uid.as_str(), thps.launch.as_deref()), ("lbra", Some("LBRA")));
        assert!(thps.verified.is_some());
        let verified: Vec<&str> = table.iter().filter(|p| p.verified.is_some()).map(|p| p.code.as_str()).collect();
        assert_eq!(verified, ["hsb", "lbra"], "only what something on the current client confirmed");
        for (i, product) in table.iter().enumerate() {
            assert!(table[i + 1..].iter().all(|p| p.code != product.code && p.uid != product.uid), "{product:?}");
            assert!(product.code.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "{product:?}");
        }
    }
}
