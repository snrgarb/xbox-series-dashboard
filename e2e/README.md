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

`standin.exe` (`crates/standin`) is five things, and has no console window in any of them:

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
  shows for that long before the game's.
- **The stand-in Steam**, `standin steam-url steam://rungameid/<appid>`: finds the game's folder from its app manifest
  under the `SteamPath` in the registry, starts its `game.exe`, and keeps `HKCU\Software\Valve\Steam\Apps\<appid>\Running`
  at 1 until it exits, as Steam does. For a game that isn't installed it does nothing and exits 1.

- **`standin toast <text>`** posts a notification to the running Dashboard, which lists it and shows its Toast: a
  `WM_COPYDATA` message to the Toast strip's window (`crates/model/src/post.rs`). The Dashboard takes it only when
  started with `--allow-toast-posts`; it exits 1 when no Dashboard took it. The Dashboard says
  `toasts: posted "<text>"`, then `toasts: shown "<text>" (Notification), <n> waiting`, `toasts: going` and
  `toasts: gone` as the Toast comes and goes, each with the time since it started and each once the frame that shows
  it has been presented.

Every start shows the greeting's Toast, "Hi <user>!", for five seconds, low in the middle of the screen. A test that
looks at Home or taps the Guide button waits it out first: `Wait-Greeting`.

`Initialize-FakeSteam` (`payloads/lib/E2E.psm1`) puts them in place for the signed-in user: the fake install at
`C:\e2e\steam`, `HKCU\Software\Valve\Steam\SteamPath` naming it, and `standin.exe steam-url "%1"` registered as the
`steam://` handler under `HKCU\Software\Classes\steam`. The Dashboard then finds Steam and launches through it exactly
as on a real machine.

## Writing a test

A test is a folder under `payloads/` with a `run.ps1` that imports `lib\E2E.psm1`, collects what is wrong in a list,
says `result: PASS` or `result: FAIL`, and exits 0 or 1; `payloads/smoke/run.ps1` is the model. The module's functions:

| | |
|---|---|
| `Say`, `Save-Screen`, `Wait-Until` | a line in `steps.txt`; the screen as a PNG; poll a condition |
| `Get-ScreenPixel`, `Get-DesignPixel`, `Test-Colour`, `Format-Colour` | the screen's colour at a pixel, or at a point of the Dashboard's 1920x1080 canvas; whether it is a given colour; a colour for `steps.txt` |
| `Get-Foreground`, `Set-Foreground`, `Close-WindowsPrompts` | the window in front; put one in front; Windows' own full-screen prompts closed |
| `Connect-Pad`, `Push-Button`, `Disconnect-Pad` | the virtual pad |
| `Start-Dashboard [-Arguments ...]`, `Stop-Dashboard` | the Dashboard, its stderr to `dashboard.log` |
| `Get-DashboardLog`, `Get-DashboardLogLength`, `Wait-DashboardLog <pattern> <seconds> [-After <lines>]` | what it logged |
| `Initialize-FakeSteam`, `Get-StandInGame`, `Stop-StandInGame` | the fake Steam and its games |
| `Watch-Pulse <seconds>`, `Test-PulseColour`, `Test-Suspended <pid>` | the colours the stand-in game's pulse showed (two when it runs, one when it is frozen); whether a pixel is the pulse, dimmed or not; whether every thread of a process is suspended |

The tests: `smoke` (Home, a launch through the Launch screen, the Guide over the game, Home again), `home` (Home
walked with the pad), `needs-user` (a launcher that stays: the Launch screen steps aside for it), `guide-hold` (the
Guide on the Overlay at Home and over the game, on the monitor of the window in front; the game frozen while it is
open, thawed as it closes, and thawed at the next start after the Dashboard is killed with the Guide open), `pages`
(every way from Home into a page and B back from each), `my-games` (My games & apps on the full shelf: along its nav,
the grid's edges and its scroll, Play history, Manage, and B and Y), `toast` (the greeting, Toasts over the game and
the Guide, the queue, the Guide button during a Toast), and `overlay` and `video` for the spikes.

Every wait is timed by a stopwatch and by its own sleeps, never by the clock, which the VM's time sync can move by hours
mid-test; each line of `steps.txt` has the clock's time and then the seconds since the test began. The smoke test sets
`DASHBOARD_NEEDS_USER_MS`, so that a slow VM keeping the game's launcher up does not count as a window that needs the
user.

A test waits on the Dashboard's log lines rather than sleeping, and matches them exactly as the code prints them: when
a log line changes, so must the tests that wait for it.
