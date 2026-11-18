//! Battle.net's `%PROGRAMDATA%\Battle.net\Agent\product.db`: the products its agent has installed or is
//! installing, protobuf (docs/research/stores.md, "Battle.net"; field numbers from TACTLib's schema):
//!
//! ```proto
//! message Database { repeated ProductInstall product_install = 1; ... }
//! message ProductInstall {
//!   string uid = 1; string product_code = 2; UserSettings settings = 3;   // install_path = 1
//!   CachedProductState cached_product_state = 4;                          // base_product_state = 1
//!   string product_family = 6; bool hidden = 7;
//! }
//! message InstallHandshake { string product = 1; string uid = 2; UserSettings settings = 3; }  // Database's 2
//! message BaseProductState {
//!   bool installed = 1; bool playable = 2; bool update_complete = 3;
//!   bool background_download_available = 4; bool background_download_complete = 5;
//!   string current_version = 6; string current_version_str = 7; ...
//! }
//! ```
//!
//! `BaseProductState` as galaxy_blizzard_plugin's `product_db.proto` (`83cfb024`) and Lutris' (`eb1cedf`) have it.
//! A product is **listed installed** on `installed`, as before; an install the Dashboard asked for is **done** only
//! on `installed`, `playable` and `update_complete` together ([`Product::done`]), #172: the TV PC's first install
//! was taken for done on `installed` alone.
//!
//! An older file is one bare `ProductInstall`, which is read when the file is no `Database`.
use anyhow::{bail, Result};
use source_kit::proto::{self, Value, Writer};

/// One product the agent knows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Product {
    /// The agent's name for it: `emberwatch`.
    pub uid: String,
    /// Its short code: `emb`, which the client's launch command and `aggregate.json` take.
    pub code: String,
    pub install_path: String,
    /// The agent's word that it is installed (`base_product_state.installed`).
    pub installed: bool,
    /// `base_product_state.playable`.
    pub playable: bool,
    /// `base_product_state.update_complete`.
    pub update_complete: bool,
    /// `base_product_state.current_version_str`: `Chicago 1.0.67856`.
    pub version: String,
    pub family: String,
}

impl Product {
    /// Whether an install of it is over: installed, playable, and its update complete.
    pub fn done(&self) -> bool {
        self.installed && self.playable && self.update_complete
    }

    /// Its state as the log says it: `installed, playable, update complete, version Chicago 1.0.67856`.
    pub fn said(&self) -> String {
        let flag = |on: bool, what: &str| if on { what.to_string() } else { format!("not {what}") };
        let mut parts = vec![
            flag(self.installed, "installed"),
            flag(self.playable, "playable"),
            flag(self.update_complete, "update complete"),
        ];
        if !self.version.is_empty() {
            parts.push(format!("version {}", self.version));
        }
        parts.join(", ")
    }
}

/// Every product in the file, in its order.
pub fn parse(bytes: &[u8]) -> Result<Vec<Product>> {
    let fields = proto::fields(bytes)?;
    let installs: Vec<Product> =
        proto::all(&fields, 1).filter_map(Value::bytes).filter_map(|b| product(b).filter(named)).collect();
    if !installs.is_empty() {
        return Ok(installs);
    }
    match product(bytes).filter(named) {
        Some(bare) => Ok(vec![bare]),
        None if fields.is_empty() => Ok(Vec::new()),
        None => bail!("no product in it"),
    }
}

/// Whether a message read as a product has the names one has: a name a person reads as a uid and a code.
fn named(product: &Product) -> bool {
    let word = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c));
    word(&product.uid) && word(&product.code)
}

fn product(bytes: &[u8]) -> Option<Product> {
    let fields = proto::fields(bytes).ok()?;
    let text = |fields: &[(u32, Value)], n| proto::first(fields, n).and_then(Value::text).unwrap_or("").to_string();
    fn message<'a>(fields: &[(u32, Value<'a>)], n: u32) -> Vec<(u32, Value<'a>)> {
        proto::first(fields, n).and_then(Value::bytes).and_then(|b| proto::fields(b).ok()).unwrap_or_default()
    }
    let settings = message(&fields, 3);
    let state = message(&fields, 4);
    let base = message(&state, 1);
    let flag = |n| proto::first(&base, n).and_then(Value::varint).is_some_and(|v| v != 0);
    Some(Product {
        uid: text(&fields, 1),
        code: text(&fields, 2),
        install_path: text(&settings, 1),
        installed: flag(1),
        playable: flag(2),
        update_complete: flag(3),
        version: text(&base, 7),
        family: text(&fields, 6),
    })
}

/// `product.db` with the product `code` registered under `from` registered under `to` instead: its install's
/// `uid` and any handshake's (`InstallHandshake.uid`, the database's field 2), every other byte as it was. None where
/// nothing is under `from` with that code. An error where the file does not write back byte for byte as it was read
/// (something in it this reading would lose): then it is not to be touched.
pub fn reregister(bytes: &[u8], code: &str, from: &str, to: &str) -> Result<Option<Vec<u8>>> {
    let fields = proto::fields(bytes)?;
    let unchanged = fields.iter().fold(Writer::new(), |w, (n, v)| w.value(*n, *v)).finish();
    if unchanged != bytes {
        bail!("it does not write back byte for byte as read");
    }
    // A message with its field `uid_field` changed from `from` to `to`, where `matches` says it is the one.
    fn renamed(message: &[u8], uid_field: u32, to: &str, matches: impl Fn(&[(u32, Value)]) -> bool) -> Option<Vec<u8>> {
        let inner = proto::fields(message).ok()?;
        if !matches(&inner) {
            return None;
        }
        let w = inner.iter().fold(Writer::new(), |w, (n, v)| match n {
            n if *n == uid_field => w.text(*n, to),
            _ => w.value(*n, *v),
        });
        Some(w.finish())
    }
    let text = |f: &[(u32, Value)], n| proto::first(f, n).and_then(Value::text).map(str::to_string);
    let mut changed = false;
    let mut out = Writer::new();
    for (n, v) in &fields {
        let again = match (n, v.bytes()) {
            (1, Some(b)) => renamed(b, 1, to, |f| {
                text(f, 1).as_deref() == Some(from) && text(f, 2).is_some_and(|c| c.eq_ignore_ascii_case(code))
            }),
            (2, Some(b)) => renamed(b, 2, to, |f| text(f, 2).as_deref() == Some(from)),
            _ => None,
        };
        match again {
            Some(message) => {
                changed |= *n == 1;
                out = out.bytes(*n, &message);
            }
            None => out = out.value(*n, *v),
        }
    }
    Ok(changed.then(|| out.finish()))
}

/// Whether the product is one of the client's own parts, not a game: its agent, its beta, the client itself.
pub fn clients_own(uid: &str) -> bool {
    ["agent", "agent_beta", "beta", "battle.net"].iter().any(|own| uid.eq_ignore_ascii_case(own))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(uid: &str, code: &str, path: &str, installed: bool) -> Writer {
        let base = Writer::new().varint(1, installed.into()).varint(2, 1).text(7, "2.1.0.4711");
        Writer::new()
            .text(1, uid)
            .text(2, code)
            .message(3, Writer::new().text(1, path).text(2, "eu"))
            .message(4, Writer::new().message(1, base))
            .text(6, "EMBR")
    }

    #[test]
    fn a_database_gives_each_product_and_a_bare_install_its_one() {
        let db = Writer::new()
            .message(1, install("emberwatch", "emb", "D:/Battle.net/Emberwatch", true))
            .message(1, install("glimmer", "glm", "", false))
            .message(3, Writer::new().text(1, "Emberwatch.exe").varint(2, 4242))
            .finish();
        let products = parse(&db).unwrap();
        let emberwatch = Product {
            uid: "emberwatch".into(),
            code: "emb".into(),
            install_path: "D:/Battle.net/Emberwatch".into(),
            installed: true,
            playable: true,
            update_complete: false,
            version: "2.1.0.4711".into(),
            family: "EMBR".into(),
        };
        assert!(!emberwatch.done(), "installed and playable, its update not complete: not done");
        assert_eq!(emberwatch.said(), "installed, playable, not update complete, version 2.1.0.4711");
        assert_eq!(products[0], emberwatch);
        assert_eq!((products[1].uid.as_str(), products[1].installed), ("glimmer", false));
        assert_eq!(products.len(), 2);

        let bare = install("emberwatch", "emb", "D:/Battle.net/Emberwatch", true).finish();
        assert_eq!(parse(&bare).unwrap(), [emberwatch]);
        assert_eq!(parse(&[]).unwrap(), []);
        assert!(parse(b"\x0b").is_err());
        assert!(clients_own("Battle.net") && clients_own("agent") && !clients_own("wow"));
    }

    /// The TV PC's case (#172): Tony Hawk's Pro Skater 3 + 4 installed by the Dashboard under the table's uid
    /// `libra`, which the client does not know (its own is `lbra`): re-registered, the rest left byte for byte.
    #[test]
    fn a_product_under_an_old_uid_is_re_registered_and_nothing_else_moves() {
        let handshake = Writer::new().text(1, "lbra").text(2, "libra");
        let db = Writer::new()
            .message(1, install("agent", "agent", "C:/ProgramData/Battle.net/Agent", true))
            .message(1, install("libra", "lbra", "C:/Program Files (x86)/Tony Hawks Pro Skater 3 + 4", true))
            .message(2, handshake)
            .message(5, Writer::new().varint(1, 0))
            .finish();
        let again = reregister(&db, "lbra", "libra", "lbra").unwrap().expect("one under libra");
        let products = parse(&again).unwrap();
        assert_eq!(products[1].uid, "lbra");
        assert_eq!((products[1].code.as_str(), products[1].installed), ("lbra", true));
        assert_eq!(products[1].install_path, "C:/Program Files (x86)/Tony Hawks Pro Skater 3 + 4");
        assert_eq!(products[0], parse(&db).unwrap()[0]);
        let fields = proto::fields(&again).unwrap();
        let shake = proto::fields(proto::first(&fields, 2).and_then(Value::bytes).unwrap()).unwrap();
        assert_eq!(proto::first(&shake, 2).and_then(Value::text), Some("lbra"));
        assert_eq!(again.len(), db.len() - 2, "only the two uids, a letter shorter each");
        // Nothing under that uid: nothing to do; and done again, nothing either.
        assert_eq!(reregister(&again, "lbra", "libra", "lbra").unwrap(), None);
        assert_eq!(reregister(&db, "wow", "libra", "lbra").unwrap(), None);
        // A file that would not write back as it was read is never touched: a varint written long.
        let mut odd = db.clone();
        odd.extend_from_slice(&[0x30, 0x81, 0x00]);
        assert!(reregister(&odd, "lbra", "libra", "lbra").is_err());
    }
}
