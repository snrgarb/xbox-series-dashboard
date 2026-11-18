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
//!   (game accounts are named only in the client's log, not read: a subscription's World of Warcraft, a trial's
//!   Diablo III); and that a game whose rule tags it `play_for_free` may be installed by any account, though no
//!   licence names it (7 there: their rule asks only for a game account, which the client makes on the first
//!   play). Those are listed as owned, and marked free ([`Owned::free`]).
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
    let owned = points.map(|points| catalogue.owned(&points));
    Ok(Some(Read { catalogue: games, owned, written: source_kit::owned::modified(db).unwrap_or(0) }))
}

/// What the account's rules are judged against.
#[derive(Debug, Clone, Default)]
struct Account {
    licences: HashSet<u64>,
    region: String,
    country: String,
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

/// A product of the catalogue, from its base fragment.
#[derive(Debug, Clone)]
struct Product {
    program: String,
    name: String,
    /// Product type to uid, where the type names one.
    types: BTreeMap<String, Option<String>>,
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
            let types = types
                .map(|(kind, t)| (kind.clone(), t.get("uid").and_then(Value::as_str).map(str::to_ascii_lowercase)))
                .collect();
            let name = strings(key).unwrap_or_else(|| id.to_string());
            self.products.insert(id.to_string(), Product { program: program.to_string(), name, types });
            for variant in product.get("variants").and_then(Value::as_object).into_iter().flatten().map(|(v, _)| v) {
                self.variants.entry(variant.clone()).or_insert_with(|| id.to_string());
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

    /// The game a product id (or a variant's) is, by its retail install: None where it has none.
    fn game(&self, id: &str) -> Option<Listed> {
        let id = if self.products.contains_key(id) { id } else { self.variants.get(id)? };
        let product = &self.products[id];
        let uid = product.types.get("retail")?.clone().unwrap_or_else(|| id.to_ascii_lowercase());
        let code = self.installs.get(&uid)?.clone();
        if crate::product::clients_own(&uid) {
            return None;
        }
        Some(Listed { code, uid, launch: product.program.clone(), name: product.name.clone() })
    }

    /// Every game, by code, once.
    fn games(&self) -> Vec<Listed> {
        let mut games: Vec<Listed> = self.products.keys().filter_map(|id| self.game(id)).collect();
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
                if kind != "retail" || (free && !tags.iter().any(|t| t == "play_for_free")) {
                    continue;
                }
                let Some(game) = self.game(&id) else { continue };
                if !owned.iter().any(|o| o.game.code == game.code) {
                    owned.push(Owned { game, free });
                }
            }
        }
        owned.sort_by(|a, b| a.game.code.cmp(&b.game.code));
        owned
    }
}

/// The client's rules, judged for an account: `free`, taking every game account as there (the pass that finds the
/// games free to play).
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
            // Not in the files the Dashboard reads (the client's log alone names them).
            "game_account" => self.free,
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
