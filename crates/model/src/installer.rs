//! The one seam every store's install plugs into (docs/research/installs.md, 10.1; docs/sources.md "Installs"): a
//! store's own way to install a game to a folder, uninstall it, and steer an install under way (pause, resume,
//! cancel), with nothing of the store's in the Dashboard's way. A source that has one gives it from
//! `Source::installer`; one that has none is asked as before, by the address or program `Source::ask` gives.
//!
//! How far an install has got is not here: `Source::installs` stays the one place progress comes from, whatever
//! route started the install.
//!
//! An installer acts through an [`Opener`], the seam between the Dashboard and Windows that opens an address or
//! runs a program (`shell::store::Addresses`), so that a test sees what it was asked and nothing is run. A route of
//! its own that needs no program (a client's local API) acts directly and is given the opener all the same.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Game, Start};

/// Opens an address with whatever program is registered for it, or runs a program. Says why when nothing took it.
pub trait Opener {
    fn open(&mut self, start: &Start) -> Result<(), String>;

    /// An opener of its own, the same as this one, that a thread may keep: for an installer whose own route answers
    /// later, on a thread of its own, and falls back to the store's way only then (Steam's client over its
    /// debugger, which may not answer). None (the default) where there is none to give: the installer then takes
    /// the store's way at once.
    fn detached(&self) -> Option<Box<dyn Opener + Send>> {
        None
    }

    /// Puts the Dashboard's window in front again, where a store's own window has come up over it (Steam's main
    /// window, as its install wizard brings it up): the window that was in front as this opener was made. Nothing
    /// where it is in front already, and nothing by default.
    fn front(&mut self) {}

    /// Puts a licence agreement the store asks to be accepted before it installs `game` before the user, in the
    /// Dashboard's own prompt (#230), and waits for the user's answer: blocking, on the installer's own thread.
    /// Only the user's own Accept answers [`Reply::Accept`]; with nobody to ask (the default, a test, the Dashboard
    /// closing) it is [`Reply::Decline`], so nothing is ever accepted for the user.
    fn agreement(&mut self, _game: &Game, _agreement: &Agreement) -> Reply {
        Reply::Decline
    }

    /// Tells the user that the install of `game` stopped before it started, and why (#230): a Toast and the
    /// Queue's row. Nothing by default.
    fn stopped(&mut self, _game: &Game, _stop: &Stop) {}
}

/// A licence agreement a store asks the user to accept before it installs a game (Steam's EULAs, #230), as the
/// Dashboard's prompt shows it: which of how many, and its text as read from the store's own page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agreement {
    /// Its place among the game's agreements, from 1, and how many there are: they are shown one after another.
    pub at: usize,
    pub of: usize,
    /// Its text, as plain text; or, where it could not be had, why (the prompt then offers no Accept).
    pub text: Result<String, String>,
}

/// The user's answer to a licence agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Reply {
    /// The user's own A on Accept.
    Accept,
    /// Decline, B, or the prompt gone with no answer: the install is cancelled.
    Decline,
    /// The text could not be had: the install is left to the store's own window, where the user reads it there.
    OpenInStore,
}

/// Why an install stopped before it started, or left the Queue without installing (#230): what the Toast and the
/// Queue's row tell the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stop {
    /// Not enough room, by the store's own figures: the bytes it needs and the bytes free where it was going.
    NoSpace { needed: u64, free: u64 },
    /// The user declined its licence agreement.
    Declined,
    /// The store asks for something only its own window takes: what, as the line says it ("a product key").
    StoreAsks(String),
    /// The store did not start it: why, in the user's words.
    Failed(String),
    /// The user cancelled it here.
    Cancelled,
    /// It was stopped in the store's own client, or at the store's own install dialog.
    InStore,
    /// It was asked for and the store never started it.
    NeverStarted,
    /// The store no longer lists the game.
    Gone,
}

impl Stop {
    /// The Toast's line, and the first line of its row in the Queue: a few words.
    pub fn headline(&self, store: &str) -> String {
        match self {
            Stop::NoSpace { .. } => "Not enough space".into(),
            Stop::Declined => "Licence agreement declined".into(),
            Stop::StoreAsks(_) => format!("Install it in {store}"),
            Stop::Failed(_) => "Couldn't install".into(),
            Stop::Cancelled => "Installation cancelled".into(),
            Stop::InStore => format!("Stopped in {store}"),
            Stop::NeverStarted => format!("{store} didn't start it"),
            Stop::Gone => format!("No longer in {store}"),
        }
    }

    /// Whether it keeps a row under Stopped in Manage Queue: only what stopped without the user asking. The user's
    /// own Cancel or Decline, and a drop in the store's own client (as a player cancels there), are told in a
    /// Toast, and the row simply goes.
    pub fn keeps_row(&self) -> bool {
        !matches!(self, Stop::Cancelled | Stop::Declined | Stop::InStore)
    }

    /// The row's last line: what happened, and what the user can do, the sizes as the console writes them.
    pub fn line(&self, store: &str) -> String {
        match self {
            Stop::NoSpace { needed, free } => format!(
                "It needs {} and {} is free. Free up space in Settings > Storage, or install it somewhere else.",
                crate::storage::size(*needed),
                crate::storage::size(*free)
            ),
            Stop::Declined => "Nothing was installed.".into(),
            Stop::StoreAsks(what) => {
                format!("{store} asked for {what}, which the Dashboard can't give. Install it in {store}.")
            }
            Stop::Failed(why) => why.clone(),
            Stop::Cancelled => "What had downloaded was deleted.".into(),
            Stop::InStore => format!("{store} stopped installing it."),
            Stop::NeverStarted => format!("{store} was asked and never started installing it."),
            Stop::Gone => format!("{store} no longer lists it."),
        }
    }
}

/// What a store's installer can do on this machine, as it is now: each false where the route it would take is not
/// there (a client's debugger off, a helper not added, a call not built yet). The Queue's menu rows follow it: a
/// row for what the store cannot do is not shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Can {
    /// Pause a download under way, and resume one paused.
    pub pause: bool,
    /// Cancel an install under way, deleting what was fetched.
    pub cancel: bool,
    /// Install into a folder of [`Installer::folders`] other than its default.
    pub choose_folder: bool,
    /// Uninstall with nothing of the store's or the game's asking the user anything.
    pub quiet_uninstall: bool,
}

impl Can {
    /// Nothing but to install and uninstall as the store does.
    pub const NOTHING: Can = Can { pause: false, cancel: false, choose_folder: false, quiet_uninstall: false };
}

/// A folder a store may install into: a Steam library, a drive's games folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub path: PathBuf,
    /// Bytes free on its drive, where known.
    pub free: Option<u64>,
    /// The store's own default: where an install goes with no folder chosen.
    pub default: bool,
}

/// What the Queue asks of an install under way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Steer {
    Pause,
    Resume,
    /// Stop it and delete what was fetched: the game leaves the Queue not installed.
    Cancel,
}

impl Steer {
    /// The verb, as the log says it.
    pub fn verb(self) -> &'static str {
        match self {
            Steer::Pause => "pause",
            Steer::Resume => "resume",
            Steer::Cancel => "cancel",
        }
    }

    /// Whether a store that can do `can` can be asked this.
    pub fn allowed(self, can: Can) -> bool {
        match self {
            Steer::Pause | Steer::Resume => can.pause,
            Steer::Cancel => can.cancel,
        }
    }
}

/// Why an installer did not do what it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The store's route cannot do it here (the default for everything an installer does not build): the Dashboard
    /// falls back to the store's own way (`Source::ask`) where there is one, else says so.
    Unsupported,
    /// It was tried and failed: why.
    Failed(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Unsupported => f.write_str("not supported"),
            Refusal::Failed(why) => f.write_str(why),
        }
    }
}

/// A store's own way to install, uninstall and steer installs. Every method but [`can`](Installer::can) has a
/// default that refuses as [`Refusal::Unsupported`], so a store builds only what its route can do; `can` must say
/// the same. Each call returns once the store has been asked, not once it has done it: the Queue follows what
/// happens from `Source::installs`.
pub trait Installer: Send + Sync {
    /// What it can do now.
    fn can(&self) -> Can;

    /// Where it may install: each folder with its free space where known, one of them the default. Empty where the
    /// store chooses for itself. With more than one, and `Can::choose_folder`, Install asks the user which, starting
    /// on the default Settings > Storage keeps (docs/spec/storage.md).
    fn folders(&self) -> Vec<Folder> {
        Vec::new()
    }

    /// Install the game: into `folder`, one of [`folders`](Installer::folders), or the store's default with none.
    fn install(&self, _game: &Game, _folder: Option<&Folder>, _open: &mut dyn Opener) -> Result<(), Refusal> {
        Err(Refusal::Unsupported)
    }

    fn uninstall(&self, _game: &Game, _open: &mut dyn Opener) -> Result<(), Refusal> {
        Err(Refusal::Unsupported)
    }

    /// Pause, resume or cancel the game's install under way.
    fn steer(&self, _game: &Game, _steer: Steer, _open: &mut dyn Opener) -> Result<(), Refusal> {
        Err(Refusal::Unsupported)
    }

    /// Brings the store's list of the games the account owns up to date where only the network has it (a helper's
    /// own listing, legendary's `status`), so that reading the source, which reads only files, finds it: blocking,
    /// and never on the window's thread. Whether it did, and the source is to be read again. False (the default):
    /// nothing to do.
    fn refresh(&self) -> bool {
        false
    }
}

/// The folder an install goes to while the Dashboard asks nothing: the default among `folders`, none when the
/// store chooses for itself: where an install goes that was given no folder. Settings > Storage keeps the user's own
/// choice (`model::storage`), which the executable passes as the folder.
pub fn default_folder(folders: &[Folder]) -> Option<&Folder> {
    folders.iter().find(|folder| folder.default)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Nothing;

    impl Installer for Nothing {
        fn can(&self) -> Can {
            Can::NOTHING
        }
    }

    struct Kept(Vec<String>);

    impl Opener for Kept {
        fn open(&mut self, start: &Start) -> Result<(), String> {
            self.0.push(start.to_string());
            Ok(())
        }
    }

    #[test]
    fn an_installer_refuses_what_it_does_not_build_and_opens_nothing() {
        let (game, mut kept) = (Game::default(), Kept(Vec::new()));
        assert_eq!(Nothing.install(&game, None, &mut kept), Err(Refusal::Unsupported));
        assert_eq!(Nothing.uninstall(&game, &mut kept), Err(Refusal::Unsupported));
        for steer in [Steer::Pause, Steer::Resume, Steer::Cancel] {
            assert_eq!(Nothing.steer(&game, steer, &mut kept), Err(Refusal::Unsupported));
            assert!(!steer.allowed(Nothing.can()));
        }
        assert!(kept.0.is_empty() && Nothing.folders().is_empty());
    }

    #[test]
    fn pause_and_resume_go_together_and_cancel_alone() {
        let pause = Can { pause: true, ..Can::NOTHING };
        assert!(Steer::Pause.allowed(pause) && Steer::Resume.allowed(pause) && !Steer::Cancel.allowed(pause));
        assert!(Steer::Cancel.allowed(Can { cancel: true, ..Can::NOTHING }));
    }

    #[test]
    fn the_default_folder_is_the_one_marked_so() {
        let folder = |path: &str, default| Folder { path: path.into(), free: None, default };
        let folders = [folder("D:/Games", false), folder("C:/Steam", true)];
        assert_eq!(default_folder(&folders).map(|f| f.path.clone()), Some("C:/Steam".into()));
        assert_eq!(default_folder(&[]), None);
    }
}
