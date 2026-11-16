//! GOG Galaxy's database, `Galaxy\storage\galaxy-2.0.db` (SQLite), read-only: what the account owns, play time,
//! last played, and the pictures Galaxy has cached (docs/research/stores.md, "GOG"). Only the tables and columns
//! the research names are read:
//! - `ProductPurchaseDates(gameReleaseKey)`: what was bought or added. A release key is `<platform>_<id>`, and only
//!   GOG's own, `gog_<product id>`, are GOG's: Galaxy lists other stores' games here too.
//! - `GamePieces(releaseKey, gamePieceTypeId, value)` with `GamePieceTypes(id, type)`: the `title` piece,
//!   `{"title": "..."}`. Type ids differ between databases, so they are looked up by name.
//! - `GameTimes(releaseKey, minutesInGame)` and `LastPlayedDates(gameReleaseKey, lastPlayedDate)`.
//! - `WebCache(id, releaseKey, userId)`, `WebCacheResources(webCacheId, webCacheResourceTypeId, filename)` and
//!   `WebCacheResourceTypes(id, type)`: the file names of the cached pictures, which are under
//!   `Galaxy\webcache\<user id>\<platform>\<product id>\`; `verticalCover` is the cover, `background` the hero.
//!
//! The owned list is what must be read; the rest, where a table is not there or not as expected, is left out.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use model::Game;
use rusqlite::{Connection, OpenFlags};

/// What Galaxy's database says, by product id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Galaxy {
    /// Every product of GOG's the account has, in the database's order.
    pub owned: Vec<String>,
    pub titles: HashMap<String, String>,
    pub minutes: HashMap<String, u32>,
    /// Unix seconds.
    pub played: HashMap<String, u64>,
    pub covers: HashMap<String, PathBuf>,
    pub heroes: HashMap<String, PathBuf>,
}

impl Galaxy {
    /// The game `id`, named `name`, installed in `dir` if it is, with what the database says of it.
    pub fn game(&self, id: &str, name: String, dir: Option<PathBuf>) -> Game {
        Game {
            source: crate::SOURCE.to_string(),
            source_id: id.to_string(),
            name,
            last_played: self.played.get(id).copied().unwrap_or(0),
            playtime_minutes: self.minutes.get(id).copied().unwrap_or(0),
            installed: dir.is_some(),
            install_dir: dir,
            cover: self.covers.get(id).cloned(),
            hero: self.heroes.get(id).cloned(),
            ..Game::default()
        }
    }
}

/// The product id of a release key of GOG's: `gog_1900000042` -> `1900000042`.
fn gog_id(release_key: &str) -> Option<&str> {
    release_key.strip_prefix("gog_").filter(|id| !id.is_empty())
}

/// Reads the database at `db`; `data` is `%ProgramData%\GOG.com`, under which the cached pictures are.
pub fn read(db: &Path, data: &Path) -> Result<Galaxy> {
    // Read-only: Galaxy has it open, and it is Galaxy's.
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let db = Connection::open_with_flags(db, flags).context("not opened")?;
    let mut galaxy = Galaxy::default();
    let pairs = |sql: &str| -> rusqlite::Result<Vec<(String, rusqlite::types::Value)>> {
        let mut statement = db.prepare(sql)?;
        let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get(1)?)))?;
        rows.collect()
    };

    let owned =
        pairs("SELECT gameReleaseKey, 0 FROM ProductPurchaseDates ORDER BY rowid").context("the owned games")?;
    for (key, _) in owned {
        if let Some(id) = gog_id(&key).filter(|id| !galaxy.owned.iter().any(|known| known == id)) {
            galaxy.owned.push(id.to_string());
        }
    }
    let titles = "SELECT p.releaseKey, p.value FROM GamePieces p JOIN GamePieceTypes t ON t.id = p.gamePieceTypeId
                  WHERE t.type = 'title'";
    for (key, value) in pairs(titles).unwrap_or_default() {
        let rusqlite::types::Value::Text(value) = value else { continue };
        let title = serde_json::from_str::<serde_json::Value>(&value)
            .ok()
            .and_then(|v| Some(v.get("title")?.as_str()?.trim().to_string()));
        if let (Some(id), Some(title)) = (gog_id(&key), title.filter(|title| !title.is_empty())) {
            galaxy.titles.insert(id.to_string(), title);
        }
    }
    for (key, value) in pairs("SELECT releaseKey, minutesInGame FROM GameTimes").unwrap_or_default() {
        if let (Some(id), rusqlite::types::Value::Integer(minutes)) = (gog_id(&key), value) {
            galaxy.minutes.insert(id.to_string(), minutes.clamp(0, u32::MAX as i64) as u32);
        }
    }
    for (key, value) in pairs("SELECT gameReleaseKey, lastPlayedDate FROM LastPlayedDates").unwrap_or_default() {
        if let (Some(id), rusqlite::types::Value::Text(date)) = (gog_id(&key), value) {
            if let Some(seconds) = unix_seconds(&date) {
                galaxy.played.insert(id.to_string(), seconds);
            }
        }
    }
    let art = "SELECT c.releaseKey, c.userId, t.type, r.filename FROM WebCache c
               JOIN WebCacheResources r ON r.webCacheId = c.id
               JOIN WebCacheResourceTypes t ON t.id = r.webCacheResourceTypeId
               WHERE t.type IN ('verticalCover', 'background')";
    let pictures = db.prepare(art).and_then(|mut statement| {
        let rows = statement.query_map([], |row| {
            let user: rusqlite::types::Value = row.get(1)?;
            let user = match user {
                rusqlite::types::Value::Integer(user) => user.to_string(),
                rusqlite::types::Value::Text(user) => user,
                _ => String::new(),
            };
            Ok((row.get::<_, String>(0)?, user, row.get::<_, String>(2)?, row.get::<_, String>(3)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    });
    let cache = data.join("Galaxy").join("webcache");
    for (key, user, kind, file) in pictures.unwrap_or_default() {
        let Some((platform, id)) = key.split_once('_').filter(|(platform, _)| *platform == "gog") else { continue };
        if file.is_empty() || file.contains(['/', '\\']) || user.is_empty() {
            continue;
        }
        let path = cache.join(&user).join(platform).join(id).join(&file);
        if !path.is_file() {
            continue;
        }
        let into = if kind == "verticalCover" { &mut galaxy.covers } else { &mut galaxy.heroes };
        into.insert(id.to_string(), path);
    }
    Ok(galaxy)
}

/// `2024-06-01 10:00:00` (or with a `T`, and anything after the seconds), taken as UTC, in Unix seconds.
pub fn unix_seconds(date: &str) -> Option<u64> {
    let date = date.trim();
    let number = |range: std::ops::Range<usize>| date.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) =
        (number(11..13).unwrap_or(0), number(14..16).unwrap_or(0), number(17..19).unwrap_or(0));
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    // Days from 1970-01-01 to the date, by the civil calendar (Howard Hinnant's days_from_civil).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hour * 3600 + minute * 60 + second).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_read_as_utc() {
        assert_eq!(unix_seconds("1970-01-01 00:00:00"), Some(0));
        assert_eq!(unix_seconds("2024-06-01 10:00:00"), Some(1_717_236_000));
        assert_eq!(unix_seconds("2025-01-12T18:30:00.000Z"), Some(1_736_706_600));
        assert_eq!(unix_seconds("2000-02-29"), Some(951_782_400));
        assert_eq!(unix_seconds("yesterday"), None);
        assert_eq!(unix_seconds("2024-13-01 00:00:00"), None);
    }

    #[test]
    fn the_database_gives_gogs_own_owned_games_titles_times_and_cached_art() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = crate::fixture::Fixture::standard();
        spec.games.retain(|game| !game.installed);
        crate::fixture::write(dir.path(), &spec).unwrap();
        let places = crate::Places::under(dir.path());
        let galaxy = read(&places.database(), dir.path()).unwrap();
        let ids: Vec<&str> = spec.games.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(galaxy.owned, ids, "only GOG's own, each once");
        let first = &spec.games[0];
        assert_eq!(galaxy.titles[&first.id], first.name);
        assert_eq!(galaxy.minutes.get(&first.id).copied(), Some(first.minutes).filter(|&m| m > 0));
        assert!(galaxy.covers[&first.id].is_file());
        assert!(galaxy.covers[&first.id].starts_with(dir.path().join("Galaxy").join("webcache")));
    }

    #[test]
    fn a_database_without_the_owned_list_is_an_error_and_one_without_the_rest_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("galaxy-2.0.db");
        let db = Connection::open(&file).unwrap();
        db.execute_batch("CREATE TABLE Other (x)").unwrap();
        assert!(read(&file, dir.path()).is_err());
        db.execute_batch(
            "CREATE TABLE ProductPurchaseDates (gameReleaseKey TEXT, purchaseDate TEXT);
             INSERT INTO ProductPurchaseDates VALUES ('gog_5', '2024-06-01 10:00:00'), ('steam_440', ''), ('gog_', '');",
        )
        .unwrap();
        let galaxy = read(&file, dir.path()).unwrap();
        assert_eq!(galaxy, Galaxy { owned: vec!["5".into()], ..Galaxy::default() });
        assert!(read(&dir.path().join("none.db"), dir.path()).is_err());
    }
}
