//! The launchers' own apps (Settings > Set up this PC, docs/spec/set-up.md, #178): Steam, the Epic Games Launcher,
//! GOG Galaxy, the EA app, Ubisoft Connect and Battle.net, installed and removed from the Dashboard.
//!
//! **One table, in the Dashboard's own source** ([`builtin`]), as the helpers' is: each app's installer at its
//! vendor's own address, at one fixed version, with its size and SHA-256, and the switches that keep it quiet. The
//! ids, switches and hashes are those of the app's manifest in microsoft/winget-pkgs (winget's own catalogue, which
//! points at the same vendors' addresses), read on 2026-11-17; every update is a reviewed change of this table. On
//! the user's Yes the installer is downloaded, checked against that SHA-256 before it is kept, run, and deleted.
//! Nothing is downloaded or run otherwise.
//!
//! **What is installed** is what Windows' own list of installed programs says: each app's Uninstall key, matched as
//! winget matches it (its key's name, else its display name), read through `source_kit::registry` so a test's file
//! stands in for the registry. Removing an app runs the uninstaller that key names, with the switches that keep it
//! quiet where the vendor's installer kind has them.
//!
//! Nothing here needs Windows but the run itself; the threads and the log are the executable's
//! (`app::set_up`).
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use source_kit::registry::{self, Registry, Values, UNINSTALL};

use crate::Error;

/// The folder under the data folder an installer is downloaded to, one folder an app, and emptied after its run.
pub const FOLDER: &str = "set-up";

/// How an installer is run, as winget's manifest names its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// Nullsoft (NSIS): its own program, `/S`.
    Nsis,
    /// Inno Setup: its own program, `/VERYSILENT` and the rest.
    Inno,
    /// A Windows Installer package: run by `msiexec /i`, which never asks Windows for an administrator itself when
    /// told to be quiet, so it is always started through Windows' permission prompt.
    Msi,
    /// WiX Burn: its own program.
    Burn,
    /// Any other program.
    Exe,
}

/// A launcher's app, as the table names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Client {
    /// Its launcher's key, as its source has it: `steam`.
    pub key: String,
    pub name: String,
    pub vendor: String,
    /// The version installed: winget's for the manifest read.
    pub version: String,
    /// Its id in winget's catalogue, where the table's facts come from.
    pub winget: String,
    /// The vendor's own download page, which links the installer (or, where it links a web installer of no fixed
    /// version, the vendor's full installer at the same host).
    pub page: String,
    /// The installer's address: the vendor's own.
    pub url: String,
    /// Its size in bytes, as the vendor's address gave it.
    pub size: u64,
    /// Its SHA-256, lower-case hex. None: it is not fetched.
    pub sha256: Option<String>,
    /// Where the SHA-256 comes from, for whoever reviews the table.
    pub sha256_from: String,
    /// The name it is kept under while it runs.
    pub file: String,
    pub kind: Kind,
    /// Its switches, as one command line, exactly as winget passes them: `<INSTALLPATH>` stands for `location`.
    pub switches: String,
    /// Where it installs, for an installer that must be told (Battle.net's): `%VAR%`s expanded.
    #[serde(default)]
    pub location: Option<String>,
    /// Its installer shows a window of its own whatever it is told.
    pub window: bool,
    /// Its Uninstall key's names (the product codes winget's manifest gives).
    pub entries: Vec<String>,
    /// Its display name in Windows' list, where its key's name changes with each version.
    pub display: String,
    /// What makes its uninstaller quiet, added to the key's `UninstallString` where the key has no
    /// `QuietUninstallString` of its own. An MSI's is made from its product code instead.
    pub uninstall_switches: String,
    /// Its uninstaller shows a window of its own.
    pub uninstall_window: bool,
    /// What removing it does to its games, for the question.
    pub removing: String,
}

/// winget-pkgs, at the manifest each entry was read from.
const WINGET: &str = "https://github.com/microsoft/winget-pkgs/tree/master/manifests";

/// The apps the Dashboard installs: one fixed installer each. Looked up on 2026-11-17 in winget-pkgs and against
/// the vendors' own download pages (docs/spec/set-up.md, 2, has the links).
pub fn builtin() -> Vec<Client> {
    let from = |path: &str, checked: bool| {
        let checked = if checked {
            "; the file at the address, downloaded on 2026-11-17, has the same SHA-256"
        } else {
            "; not downloaded here (the vendor's address gave the same size and date as the manifest's)"
        };
        format!("winget-pkgs {WINGET}/{path} InstallerSha256{checked}")
    };
    vec![
        Client {
            key: "steam".into(),
            name: "Steam".into(),
            vendor: "Valve".into(),
            version: "2.10.91.91".into(),
            winget: "Valve.Steam".into(),
            page: "https://store.steampowered.com/about/".into(),
            url: "https://cdn.akamai.steamstatic.com/client/installer/SteamSetup.exe".into(),
            size: 2_380_800,
            sha256: Some("7d3654531c32d941b8cae81c4137fc542172bfa9635f169cb392f245a0a12bcb".into()),
            sha256_from: from("v/Valve/Steam/2.10.91.91", true),
            file: "SteamSetup.exe".into(),
            kind: Kind::Nsis,
            switches: "/S".into(),
            location: None,
            window: false,
            entries: vec!["Steam".into()],
            display: "Steam".into(),
            uninstall_switches: "/S".into(),
            uninstall_window: false,
            removing: "Steam's uninstaller deletes the games in its own folder with it; games in Steam libraries on \
                       other drives stay, and need Steam to play."
                .into(),
        },
        Client {
            key: "epic".into(),
            name: "Epic Games Launcher".into(),
            vendor: "Epic Games".into(),
            version: "1.3.210.0".into(),
            winget: "EpicGames.EpicGamesLauncher".into(),
            page: "https://store.epicgames.com/download".into(),
            url: "https://epicgames-download1.akamaized.net/Builds/UnrealEngineLauncher/Installers/Windows/\
                  EpicInstaller-20.3.3.msi?launcherfilename=EpicInstaller-20.3.3.msi"
                .into(),
            size: 264_237_056,
            sha256: Some("d55d79710edfeaa107a62f2ea007bec1eeeac8b715a43c2ba3858eec7bc60337".into()),
            sha256_from: from("e/EpicGames/EpicGamesLauncher/1.3.210.0", false),
            file: "EpicInstaller-20.3.3.msi".into(),
            kind: Kind::Msi,
            switches: "/quiet /norestart".into(),
            location: None,
            window: false,
            entries: vec!["{0716D12C-C7A6-4C1B-A5BB-E23B7160250D}".into()],
            display: "Epic Games Launcher".into(),
            uninstall_switches: "/quiet /norestart".into(),
            uninstall_window: false,
            removing: "Its games are reported to go with it, and most can't be played without it (or legendary)."
                .into(),
        },
        Client {
            key: "gog".into(),
            name: "GOG Galaxy".into(),
            vendor: "GOG".into(),
            version: "2.1.10.56".into(),
            winget: "GOG.Galaxy".into(),
            page: "https://www.gog.com/galaxy".into(),
            url:
                "https://content-system.gog.com/open_link/download?path=/open/galaxy/client/setup_galaxy_2.1.10.56.exe"
                    .into(),
            size: 342_291_968,
            sha256: Some("ec1ef3744a6ba365c022d5e114a482b74ff0496e00a91924ebdae401e2c9291b".into()),
            sha256_from: from("g/GOG/Galaxy/2.1.10.56", false),
            file: "setup_galaxy_2.1.10.56.exe".into(),
            kind: Kind::Inno,
            switches: "/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART".into(),
            location: None,
            window: false,
            entries: vec!["{7258BA11-600C-430E-A759-27E2C691A335}_is1".into()],
            display: "GOG GALAXY".into(),
            uninstall_switches: "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART".into(),
            uninstall_window: false,
            removing: "Its games stay and still play: they don't need Galaxy. Their achievements and some online \
                       play do."
                .into(),
        },
        Client {
            key: "ea".into(),
            name: "EA app".into(),
            vendor: "Electronic Arts".into(),
            version: "13.827.2.6342".into(),
            winget: "ElectronicArts.EADesktop".into(),
            page: "https://www.ea.com/ea-app".into(),
            url: "https://origin-a.akamaihd.net/EA-Desktop-Client-Download/installer-releases/\
                  EAappInstaller-13.827.2.6342-16601618.exe"
                .into(),
            size: 2_141_208,
            sha256: Some("6eafd193300fdfb7be5a014a2261b8c08e51d867a0d5590cb92aa055917c5cbd".into()),
            sha256_from: from("e/ElectronicArts/EADesktop/13.827.2.6342", true),
            file: "EAappInstaller-13.827.2.6342.exe".into(),
            kind: Kind::Burn,
            // The manifest's own Silent switches: a progress window shows.
            switches: "/passive /norestart".into(),
            location: None,
            window: true,
            entries: Vec::new(),
            display: "EA app".into(),
            uninstall_switches: String::new(),
            uninstall_window: true,
            removing: "Its games stay on the PC, but can't be played until it's back.".into(),
        },
        Client {
            key: "ubisoft".into(),
            name: "Ubisoft Connect".into(),
            vendor: "Ubisoft".into(),
            version: "175.1.0.13416".into(),
            winget: "Ubisoft.Connect".into(),
            page: "https://www.ubisoft.com/en-us/ubisoft-connect/download".into(),
            url: "https://static3.cdn.ubi.com/orbit/launcher_installer/UbisoftConnectInstaller.exe".into(),
            size: 263_096_496,
            sha256: Some("e907f33e1fc74fddd61eaf548647201ebc4baac8c439c3fa8e7043c67dc11576".into()),
            sha256_from: from("u/Ubisoft/Connect/175.1.0.13416", false),
            file: "UbisoftConnectInstaller.exe".into(),
            kind: Kind::Nsis,
            switches: "/S".into(),
            location: None,
            window: false,
            entries: vec!["Uplay".into()],
            display: "Ubisoft Connect".into(),
            uninstall_switches: "/S".into(),
            uninstall_window: false,
            removing: "Its games are reported to stay, but most can't be played without it.".into(),
        },
        Client {
            key: "battlenet".into(),
            name: "Battle.net".into(),
            vendor: "Blizzard".into(),
            version: "2.0.0.4232".into(),
            winget: "Blizzard.BattleNet".into(),
            page: "https://download.battle.net/en-us/desktop".into(),
            url: "https://downloader.battle.net/download/getInstaller?os=win&installer=Battle.net-Setup.exe".into(),
            size: 4_700_880,
            sha256: Some("f4643a2086c0cecb1ab68cc601bf38c1a0a6f7ef5914ad03375cc9449385bec6".into()),
            sha256_from: from("b/Blizzard/BattleNet/2.0.0.4232", true),
            file: "Battle.net-Setup.exe".into(),
            kind: Kind::Exe,
            switches: "--lang=enUS --installpath=\"<INSTALLPATH>\"".into(),
            location: Some(r"%ProgramFiles(x86)%\Battle.net".into()),
            window: true,
            entries: vec!["Battle.net".into()],
            display: "Battle.net".into(),
            uninstall_switches: String::new(),
            uninstall_window: true,
            removing: "Its games stay on the PC, but start and update only through it.".into(),
        },
    ]
}

/// A table of a test's own in the builtin one's place (`dashboard --client-table <file>`): the same fields, as JSON.
pub fn read_table(path: &Path) -> Result<Vec<Client>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The app as Settings shows it, standing as `state`.
pub fn shown(client: &Client, state: model::setup::ClientState) -> model::setup::Client {
    model::setup::Client {
        key: client.key.clone(),
        name: client.name.clone(),
        vendor: client.vendor.clone(),
        version: client.version.clone(),
        address: client.url.clone(),
        size: client.size,
        window: client.window,
        uninstall_window: client.uninstall_window,
        removing: client.removing.clone(),
        state,
    }
}

// ------------------------------------------------------------------------------------------------- what is installed

/// An app's Uninstall key, as Windows' list of installed programs has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The key, in full.
    pub key: String,
    pub display: String,
    /// The command that removes it, quiet where it can be: none where the key names none.
    pub uninstall: Option<Uninstall>,
    /// Where it is installed: the key's `InstallLocation`, else its uninstaller's folder.
    pub location: Option<PathBuf>,
}

/// The uninstaller, as it is run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uninstall {
    pub program: PathBuf,
    /// Its arguments, as one command line.
    pub line: String,
    /// Run through Windows' permission prompt from the start (an MSI's `msiexec /x`).
    pub elevate: bool,
}

/// The app's Uninstall key, if Windows' list has it: in either hive and either view, by its key's name, else its
/// display name. A key Windows hides (`SystemComponent` 1: an MSI a bundle installed) is passed over.
pub fn entry(registry: &Registry, client: &Client) -> Option<Entry> {
    UNINSTALL.iter().find_map(|root| {
        registry.subkeys(root).into_iter().find_map(|(name, values)| {
            if registry::get(&values, "SystemComponent") == Some("1") {
                return None;
            }
            let display = registry::get(&values, "DisplayName").unwrap_or_default().trim().to_string();
            let named = client.entries.iter().any(|entry| entry.eq_ignore_ascii_case(&name));
            let shown = !client.display.is_empty() && display.eq_ignore_ascii_case(&client.display);
            (named || shown).then(|| made(client, format!(r"{root}\{name}"), &name, display, &values))
        })
    })
}

fn made(client: &Client, key: String, name: &str, display: String, values: &Values) -> Entry {
    let uninstall = uninstall(client, name, values);
    let location = registry::get(values, "InstallLocation")
        .map(|location| PathBuf::from(location.trim().trim_matches('"')))
        .or_else(|| {
            // The uninstaller's folder; a Windows path, cut at its last slash of either kind wherever this runs.
            let program =
                uninstall.as_ref().filter(|u| !is_msiexec(&u.program))?.program.to_string_lossy().into_owned();
            let at = program.rfind(['\\', '/']).filter(|&at| at > 0)?;
            Some(PathBuf::from(&program[..at]))
        });
    Entry { key, display, uninstall, location }
}

fn is_msiexec(program: &Path) -> bool {
    let program = program.to_string_lossy();
    let name = program.rsplit(['\\', '/']).next().unwrap_or_default();
    name.eq_ignore_ascii_case("msiexec") || name.eq_ignore_ascii_case("msiexec.exe")
}

/// The command that removes it: the key's `QuietUninstallString` where it has one; an MSI's own `msiexec /x` with
/// its product code and the table's quiet switches; else its `UninstallString` and the table's switches for its kind.
fn uninstall(client: &Client, name: &str, values: &Values) -> Option<Uninstall> {
    if let Some(quiet) = registry::get(values, "QuietUninstallString") {
        let (program, words) = split(quiet)?;
        let line = words.iter().map(|word| quoted(word)).collect::<Vec<_>>().join(" ");
        let elevate = is_msiexec(&program);
        return Some(Uninstall { program, line, elevate });
    }
    let line = registry::get(values, "UninstallString")?;
    let (program, words) = split(line)?;
    if is_msiexec(&program) {
        // `MsiExec.exe /I{code}` or `/X{code}`: the product code is the key's own name.
        let code =
            words.iter().find_map(|w| w.get(2..).filter(|_| w.len() > 2 && w.starts_with('/'))).map(str::to_string);
        let code = code.filter(|code| code.starts_with('{')).unwrap_or_else(|| name.to_string());
        let line = format!("/x {code} {}", client.uninstall_switches).trim().to_string();
        return Some(Uninstall { program, line, elevate: true });
    }
    let given: Vec<String> = words.iter().map(|word| quoted(word)).collect();
    let line = [given.join(" "), client.uninstall_switches.clone()].join(" ").trim().to_string();
    Some(Uninstall { program, line, elevate: false })
}

/// A key's command line as its program and its words. Many keys leave the program's path unquoted, spaces and all
/// (Steam's `C:\Program Files (x86)\Steam\uninstall.exe`): as Windows reads them, an unquoted program runs to its
/// first `.exe`.
fn split(line: &str) -> Option<(PathBuf, Vec<String>)> {
    let line = line.trim();
    if !line.starts_with('"') {
        let lower = line.to_ascii_lowercase();
        let end = lower
            .match_indices(".exe")
            .map(|(at, _)| at + 4)
            .find(|&end| line[end..].chars().next().is_none_or(char::is_whitespace));
        if let Some(end) = end {
            return Some((PathBuf::from(&line[..end]), source_kit::command::words(&line[end..])));
        }
    }
    let mut words = source_kit::command::words(line);
    let program = words.first().filter(|program| !program.is_empty())?.clone();
    words.remove(0);
    Some((PathBuf::from(program), words))
}

/// A word as a command line has it: in quotes where it has a space in it.
fn quoted(word: &str) -> String {
    if word.is_empty() || word.contains([' ', '\t']) {
        format!("\"{word}\"")
    } else {
        word.to_string()
    }
}

// ------------------------------------------------------------------------------------------------- download and run

/// The app's folder under the data folder, where its installer is kept while it runs.
pub fn folder(data: &Path, client: &Client) -> PathBuf {
    data.join(FOLDER).join(&client.key)
}

/// Downloads the app's installer from its address into its folder, checking it against the table's SHA-256 as it
/// comes; only a file that is the one expected is kept, under its own name. `progress` is told the bytes so far, a
/// megabyte at a time. The installer's path.
pub fn download(
    agent: &ureq::Agent,
    data: &Path,
    client: &Client,
    progress: &mut dyn FnMut(u64),
) -> Result<PathBuf, Error> {
    let want = client.sha256.as_deref().filter(|hash| hash.len() == 64).ok_or(Error::NoHash)?.to_ascii_lowercase();
    let failed = |e: std::io::Error| Error::Failed(format!("it couldn't be written to the data folder ({e})"));
    let folder = folder(data, client);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).map_err(failed)?;
    let part = folder.join(format!("{}.part", client.file));
    let mut response = agent.get(&client.url).call().map_err(crate::transport)?;
    match response.status().as_u16() {
        200..=299 => {}
        status => return Err(Error::Failed(format!("its address answered {status}"))),
    }
    // A little more than the table's size, in case the vendor's server pads it: anything far bigger is not it.
    let most = client.size + client.size / 10 + (1 << 20);
    let mut body = response.body_mut().with_config().limit(most).reader();
    let mut file = std::fs::File::create(&part).map_err(failed)?;
    let (mut hash, mut done, mut said) = (Sha256::new(), 0u64, 0u64);
    let mut buffer = vec![0u8; 64 << 10];
    loop {
        let read = match body.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(e) => {
                drop(file);
                let _ = std::fs::remove_file(&part);
                return Err(Error::Failed(read_failed(&e)));
            }
        };
        hash.update(&buffer[..read]);
        file.write_all(&buffer[..read]).map_err(failed)?;
        done += read as u64;
        if done - said >= 1 << 20 {
            said = done;
            progress(done);
        }
    }
    file.flush().map_err(failed)?;
    drop(file);
    progress(done);
    let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if got != want {
        let _ = std::fs::remove_dir_all(&folder);
        return Err(Error::BadHash { want, got });
    }
    let installer = folder.join(&client.file);
    std::fs::rename(&part, &installer).map_err(failed)?;
    Ok(installer)
}

fn read_failed(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::TimedOut => "its address took too long to answer".into(),
        _ if e.to_string().to_lowercase().contains("limit") => "it is far bigger than it should be".into(),
        _ => "the download was cut off".into(),
    }
}

/// The installer's command: its program and its switches, with `<INSTALLPATH>` the table's place, and whether it
/// goes through Windows' permission prompt from the start (an MSI's `msiexec /i`).
pub fn install_command(client: &Client, installer: &Path) -> Uninstall {
    let location = client.location.as_deref().map(expand).unwrap_or_default();
    let switches = client.switches.replace("<INSTALLPATH>", &location);
    match client.kind {
        Kind::Msi => Uninstall {
            program: PathBuf::from("msiexec"),
            line: format!("/i \"{}\" {switches}", installer.display()).trim().to_string(),
            elevate: true,
        },
        _ => Uninstall { program: installer.to_path_buf(), line: switches, elevate: false },
    }
}

/// `%NAME%`s replaced by the environment's values; one the environment lacks is left as it is.
pub fn expand(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let Some(len) = rest[start + 1..].find('%') else { break };
        let name = &rest[start + 1..start + 1 + len];
        out.push_str(&rest[..start]);
        match std::env::var(name) {
            Ok(value) if !name.is_empty() => out.push_str(&value),
            _ => out.push_str(&rest[start..start + len + 2]),
        }
        rest = &rest[start + len + 2..];
    }
    out.push_str(rest);
    out
}

/// A program the Dashboard started for a set-up step, directly or through Windows' permission prompt.
pub struct Run {
    child: Child,
    /// It went through the permission prompt (`powershell Start-Process -Verb RunAs`), whose own exit is the
    /// program's.
    pub elevated: bool,
}

impl Run {
    /// Its exit code once it has ended; None while it runs.
    pub fn ended(&mut self) -> Option<Option<i32>> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(status.code()),
            Ok(None) => None,
            Err(_) => Some(None),
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

/// Starts the command as it is; where Windows says it needs an administrator (error 740) or it must have one from
/// the start, through `powershell Start-Process -Verb RunAs`, which puts Windows' permission prompt up and ends with
/// the program's exit code (1 if the prompt was declined). The prompt is on Windows' secure desktop, which no
/// program's input reaches, the controller's included: the user answers it with a mouse or keyboard.
pub fn start(command: &Uninstall, env: &[(String, std::ffi::OsString)]) -> std::io::Result<Run> {
    if !command.elevate {
        let mut plain = Command::new(&command.program);
        args(&mut plain, &command.line);
        plain.envs(env.iter().map(|(k, v)| (k, v)));
        if let Some(folder) = command.program.parent().filter(|f| !f.as_os_str().is_empty()) {
            plain.current_dir(folder);
        }
        match plain.spawn() {
            Ok(child) => return Ok(Run { child, elevated: false }),
            Err(e) if e.raw_os_error() != Some(740) => return Err(e),
            Err(_) => {}
        }
    }
    let quote = |text: &str| format!("'{}'", text.replace('\'', "''"));
    let list = if command.line.is_empty() { String::new() } else { format!(" -ArgumentList {}", quote(&command.line)) };
    let script = format!(
        "$p = Start-Process -FilePath {}{list} -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
        quote(&command.program.display().to_string())
    );
    let mut elevated = Command::new("powershell");
    elevated.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    elevated.envs(env.iter().map(|(k, v)| (k, v)));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: PowerShell's own console never shows; the prompt and the installer's window do.
        elevated.creation_flags(0x0800_0000);
    }
    Ok(Run { child: elevated.spawn()?, elevated: true })
}

/// The switches given as Windows takes a command line: as they are written, on Windows; split as Windows would split
/// them elsewhere (the tests).
fn args(command: &mut Command, line: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if !line.is_empty() {
            command.raw_arg(line);
        }
    }
    #[cfg(not(windows))]
    {
        let mut words = source_kit::command::words(&format!("x {line}"));
        words.remove(0);
        command.args(words);
    }
}

/// How long an installer may run before it is given up on, and how long an app's key may be there while its
/// installer still runs (one that starts the app and waits on it) before the install counts as done.
pub const INSTALL_MOST: Duration = Duration::from_secs(45 * 60);
pub const SETTLED: Duration = Duration::from_secs(20);
/// How long, after its program ends, Windows' list is looked at again before a step counts as failed: an
/// uninstaller that hands over to a copy of itself in the temporary folder (NSIS's) ends at once.
pub const AFTER_END: Duration = Duration::from_secs(60);
pub const REMOVE_MOST: Duration = Duration::from_secs(20 * 60);

/// How a set-up step ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Windows' list says what was asked: installed, or removed.
    Done,
    /// Its program ended and Windows' list does not say so: its exit code.
    NotDone(Option<i32>),
    /// It ran past its time.
    TooLong,
}

/// Follows a running step until Windows' list says it is done (`done` asked of the registry as it is now) and its
/// program has ended, or the list has said so for `SETTLED` with the program still running; or until it plainly did
/// not happen. `tick` is how often to look.
pub fn follow(
    run: &mut Run,
    done: &mut dyn FnMut() -> bool,
    most: Duration,
    tick: Duration,
    say: &mut dyn FnMut(&str),
) -> Outcome {
    let began = Instant::now();
    let (mut since, mut ended): (Option<Instant>, Option<(Instant, Option<i32>)>) = (None, None);
    loop {
        if ended.is_none() {
            if let Some(code) = run.ended() {
                say(&format!("ended with {}", code.map_or("no code".into(), |c| c.to_string())));
                ended = Some((Instant::now(), code));
            }
        }
        if done() {
            let since = *since.get_or_insert_with(Instant::now);
            if ended.is_some() || since.elapsed() >= SETTLED {
                return Outcome::Done;
            }
        } else {
            since = None;
        }
        if let Some((at, code)) = ended {
            if at.elapsed() >= AFTER_END {
                return Outcome::NotDone(code);
            }
        }
        if began.elapsed() >= most {
            return Outcome::TooLong;
        }
        std::thread::sleep(tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn values(pairs: &[(&str, &str)]) -> Values {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn client(key: &str) -> Client {
        builtin().into_iter().find(|client| client.key == key).unwrap()
    }

    #[test]
    fn the_table_fixes_a_vendors_address_a_size_and_a_sha256_for_each_launcher() {
        let clients = builtin();
        let keys: Vec<&str> = clients.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["steam", "epic", "gog", "ea", "ubisoft", "battlenet"]);
        let hosts = [
            ("steam", "cdn.akamai.steamstatic.com"),
            ("epic", "epicgames-download1.akamaized.net"),
            ("gog", "content-system.gog.com"),
            ("ea", "origin-a.akamaihd.net"),
            ("ubisoft", "static3.cdn.ubi.com"),
            ("battlenet", "downloader.battle.net"),
        ];
        for (client, (key, host)) in clients.iter().zip(hosts) {
            assert_eq!(client.key, key);
            assert!(client.url.starts_with(&format!("https://{host}/")), "{}", client.url);
            let hash = client.sha256.as_deref().unwrap();
            assert!(hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            assert!(client.size > 1_000_000 && client.page.starts_with("https://"));
            assert!(client.sha256_from.contains(&client.version), "{}", client.sha256_from);
            assert!(!client.url.contains(' '), "{}", client.url);
            assert!(!client.removing.is_empty() && !(client.entries.is_empty() && client.display.is_empty()));
        }
    }

    #[test]
    fn an_apps_key_is_found_by_its_name_or_its_display_name_and_hidden_ones_are_passed_over() {
        let root = r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
        let user = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
        let keys = BTreeMap::from([
            (
                format!(r"{root}\Steam"),
                values(&[("DisplayName", "Steam"), ("UninstallString", r"C:\Program Files (x86)\Steam\uninstall.exe")]),
            ),
            (
                format!(r"{user}\{{11111111-0000-0000-0000-000000000000}}"),
                values(&[
                    ("DisplayName", "Epic Games Launcher"),
                    ("UninstallString", "MsiExec.exe /I{11111111-0000-0000-0000-000000000000}"),
                    ("InstallLocation", r"C:\Program Files (x86)\Epic Games\"),
                ]),
            ),
            (
                format!(r"{root}\{{AAAA}}"),
                values(&[
                    ("DisplayName", "EA app"),
                    ("SystemComponent", "1"),
                    ("UninstallString", "MsiExec.exe /X{AAAA}"),
                ]),
            ),
            (
                format!(r"{root}\{{BBBB}}"),
                values(&[
                    ("DisplayName", "EA app"),
                    ("UninstallString", r#""C:\ProgramData\Package Cache\{BBBB}\EAappInstaller.exe"  /uninstall"#),
                    (
                        "QuietUninstallString",
                        r#""C:\ProgramData\Package Cache\{BBBB}\EAappInstaller.exe" /uninstall /quiet"#,
                    ),
                ]),
            ),
            (
                format!(r"{root}\Battle.net"),
                values(&[
                    ("DisplayName", "Battle.net"),
                    (
                        "UninstallString",
                        r#""C:\Program Files (x86)\Battle.net\Battle.net Uninstaller.exe" --lang=enUS --uid=battle.net"#,
                    ),
                    ("InstallLocation", r"C:\Program Files (x86)\Battle.net"),
                ]),
            ),
        ]);
        let registry = Registry::File(keys);

        let steam = entry(&registry, &client("steam")).unwrap();
        let uninstall = steam.uninstall.unwrap();
        assert_eq!(uninstall.program, PathBuf::from(r"C:\Program Files (x86)\Steam\uninstall.exe"));
        assert_eq!((uninstall.line.as_str(), uninstall.elevate), ("/S", false));
        assert_eq!(steam.location, Some(PathBuf::from(r"C:\Program Files (x86)\Steam")));

        // By its display name, its product code changing with each version; msiexec, quiet, through the prompt.
        let epic = entry(&registry, &client("epic")).unwrap();
        let uninstall = epic.uninstall.unwrap();
        assert_eq!(uninstall.line, "/x {11111111-0000-0000-0000-000000000000} /quiet /norestart");
        assert!(uninstall.elevate && is_msiexec(&uninstall.program));
        assert_eq!(epic.location, Some(PathBuf::from(r"C:\Program Files (x86)\Epic Games\")));

        // The bundle's key, not its hidden MSI's; the key's own quiet command.
        let ea = entry(&registry, &client("ea")).unwrap();
        assert!(ea.key.ends_with("{BBBB}"));
        assert_eq!(ea.uninstall.unwrap().line, "/uninstall /quiet");

        // Battle.net's own uninstaller, as the key names it.
        let battlenet = entry(&registry, &client("battlenet")).unwrap();
        assert_eq!(battlenet.uninstall.unwrap().line, "--lang=enUS --uid=battle.net");

        assert_eq!(entry(&registry, &client("gog")), None);
        assert_eq!(entry(&Registry::File(BTreeMap::new()), &client("ubisoft")), None);
    }

    #[test]
    fn an_installers_command_has_its_switches_and_an_msi_goes_through_msiexec_and_the_prompt() {
        let steam = install_command(&client("steam"), Path::new("SteamSetup.exe"));
        assert_eq!((steam.program, steam.line, steam.elevate), (PathBuf::from("SteamSetup.exe"), "/S".into(), false));
        let epic = install_command(&client("epic"), Path::new("x.msi"));
        assert_eq!(
            (epic.program, epic.line, epic.elevate),
            (PathBuf::from("msiexec"), "/i \"x.msi\" /quiet /norestart".into(), true)
        );
        let gog = install_command(&client("gog"), Path::new("g.exe"));
        assert_eq!(gog.line, "/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART");
        let battlenet = Client { location: Some("%E2E_NO_SUCH_VAR%\\Battle.net".into()), ..client("battlenet") };
        assert_eq!(
            install_command(&battlenet, Path::new("b.exe")).line,
            "--lang=enUS --installpath=\"%E2E_NO_SUCH_VAR%\\Battle.net\""
        );
    }

    #[test]
    fn environment_variables_are_expanded_and_unknown_ones_kept() {
        std::env::set_var("SETUP_TEST_DIR", "C:\\x");
        assert_eq!(expand("%SETUP_TEST_DIR%\\Battle.net"), "C:\\x\\Battle.net");
        assert_eq!(expand("%NOT_SET_HERE_EVER%\\a"), "%NOT_SET_HERE_EVER%\\a");
        assert_eq!(expand("100%"), "100%");
    }

    #[test]
    fn an_installer_is_kept_only_once_it_is_the_one_expected() {
        let good = b"MZ stand-in installer".to_vec();
        let service = crate::fake::FakeHelpers::start(0, good.clone()).unwrap();
        let data = tempfile::tempdir().unwrap();
        let table = service.client_table(data.path());
        let agent = crate::agent();
        let mut said = Vec::new();
        let steam = table.iter().find(|c| c.key == "steam").unwrap();
        let kept = download(&agent, data.path(), steam, &mut |n| said.push(n)).unwrap();
        assert_eq!(std::fs::read(&kept).unwrap(), good);
        assert_eq!(said.last(), Some(&(good.len() as u64)));
        let gog = table.iter().find(|c| c.key == "gog").unwrap();
        let refused = download(&agent, data.path(), gog, &mut |_| {});
        assert!(matches!(refused, Err(Error::BadHash { .. })), "{refused:?}");
        assert!(!folder(data.path(), gog).exists());
        let requests = service.requests();
        assert!(requests
            .iter()
            .all(|r| r.user_agent.as_deref() == Some(crate::USER_AGENT) && r.authorization.is_none()));
    }
}
