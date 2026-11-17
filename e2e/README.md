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
- `boot.log`, `exitcode.txt`: the harness's own. On the VM, run.ps1's own output is in `run.out.log` and
  `run.err.log`, and `boot-error.txt` says so if run.ps1 was ended for running past the limit (the timeout less two
  minutes; exit code 124): out.zip goes back either way.

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
  the game's `game.exe`). Run as **`steam.exe --standin-client [--seconds <n>]`** with `.cef-enable-remote-debugging`
  in its folder (`make-steam` marks the folder `standin-client.txt`), it stays up as Steam's client does and opens a
  stand-in CEF debugger on a free port, written in `standin-cef-port.txt`: the Dashboard asks it Steam's own calls
  (#171: install into a library, pause, resume, cancel, uninstall with no prompt; docs/sources.md, "Steam's own
  calls") over it exactly as it asks the real one, and it does each to the fake's files (`steam: CEF: asked to pause
  <appid>: <appid> is Paused ...`). **`standin steam-workshop <root> <appid> <done>/<items>`** writes the fake's
  Workshop record for the app and fills its content folders to `<done>` (#188). Nothing is ever downloaded, and nothing but the fake's own files is touched; it is the same on
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
through), `guide-downloads` (#193: the Guide's Downloads Utility reached with the pad and A on it opening Manage
Queue, at Home and over the stand-in game with the Dashboard coming in front of it), `pad-battery` (#191: the pad's battery faked in Windows' place as a percentage, a level and nothing, each seen in the
log and pictured on Home and in the Guide, only the percentage drawn as a battery; the virtual pad's own, unfaked, the
controller glyph), `queue-steer` (#170, #171, #188: through the stand-in Steam's CEF debugger and Steam's own calls, an install paused,
resumed and cancelled from a queued game's menu in Manage Queue, its question declined once; a second install to the
end with its Workshop items coming in, Play offered and the Ready to play Toast meanwhile, the row reading Downloading
Workshop content until all are in; and an uninstall with no prompt of Steam's; the fake named with `--steam` on the
VM too, as only a named install is taken for the stand-in), `storage` (#179: the fake Steam given a second library by
`standin steam-library`, its client's CEF debugger on; Settings > Storage listing both with the drives, the second
chosen as Steam's default; the storage ring on My games & apps; Install asking "Where do you want to install X?" with
the second focused, and Steam's client asked to install into it; then the Dashboard started again on the same data
folder with the choice kept), `epic-legendary` (#175: standin.exe kept as legendary in the test's data folder, its records under
`--helper-home`; not signed in said and shown on Settings > Helpers; signed in with the stand-in's `legendary auth`
and the library listed; Zinnia Gate installed from its Game hub with legendary's own progress in the Queue, paused,
resumed and cancelled from Manage Queue, installed to the end with egl-sync's record, played through `legendary
launch`, and uninstalled from its Tile menu), `gog-gogdl` (#176: the same for GOG with the stand-in gogdl and the fake
GOG's Pewter Isles: signed in with the stand-in's `auth --code`; downloaded, paused, resumed and cancelled; to the
end with its set-up step, the stand-in script interpreter; played as its own program; uninstalled with its
`unins000.exe /VERYSILENT`), `mixed` (M4's run: every store's fake at once with an App and a game added by hand, each store's games on
Home, in Games a screenful at a time and in Full library, a Game hub of each store's, Search finding two stores' games
by one word, a Steam install through the Queue to Recently installed, and an Uninstall prompt declined; a picture at
each), `launchers` (Settings > Launchers on the fake Steam and Epic, docs/spec/launchers.md: Steam switched off with
the pad, its games gone from Home's Recent row and Search and the page's count line, then on again with them back in
their play order; a game added to the fake Steam and another's play time changed by `standin steam-change`, and Update
now bringing both in with its summary line), `refresh-art` (#190, on the fake Steam and `standin serve-art`: with no
square anywhere a Recent tile shows its cover whole, never cropped; a key typed in and SteamGridDB's yellow squares on
the Recent row; Settings > Art's Refresh all art and details asked and confirmed, the cache emptied, the page saying how
far it has got, the Dashboard killed part way and the refresh going on at the next start until the cache is full again
and the squares are back; then one game's Refresh art from the Tile menu's More options), `art-picker` (#213, on the fake Steam and `standin serve-art --candidates 60`: a key typed in, the Tile menu's Change art, Right past the grid's edge to the Square tab's last page and a candidate chosen there, the stand-in asked for the square list's pages 0 and 1 once each and in order; a candidate on Background's third page; both still in use after Refresh all art and details and after the game's Refresh art; docs/spec/art-picker.md), `boot-video` (Start-up's Boot video: the spike's clip as Xbox Series X in a media folder of the test's own,
its first frame, its end and Home's first frame logged in that order, then Off set on Settings with the pad and the
Dashboard straight to Who's playing today? at the next start; it needs `video-spike.exe`, so build `-p video-spike` too),
`whos-playing` (Start-up with the Boot video Off in folders of the test's own: a second Profile added, Who's playing
today? at the next start and the other Profile picked, Signing you in, Home and its greeting; the Guide's Switch profile
back to the first; a wake from sleep pretended by `standin wake`; then Sign in the last profile set, a wake asking
nothing, and the next start straight to Signing you in), `idle` (the
Idle screen with `--idle-after 3,6`: Dimmed then Showing things over Home and over the stand-in game, a picture of
each, and a press that wakes it seen to move no focus and start nothing; a stage woken early by someone else's input
on the TV PC is said in the steps and waited for again), `launch-backdrop` (the desktop never seen round a game: the
stand-in game slow to start with a launcher and a gap, windowed, quit from the Guide, crashing and handing off to
another process, the screen sampled throughout by the window at each point and its colour; docs/spec/launch-screen.md),
and `overlay` and `video` for the spikes.

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

## Shell mode

Two payloads (docs/spec/shell-mode.md):

- **`shell-dry`**, safe anywhere, the TV PC included: `dashboard.exe --shell-on --dry-shell --as-installed` and
  `--shell-off --dry-shell`. They read the PC for real and write nothing. It passes when the lines say a dry run, the
  four writes in order (or each refusal, where the PC refuses, as the TV with the Bundle's Boot shell would), the
  `Shell` line exactly, no line says it wrote, and `reg query` and `schtasks /Query` read every place the same before
  and after. Then it starts `dashboard.exe --from-shell --dry-shell` (the window as the shell starts it, topmost for
  Start-up, the shell's events left alone) and needs `first frame presented` in `<data>\shell\logs\dashboard-*.log`
  with no `window: ERROR`: run it before `shell-mode`. Last, a plain start (ShellExecute, as Explorer starts it) must
  show the Dashboard's window and no console window (conhost's or Windows Terminal's). It ends within 170 s whatever happens: every process it starts
  has its output in files of its own, is waited on with a limit, and is ended with its whole tree, by pid. `e2e/run.sh shell-dry`, or `e2e/htpc/run.sh shell-dry`.
- **`shell-mode`**, the VM only: it turns shell mode on for real and restarts the VM five times. In safe mode it
  throws at once (`Assert-VmOnly`). It needs longer than the default wait:

  ```sh
  cargo build --release --target x86_64-pc-windows-gnullvm -j 3
  e2e/run.sh shell-mode --timeout 3600            # add --keep to look at the VM in the Proxmox console after
  ```

  It runs in six phases, one per sign-in. Each phase keeps its state in `C:\e2e\shell-mode\state.json`, registers the
  next as a logon task (`xbox-e2e-shell-next`, interactive, this user, highest) and restarts with `shutdown /r`; it never
  returns before the restart, so `boot.ps1` sends nothing back early. The last phase (or the first error after phase 1)
  turns shell mode off, copies `%LOCALAPPDATA%\XboxSeriesDashboard\shell\logs` to `shell-logs\`, and sends the output
  itself to `$env:E2E_UPLOAD`, which `vm.sh`'s `boot.ps1` now sets. A restart signs in again by the template's autologon.

  | Phase | Signed in with | What it proves (research section 9) |
  |---|---|---|
  | 1 | explorer | Copies `dashboard.exe` to the installed path. P2: a Bundle sentinel, another HKCU `Shell`, HKLM `Shell` = cmd.exe, `OverrideShellProgram` = 1 (the payload runs elevated, and puts each back) and the payload's own copy are each refused, exit 3, nothing written. P1: the dry run says the four writes and writes nothing; then on for real, read back value by value, the record there, HKLM unchanged, on twice says on already. |
  | 2 | `--shell` | P3: the shell log's black up, `ShellDesktopSwitchEvent`, events, the start, FirstFrame, black down, BootVideoEnded, explorer started; the shell window is that explorer's; no File Explorer window; the sentinel did nothing. P4: Exit to desktop from the Guide's Power (exit 0, stays down, a desktop), Guide brings it back, View+Menu held 6.5 s closes it. P5: the test's crash marker at 3: three starts, three crashes, the fallback line, the desktop, nothing more; Guide brings it back. P6b: `--shell` killed after explorer changes nothing. P7: deletes HKCU `Shell`. |
  | 3 | explorer | P7: the sentinel saw no `--shell`, wrote `Shell` back, started the Dashboard; no `--shell` log this sign-in. |
  | 4 | `--shell` | P6a: kills `--shell` as soon as the logon task runs. If explorer wasn't up yet: the safety net's line within 150 s, then the sentinel's. If it was, a `note:` line says P6a wasn't reached. P8: renames the running `dashboard.exe` aside and copies a new one in. |
  | 5 | `--shell` | P8: the copy runs as `--shell`; the old file can be deleted. P9: off for real, each place as phase 1 read it before, the record renamed, off twice says off already. |
  | 6 | explorer | P9: no `--shell` this sign-in, explorer is the shell, no Dashboard runs. |

  What to check by eye in the output: `p3-signed-in.png` (Start-up over everything, no taskbar), `p5-fallback-line.png`
  (the line on black), `p4-desktop.png`, `p9-desktop.png`; and `shell-logs\shell-*.log` read through once, a line a
  thing done.

  Not proved here: P10 (the uninstaller is #155's) and P8's handover and rollback (the updater is #156's); a Game Pass
  launch; the real black gap and cold-boot time (WARP); the audio device settling on HDMI; F6, a `Shell` naming a
  missing exe (try it with `--keep` and look in the console).

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

**It does not start while the TV is in use** (exit 3, `refused.txt` saying why): when a game Steam started is running,
a full-screen window that is neither the desktop's nor the live Dashboard's is in front (a game, a film, the lock
screen), the live Dashboard is in front of another program's full-screen window (its Guide over a game), the installed
Dashboard runs but not as the shell, or another Dashboard or Playnite is running. The check runs on the TV's desktop,
in the launcher, and `launcher.txt` logs each thing it looked at.

**The live Dashboard as the shell is not the TV in use** (`e2e/htpc/shell.ps1`). The live shell is only what runs from
the installed copy, `%LOCALAPPDATA%\Programs\XboxSeriesDashboard\dashboard.exe` (the watchdog `--shell` and its
`--from-shell` Dashboard), judged by the program's path, so a payload's own Dashboard is never taken for it. With
nothing else going on the launcher stops it, the watchdog first so it starts nothing again, and writes
`shell-stopped.txt` in the test's folder; after the payload, whatever it did, it starts `dashboard.exe --shell` again in
the signed-in session through a one-off task `XsdE2EShell` (deleted once the watchdog is seen running), and removes the
file. If the launcher was ended first (a timeout, a run stopped from Linux), `cleanup.ps1` finds the file and starts
the shell the same way; its lines (`shell: ...`) are in the runner's output. The Dashboard comes back through Start-up,
as at sign-in.

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

## The installer

**`install`** (docs/release.md, "Testing a package"), safe on the TV PC: it installs CI's
`XboxSeriesDashboard-<version>-setup.exe` silently into a folder of the test's own (`/S /D=<folder>`, never
`%LOCALAPPDATA%\Programs`), runs the installed `--version` and an offscreen frame of Settings > About, uninstalls
with `/DRYSHELL` (the uninstaller's `--shell-off` as a dry run) and `/DATADIR=` (a data folder of the test's own), and
checks what is left; then installs again and uninstalls with `/REMOVEDATA`. The installer's own writes, its uninstall
entry under HKCU and its Start menu shortcut, are the one exception to safe mode's no-registry rule: the test refuses
to start where either is there already, and checks both are gone after. Stage the installer beside `dashboard.exe`:

```sh
gh run download <run id> -n windows -D target/release-test
gh run download <run id> -n XboxSeriesDashboard-<version> -D target/release-test
e2e/htpc/run.sh install --exe target/release-test/dashboard.exe --standin target/release-test/standin.exe
```

## Helpers

**`helpers`** (docs/spec/helpers.md, 5), safe on the TV PC. `standin serve-helpers` on 127.0.0.1 serves standin.exe
and writes a helpers' table pointing at itself, which the Dashboard is given with `--helper-table`, on a data folder
of the test's own. Walked by the virtual pad to Settings > Helpers: Add legendary asks, Yes fetches it, checks its
SHA-256, keeps it as `<data>\helpers\legendary\legendary.exe` and runs it once with `--version` through the shell's
runner (standin.exe, named so, is the stand-in helper), its lines logged; gogdl's address serves other bytes and is
refused with nothing kept; legendary's Remove deletes its folder. Judged by the focus lines, the `helpers:` lines and
`standin.log` (the one request, plain). Nothing is asked of the projects' real release pages. It needs the Windows
build's two programs:

```sh
cargo build --release --target x86_64-pc-windows-gnullvm -j 3 -p app -p standin
e2e/htpc/run.sh helpers
```

## Updates

**`updates`** (docs/spec/updates.md, 7), safe on the TV PC. A copy of the staged `dashboard.exe` in a folder of the
test's own, started with `--pretend-version 0.0.1` and `--update-source` at `standin serve-updates` (on 127.0.0.1,
serving the staged build as a release of its real version), is driven by the virtual pad to Settings > About: Check
for updates finds, downloads and checks the release, Install puts it in place by renaming, the new version starts with
`--handover` and the old one exits 0, kept as `dashboard.0.0.1.old`. Then a fresh copy against `serve-updates
--bad-hash` sees the download refused and its folder unchanged. Judged by the copies' logs (`update-app.log`,
`update-bad.log`) and `standin.log`. Nothing asks GitHub, and the user's installed copy is never touched. It needs
only the Windows build's two programs:

```sh
cargo build --release --target x86_64-pc-windows-gnullvm -j 3 -p app -p standin
e2e/htpc/run.sh updates
```
