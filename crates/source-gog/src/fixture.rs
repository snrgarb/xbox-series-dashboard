//! Writes a fake GOG: made-up games installed under one folder, each with its `goggame-<id>.info`, and a GOG Galaxy
//! database with only the tables and columns `galaxy` reads, with generated art in Galaxy's picture cache. Test
//! support, for the unit tests here and for the end-to-end tests (`crates/standin`), which write it on a test VM
//! that has no GOG.
//!
//! The folder is laid out as `Places::under` reads it: `<root>` is `%ProgramData%\GOG.com`, and the games are the
//! folders under `<root>\Games`, which stand for the registry's list. No Galaxy program: a GOG game is started as
//! its own program.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::{ImageFormat, Rgb, RgbImage};
use rusqlite::{params, Connection};
use serde_json::json;

use crate::Places;

/// Every installed game's program, in its folder: its primary play task.
pub const EXE: &str = "game.exe";

/// A fake GOG to write.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The Galaxy user whose picture cache the art is in.
    pub user_id: u64,
    pub games: Vec<FixtureGame>,
    /// A program copied into every installed game's folder as `EXE`. Without one the folders hold only their
    /// `.info` files.
    pub game_exe: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct FixtureGame {
    /// The product id: the game's `source_id`.
    pub id: String,
    pub name: String,
    /// In its folder under `Games`; else only in Galaxy's database, which is owned and not installed.
    pub installed: bool,
    /// Galaxy's play time, in minutes; 0 for none.
    pub minutes: u32,
    /// Galaxy's last played, as it writes it (`2023-11-14 22:13:20`); empty for never.
    pub last_played: String,
    /// A cover and a background in Galaxy's picture cache.
    pub art: bool,
}

impl FixtureGame {
    /// Where an installed game is: `<root>\Games\<name>`.
    pub fn install_dir(&self, root: &Path) -> PathBuf {
        root.join("Games").join(&self.name)
    }
}

impl Fixture {
    /// Three made-up games:
    ///
    /// | game            | id         | installed | played                  | art |
    /// |-----------------|------------|-----------|-------------------------|-----|
    /// | Juniper Signal  | 1900000101 | yes       | 2 h, on 14 Nov 2023     | yes |
    /// | Owlglass Abbey  | 1900000102 | yes       | never                   | no  |
    /// | Pewter Isles    | 1900000103 | no        | never                   | yes |
    ///
    /// Juniper Signal's folder also holds the `.info` of its DLC, and the database lists a game of another store
    /// Galaxy was connected to: `load` leaves both out.
    pub fn standard() -> Fixture {
        let game = |id: &str, name: &str, installed, minutes, last_played: &str, art| FixtureGame {
            id: id.into(),
            name: name.into(),
            installed,
            minutes,
            last_played: last_played.into(),
            art,
        };
        Fixture {
            user_id: 48_151_623_420_001,
            games: vec![
                game("1900000101", "Juniper Signal", true, 120, "2023-11-14 22:13:20", true),
                game("1900000102", "Owlglass Abbey", true, 0, "", false),
                game("1900000103", "Pewter Isles", false, 0, "", true),
            ],
            game_exe: None,
        }
    }
}

/// The DLC written in the first installed game's folder.
const DLC: &str = "1900000199";

/// Writes `spec` at `root`, making the folders. What is already there is written over.
pub fn write(root: &Path, spec: &Fixture) -> io::Result<()> {
    let root = std::path::absolute(root)?;
    fs::create_dir_all(root.join("Games"))?;
    let mut dlc_written = false;
    for game in spec.games.iter().filter(|game| game.installed) {
        let dir = game.install_dir(&root);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(format!("goggame-{}.info", game.id)), info(game).to_string())?;
        if !dlc_written {
            let dlc = json!({ "gameId": DLC, "rootGameId": game.id, "name": format!("{}: Soundtrack", game.name), "playTasks": [] });
            fs::write(dir.join(format!("goggame-{DLC}.info")), dlc.to_string())?;
            dlc_written = true;
        }
        if let Some(from) = &spec.game_exe {
            fs::copy(from, dir.join(EXE))?;
        }
    }
    database(&root, spec).map_err(io::Error::other)?;
    for game in spec.games.iter().filter(|game| game.art) {
        let dir = art_dir(&root, spec, game);
        fs::create_dir_all(&dir)?;
        for (file, image) in art(game) {
            image.save_with_format(dir.join(file), ImageFormat::WebP).map_err(io::Error::other)?;
        }
    }
    Ok(())
}

/// The game's `goggame-<id>.info`, as the sample in docs/research/stores.md.
fn info(game: &FixtureGame) -> serde_json::Value {
    json!({
        "gameId": game.id,
        "rootGameId": game.id,
        "name": game.name,
        "language": "English",
        "playTasks": [
            { "isPrimary": true, "type": "FileTask", "path": EXE, "workingDir": "", "arguments": "", "name": game.name },
            { "isPrimary": false, "type": "URLTask", "link": "https://example.invalid/manual", "name": "Manual" },
        ],
    })
}

/// Galaxy's database, with every game of `spec` owned and the game of another store.
fn database(root: &Path, spec: &Fixture) -> rusqlite::Result<()> {
    let file = Places::under(root).database();
    fs::create_dir_all(file.parent().expect("the database is in a folder"))
        .map_err(|e| rusqlite::Error::InvalidPath(PathBuf::from(format!("{}: {e}", file.display()))))?;
    let _ = fs::remove_file(&file);
    let db = Connection::open(&file)?;
    db.execute_batch(
        "CREATE TABLE GamePieceTypes (id INTEGER PRIMARY KEY, type TEXT);
         CREATE TABLE GamePieces (releaseKey TEXT, gamePieceTypeId INTEGER, value TEXT);
         CREATE TABLE ProductPurchaseDates (gameReleaseKey TEXT, purchaseDate TEXT);
         CREATE TABLE GameTimes (releaseKey TEXT, minutesInGame INTEGER);
         CREATE TABLE LastPlayedDates (gameReleaseKey TEXT, lastPlayedDate TEXT);
         CREATE TABLE WebCache (id INTEGER PRIMARY KEY, releaseKey TEXT, userId INTEGER);
         CREATE TABLE WebCacheResourceTypes (id INTEGER PRIMARY KEY, type TEXT);
         CREATE TABLE WebCacheResources (webCacheId INTEGER, webCacheResourceTypeId INTEGER, filename TEXT);
         INSERT INTO GamePieceTypes VALUES (12, 'title'), (17, 'originalImages');
         INSERT INTO WebCacheResourceTypes VALUES (3, 'squareIcon'), (5, 'verticalCover'), (8, 'background');
         INSERT INTO ProductPurchaseDates VALUES ('steam_9100010', '2024-02-01 09:00:00');
         INSERT INTO GamePieces VALUES ('steam_9100010', 12, '{\"title\":\"Of another store\"}');",
    )?;
    for (i, game) in spec.games.iter().enumerate() {
        let key = format!("gog_{}", game.id);
        db.execute("INSERT INTO ProductPurchaseDates VALUES (?1, '2024-06-01 10:00:00')", params![key])?;
        db.execute(
            "INSERT INTO GamePieces VALUES (?1, 12, ?2)",
            params![key, json!({ "title": game.name }).to_string()],
        )?;
        if game.minutes > 0 {
            db.execute("INSERT INTO GameTimes VALUES (?1, ?2)", params![key, game.minutes])?;
        }
        if !game.last_played.is_empty() {
            db.execute("INSERT INTO LastPlayedDates VALUES (?1, ?2)", params![key, game.last_played])?;
        }
        if game.art {
            let cache = i as i64 + 1;
            db.execute("INSERT INTO WebCache VALUES (?1, ?2, ?3)", params![cache, key, spec.user_id as i64])?;
            for (file, kind) in [(COVER, 5), (BACKGROUND, 8)] {
                db.execute("INSERT INTO WebCacheResources VALUES (?1, ?2, ?3)", params![cache, kind, file])?;
            }
        }
    }
    Ok(())
}

const COVER: &str = "vertical_cover.webp";
const BACKGROUND: &str = "background.webp";

/// Where Galaxy caches the game's pictures: `Galaxy\webcache\<user id>\gog\<product id>`.
fn art_dir(root: &Path, spec: &Fixture, game: &FixtureGame) -> PathBuf {
    root.join("Galaxy").join("webcache").join(spec.user_id.to_string()).join("gog").join(&game.id)
}

/// A 342x482 cover, top to bottom, and a 960x540 background, left to right: each a gradient of the game's own
/// colour, which is unlike any of Steam's fixture (a cool violet to a warm amber, by the id).
fn art(game: &FixtureGame) -> [(&'static str, RgbImage); 2] {
    let n = game.id.bytes().fold(0u32, |n, b| n.wrapping_mul(31).wrapping_add(b as u32));
    let top = [120 + (n % 100) as u8, 60 + (n / 7 % 60) as u8, 200 - (n / 13 % 120) as u8];
    let shade = top.map(|c| c / 4);
    let mix = |t: f32| Rgb(std::array::from_fn(|i| (top[i] as f32 + (shade[i] as f32 - top[i] as f32) * t) as u8));
    [
        (COVER, RgbImage::from_fn(342, 482, |_, y| mix(y as f32 / 481.0))),
        (BACKGROUND, RgbImage::from_fn(960, 540, |x, _| mix(1.0 - x as f32 / 959.0))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_fixture_reads_back_with_its_program_in_its_installed_games() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("standin");
        fs::write(&exe, b"not really a program").unwrap();
        let mut spec = Fixture::standard();
        spec.game_exe = Some(exe);
        let root = dir.path().join("gog");
        write(&root, &spec).unwrap();
        let places = Places::under(&root);
        let library = crate::load(&places, places.records().unwrap()).unwrap();
        let said: Vec<(&str, bool, u32)> =
            library.games.iter().map(|g| (g.name.as_str(), g.installed, g.playtime_minutes)).collect();
        // The played one first, then by name.
        assert_eq!(said, [("Juniper Signal", true, 120), ("Owlglass Abbey", true, 0), ("Pewter Isles", false, 0)]);
        assert_eq!(library.games[0].last_played, 1_700_000_000);
        let juniper = &spec.games[0];
        assert_eq!(library.games[0].install_dir.as_deref(), Some(juniper.install_dir(&root).as_path()));
        assert_eq!(fs::read(juniper.install_dir(&root).join(EXE)).unwrap(), b"not really a program");
        // Art where Galaxy cached it, and it decodes.
        for game in [&library.games[0], &library.games[2]] {
            for art in [game.cover.as_ref().unwrap(), game.hero.as_ref().unwrap()] {
                let image = image::ImageReader::open(art).unwrap().with_guessed_format().unwrap();
                assert_eq!(image.format(), Some(ImageFormat::WebP));
                image.decode().unwrap();
            }
        }
        assert_eq!((library.games[1].cover.as_ref(), library.games[1].hero.as_ref()), (None, None));
        // Written again over itself, it reads the same.
        write(&root, &spec).unwrap();
        assert_eq!(crate::load(&places, places.records().unwrap()).unwrap(), library);
    }
}
