//! `--dms`: the saved session, hidden; the app page loaded; ONE script (`js/dms.js`) reading the DM list as the page
//! shows it. The log gets counts and which selectors matched; the names go to `--names` only. Then the web view's
//! memory, and its processor time at idle after `--wait-seconds`.
use std::io::Write;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::host::{self, pump, yes, Host, Loader, Show};
use super::procs;
use crate::args::Options;
use crate::say;

const DMS: &str = include_str!("../js/dms.js");

pub fn run(options: &Options, loader: &Loader) -> i32 {
    let show = if options.visible { Show::Full } else { Show::Hidden(options.hide) };
    let host = match Host::open(loader, &options.data, show, options.browser_args.as_deref()) {
        Ok(host) => host,
        Err(e) => {
            say!("dms: no web view: {e}");
            return 8;
        }
    };
    host.navigate("https://discord.com/channels/@me");
    if let Err(code) = host.wait_for_app(Duration::from_secs(90)) {
        host.close();
        return code;
    }
    if let Ok(v) = host.eval("({ visibility: document.visibilityState, focus: document.hasFocus() })") {
        say!("dms: the page says visibility {}, focus {}", v["visibility"].as_str().unwrap_or("?"), v["focus"]);
    }
    // The list fills in after the app shows.
    pump(Duration::from_secs(3));
    let started = Instant::now();
    let list = match host.evaluate(DMS) {
        Ok(list) => list,
        Err(e) => {
            say!("dms: the script failed: {e}");
            host.close();
            return 5;
        }
    };
    say!("dms: the script took {} ms", host::ms(started));
    log_structure(&list);
    write_names(options, &list);

    procs::report(&host.env, "app page, hidden");
    say!("dms: idle for {} s before measuring processor time", options.wait_seconds);
    pump(Duration::from_secs(options.wait_seconds));
    let (before, at) = (procs::cpu(&host.env), Instant::now());
    pump(Duration::from_secs(options.idle_seconds));
    let (after, wall) = (procs::cpu(&host.env), at.elapsed());
    let busy = after.saturating_sub(before) as f64 / 10_000_000.0;
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
    say!(
        "cpu: web view at idle over {:.0} s: {:.2} s of processor time, {:.2}% of one core, {:.2}% of all {} threads",
        wall.as_secs_f64(),
        busy,
        100.0 * busy / wall.as_secs_f64(),
        100.0 * busy / wall.as_secs_f64() / cores,
        cores
    );
    procs::report(&host.env, "after idle");
    host.close();
    0
}

fn log_structure(list: &Value) {
    if let Some(counts) = list["counts"].as_object() {
        let line: Vec<String> = counts.iter().map(|(k, v)| format!("{k}={v}")).collect();
        say!("dms: selectors matched: {}", line.join(" "));
    }
    let rows = list["rows"].as_array().cloned().unwrap_or_default();
    let count = |f: &dyn Fn(&Value) -> bool| rows.iter().filter(|r| f(r)).count();
    let name_from = |k: &str| count(&|r| r["nameFrom"] == k);
    let mentions_from = |k: &str| count(&|r| r["mentionsFrom"] == k);
    say!(
        "dms: {} conversations (list scroller found {}, read over {} pages); names from name_ class {}, aria-label {}, text {}",
        rows.len(),
        yes(list["scroller"].as_bool() == Some(true)),
        list["pages"],
        name_from("name_class"),
        name_from("aria-label"),
        name_from("text")
    );
    let bar = count(&|r| r["unreadBar"] == true);
    let class = count(&|r| r["unreadClass"] == true);
    let weight = count(&|r| r["unreadWeight"] == true);
    let any = count(&|r| r["unreadBar"] == true || r["unreadClass"] == true || r["unreadWeight"] == true);
    say!(
        "dms: unread: {any} by any sign; in the server bar {bar}, by an unread class {class}, by a heavier name {weight}; server bar entries {}",
        list["bar"]
    );
    let total: u64 = rows.iter().filter_map(|r| r["mentions"].as_u64()).sum();
    say!(
        "dms: with a count: {} (row badge {}, aria-label {}, server bar badge {}); counts sum to {total}",
        count(&|r| r["mentions"].as_u64().unwrap_or(0) > 0),
        mentions_from("row_badge"),
        mentions_from("aria-label"),
        mentions_from("guild_bar_badge")
    );
}

/// The names, for the lead's eyes only: a file beside the log, never the log.
fn write_names(options: &Options, list: &Value) {
    let rows = list["rows"].as_array().cloned().unwrap_or_default();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut text = format!(
        "# discord-spike --dms at unix time {now}: the DM list as the page showed it. PRIVATE: keep it out of the repository.\n# name\tunread (bar/class/weight)\tcount\n"
    );
    for r in &rows {
        let flag = |k: &str| if r[k] == true { "y" } else { "-" };
        text.push_str(&format!(
            "{}\t{}{}{}\t{}\n",
            r["name"].as_str().unwrap_or("").replace(['\t', '\n'], " "),
            flag("unreadBar"),
            flag("unreadClass"),
            flag("unreadWeight"),
            r["mentions"]
        ));
    }
    let written = std::fs::File::create(&options.names).and_then(|mut f| f.write_all(text.as_bytes()));
    match written {
        Ok(()) => say!("dms: {} names written to {} (not to this log)", rows.len(), options.names.display()),
        Err(e) => say!("dms: the names could not be written to {}: {e}", options.names.display()),
    }
}
