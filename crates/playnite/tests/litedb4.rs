//! The reader against files the real LiteDB 4.1.4 wrote, with Playnite's own models and mapper settings: a made-up
//! library, `tests/litedb4/`, made by `tools/playnite-litedb write` (its `Program.cs` says what is in it). Not a file
//! from anyone's machine.
use std::path::{Path, PathBuf};

use model::playnite::Picks;
use model::{Rule, Sort};
use playnite::bson::{Guid, Value};
use playnite::litedb::File;

fn library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("litedb4")
}

#[test]
fn every_game_litedb_wrote_is_read() {
    let file = File::open(&library().join("games.db")).unwrap();
    assert_eq!(file.collections().collect::<Vec<_>>(), ["Game"]);
    let games = file.documents("Game").unwrap();
    // Five, and sixty fillers less the one deleted.
    assert_eq!(games.len(), 64);
    let named = |name: &str| games.iter().filter(|g| g.str("Name") == Some(name)).collect::<Vec<_>>();
    assert!(named("Filler 07").is_empty());
    assert_eq!(named("Filler 08")[0].get("Playtime"), Some(&Value::Int64(99_999)));

    let kestrel = named("Kestrel Bay");
    let steam = kestrel.iter().find(|g| g.str("GameId") == Some("9200010")).unwrap();
    assert_eq!(steam.guid("_id"), Guid::parse("4a1b2c3d-5e6f-4789-9abc-def012345678"));
    assert_eq!(steam.guid("PluginId"), Guid::parse("cb91dfc9-b977-43bf-8e70-55f46e410fab"));
    assert_eq!(steam.get("Playtime"), Some(&Value::Int64(7260)));
    assert_eq!(steam.get("LastActivity"), Some(&Value::DateTime(1_777_636_800_000)));
    assert!(steam.bool("Favorite") && steam.bool("IsInstalled") && !steam.bool("Hidden"));
    assert_eq!(steam.guids("CategoryIds"), [Guid::parse("11223344-5566-4778-899a-abbccddeeff0").unwrap()]);
    // Playnite's own type for a release date is kept as text.
    assert_eq!(steam.str("ReleaseDate"), Some("2019-5-4"));

    // A document longer than a page, from its extend pages.
    let paper = named("Paper Lighthouse")[0];
    assert_eq!(paper.str("Description").map(str::len), Some("A lighthouse folded from paper. ".len() * 300));
    // A game added by hand has no plugin, and an id of its own that Playnite made up.
    let by_hand = named("By Hand")[0];
    assert_eq!(by_hand.guid("PluginId"), Some(Guid::EMPTY));
    assert!(by_hand.str("GameId").and_then(Guid::parse).is_some());
    assert!(named("Lowland Signal")[0].bool("Hidden"));
}

#[test]
fn the_presets_and_their_order_are_read() {
    let file = File::open(&library().join("filterpresets.db")).unwrap();
    let mut names: Vec<_> = file.collections().collect();
    names.sort();
    assert_eq!(names, ["FilterPreset", "FilterPresetsSettings"]);
    let settings = file.documents("FilterPresetsSettings").unwrap();
    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0].get("_id"), Some(&Value::Int32(0)));

    let playnite = playnite::read(&library()).unwrap();
    let presets: Vec<_> = playnite.presets.iter().map(|p| (p.name.as_str(), p.flagged)).collect();
    assert_eq!(presets, [("Installed Steam", true), ("Couch", true), ("Not flagged", false)]);
    let couch = &playnite.presets[1];
    assert_eq!((couch.sort.as_deref(), couch.descending), (Some("LastActivity"), true));
}

#[test]
fn the_plan_of_a_library_litedb_wrote() {
    let plan = playnite::look(&library()).unwrap();
    assert_eq!((plan.games_read, plan.games.len(), plan.other_stores), (64, 63, 1));
    let kestrel = plan.games.iter().find(|m| m.source_id == "9200010").unwrap();
    assert_eq!((kestrel.source.as_str(), kestrel.playtime_minutes, kestrel.last_played), ("steam", 121, 1_777_636_800));
    assert!(kestrel.favourite);
    // The Epic game of the same name is its own.
    let epic = plan.games.iter().find(|m| m.source == "epic").unwrap();
    assert_eq!((epic.source_id.as_str(), epic.name.as_str(), epic.favourite), ("kestrelbay", "Kestrel Bay", false));

    let groups: Vec<_> = plan.groups.iter().map(|g| (g.name.as_str(), g.picks.clone(), g.sort, g.descending)).collect();
    let installed = Rule { installed: Some(true), sources: vec!["steam".into()], ..Rule::default() };
    assert_eq!(
        groups,
        [
            ("Installed Steam", Picks::Rule(installed), Sort::Name, false),
            ("Couch", Picks::Games(vec![("steam".into(), "9200010".into())]), Sort::LastPlayed, true),
        ]
    );
    assert!(plan.skipped.is_empty());
}
