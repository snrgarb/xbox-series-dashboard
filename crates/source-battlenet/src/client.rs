//! What the Battle.net client keeps of its own, read and never written (#172): `%APPDATA%\Battle.net\
//! Battle.net.config` (JSON: `Games`, keyed by the uid the client knows each game by; `Client.Install.
//! DefaultInstallPath`) and `%LOCALAPPDATA%\Battle.net\CachedData.db` (SQLite: `key_value_store`'s
//! `features_cached_data_points`, with the account's country and where its address is). A fake keeps both in its
//! own folder (`crate::fixture`).
//!
//! **Why the country is read**: the install form's `account_country` and `geo_ip_country` were sent as `USA` and
//! `US` whatever the account (as barncastle's `ProductModel` defaults them), and the TV PC's install of an Australian
//! account went into `product.db` tagged `acct-USA? geoip-US?` where the client's own Agent's tags read
//! `acct-AUS? geoip-AU?`. They are now the client's cache's, where it has them, else not sent at all: redact7's
//! bnetinstaller sends neither, and its installs go through (`InstallFinal`: `finalized`, `game_dir`, `language`,
//! `selected_asset_locale`, `selected_locale` only). Whether the tags change anything the client does is not known.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde_json::Value;

/// `Battle.net.config`, parsed; None where it is not there or not JSON.
pub fn config(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// The uids `Battle.net.config`'s `Games` names: the client's own, for the games installed, or looked at, there.
pub fn games(config: &Value) -> Vec<String> {
    let games = config.get("Games").and_then(Value::as_object);
    games.map(|games| games.keys().cloned().collect()).unwrap_or_default()
}

/// `Client.Install.DefaultInstallPath`: the folder the client offers for a new install. Read as the config is
/// commonly read; not seen on the TV PC yet.
pub fn default_install_path(config: &Value) -> Option<PathBuf> {
    let path = config.get("Client")?.get("Install")?.get("DefaultInstallPath")?.as_str()?.trim();
    (!path.is_empty()).then(|| source_kit::native(path))
}

/// The account's country (ISO 3166 alpha-3, `AUS`) and where its address is (alpha-2, `AU`), and where they were
/// read, as the install form takes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Country {
    pub account: String,
    pub geo_ip: String,
    pub from: String,
}

/// The key of the row with the account's country.
pub const POINTS: &str = "features_cached_data_points";

/// The country in the client's cache at `path`: `key_value_store`'s row whose key is [`POINTS`], its value JSON
/// with `account_country` and `geoip_country`. Read only, and tolerantly: any column holding the key, any holding
/// the JSON, as text or bytes, since the table's columns are not documented. None where the file has no such row.
pub fn country(path: &Path) -> Result<Option<Country>> {
    if !path.is_file() {
        return Ok(None);
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let db = Connection::open_with_flags(path, flags).context("not opened")?;
    let mut rows = db.prepare("SELECT * FROM key_value_store").context("no key_value_store")?;
    let columns = rows.column_count();
    let mut rows = rows.query([])?;
    while let Some(row) = rows.next()? {
        let cells: Vec<String> = (0..columns)
            .map(|i| match row.get_ref(i) {
                Ok(ValueRef::Text(t) | ValueRef::Blob(t)) => String::from_utf8_lossy(t).into_owned(),
                _ => String::new(),
            })
            .collect();
        if !cells.iter().any(|cell| cell == POINTS) {
            continue;
        }
        for cell in &cells {
            let Ok(json) = serde_json::from_str::<Value>(cell) else { continue };
            let text = |key: &str| json.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
            if let (Some(account), Some(geo_ip)) = (text("account_country"), text("geoip_country")) {
                return Ok(Some(Country {
                    account: account.to_string(),
                    geo_ip: geo_ip.to_string(),
                    from: format!("{}'s {POINTS}", path.display()),
                }));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{self, Fixture};

    #[test]
    fn the_clients_own_files_give_its_uids_and_the_accounts_country() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let config = config(&root.join(fixture::CONFIG)).unwrap();
        let mut uids = games(&config);
        uids.sort();
        // The installed games' and the catalogue's.
        assert_eq!(uids, ["battle_net", "emberwatch", "glimmerfall", "junipervault", "larkspur", "quarrybell"]);
        assert_eq!(default_install_path(&config), None);
        let country = country(&root.join(fixture::CACHE)).unwrap().unwrap();
        assert_eq!((country.account.as_str(), country.geo_ip.as_str()), fixture::ACCOUNT_COUNTRY);
        assert!(country.from.ends_with("CachedData.db's features_cached_data_points"), "{}", country.from);
        // No cache, or one without the row: nothing.
        assert_eq!(super::country(&root.join("nothing.db")).unwrap(), None);
        let other = root.join("other.db");
        Connection::open(&other).unwrap().execute_batch("CREATE TABLE key_value_store (k, v)").unwrap();
        assert_eq!(super::country(&other).unwrap(), None);
        // Columns named otherwise, the value as bytes: read all the same.
        let odd = root.join("odd.db");
        let db = Connection::open(&odd).unwrap();
        db.execute_batch("CREATE TABLE key_value_store (name TEXT, data BLOB)").unwrap();
        let json = br#"{"account_country":"AUS","geoip_country":"AU","account_region":"US"}"#.to_vec();
        db.execute("INSERT INTO key_value_store VALUES (?1, ?2)", rusqlite::params![POINTS, json]).unwrap();
        let read = super::country(&odd).unwrap().unwrap();
        assert_eq!((read.account.as_str(), read.geo_ip.as_str()), ("AUS", "AU"));
        // Not a database at all: an error, said, never a panic.
        std::fs::write(root.join("bad.db"), b"not sqlite").unwrap();
        assert!(super::country(&root.join("bad.db")).is_err());
    }
}
