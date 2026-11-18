//! The games the GOG account owns, from GOG's own library service with gogdl's sign-in, where GOG Galaxy's database
//! is not on the PC (#276; docs/research/owned-games.md 4, route (c)). As Heroic does it (`gog/library.ts`
//! `getGalaxyLibrary` at `2dbb80272a`): `GET https://galaxy-library.gog.com/users/<user_id>/releases` with
//! `Authorization: Bearer <access_token>`, every page (`?page_token=<next_page_token>` until there is none), the
//! entries whose `platform_id` is `gog` taken, each `external_id` a product id; the names from Galaxy's own
//! `gamesdb.gog.com/platforms/gog/external_releases/<id>` (`title["*"]`), else `api.gog.com/products/<id>`
//! (`title`), a name once had kept from the last list.
//!
//! **The token is gogdl's**, read from its own `auth.json` for the call and never kept anywhere else, logged or
//! written: one that has run out (`loginTime` + `expires_in`) is renewed first by gogdl itself (`gogdl auth`, which
//! refreshes and rewrites its file, `auth.py` 42-122), whose output, holding the token, is read and dropped. Nothing
//! new is signed in to. The list is kept as `<data>\owned\gog.json` (`source_kit::owned`), with the account's GOG user
//! id, and refreshed at start and on Update now off the window's thread (`model::Source::refresh_owned`).
//!
//! The tests point it at a stand-in on this PC (`Web::stand_in`, `DASHBOARD_STORE_WEB`), taken only for a loopback
//! address, so that gogdl's token can never be sent anywhere but GOG or this PC.
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::Value;
use source_kit::owned::{List, Owned};

use super::{read_json, signed_in, Gogdl, CLIENT_ID, LOG};

/// What the tests set to point the Dashboard at a stand-in of the stores' web services: `http://127.0.0.1:<port>`.
pub const STORE_WEB: &str = "DASHBOARD_STORE_WEB";

/// The most pages of a library asked for: a guard against a service that never ends its list.
const MOST_PAGES: usize = 200;

/// Where GOG's services are asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Web {
    /// The library service: `<it>/users/<id>/releases`.
    pub library: String,
    /// Galaxy's games database: `<it>/platforms/gog/external_releases/<id>`.
    pub gamesdb: String,
    /// GOG's product API: `<it>/products/<id>`.
    pub api: String,
}

impl Web {
    /// GOG's own.
    pub fn gog() -> Web {
        Web {
            library: "https://galaxy-library.gog.com".into(),
            gamesdb: "https://gamesdb.gog.com".into(),
            api: "https://api.gog.com".into(),
        }
    }

    /// A stand-in at `base` (`http://127.0.0.1:<port>`): its `/galaxy-library`, `/gamesdb` and `/api`. None for an
    /// address that is not this PC's.
    pub fn stand_in(base: &str) -> Option<Web> {
        let base = base.trim().trim_end_matches('/');
        let host = base.strip_prefix("http://")?.split(['/', ':']).next()?;
        if !matches!(host, "127.0.0.1" | "localhost") {
            return None;
        }
        Some(Web {
            library: format!("{base}/galaxy-library"),
            gamesdb: format!("{base}/gamesdb"),
            api: format!("{base}/api"),
        })
    }

    /// The stand-in [`STORE_WEB`] names, where it names one on this PC; else GOG's own (one elsewhere is said, and
    /// not taken).
    pub fn from_env() -> Web {
        match std::env::var(STORE_WEB) {
            Ok(base) if !base.trim().is_empty() => Web::stand_in(&base).unwrap_or_else(|| {
                eprintln!("{LOG}: {STORE_WEB} is not an address on this PC: GOG's own services are asked");
                Web::gog()
            }),
            _ => Web::gog(),
        }
    }

    /// The library service's host, as the log and the list say where it came from.
    pub fn from(&self) -> String {
        let host = self.library.split("://").nth(1).unwrap_or(&self.library);
        host.to_string()
    }
}

/// A page of `/users/<id>/releases`: the GOG product ids on it, in its order, and the next page's token.
pub fn releases(page: &Value) -> (Vec<String>, Option<String>) {
    let items = page.get("items").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    let ids = items
        .iter()
        .filter(|item| item.get("platform_id").and_then(Value::as_str) == Some("gog"))
        .filter_map(|item| match item.get("external_id")? {
            Value::String(id) if !id.trim().is_empty() => Some(id.trim().to_string()),
            Value::Number(id) => Some(id.to_string()),
            _ => None,
        })
        .collect();
    let next = page.get("next_page_token").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty());
    (ids, next.map(str::to_string))
}

/// A game's name: gamesdb's `title` (`{"*": <name>, ...}`), or the product API's (`<name>`).
pub fn title(answer: &Value) -> Option<String> {
    let title = answer.get("title")?;
    let name = match title {
        Value::String(name) => name.as_str(),
        Value::Object(names) => names.get("*").or_else(|| names.get("en-US")).and_then(Value::as_str)?,
        _ => return None,
    };
    Some(name.trim().to_string()).filter(|name| !name.is_empty())
}

/// gogdl's credentials in its `auth.json`: the access token, the account's user id, and whether it has run out
/// (or will within a minute).
struct Credentials {
    token: String,
    user: String,
    expired: bool,
}

fn credentials(auth: &Path) -> Option<Credentials> {
    let client = read_json(auth)?.get(CLIENT_ID)?.clone();
    let text = |key: &str| match client.get(key)? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    };
    let login = client.get("loginTime").and_then(Value::as_f64).unwrap_or(0.0);
    let lasts = client.get("expires_in").and_then(Value::as_f64).unwrap_or(0.0);
    let now = source_kit::owned::now() as f64;
    Some(Credentials { token: text("access_token")?, user: text("user_id")?, expired: now + 60.0 >= login + lasts })
}

impl Gogdl {
    /// `gogdl auth`, which renews a token that has run out and rewrites `auth.json` (`auth.py`): run quietly, as
    /// what it prints is the token. Whether it ended well.
    fn renew(&self) -> bool {
        let Some(program) = self.program() else { return false };
        let args = vec!["--auth-config-path".to_string(), self.auth().display().to_string(), "auth".into()];
        let Ok(job) = source_kit::helper::Job::start_with(&program, &args, &self.env(), || {}) else { return false };
        let until = std::time::Instant::now() + Duration::from_secs(60);
        loop {
            let left = until.saturating_duration_since(std::time::Instant::now());
            match job.next(left) {
                // What it prints holds the token: read, and never kept or said.
                Some(source_kit::helper::Said::Out(_) | source_kit::helper::Said::Err(_)) => {}
                Some(source_kit::helper::Said::Ended(code)) => return code == Some(0),
                None => {
                    job.stop();
                    return false;
                }
            }
        }
    }
}

/// The account's games from GOG's library service, with gogdl's sign-in; None where gogdl is not added or not
/// signed in (nothing to take it from). Names a game had in `last` are kept, not asked again.
pub fn take(gogdl: &Gogdl, web: &Web, last: Option<&List>) -> Result<Option<List>> {
    if gogdl.program().is_none() || !signed_in(&gogdl.auth()) {
        return Ok(None);
    }
    let mut signed = credentials(&gogdl.auth()).context("gogdl's auth.json has no access token")?;
    if signed.expired {
        if !gogdl.renew() {
            bail!("gogdl could not renew its sign-in (gogdl auth): sign in to gogdl again in Settings > Helpers");
        }
        signed = credentials(&gogdl.auth()).context("gogdl's auth.json has no access token after gogdl auth")?;
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(concat!("xbox-series-dashboard/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let bearer = format!("Bearer {}", signed.token);
    let mut ids: Vec<String> = Vec::new();
    let mut next: Option<String> = None;
    for page in 0.. {
        if page >= MOST_PAGES {
            bail!("GOG's library gave more than {MOST_PAGES} pages");
        }
        let mut url = format!("{}/users/{}/releases", web.library, signed.user);
        if let Some(token) = &next {
            url.push_str(&format!("?page_token={}", encode(token)));
        }
        // ureq's own errors may carry the address, which has the user id in it: none of them is said.
        let answer = agent.get(&url).header("Authorization", &bearer).call();
        let mut answer = answer.map_err(|_| anyhow::anyhow!("GOG's library service did not answer"))?;
        match answer.status().as_u16() {
            200..=299 => {}
            401 | 403 => bail!(
                "GOG's library service refused gogdl's sign-in ({}): sign in to gogdl again in Settings > Helpers",
                answer.status().as_u16()
            ),
            status => bail!("GOG's library service answered {status}"),
        }
        let body = answer.body_mut().read_to_string().map_err(|_| anyhow::anyhow!("GOG's library: no answer read"))?;
        let page: Value = serde_json::from_str(&body).context("GOG's library: its answer is not JSON")?;
        let (more, token) = releases(&page);
        for id in more {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        match token {
            Some(token) => next = Some(token),
            None => break,
        }
    }
    let kept = |id: &str| last?.games.iter().find(|g| g.id == id && !g.name.is_empty()).map(|g| g.name.clone());
    let named = |url: String| -> Option<String> {
        let mut answer = agent.get(&url).call().ok()?;
        if !answer.status().is_success() {
            return None;
        }
        title(&serde_json::from_str(&answer.body_mut().read_to_string().ok()?).ok()?)
    };
    let mut games = Vec::new();
    let mut unnamed = 0;
    for id in ids {
        let name = kept(&id)
            .or_else(|| named(format!("{}/platforms/gog/external_releases/{id}", web.gamesdb)))
            .or_else(|| named(format!("{}/products/{id}", web.api)));
        match name {
            Some(name) => games.push(Owned { id, name, ..Owned::default() }),
            None => unnamed += 1,
        }
    }
    if unnamed > 0 {
        eprintln!("gog: owned: {unnamed} games GOG gave no name for: left out until it does");
    }
    let from = web.from();
    let account = Some(signed.user);
    Ok(Some(List { store: crate::SOURCE.into(), taken: source_kit::owned::now(), from, account, of: None, games }))
}

/// A page token as a query takes it: every byte but letters, digits and `-._~` written `%XX`.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_page_gives_gogs_own_releases_and_the_next_token() {
        let page = json!({
            "total_count": 4, "limit": 2,
            "items": [
                {"platform_id": "gog", "external_id": "1900000101", "owned": true},
                {"platform_id": "steam", "external_id": "620"},
                {"platform_id": "gog", "external_id": 1900000103},
                {"platform_id": "gog", "external_id": ""}
            ],
            "next_page_token": "abc=="
        });
        assert_eq!(releases(&page), (vec!["1900000101".into(), "1900000103".into()], Some("abc==".into())));
        assert_eq!(releases(&json!({"items": [], "next_page_token": ""})), (vec![], None));
        assert_eq!(releases(&json!({})), (vec![], None));
        assert_eq!(encode("abc=="), "abc%3D%3D");
    }

    #[test]
    fn names_from_gamesdb_or_the_product_api() {
        assert_eq!(
            title(&json!({"title": {"*": "Pewter Isles", "fr-FR": "Les Isles"}})).as_deref(),
            Some("Pewter Isles")
        );
        assert_eq!(title(&json!({"title": {"en-US": "Owlglass Abbey"}})).as_deref(), Some("Owlglass Abbey"));
        assert_eq!(title(&json!({"id": 1, "title": "Juniper Signal"})).as_deref(), Some("Juniper Signal"));
        assert_eq!(title(&json!({"title": "  "})), None);
        assert_eq!(title(&json!({})), None);
    }

    #[test]
    fn only_an_address_on_this_pc_stands_in() {
        let web = Web::stand_in("http://127.0.0.1:4567/").unwrap();
        assert_eq!(web.library, "http://127.0.0.1:4567/galaxy-library");
        assert_eq!(web.from(), "127.0.0.1:4567/galaxy-library");
        assert!(Web::stand_in("http://localhost:80").is_some());
        assert_eq!(Web::stand_in("https://127.0.0.1:4567"), None);
        assert_eq!(Web::stand_in("http://example.com"), None);
        assert_eq!(Web::stand_in("http://127.0.0.1.example.com"), None);
        assert_eq!(Web::gog().from(), "galaxy-library.gog.com");
    }
}
