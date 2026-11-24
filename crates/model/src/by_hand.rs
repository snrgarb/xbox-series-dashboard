//! What the user adds by hand (docs/spec/apps.md): games no store brings in, and Apps.
//!
//! A **game added by hand** is a game like any other, of a source of its own, [`MANUAL`]: a row of the library
//! database's `games` whose `source_id` is the program's full path, its install folder the program's folder. It is
//! started as that program, followed by its windows, and has its Play sessions, Recent tile and Groups as every
//! game does.
//!
//! An **App** is not a game (the Bundle's glossary, "Apps"): it is in no list of games, so never in the Recent row,
//! a Group, Full library or Search's Games row, and is started without the Launch screen or a Play session. It is a
//! program added by hand, or a Microsoft Store app the user ticked in the list of those installed.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Start;

/// The source key of the games added by hand, in the library database's `games.source`. Never changed: the
/// user's history of them is kept under it.
pub const MANUAL: &str = "manual";

/// How a program added by hand is started: itself, with no arguments, in its own folder.
pub fn program(path: &Path) -> Start {
    Start::Program { path: path.to_path_buf(), args: Vec::new(), dir: None }
}

/// What an App is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppKind {
    /// A program added by hand: `target` is its full path.
    Program,
    /// A Microsoft Store app: `target` is its Application User Model ID (`<package family>!<app id>`).
    Store,
}

impl AppKind {
    /// Its name in the library database's `apps.kind`.
    pub fn key(self) -> &'static str {
        match self {
            AppKind::Program => "program",
            AppKind::Store => "store",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "program" => Some(AppKind::Program),
            "store" => Some(AppKind::Store),
            _ => None,
        }
    }
}

/// An App, as the library database keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct App {
    /// Its row in the library database's `apps`.
    pub id: i64,
    pub name: String,
    pub kind: AppKind,
    /// The program's path, or the Store app's AUMID.
    pub target: String,
    /// Its picture: the icon file given with a program, or a Store app's tile art. None: the tile shows the
    /// generic app glyph.
    #[serde(default)]
    pub icon: Option<PathBuf>,
    /// Unix seconds: when the Dashboard last started it; 0 never. What places it among the games played in the
    /// Guide's list (#345).
    #[serde(default)]
    pub last_opened: u64,
}

impl App {
    /// How the shell starts it: the program, or the Store app through the shell's Apps folder, as Windows' own
    /// Start menu does.
    pub fn start(&self) -> Start {
        match self.kind {
            AppKind::Program => program(Path::new(&self.target)),
            AppKind::Store => Start::Address(format!("shell:AppsFolder\\{}", self.target)),
        }
    }

    pub fn is_store(&self) -> bool {
        self.kind == AppKind::Store
    }
}

/// A Microsoft Store app installed on this machine, as the list the user ticks from has it. Not an App until it
/// is ticked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreApp {
    pub name: String,
    /// Its Application User Model ID: `Microsoft.WindowsCalculator_8wekyb3d8bbwe!App`.
    pub aumid: String,
    /// Its package's tile art, if one was found on disk.
    #[serde(default)]
    pub logo: Option<PathBuf>,
}

/// One thing added by hand, by its row: a game's in `games`, an App's in `apps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Added {
    Game(i64),
    App(i64),
}

/// A change to what is added by hand, as Settings' Games & apps pages ask for it. Whoever owns the library
/// database carries it out (`library::Database::by_hand`) and shows the snapshot after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByHand {
    /// A game: the program the user typed the path of, its name, and the art for its tiles if one was given.
    AddGame { path: PathBuf, name: String, art: Option<PathBuf> },
    /// A program as an App, with its icon if one was given.
    AddApp { path: PathBuf, name: String, icon: Option<PathBuf> },
    /// A Store app ticked in the list of those installed.
    AddStoreApp(StoreApp),
    /// A new name for a game added by hand or an App.
    Rename { item: Added, name: String },
    /// It goes: a game with its Play sessions, an App.
    Remove(Added),
}

/// A path as the user typed it: without the spaces round it, or the quotes round the whole of it that Windows'
/// "Copy as path" puts there. None when nothing is left.
pub fn typed_path(text: &str) -> Option<PathBuf> {
    let text = text.trim();
    let text = text.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')).unwrap_or(text).trim();
    (!text.is_empty()).then(|| PathBuf::from(text))
}

/// The name a program is offered under before one is typed: its file's name without the extension, as Windows
/// writes either slash. Empty for a path with no file name.
pub fn name_of(path: &Path) -> String {
    let text = path.to_string_lossy();
    let file = text.rsplit(['\\', '/']).next().unwrap_or_default();
    let stem = match file.rfind('.') {
        Some(dot) if dot > 0 => &file[..dot],
        _ => file,
    };
    stem.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_app_runs_the_program_and_a_store_app_goes_through_the_apps_folder() {
        let program = App {
            id: 1,
            name: "Notepad".into(),
            kind: AppKind::Program,
            target: r"C:\Windows\System32\notepad.exe".into(),
            icon: None,
            last_opened: 0,
        };
        assert_eq!(program.start().to_string(), r"C:\Windows\System32\notepad.exe");
        assert!(!program.is_store());
        let store =
            App { kind: AppKind::Store, target: "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into(), ..program };
        assert_eq!(
            store.start(),
            Start::Address(r"shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into())
        );
        for kind in [AppKind::Program, AppKind::Store] {
            assert_eq!(AppKind::from_key(kind.key()), Some(kind));
        }
        assert_eq!(AppKind::from_key("game"), None);
    }

    #[test]
    fn a_typed_path_loses_its_quotes_and_spaces_and_offers_its_files_name() {
        assert_eq!(typed_path("  \"C:\\Games\\Moss\\moss.exe\" "), Some(PathBuf::from(r"C:\Games\Moss\moss.exe")));
        assert_eq!(typed_path(r"C:\a b\c.exe"), Some(PathBuf::from(r"C:\a b\c.exe")));
        assert_eq!(typed_path(" \"\" "), None);
        assert_eq!(typed_path(""), None);
        assert_eq!(name_of(Path::new(r"C:\Games\Moss\moss.exe")), "moss");
        assert_eq!(name_of(Path::new("/opt/games/Tern.v2.x86_64")), "Tern.v2");
        assert_eq!(name_of(Path::new(r"C:\Tools\runner")), "runner");
        assert_eq!(name_of(Path::new(r"C:\Tools\")), "");
        assert_eq!(name_of(Path::new(".hidden")), ".hidden");
    }
}
