//! The index from Discord's in-page stores (`js/store_find.js`, `js/index_store.js`), read-only, and `--index`:
//! counts, which store methods were found and timings to the log; names and ids to `--index-file`.
use std::time::Instant;

use serde_json::Value;

use super::host::{self, Host, Loader};
use super::voice::{self, STORE_FIND};
use crate::args::Options;
use crate::index::Index;
use crate::say;

const INDEX_STORE: &str = include_str!("../js/index_store.js");

/// Builds the index; logs how; writes `--index-file`. None (logged) if the stores were not found.
pub fn build(host: &Host, options: &Options) -> Option<Index> {
    let started = Instant::now();
    let v = match host.evaluate(&format!("{{ {STORE_FIND}\n{INDEX_STORE}\n}}")) {
        Ok(v) => v,
        Err(e) => {
            say!("index: the store script failed: {e}");
            return None;
        }
    };
    if let Some(found) = v["found"].as_object() {
        let line: Vec<String> = found
            .iter()
            .map(|(k, m)| {
                let names: Vec<&str> = m.as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                format!("{k} [{}]", if names.is_empty() { "NOT FOUND".to_string() } else { names.join(" ") })
            })
            .collect();
        say!("index: stores found among {} modules: {}", v["modules"], line.join("; "));
    }
    if v["ok"] != true {
        say!("index: not built: {}", v["why"].as_str().unwrap_or("?"));
        return None;
    }
    let index = Index::from_json(&v);
    say!(
        "index: built from Discord's stores in {} ms (the page's script {} ms): {}",
        host::ms(started),
        v["ms"],
        index.counts()
    );
    match std::fs::write(&options.index_file, index.text()) {
        Ok(()) => say!("index: names and ids written to {} (not to this log)", options.index_file.display()),
        Err(e) => say!("index: not written to {}: {e}", options.index_file.display()),
    }
    Some(index)
}

/// `--index`: the signed-in page, its index built, and again after 10 s to time a rebuild.
pub fn run(options: &Options, loader: &Loader) -> i32 {
    let host = match voice::open(options, loader) {
        Ok(host) => host,
        Err(code) => return code,
    };
    let code = match build(&host, options) {
        Some(_) => {
            host::pump(std::time::Duration::from_secs(10));
            say!("index: built again after 10 s, to time a refresh");
            build(&host, options);
            0
        }
        None => 7,
    };
    host.close();
    code
}
