//! On made-up products in the real answers' shapes (docs/research/game-pass.md): never a real answer pasted.
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::*;

/// The list: its header, then three games and one the catalogue does not know.
fn sigl() -> Value {
    json!([
        {"siglId": ALL_PC_GAMES, "title": "All PC Games", "description": "Made up", "requiresShuffling": "False",
         "imageUrl": ""},
        {"id": "9ZZZGPASS001"},
        {"id": "9zzzgpass002"},
        {"id": "9ZZZGPASS003"},
        {"id": "9ZZZGPASS001"},
        {"id": "9ZZZGPASS009"},
        {"id": "not an id"}
    ])
}

fn image(purpose: &str, width: u32, height: u32) -> Value {
    json!({"ImagePurpose": purpose, "Uri": format!("//images.example/{purpose}.png"), "Width": width,
           "Height": height})
}

/// A product as the display catalogue gives one: `family` its own package family name, `packaged` its SKU's,
/// `bundled` the products it is made of.
fn product(id: &str, title: &str, family: &str, packaged: &str, bundled: &[&str]) -> Value {
    let packages: Vec<Value> = [packaged]
        .iter()
        .filter(|f| !f.is_empty())
        .map(|f| json!({"PackageFamilyName": f, "PlatformDependencies": [{"PlatformName": "Windows.Desktop"}]}))
        .collect();
    let bundled: Vec<Value> = bundled.iter().map(|id| json!({"BigId": id})).collect();
    json!({
        "ProductId": id,
        "ProductKind": "Game",
        "LocalizedProperties": [{
            "ProductTitle": title,
            "DeveloperName": "Made Up Studio",
            "PublisherName": "Made Up Publishing",
            "Images": [image("BoxArt", 1080, 1080), image("Poster", 720, 1080), image("SuperHeroArt", 1920, 1080),
                       image("TitledHeroArt", 1920, 1080), image("Screenshot", 1920, 1080)]
        }],
        "MarketProperties": [{"OriginalReleaseDate": "2021-11-09T00:00:00.0000000Z"}],
        "Properties": {"PackageFamilyName": family},
        "DisplaySkuAvailabilities": [{"Sku": {"SkuId": "0010", "Properties": {
            "Packages": packages, "IsBundle": !bundled.is_empty(), "BundledSkus": bundled}}}]
    })
}

fn answer(ids: &[&str]) -> Value {
    let all = [
        product("9ZZZGPASS001", "Fernway", "MadeUp.Fernway_0000000000000", "", &[]),
        product("9ZZZGPASS002", "Halyard Point", "", "MadeUp.HalyardPoint_0000000000000", &[]),
        product("9ZZZGPASS003", "Lumen Hollow Deluxe", "", "", &["9ZZZGPASS004"]),
        product("9ZZZGPASS004", "Lumen Hollow", "MadeUp.LumenHollow_0000000000000", "", &[]),
    ];
    let products: Vec<Value> = all
        .into_iter()
        .filter(|p| ids.iter().any(|id| p["ProductId"].as_str().is_some_and(|got| got.eq_ignore_ascii_case(id))))
        .collect();
    json!({"Products": products})
}

#[test]
fn the_list_is_its_ids_in_order_each_once() {
    assert_eq!(ids(&sigl()), ["9ZZZGPASS001", "9ZZZGPASS002", "9ZZZGPASS003", "9ZZZGPASS009"]);
    assert!(ids(&json!({"Products": []})).is_empty());
}

#[test]
fn a_product_is_its_name_its_family_its_makers_its_date_and_its_pictures() {
    let found = products(&answer(&["9ZZZGPASS001", "9ZZZGPASS002", "9ZZZGPASS003"]), "https:");
    let fernway = &found[0];
    assert_eq!((fernway.id.as_str(), fernway.name.as_str()), ("9ZZZGPASS001", "Fernway"));
    assert_eq!(fernway.family.as_deref(), Some("MadeUp.Fernway_0000000000000"));
    assert_eq!(
        (fernway.developers.as_slice(), fernway.publishers.as_slice()),
        (&["Made Up Studio".to_string()][..], &["Made Up Publishing".to_string()][..])
    );
    assert_eq!(fernway.released, 1_636_416_000, "2021-11-09");
    assert_eq!(fernway.art.product.as_deref(), Some("9ZZZGPASS001"));
    assert_eq!(fernway.art.square.as_deref(), Some("https://images.example/BoxArt.png?w=600"));
    assert_eq!(fernway.art.cover.as_deref(), Some("https://images.example/Poster.png?w=600"));
    assert_eq!(fernway.art.hero.as_deref(), Some("https://images.example/SuperHeroArt.png?w=1920"));
    assert_eq!(fernway.art.header.as_deref(), Some("https://images.example/TitledHeroArt.png?w=920"));
    // Its family name only in its SKU's packages.
    assert_eq!(found[1].family.as_deref(), Some("MadeUp.HalyardPoint_0000000000000"));
    // A bundle: none of its own, its products named.
    assert_eq!((found[2].family.as_deref(), found[2].bundled.as_slice()), (None, &["9ZZZGPASS004".to_string()][..]));
    // Something that is not a product, or has no name, is passed over.
    assert!(products(&json!({"Products": [{"ProductId": "9ZZZGPASS005"}, {"x": 1}]}), "https:").is_empty());
}

#[test]
fn a_local_catalogue_is_read_as_the_answers_are_and_a_bundle_takes_its_products_family() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path().join("GB");
    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(at.join("sigl.json"), sigl().to_string()).unwrap();
    let all = answer(&["9ZZZGPASS001", "9ZZZGPASS002", "9ZZZGPASS003", "9ZZZGPASS004"]);
    std::fs::write(at.join("products.json"), all.to_string()).unwrap();
    let from = From::Folder(dir.path().to_path_buf());
    let gb = Locale { market: "GB".into(), language: "en-GB".into() };
    let got = catalogue(&from, &gb, 1000).expect("the GB catalogue");
    let names: Vec<&str> = got.products.iter().map(|p| p.name.as_str()).collect();
    // In the list's order; the bundle's product that is not in the list is not a game of it.
    assert_eq!(names, ["Fernway", "Halyard Point", "Lumen Hollow Deluxe"]);
    assert_eq!(got.products[2].family.as_deref(), Some("MadeUp.LumenHollow_0000000000000"));
    assert_eq!((got.market.as_str(), got.language.as_str()), ("GB", "en-GB"));
    // No catalogue for a market the folder has none of.
    let us = Locale { market: "US".into(), language: "en-US".into() };
    assert_eq!(catalogue(&from, &us, 1000), None);
}

/// A stand-in for the two hosts on 127.0.0.1: what each request asked, and how many were made.
struct Stand {
    base: String,
    asked: Arc<Mutex<Vec<String>>>,
    down: Arc<Mutex<bool>>,
}

fn stand_in() -> Stand {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let down = Arc::new(Mutex::new(false));
    let (kept, is_down) = (asked.clone(), down.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut headers = Vec::new();
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap() == 0 || header.trim().is_empty() {
                    break;
                }
                headers.push(header.trim().to_string());
            }
            let path = line.split_whitespace().nth(1).unwrap_or_default().to_string();
            let agent = headers.iter().find(|h| h.to_ascii_lowercase().starts_with("user-agent:")).cloned();
            kept.lock().unwrap().push(format!("{path} [{}]", agent.unwrap_or_default()));
            let (status, body) = if *is_down.lock().unwrap() {
                ("503 Service Unavailable", String::new())
            } else if path.starts_with("/gamepass/sigls/v2?") {
                ("200 OK", sigl().to_string())
            } else if let Some(query) = path.strip_prefix("/microsoft/v7.0/products?bigIds=") {
                let ids: Vec<&str> = query.split('&').next().unwrap_or_default().split(',').collect();
                ("200 OK", answer(&ids).to_string())
            } else {
                ("404 Not Found", String::new())
            };
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    Stand { base, asked, down }
}

#[test]
fn fetched_once_a_day_kept_and_the_kept_copy_used_when_the_service_fails() {
    let stand = stand_in();
    let dir = tempfile::tempdir().unwrap();
    let kept = dir.path().join(FOLDER);
    let from = From::Online { hosts: Hosts::all_at(&stand.base), kept: Some(kept.clone()) };
    let gb = Locale { market: "GB".into(), language: "en-GB".into() };
    let now = 1_800_000_000;

    let got = catalogue(&from, &gb, now).expect("fetched");
    assert_eq!(got.products.len(), 3);
    assert_eq!(got.products[2].family.as_deref(), Some("MadeUp.LumenHollow_0000000000000"));
    let asked = stand.asked.lock().unwrap().clone();
    // The list, its products in one batch, then the bundle's product: nothing about the user, a plain user agent.
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert!(asked[0].starts_with(&format!("/gamepass/sigls/v2?id={ALL_PC_GAMES}&language=en-GB&market=GB [")));
    assert!(asked[1].starts_with(
        "/microsoft/v7.0/products?bigIds=9ZZZGPASS001,9ZZZGPASS002,9ZZZGPASS003,9ZZZGPASS009&market=GB&languages=en-GB"
    ));
    assert!(asked[2].starts_with("/microsoft/v7.0/products?bigIds=9ZZZGPASS004&"));
    assert!(asked.iter().all(|line| line.contains("xbox-series-dashboard/")), "{asked:?}");
    assert!(kept.join("GB.json").is_file());

    // Within the day: the kept copy, nothing asked.
    assert_eq!(catalogue(&from, &gb, now + EVERY - 1), Some(got.clone()));
    assert_eq!(stand.asked.lock().unwrap().len(), 3);

    // A day on, with the service down: the kept copy, and no new try for an hour.
    *stand.down.lock().unwrap() = true;
    let later = now + EVERY + 10;
    let fell_back = catalogue(&from, &gb, later).expect("the kept copy");
    assert_eq!(fell_back.products, got.products);
    assert_eq!(stand.asked.lock().unwrap().len(), 4);
    assert!(catalogue(&from, &gb, later + 60).is_some());
    assert_eq!(stand.asked.lock().unwrap().len(), 4, "tried again within the hour");
    // Up again an hour on: fetched.
    *stand.down.lock().unwrap() = false;
    let again = catalogue(&from, &gb, later + AGAIN).expect("fetched again");
    assert_eq!((again.fetched, again.tried), (later + AGAIN, 0));
    assert_eq!(stand.asked.lock().unwrap().len(), 7);

    // Another market is its own: fetched now.
    let us = Locale { market: "US".into(), language: "en-US".into() };
    assert!(catalogue(&from, &us, later + AGAIN).is_some());
    assert!(kept.join("US.json").is_file());
}

#[test]
fn with_nothing_kept_a_failed_fetch_gives_no_catalogue() {
    let stand = stand_in();
    *stand.down.lock().unwrap() = true;
    let dir = tempfile::tempdir().unwrap();
    let from = From::Online { hosts: Hosts::all_at(&stand.base), kept: Some(dir.path().join(FOLDER)) };
    let gb = Locale { market: "GB".into(), language: "en-GB".into() };
    assert_eq!(catalogue(&from, &gb, 5000), None);
    assert_eq!(catalogue(&from, &gb, 5060), None);
    assert_eq!(stand.asked.lock().unwrap().len(), 1, "not tried again within the hour");
}

#[test]
fn a_date_is_its_day_in_unix_seconds() {
    assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(unix_seconds("2000-03-01"), Some(951_868_800));
    assert_eq!(unix_seconds("1601-01-01T00:00:00Z"), None);
    assert_eq!(unix_seconds("soon"), None);
}

/// The real catalogue, by hand only (`cargo test -p library --lib game_pass -- --ignored --nocapture`): counts for
/// two markets, and nothing kept.
#[test]
#[ignore = "asks Microsoft's real catalogue"]
fn live_counts_for_two_markets() {
    for (market, language) in [("US", "en-US"), ("GB", "en-GB")] {
        let locale = Locale { market: market.into(), language: language.into() };
        let started = std::time::Instant::now();
        match fetch(&Hosts::default(), &locale, None) {
            Ok(Fetched::New { products, etag }) => {
                let family = products.iter().filter(|p| p.family.is_some()).count();
                let bundles = products.iter().filter(|p| !p.bundled.is_empty()).count();
                let square = products.iter().filter(|p| p.art.square.is_some()).count();
                let cover = products.iter().filter(|p| p.art.cover.is_some()).count();
                println!(
                    "{market}: {} games, {family} with a family name, {bundles} bundles, {square} with box art, \
                     {cover} with a poster; ETag {}; {} ms",
                    products.len(),
                    if etag.is_some() { "given" } else { "none" },
                    started.elapsed().as_millis()
                );
            }
            Ok(Fetched::Same) => println!("{market}: as it was"),
            Err(why) => println!("{market}: failed: {why}"),
        }
    }
}
