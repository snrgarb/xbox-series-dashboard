//! The stand-in discord.com (#197), for the Discord messages tests (`standin serve-discord-web`, the Dashboard's
//! `--discord-web-url`): a made-up sign-in form at `/login`, which takes any e-mail and password that are not empty,
//! sets a session cookie that lasts a year and sends the browser on to `/app`; and the app page, in Discord's DOM shape
//! as the spike found it (docs/research/discord-web-spike.md, 8): the server bar (`[data-list-id="guildsnav"]`) and a
//! DM list of links to `/channels/@me/<id>` with `name_` classes, which moves itself on to `/channels/@me` as Discord
//! does. Without the cookie every page but `/login` sends the browser to `/login`.
//!
//! Nothing of Discord's is copied: the words, the shapes and the names are made up. It never logs what was typed: a
//! request's path and whether it came signed in, nothing more.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
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
}

impl Web {
    /// Listens on 127.0.0.1 at `port` (0: any free one), answering on threads of its own for as long as it runs.
    pub fn start(port: u16) -> std::io::Result<Web> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let addr = listener.local_addr()?;
        let asked = Arc::new(Mutex::new(Vec::new()));
        let kept = asked.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let kept = kept.clone();
                std::thread::spawn(move || {
                    let _ = answer(stream, &kept);
                });
            }
        });
        Ok(Web { addr, asked })
    }

    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }
}

const LOGIN: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Stand-in sign-in</title>
<style>body{background:#202225;color:#eee;font-family:sans-serif;display:flex;justify-content:center;padding-top:12vh}
form{background:#2f3136;padding:32px;width:420px;border-radius:6px}label{display:block;margin:14px 0 6px}
input{width:100%;padding:10px;font-size:18px;box-sizing:border-box}button{margin-top:22px;width:100%;padding:12px;font-size:18px}
input:focus,button:focus{outline:4px solid #fff}</style></head>
<body><form method="post" action="/login">
<h1>Stand-in sign-in</h1><p>A made-up page for the Dashboard's tests.</p>
<label for="email">E-mail</label><input id="email" name="email" type="text" autocomplete="off">
<label for="password">Password</label><input id="password" name="password" type="password" autocomplete="off">
<button type="submit">Sign in</button>
</form></body></html>"#;

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
</body></html>"#
    )
}

fn answer(stream: TcpStream, kept: &Mutex<Vec<String>>) -> std::io::Result<()> {
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
    kept.lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(format!("{method} {route} ({})", if signed { "signed in" } else { "not signed in" }));
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
}
