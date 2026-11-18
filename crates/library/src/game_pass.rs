//! The PC Game Pass catalogue of a region (#277; docs/research/game-pass.md, docs/sources.md "Xbox"), fetched from
//! Microsoft's public catalogue with no sign-in and kept in the data folder, so the library has it offline. The Xbox
//! source lists its games (`source_xbox`); fetching it is here, beside the art lookup's own requests to the same
//! display catalogue (`art::store_online`), with the same manners: a plain user agent, nothing about the user sent
//! (the market and the language alone), and a quiet fall back to what is kept.
//!
//! Two requests make it:
//!
//! 1. The list, `catalog.gamepass.com/sigls/v2?id=<list>&language=<xx-YY>&market=<CC>`: a JSON array whose first
//!    element is the list's own header (`siglId`, `title`) and the rest `{"id": "<product id>"}`. The list is
//!    [`ALL_PC_GAMES`].
//! 2. The products, `displaycatalog.mp.microsoft.com/v7.0/products?bigIds=<ids>&market=<CC>&languages=<xx-YY>`,
//!    [`BATCH`] ids a request (a URL of about 1.4 KB): each product's name, its PC package's family name (what an
//!    installed Xbox game is known by, so the two are one game), who made and published it, when it came out, and its pictures. A bundle
//!    has no package of its own: its products (`BundledSkus`) are asked for too, and its family name is theirs.
//!
//! **Politeness**: the catalogue of a region is fetched at most once a day ([`EVERY`]). The list sent no `ETag` or
//! `Last-Modified` when checked (2026-11-18), so no request can be conditional today; where it gives an `ETag` it is
//! kept and sent back (`If-None-Match`), and a 304 keeps the products kept. A fetch that fails is said in one line
//! and the kept copy is used; it is not tried again for an hour ([`AGAIN`]).
//!
//! **A local catalogue** stands in for Microsoft's where a test says (`--gamepass <folder>`): `<folder>/<CC>/sigl.json`
//! and `<folder>/<CC>/products.json`, the two answers in their real shapes, read as they are, with no request made.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use model::game_pass::{Catalogue, Locale, Product};
use model::OnlineArt;
use serde_json::Value;

use crate::art::steamgriddb::{agent, transport, Error};
use crate::art::store_online::microsoft_product_picture;
use crate::art::Kind;

/// The PC Game Pass "All PC Games" list with EA Play's, the one xbox.com's own catalogue page lists PC games by
/// (docs/research/game-pass.md, where the other lists are).
pub const ALL_PC_GAMES: &str = "609d944c-d395-4c0a-9ea4-e9f39b52c1ad";

/// How many products one request to the display catalogue asks for.
pub const BATCH: usize = 100;

/// How long a fetched catalogue is kept before it is asked for again: a day.
pub const EVERY: u64 = 86_400;

/// How long after a fetch that failed it is tried again: an hour.
pub const AGAIN: u64 = 3600;

/// The folder of the data folder the catalogues are kept in, one file a market (`GB.json`).
pub const FOLDER: &str = "gamepass";

/// Where the catalogue is asked for: Microsoft's own services, or a test's stand-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hosts {
    /// `catalog.gamepass.com`, the lists.
    pub lists: String,
    /// The Microsoft Store's display catalogue, the products.
    pub catalogue: String,
}

impl Default for Hosts {
    fn default() -> Self {
        Self {
            lists: "https://catalog.gamepass.com".into(),
            catalogue: "https://displaycatalog.mp.microsoft.com".into(),
        }
    }
}

impl Hosts {
    /// Both at one stand-in: `<base>/gamepass` and `<base>/microsoft`.
    pub fn all_at(base: &str) -> Self {
        let base = base.trim_end_matches('/');
        Self { lists: format!("{base}/gamepass"), catalogue: format!("{base}/microsoft") }
    }
}

/// Where the catalogue comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum From {
    /// Microsoft's services (or a stand-in's), kept in this folder (the data folder's [`FOLDER`]; none: not kept).
    Online { hosts: Hosts, kept: Option<PathBuf> },
    /// A local catalogue, `<folder>/<CC>/sigl.json` and `products.json` (`--gamepass`): no request.
    Folder(PathBuf),
}

/// The catalogue's games of `locale` at the time `now`: the kept copy while it is less than a day old, else fetched
/// again (and kept), else the kept copy whatever its age. None where there is none at all. Says in one line where
/// it came from.
pub fn catalogue(from: &From, locale: &Locale, now: u64) -> Option<Catalogue> {
    let place = format!("{} ({})", locale.market, locale.language);
    match from {
        From::Folder(dir) => {
            let at = dir.join(&locale.market);
            let read = |name: &str| -> Option<Value> {
                let text = std::fs::read_to_string(at.join(name)).ok()?;
                serde_json::from_str(&text).ok()
            };
            let (Some(sigl), Some(answer)) = (read("sigl.json"), read("products.json")) else {
                eprintln!("gamepass: no catalogue for {place} in {}", dir.display());
                return None;
            };
            let found = products(&answer, "https:");
            let products = in_order(&ids(&sigl), bundles_resolved(found.clone(), &found));
            eprintln!("gamepass: {} games for {place} from {}", products.len(), dir.display());
            Some(Catalogue {
                market: locale.market.clone(),
                language: locale.language.clone(),
                fetched: now,
                products,
                ..Catalogue::default()
            })
        }
        From::Online { hosts, kept } => {
            let file = kept.as_ref().map(|dir| dir.join(format!("{}.json", locale.market)));
            let had = file.as_deref().and_then(read_kept).filter(|had| had.language == locale.language);
            let fresh = |had: &Catalogue| had.fetched > 0 && now.saturating_sub(had.fetched) < EVERY;
            let tried = |had: &Catalogue| had.tried > 0 && now.saturating_sub(had.tried) < AGAIN;
            if let Some(had) = had.as_ref().filter(|had| fresh(had) || tried(had)) {
                let since = if fresh(had) { "fetched" } else { "kept; the last fetch failed" };
                eprintln!("gamepass: {} games for {place}, {since} {} ago", had.products.len(), ago(now, had));
                return Some(had.clone()).filter(|had| had.fetched > 0);
            }
            let started = Instant::now();
            match fetch(hosts, locale, had.as_ref().and_then(|had| had.etag.as_deref())) {
                Ok(Fetched::Same) => {
                    let kept = Catalogue { fetched: now, tried: 0, ..had.clone().unwrap_or_default() };
                    eprintln!(
                        "gamepass: {} games for {place}: the list is as it was ({} ms)",
                        kept.products.len(),
                        started.elapsed().as_millis()
                    );
                    keep(file.as_deref(), &kept);
                    Some(kept)
                }
                Ok(Fetched::New { etag, products }) => {
                    let got = Catalogue {
                        market: locale.market.clone(),
                        language: locale.language.clone(),
                        fetched: now,
                        tried: 0,
                        etag,
                        products,
                    };
                    eprintln!(
                        "gamepass: {} games for {place} fetched in {} ms",
                        got.products.len(),
                        started.elapsed().as_millis()
                    );
                    keep(file.as_deref(), &got);
                    Some(got)
                }
                Err(why) => {
                    let mut kept = had.unwrap_or_else(|| Catalogue {
                        market: locale.market.clone(),
                        language: locale.language.clone(),
                        ..Catalogue::default()
                    });
                    kept.tried = now;
                    let using = match kept.fetched {
                        0 => "none is kept".to_string(),
                        _ => format!("the kept copy of {} games is used", kept.products.len()),
                    };
                    eprintln!("gamepass: the catalogue for {place} couldn't be fetched ({why}): {using}");
                    keep(file.as_deref(), &kept);
                    Some(kept).filter(|kept| kept.fetched > 0)
                }
            }
        }
    }
}

/// `5 hours`, how long ago the kept copy was fetched or tried.
fn ago(now: u64, had: &Catalogue) -> String {
    let seconds = now.saturating_sub(had.fetched.max(had.tried));
    match seconds {
        0..120 => format!("{seconds} s"),
        120..7200 => format!("{} minutes", seconds / 60),
        _ => format!("{} hours", seconds / 3600),
    }
}

fn read_kept(file: &Path) -> Option<Catalogue> {
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

/// Writes the catalogue where it is kept, whole or not at all (a file beside it, then renamed over it).
fn keep(file: Option<&Path>, catalogue: &Catalogue) {
    let Some(file) = file else { return };
    let written = (|| -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let part = file.with_extension("json.part");
        std::fs::write(&part, serde_json::to_vec(catalogue)?)?;
        std::fs::rename(&part, file)
    })();
    if let Err(e) = written {
        eprintln!("gamepass: {} couldn't be written: {e}", file.display());
    }
}

/// What a fetch found.
#[derive(Debug, Clone, PartialEq)]
pub enum Fetched {
    /// The list is as it was when it gave this `ETag`: what is kept stands.
    Same,
    New {
        etag: Option<String>,
        products: Vec<Product>,
    },
}

/// Asks for the list (conditionally, with the `ETag` it gave last) and then its products, [`BATCH`] at a time.
pub fn fetch(hosts: &Hosts, locale: &Locale, etag: Option<&str>) -> Result<Fetched, String> {
    let agent = agent();
    let say = |e: Error| match e {
        Error::Failed(why) => why,
        Error::Busy(_) => "asked to wait".to_string(),
        Error::KeyRefused => "refused".to_string(),
    };
    let lists = "the Game Pass catalogue";
    let url = format!(
        "{}/sigls/v2?id={ALL_PC_GAMES}&language={}&market={}",
        hosts.lists,
        query(&locale.language),
        query(&locale.market)
    );
    let mut request = agent.get(&url);
    if let Some(etag) = etag {
        request = request.header("If-None-Match", etag);
    }
    let mut response = request.call().map_err(|e| say(transport(e, lists)))?;
    let status = response.status().as_u16();
    if status == 304 && etag.is_some() {
        return Ok(Fetched::Same);
    }
    if !(200..300).contains(&status) {
        return Err(format!("{lists} answered {status}"));
    }
    let etag = response.headers().get("etag").and_then(|value| value.to_str().ok()).map(str::to_string);
    let text = response.body_mut().read_to_string().map_err(|e| say(transport(e, lists)))?;
    let sigl: Value = serde_json::from_str(&text).map_err(|_| format!("{lists}'s reply is not JSON"))?;
    let ids = ids(&sigl);
    if !sigl.is_array() {
        return Err(format!("{lists}'s reply is not a list"));
    }
    let scheme = hosts.catalogue.split_once("//").map_or("https:", |(scheme, _)| scheme);
    let asked = |ids: &[String]| -> Result<Vec<Product>, String> {
        let store = "the Microsoft Store";
        let mut found = Vec::new();
        for (i, batch) in ids.chunks(BATCH).enumerate() {
            if i > 0 {
                // A quarter of a second between two requests to one host, as the art lookup keeps.
                std::thread::sleep(Duration::from_millis(250));
            }
            let url = format!(
                "{}/v7.0/products?bigIds={}&market={}&languages={}",
                hosts.catalogue,
                batch.join(","),
                query(&locale.market),
                query(&locale.language)
            );
            let mut response = agent.get(&url).call().map_err(|e| say(transport(e, store)))?;
            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                return Err(format!("{store} answered {status}"));
            }
            let text = response.body_mut().read_to_string().map_err(|e| say(transport(e, store)))?;
            let answer: Value = serde_json::from_str(&text).map_err(|_| format!("{store}'s reply is not JSON"))?;
            found.extend(products(&answer, scheme));
        }
        Ok(found)
    };
    let found = asked(&ids)?;
    // A bundle's family name is its products': those not in the list are asked for too.
    let mut parts: Vec<String> = Vec::new();
    for product in found.iter().filter(|product| product.family.is_none()) {
        for id in &product.bundled {
            if !ids.contains(id) && !parts.contains(id) {
                parts.push(id.clone());
            }
        }
    }
    let mut known = if parts.is_empty() { Vec::new() } else { asked(&parts)? };
    known.extend(found.iter().cloned());
    Ok(Fetched::New { etag, products: in_order(&ids, bundles_resolved(found, &known)) })
}

/// Each bundle with no family name of its own given the first of its products' that has one, from `known`.
fn bundles_resolved(mut products: Vec<Product>, known: &[Product]) -> Vec<Product> {
    for product in products.iter_mut().filter(|product| product.family.is_none()) {
        product.family = product.bundled.iter().find_map(|id| {
            known.iter().find(|part| part.id.eq_ignore_ascii_case(id)).and_then(|part| part.family.clone())
        });
    }
    products
}

/// A market's code or a language tag as a query's value: only letters, digits and `-`.
fn query(text: &str) -> String {
    text.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

/// The list's product ids, in its order, each once: every element but the header that has an `id` of twelve letters
/// and digits.
pub fn ids(sigl: &Value) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for item in sigl.as_array().into_iter().flatten() {
        let Some(id) = item.get("id").and_then(Value::as_str) else { continue };
        let id = id.trim().to_ascii_uppercase();
        if id.len() == 12 && id.bytes().all(|b| b.is_ascii_alphanumeric()) && !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// The products in the list's order; one the catalogue did not give is left out.
fn in_order(ids: &[String], mut products: Vec<Product>) -> Vec<Product> {
    let mut ordered = Vec::with_capacity(products.len());
    for id in ids {
        if let Some(at) = products.iter().position(|product| product.id.eq_ignore_ascii_case(id)) {
            ordered.push(products.swap_remove(at));
        }
    }
    ordered
}

/// The display catalogue's answer's games (`Products[]`), each with a name. `scheme` goes before an image address
/// that starts `//`.
pub fn products(answer: &Value, scheme: &str) -> Vec<Product> {
    let list = answer.get("Products").and_then(Value::as_array).into_iter().flatten();
    list.filter_map(|product| product_of(product, scheme)).collect()
}

fn product_of(product: &Value, scheme: &str) -> Option<Product> {
    let text = |at: &str| product.pointer(at).and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty());
    let id = text("/ProductId").filter(|id| id.len() == 12 && id.bytes().all(|b| b.is_ascii_alphanumeric()))?;
    let name = text("/LocalizedProperties/0/ProductTitle")?.to_string();
    let names = |at: &str| text(at).map(|name| vec![name.to_string()]).unwrap_or_default();
    let art = |kind| microsoft_product_picture(product, &format!("{scheme}//"), kind);
    Some(Product {
        id: id.to_ascii_uppercase(),
        name,
        family: family(product),
        bundled: bundled(product),
        developers: names("/LocalizedProperties/0/DeveloperName"),
        publishers: names("/LocalizedProperties/0/PublisherName"),
        released: text("/MarketProperties/0/OriginalReleaseDate").and_then(unix_seconds).unwrap_or(0),
        art: OnlineArt {
            product: Some(id.to_ascii_uppercase()),
            cover: art(Kind::Cover),
            hero: art(Kind::Hero),
            header: art(Kind::Header),
            square: art(Kind::Square),
        },
    })
}

/// The product's PC package's family name: the product's own `Properties.PackageFamilyName`, else the first of its
/// SKUs' packages'.
fn family(product: &Value) -> Option<String> {
    let plain = |value: Option<&Value>| {
        value.and_then(Value::as_str).map(str::trim).filter(|name| name.contains('_')).map(str::to_string)
    };
    if let Some(name) = plain(product.pointer("/Properties/PackageFamilyName")) {
        return Some(name);
    }
    let skus = product.get("DisplaySkuAvailabilities").and_then(Value::as_array).into_iter().flatten();
    skus.flat_map(|sku| sku.pointer("/Sku/Properties/Packages").and_then(Value::as_array).into_iter().flatten())
        .find_map(|package| plain(package.get("PackageFamilyName")))
}

/// The ids of the products a bundle is made of: its SKUs' `BundledSkus[].BigId`, each once.
fn bundled(product: &Value) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let skus = product.get("DisplaySkuAvailabilities").and_then(Value::as_array).into_iter().flatten();
    let parts = skus.flat_map(|sku| sku.pointer("/Sku/Properties/BundledSkus").and_then(Value::as_array));
    for part in parts.flatten() {
        let Some(id) = part.get("BigId").and_then(Value::as_str).map(|id| id.trim().to_ascii_uppercase()) else {
            continue;
        };
        if id.len() == 12 && id.bytes().all(|b| b.is_ascii_alphanumeric()) && !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// `2021-11-09T00:00:00.0000000Z` (its date alone counts) as Unix seconds.
fn unix_seconds(date: &str) -> Option<u64> {
    let date = date.get(..10)?;
    let mut parts = date.split('-').map(|part| part.parse::<i64>().ok());
    let (year, month, day) = (parts.next()??, parts.next()??, parts.next()??);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year < 1970 {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400).ok()
}

#[cfg(test)]
mod tests;
