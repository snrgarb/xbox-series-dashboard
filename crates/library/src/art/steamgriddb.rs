//! SteamGridDB's API, version 2 (`https://www.steamgriddb.com/api/v2`), as far as the art lookup needs it: find a
//! game, by its store's id where the API knows the store and else by its name, then its first static, safe picture
//! of each kind. Every request carries the user's key as `Authorization: Bearer <key>`; the key goes nowhere else
//! (not to the pictures' own addresses, not into an error) and only the game's store id or name is sent.
//!
//! Each reply is `{"success": true, "data": ...}`, or `{"success": false, "errors": [...]}` with 404 for a game it
//! does not know and 401 for a key it does not take. What it was checked against is in docs/sources.md, "Art".
use std::time::Duration;

use serde_json::Value;

use model::art_match::{Hit, MOST_HITS};
use model::art_picker::{Candidate, Reply, Tab, LIMIT};

use super::cache::Kind;

/// The service's own address.
pub const SERVICE: &str = "https://www.steamgriddb.com/api/v2";

/// What every list of pictures is narrowed to: static, neither adult nor a joke nor a flasher, PNG, JPEG or WebP.
const SAFE: &str = "types=static&nsfw=false&humor=false&epilepsy=false&mimes=image/png,image/jpeg,image/webp";

/// The style of grid and hero asked for first: the plainest the API has (`Styles` and `StylesHero` in its OpenAPI
/// description, docs/research/art-sources.md; only logos and icons have an `official` style).
pub const PLAIN: &str = "alternate";

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

pub use super::names::plain_name;

/// The game a search's results are, by the matching rule (`names::pick`): only one that is the game's name once
/// written plainly and set apart from its edition, the very name before an edition of it, a verified one before
/// another, else the first. A near name is no match: no art is better than another game's.
pub fn best_match(name: &str, results: &[(u64, String, bool)]) -> Option<u64> {
    let ranked: Vec<(String, u64, u8)> =
        results.iter().map(|(id, n, verified)| (n.clone(), *id, u8::from(!*verified))).collect();
    super::names::pick(name, &ranked).copied()
}

/// A hit's id as SteamGridDB's own number: its search gives no other.
fn hit_id(hit: &Hit) -> u64 {
    hit.id.parse().unwrap_or_default()
}

/// The year of a time in Unix seconds, by the proleptic Gregorian calendar; None before 1970 or past 9999.
pub(crate) fn year_of(seconds: i64) -> Option<u16> {
    if seconds < 0 {
        return None;
    }
    // Days since 1970-01-01 to a civil year (Howard Hinnant's `civil_from_days`).
    let days = seconds / 86_400 + 719_468;
    let era = days / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let year = yoe + era * 400 + i64::from(mp >= 10);
    u16::try_from(year).ok().filter(|y| *y <= 9999)
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

    /// The address it is asked at.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The API's reply to `path`, its `data`; None for 404.
    fn get(&self, path: &str) -> Result<Option<Value>, Error> {
        Ok(self.get_reply(path)?.and_then(|reply| reply.get("data").cloned()))
    }

    /// The API's whole reply to `path`, its paging with its `data`; None for 404.
    fn get_reply(&self, path: &str) -> Result<Option<Value>, Error> {
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
        Ok(Some(reply))
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
        let results: Vec<(u64, String, bool)> =
            self.autocomplete(name)?.into_iter().map(|(hit, verified)| (hit_id(&hit), hit.name, verified)).collect();
        Ok(best_match(name, &results))
    }

    /// The games SteamGridDB's search finds for these words, in its order, each with whether it is verified: for
    /// the Match game page (docs/spec/match-game.md), and for finding a game by its name. Only the words are sent.
    pub fn search(&self, words: &str) -> Result<Vec<Hit>, Error> {
        if plain_name(words).is_empty() {
            return Ok(Vec::new());
        }
        Ok(self.autocomplete(words)?.into_iter().map(|(hit, _)| hit).take(MOST_HITS).collect())
    }

    fn autocomplete(&self, words: &str) -> Result<Vec<(Hit, bool)>, Error> {
        let Some(found) = self.get(&format!("search/autocomplete/{}", segment(words)))? else { return Ok(Vec::new()) };
        Ok(found
            .as_array()
            .map(|games| {
                games
                    .iter()
                    .filter_map(|game| {
                        let id = game.get("id")?.as_u64()?;
                        let name = game.get("name")?.as_str()?.to_string();
                        let year = game.get("release_date").and_then(Value::as_i64).and_then(year_of);
                        let verified = game.get("verified").and_then(Value::as_bool).unwrap_or(false);
                        Some((Hit { id: id.to_string(), name, year }, verified))
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// The address of the game's first picture of this kind that is static, neither adult nor a joke nor a
    /// flasher, and a PNG, JPEG or WebP; None when it has none. The plainest is asked for first: style [`PLAIN`]
    /// (SteamGridDB has no "official" style for grids or heroes: its others, `blurred`, `white_logo`, `material` and
    /// `no_logo`, are each a made treatment); only when the game has none of that style, any. The service's own
    /// order is kept within each.
    pub fn picture(&self, game: u64, kind: Kind) -> Result<Option<String>, Error> {
        let path = match kind {
            Kind::Cover => format!("grids/game/{game}?dimensions=600x900&{SAFE}&limit=5"),
            Kind::Hero => format!("heroes/game/{game}?{SAFE}&limit=5"),
            Kind::Header => format!("grids/game/{game}?dimensions=920x430,460x215&{SAFE}&limit=5"),
            Kind::Square => format!("grids/game/{game}?dimensions=1024x1024,512x512&{SAFE}&limit=5"),
        };
        if let Some(url) = self.first_safe(&format!("{path}&styles={PLAIN}"))? {
            return Ok(Some(url));
        }
        self.first_safe(&path)
    }

    fn first_safe(&self, path: &str) -> Result<Option<String>, Error> {
        let Some(pictures) = self.get(path)? else { return Ok(None) };
        let safe = |picture: &&Value| {
            let no = |field: &str| picture.get(field).and_then(Value::as_bool) != Some(true);
            let mime = picture.get("mime").and_then(Value::as_str).unwrap_or_default();
            no("nsfw") && no("humor") && no("epilepsy") && matches!(mime, "image/png" | "image/jpeg" | "image/webp")
        };
        let url = pictures.as_array().and_then(|all| all.iter().find(safe)).and_then(|p| p.get("url")?.as_str());
        Ok(url.map(str::to_string))
    }

    /// One page of the game's candidates of a kind, for the art picker (docs/spec/art-picker.md, 2): static, safe
    /// PNG, JPEG and WebP pictures of the kind's shapes, the service's own page `page` (from 0) of [`LIMIT`], and
    /// the total it says the list has. A game it does not know is an empty list.
    pub fn candidates(&self, game: u64, tab: Tab, page: u32) -> Result<Reply, Error> {
        let path = match tab {
            Tab::Cover => format!("grids/game/{game}?dimensions=600x900,342x482,660x930&{SAFE}"),
            Tab::Square => format!("grids/game/{game}?dimensions=1024x1024,512x512&{SAFE}"),
            Tab::Background => format!("heroes/game/{game}?{SAFE}"),
        };
        let path = format!("{path}&limit={LIMIT}&page={page}");
        let Some(reply) = self.get_reply(&path)? else {
            return Ok(Reply { page, total: 0, limit: LIMIT, items: Vec::new() });
        };
        let number = |field: &str| reply.get(field).and_then(Value::as_u64);
        let items: Vec<Candidate> = reply
            .get("data")
            .and_then(Value::as_array)
            .map(|all| {
                all.iter()
                    .filter_map(|picture| {
                        let url = picture.get("url")?.as_str()?.to_string();
                        let thumb = picture.get("thumb").and_then(Value::as_str).unwrap_or(&url).to_string();
                        let size = |field: &str| picture.get(field).and_then(Value::as_u64).unwrap_or(0) as u32;
                        Some(Candidate {
                            id: picture.get("id")?.as_u64()?,
                            url,
                            thumb,
                            width: size("width"),
                            height: size("height"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Reply {
            page: number("page").map_or(page, |p| p as u32),
            // A reply without a total: a full page may have one after it, which a short or empty one then ends.
            total: number("total")
                .map_or(page as usize * LIMIT + items.len() + usize::from(items.len() == LIMIT), |t| t as usize),
            limit: number("limit").map_or(LIMIT, |l| l as usize),
            items,
        })
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
        let results = [
            (1, "Hadrel II".to_string(), true),
            (2, "Hadrel's Star".to_string(), true),
            (3, "HADREL".to_string(), false),
            (4, "Hadrel".to_string(), true),
        ];
        // The verified one of the two that are the name.
        assert_eq!(best_match("Hadrel", &results), Some(4));
        assert_eq!(best_match("Hadrel: Battle Out of Hollow", &results), None);
        assert_eq!(best_match("Hadrel 2", &results), None);
        assert_eq!(best_match("Hadrel II", &results[..1]), Some(1));
        assert_eq!(best_match("", &results), None);
        assert_eq!(best_match("™", &results), None);
    }

    #[test]
    fn a_release_date_is_its_year() {
        assert_eq!(year_of(0), Some(1970));
        assert_eq!(year_of(1_100_563_200), Some(2004), "2004-11-16");
        assert_eq!(year_of(1_704_067_199), Some(2023), "the last second of 2023");
        assert_eq!(year_of(1_704_067_200), Some(2024));
        assert_eq!(year_of(-1), None);
    }

    #[test]
    fn a_name_is_one_part_of_the_path() {
        assert_eq!(segment("Oda & the Blind Orchard"), "Oda%20%26%20the%20Blind%20Orchard");
        assert_eq!(segment("a/b?c#d"), "a%2Fb%3Fc%23d");
    }

    #[test]
    fn the_service_never_shows_its_key() {
        let service = Service::new("http://127.0.0.1:1", "secret-key-123");
        assert!(!format!("{service:?}").contains("secret-key-123"));
    }
}
