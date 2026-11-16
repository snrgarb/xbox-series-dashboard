//! The user's Steam collections, from the client's own copy of what it keeps in Steam Cloud:
//! `userdata/<id>/config/cloudstorage/cloud-storage-namespace-1.json`.
//!
//! The file is a JSON array of `[key, entry]` pairs, of which a collection's key is `user-collections.<id>`. Its
//! entry has `is_deleted` set for a collection the user removed, and otherwise a `value`: a string that is itself
//! JSON, `{"id": "uc-...", "name": "...", "added": [appid, ...], "removed": [appid, ...]}`. Two ids are Steam's own,
//! `favorite` and `hidden`. A dynamic collection has a `filterSpec` there too: Steam works its games out in the
//! client and stores only the ones the user added or took out by hand.
//!
//! This is the format as Steam's client is known to write it, not one read from a real install: the tests' copy of
//! the user's Steam files has no `cloudstorage` folder (docs/spec/filters.md, section 5.1). So the reader takes
//! what it understands and passes over the rest, and a file it cannot make sense of at all reads as no answer,
//! which leaves the library's collections as they were.
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use model::Collection;
use serde_json::Value;

/// Under the user's `config` folder.
pub const FOLDER: &str = "cloudstorage";
pub const FILE: &str = "cloud-storage-namespace-1.json";

const KEY_PREFIX: &str = "user-collections.";
/// Steam's own two collections, by their ids.
const FAVOURITES: &str = "favorite";
const HIDDEN: &str = "hidden";

/// What the file says of the user's games.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Collections {
    /// The collections the user made and filled by hand, by name: none deleted, none dynamic.
    pub collections: Vec<Collection>,
    /// The games in Steam's Favourites.
    pub favourites: HashSet<u32>,
    /// The games hidden in Steam.
    pub hidden: HashSet<u32>,
}

/// The collections in the user's `config` folder; an error where there is no file or it is not what is expected.
pub fn load(config: &Path) -> Result<Collections> {
    let path = crate::child(config, FOLDER).and_then(|folder| crate::child(&folder, FILE));
    let path = path.with_context(|| format!("no {FOLDER}/{FILE} in {}", config.display()))?;
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    parse(&text).with_context(|| format!("reading {}", path.display()))
}

pub fn parse(text: &str) -> Result<Collections> {
    let file: Value = serde_json::from_str(text)?;
    let pairs = file.as_array().context("not a list of keys and entries")?;
    let mut out = Collections::default();
    for pair in pairs {
        let (Some(key), Some(entry)) = (pair.get(0).and_then(Value::as_str), pair.get(1)) else { continue };
        let Some(id) = key.strip_prefix(KEY_PREFIX) else { continue };
        if entry.get("is_deleted").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        // The value is JSON in a string; an object as it stands is taken too.
        let value = match entry.get("value") {
            Some(Value::String(text)) => serde_json::from_str(text).unwrap_or(Value::Null),
            Some(value) => value.clone(),
            None => Value::Null,
        };
        if !value.is_object() {
            continue;
        }
        let appids = || value.get("added").and_then(Value::as_array).into_iter().flatten().filter_map(appid);
        match id {
            FAVOURITES => out.favourites.extend(appids()),
            HIDDEN => out.hidden.extend(appids()),
            // A dynamic collection: its games are not in the file.
            _ if value.get("filterSpec").is_some_and(|spec| !spec.is_null()) => {}
            _ => {
                let name = value.get("name").and_then(Value::as_str).unwrap_or("").trim();
                if name.is_empty() {
                    continue;
                }
                let mut appids: Vec<u32> = appids().collect();
                appids.sort_unstable();
                appids.dedup();
                let games = appids.iter().map(u32::to_string).collect();
                out.collections.push(Collection { source_id: id.to_string(), name: name.to_string(), games });
            }
        }
    }
    // As Steam lists them: by name. It is the order they are first given a place among the Groups in.
    out.collections.sort_by(|a, b| {
        (a.name.to_lowercase(), &a.name, &a.source_id).cmp(&(b.name.to_lowercase(), &b.name, &b.source_id))
    });
    Ok(out)
}

/// An appid as the file has it: a number, or (in case a client writes one so) a number in a string.
fn appid(value: &Value) -> Option<u32> {
    match value {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, value: &str) -> String {
        format!(
            r#"["user-collections.{id}", {{"key": "user-collections.{id}", "timestamp": 1790000000, "value": {}, "version": "7"}}]"#,
            serde_json::to_string(value).unwrap()
        )
    }

    #[test]
    fn collections_are_read_with_steams_own_two_apart() {
        let file = format!(
            "[{}, {}, {}, {}]",
            entry("uc-Bb22", r#"{"id":"uc-Bb22","name":"Rainy days","added":[30,10,30],"removed":[]}"#),
            entry("favorite", r#"{"id":"favorite","name":"Favorites","added":[10],"removed":[20]}"#),
            entry("hidden", r#"{"id":"hidden","name":"Hidden","added":[40]}"#),
            entry("uc-Aa11", r#"{"id":"uc-Aa11","name":"couch co-op","added":["20"]}"#),
        );
        let read = parse(&file).unwrap();
        let names: Vec<_> = read.collections.iter().map(|c| (c.source_id.as_str(), c.name.as_str())).collect();
        assert_eq!(names, [("uc-Aa11", "couch co-op"), ("uc-Bb22", "Rainy days")]);
        assert_eq!(read.collections[0].games, ["20"]);
        assert_eq!(read.collections[1].games, ["10", "30"]);
        assert_eq!(read.favourites, HashSet::from([10]));
        assert_eq!(read.hidden, HashSet::from([40]));
    }

    #[test]
    fn deleted_and_dynamic_collections_and_other_keys_are_passed_over() {
        let file = format!(
            r#"[{},
                ["user-collections.uc-Gone", {{"key": "user-collections.uc-Gone", "timestamp": 1, "is_deleted": true}}],
                ["showcase.pins", {{"key": "showcase.pins", "value": "{{\"name\":\"not a collection\",\"added\":[1]}}"}}],
                ["user-collections.uc-Odd", {{"value": "not json"}}],
                ["user-collections.uc-Bare", {{"value": {{"id": "uc-Bare", "name": "As an object", "added": [5]}}}}],
                ["user-collections.uc-Anon", {{"value": "{{\"id\":\"uc-Anon\",\"name\":\" \",\"added\":[5]}}"}}],
                "neither a pair", [1, 2]]"#,
            entry("uc-Dyn", r#"{"id":"uc-Dyn","name":"Roguelikes","added":[10],"filterSpec":{"nFormatVersion":2}}"#),
        );
        let read = parse(&file).unwrap();
        assert_eq!(
            read.collections,
            [Collection { source_id: "uc-Bare".into(), name: "As an object".into(), games: vec!["5".into()] }]
        );
    }

    #[test]
    fn a_file_that_is_not_the_list_expected_is_an_error_and_an_empty_one_is_no_collections() {
        assert!(parse("{}").is_err());
        assert!(parse("[[").is_err());
        assert_eq!(parse("[]").unwrap(), Collections::default());
    }
}
