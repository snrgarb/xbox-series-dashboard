//! Steam's achievements, from the client's own files: no Web API key and no sign-in (decided in #7).
//!
//! - `appcache/stats/UserGameStatsSchema_<appid>.bin`: the game's achievements. Binary KeyValues with inline keys:
//!   `<appid>/stats/<stat id>/bits/<bit>` is one achievement, its API name `name`, and under `display` its `name`
//!   and `desc` (each a map by language, `english` among them, or a plain string), `hidden`, and its icons `icon`
//!   and `icon_gray`, file names under Steam's CDN. Each stat with `bits` is a group of up to 32 achievements.
//! - `appcache/stats/UserGameStats_<account id>_<appid>.bin`: the signed-in user's unlocks. Its `cache/<stat id>`
//!   holds `data`, an int32 whose bit `n` is set when the group's achievement `n` is unlocked, and
//!   `AchievementTimes/<n>`, when, in Unix seconds.
//!
//! The layout is as two open-source readers have it: Steam Achievement Manager (gibbed/SteamAchievementManager,
//! `SAM.Game/Manager.cs`, the schema) and PlayniteAchievements (`Providers/Steam/Local/SteamLocalStatsReader.cs`,
//! both files). The account id is the signed-in user's folder under `userdata` ([`crate::signed_in_account`]).
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use model::achievement::{Achievement, AchievementSource};

use crate::vdf::{self, Value};

/// Where an achievement's icon file is on Steam's CDN, followed by `/<appid>/<file>`. Checked on 2026-11-17 with a
/// well-known public icon (Portal 2's, appid 620): 200, `image/jpeg`.
pub const ICONS: &str = "https://cdn.steamstatic.com/steamcommunity/public/images/apps";

/// The folder the files are in, under the Steam root.
const STATS: [&str; 2] = ["appcache", "stats"];

/// One achievement as the schema has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defined {
    /// Its stat's id, and its bit in that stat's `data`.
    pub group: u32,
    pub bit: u32,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub hidden: bool,
    /// The icons' file names on the CDN.
    pub icon: Option<String>,
    pub icon_gray: Option<String>,
}

/// The game's achievements from its schema file's bytes, in the schema's order. A schema with no achievement
/// (a game with stats only) is an empty list.
pub fn parse_schema(buf: &[u8], appid: u32) -> Result<Vec<Defined>> {
    let root = vdf::parse_binary_file(buf).context("the schema")?;
    let stats = root
        .path(&[&appid.to_string(), "stats"])
        .or_else(|| find(&root, "stats"))
        .context("the schema has no stats")?;
    let mut out = Vec::new();
    for (group, stat) in stats.entries() {
        let (Ok(group), Some(bits)) = (group.trim().parse::<u32>(), stat.get("bits")) else { continue };
        for (bit, entry) in bits.entries() {
            let Ok(bit) = bit.trim().parse::<u32>() else { continue };
            let Some(key) = entry.str_at(&["name"]).map(str::trim).filter(|key| !key.is_empty()) else { continue };
            let display = entry.get("display");
            let text = |what: &str| display.and_then(|d| d.get(what)).and_then(localised);
            let file = |what: &str| {
                let name = display?.str_at(&[what])?.trim();
                (!name.is_empty()).then(|| name.to_string())
            };
            out.push(Defined {
                group,
                bit,
                key: key.to_string(),
                name: text("name").unwrap_or_else(|| key.to_string()),
                description: text("desc"),
                hidden: display.and_then(|d| d.str_at(&["hidden"])).is_some_and(|h| h.trim() == "1"),
                icon: file("icon"),
                icon_gray: file("icon_gray"),
            });
        }
    }
    Ok(out)
}

/// The unlocks in a user's stats file's bytes: each unlocked `(stat id, bit)`, with when where the file says (else
/// 0, unlocked at a time not known).
pub fn parse_stats(buf: &[u8]) -> Result<HashMap<(u32, u32), u64>> {
    let root = vdf::parse_binary_file(buf).context("the stats")?;
    let cache = find(&root, "cache").context("the stats have no cache")?;
    let mut out = HashMap::new();
    for (group, stat) in cache.entries() {
        let Ok(group) = group.trim().parse::<u32>() else { continue };
        // An int32: bit 31 reads as a negative number.
        let Some(data) = stat.str_at(&["data"]).and_then(|d| d.trim().parse::<i64>().ok()) else { continue };
        let bits = data as u32;
        let times = stat.get("AchievementTimes");
        for bit in (0..32).filter(|bit| bits & (1 << bit) != 0) {
            let at = times.and_then(|t| t.u64_at(&[&bit.to_string()])).unwrap_or(0);
            out.insert((group, bit), at);
        }
    }
    Ok(out)
}

/// An icon's address from what the schema says of it, None for nothing:
/// - a full address (`http://`, `https://`) is taken as it is;
/// - a file name, or a path whose last part is one, is under [`ICONS`]`/<appid>/`, `.jpg` added when it has no
///   extension and a SHA-1's forty hex digits in lower case, as the CDN has them.
///
/// Checked on 2026-11-17 against the CDN with public icons of 1145360 and 440: `<hash>.jpg` and
/// `tf_..._bw.jpg` are 200; the same without `.jpg` 404 or 503, and the hash in upper case 503.
pub fn icon_address(appid: u32, given: &str) -> Option<String> {
    let given = given.trim();
    if given.is_empty() {
        return None;
    }
    let lower = given.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return Some(given.to_string());
    }
    let file = given.rsplit(['/', '\\']).next().unwrap_or(given).trim();
    if file.is_empty() || file == "." || file == ".." {
        return None;
    }
    let (stem, extension) = match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => (stem, Some(ext)),
        _ => (file.trim_end_matches('.'), None),
    };
    let hash = stem.len() == 40 && stem.bytes().all(|b| b.is_ascii_hexdigit());
    let stem = if hash { stem.to_ascii_lowercase() } else { stem.to_string() };
    let extension = extension.map_or("jpg".to_string(), |ext| if hash { ext.to_ascii_lowercase() } else { ext.into() });
    Some(format!("{ICONS}/{appid}/{stem}.{extension}"))
}

/// The game's achievements: the schema's, each unlocked as the stats say, with its icons' addresses.
pub fn combine(appid: u32, schema: Vec<Defined>, unlocks: &HashMap<(u32, u32), u64>) -> Vec<Achievement> {
    let icon = |file: Option<String>| file.and_then(|file| icon_address(appid, &file));
    schema
        .into_iter()
        .map(|d| Achievement {
            unlocked: unlocks.get(&(d.group, d.bit)).copied(),
            key: d.key,
            name: d.name,
            description: d.description,
            hidden: d.hidden,
            icon: icon(d.icon),
            icon_locked: icon(d.icon_gray),
            ..Achievement::default()
        })
        .collect()
}

/// A text in the schema: English where it is a map by language, else the text itself.
fn localised(value: &Value) -> Option<String> {
    let text = match value {
        Value::Str(text) => Some(text.as_str()),
        Value::Map(_) => value.str_at(&["english"]),
    };
    text.map(str::trim).filter(|text| !text.is_empty()).map(str::to_string)
}

/// The first map of this name anywhere under `value`, depth first.
fn find<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value.entries().iter().find_map(|(key, child)| match child {
        Value::Map(_) if key.eq_ignore_ascii_case(name) => Some(child),
        Value::Map(_) => find(child, name),
        Value::Str(_) => None,
    })
}

/// The schema's file name for a game.
pub fn schema_file(appid: u32) -> String {
    format!("UserGameStatsSchema_{appid}.bin")
}

/// The user's stats file name for a game.
pub fn stats_file(account: u32, appid: u32) -> String {
    format!("UserGameStats_{account}_{appid}.bin")
}

/// Steam's achievements, as the Dashboard reads them: the install `crate::Steam` reads.
#[derive(Debug, Clone)]
pub struct SteamAchievements {
    steam: crate::Steam,
}

impl SteamAchievements {
    /// Those of the install at `root`, or with none the one the registry names (`--steam`).
    pub fn at(root: Option<PathBuf>) -> Self {
        Self { steam: crate::Steam::at(root) }
    }

    fn stats_dir(&self) -> Option<PathBuf> {
        let root = self.steam.installed_root()?;
        STATS.iter().try_fold(root, |dir, name| crate::child(&dir, name))
    }

    fn files_of(&self, appid: u32) -> Option<(PathBuf, Option<PathBuf>)> {
        let root = self.steam.installed_root()?;
        let dir = self.stats_dir()?;
        let stats = crate::signed_in_account(&root).map(|account| dir.join(stats_file(account, appid)));
        Some((dir.join(schema_file(appid)), stats))
    }
}

impl AchievementSource for SteamAchievements {
    fn key(&self) -> &'static str {
        crate::SOURCE
    }

    /// Every app with a schema file.
    fn games(&self) -> Option<Vec<String>> {
        self.steam.installed_root()?;
        let Some(dir) = self.stats_dir() else { return Some(Vec::new()) };
        let mut appids: Vec<u32> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_ascii_lowercase();
                name.strip_prefix("usergamestatsschema_")?.strip_suffix(".bin")?.parse().ok()
            })
            .collect();
        appids.sort_unstable();
        Some(appids.into_iter().map(|appid| appid.to_string()).collect())
    }

    /// The schema's achievements; with no stats file for the signed-in user (a game never played on this PC), all
    /// locked. A schema that says no achievement is none.
    fn read(&self, source_id: &str) -> Result<Option<Vec<Achievement>>> {
        let Ok(appid) = source_id.parse::<u32>() else { return Ok(None) };
        let Some((schema, stats)) = self.files_of(appid) else { return Ok(None) };
        let bytes = match fs::read(&schema) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| schema.display().to_string()),
        };
        let defined = parse_schema(&bytes, appid).with_context(|| schema.display().to_string())?;
        if defined.is_empty() {
            return Ok(None);
        }
        let unlocks = match stats.as_deref().map(fs::read) {
            Some(Ok(bytes)) => parse_stats(&bytes).with_context(|| stats_path(&stats))?,
            Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(e).with_context(|| stats_path(&stats));
            }
            _ => HashMap::new(),
        };
        Ok(Some(combine(appid, defined, &unlocks)))
    }

    fn files(&self, source_id: &str) -> Vec<PathBuf> {
        let Some((schema, stats)) = source_id.parse().ok().and_then(|appid| self.files_of(appid)) else {
            return Vec::new();
        };
        std::iter::once(schema).chain(stats).collect()
    }
}

fn stats_path(stats: &Option<PathBuf>) -> String {
    stats.as_deref().map_or_else(String::new, |p: &Path| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Binary KeyValues with inline keys, written by hand: a map entry, a string, an int32, the end.
    struct Kv(Vec<u8>);

    impl Kv {
        fn new() -> Self {
            Kv(Vec::new())
        }
        fn open(mut self, key: &str) -> Self {
            self.0.push(0);
            self.0.extend(key.as_bytes());
            self.0.push(0);
            self
        }
        fn s(mut self, key: &str, value: &str) -> Self {
            self.0.push(1);
            self.0.extend(key.as_bytes());
            self.0.push(0);
            self.0.extend(value.as_bytes());
            self.0.push(0);
            self
        }
        fn i(mut self, key: &str, value: i32) -> Self {
            self.0.push(2);
            self.0.extend(key.as_bytes());
            self.0.push(0);
            self.0.extend(value.to_le_bytes());
            self
        }
        fn close(mut self) -> Self {
            self.0.push(8);
            self
        }
    }

    /// A made-up game's schema: two groups, the second's last bit used; a stat with no bits; a name only in
    /// French; a plain-string name; a hidden one; and one with no name, left out.
    fn schema() -> Vec<u8> {
        let kv = Kv::new()
            .open("4242")
            .s("gamename", "Made Up")
            .s("version", "3")
            .open("stats")
            .open("1")
            .s("type", "4")
            .s("id", "1")
            .open("bits")
            .open("0")
            .s("name", "ACH_ONE")
            .i("bit", 0)
            .open("display")
            .open("name")
            .s("english", "First Steps")
            .s("french", "Premiers pas")
            .s("token", "NEW_ACHIEVEMENT_1_0_NAME")
            .close()
            .open("desc")
            .s("english", "Walk a little.")
            .close()
            .s("hidden", "0")
            .s("icon", "aaaa.jpg")
            .s("icon_gray", "bbbb.jpg")
            .close()
            .close()
            .open("1")
            .s("name", "ACH_SECRET")
            .open("display")
            .s("name", "A Secret")
            .s("hidden", "1")
            .close()
            .close()
            .open("2")
            .s("name", "")
            .close()
            .close()
            .close()
            .open("2")
            .s("type", "1")
            .s("name", "STAT_STEPS")
            .close()
            .open("3")
            .s("type_int", "5")
            .open("bits")
            .open("31")
            .s("name", "ACH_LATE")
            .open("display")
            .open("name")
            .s("french", "Tard")
            .close()
            .close()
            .close()
            .close()
            .close()
            .close()
            .close()
            .close();
        kv.0
    }

    /// The user's unlocks: ACH_ONE with a time, ACH_LATE (bit 31, a negative int32) with none; the stat's own value
    /// beside them, and the cache's other entries.
    fn stats() -> Vec<u8> {
        Kv::new()
            .open("cache")
            .i("crc", 12345)
            .i("PendingChanges", 0)
            .open("1")
            .i("data", 0b01)
            .open("AchievementTimes")
            .i("0", 1_700_000_000)
            .close()
            .close()
            .open("2")
            .i("data", 77)
            .close()
            .open("3")
            .i("data", i32::MIN)
            .close()
            .close()
            .close()
            .0
    }

    #[test]
    fn the_schema_gives_each_achievement_in_order() {
        let defined = parse_schema(&schema(), 4242).unwrap();
        let keys: Vec<_> = defined.iter().map(|d| (d.group, d.bit, d.key.as_str())).collect();
        assert_eq!(keys, [(1, 0, "ACH_ONE"), (1, 1, "ACH_SECRET"), (3, 31, "ACH_LATE")]);
        let one = &defined[0];
        assert_eq!((one.name.as_str(), one.description.as_deref()), ("First Steps", Some("Walk a little.")));
        assert_eq!((one.icon.as_deref(), one.icon_gray.as_deref()), (Some("aaaa.jpg"), Some("bbbb.jpg")));
        assert!(!one.hidden);
        assert_eq!(
            (defined[1].name.as_str(), defined[1].hidden, defined[1].description.as_deref()),
            ("A Secret", true, None)
        );
        // No English name: its key.
        assert_eq!(defined[2].name, "ACH_LATE");
    }

    #[test]
    fn the_stats_give_the_unlocked_bits_and_when() {
        let unlocks = parse_stats(&stats()).unwrap();
        assert_eq!(unlocks.get(&(1, 0)), Some(&1_700_000_000));
        assert_eq!(unlocks.get(&(1, 1)), None);
        assert_eq!(unlocks.get(&(3, 31)), Some(&0));
        // A stat's own value is no group of achievements the schema knows, so it unlocks nothing in `combine`.
        let all = combine(4242, parse_schema(&schema(), 4242).unwrap(), &unlocks);
        let unlocked: Vec<_> = all.iter().map(|a| (a.key.as_str(), a.unlocked)).collect();
        assert_eq!(unlocked, [("ACH_ONE", Some(1_700_000_000)), ("ACH_SECRET", None), ("ACH_LATE", Some(0))]);
        assert_eq!(all[0].icon.as_deref(), Some(&*format!("{ICONS}/4242/aaaa.jpg")));
        assert_eq!(all[0].icon_locked.as_deref(), Some(&*format!("{ICONS}/4242/bbbb.jpg")));
        assert_eq!(all[1].icon, None);
    }

    #[test]
    fn an_icon_is_addressed_by_whatever_the_schema_holds() {
        let hash = "4c936bef605fe82e00145c7b37287a13cd690af3";
        let at = |file: &str| format!("{ICONS}/7/{file}");
        let cases: [(&str, Option<String>); 10] = [
            (&format!("{hash}.jpg"), Some(at(&format!("{hash}.jpg")))),
            // A hash with no extension, or in upper case: as the CDN has it.
            (hash, Some(at(&format!("{hash}.jpg")))),
            (&format!("{}.JPG", hash.to_uppercase()), Some(at(&format!("{hash}.jpg")))),
            // A name of the older kind keeps its case.
            ("tf_Burn_bw.jpg", Some(at("tf_Burn_bw.jpg"))),
            ("tf_Burn_bw", Some(at("tf_Burn_bw.jpg"))),
            // A path: its last part.
            (&format!("images/apps/7/{hash}.jpg"), Some(at(&format!("{hash}.jpg")))),
            // A full address, as it is.
            ("https://example.com/a/b.png", Some("https://example.com/a/b.png".to_string())),
            // Nothing: no icon.
            ("", None),
            ("   ", None),
            ("apps/7/", None),
        ];
        for (given, want) in cases {
            assert_eq!(icon_address(7, given), want, "{given:?}");
        }
    }

    #[test]
    fn broken_files_are_errors() {
        let schema = schema();
        assert!(parse_schema(&schema[..schema.len() / 2], 4242).is_err());
        assert!(parse_schema(&Kv::new().s("x", "y").0, 4242).is_err());
        assert!(parse_stats(&Kv::new().open("other").close().0).is_err());
    }

    #[test]
    fn a_steam_install_is_read_for_the_signed_in_user() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let stats_dir = root.join("appcache").join("stats");
        fs::create_dir_all(&stats_dir).unwrap();
        fs::create_dir_all(root.join("userdata").join("55").join("config")).unwrap();
        fs::write(root.join("userdata").join("55").join("config").join("localconfig.vdf"), "\"x\" {}").unwrap();
        fs::write(stats_dir.join(schema_file(4242)), schema()).unwrap();
        fs::write(stats_dir.join("UserGameStatsSchema_7.bin"), Kv::new().open("7").open("stats").close().close().0)
            .unwrap();
        fs::write(stats_dir.join("unrelated.bin"), b"").unwrap();
        let steam = SteamAchievements::at(Some(root.to_path_buf()));
        assert_eq!(steam.games(), Some(vec!["7".to_string(), "4242".to_string()]));
        // No stats file yet: every one locked.
        let read = steam.read("4242").unwrap().unwrap();
        assert!(read.iter().all(|a| a.unlocked.is_none()));
        fs::write(stats_dir.join(stats_file(55, 4242)), stats()).unwrap();
        let read = steam.read("4242").unwrap().unwrap();
        assert_eq!(read.iter().filter(|a| a.is_unlocked()).count(), 2);
        // A schema with no achievement, and an app with no schema: none.
        assert_eq!(steam.read("7").unwrap(), None);
        assert_eq!(steam.read("8").unwrap(), None);
        assert_eq!(steam.files("4242"), [stats_dir.join(schema_file(4242)), stats_dir.join(stats_file(55, 4242))]);

        // No Steam: nothing.
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(SteamAchievements::at(Some(empty.path().to_path_buf())).games(), None);
    }
}
