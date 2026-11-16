//! SteamGridDB's API, version 2 (`https://www.steamgriddb.com/api/v2`), as far as the art lookup needs it: find a
//! game, by its store's id where the API knows the store and else by its name, then its first static, safe picture
//! of each kind. Every request carries the user's key as `Authorization: Bearer <key>`; the key goes nowhere else
//! (not to the pictures' own addresses, not into an error) and only the game's store id or name is sent.
//!
//! Each reply is `{"success": true, "data": ...}`, or `{"success": false, "errors": [...]}` with 404 for a game it
//! does not know and 401 for a key it does not take. What it was checked against is in docs/sources.md, "Art".
use std::time::Duration;

use serde_json::Value;

use super::cache::Kind;

/// The service's own address.
pub const SERVICE: &str = "https://www.steamgriddb.com/api/v2";

/// The largest picture taken.
const MOST_BYTES: u64 = 24 << 20;

/// Why a request came to nothing. None of them carries the key, or anything of the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// 401: the key is wrong, or revoked.
    KeyRefused,
    /// 429, or 503: asked to wait, for this long if it said.
    Busy(Option<Duration>),
    /// Anything else: the network, the service's own trouble, a reply not as expected. Said in the log in these
    /// words.
    Failed(String),
}

/// What the store's key says the game is on SteamGridDB, where it knows the store: Steam's appid, Epic's
/// `AppName`, GOG's product id. The others' ids (EA's content ids, Ubisoft's and Battle.net's) were not found
/// under any platform name, so their games are found by name.
pub fn platform(source: &str) -> Option<&'static str> {
    match source {
        "steam" => Some("steam"),
        "epic" => Some("egs"),
        "gog" => Some("gog"),
        _ => None,
    }
}

/// A name as names are matched: lower case, its marks (™ ® ©) gone, `&` as "and", and every run of anything but
/// letters and digits one space. "DOOM Eternal™" and "Doom: Eternal" are one name; "Doom" and "Doom 3" are not.
pub fn plain_name(name: &str) -> String {
    let name = name.replace('&', " and ");
    let mut out = String::new();
    for c in name.chars().filter(|c| !matches!(c, '™' | '®' | '©')) {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.trim().to_string()
}

/// The game a search's results are, by the conservative rule: only one whose name is the game's (`plain_name`),
/// the first verified one among those, else the first. A near name is no match: no art is better than another
/// game's.
pub fn best_match(name: &str, results: &[(u64, String, bool)]) -> Option<u64> {
    let wanted = plain_name(name);
    if wanted.is_empty() {
        return None;
    }
    let same: Vec<&(u64, String, bool)> = results.iter().filter(|(_, n, _)| plain_name(n) == wanted).collect();
    same.iter().find(|(_, _, verified)| *verified).or(same.first()).map(|(id, _, _)| *id)
}

pub struct Service {
    base: String,
    key: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the key.
        f.debug_struct("Service").field("base", &self.base).finish_non_exhaustive()
    }
}

impl Service {
    /// The service at `base` (`SERVICE`, or a test's), asked with `key`.
    pub fn new(base: &str, key: &str) -> Self {
        Self { base: base.trim_end_matches('/').to_string(), key: key.to_string(), agent: agent() }
    }

    /// The API's reply to `path`, its `data`; None for 404.
    fn get(&self, path: &str) -> Result<Option<Value>, Error> {
        let request =
            self.agent.get(format!("{}/{path}", self.base)).header("Authorization", format!("Bearer {}", self.key));
        let mut response = request.call().map_err(|e| transport(e, NAME))?;
        let status = response.status().as_u16();
        match status {
            200..=299 => {}
            401 | 403 => return Err(Error::KeyRefused),
            404 => return Ok(None),
            429 | 503 => return Err(Error::Busy(retry_after(&response))),
            _ => return Err(Error::Failed(format!("SteamGridDB answered {status}"))),
        }
        let text = response.body_mut().read_to_string().map_err(|e| transport(e, NAME))?;
        let reply: Value =
            serde_json::from_str(&text).map_err(|_| Error::Failed("SteamGridDB's reply is not JSON".into()))?;
        if reply.get("success").and_then(Value::as_bool) != Some(true) {
            return Err(Error::Failed("SteamGridDB said it did not succeed".into()));
        }
        Ok(reply.get("data").cloned())
    }

    /// The game's id on SteamGridDB: by its store's id where the API knows the store, else (or when that is not
    /// known there) by its name. None when there is no such game, or no name that matches.
    pub fn find_game(&self, source: &str, source_id: &str, name: &str) -> Result<Option<u64>, Error> {
        if let Some(platform) = platform(source).filter(|_| !source_id.is_empty()) {
            if let Some(game) = self.get(&format!("games/{platform}/{}", segment(source_id)))? {
                if let Some(id) = game.get("id").and_then(Value::as_u64) {
                    return Ok(Some(id));
                }
            }
        }
        if plain_name(name).is_empty() {
            return Ok(None);
        }
        let Some(found) = self.get(&format!("search/autocomplete/{}", segment(name)))? else { return Ok(None) };
        let results: Vec<(u64, String, bool)> = found
            .as_array()
            .map(|games| {
                games
                    .iter()
                    .filter_map(|game| {
                        let id = game.get("id")?.as_u64()?;
                        let name = game.get("name")?.as_str()?.to_string();
                        Some((id, name, game.get("verified").and_then(Value::as_bool).unwrap_or(false)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(best_match(name, &results))
    }

    /// The address of the game's first picture of this kind that is static, neither adult nor a joke nor a
    /// flasher, and a PNG, JPEG or WebP; None when it has none.
    pub fn picture(&self, game: u64, kind: Kind) -> Result<Option<String>, Error> {
        const SAFE: &str = "types=static&nsfw=false&humor=false&epilepsy=false&mimes=image/png,image/jpeg,image/webp";
        let path = match kind {
            Kind::Cover => format!("grids/game/{game}?dimensions=600x900&{SAFE}&limit=5"),
            Kind::Hero => format!("heroes/game/{game}?{SAFE}&limit=5"),
            Kind::Header => format!("grids/game/{game}?dimensions=920x430,460x215&{SAFE}&limit=5"),
            Kind::Square => format!("grids/game/{game}?dimensions=1024x1024,512x512&{SAFE}&limit=5"),
        };
        let Some(pictures) = self.get(&path)? else { return Ok(None) };
        let safe = |picture: &&Value| {
            let no = |field: &str| picture.get(field).and_then(Value::as_bool) != Some(true);
            let mime = picture.get("mime").and_then(Value::as_str).unwrap_or_default();
            no("nsfw") && no("humor") && no("epilepsy") && matches!(mime, "image/png" | "image/jpeg" | "image/webp")
        };
        let url = pictures.as_array().and_then(|all| all.iter().find(safe)).and_then(|p| p.get("url")?.as_str());
        Ok(url.map(str::to_string))
    }

    /// A picture's bytes, from its own address: no key goes with it.
    pub fn download(&self, url: &str) -> Result<Vec<u8>, Error> {
        download(&self.agent, url)
    }
}

/// The service's name, as the log says it.
pub const NAME: &str = "SteamGridDB";

/// An agent as every request for art is made with: a time limit, the program's name, and an answer of any status
/// given back to be read.
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(concat!("xbox-series-dashboard/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// A picture's bytes, from its own address, with nothing else sent: at most [`MOST_BYTES`].
pub fn download(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, Error> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Error::Failed("a picture's address is not a web address".into()));
    }
    let mut response = agent.get(tidy(url)).call().map_err(|e| transport(e, "a picture's server"))?;
    match response.status().as_u16() {
        200..=299 => {}
        429 | 503 => return Err(Error::Busy(retry_after(&response))),
        status => return Err(Error::Failed(format!("a picture's server answered {status}"))),
    }
    let body = response.body_mut().with_config().limit(MOST_BYTES).read_to_vec();
    body.map_err(|e| transport(e, "a picture's server"))
}

/// An address as a request can carry it: a store's catalogue may give one with spaces or other letters in it
/// (Epic's does: `.../Landscaped Store-2560x1440-<hash>.jpg`, seen 2026-11-16), which a browser would mend. Every
/// byte an address can't hold is written `%XX`; what is already an address is left as it is.
pub fn tidy(url: &str) -> String {
    let mut tidy = String::with_capacity(url.len());
    for byte in url.bytes() {
        let fine = byte.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=%".contains(&byte);
        if fine {
            tidy.push(byte as char);
        } else {
            tidy.push_str(&format!("%{byte:02X}"));
        }
    }
    tidy
}

/// How long a reply asks to be waited for: `Retry-After` in seconds (a date is not read, and waits the default).
pub fn retry_after(response: &ureq::http::Response<ureq::Body>) -> Option<Duration> {
    let value = response.headers().get("retry-after")?.to_str().ok()?;
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// What went wrong on the way to `who`, in words that name no address: ureq's own may carry the request's.
pub fn transport(error: ureq::Error, who: &str) -> Error {
    let what = match error {
        ureq::Error::Timeout(_) => format!("{who} took too long to answer"),
        ureq::Error::HostNotFound => format!("{who} could not be found: is the PC online?"),
        ureq::Error::ConnectionFailed | ureq::Error::Io(_) => format!("{who} could not be reached"),
        ureq::Error::BodyExceedsLimit(_) => "a picture was too large".to_string(),
        ureq::Error::BadUri(_) | ureq::Error::Http(_) => format!("{who}'s address is not one a request can be made to"),
        _ => format!("{who}'s answer could not be read"),
    };
    Error::Failed(what)
}

/// Text as one part of an address's path: every byte but letters, digits and `-._~` as `%XX`.
pub(crate) fn segment(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_with_spaces_or_other_letters_is_made_one_a_request_can_carry() {
        assert_eq!(
            tidy("https://cdn.example/item/A Game Store-2560x1440-79aa.jpg"),
            "https://cdn.example/item/A%20Game%20Store-2560x1440-79aa.jpg"
        );
        assert_eq!(tidy("https://cdn.example/é.png"), "https://cdn.example/%C3%A9.png");
        let whole = "https://cdn.example/a%20b/c.jpg?w=600&h=900#x";
        assert_eq!(tidy(whole), whole);
    }

    #[test]
    fn names_match_only_when_they_are_the_same_name() {
        assert_eq!(plain_name("DOOM Eternal™"), "doom eternal");
        assert_eq!(plain_name("Doom: Eternal"), "doom eternal");
        assert_eq!(plain_name("Ori & the Blind Forest"), "ori and the blind forest");
        let results = [
            (1, "Hades II".to_string(), true),
            (2, "Hades' Star".to_string(), true),
            (3, "HADES".to_string(), false),
            (4, "Hades".to_string(), true),
        ];
        // The verified one of the two that are the name.
        assert_eq!(best_match("Hades", &results), Some(4));
        assert_eq!(best_match("Hades: Battle Out of Hell", &results), None);
        assert_eq!(best_match("Hades 2", &results), None);
        assert_eq!(best_match("Hades II", &results[..1]), Some(1));
        assert_eq!(best_match("", &results), None);
        assert_eq!(best_match("™", &results), None);
    }

    #[test]
    fn a_name_is_one_part_of_the_path() {
        assert_eq!(segment("Ori & the Blind Forest"), "Ori%20%26%20the%20Blind%20Forest");
        assert_eq!(segment("a/b?c#d"), "a%2Fb%3Fc%23d");
    }

    #[test]
    fn the_service_never_shows_its_key() {
        let service = Service::new("http://127.0.0.1:1", "secret-key-123");
        assert!(!format!("{service:?}").contains("secret-key-123"));
    }
}
