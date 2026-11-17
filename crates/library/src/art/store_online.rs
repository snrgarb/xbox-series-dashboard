//! Each store's own art online: its CDN or public catalogue, asked with no sign-in and no key, sending nothing but the
//! game's id at that store. What each store has, and what was checked against the real services, is in
//! docs/sources.md, "Art":
//!
//! - **Steam:** `IStoreBrowseService/GetItems` (Steam's public Web API, no key) gives an app's asset file names,
//!   which are on its CDN under `store_item_assets`: cover (`library_capsule`), hero (`library_hero`), header.
//!   Nothing square.
//! - **Xbox:** the Microsoft Store's display catalogue, by the product id the game's `MicrosoftGame.config` names
//!   (`OnlineArt::product`): `BoxArt` (square), `Poster` (portrait), `SuperHeroArt` (hero), `TitledHeroArt` (header).
//! - **GOG:** `api.gog.com/v2/games/<id>`: the box art (portrait), Galaxy's background (hero), and the product's
//!   card (header). Nothing square large enough for a tile.
//! - **Any store whose client named its pictures' addresses** (`OnlineArt::cover` and the rest: the Epic launcher's
//!   catalogue cache): those, with no request but the pictures'.
//!
//! This is one table keyed by the source's key, not a method of `model::Source`: a source reads only its client's
//! files and never the network (docs/sources.md), and every request for art, with its pace and its manners, is the
//! lookup's.
use serde_json::Value;

use super::cache::Kind;
use super::lookup::Want;
use super::steamgriddb::{self, retry_after, segment, transport, Error};

/// Where each store's art is asked for: its own services, or a test's stand-in (`--store-art-service`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hosts {
    /// Steam's Web API.
    pub steam_api: String,
    /// Steam's CDN, where an app's `asset_url_format` is.
    pub steam_cdn: String,
    /// The Microsoft Store's display catalogue.
    pub microsoft: String,
    /// GOG's API.
    pub gog: String,
}

impl Default for Hosts {
    fn default() -> Self {
        Self {
            steam_api: "https://api.steampowered.com".into(),
            steam_cdn: "https://shared.steamstatic.com/store_item_assets".into(),
            microsoft: "https://displaycatalog.mp.microsoft.com".into(),
            gog: "https://api.gog.com".into(),
        }
    }
}

impl Hosts {
    /// Every store at one stand-in (`library::art::fake::FakeStores`): `<base>/steam-api`, `<base>/steam-cdn`,
    /// `<base>/microsoft`, `<base>/gog`.
    pub fn all_at(base: &str) -> Self {
        let base = base.trim_end_matches('/');
        Self {
            steam_api: format!("{base}/steam-api"),
            steam_cdn: format!("{base}/steam-cdn"),
            microsoft: format!("{base}/microsoft"),
            gog: format!("{base}/gog"),
        }
    }
}

/// The stores' own art online.
pub struct StoreArt {
    hosts: Hosts,
    agent: ureq::Agent,
}

impl std::fmt::Debug for StoreArt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreArt").field("hosts", &self.hosts).finish_non_exhaustive()
    }
}

/// What a store's catalogue is asked, for a game: the address, and the store's name as the log says it.
pub struct Ask {
    pub url: String,
    pub name: &'static str,
}

impl StoreArt {
    pub fn new(hosts: Hosts) -> Self {
        Self { hosts, agent: steamgriddb::agent() }
    }

    /// The kinds of picture the game's store has online for it, as far as can be told without asking.
    pub fn offers(&self, want: &Want) -> Vec<Kind> {
        let catalogue: &[Kind] = match want.source.as_str() {
            "steam" if steam_app(&want.source_id) => &[Kind::Cover, Kind::Hero, Kind::Header],
            "xbox" if want.online.product.as_deref().is_some_and(product_id) => &Kind::ALL,
            "gog" if want.source_id.bytes().all(|b| b.is_ascii_digit()) && !want.source_id.is_empty() => {
                &[Kind::Cover, Kind::Hero, Kind::Header]
            }
            _ => &[],
        };
        let named = Kind::ALL.into_iter().filter(|&kind| named(want, kind).is_some());
        let mut kinds: Vec<Kind> = catalogue.to_vec();
        kinds.extend(named.filter(|kind| !catalogue.contains(kind)));
        kinds
    }

    /// The request to the store's catalogue that gives the game's pictures' addresses; none where the client
    /// named them all, or the store has no catalogue to ask.
    pub fn ask(&self, want: &Want, kinds: &[Kind]) -> Option<Ask> {
        if kinds.iter().all(|&kind| named(want, kind).is_some()) {
            return None;
        }
        let id = want.source_id.as_str();
        match want.source.as_str() {
            "steam" if steam_app(id) => {
                let input = format!(
                    r#"{{"ids":[{{"appid":{id}}}],"context":{{"language":"english","country_code":"US"}},"data_request":{{"include_assets":true}}}}"#
                );
                let url =
                    format!("{}/IStoreBrowseService/GetItems/v1/?input_json={}", self.hosts.steam_api, segment(&input));
                Some(Ask { url, name: "Steam" })
            }
            "xbox" => {
                let product = want.online.product.as_deref().filter(|id| product_id(id))?;
                let url = format!("{}/v7.0/products?bigIds={product}&market=US&languages=en-us", self.hosts.microsoft);
                Some(Ask { url, name: "the Microsoft Store" })
            }
            "gog" => {
                Some(Ask { url: format!("{}/v2/games/{}?locale=en-US", self.hosts.gog, segment(id)), name: "GOG" })
            }
            _ => None,
        }
    }

    /// The catalogue's answer: its JSON, None for a game it does not know (404).
    pub fn get(&self, ask: &Ask) -> Result<Option<Value>, Error> {
        let mut response = self.agent.get(&ask.url).call().map_err(|e| transport(e, ask.name))?;
        match response.status().as_u16() {
            200..=299 => {}
            404 | 410 => return Ok(None),
            429 | 503 => return Err(Error::Busy(retry_after(&response))),
            status => return Err(Error::Failed(format!("{} answered {status}", ask.name))),
        }
        let text = response.body_mut().read_to_string().map_err(|e| transport(e, ask.name))?;
        serde_json::from_str(&text).map(Some).map_err(|_| Error::Failed(format!("{}'s reply is not JSON", ask.name)))
    }

    /// The addresses of the game's pictures of these kinds: the client's own where it named them, else from the
    /// catalogue's `answer`.
    pub fn pictures(&self, want: &Want, kinds: &[Kind], answer: Option<&Value>) -> Vec<(Kind, String)> {
        let from_catalogue = |kind| -> Option<String> {
            let answer = answer?;
            match want.source.as_str() {
                "steam" => steam_picture(answer, &self.hosts.steam_cdn, kind),
                "xbox" => microsoft_picture(answer, &self.hosts.microsoft, kind),
                "gog" => gog_picture(answer, kind),
                _ => None,
            }
        };
        kinds
            .iter()
            .filter_map(|&kind| {
                named(want, kind).map(str::to_string).or_else(|| from_catalogue(kind)).map(|url| (kind, url))
            })
            .filter(|(_, url)| url.starts_with("https://") || url.starts_with("http://"))
            .collect()
    }

    /// A picture's bytes, with nothing else sent.
    pub fn download(&self, url: &str) -> Result<Vec<u8>, Error> {
        steamgriddb::download(&self.agent, url)
    }
}

/// The address the client named for this kind.
fn named(want: &Want, kind: Kind) -> Option<&str> {
    let online = &want.online;
    match kind {
        Kind::Cover => online.cover.as_deref(),
        Kind::Hero => online.hero.as_deref(),
        Kind::Header => online.header.as_deref(),
        Kind::Square => online.square.as_deref(),
    }
}

/// A Steam appid: digits.
fn steam_app(id: &str) -> bool {
    !id.is_empty() && id.len() <= 10 && id.bytes().all(|b| b.is_ascii_digit())
}

/// A Microsoft Store product id: twelve letters and digits (`9ZZZMADEUP12`).
fn product_id(id: &str) -> bool {
    id.len() == 12 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// GetItems' `response.store_items[0].assets`, and the asset of this kind on the CDN.
fn steam_picture(answer: &Value, cdn: &str, kind: Kind) -> Option<String> {
    let item = answer.pointer("/response/store_items/0")?;
    if item.get("success").and_then(Value::as_i64) != Some(1) {
        return None;
    }
    let assets = item.get("assets")?;
    let format = assets.get("asset_url_format")?.as_str()?;
    let name = match kind {
        Kind::Cover => "library_capsule",
        Kind::Hero => "library_hero",
        Kind::Header => "header",
        Kind::Square => return None,
    };
    let file = assets.get(name)?.as_str().filter(|file| !file.is_empty())?;
    Some(format!("{cdn}/{}", format.replace("${FILENAME}", file)))
}

/// The display catalogue's `Products[0].LocalizedProperties[0].Images`, the first of the kind's purposes, at the
/// width a screen needs (the store's images take `w`). Its addresses start `//`: the catalogue's own scheme goes on.
fn microsoft_picture(answer: &Value, catalogue: &str, kind: Kind) -> Option<String> {
    let images = answer.pointer("/Products/0/LocalizedProperties/0/Images")?.as_array()?;
    let (purposes, width): (&[&str], u32) = match kind {
        Kind::Cover => (&["Poster"], 600),
        Kind::Hero => (&["SuperHeroArt"], 1920),
        Kind::Header => (&["TitledHeroArt"], 920),
        Kind::Square => (&["BoxArt", "FeaturePromotionalSquareArt"], 600),
    };
    let uri = purposes.iter().find_map(|purpose| {
        let image = images.iter().find(|image| image.get("ImagePurpose").and_then(Value::as_str) == Some(purpose))?;
        image.get("Uri")?.as_str()
    })?;
    let scheme = catalogue.split_once("//").map_or("https:", |(scheme, _)| scheme);
    let url = if uri.starts_with("//") { format!("{scheme}{uri}") } else { uri.to_string() };
    let joiner = if url.contains('?') { '&' } else { '?' };
    Some(format!("{url}{joiner}w={width}"))
}

/// `v2/games/<id>`'s `_links`: the box art, Galaxy's background, and the product's card at 800 wide.
fn gog_picture(answer: &Value, kind: Kind) -> Option<String> {
    let link = |path: &str| answer.pointer(path).and_then(Value::as_str).map(str::to_string);
    match kind {
        Kind::Cover => link("/_links/boxArtImage/href"),
        Kind::Hero => link("/_links/galaxyBackgroundImage/href").or_else(|| link("/_links/backgroundImage/href")),
        Kind::Header => link("/_embedded/product/_links/image/href").map(|href| href.replace("{formatter}", "800")),
        Kind::Square => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn want(source: &str, id: &str) -> Want {
        Want { source: source.into(), source_id: id.into(), ..Want::default() }
    }

    #[test]
    fn each_store_offers_what_its_catalogue_has() {
        let stores = StoreArt::new(Hosts::default());
        assert_eq!(stores.offers(&want("steam", "620")), [Kind::Cover, Kind::Hero, Kind::Header]);
        assert!(stores.offers(&want("steam", "not-an-appid")).is_empty());
        // An Xbox game only with its product id.
        assert!(stores.offers(&want("xbox", "Family_abc")).is_empty());
        let mut xbox = want("xbox", "Family_abc");
        xbox.online.product = Some("9ZZZMADEUP12".into());
        assert_eq!(stores.offers(&xbox), Kind::ALL);
        assert_eq!(stores.offers(&want("gog", "1207664663")), [Kind::Cover, Kind::Hero, Kind::Header]);
        for store in ["ea", "ubisoft", "battlenet", "epic", "manual"] {
            assert!(stores.offers(&want(store, "1")).is_empty(), "{store}");
        }
        // What the client named, with no request at all.
        let mut epic = want("epic", "Fernway");
        epic.online.cover = Some("https://cdn1.epicgames.com/tall.jpg".into());
        assert_eq!(stores.offers(&epic), [Kind::Cover]);
        assert!(stores.ask(&epic, &[Kind::Cover]).is_none());
        assert_eq!(
            stores.pictures(&epic, &[Kind::Cover], None),
            [(Kind::Cover, "https://cdn1.epicgames.com/tall.jpg".into())]
        );
    }

    #[test]
    fn only_the_games_id_is_sent() {
        let stores = StoreArt::new(Hosts::default());
        let steam = stores.ask(&want("steam", "620"), &[Kind::Cover]).unwrap();
        assert!(steam.url.starts_with("https://api.steampowered.com/IStoreBrowseService/GetItems/v1/?input_json="));
        assert!(steam.url.contains("%22appid%22%3A620%7D"), "{}", steam.url);
        let mut xbox = want("xbox", "Family_abc");
        xbox.online.product = Some("9ZZZMADEUP12".into());
        let microsoft = stores.ask(&xbox, &[Kind::Square]).unwrap().url;
        assert_eq!(
            microsoft,
            "https://displaycatalog.mp.microsoft.com/v7.0/products?bigIds=9ZZZMADEUP12&market=US&languages=en-us"
        );
        assert_eq!(
            stores.ask(&want("gog", "42"), &[Kind::Cover]).unwrap().url,
            "https://api.gog.com/v2/games/42?locale=en-US"
        );
    }

    #[test]
    fn the_answers_as_the_real_services_give_them() {
        // Shapes as seen on 2026-11-16 (docs/sources.md, "Art"), cut to what is read.
        let steam = json!({"response": {"store_items": [{"id": 620, "success": 1, "assets": {
            "asset_url_format": "steam/apps/620/${FILENAME}?t=1",
            "library_capsule": "abc/library_600x900.jpg", "library_hero": "library_hero.jpg", "header": "header.jpg"}}]}});
        let cdn = "https://shared.steamstatic.com/store_item_assets";
        assert_eq!(
            steam_picture(&steam, cdn, Kind::Cover).as_deref(),
            Some("https://shared.steamstatic.com/store_item_assets/steam/apps/620/abc/library_600x900.jpg?t=1")
        );
        assert_eq!(steam_picture(&steam, cdn, Kind::Square), None);
        let unknown = json!({"response": {"store_items": [{"id": 9, "success": 15}]}});
        assert_eq!(steam_picture(&unknown, cdn, Kind::Cover), None);

        let microsoft = json!({"Products": [{"LocalizedProperties": [{"Images": [
            {"ImagePurpose": "Poster", "Uri": "//store-images.s-microsoft.com/image/apps.1.poster", "Width": 1440, "Height": 2160},
            {"ImagePurpose": "BoxArt", "Uri": "//store-images.s-microsoft.com/image/apps.2.box", "Width": 2160, "Height": 2160}]}]}]});
        let catalogue = "https://displaycatalog.mp.microsoft.com";
        assert_eq!(
            microsoft_picture(&microsoft, catalogue, Kind::Square).as_deref(),
            Some("https://store-images.s-microsoft.com/image/apps.2.box?w=600")
        );
        assert_eq!(microsoft_picture(&microsoft, catalogue, Kind::Hero), None);
        assert_eq!(microsoft_picture(&json!({"Products": []}), catalogue, Kind::Cover), None);

        let gog = json!({"_links": {"boxArtImage": {"href": "https://images.gog-statics.com/box.jpg"},
                                    "galaxyBackgroundImage": {"href": "https://images.gog-statics.com/bg.jpg"}},
                         "_embedded": {"product": {"_links": {"image": {"href": "https://images.gog-statics.com/card_{formatter}.png"}}}}});
        assert_eq!(gog_picture(&gog, Kind::Header).as_deref(), Some("https://images.gog-statics.com/card_800.png"));
        assert_eq!(gog_picture(&gog, Kind::Cover).as_deref(), Some("https://images.gog-statics.com/box.jpg"));
        assert_eq!(gog_picture(&gog, Kind::Square), None);
    }
}
