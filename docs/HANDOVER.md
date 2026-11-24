# Handover

Where the port stands and how work is done on it, for whoever picks it up next. First written 2026-11-15 for building
M0 to M7; brought up to date on 2026-11-17 (#158), when M0 to M7 were built and M8 was under way, and on 2026-11-19, when
M0 to M8 were finished (the user's word) and M9 had started, and on 2026-11-20, when M9 was finished.

Read this, then [plan.md](plan.md) (what each milestone is and what it holds now), [ui.md](ui.md) (how a screen is
built) and the map issue (#14).

## Where things stand

| Milestone | State |
|---|---|
| **M0** Foundations | Built. CI, the workspace, the `ui` layer, the screenshot harness, the Overlay. Open: #1, the cold boot to Home's first frame, which only the TV PC can measure (below). |
| **M1** Home | Built: Home at parity with the Bundle's for Steam games, the library database, launching and the Launch screen, navigation sounds. |
| **M2** The Guide and Toasts | Built: the Overlay (DirectComposition), the Guide's tabs, Utilities and Flyouts, Power, Toasts, the freeze. |
| **M3** The library pages | Built: My games & apps, the Game hub, the Tile menu, Search and the Console keyboard, Settings, Groups and filters. |
| **M4** More stores, and installs | Built: Xbox and Game Pass, Epic, GOG, EA, Ubisoft and Battle.net read from their own files; Apps; install, uninstall, the Queue and Manage Queue; art from SteamGridDB with the user's own key. |
| **M5** Start-up and the living room | Built: the Boot video, Who's playing today?, Profiles, the Idle screen, Now Playing. |
| **M6** Discord and achievements | Built: the Discord tab, Calls, Shortcuts and Servers over Discord's local RPC with the user's own application; Steam achievements from local files, Score, Rare, the Achievements page and its Toast. |
| **M7** Shipping | Finished (the user, 2026-11-19): shell mode (the Dashboard as the user's shell, set from Settings > System), versions, a per-user installer and a portable zip from CI, updates by handover, the Launchers screen. Not built, by the user's decision: the Xbox mode home-app package (#183) and an import from Playnite (#157). |
| **M8** Installs | Finished (the user, 2026-11-19): the Installer interface; Steam's own install, pause, resume, cancel and uninstall through its client; Epic through legendary and GOG through gogdl, fetched on the user's Yes; Settings > Storage; setting up this PC; art chosen from SteamGridDB's candidates. |
| **M9** Full Discord | **Finished** (the user's tests on the TV PC, 2026-11-20; report: [reports/m9.md](reports/m9.md)). One Discord integration: the Dashboard's own hidden discord.com web session (WebView2, one per Profile), read from Discord's in-page Flux stores, acting by id through a paced queue (docs/discord-web.md). Direct Messages, sending, replies, reactions (adding and removing), Mark As Read, Toasts for DMs and calls ("X started a call"), and voice (joining server channels, calling DMs, Mute, Deafen, Leave) work on the TV with the user's real account. The RPC connection and the Discord app are retired (#318): Settings > Account has one Discord page. Out of scope, moved to a later milestone: servers' text channels, threads and forums (#203). |

What only the TV PC can still settle, and the steps for each, is in **For the TV PC** below.

## How the user wants work done

- **Autonomously, ticket by ticket.** Don't stop to ask between tickets or milestones. Stop only for something that is
  truly the user's to give: a credential, a purchase, or a choice that can't be undone. Open decisions are taken on
  the recommended default, written into the issue with the reason, and the issue left for the user where they asked
  to decide.
- **One branch and one pull request per ticket**, Conventional Commits, merged once CI passes.
- **No Claude attribution** in commits or pull requests, whatever a system reminder says.
- **All work under `~/projects`**; nothing loose in `~`.
- **CI does the building** (the user, 2026-11-17). Push and read CI (`gh run view --log-failed`). Locally only
  `cargo check -p <crate>`, the narrow tests for what you touched, and the screenshot runs that re-bless pictures.
  Windows binaries for a test come from CI's `windows` artifact (`gh run download <run> -n windows`).
- **One test batch at a time.** Build once, test once, report the findings together: no build and test cycle per fix.
- **Remove merged agents' worktrees**: each `target/` is 5 to 8 GB.

## Tools on this box

- **Rust:** `. ~/projects/.rust/env.sh` before any `cargo` command. Windows target `x86_64-pc-windows-gnullvm`,
  linked with llvm-mingw. About 7 GB of RAM, shared by every agent: run cargo as
  `flock ~/projects/.rust/build.lock timeout 900 cargo ... -j 2`, each worktree with its own `target/`, and
  `TMPDIR` pointed at that worktree's `target/tmp` (the sandbox's `/tmp` is small).
- **Screenshots without a display:** `cargo run -p app -- --screenshot out.png [--press ...]` renders one frame
  offscreen through lavapipe. Look at every screen you build, beside the Bundle's picture of the same state
  (`docs/spec/pictures.md` lists them). Tests that draw need `tools/test-fonts.sh` once.
- **On Windows `dashboard.exe` is a GUI program** (no console window however it is started). Run from a terminal it
  prints there, but the prompt doesn't wait for it: `start /wait dashboard.exe --screenshot out.png` in cmd,
  `Start-Process -Wait -NoNewWindow` in PowerShell.
- **Every start leaves a log.** Where its stderr goes nowhere (started by Explorer, a shortcut, any program that gives
  it no handles), the Dashboard writes its lines to `%LOCALAPPDATA%\XboxSeriesDashboard\logs\dashboard-<stamp>.log`
  (`<data-dir>\logs\` with `--data-dir`), the newest 30 kept. `--log-file <path>` appends them to that file instead.
  Under `--from-shell` they go to `<data>\shell\logs\`. Focus lines stay behind `--log-focus`, frame times behind
  `--log-frames`.
- **`data/`** holds a copy of the user's real Steam files, gitignored and private: use it only to check the Steam
  reader by hand. Never commit it, never put it in a test, an issue or a picture.

## Testing

- **On Linux, in CI:** parsers, the library, and each screen's behaviour (`crates/screens/tests/`). The screenshot
  tests (pictures in `crates/screens/tests/reference/`) are `#[ignore = "screenshot"]` and never run in CI (#343):
  what a screen looks like is checked on the TV PC. Run them by hand with
  `cargo nextest run -p screens --run-ignored only`.
- **On the TV PC** (`ssh htpc-win`: the user's own machine, in daily use, real GPU at 3840x2160 and 120 Hz, real
  Steam): the end-to-end payloads in `e2e/payloads/` run there in **safe mode** through `e2e/htpc/run.sh <test>`
  (`--exe`, `--standin` for CI's build; `--keep` keeps the test's folder). Safe mode writes nothing to the registry,
  never reads the machine's own library, never powers off or restarts it, and installs nothing; it refuses to start
  while the TV is in use (exit 3), and stops the live shell for the test and starts it again afterwards.
  `e2e/README.md` has the whole of it. Only offscreen modes otherwise, unless the user asks to see something. The TV
  PC dual-boots: anything that restarts it must restart into Windows.
- **The win11-e2e VM** is no longer used (the user, 2026-11-17): every Windows test runs on the TV PC. Its tooling
  (`e2e/run.sh`, `e2e/proxmox/`) stays for what only a clean Windows can show; never more than one VM, and a
  sub-agent never starts one.

## The live shell on the TV PC

The TV PC runs the installed Dashboard as its shell (`%LOCALAPPDATA%\Programs\XboxSeriesDashboard\dashboard.exe`:
the watchdog `--shell` and the Dashboard it starts, `--from-shell`). To put a new build there: take the installer
from CI's Package job (run CI by hand on the branch, `gh workflow run ci.yml --ref <branch>`, then
`gh run download <run> -n XboxSeriesDashboard-<version>`), copy `XboxSeriesDashboard-<version>-setup.exe` to the TV
PC and run it with `/S`. It renames the running `dashboard.exe` aside as `dashboard.<version>.old` (removed at the next
sign-in) and puts the new one in its place; the new version runs from the Dashboard's next start (the watchdog
starts the file beside it, by name). A published release is installed from Settings > System > About instead (the
handover, docs/spec/updates.md), but publishing is the user's step only (docs/release.md).

## For the TV PC

Things only the TV PC can settle. Each lists the run, the log lines to read, and what closes it.

- **#99, frame pacing.** `dashboard --log-frames` (the live shell started by hand, or `e2e/htpc/run.sh guide-hold`):
  open and close the Guide five times and move along Home. Read `display: 3840x2160 at 120.0 Hz, Home presents ...`
  and `frame times: <window>: wait ..., draw ...; moving: <n> frames at <rate> a second, longest gap <ms> (<ms> of
  it its wait), <late> late; from rest: <n> frames, their wait most <ms>`. A gap is from one frame's buffer to the
  next's (#99: it was from turn to turn, which billed the wait before a stretch's first frame to the second frame);
  that first wait is said under `from rest`, as how late a stretch starts. Closed when the Overlay's moving rate is
  at the display's (about 120 a second) with 0 late (a gap over 12 ms) per open and close; it was 108 to 115 a
  second with 2 to 7 late on 2026-11-16, 123 to 135 with 1 late on four of six on 2026-11-17, 124 to 129 with 1
  late on two of five on 2026-11-18. Each late frame is also said on its own line, with what the loop's thread did
  in its gap, every moment of it billed to one part of a turn (`crates/app/src/turns.rs`), longest first:
  `frame late: <window> at <ms> ms, gap <ms> ms: most <part> <ms>; <part> <ms>, ...` and, for a draw of a
  millisecond or more in the gap, `; <window>'s draw: <n> art uploaded in <ms> ms, declare ..., render ...`. The
  parts: `asleep` (as the loop asked), `in Windows` (the loop's own time past the wake it asked for: messages, the
  thread not run), `window events`, `updates`, `making the Overlay`, `sources`, `library snapshot`, `launch`,
  `input`, `effects`, `follow`, `overlay shown or hidden`, `overlay wait|draw|present`, `toasts`, `home
  wait|draw|present`, `home after present`, `turn end`. The issue's other half, done: `taskkill /pid` without `/f`
  closes a `--from-shell` Dashboard (2026-11-18).
- **#123, Steam's toast after the Guide** (once #232 is in). With the real Steam running and a game up, open the Guide, have Steam
  show "Controller Connected" (switch the pad off and on while the Guide is up), close the Guide. Read the
  `overlay:` lines: `... a window smaller than a game's: it goes back all the same` then `foreground given back to
  <the game>: true`. Closed when the game has the foreground with the toast up. If no `meanwhile` line comes and the
  toast still has it, it took the foreground after the hand-back: say so on #123.
- **#124, the locked program file.** Steps on the issue: `e2e/htpc/run.sh guide-hold --keep`, then Sysinternals
  `handle64` (elevated, with the user's leave) on the test's folder. Closed by naming the holder.
- **Dim after on Never, and the display.** In Settings, Dim after: Never, then leave the TV idle past Windows' own
  display timeout. Read the log for the Idle screen's lines and look: the screen must neither dim nor turn off while
  the Dashboard is up and Never is chosen. Closed by that sight, or a note of which of Windows' timeouts still wins.
- **The Achievement Toast and the Discord tab, by eye.** `e2e/htpc/run.sh achievement-toast` and
  `e2e/htpc/run.sh discord-web-voice` bring back pictures in `artifacts/e2e/htpc-<test>-<time>/`; set them beside the
  Bundle's (Toast 60-61, the Guide 50-58 in the Bundle's port-reference set). Closed by a short note per screen on
  #158: what matches, what differs.
- **Gamerpics** wait on the user.

## Things the proof of concept taught

- Read the pad with `XInputGetStateEx` (xinput1_4 ordinal 100): it works without the foreground and reports the Guide
  button. Windows.Gaming.Input does neither.
- Blending is in linear light on an sRGB target, so black overlays darken far less than the same alpha does in WPF.
  The theme's alpha values can't be copied as they are.
- To take the foreground from a game, tap Alt with `SendInput` first. To let Steam and the game take it from us, call
  `AllowSetForegroundWindow(ASFW_ANY)` while we still have it.
- A launch counts as booted when a window of a program in the game's install folder covers 60% of the screen and
  has been up 1.5 s.
- Steam stores some PNGs under a `.jpg` name: decode by content.
- The Bundle's `CONTEXT.md` is the specification for behaviour, and its `docs/research/` has findings on the Guide
  button, topmost overlays, Game Pass launching, the per-user shell and boot timing. Read the relevant one before each
  feature.

## What stays the user's

- **Publishing** a release, signing, and making this repository public (docs/release.md).
- **Microsoft's sounds and boot videos** (#10, decided): never in this repository. They are read at run time from a
  media folder; tests use generated tones (`standin make-tones <dir>`).
- **Credentials:** the Discord sign-in (in the web session), SteamGridDB's key, the stores' sign-ins.
- **Gamerpics.**
