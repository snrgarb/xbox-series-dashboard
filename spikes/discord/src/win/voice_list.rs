//! `--voice-list`: what can be joined, read hidden: the DM rows and servers counted; with `--server-file`, that
//! server opened by its route and its voice channels read. The log gets counts and selector hits; the names go to
//! `--voice-names` (servers always, the server's voice channels when one is given) for the lead to pick from.
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::host::Loader;
use super::voice::{self, with, CHANNELS, SERVERS};
use crate::args::Options;
use crate::say;

pub fn run(options: &Options, loader: &Loader) -> i32 {
    let server = match options.server_file.as_deref() {
        Some(path) => match voice::read_name(Some(path), "server") {
            Ok(name) => Some(name),
            Err(e) => {
                say!("voice-list: {e}");
                return 10;
            }
        },
        None => None,
    };
    let host = match voice::open(options, loader) {
        Ok(host) => host,
        Err(code) => return code,
    };
    let servers = match host.eval(&with("TARGET", &Value::Null, SERVERS)) {
        Ok(v) => v,
        Err(e) => {
            say!("voice-list: reading the server bar failed: {e}");
            host.close();
            return 7;
        }
    };
    voice::log_servers(&servers);
    let mut text = header();
    text.push_str("# servers, as the server bar shows them\n");
    for name in servers["names"].as_array().into_iter().flatten() {
        text.push_str(&format!("server\t{}\n", clean(name)));
    }
    let mut code = 0;
    if let Some(server) = server {
        say!("voice-list: a server named in {} characters", server.chars().count());
        if voice::open_server(&host, &server).is_some() {
            match host.evaluate(&with("TARGET", &json!(null), CHANNELS)) {
                Ok(v) => {
                    voice::log_channels(&v);
                    let list = v["voice"].as_array().cloned().unwrap_or_default();
                    say!(
                        "voice-list: {} voice channels in that server (stage channels among them {})",
                        list.len(),
                        list.iter().filter(|c| c["stage"] == true).count()
                    );
                    text.push_str(
                        "# that server's channel rows: name, users shown in it, told as voice (yes/no), a link (yes/no)\n",
                    );
                    for c in v["rows"].as_array().into_iter().flatten() {
                        text.push_str(&format!(
                            "channel\t{}\t{}\t{}\t{}\n",
                            clean(&c["name"]),
                            c["users"],
                            if c["voice"] == true { "voice" } else { "-" },
                            if c["link"] == true { "link" } else { "-" }
                        ));
                    }
                }
                Err(e) => {
                    say!("voice-list: reading the channels failed: {e}");
                    code = 7;
                }
            }
        } else {
            code = 4;
        }
    }
    match std::fs::File::create(&options.voice_names).and_then(|mut f| f.write_all(text.as_bytes())) {
        Ok(()) => say!("voice-list: the names written to {} (not to this log)", options.voice_names.display()),
        Err(e) => say!("voice-list: the names could not be written to {}: {e}", options.voice_names.display()),
    }
    host.close();
    code
}

fn header() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("# discord-spike --voice-list at unix time {now}. PRIVATE: keep it out of the repository.\n")
}

fn clean(v: &Value) -> String {
    v.as_str().unwrap_or("").replace(['\t', '\n', '\r'], " ")
}
