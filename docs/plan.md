# Plan: the full port to Rust

Written 2026-11-15, after the proof of concept. It is a first plan, to be sharpened ticket by ticket. The research behind it is [research/standalone-rust-xbox-mode.md](research/standalone-rust-xbox-mode.md).

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

A cold-boot figure for the Rust side has not been measured yet; that is the first thing M0 does.

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
5. **Two ways to run it, as the research found.** The Boot shell route is the main one, where the Guide button stays ours. A packaged home app for Xbox mode comes last, with the Guide on View+Menu there.
6. **The Playnite Bundle keeps shipping** until the port reaches parity. Its glossary (`CONTEXT.md`) is the specification for behaviour; the port copies it and maintains its own from M1.
7. **Tests look at pixels.** Every screen has an offscreen screenshot test that runs on Linux in CI. End-to-end tests run on the existing win11-e2e VM, one at a time.

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

| | Milestone | Ends with |
|---|---|---|
| **M0** | **Foundations.** CI, the workspace split, the `ui` layer's core, the screenshot harness, a cold-boot measurement, the Overlay spike. | The proof of concept's Home rebuilt on the real `ui` layer, tested by screenshot in CI. |
| **M1** | **Home.** The whole Home page: top bar, Recent row, wide tiles, Groups, scrolling, the background's delay and cross-fade, the swirl. The library database, Steam source, launching and the Launch screen done properly. Navigation sounds. | Home at parity with the Bundle's, for Steam games. |
| **M2** | **The Guide and Toasts.** The real Overlay. The Guide's landing tab, Profile & system, the Utilities and their Flyouts (Audio, Notifications), Power on a hold. Toasts. | The Guide over any game, dimmed and translucent as the Bundle's is. |
| **M3** | **The library pages.** My games & apps, the Game hub, the Tile menu, Search with the Console keyboard, Settings. Groups and filters without Playnite's presets. | Every Dashboard page navigable. |
| **M4** | **More stores, and installs.** Xbox and Game Pass, Epic, GOG, EA, Ubisoft, Battle.net. Apps. Install, uninstall, the Queue and Manage Queue. Art and metadata for games a store gives none for. | A mixed library from every store on the TV PC. |
| **M5** | **Start-up and the living room.** The Boot video, Who's playing today?, Profiles, the Idle screen, Now Playing. | A cold boot into the Dashboard that looks like the console's. |
| **M6** | **Discord and achievements.** The Discord tab and Calls. Achievements, Score, the Achievements page and their Toasts, without PlayniteAchievements. | The Bundle's last features ported. |
| **M7** | **Shipping.** The Boot shell's job folded in or pointed at the new exe. The Xbox mode home-app package. Releases, signing, updates. The Bundle retired. | A public release. |

## Parity list

From the Bundle's glossary. Every item ends up ported or dropped on purpose.

- **M1:** Home, Recent row, Group, Launch screen, Play session, Spinner
- **M2:** Guide, Overlay, Status area, Utility, Flyout, Toast, Guide button
- **M3:** My games & apps, Tile menu, Game hub, Search, Console keyboard, Settings, Manage
- **M4:** Apps, Queue, Manage Queue
- **M5:** Start-up, Boot video, Profile, Who's playing today?, Switch profile, Idle screen, Now Playing, Now Playing source
- **M6:** Discord tab, Call, Known channel, Shortcut, Servers, Achievements page, Score, Rare
- **M7:** Boot shell
- **Goes away:** Playnite settings, and "Bundle" and "Dashboard" as separate things (it is one program now)

## Open questions

1. **Achievements without PlayniteAchievements.** Steam's can be read with a Web API key or from local files; Xbox's need sign-in. How much of this is worth building?
2. **Groups and filters.** In the Bundle they are Playnite filter presets. What replaces them: saved filters of our own, Steam collections, or both?
3. **Art and metadata for non-Steam games.** Which source, and under what terms (SteamGridDB, IGDB)?
4. **The console's sounds and boot videos.** The Bundle ships recordings that belong to Microsoft, under its NOTICE. Do they come over, and does that change if this repository goes public?
5. **Licence and name.** PolyForm Noncommercial as the Bundle, or something else? Is "Xbox Series Dashboard" the name?
6. **Games that need more than a store launch:** emulators, manually added games, launch options. Playnite handles these today.
7. **Exclusive-fullscreen games** under the Overlay: accept the Bundle's limits, or look again?
8. **How the user's Playnite data moves over:** play time, favourites, hidden games, Profiles.
