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

A source holds no state that changes: one is made at start and shared, as `Arc<dyn Source>`, by the thread that reads the sources and the window that starts games.

## How the library merges them

`library::sources::read(&sources)` reads each in turn and logs what it gave; `Database::sync_all` merges each that was read under its key (`Database::sync`, per source). In the database a game is the row `(source, source_id)`:

- A game owned in two stores is **two games**, each with its own row, history and store mark.
- A game a source no longer lists (uninstalled, or no longer owned) is marked gone and not shown; it keeps its row, its Play sessions, favourite and hidden, and comes back as the same game if the source lists it again.
- Play sessions, favourite and hidden are the user's: a refresh never touches them. A source's own word on favourites or hidden is taken once, as the game is first seen.
- The snapshot's `user` is that of the source with the most games shown.

## Games added by hand

The source `manual` (`library::by_hand::Manual`, docs/spec/apps.md) is the user's: its games are added, renamed and removed in Settings' Games & apps, straight into the database (`Database::by_hand`), with `source_id` the program's full path. Its `read` gives nothing, so a sync never touches them; its `start` runs the program in its own folder. Apps are not games and not a source's: they are kept apart, in the `apps` table, and started as they are.

## Adding a source

1. A crate `crates/source-<store>` depending on `model` and `anyhow`, with a type that implements `model::Source`, its parsers tested on made-up fixtures only (never a real machine's files: `data/` is private).
2. One line in `crates/app/src/sources.rs` (`sources::all`), which is the list, in order, and one dependency in `crates/app/Cargo.toml`.
3. Its mark on tiles: a variant of `screens::home::Source` by its key, with the name the Game hub writes. Until it has one its games carry no mark. The mark is a plain-shape placeholder (`source_mark` is the one place to draw a real one): no store's logo comes into this repository.

Nothing in `library` or `shell` changes. A capability a later ticket needs from every source (installs in progress for the Queue, #78) is a new method on the trait with a default, so the sources that have nothing to say need no change.
