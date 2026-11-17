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
//! - Install: `InstallFolder.GetInstallFolders()` (the libraries, to find the chosen one's `nFolderIndex`, or the
//!   one Steam marks `bIsDefaultFolder`), `Installs.OpenInstallWizard([appid], "0")`, then
//!   `Installs.GetInstallManagerInfo()` every 50 ms until its `eInstallState` is not 0, nor a state past the wizard
//!   that an earlier install left (7 within half a second on the TV PC; not there after [`WIZARD_WAIT`]: not taken), `Installs.SetInstallFolder(index)`,
//!   `Installs.SetCreateShortcuts(false, false)`, `Installs.ContinueInstall()`, and `GetInstallManagerInfo()` again
//!   until the state moves on from the wizard's (14 on the TV PC, the download started; not within [`WIZARD_WAIT`]:
//!   not taken). No `SetAppList`: Steam's own flow does not call it before the wizard.
//! - Uninstall: `Installs.OpenUninstallWizard([appid], "0", true)`, the `true` passing over Steam's own question:
//!   gone within a second on the TV PC, with no dialog (the two-argument call returns `undefined` and does nothing).
//!   Taken only once the app's manifest says so ([`confirm`]).
//! - Pause: `Downloads.PauseAppUpdate(appid, "0")`; resume: `Downloads.ResumeAppUpdate(appid, "0")`, seen to stop a
//!   download and to have it go on. No `EnableAllDownloads`: resume worked without it, and Steam's script calls it
//!   (as `EnableAllDownloads(false, id)`) in one place in a sense not known. Cancel: pause,
//!   `Downloads.RemoveFromDownloadList(appid, "0")`, then the uninstall, which deletes what was fetched; taken once
//!   the manifest is gone. Seen only on a download that had already finished: in mid-download it is unproven.
//!
//! **Steam's main window.** `OpenInstallWizard` has Steam open its Install dialog (a page titled `Install`), which
//! closes itself after `ContinueInstall`, and bring its main window up over the Dashboard, where it stays. So an
//! install looks first at how the main window stands ([`main_window`]: the window itself on Windows, its page
//! [`MAIN`] besides), and puts it back that way straight after ([`put_back`]): hidden again with
//! `SteamClient.Window.HideWindow()` on its page (seen to work on the TV PC), or minimized; left where it was
//! showing. The installer then has the Dashboard put in front again (`model::Opener::front`).
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
    /// two waits.
    pub fn calls(&self) -> Vec<String> {
        let appid = self.appid();
        match self {
            Call::Install { .. } => vec![
                "SteamClient.InstallFolder.GetInstallFolders()".to_string(),
                Call::wizard(appid),
                WIZARD_OPEN.to_string(),
                "SteamClient.Installs.SetInstallFolder(index)".to_string(),
                "SteamClient.Installs.SetCreateShortcuts(false, false)".to_string(),
                "SteamClient.Installs.ContinueInstall()".to_string(),
                INSTALL_GOING.to_string(),
            ],
            Call::Uninstall { .. } => vec![Call::uninstall_wizard(appid)],
            Call::Steer { steer: Steer::Pause, .. } => vec![Call::pause(appid)],
            Call::Steer { steer: Steer::Resume, .. } => vec![Call::resume(appid)],
            Call::Steer { steer: Steer::Cancel, .. } => {
                vec![Call::pause(appid), Call::remove(appid), Call::uninstall_wizard(appid)]
            }
        }
    }

    /// The script that makes the calls in the client's windowless page and gives back an [`Answer`] as JSON.
    pub fn script(&self) -> String {
        let header = serde_json::to_string(self).unwrap_or_default();
        let appid = self.appid();
        // `await call('<name>', () => <the call>);`, the call being its name in single quotes.
        let plain = |name: String| format!("await call('{name}', () => {});", code(&name));
        let body = match self {
            Call::Install { folder, .. } => {
                let want = folder.as_deref().map_or("null".to_string(), |folder| {
                    serde_json::to_string(folder).unwrap_or_else(|_| "null".to_string())
                });
                let wizard = plain(Call::wizard(appid));
                let wait = WIZARD_WAIT.as_millis();
                format!(
                    r#"const want = {want};
    const norm = p => String(p || '').replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
    const folders = await call('SteamClient.InstallFolder.GetInstallFolders()', () => SteamClient.InstallFolder.GetInstallFolders());
    const list = Array.isArray(folders) ? folders : [];
    const chosen = want === null ? list.find(f => f && f.bIsDefaultFolder) : list.find(f => f && norm(f.strFolderPath) === norm(want));
    if (want !== null && !chosen) fail('choose the library', 'Steam has no library at ' + want);
    note('choose the library', true, chosen ? 'index ' + chosen.nFolderIndex + ', ' + chosen.strFolderPath : 'none marked default: left to Steam');
    const stateOf = v => (v && typeof v.eInstallState === 'number') ? v.eInstallState : 0;
    const until = async (name, done) => {{
      const end = Date.now() + {wait};
      for (;;) {{
        let now;
        try {{ now = await SteamClient.Installs.GetInstallManagerInfo(); }} catch (e) {{ fail(name, String(e)); }}
        if (done(stateOf(now))) {{ note(name, true, said(now)); return stateOf(now); }}
        if (Date.now() >= end) fail(name, 'eInstallState still ' + stateOf(now) + ' after {wait} ms: Steam did not take it; ' + said(now));
        await sleep(50);
      }}
    }};
    // Where an earlier install left the manager past the wizard (14), the same state again is no wizard opened.
    const was = stateOf(await call('SteamClient.Installs.GetInstallManagerInfo() before', () => SteamClient.Installs.GetInstallManagerInfo(), false));
    {wizard}
    const wizard = await until('{WIZARD_OPEN}', s => s !== 0 && (s !== was || s === 7));
    if (chosen) await call('SteamClient.Installs.SetInstallFolder(index)', () => SteamClient.Installs.SetInstallFolder(chosen.nFolderIndex));
    else note('SteamClient.Installs.SetInstallFolder(index)', true, 'not called: Steam keeps its own');
    await call('SteamClient.Installs.SetCreateShortcuts(false, false)', () => SteamClient.Installs.SetCreateShortcuts(false, false));
    await call('SteamClient.Installs.ContinueInstall()', () => SteamClient.Installs.ContinueInstall());
    await until('{INSTALL_GOING}', s => s !== wizard);"#
                )
            }
            Call::Uninstall { .. } | Call::Steer { .. } => {
                self.calls().into_iter().map(plain).collect::<Vec<_>>().join("\n    ")
            }
        };
        format!(
            r#"{HEADER}{header} */
(async () => {{
  const steps = [];
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
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  try {{
    {body}
    return JSON.stringify({{ done: true, steps }});
  }} catch (e) {{
    return JSON.stringify({{ done: false, steps }});
  }}
}})()"#
        )
    }

    /// What a script of [`script`](Call::script)'s asks, from its first line: what the stand-in Steam reads.
    pub fn of_script(script: &str) -> Option<Call> {
        let rest = script.strip_prefix(HEADER)?;
        let (json, _) = rest.split_once(" */")?;
        serde_json::from_str(json).ok()
    }
}

/// What a script gives back: whether it got to the end, and each call it made.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Answer {
    pub done: bool,
    pub steps: Vec<Step>,
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
    let what = call.what();
    let list: Vec<String> = ports.iter().map(u16::to_string).collect();
    log(&format!("{what}: asking Steam's client over its CEF debugger on 127.0.0.1:{}", list.join("/")));
    let before = matches!(call, Call::Install { .. }).then(|| main_window(ports, root, &what));
    let sent = cef::evaluate_within(ports, cef::SHARED, &call.script(), WAIT);
    let reached = !matches!(sent, Err(Failed::NotSent(_)));
    let outcome = match sent {
        Ok(Evaluated::Value(text)) => match serde_json::from_str::<Answer>(&text) {
            Ok(answer) => {
                for step in &answer.steps {
                    let arrow = if step.ok { "->" } else { "threw" };
                    log(&format!("{what}: {} {arrow} {}", step.call, step.said));
                }
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
    // Whatever came of it, a script that reached Steam may have brought its main window up.
    if let Some(before) = before.filter(|_| reached) {
        put_back(ports, before, &what);
    }
    let outcome = match (&outcome, root) {
        (Outcome::Took, Some(root)) if done_on_disk(&crate::libraries(root), call).is_some() => {
            let steamapps = crate::libraries(root);
            confirm(call, || done_on_disk(&steamapps, call) == Some(true))
        }
        _ => outcome,
    };
    match &outcome {
        Outcome::Took => log(&format!("{what}: Steam's client took it")),
        Outcome::NotTaken(why) => log(&format!("{what}: not taken: {why}")),
        Outcome::Lost(why) => log(&format!("{what}: lost after sending ({why}); not asked again")),
    }
    outcome
}

/// Whether the app's manifest, in any of `steamapps`, says what `call` asked is done: an uninstall's app no longer
/// installed (no manifest, or one that says it is not, or is being uninstalled); a cancel's manifest gone, or no
/// longer under way and not installed. None for the other calls: nothing on disk says when they are done.
pub fn done_on_disk(steamapps: &[PathBuf], call: &Call) -> Option<bool> {
    let appid = call.appid();
    // None: no manifest; Some(None): one with no StateFlags.
    let flags: Option<Option<u64>> = steamapps.iter().find_map(|dir| {
        let text = std::fs::read(dir.join(format!("appmanifest_{appid}.acf"))).ok()?;
        let manifest = crate::vdf::parse_text(&String::from_utf8_lossy(&text)).ok();
        Some(manifest.and_then(|manifest| manifest.u64_at(&["AppState", "StateFlags"])))
    });
    match call {
        Call::Uninstall { .. } => Some(flags.is_none_or(|flags| !crate::installs::installed(flags))),
        Call::Steer { steer: Steer::Cancel, .. } => Some(flags.is_none_or(|flags| {
            flags.is_some_and(|flags| {
                crate::installs::state(flags, 0, 0, false).is_none() && !crate::installs::installed(Some(flags))
            })
        })),
        _ => None,
    }
}

/// After Steam's client took an uninstall or a cancel: waits [`CONFIRM_WAIT`] at most for its manifest to say so
/// (`done`, looked at every 250 ms), and says in the log what it found. Not seen: [`Outcome::NotTaken`], so that an
/// uninstall is asked through `steam.exe` instead.
pub fn confirm(call: &Call, done: impl Fn() -> bool) -> Outcome {
    let what = call.what();
    let start = Instant::now();
    loop {
        if done() {
            log(&format!("{what}: its manifest says so {} ms on", start.elapsed().as_millis()));
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

    #[test]
    fn every_script_says_what_it_asks_on_its_first_line() {
        let calls = [
            Call::Install { appid: 620, folder: Some(r"D:\Steam Library".into()) },
            Call::Install { appid: 620, folder: None },
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
    }

    /// The signatures seen on the TV PC (2026-11-17): every call with the client id "0", the uninstall with three
    /// arguments, no SetAppList and no EnableAllDownloads.
    #[test]
    fn the_calls_are_made_as_steams_own_script_makes_them() {
        let install = Call::Install { appid: 620, folder: None }.script();
        assert!(install.contains("() => SteamClient.Installs.OpenInstallWizard([620], '0'));"), "{install}");
        assert!(!install.contains("SetAppList") && !install.contains("OpenInstallWizard([620])"));
        // The wizard taken only once Steam's install manager moves, and the install once it moves on again.
        assert!(install.contains(&format!("await until('{WIZARD_OPEN}', s => s !== 0 && (s !== was || s === 7));")));
        assert!(install.contains(&format!("await until('{INSTALL_GOING}', s => s !== wizard);")));
        let uninstall = Call::Uninstall { appid: 70 }.script();
        assert!(uninstall.contains("() => SteamClient.Installs.OpenUninstallWizard([70], '0', true));"), "{uninstall}");
        let resume = Call::Steer { appid: 70, steer: Steer::Resume }.script();
        assert!(resume.contains("() => SteamClient.Downloads.ResumeAppUpdate(70, '0'));"));
        assert!(!resume.contains("EnableAllDownloads"));
        let cancel = Call::Steer { appid: 70, steer: Steer::Cancel }.script();
        for made in [
            "SteamClient.Downloads.PauseAppUpdate(70, '0')",
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
        assert_eq!(done_on_disk(&steamapps, &uninstall), Some(true), "no manifest");
        assert_eq!(done_on_disk(&steamapps, &cancel), Some(true), "no manifest");
        manifest(Some(4));
        assert_eq!(done_on_disk(&steamapps, &uninstall), Some(false));
        manifest(Some(4 | 2048));
        assert_eq!(done_on_disk(&steamapps, &uninstall), Some(true), "uninstalling");
        manifest(None);
        assert_eq!(done_on_disk(&steamapps, &uninstall), Some(false), "no flags: taken at its word");
        manifest(Some(2 | 1024 | 256));
        assert_eq!(done_on_disk(&steamapps, &cancel), Some(false), "downloading");
        manifest(Some(2 | 512));
        assert_eq!(done_on_disk(&steamapps, &cancel), Some(true), "left behind, no longer under way");
        assert_eq!(done_on_disk(&steamapps, &Call::Steer { appid: 70, steer: Steer::Pause }), None);
    }
}
