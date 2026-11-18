//! The stores' art online: their CDNs and public catalogues, asked with no sign-in and no key, sending nothing but a
//! game's id at that store or, where the game is not that store's, its name. What each has, and what was checked
//! against the real services, is in docs/sources.md, "Art", and docs/research/art-sources.md:
//!
//! - **The game's own store** ([`Store::Own`]), first: GOG's `api.gog.com/v2/games/<id>` for a GOG game (box art,
//!   Galaxy's background, the product's card), and the addresses a client named (`OnlineArt::cover` and the rest:
//!   the Epic launcher's catalogue cache), with no request but the pictures'.
//! - **Steam** ([`Store::Steam`]): a Steam game by its appid, any other by its name in the store's search
//!   (`store.steampowered.com/api/storesearch`); then `IStoreBrowseService/GetItems` (Steam's public Web API, no key)
//!   gives the app's asset file names on its CDN: cover (`library_capsule`), hero (`library_hero`), header. Nothing
//!   square.
//! - **Xbox** ([`Store::Microsoft`]): the Microsoft Store's display catalogue, an Xbox game by the product id its
//!   `MicrosoftGame.config` names (`OnlineArt::product`), any other by name in the catalogue's `autosuggest`; then
//!   `products?bigIds=`: `BoxArt` (square), `Poster` (portrait), `SuperHeroArt` (hero), `TitledHeroArt` (header).
//! - **PlayStation** ([`Store::PlayStation`]): by name in the PlayStation Store's `chihiro` search, whose results
//!   carry their images: the 1024 square (type 10) and the 1920x1080 background (type 12). No PlayStation library
//!   is read, so every game is matched by name.
//!
//! The order Steam, Xbox, PlayStation (and SteamGridDB) is the user's (`model::ArtOrder`); the lookup walks it. A
//! name is matched by [`super::names`]' rule alone: no picture is better than another game's.
//!
//! This is one table keyed by the store, not a method of `model::Source`: a source reads only its client's files and
//! never the network (docs/sources.md), and every request for art, with its pace and its manners, is the lookup's.
use serde_json::Value;

use model::ArtSource;

use super::cache::{Kind, Online};
use super::lookup::Want;
use super::names::{more_searches, pick, plain_name, search_name};
use super::steamgriddb::{self, retry_after, segment, transport, Error};

/// Where each store's art is asked for: its own services, or a test's stand-in (`--store-art-service`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hosts {
    /// Steam's Web API.
    pub steam_api: String,
    /// Steam's CDN, where an app's `asset_url_format` is.
    pub steam_cdn: String,
    /// Steam's store, whose search finds an app by name.
    pub steam_store: String,
    /// The Microsoft Store's display catalogue.
    pub microsoft: String,
    /// The PlayStation Store.
    pub playstation: String,
    /// GOG's API.
    pub gog: String,
}

impl Default for Hosts {
    fn default() -> Self {
        Self {
            steam_api: "https://api.steampowered.com".into(),
            steam_cdn: "https://shared.steamstatic.com/store_item_assets".into(),
            steam_store: "https://store.steampowered.com".into(),
            microsoft: "https://displaycatalog.mp.microsoft.com".into(),
            playstation: "https://store.playstation.com".into(),
            gog: "https://api.gog.com".into(),
        }
    }
}

impl Hosts {
    /// Every store at one stand-in (`library::art::fake::FakeStores`): `<base>/steam-api`, `<base>/steam-cdn`,
    /// `<base>/steam-store`, `<base>/microsoft`, `<base>/playstation`, `<base>/gog`.
    pub fn all_at(base: &str) -> Self {
        let base = base.trim_end_matches('/');
        Self {
            steam_api: format!("{base}/steam-api"),
            steam_cdn: format!("{base}/steam-cdn"),
            steam_store: format!("{base}/steam-store"),
            microsoft: format!("{base}/microsoft"),
            playstation: format!("{base}/playstation"),
            gog: format!("{base}/gog"),
        }
    }
}

/// A store asked for art.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Store {
    /// The game's own store where it is none of the others: GOG's catalogue, a client's named addresses.
    Own,
    Steam,
    Microsoft,
    PlayStation,
}

impl Store {
    /// The store of an online art source; None for SteamGridDB, which is not a store.
    pub fn of(source: ArtSource) -> Option<Self> {
        match source {
            ArtSource::Steam => Some(Store::Steam),
            ArtSource::Xbox => Some(Store::Microsoft),
            ArtSource::PlayStation => Some(Store::PlayStation),
            ArtSource::SteamGridDb => None,
        }
    }

    /// Where its misses are remembered.
    pub fn online(self) -> Online {
        match self {
            Store::Own => Online::Store,
            Store::Steam => Online::Steam,
            Store::Microsoft => Online::Microsoft,
            Store::PlayStation => Online::PlayStation,
        }
    }

    /// Its name as the log says where a picture came from.
    pub fn name(self) -> &'static str {
        match self {
            Store::Own => "its store",
            Store::Steam => "Steam",
            Store::Microsoft => "Xbox",
            Store::PlayStation => "PlayStation",
        }
    }
}

/// The stores' art online.
pub struct StoreArt {
    hosts: Hosts,
    agent: ureq::Agent,
}

impl std::fmt::Debug for StoreArt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreArt").field("hosts", &self.hosts).finish_non_exhaustive()
    }
}

/// What a store is asked, for a game: the address, and the store's name as the log says it (and its gate is kept
/// by).
pub struct Ask {
    pub url: String,
    pub name: &'static str,
}

/// The game a store's search found: its id there, and, where the search's own answer has its pictures (the
/// PlayStation Store's), that answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub id: String,
    pub item: Option<Value>,
}

impl StoreArt {
    pub fn new(hosts: Hosts) -> Self {
        Self { hosts, agent: steamgriddb::agent() }
    }

    /// The kinds of picture the store may have of the game, as far as can be told without asking.
    pub fn offers(&self, store: Store, want: &Want) -> Vec<Kind> {
        let named = || Kind::ALL.into_iter().filter(|&kind| named(want, kind).is_some());
        match store {
            Store::Own => {
                let gog = want.source == "gog" && !want.source_id.is_empty();
                let gog = gog && want.source_id.bytes().all(|b| b.is_ascii_digit());
                let catalogue: &[Kind] = if gog { &[Kind::Cover, Kind::Hero, Kind::Header] } else { &[] };
                let mut kinds = catalogue.to_vec();
                kinds.extend(named().filter(|kind| !catalogue.contains(kind)));
                kinds
            }
            _ if self.own_id(store, want).is_none() && plain_name(&want.name).is_empty() => Vec::new(),
            Store::Steam => vec![Kind::Cover, Kind::Hero, Kind::Header],
            Store::Microsoft => Kind::ALL.to_vec(),
            Store::PlayStation => vec![Kind::Square, Kind::Hero],
        }
    }

    /// The game's id at the store where it is the store's own: a Steam game's appid, an Xbox game's product id.
    pub fn own_id(&self, store: Store, want: &Want) -> Option<String> {
        match store {
            Store::Steam if want.source == "steam" && steam_app(&want.source_id) => Some(want.source_id.clone()),
            Store::Microsoft if want.source == "xbox" => want.online.product.clone().filter(|id| product_id(id)),
            _ => None,
        }
    }

    /// The store's search for the game by its name, for a game that is not the store's own. Only the name is sent.
    pub fn search(&self, store: Store, want: &Want) -> Option<Ask> {
        self.search_for(store, &search_name(&want.name))
    }

    /// The store's searches for the game by its name, in the order they are asked until one finds it: its name, then
    /// [`more_searches`] (its words alone, then without the last word).
    pub fn searches(&self, store: Store, want: &Want) -> Vec<Ask> {
        let more = more_searches(&want.name).into_iter().filter_map(|terms| self.search_for(store, &terms));
        self.search(store, want).into_iter().chain(more).collect()
    }

    fn search_for(&self, store: Store, name: &str) -> Option<Ask> {
        if plain_name(name).is_empty() {
            return None;
        }
        let term = segment(name);
        match store {
            Store::Own => None,
            Store::Steam => Some(Ask {
                url: format!("{}/api/storesearch/?term={term}&l=english&cc=US", self.hosts.steam_store),
                name: "Steam",
            }),
            Store::Microsoft => Some(Ask {
                url: format!(
                    "{}/v7.0/productFamilies/autosuggest?market=US&languages=en-us&productFamilyNames=Games&query={term}",
                    self.hosts.microsoft
                ),
                name: "the Microsoft Store",
            }),
            Store::PlayStation => Some(Ask {
                url: format!(
                    "{}/store/api/chihiro/00_09_000/tumbler/US/en/999/{term}?suggested_size=10&mode=game",
                    self.hosts.playstation
                ),
                name: "the PlayStation Store",
            }),
        }
    }

    /// Of a search's answer, the game that is the one wanted by the matching rule (`names::pick`); None when none
    /// is.
    pub fn matched(&self, store: Store, want: &Want, answer: &Value) -> Option<Found> {
        pick(&want.name, &self.candidates(store, answer)).cloned()
    }

    /// A search's answer's games, as the matching rule weighs them: each one's name there, what it is, and its rank
    /// (the store's preference, lower first). What is not a game (an add-on, a package) is not among them.
    pub fn candidates(&self, store: Store, answer: &Value) -> Vec<(String, Found, u8)> {
        match store {
            Store::Own => Vec::new(),
            // `items[]`: `{type, name, id}`; apps alone, not bundles or packages.
            Store::Steam => list(answer.get("items"))
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("app"))
                .filter_map(|item| {
                    let id = item.get("id")?.as_u64()?;
                    Some((text(item, "name")?, Found { id: id.to_string(), item: None }, 0))
                })
                .collect(),
            // `Results[].Products[]`: `{ProductId, Type, Title}`; games alone, not their add-ons (`Durable`).
            Store::Microsoft => list(answer.get("Results"))
                .flat_map(|family| list(family.get("Products")))
                .filter(|product| product.get("Type").and_then(Value::as_str) == Some("Game"))
                .filter_map(|product| {
                    let id = text(product, "ProductId").filter(|id| product_id(id))?;
                    Some((text(product, "Title")?, Found { id, item: None }, 0))
                })
                .collect(),
            // `links[]`: `{name, id, top_category, game_contentType, images[]}`; a full game before a bundle of it,
            // never an add-on.
            Store::PlayStation => list(answer.get("links"))
                .filter(|link| {
                    let category = link.get("top_category").and_then(Value::as_str).unwrap_or_default();
                    matches!(category, "downloadable_game" | "disc_based_game")
                })
                .filter_map(|link| {
                    let rank = match link.get("game_contentType").and_then(Value::as_str) {
                        Some("Full Game") => 0,
                        Some("Bundle") => 1,
                        _ => return None,
                    };
                    let found = Found { id: text(link, "id")?, item: Some(link.clone()) };
                    Some((text(link, "name")?, found, rank))
                })
                .collect(),
        }
    }

    /// The request to the store that gives the game's pictures' addresses, by its id there; none where the client
    /// named them all, the search's answer had them, or the store has nothing to ask.
    pub fn ask(&self, store: Store, want: &Want, id: Option<&str>, kinds: &[Kind]) -> Option<Ask> {
        match store {
            Store::Own => {
                if kinds.iter().all(|&kind| named(want, kind).is_some()) || want.source != "gog" {
                    return None;
                }
                let url = format!("{}/v2/games/{}?locale=en-US", self.hosts.gog, segment(&want.source_id));
                Some(Ask { url, name: "GOG" })
            }
            Store::Steam => {
                let id = id.filter(|id| steam_app(id))?;
                let input = format!(
                    r#"{{"ids":[{{"appid":{id}}}],"context":{{"language":"english","country_code":"US"}},"data_request":{{"include_assets":true}}}}"#
                );
                let url =
                    format!("{}/IStoreBrowseService/GetItems/v1/?input_json={}", self.hosts.steam_api, segment(&input));
                Some(Ask { url, name: "Steam" })
            }
            Store::Microsoft => {
                let id = id.filter(|id| product_id(id))?;
                let url = format!("{}/v7.0/products?bigIds={id}&market=US&languages=en-us", self.hosts.microsoft);
                Some(Ask { url, name: "the Microsoft Store" })
            }
            Store::PlayStation => None,
        }
    }

    /// The answer: its JSON, None for something it does not know (404).
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

    /// The addresses of the game's pictures of these kinds: for its own store the client's where it named them,
    /// else from the store's `answer`.
    pub fn pictures(&self, store: Store, want: &Want, kinds: &[Kind], answer: Option<&Value>) -> Vec<(Kind, String)> {
        let from_answer = |kind| -> Option<String> {
            let answer = answer?;
            match store {
                Store::Own => gog_picture(answer, kind),
                Store::Steam => steam_picture(answer, &self.hosts.steam_cdn, kind),
                Store::Microsoft => microsoft_picture(answer, &self.hosts.microsoft, kind),
                Store::PlayStation => playstation_picture(answer, kind),
            }
        };
        kinds
            .iter()
            .filter_map(|&kind| {
                let named = if store == Store::Own { named(want, kind).map(str::to_string) } else { None };
                named.or_else(|| from_answer(kind)).map(|url| (kind, url))
            })
            .filter(|(_, url)| url.starts_with("https://") || url.starts_with("http://"))
            .collect()
    }

    /// A picture's bytes, with nothing else sent.
    pub fn download(&self, url: &str) -> Result<Vec<u8>, Error> {
        steamgriddb::download(&self.agent, url)
    }
}

fn list(value: Option<&Value>) -> impl Iterator<Item = &Value> {
    value.and_then(Value::as_array).into_iter().flatten()
}

fn text(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().filter(|text| !text.is_empty()).map(str::to_string)
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

/// A `chihiro` search result's `images[]`, `{type, url}`: type 10 is the 1024x1024 square, type 12 the 1920x1080
/// background (measured 2026-11-18, docs/research/art-sources.md). Nothing portrait or titled-landscape is used.
fn playstation_picture(item: &Value, kind: Kind) -> Option<String> {
    let wanted = match kind {
        Kind::Square => 10,
        Kind::Hero => 12,
        Kind::Cover | Kind::Header => return None,
    };
    let image = list(item.get("images")).find(|image| image.get("type").and_then(Value::as_u64) == Some(wanted))?;
    text(image, "url")
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
        Want { source: source.into(), source_id: id.into(), name: "Made-up Game".into(), ..Want::default() }
    }

    #[test]
    fn each_store_offers_what_it_has() {
        let stores = StoreArt::new(Hosts::default());
        let steam = want("steam", "620");
        assert_eq!(stores.offers(Store::Steam, &steam), [Kind::Cover, Kind::Hero, Kind::Header]);
        assert_eq!(stores.own_id(Store::Steam, &steam).as_deref(), Some("620"));
        // Any game, by its name, at every store.
        let epic = want("epic", "Fernway");
        assert_eq!(stores.offers(Store::Steam, &epic), [Kind::Cover, Kind::Hero, Kind::Header]);
        assert_eq!(stores.offers(Store::Microsoft, &epic), Kind::ALL);
        assert_eq!(stores.offers(Store::PlayStation, &epic), [Kind::Square, Kind::Hero]);
        assert!(stores.own_id(Store::Steam, &epic).is_none());
        // With no name to search by, only its own store's id.
        let nameless = Want { name: "™".into(), ..want("epic", "Fernway") };
        assert!(stores.offers(Store::Microsoft, &nameless).is_empty());
        // An Xbox game by its product id.
        let mut xbox = want("xbox", "Family_abc");
        xbox.online.product = Some("9ZZZMADEUP12".into());
        assert_eq!(stores.own_id(Store::Microsoft, &xbox).as_deref(), Some("9ZZZMADEUP12"));
        // Its own store: GOG's catalogue and the addresses a client named.
        assert_eq!(stores.offers(Store::Own, &want("gog", "1207664663")), [Kind::Cover, Kind::Hero, Kind::Header]);
        for source in ["ea", "ubisoft", "battlenet", "epic", "steam", "xbox"] {
            assert!(stores.offers(Store::Own, &want(source, "1")).is_empty(), "{source}");
        }
        let mut epic = want("epic", "Fernway");
        epic.online.cover = Some("https://cdn1.epicgames.com/tall.jpg".into());
        assert_eq!(stores.offers(Store::Own, &epic), [Kind::Cover]);
        assert!(stores.ask(Store::Own, &epic, None, &[Kind::Cover]).is_none());
        assert_eq!(
            stores.pictures(Store::Own, &epic, &[Kind::Cover], None),
            [(Kind::Cover, "https://cdn1.epicgames.com/tall.jpg".into())]
        );
    }

    #[test]
    fn only_the_games_id_or_name_is_sent() {
        let stores = StoreArt::new(Hosts::default());
        let steam = stores.ask(Store::Steam, &want("steam", "620"), Some("620"), &[Kind::Cover]).unwrap();
        assert!(steam.url.starts_with("https://api.steampowered.com/IStoreBrowseService/GetItems/v1/?input_json="));
        assert!(steam.url.contains("%22appid%22%3A620%7D"), "{}", steam.url);
        let microsoft = stores.ask(Store::Microsoft, &want("xbox", "F"), Some("9ZZZMADEUP12"), &[Kind::Square]);
        assert_eq!(
            microsoft.unwrap().url,
            "https://displaycatalog.mp.microsoft.com/v7.0/products?bigIds=9ZZZMADEUP12&market=US&languages=en-us"
        );
        assert_eq!(
            stores.ask(Store::Own, &want("gog", "42"), None, &[Kind::Cover]).unwrap().url,
            "https://api.gog.com/v2/games/42?locale=en-US"
        );
        let game = Want { name: "Oda™ & the Orchard".into(), ..want("epic", "Oda") };
        assert_eq!(
            stores.search(Store::Steam, &game).unwrap().url,
            "https://store.steampowered.com/api/storesearch/?term=Oda%20%26%20the%20Orchard&l=english&cc=US"
        );
        assert_eq!(
            stores.search(Store::Microsoft, &game).unwrap().url,
            "https://displaycatalog.mp.microsoft.com/v7.0/productFamilies/autosuggest?market=US&languages=en-us\
             &productFamilyNames=Games&query=Oda%20%26%20the%20Orchard"
        );
        assert_eq!(
            stores.search(Store::PlayStation, &game).unwrap().url,
            "https://store.playstation.com/store/api/chihiro/00_09_000/tumbler/US/en/999/Oda%20%26%20the%20Orchard\
             ?suggested_size=10&mode=game"
        );
        assert!(stores.search(Store::Own, &game).is_none());
        // Then, while none finds the game: its words alone, then without the last word.
        let searches: Vec<String> = stores.searches(Store::Steam, &game).into_iter().map(|ask| ask.url).collect();
        assert_eq!(searches.len(), 3);
        assert!(searches[1].contains("term=Oda%20the%20Orchard&"), "{searches:?}");
        assert!(searches[2].contains("term=Oda%20the&"), "{searches:?}");
        let one = Want { name: "Hades".into(), ..want("epic", "H") };
        assert_eq!(stores.searches(Store::Steam, &one).len(), 1);
    }

    #[test]
    fn a_search_is_taken_only_for_the_same_game() {
        let stores = StoreArt::new(Hosts::default());
        let game = Want { name: "Made-up Reforger".into(), ..want("steam", "1") };
        // Shapes as seen on 2026-11-18 (docs/research/art-sources.md), cut to what is read; names made up.
        let steam = json!({"total": 3, "items": [
            {"type": "app", "name": "Made-up Reforger - Rally", "id": 5},
            {"type": "sub", "name": "Made-up Reforger", "id": 6},
            {"type": "app", "name": "Made-up Reforger", "id": 7}]});
        assert_eq!(stores.matched(Store::Steam, &game, &steam).unwrap().id, "7");
        let steam = json!({"total": 1, "items": [{"type": "app", "name": "Made-up Reforger Soundtrack", "id": 8}]});
        assert_eq!(stores.matched(Store::Steam, &game, &steam), None, "never the first hit regardless");

        let microsoft = json!({"Results": [
            {"ProductFamilyName": "Games", "Products": [{"ProductId": "9NZZZMADEUP1", "Type": "Durable", "Title": "Made-up Reforger"}]},
            {"ProductFamilyName": "Games", "Products": [{"ProductId": "9NZZZMADEUP2", "Type": "Game", "Title": "Made-up Reforger™"}]}],
            "TotalResultCount": 2});
        assert_eq!(stores.matched(Store::Microsoft, &game, &microsoft).unwrap().id, "9NZZZMADEUP2");
        assert_eq!(stores.matched(Store::Microsoft, &game, &json!({"Results": []})), None);

        let image = |t: u64| json!({"type": t, "url": format!("https://image.example/{t}.png")});
        let playstation = json!({"links": [
            {"name": "Made-up Reforger Deluxe Edition PS4 & PS5", "id": "EP1-B", "top_category": "downloadable_game",
             "game_contentType": "Bundle", "images": [image(10)]},
            {"name": "Made-up Reforger", "id": "EP1-A", "top_category": "add_on", "game_contentType": "Map", "images": []},
            {"name": "MADE-UP REFORGER PS5", "id": "EP1-C", "top_category": "downloadable_game",
             "game_contentType": "Full Game", "images": [image(10), image(12), image(13)]}]});
        let found = stores.matched(Store::PlayStation, &game, &playstation).unwrap();
        assert_eq!(found.id, "EP1-C", "the full game before its bundle, never the add-on");
        let item = found.item.as_ref();
        let pictures = stores.pictures(Store::PlayStation, &game, &[Kind::Cover, Kind::Square, Kind::Hero], item);
        assert_eq!(
            pictures,
            [
                (Kind::Square, "https://image.example/10.png".into()),
                (Kind::Hero, "https://image.example/12.png".into())
            ]
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
