//! The made-up Playnite library (`playnite::fixture`, what `standin make-playnite` writes) imported into a library of
//! the fake Steam (`source_steam::fixture`), as the end-to-end test does it on Windows (e2e/payloads/playnite-import).
use library::Database;
use model::playnite::Preview;
use source_steam::fixture::{self, Fixture};

const NOW: u64 = 1_800_000_000;

fn names(games: &[i64], db: &Database) -> Vec<String> {
    games.iter().map(|&id| db.game(id).unwrap().unwrap().name).collect()
}

#[test]
fn the_fake_playnite_imports_into_the_fake_steam() {
    let dir = tempfile::tempdir().unwrap();
    fixture::write(&dir.path().join("steam"), &Fixture::standard()).unwrap();
    let mut db = Database::open(&dir.path().join("data").join(library::FILE)).unwrap();
    db.sync("steam", &source_steam::load(&dir.path().join("steam")).unwrap(), NOW).unwrap();

    let library = playnite::fixture::write(&dir.path().join("Playnite")).unwrap();
    let found = playnite::find(Some(&dir.path().join("Playnite"))).unwrap();
    assert_eq!(found, library);
    let plan = playnite::look(&found).unwrap();
    let before = db.snapshot_at(NOW).unwrap();
    let preview = db.import_preview(&plan).unwrap();
    assert_eq!(db.snapshot_at(NOW).unwrap(), before, "a preview changes nothing");
    assert_eq!(
        preview,
        Preview {
            from: found.display().to_string(),
            games_read: 8,
            matched: 5,
            playtime: 1,
            last_played: 1,
            favourites: 1,
            hidden: 1,
            groups: vec!["Couch picks".into(), "Installed on Steam".into()],
            // Ghost Pier, which the fake Steam has not; and the Epic Ashfall Courier, there being no Epic.
            not_here: 2,
            other_stores: 1,
            skipped: vec![("Released in 2019".into(), "it filters by release year".into())],
        }
    );

    let imported = db.import(&plan, NOW).unwrap();
    assert_eq!(imported.preview, preview);
    let snapshot = db.snapshot_at(NOW).unwrap();
    // Dune Cartographer, never played in Steam, has Playnite's 12 h 34 min and is the latest played.
    let dune = &snapshot.games[0];
    assert_eq!((dune.name.as_str(), dune.playtime_minutes), ("Dune Cartographer", 754));
    assert!(snapshot.games.iter().any(|g| g.name == "Fathom Nine" && g.favourite));
    assert_eq!(snapshot.hidden.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["Hollow Pines Motel"]);
    // Steam's own Ashfall Courier keeps Steam's count: the Epic game of that name is not it.
    let ashfall = snapshot.games.iter().find(|g| g.name == "Ashfall Courier").unwrap();
    assert!(!ashfall.favourite);
    assert_eq!(ashfall.playtime_minutes, Fixture::standard().games[0].playtime_minutes);

    let filters = &snapshot.filters;
    let couch = filters.iter().find(|f| f.name == "Couch picks").unwrap();
    assert!(couch.shown);
    assert_eq!(names(&couch.games, &db), ["Dune Cartographer", "Fathom Nine"]);
    let installed = filters.iter().find(|f| f.name == "Installed on Steam").unwrap();
    assert!(names(&installed.games, &db).contains(&"Brass Meridian".to_string()));
    // After every Group there was, in Playnite's order.
    let last: Vec<_> = filters.iter().rev().take(2).map(|f| f.name.as_str()).collect();
    assert_eq!(last, ["Installed on Steam", "Couch picks"]);

    // Again: nothing new.
    let again = db.import(&plan, NOW + 60).unwrap();
    assert!(again.preview.is_empty() && again.lines.is_empty(), "{again:?}");
    assert_eq!(db.snapshot_at(NOW).unwrap(), snapshot);
}
