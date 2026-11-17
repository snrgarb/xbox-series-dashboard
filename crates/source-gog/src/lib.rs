//! Reads GOG's games into a `model::Library` (docs/research/stores.md, "GOG").
//!
//! Sources:
//! - What is installed: `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<product id>`, which a GOG game's installer writes
//!   whether or not GOG Galaxy is installed (`gameName`, `path`, `exe`, `launchParam`, `workingDir`; `dependsOn`
//!   marks DLC); and in the game's folder `goggame-<product id>.info`, which tells a game from DLC and says what to
//!   run.
//! - What else the account owns, its play time and its art: Galaxy's database,
//!   `%ProgramData%\GOG.com\Galaxy\storage\galaxy-2.0.db` (`galaxy`), there only when Galaxy was signed in on this
//!   PC once; and the pictures Galaxy has cached beside it, under `Galaxy\webcache`.
//!
//! [`Gog`] is the source the library reads (`model::Source`). A GOG game has no launcher check, so it is started
//! as its own program, with the `.info` file's primary play task: no client is needed. Galaxy is asked to install
//! a game where it is installed; a game's own uninstaller uninstalls it.
// Every log line reaches stderr in one write, so two Dashboards appending to one log never tear each other's lines
// (crates/log-line). Defined before the modules, so they all take it rather than std's.
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        log_line::eprintln!($($arg)*)
    };
}

pub mod fixture;
pub mod galaxy;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use model::{Ask, Found, Game, Library, Source, Start};
use serde_json::Value;

/// GOG, as a game's `source` names it.
pub const SOURCE: &str = "gog";

/// GOG, as the library reads it and the shell starts its games.
#[derive(Debug, Clone, Default)]
pub struct Gog {
    /// A folder standing for GOG's on this machine (`--gog`, `Places::under`); with none, the machine's own.
    root: Option<PathBuf>,
}

impl Gog {
    /// GOG's files under `root` (`--gog` on the command line, `Places::under`), or with none the machine's own.
    pub fn at(root: Option<PathBuf>) -> Self {
        Self { root }
    }

    fn places(&self) -> Option<Places> {
        match &self.root {
            Some(root) => Some(Places::under(root)),
            None => Places::machine(),
        }
    }
}

/// Where GOG's games and Galaxy's files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// `%ProgramData%\GOG.com`: Galaxy's database and picture cache, under `Galaxy`.
    pub data: PathBuf,
    /// Where the installed games are listed.
    pub installed: Installed,
    /// `GalaxyClient.exe`, where Galaxy is installed.
    pub client: Option<PathBuf>,
}

/// Where the installed games are listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Installed {
    /// `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games`, as on a real machine.
    Registry,
    /// The folders under this one, each a game's with its `goggame-<id>.info`: what a fake has in place of the
    /// registry.
    Folders(PathBuf),
}

impl Places {
    /// The machine's own, on Windows: `%ProgramData%\GOG.com`, the registry, and the client the registry names
    /// (`HKLM\SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths`, `client`).
    pub fn machine() -> Option<Places> {
        if !cfg!(windows) {
            return None;
        }
        let data = PathBuf::from(std::env::var_os("ProgramData")?).join("GOG.com");
        let client = registry::client().map(|dir| dir.join("GalaxyClient.exe")).filter(|exe| exe.is_file());
        Some(Places { data, installed: Installed::Registry, client })
    }

    /// A folder standing for all three, as the fixture writes one: `<root>` is `%ProgramData%\GOG.com`, the games
    /// are the folders under `<root>\Games`, and Galaxy's program, where there is one, `<root>\Client\
    /// GalaxyClient.exe`.
    pub fn under(root: &Path) -> Places {
        let client = Some(root.join("Client").join("GalaxyClient.exe")).filter(|exe| exe.is_file());
        Places { data: root.to_path_buf(), installed: Installed::Folders(root.join("Games")), client }
    }

    /// `Galaxy\storage\galaxy-2.0.db`.
    pub fn database(&self) -> PathBuf {
        self.data.join("Galaxy").join("storage").join("galaxy-2.0.db")
    }

    /// The installed games as they are listed, DLC and all; None where nothing lists any (no registry key).
    pub fn records(&self) -> Option<Vec<Record>> {
        match &self.installed {
            Installed::Registry => registry::games(),
            Installed::Folders(dir) => Some(folders(dir)),
        }
    }
}

/// An installed product, as the registry lists it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    /// The product id: the game's `source_id`.
    pub id: String,
    pub name: String,
    /// The install folder.
    pub path: PathBuf,
    /// `exe`, `launchParam` and `workingDir`: what to run where the folder has no `.info` file.
    pub exe: Option<PathBuf>,
    pub args: String,
    pub working_dir: Option<PathBuf>,
    /// `dependsOn` names another product: DLC.
    pub dlc: bool,
}

impl Source for Gog {
    fn key(&self) -> &'static str {
        SOURCE
    }

    fn name(&self) -> &'static str {
        "GOG"
    }

    fn read(&self) -> Result<Option<Found>> {
        let Some(places) = self.places() else { return Ok(None) };
        if self.root.is_some() && !places.data.is_dir() {
            bail!("{}: no such folder", places.data.display());
        }
        let records = places.records();
        if records.is_none() && !places.database().is_file() {
            return Ok(None);
        }
        let library = load(&places, records.unwrap_or_default()).with_context(|| places.data.display().to_string())?;
        Ok(Some(Found { library, from: places.data.display().to_string() }))
    }

    /// The game's own program: the `.info` file's primary play task, else what the registry says to run.
    fn start(&self, game: &Game) -> Option<Start> {
        let id = product_id(game)?;
        let dir = game.install_dir.as_deref()?;
        if let Some(task) = info(dir, id).and_then(|info| info.task) {
            return Some(task.start(dir));
        }
        let record = self.places()?.records()?.into_iter().find(|record| record.id == id)?;
        Some(Start::Program { path: record.exe?, args: split_args(&record.args), dir: record.working_dir })
    }

    /// Install: `GalaxyClient.exe /gameId=<id> /command=installGame`, where Galaxy is installed. Uninstall: the
    /// game's own `unins000.exe`, which asks the user itself.
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
        let id = product_id(game)?;
        match ask {
            Ask::Install => {
                let client = self.places()?.client?;
                let args = vec![format!("/gameId={id}"), "/command=installGame".into()];
                Some(Start::Program { path: client, args, dir: None })
            }
            Ask::Uninstall => {
                let dir = game.install_dir.as_deref()?;
                let uninstaller = dir.join("unins000.exe");
                uninstaller.is_file().then(|| Start::Program {
                    path: uninstaller,
                    args: Vec::new(),
                    dir: Some(dir.into()),
                })
            }
        }
    }
}

/// The game's product id, if it is one of GOG's.
fn product_id(game: &Game) -> Option<&str> {
    (game.source == SOURCE && !game.source_id.is_empty()).then_some(game.source_id.as_str())
}

/// Reads GOG's games: those `records` list as installed, and with Galaxy's database the rest the account owns,
/// with play time and cached art for both.
pub fn load(places: &Places, records: Vec<Record>) -> Result<Library> {
    let galaxy = match places.database() {
        db if db.is_file() => galaxy::read(&db, &places.data).unwrap_or_else(|e| {
            // The installed games are still GOG's to show.
            eprintln!("gog: {}: {e:#}", db.display());
            galaxy::Galaxy::default()
        }),
        _ => galaxy::Galaxy::default(),
    };
    let mut seen = HashSet::new();
    let mut games = Vec::new();
    for record in records {
        if record.dlc || !record.path.is_dir() || seen.contains(&record.id) {
            continue;
        }
        let info = info(&record.path, &record.id);
        if info.as_ref().is_some_and(|info| !info.is_game()) {
            continue;
        }
        let name = Some(record.name.clone())
            .filter(|name| !name.is_empty())
            .or_else(|| info.map(|info| info.name).filter(|name| !name.is_empty()))
            .or_else(|| galaxy.titles.get(&record.id).cloned());
        let Some(name) = name else { continue };
        seen.insert(record.id.clone());
        games.push(galaxy.game(&record.id, name, Some(record.path)));
    }
    for id in &galaxy.owned {
        if seen.contains(id) {
            continue;
        }
        let Some(name) = galaxy.titles.get(id) else { continue };
        seen.insert(id.clone());
        games.push(galaxy.game(id, name.clone(), None));
    }
    games.sort_by(|a, b| {
        b.last_played
            .cmp(&a.last_played)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a.source_id.cmp(&b.source_id))
    });
    Ok(Library { games, ..Library::default() })
}

/// The games installed in the folders under `dir`: each folder's `goggame-<id>.info` files that are games'.
fn folders(dir: &Path) -> Vec<Record> {
    let mut folders: Vec<PathBuf> =
        fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    folders.sort();
    let mut records = Vec::new();
    for folder in folders {
        let mut files: Vec<PathBuf> = fs::read_dir(&folder).into_iter().flatten().flatten().map(|e| e.path()).collect();
        files.sort();
        for file in files {
            let name = file.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
            let Some(id) = name.strip_prefix("goggame-").and_then(|rest| rest.strip_suffix(".info")) else { continue };
            let Some(info) = fs::read_to_string(&file).ok().and_then(|text| parse_info(&text)) else { continue };
            if info.id == id && info.is_game() {
                records.push(Record { id: info.id, name: info.name, path: folder.clone(), ..Record::default() });
            }
        }
    }
    records
}

/// What a `goggame-<id>.info` file says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    pub id: String,
    /// The game this is part of: its own id for a game, its game's for DLC.
    pub root_id: String,
    pub name: String,
    /// The primary play task, if it is a program: a game has one, DLC none.
    pub task: Option<Task>,
}

impl Info {
    /// A game, not DLC: it has a primary play task and is its own root.
    pub fn is_game(&self) -> bool {
        self.task.is_some() && (self.root_id.is_empty() || self.root_id == self.id)
    }
}

/// A `FileTask`: a program in the game's folder, its working folder and its arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Task {
    /// Relative to the game's folder.
    pub path: String,
    pub working_dir: String,
    pub arguments: String,
}

impl Task {
    /// The task, run from the game's folder `dir`.
    pub fn start(&self, dir: &Path) -> Start {
        let working = Some(&self.working_dir).filter(|working| !working.is_empty()).map(|working| join(dir, working));
        Start::Program { path: join(dir, &self.path), args: split_args(&self.arguments), dir: working }
    }
}

/// The `.info` file of `id` in the folder `dir`, if it is there and is one.
pub fn info(dir: &Path, id: &str) -> Option<Info> {
    parse_info(&fs::read_to_string(dir.join(format!("goggame-{id}.info"))).ok()?)
}

/// A `goggame-<id>.info` file's text.
pub fn parse_info(text: &str) -> Option<Info> {
    let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let id_of = |key: &str| match value.get(key)? {
        Value::String(id) => Some(id.clone()),
        Value::Number(id) => Some(id.to_string()),
        _ => None,
    };
    let text = |of: &Value, key: &str| of.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let tasks = value.get("playTasks").and_then(Value::as_array);
    let primary = tasks.into_iter().flatten().find(|task| {
        task.get("isPrimary").and_then(Value::as_bool) == Some(true)
            && task.get("type").and_then(Value::as_str) == Some("FileTask")
    });
    Some(Info {
        id: id_of("gameId")?,
        root_id: id_of("rootGameId").unwrap_or_default(),
        name: text(&value, "name"),
        task: primary.map(|task| Task {
            path: text(task, "path"),
            working_dir: text(task, "workingDir"),
            arguments: text(task, "arguments"),
        }),
    })
}

/// `relative` under `dir`, whichever way its separators lean.
fn join(dir: &Path, relative: &str) -> PathBuf {
    relative
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .fold(dir.to_path_buf(), |p, s| p.join(s))
}

/// A command line split into its arguments: at white space outside double quotes, the quotes taken away; `\"` is
/// a quote.
pub fn split_args(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let (mut arg, mut quoted, mut any) = (String::new(), false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'"') => {
                arg.push('"');
                chars.next();
                any = true;
            }
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    args.push(std::mem::take(&mut arg));
                    any = false;
                }
            }
            c => {
                arg.push(c);
                any = true;
            }
        }
    }
    if any {
        args.push(arg);
    }
    args
}

#[cfg(windows)]
mod registry {
    //! `HKLM\SOFTWARE\WOW6432Node\GOG.com`: the 32-bit view, where GOG's installers and Galaxy write.
    use std::ffi::c_void;
    use std::path::PathBuf;

    use super::Record;

    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
    const KEY_READ: u32 = 0x2_0019;
    const KEY_WOW64_32KEY: u32 = 0x0200;
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(hkey: isize, subkey: *const u16, options: u32, sam: u32, result: *mut isize) -> i32;
        fn RegEnumKeyExW(
            hkey: isize,
            index: u32,
            name: *mut u16,
            name_len: *mut u32,
            reserved: *mut u32,
            class: *mut u16,
            class_len: *mut u32,
            written: *mut c_void,
        ) -> i32;
        fn RegGetValueW(
            hkey: isize,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            ty: *mut u32,
            data: *mut c_void,
            len: *mut u32,
        ) -> i32;
        fn RegCloseKey(hkey: isize) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// An open key, closed when dropped.
    struct Key(isize);

    impl Key {
        /// `HKLM\SOFTWARE\<path>` in the 32-bit view.
        fn open(path: &str) -> Option<Key> {
            let path = wide(&format!("SOFTWARE\\{path}"));
            let mut key = 0isize;
            // SAFETY: the path is a valid wide string, `key` a place for the handle.
            let rc =
                unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_READ | KEY_WOW64_32KEY, &mut key) };
            (rc == 0).then_some(Key(key))
        }

        /// The names of its subkeys.
        fn subkeys(&self) -> Vec<String> {
            let mut names = Vec::new();
            for index in 0.. {
                let mut name = [0u16; 256];
                let mut len = name.len() as u32;
                use std::ptr::null_mut as null;
                // SAFETY: `name` holds `len` characters; the rest may be null.
                let rc = unsafe {
                    RegEnumKeyExW(self.0, index, name.as_mut_ptr(), &mut len, null(), null(), null(), null())
                };
                if rc != 0 {
                    break;
                }
                names.push(String::from_utf16_lossy(&name[..len as usize]));
            }
            names
        }

        /// A string value of the key's subkey `sub` (the key itself when empty).
        fn text(&self, sub: &str, value: &str) -> Option<String> {
            let (sub, value) = (wide(sub), wide(value));
            let mut buf = vec![0u16; 2048];
            let mut len = (buf.len() * 2) as u32;
            // SAFETY: all pointers are valid for the call; `len` is the buffer size in bytes.
            let rc = unsafe {
                RegGetValueW(
                    self.0,
                    sub.as_ptr(),
                    value.as_ptr(),
                    RRF_RT_REG_SZ,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr().cast(),
                    &mut len,
                )
            };
            if rc != 0 {
                return None;
            }
            let chars = &buf[..(len as usize / 2).min(buf.len())];
            let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
            Some(String::from_utf16_lossy(&chars[..end]))
        }
    }

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle came from RegOpenKeyExW and is closed once.
            unsafe { RegCloseKey(self.0) };
        }
    }

    /// Every product under `GOG.com\Games`; None when there is no such key.
    pub fn games() -> Option<Vec<Record>> {
        let key = Key::open("GOG.com\\Games")?;
        let records = key
            .subkeys()
            .into_iter()
            .filter_map(|sub| {
                let text = |value: &str| key.text(&sub, value).filter(|text| !text.is_empty());
                let id = text("gameID").unwrap_or_else(|| sub.clone());
                Some(Record {
                    name: text("gameName").unwrap_or_default(),
                    path: PathBuf::from(text("path")?),
                    exe: text("exe").or_else(|| text("launchCommand")).map(PathBuf::from),
                    args: text("launchParam").unwrap_or_default(),
                    working_dir: text("workingDir").map(PathBuf::from),
                    dlc: text("dependsOn").is_some(),
                    id,
                })
            })
            .collect();
        Some(records)
    }

    /// `GOG.com\GalaxyClient\paths`, `client`: Galaxy's folder.
    pub fn client() -> Option<PathBuf> {
        Key::open("GOG.com\\GalaxyClient\\paths")?.text("", "client").filter(|dir| !dir.is_empty()).map(PathBuf::from)
    }
}

#[cfg(not(windows))]
mod registry {
    use std::path::PathBuf;

    use super::Record;

    pub fn games() -> Option<Vec<Record>> {
        None
    }

    pub fn client() -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn info_text(id: &str, root: &str, primary: bool) -> String {
        json!({
            "gameId": id, "rootGameId": root, "name": format!("Game {id}"), "language": "English",
            "playTasks": [
                { "isPrimary": primary, "type": "FileTask", "path": "bin\\game.exe", "workingDir": "bin", "arguments": "-windowed \"save slot\"", "name": "Play" },
                { "isPrimary": false, "type": "URLTask", "link": "https://example.invalid/manual", "name": "Manual" },
            ],
        })
        .to_string()
    }

    #[test]
    fn an_info_file_tells_a_game_from_dlc_and_says_what_to_run() {
        let game = parse_info(&info_text("1900000042", "1900000042", true)).unwrap();
        assert!(game.is_game());
        assert_eq!(game.name, "Game 1900000042");
        let task = game.task.clone().unwrap();
        assert_eq!(
            task.start(Path::new("D:/GOG/Saltmarsh")),
            Start::Program {
                path: Path::new("D:/GOG/Saltmarsh").join("bin").join("game.exe"),
                args: vec!["-windowed".into(), "save slot".into()],
                dir: Some(Path::new("D:/GOG/Saltmarsh").join("bin")),
            }
        );
        assert!(!parse_info(&info_text("1900000043", "1900000042", true)).unwrap().is_game());
        assert!(!parse_info(&info_text("1900000042", "1900000042", false)).unwrap().is_game());
        // The id as a number, a byte-order mark, no root: still a game.
        let number = "\u{feff}{\"gameId\": 7, \"name\": \"Seven\", \"playTasks\": [{\"isPrimary\": true, \"type\": \"FileTask\", \"path\": \"seven.exe\"}]}";
        let seven = parse_info(number).unwrap();
        assert_eq!((seven.id.as_str(), seven.is_game()), ("7", true));
        assert_eq!(
            seven.task.unwrap().start(Path::new("G")),
            Start::Program { path: Path::new("G").join("seven.exe"), args: vec![], dir: None }
        );
        assert_eq!(parse_info("not json"), None);
        assert_eq!(parse_info("{\"name\": \"no id\"}"), None);
    }

    #[test]
    fn a_command_line_splits_at_spaces_outside_quotes() {
        assert_eq!(split_args(""), Vec::<String>::new());
        assert_eq!(split_args("  -a  -b "), ["-a", "-b"]);
        assert_eq!(split_args(r#"-save "slot one" "" -say=\"hi\""#), ["-save", "slot one", "", "-say=\"hi\""]);
        assert_eq!(split_args(r#"-path="C:\Games\x y""#), [r"-path=C:\Games\x y"]);
    }

    #[test]
    fn installed_games_come_from_their_folders_and_dlc_is_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let games = dir.path().join("Games");
        let (a, b) = (games.join("A"), games.join("B"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("goggame-1.info"), info_text("1", "1", true)).unwrap();
        fs::write(a.join("goggame-2.info"), info_text("2", "1", true)).unwrap();
        fs::write(b.join("goggame-3.info"), info_text("4", "4", true)).unwrap();
        fs::write(b.join("readme.txt"), "").unwrap();
        let records = folders(&games);
        assert_eq!(records, [Record { id: "1".into(), name: "Game 1".into(), path: a, ..Record::default() }]);
        assert!(folders(&dir.path().join("none")).is_empty());
    }

    #[test]
    fn registry_records_are_games_unless_dlc_gone_or_their_info_says_otherwise() {
        let dir = tempfile::tempdir().unwrap();
        let places = Places::under(dir.path());
        let folder = |name: &str| {
            let path = dir.path().join("Installed").join(name);
            fs::create_dir_all(&path).unwrap();
            path
        };
        let record = |id: &str, name: &str, path: PathBuf| Record {
            id: id.into(),
            name: name.into(),
            path,
            ..Record::default()
        };
        let (with_info, dlc_info) = (folder("WithInfo"), folder("DlcInfo"));
        fs::write(with_info.join("goggame-10.info"), info_text("10", "10", true)).unwrap();
        fs::write(dlc_info.join("goggame-11.info"), info_text("11", "10", true)).unwrap();
        let records = vec![
            record("10", "", with_info),
            record("11", "Its DLC", dlc_info),
            Record { dlc: true, ..record("12", "Marked DLC", folder("Marked")) },
            record("13", "No info", folder("NoInfo")),
            record("14", "Uninstalled", dir.path().join("gone")),
            record("13", "Twice", folder("Twice")),
        ];
        let library = load(&places, records).unwrap();
        let said: Vec<(&str, &str)> = library.games.iter().map(|g| (g.source_id.as_str(), g.name.as_str())).collect();
        assert_eq!(said, [("10", "Game 10"), ("13", "No info")]);
        assert!(library.games.iter().all(|g| g.source == SOURCE && g.installed && g.install_dir.is_some()));
    }

    #[test]
    fn gog_as_a_source() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("gog");
        let spec = fixture::Fixture::standard();
        fixture::write(&root, &spec).unwrap();
        let gog = Gog::at(Some(root.clone()));
        assert_eq!((gog.key(), gog.name()), ("gog", "GOG"));
        let found = gog.read().unwrap().expect("GOG");
        assert_eq!(found.from, root.display().to_string());

        let installed = found.library.games.iter().find(|g| g.installed).unwrap();
        let fake = spec.games.iter().find(|g| g.id == installed.source_id).unwrap();
        let dir = fake.install_dir(&root);
        assert_eq!(
            gog.start(installed),
            Some(Start::Program { path: dir.join(fixture::EXE), args: vec![], dir: None })
        );
        // No Galaxy in the fixture, and no uninstaller in the folder.
        assert_eq!((gog.ask(installed, Ask::Install), gog.ask(installed, Ask::Uninstall)), (None, None));
        fs::write(dir.join("unins000.exe"), b"").unwrap();
        let uninstall = Start::Program { path: dir.join("unins000.exe"), args: vec![], dir: Some(dir.clone()) };
        assert_eq!(gog.ask(installed, Ask::Uninstall), Some(uninstall));
        fs::create_dir_all(root.join("Client")).unwrap();
        fs::write(root.join("Client").join("GalaxyClient.exe"), b"").unwrap();
        assert_eq!(
            gog.ask(installed, Ask::Install).map(|start| start.to_string()),
            Some(format!(
                "{} /gameId={} /command=installGame",
                root.join("Client").join("GalaxyClient.exe").display(),
                fake.id
            ))
        );
        assert!(!gog.running(installed));
        let other = Game { source: "epic".into(), ..installed.clone() };
        assert_eq!((gog.start(&other), gog.ask(&other, Ask::Install)), (None, None));
        let elsewhere = Game { install_dir: None, ..installed.clone() };
        assert_eq!(gog.start(&elsewhere), None);
    }

    #[test]
    fn nothing_of_gogs_is_none_and_a_named_folder_that_is_not_there_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        // A folder with no games and no Galaxy: still read, as the registry's key with no games under it is.
        let read = Gog::at(Some(dir.path().to_path_buf())).read().unwrap().unwrap();
        assert!(read.library.games.is_empty());
        let missing = dir.path().join("no gog here");
        let error = Gog::at(Some(missing.clone())).read().unwrap_err();
        assert!(format!("{error:#}").contains(&missing.display().to_string()), "{error:#}");
        if cfg!(not(windows)) {
            assert_eq!(Gog::at(None).read().unwrap(), None);
        }
    }
}
