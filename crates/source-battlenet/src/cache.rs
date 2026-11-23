//! The Battle.net client's own catalogue and the account's licences, as the client caches them on the PC, read and
//! never written (#276; docs/research/owned-games.md 6): which games the account owns, with no sign-in of the
//! Dashboard's, and the uid, product code, launch code and name the client itself uses for every game.
//!
//! What the TV PC's files showed (read 2026-11-18 from a copy; counts only, no value of theirs is in this
//! repository):
//!
//! - **The licences.** `%LOCALAPPDATA%\Battle.net\CachedData.db` (SQLite), `key_value_store`'s row
//!   `features_cached_data_points`: JSON with `account_country`, `account_region`, `geoip_country` and `licenses`, an
//!   array of numbers (41 there). Nothing else in the file names a game.
//! - **The catalogue.** The same file's `catalog_cache` (`component`, `version`, `digest_hash`, `signature_hash`;
//!   one row there, `PUB`) names the catalogue's root by its MD5: every file of the client's `Cache` folder is named
//!   by the MD5 of its bytes, at `Cache\<2>\<2>\<md5>` (744 of 744 there). The root is plain JSON: `fragments`, a
//!   list of `{name, hash}` (227 there, 224 of them cached), each fragment plain JSON again. Not protobuf, not
//!   compressed.
//! - **A fragment** has `products` (each `{id, base, variants?}`: the base fragment's `base` has `program_id`,
//!   `name` (a key into the fragment's `strings`, by locale, `default` among them), `title_id` and `types`, a map of
//!   product type (`retail`, `ptr`, `beta`, ...) to `{uid}`); `installs`, uid to `{tact_product}` (the NGDP code,
//!   `product.db`'s and the patch server's); and `program_configuration`, program id to `run_each_rule` and
//!   `run_first_rule`: rules `{match, actions}` whose actions `add_product {product_id {id, type}}` (`id` a product's,
//!   or one of its `variants`'), `add_tag {name}` and nested `run_first_rule`, and whose `match` is `license_id` (one
//!   number, or a list of which any will do), `all_of`, `any_of`, `none_of`, `not`, `game_account {program_id,
//!   region?}`, `account_region`, `account_country`, `igr` (an internet cafe's PC), `flag`, `true`, or nothing
//!   (always). Other fragments, named `<base>_<what>` (`_gamepass`, `_cn`, `_legacyplaypanel`), only override parts
//!   of a base fragment's products for some accounts: their uids are not taken.
//! - **Proven by the files:** the catalogue's retail uid for each of its 36 games (of 41 products: 5 have no retail
//!   install) is one the client's own log names (`Game product state created: uid=...`, 36 of 36); against the
//!   built-in table (`crate::catalogue`) the cache has every one of its 27 codes, the same launch code for all 27
//!   (the `program_id`), and the same uid for 24. The 41 licences, run through the rules, give 18 retail products,
//!   every one resolving to a uid and code; 26 of the 41 licences are named by some rule.
//! - **Inferred, not proven:** that `features_cached_data_points`' `licenses` is every licence of the account (it is
//!   the client's cache for its features' targeting; 15 of its licences are named by no game's rule); that the
//!   client's rules mean what they read as (a list is any of it; a rule with no `match` always runs, as the beta and
//!   test types it adds for everyone suggest); that a game reached only through a **game account** is not owned
//!   (game accounts are named only in the client's log: see below); and that a game whose rule tags it `play_for_free` may be installed by any account, though no
//!   licence names it (7 there: their rule asks only for a game account, which the client makes on the first
//!   play). Those are listed as owned, and marked free ([`Owned::free`]).
//!
//! **Game accounts** (#331). Some games' rules ask for no licence but for a game account of the program (World of
//! Warcraft: `game_account {program_id: "WoW", region: [...]}`, plus a `subscription_license` the Dashboard does not
//! check). The cache does not hold the account's game accounts; the client's log does, as lines like
//! `Querying game level information for entity: (Game: <REGION>-<Program>-<id>)` in
//! `%LOCALAPPDATA%\Battle.net\Logs\battle.net-*.log` (next to `CachedData.db`). The newest of the last few logs that has
//! any such line says which programs and regions the account has a game account for, and `game_account` holds for
//! those (the program case-blind, a rule with no `region` for any region). A game so reached is listed as owned,
//! as the client lists it, even where the subscription has lapsed. With no log, or none with such a line, `game_account`
//! holds for no game, as before.
//!
//! **Flavours** (#338). World of Warcraft is one program and one base product (`WoW`) with a product *type* for each
//! flavour (`wow_classic`, `wow_classic_era`, `wow_classic_anniversary`, `wowf`: WoW: Forever), each with its own uid
//! (the client's log makes a product state for each), its own install (`wow_anniversary` is the NGDP code of
//! `wow_classic_anniversary`) and its own name; the program's rules add each for any game account. The types of the
//! categories in [`FLAVOUR_CATEGORIES`] are games of their own; test realms, betas and vendor builds are not. A flavour
//! launches as the product the client files it under (`override_product_id`: `WoWC` for the three Classics, `WoWF`);
//! that `launch WoWC` picks among the Classics is not confirmed. (The Bundle, `xbox-series-theme`, says nothing of Battle.net ownership or WoW.)
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

/// A game of the client's catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// The NGDP code (its `tact_product`): `product.db`'s, the game's `source_id`.
    pub code: String,
    /// The uid the client knows it by, lower case as the client writes it.
    pub uid: String,
    /// The program id, what `--exec="launch <it>"` takes.
    pub launch: String,
    pub name: String,
}

/// A game the account owns, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owned {
    pub game: Listed,
    /// Owned only as free to play: no licence names it.
    pub free: bool,
}

/// What the client's cache says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// Every game of its catalogue, by code.
    pub catalogue: Vec<Listed>,
    /// The games the account owns; None where the cache has no licences (signed out).
    pub owned: Option<Vec<Owned>>,
    /// When the client last wrote `CachedData.db`, in Unix seconds.
    pub written: u64,
}

/// The client's cache: `CachedData.db` and its `Cache` folder beside it.
pub fn read(db: &Path) -> Result<Option<Read>> {
    if !db.is_file() {
        return Ok(None);
    }
    let cache = db.with_file_name("Cache");
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(db, flags).context("not opened")?;
    let points = points(&connection)?;
    let root = root(&connection, &cache)?;
    let catalogue = Catalogue::of(&root, &cache)?;
    let games = catalogue.games();
    let owned = points.map(|mut points| {
        points.game_accounts = game_accounts(&db.with_file_name("Logs"));
        catalogue.owned(&points)
    });
    Ok(Some(Read { catalogue: games, owned, written: source_kit::owned::modified(db).unwrap_or(0) }))
}

/// What the account's rules are judged against.
#[derive(Debug, Clone, Default)]
struct Account {
    licences: HashSet<u64>,
    region: String,
    country: String,
    /// The `(program lower case, region)` of each game account the client's log names; None where it names none.
    game_accounts: Option<HashSet<(String, String)>>,
}

/// How many of the newest logs are looked through for game accounts (the newest may be a short run that never got as
/// far as querying them).
const LOGS_LOOKED_AT: usize = 5;

/// The game accounts the client's newest log (of the last few) names, as `(program lower case, region)`.
fn game_accounts(logs: &Path) -> Option<HashSet<(String, String)>> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(logs)
        .ok()?
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            name.starts_with("battle.net-") && name.ends_with(".log")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by(|a, b| b.cmp(a));
    for (_, path) in files.into_iter().take(LOGS_LOOKED_AT) {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let found = parse_game_accounts(&String::from_utf8_lossy(&bytes));
        if !found.is_empty() {
            return Some(found);
        }
    }
    None
}

/// The `(Game: <REGION>-<Program>-<id>)` entries of a log's text, as `(program lower case, region)`.
fn parse_game_accounts(log: &str) -> HashSet<(String, String)> {
    let mut found = HashSet::new();
    for line in log.lines() {
        let Some(at) = line.find("(Game: ") else { continue };
        let entry = line[at + "(Game: ".len()..].split(')').next().unwrap_or_default();
        let mut parts = entry.split('-');
        if let (Some(region), Some(program), Some(_id)) = (parts.next(), parts.next(), parts.next()) {
            if !region.is_empty() && !program.is_empty() {
                found.insert((program.to_ascii_lowercase(), region.to_string()));
            }
        }
    }
    found
}

/// `features_cached_data_points`' licences, region and country; None where it has no `licenses`.
fn points(db: &Connection) -> Result<Option<Account>> {
    let value: Option<String> = db
        .query_row("SELECT value FROM key_value_store WHERE key = ?1", [crate::client::POINTS], |row| row.get(0))
        .map(Some)
        .or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(e) })
        .context("key_value_store not read")?;
    let Some(value) = value else { return Ok(None) };
    let json: Value = serde_json::from_str(&value).context("its data points are not JSON")?;
    let Some(licences) = json.get("licenses").and_then(Value::as_array) else { return Ok(None) };
    let text = |key: &str| json.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    Ok(Some(Account {
        licences: licences.iter().filter_map(Value::as_u64).collect(),
        region: text("account_region"),
        country: text("account_country"),
        game_accounts: None,
    }))
}

/// A file of the cache by its MD5, as the client keeps it: `Cache\<2>\<2>\<md5>`.
pub(crate) fn cached(cache: &Path, hash: &str) -> Option<PathBuf> {
    let hash = hash.trim().to_ascii_lowercase();
    if hash.len() < 4 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(cache.join(&hash[..2]).join(&hash[2..4]).join(&hash))
}

/// The catalogue's root: `catalog_cache`'s `digest_hash`, the newest version whose file is there.
fn root(db: &Connection, cache: &Path) -> Result<Value> {
    let mut rows = db
        .prepare("SELECT digest_hash FROM catalog_cache ORDER BY version DESC")
        .context("no catalog_cache: the client has cached no catalogue")?;
    let hashes: Vec<Option<String>> = rows.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
    for hash in hashes.into_iter().flatten() {
        let Some(path) = cached(cache, &hash).filter(|path| path.is_file()) else { continue };
        let text = std::fs::read(&path).with_context(|| path.display().to_string())?;
        return serde_json::from_slice(&text).with_context(|| format!("{}: not JSON", path.display()));
    }
    bail!("the catalogue catalog_cache names is not in {}", cache.display())
}

/// The catalogue's categories of product type that are games to play in their own right, not test realms or builds:
/// World of Warcraft's `cat_wow_classic_live` (Classic, Classic Era, Anniversary) and `cat_wow_forever` (WoW: Forever).
/// A type of one of these categories, and not `deprecated`, is a **flavour** (#338): a rule that adds it to the
/// account gives a game of its own, with the type's own uid, code, launch product and name.
const FLAVOUR_CATEGORIES: &[&str] = &["cat_wow_classic_live", "cat_wow_forever"];

/// A product type of a base product: what it says of itself.
#[derive(Debug, Clone, Default)]
struct Kind {
    uid: Option<String>,
    /// Its own name, where it has one (`name`, a key into the fragment's strings).
    name: Option<String>,
    /// The product the client launches it as (`override_product_id`: Classic's `WoWC`).
    launch: Option<String>,
    deprecated: bool,
}

/// A product of the catalogue, from its base fragment.
#[derive(Debug, Clone)]
struct Product {
    program: String,
    name: String,
    /// Product type to what it says of itself.
    types: BTreeMap<String, Kind>,
}

/// The catalogue's fragments, put together.
#[derive(Debug, Default)]
struct Catalogue {
    /// By product id.
    products: HashMap<String, Product>,
    /// A variant's id to its product's.
    variants: HashMap<String, String>,
    /// Uid (lower case) to NGDP code.
    installs: HashMap<String, String>,
    /// Product type to its category, from the fragments' `types.definitions`.
    categories: HashMap<String, String>,
    /// Each program's rules, `run_each_rule` then `run_first_rule`.
    rules: Vec<(Vec<Value>, Vec<Value>)>,
}

impl Catalogue {
    fn of(root: &Value, cache: &Path) -> Result<Catalogue> {
        let fragments = root.get("fragments").and_then(Value::as_array).context("its root lists no fragments")?;
        let mut catalogue = Catalogue::default();
        for fragment in fragments {
            let Some(path) = fragment.get("hash").and_then(Value::as_str).and_then(|h| cached(cache, h)) else {
                continue;
            };
            // A fragment the client has not cached is passed over, as it is by the client until it fetches it.
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let json: Value =
                serde_json::from_slice(&bytes).with_context(|| format!("{}: not JSON", path.display()))?;
            catalogue.add(&json);
        }
        Ok(catalogue)
    }

    fn add(&mut self, fragment: &Value) {
        let strings = |key: &str| {
            let strings = fragment.get("strings");
            ["default", "enUS"].iter().find_map(|locale| strings?.get(locale)?.get(key)?.as_str()).map(str::to_string)
        };
        for product in fragment.get("products").and_then(Value::as_array).into_iter().flatten() {
            let (Some(id), Some(base)) = (product.get("id").and_then(Value::as_str), product.get("base")) else {
                continue;
            };
            // Only the base fragment's product has its program; the others override parts of it.
            let Some(program) = base.get("program_id").and_then(Value::as_str) else { continue };
            if self.products.contains_key(id) {
                continue;
            }
            let key = base.get("name").and_then(Value::as_str).unwrap_or_default();
            let types = base.get("types").and_then(Value::as_object).into_iter().flatten();
            let text = |t: &Value, key: &str| t.get(key).and_then(Value::as_str).map(str::to_string);
            let types = types
                .map(|(kind, t)| {
                    let name = text(t, "name").and_then(|key| strings(&key));
                    let uid = text(t, "uid").map(|uid| uid.to_ascii_lowercase());
                    let deprecated = t.get("deprecated").and_then(Value::as_bool) == Some(true);
                    (kind.clone(), Kind { uid, name, launch: text(t, "override_product_id"), deprecated })
                })
                .collect();
            let name = strings(key).unwrap_or_else(|| id.to_string());
            self.products.insert(id.to_string(), Product { program: program.to_string(), name, types });
            for variant in product.get("variants").and_then(Value::as_object).into_iter().flatten().map(|(v, _)| v) {
                self.variants.entry(variant.clone()).or_insert_with(|| id.to_string());
            }
        }
        let definitions = fragment.get("types").and_then(|t| t.get("definitions")).and_then(Value::as_array);
        for definition in definitions.into_iter().flatten() {
            if let (Some(id), Some(category)) =
                (definition.get("id").and_then(Value::as_str), definition.get("category").and_then(Value::as_str))
            {
                self.categories.entry(id.to_string()).or_insert_with(|| category.to_string());
            }
        }
        for (uid, install) in fragment.get("installs").and_then(Value::as_object).into_iter().flatten() {
            if let Some(code) = install.get("tact_product").and_then(Value::as_str) {
                self.installs.entry(uid.to_ascii_lowercase()).or_insert_with(|| code.to_string());
            }
        }
        for (_, config) in fragment.get("program_configuration").and_then(Value::as_object).into_iter().flatten() {
            let list = |key: &str| config.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
            self.rules.push((list("run_each_rule"), list("run_first_rule")));
        }
    }

    /// Whether a product type is a flavour: a game of its own (see [`FLAVOUR_CATEGORIES`]).
    fn flavour(&self, kind: &str, of: &Kind) -> bool {
        !of.deprecated && self.categories.get(kind).is_some_and(|c| FLAVOUR_CATEGORIES.contains(&c.as_str()))
    }

    /// The game a product id (or a variant's) is as product type `kind`: by its retail install for `retail`, by the
    /// flavour's own uid (which it must name) for a flavour; None for any other type, and where it has no install.
    fn game(&self, id: &str, kind: &str) -> Option<Listed> {
        let id = if self.products.contains_key(id) { id } else { self.variants.get(id)? };
        let product = &self.products[id];
        let of = product.types.get(kind);
        let (uid, launch, name) = if kind == "retail" {
            let uid = of.and_then(|k| k.uid.clone()).unwrap_or_else(|| id.to_ascii_lowercase());
            (uid, product.program.clone(), product.name.clone())
        } else {
            let of = of.filter(|of| self.flavour(kind, of))?;
            let name = of.name.clone().unwrap_or_else(|| product.name.clone());
            (of.uid.clone()?, of.launch.clone().unwrap_or_else(|| product.program.clone()), edition_name(kind, name))
        };
        let code = self.installs.get(&uid)?.clone();
        if crate::product::clients_own(&uid) {
            return None;
        }
        Some(Listed { code, uid, launch, name })
    }

    /// Every game, by code, once: each product's retail game and each flavour of it.
    fn games(&self) -> Vec<Listed> {
        let mut games: Vec<Listed> = Vec::new();
        for (id, product) in &self.products {
            let kinds = product.types.iter().filter(|(k, of)| self.flavour(k, of)).map(|(k, _)| k.as_str());
            games.extend(std::iter::once("retail").chain(kinds).filter_map(|kind| self.game(id, kind)));
        }
        games.sort_by(|a, b| a.code.cmp(&b.code));
        games.dedup_by(|a, b| a.code == b.code);
        games
    }

    /// The games the account owns: the retail products its licences' rules add, then those a rule tags
    /// `play_for_free` (taken as anyone's), by code, once.
    fn owned(&self, account: &Account) -> Vec<Owned> {
        let mut owned: Vec<Owned> = Vec::new();
        for free in [false, true] {
            let judge = Judge { account, free };
            let mut added = Vec::new();
            for (each, first) in &self.rules {
                judge.run(each, false, &mut added);
                judge.run(first, true, &mut added);
            }
            for (id, kind, tags) in added {
                if free && !tags.iter().any(|t| t == "play_for_free") {
                    continue;
                }
                let Some(game) = self.game(&id, &kind) else { continue };
                if !owned.iter().any(|o| o.game.code == game.code) {
                    owned.push(Owned { game, free });
                }
            }
        }
        owned.sort_by(|a, b| a.game.code.cmp(&b.game.code));
        owned
    }
}

/// A flavour's name as the Dashboard lists it. The client's own string for Classic Era is "World of Warcraft Classic",
/// which reads as the Classic of the day (Mists of Pandaria Classic) and would be taken for it by a search.
fn edition_name(kind: &str, name: String) -> String {
    match kind {
        k if k.ends_with("_era") && !name.to_ascii_lowercase().contains("era") => format!("{name} Era"),
        _ => name,
    }
}

/// The client's rules, judged for an account: `free`, taking every game account as there (the pass that finds the
/// games free to play); otherwise a game account is there where the client's log named it.
struct Judge<'a> {
    account: &'a Account,
    free: bool,
}

impl Judge<'_> {
    /// Runs `rules`, each that matches (or with `first`, the first that does), adding `(product id, type, the tags
    /// its actions add)` to `added`.
    fn run(&self, rules: &[Value], first: bool, added: &mut Vec<(String, String, Vec<String>)>) {
        for rule in rules {
            if !self.holds(rule.get("match")) {
                continue;
            }
            let actions = rule.get("actions").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
            let tags: Vec<String> =
                actions.iter().filter_map(|a| a.get("add_tag")?.get("name")?.as_str().map(str::to_string)).collect();
            for action in actions {
                if let Some(product) = action.get("add_product").and_then(|a| a.get("product_id")) {
                    let text = |key: &str| product.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
                    added.push((text("id"), text("type"), tags.clone()));
                }
                if let Some(nested) = action.get("run_first_rule").and_then(Value::as_array) {
                    self.run(nested, true, added);
                }
            }
            if first {
                return;
            }
        }
    }

    /// Whether a rule's `match` holds: nothing, or `true`, always; an object, when every one of its keys does.
    fn holds(&self, matching: Option<&Value>) -> bool {
        match matching {
            None | Some(Value::Null) => true,
            Some(Value::Bool(b)) => *b,
            Some(Value::Object(keys)) => keys.iter().all(|(key, value)| self.key(key, value)),
            Some(_) => false,
        }
    }

    /// Whether the account has a game account for `{program_id, region?}`: the program case-blind, no region for any.
    fn game_account(&self, value: &Value) -> bool {
        let Some(accounts) = &self.account.game_accounts else { return false };
        let Some(program) = value.get("program_id").and_then(Value::as_str) else { return false };
        let program = program.to_ascii_lowercase();
        let regions: Vec<&str> = match value.get("region") {
            Some(Value::Array(r)) => r.iter().filter_map(Value::as_str).collect(),
            Some(Value::String(r)) => vec![r.as_str()],
            _ => Vec::new(),
        };
        accounts.iter().any(|(p, r)| *p == program && (regions.is_empty() || regions.iter().any(|x| x == r)))
    }

    fn key(&self, key: &str, value: &Value) -> bool {
        let list = |value: &Value| -> Vec<Value> {
            match value {
                Value::Array(items) => items.clone(),
                other => vec![other.clone()],
            }
        };
        let named = |value: &Value, what: &str| list(value).iter().any(|v| v.as_str().is_some_and(|v| v == what));
        match key {
            "license_id" => list(value).iter().filter_map(Value::as_u64).any(|id| self.account.licences.contains(&id)),
            "all_of" => list(value).iter().all(|m| self.holds(Some(m))),
            "any_of" => list(value).iter().any(|m| self.holds(Some(m))),
            "none_of" => !list(value).iter().any(|m| self.holds(Some(m))),
            "not" => !self.holds(Some(value)),
            "account_region" => named(value, &self.account.region),
            "account_country" => named(value, &self.account.country),
            // A PC of an internet cafe's licence: this is none.
            "igr" => value.as_bool() == Some(false),
            // `{program_id, region?}`: the client's log names the game accounts (see the header); with none known,
            // only the pass that finds the free games takes it as held.
            "game_account" => self.free || self.game_account(value),
            // Anything else (`flag`, what a later client adds): not taken as held.
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A made-up client cache at `dir` (`fixture::write_client_cache`): `licences` (None: no `licenses` at all)
    /// and a catalogue of `fragments`.
    fn write(dir: &Path, licences: Option<&[u64]>, fragments: &[(&str, Value)]) {
        let mut points =
            json!({"account_country": "NZL", "account_id": 1, "account_region": "EU", "geoip_country": "NZ"});
        if let Some(licences) = licences {
            points["licenses"] = json!(licences);
        }
        let fragments: Vec<(String, Value)> = fragments.iter().map(|(n, f)| (n.to_string(), f.clone())).collect();
        crate::fixture::write_client_cache(&dir.join("CachedData.db"), &points, Some(&fragments)).unwrap();
    }

    fn rule(licence: Value, product: &str, kind: &str) -> Value {
        json!({"match": {"license_id": licence}, "actions": [{"add_product": {"product_id": {"id": product, "type": kind}}}]})
    }

    /// A base fragment of one program: its product's retail uid `uid` (None: none named) installed as `code`.
    fn fragment(program: &str, uid: Option<&str>, code: &str, name: &str, rules: Value) -> Value {
        let retail = match uid {
            Some(uid) => json!({"uid": uid}),
            None => json!({}),
        };
        let install = uid.unwrap_or(&program.to_ascii_lowercase()).to_ascii_lowercase();
        json!({
            "fragment_id": program.to_ascii_lowercase(),
            "products": [{"id": program, "base": {"program_id": program, "name": format!("x#{program}_NAME"),
                "types": {"retail": retail, "ptr": {"uid": format!("{install}_ptr")}}}}],
            "installs": {install.clone(): {"tact_product": code}, format!("{install}_ptr"): {"tact_product": format!("{code}t")}},
            "strings": {"default": {format!("x#{program}_NAME"): name}},
            "program_configuration": {program: rules},
            "version": 1
        })
    }

    #[test]
    fn the_licences_rules_give_the_owned_games_and_the_free_ones() {
        let dir = tempfile::tempdir().unwrap();
        let fragments = [
            // Owned by its licence, a list of which any will do; its uid named in upper case, as one was.
            (
                "aster",
                fragment(
                    "ASTR",
                    Some("ASTR"),
                    "astr",
                    "Aster Vale",
                    json!({"run_each_rule": [rule(json!([7, 70001]), "ASTR", "retail")]}),
                ),
            ),
            // Not owned: its licence is not the account's; a beta everyone has, which is not a game owned.
            (
                "birch",
                fragment(
                    "BRCH",
                    Some("birch"),
                    "brch",
                    "Birch Run",
                    json!({"run_each_rule": [
                rule(json!(70002), "BRCH", "retail"), {"actions": [{"add_product": {"product_id": {"id": "BRCH", "type": "ptr"}}}]}]}),
                ),
            ),
            // Free to play: a game account (which the files never say) gives it, tagged play_for_free.
            (
                "cedar",
                fragment(
                    "CDR",
                    Some("cedar"),
                    "cdr",
                    "Cedar Arena",
                    json!({"run_each_rule": [
                {"match": {"game_account": {"program_id": "CDR"}}, "actions": [
                    {"add_product": {"product_id": {"id": "CDR", "type": "retail"}}}, {"add_tag": {"name": "play_for_free"}}]}]}),
                ),
            ),
            // A trial through a game account, not tagged free: not owned.
            (
                "dune",
                fragment(
                    "DUN",
                    Some("dune"),
                    "dun",
                    "Dune Hollow",
                    json!({"run_first_rule": [
                rule(json!(70003), "DUN", "retail"),
                {"match": {"game_account": {"program_id": "DUN"}}, "actions": [
                    {"add_product": {"product_id": {"id": "DUN", "type": "retail"}}}, {"add_tag": {"name": "trial"}}]}]}),
                ),
            ),
            // Owned through a variant, in a first rule: the first that holds alone, with every kind of match.
            ("elm", {
                let mut f = fragment(
                    "ELM",
                    None,
                    "elm",
                    "Elm Reach",
                    json!({"run_first_rule": [
                    {"match": {"all_of": [{"license_id": 70004}, {"none_of": [{"license_id": [7]}]}]},
                     "actions": [{"add_product": {"product_id": {"id": "ELM_Deluxe", "type": "retail"}}}]},
                    {"match": {"all_of": [{"any_of": [{"license_id": 7}, {"igr": true}]}, {"not": {"account_region": ["CN"]}},
                        {"account_country": "NZL"}]},
                     "actions": [{"run_first_rule": [rule(json!(1), "ELM_Standard", "retail")]}]}]}),
                );
                f["products"][0]["variants"] = json!({"ELM_Deluxe": {}, "ELM_Standard": {}});
                f
            }),
            // An override fragment of Aster's: its uid is not taken.
            (
                "aster_cn",
                json!({"fragment_id": "aster_cn", "products": [{"id": "ASTR", "base": {"types": {"retail": {"uid": "astr_cn"}}}}],
                "installs": {"astr_cn": {"tact_product": "astrcn"}}}),
            ),
        ];
        write(dir.path(), Some(&[1, 7, 99]), &fragments);
        let read = read(&dir.path().join("CachedData.db")).unwrap().unwrap();
        let codes: Vec<(&str, &str, &str, &str)> = read
            .catalogue
            .iter()
            .map(|g| (g.code.as_str(), g.uid.as_str(), g.launch.as_str(), g.name.as_str()))
            .collect();
        assert_eq!(
            codes,
            [
                ("astr", "astr", "ASTR", "Aster Vale"),
                ("brch", "birch", "BRCH", "Birch Run"),
                ("cdr", "cedar", "CDR", "Cedar Arena"),
                ("dun", "dune", "DUN", "Dune Hollow"),
                ("elm", "elm", "ELM", "Elm Reach"),
            ]
        );
        let owned: Vec<(&str, bool)> =
            read.owned.as_ref().unwrap().iter().map(|o| (o.game.code.as_str(), o.free)).collect();
        assert_eq!(owned, [("astr", false), ("cdr", true), ("elm", false)]);
        assert!(read.written > 0);

        // No licences in the file (signed out): the catalogue, ownership unknown.
        let none = tempfile::tempdir().unwrap();
        write(none.path(), None, &fragments);
        let read = super::read(&none.path().join("CachedData.db")).unwrap().unwrap();
        assert_eq!((read.catalogue.len(), read.owned), (5, None));
        // No licences at all: the free games alone.
        let empty = tempfile::tempdir().unwrap();
        write(empty.path(), Some(&[]), &fragments);
        let read = super::read(&empty.path().join("CachedData.db")).unwrap().unwrap();
        assert_eq!(read.owned.unwrap().iter().map(|o| o.game.code.as_str()).collect::<Vec<_>>(), ["cdr"]);
    }

    /// A made-up log line of the client's, for the game account `entity` (`REGION-Program-id`).
    fn line(entity: &str) -> String {
        format!("D 2026-01-01 00:00:00.000 [GSAccountProvider] {{Main}} Querying game level information for entity: (Game: {entity})\n")
    }

    #[test]
    fn the_logs_game_accounts_are_parsed() {
        let log = format!(
            "noise\n{}{}{}D x Querying game time info for entity Id: (Game: US-Ash-9)\n(Game: broken)\n(Game: -X-1)\n",
            line("US-Ash-1"),
            line("GLOBAL-Fen-2"),
            line("EU-Ash-3")
        );
        let found = parse_game_accounts(&log);
        let mut found: Vec<_> = found.into_iter().collect();
        found.sort();
        let pair = |p: &str, r: &str| (p.to_string(), r.to_string());
        assert_eq!(found, [pair("ash", "EU"), pair("ash", "US"), pair("fen", "GLOBAL")]);
        assert!(parse_game_accounts("nothing here").is_empty());
    }

    #[test]
    fn a_game_account_in_the_log_gives_the_game_and_without_one_it_is_as_before() {
        let wow = |region: Value| {
            fragment(
                "WoW",
                Some("wow"),
                "wow",
                "Realm Quest",
                json!({"is_game_account_level": true, "subscription_license": 1, "run_each_rule": [
                    {"match": {"game_account": {"program_id": "WoW", "region": region}},
                     "actions": [{"add_product": {"product_id": {"id": "WoW", "type": "retail"}}}]}]}),
            )
        };
        let owned_codes = |logs: &[(&str, String)], fragments: &[(&str, Value)]| {
            let dir = tempfile::tempdir().unwrap();
            write(dir.path(), Some(&[]), fragments);
            if !logs.is_empty() {
                std::fs::create_dir(dir.path().join("Logs")).unwrap();
            }
            for (name, text) in logs {
                std::fs::write(dir.path().join("Logs").join(name), text).unwrap();
            }
            let read = read(&dir.path().join("CachedData.db")).unwrap().unwrap();
            read.owned.unwrap().into_iter().map(|o| (o.game.code, o.free)).collect::<Vec<_>>()
        };
        let regions = json!(["US", "EU"]);
        let fragments = [("wow", wow(regions.clone()))];
        // A game account of the program in a listed region: owned, not free.
        let log = ("battle.net-1.log", line("US-WoW-5"));
        assert_eq!(owned_codes(&[log], &fragments), [("wow".to_string(), false)]);
        // Program case-blind; a rule naming no region takes any.
        let log2 = ("battle.net-1.log", line("XX-wow-5"));
        assert_eq!(owned_codes(&[log2], &fragments), []);
        assert_eq!(owned_codes(&[("battle.net-1.log", line("XX-wow-5"))], &[("wow", wow(Value::Null))]).len(), 1);
        // Another program's, or another region's: not owned.
        assert_eq!(owned_codes(&[("battle.net-1.log", line("US-Ash-5"))], &fragments), []);
        assert_eq!(owned_codes(&[("battle.net-1.log", line("KR-WoW-5"))], &fragments), []);
        // No log, or a log with no such line: as before.
        assert_eq!(owned_codes(&[], &fragments), []);
        assert_eq!(owned_codes(&[("battle.net-1.log", "nothing\n".to_string())], &fragments), []);
        // A log that is not the client's is not read.
        assert_eq!(owned_codes(&[("other.log", line("US-WoW-5"))], &fragments), []);
    }

    /// Flavours (#338): one program, one base product, several product types that are games of their own (a live
    /// category), each with its own uid, install, launch product and name; the test realms and the deprecated left out.
    #[test]
    fn each_flavour_a_game_account_grants_is_a_game_of_its_own() {
        let flavour =
            |uid: &str, name: &str, launch: &str| json!({"uid": uid, "name": name, "override_product_id": launch});
        let fragment = json!({
            "fragment_id": "quest",
            "products": [
                {"id": "QST", "base": {"program_id": "QST", "name": "x#QST", "types": {
                    "retail": {},
                    "q_classic": flavour("q_classic", "x#QCLASSIC", "QSTC"),
                    "q_era": flavour("q_era", "x#QCLASSIC_BASE", "QSTC"),
                    "q_forever": flavour("q_forever", "x#QFOREVER", "QSTF"),
                    "q_old": {"uid": "q_old", "name": "x#QOLD", "deprecated": true},
                    "q_ptr": {"uid": "q_ptr", "name": "x#QPTR"},
                    "q_dev": {"uid": "q_dev"}}},
                 "variants": {"QSTNoUpgrade": {}}},
            ],
            "installs": {"qst": {"tact_product": "qst"}, "q_classic": {"tact_product": "q_classic"},
                "q_era": {"tact_product": "q_classic_era"}, "q_forever": {"tact_product": "qstf"},
                "q_old": {"tact_product": "qo"}, "q_ptr": {"tact_product": "qp"}, "q_dev": {"tact_product": "qd"}},
            "types": {"definitions": [
                {"id": "q_classic", "category": "cat_wow_classic_live"},
                {"id": "q_era", "category": "cat_wow_classic_live"},
                {"id": "q_forever", "category": "cat_wow_forever"},
                {"id": "q_old", "category": "cat_wow_forever"},
                {"id": "q_ptr", "category": "cat_in_development"}]},
            "strings": {"default": {"x#QST": "Quest of Realms", "x#QCLASSIC": "Quest Classic",
                "x#QCLASSIC_BASE": "Quest of Realms Classic", "x#QFOREVER": "Quest: Forever",
                "x#QOLD": "Quest Old", "x#QPTR": "PTR"}},
            "program_configuration": {"QST": {"is_game_account_level": true, "run_each_rule": [
                {"match": {"game_account": {"program_id": "QST", "region": ["US"]}}, "actions": [
                    {"add_product": {"product_id": {"id": "QST", "type": "q_classic"}}},
                    {"add_product": {"product_id": {"id": "QST", "type": "q_era"}}},
                    {"add_product": {"product_id": {"id": "QST", "type": "q_ptr"}}},
                    {"add_product": {"product_id": {"id": "QST", "type": "q_old"}}},
                    {"add_product": {"product_id": {"id": "QST", "type": "q_dev"}}},
                    {"add_product": {"product_id": {"id": "QST", "type": "retail"}}}]},
                {"match": {"game_account": {"program_id": "QST", "region": ["US"]}}, "actions": [
                    {"add_product": {"product_id": {"id": "QSTNoUpgrade", "type": "q_forever"}}}]}]}},
            "version": 1
        });
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), Some(&[]), &[("quest", fragment)]);
        std::fs::create_dir(dir.path().join("Logs")).unwrap();
        std::fs::write(dir.path().join("Logs").join("battle.net-1.log"), line("US-QST-5")).unwrap();
        let read = read(&dir.path().join("CachedData.db")).unwrap().unwrap();
        let games: Vec<(&str, &str, &str, &str)> = read
            .owned
            .as_ref()
            .unwrap()
            .iter()
            .map(|o| (o.game.code.as_str(), o.game.uid.as_str(), o.game.launch.as_str(), o.game.name.as_str()))
            .collect();
        assert_eq!(
            games,
            [
                ("q_classic", "q_classic", "QSTC", "Quest Classic"),
                ("q_classic_era", "q_era", "QSTC", "Quest of Realms Classic Era"),
                ("qst", "qst", "QST", "Quest of Realms"),
                ("qstf", "q_forever", "QSTF", "Quest: Forever"),
            ]
        );
        // The catalogue lists the same four: no test realm, no deprecated type, no type of no category.
        assert_eq!(read.catalogue.len(), 4);
        // With no game account known, none of them is owned.
        std::fs::remove_file(dir.path().join("Logs").join("battle.net-1.log")).unwrap();
        assert!(super::read(&dir.path().join("CachedData.db")).unwrap().unwrap().owned.unwrap().is_empty());
    }

    #[test]
    fn no_cache_is_nothing_and_a_broken_one_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(&dir.path().join("CachedData.db")).unwrap(), None);
        // The cache of the country alone (no catalogue cached): an error, said.
        let db = Connection::open(dir.path().join("CachedData.db")).unwrap();
        db.execute_batch("CREATE TABLE key_value_store (key TEXT, value TEXT)").unwrap();
        let e = read(&dir.path().join("CachedData.db")).unwrap_err();
        assert!(format!("{e:#}").contains("no catalog_cache"), "{e:#}");
        // The root named but not cached.
        db.execute_batch("CREATE TABLE catalog_cache (component, version, digest_hash, signature_hash)").unwrap();
        db.execute("INSERT INTO catalog_cache VALUES ('PUB', 1, '00112233445566778899aabbccddeeff', '')", []).unwrap();
        let e = read(&dir.path().join("CachedData.db")).unwrap_err();
        assert!(format!("{e:#}").contains("is not in"), "{e:#}");
        // Not a database at all.
        std::fs::write(dir.path().join("bad.db"), b"not sqlite").unwrap();
        assert!(read(&dir.path().join("bad.db")).is_err());
    }
}
