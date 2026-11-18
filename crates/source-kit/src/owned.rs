//! A store's list of the games the account owns, as a file the Dashboard keeps (#276; docs/research/owned-games.md
//! 1, 11): `<data>\owned\<store>.json`. Where a store's owned games are not in its client's own files in a form its
//! source reads at once (a list online, behind a helper's sign-in; a client's cache that has to be worked out), a
//! **refresh** brings it up to date off the window's thread, at start and on Update now (`model::Source::
//! refresh_owned`), and the source's `read` takes the file as one more of the store's files. A refresh that fails
//! (offline, signed out, a cache that can't be read) leaves the last file as it was and says why in one line of the
//! log: the library never loses games because a refresh failed.
//!
//! The file: the store's key, when the list was taken and from where, the account's public id where the store has
//! one, how many games the store's catalogue had where the list is taken from one, and each game: its id at the
//! store (the source's `source_id`), its name, whether it is the account's only because it is free to play, the
//! addresses of its art where the store named any, and what else the store's source needs of it (`details`).
//! Nothing secret is ever in it: no token, no cookie, no licence number.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The folder under the data folder the lists are kept in.
pub const DIR: &str = "owned";

/// One store's owned games.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct List {
    /// The store's source key: `battlenet`, `gog`.
    pub store: String,
    /// When the list was taken, in Unix seconds: when the store's service answered, or when the client last wrote
    /// the files it was worked out from.
    pub taken: u64,
    /// Where it was taken from, as the log says it: `CachedData.db`, `galaxy-library.gog.com`.
    pub from: String,
    /// The account's public id at the store, where it has one; the M10 Profiles show each its own (account-switching
    /// .md 5.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// How many games the store's catalogue had, where the list was worked out from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<usize>,
    pub games: Vec<Owned>,
}

/// A game the account owns.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owned {
    /// Its id at the store: the source's `source_id`.
    pub id: String,
    pub name: String,
    /// The account's only because the store lets anyone play it (no licence names it).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub free: bool,
    /// Its pictures' addresses, where the store named any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub art: Vec<String>,
    /// What else the store's source keeps of it (Battle.net's `uid` and `launch`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl Owned {
    /// The detail `key`, where the list has it.
    pub fn detail(&self, key: &str) -> Option<&str> {
        self.details.get(key).map(String::as_str)
    }
}

/// The file of `store`'s list in `dir` (the data folder's [`DIR`]).
pub fn path(dir: &Path, store: &str) -> PathBuf {
    dir.join(format!("{store}.json"))
}

/// `store`'s list in `dir`; None where there is none yet.
pub fn read(dir: &Path, store: &str) -> Result<Option<List>> {
    let path = path(dir, store);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| path.display().to_string()),
    };
    serde_json::from_str(&text).map(Some).with_context(|| path.display().to_string())
}

/// Writes `list` in `dir`, whole or not at all: beside its place first, then put in it.
pub fn write(dir: &Path, list: &List) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
    let path = path(dir, &list.store);
    let fresh = path.with_extension("json.new");
    let text = serde_json::to_string_pretty(list)?;
    std::fs::write(&fresh, text).with_context(|| fresh.display().to_string())?;
    std::fs::rename(&fresh, &path).with_context(|| path.display().to_string())
}

/// Brings `store`'s list in `dir` up to date with what `take` gives: written where it gave one (`Ok(Some)`), left
/// as it was where it gave none (`Ok(None)`: nothing to take it from, such as a client never signed in) or failed,
/// which one line of the log says, with what is kept: `<store>: owned: not taken (<why>): the last list kept (3 games,
/// taken 2026-11-18 09:30 UTC)`. The list written, where it differs from the last one in its games: the source is
/// then to be read again.
pub fn refresh(dir: &Path, store: &str, take: impl FnOnce() -> Result<Option<List>>) -> Option<List> {
    let last = read(dir, store);
    match take() {
        Ok(Some(list)) => {
            let changed = !matches!(&last, Ok(Some(last)) if last.games == list.games && last.account == list.account);
            if let Err(e) = write(dir, &list) {
                eprintln!("{store}: owned: not kept ({e:#})");
                return None;
            }
            changed.then_some(list)
        }
        Ok(None) => None,
        Err(e) => {
            let kept = match last {
                Ok(Some(last)) => {
                    format!("the last list kept ({} games, taken {})", last.games.len(), when(last.taken))
                }
                Ok(None) => "no list kept yet".to_string(),
                Err(e) => format!("the last list unreadable too ({e:#})"),
            };
            eprintln!("{store}: owned: not taken ({e:#}): {kept}");
            None
        }
    }
}

/// Now, in Unix seconds.
pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// When a file was last written, in Unix seconds.
pub fn modified(path: &Path) -> Option<u64> {
    let time = std::fs::metadata(path).ok()?.modified().ok()?;
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Unix seconds as the log says them: `2026-11-18 09:30 UTC`.
pub fn when(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let (hour, minute) = ((secs % 86_400) / 3600, (secs % 3600) / 60);
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(games: &[&str]) -> List {
        let owned = |id: &&str| Owned { id: id.to_string(), name: format!("Game {id}"), ..Owned::default() };
        List {
            store: "made-up".into(),
            taken: 1_795_000_000,
            from: "here".into(),
            games: games.iter().map(owned).collect(),
            ..List::default()
        }
    }

    #[test]
    fn a_list_is_kept_as_a_file_and_a_failure_leaves_the_last_one() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().join(DIR);
        assert_eq!(read(&dir, "made-up").unwrap(), None);
        // The first list is written, and is news.
        assert_eq!(refresh(&dir, "made-up", || Ok(Some(list(&["a", "b"])))), Some(list(&["a", "b"])));
        assert_eq!(read(&dir, "made-up").unwrap(), Some(list(&["a", "b"])));
        // The same games again: written (its time), and not news.
        let later = List { taken: 1_795_000_600, ..list(&["a", "b"]) };
        assert_eq!(refresh(&dir, "made-up", || Ok(Some(later.clone()))), None);
        assert_eq!(read(&dir, "made-up").unwrap(), Some(later.clone()));
        // A failure, or nothing to take it from: the last list as it was.
        assert_eq!(refresh(&dir, "made-up", || Err(anyhow::anyhow!("offline"))), None);
        assert_eq!(refresh(&dir, "made-up", || Ok(None)), None);
        assert_eq!(read(&dir, "made-up").unwrap(), Some(later));
        // A game more: news.
        assert!(refresh(&dir, "made-up", || Ok(Some(list(&["a", "b", "c"])))).is_some());
        assert!(!dir.join("made-up.json.new").exists());
        // A file that is not a list: an error, said, never a panic.
        std::fs::write(path(&dir, "broken"), "not json").unwrap();
        assert!(read(&dir, "broken").is_err());
    }

    #[test]
    fn the_file_leaves_out_what_a_game_does_not_have() {
        let mut game = Owned { id: "x1".into(), name: "X".into(), ..Owned::default() };
        let text = serde_json::to_string(&game).unwrap();
        assert_eq!(text, r#"{"id":"x1","name":"X"}"#);
        game.free = true;
        game.details.insert("uid".into(), "x_one".into());
        let back: Owned = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert_eq!((back.free, back.detail("uid")), (true, Some("x_one")));
    }

    #[test]
    fn times_read_as_utc() {
        assert_eq!(when(0), "1970-01-01 00:00 UTC");
        assert_eq!(when(951_782_400), "2000-02-29 00:00 UTC");
        assert_eq!(when(1_795_000_000), "2026-11-18 11:06 UTC");
    }
}
