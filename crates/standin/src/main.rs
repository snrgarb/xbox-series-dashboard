//! Stand-ins for what a test machine doesn't have: Steam, and a game.
//!
//!   standin make-steam <dir> [--json <file>] [--full-shelf]
//!       Writes a fake Steam install at <dir> (`source_steam::fixture`), with a copy of this program as `game.exe` in
//!       every installed game's folder, reads it back, and with --json writes the Library it read there. With
//!       --full-shelf it is `Fixture::full_shelf`, twenty installed games, where it is otherwise `Fixture::standard`.
//!   standin make-tones <dir>
//!       Writes a short tone into <dir> under each of the names the Dashboard's sounds have in the Bundle, for
//!       `dashboard --media <dir>`: the console's own sounds are in no repository and on no test machine.
//!   standin game [--title <t>] [--splash-ms <n>] [--seconds <n>] [--ignore-close]
//!       The stand-in game: a borderless, full-screen, dark-blue window titled <t> ("Stand-in game"). It closes when
//!       asked to, on Esc, and after <seconds> (600) in case a test leaves it behind. With --splash-ms, a small window
//!       titled "<t> launcher" shows for that long first, as a game's launcher or splash screen does. With
//!       --ignore-close it does not close when asked to, as a game that has hung does not, and says so in the log.
//!       Its pulse, a square towards the window's bottom right, changes colour four times a second while the game
//!       runs, and so stands still while the game is frozen; a beat that comes late says in the log how long the
//!       game stood still.
//!       This is also what the program does when it is named `game.exe` and given no arguments, its arguments then
//!       coming from `game-args.txt` beside it, one a line, if that exists.
//!   standin steam-url steam://rungameid/<appid>
//!       The stand-in Steam, registered as the `steam://` handler: starts the installed game's `game.exe`, and keeps
//!       `HKCU\Software\Valve\Steam\Apps\<appid>\Running` at 1 while it runs, as Steam does. Exits 1, having done
//!       nothing, when the game isn't installed.
//!   standin toast <text>
//!       Posts a notification to the running Dashboard, which shows its Toast (`model::post`). Exits 1 when no
//!       Dashboard took it: none is running, or it has no Toast strip yet.
//!   standin steam-url steam://install/<appid> | steam://uninstall/<appid>
//!       Says in the log which it was asked, the address with it, and does nothing more: a test reads the line.
//!   standin make-ea <dir> [--json <file>] | make-ubisoft <dir> [--json <file>] | make-battlenet <dir> [--json <file>]
//!       Writes that store's fake at <dir> (`source_ea::fixture`, `source_ubisoft::fixture`,
//!       `source_battlenet::fixture`: made-up games, each installed one with a copy of this program as its
//!       `game.exe`, titled with its name), reads it back as the Dashboard does with `--ea <dir>` (and the others),
//!       and with --json writes the Library it read there. Battle.net's client in the fake, `Battle.net.exe`, is a
//!       copy of this program too.
//!   standin uplay-url <dir> uplay://launch/<id> | uplay://install/<id> | uplay://uninstall/<id>
//!       The stand-in Ubisoft Connect, registered as the `uplay://` handler for the fake at <dir>: starts the game's
//!       `game.exe` from its install folder and waits for it; asked to install or uninstall, says so in the log and
//!       does nothing more. Exits 1, having done nothing, when the game isn't installed.
//!   Battle.net.exe --exec="launch <code>" | --game=<uid>
//!       The stand-in Battle.net, when this program is named `Battle.net.exe` in a fake Battle.net's folder: starts
//!       the game with that product code from its install folder and waits for it; asked for a game's page, says so
//!       in the log.
//!
//! It has no console on Windows, so nothing it starts shows one: what it has to say goes to `standin.log` in
//! `%E2E_OUT%`, when that is set, and `make-steam`'s failure to `standin-error.txt` beside <dir> as well.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod win;

use std::io::Write;
use std::path::{Path, PathBuf};

use source_steam::fixture::{self, Fixture};

const USAGE: &str = "usage: standin make-steam <dir> [--json <file>] [--full-shelf]
       standin make-tones <dir>
       standin game [--title <t>] [--splash-ms <n>] [--seconds <n>] [--ignore-close]
       standin steam-url steam://rungameid/<appid> | steam://install/<appid> | steam://uninstall/<appid>
       standin make-ea | make-ubisoft | make-battlenet <dir> [--json <file>]
       standin uplay-url <dir> uplay://launch/<id> | uplay://install/<id> | uplay://uninstall/<id>
       standin toast <text>";

/// The arguments a `game.exe` run with none takes instead, one a line.
const GAME_ARGS_FILE: &str = "game-args.txt";

#[derive(Debug, PartialEq)]
struct GameArgs {
    title: String,
    /// How long the launcher's small window shows first; 0 for none.
    splash_ms: u32,
    seconds: u32,
    /// Whether it stays up when its window is asked to close, as a game that has hung does.
    ignore_close: bool,
}

impl GameArgs {
    fn parse(args: &[String]) -> Result<GameArgs, String> {
        let mut out = GameArgs { title: "Stand-in game".into(), splash_ms: 0, seconds: 600, ignore_close: false };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let mut value = || it.next().ok_or(format!("{arg} needs a value"));
            let number = |v: &String| v.parse::<u32>().map_err(|e| format!("bad {arg} {v}: {e}"));
            match arg.as_str() {
                "--title" => out.title = value()?.clone(),
                "--splash-ms" => out.splash_ms = number(value()?)?,
                "--seconds" => out.seconds = number(value()?)?,
                "--ignore-close" => out.ignore_close = true,
                _ => return Err(format!("unknown argument {arg}")),
            }
        }
        Ok(out)
    }
}

/// The appid in `steam://rungameid/<appid>`.
fn appid_of(url: &str) -> Option<u32> {
    let rest = url.trim().strip_prefix("steam://rungameid/")?;
    rest.trim_end_matches('/').parse().ok().filter(|&id| id != 0)
}

/// What `steam://install/<appid>` or `steam://uninstall/<appid>` asks for, and of which game.
fn asked_of(url: &str) -> Option<(&'static str, u32)> {
    let rest = url.trim().strip_prefix("steam://")?;
    let (verb, appid) = rest.trim_end_matches('/').split_once('/')?;
    let verb = ["install", "uninstall"].into_iter().find(|&known| known == verb)?;
    appid.parse().ok().filter(|&id| id != 0).map(|id| (verb, id))
}

/// Where the Steam install at `root` has `appid` installed, by its app manifest.
#[cfg_attr(not(windows), allow(dead_code))]
fn install_dir(root: &Path, appid: u32) -> Option<PathBuf> {
    let steamapps = root.join("steamapps");
    let manifest = std::fs::read_to_string(steamapps.join(format!("appmanifest_{appid}.acf"))).ok()?;
    let manifest = source_steam::vdf::parse_text(&manifest).ok()?;
    Some(steamapps.join("common").join(manifest.str_at(&["AppState", "installdir"])?))
}

/// Writes `spec` at `dir` with this program as every installed game's `game.exe`, and reads it back.
fn make_steam(dir: &Path, json: Option<&Path>, mut spec: Fixture) -> Result<(), String> {
    let dir = std::path::absolute(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    spec.game_exe = Some(std::env::current_exe().map_err(|e| format!("no path to this program: {e}"))?);
    fixture::write(&dir, &spec).map_err(|e| format!("writing {}: {e}", dir.display()))?;
    for game in spec.games.iter().filter(|g| g.installed) {
        // Each game's window is titled with its name, so a test can tell which one is in front.
        let args = dir.join("steamapps").join("common").join(&game.install_dir).join(GAME_ARGS_FILE);
        std::fs::write(&args, format!("--title\n{}\n", game.name)).map_err(|e| format!("{}: {e}", args.display()))?;
    }

    // Read back with the reader the Dashboard uses, on the machine it will run on: a fixture it can't read should
    // fail here, not as an empty Home.
    let library = source_steam::load(&dir).map_err(|e| format!("reading {} back: {e:#}", dir.display()))?;
    if library.games.len() != spec.games.len() {
        return Err(format!("wrote {} games, read {} back", spec.games.len(), library.games.len()));
    }
    if let Some(json) = json {
        let text = serde_json::to_string_pretty(&library).map_err(|e| e.to_string())?;
        std::fs::write(json, text).map_err(|e| format!("{}: {e}", json.display()))?;
    }
    note(&format!("make-steam: {} games at {}", library.games.len(), dir.display()));
    Ok(())
}

/// Writes the fake `store` (ea, ubisoft or battlenet) at `dir`, with this program as every installed game's
/// `game.exe`, and reads it back as the Dashboard does.
fn make_store(store: &str, dir: &Path, json: Option<&Path>) -> Result<(), String> {
    let dir = std::path::absolute(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let exe = std::env::current_exe().map_err(|e| format!("no path to this program: {e}"))?;
    let written = |e: std::io::Error| format!("writing {}: {e}", dir.display());
    let names = |names: Vec<&str>| names.into_iter().map(str::to_string).collect::<Vec<_>>();
    let (source, expected): (Box<dyn model::Source>, Vec<String>) = match store {
        "ea" => {
            let spec = source_ea::fixture::Fixture { game_exe: Some(exe), ..source_ea::fixture::Fixture::standard() };
            source_ea::fixture::write(&dir, &spec).map_err(written)?;
            (Box::new(source_ea::Ea::at(Some(dir.clone()))), names(spec.expected()))
        }
        "ubisoft" => {
            let standard = source_ubisoft::fixture::Fixture::standard();
            let spec = source_ubisoft::fixture::Fixture { game_exe: Some(exe), ..standard };
            source_ubisoft::fixture::write(&dir, &spec).map_err(written)?;
            (Box::new(source_ubisoft::Ubisoft::at(Some(dir.clone()))), names(spec.expected()))
        }
        "battlenet" => {
            let standard = source_battlenet::fixture::Fixture::standard();
            let spec = source_battlenet::fixture::Fixture { game_exe: Some(exe), ..standard };
            source_battlenet::fixture::write(&dir, &spec).map_err(written)?;
            (Box::new(source_battlenet::BattleNet::at(Some(dir.clone()))), names(spec.expected()))
        }
        _ => return Err(format!("no fake store {store}")),
    };
    // Read back with the reader the Dashboard uses, on the machine it will run on.
    let found = source.read().map_err(|e| format!("reading {} back: {e:#}", dir.display()))?;
    let library = found.ok_or_else(|| format!("reading {} back: no {store} there", dir.display()))?.library;
    let read: Vec<String> = library.games.iter().map(|game| game.name.clone()).collect();
    if read != expected {
        return Err(format!("wrote {expected:?}, read {read:?} back"));
    }
    if let Some(json) = json {
        let text = serde_json::to_string_pretty(&library).map_err(|e| e.to_string())?;
        std::fs::write(json, text).map_err(|e| format!("{}: {e}", json.display()))?;
    }
    note(&format!("make-{store}: {} games at {}", library.games.len(), dir.display()));
    Ok(())
}

/// What a `uplay://` address asks: launch, install or uninstall, and of which game.
fn uplay_asked(url: &str) -> Option<(&'static str, u32)> {
    let rest = url.trim().strip_prefix("uplay://")?;
    let mut parts = rest.trim_end_matches('/').split('/');
    let verb = parts.next()?;
    let verb = ["launch", "install", "uninstall"].into_iter().find(|&known| known == verb)?;
    parts.next()?.parse().ok().map(|id| (verb, id))
}

/// What the client is asked on its command line: `--exec="launch <code>"` (which Windows hands over as one
/// argument, `--exec=launch <code>`, or as two when unquoted) or `--game=<uid>`.
fn battlenet_asked(args: &[String]) -> Option<(&'static str, String)> {
    let line = args.join(" ");
    if let Some(exec) = line.split("--exec=").nth(1) {
        let code = exec.trim().trim_matches('"').strip_prefix("launch")?.trim().trim_matches('"');
        return code.split_whitespace().next().map(|code| ("launch", code.to_string()));
    }
    let uid = args.iter().find_map(|arg| arg.strip_prefix("--game="))?;
    Some(("game", uid.to_string()))
}

/// Starts the game's `game.exe` from its install folder, as its store's client would, and waits for it. `who` is
/// what the log calls the store's stand-in, `id` the game's id in the store.
fn run_game(who: &str, game: Option<&model::Game>, id: &str) -> i32 {
    let Some(dir) = game.and_then(|game| game.install_dir.clone()) else {
        note(&format!("{who}: {id} isn't installed"));
        return 1;
    };
    let exe = dir.join("game.exe");
    let mut child = match std::process::Command::new(&exe).current_dir(&dir).spawn() {
        Ok(child) => child,
        Err(e) => {
            note(&format!("{who}: could not start {}: {e}", exe.display()));
            return 1;
        }
    };
    note(&format!("{who}: {id} running, {} pid {}", exe.display(), child.id()));
    let status = child.wait();
    note(&format!("{who}: {id} ended ({})", status.map_or_else(|e| e.to_string(), |s| s.to_string())));
    0
}

/// The installed game with this `source_id` in a store's fake.
fn game_of(source: &dyn model::Source, id: &str) -> Option<model::Game> {
    let library = source.read().ok()??.library;
    library.games.into_iter().find(|game| game.source_id == id && game.installed)
}

/// The stand-in Ubisoft Connect, for the fake at `dir`.
fn uplay_url(dir: &Path, url: &str) -> i32 {
    match uplay_asked(url) {
        Some(("launch", id)) => {
            let id = id.to_string();
            run_game("uplay-url", game_of(&source_ubisoft::Ubisoft::at(Some(dir.to_path_buf())), &id).as_ref(), &id)
        }
        Some((verb, id)) => {
            note(&format!("uplay-url: asked to {verb} {id}: {}", url.trim()));
            0
        }
        None => {
            note(&format!("uplay-url: not an address the stand-in Ubisoft Connect knows: {url:?}"));
            2
        }
    }
}

/// The stand-in Battle.net, as this program is named in a fake Battle.net's folder, which it reads.
fn battlenet(args: &[String]) -> i32 {
    let dir = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.to_path_buf()));
    match (battlenet_asked(args), dir) {
        (Some(("launch", code)), Some(dir)) => {
            note(&format!("battlenet: asked to launch {code}: {}", args.join(" ")));
            run_game("battlenet", game_of(&source_battlenet::BattleNet::at(Some(dir)), &code).as_ref(), &code)
        }
        (Some((verb, id)), _) => {
            note(&format!("battlenet: asked for {verb} {id}: {}", args.join(" ")));
            0
        }
        (None, _) => {
            note(&format!("battlenet: not a command the stand-in Battle.net knows: {args:?}"));
            2
        }
    }
}

/// A line in `%E2E_OUT%\standin.log`, and on stderr where there is one.
fn note(message: &str) {
    eprintln!("standin: {message}");
    let Some(out) = std::env::var_os("E2E_OUT") else { return };
    let file = std::fs::OpenOptions::new().create(true).append(true).open(Path::new(&out).join("standin.log"));
    if let Ok(mut file) = file {
        let _ = writeln!(file, "[pid {}] {message}", std::process::id());
    }
}

#[cfg(not(windows))]
fn game(_: &GameArgs) -> i32 {
    note("game: Windows only");
    2
}

#[cfg(not(windows))]
fn steam_url(_: u32) -> i32 {
    note("steam-url: Windows only");
    2
}

#[cfg(not(windows))]
fn toast(_: &str) -> i32 {
    note("toast: Windows only");
    2
}

#[cfg(windows)]
use win::{game, steam_url, toast};

fn run(args: &[String]) -> i32 {
    let parsed_game = |args: &[String]| match GameArgs::parse(args) {
        Ok(args) => game(&args),
        Err(e) => {
            note(&format!("game: {e}\n{USAGE}"));
            2
        }
    };
    match args.first().map(String::as_str) {
        Some("make-steam") => {
            let full_shelf = args.last().is_some_and(|last| last == "--full-shelf");
            let spec = if full_shelf { Fixture::full_shelf() } else { Fixture::standard() };
            let (dir, json) = match &args[1..args.len() - usize::from(full_shelf)] {
                [dir] => (Path::new(dir), None),
                [dir, flag, json] if flag == "--json" => (Path::new(dir), Some(Path::new(json))),
                _ => {
                    note(USAGE);
                    return 2;
                }
            };
            match make_steam(dir, json, spec) {
                Ok(()) => 0,
                Err(e) => {
                    note(&format!("make-steam: {e}"));
                    let beside =
                        std::path::absolute(dir).ok().and_then(|d| Some(d.parent()?.join("standin-error.txt")));
                    if let Some(file) = beside {
                        let _ = std::fs::write(file, format!("{e}\n"));
                    }
                    1
                }
            }
        }
        Some(command @ ("make-ea" | "make-ubisoft" | "make-battlenet")) => {
            let (dir, json) = match &args[1..] {
                [dir] => (Path::new(dir), None),
                [dir, flag, json] if flag == "--json" => (Path::new(dir), Some(Path::new(json))),
                _ => {
                    note(USAGE);
                    return 2;
                }
            };
            match make_store(&command["make-".len()..], dir, json) {
                Ok(()) => 0,
                Err(e) => {
                    note(&format!("{command}: {e}"));
                    let beside =
                        std::path::absolute(dir).ok().and_then(|d| Some(d.parent()?.join("standin-error.txt")));
                    if let Some(file) = beside {
                        let _ = std::fs::write(file, format!("{e}\n"));
                    }
                    1
                }
            }
        }
        Some("uplay-url") => match &args[1..] {
            [dir, url] => uplay_url(Path::new(dir), url),
            _ => {
                note(USAGE);
                2
            }
        },
        Some("make-tones") => {
            let [dir] = &args[1..] else {
                note(USAGE);
                return 2;
            };
            let names = model::Sound::all_files();
            match audio::tones::write(Path::new(dir), &names) {
                Ok(()) => {
                    note(&format!("make-tones: {} tones in {dir}", names.len()));
                    0
                }
                Err(e) => {
                    note(&format!("make-tones: {dir}: {e}"));
                    1
                }
            }
        }
        Some("game") => parsed_game(&args[1..]),
        Some("steam-url") => match args.get(1).map(|url| (appid_of(url), asked_of(url), url)) {
            Some((Some(appid), _, _)) => steam_url(appid),
            // The stand-in Steam installs and uninstalls nothing: that it was asked is what a test looks for.
            Some((None, Some((verb, appid)), url)) => {
                note(&format!("steam-url: asked to {verb} {appid}: {}", url.trim()));
                0
            }
            _ => {
                note(&format!("steam-url: not an address the stand-in Steam knows: {:?}", args.get(1)));
                2
            }
        },
        Some("toast") => match &args[1..] {
            [text] => toast(text),
            _ => {
                note(USAGE);
                2
            }
        },
        // As a fake Battle.net's client.
        _ if named("Battle.net") => battlenet(args),
        // As a game's own program, in its install folder.
        None if named("game") => {
            let beside = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join(GAME_ARGS_FILE)));
            let lines = beside.and_then(|file| std::fs::read_to_string(file).ok()).unwrap_or_default();
            let args: Vec<String> = lines.lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
            parsed_game(&args)
        }
        _ => {
            note(USAGE);
            2
        }
    }
}

/// Whether this program is running as `<name>.exe`: `game.exe`, a game's; `Battle.net.exe`, a fake Battle.net's.
fn named(name: &str) -> bool {
    std::env::current_exe().is_ok_and(|exe| exe.file_stem().is_some_and(|s| s.eq_ignore_ascii_case(name)))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn game_arguments() {
        assert_eq!(
            GameArgs::parse(&[]).unwrap(),
            GameArgs { title: "Stand-in game".into(), splash_ms: 0, seconds: 600, ignore_close: false }
        );
        assert_eq!(
            GameArgs::parse(&strings(&["--title", "Brass Meridian", "--splash-ms", "2500", "--seconds", "30"]))
                .unwrap(),
            GameArgs { title: "Brass Meridian".into(), splash_ms: 2500, seconds: 30, ignore_close: false }
        );
        assert!(GameArgs::parse(&strings(&["--ignore-close"])).unwrap().ignore_close);
        assert!(GameArgs::parse(&strings(&["--title"])).is_err());
        assert!(GameArgs::parse(&strings(&["--seconds", "soon"])).is_err());
        assert!(GameArgs::parse(&strings(&["--fullscreen"])).is_err());
    }

    #[test]
    fn make_tones_writes_a_sound_for_every_file_the_dashboard_plays() {
        let dir = tempfile::tempdir().unwrap();
        let tones = dir.path().join("tones");
        assert_eq!(run(&strings(&["make-tones", tones.to_str().unwrap()])), 0);
        for name in model::Sound::all_files() {
            let clip = audio::wav::decode(&std::fs::read(tones.join(name)).unwrap()).unwrap();
            assert!(clip.seconds() > 0.1 && clip.seconds() < 0.5, "{name} is {} s", clip.seconds());
        }
        assert_eq!(run(&strings(&["make-tones"])), 2);
    }

    #[test]
    fn steam_addresses() {
        assert_eq!(appid_of("steam://rungameid/9100030"), Some(9_100_030));
        assert_eq!(appid_of("steam://rungameid/9100030/"), Some(9_100_030));
        assert_eq!(appid_of("steam://rungameid/"), None);
        assert_eq!(appid_of("steam://rungameid/0"), None);
        assert_eq!(appid_of("steam://store/9100030"), None);
        assert_eq!(appid_of("https://example.com/rungameid/5"), None);
        assert_eq!(asked_of("steam://install/9100040"), Some(("install", 9_100_040)));
        assert_eq!(asked_of("steam://uninstall/9100010/"), Some(("uninstall", 9_100_010)));
        assert_eq!(asked_of("steam://rungameid/9100030"), None);
        assert_eq!(asked_of("steam://install/0"), None);
        assert_eq!(asked_of("steam://install/"), None);
        assert_eq!(run(&strings(&["steam-url", "steam://uninstall/9100010"])), 0);
        assert_eq!(run(&strings(&["steam-url", "steam://store/9100010"])), 2);
    }

    #[test]
    fn make_steam_writes_an_install_the_stand_in_steam_can_start_games_from() {
        let dir = tempfile::tempdir().unwrap();
        let (root, json) = (dir.path().join("steam"), dir.path().join("library.json"));
        // The Recent row's first four, without their art: enough to start a game from, and quick to write.
        let mut spec = Fixture::standard();
        spec.games.truncate(4);
        spec.games.iter_mut().for_each(|g| g.art = fixture::Art::None);
        make_steam(&root, Some(&json), spec).unwrap();

        // Brass Meridian, the Recent row's third: installed, with this program as its game.exe and its title to show.
        let game = install_dir(&root, 9_100_030).unwrap();
        assert_eq!(game, root.join("steamapps").join("common").join("Brass Meridian"));
        assert_eq!(
            std::fs::read(game.join("game.exe")).unwrap(),
            std::fs::read(std::env::current_exe().unwrap()).unwrap()
        );
        let args: Vec<String> =
            std::fs::read_to_string(game.join(GAME_ARGS_FILE)).unwrap().lines().map(str::to_string).collect();
        assert_eq!(GameArgs::parse(&args).unwrap().title, "Brass Meridian");
        // Quiet Orbit, the fourth, isn't installed.
        assert_eq!(install_dir(&root, 9_100_040), None);

        let library: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
        assert_eq!(library["user"], "Tester");
        assert_eq!(library["games"].as_array().unwrap().len(), 4);
        assert_eq!(library["games"][2]["name"], "Brass Meridian");
        assert_eq!(library["games"][2]["installed"], true);
    }

    #[test]
    fn make_store_writes_each_fake_and_reads_it_back() {
        let dir = tempfile::tempdir().unwrap();
        for store in ["ea", "ubisoft", "battlenet"] {
            let (root, json) = (dir.path().join(store), dir.path().join(format!("{store}.json")));
            let args = strings(&[&format!("make-{store}"), root.to_str().unwrap(), "--json", json.to_str().unwrap()]);
            assert_eq!(run(&args), 0, "make-{store}");
            let library: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
            let games = library["games"].as_array().unwrap();
            assert!(games.len() >= 2, "{store}: {games:?}");
            // Every installed game has this program as its game.exe.
            for game in games.iter().filter(|g| g["installed"] == true) {
                let exe = Path::new(game["install_dir"].as_str().unwrap()).join("game.exe");
                assert!(exe.is_file(), "{}", exe.display());
            }
        }
        assert!(dir.path().join("battlenet").join("Battle.net.exe").is_file());
        assert_eq!(run(&strings(&["make-ea"])), 2);
    }

    #[test]
    fn uplay_and_battlenet_commands() {
        assert_eq!(uplay_asked("uplay://launch/90310"), Some(("launch", 90_310)));
        assert_eq!(uplay_asked("uplay://launch/90310/0"), Some(("launch", 90_310)));
        assert_eq!(uplay_asked("uplay://install/90410/"), Some(("install", 90_410)));
        assert_eq!(uplay_asked("uplay://uninstall/x"), None);
        assert_eq!(uplay_asked("steam://launch/5"), None);
        assert_eq!(battlenet_asked(&strings(&["--exec=launch emb"])), Some(("launch", "emb".into())));
        assert_eq!(battlenet_asked(&strings(&["--exec=launch", "emb"])), Some(("launch", "emb".into())));
        assert_eq!(battlenet_asked(&strings(&["--exec=\"launch jnv\""])), Some(("launch", "jnv".into())));
        assert_eq!(battlenet_asked(&strings(&["--game=emberwatch"])), Some(("game", "emberwatch".into())));
        assert_eq!(battlenet_asked(&strings(&["--exec=install emb"])), None);
        assert_eq!(battlenet_asked(&[]), None);

        // A game the fake does not have installed starts nothing.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ubisoft");
        assert_eq!(run(&strings(&["make-ubisoft", root.to_str().unwrap()])), 0);
        assert_eq!(uplay_url(&root, "uplay://launch/90410"), 1);
        assert_eq!(uplay_url(&root, "uplay://install/90410"), 0);
        assert_eq!(uplay_url(&root, "uplay://store/90410"), 2);
    }

    #[test]
    fn make_steam_reports_a_folder_it_cannot_write() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a-file");
        std::fs::write(&file, b"").unwrap();
        assert_eq!(run(&strings(&["make-steam", file.join("steam").to_str().unwrap()])), 1);
        assert_eq!(run(&strings(&["make-steam"])), 2);
        assert_eq!(run(&strings(&["play"])), 2);
    }
}
