//! Battle.net's `%PROGRAMDATA%\Battle.net\Agent\product.db`: the products its agent has installed or is
//! installing, protobuf (docs/research/stores.md, "Battle.net"; field numbers from TACTLib's schema):
//!
//! ```proto
//! message Database { repeated ProductInstall product_install = 1; ... }
//! message ProductInstall {
//!   string uid = 1; string product_code = 2; UserSettings settings = 3;   // install_path = 1
//!   CachedProductState cached_product_state = 4;                          // base_product_state = 1: installed = 1
//!   string product_family = 6; bool hidden = 7;
//! }
//! ```
//!
//! An older file is one bare `ProductInstall`, which is read when the file is no `Database`.
use anyhow::{bail, Result};
use source_kit::proto::{self, Value};

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
    pub family: String,
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
    Some(Product {
        uid: text(&fields, 1),
        code: text(&fields, 2),
        install_path: text(&settings, 1),
        installed: proto::first(&base, 1).and_then(Value::varint).is_some_and(|v| v != 0),
        family: text(&fields, 6),
    })
}

/// Whether the product is one of the client's own parts, not a game: its agent, its beta, the client itself.
pub fn clients_own(uid: &str) -> bool {
    ["agent", "agent_beta", "beta", "battle.net"].iter().any(|own| uid.eq_ignore_ascii_case(own))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::Writer;

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
            family: "EMBR".into(),
        };
        assert_eq!(products[0], emberwatch);
        assert_eq!((products[1].uid.as_str(), products[1].installed), ("glimmer", false));
        assert_eq!(products.len(), 2);

        let bare = install("emberwatch", "emb", "D:/Battle.net/Emberwatch", true).finish();
        assert_eq!(parse(&bare).unwrap(), [emberwatch]);
        assert_eq!(parse(&[]).unwrap(), []);
        assert!(parse(b"\x0b").is_err());
        assert!(clients_own("Battle.net") && clients_own("agent") && !clients_own("wow"));
    }
}
