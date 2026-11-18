//! What is asked of the Agent and how its answers read: the one place where the route's unknowns are. Each is
//! listed in docs/sources.md ("Battle.net through its Agent") with how it is known; the sources are barncastle's
//! Battle.Net-Installer (`3aa2d98`), HSCentric's `BattleNetAgentClient.cs` (`14f79ee`), DroidDeck's
//! `droiddeck-battlenet` (`0ce168b`), redact7's `bnetinstaller` (`6ba7a3c`), the wowdev wiki's "Agent" (revision
//! 36960), and the TV PC's own Agent (2.42.1.9886, 2026-11-18), docs/research/installs.md 7. The stand-in Agent
//! (`crate::standin`) answers exactly these.
//!
//! **An install is a form** the Agent fills in with the Dashboard (the TV PC's answers; redact7's flow): `POST
//! /install` answers 201 with the form as the Agent first has it (`language.error` 704 and its `list` of locales,
//! `game_dir` with the Agent's own `dirname` and `default`, `required_space_info.state` 1005 while the size is
//! worked out); [`install_options`] posts the choices **unfinalized**, again each second while the size is worked out
//! (1005, until 1009, redact7's reading), and the answer is judged ([`problem`]); then the same choices finalized,
//! which starts the download (DroidDeck and redact7 ask nothing more; barncastle's `POST /update` follows, as here).
use serde_json::{json, Value};

use crate::http::Answer;

/// How the Agent is started when it is not running, with no window (all three tools start it so).
pub const START_ARGS: &[&str] = &["--internalclienttools"];

/// The session: `GET /agent`, whose answer's `authorization` is sent back, as it is, as the `Authorization` header.
pub const SESSION: &str = "/agent";

/// The locale taken where Windows' own is not in the form's `list`, as the form writes it (the TV PC's form lists
/// `enUS`; DroidDeck's fallback is the same).
pub const LOCALE: &str = "enUS";

/// The header with the Agent's own process id that redact7's bnetinstaller sends to an Agent it finds running (its
/// `get_authorization`): asked once of an Agent the Dashboard did not start that gives no token without it.
pub const PID_HEADER: &str = "pid";

/// The form's sections whose error is a refusal from the first answer on: the account, the licence, the PC
/// (barncastle's `InstallEndpoint` looks at `authentication`, `game_dir` and `min_spec`; redact7's at every one).
/// The others (`language`, `game_dir`, `tome_path`) are the Dashboard's to fill in, and judged once it has.
pub const REFUSING: &[&str] = &["authentication", "eula", "min_spec"];

/// `required_space_info.state` while the Agent works out the size: the form is posted again until it is not
/// (redact7: "1005 should mean in progress", 1009 done; the TV PC's first answer had 1005 with a size of 0).
pub const SIZING: i64 = 1005;

/// Whether to stop before the download: `DASHBOARD_BATTLENET_NO_DOWNLOAD=1` fills in the form, has the Agent judge
/// it unfinalized, and abandons it there, nothing downloaded: a try on the real Agent with a game of tens of GB.
pub fn no_download() -> bool {
    std::env::var("DASHBOARD_BATTLENET_NO_DOWNLOAD").is_ok_and(|on| on == "1")
}

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

/// `POST /install/<uid>`: where (the game's own folder, `<chosen>\<the form's dirname>`, as the form's own
/// `game_dir` is the default and the dirname), in which locale (one of the form's `list`, as it writes it), and
/// whether finalized (barncastle's `ProductModel`; redact7's `InstallFinal`).
pub fn install_options(uid: &str, game_dir: &str, locale: &str, finalized: bool) -> (&'static str, String, Value) {
    let body = json!({
        "account_country": "USA",
        "finalized": finalized,
        "game_dir": game_dir.trim_end_matches(['\\', '/']),
        "geo_ip_country": "US",
        "language": [locale],
        "selected_asset_locale": locale,
        "selected_locale": locale,
        "shortcut": "none",
        "tome_torrent": "",
    });
    ("POST", format!("/install/{uid}"), body)
}

/// The form in an answer to `POST /install` or `POST /install/<uid>`, as far as the Dashboard reads it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Form {
    /// `language.list`: the locales the game comes in, as the Agent writes them (`enUS`).
    pub languages: Vec<String>,
    /// `game_dir.dirname`: the folder the Agent names the game's (`Tony Hawks Pro Skater 3 + 4`).
    pub dirname: Option<String>,
    /// `game_dir.default`: where it would put it.
    pub default_dir: Option<String>,
    /// `game_dir.required_space`, bytes.
    pub required_space: Option<u64>,
    /// `game_dir.required_space_info.state`.
    pub space_state: Option<i64>,
    pub finalized: bool,
    /// Each section's error above 0, as `(section, error)`.
    pub errors: Vec<(String, i64)>,
    /// Each section's warning above 0 (`game_dir.warning` 806 on the TV PC: not known what it says).
    pub warnings: Vec<(String, i64)>,
}

impl Form {
    /// Whether the Agent is still working out the size.
    pub fn sizing(&self) -> bool {
        self.space_state == Some(SIZING)
    }

    /// How the log says it: `language 704 (no language chosen yet); locales enUS, enGB; its folder "X" (default
    /// under Y); warning game_dir 806; 580000000 bytes; size state 1005; not finalized`.
    pub fn said(&self) -> String {
        let mut parts = Vec::new();
        for (section, error) in &self.errors {
            match meaning(&error.to_string()) {
                Some(meaning) => parts.push(format!("{section} {error} ({meaning})")),
                None => parts.push(format!("{section} {error}")),
            }
        }
        if !self.languages.is_empty() {
            parts.push(format!("locales {}", self.languages.join(", ")));
        }
        if let Some(dirname) = &self.dirname {
            let default = self.default_dir.as_deref().unwrap_or("?");
            parts.push(format!("its folder \"{dirname}\" (default under {default})"));
        }
        for (section, warning) in &self.warnings {
            parts.push(format!("warning {section} {warning}"));
        }
        if let Some(bytes) = self.required_space {
            parts.push(format!("{bytes} bytes"));
        }
        if let Some(state) = self.space_state {
            parts.push(format!("size state {state}"));
        }
        parts.push(if self.finalized { "finalized".into() } else { "not finalized".into() });
        parts.join("; ")
    }
}

fn number(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_str()?.parse().ok())
}

/// The form an answer holds, where it holds one.
pub fn read_form(answer: &Answer) -> Option<Form> {
    let form = answer.body.get("form")?.as_object()?;
    let section = |name: &str| form.get(name);
    let texts = |v: Option<&Value>| -> Vec<String> {
        v.and_then(Value::as_array)
            .map(|items| items.iter().filter_map(|i| i.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    let game_dir = section("game_dir");
    let text = |key: &str| game_dir.and_then(|g| g.get(key)).and_then(Value::as_str).map(String::from);
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for (name, value) in form {
        if let Some(n) = value.get("error").and_then(number).filter(|&n| n > 0) {
            errors.push((name.clone(), n));
        }
        if let Some(n) = value.get("warning").and_then(number).filter(|&n| n > 0) {
            warnings.push((name.clone(), n));
        }
    }
    Some(Form {
        languages: texts(section("language").and_then(|l| l.get("list"))),
        dirname: text("dirname").filter(|d| !d.trim().is_empty()),
        default_dir: text("default"),
        required_space: game_dir.and_then(|g| g.get("required_space")).and_then(Value::as_u64),
        space_state: game_dir.and_then(|g| g.get("required_space_info")).and_then(|i| i.get("state")).and_then(number),
        finalized: form.get("finalized").and_then(Value::as_bool).unwrap_or(false),
        errors,
        warnings,
    })
}

/// Whether an answer refuses the install outright: a status that is not 2xx, an `error` above 0, or a section of
/// [`REFUSING`] in error. A form still to be filled in (`language` 704) is not a refusal.
pub fn refusal(answer: &Answer) -> Option<String> {
    if let Some(why) = top_error(answer) {
        return Some(why);
    }
    let form = read_form(answer)?;
    let refusing = form.errors.iter().find(|(section, _)| REFUSING.contains(&section.as_str()));
    refusing.map(|(section, n)| format!("{section}: error {n}"))
}

/// Whether the form, filled in, still has something wrong: a refusal, or any section's error above 0.
pub fn problem(answer: &Answer) -> Option<String> {
    if let Some(why) = refusal(answer) {
        return Some(why);
    }
    let form = read_form(answer)?;
    form.errors.first().map(|(section, n)| format!("{section}: error {n}"))
}

/// The locale to ask for, of the form's `list`, as the form writes it: Windows' own where it is listed (`en-US` is
/// `enUS`), else [`LOCALE`], else the first. None where the list is empty.
pub fn choose_locale(list: &[String], windows: Option<&str>) -> Option<String> {
    let plain = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
    let find = |wanted: &str| list.iter().find(|l| plain(l) == plain(wanted)).cloned();
    windows.and_then(find).or_else(|| find(LOCALE)).or_else(|| list.first().cloned())
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
/// wiki has only for `/repair/<uid>`, "Cancels the current Repairing process"), then what was fetched removed by
/// [`uninstall`].
pub fn cancel(uid: &str) -> (&'static str, String) {
    ("DELETE", format!("/install/{uid}"))
}

/// A form the Agent took and the Dashboard will not finish (refused, or stopped before the download): the same
/// `DELETE`, **a guess**: no source abandons a form. Then the Agent the Dashboard started is stopped ([`stop`]), which
/// drops whatever it still held (barncastle's and HSCentric's only way out: they kill the Agent they started).
pub fn abandon(uid: &str) -> (&'static str, String) {
    cancel(uid)
}

/// `DELETE /agent`: the Agent asked to end (the wiki: "will destroy the Agent instance and (possibly) shutdown the
/// Agent process"; barncastle's last call).
pub fn stop() -> (&'static str, &'static str) {
    ("DELETE", "/agent")
}

/// An answer's error, where it gives one: its `error` above 0, else a section's (`form.<section>.error`,
/// barncastle's `InstallEndpoint`), else a status that is not 2xx. For the calls that are no form (progress,
/// uninstall, steering); a form is judged by [`refusal`] and [`problem`].
pub fn error(answer: &Answer) -> Option<String> {
    if let Some(Value::Object(form)) = answer.body.get("form") {
        for (section, value) in form {
            if let Some(n) = value.get("error").and_then(number).filter(|&n| n > 0) {
                return top_error(answer).or(Some(format!("{section}: error {n}")));
            }
        }
    }
    top_error(answer)
}

/// The answer's own `error` above 0, or a status that is not 2xx.
fn top_error(answer: &Answer) -> Option<String> {
    match answer.body.get("error") {
        Some(v) if number(v).is_some_and(|n| n > 0) => return Some(format!("error {}", number(v).unwrap_or(0))),
        Some(Value::String(s)) if !s.is_empty() && s.parse::<i64>().is_err() => return Some(s.clone()),
        _ => {}
    }
    (!answer.ok()).then(|| format!("status {}", answer.status))
}

/// What the errors met so far mean, for the log and the user (barncastle's README; DroidDeck; 704 as the TV PC's
/// first form had it, before a language was posted: documented nowhere). Each has Blizzard's own article,
/// `BLZBNTAGT` and the code in eight hex digits (redact7's `error_code_to_support_article`), behind a sign-in.
pub fn meaning(error: &str) -> Option<&'static str> {
    let code = error.split_whitespace().last()?;
    Some(match code {
        "704" => "no language chosen yet",
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

    /// The TV PC's Agent's first answer to `POST /install` for Tony Hawk's Pro Skater 3 + 4 (2026-11-18), as the
    /// lead copied it, `min_spec` cut short.
    fn tv_form() -> Answer {
        let list = json!(["enUS", "enGB", "frFR", "deDE", "itIT", "jaJP", "ptBR", "esMX", "esES"]);
        answer(
            201,
            json!({"form": {"authentication": {"error": 0}, "eula": {"error": 0, "eula": true}, "finalized": false,
                "game_dir": {"default": "C:\\Program Files (x86)\\", "dirname": "Tony Hawks Pro Skater 3 + 4",
                    "error": 0, "game_dir": "C:/Program Files (x86)/Tony Hawks Pro Skater 3 + 4",
                    "required_space": 580000000u64, "required_space_info": {"size": 0, "state": 1005}, "warning": 806},
                "language": {"error": 704, "list": list, "speech": list, "text": list},
                "min_spec": {"error": 0}, "shortcut": "all", "tome_path": {"error": 0}},
                "response_uri": "/install/libra"}),
        )
    }

    #[test]
    fn the_first_form_is_filled_in_not_refused() {
        let first = tv_form();
        assert_eq!(refusal(&first), None, "704 is a form to fill in");
        assert_eq!(problem(&first).as_deref(), Some("language: error 704"));
        let form = read_form(&first).unwrap();
        assert_eq!(form.languages[..2], ["enUS".to_string(), "enGB".to_string()]);
        assert_eq!(form.dirname.as_deref(), Some("Tony Hawks Pro Skater 3 + 4"));
        assert_eq!((form.sizing(), form.finalized, form.required_space), (true, false, Some(580_000_000)));
        assert_eq!(form.warnings, [("game_dir".to_string(), 806)]);
        assert!(
            form.said().starts_with("language 704 (no language chosen yet); locales enUS, enGB"),
            "{}",
            form.said()
        );
        // A refusal is the account's, the licence's or the PC's.
        let mut refused = tv_form();
        refused.body["form"]["authentication"]["error"] = json!(1);
        assert_eq!(refusal(&refused).as_deref(), Some("authentication: error 1"));
        assert_eq!(refusal(&answer(200, json!({"error": 2221}))).as_deref(), Some("error 2221"));
        assert_eq!(refusal(&answer(500, Value::Null)).as_deref(), Some("status 500"));
        // Windows' own locale where the form lists it, as the form writes it; else enUS; else the first.
        let list = form.languages.clone();
        assert_eq!(choose_locale(&list, Some("fr-FR")).as_deref(), Some("frFR"));
        assert_eq!(choose_locale(&list, Some("en-AU")).as_deref(), Some("enUS"));
        assert_eq!(choose_locale(&list, None).as_deref(), Some("enUS"));
        assert_eq!(choose_locale(&["koKR".to_string()], Some("en-US")).as_deref(), Some("koKR"));
        assert_eq!(choose_locale(&[], None), None);
        let (_, path, body) = install_options("libra", r"D:\Games\Tony Hawks Pro Skater 3 + 4\", "enUS", false);
        assert_eq!(path, "/install/libra");
        assert_eq!(body["game_dir"], r"D:\Games\Tony Hawks Pro Skater 3 + 4");
        assert_eq!((body["selected_locale"].as_str(), body["finalized"].as_bool()), (Some("enUS"), Some(false)));
        assert_eq!(body["language"], json!(["enUS"]));
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
        assert_eq!(stop(), ("DELETE", "/agent"));
    }
}
