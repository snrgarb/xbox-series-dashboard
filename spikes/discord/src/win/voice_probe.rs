//! `--voice-probe`: the signed-in session hidden one way (`--hide`) with one set of browser switches
//! (`--args-set`): the microphone as the page gets it (permission, devices, getUserMedia), then an AudioContext of
//! the spike's own (a silent tone and the microphone into an analyser) and a 1 s timer watched for `--hold` s, every
//! 10 s, beside the web view's processor time and memory. Ends with one verdict line for this combination. Nothing
//! of Discord's is pressed; nothing is joined.
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::host::{pump, yes, Loader};
use super::voice::{self, with, Costs};
use crate::args::Options;
use crate::say;

const DEVICES: &str = include_str!("../js/probe_devices.js");
const AUDIO: &str = include_str!("../js/probe_audio.js");

fn devices_line(when: &str, d: &Value) -> String {
    if let Some(e) = d["error"].as_str() {
        return format!("probe: devices {when}: enumerateDevices failed: {e}");
    }
    format!(
        "probe: devices {when}: {} inputs, {} outputs, {} cameras; labels shown {}; a \"default\" input {}, a \"communications\" input {}",
        d["inputs"],
        d["outputs"],
        d["cameras"],
        yes(d["labels"] == true),
        yes(d["defaultInput"] == true),
        yes(d["communicationsInput"] == true)
    )
}

pub fn run(options: &Options, loader: &Loader) -> i32 {
    let host = match voice::open(options, loader) {
        Ok(host) => host,
        Err(code) => return code,
    };
    let d = match host.evaluate(DEVICES) {
        Ok(d) => d,
        Err(e) => {
            say!("probe: the microphone script failed: {e}");
            host.close();
            return 7;
        }
    };
    say!("probe: the page says visibility {}, focus {}", d["visibility"].as_str().unwrap_or("?"), d["focus"]);
    say!("{}", devices_line("before getUserMedia", &d["before"]));
    say!("probe: the microphone permission as the page reads it: {}", d["permission"].as_str().unwrap_or("?"));
    let t = &d["track"];
    say!(
        "probe: getUserMedia({{audio: true}}) hidden: {} after {} ms; tracks {}, readyState {}, enabled {}, muted {}, sample rate {}, channels {}; the browser's echo cancellation {}, noise suppression {}, auto gain {}",
        d["gum"].as_str().unwrap_or("?"),
        d["gumMs"],
        t["count"],
        t["readyState"].as_str().unwrap_or("-"),
        t["enabled"],
        t["muted"],
        t["sampleRate"],
        t["channelCount"],
        t["echoCancellation"],
        t["noiseSuppression"],
        t["autoGainControl"]
    );
    say!("{}", devices_line("after getUserMedia", &d["after"]));
    say!(
        "probe: an output device can be chosen by the page: setSinkId on media elements {}, on AudioContext {}",
        yes(d["sinkMedia"] == true),
        yes(d["sinkContext"] == true)
    );

    let audio = |stage: &str, gesture: bool| {
        host.evaluate_as(&with("STAGE", &json!(stage), AUDIO), gesture).unwrap_or_else(|e| json!({ "error": e }))
    };
    let start = audio("start", false);
    say!(
        "probe: AudioContext made hidden: born {}, resume() {}, now {}; sample rate {}, base latency {:.3} s; the microphone into an analyser {}",
        start["born"].as_str().unwrap_or("?"),
        start["resumed"].as_str().unwrap_or("?"),
        start["state"].as_str().unwrap_or("?"),
        start["sampleRate"],
        start["baseLatency"].as_f64().unwrap_or(0.0),
        yes(start["mic"] == true)
    );
    if start["state"] != "running" {
        let again = audio("resume", true);
        say!(
            "probe: resume() again as a user gesture (DevTools userGesture): {}, now {}",
            again["resumed"].as_str().unwrap_or("?"),
            again["state"].as_str().unwrap_or("?")
        );
    }

    // The timer's count from the page's start is dropped: the hold counts from here.
    let first = audio("sample", false);
    let (mut clock, mut wall) = (first["time"].as_f64().unwrap_or(0.0), first["wall"].as_f64().unwrap_or(0.0));
    let (clock0, wall0) = (clock, wall);
    let (mut samples, mut running, mut ticks, mut gap, mut loudest) = (0u32, 0u32, 0u64, 0u64, 0f64);
    let mut costs = Costs::start(&host);
    let started = Instant::now();
    let mut ten = 0;
    say!("probe: holding {} s, a sample every 10 s", options.hold);
    while started.elapsed() < Duration::from_secs(options.hold) {
        ten += 1;
        let next = Duration::from_secs((ten * 10).min(options.hold));
        pump(next.saturating_sub(started.elapsed()));
        let s = audio("sample", false);
        if let Some(e) = s["error"].as_str() {
            say!("probe: a sample failed: {e}");
            continue;
        }
        let (c, w) = (s["time"].as_f64().unwrap_or(0.0), s["wall"].as_f64().unwrap_or(0.0));
        samples += 1;
        if s["state"] == "running" {
            running += 1;
        }
        ticks += s["timer"]["ticks"].as_u64().unwrap_or(0);
        gap = gap.max(s["timer"]["maxGap"].as_u64().unwrap_or(0));
        loudest = loudest.max(s["level"].as_f64().unwrap_or(0.0));
        say!(
            "probe {} s: AudioContext {}, its clock +{:.2} s over {:.2} s; timer ticks {} (max gap {} ms); mic track {} muted {}, level now {:.4}; page {} focus {}",
            started.elapsed().as_secs(),
            s["state"].as_str().unwrap_or("?"),
            c - clock,
            w - wall,
            s["timer"]["ticks"],
            s["timer"]["maxGap"],
            s["track"].as_str().unwrap_or("?"),
            s["muted"],
            s["level"].as_f64().unwrap_or(0.0),
            s["visibility"].as_str().unwrap_or("?"),
            s["focus"]
        );
        costs.report(&host, &format!("at {} s", started.elapsed().as_secs()));
        (clock, wall) = (c, w);
    }
    let (clock_span, wall_span) = (clock - clock0, (wall - wall0).max(0.001));
    let expected = wall_span.floor() as u64;
    let kept_running = samples > 0 && running == samples;
    let clock_kept = clock_span / wall_span >= 0.95;
    let timer_kept = ticks as f64 >= 0.9 * expected as f64 && gap <= 1500;
    say!(
        "probe verdict (hide {}, switches {}): AudioContext running {} ({running} of {samples} samples); its clock {:.1} s over {:.1} s ({}); timer {ticks} ticks of {expected} expected, max gap {gap} ms ({}); loudest mic level {:.4}: {}",
        options.hide.name(),
        if options.args_set_b { "b" } else { "a" },
        yes(kept_running),
        clock_span,
        wall_span,
        if clock_kept { "kept pace" } else { "fell behind" },
        if timer_kept { "on time" } else { "slowed" },
        loudest,
        if kept_running && clock_kept && timer_kept { "PASS" } else { "FAIL" }
    );
    let end = audio("end", false);
    say!(
        "probe: the microphone stopped, the AudioContext closed: {} (state {})",
        end["closed"].as_str().unwrap_or("?"),
        end["state"].as_str().unwrap_or("?")
    );
    say!("voice: notifications taken this run: {}", super::host::notifications());
    host.close();
    if kept_running && clock_kept && timer_kept {
        0
    } else {
        11
    }
}
