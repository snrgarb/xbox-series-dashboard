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
    let (x, y) = (found["x"].as_f64().unwrap_or(0.0), found["y"].as_f64().unwrap_or(0.0));
    host.click(x, y);

    let composer_js = with("HREF", &json!(href), COMPOSER);
    let started = Instant::now();
    let mut composer = Value::Null;
    while started.elapsed() < Duration::from_secs(10) {
        pump(Duration::from_millis(250));
        if let Ok(c) = host.eval(&composer_js) {
            let ready = c["opened"] == true && c["box"] == true;
            composer = c;
            if ready {
                break;
            }
        }
    }
    if composer["box"] != true {
        say!(
            "send: the conversation did not open with its message box in 10 s (route reached: {}): nothing sent",
            yes(composer["opened"] == true)
        );
        return 5;
    }
    say!("send: opened by the click in {} ms; it shows {} messages", host::ms(started), composer["messages"]);
    if composer["empty"] != true {
        say!("send: the message box already holds a draft: nothing sent");
        return 6;
    }
    host.click(composer["x"].as_f64().unwrap_or(0.0), composer["y"].as_f64().unwrap_or(0.0));
    pump(Duration::from_millis(400));
    let focused = host.eval(&composer_js).map(|c| c["focused"] == true).unwrap_or(false);
    say!("send: the message box has focus after a click: {}", yes(focused));
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
