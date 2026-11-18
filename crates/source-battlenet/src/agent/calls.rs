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

/// A path the Agent names in an answer (`response_uri`, `result_uri`), where it is one of its own: `/` and a name,
/// never another host (HSCentric's `RequestAsync` checks the same).
fn named_path(answer: &Answer, key: &str) -> Option<String> {
    let path = answer.text(key)?.trim();
    (path.starts_with('/') && !path.starts_with("//") && !path.contains(' ')).then(|| path.to_string())
}

/// Where a finalized install's progress is read, in order (#172, the TV PC's real install, 2026-11-18): the
/// `response_uri` of each answer given, the `POST /update` answer's first (`/update/<uid>`), else `/update/<uid>`
/// itself; then `/install/<uid>`. On Agent 2.42.1.9886 `GET /install/<uid>` answered **404 `{}`** from the moment
/// the form was finalized, as the wiki says of it ("returns 404 otherwise", when no install is going on there), while
/// `POST /update` had answered `{"response_uri":"/update/libra","result_uri":"/game/libra"}`. barncastle's
/// `InstallProductTask` reads the install's path and, where that gives no `progress`, the update's; HSCentric reads
/// only `/update/<uid>`; DroidDeck and redact7 only `/install/<uid>`. All are asked; the first that says anything is
/// read.
pub fn progress_paths(uid: &str, answers: &[&Answer]) -> Vec<String> {
    let mut paths: Vec<String> = answers.iter().filter_map(|answer| named_path(answer, "response_uri")).collect();
    paths.push(format!("/update/{uid}"));
    paths.push(format!("/install/{uid}"));
    let mut seen = Vec::new();
    paths.retain(|path| {
        let new = !seen.contains(path);
        seen.push(path.clone());
        new
    });
    // The update's first: the install's own path is where the form was, and 404 once it is finalized.
    paths.sort_by_key(|path| !path.starts_with("/update/"));
    paths
}

/// Where the game itself is read while it downloads: the `result_uri` the answers name (`/game/<uid>` on the TV
/// PC), else `/game/<uid>` (HSCentric reads it beside `/update/<uid>`; DroidDeck at the end; the wiki's example has
/// `installed`, `download_complete`, `playable`, `update_progress`, `install_dir`).
pub fn game_path(uid: &str, answers: &[&Answer]) -> String {
    answers.iter().find_map(|answer| named_path(answer, "result_uri")).unwrap_or_else(|| format!("/game/{uid}"))
}

/// `POST /uninstall` (the wiki; DroidDeck's), with the compaction the client asks for.
pub fn uninstall(uid: &str) -> (&'static str, String, Value) {
    ("POST", "/uninstall".into(), json!({ "uid": uid, "run_compaction": true }))
}

/// Pause and resume: **a guess**, asked only with [`steer_on`], on the path the progress is read on (`/update/<uid>`
/// on the TV PC). Every source only reads `paused` (the wiki's example, DroidDeck's `REPORTED`); none sets it, and
/// the wiki has no pause call.
pub fn pause(path: &str, paused: bool) -> (&'static str, String, Value) {
    ("POST", path.to_string(), json!({ "paused": paused }))
}

/// Cancel: **a guess**, asked only with [`steer_on`]: `DELETE` on the path the progress is read on, as the wiki's
/// `DELETE /repair/<uid>` "Cancels the current Repairing process" (`POST /repair` answers a `response_uri` and a
/// `result_uri` as `POST /update` does). What is sure is the other half of a Cancel: the Agent the Dashboard started
/// is stopped (`DELETE /agent`, then killed), which ends its download, barncastle's and HSCentric's only way out.
pub fn cancel(path: &str) -> (&'static str, String) {
    ("DELETE", path.to_string())
}

/// A form the Agent took and the Dashboard will not finish (refused, or stopped before the download): `DELETE
/// /install/<uid>`, a guess no source makes, which the TV PC's Agent took (200, 2026-11-18). Then the Agent the
/// Dashboard started is stopped ([`stop`]), which drops whatever it still held.
pub fn abandon(uid: &str) -> (&'static str, String) {
    ("DELETE", format!("/install/{uid}"))
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

/// What a progress answer says (`GET /update/<uid>`, `GET /install/<uid>`, `GET /game/<uid>`), read tolerantly: any
/// of the shapes the sources show, each field where it is.
///
/// - The share, 0 to 1: `progress` (the wiki's 0.788332; barncastle prints it as a percentage of 1, redact7
///   multiplies it by 100, HSCentric clamps it to 0..1), else `update_progress` (the wiki's `/game/<uid>`, 1.0 when
///   done), else `playable_progress` (DroidDeck). A value above 1 and at most 100 is taken as a percentage, as is any
///   field named `percent`.
/// - The bytes: `download_current` and `download_total`, each a number or **an array whose parts add up** (the wiki's
///   example: three parts, and its `download_remaining` is exactly the totals' sum less the currents' sum), else
///   `download_total` less `download_remaining`; else, in a `download_details` object or at the top, `downloaded_bytes`
///   and `total_bytes` or `total`; else the share times the form's `required_space`.
/// - `download_rate` (the wiki's 6336703.5): taken as bytes a second, which no source says.
/// - `paused`, `playable`, `download_complete`, `installed` (`/game/<uid>`), `state` (1003 under way, 1004 done:
///   HSCentric, DroidDeck; DroidDeck also 1000 started), `extended_status.state` (DroidDeck's "phase"), `error`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Progress {
    /// 0 to 1, where it says.
    pub fraction: Option<f64>,
    /// Bytes downloaded, and in all.
    pub done: Option<u64>,
    pub total: Option<u64>,
    /// Bytes a second.
    pub rate: Option<u64>,
    pub paused: bool,
    pub playable: bool,
    /// `download_complete`.
    pub complete: bool,
    /// `installed`, where it says (`/game/<uid>`).
    pub installed: Option<bool>,
    pub state: Option<i64>,
    /// `extended_status.state`.
    pub phase: Option<i64>,
    /// `error` above 0.
    pub error: Option<i64>,
}

/// `state` once the Agent is done with an operation (HSCentric's `IsUpdateComplete`, DroidDeck's `DONE_STATE`).
pub const DONE_STATE: i64 = 1004;

impl Progress {
    /// Whether it says anything of an install at all: a share, bytes, a state or whether it is installed. `{}`, the
    /// TV PC's 404, says nothing.
    pub fn says_anything(&self) -> bool {
        self.fraction.is_some() || self.total.is_some() || self.state.is_some() || self.installed.is_some()
    }

    /// Whether it says the download is over: playable and complete (barncastle, HSCentric), installed, state 1004,
    /// or all of it. Only `product.db` saying installed ends an install; this says when to look.
    pub fn says_done(&self) -> bool {
        (self.playable && (self.complete || self.fraction.is_some_and(|f| f >= 1.0)))
            || self.installed == Some(true)
            || self.state == Some(DONE_STATE)
            || self.fraction.is_some_and(|f| f >= 1.0)
    }
}

/// A number, from a number or a text.
fn amount(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok())
}

/// Bytes, from a number or an array of parts, added up.
fn byte_count(v: &Value) -> Option<u64> {
    let total = match v {
        Value::Array(parts) if !parts.is_empty() => parts.iter().map(amount).sum::<Option<f64>>()?,
        other => amount(other)?,
    };
    (total >= 0.0).then_some(total as u64)
}

/// `answer` read as [`Progress`]; `required` is the form's `required_space`, for an answer with a share and no bytes.
pub fn read_progress(answer: &Answer, required: Option<u64>) -> Progress {
    let body = &answer.body;
    let details = body.get("download_details").filter(|d| d.is_object());
    // A field at the top, else in `download_details`.
    let field = |key: &str| body.get(key).filter(|v| !v.is_null()).or_else(|| details?.get(key));
    let first_bytes = |keys: &[&str]| keys.iter().find_map(|key| byte_count(field(key)?));
    let flag = |key: &str| field(key).and_then(Value::as_bool);
    let share = |key: &str| {
        let v = amount(field(key)?)?;
        let v = if key.contains("percent") || (v > 1.0 && v <= 100.0) { v / 100.0 } else { v };
        (v >= 0.0).then(|| v.min(1.0))
    };
    let fraction = ["progress", "percent", "progress_percent", "update_progress", "playable_progress"]
        .iter()
        .find_map(|key| share(key));
    let mut total = first_bytes(&["download_total", "total_bytes", "total"]);
    let mut done = first_bytes(&["download_current", "downloaded_bytes", "bytes_downloaded"])
        .or_else(|| Some(total?.saturating_sub(first_bytes(&["download_remaining", "remaining_bytes"])?)));
    if total.is_none_or(|t| t == 0) {
        if let (Some(f), Some(required)) = (fraction, required.filter(|&r| r > 0)) {
            (done, total) = (Some((f * required as f64) as u64), Some(required));
        }
    }
    let code = |v: &Value| v.as_i64().or_else(|| amount(v).map(|f| f as i64));
    Progress {
        fraction,
        done,
        total,
        rate: first_bytes(&["download_rate", "rate", "bytes_per_second"]),
        paused: flag("paused").unwrap_or(false),
        playable: flag("playable").unwrap_or(false),
        complete: flag("download_complete").unwrap_or(false),
        installed: flag("installed"),
        state: body.get("state").and_then(code),
        phase: body.get("extended_status").and_then(|e| e.get("state")).and_then(code),
        error: body.get("error").and_then(code).filter(|&n| n > 0),
    }
}

/// What an Agent's error code is, said for the user: `error 2421: the PC does not meet its needs, or there is not
/// enough space`, or only the code where nothing says what it means.
pub fn error_said(code: i64) -> String {
    let error = format!("error {code}");
    match meaning(&error) {
        Some(meaning) => format!("{error}: {meaning}"),
        None => error,
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
    }

    /// The TV PC's real install of Tony Hawk's Pro Skater 3 + 4 (Agent 2.42.1.9886, 2026-11-18): the finalized post's
    /// and `POST /update`'s answers name where the progress is, and `/install/libra` is 404 `{}` from then on.
    #[test]
    fn after_the_finalized_form_progress_is_read_where_the_agent_says() {
        let finalized = answer(
            200,
            json!({"form": {"finalized": true}, "response_uri": "/install/libra",
            "result_uri": "/game/libra"}),
        );
        let update = answer(200, json!({"response_uri": "/update/libra", "result_uri": "/game/libra"}));
        assert_eq!(progress_paths("libra", &[&update, &finalized]), ["/update/libra", "/install/libra"]);
        // Asked in the other order, the update's still comes first.
        assert_eq!(progress_paths("libra", &[&finalized, &update]), ["/update/libra", "/install/libra"]);
        assert_eq!(game_path("libra", &[&update, &finalized]), "/game/libra");
        // No answer named any: the uid's own.
        assert_eq!(progress_paths("libra", &[]), ["/update/libra", "/install/libra"]);
        assert_eq!(game_path("libra", &[&answer(200, json!({"result_uri": "http://elsewhere/x"}))]), "/game/libra");
        // The 404 says nothing, and names no error: not an end by itself.
        let gone = read_progress(&answer(404, json!({})), Some(57_000_000_000));
        assert!(!gone.says_anything() && gone.error.is_none() && !gone.says_done(), "{gone:?}");
    }

    #[test]
    fn progress_reads_in_every_shape_the_sources_show() {
        // The wiki's `/install/<uid>` example: three parts each, added up; `download_remaining` agrees.
        let wiki = read_progress(
            &answer(
                200,
                json!({"state": 1003.0, "playable": true, "progress": 0.788332, "download_rate": 6336703.523654,
                "download_current": [151679743.0, 4879705322.0, 241012436.0],
                "download_total": [151679743.0, 5467787030.0, 1076909397.0], "download_remaining": 1423978669.0,
                "paused": false}),
            ),
            None,
        );
        assert_eq!((wiki.done, wiki.total), (Some(5_272_397_501), Some(6_696_376_170)));
        assert_eq!(wiki.total.unwrap() - wiki.done.unwrap(), 1_423_978_669);
        assert_eq!((wiki.fraction, wiki.rate, wiki.state), (Some(0.788332), Some(6_336_703), Some(1003)));
        assert!(wiki.playable && !wiki.says_done(), "playable while downloading is not done");
        // HSCentric's `/update/<uid>`: a share, what is left, the phase; done by state 1004 and its flags.
        let hsc = read_progress(
            &answer(
                200,
                json!({"state": 1004, "progress": 1, "download_complete": true, "playable": true,
                "patch_application_complete": true, "download_remaining": 0, "extended_status": {"current": 3, "state": 7}}),
            ),
            None,
        );
        assert_eq!((hsc.fraction, hsc.phase), (Some(1.0), Some(7)));
        assert!(hsc.says_done());
        // The wiki's `/game/<uid>`: installed, `update_progress`.
        let game = read_progress(
            &answer(
                200,
                json!({"installed": true, "download_complete": true, "playable": true,
                "update_progress": 1.0, "install_dir": "C:/Program Files (x86)/World of Warcraft"}),
            ),
            None,
        );
        assert_eq!((game.installed, game.fraction), (Some(true), Some(1.0)));
        // A percentage, and bytes in `download_details`.
        let percent = read_progress(
            &answer(200, json!({"progress": 42.5, "download_details": {"downloaded_bytes": 425, "total": 1000}})),
            None,
        );
        assert_eq!((percent.fraction, percent.done, percent.total), (Some(0.425), Some(425), Some(1000)));
        assert_eq!(read_progress(&answer(200, json!({"percent": 0.5})), None).fraction, Some(0.005));
        // A share and no bytes: the share of the form's size.
        let share = read_progress(&answer(200, json!({"progress": 0.25, "state": 1003})), Some(57_000_000_000));
        assert_eq!((share.done, share.total), (Some(14_250_000_000), Some(57_000_000_000)));
        // Only what is left.
        let left = read_progress(
            &answer(200, json!({"download_total": 1000, "download_remaining": 100, "paused": true})),
            None,
        );
        assert_eq!((left.done, left.paused), (Some(900), true));
        // An error named.
        let failed = read_progress(&answer(200, json!({"error": 2421.0, "state": 1003})), None);
        assert_eq!(failed.error, Some(2421));
        assert_eq!(error_said(2421), "error 2421: the PC does not meet its needs, or there is not enough space");
        assert_eq!(error_said(9), "error 9");
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
