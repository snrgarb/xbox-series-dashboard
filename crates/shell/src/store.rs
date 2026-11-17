//! Asking a game's store to install or uninstall it (docs/spec/tile-menu.md, sections 10 and 13), and to pause,
//! resume or cancel an install under way (docs/spec/manage-queue.md, section 8). A store with an installer of its
//! own (`model::Source::installer`, `model::Installer`) is asked through it; one without, or one whose installer
//! says it cannot, through what its source gives (`model::Source::ask`: the store's own address for the game, or a
//! program to run), opened as Windows opens it, the store showing its own window and doing the rest. Either way
//! what is opened goes through [`Addresses`]: only [`SystemAddresses`] and [`ProgramAddresses`] open anything, which
//! the executable makes only when it runs in its window; a test has a [`Recorded`], which keeps what it was asked.
use std::path::{Path, PathBuf};

use model::{installer, Game, Opener, Refusal, Source, Start, Steer};

pub use model::Ask;
/// Whoever opens what a store is asked by: the seam between what the Dashboard asks of a store and Windows. Opens
/// an address with whatever program is registered for it, or runs a program; says why when nothing took it: on
/// Windows, ShellExecute's answer. It is the `model::Opener` every store's installer acts through.
pub use model::Opener as Addresses;

/// Says in the log what each address or program it opens asks of the game's store: `store: asked Epic Games to
/// install <name>: com.epicgames...`; for Steam's own program, `store: asking Steam to install <name>: app_install
/// <appid>`, the way it is asked said after (Steam's own program is only one of two ways of asking it, and the way
/// is taken on a thread of its own, `crate::steam`, which says which in a line of its own).
struct Saying<'a> {
    addresses: &'a mut dyn Addresses,
    store: &'a str,
    what: &'a str,
    game: &'a Game,
}

impl Opener for Saying<'_> {
    fn open(&mut self, start: &Start) -> Result<(), String> {
        let opened = self.addresses.open(start);
        let (store, what, name) = (self.store, self.what, &self.game.name);
        match (&opened, start) {
            (Ok(()), Start::Program { args, .. }) if crate::steam::is_steam(start).is_some() => {
                let command = source_steam::cef::console_command(args).unwrap_or_else(|| args.join(" "));
                eprintln!("store: asking {store} to {what} {name}: {command}");
            }
            (Ok(()), _) => eprintln!("store: asked {store} to {what} {name}: {start}"),
            (Err(_), _) => eprintln!("store: nothing took {start}: {name} is as it was"),
        }
        opened
    }
}

/// Asks the game's store, by way of its source (none for a game of no source the Dashboard has) and `addresses`:
/// through its installer where it has one that can, into its default folder (the folder question is Settings >
/// Storage's, #179), else as its source says. A game whose store can't be asked is said so and left alone. Whether
/// the store was asked.
pub fn ask(game: &Game, source: Option<&dyn Source>, ask: Ask, addresses: &mut dyn Addresses) -> bool {
    let what = ask.verb();
    let Some(source) = source else {
        eprintln!("store: {} is from no store that can {what} it", game.name);
        return false;
    };
    let mut saying = Saying { addresses, store: source.name(), what, game };
    if let Some(installer) = source.installer() {
        let done = match ask {
            Ask::Install => {
                let folders = installer.folders();
                installer.install(game, installer::default_folder(&folders), &mut saying)
            }
            Ask::Uninstall => installer.uninstall(game, &mut saying),
        };
        match done {
            Ok(()) => return true,
            // The store's own way, as before there were installers.
            Err(Refusal::Unsupported) => {}
            Err(Refusal::Failed(why)) => {
                eprintln!("store: {} couldn't {what} {}: {why}", source.name(), game.name);
                return false;
            }
        }
    }
    match source.ask(game, ask) {
        Some(address) => saying.open(&address).is_ok(),
        None => {
            eprintln!("store: {} is from no store that can {what} it", game.name);
            false
        }
    }
}

/// Asks the game's store to pause, resume or cancel its install under way, through its installer. A store with
/// none, or whose installer cannot, is said so: `store: Steam can't pause <name>'s install from here`. Whether the
/// store was asked.
pub fn steer(game: &Game, source: Option<&dyn Source>, steer: Steer, addresses: &mut dyn Addresses) -> bool {
    let what = steer.verb();
    let refused = match source.and_then(|source| Some((source, source.installer()?))) {
        Some((source, installer)) => {
            let mut saying = Saying { addresses, store: source.name(), what, game };
            match installer.steer(game, steer, &mut saying) {
                Ok(()) => return true,
                Err(refusal) => (source.name(), refusal),
            }
        }
        None => (source.map_or("No store", |source| source.name()), Refusal::Unsupported),
    };
    match refused {
        (store, Refusal::Unsupported) => eprintln!("store: {store} can't {what} {}'s install from here", game.name),
        (store, Refusal::Failed(why)) => eprintln!("store: {store} couldn't {what} {}'s install: {why}", game.name),
    }
    false
}

/// Keeps what it was asked to open, as the log says it, and opens nothing.
#[derive(Debug, Default)]
pub struct Recorded {
    pub opened: Vec<String>,
}

impl Addresses for Recorded {
    fn open(&mut self, start: &Start) -> Result<(), String> {
        self.opened.push(start.to_string());
        Ok(())
    }
}

/// Windows' own way of opening an address. Anywhere else it opens nothing.
#[derive(Debug, Default)]
pub struct SystemAddresses;

impl Addresses for SystemAddresses {
    #[cfg(windows)]
    fn open(&mut self, start: &Start) -> Result<(), String> {
        // Steam is asked without its windows coming forward (`crate::steam`).
        if crate::steam::is_steam(start).is_some() {
            return crate::steam::ask(start);
        }
        // A packaged app (the Xbox source's games, Store apps) is activated as Windows activates it, which works
        // with no shell window, as under shell mode before explorer is up (docs/research/shell.md, 5); the
        // address is opened as any other if that fails.
        if let Start::Address(address) = start {
            if let Some(aumid) = apps_folder(address) {
                match activate(aumid) {
                    Ok(pid) => {
                        eprintln!("store: activated {aumid}, pid {pid}");
                        return Ok(());
                    }
                    Err(why) => eprintln!("store: couldn't activate {aumid} ({why}); opening {address}"),
                }
            }
        }
        // Home has the foreground, and with it the right to let the store's window take it.
        match crate::shell_open(start) {
            result if result > 32 => Ok(()),
            result => Err(result.to_string()),
        }
    }

    #[cfg(not(windows))]
    fn open(&mut self, _start: &Start) -> Result<(), String> {
        Err("only Windows opens anything".into())
    }
}

/// The app user model ID in a `shell:AppsFolder\<AUMID>` address, as Windows reads it: any case.
pub fn apps_folder(address: &str) -> Option<&str> {
    const PREFIX: &str = r"shell:AppsFolder\";
    let head = address.get(..PREFIX.len())?;
    let aumid = &address[PREFIX.len()..];
    (head.eq_ignore_ascii_case(PREFIX) && aumid.contains('!')).then_some(aumid)
}

/// `IApplicationActivationManager::ActivateApplication`: the app's pid.
#[cfg(windows)]
fn activate(aumid: &str) -> Result<u32, String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{ApplicationActivationManager, IApplicationActivationManager, AO_NONE};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let manager: IApplicationActivationManager =
            CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER).map_err(|e| e.message())?;
        manager.ActivateApplication(&HSTRING::from(aumid), None, AO_NONE).map_err(|e| e.message())
    }
}

/// Every address given to one program to open, in place of the program Windows has registered for it: `<program>
/// open-address <address>`, run in the program's folder (`dashboard --open-addresses-with <program>`). A test
/// machine whose registry must not be touched has the stand-in stores take the addresses this way. A program to
/// run is run as Windows runs it.
#[derive(Debug, Clone)]
pub struct ProgramAddresses {
    program: PathBuf,
}

impl ProgramAddresses {
    pub fn new(program: &Path) -> Self {
        Self { program: program.to_path_buf() }
    }

    /// What is run in place of `start`: an address becomes the program given it; a program stays as it is.
    pub fn through(&self, start: &Start) -> Start {
        match start {
            Start::Address(address) => Start::Program {
                path: self.program.clone(),
                args: vec!["open-address".into(), address.clone()],
                dir: self.program.parent().filter(|dir| !dir.as_os_str().is_empty()).map(PathBuf::from),
            },
            program => program.clone(),
        }
    }
}

impl Addresses for ProgramAddresses {
    fn open(&mut self, start: &Start) -> Result<(), String> {
        SystemAddresses.open(&self.through(start))
    }
}

#[cfg(test)]
mod tests {
    use model::Found;

    use super::*;

    /// A store whose addresses are `fake://<verb>/<id>`, and which has none for a game with no id.
    struct Fake;

    impl Source for Fake {
        fn key(&self) -> &'static str {
            "fake"
        }

        fn name(&self) -> &'static str {
            "Fake Store"
        }

        fn read(&self) -> anyhow::Result<Option<Found>> {
            Ok(None)
        }

        fn start(&self, _: &Game) -> Option<Start> {
            None
        }

        fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
            (!game.source_id.is_empty()).then(|| Start::Address(format!("fake://{}/{}", ask.verb(), game.source_id)))
        }
    }

    fn game(source_id: &str) -> Game {
        Game { source: "fake".into(), source_id: source_id.into(), name: "Quiet Orbit".into(), ..Game::default() }
    }

    #[test]
    fn asking_opens_the_address_the_games_source_gives_and_nothing_where_it_has_none() {
        let mut addresses = Recorded::default();
        ask(&game("9100040"), Some(&Fake), Ask::Install, &mut addresses);
        ask(&game("9100010"), Some(&Fake), Ask::Uninstall, &mut addresses);
        ask(&game(""), Some(&Fake), Ask::Uninstall, &mut addresses);
        ask(&game("5"), None, Ask::Uninstall, &mut addresses);
        assert_eq!(addresses.opened, ["fake://install/9100040", "fake://uninstall/9100010"]);
    }

    /// A store with an installer that installs and steers by `inst://`, uninstalls by nothing of its own (so its
    /// source's address is opened), and cannot cancel.
    struct WithInstaller;

    impl model::Installer for WithInstaller {
        fn can(&self) -> model::Can {
            model::Can { pause: true, ..model::Can::NOTHING }
        }

        fn install(&self, game: &Game, folder: Option<&model::Folder>, open: &mut dyn Opener) -> Result<(), Refusal> {
            let folder = folder.map_or("none".into(), |folder| folder.path.display().to_string());
            let start = Start::Address(format!("inst://install/{}?into={folder}", game.source_id));
            open.open(&start).map_err(Refusal::Failed)
        }

        fn folders(&self) -> Vec<model::Folder> {
            let folder = |path: &str, default| model::Folder { path: path.into(), free: None, default };
            vec![folder("E", false), folder("D", true)]
        }

        fn steer(&self, game: &Game, steer: Steer, open: &mut dyn Opener) -> Result<(), Refusal> {
            if !steer.allowed(self.can()) {
                return Err(Refusal::Unsupported);
            }
            open.open(&Start::Address(format!("inst://{}/{}", steer.verb(), game.source_id))).map_err(Refusal::Failed)
        }
    }

    impl Source for WithInstaller {
        fn key(&self) -> &'static str {
            "fake"
        }

        fn name(&self) -> &'static str {
            "Installing Store"
        }

        fn read(&self) -> anyhow::Result<Option<Found>> {
            Ok(None)
        }

        fn start(&self, _: &Game) -> Option<Start> {
            None
        }

        fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
            Fake.ask(game, ask)
        }

        fn installer(&self) -> Option<&dyn model::Installer> {
            Some(self)
        }
    }

    #[test]
    fn a_store_with_an_installer_is_asked_through_it_into_its_default_folder_and_else_as_before() {
        let mut addresses = Recorded::default();
        ask(&game("7"), Some(&WithInstaller), Ask::Install, &mut addresses);
        // Its installer does not uninstall: the source's own address, as before installers.
        ask(&game("7"), Some(&WithInstaller), Ask::Uninstall, &mut addresses);
        assert_eq!(addresses.opened, ["inst://install/7?into=D", "fake://uninstall/7"]);
    }

    #[test]
    fn steering_goes_through_the_installer_and_nothing_is_opened_where_it_cannot() {
        let mut addresses = Recorded::default();
        assert!(steer(&game("7"), Some(&WithInstaller), Steer::Pause, &mut addresses));
        assert!(steer(&game("7"), Some(&WithInstaller), Steer::Resume, &mut addresses));
        assert!(!steer(&game("7"), Some(&WithInstaller), Steer::Cancel, &mut addresses));
        // A store with no installer, and a game of none.
        assert!(!steer(&game("7"), Some(&Fake), Steer::Pause, &mut addresses));
        assert!(!steer(&game("7"), None, Steer::Pause, &mut addresses));
        assert_eq!(addresses.opened, ["inst://pause/7", "inst://resume/7"]);
    }

    #[test]
    fn a_packaged_apps_address_gives_its_app_user_model_id() {
        assert_eq!(apps_folder(r"shell:AppsFolder\Calc_8wekyb3d8bbwe!App"), Some("Calc_8wekyb3d8bbwe!App"));
        assert_eq!(apps_folder(r"SHELL:appsfolder\Calc_8wekyb3d8bbwe!App"), Some("Calc_8wekyb3d8bbwe!App"));
        assert_eq!(apps_folder(r"shell:AppsFolder\Calc"), None);
        assert_eq!(apps_folder("steam://run/10"), None);
        assert_eq!(apps_folder("shell:"), None);
    }

    #[test]
    fn a_program_given_the_addresses_is_run_with_each_in_its_own_folder() {
        let folder = Path::new("e2e").join("Test User");
        let program = folder.join("standin.exe");
        let through = ProgramAddresses::new(&program);
        let launch = "com.epicgames.launcher://apps/ns%3Aitem%3AKite?action=launch&silent=true";
        let run = through.through(&Start::Address(launch.into()));
        assert_eq!(
            run,
            Start::Program {
                path: program.clone(),
                args: vec!["open-address".into(), launch.into()],
                dir: Some(folder.clone()),
            }
        );
        // As ShellExecute is given it: the address one argument, whatever it has in it.
        assert_eq!(run.target(), program.to_string_lossy());
        assert_eq!(run.folder(), Some(folder));
        assert_eq!(run.parameters().as_deref(), Some(format!("open-address {launch}").as_str()));
        let spaced = through.through(&Start::Address("steam://run/1 2".into()));
        assert_eq!(spaced.parameters().as_deref(), Some("open-address \"steam://run/1 2\""));
        // A program to run is not an address: run as it is.
        let game = Start::Program { path: "game.exe".into(), args: vec!["-x".into()], dir: None };
        assert_eq!(through.through(&game), game);
        // A program named with no folder runs in the working folder.
        let bare =
            ProgramAddresses::new(Path::new("standin.exe")).through(&Start::Address("steam://rungameid/1".into()));
        assert!(matches!(bare, Start::Program { dir: None, .. }), "{bare:?}");
    }
}
