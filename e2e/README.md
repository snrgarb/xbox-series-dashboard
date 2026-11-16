# End-to-end tests

Each test runs the Windows build on a throwaway Windows 11 VM, cloned from the `win11-e2e` template on the Proxmox host
and destroyed after, and brings back screenshots and logs.

## Running one

```sh
cargo build --release --target x86_64-pc-windows-gnullvm -j 6   # dashboard.exe and standin.exe
e2e/proxmox/vm.sh list                                           # must print nothing
e2e/run.sh smoke
```

`e2e/run.sh <test> [--exe <dashboard.exe>] [--standin <standin.exe>] [--keep]` stages `payloads/<test>/`,
`payloads/lib/` and the two programs, brings one VM up, runs the test's `run.ps1` on the signed-in user's desktop,
collects what it wrote into `artifacts/e2e/<test>-<time>/`, and destroys the VM whatever happened. It exits 0 when the
test passed. `--keep` leaves the VM up to look at in the Proxmox console; `e2e/proxmox/vm.sh down <vmid>` destroys it.

What comes back:

- `steps.txt`: the test's own account, a line a step, ending `result: PASS` or `result: FAIL` after a `failed:` line for
  each thing that was wrong. `run.sh` prints it.
- `dashboard.log`: everything the Dashboard logged (its stderr).
- `standin.log`: what the stand-in Steam and games did.
- `fake-steam.json`: the fake library as the Dashboard's Steam reader read it.
- `*.png`: the screen at each step. `error.png` is the screen when the test threw.
- `boot.log`, `exitcode.txt`: the harness's own.

The VM's settings and API token are in `~/.config/xbox-e2e/proxmox.env`, outside the repository; `proxmox/vm.sh`'s header
says what goes in it.

## One VM at a time

**Never more than one test VM.** The Proxmox host has 32 GB of memory and concurrent VMs have crashed it. `vm.sh up`
refuses while `vm.sh list` shows any test VM (tagged `xbox-e2e`), and so `run.sh` does too. Run tests one after another,
and if a run was killed before it could clean up, `vm.sh list` then `vm.sh down <vmid>`.

Write and unit-test a whole batch of fixes first, then test them serially on one VM at a time.

## What the VM is not

- **It has no real GPU.** The Dashboard draws through WARP, Microsoft's software adapter: the `adapter:` line in
  `dashboard.log` says which adapter it got. Frame times and time-to-first-frame from the VM mean nothing; what is on
  the screenshots does.
- **It has no Steam and no games.** The tests bring a fake Steam (below).
- It has no real controller, display or audio device. The pad is a virtual Xbox 360 pad on ViGEmBus, which the template
  has installed; XInput sees it as a real one, the Guide button included.

Anything only real hardware can show (GPU timings, a cold boot, exclusive-fullscreen games, real pads, the TV) is for
the TV PC, not for here.

## The fake Steam and the stand-ins

`standin.exe` (`crates/standin`) is several things, and has no console window in any of them:

- **`standin make-steam <dir>`** writes a fake Steam install: binary `appcache/appinfo.vdf` and `packageinfo.vdf`, the
  user's `localconfig.vdf`, `steamapps/libraryfolders.vdf`, an `appmanifest_<appid>.acf` and a folder under
  `steamapps/common/` for each installed game, and cover, hero and header art under `appcache/librarycache/`. The writer
  is `source_steam::fixture`, which the unit tests on Linux use too. `Fixture::standard()` is fourteen made-up games:
  six played, so the Recent row is, in order, Ashfall Courier, Lanternfish Deep, Brass Meridian, Quiet Orbit (not
  installed), Thimble & Thorn (its art a PNG named `.jpg`) and Saltwind Rally (not installed); eight never played, two
  of them with no art. Each game's art is a gradient of its own colour. With `--full-shelf` it is
  `Fixture::full_shelf()`: those and twelve more, installed and never played, so that twenty are installed and
  My games & apps' grid has rows to scroll (`Initialize-FakeSteam -FullShelf`).
- **`standin make-tones <dir>`** writes a 0.15 s tone under each of the names the Dashboard's sounds have in the
  Bundle (`model::Sound`), a semitone apart. The console's own sounds are in no repository and on no test machine;
  a test points the Dashboard at the tones with `--media <dir>` and has it log what it plays with `--log-sounds`
  (`audio: 12 sounds from <dir>`, `audio: device "<name>" started, ...` at the first sound, `audio: played <file>`
  for each).
- **The stand-in game.** Every installed game's folder holds a copy of `standin.exe` named `game.exe`, which shows a
  borderless, full-screen, dark-blue window titled with the game's name (from `game-args.txt` beside it) until it is
  closed, Esc is pressed, or ten minutes pass. So "a program in the game's install folder has a window covering the
  screen", which is how the Dashboard knows a game has booted, is true of it. Its **pulse**, a square at x 1700..1820,
  y 900..1020 of a 1920x1080 screen, is one of two colours and changes four times a second while the game runs: a
  frozen game paints nothing, so its pulse stands still, and when a beat comes more than 1.5 s late the game says
  `game: stood still for <n> ms` in `standin.log`. A test that wants a launcher or splash
  first adds `--splash-ms` and its length on two lines of `game-args.txt`: a small window titled "<name> launcher" then
  shows for that long before the game's. A test that wants a game that has hung adds `--ignore-close` on a line: the
  game then stays when its window is asked to close, and says `game: asked to close, and staying`.
- **The stand-in Steam**, `standin steam-url steam://rungameid/<appid>`: finds the game's folder from its app manifest
  under the `SteamPath` in the registry, starts its `game.exe`, and keeps `HKCU\Software\Valve\Steam\Apps\<appid>\Running`
  at 1 until it exits, as Steam does. For a game that isn't installed it does nothing and exits 1.

- **The stand-in Steam client.** `make-steam` also puts a copy of `standin.exe` in the fake's folder as `steam.exe`,
  which the Dashboard asks to install and uninstall as it asks the real one (`steam.exe -silent +app_install <appid>`,
  `-silent -console +app_uninstall <appid>`). It says `steam: asked to install <appid>: <arguments>` in `standin.log`,
  and writes the game's app manifest as Steam does as it takes an install on (queued), or removes it (uninstalled). A
  test then moves the install along as Steam's download would, by rewriting that manifest: **`standin steam-install
  <root> <appid> --queued | --bytes <n>/<m> | --paused <n>/<m> | --done | --drop`** (done copies the stand-in game in as
  the game's `game.exe`). Nothing is ever downloaded, and nothing but the fake's own files is touched; it is the same on
  the VM and in safe mode, no handler being involved.

- **`standin toast <text>`** posts a notification to the running Dashboard, which lists it and shows its Toast: a
  `WM_COPYDATA` message to the Toast strip's window (`crates/model/src/post.rs`). The Dashboard takes it only when
  started with `--allow-toast-posts`; it exits 1 when no Dashboard took it. The Dashboard says
  `toasts: posted "<text>"`, then `toasts: shown "<text>" (Notification), <n> waiting`, `toasts: going` and
  `toasts: gone` as the Toast comes and goes, each with the time since it started and each once the frame that shows
  it has been presented.
- **`standin media [--title <t>] [--artist <a>] [--picture <file>] [--position <s>] [--length <s>] [--paused]
  [--no-previous] [--seconds <n>]`** is a media session of its own, as a music player's: a hidden window's System
  Media Transport Controls, which Windows lists among every app's under the program's name (`standin.exe`, which the
  Dashboard calls "Standin"), playing <t> by <a> with a picture (a green square drawn when none is given), 65 s into a
  track 200 s long unless told otherwise. It says `media: session up for <n> s`, then `media: pressed Play|Pause|Next|Previous`
  for each button Windows passes on, in `standin.log`, and answers as a player would (Next puts " (2)" after the
  title). The `now-playing` test chooses it as Settings' Now Playing source before it presses anything, so that on the
  TV PC nothing it presses reaches the user's own music; it ends only the stand-in it started.
- **`standin serve-art --port <n> --key <key>`** is a stand-in SteamGridDB on `127.0.0.1:<n>` until it is killed, for
  `dashboard --art-service http://127.0.0.1:<n>`: it takes only `<key>`, finds a game of any name, and gives each one
  a plain orange cover (`F08010`), teal hero, violet header and yellow square (`E0D020`). Under `/stores` it is the
  stores' own art online too (Steam's, the Microsoft Store's and GOG's), for `dashboard --store-art-service
  http://127.0.0.1:<n>/stores`: a game of any id has a red cover, blue hero, green header and magenta square
  (`D02090`). It says `serve-art: listening on <url>` and `serve-art: the stores' own art online at <url>/stores`,
  then `serve-art: <path> (no key|the key|another key)` a request in `standin.log`. No test asks a real service:
  `Start-Dashboard` gives every run `--store-art-service http://127.0.0.1:9`, which answers nothing, unless the test
  gives its own.

Every start shows the greeting's Toast, "Hi <user>!", for five seconds, low in the middle of the screen. A test that
looks at Home or taps the Guide button waits it out first: `Wait-Greeting`.

- **`standin open-address <address>`** is the stand-in for whatever handler the address's scheme has, for a Dashboard
  started with `--open-addresses-with standin.exe` (safe mode, below): `steam://` as `steam-url`, the Epic launcher's
  links as `epic-url`, with each fake's folder from `handlers.json` beside `standin.exe`, never from the registry.

`Initialize-FakeSteam` (`payloads/lib/E2E.psm1`) puts them in place for the signed-in user: the fake install at
`C:\e2e\steam`, `HKCU\Software\Valve\Steam\SteamPath` naming it, and `standin.exe steam-url "%1"` registered as the
`steam://` handler under `HKCU\Software\Classes\steam`. The Dashboard then finds Steam and launches through it exactly
as on a real machine. In safe mode none of that is registered (On the TV PC, below).

`Initialize-FakeEa`, `Initialize-FakeUbisoft` and `Initialize-FakeBattleNet` (each on `New-FakeStore`, as Epic's and
GOG's are) write a fake of that store at `C:\e2e\ea`, `C:\e2e\ubisoft` or `C:\e2e\battlenet` with
`standin make-<store> <root>`: made-up games, each installed one's `game.exe` the stand-in game, and the store's
registry as `registry.json` in `<root>`. The Dashboard reads it when started with `--<store> <root>`. An EA game starts
by its own `game.exe`; `standin.exe uplay-url <root> "%1"` is registered as the `uplay://` handler for the fake Ubisoft
Connect; the fake Battle.net's `Battle.net.exe` is a copy of `standin.exe` that starts the game it is asked to launch.
Each says what it started in `standin.log`. In safe mode the fakes are in the test's folder and `uplay://` is not
registered: `standin open-address` takes it, as it does Steam's and Epic's.

`Initialize-FakeXbox` writes a fake drive of Xbox games at `C:\e2e\xbox` with `standin make-xbox <root>`: a
`.GamingRoot` naming `XboxGames`, and under it made-up packages (three games, an app with no `MicrosoftGame.config`, a
game whose manifest is broken), each game's `game.exe` the stand-in game and its logos plain-colour pictures. The
Dashboard reads it when started with `--xbox <root>`. Nothing is registered by it.

## Writing a test

A test is a folder under `payloads/` with a `run.ps1` that imports `lib\E2E.psm1`, collects what is wrong in a list,
says `result: PASS` or `result: FAIL`, and exits 0 or 1; `payloads/smoke/run.ps1` is the model. The module's functions:

| | |
|---|---|
| `Say`, `Save-Screen`, `Wait-Until` | a line in `steps.txt`; the screen as a PNG; poll a condition |
| `Get-ScreenPixel`, `Get-DesignPixel`, `Test-Colour`, `Format-Colour` | the screen's colour at a pixel, or at a point of the Dashboard's 1920x1080 canvas; whether it is a given colour; a colour for `steps.txt` |
| `Get-Foreground`, `Set-Foreground`, `Close-WindowsPrompts` | the window in front; put one in front; Windows' own full-screen prompts closed |
| `Connect-Pad`, `Push-Button`, `Disconnect-Pad` | the virtual pad |
| `Start-Dashboard [-Arguments ...] [-Startup]`, `Stop-Dashboard` | the Dashboard, its stderr to `dashboard.log`; with `--no-startup` (straight to Home, no Boot video) unless `-Startup` |
| `Get-DashboardLog`, `Get-DashboardLogLength`, `Wait-DashboardLog <pattern> <seconds> [-After <lines>]` | what it logged |
| `Close-Dashboard [<seconds>]` | the Dashboard asked to close as `taskkill` without `/f` asks; whether it went |
| `Initialize-FakeSteam`, `Initialize-FakeEpic`, `Initialize-FakeGog`, `Get-StandInGame`, `Stop-StandInGame` | the fakes and their games (only those started from the test's own folders) |
| `Get-FakeRoot <store>`, `Get-TestFolder <name>`, `Get-DashboardDataDir` | where a fake, a folder of the test's own, and the Dashboard's data folder are, on the VM or in safe mode |
| `Test-SafeMode`, `Assert-VmOnly <why>`, `Set-RegistryValue`, `Get-OwnProcess <name>` | safe mode (below); the one registry write, which throws in it; processes started from the test's folders |
| `Initialize-FakeEa`, `Initialize-FakeUbisoft`, `Initialize-FakeBattleNet`, `Initialize-FakeXbox` | a fake EA app, Ubisoft Connect, Battle.net or drive of Xbox games |
| `Watch-Pulse <seconds>`, `Test-PulseColour`, `Test-Suspended <pid>` | the colours the stand-in game's pulse showed (two when it runs, one when it is frozen); whether a pixel is the pulse, dimmed or not; whether every thread of a process is suspended |

The tests: `smoke` (Home, a launch through the Launch screen, the Guide over the game, Home again, and the Dashboard
closing when asked as `taskkill` without `/f` asks), `home` (Home
walked with the pad), `needs-user` (a launcher that stays: the Launch screen steps aside for it), `game-hub` (A on a tile opens the game's
hub, its actions, cards and two pages by `--log-focus`, its Tile menu, Install, B back on the tile, and a double tap
playing the game), `guide-hold` (the
Guide on the Overlay at Home and over the game, on the monitor of the window in front; the game frozen while it is
open, thawed as it closes, and thawed at the next start after the Dashboard is killed with the Guide open), `pages`
(every way from Home into a page and B back from each), `my-games` (My games & apps on the full shelf: along its nav,
the grid's edges and its scroll, Play history, Manage, and B and Y), `search` (Y on Home, a name typed on the Console
keyboard with the pad, the symbols, the keyboard put away, A on a result opening its hub, and B back step by step),
`flyouts` (the Guide's Notifications and Audio Flyouts over a game; where the VM has an
the grid's edges and its scroll, Play history, Manage, and B and Y), `groups` (Settings' Groups: a Group hidden, two swapped and one added by
name, Home's Groups seen to follow each, and all put back), `profiles` (Settings' Profiles in a data folder of the
test's own: the first Profile made from the Windows user, one added by its gamertag, renamed, given a real name and a
plain picture, the Windows user's removed so the new one signs in, Home's top bar and the Guide seen to follow by the
`profile:` line, and the Dashboard started again with it still signed in), `apps` (Settings' Games & apps: a copy of the stand-in
game added as a game and Notepad as an App by typing their paths, the App started from My games & apps' Apps, the game
launched from its hub and first in the Recent row after, and both removed), `flyouts` (the Guide's Notifications and Audio Flyouts over a game; where the VM has an
output device it moves the real volume and mute and puts them back), `now-playing` (`standin media`'s session chosen
as Settings' Now Playing source, the Audio Flyout's card showing it, and Play/Pause and Next pressed on the card, each
seen to reach the stand-in and the card to follow its session), `toast` (the greeting, Toasts over the game and
the Guide, the queue, the Guide button during a Toast), `stores-ea-ubisoft-battlenet` (a fake of each beside Steam's:
each read, and one game of each found through Search and launched from its hub through its store), `stores-xbox`
(the fake drive's Tidewater Trials registered for the user as a package of loose files, with developer mode on for the
run; the machine read with no `--xbox`; the game found through Search, launched from its hub by
`shell:AppsFolder\<AUMID>`, run from the package's folder, and Home back; the package removed after. Where it cannot
be registered, or in safe mode, it says why and reads the fake drive with `--xbox` instead, and the launch is left for
the TV PC), `queue` (Install
from the hub and the Tile menu, the fake install moved along by `standin steam-install`: Queued and Installing...
tiles, the hub's Queue action, Downloads' count, Manage Queue from all three ways in (the hub, Downloads, Manage's
box) and B back to each with focus restored, its row following the bytes, View on it, an install started in the
client, one dropped, one done with Manage Queue up and focus going with it to Recently installed, and Uninstall seen
through), `mixed` (M4's run: every store's fake at once with an App and a game added by hand, each store's games on
Home, in Games a screenful at a time and in Full library, a Game hub of each store's, Search finding two stores' games
by one word, a Steam install through the Queue to Recently installed, and an Uninstall prompt declined; a picture at
each), `boot-video` (Start-up's Boot video: the spike's clip as Xbox Series X in a media folder of the test's own,
its first frame, its end and Home's first frame logged in that order, then Off set on Settings with the pad and the
Dashboard straight to Home at the next start; it needs `video-spike.exe`, so build `-p video-spike` too), `idle` (the
Idle screen with `--idle-after 3,6`: Dimmed then Showing things over Home and over the stand-in game, a picture of
each, and a press that wakes it seen to move no focus and start nothing; a stage woken early by someone else's input
on the TV PC is said in the steps and waited for again), and `overlay` and `video` for the spikes.

Every wait is timed by a stopwatch and by its own sleeps, never by the clock, which the VM's time sync can move by hours
mid-test; each line of `steps.txt` has the clock's time and then the seconds since the test began. The smoke test sets
`DASHBOARD_NEEDS_USER_MS`, so that a slow VM keeping the game's launcher up does not count as a window that needs the
user.

A test waits on the Dashboard's log lines rather than sleeping, and matches them exactly as the code prints them: when
a log line changes, so must the tests that wait for it.

A test never names `C:\e2e\...`, `%TEMP%` or the Dashboard's data folder itself, and never stops a process by its name
alone: it asks the module (`Get-FakeRoot`, `Get-TestFolder`, `Get-DashboardDataDir`, `Get-OwnProcess`), so that the same
`run.ps1` runs on the VM and on the TV PC. A folder passed to the Dashboard is put in quotes: Start-Process quotes
nothing, and on the TV PC the test's folder is under the user's profile.

## On the TV PC

The TV PC (`ssh htpc-win`: a real GPU at 3840x2160 and 120 Hz, real Steam, ViGEmBus installed) is someone's own
machine in daily use. The same payloads run there in **safe mode**, which changes nothing of it:

```sh
cargo build --release --target x86_64-pc-windows-gnullvm -j 3 -p app -p standin
e2e/htpc/run.sh smoke
```

`e2e/htpc/run.sh <test> [--exe <dashboard.exe>] [--standin <standin.exe>] [--timeout <minutes>] [--keep]` stages the
payload exactly as `e2e/run.sh` does (`e2e/stage.sh`), copies it to `%TEMP%\xsd-e2e\<test>` on `$HTPC_HOST`
(`htpc-win`), and runs it on the TV's desktop through a one-off interactive scheduled task (`XsdE2E`): `run.cmd`, then
`launcher.ps1`, which sets `E2E_SAFE=1` and `E2E_OUT=<folder>\out`, runs the payload's `run.ps1`, and writes
`out\exitcode.txt` last. The runner waits for that file (15 minutes at most), brings `out\` back to
`artifacts/e2e/htpc-<test>-<time>/` (`launcher.txt` and `console.txt` there are the launcher's and the payload's own
output), prints `steps.txt`, and exits with the test's code. Whatever happened, it then stops every process whose program
is under that folder (and the launcher and the payload's PowerShell), deletes the task, and deletes the folder
(`--keep` leaves the folder to look at). If a run is killed before it could clean up, the next refuses to start while
the task is there: `ssh htpc-win schtasks /delete /tn XsdE2E /f`.

**It does not start while the TV is in use** (exit 3, `refused.txt` saying why): when a full-screen window that is not
the desktop's is in front (a game, a film, the lock screen), or the Dashboard or Playnite is running. The check runs on
the TV's desktop, in the launcher.

What safe mode never does:

- **No registry write**, anywhere. Every write in `E2E.psm1` goes through `Set-RegistryValue`, which throws in safe mode,
  so a helper written later cannot make one by mistake. Nothing registers a `steam://` or any other handler, and
  Windows' prompts are left as they are (`Close-WindowsPrompts` only says one is up).
- **The machine's own library is never read.** The Dashboard is started with every store's flag (`$StoreFlags`): the
  fake the test made, else an empty folder, which a source reads as no games, never as "the machine's then". A store's
  address goes to `standin.exe open-address` (`dashboard --open-addresses-with <program>`) in place of the program
  Windows has for it; the stand-in finds each fake in `handlers.json` beside it. The stand-in Steam started this way
  writes no `Running` value: the Dashboard follows the game by its windows, as it does Epic's and GOG's.
- **No power action and never a restart**: the Dashboard always has `--dry-power`. (The TV PC dual-boots, and a restart
  would come back in the other system.)
- **Nothing installed** and nothing left: every fake (`<folder>\fakes\<store>`), the data folder (`<folder>\data`) and
  the output are in the test's folder, which goes after.
- **Only its own processes stopped**: a process is the test's when its program is under the test's folder; none is
  stopped by its name alone.
- The volume and mute are not touched (`flyouts` opens the Audio Flyout and the Output list, and moves nothing).
- **The user's own music is never pressed**: `now-playing` makes the stand-in's app the Now Playing source in a data
  folder of its own, and presses a control only while the card's last `now playing:` line names the stand-in's track.

What it does do: the Dashboard's sounds are heard (the tests that give it `--media` play `standin make-tones`' tones),
and the Dashboard, the stand-in games and a virtual pad come and go on the TV's screen for the test's length.

The pixel checks are at points of the 1920x1080 canvas, which `Get-DesignPixel` scales to the real screen (the middle
of the 2x2 a canvas pixel is at 3840x2160); the module is DPI-aware, and says in `steps.txt` if Windows showed it a
scaled screen anyway. Pictures are saved at 1920x1080, scaled down from the TV's. Every wait is an upper bound, so
waits sized for WARP only make a fast machine wait less.

What stays VM-only: the **real protocol handlers** (the registry's `steam://`, `com.epicgames.launcher://` and `uplay://`
pointing at the stand-ins, and the Dashboard handing addresses to Windows' shell), a **first run on a clean Windows** (no Steam, no
data folder, Windows' own first-sign-in prompts), the `overlay` spike (`Assert-VmOnly`: it opens Notepad and cannot
tell its own from the user's), and the volume and mute steps of `flyouts`.
