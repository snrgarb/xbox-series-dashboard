//! Writes a fake Steam install: the files `load` reads, in the formats a current Steam client writes, with made-up
//! games and generated art. Test support, for the unit tests here and for the end-to-end tests' stand-in Steam
//! (`crates/standin`), which runs it on a test VM that has no Steam.
//!
//! Besides the fixture's games it writes three apps `load` must leave out: a tool and a piece of DLC the account's
//! packages grant, and a game only package 0 grants, which everyone "has" and nobody owns.
//!
//! The collections file is written as `collections` reads it. That format is from knowledge of Steam's client and
//! has not been set against a real file (docs/spec/filters.md, section 5.1); every collection in it is made up.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::{ImageFormat, Rgb, RgbImage};

/// A fake Steam install to write.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The signed-in user's persona name.
    pub persona: String,
    /// The user's folder under `userdata`.
    pub user_id: u64,
    pub games: Vec<FixtureGame>,
    /// A program copied into every installed game's folder as `game.exe`. Without one the folders are empty.
    pub game_exe: Option<PathBuf>,
    /// The user's collections file. `None` writes none, as for an install whose client has never synced any.
    pub collections: Option<FixtureCollections>,
}

/// What `userdata/<id>/config/cloudstorage/cloud-storage-namespace-1.json` says.
#[derive(Debug, Clone, Default)]
pub struct FixtureCollections {
    pub collections: Vec<FixtureCollection>,
    /// The appids in Steam's own Favourites.
    pub favourites: Vec<u32>,
    /// The appids hidden in Steam.
    pub hidden: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct FixtureCollection {
    /// Steam's id for it: `uc-` and twelve characters.
    pub id: String,
    pub name: String,
    pub appids: Vec<u32>,
    pub kind: CollectionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionKind {
    /// Filled by the user, game by game: the only kind `load` gives.
    ByHand,
    /// Worked out by Steam from a filter; `appids` are the games the user added to it by hand.
    Dynamic,
    /// Removed by the user: its key is still in the file, marked deleted, with no value.
    Deleted,
}

#[derive(Debug, Clone)]
pub struct FixtureGame {
    pub appid: u32,
    pub name: String,
    /// The game's folder under `steamapps/common`, written only for an installed game.
    pub install_dir: String,
    pub installed: bool,
    /// Unix seconds; 0 = never played.
    pub last_played: u64,
    pub playtime_minutes: u32,
    pub art: Art,
    /// Who made it and who published it, when it came out and what it scores: what appinfo may say of a game. All
    /// made up.
    pub facts: Facts,
    /// What the manifest of an installed game says it takes on disk, in bytes.
    pub size_on_disk: u64,
}

/// What `appinfo.vdf` says of a game beyond its name and art. Whatever is empty, 0 or `None` is left out of the
/// file, as Steam leaves it out for a game it has not cached it for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    /// How they are written: as `common/associations`, the current way, or as `extended/developer` and
    /// `extended/publisher`, which is all an older entry has and holds one name each.
    pub credits: Credits,
    /// `common/steam_release_date`, in Unix seconds.
    pub released: u64,
    /// `common/original_release_date`: set for a game that came out before it came to Steam.
    pub first_released: u64,
    /// `common/metacritic_score`.
    pub critic_score: Option<u8>,
    /// `common/review_percentage`.
    pub community_score: Option<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Credits {
    #[default]
    Associations,
    Extended,
}

/// How a game's cover, hero and header are stored under `appcache/librarycache/<appid>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    /// The older layout: `library_600x900.jpg`, `library_hero.jpg` and `header.jpg` in the folder itself.
    Flat,
    /// The newer one: each in a folder named for its hash, which appinfo's `library_assets_full` names.
    Hashed,
    /// As `Flat`, but the files are PNGs under their `.jpg` names, as some of Steam's are.
    PngNamedJpg,
    /// No art on disk.
    None,
}

/// The newest last-played time in `Fixture::standard`; each played game after the first was played a day earlier.
pub const NEWEST_PLAY: u64 = 1_790_000_000;

const TOOL: u32 = 9_100_900;
const DLC: u32 = 9_100_901;
const EVERYONES: u32 = 9_100_902;

impl Fixture {
    /// Fourteen invented games, by appid 9100010, 9100020, ...
    ///
    /// The first six were played, a day apart, so they are the Recent row in this order:
    ///
    /// | # | game             | installed | art                 |
    /// |---|------------------|-----------|---------------------|
    /// | 0 | Ashfall Courier  | yes       | hashed folders      |
    /// | 1 | Lanternfish Deep | yes       | flat                |
    /// | 2 | Brass Meridian   | yes       | hashed folders      |
    /// | 3 | Quiet Orbit      | no        | flat                |
    /// | 4 | Thimble & Thorn  | yes       | a PNG named `.jpg`  |
    /// | 5 | Saltwind Rally   | no        | hashed folders      |
    ///
    /// The other eight were never played and come after, by name; every second one is installed, and Ember Choir and
    /// Hollow Pines Motel have no art.
    ///
    /// Each has made-up facts (`facts`): most a developer, a publisher, a release date and scores; Brass Meridian
    /// two developers; Lanternfish Deep an older entry with its credits in `extended`; Thimble & Thorn a date from
    /// before it came to Steam; Quiet Orbit no scores; Hollow Pines Motel nothing at all.
    ///
    /// Two collections made by hand: Couch co-op (Brass Meridian, Saltwind Rally, Glasswing Atlas, Marrow Street)
    /// and Short sessions (Lanternfish Deep, Thimble & Thorn, Copperhead Station, and the DLC, which is no game).
    /// One dynamic and one deleted, which `load` leaves out. Brass Meridian and Ember Choir are in Steam's
    /// Favourites; nothing is hidden.
    pub fn standard() -> Fixture {
        use CollectionKind::ByHand;
        let games = [
            ("Ashfall Courier", true, true, Art::Hashed),
            ("Lanternfish Deep", true, true, Art::Flat),
            ("Brass Meridian", true, true, Art::Hashed),
            ("Quiet Orbit", false, true, Art::Flat),
            ("Thimble & Thorn", true, true, Art::PngNamedJpg),
            ("Saltwind Rally", false, true, Art::Hashed),
            ("Copperhead Station", true, false, Art::Flat),
            ("Dune Cartographer", false, false, Art::Hashed),
            ("Ember Choir", true, false, Art::None),
            ("Fathom Nine", false, false, Art::Flat),
            ("Glasswing Atlas", true, false, Art::Hashed),
            ("Hollow Pines Motel", false, false, Art::None),
            ("Marrow Street", true, false, Art::Flat),
            ("Nightjar Protocol", false, false, Art::Hashed),
        ];
        let games = games.iter().enumerate().map(|(i, &(name, installed, played, art))| FixtureGame {
            appid: 9_100_010 + i as u32 * 10,
            name: name.to_string(),
            install_dir: name.replace(" & ", " and "),
            installed,
            last_played: if played { NEWEST_PLAY - i as u64 * 86_400 } else { 0 },
            playtime_minutes: if played { 90 + (6 - i as u32) * 240 } else { 0 },
            art,
            facts: facts(i),
            // From a little over 1 GB, each game 3.3 GB more than the last.
            size_on_disk: 1_234_567_890 + i as u64 * 3_543_210_000,
        });
        let games: Vec<FixtureGame> = games.collect();
        let appids = |of: &[usize]| of.iter().map(|&i| games[i].appid).collect::<Vec<u32>>();
        let collection = |id: &str, name: &str, appids: Vec<u32>, kind| FixtureCollection {
            id: id.to_string(),
            name: name.to_string(),
            appids,
            kind,
        };
        let collections = FixtureCollections {
            collections: vec![
                collection("uc-Qm4tZx81LwPe", "Short sessions", [appids(&[1, 4, 6]), vec![DLC]].concat(), ByHand),
                collection("uc-B7hNc02KdRsy", "Roguelikes", appids(&[0]), CollectionKind::Dynamic),
                collection("uc-Vj9aG3fT6uXo", "Couch co-op", appids(&[2, 5, 10, 12]), ByHand),
                collection("uc-D1eY5pWq8MzC", "An old shelf", Vec::new(), CollectionKind::Deleted),
            ],
            favourites: appids(&[2, 8]),
            hidden: Vec::new(),
        };
        Fixture {
            persona: "Tester".into(),
            user_id: 100_200_300,
            games,
            game_exe: None,
            collections: Some(collections),
        }
    }

    /// `standard` and twelve more games after it, by appid 9100150, 9100160, ...: installed, never played, every
    /// third with no art. Twenty installed games in all, which is four rows of My games & apps' grid: enough for it
    /// to scroll. Nothing `standard` says of its own games changes.
    pub fn full_shelf() -> Fixture {
        const MORE: [&str; 12] = [
            "Oakum & Tar",
            "Pale Meridian Club",
            "Quarry Lights",
            "Rookwood Signal",
            "Sable Ferry",
            "Tin Lantern Derby",
            "Umber Coast",
            "Vesper Line",
            "Wren and the Long Winter",
            "Yarrow Field",
            "Zenith Yard",
            "Zinc Harbour",
        ];
        let mut spec = Fixture::standard();
        let after = spec.games.len();
        spec.games.extend(MORE.iter().enumerate().map(|(i, name)| FixtureGame {
            appid: 9_100_010 + (after + i) as u32 * 10,
            name: name.to_string(),
            install_dir: name.replace(" & ", " and "),
            installed: true,
            last_played: 0,
            playtime_minutes: 0,
            art: if i % 3 == 2 { Art::None } else { Art::Flat },
            facts: Facts::default(),
            size_on_disk: 0,
        }));
        spec
    }
}

/// The made-up facts of the standard fixture's game `i`.
fn facts(i: usize) -> Facts {
    const STUDIOS: [&str; 5] = ["Tin Lantern", "Second Kettle", "Mossback Works", "Paper Comet", "Low Tide Collective"];
    const PUBLISHERS: [&str; 3] = ["Harbour & Hull", "Ninth Bell Games", "Oddfellow Interactive"];
    // The first came out on 12 March 2019; each after it 173 days later.
    let released = 1_552_348_800 + i as u64 * 173 * 86_400;
    let mut facts = Facts {
        developers: vec![STUDIOS[i % STUDIOS.len()].to_string()],
        publishers: vec![PUBLISHERS[i % PUBLISHERS.len()].to_string()],
        credits: Credits::Associations,
        released,
        first_released: 0,
        critic_score: Some(62 + (i as u8 * 7) % 33),
        community_score: Some(55 + (i as u8 * 11) % 44),
    };
    match i {
        1 => facts.credits = Credits::Extended,
        2 => facts.developers.push(STUDIOS[4].to_string()),
        3 => (facts.critic_score, facts.community_score) = (None, None),
        // Out for eleven years before it came to Steam.
        4 => facts.first_released = released - 11 * 365 * 86_400,
        11 => facts = Facts::default(),
        _ => {}
    }
    facts
}

/// Writes `spec` as a Steam install at `root`, making the folder. Files already there are written over.
pub fn write(root: &Path, spec: &Fixture) -> io::Result<()> {
    let root = std::path::absolute(root)?;
    let appcache = root.join("appcache");
    let steamapps = root.join("steamapps");
    let config = root.join("userdata").join(spec.user_id.to_string()).join("config");
    for dir in [&appcache, &steamapps, &config] {
        fs::create_dir_all(dir)?;
    }
    // Written afresh: no change made to an earlier fixture here ([`change`]) is kept.
    let mut gone = vec![root.join(CHANGES)];
    if spec.games.iter().all(|game| game.appid != ADDED_GAME.0) {
        gone.push(steamapps.join(format!("appmanifest_{}.acf", ADDED_GAME.0)));
    }
    for file in gone {
        match fs::remove_file(&file) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    fs::write(appcache.join("appinfo.vdf"), appinfo(spec))?;
    fs::write(appcache.join("packageinfo.vdf"), packageinfo(spec))?;
    fs::write(config.join("localconfig.vdf"), localconfig(spec))?;
    fs::write(steamapps.join("libraryfolders.vdf"), libraryfolders(&root, spec))?;
    let cloud = config.join(crate::collections::FOLDER);
    match &spec.collections {
        Some(collections) => {
            fs::create_dir_all(&cloud)?;
            fs::write(cloud.join(crate::collections::FILE), cloud_storage(collections))?;
        }
        // Written over an install that had one: it has none now.
        None => match fs::remove_file(cloud.join(crate::collections::FILE)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        },
    }
    if spec.games.iter().any(|game| game.appid == ACHIEVEMENTS_GAME) {
        let unlocked: Vec<Option<u64>> = ACHIEVEMENTS.iter().map(|a| a.unlocked).collect();
        write_achievements(&root, spec.user_id, &unlocked)?;
    }
    for game in &spec.games {
        if game.installed {
            fs::write(steamapps.join(format!("appmanifest_{}.acf", game.appid)), manifest(game))?;
            let dir = steamapps.join("common").join(&game.install_dir);
            fs::create_dir_all(&dir)?;
            if let Some(exe) = &spec.game_exe {
                fs::copy(exe, dir.join("game.exe"))?;
            }
        }
        for (asset, image) in art(game) {
            let file = appcache.join("librarycache").join(game.appid.to_string()).join(asset.path(game));
            fs::create_dir_all(file.parent().expect("an art file is in a folder"))?;
            let format = if game.art == Art::PngNamedJpg { ImageFormat::Png } else { ImageFormat::Jpeg };
            image.save_with_format(&file, format).map_err(io::Error::other)?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ KeyValues

/// A KeyValues tree to write, as text or as binary.
enum Kv {
    Map(Vec<(String, Kv)>),
    Str(String),
    Int(u32),
}

fn map<const N: usize>(entries: [(&str, Kv); N]) -> Kv {
    Kv::Map(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn s(value: impl ToString) -> Kv {
    Kv::Str(value.to_string())
}

impl Kv {
    /// The entries of a map in binary, then its end marker. `key` writes a key: inline, or as an index into a table.
    fn binary(&self, out: &mut Vec<u8>, key: &mut dyn FnMut(&str, &mut Vec<u8>)) {
        let Kv::Map(entries) = self else { panic!("only a map has a binary body") };
        for (k, v) in entries {
            match v {
                Kv::Map(_) => {
                    out.push(0);
                    key(k, out);
                    v.binary(out, key);
                }
                Kv::Str(text) => {
                    out.push(1);
                    key(k, out);
                    out.extend(text.as_bytes());
                    out.push(0);
                }
                Kv::Int(n) => {
                    out.push(2);
                    key(k, out);
                    out.extend(n.to_le_bytes());
                }
            }
        }
        out.push(8);
    }

    /// The entries of a map as text, as Steam lays its files out: tabs, a brace a line.
    fn text(&self, out: &mut String, depth: usize) {
        let Kv::Map(entries) = self else { panic!("only a map has entries") };
        let quoted = |text: &str| format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
        let indent = "\t".repeat(depth);
        for (k, v) in entries {
            match v {
                Kv::Map(_) => {
                    out.push_str(&format!("{indent}{}\n{indent}{{\n", quoted(k)));
                    v.text(out, depth + 1);
                    out.push_str(&format!("{indent}}}\n"));
                }
                Kv::Str(text) => out.push_str(&format!("{indent}{}\t\t{}\n", quoted(k), quoted(text))),
                Kv::Int(n) => out.push_str(&format!("{indent}{}\t\t{}\n", quoted(k), quoted(&n.to_string()))),
            }
        }
    }

    fn to_text(&self) -> String {
        let mut out = String::new();
        self.text(&mut out, 0);
        out
    }
}

// ---------------------------------------------------------------------- files

/// `appcache/appinfo.vdf`, version 29: keys are indexes into a string table at the end of the file.
fn appinfo(spec: &Fixture) -> Vec<u8> {
    let mut apps: Vec<(u32, Kv)> = spec.games.iter().map(|g| (g.appid, game_info(g))).collect();
    let other = |appid: u32, name: &str, ty: &str| {
        (appid, map([("appid", Kv::Int(appid)), ("common", map([("name", s(name)), ("type", s(ty))]))]))
    };
    apps.push(other(TOOL, "Fixture Redistributables", "Tool"));
    apps.push(other(DLC, "Ashfall Courier: The Long Road", "DLC"));
    apps.push(other(EVERYONES, "Everyone's Sandbox", "Game"));

    let mut table: Vec<String> = Vec::new();
    let mut key = |k: &str, out: &mut Vec<u8>| {
        let index = table.iter().position(|t| t == k).unwrap_or_else(|| {
            table.push(k.to_string());
            table.len() - 1
        });
        out.extend((index as u32).to_le_bytes());
    };
    let mut entries = Vec::new();
    for (appid, info) in &apps {
        let mut body = Vec::new();
        body.push(0);
        key("appinfo", &mut body);
        info.binary(&mut body, &mut key);
        body.push(8);

        let mut header = Vec::with_capacity(60);
        header.extend(2u32.to_le_bytes()); // info state: available
        header.extend((NEWEST_PLAY as u32).to_le_bytes()); // last updated
        header.extend(0u64.to_le_bytes()); // PICS token
        header.extend([0x11; 20]); // SHA-1 of the text form
        header.extend(1000u32.to_le_bytes()); // change number
        header.extend([0x22; 20]); // SHA-1 of the binary form
        entries.extend(appid.to_le_bytes());
        entries.extend(((header.len() + body.len()) as u32).to_le_bytes());
        entries.extend(header);
        entries.extend(body);
    }

    let mut file = 0x0756_4429u32.to_le_bytes().to_vec();
    file.extend(1u32.to_le_bytes()); // universe: public
    file.extend(((16 + entries.len() + 4) as u64).to_le_bytes());
    file.extend(entries);
    file.extend(0u32.to_le_bytes());
    file.extend((table.len() as u32).to_le_bytes());
    for k in &table {
        file.extend(k.as_bytes());
        file.push(0);
    }
    file
}

fn game_info(game: &FixtureGame) -> Kv {
    let mut common = vec![("name".to_string(), s(&game.name)), ("type".to_string(), s("Game"))];
    if game.art == Art::Hashed {
        let asset = |a: Asset| map([("image", map([("english", s(a.path(game)))]))]);
        common.push((
            "library_assets_full".to_string(),
            map([
                ("library_capsule", asset(Asset::Cover)),
                ("library_hero", asset(Asset::Hero)),
                ("library_header", asset(Asset::Header)),
            ]),
        ));
    }
    common.push(("gameid".to_string(), Kv::Int(game.appid)));

    // What the Game hub tells of a game, where Steam keeps it: the credits as associations (with a franchise among
    // them, which is neither) or in `extended`; the dates and the scores as numbers in `common`.
    let facts = &game.facts;
    let mut extended: Vec<(String, Kv)> = Vec::new();
    match facts.credits {
        Credits::Associations => {
            let credit = |ty: &str, name: &String| map([("type", s(ty)), ("name", s(name))]);
            let developers = facts.developers.iter().map(|name| credit("developer", name));
            let franchise = (!facts.developers.is_empty()).then(|| credit("franchise", &game.name));
            let publishers = facts.publishers.iter().map(|name| credit("publisher", name));
            let all = developers.chain(franchise).chain(publishers).enumerate();
            let all: Vec<(String, Kv)> = all.map(|(at, credit)| (at.to_string(), credit)).collect();
            if !all.is_empty() {
                common.push(("associations".to_string(), Kv::Map(all)));
            }
        }
        Credits::Extended => {
            for (key, names) in [("developer", &facts.developers), ("publisher", &facts.publishers)] {
                extended.extend(names.first().map(|name| (key.to_string(), s(name))));
            }
        }
    }
    let numbers = [
        ("steam_release_date", facts.released),
        ("original_release_date", facts.first_released),
        ("metacritic_score", facts.critic_score.map_or(0, u64::from)),
        ("review_percentage", facts.community_score.map_or(0, u64::from)),
    ];
    for (key, number) in numbers {
        let present = number > 0 || (key == "review_percentage" && facts.community_score.is_some());
        if present {
            common.push((key.to_string(), Kv::Int(number as u32)));
        }
    }
    let mut info = vec![("appid".to_string(), Kv::Int(game.appid)), ("common".to_string(), Kv::Map(common))];
    if !extended.is_empty() {
        info.push(("extended".to_string(), Kv::Map(extended)));
    }
    Kv::Map(info)
}

/// `appcache/packageinfo.vdf`, version 28. The games are split over two packages, as bought one by one and in a
/// bundle; the second also grants the tool and the DLC. Package 0 grants the game nobody owns.
fn packageinfo(spec: &Fixture) -> Vec<u8> {
    let ids: Vec<u32> = spec.games.iter().map(|g| g.appid).collect();
    let (first, rest) = ids.split_at(ids.len() / 2);
    let packages = [(0u32, vec![EVERYONES]), (510_001, first.to_vec()), (510_002, [rest, &[TOOL, DLC][..]].concat())];

    let mut inline = |k: &str, out: &mut Vec<u8>| {
        out.extend(k.as_bytes());
        out.push(0);
    };
    let mut file = 0x0656_5528u32.to_le_bytes().to_vec();
    file.extend(1u32.to_le_bytes());
    for (id, apps) in packages {
        file.extend(id.to_le_bytes());
        file.extend([0x33; 20]); // SHA-1
        file.extend(1000u32.to_le_bytes()); // change number
        file.extend(0u64.to_le_bytes()); // PICS token
        let appids = Kv::Map(apps.iter().enumerate().map(|(i, &a)| (i.to_string(), Kv::Int(a))).collect());
        let body = map([("packageid", Kv::Int(id)), ("billingtype", Kv::Int(10)), ("appids", appids)]);
        Kv::Map(vec![(id.to_string(), body)]).binary(&mut file, &mut inline);
    }
    file.extend(u32::MAX.to_le_bytes());
    file
}

/// `userdata/<id>/config/localconfig.vdf`: the persona name, and when and how long each played game was played. A
/// game never played has an entry too, as one Steam has synced saves for does.
fn localconfig(spec: &Fixture) -> Vec<u8> {
    let apps = spec.games.iter().map(|g| {
        let entry = if g.last_played > 0 {
            map([("LastPlayed", s(g.last_played)), ("Playtime", s(g.playtime_minutes))])
        } else {
            map([("cloud", map([("last_sync_state", s("synchronized"))]))])
        };
        (g.appid.to_string(), entry)
    });
    let friends = map([
        ("76500000000000001", map([("name", s("Someone Else")), ("NameHistory", map([("0", s("Someone Else"))]))])),
        ("PersonaName", s(&spec.persona)),
    ]);
    let steam = map([("apps", Kv::Map(apps.collect()))]);
    let store = map([("friends", friends), ("Software", map([("Valve", map([("Steam", steam)]))]))]);
    map([("UserLocalConfigStore", store)]).to_text().into_bytes()
}

/// `steamapps/libraryfolders.vdf`: one library, the Steam root itself.
fn libraryfolders(root: &Path, spec: &Fixture) -> Vec<u8> {
    let apps = spec.games.iter().filter(|g| g.installed).map(|g| (g.appid.to_string(), s(1_048_576)));
    let library = map([("path", s(root.display())), ("label", s("")), ("apps", Kv::Map(apps.collect()))]);
    map([("libraryfolders", map([("0", library)]))]).to_text().into_bytes()
}

/// Adds a library at `library` to the fake Steam at `root`, as Steam's Storage Manager adds one on another drive
/// (#179): the folder and its `steamapps` made, and `libraryfolders.vdf` written again with it before the Steam
/// folder's own, in Steam's order. A library already there is left as it is. The libraries' app lists are written
/// empty: the Dashboard reads installs from the manifests, not from them.
pub fn add_library(root: &Path, library: &Path) -> io::Result<()> {
    let (root, library) = (std::path::absolute(root)?, std::path::absolute(library)?);
    fs::create_dir_all(library.join("steamapps"))?;
    let mut roots: Vec<PathBuf> =
        crate::libraries(&root).iter().filter_map(|dir| dir.parent().map(Path::to_path_buf)).collect();
    if roots.iter().any(|known| model::storage::same_folder(known, &library)) {
        return Ok(());
    }
    roots.insert(roots.len().saturating_sub(1), library);
    // The Steam folder's own first, as Steam writes it ("0"), then the others.
    roots.rotate_right(1);
    let entry = |path: &Path| map([("path", s(path.display())), ("label", s("")), ("apps", Kv::Map(Vec::new()))]);
    let entries = roots.iter().enumerate().map(|(at, path)| (at.to_string(), entry(path))).collect();
    let text = map([("libraryfolders", Kv::Map(entries))]).to_text();
    fs::write(root.join("steamapps").join("libraryfolders.vdf"), text)
}

/// `userdata/<id>/config/cloudstorage/cloud-storage-namespace-1.json`: a list of `[key, entry]` pairs. A collection's
/// entry carries its value as JSON in a string. Steam's own two collections are always there, and so is a key of
/// another kind, which is no collection.
fn cloud_storage(spec: &FixtureCollections) -> Vec<u8> {
    use serde_json::{json, Value};
    let entry = |id: &str, value: Option<Value>| {
        let key = format!("user-collections.{id}");
        let mut entry = json!({ "key": key, "timestamp": NEWEST_PLAY, "version": "41" });
        match value {
            Some(value) => {
                entry["value"] = json!(value.to_string());
                entry["conflictResolutionMethod"] = json!("custom");
                entry["strMethodId"] = json!("union-collections");
            }
            None => entry["is_deleted"] = json!(true),
        }
        json!([key, entry])
    };
    let own = |id: &str, name: &str, appids: &[u32]| {
        entry(id, Some(json!({ "id": id, "name": name, "added": appids, "removed": [] })))
    };
    let mut pairs = vec![own("favorite", "Favorites", &spec.favourites), own("hidden", "Hidden", &spec.hidden)];
    for c in &spec.collections {
        let mut value = json!({ "id": c.id, "name": c.name, "added": c.appids, "removed": [] });
        match c.kind {
            CollectionKind::ByHand => {}
            CollectionKind::Dynamic => {
                value["filterSpec"] = json!({
                    "nFormatVersion": 2,
                    "strSearchText": "",
                    "filterGroups": [{ "rgOptions": [], "bAcceptUnion": false }],
                });
            }
            CollectionKind::Deleted => {
                pairs.push(entry(&c.id, None));
                continue;
            }
        }
        pairs.push(entry(&c.id, Some(value)));
    }
    let pinned = json!({ "key": "showcase-pins", "timestamp": NEWEST_PLAY, "value": "[]", "version": "3" });
    pairs.push(json!(["showcase-pins", pinned]));
    serde_json::to_vec(&Value::Array(pairs)).expect("JSON values always serialise")
}

/// `steamapps/appmanifest_<appid>.acf`.
fn manifest(game: &FixtureGame) -> Vec<u8> {
    let state = map([
        ("appid", s(game.appid)),
        ("universe", s(1)),
        ("name", s(&game.name)),
        ("StateFlags", s(4)),
        ("installdir", s(&game.install_dir)),
        ("LastPlayed", s(game.last_played)),
        ("SizeOnDisk", s(game.size_on_disk)),
    ]);
    map([("AppState", state)]).to_text().into_bytes()
}

/// Where an install the fake Steam has under way is, as `write_progress` writes it into its manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Taken on and waiting its turn: what Steam writes as it is asked (`UpdateRequired | UpdateStarted`), no bytes.
    Queued,
    /// Downloading (`UpdateRunning` too), `done` of `total` bytes.
    Downloading { done: u64, total: u64 },
    /// Paused by the user (`UpdatePaused` in place of `UpdateRunning`).
    Paused { done: u64, total: u64 },
    /// Finished: `FullyInstalled`, its folder made under `steamapps/common`.
    Done,
    /// Dropped, cancelled in Steam, or uninstalled: the manifest goes.
    Dropped,
    /// Cancelled while the download still ran: the files gone, the manifest left behind at `UpdateRequired |
    /// UpdatePaused` (StateFlags 514), as the real Steam left one on the TV PC (2026-11-17, #171).
    Abandoned { done: u64, total: u64 },
}

/// Writes the manifest of the fixture's game `appid` (`Fixture::full_shelf`'s, else a made-up name and folder) as the
/// fake Steam has it at `progress`: what the end-to-end tests' stand-in Steam does as it is asked to install or
/// uninstall, and as a test moves an install along (`standin steam-install`). Nothing is downloaded.
pub fn write_progress(root: &Path, appid: u32, progress: Progress) -> io::Result<()> {
    let steamapps = root.join("steamapps");
    let file = steamapps.join(format!("appmanifest_{appid}.acf"));
    let known = Fixture::full_shelf().games.into_iter().find(|game| game.appid == appid);
    let (name, install_dir, size) = match known {
        Some(game) => (game.name, game.install_dir, game.size_on_disk),
        None => (format!("App {appid}"), appid.to_string(), 0),
    };
    let (flags, done, total) = match progress {
        Progress::Queued => (crate::installs::QUEUED, 0, 0),
        Progress::Downloading { done, total } => {
            (crate::installs::QUEUED | crate::installs::UPDATE_RUNNING, done, total)
        }
        Progress::Paused { done, total } => (crate::installs::QUEUED | crate::installs::UPDATE_PAUSED, done, total),
        Progress::Abandoned { done, total } => (2 | crate::installs::UPDATE_PAUSED, done, total),
        Progress::Done => (crate::installs::FULLY_INSTALLED, size, size),
        Progress::Dropped => {
            return match fs::remove_file(&file) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
    };
    if progress == Progress::Done {
        fs::create_dir_all(steamapps.join("common").join(&install_dir))?;
    }
    let state = map([
        ("appid", s(appid)),
        ("universe", s(1)),
        ("name", s(&name)),
        ("StateFlags", s(flags)),
        ("installdir", s(&install_dir)),
        ("LastPlayed", s(0)),
        ("SizeOnDisk", s(if progress == Progress::Done { size } else { 0 })),
        ("BytesToDownload", s(total)),
        ("BytesDownloaded", s(done)),
    ]);
    fs::create_dir_all(&steamapps)?;
    // Written whole and then moved into place, as a reader may look at any moment.
    let part = steamapps.join(format!("appmanifest_{appid}.acf.part"));
    fs::write(&part, map([("AppState", state)]).to_text())?;
    fs::rename(part, file)
}

/// The file beside the stand-in Steam's `steam.exe` that says how its install wizard goes (#230), as `standin
/// steam-wizard` writes it: which apps have licence agreements, which ask for a product key, and how much is free.
pub const WIZARD_FILE: &str = "standin-wizard.json";

/// How the stand-in Steam's install wizard goes for each app (`crate::debugger`): with no file, as it goes for
/// most games, with no agreement, no key, and room for anything.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct WizardSetUp {
    /// The apps with licence agreements, and how many each has: they are shown one after another.
    #[serde(default)]
    pub agreements: std::collections::BTreeMap<u32, u32>,
    /// The apps whose wizard asks for a product key (`eInstallState` 4) in place of its config.
    #[serde(default)]
    pub keys: Vec<u32>,
    /// The bytes free in every library, as the install manager says (`nDiskSpaceAvailable`); none: a TiB.
    #[serde(default)]
    pub free: Option<u64>,
}

impl WizardSetUp {
    /// What the fake at `root` has written, or the plain wizard.
    pub fn read(root: &Path) -> WizardSetUp {
        fs::read_to_string(root.join(WIZARD_FILE))
            .ok()
            .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn write(&self, root: &Path) -> io::Result<()> {
        fs::write(root.join(WIZARD_FILE), serde_json::to_string_pretty(self).map_err(io::Error::other)?)
    }

    /// The bytes free, as the install manager says.
    pub fn free(&self) -> u64 {
        self.free.unwrap_or(1 << 40)
    }
}

/// What the stand-in's install manager says an app needs (`nDiskSpaceRequired`): the fixture's size for it
/// (`Fixture::full_shelf`), else 1 GiB.
pub fn needed(appid: u32) -> u64 {
    Fixture::full_shelf().games.into_iter().find(|game| game.appid == appid).map_or(1 << 30, |game| game.size_on_disk)
}

/// The file beside the stand-in Steam's `steam.exe` that says it is the stand-in (written by `standin make-steam`):
/// its debugger (`crate::debugger`) is asked on the port it writes down, never on a real Steam's (`cef::ports`).
/// Looked for only in an install named on the command line (`--steam`), never in the one the registry names.
pub const STANDIN_CLIENT: &str = "standin-client.txt";

/// How big each of the stand-in's Workshop items is, in bytes: item `n` (from 0) is `(n + 1)` times this.
pub const WORKSHOP_ITEM: u64 = 4 << 20;

/// The id of the stand-in's Workshop item `n` (from 0) of an app: made up, never a real item's.
fn workshop_item(appid: u32, n: u32) -> u64 {
    appid as u64 * 1000 + n as u64 + 1
}

/// Writes the fake Steam's Workshop record of `appid` (`steamapps/workshop/appworkshop_<appid>.acf`) as Steam's
/// would be with `done` of `items` subscribed items fetched (#188): every item in `WorkshopItemDetails` with its
/// size, the fetched ones in `WorkshopItemsInstalled` with their folders in `workshop/content/<appid>` (a small file
/// each), and `NeedsDownload` 1 until all are in. What the stand-in Steam does over time as a test calls it (`standin
/// steam-workshop`). The real record's item lists are as Steam's files are commonly read (`installs::workshop`).
pub fn write_workshop(root: &Path, appid: u32, done: u32, items: u32) -> io::Result<()> {
    let done = done.min(items);
    let workshop = root.join("steamapps").join("workshop");
    let content = workshop.join("content").join(appid.to_string());
    fs::create_dir_all(&content)?;
    let size = |n: u32| WORKSHOP_ITEM * (n as u64 + 1);
    let mut installed = Vec::new();
    let mut details = Vec::new();
    for n in 0..items {
        let id = workshop_item(appid, n).to_string();
        let folder = content.join(&id);
        if n < done {
            fs::create_dir_all(&folder)?;
            fs::write(folder.join("item.txt"), format!("stand-in Workshop item {id}\n"))?;
            installed.push((id.clone(), map([("size", s(size(n))), ("timeupdated", s(0)), ("manifest", s(n))])));
        } else if folder.exists() {
            fs::remove_dir_all(&folder)?;
        }
        let subscribed = [("manifest", s(n)), ("timeupdated", s(0)), ("subscribedby", s(1)), ("size", s(size(n)))];
        details.push((id, map(subscribed)));
    }
    let on_disk: u64 = (0..done).map(size).sum();
    let record = map([
        ("appid", s(appid)),
        ("SizeOnDisk", s(on_disk)),
        ("NeedsUpdate", s(0)),
        ("NeedsDownload", s(u8::from(done < items))),
        ("TimeLastUpdated", s(0)),
        ("TimeLastAppRan", s(0)),
        ("WorkshopItemsInstalled", Kv::Map(installed)),
        ("WorkshopItemDetails", Kv::Map(details)),
    ]);
    let part = workshop.join(format!("appworkshop_{appid}.acf.part"));
    fs::write(&part, map([("AppWorkshop", record)]).to_text())?;
    fs::rename(part, workshop.join(format!("appworkshop_{appid}.acf")))
}

/// Removes the fake Steam's Workshop record and items of `appid`, as uninstalling the app does.
pub fn remove_workshop(root: &Path, appid: u32) -> io::Result<()> {
    let workshop = root.join("steamapps").join("workshop");
    for gone in [workshop.join(format!("appworkshop_{appid}.acf")), workshop.join("content").join(appid.to_string())] {
        let removed = if gone.is_dir() { fs::remove_dir_all(&gone) } else { fs::remove_file(&gone) };
        match removed {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// Where the fake Steam's install of `appid` at `root` goes as its client is asked to pause, resume or cancel it,
/// from its manifest as it is: paused or downloading at the bytes it has, or dropped, what was fetched gone with
/// it. An app with no install under way has nothing to steer.
pub fn steered(root: &Path, appid: u32, steer: model::Steer) -> io::Result<Progress> {
    let file = root.join("steamapps").join(format!("appmanifest_{appid}.acf"));
    let counts = crate::installs::counts(&file)
        .filter(|counts| crate::installs::state(counts.flags, 0, 0, false).is_some())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no install of {appid} under way")))?;
    let (done, total) = (counts.downloaded, counts.to_download);
    Ok(match steer {
        model::Steer::Pause => Progress::Paused { done, total },
        model::Steer::Resume => Progress::Downloading { done, total },
        model::Steer::Cancel => Progress::Dropped,
    })
}

// ------------------------------------------------------------------- changes

/// The game a change adds to the standard fixture ([`Change::AddGame`]): installed, never played.
pub const ADDED_GAME: (u32, &str) = (9_100_800, "Paper Lighthouse");

/// The file in a fake Steam's folder that keeps the changes made to it, one a line, so that each is written over
/// the standard fixture with those before it.
const CHANGES: &str = "fixture-changes.txt";

/// A change to the standard fixture while the Dashboard reads it, as Steam's own files change: what Settings >
/// Launchers' Update now is to bring in (`standin steam-change`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// [`ADDED_GAME`] is the user's, installed.
    AddGame,
    /// The game `appid` was played `minutes` more, last at `at` (Unix seconds).
    Played { appid: u32, minutes: u32, at: u64 },
}

impl Change {
    fn line(self) -> String {
        match self {
            Change::AddGame => "add".to_string(),
            Change::Played { appid, minutes, at } => format!("played {appid} {minutes} {at}"),
        }
    }

    fn parse(line: &str) -> Option<Change> {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["add"] => Some(Change::AddGame),
            ["played", appid, minutes, at] => Some(Change::Played {
                appid: appid.parse().ok()?,
                minutes: minutes.parse().ok()?,
                at: at.parse().ok()?,
            }),
            _ => None,
        }
    }

    fn apply(self, spec: &mut Fixture) {
        match self {
            Change::AddGame if spec.games.iter().all(|game| game.appid != ADDED_GAME.0) => {
                spec.games.push(FixtureGame {
                    appid: ADDED_GAME.0,
                    name: ADDED_GAME.1.to_string(),
                    install_dir: ADDED_GAME.1.to_string(),
                    installed: true,
                    last_played: 0,
                    playtime_minutes: 0,
                    art: Art::Flat,
                    facts: Facts::default(),
                    size_on_disk: 2_000_000_000,
                });
            }
            Change::AddGame => {}
            Change::Played { appid, minutes, at } => {
                if let Some(game) = spec.games.iter_mut().find(|game| game.appid == appid) {
                    game.playtime_minutes += minutes;
                    game.last_played = game.last_played.max(at);
                }
            }
        }
    }
}

/// Makes `change` to the standard fixture written at `root` (with `game_exe` as an added game's program), on top of
/// the changes made before it: the files that list the games and their play time are written again, and an added
/// game's manifest, folder and art. Everything else is left as it is, an install under way and the achievements'
/// stats among them.
pub fn change(root: &Path, change: Change, game_exe: Option<&Path>) -> io::Result<()> {
    let root = std::path::absolute(root)?;
    let kept = root.join(CHANGES);
    let mut changes: Vec<Change> = match fs::read_to_string(&kept) {
        Ok(text) => text.lines().filter_map(Change::parse).collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    changes.push(change);
    let mut spec = Fixture::standard();
    for change in &changes {
        change.apply(&mut spec);
    }
    let config = root.join("userdata").join(spec.user_id.to_string()).join("config");
    let steamapps = root.join("steamapps");
    fs::write(root.join("appcache").join("appinfo.vdf"), appinfo(&spec))?;
    fs::write(root.join("appcache").join("packageinfo.vdf"), packageinfo(&spec))?;
    fs::write(config.join("localconfig.vdf"), localconfig(&spec))?;
    fs::write(steamapps.join("libraryfolders.vdf"), libraryfolders(&root, &spec))?;
    if let Some(game) = spec.games.iter().find(|game| game.appid == ADDED_GAME.0) {
        fs::write(steamapps.join(format!("appmanifest_{}.acf", game.appid)), manifest(game))?;
        let dir = steamapps.join("common").join(&game.install_dir);
        fs::create_dir_all(&dir)?;
        if let Some(exe) = game_exe {
            fs::copy(exe, dir.join("game.exe"))?;
        }
        for (asset, image) in art(game) {
            let file = root.join("appcache").join("librarycache").join(game.appid.to_string()).join(asset.path(game));
            fs::create_dir_all(file.parent().expect("an art file is in a folder"))?;
            image.save_with_format(&file, ImageFormat::Jpeg).map_err(io::Error::other)?;
        }
    }
    let lines: Vec<String> = changes.iter().map(|change| change.line()).collect();
    fs::write(kept, lines.join("\n") + "\n")
}

// --------------------------------------------------------------- achievements

/// The game of the fixture's that has achievements: Ashfall Courier, the first of the Recent row.
pub const ACHIEVEMENTS_GAME: u32 = 9_100_010;

/// One of its made-up achievements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixtureAchievement {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub hidden: bool,
    /// The share of players the stand-in of Steam's global percentages says have it.
    pub rarity: f64,
    /// When the user unlocked it, in Unix seconds; none for one still locked.
    pub unlocked: Option<u64>,
}

/// Ashfall Courier's achievements: six, in one group, two of them unlocked an hour and half an hour before the
/// game was last played. With the stand-in's rarity their Score is 10 and 15, 25 in all; No Detours (7.4%) and
/// Perfect Ledger (0.8%, hidden) are Rare.
pub const ACHIEVEMENTS: [FixtureAchievement; 6] = [
    FixtureAchievement {
        key: "ACH_FIRST_DELIVERY",
        name: "First Delivery",
        description: "Deliver your first parcel.",
        hidden: false,
        rarity: 82.5,
        unlocked: Some(NEWEST_PLAY - 3600),
    },
    FixtureAchievement {
        key: "ACH_ASH_RUNNER",
        name: "Ash Runner",
        description: "Cross the ash plains without stopping.",
        hidden: false,
        rarity: 31.0,
        unlocked: Some(NEWEST_PLAY - 1800),
    },
    FixtureAchievement {
        key: "ACH_NIGHT_ROUTE",
        name: "Night Route",
        description: "Finish a delivery after dark.",
        hidden: false,
        rarity: 18.2,
        unlocked: None,
    },
    FixtureAchievement {
        key: "ACH_NO_DETOURS",
        name: "No Detours",
        description: "Finish a chapter without leaving the road.",
        hidden: false,
        rarity: 7.4,
        unlocked: None,
    },
    FixtureAchievement {
        key: "ACH_PERFECT_LEDGER",
        name: "Perfect Ledger",
        description: "Finish the game with every parcel on time.",
        hidden: true,
        rarity: 0.8,
        unlocked: None,
    },
    FixtureAchievement {
        key: "ACH_LAST_PARCEL",
        name: "The Last Parcel",
        description: "Reach the end of the road.",
        hidden: false,
        rarity: 45.0,
        unlocked: None,
    },
];

/// The stat whose bits the fixture's achievements are.
const ACHIEVEMENT_GROUP: &str = "1";

/// Writes the schema of the fixture's game with achievements, and the user's stats for it with `unlocked` (each
/// achievement's time by its place in [`ACHIEVEMENTS`]), as Steam keeps them in `appcache/stats`.
fn write_achievements(root: &Path, account: u64, unlocked: &[Option<u64>]) -> io::Result<()> {
    let dir = root.join("appcache").join("stats");
    fs::create_dir_all(&dir)?;
    let mut inline = |k: &str, out: &mut Vec<u8>| {
        out.extend(k.as_bytes());
        out.push(0);
    };
    let bits = ACHIEVEMENTS.iter().enumerate().map(|(bit, a)| {
        let display = map([
            ("name", map([("english", s(a.name)), ("token", s(format!("NEW_ACHIEVEMENT_1_{bit}_NAME")))])),
            ("desc", map([("english", s(a.description))])),
            ("hidden", s(u8::from(a.hidden))),
            ("icon", s(format!("{}.jpg", icon_name(bit, false)))),
            ("icon_gray", s(format!("{}.jpg", icon_name(bit, true)))),
        ]);
        (bit.to_string(), map([("name", s(a.key)), ("bit", s(bit)), ("display", display)]))
    });
    let group = map([("type", s(4)), ("id", s(ACHIEVEMENT_GROUP)), ("bits", Kv::Map(bits.collect()))]);
    let schema = map([
        ("gamename", s("Ashfall Courier")),
        ("version", s(1)),
        ("stats", Kv::Map(vec![(ACHIEVEMENT_GROUP.to_string(), group)])),
    ]);
    let mut file = Vec::new();
    Kv::Map(vec![(ACHIEVEMENTS_GAME.to_string(), schema)]).binary(&mut file, &mut inline);
    fs::write(dir.join(crate::achievements::schema_file(ACHIEVEMENTS_GAME)), file)?;

    let data = unlocked.iter().enumerate().filter(|(_, at)| at.is_some()).fold(0u32, |bits, (bit, _)| bits | 1 << bit);
    let times = unlocked.iter().enumerate().filter_map(|(bit, at)| Some((bit.to_string(), Kv::Int((*at)? as u32))));
    let stats = map([
        ("crc", Kv::Int(0x5EED)),
        ("PendingChanges", Kv::Int(0)),
        (ACHIEVEMENT_GROUP, map([("data", Kv::Int(data)), ("AchievementTimes", Kv::Map(times.collect()))])),
    ]);
    let mut file = Vec::new();
    map([("cache", stats)]).binary(&mut file, &mut inline);
    let name = crate::achievements::stats_file(account as u32, ACHIEVEMENTS_GAME);
    // Written whole and then moved into place, as the Dashboard may be reading it at any moment.
    let part = dir.join(format!("{name}.part"));
    fs::write(&part, file)?;
    fs::rename(part, dir.join(name))
}

/// A made-up icon file name, forty hex digits as Steam's are.
fn icon_name(bit: usize, gray: bool) -> String {
    let n = (ACHIEVEMENTS_GAME as u64 * 64 + bit as u64 * 2 + u64::from(gray)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!("{n:016x}{:016x}{:08x}", n.rotate_left(17), (n >> 9) as u32)
}

/// Unlocks the fixture's achievement `key` at `at` (Unix seconds) in the fake Steam at `root`, as Steam writes its
/// stats file while the game runs: what `standin unlock-achievement` does. Those already unlocked stay as they
/// were. Gives false when there is no such achievement, or no signed-in user.
pub fn unlock_achievement(root: &Path, key: &str, at: u64) -> io::Result<bool> {
    let Some(place) = ACHIEVEMENTS.iter().position(|a| a.key.eq_ignore_ascii_case(key)) else { return Ok(false) };
    let Some(account) = crate::signed_in_account(root) else { return Ok(false) };
    let file = root.join("appcache").join("stats").join(crate::achievements::stats_file(account, ACHIEVEMENTS_GAME));
    let mut unlocked: Vec<Option<u64>> = match fs::read(&file) {
        Ok(bytes) => {
            let read = crate::achievements::parse_stats(&bytes).map_err(io::Error::other)?;
            (0..ACHIEVEMENTS.len()).map(|bit| read.get(&(1, bit as u32)).copied()).collect()
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => vec![None; ACHIEVEMENTS.len()],
        Err(e) => return Err(e),
    };
    unlocked[place] = unlocked[place].or(Some(at));
    write_achievements(root, account.into(), &unlocked)?;
    Ok(true)
}

// ------------------------------------------------------------------------ art

#[derive(Clone, Copy)]
enum Asset {
    Cover,
    Hero,
    Header,
}

impl Asset {
    /// Where the asset is under the game's `librarycache` folder.
    fn path(self, game: &FixtureGame) -> String {
        let (index, name) = match self {
            Asset::Cover => (1u64, "library_600x900.jpg"),
            Asset::Hero => (2, "library_hero.jpg"),
            Asset::Header => (3, "header.jpg"),
        };
        if game.art == Art::Hashed {
            // Forty hex digits, as a SHA-1 is: not one, only different for every picture.
            let n = (game.appid as u64 * 4 + index).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            format!("{n:016x}{:016x}{:08x}/{name}", n.rotate_left(21), (n >> 7) as u32)
        } else {
            name.to_string()
        }
    }
}

/// The game's pictures, at the sizes Steam's are: a 600x900 cover, a 1920x620 hero and a 460x215 header, each a
/// gradient of the game's own colour, so every game and every kind of picture can be told apart on a screenshot.
fn art(game: &FixtureGame) -> Vec<(Asset, RgbImage)> {
    if game.art == Art::None {
        return Vec::new();
    }
    let top = colour(game.appid);
    let shade = |by: f32| top.map(|c| (c as f32 * by) as u8);
    // Top to bottom for the cover, left to right for the hero, and the header flat.
    let cover = RgbImage::from_fn(600, 900, |_, y| Rgb(mix(top, shade(0.35), y as f32 / 899.0)));
    let hero = RgbImage::from_fn(1920, 620, |x, _| Rgb(mix(shade(0.25), top, x as f32 / 1919.0)));
    let header = RgbImage::from_pixel(460, 215, Rgb(shade(0.7)));
    vec![(Asset::Cover, cover), (Asset::Hero, hero), (Asset::Header, header)]
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8)
}

/// A strong colour whose hue follows from the appid, neighbouring appids getting hues far apart.
fn colour(appid: u32) -> [u8; 3] {
    let hue = (appid / 10 * 137 % 360) as f32 / 60.0;
    let x = 1.0 - (hue % 2.0 - 1.0).abs();
    let (r, g, b) = match hue as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    // Kept off pure primaries: 0.2 of white mixed in, then 0.85 of full brightness.
    [r, g, b].map(|c: f32| ((0.2 + 0.8 * c) * 0.85 * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;

    fn decoded(path: &Path) -> (ImageFormat, (u32, u32)) {
        let reader = image::ImageReader::open(path).unwrap().with_guessed_format().unwrap();
        let format = reader.format().unwrap();
        (format, reader.into_dimensions().unwrap())
    }

    #[test]
    fn a_library_added_is_a_folder_installs_may_go_to_and_the_games_are_read_as_before() {
        use model::Installer;
        let dir = tempfile::tempdir().unwrap();
        let (root, other) = (dir.path().join("steam"), dir.path().join("second"));
        write(&root, &Fixture::standard()).unwrap();
        let before = load(&root).unwrap().games.len();
        add_library(&root, &other).unwrap();
        add_library(&root, &other).unwrap();
        let steam = crate::Steam::at(Some(root.clone()));
        let folders: Vec<(PathBuf, bool)> = steam.folders().into_iter().map(|f| (f.path, f.default)).collect();
        let root = std::path::absolute(&root).unwrap();
        assert_eq!(folders, [(std::path::absolute(&other).unwrap(), false), (root.clone(), true)]);
        assert_eq!(load(&root).unwrap().games.len(), before);
    }

    #[test]
    fn a_change_adds_a_game_and_moves_play_time_on_over_the_changes_before_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        write(&root, &Fixture::standard()).unwrap();
        let before = load(&root).unwrap();
        let ashfall = ACHIEVEMENTS_GAME.to_string();
        let played = |library: &model::Library| {
            let game = library.games.iter().find(|game| game.source_id == ashfall).unwrap();
            (game.playtime_minutes, game.last_played)
        };
        let (minutes, _) = played(&before);

        change(&root, Change::AddGame, None).unwrap();
        let added = load(&root).unwrap();
        assert_eq!(added.games.len(), before.games.len() + 1);
        let paper = added.games.iter().find(|game| game.source_id == ADDED_GAME.0.to_string()).unwrap();
        assert!(paper.name == ADDED_GAME.1 && paper.installed && paper.cover.is_some());

        let at = NEWEST_PLAY + 3600;
        change(&root, Change::Played { appid: ACHIEVEMENTS_GAME, minutes: 45, at }, None).unwrap();
        let after = load(&root).unwrap();
        assert_eq!(after.games.len(), added.games.len(), "the game added before is still there");
        assert_eq!(played(&after), (minutes + 45, at));
        // Its achievements' stats are left as they were.
        let steam = crate::achievements::SteamAchievements::at(Some(root.clone()));
        use model::achievement::AchievementSource;
        assert!(steam.read(&ashfall).unwrap().is_some());
    }

    #[test]
    fn the_fixtures_achievements_read_back_and_one_can_be_unlocked() {
        use model::achievement::AchievementSource;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        write(&root, &Fixture::standard()).unwrap();
        let steam = crate::achievements::SteamAchievements::at(Some(root.clone()));
        assert_eq!(steam.games(), Some(vec![ACHIEVEMENTS_GAME.to_string()]));
        let read = steam.read(&ACHIEVEMENTS_GAME.to_string()).unwrap().unwrap();
        let got: Vec<_> = read.iter().map(|a| (a.key.as_str(), a.name.as_str(), a.hidden, a.unlocked)).collect();
        let want: Vec<_> = ACHIEVEMENTS.iter().map(|a| (a.key, a.name, a.hidden, a.unlocked)).collect();
        assert_eq!(got, want);
        assert!(read.iter().all(|a| a.icon.is_some() && a.icon_locked.is_some() && a.icon != a.icon_locked));

        assert!(unlock_achievement(&root, "ach_no_detours", 1_800_000_000).unwrap());
        assert!(!unlock_achievement(&root, "ACH_NOT_ONE", 1_800_000_000).unwrap());
        // Unlocked again later: it keeps its first time.
        assert!(unlock_achievement(&root, "ACH_FIRST_DELIVERY", 1_800_000_000).unwrap());
        let read = steam.read(&ACHIEVEMENTS_GAME.to_string()).unwrap().unwrap();
        let unlocked: Vec<_> = read.iter().filter_map(|a| Some((a.key.as_str(), a.unlocked?))).collect();
        assert_eq!(
            unlocked,
            [
                ("ACH_FIRST_DELIVERY", NEWEST_PLAY - 3600),
                ("ACH_ASH_RUNNER", NEWEST_PLAY - 1800),
                ("ACH_NO_DETOURS", 1_800_000_000)
            ]
        );
    }

    #[test]
    fn the_standard_fixture_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let spec = Fixture::standard();
        write(&root, &spec).unwrap();
        let library = load(&root).unwrap();

        assert_eq!(library.user.as_deref(), Some("Tester"));
        // The games only: not the tool, the DLC or package 0's game. Played ones first, the latest first, then the
        // rest by name, which is the order the fixture lists them in.
        let names: Vec<&str> = library.games.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, spec.games.iter().map(|g| g.name.as_str()).collect::<Vec<_>>());
        let recent: Vec<&str> = library.recent(8).iter().map(|g| g.name.as_str()).collect();
        assert_eq!(
            recent,
            [
                "Ashfall Courier",
                "Lanternfish Deep",
                "Brass Meridian",
                "Quiet Orbit",
                "Thimble & Thorn",
                "Saltwind Rally"
            ]
        );
        // The end-to-end tests press Right twice and launch what is there.
        assert!(library.recent(8)[2].installed);

        for (game, want) in library.games.iter().zip(&spec.games) {
            let name = &game.name;
            assert_eq!(game.source_id, want.appid.to_string(), "{name}");
            assert_eq!(game.installed, want.installed, "{name}");
            assert_eq!(game.last_played, want.last_played, "{name}");
            assert_eq!(game.playtime_minutes, want.playtime_minutes, "{name}");
            // What appinfo and the manifest say of it. An older entry's `extended` holds one name of each.
            let facts = &want.facts;
            assert_eq!(game.developers, facts.developers, "{name}");
            assert_eq!(game.publishers, facts.publishers, "{name}");
            let released = if facts.first_released > 0 { facts.first_released } else { facts.released };
            assert_eq!(game.released, released, "{name}");
            assert_eq!(
                (game.critic_score, game.community_score),
                (facts.critic_score, facts.community_score),
                "{name}"
            );
            assert_eq!(game.size_on_disk, if want.installed { want.size_on_disk } else { 0 }, "{name}");
            let dir = want.installed.then(|| root.join("steamapps").join("common").join(&want.install_dir));
            assert_eq!(game.install_dir, dir, "{name}");
            assert!(dir.is_none_or(|d| d.is_dir()), "{name}");

            let art = [(&game.cover, (600, 900)), (&game.hero, (1920, 620)), (&game.header, (460, 215))];
            for (path, size) in art {
                if want.art == Art::None {
                    assert_eq!(*path, None, "{name}");
                    continue;
                }
                let path = path.as_ref().unwrap_or_else(|| panic!("{name} has art missing"));
                assert!(path.extension().is_some_and(|e| e == "jpg"), "{name}");
                let hashed = path.parent().unwrap().file_name().unwrap() != want.appid.to_string().as_str();
                assert_eq!(hashed, want.art == Art::Hashed, "{name}");
                let format = if want.art == Art::PngNamedJpg { ImageFormat::Png } else { ImageFormat::Jpeg };
                assert_eq!(decoded(path), (format, size), "{name}");
            }
        }
        let played = spec.games.iter().filter(|g| g.last_played > 0);
        assert!(played.clone().count() == 6 && played.map(|g| g.last_played).is_sorted_by(|a, b| a > b));

        // The fixture has every way a game's facts are written, and a game with none.
        let by_name = |name: &str| library.games.iter().find(|g| g.name == name).unwrap();
        assert_eq!(by_name("Brass Meridian").developers, ["Mossback Works", "Low Tide Collective"]);
        assert_eq!(by_name("Lanternfish Deep").publishers, ["Ninth Bell Games"]);
        assert!(by_name("Thimble & Thorn").released < by_name("Ashfall Courier").released);
        assert_eq!(by_name("Quiet Orbit").critic_score, None);
        let none = by_name("Hollow Pines Motel");
        assert!(none.developers.is_empty() && none.publishers.is_empty() && none.released == 0);
    }

    #[test]
    fn the_full_shelf_is_the_standard_fixture_and_twelve_installed_games_more() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let (standard, mut spec) = (Fixture::standard(), Fixture::full_shelf());
        spec.games.iter_mut().for_each(|g| g.art = Art::None);
        write(&root, &spec).unwrap();
        let library = load(&root).unwrap();

        let names: Vec<&str> = library.games.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, spec.games.iter().map(|g| g.name.as_str()).collect::<Vec<_>>());
        assert_eq!(names[..standard.games.len()], standard.games.iter().map(|g| g.name.as_str()).collect::<Vec<_>>());
        assert_eq!(library.games.len(), 26);
        assert_eq!(library.games.iter().filter(|g| g.installed).count(), 20);
        assert_eq!(library.recent(8).len(), 6, "the games added were never played");
    }

    #[test]
    fn the_standard_fixtures_collections_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let mut spec = Fixture::standard();
        spec.games.iter_mut().for_each(|g| g.art = Art::None);
        write(&root, &spec).unwrap();
        let library = load(&root).unwrap();
        let appids = |names: &[&str]| -> Vec<u32> {
            names.iter().map(|n| spec.games.iter().find(|g| g.name == *n).unwrap().appid).collect()
        };
        let named = |names: &[&str]| -> Vec<String> { appids(names).iter().map(u32::to_string).collect() };

        // Those made by hand, by name: not the dynamic one, nor the deleted one. The DLC's appid is still there:
        // which appids are games is for whoever takes the collections in.
        let collections = library.collections.clone().expect("the collections file was written");
        let names: Vec<&str> = collections.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Couch co-op", "Short sessions"]);
        assert_eq!(collections[0].source_id, "uc-Vj9aG3fT6uXo");
        let couch = ["Brass Meridian", "Saltwind Rally", "Glasswing Atlas", "Marrow Street"];
        assert_eq!(collections[0].games, named(&couch));
        let mut short = named(&["Lanternfish Deep", "Thimble & Thorn", "Copperhead Station"]);
        short.push(DLC.to_string());
        assert_eq!(collections[1].games, short);

        let favourites: Vec<&str> = library.games.iter().filter(|g| g.favourite).map(|g| g.name.as_str()).collect();
        assert_eq!(favourites, ["Brass Meridian", "Ember Choir"]);
        assert!(library.games.iter().all(|g| !g.hidden && g.source == crate::SOURCE));

        // Steam's hidden games are marked, and still read: hiding is the library's to do.
        spec.collections.as_mut().unwrap().hidden = appids(&["Quiet Orbit"]);
        write(&root, &spec).unwrap();
        let library = load(&root).unwrap();
        let hidden: Vec<&str> = library.games.iter().filter(|g| g.hidden).map(|g| g.name.as_str()).collect();
        assert_eq!((hidden, library.games.len()), (vec!["Quiet Orbit"], spec.games.len()));
    }

    #[test]
    fn with_no_collections_file_or_a_broken_one_there_is_no_word_on_collections() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let mut spec = Fixture::standard();
        spec.games.iter_mut().for_each(|g| g.art = Art::None);
        write(&root, &spec).unwrap();
        assert_eq!(load(&root).unwrap().collections.map(|c| c.len()), Some(2));

        // A file that is not what is expected: not "no collections".
        let file = root.join("userdata").join(spec.user_id.to_string()).join("config");
        let file = file.join(crate::collections::FOLDER).join(crate::collections::FILE);
        fs::write(&file, br#"{"not": "a list"}"#).unwrap();
        let library = load(&root).unwrap();
        assert_eq!(library.collections, None);
        assert!(library.games.iter().all(|g| !g.favourite) && library.games.len() == spec.games.len());

        // An empty list is a word: there are none.
        fs::write(&file, b"[]").unwrap();
        assert_eq!(load(&root).unwrap().collections, Some(Vec::new()));

        spec.collections = None;
        write(&root, &spec).unwrap();
        assert!(!file.exists());
        assert_eq!(load(&root).unwrap().collections, None);
    }

    #[test]
    fn the_game_exe_is_copied_into_installed_games_only() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("standin");
        fs::write(&exe, b"not really a program").unwrap();
        let mut spec = Fixture::standard();
        spec.games.truncate(4);
        spec.games.iter_mut().for_each(|g| g.art = Art::None);
        spec.game_exe = Some(exe);
        let root = dir.path().join("steam");
        write(&root, &spec).unwrap();

        let common = root.join("steamapps").join("common");
        assert_eq!(fs::read(common.join("Brass Meridian").join("game.exe")).unwrap(), b"not really a program");
        assert!(!common.join("Quiet Orbit").exists());
        assert_eq!(load(&root).unwrap().games.len(), 4);
    }

    #[test]
    fn text_values_are_escaped() {
        let text = map([("libraryfolders", map([("0", map([("path", s(r#"C:\e2e\"steam""#))]))]))]).to_text();
        let read = crate::vdf::parse_text(&text).unwrap();
        assert_eq!(read.str_at(&["libraryfolders", "0", "path"]), Some(r#"C:\e2e\"steam""#));
    }

    #[test]
    fn neighbouring_games_have_different_colours() {
        let games = Fixture::standard().games;
        for pair in games.windows(2) {
            assert_ne!(colour(pair[0].appid), colour(pair[1].appid));
        }
    }
}
