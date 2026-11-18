//! The stand-in discord.com (#197), for the Discord messages tests (`standin serve-discord-web`, the Dashboard's
//! `--discord-web-url`): a made-up sign-in form at `/login`, which takes any e-mail and password that are not empty,
//! sets a session cookie that lasts a year and sends the browser on to `/app`; and the app page, in Discord's DOM shape
//! as the spike found it (docs/research/discord-web-spike.md, 8): the server bar (`[data-list-id="guildsnav"]`) and a
//! DM list of links to `/channels/@me/<id>` with `name_` classes, which moves itself on to `/channels/@me` as Discord
//! does. Without the cookie every page but `/login` sends the browser to `/login`.
//!
//! The QR way, as Discord's login page has it: beside the form a QR-shaped block (made up, it encodes nothing), dropped
//! when the page is narrower than 830 CSS pixels as Discord drops its QR panel. A phone is played by the test: `POST
//! /standin/qr-confirm` is "scanned and confirmed", and the login page, which asks `/standin/qr` every half second,
//! then goes to `/qr-login`, which signs the browser in as the form does. The e-mail field takes focus by itself, as
//! Discord's does.
//!
//! The app page also carries a made-up Discord world for the Direct Messages tests (#198): the same fake webpack chunk
//! global and Flux stores as the page script's node tests (`js/fake_discord.js`: five DMs, one Group DM, unread counts,
//! a conversation of varied messages in the first), which the test drives by POST:
//!
//! - `POST /standin/dm-message?channel=<id>&text=<words>` a message arrives in that DM (from its first other member);
//!   unread goes up by one unless the DM is open in the page, and the DM moves to the top.
//! - `POST /standin/unread?channel=<id>&count=<n>` sets the DM's unread count (0 reads it).
//! - `POST /standin/ring?channel=<id>&on=1|0` starts or stops a DM call ringing the user.
//! - `GET /avatars/..`, `GET /channel-icons/..` a plain made-up picture, so `--discord-cdn-url` can name this stand-in.
//!
//! The channels are the three DM list links' ids ([`FRIENDS`]), `100000000000000010` (a Group DM of two) and
//! `100000000000000011` ("Made-up Group", with 60 older messages). The page polls `GET /standin/world?since=<n>` for
//! the controls, which are replayed when it loads again.
//!
//! Nothing of Discord's is copied: the words, the shapes and the names are made up. It never logs what was typed: a
//! request's path and whether it came signed in, nothing more (a control's query is not logged).
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The cookie the stand-in signs a browser in with.
pub const COOKIE: &str = "standin_session";

/// The made-up people in the DM list.
pub const FRIENDS: [(&str, &str); 3] = [
    ("100000000000000001", "Stand-in Friend"),
    ("100000000000000002", "Test Pal"),
    ("100000000000000003", "Made-up Mate"),
];

pub struct Web {
    pub addr: SocketAddr,
    /// Every request: its method and path, and whether it came signed in.
    pub asked: Arc<Mutex<Vec<String>>>,
    /// The controls so far, as JSON, in order: what the page's world replays.
    #[allow(dead_code)]
    pub world: Arc<Mutex<Vec<String>>>,
}

impl Web {
    /// Listens on 127.0.0.1 at `port` (0: any free one), answering on threads of its own for as long as it runs.
    pub fn start(port: u16) -> std::io::Result<Web> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let addr = listener.local_addr()?;
        let asked = Arc::new(Mutex::new(Vec::new()));
        // The phone has scanned the QR code and confirmed, and the login page has not yet signed in by it.
        let phone = Arc::new(AtomicBool::new(false));
        let kept = asked.clone();
        let world = Arc::new(Mutex::new(Vec::new()));
        let controls = world.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (kept, phone, controls) = (kept.clone(), phone.clone(), controls.clone());
                std::thread::spawn(move || {
                    let _ = answer(stream, &kept, &phone, &controls);
                });
            }
        });
        Ok(Web { addr, asked, world })
    }

    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }
}

const LOGIN: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Stand-in sign-in</title>
<style>body{background:#202225;color:#eee;font-family:sans-serif;margin:0;min-height:100vh;display:flex}
.box{margin:auto;display:flex;align-items:center;gap:32px;background:#2f3136;padding:28px 32px;border-radius:6px}
form{width:400px}h1{margin:0 0 6px}p{margin:0 0 4px}label{display:block;margin:10px 0 4px}
input{width:100%;padding:8px;font-size:16px;box-sizing:border-box}button{margin-top:16px;width:100%;padding:10px;font-size:16px}
input:focus,button:focus{outline:4px solid #fff}
.qr{width:240px;text-align:center}.qrCode-standin{width:176px;height:176px;margin:0 auto 12px;border:8px solid #fff;box-sizing:border-box;
background:repeating-conic-gradient(#000 0 25%,#fff 0 50%) 0 0/20px 20px}
@media (max-width:830px){.qr{display:none}}</style></head>
<body><div class="box"><form method="post" action="/login">
<h1>Stand-in sign-in</h1><p>A made-up page for the Dashboard's tests.</p>
<label for="email">E-mail</label><input id="email" name="email" type="text" autocomplete="off" autofocus>
<label for="password">Password</label><input id="password" name="password" type="password" autocomplete="off">
<button type="submit">Sign in</button>
</form>
<div class="qr"><div class="qrCode-standin" role="img" aria-label="Stand-in QR code"></div>
<h2>Or the phone</h2><p>A made-up code: the test plays the phone.</p></div></div>
<script>setInterval(() => fetch("/standin/qr").then((r) => r.text()).then((t) => {
  if (t === "confirmed") location.href = "/qr-login"; }).catch(() => {}), 500);</script>
</body></html>"#;

fn app() -> String {
    let rows: String = FRIENDS
        .iter()
        .map(|(id, name)| {
            format!(
                r#"<li role="listitem"><a href="/channels/@me/{id}" class="link_x1"><div class="name_x2">{name}</div></a></li>"#
            )
        })
        .collect();
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Stand-in app</title>
<style>body{{background:#313338;color:#eee;font-family:sans-serif;display:flex;margin:0;height:100vh}}
nav{{width:72px;background:#1e1f22}}aside{{width:240px;background:#2b2d31;padding:12px}}a{{color:#ddd}}</style></head>
<body><nav data-list-id="guildsnav" class="guilds_x0" aria-label="Servers"></nav>
<aside><h2>Direct Messages</h2><ul role="list" aria-label="Direct Messages">{rows}</ul></aside>
<main><p>Signed in to the stand-in.</p></main>
<script>if (location.pathname === "/app") history.replaceState(null, "", "/channels/@me");</script>
<script>{{FAKE}}</script>
<script>
// The made-up Discord world (js/fake_discord.js), and the test's controls replayed into it.
const world = installFakeDiscord(window);
window.__standinWorld = world;
window.dispatchEvent(new PopStateEvent("popstate"));
let seen = 0;
const poll = () => fetch("/standin/world?since=" + seen).then((r) => r.json()).then((j) => {{
  for (const c of j.commands) world.apply(c);
  seen = j.total;
}}).catch(() => {{}}).finally(() => setTimeout(poll, 300));
poll();
</script>
</body></html>"#
    )
    .replace("{FAKE}", FAKE)
}

/// The made-up Discord's stores, in the page's own scripts (and the Dashboard's page-script tests).
const FAKE: &str = include_str!("../js/fake_discord.js");

fn answer(
    stream: TcpStream,
    kept: &Mutex<Vec<String>>,
    scanned: &AtomicBool,
    controls: &Mutex<Vec<String>>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut words = line.split_whitespace();
    let method = words.next().unwrap_or("GET").to_string();
    let path = words.next().unwrap_or("/").to_string();
    let (mut signed, mut length) = (false, 0usize);
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let name = name.trim();
            if name.eq_ignore_ascii_case("cookie") {
                signed = value.split(';').any(|c| c.trim().starts_with(&format!("{COOKIE}=")));
            } else if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0).min(64 * 1024);
            }
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let route = path.split(['?', '#']).next().unwrap_or("/").to_string();
    if route != "/standin/qr" && route != "/standin/world" {
        kept.lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(format!("{method} {route} ({})", if signed { "signed in" } else { "not signed in" }));
    }
    let mut out = stream;
    match (method.as_str(), route.as_str()) {
        ("POST", "/login") => {
            if filled(&String::from_utf8_lossy(&body)) {
                let cookie = format!("{COOKIE}=stand-in; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax");
                redirect(&mut out, "/app", Some(&cookie))
            } else {
                redirect(&mut out, "/login", None)
            }
        }
        ("POST", "/standin/qr-confirm") => {
            scanned.store(true, Ordering::SeqCst);
            page(&mut out, "200 OK", "scanned and confirmed")
        }
        (_, "/standin/world") => {
            let since: usize = query(&path, "since").and_then(|v| v.parse().ok()).unwrap_or(0);
            let controls = controls.lock().unwrap_or_else(|p| p.into_inner());
            let news = controls.get(since..).unwrap_or(&[]).join(",");
            let json = format!(r#"{{"total":{},"commands":[{news}]}}"#, controls.len());
            respond(&mut out, "200 OK", "application/json", json.as_bytes())
        }
        ("POST", "/standin/dm-message") | ("POST", "/standin/unread") | ("POST", "/standin/ring") => {
            let channel = query(&path, "channel").unwrap_or_default();
            let command = match route.as_str() {
                "/standin/dm-message" => {
                    let text = query(&path, "text").unwrap_or_else(|| "a made-up message".into());
                    serde_json::json!({ "do": "say", "channel": channel, "text": text })
                }
                "/standin/unread" => {
                    let count: u32 = query(&path, "count").and_then(|v| v.parse().ok()).unwrap_or(0);
                    serde_json::json!({ "do": "unread", "channel": channel, "count": count })
                }
                _ => {
                    let on = matches!(query(&path, "on").as_deref(), Some("1" | "true" | "on"));
                    serde_json::json!({ "do": "ring", "channel": channel, "on": on })
                }
            };
            controls.lock().unwrap_or_else(|p| p.into_inner()).push(command.to_string());
            page(&mut out, "200 OK", "done")
        }
        ("GET", r) if r.starts_with("/avatars/") || r.starts_with("/channel-icons/") => {
            respond(&mut out, "200 OK", "image/png", &picture(r))
        }
        (_, "/standin/qr") => {
            page(&mut out, "200 OK", if scanned.load(Ordering::SeqCst) { "confirmed" } else { "waiting" })
        }
        (_, "/qr-login") if scanned.swap(false, Ordering::SeqCst) => {
            let cookie = format!("{COOKIE}=stand-in; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax");
            redirect(&mut out, "/app", Some(&cookie))
        }
        (_, "/login") if signed => redirect(&mut out, "/app", None),
        (_, "/login") => page(&mut out, "200 OK", LOGIN),
        (_, "/favicon.ico") => page(&mut out, "404 Not Found", ""),
        _ if signed => page(&mut out, "200 OK", &app()),
        _ => redirect(&mut out, "/login", None),
    }
}

/// Whether a form's body has an e-mail and a password, neither empty. What they are is never looked at again.
fn filled(body: &str) -> bool {
    let value = |name: &str| {
        body.split('&').find_map(|pair| pair.split_once('=').filter(|(n, _)| *n == name).map(|(_, v)| !v.is_empty()))
    };
    value("email") == Some(true) && value("password") == Some(true)
}

fn redirect(out: &mut TcpStream, to: &str, cookie: Option<&str>) -> std::io::Result<()> {
    let cookie = cookie.map(|c| format!("Set-Cookie: {c}\r\n")).unwrap_or_default();
    write!(out, "HTTP/1.1 303 See Other\r\nLocation: {to}\r\n{cookie}Content-Length: 0\r\nConnection: close\r\n\r\n")?;
    out.flush()
}

/// A query parameter's value, percent-decoded (`+` is a space).
fn query(path: &str, key: &str) -> Option<String> {
    let (_, query) = path.split_once('?')?;
    let query = query.split('#').next().unwrap_or("");
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == key).then(|| decode(v))
    })
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if bytes.get(i + 1..i + 3).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)) => {
                out.push(u8::from_str_radix(&value[i + 1..i + 3], 16).unwrap_or(b'?'));
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A plain made-up picture, one colour for each address (as the voice part's stand-in CDN serves).
fn picture(path: &str) -> Vec<u8> {
    let sum = path.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
    let colour = image::Rgba([(sum & 0xff) as u8, ((sum >> 8) & 0xff) as u8, ((sum >> 16) & 0xff) as u8, 255]);
    let image = image::RgbaImage::from_pixel(32, 32, colour);
    let mut png = std::io::Cursor::new(Vec::new());
    let _ = image.write_to(&mut png, image::ImageFormat::Png);
    png.into_inner()
}

fn respond(out: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    out.write_all(body)?;
    out.flush()
}

fn page(out: &mut TcpStream, status: &str, body: &str) -> std::io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(base: &str, path: &str, cookie: Option<&str>, body: Option<&str>) -> String {
        let addr = base.trim_start_matches("http://");
        let mut stream = TcpStream::connect(addr).unwrap();
        let method = if body.is_some() { "POST" } else { "GET" };
        let cookie = cookie.map(|c| format!("Cookie: {c}\r\n")).unwrap_or_default();
        let body = body.unwrap_or("");
        write!(stream, "{method} {path} HTTP/1.1\r\nHost: x\r\n{cookie}Content-Length: {}\r\n\r\n{body}", body.len())
            .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        answer
    }

    #[test]
    fn a_filled_form_signs_in_and_the_app_page_has_discords_shape() {
        let web = Web::start(0).unwrap();
        let base = web.base();
        assert!(get(&base, "/app", None, None).contains("Location: /login"));
        assert!(get(&base, "/login", None, None).contains(r#"type="password""#));
        assert!(get(&base, "/login", None, Some("email=&password=x")).contains("Location: /login"));
        let signed = get(&base, "/login", None, Some("email=a&password=b"));
        assert!(signed.contains("Location: /app") && signed.contains(&format!("Set-Cookie: {COOKIE}=")));
        let app = get(&base, "/app", Some(&format!("{COOKIE}=stand-in")), None);
        assert!(app.contains(r#"data-list-id="guildsnav""#));
        assert_eq!(app.matches(r#"href="/channels/@me/"#).count(), FRIENDS.len());
        assert!(app.contains(r#"class="name_"#) && app.contains("history.replaceState"));
        let asked = web.asked.lock().unwrap().clone();
        assert!(asked.iter().all(|line| !line.contains("password=")), "nothing typed is kept");
    }

    #[test]
    fn the_app_page_carries_the_made_up_discord_world_and_the_controls_drive_it() {
        let web = Web::start(0).unwrap();
        let base = web.base();
        let signed = format!("{COOKIE}=stand-in");
        let app = get(&base, "/app", Some(&signed), None);
        assert!(app.contains("webpackChunkdiscord_app") && app.contains("installFakeDiscord(window)"));
        assert!(app.contains("/standin/world?since="), "the page polls for the controls");
        assert!(!app.contains("{FAKE}") && !app.contains("{{"), "the template is filled");
        let world = |since: usize| get(&base, &format!("/standin/world?since={since}"), None, None);
        assert!(world(0).ends_with(r#"{"total":0,"commands":[]}"#));
        get(&base, "/standin/dm-message?channel=100000000000000002&text=are+you%20on%3F", None, Some(""));
        get(&base, "/standin/unread?channel=100000000000000001&count=4", None, Some(""));
        get(&base, "/standin/ring?channel=100000000000000003&on=1", None, Some(""));
        get(&base, "/standin/ring?channel=100000000000000003&on=0", None, Some(""));
        let all = world(0);
        let json: serde_json::Value = serde_json::from_str(all.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(json["total"], 4);
        let commands = json["commands"].as_array().unwrap();
        assert_eq!(
            commands[0],
            serde_json::json!({"do": "say", "channel": "100000000000000002", "text": "are you on?"})
        );
        assert_eq!(commands[1], serde_json::json!({"do": "unread", "channel": "100000000000000001", "count": 4}));
        assert_eq!(commands[2]["on"], true);
        assert_eq!(commands[3]["on"], false);
        assert!(world(3).contains(r#""total":4,"commands":[{"channel":"100000000000000003","do":"ring","on":false}]"#));
        assert!(world(9).ends_with(r#""commands":[]}"#));
        let asked = web.asked.lock().unwrap().clone();
        assert!(asked.iter().all(|l| !l.contains("are you") && !l.contains("/standin/world")), "{asked:?}");
    }

    #[test]
    fn avatars_and_icons_are_plain_pictures() {
        let web = Web::start(0).unwrap();
        let mut stream = TcpStream::connect(web.base().trim_start_matches("http://")).unwrap();
        write!(stream, "GET /avatars/300000000000000001/ha1.png?size=128 HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).unwrap();
        let at = answer.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert!(String::from_utf8_lossy(&answer[..at]).contains("image/png"));
        assert_eq!(&answer[at..at + 8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn the_phone_scanning_the_qr_code_signs_the_page_in() {
        let web = Web::start(0).unwrap();
        let base = web.base();
        let login = get(&base, "/login", None, None);
        assert!(login.contains("qrCode-standin") && login.contains("max-width:830px") && login.contains("autofocus"));
        assert!(get(&base, "/qr-login", None, None).contains("Location: /login"), "not before the phone");
        assert!(get(&base, "/standin/qr", None, None).ends_with("waiting"));
        get(&base, "/standin/qr-confirm", None, Some(""));
        assert!(get(&base, "/standin/qr", None, None).ends_with("confirmed"));
        let signed = get(&base, "/qr-login", None, None);
        assert!(signed.contains("Location: /app") && signed.contains(&format!("Set-Cookie: {COOKIE}=")));
        assert!(get(&base, "/standin/qr", None, None).ends_with("waiting"), "one scan, one sign-in");
    }
}
