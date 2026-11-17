//! Steam's own client calls, made over its CEF debugger (`crate::cef`) in its windowless page (#171): install into a
//! library, uninstall with no prompt of Steam's, and pause, resume or cancel a download. They are Steam's private
//! `SteamClient.Installs`, `.InstallFolder` and `.Downloads` objects, the ones its own Library page calls. Nothing of
//! Valve's documents them: what is known, and from where, is in docs/research/installs.md (section 2) and
//! docs/sources.md ("Steam's own calls"); every call is marked there as seen in others' code or as to be confirmed on
//! the TV PC.
//!
//! **The calls** ([`Call::calls`] names them, as the log does):
//!
//! - Install: `InstallFolder.GetInstallFolders()` (the libraries, to find the chosen one's `nFolderIndex`, or the
//!   one Steam marks `bIsDefaultFolder`), `Installs.GetInstallManagerInfo()`, `Installs.SetAppList([appid])`, then
//!   `GetInstallManagerInfo()` again until it changes, `Installs.OpenInstallWizard([appid])` and again until it
//!   changes, `Installs.SetInstallFolder(index)`, `Installs.SetCreateShortcuts(false, false)`,
//!   `Installs.ContinueInstall()`. The waits are a few seconds at most each and say in the log what came back.
//! - Uninstall: `Installs.OpenUninstallWizard([appid], true)`, the `true` passing over Steam's own question.
//! - Pause: `Downloads.PauseAppUpdate(appid, "0")`; resume: `Downloads.ResumeAppUpdate(appid, "0")` then
//!   `Downloads.EnableAllDownloads(true, "0")`; cancel: pause, `Downloads.RemoveFromDownloadList(appid, "0")`, then
//!   uninstall, which deletes what was fetched. `"0"` is the client's id for this PC.
//!
//! **What comes back.** Each script runs every call in turn, catching what each throws, and gives back as JSON
//! whether it got to the end and, for each call, what it returned or threw ([`Answer`]). Every step is a line in the
//! log (`steam client: pause 620: SteamClient.Downloads.PauseAppUpdate(620, "0") -> undefined`), so one run on the
//! real client says which calls are there and what they give. A call that is missing throws (`TypeError: ... is not
//! a function`), and the script stops there.
//!
//! **The stand-in.** Each script starts with a comment that says, as JSON, what it asks ([`Call::of_script`]). Steam
//! passes over a comment; the stand-in Steam's debugger (`crate::debugger`) reads it and does to the fake's files what
//! Steam would, so that the end-to-end tests take this same route.
use model::Steer;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::cef::{self, Evaluated, Failed};

/// How long a call's answer is waited for: the install's waits for Steam take ten seconds at most.
pub const WAIT: Duration = Duration::from_secs(25);

/// What the first line of every script starts with, before the call as JSON.
const HEADER: &str = "/* xbox-series-dashboard ";

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

    /// The calls that must go through for it to be done, in order, as the log names them. Install has its waits
    /// and its library question besides, which the log names as they happen.
    pub fn calls(&self) -> Vec<String> {
        let appid = self.appid();
        let uninstall = format!("SteamClient.Installs.OpenUninstallWizard([{appid}], true)");
        let pause = format!("SteamClient.Downloads.PauseAppUpdate({appid}, \"0\")");
        match self {
            Call::Install { .. } => vec![
                "SteamClient.InstallFolder.GetInstallFolders()".to_string(),
                format!("SteamClient.Installs.SetAppList([{appid}])"),
                format!("SteamClient.Installs.OpenInstallWizard([{appid}])"),
                "SteamClient.Installs.SetInstallFolder(index)".to_string(),
                "SteamClient.Installs.SetCreateShortcuts(false, false)".to_string(),
                "SteamClient.Installs.ContinueInstall()".to_string(),
            ],
            Call::Uninstall { .. } => vec![uninstall],
            Call::Steer { steer: Steer::Pause, .. } => vec![pause],
            Call::Steer { steer: Steer::Resume, .. } => vec![
                format!("SteamClient.Downloads.ResumeAppUpdate({appid}, \"0\")"),
                "SteamClient.Downloads.EnableAllDownloads(true, \"0\")".to_string(),
            ],
            Call::Steer { steer: Steer::Cancel, .. } => {
                vec![pause, format!("SteamClient.Downloads.RemoveFromDownloadList({appid}, \"0\")"), uninstall]
            }
        }
    }

    /// The script that makes the calls in the client's windowless page and gives back an [`Answer`] as JSON.
    pub fn script(&self) -> String {
        let header = serde_json::to_string(self).unwrap_or_default();
        let appid = self.appid();
        let body = match self {
            Call::Install { folder, .. } => {
                let want = folder.as_deref().map_or("null".to_string(), |folder| {
                    serde_json::to_string(folder).unwrap_or_else(|_| "null".to_string())
                });
                format!(
                    r#"const appid = {appid}, want = {want};
    const norm = p => String(p || '').replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
    const folders = await call('SteamClient.InstallFolder.GetInstallFolders()', () => SteamClient.InstallFolder.GetInstallFolders());
    const list = Array.isArray(folders) ? folders : [];
    const chosen = want === null ? list.find(f => f && f.bIsDefaultFolder) : list.find(f => f && norm(f.strFolderPath) === norm(want));
    if (want !== null && !chosen) fail('choose the library', 'Steam has no library at ' + want);
    note('choose the library', true, chosen ? 'index ' + chosen.nFolderIndex + ', ' + chosen.strFolderPath : 'none marked default: left to Steam');
    const info = () => SteamClient.Installs.GetInstallManagerInfo();
    let before = await call('SteamClient.Installs.GetInstallManagerInfo()', info, false);
    const changed = async name => {{
        let now = before;
        for (let i = 0; i < 20 && said(now) === said(before); i++) {{ await sleep(250); try {{ now = await info(); }} catch (e) {{ break; }} }}
        note(name, true, said(now));
        before = now;
    }};
    await call('SteamClient.Installs.SetAppList([{appid}])', () => SteamClient.Installs.SetAppList([appid]));
    await changed('SteamClient.Installs.GetInstallManagerInfo() once changed');
    await call('SteamClient.Installs.OpenInstallWizard([{appid}])', () => SteamClient.Installs.OpenInstallWizard([appid]));
    await changed('SteamClient.Installs.GetInstallManagerInfo() once changed');
    if (chosen) await call('SteamClient.Installs.SetInstallFolder(index)', () => SteamClient.Installs.SetInstallFolder(chosen.nFolderIndex));
    else note('SteamClient.Installs.SetInstallFolder(index)', true, 'not called: Steam keeps its own');
    await call('SteamClient.Installs.SetCreateShortcuts(false, false)', () => SteamClient.Installs.SetCreateShortcuts(false, false));
    await call('SteamClient.Installs.ContinueInstall()', () => SteamClient.Installs.ContinueInstall());"#
                )
            }
            Call::Uninstall { .. } => format!(
                "await call('SteamClient.Installs.OpenUninstallWizard([{appid}], true)', () => \
                 SteamClient.Installs.OpenUninstallWizard([{appid}], true));"
            ),
            Call::Steer { steer: Steer::Pause, .. } => format!(
                "await call('SteamClient.Downloads.PauseAppUpdate({appid}, \"0\")', () => \
                 SteamClient.Downloads.PauseAppUpdate({appid}, '0'));"
            ),
            Call::Steer { steer: Steer::Resume, .. } => format!(
                "await call('SteamClient.Downloads.ResumeAppUpdate({appid}, \"0\")', () => \
                 SteamClient.Downloads.ResumeAppUpdate({appid}, '0'));\n    \
                 await call('SteamClient.Downloads.EnableAllDownloads(true, \"0\")', () => \
                 SteamClient.Downloads.EnableAllDownloads(true, '0'));"
            ),
            Call::Steer { steer: Steer::Cancel, .. } => format!(
                "await call('SteamClient.Downloads.PauseAppUpdate({appid}, \"0\")', () => \
                 SteamClient.Downloads.PauseAppUpdate({appid}, '0'));\n    \
                 await call('SteamClient.Downloads.RemoveFromDownloadList({appid}, \"0\")', () => \
                 SteamClient.Downloads.RemoveFromDownloadList({appid}, '0'));\n    \
                 await call('SteamClient.Installs.OpenUninstallWizard([{appid}], true)', () => \
                 SteamClient.Installs.OpenUninstallWizard([{appid}], true));"
            ),
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
    /// Every call went through.
    Took,
    /// Not done, and safe to ask another way: the debugger was not reached, or a call was missing or failed.
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
/// log what was asked, each call and what it came to, and the outcome. Blocks: run it on a thread of its own.
pub fn ask(ports: &[u16], call: &Call) -> Outcome {
    let what = call.what();
    let list: Vec<String> = ports.iter().map(u16::to_string).collect();
    log(&format!("{what}: asking Steam's client over its CEF debugger on 127.0.0.1:{}", list.join("/")));
    let outcome = match cef::evaluate_within(ports, cef::SHARED, &call.script(), WAIT) {
        Ok(Evaluated::Value(text)) => match serde_json::from_str::<Answer>(&text) {
            Ok(answer) => {
                for step in &answer.steps {
                    let arrow = if step.ok { "->" } else { "threw" };
                    log(&format!("{what}: {} {arrow} {}", step.call, step.said));
                }
                match answer.steps.iter().find(|step| !step.ok) {
                    _ if answer.done => Outcome::Took,
                    Some(step) => Outcome::NotTaken(format!("{} threw {}", step.call, step.said)),
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
    match &outcome {
        Outcome::Took => log(&format!("{what}: Steam's client took it")),
        Outcome::NotTaken(why) => log(&format!("{what}: not taken: {why}")),
        Outcome::Lost(why) => log(&format!("{what}: lost after sending ({why}); not asked again")),
    }
    outcome
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
        let outcome = ask(&[port], &Call::Steer { appid: 70, steer: Steer::Pause });
        assert!(matches!(outcome, Outcome::NotTaken(why) if why.contains("could not be reached")));
        assert!(matches!(ask(&[], &Call::Uninstall { appid: 70 }), Outcome::NotTaken(_)));
    }
}
