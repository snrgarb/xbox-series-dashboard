//! `--probe`: the PC (Windows, session, desktop, shell), the WebView2 runtime (registry, loader), then an environment
//! and a controller made in `--data` on a hidden window, a blank page's user agent, and the processes' memory.
//! `--online` also loads discord.com/login hidden and says what it showed. Nothing is signed in.
use std::path::Path;
use std::time::{Duration, Instant};

use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

use super::host::{self, pump, yes, Host, Loader, Show};
use super::{procs, system};
use crate::args::{Hide, Options};
use crate::say;

/// The WebView2 runtime's id in EdgeUpdate's registry keys (Microsoft's distribution docs).
const RUNTIME: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

pub fn run(options: &Options, loader: &Loader) -> i32 {
    the_pc();
    say!("loader: {} loaded", loader.path.display());
    match loader.runtime_version() {
        Ok(v) => say!("runtime: the loader finds Evergreen {v}"),
        Err(e) => {
            say!("runtime: the loader finds none: {e}");
            return 2;
        }
    }
    let started = Instant::now();
    let host = match Host::open(loader, &options.data, Show::Hidden(Hide::Controller), options.browser_args.as_deref())
    {
        Ok(host) => host,
        Err(e) => {
            say!("probe: no web view: {e}");
            return 4;
        }
    };
    say!("probe: environment and controller made from this unpackaged exe in {} ms", host::ms(started));
    host.navigate("about:blank");
    pump(Duration::from_secs(2));
    match host.eval("({ ua: navigator.userAgent, visibility: document.visibilityState, two: 1 + 1 })") {
        Ok(v) => say!(
            "probe: a script runs ({}); page visibility {}; user agent {}",
            v["two"],
            v["visibility"].as_str().unwrap_or("?"),
            v["ua"].as_str().unwrap_or("?")
        ),
        Err(e) => say!("probe: a script did not run: {e}"),
    }
    match host.cdp("Browser.getVersion", serde_json::json!({})) {
        Ok(v) => say!(
            "probe: DevTools protocol answers: {} (protocol {})",
            v["product"].as_str().unwrap_or("?"),
            v["protocolVersion"].as_str().unwrap_or("?")
        ),
        Err(e) => say!("probe: DevTools protocol does not answer: {e}"),
    }
    procs::report(&host.env, "blank page, hidden");
    if options.online {
        online(&host);
    }
    host.close();
    folder(&options.data);
    0
}

/// When there is no WebView2Loader.dll: what the PC and the registry say, still.
pub fn without_loader(_: &Options) {
    the_pc();
}

fn the_pc() {
    let nt = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    say!(
        "windows: {} {} build {}.{} (ProductName says Windows 10 on 11 too: the build decides, 22000 and up is 11)",
        system::reg_string(HKEY_LOCAL_MACHINE, nt, "ProductName").unwrap_or_default(),
        system::reg_string(HKEY_LOCAL_MACHINE, nt, "DisplayVersion").unwrap_or_default(),
        system::reg_string(HKEY_LOCAL_MACHINE, nt, "CurrentBuild").unwrap_or_default(),
        system::reg_dword(HKEY_LOCAL_MACHINE, nt, "UBR").unwrap_or_default()
    );
    let (mine, console) = system::sessions();
    say!(
        "session: this process in session {mine}, the console's is {console}; input desktop {}",
        system::input_desktop()
    );
    let shell =
        system::reg_string(HKEY_CURRENT_USER, r"Software\Microsoft\Windows NT\CurrentVersion\Winlogon", "Shell");
    let shell = match shell {
        Some(s) => {
            let exe = s.trim_matches('"').split('"').next().unwrap_or("").to_string();
            let file = Path::new(&exe).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or(exe);
            format!(
                "{file} (this user's Winlogon Shell: shell mode {})",
                yes(file.eq_ignore_ascii_case("dashboard.exe"))
            )
        }
        None => "not set for this user (explorer.exe, the machine's)".into(),
    };
    let counts =
        system::processes(&["explorer.exe", "dashboard.exe", "msedgewebview2.exe", "TextInputHost.exe", "TabTip.exe"]);
    say!("shell: {shell}");
    say!(
        "shell: explorer.exe processes {}, a shell window {}; dashboard.exe processes {}",
        counts[0],
        yes(system::shell_window()),
        counts[1]
    );
    say!(
        "processes before: msedgewebview2.exe {} (other programs' web views), TextInputHost.exe {}, TabTip.exe {}",
        counts[2],
        counts[3],
        counts[4]
    );
    unsafe {
        say!(
            "screen: {}x{} px, system DPI {}",
            GetSystemMetrics(SM_CXSCREEN),
            GetSystemMetrics(SM_CYSCREEN),
            GetDpiForSystem()
        );
    }
    let mut found = false;
    for (root, root_name, key) in [
        (HKEY_LOCAL_MACHINE, "HKLM", format!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{RUNTIME}")),
        (HKEY_LOCAL_MACHINE, "HKLM", format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{RUNTIME}")),
        (HKEY_CURRENT_USER, "HKCU", format!(r"Software\Microsoft\EdgeUpdate\Clients\{RUNTIME}")),
    ] {
        if let Some(pv) = system::reg_string(root, &key, "pv") {
            found = true;
            say!("runtime: registry {root_name}\\{key}: Evergreen version {pv}");
        }
    }
    if !found {
        say!("runtime: no Evergreen WebView2 runtime in the registry (machine or user)");
    }
    match std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER") {
        Some(dir) => {
            say!("runtime: WEBVIEW2_BROWSER_EXECUTABLE_FOLDER names a fixed version: {}", dir.to_string_lossy())
        }
        None => say!("runtime: no fixed version (no browser folder given, WEBVIEW2_BROWSER_EXECUTABLE_FOLDER not set)"),
    }
}

/// discord.com/login, hidden: does it load in this web view, and what does it show.
fn online(host: &Host) {
    host.navigate("https://discord.com/login");
    let started = Instant::now();
    let mut last = None;
    while started.elapsed() < Duration::from_secs(45) {
        pump(Duration::from_secs(1));
        let Ok(state) = host.state() else { continue };
        let done = state["ready"].as_str() == Some("complete")
            && (state["login_form"].as_bool() == Some(true) || state["app"].as_bool() == Some(true));
        last = Some(state);
        if done {
            break;
        }
    }
    match last {
        Some(s) => say!(
            "online: after {} ms: route {}, login form {}, captcha {}, QR {}, app {}",
            host::ms(started),
            s["route"].as_str().unwrap_or("?"),
            yes(s["login_form"].as_bool() == Some(true)),
            yes(s["captcha"].as_bool() == Some(true)),
            yes(s["qr"].as_bool() == Some(true)),
            yes(s["app"].as_bool() == Some(true))
        ),
        None => say!("online: the page never answered a script"),
    }
    procs::report(&host.env, "discord.com/login, hidden");
}

/// What the session folder holds now.
pub fn folder(dir: &Path) {
    let (files, bytes) = walk(dir);
    say!("session folder {}: {files} files, {:.1} MB", dir.display(), bytes as f64 / 1_048_576.0);
}

pub fn walk(dir: &Path) -> (u64, u64) {
    let mut out = (0, 0);
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            let (f, b) = walk(&entry.path());
            out = (out.0 + f, out.1 + b);
        } else {
            out = (out.0 + 1, out.1 + meta.len());
        }
    }
    out
}
