//! Xbox and Microsoft Store PC games (docs/research/stores.md, "Xbox: packaged games and Game Pass";
//! docs/sources.md). Read from the machine alone: no sign-in, no network.
//!
//! - **What is a game:** a package whose root has a `MicrosoftGame.config`. Every GDK game has one, and no app does;
//!   older UWP games from the Store have none and are not listed (they can be added by hand as Apps).
//! - **Where the packages are:** two ways, both read. The folders each drive's `.GamingRoot` names (by default
//!   `XboxGames`), each game's package in `<folder>\<game>\Content`, and `<drive>\Program Files\ModifiableWindowsApps`;
//!   and the package manager's list of the packages registered for this user, which finds the games still under
//!   `WindowsApps` (a folder no user can list) and those registered from anywhere else. A game in a folder that the
//!   package manager does not list for this user (another user's) is passed over: it cannot be started.
//! - **Each game:** `source_id` its package family name, which stays through updates; its name from the config's
//!   `ShellVisuals`, else the manifest's (an `ms-resource:` name looked up in the package's resources); its install
//!   folder the package's root, where its processes' programs are, which is how the shell follows its windows. Size on
//!   disk is not known without walking the whole folder, and is left at 0.
//! - **Art,** from the package's own logos (the largest scale where the plain file is absent): **square** the large
//!   tile's logo (the config's `Square480x480Logo`, else the manifest's `Square310x310Logo`); the 150, the store logo
//!   and the 44 are icons, taken only for a game with no product in the Store; **hero** the config's
//!   `SplashScreenImage` (1920x1080 in a GDK game), else the manifest's splash screen; **header** the manifest's
//!   `Wide310x150Logo`; no **cover**: a package has nothing portrait. The config's `StoreId` is `OnlineArt::product`,
//!   by which the art lookup asks the Microsoft Store's catalogue for the rest (#110; docs/sources.md, "Art").
//! - **Not installed:** nothing. What the user owns, and the Game Pass catalogue, are Xbox's servers' to say.
//! - **Start:** the app user model ID, `<family name>!<application id>`, as an address: `shell:AppsFolder\<AUMID>`,
//!   which Windows activates as the Start menu does. Never the game's exe, which would skip its app-launch.
//! - **Install:** the Store's page for the config's `StoreId`, `ms-windows-store://pdp/?ProductId=<id>`, where known.
//!   **Uninstall:** Settings' installed apps, `ms-settings:appsfeatures`, as Playnite opens: whether removing a
//!   package from outside removes a GDK game cleanly is not known.
//! - **Running:** a process of this session runs as the package's family (`GetPackageFamilyName`), which covers the
//!   hand-over from `GameLaunchHelper.exe` to the game.
//!
//! `Xbox::at(Some(dir))` reads a fake in place of the machine (`--xbox <dir>`, and `fixture`): `<dir>` stands for a
//! drive, with its `.GamingRoot` and the folders it names, and no package manager. An empty folder has no games.
pub mod family;
pub mod fixture;
pub mod gaming_root;
pub mod manifest;
#[cfg(windows)]
mod packaged;

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use model::{Ask, Found, Game, Library, OnlineArt, Source, Start};

use manifest::{asset, plain};

/// Xbox, as a game's `source` names it.
pub const SOURCE: &str = "xbox";

/// Where a drive's packaged games may be that `.GamingRoot` does not name.
const MODIFIABLE: [&str; 2] = ["Program Files", "ModifiableWindowsApps"];

/// The Xbox games on this machine, as the library reads them and the shell starts them.
#[derive(Debug, Clone, Default)]
pub struct Xbox {
    /// A fake drive to read in place of this machine's.
    fake: Option<PathBuf>,
}

/// A package the package manager says is registered for this user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    pub family_name: String,
    pub full_name: String,
    /// Where it is installed: its root, with its manifest.
    pub root: PathBuf,
}

impl Xbox {
    /// This machine's packages, or with `Some(dir)` the fake drive at `dir` (`--xbox`).
    pub fn at(fake: Option<PathBuf>) -> Self {
        Self { fake }
    }
}

/// The package roots on the drive (or the folder standing for one) at `drive`: under each folder its `.GamingRoot`
/// names, each game's `Content`; and each folder of `Program Files\ModifiableWindowsApps`. Whether there is a
/// `.GamingRoot`, as well.
pub fn package_roots(drive: &Path) -> (Vec<PathBuf>, bool) {
    let mut roots = Vec::new();
    let gaming_root = drive.join(gaming_root::FILE);
    let folders = match std::fs::read(&gaming_root) {
        Ok(bytes) => gaming_root::parse(&bytes).unwrap_or_else(|e| {
            eprintln!("xbox: {}: {e:#}", gaming_root.display());
            Vec::new()
        }),
        Err(_) => Vec::new(),
    };
    let found = gaming_root.is_file();
    let subfolders = |folder: PathBuf| -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
        let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        dirs
    };
    for folder in folders {
        let folder = folder.split(['\\', '/']).filter(|p| !p.is_empty()).fold(drive.to_path_buf(), |p, s| p.join(s));
        roots.extend(subfolders(folder).into_iter().map(|game| game.join("Content")).filter(|c| c.is_dir()));
    }
    roots.extend(subfolders(MODIFIABLE.iter().fold(drive.to_path_buf(), |p, s| p.join(s))));
    (roots, found)
}

/// The game whose package is at `root`, with the package manager's word on it where there is one; `Ok(None)` when
/// the package is no game (no `MicrosoftGame.config`, or a framework).
pub fn game_at(root: &Path, registered: Option<&Registered>) -> Result<Option<Game>> {
    let Some(config) = manifest::read_config(root)? else { return Ok(None) };
    let manifest = manifest::read_manifest(root)?;
    if manifest.framework {
        return Ok(None);
    }
    let Some(app) = manifest.game_application(&config) else { bail!("{}: no Application", root.display()) };
    let family_name = match registered {
        Some(registered) => registered.family_name.clone(),
        None => family::family_name(&manifest.name, &manifest.publisher),
    };
    let names = [&config.display_name, &manifest.display_name, &app.display_name];
    let name = names
        .iter()
        .find_map(|name| plain(name).map(str::to_string))
        .or_else(|| names.iter().find_map(|name| looked_up(registered, &manifest.name, name.as_deref()?)))
        .unwrap_or_else(|| manifest.name.clone());
    let first = |logos: &[&Option<String>]| logos.iter().find_map(|logo| asset(root, logo.as_deref()?));
    let product = config.store_id.clone().filter(|id| id.chars().all(|c| c.is_ascii_alphanumeric()));
    // The large tile's logo is the game's square art; the smaller ones are icons, left to the Store's box art
    // online where the game has a product there, and the art only of a game that has none.
    let large = first(&[&config.square480, &app.square310]);
    let small = || {
        let icons =
            [&config.square150, &app.square150, &config.store_logo, &manifest.logo, &config.square44, &app.square44];
        first(&icons)
    };
    let square = large.or_else(|| if product.is_none() { small() } else { None });
    let publishers = plain(&config.publisher_display_name)
        .or(plain(&manifest.publisher_display_name))
        .map(|p| vec![p.trim().to_string()])
        .unwrap_or_default();
    Ok(Some(Game {
        source: SOURCE.into(),
        source_id: family_name,
        name: name.trim().to_string(),
        installed: true,
        install_dir: Some(root.to_path_buf()),
        // No portrait picture in a package: the Store's poster online, else SteamGridDB's, else the square cropped.
        cover: None,
        hero: first(&[&config.splash, &app.splash]),
        header: first(&[&app.wide310]),
        square,
        online: OnlineArt { product, ..OnlineArt::default() },
        publishers,
        ..Game::default()
    }))
}

/// An `ms-resource:` name looked up in a registered package's resources: only Windows can.
#[cfg_attr(not(windows), allow(unused_variables))]
fn looked_up(registered: Option<&Registered>, name: &str, reference: &str) -> Option<String> {
    #[cfg(windows)]
    return packaged::resource(&registered?.full_name, name, reference);
    #[cfg(not(windows))]
    None
}

/// The games of these package roots, each with what the package manager says of it, one per family, by name.
fn games(roots: impl IntoIterator<Item = (PathBuf, Option<Registered>)>) -> Vec<Game> {
    let mut games: Vec<Game> = Vec::new();
    for (root, registered) in roots {
        match game_at(&root, registered.as_ref()) {
            Ok(Some(game)) if !games.iter().any(|g| g.source_id == game.source_id) => games.push(game),
            Ok(_) => {}
            Err(e) => eprintln!("xbox: passed over: {e:#}"),
        }
    }
    games.sort_by_key(|game| game.name.to_lowercase());
    games
}

/// This machine's drives that have a letter and are there.
#[cfg(windows)]
fn drives() -> Vec<PathBuf> {
    (b'A'..=b'Z').map(|letter| PathBuf::from(format!("{}:\\", letter as char))).filter(|d| d.is_dir()).collect()
}

/// This machine's games: the drives' package folders and the package manager's list.
#[cfg(windows)]
fn read_machine() -> Result<Option<Found>> {
    let mut folders = Vec::new();
    let mut gaming_roots = Vec::new();
    for drive in drives() {
        let (roots, found) = package_roots(&drive);
        if found {
            gaming_roots.push(drive.join(gaming_root::FILE).display().to_string());
        }
        folders.extend(roots);
    }
    let registered = packaged::packages();
    if let Err(e) = &registered {
        eprintln!("xbox: the package manager gave no list ({e:#}): the games in the folders are read as they are");
    }
    let mut roots: Vec<(PathBuf, Option<Registered>)> = Vec::new();
    for folder in folders {
        match &registered {
            // Another user's game, or a folder left behind: it can't be started.
            Ok(list) => {
                if let Some(package) = list.iter().find(|p| source_kit::same_folder(&p.root, &folder)) {
                    roots.push((folder, Some(package.clone())));
                }
            }
            Err(_) => roots.push((folder, None)),
        }
    }
    for package in registered.iter().flatten() {
        // Only a game has a config: an app's package is never opened further.
        if package.root.join(manifest::CONFIG).is_file()
            && !roots.iter().any(|(r, _)| source_kit::same_folder(r, &package.root))
        {
            roots.push((package.root.clone(), Some(package.clone())));
        }
    }
    let games = games(roots);
    if games.is_empty() && gaming_roots.is_empty() {
        return Ok(None);
    }
    let mut from = vec!["the user's packages".to_string()];
    from.extend(gaming_roots);
    Ok(Some(Found { library: Library { games, ..Library::default() }, from: from.join(" and ") }))
}

#[cfg(not(windows))]
fn read_machine() -> Result<Option<Found>> {
    Ok(None)
}

impl Source for Xbox {
    fn key(&self) -> &'static str {
        SOURCE
    }

    fn name(&self) -> &'static str {
        "Xbox"
    }

    fn read(&self) -> Result<Option<Found>> {
        let Some(dir) = &self.fake else { return read_machine() };
        if !dir.is_dir() {
            bail!("{}: no such folder", dir.display());
        }
        let (roots, _) = package_roots(dir);
        let library = Library { games: games(roots.into_iter().map(|root| (root, None))), ..Library::default() };
        Ok(Some(Found { library, from: dir.display().to_string() }))
    }

    /// `shell:AppsFolder\<family name>!<application id>`, the application the game's config names.
    fn start(&self, game: &Game) -> Option<Start> {
        if game.source != SOURCE || game.source_id.is_empty() {
            return None;
        }
        Some(Start::Address(format!(r"shell:AppsFolder\{}", app_user_model_id(game)?)))
    }

    /// Install: the Store's page for the game, when its config gives its product. Uninstall: Settings' installed
    /// apps, where the user removes it.
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
        if game.source != SOURCE {
            return None;
        }
        match ask {
            Ask::Install => {
                let config = manifest::read_config(game.install_dir.as_deref()?).ok()??;
                let id = config.store_id.filter(|id| id.chars().all(|c| c.is_ascii_alphanumeric()))?;
                Some(Start::Address(format!("ms-windows-store://pdp/?ProductId={id}")))
            }
            Ask::Uninstall => Some(Start::Address("ms-settings:appsfeatures".into())),
        }
    }

    /// Whether a process of this session runs as the game's package.
    fn running(&self, game: &Game) -> bool {
        #[cfg(windows)]
        if game.source == SOURCE && self.fake.is_none() {
            return packaged::running(&game.source_id);
        }
        let _ = game;
        false
    }
}

/// The game's app user model ID, `<family name>!<application id>`, by its package's manifest and config.
pub fn app_user_model_id(game: &Game) -> Option<String> {
    let root = game.install_dir.as_deref()?;
    let config = manifest::read_config(root).ok()??;
    let manifest = manifest::read_manifest(root).ok()?;
    let app = manifest.game_application(&config)?;
    Some(format!("{}!{}", game.source_id, app.id))
}

/// Where a fake's game's package is, and its `source_id`, for the stand-ins: the game whose app user model ID is
/// `aumid` in the fake drive at `dir`.
pub fn fake_game(dir: &Path, aumid: &str) -> Option<Game> {
    let found = Xbox::at(Some(dir.to_path_buf())).read().ok()??;
    found
        .library
        .games
        .into_iter()
        .find(|game| app_user_model_id(game).is_some_and(|id| id.eq_ignore_ascii_case(aumid)))
}

/// The app user model ID in a `shell:AppsFolder\<AUMID>` address.
pub fn aumid_of(address: &str) -> Option<&str> {
    const PREFIX: &str = "shell:AppsFolder";
    let address = address.trim();
    address.get(..PREFIX.len()).filter(|prefix| prefix.eq_ignore_ascii_case(PREFIX))?;
    let aumid = address[PREFIX.len()..].trim_start_matches(['\\', '/']);
    aumid.contains('!').then_some(aumid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixture::Fixture;

    #[test]
    fn the_fake_reads_back_as_its_games_and_never_its_apps() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("xbox");
        let spec = Fixture::standard();
        fixture::write(&root, &spec).unwrap();
        let xbox = Xbox::at(Some(root.clone()));
        assert_eq!((xbox.key(), xbox.name()), ("xbox", "Xbox"));
        let found = xbox.read().unwrap().expect("a fake drive");
        assert_eq!(found.from, root.display().to_string());
        let games = &found.library.games;
        let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
        // Not the app with no MicrosoftGame.config, nor the game whose manifest is broken.
        assert_eq!(names, spec.expected());

        let tidewater = games.iter().find(|g| g.name == "Tidewater Trials").unwrap();
        let content = root.join("XboxGames").join("Tidewater Trials").join("Content");
        assert_eq!(tidewater.install_dir.as_deref(), Some(content.as_path()));
        assert_eq!(tidewater.source_id, family::family_name("MadeUpStudio.TidewaterTrials", fixture::PUBLISHER));
        assert!(tidewater.installed);
        assert_eq!(tidewater.publishers, ["Made Up Studio"]);
        let assets = content.join("Assets");
        assert_eq!(tidewater.square.as_deref(), Some(assets.join("Logo480.png").as_path()));
        assert_eq!(tidewater.cover, None);
        assert_eq!(tidewater.online.product.as_deref(), Some("9ZZZMADEUP01"));
        assert_eq!(tidewater.hero.as_deref(), Some(assets.join("Splash.png").as_path()));
        assert_eq!(tidewater.header.as_deref(), Some(assets.join("Wide310.png").as_path()));

        // Its logos only at scales, its config's name the one to show over the manifest's ms-resource: name.
        let lumen = games.iter().find(|g| g.name == "Lumen Hollow").unwrap();
        let assets = root.join("XboxGames").join("Lumen Hollow").join("Content").join("Assets");
        // Its logos are icons, and it has a product in the Store: its square is the Store's box art, online.
        assert_eq!((lumen.square.as_deref(), lumen.cover.as_deref()), (None, None));
        assert_eq!(lumen.online.product.as_deref(), Some("9ZZZMADEUP02"));
        assert!(assets.join("Logo150.scale-200.png").is_file());
        // No splash screen in its config: the manifest's, and no wide tile.
        assert_eq!(lumen.hero.as_deref(), Some(assets.join("Splash620.scale-100.png").as_path()));
        assert_eq!(lumen.header, None);

        // The one under ModifiableWindowsApps, with no product in the Store: its own large logo.
        let pebble = games.iter().find(|g| g.name == "Pebble Courier").unwrap();
        assert!(pebble.square.as_deref().is_some_and(|logo| logo.ends_with("Logo480.png")));
        assert_eq!(pebble.online.product, None);
    }

    #[test]
    fn a_game_starts_by_its_app_user_model_id_and_asks_the_store_and_settings() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("xbox");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let xbox = Xbox::at(Some(root.clone()));
        let games = xbox.read().unwrap().unwrap().library.games;
        let tidewater = games.iter().find(|g| g.name == "Tidewater Trials").unwrap();
        let aumid = format!("{}!Game", tidewater.source_id);
        let address = format!(r"shell:AppsFolder\{aumid}");
        assert_eq!(xbox.start(tidewater), Some(Start::Address(address.clone())));
        assert_eq!(aumid_of(&address), Some(aumid.as_str()));
        assert_eq!(aumid_of("shell:AppsFolder"), None);
        assert_eq!(aumid_of("steam://rungameid/1"), None);
        assert_eq!(fake_game(&root, &aumid).map(|g| g.name), Some("Tidewater Trials".into()));
        assert_eq!(fake_game(&root, "Made.Up_0000000000000!Game"), None);
        assert_eq!(
            xbox.ask(tidewater, Ask::Install),
            Some(Start::Address("ms-windows-store://pdp/?ProductId=9ZZZMADEUP01".into()))
        );
        assert_eq!(xbox.ask(tidewater, Ask::Uninstall), Some(Start::Address("ms-settings:appsfeatures".into())));
        assert!(!xbox.running(tidewater));

        // Pebble Courier's config has no StoreId: nothing to install from.
        let pebble = games.iter().find(|g| g.name == "Pebble Courier").unwrap();
        assert_eq!(xbox.ask(pebble, Ask::Install), None);
        // Its package gone: nothing to start.
        let gone = Game { install_dir: Some(dir.path().join("gone")), ..tidewater.clone() };
        assert_eq!(xbox.start(&gone), None);
        // Not Xbox's: nothing.
        let steam = Game { source: "steam".into(), ..tidewater.clone() };
        assert_eq!((xbox.start(&steam), xbox.ask(&steam, Ask::Uninstall)), (None, None));
    }

    #[test]
    fn an_empty_folder_has_no_games_and_one_that_is_not_there_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let found = Xbox::at(Some(dir.path().to_path_buf())).read().unwrap().expect("a fake drive with nothing on it");
        assert!(found.library.games.is_empty());
        let missing = dir.path().join("no xbox here");
        let error = Xbox::at(Some(missing.clone())).read().unwrap_err();
        assert!(format!("{error:#}").starts_with(&missing.display().to_string()), "{error:#}");
        if cfg!(not(windows)) {
            assert_eq!(Xbox::at(None).read().unwrap(), None);
        }
    }

    #[test]
    fn the_package_managers_word_gives_the_family_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("xbox");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let content = root.join("XboxGames").join("Tidewater Trials").join("Content");
        let registered = Registered {
            family_name: "MadeUpStudio.TidewaterTrials_registered1".into(),
            full_name: "MadeUpStudio.TidewaterTrials_1.2.0.0_x64__registered1".into(),
            root: content.clone(),
        };
        let game = game_at(&content, Some(&registered)).unwrap().unwrap();
        assert_eq!(game.source_id, "MadeUpStudio.TidewaterTrials_registered1");
        // An app's package is no game.
        let notes = root.join("XboxGames").join("Made Up Notes").join("Content");
        assert_eq!(game_at(&notes, None).unwrap(), None);
    }
}
