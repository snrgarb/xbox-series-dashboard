//! The new launcher's files, under `%LOCALAPPDATA%\com.epicgames.eglv2` (docs/research/stores.md: reported by one
//! project only, and not yet seen on a real machine):
//! - `cache\prod\account\<account>\library-cache.json`: the account's library, `{"items": [{"appId": {"namespaceId",
//!   "itemId", "appName"}, "title", "owned"}]}`. The most recently written account's is read.
//! - `cache\prod\catalog-cache.json`: `{"items": {"<itemId>": {"item": <a catalogue item>}}}`, which tells a game
//!   from DLC and extras.
//! - `installations.jsonl`: one JSON object a line, `installationId`, `complete`, `dir`, `itemId`,
//!   `launchExecutable`; an installation's last line is the one that counts, and only a complete one is installed.
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::Item;

/// `installations.jsonl`.
pub const INSTALLATIONS: &str = "installations.jsonl";

/// `cache\prod`.
fn prod(root: &Path) -> PathBuf {
    root.join("cache").join("prod")
}

/// Whether the new launcher has left its lists under `root`.
pub fn any(root: &Path) -> bool {
    root.join(INSTALLATIONS).is_file() || library_cache(root).is_some()
}

/// The most recently written `cache\prod\account\*\library-cache.json`.
fn library_cache(root: &Path) -> Option<PathBuf> {
    let accounts = fs::read_dir(prod(root).join("account")).ok()?;
    accounts
        .flatten()
        .map(|account| account.path().join("library-cache.json"))
        .filter(|file| file.is_file())
        .max_by_key(|file| fs::metadata(file).and_then(|m| m.modified()).ok())
}

/// The new launcher's games under `root`: every owned game of its library cache, installed where an installation
/// says so. None when it has left no lists there.
pub fn items(root: &Path) -> Result<Option<Vec<Item>>> {
    if !any(root) {
        return Ok(None);
    }
    let read = |file: PathBuf| -> Result<Option<String>> {
        match fs::read_to_string(&file) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| file.display().to_string()),
        }
    };
    let installs = read(root.join(INSTALLATIONS))?.map(|text| installations(&text)).unwrap_or_default();
    let catalogue = read(prod(root).join("catalog-cache.json"))?.map(|text| catalog_cache(&text));
    let library = match library_cache(root) {
        Some(file) => read(file)?.unwrap_or_default(),
        None => String::new(),
    };
    Ok(Some(library_items(&library, catalogue.as_ref(), &installs)))
}

/// An installation: its folder and its program relative to there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    pub dir: PathBuf,
    pub exe: String,
}

/// `installations.jsonl`: `itemId` -> its installation, for every complete one whose folder is there. A line that is
/// not JSON is passed over.
pub fn installations(text: &str) -> HashMap<String, Installation> {
    let mut last: Vec<(String, Value)> = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        let id = value.get("installationId").and_then(Value::as_str).unwrap_or_default().to_string();
        match last.iter_mut().find(|(known, _)| *known == id) {
            Some((_, known)) => *known = value,
            None => last.push((id, value)),
        }
    }
    last.into_iter()
        .filter(|(_, value)| value.get("complete").and_then(Value::as_bool) == Some(true))
        .filter_map(|(_, value)| {
            let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
            let dir = PathBuf::from(text("dir")?);
            let item = text("itemId").filter(|_| dir.is_dir())?;
            Some((item, Installation { dir, exe: text("launchExecutable").unwrap_or_default() }))
        })
        .collect()
}

/// `catalog-cache.json`: `itemId` -> its catalogue item, or None where the item is not a game.
fn catalog_cache(text: &str) -> HashMap<String, Option<Item>> {
    let value: Value = serde_json::from_str(text).unwrap_or_default();
    let items = value.get("items").and_then(Value::as_object);
    items
        .into_iter()
        .flatten()
        .map(|(id, entry)| (id.clone(), entry.get("item").and_then(crate::catalogue_item)))
        .collect()
}

/// The library cache's owned items that are games: by the catalogue where it has the item, else taken as one.
fn library_items(
    library: &str,
    catalogue: Option<&HashMap<String, Option<Item>>>,
    installs: &HashMap<String, Installation>,
) -> Vec<Item> {
    let value: Value = serde_json::from_str(library).unwrap_or_default();
    let list = value.get("items").and_then(Value::as_array);
    list.into_iter()
        .flatten()
        .filter(|entry| entry.get("owned").and_then(Value::as_bool) != Some(false))
        .filter_map(|entry| {
            let id = |key: &str| entry.pointer(&format!("/appId/{key}")).and_then(Value::as_str).unwrap_or_default();
            let (app, namespace, item_id) = (id("appName"), id("namespaceId"), id("itemId"));
            let known = catalogue.and_then(|catalogue| catalogue.get(item_id));
            if app.is_empty() || matches!(known, Some(None)) {
                return None;
            }
            let title = entry.get("title").and_then(Value::as_str).filter(|title| !title.is_empty());
            let title = title.map(str::to_string).or_else(|| Some(known?.as_ref()?.title.clone()))?;
            let install = installs.get(item_id).map(|install| (install.dir.clone(), install.exe.clone()));
            Some(Item {
                app_name: app.to_string(),
                namespace: namespace.to_string(),
                item_id: item_id.to_string(),
                title,
                install,
                size: 0,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_installation_is_its_last_line_and_counts_when_complete_and_there() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("A"), dir.path().join("B"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let line = |install: &str, complete: bool, dir: &Path, item: &str| {
            json!({ "installationId": install, "complete": complete, "dir": dir, "itemId": item, "launchExecutable": "a.exe" })
                .to_string()
        };
        let text = [
            line("1", false, &a, "item-a"),
            line("1", true, &a, "item-a"),
            line("2", true, &b, "item-b"),
            line("2", false, &b, "item-b"),
            line("3", true, &dir.path().join("gone"), "item-c"),
            "not json".into(),
        ]
        .join("\n");
        let read = installations(&text);
        assert_eq!(read.len(), 1);
        assert_eq!(read["item-a"], Installation { dir: a, exe: "a.exe".into() });
    }

    #[test]
    fn the_new_launcher_lists_the_owned_games_of_its_library_installed_or_not() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("com.epicgames.eglv2");
        let game = dir.path().join("Kiteborne");
        fs::create_dir_all(&game).unwrap();
        let account = prod(&root).join("account").join("0a1b2c");
        fs::create_dir_all(&account).unwrap();
        let entry = |app: &str, item: &str, title: &str, owned: bool| json!({ "appId": { "namespaceId": format!("ns-{app}"), "itemId": item, "appName": app }, "title": title, "owned": owned });
        let library = json!({ "items": [
            entry("kite", "item-kite", "Kiteborne", true),
            entry("rust", "item-rust", "", true),
            entry("dlc", "item-dlc", "A soundtrack", true),
            entry("lent", "item-lent", "Not owned", false),
        ]});
        fs::write(account.join("library-cache.json"), library.to_string()).unwrap();
        let catalogue = json!({ "items": {
            "item-rust": { "item": { "id": "item-rust", "title": "Rusted Vigil", "categories": [{ "path": "games" }], "releaseInfo": [{ "appId": "rust" }] } },
            "item-dlc": { "item": { "id": "item-dlc", "title": "A soundtrack", "categories": [{ "path": "addons" }], "releaseInfo": [{ "appId": "dlc" }] } },
        }});
        fs::write(prod(&root).join("catalog-cache.json"), catalogue.to_string()).unwrap();
        let install = json!({ "installationId": "x", "complete": true, "dir": game, "itemId": "item-kite", "launchExecutable": "kite.exe" });
        fs::write(root.join(INSTALLATIONS), install.to_string()).unwrap();

        let items = items(&root).unwrap().unwrap();
        let said: Vec<(&str, &str, bool)> =
            items.iter().map(|i| (i.app_name.as_str(), i.title.as_str(), i.install.is_some())).collect();
        assert_eq!(said, [("kite", "Kiteborne", true), ("rust", "Rusted Vigil", false)]);
        assert_eq!(items[0].install, Some((game, "kite.exe".into())));
        assert_eq!(
            crate::link(&items[1], "launch").split('?').next(),
            Some("com.epicgames.launcher://apps/ns-rust%3Aitem-rust%3Arust")
        );
        assert_eq!(super::items(&dir.path().join("nothing")).unwrap(), None);
    }
}
