//! `--send --to-file <f> --text-file <f>`: the conversation the user named, found in the DM list and opened by a
//! click; the text put in Discord's message box by DevTools' `Input.insertText` and sent with Enter
//! (`Input.dispatchKeyEvent`), ONCE, never retried. Run only with the two files the lead writes after the user named
//! the conversation and the words. The log says lengths and whether the message showed, never the name or text.
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::host::{self, pump, yes, Host, Key, Loader, Show};
use crate::args::Options;
use crate::say;

const FIND: &str = include_str!("../js/find.js");
const COMPOSER: &str = include_str!("../js/composer.js");
const SENT: &str = include_str!("../js/sent.js");
const VIEWPORT: &str = include_str!("../js/viewport.js");
/// The page's router follows the history: a new entry, and the event it listens for.
const PUSH: &str =
    "history.pushState(history.state, '', HREF); dispatchEvent(new PopStateEvent('popstate', { state: history.state })); true";
/// Discord's message box.
const BOX_JS: &str = r#"(document.querySelector('[role="textbox"][data-slate-editor="true"]') || document.querySelector('div[role="textbox"][contenteditable="true"]'))"#;
const BOX_SELECTOR: &str = r#"[role="textbox"][contenteditable="true"]"#;

#[derive(Clone, Copy)]
enum Open {
    Push,
    Navigate,
    Click,
}

impl Open {
    fn name(self) -> &'static str {
        match self {
            Open::Push => "history.pushState + popstate",
            Open::Navigate => "Navigate to the address",
            Open::Click => "a DevTools click on the row",
        }
    }
}

/// DevTools' own focus on the message box: the document, the box's node, `DOM.focus`.
fn dom_focus(host: &Host) -> Result<(), String> {
    let root = host.cdp("DOM.getDocument", json!({ "depth": 0 }))?;
    let root = root.pointer("/root/nodeId").and_then(Value::as_i64).ok_or("no document node")?;
    let node = host.cdp("DOM.querySelector", json!({ "nodeId": root, "selector": BOX_SELECTOR }))?;
    let node = node["nodeId"].as_i64().filter(|&n| n != 0).ok_or("no message box node")?;
    host.cdp("DOM.focus", json!({ "nodeId": node })).map(|_| ())
}

/// Discord's limit for one message without Nitro.
const LONGEST: usize = 2000;

/// A script given one value: in a block, so the name can be declared again by the next run in the same page.
fn with(name: &str, value: &Value, script: &str) -> String {
    format!("{{ const {name} = {value};\n{script}\n}}")
}

fn read(path: &std::path::Path, what: &str) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("the {what} file {}: {e}", path.display()))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text).trim().to_string();
    if text.is_empty() {
        return Err(format!("the {what} file {} is empty", path.display()));
    }
    Ok(text)
}

pub fn run(options: &Options, loader: &Loader) -> i32 {
    let (Some(to_file), Some(text_file)) = (&options.to_file, &options.text_file) else { return 10 };
    let (to, text) = match (read(to_file, "conversation"), read(text_file, "message")) {
        (Ok(to), Ok(text)) => (to, text),
        (Err(e), _) | (_, Err(e)) => {
            say!("send: {e}: nothing sent");
            return 10;
        }
    };
    if text.contains('\n') || text.chars().count() > LONGEST || to.contains('\n') {
        say!("send: the message must be one line of at most {LONGEST} characters, the conversation one name: nothing sent");
        return 10;
    }
    say!(
        "send: a conversation named in {} characters, a message of {} characters",
        to.chars().count(),
        text.chars().count()
    );

    let show = if options.visible { Show::Full } else { Show::Hidden(options.hide) };
    let host = match Host::open(loader, &options.data, show, options.browser_args.as_deref()) {
        Ok(host) => host,
        Err(e) => {
            say!("send: no web view: {e}");
            return 8;
        }
    };
    host.navigate("https://discord.com/channels/@me");
    if let Err(code) = host.wait_for_app(Duration::from_secs(90)) {
        host.close();
        return code;
    }
    pump(Duration::from_secs(3));
    let code = send(&host, &to, &text);
    // A moment for the send to reach Discord before the web view goes.
    pump(Duration::from_secs(3));
    host.close();
    code
}

fn send(host: &Host, to: &str, text: &str) -> i32 {
    let found = match host.evaluate(&with("TARGET", &json!(to), FIND)) {
        Ok(found) => found,
        Err(e) => {
            say!("send: finding the conversation failed: {e}: nothing sent");
            return 4;
        }
    };
    if found["found"] != 1 {
        say!(
            "send: {} conversations of that name among {} rows (read over {} pages): nothing sent",
            found["found"],
            found["rows"],
            found["pages"]
        );
        return 4;
    }
    let href = found["href"].as_str().unwrap_or_default().to_string();
    say!(
        "send: the conversation found (name matched {}, over {} pages); covered by something else: {}",
        if found["exact"] == true { "exactly" } else { "ignoring case" },
        found["pages"],
        yes(found["covered"] == true)
    );
    let composer_js = with("HREF", &json!(href), COMPOSER);
    if let Ok(v) = host.eval(&with("HREF", &json!(href), VIEWPORT)) {
        say!(
            "send: viewport {}x{} css px, visibility {}; the row {}x{} at {},{}; elementFromPoint at its centre is the row: {}",
            v["w"],
            v["h"],
            v["visibility"].as_str().unwrap_or("?"),
            v["rw"],
            v["rh"],
            v["rx"],
            v["ry"],
            yes(v["hit"] == true)
        );
    }

    // Opening it: the page's own router first, then a full load of the address, then a DevTools click on the row.
    let mut composer = Value::Null;
    let mut opened_by = None;
    for (way, limit) in [(Open::Push, 6), (Open::Navigate, 30), (Open::Click, 10)] {
        match way {
            Open::Push => {
                if let Err(e) = host.eval(&with("HREF", &json!(href), PUSH)) {
                    say!("send: {}: the script failed: {e}", way.name());
                    continue;
                }
            }
            Open::Navigate => host.navigate(&format!("https://discord.com{href}")),
            Open::Click => {
                // The row found again: the ways before may have moved or reloaded the list.
                let again = host.evaluate(&with("TARGET", &json!(to), FIND)).unwrap_or(Value::Null);
                if again["found"] != 1 {
                    say!("send: {}: the row is not found again", way.name());
                    continue;
                }
                host.click(again["x"].as_f64().unwrap_or(0.0), again["y"].as_f64().unwrap_or(0.0));
            }
        }
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(limit) {
            pump(Duration::from_millis(250));
            if let Ok(c) = host.eval(&composer_js) {
                let ready = c["opened"] == true && c["box"] == true;
                composer = c;
                if ready {
                    break;
                }
            }
        }
        say!(
            "send: {}: after {} ms the route is the conversation's {}, its message box (role textbox) present {}",
            way.name(),
            host::ms(started),
            yes(composer["opened"] == true),
            yes(composer["box"] == true)
        );
        if composer["opened"] == true && composer["box"] == true {
            opened_by = Some(way);
            break;
        }
    }
    let Some(way) = opened_by else {
        say!("send: the conversation did not open by any way: nothing sent");
        return 5;
    };
    say!("send: opened by {}; it shows {} messages", way.name(), composer["messages"]);
    if composer["empty"] != true {
        say!("send: the message box already holds a draft: nothing sent");
        return 6;
    }

    // Focus: DevTools' DOM.focus on the box, else a DevTools click in it, else the box's own focus().
    let mut focused = false;
    for way in ["DOM.focus", "a DevTools click", "the box's focus()"] {
        match way {
            "DOM.focus" => {
                if let Err(e) = dom_focus(host) {
                    say!("send: DOM.focus: {e}");
                    continue;
                }
            }
            "a DevTools click" => {
                host.click(composer["x"].as_f64().unwrap_or(0.0), composer["y"].as_f64().unwrap_or(0.0))
            }
            _ => {
                let _ = host.eval(&format!("{{ {BOX_JS}.focus(); true }}"));
            }
        }
        pump(Duration::from_millis(400));
        focused = host.eval(&composer_js).map(|c| c["focused"] == true).unwrap_or(false);
        say!("send: the message box has focus after {way}: {}", yes(focused));
        if focused {
            break;
        }
    }
    if !focused {
        say!("send: not sent: the text would go elsewhere");
        return 5;
    }
    let before = composer["messages"].as_u64().unwrap_or(0);

    if let Err(e) = host.cdp("Input.insertText", json!({ "text": text })) {
        say!("send: Input.insertText failed: {e}: nothing sent");
        return 7;
    }
    pump(Duration::from_millis(400));
    let holds = host
        .eval(&with("TEXT", &json!(text), &format!("(({BOX_JS}.textContent || '').replace(/[\\u200b\\ufeff]/g, '').replace(/\\s+/g, ' ').trim() === TEXT.replace(/\\s+/g, ' ').trim())")))
        .map(|v| v == true)
        .unwrap_or(false);
    say!("send: the message box holds exactly the text: {}", yes(holds));
    if !holds {
        say!("send: Enter not pressed: nothing sent (what went in stays as a draft in this session's box)");
        return 7;
    }
    host.key(Key::Enter, false);
    let sent_at = Instant::now();
    say!("send: Enter pressed (once)");

    let sent_js = with("TEXT", &json!(text), SENT);
    let mut last = Value::Null;
    while sent_at.elapsed() < Duration::from_secs(15) {
        pump(Duration::from_millis(250));
        let Ok(s) = host.eval(&sent_js) else { continue };
        let settled = s["last_is_text"] == true && s["sending"] != true;
        let failed = s["failed"] == true;
        last = s;
        if settled || failed {
            break;
        }
    }
    say!(
        "send: after {} ms: the newest message is the text {}, still marked sending {}, marked failed {}, box empty {}; messages {} before, {} now",
        host::ms(sent_at),
        yes(last["last_is_text"] == true),
        yes(last["sending"] == true),
        yes(last["failed"] == true),
        yes(last["box_empty"] == true),
        before,
        last["messages"]
    );
    if last["last_is_text"] == true && last["sending"] != true && last["failed"] != true {
        say!("send: the message appeared in the page");
        0
    } else {
        say!("send: the message was not seen to arrive (look in Discord itself before anything else)");
        7
    }
}
