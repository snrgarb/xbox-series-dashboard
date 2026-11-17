# Plan: the full port to Rust

Written 2026-11-15, after the proof of concept, and brought up to date on 2026-11-17 (#158): M0 to M7 are built, M8 is under way, and each milestone's state is in the table below. Decisions taken since the first plan name their issue. The research behind it is [research/standalone-rust-xbox-mode.md](research/standalone-rust-xbox-mode.md).

## Goal

One native executable that is the whole product: the Dashboard, the Guide, Toasts, the Launch screen, Start-up and the rest of what the Playnite Bundle does today, with its own game library. No Playnite, no .NET, no WPF.

**Done means** a TV PC can run it in place of Playnite and the Bundle, with every feature in the parity list below either ported or explicitly dropped.

## What the proof of concept showed

Measured on the TV PC (RX 9070 XT, DX12), 2026-11-15:

| Measure | Rust proof of concept | Playnite + Bundle today |
|---|---|---|
| Process start to first frame on screen | 0.37–0.43 s (warm) | 7.9 s cold, 3.8 s warm |
| Reading the whole Steam library (286 games) | 0.12–0.14 s | n/a (Playnite's database) |
| Frame time at 1080p, focus animating | 0.26 ms | not measured |
| Memory | about 150 MB | not measured |
| Size | one 15 MB exe | Playnite + 16 extensions |

A cold-boot figure for the Rust side is still to be measured on the TV PC (#1; the steps are in HANDOVER.md, "For the TV PC"). On the test VM, through software rendering, the first frame came 425 to 655 ms after the process started.

**What worked:**

- Steam's library, read straight from its own files: owned games from `packageinfo.vdf`, names and types from `appinfo.vdf`, play history from `localconfig.vdf`, art from `librarycache`.
- A launch through `steam://rungameid`, with the window as a topmost, click-through Launch screen until the game's own window had been up 1.5 s.
- The Guide button read through `XInputGetStateEx` while a game had the foreground, and the Guide shown over the game.
- Development from Linux: cross-compiling to Windows, and rendering screenshots with no display, here and on the TV PC over SSH.

**What it faked or skipped,** and the port must do properly:

- The Guide over a game is a window cut down to the panel's rectangle. The game beside it isn't dimmed and nothing can be translucent over it. The real Overlay needs per-pixel transparency.
- The UI is one hand-laid-out screen. There is no layout, focus or widget system.
- No sounds, no video, no settings, no persistence. Steam only.

## Decisions taken

Each is my recommended default. Any of them can be reopened in its ticket.

1. **Fully standalone.** The library is ours, read from each store's own files and APIs. Playnite is not a dependency, even in the background. A one-time import from a Playnite library is a possible later convenience.
2. **Windows only.** Linux is a development and test platform (offscreen rendering, parsers), not a target.
3. **The proof of concept's stack stays:** `wgpu` on DX12, `winit`, `glyphon` for text, XInput for the pad, Win32 through `windows-sys`.
4. **Our own small UI layer** on that stack, not a general toolkit. The Dashboard is a fixed set of 10-foot screens driven by a pad: it needs focus navigation, lists that scroll, and animation, and nothing a desktop toolkit is built around.
5. **Shell mode is the route** (#154): the Dashboard is the user's shell, set and unset from Settings > System, with a watchdog that starts it again; the Guide button stays ours. The packaged home app for Xbox mode is not built (#183, the user's decision of 2026-11-17).
6. **The Playnite Bundle keeps shipping** until the port reaches parity. Its glossary (`CONTEXT.md`) is the specification for behaviour; the port copies it and maintains its own from M1.
7. **Tests look at pixels.** Every screen has an offscreen screenshot test that runs on Linux in CI. End-to-end tests run on the TV PC in safe mode (`e2e/htpc/run.sh`, since 2026-11-17); the win11-e2e VM is kept only for what needs a clean Windows.

## Architecture

A Cargo workspace. The proof of concept's three crates grow into these:

| Crate | What it holds |
|---|---|
| `model` | The domain: games, apps, profiles, play sessions, the Queue. No I/O. |
| `library` | The database (SQLite) and the merge of every source into one library, with play history and favourites. |
| `source-*` | One crate per store: `steam` first, then `xbox`, `epic`, `gog`, `ea`, `ubisoft`, `battlenet`. Each lists games, finds art, launches, installs and uninstalls. |
| `render` | The GPU renderer: quads, images, text, blur, video frames. |
| `ui` | The UI layer: a tree of elements, layout, focus navigation, animation, scrolling lists. |
| `screens` | Home, My games & apps, the Game hub, Search, Settings, the Guide, Start-up and the rest, built on `ui`. |
| `shell` | Windows itself: the windows (main, Overlay, Toasts, Launch screen), z-order and foreground, launching and tracking games, power, audio devices, media sessions. |
| `input` | XInput, the keyboard, the Guide button, hold detection. |
| `media` | Sounds and video (Media Foundation). |
| `services` | Discord, achievements, notifications. |
| `app` | The executable: wires it together, settings, logging, updates. |

Two things in it are new ground and get a spike each before they are built on:

- **The Overlay**: a transparent, topmost window drawn by `wgpu`, most likely through DirectComposition.
- **Video in the renderer**: Media Foundation decoding into a texture, for the Boot video and the Idle screen.

## Milestones

Each ends in something that runs on the TV PC.

| | Milestone | Ends with | State (2026-11-17) |
|---|---|---|---|
| **M0** | **Foundations.** CI, the workspace split, the `ui` layer's core, the screenshot harness, a cold-boot measurement, the Overlay spike. | The proof of concept's Home rebuilt on the real `ui` layer, tested by screenshot in CI. | Built; #1 (the cold boot) waits on the TV PC |
| **M1** | **Home.** The whole Home page: top bar, Recent row, wide tiles, Groups, scrolling, the background's delay and cross-fade, the swirl. The library database, Steam source, launching and the Launch screen done properly. Navigation sounds. | Home at parity with the Bundle's, for Steam games. | Built |
| **M2** | **The Guide and Toasts.** The real Overlay. The Guide's landing tab, Profile & system, the Utilities and their Flyouts (Audio, Notifications), Power on a hold. Toasts. | The Guide over any game, dimmed and translucent as the Bundle's is. | Built |
| **M3** | **The library pages.** My games & apps, the Game hub, the Tile menu, Search with the Console keyboard, Settings. Groups and filters without Playnite's presets. | Every Dashboard page navigable. | Built |
| **M4** | **More stores, and installs.** Xbox and Game Pass, Epic, GOG, EA, Ubisoft, Battle.net. Apps. Install, uninstall, the Queue and Manage Queue. Art and metadata for games a store gives none for. | A mixed library from every store on the TV PC. | Built |
| **M5** | **Start-up and the living room.** The Boot video, Who's playing today?, Profiles, the Idle screen, Now Playing. | A cold boot into the Dashboard that looks like the console's. | Built |
| **M6** | **Discord and achievements.** The Discord tab and Calls. Achievements, Score, the Achievements page and their Toasts, without PlayniteAchievements. | The Bundle's last features ported. | Built |
| **M7** | **Shipping.** The Boot shell's job folded in or pointed at the new exe. The Xbox mode home-app package. Releases, signing, updates. The Bundle retired. | A public release. | Built, but for the M7 run and retiring the Bundle (#159) and the polish pass (#158). Not built by the user's decision: the Xbox mode package (#183), an import from Playnite (#157). Publishing is the user's |
| **M8** | **Installs.** Installing and uninstalling every launcher's games from the Dashboard alone (docs/research/installs.md): the Installer interface; Pause, Resume and Cancel in the Queue; Steam through its own client, Epic through legendary, GOG through gogdl, Xbox, Battle.net, EA and Ubisoft; Settings > Storage; setting up this PC (the launchers' own apps); art chosen from SteamGridDB's candidates. | Every store's games installed and removed from the Dashboard. | Under way: Steam, Epic, GOG, Storage, Set up this PC and the art picker built; Xbox (#173), Battle.net (#172), EA and Ubisoft (#177) open |
| **M9** | **Full Discord.** Servers, channels, DMs, messages and voice inside the Dashboard, by the route the Discord Bridge extension takes (#192): a Discord web session the Dashboard hosts, Party and chats in the Guide, Discord's notifications as Toasts. | Discord used from the couch without its own window. | Not started; after M8 (#196 to #203) |

## Parity list

From the Bundle's glossary. Every item ends up ported or dropped on purpose.

- **M1:** Home, Recent row, Group, Launch screen, Play session, Spinner
- **M2:** Guide, Overlay, Status area, Utility, Flyout, Toast, Guide button
- **M3:** My games & apps, Tile menu, Game hub, Search, Console keyboard, Settings, Manage
- **M4:** Apps, Queue, Manage Queue
- **M5:** Start-up, Boot video, Profile, Who's playing today?, Switch profile, Idle screen, Now Playing, Now Playing source
- **M6:** Discord tab, Call, Known channel, Shortcut, Servers, Achievements page, Score, Rare
- **M7:** Boot shell (as shell mode)
- **Goes away:** Playnite settings, and "Bundle" and "Dashboard" as separate things (it is one program now)

## Open questions

1. **Achievements without PlayniteAchievements.** Answered in M6: Steam's from its local files (docs/achievements.md). Xbox's, which need sign-in, are not built.
2. **Groups and filters.** Answered in M3: saved filters of our own and the stores' collections (docs/spec/filters.md).
3. **Art and metadata for non-Steam games.** Which source, and under what terms (SteamGridDB, IGDB)? Art: SteamGridDB with the user's own key (#80, docs/sources.md "Art"). Metadata beyond what the stores' files hold is still open; Refresh art and details fetches both again (#209), and the art picker chooses among SteamGridDB's candidates (#216).
4. **The console's sounds and boot videos.** Decided (#10): they never come into this repository. The Dashboard reads them at run time from a media folder, or from the Bundle where Playnite has it; tests use generated tones.
5. **Licence and name.** PolyForm Noncommercial as the Bundle, or something else? Is "Xbox Series Dashboard" the name?
6. **Games that need more than a store launch:** programs added by hand are Apps (docs/spec/apps.md); emulators and launch options are still open.
7. **Exclusive-fullscreen games** under the Overlay: the Bundle's limits are accepted, and said in the log (docs/spec/guide.md, 7.2).
8. **How the user's Playnite data moves over:** it doesn't: no import from Playnite (#157, the user's decision of 2026-11-17). The stores' own files give play time, favourites and hidden games, and the Launchers screen (#163) brings each store in.
