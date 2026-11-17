//! What the Dashboard asks of a store: the one interface every source of games is behind (docs/sources.md).
//!
//! A source is one store's client on this machine (Steam; later Epic, GOG, Xbox, the games added by hand). It
//! reads the games the client knows, with their install state and the art the client has cached, and says how to
//! start one, how to ask the client to install or uninstall one, and whether one is running. Everything else (the
//! merge of many sources into one library, Play history, the user's marks) is the library's, and the Windows side
//! of starting a game and following it is the shell's: a source only gives addresses and answers.
use std::path::PathBuf;

use crate::{Game, Install, Installer, Library};

/// What a store is asked to do with a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    Install,
    Uninstall,
}

impl Ask {
    /// The verb, as the log says it.
    pub fn verb(self) -> &'static str {
        match self {
            Ask::Install => "install",
            Ask::Uninstall => "uninstall",
        }
    }
}

/// What the shell opens to start a game, or to ask its store to install or uninstall one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// An address the store's client takes, opened as Windows opens any address: `steam://rungameid/440`.
    Address(String),
    /// A program run with its arguments, in its working folder (the program's own when none is given): a game
    /// added by hand, a store's client given a command, a game's own uninstaller.
    Program { path: PathBuf, args: Vec<String>, dir: Option<PathBuf> },
}

impl Start {
    /// What is opened: the address, or the program's path.
    pub fn target(&self) -> String {
        match self {
            Start::Address(address) => address.clone(),
            Start::Program { path, .. } => path.to_string_lossy().into_owned(),
        }
    }

    /// The program's arguments as one command line, each in quotes where it needs them; none for an address.
    pub fn parameters(&self) -> Option<String> {
        let Start::Program { args, .. } = self else { return None };
        Some(args.iter().map(|arg| quoted(arg)).collect::<Vec<_>>().join(" "))
    }

    /// The folder the program runs in: the one given, else the program's own.
    pub fn folder(&self) -> Option<PathBuf> {
        let Start::Program { path, dir, .. } = self else { return None };
        dir.clone().or_else(|| path.parent().map(PathBuf::from))
    }
}

/// How the log says it: the address, or the program and its command line.
impl std::fmt::Display for Start {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.target())?;
        match self.parameters().filter(|parameters| !parameters.is_empty()) {
            Some(parameters) => write!(f, " {parameters}"),
            None => Ok(()),
        }
    }
}

/// An argument as a command line has it: in quotes when it is empty or has a space or a quote in it.
fn quoted(arg: &str) -> String {
    if arg.is_empty() || arg.contains([' ', '\t', '"']) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

/// What a source read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    /// Every game the store's client knows the user has, installed or not: each with `source` set to the source's
    /// key and `source_id` to the store's own id for it; its install state; and its art, where the client has it on
    /// disk. The signed-in user's name and the store's collections, where it has them.
    pub library: Library,
    /// Where it was read from, as the log says it: the client's folder.
    pub from: String,
}

/// One store, as the library reads it and the shell starts its games. Made once at start, shared between the
/// thread that reads the sources and the window that starts games, so it holds no state that changes.
pub trait Source: Send + Sync {
    /// The source's name in the library database and in `Game::source`: `"steam"`. Never changed once released:
    /// the user's history is kept under it.
    fn key(&self) -> &'static str;

    /// The store's name, as the log and the screens say it: `"Steam"`.
    fn name(&self) -> &'static str;

    /// Reads the store's games from its client's own files. `Ok(None)` when its client is not installed, which is
    /// said nowhere; an error when it is but could not be read, which the library logs and passes over, leaving
    /// the source's games as they were. Called off the UI thread, at start and on return from a game.
    fn read(&self) -> anyhow::Result<Option<Found>>;

    /// How to start the game, one of this source's. None when it can't be started from here.
    fn start(&self, game: &Game) -> Option<Start>;

    /// What asks the store's client to install or uninstall the game (Steam's `steam://install/440`, a client's
    /// command, the game's own uninstaller), None when there is nothing to ask.
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start>;

    /// The store's own word that the game is running, looked at a few times a second while it is. A source whose
    /// store says nothing leaves this false: the game's windows tell.
    fn running(&self, _game: &Game) -> bool {
        false
    }

    /// The installs the store's client has under way, its own as well as those the Dashboard asked for, each with
    /// how far it has got: what the Queue is made of. `None` from a store that keeps no word of them on this machine
    /// (the default), whose games never join the Queue; `Some` of nothing from one that does, with nothing under way.
    /// An installed game being updated is not among them. Asked off the UI thread, once a second while the Queue is
    /// not empty, so keep it to small local files: no network, no `read`.
    fn installs(&self) -> Option<Vec<Install>> {
        None
    }

    /// Whether asking the store to uninstall a game (`ask`) is all it takes: nothing of the store's, or of the
    /// game's, asks the user anything, so the game should be gone within a minute or two, and a log line says so
    /// when it is not (Steam's `+app_uninstall`). Any other store's uninstall is the user's to finish in a window
    /// of its own (Settings' installed apps, a game's uninstaller), which may take as long as they like or never
    /// happen: it is followed a while longer, and the wait ends quietly. False, the default.
    fn uninstalls_by_itself(&self) -> bool {
        false
    }

    /// The store's own way to install, uninstall and steer installs (`crate::installer`), where it has one. None,
    /// the default: the store is asked by what [`ask`](Source::ask) gives, and its installs cannot be paused or
    /// cancelled from here. Where it has one, what it refuses as unsupported falls back to `ask` all the same.
    fn installer(&self) -> Option<&dyn Installer> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_is_said_with_its_command_line_and_runs_in_its_own_folder_unless_told() {
        let address = Start::Address("steam://rungameid/440".into());
        assert_eq!((address.to_string(), address.parameters(), address.folder()), (address.target(), None, None));
        let path = PathBuf::from("games").join("Moss").join("moss.exe");
        let args = vec!["-windowed".into(), "a save".into(), String::new(), "say \"hi\"".into()];
        let program = Start::Program { path: path.clone(), args, dir: None };
        let line = r#"-windowed "a save" "" "say \"hi\"""#;
        assert_eq!(program.parameters().as_deref(), Some(line));
        assert_eq!(program.to_string(), format!("{} {line}", path.display()));
        assert_eq!(program.folder(), Some(PathBuf::from("games").join("Moss")));
        let elsewhere = Start::Program { path, args: Vec::new(), dir: Some("saves".into()) };
        assert_eq!((elsewhere.folder(), elsewhere.to_string()), (Some("saves".into()), elsewhere.target()));
    }
}
