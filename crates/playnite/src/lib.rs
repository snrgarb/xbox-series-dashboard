//! Import from Playnite, once (docs/import.md): a Playnite 10 library on this PC, read from its own files and never
//! written to, as a [`model::playnite::Plan`] the library database carries out (`library::import`).
//!
//! - [`find`]: where Playnite keeps its library: `%APPDATA%\Playnite\library`, or wherever its `config.json`'s
//!   `DatabasePath` says, or beside Playnite itself when it is portable.
//! - [`read`]: its games (`games.db`) and filter presets (`filterpresets.db`), LiteDB 4 files ([`litedb`]).
//! - [`plan`]: what of it the Dashboard can take: each game under the store and the store's own id it is matched by
//!   (Playnite's library plugin and `GameId`), never its name; and the Bundle's Groups, which are Playnite's filter
//!   presets flagged for Fullscreen's quick selection, where they map onto a Group.
pub mod bson;
pub mod fixture;
pub mod litedb;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use model::playnite::{Group, Marks, Picks, Plan};
use model::{Rule, Sort};

use crate::bson::{Doc, Guid, Value};

/// Playnite's library plugins whose games the Dashboard has a store for, by the plugin's id, with the store's key.
/// The ids are the plugins' own (`PlayniteExtensions`, `source/Libraries/*/…Library.cs`), and each keeps the store's
/// own id for a game in `GameId` as the Dashboard's source does in `source_id` (docs/import.md, "Matching").
pub const PLUGINS: &[(&str, &str)] = &[
    ("cb91dfc9-b977-43bf-8e70-55f46e410fab", "steam"),
    ("00000002-dbd1-46c6-b5d0-b1ba559d10e4", "epic"),
    ("aebe8b7c-6dc3-4a66-af31-e7375c6b5e9e", "gog"),
    ("c2f038e5-8b92-4877-91f1-da9094155fc5", "ubisoft"),
    ("e3c26a3d-d695-4cb7-a769-5ff7612c7edd", "battlenet"),
    ("7e4fbb5e-2ae3-48d4-8ba0-6b30e7a4e287", "xbox"),
];

/// The store a Playnite library plugin is, if the Dashboard reads it.
pub fn store_of(plugin: Guid) -> Option<&'static str> {
    PLUGINS.iter().find(|(id, _)| Guid::parse(id) == Some(plugin)).map(|&(_, key)| key)
}

/// The plugin of a store the Dashboard reads, by the store's key.
pub fn plugin_of(store: &str) -> Option<Guid> {
    PLUGINS.iter().find(|(_, key)| *key == store).and_then(|(id, _)| Guid::parse(id))
}

/// The files of a Playnite library: one LiteDB file a collection.
pub const GAMES: &str = "games.db";
pub const PRESETS: &str = "filterpresets.db";

/// Playnite's library folder: the one `given` names (`--playnite <dir>`: Playnite's own folder, as
/// `%APPDATA%\Playnite`, or the library folder itself), else this user's Playnite's, installed or portable. None if
/// there is no library there.
pub fn find(given: Option<&Path>) -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(dir) = given {
        return library_in(dir, None);
    }
    if !cfg!(windows) {
        return None;
    }
    // Installed: the program in %LOCALAPPDATA%\Playnite, the user's data in %APPDATA%\Playnite. Portable: the
    // data beside the program, wherever it was unpacked; the default place is the only one looked in.
    let program = var("LOCALAPPDATA").map(|dir| dir.join("Playnite"));
    let installed = var("APPDATA").map(|dir| dir.join("Playnite"));
    installed
        .and_then(|dir| library_in(&dir, program.as_deref()))
        .or_else(|| program.as_deref().and_then(|dir| library_in(dir, Some(dir))))
}

/// The library of Playnite's data folder `dir`: where its `config.json` says, else `dir\library`. `program` is
/// Playnite's own folder, which `{PlayniteDir}` in the config stands for.
fn library_in(dir: &Path, program: Option<&Path>) -> Option<PathBuf> {
    let has = |library: &Path| library.join(GAMES).is_file();
    if has(dir) {
        return Some(dir.to_path_buf());
    }
    let configured = configured_library(dir, program.unwrap_or(dir));
    configured.into_iter().chain([dir.join("library")]).find(|library| has(library))
}

/// `DatabasePath` from Playnite's `config.json` in `dir`, with what it stands for filled in.
fn configured_library(dir: &Path, program: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(dir.join("config.json")).ok()?;
    let config: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let path = config.get("DatabasePath")?.as_str()?.trim();
    if path.is_empty() {
        return None;
    }
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let mut expanded = path.to_string();
    for (from, to) in [("%appdata%", appdata.as_str()), ("{playnitedir}", &program.to_string_lossy())] {
        if let Some(at) = expanded.to_ascii_lowercase().find(from) {
            expanded.replace_range(at..at + from.len(), to);
        }
    }
    let expanded = PathBuf::from(expanded.replace('\\', std::path::MAIN_SEPARATOR_STR));
    Some(if expanded.is_absolute() { expanded } else { dir.join(expanded) })
}

/// One of Playnite's games, as much of it as the import reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub id: Guid,
    pub name: String,
    /// The library plugin that brought it: the store.
    pub plugin: Guid,
    /// The store's own id for it.
    pub game_id: String,
    pub playtime_seconds: u64,
    /// Milliseconds since 1970, UTC; none for never.
    pub last_activity: Option<i64>,
    pub favourite: bool,
    pub hidden: bool,
    pub installed: bool,
    /// Its list fields by name (`CategoryIds`), and its one-of fields (`SourceId`, `CompletionStatusId`): what a
    /// filter preset picks by.
    pub fields: HashMap<&'static str, Vec<Guid>>,
}

/// A Playnite filter preset.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: Guid,
    pub name: String,
    /// Shown in Fullscreen's quick selection: a Group of the Bundle's.
    pub flagged: bool,
    /// Its filter, as Playnite keeps it (`FilterPresetSettings`).
    pub settings: Doc,
    /// Its sort, as Playnite names it (`LastActivity`); none for none.
    pub sort: Option<String>,
    pub descending: bool,
}

/// A Playnite library as read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Playnite {
    pub games: Vec<Game>,
    /// In Playnite's own order of them: the order the user gave, then the rest by name.
    pub presets: Vec<Preset>,
}

/// The id fields a filter preset's lists pick by: the preset's field, the game's, and what the page says it is.
const LISTS: &[(&str, &str, bool)] = &[
    ("Genre", "GenreIds", true),
    ("Platform", "PlatformIds", true),
    ("Publisher", "PublisherIds", true),
    ("Developer", "DeveloperIds", true),
    ("Category", "CategoryIds", true),
    ("Tag", "TagIds", true),
    ("Series", "SeriesIds", true),
    ("Region", "RegionIds", true),
    ("AgeRating", "AgeRatingIds", true),
    ("Feature", "FeatureIds", true),
    // One of: the game has a single value.
    ("Source", "SourceId", false),
    ("CompletionStatuses", "CompletionStatusId", false),
];

/// What a preset may filter by that only Playnite can work out, with how a skipped Group says it.
const UNSUPPORTED: &[(&str, &str)] = &[
    ("ReleaseYear", "release year"),
    ("UserScore", "user score"),
    ("CriticScore", "critic score"),
    ("CommunityScore", "community score"),
    ("LastActivity", "last played"),
    ("RecentActivity", "recent activity"),
    ("Added", "date added"),
    ("Modified", "date modified"),
    ("PlayTime", "time played"),
    ("InstallSize", "install size"),
];

/// Reads the Playnite library in `library`: its games, and its filter presets if it has any. Nothing is written,
/// and the files are only ever opened to read.
pub fn read(library: &Path) -> Result<Playnite> {
    for file in [GAMES, PRESETS] {
        // LiteDB 4 puts a page back from its journal the next time the file is opened: a journal left means the
        // file is half way through a write that Playnite has still to undo.
        let journal = library.join(file.replace(".db", "-journal.db"));
        if journal.exists() {
            bail!(
                "Playnite stopped while it was writing its library. Start Playnite and close it again, then try again."
            );
        }
    }
    let open = |name: &str| -> Result<litedb::File> {
        litedb::File::open(&library.join(name)).map_err(|e| {
            let shared = e.downcast_ref::<std::io::Error>().and_then(std::io::Error::raw_os_error) == Some(32);
            if shared {
                anyhow::anyhow!("Playnite has its library open. Close Playnite, then try again.")
            } else {
                e.context(format!("reading Playnite's {name}"))
            }
        })
    };
    let games = open(GAMES)?.documents("Game")?.iter().map(game).collect();
    let presets = if library.join(PRESETS).is_file() {
        let file = open(PRESETS)?;
        let order: Vec<Guid> =
            file.documents("FilterPresetsSettings")?.first().map(|doc| doc.guids("SortingOrder")).unwrap_or_default();
        let mut presets: Vec<Preset> = file.documents("FilterPreset")?.iter().filter_map(preset).collect();
        // Playnite's own order (GameDatabase.GetSortedFilterPresets): those it has a place for in that order, then
        // the rest by name.
        presets.sort_by(|a, b| {
            let place = |p: &Preset| order.iter().position(|&id| id == p.id);
            match (place(a), place(b)) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            }
        });
        presets
    } else {
        Vec::new()
    };
    Ok(Playnite { games, presets })
}

fn game(doc: &Doc) -> Game {
    let mut fields = HashMap::new();
    for &(_, field, list) in LISTS {
        let ids = if list { doc.guids(field) } else { doc.guid(field).into_iter().collect() };
        fields.insert(field, ids);
    }
    Game {
        id: doc.guid("_id").unwrap_or_default(),
        name: doc.str("Name").unwrap_or_default().to_string(),
        plugin: doc.guid("PluginId").unwrap_or_default(),
        game_id: doc.str("GameId").unwrap_or_default().to_string(),
        playtime_seconds: doc.get("Playtime").and_then(Value::as_i64).unwrap_or(0).max(0) as u64,
        last_activity: match doc.get("LastActivity") {
            Some(Value::DateTime(ms)) if *ms > 0 => Some(*ms),
            _ => None,
        },
        favourite: doc.bool("Favorite"),
        hidden: doc.bool("Hidden"),
        installed: doc.bool("IsInstalled"),
        fields,
    }
}

fn preset(doc: &Doc) -> Option<Preset> {
    // An enum is written as its name; as its number by a mapper told to.
    let name_of = |value: Option<&Value>, names: &[(i64, &str)]| -> Option<String> {
        match value? {
            Value::String(text) => Some(text.clone()),
            other => names.iter().find(|(n, _)| Some(*n) == other.as_i64()).map(|(_, name)| name.to_string()),
        }
    };
    let sorts = [(0, "Name"), (4, "LastActivity"), (16, "Playtime"), (21, "Added")];
    Some(Preset {
        id: doc.guid("_id")?,
        name: doc.str("Name").unwrap_or_default().to_string(),
        // The SDK's default is to show it.
        flagged: doc.get("ShowInFullscreeQuickSelection").and_then(Value::as_bool).unwrap_or(true),
        settings: doc.get("Settings").and_then(Value::as_doc).cloned().unwrap_or_default(),
        sort: name_of(doc.get("SortingOrder"), &sorts),
        descending: name_of(doc.get("SortingOrderDirection"), &[(0, "Ascending"), (1, "Descending")]).as_deref()
            == Some("Descending"),
    })
}

/// The store and store id a game is matched by, if it is from a store the Dashboard reads.
fn key_of(game: &Game) -> Option<(String, String)> {
    let store = store_of(game.plugin)?;
    let id = game.game_id.trim();
    (!id.is_empty()).then(|| (store.to_string(), id.to_string()))
}

/// What of `playnite`, read from `from`, an import can bring.
pub fn plan(playnite: &Playnite, from: &Path) -> Plan {
    let mut plan = Plan { from: from.to_path_buf(), games_read: playnite.games.len() as u32, ..Plan::default() };
    for game in &playnite.games {
        let Some((source, source_id)) = key_of(game) else {
            plan.other_stores += 1;
            continue;
        };
        plan.games.push(Marks {
            source,
            source_id,
            name: game.name.clone(),
            playtime_minutes: u32::try_from(game.playtime_seconds / 60).unwrap_or(u32::MAX),
            last_played: game.last_activity.map_or(0, |ms| (ms / 1000).max(0) as u64),
            favourite: game.favourite,
            hidden: game.hidden,
        });
    }
    for preset in playnite.presets.iter().filter(|p| p.flagged) {
        match group(preset, &playnite.games) {
            Ok(group) => plan.groups.push(group),
            Err(why) => plan.skipped.push((preset.name.clone(), why)),
        }
    }
    plan
}

/// A filter preset's list (`Category`): its ids, or its text if it picks by name.
fn list(settings: &Doc, field: &str) -> (Vec<Guid>, Option<String>) {
    let Some(props) = settings.get(field).and_then(Value::as_doc) else { return (Vec::new(), None) };
    let text = props.str("Text").map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    (props.guids("Ids"), text)
}

/// The Group a flagged preset becomes, or why it cannot be one.
fn group(preset: &Preset, games: &[Game]) -> Result<Group, String> {
    let s = &preset.settings;
    for &(field, says) in UNSUPPORTED {
        let values = s.get(field).and_then(Value::as_doc).and_then(|p| p.get("Values")).and_then(Value::as_array);
        if values.is_some_and(|v| !v.is_empty()) {
            return Err(format!("it filters by {says}"));
        }
    }
    for (field, says) in [("Name", "name"), ("Version", "version")] {
        if s.str(field).is_some_and(|text| !text.trim().is_empty()) {
            return Err(format!("it filters by {says}"));
        }
    }
    let (libraries, library_text) = list(s, "Library");
    if library_text.is_some() {
        return Err("it filters by a library's name".into());
    }
    let mut lists = Vec::new();
    for &(field, game_field, is_list) in LISTS {
        let (ids, text) = list(s, field);
        if text.is_some() {
            return Err(format!("it filters by a {}'s name", field.to_lowercase()));
        }
        if !ids.is_empty() {
            lists.push((game_field, is_list, ids));
        }
    }
    let sources: Vec<String> = libraries.iter().filter_map(|&id| store_of(id)).map(str::to_string).collect();
    if !libraries.is_empty() && sources.is_empty() {
        return Err("its libraries are none the Dashboard reads".into());
    }
    let installed = match (s.bool("IsInstalled"), s.bool("IsUnInstalled")) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    };
    let (sort, descending) = match preset.sort.as_deref() {
        Some("LastActivity") => (Sort::LastPlayed, preset.descending),
        Some("Added") => (Sort::Added, preset.descending),
        Some("Playtime") => (Sort::Playtime, preset.descending),
        Some("Name") => (Sort::Name, preset.descending),
        // None, or one the Bundle does not sort by: by name, upward, as the Bundle sorts it.
        _ => (Sort::Name, false),
    };
    let picks = if lists.is_empty() {
        // What a rule can say: the library works it out as the games come and go.
        let favourite = s.bool("Favorite").then_some(true);
        Picks::Rule(Rule { installed, favourite, sources, hidden: s.bool("Hidden"), ..Rule::default() })
    } else {
        // What only Playnite knows of a game: its filter, run over its games now (Playnite's FilterMatcher).
        let and = s.bool("UseAndFilteringStyle");
        let matches = |game: &Game| {
            let installed_ok = installed.is_none_or(|want| game.installed == want);
            let favourite_ok = !s.bool("Favorite") || game.favourite;
            let hidden_ok = s.bool("Hidden") == game.hidden;
            let library_ok = libraries.is_empty() || libraries.contains(&game.plugin);
            let lists_ok = lists.iter().all(|(field, is_list, ids)| {
                let has = game.fields.get(field).map(Vec::as_slice).unwrap_or_default();
                let one = |id: &Guid| {
                    if *id == Guid::EMPTY {
                        // "None": a game with nothing in the field.
                        has.is_empty() || (!is_list && has.iter().all(|h| *h == Guid::EMPTY))
                    } else {
                        has.contains(id)
                    }
                };
                if and && *is_list {
                    ids.iter().all(one)
                } else {
                    ids.iter().any(one)
                }
            });
            installed_ok && favourite_ok && hidden_ok && library_ok && lists_ok
        };
        Picks::Games(games.iter().filter(|g| matches(g)).filter_map(key_of).collect())
    };
    Ok(Group { id: preset.id.to_string(), name: preset.name.trim().to_string(), picks, sort, descending })
}

/// Reads the library at `library` and plans what an import of it brings.
pub fn look(library: &Path) -> Result<Plan> {
    let playnite = read(library).with_context(|| format!("Playnite's library at {}", library.display()))?;
    Ok(plan(&playnite, library))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stores_are_known_by_their_plugins() {
        let steam = Guid::parse("CB91DFC9-B977-43BF-8E70-55F46E410FAB").unwrap();
        assert_eq!(store_of(steam), Some("steam"));
        assert_eq!(plugin_of("steam"), Some(steam));
        assert_eq!(store_of(Guid::EMPTY), None);
        assert!(PLUGINS.iter().all(|(id, _)| Guid::parse(id).is_some()));
    }

    #[test]
    fn the_library_is_found_where_playnite_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("Playnite");
        assert_eq!(find(Some(&data)), None);
        // The library folder in Playnite's data folder.
        std::fs::create_dir_all(data.join("library")).unwrap();
        std::fs::write(data.join("library").join(GAMES), b"").unwrap();
        assert_eq!(find(Some(&data)), Some(data.join("library")));
        // The library folder itself.
        assert_eq!(find(Some(&data.join("library"))), Some(data.join("library")));
        // Where the config says, relative to the data folder.
        std::fs::create_dir_all(data.join("elsewhere")).unwrap();
        std::fs::write(data.join("elsewhere").join(GAMES), b"").unwrap();
        std::fs::write(data.join("config.json"), "\u{feff}{\"DatabasePath\": \"{PlayniteDir}\\\\elsewhere\"}").unwrap();
        assert_eq!(find(Some(&data)), Some(data.join("elsewhere")));
    }
}
