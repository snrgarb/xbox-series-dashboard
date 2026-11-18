//! Which launchers the library brings games in from (Settings > Launchers, docs/spec/launchers.md), kept in the
//! settings table beside the other options:
//!
//! - `launcher.<key>`: `false` while the launcher is switched off (`true` once it is switched on again). Saved only
//!   once set, as every option is: a launcher with no row is on.
//! - `launcher.<key>.found`: what its last read found, `installed`, `not installed` or `failed`.
//! - `launcher.<key>.read`: when it was last read with something found, in Unix seconds.
//!
//! A launcher switched off keeps every row it had: its games, their Play sessions, favourite, hidden, achievements
//! and the Groups they are in. They are left out of everything a snapshot shows ([`shown`], which every query of
//! the snapshot's lists goes through), and it is not read ([`switched_on`]); switched on again, each is as it was.
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use anyhow::Result;
use model::launcher::{self, Found, Launcher, Launchers};
use model::{Library, Source};
use rusqlite::params;

use crate::sources::Read;
use crate::Database;

/// The settings row that says whether `key` is switched on.
pub fn setting(key: &str) -> String {
    format!("launcher.{key}")
}

/// The keys of the launchers switched off, as the settings table has them, for SQL: `launcher.<key>` and nothing
/// after it, with a value that says off.
pub(crate) const OFF: &str = "(SELECT substr(key, 10) FROM settings
                    WHERE key GLOB 'launcher.*' AND key NOT GLOB 'launcher.*.*'
                    AND lower(trim(value)) IN ('false', '0', 'off', 'no'))";

/// The SQL that says a row of `games`, named `table` in the query, is shown: its source still lists it, and it is
/// not of a launcher switched off.
pub(crate) fn shown(table: &str) -> String {
    format!("({table}.gone = 0 AND {table}.source NOT IN {OFF})")
}

/// The sources that are switched on, in order: those read. A source that is no launcher (the games added by hand)
/// is always on.
pub fn switched_on(sources: &[Arc<dyn Source>], off: &HashSet<String>) -> Vec<Arc<dyn Source>> {
    sources.iter().filter(|source| !off.contains(source.key())).cloned().collect()
}

impl Database {
    /// The keys of the launchers switched off.
    pub fn launchers_off(&self) -> Result<HashSet<String>> {
        let mut rows = self.conn.prepare(&format!("SELECT * FROM {OFF}"))?;
        let rows = rows.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Switches a launcher on or off. Gives how many games the library has of it, hidden ones too: those that
    /// leave every list, or come back to them. Its filter on Full library goes and comes back with them, as it was
    /// (docs/spec/filters.md, 4.2).
    pub fn set_launcher(&self, key: &str, on: bool) -> Result<usize> {
        self.set_setting(&setting(key), if on { "true" } else { "false" })?;
        self.launcher_games(key)
    }

    fn launcher_games(&self, key: &str) -> Result<usize> {
        let games: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM games WHERE source = ?1 AND gone = 0", [key], |row| row.get(0))?;
        Ok(games.max(0) as usize)
    }

    /// Every launcher, as Settings > Launchers lists them: on or off, what its last read found and when, and how
    /// many games the library has of it.
    pub fn launchers(&self) -> Result<Launchers> {
        let off = self.launchers_off()?;
        let mut list = Vec::new();
        for key in launcher::ALL {
            let found = match self.setting(&format!("{}.found", setting(key)))?.as_deref() {
                Some("installed") => Found::Installed,
                Some("not installed") => Found::NotInstalled,
                Some("failed") => Found::Failed,
                _ => Found::NotYet,
            };
            let read = self.setting(&format!("{}.read", setting(key)))?.and_then(|at| at.trim().parse().ok());
            let games = self.launcher_games(key)?;
            list.push(Launcher { key: key.to_string(), on: !off.contains(key), found, read, games });
        }
        Ok(Launchers { list, update: model::launcher::Update::Idle })
    }

    /// Keeps what reading the sources found of each, at `now`: read, not installed, or not readable.
    pub(crate) fn note_read(&self, read: &Read, now: u64) -> Result<()> {
        let note = |key: &str, found: &str| -> Result<()> {
            self.conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2",
                params![format!("{}.found", setting(key)), found],
            )?;
            Ok(())
        };
        for (key, _) in &read.found {
            note(key, "installed")?;
            self.set_setting(&format!("{}.read", setting(key)), &now.to_string())?;
        }
        for key in &read.absent {
            note(key, "not installed")?;
        }
        for (key, _) in &read.failed {
            note(key, "failed")?;
        }
        Ok(())
    }
}

/// What an update changed in the library, from the snapshots before and after it: the one line Settings >
/// Launchers shows ("1 new game, play time for 1"), and the log's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Changes {
    /// Games that were in no list before, and are now.
    pub new: usize,
    /// Games that were, and are no longer: their launcher no longer lists them.
    pub gone: usize,
    pub installed: usize,
    pub uninstalled: usize,
    /// Games whose play time or last played moved on.
    pub played: usize,
    /// A store's collection is another Group, or holds other games.
    pub groups: bool,
}

impl Changes {
    pub fn between(before: &Library, after: &Library) -> Changes {
        let all =
            |library: &Library| -> Vec<model::Game> { library.games.iter().chain(&library.hidden).cloned().collect() };
        let (before_games, after_games) = (all(before), all(after));
        let mut changes = Changes::default();
        for game in &after_games {
            match before_games.iter().find(|was| was.is(&game.key())) {
                None => changes.new += 1,
                Some(was) => {
                    changes.installed += usize::from(game.installed && !was.installed);
                    changes.uninstalled += usize::from(!game.installed && was.installed);
                    let played = game.playtime_minutes != was.playtime_minutes || game.last_played > was.last_played;
                    changes.played += usize::from(played);
                }
            }
        }
        changes.gone = before_games.iter().filter(|was| !after_games.iter().any(|game| game.is(&was.key()))).count();
        let collections = |library: &Library| -> Vec<(String, Vec<(String, String)>)> {
            let key = |id: &i64| library.game(*id).map(|game| (game.source.clone(), game.source_id.clone()));
            let mut groups: Vec<_> = library
                .filters
                .iter()
                .filter(|filter| filter.kind == model::FilterKind::Collection)
                .map(|filter| (filter.name.clone(), filter.games.iter().filter_map(key).collect::<Vec<_>>()))
                .collect();
            groups.sort();
            groups
        };
        changes.groups = collections(before) != collections(after);
        changes
    }

    pub fn is_empty(&self) -> bool {
        *self == Changes::default()
    }
}

/// `1 new game, 2 games gone, 1 installed, 1 uninstalled, play time for 3, Groups changed`; `nothing new`.
impl fmt::Display for Changes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let games = |n: usize| if n == 1 { "1 game".to_string() } else { format!("{n} games") };
        let mut parts = Vec::new();
        if self.new > 0 {
            parts.push(if self.new == 1 { "1 new game".to_string() } else { format!("{} new games", self.new) });
        }
        if self.gone > 0 {
            parts.push(format!("{} gone", games(self.gone)));
        }
        if self.installed > 0 {
            parts.push(format!("{} installed", self.installed));
        }
        if self.uninstalled > 0 {
            parts.push(format!("{} uninstalled", self.uninstalled));
        }
        if self.played > 0 {
            parts.push(format!("play time for {}", self.played));
        }
        if self.groups {
            parts.push("Groups changed".to_string());
        }
        if parts.is_empty() {
            return f.write_str("nothing new");
        }
        f.write_str(&parts.join(", "))
    }
}

#[cfg(test)]
mod tests;
