//! Steam's own client calls, made over its CEF debugger (`crate::cef`) in its windowless page (#171): install into a
//! library, uninstall with no prompt of Steam's, and pause, resume or cancel a download. They are Steam's private
//! `SteamClient.Installs`, `.InstallFolder` and `.Downloads` objects, the ones its own Library page calls. Nothing of
//! Valve's documents them: what is known, and from where, is in docs/research/installs.md (section 2) and
//! docs/sources.md ("Steam's own calls").
//!
//! **The calls** ([`Call::calls`] names them, as the log does). Each signature below was read from Steam's own
//! client script (`sp.js`, in `SharedJSContext`) and called over the CEF debugger on the TV PC on 2026-11-17 (Steam
//! client of that date). The client id is the string `"0"` ([`CLIENT_ID`]), which that script exports as a constant
//! and passes everywhere as the remote client's id. **A call without it returns `undefined`, throws nothing and does
//! nothing**, which is why "returned" is never taken for "done" here.
//!
//! - Install ([`install`], #230): Steam's install wizard, a step a script ([`Wizard`]), its install manager looked at
//!   between them (`Installs.GetInstallManagerInfo()`, every 50 ms while it is waited on). `InstallFolder.
//!   GetInstallFolders()` (the libraries, to find the chosen one's `nFolderIndex`, or the one Steam marks
//!   `bIsDefaultFolder`) and `Installs.OpenInstallWizard([appid], "0")`; the manager waited on until its
//!   `eInstallState` is past 0 and the states on the way (1, 3, 5, 9: [`State`]), and is not a state an earlier
//!   wizard left (14 Complete or 16 Canceled, which stay until the next wizard opens) nor on another app
//!   (`currentAppID`, `rgApps`) (7 within half a second on the TV PC; still 0 after [`WIZARD_WAIT`]: not taken);
//!   `Installs.SetInstallFolder(index)` and `Installs.SetCreateShortcuts(false, false)`; the manager's own
//!   `nDiskSpaceRequired` against `nDiskSpaceAvailable`, as Steam's own UI holds them (`BShowDiskSettingsForInstall`):
//!   needed at least what is free, the install is refused with both; `Installs.ContinueInstall()`, and the manager
//!   waited on until it moves on. **Taken only at 14 (Complete), a started install** (#230: 8 was once taken for
//!   success); 15 (Failed) is a failure the user is told of. At 8
//!   (`ShowEULAs`) `Apps.LoadEula(appid)` gives the agreements (`[{id, url, version}]`), each put before the user in
//!   the Dashboard's own prompt with its text read from its url ([`crate::eula`]), and only the user's own Accept
//!   calls `Apps.MarkEulaAccepted(appid, id, version)`; once all are, `ContinueInstall()` again. Any other state (2, 4,
//!   6, 10 to 13, an unknown one), a Decline, a failure once the wizard is open, and not enough space all end in
//!   `Installs.CancelInstall()`, which takes the manager back to 0 (from `ShowConfig`, to 16 Canceled: seen on the TV
//!   PC 2026-11-18, both a clean cancel) and closes Steam's dialog, and the user is told
//!   why ([`Outcome::Stopped`]); never a fall back to `steam.exe`, which would go round them. No `SetAppList`:
//!   Steam's own flow does not call it before the wizard. Every state is said in the log by its name and number.
//! - Uninstall: `Installs.OpenUninstallWizard([appid], "0", true)`, the `true` passing over Steam's own question:
//!   gone within a second on the TV PC, with no dialog (the two-argument call returns `undefined` and does nothing).
//!   Taken only once the app's manifest says so ([`confirm`]).
//! - Pause: `Downloads.PauseAppUpdate(appid, "0")`; resume: `Downloads.ResumeAppUpdate(appid, "0")`, seen to stop a
//!   download and to have it go on. No `EnableAllDownloads`: resume worked without it, and Steam's script calls it
//!   (as `EnableAllDownloads(false, id)`) in one place in a sense not known. Cancel: the pause in a script of its own,
//!   a wait (5 s at most) for the manifest to stop saying the download runs, then
//!   `Downloads.RemoveFromDownloadList(appid, "0")` and the uninstall, which deletes what was fetched; taken once the
//!   manifest is gone or left behind with nothing under way, the log saying which ([`on_disk`]). Sent back to back in
//!   mid-download on the TV PC (2026-11-17), Steam removed the files but left the manifest at StateFlags 514; the
//!   wait is a guess at avoiding that, to be tried there.
//!
//! **Steam's windows.** `OpenInstallWizard` has Steam open its Install dialog (a page titled `Install`, where Steam
//! shows its own storage choice and licence agreements), which closes itself after `ContinueInstall` or
//! `CancelInstall`, and bring its main window up over the Dashboard, where it stays. Before #245 they were seen: the
//! main window for a moment, the dialog for as long as the Dashboard's own prompt was up. Now none is seen at all:
//! - The installer keeps the Dashboard in front of every window of Steam's from before the first script reaches it
//!   until this returns, whichever way the wizard ended (`model::Opener::keep_front`, `shell::front`): Home's window
//!   topmost, so Steam's come up beneath it; the foreground locked, so Steam cannot take it, nor with it the keyboard
//!   and Steam's own controller navigation (the pad's A on Steam's dialog); and a look every 15 ms that takes the
//!   foreground back from a window of Steam's that has it all the same, and puts Home's window above one of Steam's
//!   that came above it. Each is a `front:` line in the log.
//! - Steam's windows are left as they were found. An install looks first at how the main window stands
//!   ([`main_window`]: the window itself on Windows, its page [`MAIN`] besides), and puts it back that way after
//!   each step that may have brought it up and once more at the end ([`put_back`]): hidden again with
//!   `SteamClient.Window.HideWindow()` on its page (seen to work on the TV PC), or minimized; left where it was
//!   showing. The Install dialog is left to close itself, as it does; hiding it in mid-wizard (HideWindow on its own
//!   page) is not done: whether Steam's wizard goes on the same with its dialog hidden is not known, and with
//!   Home's window kept in front nothing of it is seen anyway.
//! - The Dashboard is put in front again after (`model::Opener::front`), where it no longer was.
//!
//! An install with no call that opens a dialog would need none of this: the console's `app_install` (#132) installs
//! with no window but passes over the licence agreements the user must accept (#230), and whether `SetAppList` and
//! `ContinueInstall` work without `OpenInstallWizard` is docs/research/installs.md's open question S1.
//!
//! **What comes back.** Each script runs every call in turn, catching what each throws, and gives back as JSON
//! whether it got to the end and, for each call, what it returned or threw ([`Answer`]). Every step is a line in the
//! log (`steam client: pause 620: SteamClient.Downloads.PauseAppUpdate(620, "0") -> undefined`), so one run on the
//! real client says which calls are there and what they give. A call that is missing throws (`TypeError: ... is not
//! a function`), a wait that runs out fails, and the script stops there.
//!
//! **The stand-in.** Each script starts with a comment that says, as JSON, what it asks ([`Call::of_script`]). Steam
//! passes over a comment; the stand-in Steam's debugger (`crate::debugger`) reads it and does to the fake's files what
//! Steam would, so that the end-to-end tests take this same route. It holds each script to the real client's
//! signatures: a call made without the client id is answered `undefined` and does nothing, as Steam's is.
use model::Steer;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::cef::{self, Evaluated, Failed};

/// How long a call's answer is waited for: the install's two waits for Steam take ten seconds at most.
pub const WAIT: Duration = Duration::from_secs(25);

/// How long the install waits for Steam's install manager to move, at each of its two waits: half a second on the
/// TV PC.
pub const WIZARD_WAIT: Duration = Duration::from_secs(5);

/// How long an uninstall or a cancel is given to show in the app's manifest before it is taken as not done.
pub const CONFIRM_WAIT: Duration = Duration::from_secs(10);

/// The id of this PC's own client, as every install, uninstall and download call takes it: Steam's script exports
/// it as the constant `"0"` (seen on the TV PC, 2026-11-17).
pub const CLIENT_ID: &str = "0";

/// The title of the page of Steam's main window, where `SteamClient.Window` is that window's.
pub const MAIN: &str = "Steam";

/// What the first line of every script starts with, before the call as JSON.
const HEADER: &str = "/* xbox-series-dashboard ";
/// What the first line of a script for Steam's main window starts with, before what it asks.
const WINDOW_HEADER: &str = "/* xbox-series-dashboard window ";

/// The install's first wait on Steam's install manager, as the log names it: the wizard open.
pub const WIZARD_OPEN: &str = "SteamClient.Installs.GetInstallManagerInfo() until eInstallState is not 0";
/// Its second: the install going, past the wizard.
pub const INSTALL_GOING: &str = "SteamClient.Installs.GetInstallManagerInfo() until eInstallState moves on";

/// A call as a script makes it, from its name in the log: the same text with its strings in single quotes, so that
/// the stand-in tells the call made (`'0'`) from its name in a note (`"0"`).
pub fn code(name: &str) -> String {
    name.replace('"', "'")
}

/// What is asked of Steam's client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "call")]
pub enum Call {
    /// Install the app into the library at `folder` (its root, as Steam names it), or with none into the library
    /// Steam marks its default.
    Install { appid: u32, folder: Option<String> },
    /// Uninstall it, with no question of Steam's.
    Uninstall { appid: u32 },
    /// Pause, resume or cancel its download.
    Steer { appid: u32, steer: Steer },
}

impl Call {
    pub fn appid(&self) -> u32 {
        match *self {
            Call::Install { appid, .. } | Call::Uninstall { appid } | Call::Steer { appid, .. } => appid,
        }
    }

    /// What it asks, as the log says it: `install 620 into Steam's default library`, `pause 620`.
    pub fn what(&self) -> String {
        match self {
            Call::Install { appid, folder: Some(folder) } => format!("install {appid} into {folder}"),
            Call::Install { appid, folder: None } => format!("install {appid} into Steam's default library"),
            Call::Uninstall { appid } => format!("uninstall {appid}"),
            Call::Steer { appid, steer } => format!("{} {appid}", steer.verb()),
        }
    }

    /// `SteamClient.Installs.OpenInstallWizard([appid], "0")`, as the log names it ([`code`] makes the call).
    pub fn wizard(appid: u32) -> String {
        format!("SteamClient.Installs.OpenInstallWizard([{appid}], \"{CLIENT_ID}\")")
    }

    /// `SteamClient.Installs.OpenUninstallWizard([appid], "0", true)`.
    pub fn uninstall_wizard(appid: u32) -> String {
        format!("SteamClient.Installs.OpenUninstallWizard([{appid}], \"{CLIENT_ID}\", true)")
    }

    /// `SteamClient.Downloads.PauseAppUpdate(appid, "0")`.
    pub fn pause(appid: u32) -> String {
        format!("SteamClient.Downloads.PauseAppUpdate({appid}, \"{CLIENT_ID}\")")
    }

    /// `SteamClient.Downloads.ResumeAppUpdate(appid, "0")`.
    pub fn resume(appid: u32) -> String {
        format!("SteamClient.Downloads.ResumeAppUpdate({appid}, \"{CLIENT_ID}\")")
    }

    /// `SteamClient.Downloads.RemoveFromDownloadList(appid, "0")`.
    pub fn remove(appid: u32) -> String {
        format!("SteamClient.Downloads.RemoveFromDownloadList({appid}, \"{CLIENT_ID}\")")
    }

    /// The calls that must go through for it to be done, in order, as the log names them; the install's with its
    /// two waits, as its wizard makes them where nothing stops it ([`Wizard`]).
    pub fn calls(&self) -> Vec<String> {
        let appid = self.appid();
        match self {
            Call::Install { .. } => vec![
                "SteamClient.InstallFolder.GetInstallFolders()".to_string(),
                Call::wizard(appid),
                WIZARD_OPEN.to_string(),
                "SteamClient.Installs.SetInstallFolder(index)".to_string(),
                "SteamClient.Installs.SetCreateShortcuts(false, false)".to_string(),
                LOOK.to_string(),
                CONTINUE.to_string(),
                INSTALL_GOING.to_string(),
            ],
            Call::Uninstall { .. } => vec![Call::uninstall_wizard(appid)],
            Call::Steer { steer: Steer::Pause, .. } => vec![Call::pause(appid)],
            Call::Steer { steer: Steer::Resume, .. } => vec![Call::resume(appid)],
            // The pause goes first in a script of its own ([`ask`]), and the download is let stop before these.
            Call::Steer { steer: Steer::Cancel, .. } => vec![Call::remove(appid), Call::uninstall_wizard(appid)],
        }
    }

    /// The script that makes the calls in the client's windowless page and gives back an [`Answer`] as JSON. An
    /// install is a script a step of its wizard ([`Wizard`]): its first is given.
    pub fn script(&self) -> String {
        let header = serde_json::to_string(self).unwrap_or_default();
        // `await call('<name>', () => <the call>);`, the call being its name in single quotes.
        let plain = |name: String| format!("await call('{name}', () => {});", code(&name));
        let body = match self {
            Call::Install { appid, folder } => return Wizard::Open { appid: *appid, folder: folder.clone() }.script(),
            Call::Uninstall { .. } | Call::Steer { .. } => {
                self.calls().into_iter().map(plain).collect::<Vec<_>>().join("\n    ")
            }
        };
        frame(&format!("{HEADER}{header} */"), &body)
    }

    /// What a script of [`script`](Call::script)'s asks, from its first line: what the stand-in Steam reads.
    pub fn of_script(script: &str) -> Option<Call> {
        let rest = script.strip_prefix(HEADER)?;
        let (json, _) = rest.split_once(" */")?;
        serde_json::from_str(json).ok()
    }
}

/// A script round `body`, its first line `header`: each call it makes noted as it goes (`call`, `note`, `fail`),
/// what `body` puts in `value` given back with them as JSON ([`Answer`]).
fn frame(header: &str, body: &str) -> String {
    format!(
        r#"{header}
(async () => {{
  const steps = [];
  let value;
  const said = v => {{
    if (v === undefined) return 'undefined';
    try {{ const t = JSON.stringify(v); return t === undefined ? String(v) : (t.length > 800 ? t.slice(0, 800) + '...' : t); }}
    catch (e) {{ return String(v); }}
  }};
  const note = (name, ok, text) => steps.push({{ call: name, ok, said: String(text) }});
  const fail = (name, text) => {{ note(name, false, text); throw new Error(text); }};
  const call = async (name, f, needed = true) => {{
    try {{ const v = await f(); note(name, true, said(v)); return v; }}
    catch (e) {{ note(name, false, String(e)); if (needed) throw e; return undefined; }}
  }};
  try {{
    {body}
    return JSON.stringify({{ done: true, steps, value }});
  }} catch (e) {{
    return JSON.stringify({{ done: false, steps, value }});
  }}
}})()"#
    )
}

/// What the first line of a script of the install wizard's starts with, before its step as JSON.
const WIZARD_HEADER: &str = "/* xbox-series-dashboard wizard ";

/// `SteamClient.Installs.GetInstallManagerInfo()`: the install manager, looked at.
pub const LOOK: &str = "SteamClient.Installs.GetInstallManagerInfo()";
/// `SteamClient.Installs.ContinueInstall()`.
pub const CONTINUE: &str = "SteamClient.Installs.ContinueInstall()";
/// `SteamClient.Installs.CancelInstall()`: the wizard closed, the manager back to 0, or at 16 (Canceled) from
/// `ShowConfig` (both seen on the TV PC, 2026-11-18).
pub const CANCEL: &str = "SteamClient.Installs.CancelInstall()";

/// A step of Steam's install wizard, each a script of its own, so that what comes between them (the state looked
/// at, the space weighed, the user asked) is the Dashboard's ([`install`]). The stand-in reads which from the
/// script's first line ([`Wizard::of_script`]) and moves its own install manager as the real one moves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "step")]
pub enum Wizard {
    /// `InstallFolder.GetInstallFolders()`, the library chosen (`folder`, its root as Steam names it, or with none
    /// the one Steam marks its default), and `Installs.OpenInstallWizard([appid], "0")`. Gives the library chosen,
    /// `{index, path}`, or null where Steam marks none default.
    Open { appid: u32, folder: Option<String> },
    /// `Installs.GetInstallManagerInfo()`: gives its answer.
    Look,
    /// `Installs.SetInstallFolder(index)` where a library was chosen, and `Installs.SetCreateShortcuts(false,
    /// false)`.
    Prepare { index: Option<i64> },
    /// `Installs.ContinueInstall()`.
    Continue,
    /// `Apps.LoadEula(appid)`: gives the agreements, `[{id, url, version}]`.
    Agreements { appid: u32 },
    /// `Apps.MarkEulaAccepted(appid, id, version)`, each as `LoadEula` gave it: only on the user's own Accept.
    Accept { appid: u32, id: serde_json::Value, version: serde_json::Value },
    /// `Installs.CancelInstall()`.
    Cancel,
}

impl Wizard {
    /// `SteamClient.Apps.LoadEula(appid)`.
    pub fn load_eula(appid: u32) -> String {
        format!("SteamClient.Apps.LoadEula({appid})")
    }

    /// `SteamClient.Apps.MarkEulaAccepted(appid, id, version)`, `id` and `version` as JSON (which a script reads as
    /// they are).
    pub fn mark_accepted(appid: u32, id: &serde_json::Value, version: &serde_json::Value) -> String {
        format!("SteamClient.Apps.MarkEulaAccepted({appid}, {id}, {version})")
    }

    /// Its calls, as the log names them and the script makes them: each is the call itself, but the folder's index.
    pub fn calls(&self) -> Vec<String> {
        match self {
            Wizard::Open { appid, .. } => {
                vec!["SteamClient.InstallFolder.GetInstallFolders()".into(), Call::wizard(*appid)]
            }
            Wizard::Look => vec![LOOK.into()],
            Wizard::Prepare { .. } => vec![
                "SteamClient.Installs.SetInstallFolder(index)".into(),
                "SteamClient.Installs.SetCreateShortcuts(false, false)".into(),
            ],
            Wizard::Continue => vec![CONTINUE.into()],
            Wizard::Agreements { appid } => vec![Wizard::load_eula(*appid)],
            Wizard::Accept { appid, id, version } => vec![Wizard::mark_accepted(*appid, id, version)],
            Wizard::Cancel => vec![CANCEL.into()],
        }
    }

    /// The step's script: its first line says what it is, as JSON.
    pub fn script(&self) -> String {
        let header = serde_json::to_string(self).unwrap_or_default();
        // A call's name as a string of the script's (JSON's), and the call itself.
        let named = |name: &str| serde_json::to_string(name).unwrap_or_default();
        let made = |name: &str, made: &str| format!("value = await call({}, () => {made});", named(name));
        let body = match self {
            Wizard::Open { appid, folder } => {
                let want = serde_json::to_string(folder).unwrap_or_else(|_| "null".to_string());
                let wizard = Call::wizard(*appid);
                format!(
                    r#"const want = {want};
    const norm = p => String(p || '').replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
    const folders = await call('SteamClient.InstallFolder.GetInstallFolders()', () => SteamClient.InstallFolder.GetInstallFolders());
    const list = Array.isArray(folders) ? folders : [];
    const chosen = want === null ? list.find(f => f && f.bIsDefaultFolder) : list.find(f => f && norm(f.strFolderPath) === norm(want));
    if (want !== null && !chosen) fail('choose the library', 'Steam has no library at ' + want);
    note('choose the library', true, chosen ? 'index ' + chosen.nFolderIndex + ', ' + chosen.strFolderPath : 'none marked default: left to Steam');
    await call({}, () => {});
    value = chosen ? {{ index: chosen.nFolderIndex, path: chosen.strFolderPath }} : null;"#,
                    named(&wizard),
                    code(&wizard)
                )
            }
            Wizard::Look => made(LOOK, LOOK),
            Wizard::Prepare { index } => {
                let shortcuts = "SteamClient.Installs.SetCreateShortcuts(false, false)";
                let folder = match index {
                    Some(index) => format!(
                        "await call('SteamClient.Installs.SetInstallFolder(index)', () => SteamClient.Installs.SetInstallFolder({index}));"
                    ),
                    None => "note('SteamClient.Installs.SetInstallFolder(index)', true, 'not called: Steam keeps its own');"
                        .to_string(),
                };
                format!("{folder}\n    await call({}, () => {shortcuts});", named(shortcuts))
            }
            Wizard::Continue => made(CONTINUE, CONTINUE),
            Wizard::Agreements { appid } => made(&Wizard::load_eula(*appid), &Wizard::load_eula(*appid)),
            Wizard::Accept { appid, id, version } => {
                let call = Wizard::mark_accepted(*appid, id, version);
                made(&call, &call)
            }
            Wizard::Cancel => made(CANCEL, CANCEL),
        };
        frame(&format!("{WIZARD_HEADER}{header} */"), &body)
    }

    /// What a script of [`script`](Wizard::script)'s asks, from its first line: what the stand-in Steam reads.
    pub fn of_script(script: &str) -> Option<Wizard> {
        let rest = script.strip_prefix(WIZARD_HEADER)?;
        let (json, _) = rest.split_once(" */")?;
        serde_json::from_str(json).ok()
    }
}

/// Steam's install manager's `eInstallState`, as its own script names them (#230, read there on the TV PC,
/// 2026-11-17 and 2026-11-18). A started install reads 14 (Complete); a cancel leaves 0, or 16 (Canceled) from
/// `ShowConfig`; either of 14 and 16 stays until the next wizard opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State(pub i64);

impl State {
    pub const NONE: State = State(0);
    pub const SHOW_CONFIG: State = State(7);
    pub const SHOW_EULAS: State = State(8);
    /// `Complete`: what the manager read once an install had started (seen on the TV PC).
    pub const STARTED: State = State(14);
    /// `Failed`.
    pub const FAILED: State = State(15);
    /// `Canceled`: where `CancelInstall()` from `ShowConfig` left the manager (seen on the TV PC).
    pub const CANCELED: State = State(16);

    /// Its name, as Steam's script has it.
    pub fn name(self) -> &'static str {
        match self.0 {
            0 => "None",
            1 => "Setup",
            2 => "WaitLicense",
            3 => "FreeLicense",
            4 => "ShowCDKey",
            5 => "WaitAppInfo",
            6 => "ShowPassword",
            7 => "ShowConfig",
            8 => "ShowEULAs",
            9 => "CreateApps",
            10 => "ReadFromMedia",
            11 => "ShowChangeMedia",
            12 => "WaitLegacyCDKeys",
            13 => "ShowSignup",
            14 => "Complete",
            15 => "Failed",
            16 => "Canceled",
            _ => "a state the Dashboard does not know",
        }
    }

    /// A state Steam passes through on its own, which is waited out: setting up, getting the app's details, a free
    /// licence, making the apps.
    pub fn on_the_way(self) -> bool {
        matches!(self.0, 1 | 3 | 5 | 9)
    }

    /// No wizard open: none, or what a wizard left behind it (Complete, Canceled), which stays until the next opens.
    pub fn closed(self) -> bool {
        matches!(self.0, 0 | 14 | 16)
    }

    /// What Steam asks for in a state only its own window answers, as the user is told it: the rest of the wizard's
    /// states but those the Dashboard goes through (none, on the way, the config, the agreements, complete) and
    /// those that end it (failed, cancelled).
    pub fn asks(self) -> Option<&'static str> {
        match self.0 {
            0 | 1 | 3 | 5 | 7 | 8 | 9 | 14 | 15 | 16 => None,
            2 => Some("a licence it is waiting for"),
            4 => Some("a product key"),
            6 => Some("a password"),
            10 | 11 => Some("the game's install discs"),
            12 => Some("a CD key"),
            13 => Some("a sign-up"),
            _ => Some("something the Dashboard can't answer"),
        }
    }
}

impl std::fmt::Display for State {
    /// `8 (ShowEULAs)`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.0, self.name())
    }
}

/// What the install manager said: its state, the apps its wizard is on where it says, and the bytes the install
/// needs and has free where it goes, where it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manager {
    pub state: State,
    pub apps: Vec<u32>,
    pub needed: Option<u64>,
    pub free: Option<u64>,
}

impl Manager {
    /// From `GetInstallManagerInfo()`'s answer: `eInstallState` (0 where there is none), the apps (`currentAppID`,
    /// and `rgApps`, each an appid or an object with `nAppID` or `appid`), `nDiskSpaceRequired` and
    /// `nDiskSpaceAvailable`, each a number or a number written as text.
    pub fn of(value: Option<&serde_json::Value>) -> Manager {
        let number = |key: &str| {
            let field = value.and_then(|value| value.get(key))?;
            field
                .as_u64()
                .or_else(|| field.as_f64().filter(|n| *n >= 0.0).map(|n| n as u64))
                .or_else(|| field.as_str().and_then(|text| text.trim().parse().ok()))
        };
        let state = value.and_then(|value| value.get("eInstallState")).and_then(serde_json::Value::as_i64).unwrap_or(0);
        let appid = |v: &serde_json::Value| {
            let v = v.get("nAppID").or_else(|| v.get("appid")).unwrap_or(v);
            v.as_u64().or_else(|| v.as_str().and_then(|text| text.trim().parse().ok())).map(|n| n as u32)
        };
        let mut apps: Vec<u32> = Vec::new();
        let current = value.and_then(|value| value.get("currentAppID")).and_then(appid).filter(|&app| app != 0);
        let listed = value.and_then(|value| value.get("rgApps")).and_then(serde_json::Value::as_array);
        for app in current.into_iter().chain(listed.into_iter().flatten().filter_map(appid)) {
            if app != 0 && !apps.contains(&app) {
                apps.push(app);
            }
        }
        Manager { state: State(state), apps, needed: number("nDiskSpaceRequired"), free: number("nDiskSpaceAvailable") }
    }

    /// Whether the wizard may be on `appid`: it says so, or names no app.
    pub fn on(&self, appid: u32) -> bool {
        self.apps.is_empty() || self.apps.contains(&appid)
    }

    /// Not enough room, as Steam's own UI weighs it: needed at least what is free. Not where either is not said,
    /// or nothing is said to be needed.
    pub fn no_space(&self) -> Option<(u64, u64)> {
        match (self.needed, self.free) {
            (Some(needed), Some(free)) if needed > 0 && needed >= free => Some((needed, free)),
            _ => None,
        }
    }
}

/// An agreement as `LoadEula` gives it: its id and version kept as Steam gave them, to be given back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eula {
    pub id: serde_json::Value,
    pub url: String,
    pub version: serde_json::Value,
}

impl Eula {
    /// `LoadEula`'s answer: each entry with an id and a url.
    pub fn list(value: Option<&serde_json::Value>) -> Vec<Eula> {
        let Some(list) = value.and_then(serde_json::Value::as_array) else { return Vec::new() };
        list.iter()
            .filter_map(|entry| {
                let id = entry.get("id").filter(|id| !id.is_null())?.clone();
                let url = entry.get("url")?.as_str()?.to_string();
                let version = entry.get("version").cloned().unwrap_or(serde_json::Value::Null);
                Some(Eula { id, url, version })
            })
            .collect()
    }
}

/// What a script gives back: whether it got to the end, each call it made, and what it was asked to give.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Answer {
    pub done: bool,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

/// A call the script made, and what it returned (`ok`) or threw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub call: String,
    pub ok: bool,
    pub said: String,
}

/// What came of asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Every call went through, and where it can be seen, Steam did it.
    Took,
    /// Not done, and safe to ask another way: the debugger was not reached, a call was missing or failed, or Steam
    /// was not seen to do it.
    NotTaken(String),
    /// The script was sent and the answer never came: it may well have run, and asking again another way would
    /// ask twice.
    Lost(String),
    /// An install that stopped, its wizard cancelled (#230): the user is told why, and nothing else is asked.
    Stopped(model::Stop),
    /// An install whose licence agreement could not be read here: its wizard cancelled, and left to Steam's own
    /// install dialog (`steam://install/<appid>`), where the user reads it.
    LeftToSteam,
}

/// A line in the log, as every line of this module's is: `steam client: ...`.
pub fn log(line: &str) {
    eprintln!("steam client: {line}");
}

/// Asks Steam's client through its debugger on `ports`, waiting [`WAIT`] at most for its answer, and says in the
/// log what was asked, each call and what it came to, and the outcome. With `root`, the Steam's folder: an install
/// puts Steam's main window back as it stood ([`main_window`], [`put_back`]), and an uninstall or a cancel is taken
/// only once the app's manifest says so ([`confirm`]). Blocks: run it on a thread of its own.
pub fn ask(ports: &[u16], root: Option<&Path>, call: &Call) -> Outcome {
    ask_user(ports, root, call, &mut |_| model::Reply::Decline)
}

/// [`ask`], an install's licence agreements put before the user by `user`, which waits for the answer: the user's
/// own Accept is the only one that accepts one.
pub fn ask_user(
    ports: &[u16],
    root: Option<&Path>,
    call: &Call,
    user: &mut dyn FnMut(&model::Agreement) -> model::Reply,
) -> Outcome {
    let what = call.what();
    let list: Vec<String> = ports.iter().map(u16::to_string).collect();
    log(&format!("{what}: asking Steam's client over its CEF debugger on 127.0.0.1:{}", list.join("/")));
    if let Call::Steer { appid, steer: Steer::Cancel } = *call {
        if let Some(lost) = pause_first(ports, root, appid, &what) {
            return lost;
        }
    }
    let outcome = match call {
        Call::Install { appid, folder } => {
            let before = main_window(ports, root, &what);
            let mut wizard = Steps { ports, what: &what, before, reached: false, user };
            let outcome = install(&mut wizard, *appid, folder.clone());
            // Whatever came of it, a script that reached Steam may have brought its main window up.
            if wizard.reached {
                put_back(ports, before, &what);
            }
            outcome
        }
        _ => run(ports, &call.script(), &what, true).0,
    };
    let outcome = match (&outcome, root) {
        (Outcome::Took, Some(root)) if on_disk(&crate::libraries(root), call).is_some() => {
            let steamapps = crate::libraries(root);
            confirm(call, || on_disk(&steamapps, call).filter(|(done, _)| *done).map(|(_, said)| said))
        }
        _ => outcome,
    };
    match &outcome {
        Outcome::Took => log(&format!("{what}: Steam's client took it")),
        Outcome::NotTaken(why) => log(&format!("{what}: not taken: {why}")),
        Outcome::Lost(why) => log(&format!("{what}: lost after sending ({why}); not asked again")),
        Outcome::Stopped(stop) => log(&format!("{what}: stopped, its wizard cancelled: {stop:?}")),
        Outcome::LeftToSteam => log(&format!("{what}: its wizard cancelled, and left to Steam's own install dialog")),
    }
    outcome
}

/// How often the install manager is looked at while it is waited on.
const LOOK_EVERY: Duration = Duration::from_millis(50);

/// The install wizard as [`install`] goes through it: each step sent ([`Steps::step`]), the manager looked at and
/// waited on, the user asked.
struct Steps<'a> {
    ports: &'a [u16],
    what: &'a str,
    /// How Steam's main window stood before the wizard brought it up.
    before: Shown,
    /// A step has reached Steam: its main window may be up.
    reached: bool,
    user: &'a mut dyn FnMut(&model::Agreement) -> model::Reply,
}

/// What a step came to: its value, or why it did not go through ([`Outcome::NotTaken`] or [`Outcome::Lost`]).
type Stepped = Result<Option<serde_json::Value>, Outcome>;

impl Steps<'_> {
    /// Sends the step, each call a line in the log (the looks, quiet, are said by [`Steps::wait`]).
    fn step(&mut self, step: &Wizard) -> Stepped {
        let quiet = *step == Wizard::Look;
        let (outcome, reached, value) = run(self.ports, &step.script(), self.what, !quiet);
        self.reached |= reached;
        match outcome {
            Outcome::Took => Ok(value),
            other => Err(other),
        }
    }

    fn look(&mut self) -> Result<Manager, Outcome> {
        self.step(&Wizard::Look).map(|value| Manager::of(value.as_ref()))
    }

    /// Looks at the manager every [`LOOK_EVERY`] until `done` says so of its state, [`WIZARD_WAIT`] at most, and
    /// says in the log each state it passed through. The manager as last seen, and whether `done` said so.
    fn wait(&mut self, name: &str, done: impl Fn(State) -> bool) -> Result<(Manager, bool), Outcome> {
        self.wait_on(name, |manager| done(manager.state))
    }

    /// [`Steps::wait`], `done` told the whole manager.
    fn wait_on(&mut self, name: &str, done: impl Fn(&Manager) -> bool) -> Result<(Manager, bool), Outcome> {
        let start = Instant::now();
        let mut said: Option<State> = None;
        loop {
            let manager = self.look()?;
            if said != Some(manager.state) {
                log(&format!(
                    "{}: {name}: eInstallState {} at {} ms",
                    self.what,
                    manager.state,
                    start.elapsed().as_millis()
                ));
                said = Some(manager.state);
            }
            if done(&manager) {
                return Ok((manager, true));
            }
            if start.elapsed() >= WIZARD_WAIT {
                log(&format!(
                    "{}: {name}: eInstallState still {} after {} ms",
                    self.what,
                    manager.state,
                    WIZARD_WAIT.as_millis()
                ));
                return Ok((manager, false));
            }
            std::thread::sleep(LOOK_EVERY);
        }
    }

    /// `CancelInstall()`, then the manager waited on until it is back at 0 or at 16 (Canceled), either a clean
    /// cancel: whatever comes of it is said in the log, and the stop given back as it was.
    fn cancel(&mut self, stop: Outcome) -> Outcome {
        match self.step(&Wizard::Cancel) {
            Ok(_) => {
                match self.wait("the wizard cancelled", |state| state == State::NONE || state == State::CANCELED) {
                    Ok((manager, true)) => {
                        log(&format!("{}: the wizard is closed, eInstallState {}", self.what, manager.state))
                    }
                    Ok((manager, false)) => log(&format!(
                        "{}: the wizard was cancelled and eInstallState is still {}: Steam's dialog may be left open",
                        self.what, manager.state
                    )),
                    Err(why) => {
                        log(&format!("{}: the wizard was cancelled and not looked at again: {why:?}", self.what))
                    }
                }
            }
            Err(why) => log(&format!("{}: CancelInstall() did not go through: {why:?}", self.what)),
        }
        stop
    }

    /// A failure once the wizard is open: cancelled, and the user told.
    fn failed(&mut self, why: Outcome, words: &str) -> Outcome {
        log(&format!("{}: {words}: {why:?}", self.what));
        self.cancel(Outcome::Stopped(model::Stop::Failed(words.to_string())))
    }

    /// Steam's main window hidden again as it stood: after a step that may have brought it up, and before the user
    /// is asked. The Dashboard is kept in front of it meanwhile (the module's "Steam's windows").
    fn put_back(&mut self) {
        if self.reached {
            put_back(self.ports, self.before, self.what);
        }
    }
}

/// The install's wizard, step by step (the module's "Install"): taken only once the manager reads a started install
/// (14), stopped with the wizard cancelled at anything else once it is open.
fn install(wizard: &mut Steps, appid: u32, folder: Option<String>) -> Outcome {
    // Where an earlier wizard left the manager (14 Complete, 16 Canceled), that state is no wizard opened; nor is
    // one that says it is on another app.
    let was = match wizard.look() {
        Ok(manager) => manager.state,
        Err(Outcome::NotTaken(why)) => return Outcome::NotTaken(why),
        Err(_) => State::NONE,
    };
    let chosen = match wizard.step(&Wizard::Open { appid, folder }) {
        Ok(value) => value.and_then(|value| value.get("index").and_then(serde_json::Value::as_i64)),
        Err(outcome) => return outcome,
    };
    let opened = |manager: &Manager| {
        let state = manager.state;
        manager.on(appid)
            && state != State::NONE
            && state != State::CANCELED
            && !state.on_the_way()
            && (state != was || state == State::SHOW_CONFIG)
    };
    let (manager, settled) = match wizard.wait_on(WIZARD_OPEN, opened) {
        Ok(seen) => seen,
        Err(Outcome::NotTaken(why)) => return wizard.failed(Outcome::NotTaken(why), "Steam's install didn't open"),
        Err(lost) => return wizard.failed(lost, "Steam's install didn't open"),
    };
    if !settled {
        // Never opened: safe to ask another way. Opened and stuck on the way: closed again, and told.
        if manager.state.closed() || manager.state == was || !manager.on(appid) {
            return Outcome::NotTaken(format!(
                "eInstallState still {} after {} ms: Steam did not take it",
                manager.state,
                WIZARD_WAIT.as_millis()
            ));
        }
        return wizard.failed(
            Outcome::NotTaken(format!("stuck at {}", manager.state)),
            "Steam didn't get the game ready to install",
        );
    }
    wizard.put_back();
    match manager.state {
        State::SHOW_CONFIG => {}
        State::SHOW_EULAS => return agreements(wizard, appid),
        State::STARTED => return Outcome::Took,
        State::FAILED => return failed_in_steam(wizard),
        state => return asks(wizard, state),
    }
    if let Err(why) = wizard.step(&Wizard::Prepare { index: chosen }) {
        return wizard.failed(why, "Steam didn't take the folder to install into");
    }
    // Not enough room, by the manager's own figures, as Steam's own UI weighs them before it goes on.
    let space = match wizard.look() {
        Ok(manager) => manager,
        Err(why) => return wizard.failed(why, "Steam didn't say how much space it needs"),
    };
    let figure = |n: Option<u64>| n.map_or("not said".to_string(), |n| format!("{n} bytes"));
    log(&format!(
        "{}: it needs {} and {} is free (nDiskSpaceRequired, nDiskSpaceAvailable)",
        wizard.what,
        figure(space.needed),
        figure(space.free)
    ));
    if let Some((needed, free)) = space.no_space() {
        log(&format!("{}: not enough space: {needed} bytes needed, {free} free; not continued", wizard.what));
        return wizard.cancel(Outcome::Stopped(model::Stop::NoSpace { needed, free }));
    }
    go_on(wizard, appid, State::SHOW_CONFIG)
}

/// `ContinueInstall()` from `from`, and what the manager then reads: a started install is taken, the agreements
/// asked, anything else stopped.
fn go_on(wizard: &mut Steps, appid: u32, from: State) -> Outcome {
    if let Err(why) = wizard.step(&Wizard::Continue) {
        return wizard.failed(why, "Steam didn't go on with the install");
    }
    let moved = |state: State| state != from && !state.on_the_way();
    let (manager, settled) = match wizard.wait(INSTALL_GOING, moved) {
        Ok(seen) => seen,
        Err(why) => return wizard.failed(why, "Steam didn't go on with the install"),
    };
    wizard.put_back();
    match manager.state {
        State::STARTED => Outcome::Took,
        _ if !settled && from == State::SHOW_EULAS => wizard.failed(
            Outcome::NotTaken(format!("still {}", manager.state)),
            "Steam still asks for its licence agreement",
        ),
        _ if !settled => {
            wizard.failed(Outcome::NotTaken(format!("still {}", manager.state)), "Steam didn't start the install")
        }
        State::SHOW_EULAS => agreements(wizard, appid),
        State::NONE | State::CANCELED => {
            log(&format!("{}: Steam closed its install wizard by itself", wizard.what));
            Outcome::Stopped(model::Stop::Failed("Steam closed its install without starting it.".into()))
        }
        State::FAILED => failed_in_steam(wizard),
        state => asks(wizard, state),
    }
}

/// Steam's wizard says its install failed (15): closed, and the user told.
fn failed_in_steam(wizard: &mut Steps) -> Outcome {
    wizard.failed(Outcome::NotTaken(format!("eInstallState {}", State::FAILED)), "Steam's install failed")
}

/// A state only Steam's own window answers (a product key, a password, one not known): cancelled, and the user told
/// what Steam asked for.
fn asks(wizard: &mut Steps, state: State) -> Outcome {
    let what = state.asks().unwrap_or("something the Dashboard can't answer");
    log(&format!("{}: Steam asks for {what} (eInstallState {state}); cancelled", wizard.what));
    wizard.cancel(Outcome::Stopped(model::Stop::StoreAsks(what.to_string())))
}

/// At `ShowEULAs`: each agreement Steam gives put before the user, its text read from its url; only the user's own
/// Accept marks one accepted, and once all are the install goes on.
fn agreements(wizard: &mut Steps, appid: u32) -> Outcome {
    let list = match wizard.step(&Wizard::Agreements { appid }) {
        Ok(value) => Eula::list(value.as_ref()),
        Err(why) => return wizard.failed(why, "Steam didn't give its licence agreement"),
    };
    if list.is_empty() {
        return wizard
            .failed(Outcome::NotTaken("LoadEula gave none".into()), "Steam didn't give its licence agreement");
    }
    let of = list.len();
    for (at, eula) in list.iter().enumerate() {
        let text = crate::eula::fetch(&eula.url);
        match &text {
            Ok(text) => log(&format!(
                "{}: licence agreement {} of {of} ({}, version {}): read, {} characters",
                wizard.what,
                at + 1,
                eula.id,
                eula.version,
                text.chars().count()
            )),
            Err(why) => log(&format!(
                "{}: licence agreement {} of {of} ({}): could not be read: {why}",
                wizard.what,
                at + 1,
                eula.id
            )),
        }
        let readable = text.is_ok();
        let agreement = model::Agreement { at: at + 1, of, text };
        wizard.put_back();
        log(&format!("{}: licence agreement {} of {of}: the user is asked", wizard.what, at + 1));
        let reply = (wizard.user)(&agreement);
        log(&format!("{}: licence agreement {} of {of}: the user answered {reply:?}", wizard.what, at + 1));
        match reply {
            model::Reply::Accept if readable => {
                let accept = Wizard::Accept { appid, id: eula.id.clone(), version: eula.version.clone() };
                if let Err(why) = wizard.step(&accept) {
                    return wizard.failed(why, "Steam didn't take the agreement as accepted");
                }
            }
            model::Reply::Accept | model::Reply::Decline => {
                return wizard.cancel(Outcome::Stopped(model::Stop::Declined));
            }
            model::Reply::OpenInStore => return wizard.cancel(Outcome::LeftToSteam),
        }
    }
    go_on(wizard, appid, State::SHOW_EULAS)
}

/// How long a cancel waits, after its pause, for the app's manifest to say the download has stopped.
pub const SETTLE_WAIT: Duration = Duration::from_secs(5);

/// A cancel's first step: the pause, in a script of its own, then (with `root`) a wait of [`SETTLE_WAIT`] at most
/// for the manifest to stop saying `UpdateRunning`, before the download is removed and the app uninstalled. On the
/// TV PC (2026-11-17) the three sent back to back in mid-download left a stale manifest (StateFlags 514, the files
/// gone) while Steam was still stopping the download; that the wait avoids it is a guess, to be tried there. The
/// pause not taken (already paused, say) is said and the cancel goes on; lost, nothing more is asked.
fn pause_first(ports: &[u16], root: Option<&Path>, appid: u32, what: &str) -> Option<Outcome> {
    let pause = Call::Steer { appid, steer: Steer::Pause };
    match run(ports, &pause.script(), what, true).0 {
        Outcome::Took | Outcome::Stopped(_) | Outcome::LeftToSteam => {}
        Outcome::NotTaken(why) => {
            log(&format!("{what}: its pause first was not taken ({why}); cancelling all the same"))
        }
        Outcome::Lost(why) => {
            log(&format!("{what}: lost after sending its pause ({why}); not asked again"));
            return Some(Outcome::Lost(why));
        }
    }
    let root = root?;
    let steamapps = crate::libraries(root);
    let start = Instant::now();
    loop {
        let flags = manifest_flags(&steamapps, appid);
        let running = flags.flatten().is_some_and(|flags| flags & crate::installs::UPDATE_RUNNING != 0);
        if !running {
            let said = match flags {
                None => "its manifest is gone".to_string(),
                Some(flags) => {
                    format!("its manifest says StateFlags {}", flags.map_or("none".into(), |f| f.to_string()))
                }
            };
            log(&format!("{what}: the download stopped ({said}) {} ms after the pause", start.elapsed().as_millis()));
            return None;
        }
        if start.elapsed() >= SETTLE_WAIT {
            log(&format!(
                "{what}: its manifest still says the download runs {} s after the pause; cancelling all the same",
                SETTLE_WAIT.as_secs()
            ));
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Sends `script` and, `saying` its steps, says in the log each call and what came back (a failed one always):
/// what it came to, whether it reached Steam at all, and the value it gave.
fn run(ports: &[u16], script: &str, what: &str, saying: bool) -> (Outcome, bool, Option<serde_json::Value>) {
    let sent = cef::evaluate_within(ports, cef::SHARED, script, WAIT);
    let reached = !matches!(sent, Err(Failed::NotSent(_)));
    let mut value = None;
    let outcome = match sent {
        Ok(Evaluated::Value(text)) => match serde_json::from_str::<Answer>(&text) {
            Ok(answer) => {
                for step in answer.steps.iter().filter(|step| saying || !step.ok) {
                    let arrow = if step.ok { "->" } else { "threw" };
                    log(&format!("{what}: {} {arrow} {}", step.call, step.said));
                }
                value = answer.value;
                match answer.steps.iter().find(|step| !step.ok) {
                    _ if answer.done => Outcome::Took,
                    Some(step) => Outcome::NotTaken(format!("{} failed: {}", step.call, step.said)),
                    None => Outcome::NotTaken("the script stopped before its end".into()),
                }
            }
            Err(_) => Outcome::NotTaken(format!("it answered {text:?}, which is not what the script gives")),
        },
        Ok(Evaluated::Threw(why)) => Outcome::NotTaken(format!("the script threw {why}")),
        Ok(Evaluated::Nothing) => Outcome::NotTaken("the script gave nothing back".into()),
        Err(Failed::NotSent(why)) => Outcome::NotTaken(format!("its debugger could not be reached ({why})")),
        Err(Failed::Lost(why)) => Outcome::Lost(why),
    };
    (outcome, reached, value)
}

/// The app's manifest's `StateFlags` in any of `steamapps`: None with no manifest, Some(None) for one without flags.
fn manifest_flags(steamapps: &[PathBuf], appid: u32) -> Option<Option<u64>> {
    steamapps.iter().find_map(|dir| {
        let text = std::fs::read(dir.join(format!("appmanifest_{appid}.acf"))).ok()?;
        let manifest = crate::vdf::parse_text(&String::from_utf8_lossy(&text)).ok();
        Some(manifest.and_then(|manifest| manifest.u64_at(&["AppState", "StateFlags"])))
    })
}

/// What the app's manifest, in any of `steamapps`, says of what `call` asked: whether it is done, and exactly what
/// was seen, for the log. An uninstall is done when the app no longer reads installed (no manifest, or flags without
/// `FullyInstalled`, or with `Uninstalling`). A cancel is done when the manifest is gone, or is left behind with no
/// download under way and not installed: what Steam left on the TV PC after a cancel in mid-download (StateFlags 514,
/// `UpdateRequired | UpdatePaused`, the files gone), which reads not installed and nothing under way here
/// (`installs::state`), so the game is offered Install. None for the other calls: nothing on disk says when they
/// are done.
pub fn on_disk(steamapps: &[PathBuf], call: &Call) -> Option<(bool, String)> {
    let flags = manifest_flags(steamapps, call.appid());
    let named = |flags: Option<u64>| flags.map_or("no StateFlags".to_string(), |flags| format!("StateFlags {flags}"));
    match (call, flags) {
        (Call::Uninstall { .. } | Call::Steer { steer: Steer::Cancel, .. }, None) => {
            Some((true, "its manifest is gone".into()))
        }
        (Call::Uninstall { .. }, Some(flags)) => Some(if crate::installs::installed(flags) {
            (false, format!("its manifest still says installed ({})", named(flags)))
        } else {
            (true, format!("its manifest says not installed ({})", named(flags)))
        }),
        (Call::Steer { steer: Steer::Cancel, .. }, Some(flags)) => {
            let left = flags.filter(|&flags| {
                crate::installs::state(flags, 0, 0, false).is_none() && !crate::installs::installed(Some(flags))
            });
            Some(match left {
                Some(flags) => (
                    true,
                    format!(
                        "its manifest is left behind at StateFlags {flags}, no download under way and not installed"
                    ),
                ),
                None => (false, format!("its manifest still says {}", named(flags))),
            })
        }
        _ => None,
    }
}

/// After Steam's client took an uninstall or a cancel: waits [`CONFIRM_WAIT`] at most for its manifest to say so
/// (`done`, looked at every 250 ms, giving what it saw), and says in the log exactly what it found. Not seen:
/// [`Outcome::NotTaken`], so that an uninstall is asked through `steam.exe` instead.
pub fn confirm(call: &Call, done: impl Fn() -> Option<String>) -> Outcome {
    let what = call.what();
    let start = Instant::now();
    loop {
        if let Some(seen) = done() {
            log(&format!("{what}: {seen}, {} ms on", start.elapsed().as_millis()));
            return Outcome::Took;
        }
        if start.elapsed() >= CONFIRM_WAIT {
            return Outcome::NotTaken(format!(
                "Steam's client returned, but the app's manifest does not say so {} s on",
                CONFIRM_WAIT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// What is asked of Steam's main window, in its own page ([`MAIN`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// How it stands: the page's `document.visibilityState` and `hasFocus()`, and `SteamClient.Window`'s
    /// `IsWindowMinimized()` and `GetWindowDetails()`, as JSON.
    Look,
    /// `SteamClient.Window.HideWindow()`: seen to hide it on the TV PC.
    Hide,
    /// `SteamClient.Window.Minimize()`.
    Minimize,
}

impl Window {
    fn word(self) -> &'static str {
        match self {
            Window::Look => "look",
            Window::Hide => "hide",
            Window::Minimize => "minimize",
        }
    }

    /// The script, its first line saying what it asks, for the stand-in.
    pub fn script(self) -> String {
        let body = match self {
            Window::Look => {
                "(async () => {\n  const w = SteamClient.Window;\n  const r = { visibility: document.visibilityState, \
                 focused: document.hasFocus() };\n  try { r.minimized = await w.IsWindowMinimized(); } catch (e) { \
                 r.minimized_threw = String(e); }\n  try { r.details = await w.GetWindowDetails(); } catch (e) { \
                 r.details_threw = String(e); }\n  return JSON.stringify(r);\n})()"
            }
            Window::Hide => "(async () => String(await SteamClient.Window.HideWindow()))()",
            Window::Minimize => "(async () => String(await SteamClient.Window.Minimize()))()",
        };
        format!("{WINDOW_HEADER}{} */\n{body}", self.word())
    }

    /// What a script of [`script`](Window::script)'s asks, from its first line.
    pub fn of_script(script: &str) -> Option<Window> {
        let rest = script.strip_prefix(WINDOW_HEADER)?;
        let (word, _) = rest.split_once(" */")?;
        [Window::Look, Window::Hide, Window::Minimize].into_iter().find(|window| window.word() == word)
    }
}

/// How Steam's main window stood.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Showing,
    Minimized,
    /// In the tray, or never opened.
    Hidden,
}

/// How Steam's main window stands by what its page says ([`Window::Look`]'s answer): minimized where
/// `IsWindowMinimized()` says so, showing where the page is `visible`, else hidden; with no page, hidden.
pub fn shown_by_page(answer: Option<&str>) -> Shown {
    let Some(value) = answer.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok()) else {
        return Shown::Hidden;
    };
    if value["minimized"].as_bool() == Some(true) {
        Shown::Minimized
    } else if value["visibility"].as_str() == Some("visible") {
        Shown::Showing
    } else {
        Shown::Hidden
    }
}

/// How long a look at, or a word to, Steam's main window is waited for.
const WINDOW_ANSWER: Duration = Duration::from_secs(3);

/// How Steam's main window stands now: by the window itself where Windows can say (the top-level window titled
/// "Steam" of `root`'s Steam: shown, minimized), else by its page; both said in the log, so that the TV PC shows
/// whether they agree.
pub fn main_window(ports: &[u16], root: Option<&Path>, what: &str) -> Shown {
    let page = match cef::evaluate_within(ports, MAIN, &Window::Look.script(), WINDOW_ANSWER) {
        Ok(Evaluated::Value(text)) => Ok(text),
        Ok(other) => Err(format!("{other:?}")),
        Err(why) => Err(why.to_string()),
    };
    let by_page = shown_by_page(page.as_ref().ok().map(String::as_str));
    let by_window = root.and_then(win::main_window);
    let shown = by_window.unwrap_or(by_page);
    let page = match page {
        Ok(text) => text,
        Err(why) => format!("none ({why})"),
    };
    let window = by_window.map_or("not looked at".to_string(), |shown| format!("{shown:?}"));
    log(&format!("{what}: Steam's main window before: {shown:?} (its page says {page}; the window {window})"));
    shown
}

/// When Steam's main window is put back after the install, from the moment the script ends: at once, and twice
/// more in case it comes up late.
const PUT_BACK_AT: [Duration; 3] = [Duration::ZERO, Duration::from_millis(400), Duration::from_millis(1200)];

/// Puts Steam's main window back as it stood `before` an install brought it up: hidden or minimized again on its
/// page; left alone where it was showing.
pub fn put_back(ports: &[u16], before: Shown, what: &str) {
    let window = match before {
        Shown::Showing => {
            log(&format!("{what}: Steam's main window left showing, as it was"));
            return;
        }
        Shown::Minimized => Window::Minimize,
        Shown::Hidden => Window::Hide,
    };
    let call = if window == Window::Hide { "HideWindow" } else { "Minimize" };
    let start = Instant::now();
    for at in PUT_BACK_AT {
        std::thread::sleep(at.saturating_sub(start.elapsed()));
        let said = match cef::evaluate_within(ports, MAIN, &window.script(), WINDOW_ANSWER) {
            Ok(Evaluated::Value(text)) => format!("-> {text}"),
            Ok(other) => format!("-> {other:?}"),
            Err(why) => format!("not asked: {why}"),
        };
        log(&format!(
            "{what}: Steam's main window put back as it was ({before:?}) {} ms on: SteamClient.Window.{call}() {said}",
            start.elapsed().as_millis()
        ));
    }
}

/// Steam's main window as Windows has it.
#[cfg(windows)]
mod win {
    use std::path::{Path, PathBuf};

    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    };

    use super::{Shown, MAIN};

    struct Search<'a> {
        root: &'a Path,
        found: Option<Shown>,
    }

    /// How the top-level window titled "Steam" of a `steam.exe` or `steamwebhelper.exe` under `root` stands:
    /// minimized, showing, or (none visible) hidden.
    pub fn main_window(root: &Path) -> Option<Shown> {
        let mut search = Search { root, found: None };
        unsafe { EnumWindows(Some(each), &mut search as *mut Search as LPARAM) };
        Some(search.found.unwrap_or(Shown::Hidden))
    }

    fn program(pid: u32) -> Option<PathBuf> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut path = [0u16; 1024];
            let mut len = path.len() as u32;
            let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
            CloseHandle(process);
            (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&path[..len as usize])))
        }
    }

    unsafe extern "system" fn each(window: HWND, param: LPARAM) -> i32 {
        let search = &mut *(param as *mut Search);
        if IsWindowVisible(window) == 0 {
            return 1;
        }
        let mut text = [0u16; 64];
        let len = GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
        if String::from_utf16_lossy(&text[..len]) != MAIN {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, &mut pid);
        let steams = program(pid).is_some_and(|path| {
            let name = path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
            crate::under(search.root, &path) && (name == "steam.exe" || name == "steamwebhelper.exe")
        });
        if !steams {
            return 1;
        }
        search.found = Some(if IsIconic(window) != 0 { Shown::Minimized } else { Shown::Showing });
        0
    }
}

/// Anywhere but Windows, the window is not looked at: its page says.
#[cfg(not(windows))]
mod win {
    pub fn main_window(_root: &std::path::Path) -> Option<super::Shown> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manager's states by Steam's own names; which a wizard leaves behind it, which ask for what only Steam's
    /// window gives; the apps it says it is on.
    #[test]
    fn the_managers_states_and_apps_read_as_steams_script_has_them() {
        assert_eq!(State(14).to_string(), "14 (Complete)");
        assert_eq!(State(16).to_string(), "16 (Canceled)");
        assert_eq!(State(15).name(), "Failed");
        assert!(State::NONE.closed() && State::STARTED.closed() && State::CANCELED.closed());
        assert!(!State::SHOW_CONFIG.closed());
        assert_eq!(State(12).asks(), Some("a CD key"));
        assert_eq!(State(13).asks(), Some("a sign-up"));
        assert_eq!(State(10).asks(), State(11).asks());
        assert!(State(10).asks().is_some());
        assert_eq!((State::FAILED.asks(), State::CANCELED.asks()), (None, None));

        let info = serde_json::json!({ "eInstallState": 16, "currentAppID": 620,
            "rgApps": [620, { "nAppID": 730 }, "440"], "nDiskSpaceRequired": "5", "nDiskSpaceAvailable": 9 });
        let manager = Manager::of(Some(&info));
        assert_eq!((manager.state, manager.apps.as_slice()), (State::CANCELED, [620, 730, 440].as_slice()));
        assert!(manager.on(730) && !manager.on(1));
        // Naming no app, it may be on any.
        let manager = Manager::of(Some(&serde_json::json!({ "eInstallState": 7, "currentAppID": 0 })));
        assert!(manager.apps.is_empty() && manager.on(1));
    }

    #[test]
    fn every_script_says_what_it_asks_on_its_first_line() {
        // An install is a step at a time, each step's script its own (#230): below.
        let calls = [
            Call::Uninstall { appid: 70 },
            Call::Steer { appid: 70, steer: Steer::Pause },
            Call::Steer { appid: 70, steer: Steer::Resume },
            Call::Steer { appid: 70, steer: Steer::Cancel },
        ];
        for call in calls {
            let script = call.script();
            assert_eq!(Call::of_script(&script), Some(call.clone()), "{script}");
            // Each call it must make is in it, by the name the log gives it.
            for name in call.calls() {
                assert!(script.contains(&format!("'{name}'")), "{name} is not in\n{script}");
            }
        }
        assert_eq!(Call::of_script("SteamClient.Console.ExecCommand('app_install 1'); 'asked'"), None);
        assert_eq!(Call::Install { appid: 1, folder: None }.what(), "install 1 into Steam's default library");
        assert_eq!(Call::Steer { appid: 1, steer: Steer::Cancel }.what(), "cancel 1");
        for window in [Window::Look, Window::Hide, Window::Minimize] {
            assert_eq!(Window::of_script(&window.script()), Some(window));
            assert_eq!(Call::of_script(&window.script()), None);
        }
        let steps = [
            Wizard::Open { appid: 620, folder: Some(r"D:\Steam Library".into()) },
            Wizard::Open { appid: 620, folder: None },
            Wizard::Look,
            Wizard::Prepare { index: Some(1) },
            Wizard::Prepare { index: None },
            Wizard::Continue,
            Wizard::Agreements { appid: 620 },
            Wizard::Accept { appid: 620, id: serde_json::json!("620_eula_0"), version: serde_json::json!(2) },
            Wizard::Cancel,
        ];
        for step in steps {
            let script = step.script();
            assert_eq!(Wizard::of_script(&script), Some(step.clone()), "{script}");
            assert_eq!(Call::of_script(&script), None);
            for name in step.calls() {
                let json = serde_json::to_string(&name).unwrap();
                assert!(script.contains(&format!("'{name}'")) || script.contains(&json), "{name} is not in\n{script}");
            }
        }
        let install = Call::Install { appid: 620, folder: None };
        assert_eq!(install.script(), Wizard::Open { appid: 620, folder: None }.script());
    }

    /// The signatures seen on the TV PC (2026-11-17): every call with the client id "0", the uninstall with three
    /// arguments, no SetAppList and no EnableAllDownloads.
    #[test]
    fn the_calls_are_made_as_steams_own_script_makes_them() {
        let install = Call::Install { appid: 620, folder: None }.script();
        assert!(install.contains("SteamClient.Installs.OpenInstallWizard([620], '0')"), "{install}");
        assert!(!install.contains("SetAppList") && !install.contains("OpenInstallWizard([620])"));
        // #230: Steam's own dialog's calls for its agreements and its Cancel.
        let eulas = Wizard::Agreements { appid: 620 }.script();
        assert!(eulas.contains("SteamClient.Apps.LoadEula(620)"), "{eulas}");
        let accept = Wizard::Accept { appid: 620, id: serde_json::json!("620_eula_0"), version: serde_json::json!(2) };
        let accept = accept.script();
        assert!(accept.contains(r#"SteamClient.Apps.MarkEulaAccepted(620, "620_eula_0", 2)"#), "{accept}");
        assert!(Wizard::Continue.script().contains("SteamClient.Installs.ContinueInstall()"));
        assert!(Wizard::Cancel.script().contains("SteamClient.Installs.CancelInstall()"));
        let uninstall = Call::Uninstall { appid: 70 }.script();
        assert!(uninstall.contains("() => SteamClient.Installs.OpenUninstallWizard([70], '0', true));"), "{uninstall}");
        let resume = Call::Steer { appid: 70, steer: Steer::Resume }.script();
        assert!(resume.contains("() => SteamClient.Downloads.ResumeAppUpdate(70, '0'));"));
        assert!(!resume.contains("EnableAllDownloads"));
        // Cancel's pause goes first in a script of its own (`pause_first`).
        let cancel = Call::Steer { appid: 70, steer: Steer::Cancel }.script();
        assert!(!cancel.contains("PauseAppUpdate"));
        for made in [
            "SteamClient.Downloads.RemoveFromDownloadList(70, '0')",
            "SteamClient.Installs.OpenUninstallWizard([70], '0', true)",
        ] {
            assert!(cancel.contains(&format!("() => {made});")), "{made} is not in\n{cancel}");
        }
    }

    #[test]
    fn a_folder_goes_into_the_script_as_a_string_whatever_is_in_it() {
        let call = Call::Install { appid: 620, folder: Some(r#"D:\it's "here""#.into()) };
        assert!(call.script().contains(r#"want = "D:\\it's \"here\"";"#), "{}", call.script());
        assert!(Call::Install { appid: 620, folder: None }.script().contains("want = null;"));
    }

    #[test]
    fn an_answer_is_read_as_the_script_writes_it() {
        let text = r#"{"done":false,"steps":[{"call":"SteamClient.Downloads.PauseAppUpdate(70, \"0\")","ok":false,
            "said":"TypeError: SteamClient.Downloads.PauseAppUpdate is not a function"}]}"#;
        let answer: Answer = serde_json::from_str(text).unwrap();
        assert!(!answer.done && !answer.steps[0].ok);
    }

    #[test]
    fn with_no_debugger_nothing_is_taken_and_another_way_is_safe() {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let outcome = ask(&[port], None, &Call::Steer { appid: 70, steer: Steer::Pause });
        assert!(matches!(outcome, Outcome::NotTaken(why) if why.contains("could not be reached")));
        assert!(matches!(ask(&[], None, &Call::Uninstall { appid: 70 }), Outcome::NotTaken(_)));
    }

    #[test]
    fn the_main_windows_page_says_how_it_stood() {
        assert_eq!(shown_by_page(Some(r#"{"visibility":"visible","focused":true,"minimized":false}"#)), Shown::Showing);
        assert_eq!(shown_by_page(Some(r#"{"visibility":"hidden","minimized":true}"#)), Shown::Minimized);
        assert_eq!(shown_by_page(Some(r#"{"visibility":"hidden","minimized":false}"#)), Shown::Hidden);
        // No page, or an answer that is not the look's: hidden, which the install then leaves it.
        assert_eq!(shown_by_page(None), Shown::Hidden);
        assert_eq!(shown_by_page(Some("undefined")), Shown::Hidden);
    }

    #[test]
    fn an_uninstall_is_done_on_disk_once_its_manifest_says_so_and_a_cancel_once_it_is_no_longer_under_way() {
        let dir = tempfile::tempdir().unwrap();
        let steamapps = vec![dir.path().to_path_buf()];
        let manifest = |flags: Option<u64>| {
            let flags = flags.map_or(String::new(), |flags| format!("\"StateFlags\" \"{flags}\""));
            std::fs::write(
                dir.path().join("appmanifest_70.acf"),
                format!("\"AppState\" {{ \"appid\" \"70\" {flags} }}"),
            )
            .unwrap();
        };
        let (uninstall, cancel) = (Call::Uninstall { appid: 70 }, Call::Steer { appid: 70, steer: Steer::Cancel });
        let seen = |call: &Call| on_disk(&steamapps, call).unwrap();
        let gone = (true, "its manifest is gone".to_string());
        assert_eq!(seen(&uninstall), gone);
        assert_eq!(seen(&cancel), gone);
        manifest(Some(4));
        assert_eq!(seen(&uninstall), (false, "its manifest still says installed (StateFlags 4)".into()));
        manifest(Some(4 | 2048));
        assert_eq!(seen(&uninstall), (true, "its manifest says not installed (StateFlags 2052)".into()));
        manifest(None);
        assert!(!seen(&uninstall).0, "no flags: taken at its word");
        manifest(Some(2 | 1024 | 256));
        assert_eq!(seen(&cancel), (false, "its manifest still says StateFlags 1282".into()));
        // What Steam left on the TV PC after a cancel in mid-download: said as it is, not as "gone".
        manifest(Some(514));
        let left = "its manifest is left behind at StateFlags 514, no download under way and not installed";
        assert_eq!(seen(&cancel), (true, left.to_string()));
        assert_eq!(on_disk(&steamapps, &Call::Steer { appid: 70, steer: Steer::Pause }), None);
    }
}
