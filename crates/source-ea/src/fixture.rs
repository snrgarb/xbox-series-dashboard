//! Writes a fake EA app: the registry keys and files `Ea` reads, with made-up games, for the tests here and for
//! the end-to-end tests' stand-in (`standin make-ea <dir>`), read with `--ea <dir>`.
//!
//! At `<dir>`: `registry.json` (the registry, as `source_kit::registry` reads a file), and each game's folder under
//! `games/` with its `__Installer\installerdata.xml` and, given a program, that program as `game.exe`, the program
//! the file names. The keys are as the EA app writes them (docs/research/stores.md, "EA app"); every name, id and
//! path in them is made up.
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use source_kit::registry::{Registry, Values};

/// The fake's registry, under its folder.
pub const REGISTRY: &str = "registry.json";

/// A fake EA app to write.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub games: Vec<FixtureGame>,
    /// A program copied into every installed game's folder as `game.exe`. Without one the folders have none.
    pub game_exe: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct FixtureGame {
    pub content_id: String,
    pub name: String,
    pub kind: Kind,
}

/// What the registry and the disk say of a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Installed: its key, its folder, its `installerdata.xml`.
    Installed,
    /// Another program's Uninstall key: its uninstaller is not EA's.
    NotEas,
    /// EA's key, but its folder is gone.
    FolderGone,
    /// EA's key and its folder, with no `installerdata.xml` in it.
    NoInstallerData,
}

impl Fixture {
    /// Two installed games, Cinderpeak Rally (1099001) and Vantage Reef (1099002); and three keys `Ea` passes
    /// over: a program that is not EA's, an EA game whose folder is gone, and one with no `installerdata.xml`.
    pub fn standard() -> Fixture {
        let game = |id: &str, name: &str, kind| FixtureGame { content_id: id.into(), name: name.into(), kind };
        Fixture {
            games: vec![
                game("1099001", "Cinderpeak Rally", Kind::Installed),
                game("1099002", "Vantage Reef", Kind::Installed),
                game("1099003", "Made Up Paint Tool", Kind::NotEas),
                game("1099004", "Lantern Tides", Kind::FolderGone),
                game("1099005", "Quarry Bells", Kind::NoInstallerData),
            ],
            game_exe: None,
        }
    }

    /// The names of the games `Ea` reads from it, as it lists them.
    pub fn expected(&self) -> Vec<&str> {
        let mut names: Vec<&str> =
            self.games.iter().filter(|g| g.kind == Kind::Installed).map(|g| g.name.as_str()).collect();
        names.sort_by_key(|name| name.to_lowercase());
        names
    }
}

/// Writes `spec` at `root`.
pub fn write(root: &Path, spec: &Fixture) -> io::Result<()> {
    let root = std::path::absolute(root)?;
    std::fs::create_dir_all(&root)?;
    let mut keys: BTreeMap<String, Values> = BTreeMap::new();
    let mut key = |name: String, values: &[(&str, String)]| {
        keys.insert(name, values.iter().map(|(k, v)| (k.to_string(), v.clone())).collect());
    };
    key(
        r"HKLM\SOFTWARE\Electronic Arts\EA Desktop".into(),
        &[("InstallLocation", root.join("client").display().to_string())],
    );
    let uninstall = r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
    for (i, game) in spec.games.iter().enumerate() {
        let folder = root.join("games").join(&game.name);
        let cleanup = root.join("EAInstaller").join(&game.name).join("Cleanup.exe");
        let uninstaller = match game.kind {
            Kind::NotEas => format!("MsiExec.exe /X{{00000000-0000-4000-8000-00000000000{i}}}"),
            _ => format!("\"{}\" uninstall_game -autologging", cleanup.display()),
        };
        let publisher = if game.kind == Kind::NotEas { "Made Up Software" } else { "Electronic Arts" };
        key(
            format!(
                r"{uninstall}\{{7A1C2E3F-4B5D-4E6F-8A9B-{:012}}}",
                game.content_id.parse::<u64>().unwrap_or(i as u64)
            ),
            &[
                ("DisplayName", game.name.clone()),
                ("Publisher", publisher.into()),
                ("InstallLocation", format!("{}{}", folder.display(), std::path::MAIN_SEPARATOR)),
                ("DisplayIcon", format!("\"{}\"", folder.join("game.exe").display())),
                ("UninstallString", uninstaller),
                ("EstimatedSize", "2048000".into()),
            ],
        );
        if game.kind == Kind::FolderGone {
            continue;
        }
        let exe = if game.kind == Kind::Installed { spec.game_exe.as_deref() } else { None };
        source_kit::write_game_folder(&folder, &game.name, exe)?;
        if game.kind == Kind::Installed {
            // The bracket names the registry value that holds the game's folder, as EA's installer writes it.
            let value = format!(r"HKEY_LOCAL_MACHINE\SOFTWARE\Made Up Studio\{}", game.name);
            key(
                format!(r"HKLM\SOFTWARE\Made Up Studio\{}", game.name),
                &[("Install Dir", folder.display().to_string())],
            );
            let installer = folder.join("__Installer");
            std::fs::create_dir_all(&installer)?;
            std::fs::write(installer.join("installerdata.xml"), installer_data(game, &value))?;
        }
    }
    Registry::write(&root.join(REGISTRY), &keys)
}

fn installer_data(game: &FixtureGame, value: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<DiPManifest version="4.0">
  <contentIDs>
    <contentID>{id}</contentID>
  </contentIDs>
  <gameTitles>
    <gameTitle locale="en_US">{name}</gameTitle>
  </gameTitles>
  <runtime>
    <launcher>
      <filePath>[{value}\Install Dir]game.exe</filePath>
      <parameters></parameters>
      <executeElevated>0</executeElevated>
      <requires64BitOS>1</requires64BitOS>
      <trial>0</trial>
    </launcher>
  </runtime>
</DiPManifest>
"#,
        id = game.content_id,
        name = game.name.replace('&', "&amp;"),
    )
}
