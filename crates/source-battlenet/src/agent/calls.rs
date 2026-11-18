//! What is asked of the Agent and how its answers read: the one place where the route's unknowns are. Each is
//! listed in docs/sources.md ("Battle.net through its Agent") with how it is known; the sources are barncastle's
//! Battle.Net-Installer (`3aa2d98`), HSCentric's `BattleNetAgentClient.cs` (`14f79ee`), DroidDeck's
//! `droiddeck-battlenet` (`0ce168b`) and the wowdev wiki's "Agent" (revision 36960), docs/research/installs.md 7.
//! The stand-in Agent (`crate::standin`) answers exactly these.
use serde_json::{json, Value};

use crate::http::Answer;

/// How the Agent is started when it is not running, with no window (all three tools start it so).
pub const START_ARGS: &[&str] = &["--internalclienttools"];

/// The session: `GET /agent`, whose answer's `authorization` is sent back, as it is, as the `Authorization` header.
pub const SESSION: &str = "/agent";

/// The language asked for: the Agent's `selected_locale`, lower case as barncastle's and DroidDeck's send it.
pub const LOCALE: &str = "enus";

/// Whether the Dashboard's guesses at pause, resume and cancel (below) are asked at all. No tool calls any and the
/// wiki documents none (B3): off, Battle.net's installs offer no Pause, Resume or Cancel (`Can`), unless
/// `DASHBOARD_BATTLENET_STEER=1` is set, as the end-to-end test against the stand-in does and as the TV PC may to try
/// them on the real Agent, every answer logged.
pub fn steer_on() -> bool {
    std::env::var("DASHBOARD_BATTLENET_STEER").is_ok_and(|on| on == "1")
}

/// The token in `GET /agent`'s answer.
pub fn token(answer: &Answer) -> Option<String> {
    let token = answer.body.get("authorization").and_then(|t| match t {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    });
    token.filter(|t| answer.ok() && !t.is_empty())
}

/// Whether a status says the token is no good: 401 (HSCentric), 403 (the wiki).
pub fn refused(answer: &Answer) -> bool {
    matches!(answer.status, 401 | 403)
}

/// Where the Agent fetches a product's builds from: Blizzard's patch server of the Agent's region (`GET /agent`'s
/// `region`, as DroidDeck picks it), for its NGDP product code.
pub fn patch_url(region: &str, product: &str) -> String {
    let region = match region.trim().to_ascii_lowercase().as_str() {
        "eu" | "kr" | "tw" => region.trim().to_ascii_lowercase(),
        _ => "us".into(),
    };
    format!("http://{region}.patch.battle.net:1119/{product}")
}

/// `POST /install`: the product put in the Agent's queue under its uid, at an install's priority (900).
pub fn install(uid: &str, region: &str, product: &str) -> (&'static str, String, Value) {
    let body = json!({
        "uid": uid,
        "instructions_patch_url": patch_url(region, product),
        "instructions_product": "NGDP",
        "monitor_pid": std::process::id(),
        "priority": { "insert_at_head": true, "value": 900 },
    });
    ("POST", "/install".into(), body)
}

/// `POST /install/<uid>`: where, and in which language (barncastle's `ProductModel`). The folder ends with a
/// backslash, as theirs.
pub fn install_options(uid: &str, game_dir: &str) -> (&'static str, String, Value) {
    let mut dir = game_dir.to_string();
    if !dir.ends_with(['\\', '/']) {
        dir.push('\\');
    }
    let body = json!({
        "account_country": "USA",
        "finalized": true,
        "game_dir": dir,
        "geo_ip_country": "US",
        "language": [LOCALE],
        "selected_asset_locale": LOCALE,
        "selected_locale": LOCALE,
        "shortcut": "none",
        "tome_torrent": "",
    });
    ("POST", format!("/install/{uid}"), body)
}

/// `POST /update`: what barncastle's sends next to have the download start.
pub fn start(uid: &str) -> (&'static str, String, Value) {
    ("POST", "/update".into(), json!({ "uid": uid, "priority": { "insert_at_head": true, "value": 699 } }))
}

/// `GET /install/<uid>`: how far it has got.
pub fn progress(uid: &str) -> (&'static str, String) {
    ("GET", format!("/install/{uid}"))
}

/// `POST /uninstall` (the wiki; DroidDeck's), with the compaction the client asks for.
pub fn uninstall(uid: &str) -> (&'static str, String, Value) {
    ("POST", "/uninstall".into(), json!({ "uid": uid, "run_compaction": true }))
}

/// Pause and resume: **a guess**, asked only with [`steer_on`]. The progress has a `paused` field and nothing is
/// known to set it.
pub fn pause(uid: &str, paused: bool) -> (&'static str, String, Value) {
    ("POST", format!("/install/{uid}"), json!({ "paused": paused }))
}

/// Cancel: **a guess**, asked only with [`steer_on`]: the install taken out of the queue (`DELETE` on it, which the
/// wiki has only for `/repair/<uid>`), then what was fetched removed by [`uninstall`].
pub fn cancel(uid: &str) -> (&'static str, String) {
    ("DELETE", format!("/install/{uid}"))
}

/// An answer's error, where it gives one: its `error` above 0, else a section's (`form.<section>.error`,
/// barncastle's `InstallEndpoint`), else a status that is not 2xx.
pub fn error(answer: &Answer) -> Option<String> {
    let number = |v: &Value| v.as_i64().or_else(|| v.as_str()?.parse().ok());
    match answer.body.get("error") {
        Some(v) if number(v).is_some_and(|n| n > 0) => return Some(format!("error {}", number(v).unwrap_or(0))),
        Some(Value::String(s)) if !s.is_empty() && s.parse::<i64>().is_err() => return Some(s.clone()),
        _ => {}
    }
    if let Some(Value::Object(form)) = answer.body.get("form") {
        for (section, value) in form {
            if let Some(n) = value.get("error").and_then(number).filter(|&n| n > 0) {
                return Some(format!("{section}: error {n}"));
            }
        }
    }
    (!answer.ok()).then(|| format!("status {}", answer.status))
}

/// What the errors met so far mean, for the log and the user (barncastle's README; DroidDeck).
pub fn meaning(error: &str) -> Option<&'static str> {
    let code = error.split_whitespace().last()?;
    Some(match code {
        "2221" => "the product is not available to this account",
        "2421" => "the PC does not meet its needs, or there is not enough space",
        "3001" => "an encryption key is missing",
        "800" => "the folder already holds a game",
        _ => return None,
    })
}

/// What `GET /install/<uid>` says (the wiki's fields).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Progress {
    /// 0 to 1.
    pub progress: f64,
    /// Bytes downloaded, and in all, where it says: `download_current` and `download_total` are arrays of three, of
    /// which DroidDeck takes the first.
    pub done: Option<u64>,
    pub total: Option<u64>,
    /// `download_rate`: taken as bytes a second (the wiki's example, 6336703.5, says no unit).
    pub rate: Option<u64>,
    pub paused: bool,
    /// Done as far as the Queue goes: `playable`, and the download complete (`download_complete`, or `progress`
    /// 1), as HSCentric waits for.
    pub playable: bool,
    /// `state`: 1003 under way, 1004 idle or done (DroidDeck, HSCentric).
    pub state: Option<i64>,
}

pub fn read_progress(answer: &Answer) -> Progress {
    let body = &answer.body;
    let first = |v: &Value| match v {
        Value::Array(items) => items.first().cloned(),
        other => Some(other.clone()),
    };
    let number = |key: &str| {
        let v = first(body.get(key)?)?;
        v.as_f64().or_else(|| v.as_str()?.parse().ok())
    };
    let bytes = |key: &str| number(key).filter(|v| *v >= 0.0).map(|v| v as u64);
    let flag = |key: &str| body.get(key).and_then(Value::as_bool).unwrap_or(false);
    let total = bytes("download_total");
    let done = bytes("download_current").or_else(|| Some(total? - bytes("download_remaining")?.min(total?)));
    let progress = number("progress").unwrap_or(0.0).clamp(0.0, 1.0);
    Progress {
        progress,
        done,
        total,
        rate: bytes("download_rate"),
        paused: flag("paused"),
        playable: flag("playable") && (flag("download_complete") || progress >= 1.0),
        state: body.get("state").and_then(Value::as_i64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(status: u16, body: Value) -> Answer {
        Answer { status, body }
    }

    #[test]
    fn the_answers_read_as_the_tools_read_them() {
        assert_eq!(token(&answer(200, json!({"authorization": "T", "pid": 4}))).as_deref(), Some("T"));
        assert_eq!(
            token(&answer(200, json!({"authorization": 9487550018965591789u64}))).as_deref(),
            Some("9487550018965591789")
        );
        assert_eq!(token(&answer(401, json!({"authorization": "T"}))), None);
        assert!(refused(&answer(403, Value::Null)) && !refused(&answer(404, Value::Null)));
        let read = read_progress(&answer(
            200,
            json!({"progress": 0.25, "download_rate": 6336703.5, "download_current": [250, 0, 0],
                   "download_total": [1000, 0, 0], "paused": false, "playable": false, "state": 1003}),
        ));
        assert_eq!(
            read,
            Progress {
                progress: 0.25,
                done: Some(250),
                total: Some(1000),
                rate: Some(6_336_703),
                paused: false,
                playable: false,
                state: Some(1003)
            }
        );
        // Only what is left: done worked out.
        let read =
            read_progress(&answer(200, json!({"download_total": 1000, "download_remaining": 100, "paused": true})));
        assert_eq!((read.done, read.paused), (Some(900), true));
        assert!(read_progress(&answer(200, json!({"progress": 1.0, "playable": true}))).playable);
        assert!(!read_progress(&answer(200, json!({"progress": 0.4, "playable": true}))).playable);
        assert!(
            read_progress(&answer(200, json!({"progress": 0.4, "playable": true, "download_complete": true}))).playable
        );
    }

    #[test]
    fn errors_are_the_answers_own() {
        assert_eq!(error(&answer(200, json!({"error": 2221}))).as_deref(), Some("error 2221"));
        assert_eq!(meaning("error 2221"), Some("the product is not available to this account"));
        assert_eq!(error(&answer(200, json!({"error": 0}))), None);
        let form = json!({"form": {"game_dir": {"error": 800}, "authentication": {"error": 0}}});
        assert_eq!(error(&answer(200, form)).as_deref(), Some("game_dir: error 800"));
        assert_eq!(error(&answer(404, Value::Null)).as_deref(), Some("status 404"));
        assert_eq!(patch_url("EU", "lbra"), "http://eu.patch.battle.net:1119/lbra");
        assert_eq!(patch_url("", "lbra"), "http://us.patch.battle.net:1119/lbra");
        let (_, path, body) = install_options("libra", r"D:\Games\Tony Hawk's Pro Skater 3 + 4");
        assert_eq!(
            (path.as_str(), body["game_dir"].as_str()),
            ("/install/libra", Some(r"D:\Games\Tony Hawk's Pro Skater 3 + 4\"))
        );
    }
}
