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
//! - **Not installed:** the PC Game Pass catalogue of the region Settings names (#277), where the Dashboard is given
//!   one ([`Xbox::with_game_pass`]): each of its games not installed is listed not installed, marked `game_pass`,
//!   its `source_id` its package family name (else its product id), so that once installed it is the same game. An
//!   installed game the catalogue lists (by family name, else by its config's `StoreId`) is marked too, and listed
//!   once. Fetching the catalogue is the library's (`library::game_pass`), handed in as a function. What the user
//!   owns, and whether they subscribe, are Xbox's servers' to say: nothing is read of them.
//! - **Start:** the app user model ID, `<family name>!<application id>`, as an address: `shell:AppsFolder\<AUMID>`,
//!   which Windows activates as the Start menu does. Never the game's exe, which would skip its app-launch.
//! - **Install:** the Store's page for the config's `StoreId` (a catalogue game's product id),
//!   `ms-windows-store://pdp/?ProductId=<id>`, where known.
//!   **Uninstall:** Settings' installed apps, `ms-settings:appsfeatures`, as Playnite opens: whether removing a
//!   package from outside removes a GDK game cleanly is not known.
//! - **Running:** a process of this session runs as the package's family (`GetPackageFamilyName`), which covers the
//!   hand-over from `GameLaunchHelper.exe` to the game.
//!
//! `Xbox::at(Some(dir))` reads a fake in place of the machine (`--xbox <dir>`, and `fixture`): `<dir>` stands for a
//! drive, with its `.GamingRoot` and the folders it names, and no package manager. An empty folder has no games.
// Every log line reaches stderr in one write, so two Dashboards appending to one log never tear each other's lines
// (crates/log-line). Defined before the modules, so they all take it rather than std's.
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        log_line::eprintln!($($arg)*)
    };
}

pub mod family;
pub mod fixture;
pub mod gaming_root;
pub mod installing;
pub mod manifest;
#[cfg(windows)]
mod packaged;
pub mod region;
pub mod store_queue;
pub mod subscription;

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use std::sync::Arc;

use model::game_pass::Catalogue;
use model::{
    Ask, Can, Folder, Found, Game, Install, InstallState, Installer, Library, OnlineArt, Opener, Refusal, Source,
    Start, Steer,
};
use store_queue::StoreQueue;

use manifest::{asset, plain};

/// Xbox, as a game's `source` names it.
pub const SOURCE: &str = "xbox";

/// Where a drive's packaged games may be that `.GamingRoot` does not name.
const MODIFIABLE: [&str; 2] = ["Program Files", "ModifiableWindowsApps"];

/// `DASHBOARD_XBOX_QUEUE=off`: no install through the Store's queue, only its page.
pub const QUEUE_SWITCH: &str = "DASHBOARD_XBOX_QUEUE";

/// What gives the Game Pass catalogue to list: None while it is switched off or there is none.
pub type GamePass = Arc<dyn Fn() -> Option<Catalogue> + Send + Sync>;

/// The Xbox games on this machine, as the library reads them and the shell starts them.
#[derive(Clone, Default)]
pub struct Xbox {
    /// A fake drive to read in place of this machine's.
    fake: Option<PathBuf>,
    game_pass: Option<GamePass>,
    /// The Store's install queue (#173): Windows' own, the stand-in on a fake drive, none where neither is.
    queue: Option<Arc<dyn StoreQueue>>,
    jobs: installing::Jobs,
}

impl std::fmt::Debug for Xbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Xbox")
            .field("fake", &self.fake)
            .field("game_pass", &self.game_pass.is_some())
            .field("queue", &self.queue.is_some())
            .finish()
    }
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
    ///
    /// Installs go through the Store's queue: on a fake drive, its stand-in ([`store_queue::Fake`]); on Windows,
    /// Windows' own unless `DASHBOARD_XBOX_QUEUE=off`, which leaves the Store's page as the only way.
    pub fn at(fake: Option<PathBuf>) -> Self {
        let off = std::env::var(QUEUE_SWITCH).is_ok_and(|v| v.eq_ignore_ascii_case("off"));
        let queue: Option<Arc<dyn StoreQueue>> = match &fake {
            _ if off => None,
            Some(dir) => Some(Arc::new(store_queue::Fake::on(dir))),
            None if cfg!(windows) => Some(Arc::new(store_queue::Windows::default())),
            None => None,
        };
        Self { fake, game_pass: None, queue, jobs: installing::Jobs::default() }
    }

    /// With the Store's queue given (a test's own stand-in), or none.
    pub fn with_queue(self, queue: Option<Arc<dyn StoreQueue>>) -> Self {
        Self { queue, ..self }
    }

    /// With the PC Game Pass catalogue `game_pass` gives at each read (#277).
    pub fn with_game_pass(self, game_pass: GamePass) -> Self {
        Self { game_pass: Some(game_pass), ..self }
    }

    /// The installed games and the catalogue's: see [`with_catalogue`].
    fn listed(&self, found: Option<Found>) -> Option<Found> {
        let Some(catalogue) = self.game_pass.as_ref().and_then(|game_pass| game_pass()) else { return found };
        let mut found = found.unwrap_or_else(|| Found { library: Library::default(), from: String::new() });
        let (marked, added) = with_catalogue(&mut found.library.games, &catalogue);
        let from = format!(
            "the PC Game Pass catalogue for {} ({} games: {marked} installed, {added} not)",
            catalogue.market,
            catalogue.products.len()
        );
        found.from = if found.from.is_empty() { from } else { format!("{} and {from}", found.from) };
        Some(found)
    }
}

/// Merges the catalogue into the installed games: an installed game the catalogue lists (its family name the
/// product's, else its config's `StoreId` the product's or one of its bundle's) is marked `game_pass`; every other
/// product is a game not installed, its `source_id` its family name, else its product id. How many were marked and
/// added.
pub fn with_catalogue(games: &mut Vec<Game>, catalogue: &Catalogue) -> (usize, usize) {
    let mut marked = 0;
    let mut added = Vec::new();
    for product in &catalogue.products {
        let by_family =
            |game: &Game| product.family.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(&game.source_id));
        let by_id = |game: &Game| {
            let id = game.online.product.as_deref().is_some_and(|id| {
                id.eq_ignore_ascii_case(&product.id) || product.bundled.iter().any(|b| b.eq_ignore_ascii_case(id))
            });
            id || game.source_id.eq_ignore_ascii_case(&product.id)
        };
        let same = |game: &Game| by_family(game) || by_id(game);
        // The family name first, whichever game comes first: a Store id is only where no family name answers.
        let at = games.iter().position(by_family).or_else(|| games.iter().position(by_id));
        if let Some(game) = at.map(|at| &mut games[at]) {
            if !game.game_pass {
                game.game_pass = true;
                marked += 1;
            }
            continue;
        }
        if added.iter().any(|game: &Game| same(game)) {
            continue;
        }
        added.push(Game {
            source: SOURCE.into(),
            source_id: product.family.clone().unwrap_or_else(|| product.id.clone()),
            name: product.name.clone(),
            installed: false,
            online: product.art.clone(),
            developers: product.developers.clone(),
            publishers: product.publishers.clone(),
            released: product.released,
            game_pass: true,
            ..Game::default()
        });
    }
    let count = added.len();
    games.extend(added);
    games.sort_by_key(|game| game.name.to_lowercase());
    (marked, count)
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

#[cfg(not(windows))]
fn drives() -> Vec<PathBuf> {
    Vec::new()
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
        let Some(dir) = &self.fake else { return Ok(self.listed(read_machine()?)) };
        if !dir.is_dir() {
            bail!("{}: no such folder", dir.display());
        }
        let (roots, _) = package_roots(dir);
        let library = Library { games: games(roots.into_iter().map(|root| (root, None))), ..Library::default() };
        Ok(self.listed(Some(Found { library, from: dir.display().to_string() })))
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
                let id = store_id(game)?;
                Some(Start::Address(format!("ms-windows-store://pdp/?ProductId={id}")))
            }
            Ask::Uninstall => Some(Start::Address("ms-settings:appsfeatures".into())),
        }
    }

    /// The installs the Dashboard asked the Store's queue for, while there is a queue.
    fn installs(&self) -> Option<Vec<Install>> {
        self.queue.as_ref()?;
        Some(self.jobs.installs())
    }

    /// The games installed now, while an install the Dashboard asked for is under way or lately ended: so a game the
    /// Store has just registered, or an uninstall has just removed, is read at once. None at other times: reading
    /// every package is not cheap, and a game installed in the Store's own app is found at the next read.
    fn installed_now(&self) -> Option<Vec<String>> {
        if self.queue.is_none() || !self.jobs.lately() {
            return None;
        }
        let found = match &self.fake {
            Some(dir) => {
                let (roots, _) = package_roots(dir);
                games(roots.into_iter().map(|root| (root, None)))
            }
            None => read_machine().ok()??.library.games,
        };
        let mut ids: Vec<String> = found.into_iter().filter(|g| g.installed).map(|g| g.source_id).collect();
        ids.sort();
        ids.dedup();
        Some(ids)
    }

    /// Through the queue, removing the package is all an uninstall takes.
    fn uninstalls_by_itself(&self) -> bool {
        self.queue.is_some()
    }

    fn installer(&self) -> Option<&dyn Installer> {
        self.queue.as_ref().map(|_| self as &dyn Installer)
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

/// Xbox's installer (#173): the Store's own queue, `AppInstallManager` for this user, asked on a thread of its own
/// (the Dashboard's window never waits on the Store), falling back to the Store's page, said in the log, where the
/// queue does not take it: an unpackaged caller may be refused, and a game the account has no licence for is
/// (`0x803F8001`). Uninstall removes the package for this user (`PackageManager.RemovePackageAsync`), falling back
/// to Settings' installed apps.
impl Installer for Xbox {
    fn can(&self) -> Can {
        if self.queue.is_none() {
            return Can::NOTHING;
        }
        Can { pause: true, cancel: true, choose_folder: self.folders().len() > 1, quiet_uninstall: true }
    }

    /// The drives whose `.GamingRoot` names an Xbox games folder, the system drive's the default (or the first): the
    /// Store installs onto a drive (`AppInstallOptions.TargetVolume`), into the folder the Xbox app set there.
    fn folders(&self) -> Vec<Folder> {
        if self.queue.is_none() {
            return Vec::new();
        }
        let roots: Vec<PathBuf> = match &self.fake {
            Some(dir) => vec![dir.clone()],
            None => drives(),
        };
        let mut folders: Vec<Folder> = roots
            .into_iter()
            .filter_map(|drive| {
                let named =
                    std::fs::read(drive.join(gaming_root::FILE)).ok().and_then(|b| gaming_root::parse(&b).ok())?;
                let folder = drive.join(named.first()?);
                Some(Folder { path: folder, free: None, default: false })
            })
            .collect();
        let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_ascii_uppercase();
        let at = folders.iter().position(|f| f.path.to_string_lossy().to_ascii_uppercase().starts_with(&system));
        if let Some(first) = folders.get_mut(at.unwrap_or(0)) {
            first.default = true;
        }
        folders
    }

    fn install(&self, game: &Game, folder: Option<&Folder>, open: &mut dyn Opener) -> Result<(), Refusal> {
        let queue = self.queue.clone().ok_or(Refusal::Unsupported)?;
        let Some(product) = store_id(game) else {
            log(&format!("{}: no Store product to ask the queue for: the Store's page instead", game.name));
            return Err(Refusal::Unsupported);
        };
        let Some(mut detached) = open.detached() else { return Err(Refusal::Unsupported) };
        if !self.jobs.begin(&product, &game.source_id, &game.name) {
            log(&format!("{product}: {} is under way already", game.name));
            return Ok(());
        }
        let folder = folder.map(|f| f.path.clone());
        let fallback = self.ask(game, Ask::Install);
        let jobs = self.jobs.clone();
        let name = game.name.clone();
        let onto = folder.as_ref().map_or("the Store's default drive".to_string(), |f| f.display().to_string());
        log(&format!("{product}: asking the Store's queue to install {name} onto {onto}"));
        std::thread::spawn(move || match queue.start(&product, folder.as_deref()) {
            Ok(state) => {
                log(&format!("{product}: the Store's queue took {name} ({state})"));
                jobs.follow(queue);
            }
            Err(why) => {
                jobs.end(&product);
                fall_back(
                    &product,
                    &format!("the Store's queue did not take {name} ({why})"),
                    fallback,
                    &mut *detached,
                );
            }
        });
        Ok(())
    }

    fn uninstall(&self, game: &Game, open: &mut dyn Opener) -> Result<(), Refusal> {
        let queue = self.queue.clone().ok_or(Refusal::Unsupported)?;
        if !game.installed || store_queue::family_name(&game.source_id).is_none() {
            return Err(Refusal::Unsupported);
        }
        let Some(mut detached) = open.detached() else { return Err(Refusal::Unsupported) };
        let family = game.source_id.clone();
        let name = game.name.clone();
        let fallback = self.ask(game, Ask::Uninstall);
        let jobs = self.jobs.clone();
        log(&format!("{family}: removing {name}'s package for this user"));
        std::thread::spawn(move || {
            // Watched for a while, so the read that finds it gone comes at once.
            jobs.watch();
            match queue.remove(&family) {
                Ok(0) => {
                    fall_back(&family, &format!("no package of {name} was found to remove"), fallback, &mut *detached)
                }
                Ok(n) => log(&format!("{family}: {name} uninstalled ({n} package{})", if n == 1 { "" } else { "s" })),
                Err(why) => {
                    fall_back(&family, &format!("{name}'s package was not removed ({why})"), fallback, &mut *detached)
                }
            }
        });
        Ok(())
    }

    /// Only an install the Dashboard asked for: the Store's own are the Store's to steer.
    fn steer(&self, game: &Game, steer: Steer, _open: &mut dyn Opener) -> Result<(), Refusal> {
        let queue = self.queue.clone().ok_or(Refusal::Unsupported)?;
        let product = store_id(game).filter(|p| self.jobs.has(p)).ok_or(Refusal::Unsupported)?;
        if steer == Steer::Resume && !self.jobs.paused(&product) {
            return Ok(());
        }
        let jobs = self.jobs.clone();
        let name = game.name.clone();
        std::thread::spawn(move || match queue.steer(&product, steer) {
            Ok(()) => {
                log(&format!("{product}: {} {name}: the Store's queue took it", steer.verb()));
                match steer {
                    Steer::Pause => {
                        let (done, total) = jobs.installs_bytes(&product);
                        jobs.set(&product, InstallState::Paused { done, total });
                    }
                    Steer::Resume => {
                        let (done, total) = jobs.installs_bytes(&product);
                        jobs.set(&product, InstallState::Downloading { done, total });
                    }
                    Steer::Cancel => jobs.end(&product),
                }
            }
            Err(why) => log(&format!("{product}: {} {name}: the Store's queue refused ({why})", steer.verb())),
        });
        Ok(())
    }
}

/// Asks the store's own way where the queue did not take it, saying so.
fn fall_back(what: &str, why: &str, fallback: Option<Start>, open: &mut dyn Opener) {
    match fallback {
        Some(start) => {
            log(&format!("{what}: {why}: opening {start} instead"));
            if let Err(e) = open.open(&start) {
                log(&format!("{what}: that could not be opened either: {e}"));
            }
        }
        None => log(&format!("{what}: {why}, and there is no other way here")),
    }
}

fn log(line: &str) {
    eprintln!("xbox: {line}");
}

/// The game's product in the Store: its config's `StoreId` where it is installed, else the catalogue's product id.
pub fn store_id(game: &Game) -> Option<String> {
    let product_id = |id: &str| id.len() == 12 && id.chars().all(|c| c.is_ascii_alphanumeric());
    let config = game.install_dir.as_deref().and_then(|root| manifest::read_config(root).ok().flatten());
    match config {
        Some(config) => config.store_id.filter(|id| id.chars().all(|c| c.is_ascii_alphanumeric()) && !id.is_empty()),
        None => game.online.product.clone().filter(|id| product_id(id)),
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

    /// The fake catalogue of a market as `library::game_pass` would read it: its products, each with its family.
    fn catalogue(market: &str) -> Catalogue {
        let (_, games) = fixture::GAME_PASS.iter().find(|(m, _)| *m == market).unwrap();
        let products = games
            .iter()
            .map(|(id, name, identity)| model::game_pass::Product {
                id: id.to_string(),
                name: name.to_string(),
                family: Some(family::family_name(identity, fixture::PUBLISHER)),
                art: OnlineArt { product: Some(id.to_string()), ..OnlineArt::default() },
                ..Default::default()
            })
            .collect();
        Catalogue { market: market.into(), language: "en-GB".into(), fetched: 1, products, ..Catalogue::default() }
    }

    /// An opener that keeps what it was asked, and gives a copy of itself to a thread.
    #[derive(Clone, Default)]
    struct Kept(Arc<std::sync::Mutex<Vec<String>>>);

    impl Opener for Kept {
        fn open(&mut self, start: &Start) -> Result<(), String> {
            self.0.lock().unwrap().push(start.to_string());
            Ok(())
        }

        fn detached(&self) -> Option<Box<dyn Opener + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    fn until(what: &str, mut done: impl FnMut() -> bool) {
        let begun = std::time::Instant::now();
        while !done() {
            assert!(begun.elapsed() < std::time::Duration::from_secs(10), "never: {what}");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn a_catalogue_game_installs_through_the_queue_and_one_it_refuses_opens_the_stores_page() {
        let dir = tempfile::tempdir().unwrap();
        fixture::write(dir.path(), &Fixture::standard()).unwrap();
        let queue: Arc<dyn StoreQueue> = Arc::new(store_queue::Fake::taking(dir.path(), 1.5));
        let us = catalogue("US");
        let xbox = Xbox::at(Some(dir.path().into()))
            .with_game_pass(Arc::new(move || Some(us.clone())))
            .with_queue(Some(queue));
        let installer = xbox.installer().expect("an installer with a queue");
        let can = installer.can();
        assert!(can.pause && can.cancel && can.quiet_uninstall && !can.choose_folder);
        let folders = installer.folders();
        assert_eq!(folders, [Folder { path: dir.path().join("XboxGames"), free: None, default: true }]);
        let game =
            |name: &str| xbox.read().unwrap().unwrap().library.games.into_iter().find(|g| g.name == name).unwrap();
        let saltmarsh = game("Saltmarsh Run");
        assert!(!saltmarsh.installed && saltmarsh.game_pass);
        assert_eq!(xbox.installs(), Some(Vec::new()));
        assert_eq!(xbox.installed_now(), None, "nothing under way: not watched");

        let opened = Kept::default();
        installer.install(&saltmarsh, folders.first(), &mut opened.clone()).unwrap();
        until("downloading in the Queue", || {
            xbox.installs().unwrap().iter().any(|i| i.source_id == saltmarsh.source_id && i.state.downloading())
        });
        installer.steer(&saltmarsh, Steer::Pause, &mut opened.clone()).unwrap();
        until("paused", || xbox.installs().unwrap().iter().any(|i| i.state.paused()));
        installer.steer(&saltmarsh, Steer::Resume, &mut opened.clone()).unwrap();
        until("installed", || xbox.installs().unwrap().is_empty());
        let installed = game("Saltmarsh Run");
        assert!(installed.installed && installed.game_pass, "{installed:?}");
        assert_eq!(installed.source_id, saltmarsh.source_id, "the same row, installed");
        assert_eq!(store_id(&installed).as_deref(), Some("9ZZZGPASS101"));
        assert!(xbox.installed_now().unwrap().contains(&saltmarsh.source_id), "watched after an install");

        installer.uninstall(&installed, &mut opened.clone()).unwrap();
        until("uninstalled", || !game("Saltmarsh Run").installed);

        // Refused by the queue: the Store's page, through the detached opener.
        installer.install(&game("Ember Orchard"), None, &mut opened.clone()).unwrap();
        until("the page opened", || !opened.0.lock().unwrap().is_empty());
        assert_eq!(*opened.0.lock().unwrap(), ["ms-windows-store://pdp/?ProductId=9ZZZGPASS102"]);
        assert_eq!(xbox.installs(), Some(Vec::new()));
    }

    #[test]
    fn the_catalogues_games_not_installed_are_listed_and_the_installed_one_once() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("xbox");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let shown = std::sync::Arc::new(std::sync::Mutex::new(Some("GB")));
        let asked = shown.clone();
        let game_pass: GamePass = Arc::new(move || asked.lock().unwrap().map(catalogue));
        let xbox = Xbox::at(Some(root.clone())).with_game_pass(game_pass);
        let found = xbox.read().unwrap().unwrap();
        let games = &found.library.games;
        let names: Vec<(&str, bool, bool)> =
            games.iter().map(|g| (g.name.as_str(), g.installed, g.game_pass)).collect();
        assert_eq!(
            names,
            [
                ("Ember Orchard", false, true),
                ("Kestrel Line", false, true),
                ("Lumen Hollow", true, false),
                ("Pebble Courier", true, false),
                ("Saltmarsh Run", false, true),
                ("Tidewater Trials", true, true),
            ]
        );
        assert!(
            found.from.ends_with("the PC Game Pass catalogue for GB (4 games: 1 installed, 3 not)"),
            "{}",
            found.from
        );
        let saltmarsh = games.iter().find(|g| g.name == "Saltmarsh Run").unwrap();
        assert_eq!(saltmarsh.source_id, family::family_name("MadeUpStudio.SaltmarshRun", fixture::PUBLISHER));
        assert_eq!(
            xbox.ask(saltmarsh, Ask::Install),
            Some(Start::Address("ms-windows-store://pdp/?ProductId=9ZZZGPASS101".into()))
        );
        assert_eq!(xbox.start(saltmarsh), None);
        assert!(xbox.can_install(saltmarsh));

        // US's has no Kestrel Line; off, none of the catalogue's.
        *shown.lock().unwrap() = Some("US");
        assert_eq!(xbox.read().unwrap().unwrap().library.games.len(), 5);
        *shown.lock().unwrap() = None;
        let games = xbox.read().unwrap().unwrap().library.games;
        assert_eq!(games.len(), 3);
        assert!(games.iter().all(|g| g.installed && !g.game_pass));
    }

    #[test]
    fn an_installed_game_is_matched_by_its_store_id_where_the_family_differs() {
        let installed = Game {
            source: SOURCE.into(),
            source_id: "MadeUpStudio.Bundled_registered".into(),
            name: "Bundled".into(),
            installed: true,
            online: OnlineArt { product: Some("9ZZZGPASS204".into()), ..OnlineArt::default() },
            ..Game::default()
        };
        let mut games = vec![installed];
        let product = model::game_pass::Product {
            id: "9ZZZGPASS203".into(),
            name: "Bundled Deluxe".into(),
            bundled: vec!["9ZZZGPASS204".into()],
            ..Default::default()
        };
        let catalogue = Catalogue { products: vec![product.clone(), product], ..Catalogue::default() };
        assert_eq!(with_catalogue(&mut games, &catalogue), (1, 0));
        assert_eq!(games.len(), 1);
        assert!(games[0].game_pass);
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
