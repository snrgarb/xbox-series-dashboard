# Art online for any game: Steam, Xbox, PlayStation, SteamGridDB

Research for [#246](https://github.com/snrgarb/xbox-series-dashboard/issues/246). The user asked for Playnite's order of metadata sources for art (Steam first, Xbox second, PlayStation third, SteamGridDB fourth), for a game of any store, matched by name where the game is not that store's own. Their example: a game with no square picture on Steam or SteamGridDB that the Xbox or PlayStation store has. And: "does SteamGridDB have a category for stock artwork so it attempts to download without the custom banners?"

Checked on **18 November 2026** from Linux with plain `curl`. No sign-in, no key (except that SteamGridDB's own documentation was read, not its API), no cookies. The search terms were well-known public titles (named below) and never from the user's library. Nothing from `data/` was read. The pictures downloaded to measure them stayed in a scratch folder and are not in the repository.

Each finding carries a tag:

- **[live]**: fetched today and the answer read.
- **[docs]**: read in the service's own documentation.
- **[inferred]**: reasoned from the above. Treat it as a guess until tested.

## Steam: finding an app by name, without a key

`GET https://store.steampowered.com/api/storesearch/?term=<name>&l=english&cc=US` **[live]**

- Answers `200`, `application/json`: `{"total": n, "items": [{"type": "app", "name": "...", "id": <appid>, "tiny_image": "...", ...}]}`. Asked for "Arma Reforger" it gave four items: `Arma Reforger` (1874880), `Arma Reforger - Rally` (DLC), `Arma Reforger Soundtrack` and a supporter pack. The first hit is often the game, but not always: the DLC and soundtrack share the name's start. A name that matches nothing gives `{"total":0,"items":[]}`.
- `type` is `app` for apps. Bundles and packages are other types.
- No key, no rate-limit headers.

With the appid, the Steam path the port already has does the rest: `IStoreBrowseService/GetItems` (Steam's public Web API, no key; docs/sources.md "Art") gives the asset file names on `shared.steamstatic.com/store_item_assets`. Checked for 1874880 today: `200`. **[live]**

| Kind | Steam asset |
|---|---|
| cover | `library_capsule` (600x900) |
| hero | `library_hero` |
| header | `header` |
| square | **none**. The only square asset is the 32 px `community_icon` (docs/sources.md) |

## Xbox: the Microsoft Store's catalogue by name

`GET https://displaycatalog.mp.microsoft.com/v7.0/productFamilies/autosuggest?market=US&languages=en-us&productFamilyNames=Games&query=<name>` **[live]**

- This is the same host as the product lookup the port already uses for Xbox games (`products?bigIds=`). It answers `200`: `{"Results": [{"ProductFamilyName": "Games", "Products": [{"ProductId": "9NKCKD1WNDQ0", "Type": "Game", "Title": "Arma Reforger", "Icon": "//store-images...", ...}]}], "TotalResultCount": n}`.
- For "Arma Reforger" it gave three products: the game (`Type: Game`), `Arma Reforger - Rally` (`Type: Durable`, the DLC) and `Arma Reforger - Experimental` (`Type: Game`). For "Portal 2" it gave one: `BT2B17V20D1P`, `Type: Game`.
- `storeedgefd.dsx.mp.microsoft.com/v9.0/search?market=US&locale=en-US&deviceFamily=Windows.Desktop&query=<name>` also answers `200` with cards that carry images. It was not used: one more host, and its shapes are the Store app's own. **[live]**

Then `products?bigIds=<ProductId>&market=US&languages=en-us`, for 9NKCKD1WNDQ0 **[live]**:

| Image purpose | Size | Used for |
|---|---|---|
| `BoxArt` | 2160x2160 | **square** (at `?w=600`) |
| `FeaturePromotionalSquareArt` | 1080x1080 | square, when there is no BoxArt |
| `Poster` | 1440x2160 | cover |
| `SuperHeroArt` | 3840x2160 | hero |
| `TitledHeroArt` | 1920x1080 | header |
| `BrandedKeyArt` (584x800), `Logo` (300x300), `Screenshot` | | not used |

So the user's example holds: Arma Reforger has a square in the Microsoft Store.

## PlayStation: the PlayStation Store by name

**The store's GraphQL** (`web.np.playstation.com/api/graphql/v1/op?operationName=...&extensions={"persistedQuery":{...,"sha256Hash":...}}`) only runs persisted queries whose hash the store has whitelisted. A hash taken from elsewhere answered `400 {"message":"Query <hash> not whitelisted"}`. The hashes change with the store web app's builds and are documented nowhere. **Not used.** **[live]**

**The `valkyrie-api` faceted search** (`store.playstation.com/valkyrie-api/en/US/999/faceted-search/<name>`): `404 Not found`. **Gone.** **[live]**

**The `chihiro` search still answers**, keyless **[live]**:

`GET https://store.playstation.com/store/api/chihiro/00_09_000/tumbler/US/en/999/<name>?suggested_size=10&mode=game`

- It answers `200`, `application/json`, with `cache-control: max-age=300`: `{"size", "total_results", "links": [{"name", "short_name", "id", "top_category", "game_contentType", "playable_platform", "images": [{"type", "url"}], ...}]}`.
- Searches made: "Arma Reforger" (2 links, the same PS5 game twice), "Elden Ring" (12: the game, its editions as `Bundle`, add-ons as `add_on`), "Portal 2" (5), "Hades" (20, among them other games that only contain the word), and a name of nothing (0).
- `top_category` is `downloadable_game`, `disc_based_game` or `add_on`. `game_contentType` is `Full Game`, `Bundle`, `Map`, `Level`, `Costume` and so on.
- Names carry the store's marks and platforms: `Portal™2`, `ELDEN RING PS4 & PS5`.

Its image `type` numbers have no documentation. These are the sizes measured by downloading each one (Arma Reforger, Hades, God of War: Ascension):

| `type` | Size | What it is | Used for |
|---|---|---|---|
| 10 | 1024x1024 | the store's square game tile; on every game seen, PS3 to PS5 | **square** |
| 12 | 1920x1080 | wide key art, untitled on the PS5 games seen | **hero** |
| 13 | 1280x720 | a smaller copy of 12 | not used |
| 14 | 830x1080 | portrait, on one PS3 game only | not used: not 2:3, and rare |
| 1, 2, 9 | 240, 80, 160 square | PS3-era icons | not used: too small |

The images are on `vulcan.dl.playstation.net` or `apollo2.dl.playstation.net`, keyless. `?w=` does not resize them: `w=600` of the 1024 square gave 1024x1024.

The roles the question named (`MASTER`, `GAMEHUB_COVER_ART`, `FOUR_BY_THREE_BANNER`, `BACKGROUND`) belong to the GraphQL's media, which is not reachable without a whitelisted hash. Through `chihiro`, PlayStation gives the **square** and the **hero**, and nothing portrait or titled-landscape.

**How reliable it is [inferred]:** `chihiro` is the API of the PlayStation Store's old web store, which Sony replaced in 2020. It still answers today, but nothing promises it will. The lookup treats it like any other source: a failure is logged once and the game falls through to the next source. If it stops answering, the plan is to take it out of the table in docs/sources.md and say why there, as was done for Epic's catalogue.

## SteamGridDB: is there a "stock artwork" category?

Read in SteamGridDB's own OpenAPI description, <https://www.steamgriddb.com/static/openapi.yml> (version 2.10.0), which <https://www.steamgriddb.com/api/v2> renders. **[docs]**

- `/grids/game/{id}` takes `styles` from `Styles`: **`alternate`, `blurred`, `white_logo`, `material`, `no_logo`**.
- `/heroes/game/{id}` takes `styles` from `StylesHero`: **`alternate`, `blurred`, `material`**.
- `/logos/game/{id}` takes `StylesLogo`: **`official`**, `white`, `black`, `custom`. `/icons/game/{id}` takes `StylesIcon`: **`official`**, `custom`.
- The other filters are `dimensions`, `mimes`, `types` (`static`, `animated`), `nsfw`, `humor`, `epilepsy` (`false`, `true` or `any`), `oneoftag` (`humor`, `nsfw`, `epilepsy`), `limit` (50 at most) and `page`.
- **There is no sort parameter.** Each picture has a `score`, but the description does not say what order the list comes in.

**The answer:** SteamGridDB has an `official` style only for **logos and icons**. For **grids (covers, headers, squares) and heroes there is no stock or official category**, and no way to sort by score. The closest the API allows is the style filter. `alternate` is the style for ordinary box-art-style grids. The other four (`blurred`, `white_logo`, `material`, `no_logo`) are each a made treatment of the art, which is what the user calls custom banners. `alternate` can still be fan-made art, so the filter makes custom banners less likely without promising official art.

**What the port does:** it asks for each kind with `styles=alternate` first, keeping the other filters (static, not NSFW, not humour, no epilepsy warning, PNG, JPEG or WebP). It asks again without a style only when the game has nothing in that style. The Art picker still lists every style, since there the user chooses.

## Matching by name

None of these searches returns only the game asked for. All of them return near names: DLC, soundtracks, editions, other games that contain the word. So the rule is narrow, and a wrong picture is never taken:

1. **Plain name.** Lower-case. ™ ® © become spaces (so `Portal™2` is `portal 2`). `&` becomes "and". Every run of anything but letters and digits becomes one space.
2. **Core name.** The plain name with edition, build and platform words removed from its end, again and again. An edition is `<word> edition`, where the word is one of a fixed list: standard, digital deluxe, deluxe, digital, gold, complete, definitive, ultimate, premium, game of the year, goty, enhanced, special, collector's, legendary, launch, anniversary, pc, windows, steam, xbox. A build is a bare `goty`, `enhanced` or `legacy` (Steam's "Grand Theft Auto V Enhanced" and "... Legacy" are the consoles' "Grand Theft Auto V™"). A platform is one of: `ps4 and ps5`, `ps4`, `ps5`, `playstation 4`, `playstation 5` (the PlayStation Store's "(PlayStation®5)"), `xbox one and xbox series x s`, `xbox series x s`, `xbox one`, `for windows 10`, `windows`, `pc` and so on. A name made of nothing but such words keeps them. "Remastered" and "Remake" are **not** set aside: they are often a game of their own ("Modern Warfare" and "Modern Warfare Remastered" are two games), so "The Witcher 3: Wild Hunt" does not match the stores' "The Witcher 3: Wild Hunt — Remastered" and falls through.
3. **A match** is an equal, non-empty core name. Never a prefix, never "contains", never the first hit regardless. `Hadrel` is not `Hadrel II`, `Hadrel 2`, `Hadrel: Rally` or `Hadrel Soundtrack`.
4. **Of several matches**, one whose plain name is exactly the same comes before one that differs only by an edition or platform. Then the store's own preference: a PlayStation `Full Game` before a `Bundle` (add-ons never), and a SteamGridDB `verified` entry before another. Then the store's own order.
5. **Only these count:** Steam items of `type: app`, Microsoft products of `Type: Game`, and PlayStation links in `downloadable_game` or `disc_based_game`.

**The search's terms.** The name is sent first with ™ ® © gone. While no answer is the game, the store is asked again: with the name's words alone (a mark a space only between two letters or digits, as in `Portal™2`; a comma between digits dropped, `40,000` as `40000`; every other mark but an apostrophe a space), then with those words but the last. What any answer gives is held to the same rule. All of this came from the live test below: the PlayStation Store's search finds nothing for "Warhammer 40,000: Space Marine 2" and the game for "Warhammer 40000 Space Marine 2", while the Microsoft Store's autosuggest is the other way about; the autosuggest finds nothing for "Battlefield 6" and "Battlefield™ 6" among ten answers for "Battlefield".

**Where a game falls through**, the log says it by its id, never its name: `art: steam <id>: Xbox: 5 candidates, none the same name`.

**The live test.** `crates/library/tests/art_live.rs` is ignored by default and asks the real Microsoft and PlayStation stores for a list of public titles as Steam names them (or `ARTLIVE_NAMES="a;b"`). It prints each query, every candidate with its core name and whether it is the same game, and whether a square came of the one taken: `cargo test -p library --test art_live -- --ignored --nocapture`. CI never runs it.

A store's own game is never matched by name. A Steam game is asked about at Steam by its appid, and an Xbox game at the Microsoft Store by its product id. SteamGridDB keeps its lookup by store id first, for Steam, Epic and GOG.

## What is sent

To a search, only the game's name, with the marks removed. To a catalogue, only that store's id. To a picture's host, nothing but the address. The SteamGridDB key goes to SteamGridDB's API and nowhere else, and is never logged.
