//! The stores' searches by name against the REAL services (#246): ignored by default, run by hand to see where a
//! game falls through. For each name, each store's query, every candidate it gave, and why each was taken or not,
//! then whether the store had a square for the one taken.
//!
//! ```sh
//! cargo test -p library --test art_live -- --ignored --nocapture
//! ARTLIVE_NAMES="Portal 2;Hades" cargo test -p library --test art_live -- --ignored --nocapture
//! ```
//!
//! The names are well-known public titles as Steam names them, never a user's library. Only the name is sent.
use library::art::cache::Kind;
use library::art::names::{core_name, pick, same_game};
use library::art::{Hosts, Store, StoreArt, Want};

const NAMES: &[&str] = &[
    "Portal 2",
    "Grand Theft Auto V Enhanced",
    "Grand Theft Auto V Legacy",
    "Cyberpunk 2077",
    "Stardew Valley",
    "Call of Duty®: Black Ops II",
    "Battlefield™ 6",
    "Warhammer 40,000: Space Marine 2",
    "ELDEN RING",
    "The Witcher® 3: Wild Hunt",
    "Red Dead Redemption 2",
    "Hades",
    "Rocket League®",
    "DOOM Eternal",
    "Fallout 4",
    "Garry's Mod",
];

#[test]
#[ignore = "asks the real stores: run by hand"]
fn the_stores_searches_by_name_against_the_real_services() {
    let names: Vec<String> = match std::env::var("ARTLIVE_NAMES") {
        Ok(list) => list.split(';').map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).collect(),
        Err(_) => NAMES.iter().map(|n| n.to_string()).collect(),
    };
    let stores = StoreArt::new(Hosts::default());
    let mut table = Vec::new();
    for name in &names {
        println!("\n=== {name}  (core: {:?})", core_name(name));
        let want = Want { source: "steam".into(), source_id: "0".into(), name: name.clone(), ..Want::default() };
        let mut row = vec![name.clone()];
        for store in [Store::Microsoft, Store::PlayStation] {
            // As the lookup does: each search in turn until one finds the game.
            let mut taken = None;
            let mut seen = 0;
            for search in stores.searches(store, &want) {
                println!("  {} query: {}", store.name(), search.url);
                let answer = match stores.get(&search) {
                    Ok(Some(answer)) => answer,
                    other => {
                        println!("    no answer: {other:?}");
                        continue;
                    }
                };
                let candidates = stores.candidates(store, &answer);
                seen += candidates.len();
                for (found, _, rank) in &candidates {
                    let verdict = if same_game(name, found) { "SAME" } else { "other" };
                    println!("    {verdict:5} rank {rank}  {found:?}  (core: {:?})", core_name(found));
                }
                taken = pick(name, &candidates).cloned();
                if taken.is_some() {
                    break;
                }
            }
            let Some(found) = taken else {
                println!("    -> none of {seen} the same game");
                row.push(format!("none ({seen})"));
                continue;
            };
            let details = match (found.item.clone(), stores.ask(store, &want, Some(&found.id), &[Kind::Square])) {
                (Some(item), _) => Some(item),
                (None, Some(ask)) => stores.get(&ask).ok().flatten(),
                (None, None) => None,
            };
            let square = stores.pictures(store, &want, &[Kind::Square], details.as_ref());
            println!("    -> {} ; square: {}", found.id, square.first().map_or("none", |(_, url)| url.as_str()));
            row.push(if square.is_empty() { format!("{} no square", found.id) } else { "square".into() });
        }
        table.push(row);
    }
    println!("\n| Name | Xbox | PlayStation |\n|---|---|---|");
    for row in table {
        println!("| {} |", row.join(" | "));
    }
}
