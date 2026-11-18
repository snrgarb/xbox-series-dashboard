# Updates

The Dashboard finds a newer version of itself, downloads it, checks it and puts it in place (#156). The Bundle had
no updater of its own (Playnite updated itself and its add-ons); this is the port's own, on the plan of
`docs/research/shell.md`, 4.6 and 8, and the release `docs/release.md` makes. The code: `crates/update` (asking,
checking, renaming; none of it needs Windows), `crates/app/src/updates.rs` (the threads, the handover, the Toast),
`crates/shell-mode` (`--handover`'s guard and event), Settings > About (`crates/screens/src/settings/about.rs`).

## 1. Where updates come from

**One address, in one place**: `update::SOURCE`, GitHub Releases' API for the repository's newest release,
`https://api.github.com/repos/snrgarb/xbox-series-dashboard/releases/latest`. A test gives its stand-in's in its
place with `--update-source <url>` (`standin serve-updates`, section 7); nothing else names an address.

From the answer the Dashboard reads `tag_name` (`v` and the version, `docs/release.md`) and, among `assets` by name,
the portable zip of that version, `XboxSeriesDashboard-<version>-portable.zip`, and `SHA256SUMS.txt`, the manifest
`tools/package.sh` makes. A release without either is not one it can install ("the update source's answer has no
...", Failed).

**What is sent**: a plain unauthenticated GET to the address, then to the two files' `browser_download_url`s (which
GitHub redirects to its file host). No token, no cookie, nothing of the user's, the machine's or the running
version. The User-Agent is `dashboard-updates`, because GitHub's API refuses a request with none; it names no user,
machine or version. No telemetry, nothing posted.

**While the repository is private** the address answers 404 to a request with no token. That is said plainly on
About, "Updates can't be checked yet: the Dashboard's releases aren't public.", and logged as
`update: the update source answered 404: the releases aren't public`. It is not an error: no Toast, and the daily
check stays off by default for as long as it is so.

## 2. Settings > About

System > About (settings.md 16.10) gets a group **Updates** between the version and the licences:

- a line saying where updates stand (below);
- two buttons, **Check for updates** and **Install** beside it (18 apart, 58 tall, as Launchers' Update now). Install
  takes focus always and is dimmed, doing nothing, unless a version is ready and no game runs. Check for updates is
  dimmed while a check or an install is under way, and the Spinner turns beside Install;
- the checkbox row **Check for updates every day**, off by default, with no line under it ("Once a day while the
  Dashboard runs, ..." went in #261, settings.md 16.11): Third-party licences is 40 under it

Focus opens on Check for updates. Right goes to Install, Left back (and Left from Check for updates or the checkbox,
or B anywhere, back to System's boxes on About); Down from either button to the checkbox, Up back to Check for
updates. A on Check for updates: `Effect::CheckForUpdates`; on Install with one ready: `Effect::InstallUpdate`; on
the checkbox: the option turned over (`Setting::DailyUpdateCheck`, `updates.daily_check`), saved as every option is.

The focus line: `About: Version 0.1.0 (0123abc); <control>; <state>`, the state as `Updates::state` says it.

| State (`model::update::Updates`) | `state` | The line |
|---|---|---|
| Unchecked | `unchecked` | "Check to see whether there's a newer version." |
| Checking | `checking` | "Checking for updates..." |
| UpToDate(v) | `up-to-date <v>` | "This is the latest version." |
| NotPublic | `not-public` | "Updates can't be checked yet: the Dashboard's releases aren't public." |
| Failed | `failed` | "Couldn't check for updates. Is the PC online? The Dashboard's log says why." |
| Downloading(v) | `downloading <v>` | "Version <v> is available. Downloading it..." |
| Ready(v) | `ready <v>` | "Version <v> is ready to install. The Dashboard restarts with it."; while a game runs "Version <v> is ready. Quit the game to install it." |
| BadDownload(v) | `bad-download <v>` | "Version <v> didn't match its release's checksum, so it wasn't installed." |
| Installing(v) | `installing <v>` | "Installing version <v>..." |
| RolledBack(v) | `rolled-back <v>` | "Version <v> didn't start, so this version was kept." |
| InstallFailed(v) | `install-failed <v>` | "Version <v> couldn't be put in place, so nothing was changed. The Dashboard's log says why." |

A check finds a newer version and goes straight on to download and check it: "the version found" is the one in the
line, and Install is lit once it is Ready. A version no newer than the running one is UpToDate.

## 3. The daily check

With **Check for updates every day** on, the Dashboard looks once a minute, starting a minute after it starts,
whether a day has passed since the last check (by hand or its own; the settings row `updates.last_check`, seconds
since 1970; never checked, or a clock that went back, is due), and checks then. Off by default while the
repository is private. It runs during a game as well: it only downloads.

## 4. Download and check

1. The manifest (`SHA256SUMS.txt`, at most 1 MB), then the zip (at most 200 MB).
2. The zip's SHA-256 against the manifest's line for its name. Another hash: **refused**, BadDownload, nothing
   written, `update: refused <v>: its zip's SHA-256 is <got>, not the manifest's <want>; nothing was changed`.
3. `dashboard.exe` out of the zip (stored or deflated; its CRC checked), written beside the running exe as
   `dashboard.exe.new`. Ready. `update: <v>'s zip matches its manifest; dashboard.exe (SHA-256 <hash>) staged as ...`.

The update replaces the exe the Dashboard runs from, wherever it is: the installed copy, a portable folder, a
test's copy. The README, the licences and the uninstaller are not touched; the uninstall entry's `DisplayVersion`
keeps the installed version until the next install (a known gap, said in section 9).

## 5. Install: the handover

Install while a game runs does nothing (the line says to quit it). Otherwise, on a thread of its own:

1. **In place, by renaming.** `dashboard.exe` is renamed `dashboard.<running version>.old` and `dashboard.exe.new`
   renamed `dashboard.exe`: two renames in one folder, each whole, the first undone if the second fails
   (InstallFailed). A running exe can be renamed but never written over, so this works while the Dashboard runs
   from it, and while the `--shell` process does (research 4.6). Nothing is written over a running file.
2. **The new version started**: `dashboard.exe` with this Dashboard's own arguments but `--pretend-version`, then
   `--handover <this pid>`, in no job and with no handles, allowed to take the foreground.
3. **The old one waits** up to 30 s for the new one's first frame: the new one sets the event
   `Local\XboxSeriesDashboard.v1.Handover.<old pid>` as its first frame is presented.
   - It comes: the old one logs `update: handed over ...; exiting 0` and exits 0. **The old file is kept until the
     new one has started once.**
   - The new one exits first, or no frame in 30 s (it is ended): **put back**. `dashboard.exe` is renamed
     `dashboard.<new>.bad`, `dashboard.<old>.old` back to `dashboard.exe`; RolledBack, and the old one carries on.

The new Dashboard under `--handover <pid>`:

- takes the single-instance guard alongside the old one's, rather than giving way to it (`shell_mode::dashboard::
  taking_over`), and logs `update: taking over from pid <pid>, an update's earlier version, at <ms> ms` and, at its first frame,
  `update: first frame at <ms> ms; told pid <pid>: true` (the old one logs `started <new> (pid <pid>) at <ms> ms`
  and `has shown its first frame: this one exits (<ms> ms after its start)`, so both sides' timings are in the log);
- appends to the same `--log-file` as the old one, which shares it for writing; every line reaches the file in one
  write, newline and all (`crates/log-line`), so the two processes' lines never tear each other; a `--log-file` that can't be
  opened sends the log to the data folder's `logs\` instead;
- has no Start-up (as `--no-startup`): the user just had one;
- removes nothing as it starts (the old one is still running from its file).

**Removing what was kept.** Every Dashboard start that is not a handover removes `dashboard.*.old` and a left
`dashboard.exe.new` beside its exe where it can: one still running can't be removed and is left for a later start.

### 5.1 While the Dashboard is the shell

The handover is the same, and the watchdog does not fight it (`crates/shell-mode/src/watch.rs`):

- it watches every `dashboard.exe` at its path by image path, so the new one, which it did not start, is watched
  from its first window on;
- the old one's exit 0 with the new one left is "a handover", not Exit to desktop and not a crash;
- a new one that crashes while the old one runs is counted as a crash but relaunches nothing (a Dashboard is left);
  the old one puts the files back;
- the `--shell` process keeps running from its file, renamed `dashboard.<old>.old`, until sign-out: that file can't
  be removed until then, and is removed by the first Dashboard start of the next sign-in. Its relaunches start the
  file beside it under its own name, `dashboard.exe`: the new version. The contract between the two
  (`crates/shell-mode/src/contract.rs`, the event names' `v1`) is what lets the old `--shell` and the new Dashboard
  work together for the rest of the session.
- `Shell`, the sentinel's Run entry and the safety net name the path, which an update never changes.

## 6. The Toast

When a version becomes Ready (by hand or by the daily check), a notification: "Version <v> of the Dashboard is
ready. Install it in Settings > System > About.", a Toast with it, once per version. **Never during a game**: while one runs it
waits, and shows once the game has ended. Nothing else about updates makes a Toast: not a 404, not a failure, not a
refusal; those are on About and in the log.

## 7. Tests

- `cargo test -p update`: the release read from GitHub's JSON, the hash check (a bad one refused), the zip read
  (stored, deflated, damaged), the renames and the putting back, removing what was kept, the daily check's day,
  and a check and download against the stand-in, whose requests carry the plain User-Agent and no Authorization.
- `crates/screens/tests/settings.rs`: About's walk and asks, and pictures of its states (`settings-about`,
  `-ready`, `-not-public`, `-bad-download`, `-checking`).
- `standin serve-updates --exe <dashboard.exe> --version <x.y.z> [--port <n>] [--bad-hash] [--not-public]`: the
  stand-in update address (`update::fake`), serving a release of that version around that exe.
- `--pretend-version <x.y.z>`: the Dashboard takes itself for that version (About, the update's comparison, the
  `.old` file's name), so one build can be updated to itself as a newer version. For tests only.
- `e2e/payloads/updates` (the TV PC, safe mode): updates a copy in the test's folder from the stand-in, sees the new
  version start and the old one exit 0, then sees a bad hash refused with nothing changed (e2e/README.md).

## 8. Log lines

Every line starts `update: `: `checking <address> (<why>); this is <v>`, the state as it changes (`update:
checking`, `downloading 0.2.0`, `ready 0.2.0`, ...), the steps of sections 4 and 5, and as a Dashboard starts
`removed <file>, kept by an update`.

## 9. Not here

- Publishing a release, and making the repository public: the user's (`docs/release.md`).
- Signing: an unsigned new exe is as the old one was. Whether SmartScreen or an antivirus looks at an exe renamed
  into place and started is unverified.
- The uninstall entry's `DisplayVersion` is not changed by an update: Settings > Apps shows the version installed
  until the next install. Writing it is one registry value, left for when updates are public.
- Updating the README and the licences file beside the exe: only `dashboard.exe` is replaced.
