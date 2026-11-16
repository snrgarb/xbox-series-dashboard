//! A stand-in for SteamGridDB on 127.0.0.1, for tests and the VM's `standin serve-art`: no test and no payload asks
//! the real service. It answers the requests [`super::steamgriddb::Service`] makes, as the real one was seen to
//! (docs/sources.md, "Art"), from a [`Script`], and keeps every request it was sent, so a test can say what was
//! asked and with which key.
//!
//! Its pictures' addresses are its own (`/pictures/<game>/<kind>.png`), as the real one's are its CDN's: a
//! request for one with a key on it is kept as such, so a test can say none went there.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use super::cache::Kind;
use super::steamgriddb::plain_name;

/// What a picture is: its bytes for a game's id and a kind.
pub type Painter = Arc<dyn Fn(u64, Kind) -> Vec<u8> + Send + Sync>;

/// A game the stand-in knows.
#[derive(Debug, Clone)]
pub struct FakeGame {
    pub id: u64,
    pub name: String,
    /// The stores' ids it is known by: `("steam", "620")`, `("egs", "Fortnite")`.
    pub platforms: Vec<(String, String)>,
    /// The kinds it has a picture of.
    pub kinds: Vec<Kind>,
}

/// How the stand-in answers.
#[derive(Clone)]
pub struct Script {
    /// The one key it takes; any other is 401.
    pub key: String,
    pub games: Vec<FakeGame>,
    /// A search for any name finds a game of that very name, with every picture: the VM's stand-in, which serves
    /// whatever fake store it is shown.
    pub any_name: bool,
    /// The first this many requests to the API are answered 429, `Retry-After: 1`.
    pub busy: u32,
    pub picture: Painter,
    /// The stores' own art online, under `/stores` (`Hosts::all_at(<url>/stores)`): none without.
    pub stores: Option<StoreScript>,
}

impl Script {
    pub fn new(key: &str, picture: Painter) -> Self {
        Self { key: key.into(), games: Vec::new(), any_name: false, busy: 0, picture, stores: None }
    }
}

/// What a store's picture is: its bytes for a store's key, a game's id there and a kind.
pub type StorePainter = Arc<dyn Fn(&str, &str, Kind) -> Vec<u8> + Send + Sync>;

/// How the stand-ins of the stores' own art online answer, as the real ones were seen to (docs/sources.md, "Art"):
/// Steam's `GetItems` and CDN, the Microsoft Store's display catalogue and its images, GOG's `v2/games` and its
/// images. Asked with no key.
#[derive(Clone)]
pub struct StoreScript {
    /// The games each store knows, `("xbox", "9ZZZMADEUP02")`, and the kinds it has pictures of.
    pub games: Vec<(String, String, Vec<Kind>)>,
    /// Every id is a game, with every kind its store has online (Steam's and GOG's no square): the VM's stand-in.
    pub any_id: bool,
    /// The first this many requests to a catalogue are answered 429, `Retry-After: 1`.
    pub busy: u32,
    pub picture: StorePainter,
}

impl StoreScript {
    pub fn new(picture: StorePainter) -> Self {
        Self { games: Vec::new(), any_id: false, busy: 0, picture }
    }

    /// The kinds the store has of the game, if it knows it.
    fn kinds(&self, store: &str, id: &str) -> Option<Vec<Kind>> {
        let known = self.games.iter().find(|(s, i, _)| s == store && i == id).map(|(_, _, kinds)| kinds.clone());
        known.or_else(|| {
            self.any_id.then(|| match store {
                "xbox" => Kind::ALL.to_vec(),
                _ => vec![Kind::Cover, Kind::Hero, Kind::Header],
            })
        })
    }
}

/// One request it was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The path and query, as sent.
    pub path: String,
    /// Its `Authorization` header, if it had one.
    pub authorization: Option<String>,
}

pub struct FakeService {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl FakeService {
    /// Listens on 127.0.0.1 at `port` (0: any free one) and answers on a thread of its own for as long as the
    /// program runs.
    pub fn start(port: u16, script: Script) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let addr = listener.local_addr()?;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stores_busy = script.stores.as_ref().map_or(0, |stores| stores.busy);
        let shared = Arc::new(Shared {
            busy: AtomicU32::new(script.busy),
            stores_busy: AtomicU32::new(stores_busy),
            script,
            base: format!("http://{addr}"),
            names: Mutex::new(HashMap::new()),
        });
        let kept = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (shared, kept) = (shared.clone(), kept.clone());
                std::thread::spawn(move || {
                    let _ = answer(stream, &shared, &kept);
                });
            }
        });
        Ok(Self { addr, requests })
    }

    /// Its address, as the Dashboard is given it (`--art-service`).
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The stores' stand-ins' address, as the Dashboard is given it (`--store-art-service`).
    pub fn stores_url(&self) -> String {
        format!("http://{}/stores", self.addr)
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Every request so far, in the order they came.
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

struct Shared {
    script: Script,
    base: String,
    busy: AtomicU32,
    stores_busy: AtomicU32,
    /// With `any_name`: each name searched for, by the id given it.
    names: Mutex<HashMap<String, u64>>,
}

fn answer(stream: TcpStream, shared: &Shared, kept: &Mutex<Vec<Request>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut authorization = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.trim().eq_ignore_ascii_case("authorization") {
                authorization = Some(value.trim().to_string());
            }
        }
    }
    kept.lock().unwrap().push(Request { path: path.clone(), authorization: authorization.clone() });
    let (status, kind, body, extra) = route(shared, &path, authorization.as_deref());
    let mut stream = stream;
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()
}

type Reply = (&'static str, &'static str, Vec<u8>, &'static str);

/// Takes one from a count that is not yet nought: whether it did.
fn take_one(count: &AtomicU32) -> bool {
    let mut n = count.load(Ordering::Relaxed);
    while n > 0 {
        match count.compare_exchange(n, n - 1, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(now) => n = now,
        }
    }
    false
}

fn json(status: &'static str, value: serde_json::Value) -> Reply {
    (status, "application/json", value.to_string().into_bytes(), "")
}

fn route(shared: &Shared, path: &str, authorization: Option<&str>) -> Reply {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let parts: Vec<String> = path.trim_start_matches('/').split('/').map(decode).collect();
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    let script = &shared.script;
    if let (["stores", rest @ ..], Some(stores)) = (&parts[..], &script.stores) {
        return store_route(shared, stores, rest, query);
    }
    if let ["pictures", game, file] = parts[..] {
        let kind = Kind::ALL.into_iter().find(|kind| file.starts_with(kind.name()));
        return match (game.parse::<u64>(), kind) {
            (Ok(game), Some(kind)) => ("200 OK", "image/png", (script.picture)(game, kind), ""),
            _ => ("404 Not Found", "text/plain", b"no such picture".to_vec(), ""),
        };
    }
    if authorization != Some(format!("Bearer {}", script.key).as_str()) {
        return json("401 Unauthorized", serde_json::json!({"success": false, "errors": ["Invalid API key"]}));
    }
    if take_one(&shared.busy) {
        let body = serde_json::json!({"success": false, "errors": ["Too many requests"]}).to_string();
        return ("429 Too Many Requests", "application/json", body.into_bytes(), "Retry-After: 1\r\n");
    }
    let not_found = || json("404 Not Found", serde_json::json!({"success": false, "errors": ["Game not found"]}));
    let game_json =
        |game: &FakeGame| serde_json::json!({"id": game.id, "name": game.name, "verified": true, "types": []});
    match parts[..] {
        ["games", platform, id] => {
            let found = script.games.iter().find(|g| g.platforms.iter().any(|(p, i)| p == platform && i == id));
            match found {
                Some(game) => json("200 OK", serde_json::json!({"success": true, "data": game_json(game)})),
                None => not_found(),
            }
        }
        ["search", "autocomplete", name] => {
            let wanted = plain_name(name);
            let mut found: Vec<serde_json::Value> = script
                .games
                .iter()
                .filter(|game| !wanted.is_empty() && plain_name(&game.name).contains(&wanted))
                .map(game_json)
                .collect();
            if script.any_name && found.is_empty() && !wanted.is_empty() {
                let mut names = shared.names.lock().unwrap();
                let next = 1_000_000 + names.len() as u64;
                let id = *names.entry(wanted).or_insert(next);
                found.push(serde_json::json!({"id": id, "name": name, "verified": true, "types": []}));
            }
            json("200 OK", serde_json::json!({"success": true, "data": found}))
        }
        [what @ ("grids" | "heroes"), "game", id] => {
            let Ok(id) = id.parse::<u64>() else { return not_found() };
            let kind = match what {
                "heroes" => Kind::Hero,
                _ if query.contains("600x900") => Kind::Cover,
                _ if query.contains("1024x1024") => Kind::Square,
                _ => Kind::Header,
            };
            let known = script.games.iter().find(|game| game.id == id);
            let has = match known {
                Some(game) => game.kinds.contains(&kind),
                None => script.any_name && shared.names.lock().unwrap().values().any(|&named| named == id),
            };
            if known.is_none() && !has {
                return not_found();
            }
            let pictures: Vec<serde_json::Value> = has
                .then(|| {
                    serde_json::json!({
                        "id": id * 10 + kind as u64, "score": 0, "style": "alternate", "nsfw": false, "humor": false,
                        "epilepsy": false, "mime": "image/png", "language": "en",
                        "url": format!("{}/pictures/{id}/{}.png", shared.base, kind.name()),
                        "thumb": format!("{}/pictures/{id}/{}.png", shared.base, kind.name()),
                    })
                })
                .into_iter()
                .collect();
            json("200 OK", serde_json::json!({"success": true, "page": 0, "total": pictures.len(), "data": pictures}))
        }
        _ => not_found(),
    }
}

/// The stores' stand-ins, under `/stores`: `parts` the rest of the path.
fn store_route(shared: &Shared, stores: &StoreScript, parts: &[&str], query: &str) -> Reply {
    let base = format!("{}/stores", shared.base);
    let png = |store: &str, id: &str, kind: Kind| ("200 OK", "image/png", (stores.picture)(store, id, kind), "");
    let missing = || ("404 Not Found", "text/plain", b"no such thing".to_vec(), "");
    let catalogue = matches!(parts.first(), Some(&("steam-api" | "microsoft" | "gog")));
    if catalogue && take_one(&shared.stores_busy) {
        return ("429 Too Many Requests", "text/plain", b"slow down".to_vec(), "Retry-After: 1\r\n");
    }
    let query = decode(query);
    let param = |name: &str| {
        query.split('&').find_map(|pair| pair.strip_prefix(name)?.strip_prefix('=')).unwrap_or_default().to_string()
    };
    match parts {
        ["steam-api", "IStoreBrowseService", "GetItems", "v1", ..] => {
            // `input_json={"ids":[{"appid":620}],...}`: the appid's digits.
            let input = param("input_json");
            let id: String =
                input.split("\"appid\":").nth(1).unwrap_or_default().chars().take_while(char::is_ascii_digit).collect();
            let item = match stores.kinds("steam", &id) {
                Some(kinds) => {
                    let mut assets =
                        serde_json::json!({"asset_url_format": format!("steam/apps/{id}/${{FILENAME}}?t=1")});
                    for (kind, name, file) in [
                        (Kind::Cover, "library_capsule", "0123abcd/library_600x900.png"),
                        (Kind::Hero, "library_hero", "library_hero.png"),
                        (Kind::Header, "header", "header.png"),
                    ] {
                        if kinds.contains(&kind) {
                            assets[name] = file.into();
                        }
                    }
                    serde_json::json!({"item_type": 0, "id": id.parse::<u64>().unwrap_or(0), "success": 1, "assets": assets})
                }
                None => serde_json::json!({"id": id.parse::<u64>().unwrap_or(0), "success": 15}),
            };
            json("200 OK", serde_json::json!({"response": {"store_items": [item]}}))
        }
        ["steam-cdn", "steam", "apps", id, .., file] => {
            let kind = if file.starts_with("library_600x900") {
                Kind::Cover
            } else if file.starts_with("library_hero") {
                Kind::Hero
            } else {
                Kind::Header
            };
            match stores.kinds("steam", id) {
                Some(kinds) if kinds.contains(&kind) => png("steam", id, kind),
                _ => missing(),
            }
        }
        ["microsoft", "v7.0", "products"] => {
            let id = param("bigIds");
            let products: Vec<serde_json::Value> = stores
                .kinds("xbox", &id)
                .map(|kinds| {
                    let images: Vec<serde_json::Value> = [
                        (Kind::Cover, "Poster"),
                        (Kind::Hero, "SuperHeroArt"),
                        (Kind::Header, "TitledHeroArt"),
                        (Kind::Square, "BoxArt"),
                    ]
                    .into_iter()
                    .filter(|(kind, _)| kinds.contains(kind))
                    .map(|(_, purpose)| {
                        let uri = format!("{}/ms-images/{id}/{purpose}.png", base.trim_start_matches("http:"));
                        serde_json::json!({"ImagePurpose": purpose, "Uri": uri, "Width": 100, "Height": 100})
                    })
                    .collect();
                    serde_json::json!({"ProductId": id, "LocalizedProperties": [{"Images": images}]})
                })
                .into_iter()
                .collect();
            json("200 OK", serde_json::json!({"Products": products}))
        }
        ["ms-images", id, file] => {
            let kind = match file.trim_end_matches(".png") {
                "Poster" => Kind::Cover,
                "SuperHeroArt" => Kind::Hero,
                "TitledHeroArt" => Kind::Header,
                _ => Kind::Square,
            };
            png("xbox", id, kind)
        }
        ["gog", "v2", "games", id] => match stores.kinds("gog", id) {
            Some(kinds) => {
                let image = |kind: Kind| format!("{base}/gog-images/{id}/{}.png", kind.name());
                let mut links = serde_json::json!({});
                if kinds.contains(&Kind::Cover) {
                    links["boxArtImage"] = serde_json::json!({"href": image(Kind::Cover)});
                }
                if kinds.contains(&Kind::Hero) {
                    links["galaxyBackgroundImage"] = serde_json::json!({"href": image(Kind::Hero)});
                }
                let mut answer = serde_json::json!({"_links": links});
                if kinds.contains(&Kind::Header) {
                    let card = format!("{base}/gog-images/{id}/header_{{formatter}}.png");
                    answer["_embedded"] = serde_json::json!({"product": {"_links": {"image": {"href": card}}}});
                }
                json("200 OK", answer)
            }
            None => json("404 Not Found", serde_json::json!({"error": "not_found"})),
        },
        ["gog-images", id, file] => {
            let kind = Kind::ALL.into_iter().find(|kind| file.starts_with(kind.name())).unwrap_or(Kind::Cover);
            png("gog", id, kind)
        }
        _ => missing(),
    }
}

/// `%XX` back to bytes.
fn decode(text: &str) -> String {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((&first, tail)) = rest.split_first() {
        let hex = tail.get(..2).and_then(|hex| std::str::from_utf8(hex).ok());
        match (first, hex.and_then(|hex| u8::from_str_radix(hex, 16).ok())) {
            (b'%', Some(byte)) => {
                bytes.push(byte);
                rest = &tail[2..];
            }
            _ => {
                bytes.push(first);
                rest = tail;
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
