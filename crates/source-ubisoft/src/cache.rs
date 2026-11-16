//! Ubisoft Connect's `cache\configuration\configurations`: every product the signed-in account has had, each a
//! protobuf message with its ids and its configuration in YAML (docs/research/stores.md, "Ubisoft Connect"):
//!
//! ```proto
//! message Cache { repeated Game games = 1; }
//! message Game  { uint32 uplay_id = 1; uint32 install_id = 2; string game_info = 3; }
//! ```
use anyhow::{Context, Result};
use source_kit::proto::{self, Value};

use crate::yaml::{self, Node};

/// What the cache says of one product.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Product {
    pub uplay_id: u32,
    pub install_id: u32,
    /// Its name, looked up in the configuration's own strings where it names one.
    pub name: Option<String>,
    /// Its cover's file name (`thumb_image`), and its background's.
    pub thumb: Option<String>,
    pub background: Option<String>,
    /// Is it a game of its own, Ubisoft's: not DLC (`is_ulc`), not a copy from another store
    /// (`third_party_platform`), and with something to start (`start_game`).
    pub game: bool,
    /// The ids of the products that come with it (`addons`): DLC and the like, which are no games.
    pub addons: Vec<u32>,
}

/// Every product in the cache, in its order.
pub fn parse(bytes: &[u8]) -> Result<Vec<Product>> {
    let cache = proto::fields(bytes).context("not a cache of configurations")?;
    let mut products = Vec::new();
    for entry in proto::all(&cache, 1) {
        let fields = proto::fields(entry.bytes().context("a product that is not a message")?)?;
        let id = |number| proto::first(&fields, number).and_then(Value::varint).and_then(|v| u32::try_from(v).ok());
        let uplay_id = id(1).context("a product with no uplay_id")?;
        let install_id = id(2).unwrap_or(0);
        let info = proto::first(&fields, 3).and_then(Value::text).unwrap_or_default();
        products.push(product(uplay_id, install_id, &yaml::parse(info)));
    }
    Ok(products)
}

fn product(uplay_id: u32, install_id: u32, info: &Node) -> Product {
    let Some(root) = info.get("root") else { return Product { uplay_id, install_id, ..Product::default() } };
    // A value is the text itself, or the name of a string in `localizations.default`.
    let local = |key: &str| {
        let value = root.get(key)?.text()?.trim();
        let looked_up = info.at(&["localizations", "default", value]).and_then(Node::text);
        Some(looked_up.unwrap_or(value).trim().to_string()).filter(|text| !text.is_empty())
    };
    let addons = root.get("addons").map_or(&[][..], Node::items);
    let third_party = root.get("third_party_platform").is_some_and(|node| node != &Node::Scalar(String::new()));
    Product {
        uplay_id,
        install_id,
        name: local("name"),
        thumb: local("thumb_image"),
        background: local("background_image"),
        game: !root.get("is_ulc").is_some_and(Node::truthy) && !third_party && root.get("start_game").is_some(),
        addons: addons.iter().filter_map(|addon| addon.get("id")?.text()?.trim().parse().ok()).collect(),
    }
}

/// The products that are games the account owns: `game`, and no other product's addon.
pub fn games(products: &[Product]) -> Vec<&Product> {
    let addon = |id: u32| products.iter().any(|p| p.addons.contains(&id));
    products.iter().filter(|p| p.game && !addon(p.uplay_id)).collect()
}
