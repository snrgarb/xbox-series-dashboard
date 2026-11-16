//! A made-up Playnite library, written as Playnite 10 keeps one (LiteDB 4 files, [`crate::litedb::write`]): for the
//! tests here and for the end-to-end tests' `standin make-playnite`. Its Steam games are the fake Steam's
//! (`source_steam::fixture::Fixture::standard`, appids 9100010 on, ten apart), so that an import has games in common:
//!
//! | Playnite's game | Store, id | What it says | What it is for |
//! |---|---|---|---|
//! | Dune Cartographer | Steam 9100080 | 12 h 34 min, played two hours after the fake Steam's newest play; Couch | play time and last played arrive: first in the Recent row |
//! | Fathom Nine | Steam 9100100 | a favourite; Couch | a favourite arrives |
//! | Hollow Pines Motel | Steam 9100120 | hidden | a hidden game arrives |
//! | Ashfall Courier | Steam 9100010 | 25 h, less than Steam's own count | nothing goes down |
//! | Brass Meridian | Steam 9100030 | installed | |
//! | Ghost Pier | Steam 9109990 | 5 h | a game the library does not have: left |
//! | Ashfall Courier | Epic `ashfallcourier` | 100 h, a favourite; Couch | the same name in another store: never matched by name |
//! | Pinball Shed | added by hand | 2 h | a game of no store: left |
//!
//! Its presets, in Playnite's order: **Couch picks** (flagged; by category: Dune Cartographer and Fathom Nine, the
//! Epic game being no game of the library's), **Installed on Steam** (flagged; installed, from Steam: a rule),
//! **Released in 2019** (flagged; by release year: cannot come over), and **Everything hidden** (not flagged: not a
//! Group).
use std::path::Path;

use anyhow::{Context, Result};

use crate::bson::{Doc, Guid, Value};
use crate::{litedb, plugin_of};

/// The fake Steam's newest play (`source_steam::fixture::NEWEST_PLAY`).
pub const NEWEST_PLAY: u64 = 1_790_000_000;
pub const COUCH: &str = "5c0cc0a1-7e57-4d0e-9a11-c0c4c4c4c4c4";
pub const COUCH_PICKS: &str = "c0c4b1c5-0001-4000-8000-000000000001";
pub const INSTALLED_ON_STEAM: &str = "c0c4b1c5-0002-4000-8000-000000000002";
pub const RELEASED_2019: &str = "c0c4b1c5-0003-4000-8000-000000000003";
pub const EVERYTHING_HIDDEN: &str = "c0c4b1c5-0004-4000-8000-000000000004";

/// One made-up game.
struct Made {
    name: &'static str,
    store: Option<&'static str>,
    game_id: &'static str,
    playtime_seconds: i64,
    last_activity: Option<u64>,
    favourite: bool,
    hidden: bool,
    installed: bool,
    couch: bool,
}

fn games() -> Vec<Made> {
    let made = |name, store, game_id| Made {
        name,
        store,
        game_id,
        playtime_seconds: 0,
        last_activity: None,
        favourite: false,
        hidden: false,
        installed: false,
        couch: false,
    };
    let steam = Some("steam");
    vec![
        Made {
            playtime_seconds: (12 * 60 + 34) * 60,
            last_activity: Some(NEWEST_PLAY + 2 * 3600),
            couch: true,
            ..made("Dune Cartographer", steam, "9100080")
        },
        Made { favourite: true, couch: true, ..made("Fathom Nine", steam, "9100100") },
        Made { hidden: true, ..made("Hollow Pines Motel", steam, "9100120") },
        Made {
            playtime_seconds: 25 * 3600,
            last_activity: Some(NEWEST_PLAY - 30 * 86_400),
            installed: true,
            ..made("Ashfall Courier", steam, "9100010")
        },
        Made { installed: true, ..made("Brass Meridian", steam, "9100030") },
        Made { playtime_seconds: 5 * 3600, ..made("Ghost Pier", steam, "9109990") },
        Made {
            playtime_seconds: 100 * 3600,
            favourite: true,
            couch: true,
            ..made("Ashfall Courier", Some("epic"), "ashfallcourier")
        },
        Made { playtime_seconds: 2 * 3600, ..made("Pinball Shed", None, "") },
    ]
}

fn id(n: u32) -> Value {
    Value::Guid(Guid::parse(&format!("{n:08x}-9a3e-4b1c-8d2f-0123456789ab")).expect("a Guid"))
}

fn guid(text: &str) -> Value {
    Value::Guid(Guid::parse(text).expect("a Guid"))
}

/// A game as Playnite's mapper writes one (`tools/playnite-litedb`'s dump of a real one): every bool, the play
/// time as an Int64, the dates only when set, the lists only when they have something.
fn game_doc(n: u32, game: &Made) -> Doc {
    let plugin = game.store.and_then(plugin_of).unwrap_or(Guid::EMPTY);
    let mut doc = Doc::default()
        .with("_id", id(n))
        .with("EnableSystemHdr", Value::Bool(false))
        .with("Hidden", Value::Bool(game.hidden))
        .with("Favorite", Value::Bool(game.favourite));
    if !game.game_id.is_empty() {
        doc = doc.with("GameId", Value::String(game.game_id.into()));
    }
    doc = doc.with("PluginId", Value::Guid(plugin)).with("IncludeLibraryPluginAction", Value::Bool(true));
    if let Some(when) = game.last_activity {
        doc = doc.with("LastActivity", Value::DateTime(when as i64 * 1000));
    }
    if game.couch {
        doc = doc.with("CategoryIds", Value::Array(vec![guid(COUCH)]));
    }
    doc.with("IsInstalled", Value::Bool(game.installed))
        .with("Playtime", Value::Int64(game.playtime_seconds))
        .with("Added", Value::DateTime((NEWEST_PLAY as i64 - 400 * 86_400) * 1000))
        .with("PlayCount", Value::Int64(i64::from(game.playtime_seconds > 0)))
        .with("SourceId", Value::Guid(Guid::EMPTY))
        .with("CompletionStatusId", Value::Guid(Guid::EMPTY))
        .with("Name", Value::String(game.name.into()))
}

/// A preset's filter: every bool, as Playnite writes them, and these fields besides.
fn settings(fields: Vec<(&str, Value)>) -> Value {
    let mut doc = Doc::default();
    for flag in ["UseAndFilteringStyle", "IsInstalled", "IsUnInstalled", "Hidden", "Favorite"] {
        let on = fields.iter().any(|(name, value)| *name == flag && *value == Value::Bool(true));
        doc = doc.with(flag, Value::Bool(on));
    }
    for (name, value) in fields.into_iter().filter(|(_, value)| !matches!(value, Value::Bool(_))) {
        doc = doc.with(name, value);
    }
    Value::Document(doc)
}

fn ids(of: &[Value]) -> Value {
    Value::Document(Doc::default().with("Ids", Value::Array(of.to_vec())))
}

fn preset(id: &str, name: &str, flagged: bool, sort: Option<(&str, &str)>, settings: Value) -> Doc {
    let mut doc = Doc::default().with("_id", guid(id)).with("Settings", settings);
    if let Some((sort, direction)) = sort {
        doc = doc
            .with("SortingOrder", Value::String(sort.into()))
            .with("SortingOrderDirection", Value::String(direction.into()));
    }
    doc.with("ShowInFullscreeQuickSelection", Value::Bool(flagged)).with("Name", Value::String(name.into()))
}

/// Writes the made-up library as Playnite's data folder `dir`: `dir\library\games.db` and `filterpresets.db`.
/// Gives the library folder.
pub fn write(dir: &Path) -> Result<std::path::PathBuf> {
    let library = dir.join("library");
    std::fs::create_dir_all(&library).with_context(|| format!("making {}", library.display()))?;
    let games: Vec<Doc> = games().iter().enumerate().map(|(n, game)| game_doc(n as u32 + 1, game)).collect();
    std::fs::write(library.join(crate::GAMES), litedb::write(&[("Game", games)])?)?;

    let steam = Value::Guid(plugin_of("steam").expect("Steam's plugin"));
    let presets = vec![
        preset(INSTALLED_ON_STEAM, "Installed on Steam", true, Some(("Name", "Ascending")), {
            settings(vec![("IsInstalled", Value::Bool(true)), ("Library", ids(&[steam]))])
        }),
        preset(COUCH_PICKS, "Couch picks", true, Some(("LastActivity", "Descending")), {
            settings(vec![("Category", ids(&[guid(COUCH)]))])
        }),
        preset(RELEASED_2019, "Released in 2019", true, None, {
            let years = Value::Document(Doc::default().with("Values", Value::Array(vec![Value::String("2019".into())])));
            settings(vec![("ReleaseYear", years)])
        }),
        preset(EVERYTHING_HIDDEN, "Everything hidden", false, None, settings(vec![("Hidden", Value::Bool(true))])),
    ];
    // The order the user gave them in Playnite: Couch picks first. The preset not flagged has no place, and comes
    // after.
    let order = Doc::default()
        .with("_id", Value::Int32(0))
        .with("SortingOrder", Value::Array(vec![guid(COUCH_PICKS), guid(INSTALLED_ON_STEAM), guid(RELEASED_2019)]));
    let file = litedb::write(&[("FilterPreset", presets), ("FilterPresetsSettings", vec![order])])?;
    std::fs::write(library.join(crate::PRESETS), file)?;
    Ok(library)
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::playnite::Picks;
    use model::Sort;

    #[test]
    fn the_made_up_library_reads_back_as_it_was_made() {
        let dir = tempfile::tempdir().unwrap();
        let library = write(dir.path()).unwrap();
        assert_eq!(crate::find(Some(dir.path())), Some(library.clone()));
        let playnite = crate::read(&library).unwrap();
        assert_eq!(playnite.games.len(), 8);
        let dune = playnite.games.iter().find(|g| g.name == "Dune Cartographer").unwrap();
        assert_eq!((dune.game_id.as_str(), dune.playtime_seconds), ("9100080", 45_240));
        assert_eq!(dune.last_activity, Some((NEWEST_PLAY as i64 + 7200) * 1000));
        let names: Vec<_> = playnite.presets.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Couch picks", "Installed on Steam", "Released in 2019", "Everything hidden"]);

        let plan = crate::plan(&playnite, &library);
        assert_eq!((plan.games_read, plan.games.len(), plan.other_stores), (8, 7, 1));
        let dune = plan.games.iter().find(|m| m.source_id == "9100080").unwrap();
        assert_eq!((dune.source.as_str(), dune.playtime_minutes, dune.last_played), ("steam", 754, NEWEST_PLAY + 7200));
        let groups: Vec<_> = plan.groups.iter().map(|g| (g.name.as_str(), &g.picks, g.sort, g.descending)).collect();
        let couch = Picks::Games(vec![
            ("steam".into(), "9100080".into()),
            ("steam".into(), "9100100".into()),
            ("epic".into(), "ashfallcourier".into()),
        ]);
        let installed = Picks::Rule(model::Rule {
            installed: Some(true),
            sources: vec!["steam".into()],
            ..model::Rule::default()
        });
        assert_eq!(
            groups,
            [("Couch picks", &couch, Sort::LastPlayed, true), ("Installed on Steam", &installed, Sort::Name, false)]
        );
        assert_eq!(plan.skipped, [("Released in 2019".to_string(), "it filters by release year".to_string())]);
    }
}
