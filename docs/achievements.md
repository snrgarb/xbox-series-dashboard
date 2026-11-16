# Achievements

Decided in #7, built in #140: **Steam's achievements only, with no key and no sign-in**, from the client's own files, how rare each is from Steam's keyless public figures, and the **Score** and **Rare** by the Bundle's rules (`~/projects/xbox-series-theme/CONTEXT.md`; `extension/XboxSeries.Core/ExtensionCore.Achievements.cs`). Xbox, Epic, GOG and RetroAchievements each need the user's sign-in and are not built; they can join behind the same interface. The screens (#141: Home's Achievements tile and top bar, the Guide's Profile & system, the Idle screen's cards, the Game hub's Achievements page) and the Achievement unlocked **Toast** (#142) are built on what this gives them.

## Where they come from

| What | Where | Read by |
|---|---|---|
| A game's achievements: API name, name and description (English), hidden, icons | `<Steam>/appcache/stats/UserGameStatsSchema_<appid>.bin` | `source_steam::achievements::parse_schema` |
| The signed-in user's unlocks and when | `<Steam>/appcache/stats/UserGameStats_<account id>_<appid>.bin` | `parse_stats` |
| Who is signed in | the newest `userdata/<account id>/config/localconfig.vdf` | `source_steam::signed_in_account` |
| How rare each is | `GET https://api.steampowered.com/ISteamUserStats/GetGlobalAchievementPercentagesForApp/v2/?gameid=<appid>`, no key | `library::achievements::Online` |
| Icons | `https://shared.steamstatic.com/community_assets/images/apps/<appid>/<file>`, the schema's `icon` and `icon_gray` | `library::achievements::Online`, into our cache |

Both files are binary KeyValues with inline keys (`source_steam::vdf::parse_binary_file`, which also takes a file whose last end marker is left off). The schema's `<appid>/stats/<stat id>/bits/<bit>` is one achievement: `name` its API name, and under `display` its `name` and `desc` (each a map by language, or a plain string), `hidden` (`"1"`), `icon` and `icon_gray`. Every stat that has `bits` is a group of up to 32 achievements. The stats file's `cache/<stat id>/data` is an int32 whose bit `n` is set when that group's achievement `n` is unlocked, and `cache/<stat id>/AchievementTimes/<n>` is when, in Unix seconds; an unlock with no time is kept as unlocked at a time not known. The layout is as two open-source readers have it, Steam Achievement Manager (gibbed/SteamAchievementManager, `SAM.Game/Manager.cs`) and PlayniteAchievements (`Providers/Steam/Local/SteamLocalStatsReader.cs`, which the Bundle relies on today); the tests use made-up files only. No real stats file was at hand to check against (`data/` has none): the first run on the TV PC is the check (below).

The percentages endpoint was checked on 2026-11-17 with Portal 2 (620): `{"achievementpercentages":{"achievements":[{"name":"ACH.WAKE_UP","percent":"64.7"},...]}}`, each percent a string (a number is taken too); an app with no public figures answers `403 {}`, which is kept as "asked, none known". The icon address was checked the same day with one of Portal 2's public icons (200, `image/jpeg`).

## The rules

- **Score**: an achievement's own points where its store has them (none for Steam), else by rarity: 1% or less 100, 5% or less 50, 10% or less 30, 25% or less 20, 50% or less 15, anything commoner or unknown 10 (`model::achievement::score_of`).
- **Rare**: a known rarity under 10%. Unknown is never Rare (`Achievement::is_rare`).
- **Totals** are this PC's, as the Bundle's are: every **Profile** shows the same (the Bundle's PlayniteAchievements keeps nothing per Profile; Steam's files are the signed-in Steam user's). `Totals { score, unlocked, of, games }`, the last three unlocks newest first, and each game's `GameProgress`.
- **What is told of as new**: only an unlock found while watching a game (below) that is from since the game started (60 s of slack before the Launch screen saw it running), or has no time. What the files already had as the Dashboard started is never told of, nor an older session's unlock found later (Steam syncing one from another PC).

## The interface

`model::AchievementSource` (`crates/model/src/achievement.rs`), one per store, as `model::Source` is for games:

```rust
pub trait AchievementSource: Send + Sync {
    fn key(&self) -> &'static str;                                  // the games' source key: "steam"
    fn games(&self) -> Option<Vec<String>>;                         // source_ids with achievements; None: not installed
    fn read(&self, source_id: &str) -> anyhow::Result<Option<Vec<Achievement>>>; // the files now, no rarity
    fn files(&self, source_id: &str) -> Vec<PathBuf>;               // what to watch while the game runs
}
```

A source reads only its client's files; how rare each is and the icons are the library's to fetch (`library::achievements::Fetch`, one place for the network, as art's `StoreArt` is). Adding a store: a type implementing it in the store's crate, a line in `crates/app/src/sources.rs` (`achievements`), and, if the store has public figures of rarity, an arm in `Online::rarity`.

## What later tickets use

- **`model::Library::achievements`** (`model::Achievements`), in every snapshot of the library database: `totals: Option<Totals>` (None while no game has any: the Bundle shows no totals then, and the screens show none of it), `recent: Vec<Unlock>` (the last five, newest first, each `{ game, achievement }`: the Idle screen's cards tell of five, Profile & system shows three, `GUIDE_RECENT`), `games: Vec<GameProgress>` (`of(game)` finds one), `next: Vec<Unlock>` (for each game part done, its locked achievement most players have, `nearest_locked`; `next_of(game)`). Home's top bar, the Achievements tile, Profile & system and the Idle screen read these (#141).
- **`library::Database::achievements(game) -> Vec<Achievement>`**: one game's whole list in its store's order, with rarity and the icons the cache has (`icon_file`, `icon_locked_file`). The Achievements page reads it ([spec/achievements-page.md](spec/achievements-page.md)) through `Store::achievements_of`; the page asks for the game's icons with `Store::ask_achievements(Ask::Icons(game))` and reads again when an update's `icons` names the game.
- **`Achievement`**: `key`, `name`, `description`, `hidden`, `icon`/`icon_locked` (addresses), `icon_file`/`icon_locked_file` (cached), `rarity`, `points`, `unlocked` (Unix seconds; `Some(0)` time not known); `score()`, `is_rare()`, `is_unlocked()`.
- **`library::achievements::Update`**, taken in the window loop by `Store::achievements_update()`: `unlocked: Vec<Unlock>` (newly unlocked this session, in the order found; several in one update were found together, for "<n> achievements unlocked"), `library` (a new snapshot when the totals changed, already shown), `icons` (games some icons came in for). `crates/app/src/window.rs` takes it beside the Queue's and hands `update.unlocked` to the Achievement unlocked Toast (`Dashboard::unlocked`, [spec/achievement-toast.md](spec/achievement-toast.md)).
- **`Store::ask_achievements(Ask::Playing { game, since })`** is sent by `Store::game_booted`; nothing else needs to.

## How it runs

`library::achievements::spawn`, a thread of its own with its own connection to the database, started once the sources have been read (`Store::follow_achievements`, from the window loop). There is none without a database file (`--screenshot`, `--library`).

1. **The read.** Every game each source has files for that is in the library is read and kept (`Database::put_achievements`), and said: `achievements: <appid> <n> of <m>, score <s>` each, then `achievements: <g> games, <u> of <m> unlocked, score <s>, read in <ms> ms`. A snapshot follows.
2. **Rarity**, one game at a time, 100 ms apart, between looks at the running game, so nothing waits on it: a game never asked, or last asked a week ago or more (`RARITY_FOR`), is asked; the answer is kept per achievement with the date asked (`achievement_rarity`), `achievements: rarity of <appid>: <k> of <m> known`, and the game's line again when its Score changed. Offline, or Steam in trouble: `achievements: rarity not fetched (<why>): kept as it was for <n> games`, nothing more is asked until the next start, and what was kept stands (a game never asked has no rarity, so a Score of 10 and never Rare). Rarity is asked whatever Settings' Download art online says: it is what the Score is made of, and asks only an appid.
3. **Watching.** Once a game counts as booted, `achievements: watching <appid> (<name>)`, and its files are read every second (`WATCH`) and compared with the last read; a change is a new read. Each new unlock of this session is said, `achievements: <appid> unlocked <key> "<name>": score <s>, <r>% of players[, Rare]` (`rarity not known` without one), then the game's line, and sent. The watch stays on the game after it ends, until another starts, as the Bundle tells of the game that just exited.
4. **Icons**, only with Settings' Download art online on, into `<data folder>/achievements/<source>/<source_id>/<file>`: each new unlock's before it is sent, the last five unlocks' after the read, the running game's (each achievement's unlocked icon, which a Toast shows) as it starts, and any game's on `Ask::Icons`. A picture is kept only if its bytes are a PNG, JPEG or WebP. One the CDN has not got (400, 403, 404, 410), or that is no picture, is said once, `achievements: an icon of <appid> not found: <file>` (`... is not a picture: <file>`), and not asked for again this run; the rest go on. Offline, or the CDN in trouble (a 429 or 5xx): `achievements: icons not fetched: <why>`, and no icon is asked for again this run.

**The icon's address** (`source_steam::achievements::icon_address`), from what the schema's `icon` or `icon_gray` holds: nothing (empty, or only spaces) is no icon and is never asked for; a full `http(s)://` address is taken as it is; anything else is a file name (of a path, its last part) under the CDN's `<appid>/`, `.jpg` added when it has none, and a SHA-1's forty hex digits put in lower case. Checked on 2026-11-17 with public icons of 1145360 and 440: `<hash>.jpg` and `tf_..._bw.jpg` answer 200; the same names without `.jpg` 404 or 503, and the hash in upper case 503. The first TV PC run's single 404, which stopped every icon, was why (#147).

`--store-art-service <url>` points the rarity requests at `<url>/steam-api` and every icon at `<url>/icons/<host>/<path>` (`Hosts::all_at`), so a test never asks Steam: `standin serve-art` answers both (the fake game's percentages from `source_steam::fixture::ACHIEVEMENTS`).

## The database (migration 7)

`achievements (game, key, position, name, description, hidden, icon, icon_locked, points, rarity, unlocked)`, keyed by `(game, key)`, gone with its game; `achievement_rarity (game, asked, found)`. A read takes the store's word on everything but rarity: an achievement it no longer lists goes, and one it says is locked again (stats reset) is locked. A game gone from its store keeps its rows but is left out of the totals.

## The fake

`standin make-steam` writes Ashfall Courier's (9100010) schema and stats (six achievements, two unlocked; `fixture::ACHIEVEMENTS`), and `standin unlock-achievement <steam dir> <key>` unlocks one now, as Steam rewrites its stats file while a game runs. The VM test is `e2e/payloads/achievements`.

## Differs from the Bundle

- **Where the data comes from.** The Bundle reads PlayniteAchievements, which signs in to each platform. The port reads Steam's files alone (#7): only Steam games have achievements, and a user with no other store loses nothing. A game whose schema Steam has not cached (never started on this PC) has none until it is.
- **Live unlocks** are found by watching the stats file every second, where PlayniteAchievements raises an event (it reads the same file for Steam). Several found in one read are one update, as several found in one refresh are one Toast in the Bundle; the Bundle's one-second gathering of unlocks heard live is the Toast's (`toasts::achievement::Gather`, #142).
- **Text** is English, as the rest of the Dashboard.
- **No "Needs PlayniteAchievements".** Home's tile has nothing to need; with no totals the screens show what #141 decides.

## Needs a human

- The first run against a real Steam on the TV PC: that the parser reads real `UserGameStatsSchema_*.bin` and `UserGameStats_*.bin` files (the log's per-game lines against Steam's own count of each game's achievements), and that an unlock in a real game is said within a few seconds.
