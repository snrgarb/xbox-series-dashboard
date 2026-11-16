//! The EA app's installed games (docs/research/stores.md, "EA app"; docs/sources.md).
//!
//! - **Installed:** the Uninstall keys (both hives, both views) whose `UninstallString` runs EA's own uninstaller
//!   (it has both `EAInstaller` and `Cleanup.exe` in it): `DisplayName`, `InstallLocation`, `Publisher`,
//!   `EstimatedSize`. Each game's `__Installer\installerdata.xml` gives its content id, which is its `source_id`,
//!   and the program that starts it. A key with no such file, or whose folder is gone, is passed over.
//! - **Owned but not installed:** none. No local file lists them without EA's servers.
//! - **The client:** `HKLM\SOFTWARE\Electronic Arts\EA Desktop` (either view). With neither it nor a game's key,
//!   the EA app is not installed.
//! - **Start:** the game's own program, as `installerdata.xml` names it, in its folder: it starts the EA app itself
//!   when it needs it. With no program found, EA's link `origin2://game/launch?offerIds=<content id>`.
//! - **Install:** nothing to ask. **Uninstall:** the key's own `UninstallString`, EA's `Cleanup.exe`.
//!
//! `Ea::at(Some(dir))` reads a fake EA app instead (`--ea <dir>`, and `fixture`): its registry is
//! `<dir>/registry.json` (`source_kit::registry`), its games where that says.
pub mod fixture;
pub mod installer;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use model::{Ask, Found, Game, Library, Source, Start};
use source_kit::registry::{self, Registry, Values, UNINSTALL};

/// EA, as a game's `source` names it.
pub const SOURCE: &str = "ea";

/// Where the EA app says it is installed.
const CLIENT: [&str; 2] =
    [r"HKLM\SOFTWARE\Electronic Arts\EA Desktop", r"HKLM\SOFTWARE\WOW6432Node\Electronic Arts\EA Desktop"];

/// The EA app, as the library reads it and the shell starts its games.
#[derive(Debug, Clone, Default)]
pub struct Ea {
    /// A fake EA app to read in place of this machine's.
    fake: Option<PathBuf>,
}

impl Ea {
    /// This machine's EA app, or with `Some(dir)` the fake one at `dir` (`--ea`).
    pub fn at(fake: Option<PathBuf>) -> Self {
        Self { fake }
    }

    /// The registry to read: this machine's, or the fake's file.
    fn registry(&self) -> Result<Registry> {
        match &self.fake {
            Some(dir) => Registry::load(&dir.join(fixture::REGISTRY)).with_context(|| dir.display().to_string()),
            None => Ok(Registry::Windows),
        }
    }
}

/// One Uninstall key of EA's: an installed game.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    values: Values,
    folder: PathBuf,
}

/// Every Uninstall key whose uninstaller is EA's and that names a folder.
fn entries(registry: &Registry) -> Vec<Entry> {
    UNINSTALL
        .iter()
        .flat_map(|key| registry.subkeys(key))
        .filter_map(|(_, values)| {
            let uninstall = registry::get(&values, "UninstallString")?.to_ascii_lowercase();
            if !(uninstall.contains("eainstaller") && uninstall.contains("cleanup.exe")) {
                return None;
            }
            let folder = source_kit::native(registry::get(&values, "InstallLocation")?.trim().trim_matches('"'));
            Some(Entry { values, folder })
        })
        .collect()
}

/// The games of these keys, by their folders' `installerdata.xml`: one per content id, the first key's.
fn games(entries: &[Entry]) -> Vec<Game> {
    let mut games: Vec<Game> = Vec::new();
    for entry in entries {
        if !entry.folder.is_dir() {
            continue;
        }
        let data = match installer::read(&entry.folder) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("ea: passed over: {e:#}");
                continue;
            }
        };
        if games.iter().any(|game| game.source_id == data.content_id) {
            continue;
        }
        let display = registry::get(&entry.values, "DisplayName").map(|name| name.trim().to_string());
        let Some(name) = display.or(data.title).or_else(|| source_kit::folder_name(&entry.folder)) else { continue };
        let kb: u64 =
            registry::get(&entry.values, "EstimatedSize").and_then(|size| size.trim().parse().ok()).unwrap_or(0);
        let publishers =
            registry::get(&entry.values, "Publisher").map(|p| vec![p.trim().to_string()]).unwrap_or_default();
        games.push(Game {
            source: SOURCE.into(),
            source_id: data.content_id,
            name,
            installed: true,
            install_dir: Some(entry.folder.clone()),
            size_on_disk: kb * 1024,
            publishers,
            ..Game::default()
        });
    }
    games.sort_by_key(|game| game.name.to_lowercase());
    games
}

impl Source for Ea {
    fn key(&self) -> &'static str {
        SOURCE
    }

    fn name(&self) -> &'static str {
        "EA app"
    }

    fn read(&self) -> Result<Option<Found>> {
        let registry = self.registry()?;
        let client = CLIENT.iter().find_map(|key| registry.value(key, "InstallLocation"));
        let entries = entries(&registry);
        if self.fake.is_none() && client.is_none() && entries.is_empty() {
            return Ok(None);
        }
        let from = match (&self.fake, client) {
            (Some(dir), _) => dir.display().to_string(),
            (None, Some(client)) => client,
            (None, None) => "the registry's Uninstall keys".to_string(),
        };
        let library = Library { games: games(&entries), ..Library::default() };
        Ok(Some(Found { library, from }))
    }

    /// The game's own program where its `installerdata.xml` names one that is there; else EA's link.
    fn start(&self, game: &Game) -> Option<Start> {
        if game.source != SOURCE || game.source_id.is_empty() {
            return None;
        }
        let program = game.install_dir.as_deref().and_then(program);
        program.or_else(|| {
            Some(Start::Address(format!("origin2://game/launch?offerIds={}&autoDownload=1", game.source_id)))
        })
    }

    /// No install from outside the EA app; uninstall is the game's key's own `UninstallString`.
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
        if game.source != SOURCE || ask == Ask::Install {
            return None;
        }
        let dir = game.install_dir.as_deref()?;
        let entries = entries(&self.registry().ok()?);
        let entry = entries.iter().find(|entry| source_kit::same_folder(&entry.folder, dir))?;
        source_kit::command::program(registry::get(&entry.values, "UninstallString")?)
    }
}

/// The program `installerdata.xml` in `dir` names, when it is there, run in `dir` with its parameters.
fn program(dir: &Path) -> Option<Start> {
    let launcher = installer::read(dir).ok()?.launcher?;
    let path = dir.join(source_kit::native(&launcher.path));
    path.is_file().then(|| Start::Program {
        path,
        args: source_kit::command::words(&launcher.parameters),
        dir: Some(dir.to_path_buf()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixture::Fixture;

    #[test]
    fn the_fake_reads_back_as_its_installed_games_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ea");
        let spec = Fixture::standard();
        fixture::write(&root, &spec).unwrap();
        let ea = Ea::at(Some(root.clone()));
        assert_eq!((ea.key(), ea.name()), ("ea", "EA app"));
        let found = ea.read().unwrap().expect("a fake EA app");
        assert_eq!(found.from, root.display().to_string());
        let games = &found.library.games;
        let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
        // Not the program whose uninstaller is not EA's, nor the key whose folder is gone, nor the one with no
        // installerdata.xml.
        assert_eq!(names, ["Cinderpeak Rally", "Vantage Reef"]);
        let rally = &games[0];
        assert_eq!(rally.source_id, "1099001");
        assert!(rally.installed);
        assert_eq!(rally.install_dir.as_deref(), Some(root.join("games").join("Cinderpeak Rally").as_path()));
        assert_eq!(rally.publishers, ["Electronic Arts"]);
        assert_eq!(rally.size_on_disk, 2_048_000 * 1024);
        assert_eq!(games.len(), spec.expected().len());
    }

    #[test]
    fn a_game_starts_by_its_own_program_else_by_eas_link_and_uninstalls_by_its_key() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ea");
        let mut spec = Fixture::standard();
        // Any file stands in for the game's program.
        let exe = dir.path().join("stand-in.exe");
        std::fs::write(&exe, b"").unwrap();
        spec.game_exe = Some(exe);
        fixture::write(&root, &spec).unwrap();
        let ea = Ea::at(Some(root.clone()));
        let games = ea.read().unwrap().unwrap().library.games;
        let rally = &games[0];
        let folder = rally.install_dir.clone().unwrap();
        assert_eq!(
            ea.start(rally),
            Some(Start::Program { path: folder.join("game.exe"), args: Vec::new(), dir: Some(folder.clone()) })
        );
        let uninstall = ea.ask(rally, Ask::Uninstall).expect("its key's UninstallString");
        assert!(uninstall.target().ends_with("Cleanup.exe"), "{uninstall}");
        assert_eq!(uninstall.parameters().as_deref(), Some("uninstall_game -autologging"));
        assert_eq!(ea.ask(rally, Ask::Install), None);

        // Its program gone: EA's link.
        std::fs::remove_file(folder.join("game.exe")).unwrap();
        let link = "origin2://game/launch?offerIds=1099001&autoDownload=1";
        assert_eq!(ea.start(rally), Some(Start::Address(link.into())));
        // Not EA's: nothing.
        let steam = Game { source: "steam".into(), ..rally.clone() };
        assert_eq!((ea.start(&steam), ea.ask(&steam, Ask::Uninstall)), (None, None));
    }

    #[test]
    fn no_ea_app_is_none_and_a_fake_that_is_not_there_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no ea here");
        let error = Ea::at(Some(missing.clone())).read().unwrap_err();
        assert!(format!("{error:#}").starts_with(&missing.display().to_string()), "{error:#}");
        // An empty registry: no client, no game.
        assert!(entries(&Registry::File(Default::default())).is_empty());
        if cfg!(not(windows)) {
            assert_eq!(Ea::at(None).read().unwrap(), None);
        }
    }
}
