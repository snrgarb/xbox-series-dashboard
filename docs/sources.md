# Sources

A **source** is one store's client on this machine: Steam today; Epic, GOG, EA, Ubisoft, Battle.net, Xbox and the games added by hand next (M4). Each is a crate `source-<store>` behind one interface, `model::Source` (`crates/model/src/source.rs`), and the library merges them all into one.

## The interface

```rust
pub trait Source: Send + Sync {
    fn key(&self) -> &'static str;                         // "steam": games.source, never changed once released
    fn name(&self) -> &'static str;                        // "Steam": the log and the screens
    fn read(&self) -> anyhow::Result<Option<Found>>;       // the games; Ok(None): client not installed
    fn start(&self, game: &Game) -> Option<Start>;         // how to start one
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start>;  // what installs or uninstalls one
    fn running(&self, game: &Game) -> bool { false }       // the store's own word that it runs
    fn installs(&self) -> Option<Vec<Install>> { None }    // its installs under way: the Queue
}

pub struct Found { pub library: Library, pub from: String }
pub enum Start { Address(String), Program { path: PathBuf, args: Vec<String>, dir: Option<PathBuf> } }
pub enum Ask { Install, Uninstall }
```

- **`read`** lists every game the client's own files say the user has, installed or not. Each `Game` has `source` set to the key and `source_id` to the store's own id for it (Steam's appid, `"440"`), its install state (`installed`, `install_dir`, `size_on_disk`), its art (`cover`, `hero`, `header`: paths to what the client has cached on disk, so finding art is part of reading), its play history where the client keeps one (`last_played`, `playtime_minutes`) and what the store says of it (`developers`, `released`, ...). `Library::user` is the signed-in user's name where the store has one; `Library::collections` the store's own groupings, `None` when they could not be read. `Found::from` is where it was read from, for the log line `<key>: <n> games from <from> in <ms> ms`.
  - **Client not installed:** `Ok(None)`. Nothing is logged and the source's games stay in the database as they were.
  - **Installed but unreadable:** an error, with the place in its context. The library logs `<key>: <error>`, passes over the source, and loads the rest; the source's games stay as they were.
  - It runs off the UI thread, at start (after the first frame when the database has games). Keep it to the client's local files: no network, no sign-in.
- **`start`** gives an address the client takes (`steam://rungameid/440`, a protocol URL, `shell:AppsFolder\<AUMID>`) or a program to run with its arguments and folder (a game added by hand, a GOG game's own exe, `Battle.net.exe --exec=...`). The shell opens it, shows the Launch screen and follows the game by its windows: those of programs under `Game::install_dir`, so a game with no install folder can't be launched.
- **`ask`** gives what asks the client to install or uninstall the game, opened as `start`'s is: Steam's `steam://install/<appid>`, a client's command line (GOG's `GalaxyClient.exe /command=installGame`), a game's own uninstaller (`unins000.exe`); `None` when there is nothing to ask. The Tile menu and the Game hub offer Install and Uninstall for any game that has a source.
- **`running`** is looked at a few times a second while a game runs, alongside its windows: Steam's is its registry's `Apps\<appid>\Running`. A store with nothing like it keeps the default.
- **`installs`** says what the client has under way, from its local files: each game being installed by its `source_id`, `Queued` (waiting its turn), `Downloading { done, total }` in bytes, or `Paused { done, total }`; an update of a game already installed is not one. `Some(vec![])` is nothing under way; `None` (the default) means the store keeps no word of its installs that can be read, and its games never join the Queue: Install opens its client, whose own window shows how it goes. It is asked once a second, off the UI thread, only while the Queue holds something or an install was just asked for (`library::queue`): keep it to a few small files. A game it reports joins the Queue even when the install was started in the client; once it no longer reports it, the source is read again, and the game leaves the Queue installed or dropped by what `read` then says. Only Steam has one (`appmanifest_<appid>.acf`'s `StateFlags` and bytes, and the registry's `Apps\<appid>\Updating` for the download that is running); the research found no honest local signal of an install in progress for the others (docs/research/stores.md).

A source holds no state that changes: one is made at start and shared, as `Arc<dyn Source>`, by the thread that reads the sources and the window that starts games.

## How the library merges them

`library::sources::read(&sources)` reads each in turn and logs what it gave; `Database::sync_all` merges each that was read under its key (`Database::sync`, per source). In the database a game is the row `(source, source_id)`:

- A game owned in two stores is **two games**, each with its own row, history and store mark.
- A game a source no longer lists (uninstalled, or no longer owned) is marked gone and not shown; it keeps its row, its Play sessions, favourite and hidden, and comes back as the same game if the source lists it again.
- Play sessions, favourite and hidden are the user's: a refresh never touches them. A source's own word on favourites or hidden is taken once, as the game is first seen.
- The snapshot's `user` is that of the source with the most games shown.

## Adding a source

1. A crate `crates/source-<store>` depending on `model` and `anyhow`, with a type that implements `model::Source`, its parsers tested on made-up fixtures only (never a real machine's files: `data/` is private).
2. One line in `crates/app/src/sources.rs` (`sources::all`), which is the list, in order, and one dependency in `crates/app/Cargo.toml`.
3. Its mark on tiles: a variant of `screens::home::Source` by its key, with the name the Game hub writes. Until it has one its games carry no mark. The mark is a plain-shape placeholder (`source_mark` is the one place to draw a real one): no store's logo comes into this repository.

4. Its fake for the VM: a `fixture` module that writes a made-up install whose installed games are the stand-in (`crates/standin`, `standin make-<store> <dir>`), a flag naming a folder the source reads in place of the machine's own (`--steam`, `--epic`, `--gog`, `--ea`, `--ubisoft`, `--battlenet`), and a function in `e2e/payloads/lib/E2E.psm1` that sets it up. A source that reads the registry reads it through `source_kit::registry::Registry`, which is Windows' or, for a fake and the tests, a `registry.json` of keys and values in the fake's folder.
5. For the tests on the TV PC (safe mode, `e2e/README.md`): the flag given an empty folder reads no games and never falls back to the machine's own, and the flag's name is in `$StoreFlags` in `E2E.psm1`, which starts the Dashboard on a fake or an empty folder for every store there. A store reached by an address (`steam://`, `com.epicgames.launcher://`, `uplay://`) has its stand-in in `standin open-address`, which finds the fake's folder in `handlers.json` beside it, never the registry; the helper that sets the fake up registers nothing in safe mode (`Register-Fake` in place of `Register-UrlHandler`).

Nothing in `library` or `shell` changes.

## The stores

Each from docs/research/stores.md, where the reasons are. What only a signed-in PC can confirm is listed under "Needs a human" in #14.

**Steam** (`source-steam`, key `steam`). Its install from `HKCU\Software\Valve\Steam\SteamPath`, or `--steam <root>` (an empty folder there is no Steam, and no games); the games, art and play history from its `appcache`, `steamapps` and `userdata` (the crate's own documentation says which file gives what). Starts through `steam://`. Installs and uninstalls as the Bundle does when Steam's CEF debugger is off, without Steam's windows: `steam.exe -silent +app_install <appid>` and `-silent -console +app_uninstall <appid>` (its `steam.exe` from the registry's `SteamExe`, or in `--steam <root>`; `steam://install/<appid>` when there is none), Steam's open windows sent to the tray first and its main window sent back for fifteen seconds should it come forward (`shell::steam`). A game reads installed only once its manifest's `StateFlags` say fully installed; `installs` reads the others. `running` is its registry's `Apps\<appid>\Running`.

**Epic Games** (`source-epic`, key `epic`, the game's `source_id` its `AppName`). The new launcher's files are read first, `%LOCALAPPDATA%\com.epicgames.eglv2` (`installations.jsonl`, the library and catalogue caches), and when they give no games the old launcher's: the installed games from `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item` (DLC and Unreal Engine content left out; a manifest whose folder has moved put right by `UnrealEngineLauncher\LauncherInstalled.dat`), and those owned and not installed from the catalogue cache `Data\Catalog\catcache.bin`, which exists only once the launcher has been signed in on the PC. No art (the launcher caches the pictures' addresses, not the pictures) and no play history (Epic keeps it on its servers). A game starts through the launcher's link, `com.epicgames.launcher://apps/<namespace>%3A<item>%3A<AppName>?action=launch&silent=true`, the launcher checking the game is the user's; Install asks with the same link and `action=install`, which shows the launcher's own dialog; nothing outside the launcher can uninstall, so Uninstall asks nothing. `running` is left to the game's windows. `--epic <dir>` reads `<dir>` as `%ProgramData%\Epic` (and `<dir>\com.epicgames.eglv2` as the new launcher's); the VM's fake is `standin make-epic`, with `standin epic-url <dir>` registered for the launcher's links as the stand-in launcher.

**GOG** (`source-gog`, key `gog`, the `source_id` the product id). The installed games are the registry's `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<id>`, which a GOG game's installer writes with or without Galaxy (`dependsOn` marks DLC), each checked against `goggame-<id>.info` in its folder, which tells a game from DLC. With GOG Galaxy, signed in once on the PC, its database `%ProgramData%\GOG.com\Galaxy\storage\galaxy-2.0.db` (read-only) adds the games owned and not installed (`gog_` release keys only: Galaxy lists other stores' too), play time and last played, and the cover (`verticalCover`) and hero (`background`) Galaxy has cached as WebP under `Galaxy\webcache`. A game starts as its own program, the `.info` file's primary play task, with no client at all (else what the registry says to run); Install runs `GalaxyClient.exe /gameId=<id> /command=installGame` where Galaxy is installed (the registry's `GalaxyClient\paths\client`), else asks nothing; Uninstall runs the game's own `unins000.exe`, which asks the user itself. `running` is left to the game's windows. `--gog <dir>` reads `<dir>` as `%ProgramData%\GOG.com`, the folders under `<dir>\Games` in place of the registry's list, and `<dir>\Client\GalaxyClient.exe` as Galaxy; the VM's fake is `standin make-gog`.

For either, the library logs what was read as for every source: `epic: 3 games from C:\e2e\epic in 4 ms`, `gog: 3 games from C:\e2e\gog in 9 ms`. With no launcher or no GOG on the machine, nothing is logged. A capability a later ticket needs from every source is a new method on the trait with a default, so the sources that have nothing to say need no change, as `installs` (#78) was.

**EA app** (`source-ea`, key `ea`). Installed games only: the Uninstall keys (`HKLM` and `HKCU`, both views) whose `UninstallString` runs EA's `EAInstaller\...\Cleanup.exe`, each with its folder's `__Installer\installerdata.xml` (UTF-8 or UTF-16, either of its two shapes), which gives the content id (`source_id`) and the program that starts the game. No local file lists owned games. Start: that program, in the game's folder (it brings the EA app up itself when it needs it); with none, `origin2://game/launch?offerIds=<id>&autoDownload=1`. Install: nothing. Uninstall: the key's `UninstallString`. No art: EA keeps none on disk. Not installed: no `HKLM\SOFTWARE\Electronic Arts\EA Desktop` and no such key. The encrypted `IS` file is not read.

**Ubisoft Connect** (`source-ubisoft`, key `ubisoft`). Installed: `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>\InstallDir`. Owned: the client's `cache\configuration\configurations` (under `%LOCALAPPDATA%\Ubisoft Game Launcher`, else under the client's folder), protobuf around YAML; the games are the products that are not DLC, not another store's copy, have something to start and are no other product's addon. A game is its `uplay_id`; an `Installs` key matches it by its `uplay_id` or its `install_id`, as the readers disagree on which. With no cache, or one that can't be read (said in the log), the installed games are listed by their folders' names. Cover and background from the client's `cache\assets` where they are there. Start, install, uninstall: `uplay://launch/<id>`, `uplay://install/<id>`, `uplay://uninstall/<id>`.

**Battle.net** (`source-battlenet`, key `battlenet`). Installed games only: the products in `%PROGRAMDATA%\Battle.net\Agent\product.db` (protobuf) that its agent marks installed and whose folder is there, less the client's own (agent, beta, battle.net). A game is its product code (`source_id`). Names from `aggregate.json` beside it, else the game's Uninstall key (`--uid=<uid>` in its `UninstallString`), else the uid. No local file lists owned games. The client is the Uninstall key with `-uid=battle.net`, whose folder has `Battle.net.exe`. Start: `Battle.net.exe "--exec=launch <code>"`. Install: `Battle.net.exe --game=<uid>`, the game's page. Uninstall: the game's key's `UninstallString`.

For these three too, `running` is left to the game's windows. `--ea`, `--ubisoft` and `--battlenet <dir>` read a fake at `<dir>` (its `registry.json` for the registry), written on the VM by `standin make-ea`, `make-ubisoft` and `make-battlenet`; `standin uplay-url <dir>` is registered for `uplay://` as the stand-in Ubisoft Connect, and the fake Battle.net's `Battle.net.exe` is the standin, the stand-in client.
