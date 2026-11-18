//! The voice steps (#292): `--voice-join`, `--call`, `--voice-incoming`, and what `--voice-probe` and `--voice-list`
//! share with them. The signed-in session, hidden, granted the microphone; Discord's own buttons pressed by trusted
//! DevTools input, ONCE each (a second way is tried only when the first showed no sign at all); the call watched once
//! a second for `--hold` s; the lead's control files acted on; the call always left at the end.
//!
//! Never retried in a loop: a failure is logged once and the step exits non-zero. Every wait is capped. The log gets
//! counts, states, booleans, durations and Discord's own status words from a fixed list: never a name, an id or a
//! message. Names the lead must pick from go to `voice-names.txt` (`--voice-list`), never the log.
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, eRender, EDataFlow, IMMDeviceEnumerator, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId};

use super::host::{self, pump, yes, Host, Loader, Show};
use super::webaudio::WebAudio;
use super::{meter, procs};
use crate::args::{Options, Step, Way};
use crate::say;
use std::cell::RefCell;

const HOOKS: &str = include_str!("../js/voice_hooks.js");
const SHAPE: &str = include_str!("../js/shape.js");

thread_local! {
    /// The DevTools WebAudio follower of the step's web view.
    static AUDIO: RefCell<Option<WebAudio>> = const { RefCell::new(None) };
}

/// What else runs Discord's page: DevTools' targets by type (frames, workers), counted.
pub fn targets(host: &Host, when: &str) {
    match host.cdp("Target.getTargets", json!({})) {
        Ok(v) => {
            let mut kinds: std::collections::BTreeMap<String, u32> = Default::default();
            for t in v["targetInfos"].as_array().into_iter().flatten() {
                *kinds.entry(t["type"].as_str().unwrap_or("?").to_string()).or_default() += 1;
            }
            let line: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} {n}")).collect();
            say!("targets ({when}): {}", if line.is_empty() { "none listed".into() } else { line.join(", ") });
        }
        Err(e) => say!("targets ({when}): Target.getTargets refused: {e}"),
    }
}

/// The page's shape (`js/shape.js`) written to `--shape`; the log says only that and how many lines.
pub fn dump_shape(host: &Host, options: &Options, when: &str) {
    match host.eval(SHAPE) {
        Ok(Value::String(text)) => {
            let text = format!("# discord-spike shape, {when}. No names; check before sharing.\n{text}");
            match std::fs::write(&options.shape, &text) {
                Ok(()) => say!("shape ({when}): {} lines written to {}", text.lines().count(), options.shape.display()),
                Err(e) => say!("shape ({when}): not written to {}: {e}", options.shape.display()),
            }
        }
        Ok(_) => say!("shape ({when}): the script returned no text"),
        Err(e) => say!("shape ({when}): the script failed: {e}"),
    }
}

/// The once-a-second extras that do not depend on the page's scripts: DevTools WebAudio, and Windows' meters for
/// the web view's processes.
fn outside(host: &Host, last_clock: &mut f64) -> String {
    let audio = AUDIO.with(|a| a.borrow().as_ref().map(|w| w.read(host)));
    let pids: Vec<u32> = procs::webview(&host.env).iter().map(|u| u.pid).collect();
    let windows = meter::read(&pids).line();
    match audio {
        Some(a) => {
            let line = format!(
                "webaudio (DevTools): realtime contexts {} (running {}, made so far {}), clocks +{:.2} s, render capacity {:.2}; {windows}",
                a.count,
                a.running,
                a.created,
                a.clock - *last_clock,
                a.capacity
            );
            *last_clock = a.clock;
            line
        }
        None => windows,
    }
}
const SAMPLE: &str = include_str!("../js/voice_sample.js");
const MARK: &str = include_str!("../js/mark.js");
pub const SERVERS: &str = include_str!("../js/servers.js");
const CHANNELS: &str = include_str!("../js/channels.js");

/// One step of `js/channels.js`.
fn channels_op(host: &Host, op: &str, target: Option<&str>) -> Result<Value, String> {
    host.evaluate(&with2(("OP", &json!(op)), ("TARGET", &json!(target)), CHANNELS))
}

/// The open server's channel list read whole, a page at a time: a trusted mouse wheel over the list's centre, then
/// (if the list did not move) its scrollTop, each page waited for and logged; it stops at the end
/// (scrollTop + clientHeight >= scrollHeight), when the list moves no further, or after 60 pages. A hidden controller
/// may not render the virtual list's new rows: if a page moved but showed nothing new, the controller is told it is
/// visible for the rest (the window stays unshown) and told back after. With `until`, it stops once that channel's
/// row is shown (to bring it into view before a press). Collapsed categories are left as they are.
pub fn read_channels(host: &Host, target: Option<&str>, until: Option<&str>) -> Result<Value, String> {
    let shown = |cid: &str| {
        host.eval(&with("CID", &json!(cid), "!!document.querySelector(`[data-list-item-id=\"channels___${CID}\"]`)"))
            .map(|v| v == true)
            .unwrap_or(false)
    };
    let page_line = |n: u32, m: &Value, how: &str| {
        say!(
            "channels page {n}{how}: scrollTop {} + clientHeight {} of scrollHeight {} (at the end {}); rows seen {}, shown now {}; collapsed categories {}; scroller {} ({})",
            m["top"],
            m["client"],
            m["height"],
            yes(m["atEnd"] == true),
            m["seen"],
            m["shown"],
            m["collapsed"],
            yes(m["scroller"] == true),
            m["scrollerClass"].as_str().unwrap_or("")
        )
    };
    let mut m = channels_op(host, "start", target)?;
    page_line(1, &m, "");
    let was_visible = host.is_visible();
    let mut made_visible = false;
    for page in 2..=60 {
        if until.is_some_and(shown) || m["atEnd"] == true || m["scroller"] != true {
            break;
        }
        let (top, seen) = (m["top"].as_i64().unwrap_or(0), m["seen"].as_u64().unwrap_or(0));
        let delta = (m["client"].as_f64().unwrap_or(400.0) * 0.8).max(100.0);
        let (x, y) = (m["x"].as_f64().unwrap_or(0.0), m["y"].as_f64().unwrap_or(0.0));
        let _ = host.cdp("Input.dispatchMouseEvent", json!({ "type": "mouseMoved", "x": x, "y": y }));
        let _ = host.cdp(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseWheel", "x": x, "y": y, "deltaX": 0, "deltaY": delta }),
        );
        pump(Duration::from_millis(500));
        m = channels_op(host, "look", target)?;
        let mut how = " (wheel)";
        if m["top"].as_i64().unwrap_or(0) == top {
            channels_op(host, "nudge", target)?;
            pump(Duration::from_millis(500));
            m = channels_op(host, "look", target)?;
            how = " (the wheel did not move it: scrollTop set)";
        }
        let moved = m["top"].as_i64().unwrap_or(0) != top;
        if moved && m["seen"].as_u64().unwrap_or(0) == seen {
            // Rows render a moment after the scroll; then, if still nothing, the controller told it is visible.
            pump(Duration::from_millis(800));
            m = channels_op(host, "look", target)?;
            if m["seen"].as_u64().unwrap_or(0) == seen && !host.is_visible() && !made_visible {
                made_visible = host.set_visible(true);
                say!(
                    "channels: the list moved but showed no new rows: the controller told it is visible ({})",
                    yes(made_visible)
                );
                pump(Duration::from_millis(800));
                m = channels_op(host, "look", target)?;
                how = " (after the controller was told it is visible)";
            }
        }
        page_line(page, &m, how);
        if !moved {
            say!("channels: the list does not scroll further");
            break;
        }
    }
    if made_visible && !was_visible {
        host.set_visible(false);
        say!("channels: the controller told it is hidden again");
    }
    channels_op(host, "result", target)
}
const FIND: &str = include_str!("../js/find.js");
/// The page's router follows the history: a new entry, and the event it listens for (#196's lesson).
pub const PUSH: &str =
    "history.pushState(history.state, '', HREF); dispatchEvent(new PopStateEvent('popstate', { state: history.state })); true";

/// How long the page has to show the voice panel (connected) after the press.
const CONNECT: Duration = Duration::from_secs(20);
/// How long one way of pressing has to show any sign before the next way is tried.
const SIGN: Duration = Duration::from_secs(5);
/// How long Disconnect has to take the panel away.
const LEAVE: Duration = Duration::from_secs(10);

/// A script given one value, in a block of its own (the name can be declared again by the next run).
pub fn with(name: &str, value: &Value, script: &str) -> String {
    format!("{{ const {name} = {value};\n{script}\n}}")
}

fn with2(a: (&str, &Value), b: (&str, &Value), script: &str) -> String {
    format!("{{ const {} = {}; const {} = {};\n{script}\n}}", a.0, a.1, b.0, b.1)
}

/// One line from a file the lead wrote (a name), BOM and spaces trimmed.
pub fn read_name(path: Option<&Path>, what: &str) -> Result<String, String> {
    let path = path.ok_or_else(|| format!("no {what} file given"))?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("the {what} file {}: {e}", path.display()))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text).trim().to_string();
    if text.is_empty() || text.contains('\n') {
        return Err(format!("the {what} file {} must hold one name on one line", path.display()));
    }
    Ok(text)
}

/// The signed-in session, hidden as `--hide` says, the microphone granted, the voice hooks in place before
/// Discord's scripts, the app shown, the page told it has focus. `Err` is the step's exit code.
pub fn open(options: &Options, loader: &Loader) -> Result<Host, i32> {
    host::grant_microphone();
    say!(
        "voice: hidden as {}, browser switches set {}{}",
        options.hide.name(),
        if options.args_set_b { "b" } else { "a" },
        if options.browser_args.is_some() { " (and --browser-args)" } else { "" }
    );
    endpoints();
    let host = match Host::open(loader, &options.data, Show::Hidden(options.hide), options.switches().as_deref()) {
        Ok(host) => host,
        Err(e) => {
            say!("voice: no web view: {e}");
            return Err(8);
        }
    };
    // WebView2's own document-created script (the first TV run's DevTools Page.addScriptToEvaluateOnNewDocument
    // answered ok but never ran in Discord's page: the timer read 0 ticks).
    // The page's AudioContexts through DevTools, whatever the page's own scripts do.
    AUDIO.with(|a| *a.borrow_mut() = Some(WebAudio::start(&host)));
    match host.add_script(HOOKS) {
        Ok(()) => say!("voice: the hooks are set to run at the page's start (AddScriptToExecuteOnDocumentCreated)"),
        Err(e) => say!("voice: the hooks could not be set to run at the start ({e})"),
    }
    host.navigate("https://discord.com/channels/@me");
    if let Err(code) = host.wait_for_app(Duration::from_secs(90)) {
        host.close();
        return Err(code);
    }
    // Are they in the page? If not, put them in now: only what Discord makes from here on is then seen.
    let check = "(() => { const R = window.__xsdVoice; return R ? { in: true, ticks: R.ticks, pcs: R.pcs.length, contexts: R.contexts.length } : { in: false }; })()";
    let found = host.eval(check).unwrap_or(Value::Null);
    if found["in"] == true {
        say!(
            "voice: the hooks are in the page from its start: timer ticks so far {}, peer connections {}, audio contexts {}",
            found["ticks"],
            found["pcs"],
            found["contexts"]
        );
    } else {
        let late = host.eval(HOOKS).and_then(|_| host.eval(check)).unwrap_or(Value::Null);
        say!(
            "voice: the hooks were NOT in the page at its start; put in now: in {} (only what Discord makes from now is seen)",
            yes(late["in"] == true)
        );
    }
    match host.cdp("Emulation.setFocusEmulationEnabled", json!({ "enabled": true })) {
        Ok(_) => say!("voice: focus emulation on"),
        Err(e) => say!("voice: focus emulation refused: {e}"),
    }
    host.focus();
    if let Ok(v) =
        host.eval("({ visibility: document.visibilityState, hidden: document.hidden, focus: document.hasFocus() })")
    {
        say!(
            "voice: the page says visibility {}, document.hidden {}, focus {}",
            v["visibility"].as_str().unwrap_or("?"),
            v["hidden"],
            v["focus"]
        );
    }
    // The lists fill in after the app shows.
    pump(Duration::from_secs(3));
    Ok(host)
}

/// Windows' audio endpoints, counted: active outputs and inputs, and whether each has a default (console and
/// communications roles). Never a name.
pub fn endpoints() {
    let found = unsafe {
        CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL).map(|e| {
            let count = |flow: EDataFlow| {
                e.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE).and_then(|c| c.GetCount()).map_or(-1, i64::from)
            };
            let default = |flow, role| e.GetDefaultAudioEndpoint(flow, role).is_ok();
            (
                count(eRender),
                count(eCapture),
                default(eRender, eConsole),
                default(eRender, eCommunications),
                default(eCapture, eConsole),
                default(eCapture, eCommunications),
            )
        })
    };
    match found {
        Ok((outs, ins, out_default, out_comms, in_default, in_comms)) => say!(
            "devices (Windows): {outs} active outputs (a default {}, a communications default {}), {ins} active inputs (a default {}, a communications default {})",
            yes(out_default),
            yes(out_comms),
            yes(in_default),
            yes(in_comms)
        ),
        Err(e) => say!("devices (Windows): the endpoints could not be listed: {e}"),
    }
}

/// What the page shows about the call now (`js/voice_sample.js`).
pub fn sample(host: &Host, cid: Option<&str>) -> Value {
    host.evaluate(&with("CID", &json!(cid), SAMPLE)).unwrap_or_else(|e| json!({ "error": e }))
}

fn in_call(s: &Value) -> bool {
    s["panel"] == true || s["disconnect"] == true
}

/// One of Discord's buttons found and marked (`js/mark.js`).
fn mark(host: &Host, what: &str, cid: Option<&str>) -> Value {
    host.eval(&with2(("WHAT", &json!(what)), ("CID", &json!(cid)), MARK)).unwrap_or(Value::Null)
}

/// One `Input.dispatchKeyEvent`, waited for.
fn key(host: &Host, kind: &str, text: Option<&str>) {
    let mut event = json!({
        "type": kind, "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13,
    });
    if let Some(text) = text {
        event["text"] = json!(text);
        event["unmodifiedText"] = json!(text);
    }
    if let Err(e) = host.cdp("Input.dispatchKeyEvent", event) {
        say!("press: Enter {kind}: {e}");
    }
}

/// DevTools' own focus on the marked element.
fn focus_marked(host: &Host) -> Result<(), String> {
    let root = host.cdp("DOM.getDocument", json!({ "depth": 0 }))?;
    let root = root.pointer("/root/nodeId").and_then(Value::as_i64).ok_or("no document node")?;
    let node = host.cdp("DOM.querySelector", json!({ "nodeId": root, "selector": "[data-xsd-target]" }))?;
    let node = node["nodeId"].as_i64().filter(|&n| n != 0).ok_or("no marked node")?;
    host.cdp("DOM.focus", json!({ "nodeId": node })).map(|_| ())
}

/// Presses `what` once, by `way` (key or click). Whether anything was pressed.
fn press_once(host: &Host, what: &str, cid: Option<&str>, way: Way) -> bool {
    let m = mark(host, what, cid);
    if m["found"] != true {
        say!("press: {what}: not found on the page{}", if m["ring"] == true { " (a ring is shown)" } else { "" });
        return false;
    }
    say!(
        "press: {what} by {}: {} {}x{} css px, covered by something else {}, aria-checked before {}",
        way.name(),
        m["tag"].as_str().unwrap_or("?"),
        m["w"],
        m["h"],
        yes(m["covered"] == true),
        m["checked"].as_str().unwrap_or("(none)")
    );
    match way {
        Way::Click => host.click(m["x"].as_f64().unwrap_or(0.0), m["y"].as_f64().unwrap_or(0.0)),
        _ => {
            if let Err(e) = focus_marked(host) {
                say!("press: {what}: DOM.focus: {e}: not pressed");
                return false;
            }
            key(host, "rawKeyDown", None);
            key(host, "char", Some("\r"));
            key(host, "keyUp", None);
        }
    }
    true
}

/// Waits (capped) for `effect` on the sample; the sample it ended on and whether it came.
fn wait_for(host: &Host, cid: Option<&str>, cap: Duration, effect: &dyn Fn(&Value) -> bool) -> (bool, Value, u128) {
    let started = Instant::now();
    let mut last = Value::Null;
    while started.elapsed() < cap {
        pump(Duration::from_millis(500));
        last = sample(host, cid);
        if effect(&last) {
            return (true, last, host::ms(started));
        }
    }
    (false, last, host::ms(started))
}

/// Presses `what` by `--way`: auto tries key, then click, the second ONLY if the first showed no sign within
/// [`SIGN`]. Whether the effect was seen.
fn act(host: &Host, what: &str, cid: Option<&str>, way: Way, effect: &dyn Fn(&Value) -> bool) -> bool {
    let ways: &[Way] = match way {
        Way::Auto => &[Way::Key, Way::Click],
        Way::Click => &[Way::Click],
        _ => &[Way::Key],
    };
    for &w in ways {
        if !press_once(host, what, cid, w) {
            return false;
        }
        let (seen, _, ms) = wait_for(host, cid, SIGN, effect);
        say!("press: {what} by {}: its effect seen {} after {ms} ms", w.name(), yes(seen));
        if seen {
            return true;
        }
    }
    false
}

fn switch_state(s: &Value, name: &str) -> String {
    let sw = &s[name];
    if sw["present"] != true {
        return "absent".into();
    }
    format!(
        "{} (aria-checked {}, in the {})",
        sw["label"].as_str().unwrap_or("?"),
        sw["checked"].as_str().unwrap_or("none"),
        sw["where"].as_str().unwrap_or("?")
    )
}

/// Mute or deafen: Discord's own switch pressed once, its state before and after.
fn toggle(host: &Host, cid: Option<&str>, way: Way, name: &'static str) {
    let before = sample(host, cid);
    let was = (before[name]["checked"].clone(), before[name]["label"].clone());
    say!("{name}: before: {}", switch_state(&before, name));
    let changed = move |s: &Value| (s[name]["checked"].clone(), s[name]["label"].clone()) != was;
    let seen = act(host, name, cid, way, &changed);
    let after = sample(host, cid);
    say!("{name}: after: {} (changed {})", switch_state(&after, name), yes(seen));
}

/// Leaves the call: Discord's Disconnect pressed, the panel seen gone. False (said loudly) if it cannot confirm.
pub fn leave(host: &Host, cid: Option<&str>, way: Way) -> bool {
    let now = sample(host, cid);
    if !in_call(&now) {
        say!("leave: not in a call (no voice panel, no Disconnect button)");
        return true;
    }
    let gone = |s: &Value| !in_call(s);
    act(host, "disconnect", cid, way, &gone);
    let (gone_seen, last, ms) = wait_for(host, cid, LEAVE, &gone);
    if gone_seen {
        say!(
            "leave: the voice panel is gone after {ms} ms; peer connections open {}, connected {}",
            last["audio"]["pcs"],
            last["audio"]["pcsConnected"]
        );
        true
    } else {
        say!(
            "LEAVE NOT CONFIRMED: the voice panel is still there {ms} ms after Disconnect (status class {}): the account may still be in the call: disconnect in Discord itself (closing this web view drops it in time)",
            last["statusClass"].as_str().unwrap_or("?")
        );
        false
    }
}

/// The control files beside the exe: an old one is removed at the start, so only the lead's new ones act.
pub fn clear_controls(dir: &Path) {
    for name in ["mute", "deafen", "leave", "answer", "decline", "shape"] {
        let path = dir.join(name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
            say!("control: an old {name} file was removed");
        }
    }
}

fn take_control(dir: &Path, name: &str) -> bool {
    let path = dir.join(name);
    if path.exists() {
        let _ = std::fs::remove_file(&path);
        say!("control: {name} file seen (removed)");
        true
    } else {
        false
    }
}

/// The window in front: ours, the web view's, or another program's, and whether it covers its screen (a game).
pub fn foreground() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return "none".into();
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let full = GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info).as_bool()
            && rect.left <= info.rcMonitor.left
            && rect.top <= info.rcMonitor.top
            && rect.right >= info.rcMonitor.right
            && rect.bottom >= info.rcMonitor.bottom;
        let whose = if pid == std::process::id() { "this spike's" } else { "another program's" };
        format!("{whose} window, full screen {}", yes(full))
    }
}

/// Every 10 s: the web view's processor time over the last span and its memory; whether Chromium says the page
/// plays sound; what is in front.
pub struct Costs {
    cpu: u64,
    at: Instant,
}

impl Costs {
    pub fn start(host: &Host) -> Costs {
        Costs { cpu: procs::cpu(&host.env), at: Instant::now() }
    }

    pub fn report(&mut self, host: &Host, label: &str) {
        let list = procs::webview(&host.env);
        let cpu: u64 = list.iter().map(|u| u.cpu).sum();
        let private: u64 = list.iter().map(|u| u.private).sum();
        let ws: u64 = list.iter().map(|u| u.working_set).sum();
        let busy = cpu.saturating_sub(self.cpu) as f64 / 10_000_000.0;
        let wall = self.at.elapsed().as_secs_f64().max(0.001);
        say!(
            "cost {label}: web view {} processes, {:.1}% of one core over {:.0} s, private {:.0} MB, working set {:.0} MB; playing audio (Chromium) {}; in front: {}",
            list.len(),
            100.0 * busy / wall,
            wall,
            private as f64 / 1_048_576.0,
            ws as f64 / 1_048_576.0,
            host.playing_audio().map_or("unknown", yes),
            foreground()
        );
        (self.cpu, self.at) = (cpu, Instant::now());
    }
}

/// The deltas between two samples, for the once-a-second line.
struct Last {
    in_bytes: f64,
    out_bytes: f64,
    energy: f64,
    clock: f64,
    media: f64,
}

impl Last {
    fn of(s: &Value) -> Last {
        let f = |v: &Value| v.as_f64().unwrap_or(0.0);
        Last {
            in_bytes: f(&s["audio"]["inBytes"]),
            out_bytes: f(&s["audio"]["outBytes"]),
            energy: f(&s["audio"]["inEnergy"]),
            clock: f(&s["contextTime"]),
            media: f(&s["mediaTime"]),
        }
    }
}

/// The once-a-second line: the call, who is in it and speaking, and whether audio flows.
fn line(t: u64, s: &Value, last: &Last) -> String {
    if let Some(e) = s["error"].as_str() {
        return format!("hold {t} s: the sample failed: {e}");
    }
    let now = Last::of(s);
    let a = &s["audio"];
    let participants = s["voiceUsers"].as_u64().unwrap_or(0).max(s["tiles"].as_u64().unwrap_or(0));
    let speaking = s["speakingNames"]
        .as_u64()
        .unwrap_or(0)
        .max(s["speakingAvatars"].as_u64().unwrap_or(0))
        .max(s["speakingTiles"].as_u64().unwrap_or(0));
    format!(
        "hold {t} s{}: rtc {} \"{}\", disconnect {}; participants {participants} (channel users {}, page users {}, call tiles {}), speaking {speaking} (names {}, avatars {}, tiles {}); audio in +{:.1} kB over {} streams, level {:.3}, energy +{:.4}; out +{:.1} kB, mic level {:.3}; peer connections {} (connected {}); mic tracks {} (enabled {}, muted {}); audio contexts {} (running {}), clock +{:.2} s; media elements {} (with a stream {}, playing {}), time +{:.2} s; timer ticks {} (max gap {} ms); page {} focus {}",
        if s["hooks"] == true { "" } else { " [HOOKS MISSING: audio, contexts and timer read as none]" },
        s["statusClass"].as_str().unwrap_or("?"),
        s["status"].as_str().unwrap_or(""),
        yes(s["disconnect"] == true),
        s["voiceUsers"],
        s["voiceUsersPage"],
        s["tiles"],
        s["speakingNames"],
        s["speakingAvatars"],
        s["speakingTiles"],
        (now.in_bytes - last.in_bytes) / 1000.0,
        a["inStreams"],
        a["inLevel"].as_f64().unwrap_or(0.0),
        now.energy - last.energy,
        (now.out_bytes - last.out_bytes) / 1000.0,
        a["micLevel"].as_f64().unwrap_or(0.0),
        a["pcs"],
        a["pcsConnected"],
        s["mics"],
        s["micEnabled"],
        s["micMuted"],
        s["contexts"],
        s["contextsRunning"],
        now.clock - last.clock,
        s["media"],
        s["mediaWithStream"],
        s["mediaPlaying"],
        now.media - last.media,
        s["timer"]["ticks"],
        s["timer"]["maxGap"],
        s["visibility"].as_str().unwrap_or("?"),
        s["focus"]
    )
}

/// How a hold ended.
#[derive(PartialEq)]
enum Held {
    Time,
    LeaveFile,
}

/// Holds for `--hold` s: the line once a second (while in a call), costs every 10 s, the control files acted on.
/// `incoming`: the ring is watched, and answer / decline act on it.
fn hold(host: &Host, options: &Options, cid: Option<&str>, incoming: bool) -> Held {
    let mut costs = Costs::start(host);
    let mut last = Last::of(&sample(host, cid));
    let mut ring = false;
    let started = Instant::now();
    let mut second = 0;
    let mut clock = 0.0;
    let _ = outside(host, &mut clock);
    targets(host, "the hold's start");
    say!(
        "hold: {} s; control files in {}: mute, deafen, leave, shape{}",
        options.hold,
        options.control.display(),
        if incoming { ", answer, decline" } else { "" }
    );
    while started.elapsed() < Duration::from_secs(options.hold) {
        second += 1;
        let next = Duration::from_secs(second);
        let wait = next.saturating_sub(started.elapsed());
        pump(wait);
        let s = sample(host, cid);
        if !incoming || in_call(&s) {
            say!("{}", line(second, &s, &last));
            say!("hold {second} s, outside the page: {}", outside(host, &mut clock));
        }
        if take_control(&options.control, "shape") {
            dump_shape(host, options, &format!("at {second} s of the hold"));
        }
        if incoming && (s["ring"] == true) != ring {
            ring = s["ring"] == true;
            say!(
                "ring: {} at {second} s (accept buttons {}, decline buttons {}); notifications so far {}",
                if ring { "shown" } else { "gone" },
                s["ringAccept"],
                s["ringDecline"],
                host::notifications()
            );
        }
        last = Last::of(&s);
        if second % 10 == 0 {
            costs.report(host, &format!("at {second} s"));
        }
        if take_control(&options.control, "mute") {
            toggle(host, cid, options.way, "mute");
        }
        if take_control(&options.control, "deafen") {
            toggle(host, cid, options.way, "deafen");
        }
        if incoming && take_control(&options.control, "answer") {
            let joined = act(host, "accept", cid, options.way, &in_call);
            say!("answer: the voice panel shows {}", yes(joined));
        }
        if incoming && take_control(&options.control, "decline") {
            let gone = act(host, "decline", cid, options.way, &|s: &Value| s["ring"] != true);
            say!("decline: the ring is gone {}", yes(gone));
        }
        if take_control(&options.control, "leave") {
            return Held::LeaveFile;
        }
    }
    Held::Time
}

/// The end of every voice step that may be in a call: leave, say so, close. The step's exit code.
fn finish(host: Host, options: &Options, cid: Option<&str>, code: i32) -> i32 {
    let left = leave(&host, cid, options.way);
    say!("voice: notifications taken this run: {}", host::notifications());
    host.close();
    if !left {
        return 13;
    }
    code
}

/// `--voice-join`: the server and its voice channel found from the lead's files, joined once, held, left.
pub fn join(options: &Options, loader: &Loader) -> i32 {
    let (server, channel) = match (
        read_name(options.server_file.as_deref(), "server"),
        read_name(options.channel_file.as_deref(), "channel"),
    ) {
        (Ok(s), Ok(c)) => (s, c),
        (Err(e), _) | (_, Err(e)) => {
            say!("voice-join: {e}: nothing joined");
            return 10;
        }
    };
    say!(
        "voice-join: a server named in {} characters, a channel in {}",
        server.chars().count(),
        channel.chars().count()
    );
    let host = match open(options, loader) {
        Ok(host) => host,
        Err(code) => return code,
    };
    clear_controls(&options.control);
    let Some(gid) = open_server(&host, &server) else {
        host.close();
        return 4;
    };
    let found = match read_channels(&host, Some(&channel), None) {
        Ok(v) => v,
        Err(e) => {
            say!("voice-join: reading the channels failed: {e}: nothing joined");
            host.close();
            return 7;
        }
    };
    log_channels(&found);
    write_channel_names(options, &found);
    if found["found"] != 1 || found["stage"] == true {
        say!(
            "voice-join: {} channels of that name ({} kind){}: nothing joined",
            found["found"],
            found["kind"].as_str().unwrap_or("?"),
            if found["stage"] == true { " (a stage channel: not tried)" } else { "" }
        );
        host.close();
        return 4;
    }
    let channel_id = found["cid"].as_str().unwrap_or_default().to_string();
    let cid = Some(channel_id.as_str());
    say!(
        "voice-join: the channel found (name matched {}, told as {}: {}); already in a call: {}",
        if found["exact"] == true { "exactly" } else { "ignoring case" },
        found["kind"].as_str().unwrap_or("?"),
        if found["kind"] == "voice" {
            "labelled voice or showing users"
        } else {
            "no voice sign found: the only row of that name; the press shows what it is"
        },
        yes(in_call(&sample(&host, cid)))
    );
    // The list was read to its end: its row may be out of the virtual list now. Paged to again from the top.
    match read_channels(&host, None, cid) {
        Ok(_) => {
            say!("voice-join: the channel's row shown again: {}", yes(mark(&host, "channel", cid)["found"] == true))
        }
        Err(e) => say!("voice-join: paging back to the channel failed: {e}"),
    }
    let pressed = Instant::now();
    // Each way only if the one before showed no voice panel at all: never the same press twice.
    let ways: &[Way] = match options.way {
        Way::Auto => &[Way::Key, Way::Click, Way::Route],
        Way::Key => &[Way::Key],
        Way::Click => &[Way::Click],
        Way::Route => &[Way::Route],
    };
    let mut sign = false;
    for &w in ways {
        if w == Way::Route {
            let href = format!("/channels/{gid}/{channel_id}");
            if let Err(e) = host.eval(&with("HREF", &json!(href), PUSH)) {
                say!("voice-join: route: the script failed: {e}");
                continue;
            }
            let (shown, _, ms) = wait_for(&host, cid, SIGN, &|_| mark(&host, "join_voice", None)["found"] == true);
            say!("voice-join: route: the channel's Join Voice button shown {} after {ms} ms", yes(shown));
            if !shown || !press_once(&host, "join_voice", None, Way::Key) {
                continue;
            }
        } else if !press_once(&host, "channel", cid, w) {
            break;
        }
        let (seen, s, ms) = wait_for(&host, cid, SIGN, &in_call);
        say!(
            "voice-join: by {}: a voice panel shown {} after {ms} ms (status class {})",
            w.name(),
            yes(seen),
            s["statusClass"].as_str().unwrap_or("?")
        );
        if seen {
            sign = true;
            break;
        }
    }
    if !sign {
        say!("voice-join: no way showed a voice panel: not joined");
        return finish(host, options, cid, 5);
    }
    let code = connected(&host, cid, pressed, options);
    host.close();
    code
}

/// After a join, a call or an answer: waits (capped) for "connected", says what showed, holds, leaves.
fn connected(host: &Host, cid: Option<&str>, pressed: Instant, options: &Options) -> i32 {
    // 20 s from the press, and never less than 5 s from now (a second way may have used most of it).
    let rest = CONNECT.saturating_sub(pressed.elapsed()).max(SIGN);
    let (ok, s, _) = wait_for(host, cid, rest, &|s| {
        s["statusClass"] == "connected" || s["audio"]["pcsConnected"].as_u64().unwrap_or(0) > 0
    });
    say!(
        "connected: {} {} ms after the press: panel {}, status class {} \"{}\", Disconnect {}, mute switch {}, deafen switch {}, peer connections connected {}",
        yes(ok),
        host::ms(pressed),
        yes(s["panel"] == true),
        s["statusClass"].as_str().unwrap_or("?"),
        s["status"].as_str().unwrap_or(""),
        yes(s["disconnect"] == true),
        switch_state(&s, "mute"),
        switch_state(&s, "deafen"),
        s["audio"]["pcsConnected"]
    );
    let held = hold(host, options, cid, false);
    if held == Held::LeaveFile {
        say!("hold: ended by the leave file");
    }
    let code = if ok { 0 } else { 6 };
    let left = leave(host, cid, options.way);
    say!("voice: notifications taken this run: {}", host::notifications());
    if !left {
        13
    } else {
        code
    }
}

/// The server named, found in the server bar and opened by its route. Its id, or None (logged).
pub fn open_server(host: &Host, server: &str) -> Option<String> {
    let found = match host.eval(&with("TARGET", &json!(server), SERVERS)) {
        Ok(v) => v,
        Err(e) => {
            say!("server: reading the server bar failed: {e}");
            return None;
        }
    };
    log_servers(&found);
    if found["found"] != 1 {
        say!("server: {} servers of that name: nothing opened", found["found"]);
        return None;
    }
    let gid = found["gid"].as_str().unwrap_or_default().to_string();
    let href = format!("/channels/{gid}");
    if let Err(e) = host.eval(&with("HREF", &json!(href), PUSH)) {
        say!("server: opening by route failed: {e}");
        return None;
    }
    let started = Instant::now();
    let mut shown = false;
    while started.elapsed() < Duration::from_secs(10) && !shown {
        pump(Duration::from_millis(250));
        shown = host
            .eval(&with("HREF", &json!(href), "location.pathname.startsWith(HREF) && !!document.querySelector('[data-list-item-id^=\"channels___\"]')"))
            .map(|v| v == true)
            .unwrap_or(false);
    }
    say!(
        "server: found (name matched {}), opened by history.pushState + popstate: its channel list shown {} after {} ms",
        if found["exact"] == true { "exactly" } else { "ignoring case" },
        yes(shown),
        host::ms(started)
    );
    shown.then_some(gid)
}

pub fn log_servers(v: &Value) {
    let c = &v["counts"];
    say!(
        "servers: selectors matched: guildsnav={} items={} servers={} name_from_dnd={} name_from_aria={} folders={}; DM rows on the first screen {}",
        c["guildsnav"],
        c["items_all"],
        c["servers"],
        c["name_dnd"],
        c["name_aria"],
        c["folders"],
        c["dm_rows"]
    );
}

pub fn log_channels(v: &Value) {
    let c = &v["counts"];
    say!(
        "channels: {} rows read: with any aria-label {}, links {}, voice by label {}, stage by label {}, with users {} ({} users), name_ class {}",
        c["rows"],
        c["labelled"],
        c["links"],
        c["voice_by_label"],
        c["stage_by_label"],
        c["with_users"],
        c["voice_users"],
        c["name_class"]
    );
}

/// The channel rows read, to `--voice-names` for the lead's eyes (never the log): name, users shown, voice or not,
/// a link or not.
pub fn channel_names(v: &Value) -> String {
    let mut text = String::from(
        "# that server's channel rows: name, users shown in it, told as voice (voice/-), a link (link/-)\n",
    );
    for c in v["rows"].as_array().into_iter().flatten() {
        text.push_str(&format!(
            "channel\t{}\t{}\t{}\t{}\n",
            c["name"].as_str().unwrap_or("").replace(['\t', '\n', '\r'], " "),
            c["users"],
            if c["voice"] == true { "voice" } else { "-" },
            if c["link"] == true { "link" } else { "-" }
        ));
    }
    text
}

fn write_channel_names(options: &Options, v: &Value) {
    let text = format!("# discord-spike --voice-join. PRIVATE: keep it out of the repository.\n{}", channel_names(v));
    match std::fs::write(&options.voice_names, text) {
        Ok(()) => say!("voice-join: the rows' names written to {} (not to this log)", options.voice_names.display()),
        Err(e) => say!("voice-join: the names could not be written to {}: {e}", options.voice_names.display()),
    }
}

/// `--dump-shape [--server-file f]`: the signed-in page (that server open, if given) written as its shape.
pub fn shape(options: &Options, loader: &Loader) -> i32 {
    let server = match options.server_file.as_deref() {
        Some(path) => match read_name(Some(path), "server") {
            Ok(name) => Some(name),
            Err(e) => {
                say!("dump-shape: {e}");
                return 10;
            }
        },
        None => None,
    };
    let host = match open(options, loader) {
        Ok(host) => host,
        Err(code) => return code,
    };
    targets(&host, "the app shown");
    let mut code = 0;
    if let Some(server) = server {
        if open_server(&host, &server).is_some() {
            // The list read to its end; then a voice row brought into view if one was told, else the end stays shown
            // (where a server's voice channels usually are).
            match read_channels(&host, None, None) {
                Ok(v) => {
                    log_channels(&v);
                    if let Some(id) = v["firstVoice"].as_str() {
                        let _ = read_channels(&host, None, Some(id));
                        say!(
                            "dump-shape: a voice row in view: {}",
                            yes(mark(&host, "channel", Some(id))["found"] == true)
                        );
                    } else {
                        say!("dump-shape: no row told as voice: the list's end is shown");
                    }
                }
                Err(e) => say!("dump-shape: reading the channels failed: {e}"),
            }
        } else {
            code = 4;
        }
    }
    dump_shape(&host, options, "--dump-shape");
    host.close();
    code
}

/// `--call`: the DM named, opened by route, its Start Voice Call button pressed once; held; left.
pub fn call(options: &Options, loader: &Loader) -> i32 {
    let to = match read_name(options.to_file.as_deref(), "conversation") {
        Ok(to) => to,
        Err(e) => {
            say!("call: {e}: nobody called");
            return 10;
        }
    };
    say!("call: a conversation named in {} characters", to.chars().count());
    let host = match open(options, loader) {
        Ok(host) => host,
        Err(code) => return code,
    };
    clear_controls(&options.control);
    let found = match host.evaluate(&with("TARGET", &json!(to), FIND)) {
        Ok(v) => v,
        Err(e) => {
            say!("call: finding the conversation failed: {e}: nobody called");
            host.close();
            return 7;
        }
    };
    if found["found"] != 1 {
        say!("call: {} conversations of that name among {} rows: nobody called", found["found"], found["rows"]);
        host.close();
        return 4;
    }
    let href = found["href"].as_str().unwrap_or_default().to_string();
    if let Err(e) = host.eval(&with("HREF", &json!(href), PUSH)) {
        say!("call: opening by route failed: {e}: nobody called");
        host.close();
        return 5;
    }
    let (shown, _, ms) =
        wait_for(&host, None, Duration::from_secs(10), &|_| mark(&host, "call", None)["found"] == true);
    say!(
        "call: the conversation found (name matched {}), opened by route; its Start Voice Call button shown {} after {ms} ms",
        if found["exact"] == true { "exactly" } else { "ignoring case" },
        yes(shown)
    );
    if !shown {
        host.close();
        return 5;
    }
    let pressed = Instant::now();
    if !act(&host, "call", None, options.way, &|s| in_call(s) || s["callView"] == true) {
        say!("call: no way showed a call: nobody called");
        return finish(host, options, None, 5);
    }
    let code = connected(&host, None, pressed, options);
    host.close();
    code
}

/// `--voice-incoming`: waits `--hold` s for a call to come in; answer and decline press its buttons; a call
/// answered is held to the end and left.
pub fn incoming(options: &Options, loader: &Loader) -> i32 {
    let host = match open(options, loader) {
        Ok(host) => host,
        Err(code) => return code,
    };
    clear_controls(&options.control);
    let s = sample(&host, None);
    say!(
        "incoming: waiting {} s; a ring shown now {}; already in a call {}",
        options.hold,
        yes(s["ring"] == true),
        yes(in_call(&s))
    );
    hold(&host, options, None, true);
    let end = sample(&host, None);
    if end["ring"] == true {
        say!("incoming: a ring is still shown at the end: left as it is (not declined)");
    }
    finish(host, options, None, 0)
}

/// The step's code from the step: the probe and the list live in their own files.
pub fn run(options: &Options, loader: &Loader) -> i32 {
    match options.step {
        Step::VoiceJoin => join(options, loader),
        Step::Call => call(options, loader),
        Step::VoiceIncoming => incoming(options, loader),
        Step::DumpShape => shape(options, loader),
        _ => unreachable!(),
    }
}
