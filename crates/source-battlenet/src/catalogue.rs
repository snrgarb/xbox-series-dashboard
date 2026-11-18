//! The Battle.net products the Dashboard knows how to ask the Agent for (#172; docs/sources.md "Battle.net"). No
//! file on the PC lists the games the account owns without Blizzard's servers: `Battle.net.config`'s `Games` names
//! only those installed or played (galaxy_blizzard_plugin's README), and the client's `CachedData.db` licences map to
//! products only through cached catalogue fragments it keeps in part (unifideck's notes): docs/research/installs.md
//! 7. So, as Lutris does, a table: each product's NGDP code (its `product.db` code, and what the patch server knows it
//! by), the Agent's uid for it, the code `--exec="launch ..."` takes, and its name. **Ownership is not known**: a
//! game of the table is "available to install" while the client is installed, and only the Agent's answer to an
//! install says whether the account may have it (a refusal is said, and the client is opened on the game instead).
//!
//! Sources, each row checked against Blizzard's public patch server's product list (`http://us.patch.battle.net:
//! 1119/v2/summary`, read 2026-11-18, where every code below is a product with live versions): the uids and launch
//! codes from Lutris' `lutris/util/battlenet/definitions.py` (`eb1cedf`) and Playnite's `BattleNetLibrary`
//! (`886468f`), the wowdev wiki's TACT product list; Tony Hawk's Pro Skater 3 + 4 (`lbra`, uid `libra`, launch
//! `LBRA`) from Playnite's table and NonSteamLaunchers' scanner (`02dfcd0`), its code confirmed by the patch server
//! (`lbra/versions`: build 67856). Tony Hawk's Pro Skater 1 + 2 is on no Battle.net list and the patch server has no
//! product for it: not on Battle.net.
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
}

/// (NGDP code, Agent uid, launch code, name).
const BUILT_IN: &[(&str, &str, &str, &str)] = &[
    ("anbs", "anbs", "ANBS", "Diablo Immortal"),
    ("aqua", "aqua", "AQUA", "Avowed"),
    ("aris", "aris", "ARIS", "DOOM: The Dark Ages"),
    ("ark", "arkansas", "ARK", "The Outer Worlds 2"),
    ("auks", "auks", "AUKS", "Call of Duty"),
    ("d3", "diablo3_enus", "D3", "Diablo III"),
    ("fenris", "fenris", "Fen", "Diablo IV"),
    ("fore", "fore", "FORE", "Call of Duty: Vanguard"),
    ("gryphon", "gryphon", "GRY", "Warcraft Rumble"),
    ("hero", "heroes", "Hero", "Heroes of the Storm"),
    ("hsb", "hs_beta", "WTCG", "Hearthstone"),
    ("lazr", "lazarus", "LAZR", "Call of Duty: Modern Warfare 2 Campaign Remastered"),
    ("lbra", "libra", "LBRA", "Tony Hawk's Pro Skater 3 + 4"),
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

/// The built-in table.
pub fn built_in() -> Vec<Product> {
    BUILT_IN
        .iter()
        .map(|&(code, uid, launch, name)| Product {
            code: code.into(),
            uid: uid.into(),
            launch: Some(launch.into()),
            name: name.into(),
        })
        .collect()
}

/// A fake's table, `catalogue.json` (`[{"uid", "code", "name"}]`): what it reads in place of the built-in one.
pub fn of_fake(root: &Path) -> Vec<Product> {
    let Ok(text) = std::fs::read_to_string(root.join(crate::fixture::CATALOGUE)) else { return Vec::new() };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str(&text) else { return Vec::new() };
    items
        .iter()
        .filter_map(|item| {
            let text = |key: &str| item.get(key).and_then(serde_json::Value::as_str).map(str::to_string);
            Some(Product { code: text("code")?, uid: text("uid")?, launch: text("launch"), name: text("name")? })
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
        assert_eq!((thps.uid.as_str(), thps.launch.as_deref()), ("libra", Some("LBRA")));
        for (i, product) in table.iter().enumerate() {
            assert!(table[i + 1..].iter().all(|p| p.code != product.code && p.uid != product.uid), "{product:?}");
            assert!(product.code.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "{product:?}");
        }
    }
}
