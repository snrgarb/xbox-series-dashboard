# The Dashboard as the PC's shell, and how the Boot shell's job comes over

Research for [#153](https://github.com/snrgarb/xbox-series-dashboard/issues/153). It shapes #154 (shell mode), #155 (packaging) and #156 (updates). Researched 2026-11-17, on Linux. **Nothing was run on any machine and nothing was changed on one**: no VM, no TV PC. Everything here is what the Bundle's own research, code and TV runs say, what this repository's code does today, and Microsoft's documentation.

Each claim carries a tag:

- **[Bundle research]**: the Bundle's `docs/research/` at [`f11e4f9`][b]: [per-user-shell.md][pus], [boot-shell-updates.md][bsu], [game-pass-without-explorer.md][gp], [xbox-mode-home-app.md][xm], [os-boot-checklist.md][obc], [playnite-cold-start.md][pcs], [guide-button-capture.md][gbc]. Their own claims carry their own confidence, which is kept here.
- **[Bundle code]**: the Boot shell's sources at [`bootshell/XboxSeries.BootShell/`][bs] and its README, at `f11e4f9`.
- **[TV, #107]**: the Bundle's prototype runs on the TV PC, 2026-10-22 ([xbox-series-theme#107][i107]), and the decisions after them ([#104][i104], [#112][i112]). These are the only facts in this note seen on a real machine.
- **[here]**: this repository's code and docs at `44974ac`.
- **[MS]**: Microsoft Learn, read 2026-11-17.
- **[inference]**: my reasoning from the above. Nobody has tested it.
- **[unverified]**: a fact nobody has seen on a machine; a test in section 9 is written for it.

[b]: https://github.com/snrgarb/xbox-series-theme/tree/f11e4f9
[pus]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/research/per-user-shell.md
[bsu]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/research/boot-shell-updates.md
[gp]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/research/game-pass-without-explorer.md
[xm]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/research/xbox-mode-home-app.md
[obc]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/research/os-boot-checklist.md
[pcs]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/research/playnite-cold-start.md
[gbc]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/research/guide-button-capture.md
[bs]: https://github.com/snrgarb/xbox-series-theme/tree/f11e4f9/bootshell/XboxSeries.BootShell
[i107]: https://github.com/snrgarb/xbox-series-theme/issues/107
[i104]: https://github.com/snrgarb/xbox-series-theme/issues/104
[i112]: https://github.com/snrgarb/xbox-series-theme/issues/112
[adr]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/adr/0002-boot-shell-not-xbox-mode.md

## Answer

**Fold the Boot shell into the Dashboard's executable, but not into the Dashboard's process.** One file, `dashboard.exe`, run two ways:

- `dashboard.exe --shell` is what the HKCU Winlogon `Shell` value names. It is the Boot shell's job in Rust: a black window first, the Dashboard started behind it, explorer started behind Start-up, the watchdog, the pad while no Dashboard runs, the sentinel and the safety net. It never touches the GPU, the library or the stores.
- `dashboard.exe` with no `--shell` is the Dashboard, as today. Under shell mode the `--shell` process starts it, and it tells that process two moments through named events, as Start-up tells the Boot shell today.

A process can't be its own watchdog, and the Guide button has to bring the Dashboard back after Exit to desktop, when no Dashboard is running. Both need a small resident process that isn't the Dashboard. One exe keeps install, update and uninstall to one file, one version and one path for the `Shell` value. The update is a rename, which Windows allows on a running exe, and not a replace. Section 3 has the reasons and section 4 has the moving parts.

The rest, in short:

- **The Dashboard already does the Boot video, Start-up's sign-in (Who's playing today?), Power and Exit to desktop.** It sets no events, has no black window, starts no explorer, has no single-instance guard, and its window isn't topmost during Start-up. Section 1 says what has to be added, on which side.
- **A PC must never be left with no shell.** The defences are the Bundle's, and all of them came through a TV run or its unit tests: `Shell` written last and taken off first, explorer every boot, the watchdog's fall back to the desktop, a sentinel in HKCU Run, and a logon task that doesn't depend on the exe. Section 6 lists each failure and how to recover from it. Two are new: the Bundle's Boot shell still installed beside ours, and an uninstall or update that leaves `Shell` naming an exe that isn't there.
- **Game Pass and Store games.** On the TV, package activation worked with no shell at all, but `explorer.exe shell:AppsFolder\…` did not [TV, #107]. The port opens `shell:AppsFolder\…` through `ShellExecuteW` in its own process [here], which nobody has tried with no shell. Explorer starts every boot, behind Start-up, so this only matters in the seconds before it is up, or after it has died. The Xbox source should activate through `IApplicationActivationManager::ActivateApplication` (section 5).
- **The Xbox mode home-app route is left as a documented spike. M7 doesn't build it.** It needs a signed MSIX with the `gamingHome` custom capability, Developer Mode and a certificate trusted machine-wide (admin, once), and a handheld or the `DeviceForm` fake that ADR 0002 rules out. It hands the Guide button to Game Bar. A packaged build can't offer shell mode either, because MSIX virtualises HKCU writes by default (section 7).
- **Needs a human:** turning shell mode on on the TV PC; taking the Bundle's Boot shell off it first, if it's installed there (not checked: the TV PC is off limits); judging the black gap and cold-boot timing on real hardware; a Game Pass launch; a signing certificate; and anything about Xbox mode (section 10).

## 1. What the Boot shell does today, and what the Dashboard already does

The Boot shell is 3,150 lines of C#, published NativeAOT [Bundle code; standalone-rust-xbox-mode.md §2]. Step by step, at a sign-in [Bundle code: `Program.cs`, README]:

| # | Step | How the Boot shell does it | The Dashboard today [here] |
|---|---|---|---|
| 1 | **Black first.** A topmost, borderless, cursorless black window over the screen before anything else. Then `SetEvent(OpenEvent("ShellDesktopSwitchEvent"))`, so Winlogon's Welcome screen gives way straight to the black. | `BlackWindow.Show`, `SwitchDesktop`. On the TV it was up 235–274 ms after the process started at a cold sign-in, with no desktop flash [TV, #107]. | **No.** Its window is winit's, made fullscreen-borderless (`window.rs` `Fullscreen::Borderless`), not topmost, and its first frame comes 0.37–0.43 s after the process starts, warm [plan.md]. It never sets `ShellDesktopSwitchEvent`. |
| 2 | **Two named events**, `Local\XboxSeries.BootShell.FirstFrame` and `…BootVideoEnded`, made unset, which the Bundle's Start-up sets. | `MakeEvent`, `shared/BootShellEvents.cs` | **The moments exist, the events don't.** `crates/app/src/startup.rs` logs `startup: first video frame at …` and the video's end, skip, failure or 20 s stop (start-up.md 11). Nothing is signalled outside the process. |
| 3 | **Wait for the audio device.** Fullscreen is started only once Windows' default render device has stayed the same for 1.5 s, or 6 s at most. Playnite's SDL opens audio once, and is silent for the whole run if Windows is still choosing the TV's HDMI device. | `AudioSettle`, `WaitForAudio` | **Half.** The Dashboard's sounds open the default device afresh after it changes (`audio::engine`: "the next sound tries the default device again"). The Boot video's sound is Media Foundation's (`crates/media`), opened once as the video starts, and nobody knows what it does if the device changes under it **[unverified]**. |
| 4 | **Start the program behind the black**: `Playnite.FullscreenApp.exe --hidesplashscreen`, by `CreateProcess`, no job, no handles inherited, so it outlives the Boot shell. | `Launcher.StartFullscreen` | n/a: under shell mode this becomes "start `dashboard.exe`". |
| 5 | **Black down** at FirstFrame, or 60 s after the launch at most. | `Watchdog.Black`, `ShowBlack` | n/a |
| 6 | **Explorer**, once per sign-in and only while `GetShellWindow()` is null: at BootVideoEnded, or 20 s after FirstFrame without it, or at the 60 s limit. A plain `C:\Windows\explorer.exe` with no arguments then becomes the full shell, because HKLM `Shell` still says `explorer.exe`. | `SignInSchedule`, `Launcher.StartExplorer`; [Bundle research: per-user-shell §1]; seen working [TV, #107] | **No.** The Dashboard never starts explorer. |
| 7 | **Steam, Discord and Spotify** straight after explorer, from command lines compiled into the exe, with their own Startup-apps entries switched off so explorer doesn't start them again. | `Launcher.SteamDiscordSpotify`; [#104][i104] | **No**, and section 4.4 recommends not carrying this over. |
| 8 | **The watchdog.** Every 100 ms it looks at Playnite's processes in the session by name, never at its own child, because Playnite restarts itself for updates and mode switches. When the last one exits: exit 0 with nothing left is a quit the user meant, and it stays down. Any other code, or any exit before that start's first frame, is a crash: it deletes `safestart.flag` and relaunches at once. The third crash in 5 minutes shows a line on the black for 5 s, then starts the desktop and relaunches nothing more on its own. The black is raised again for each relaunch, and lifted at once when Playnite's own crash window shows. | `Watchdog`, `PlayniteProcesses`, `PlayniteLog` | **No.** |
| 9 | **Explorer after it dies.** Windows usually restarts explorer itself. If no shell window has existed for 10 s, the Boot shell starts it, then waits 10 s more before trying again. | `ShellWatch` | **No.** |
| 10 | **The pad**, read by its own thread every 30 ms through `XInputGetStateEx`. While no Fullscreen runs, the Guide button starts it. View+Menu held 5 s, at any time, sends `WM_CLOSE` to Playnite and ends it 10 s later, then starts the desktop. | `XInputPad`, `PadWatch` | **The Dashboard reads the pad the same way** (`crates/input`, `XInputGetStateEx`, background-safe [HANDOVER.md]), but only while it runs. Nothing reads it once Exit to desktop has closed the Dashboard. |
| 11 | **The mutex `Local\XboxSeries.BootShell`**, held until sign-out, which the sentinel looks for. | `Program.Main` | n/a |
| 12 | **The end of the session.** When Windows says the session is ending (`WM_QUERYENDSESSION` on the black window), watching stops, so Playnite closing under a shutdown isn't taken for a crash. Watching starts again if the end is called off. | `BlackWindow.SessionEnding` | n/a. Power's Turn off and Restart run `shutdown.exe` (`shell::power`), so this matters as soon as there is a watchdog. |
| 13 | **A log per sign-in**, `%LOCALAPPDATA%\XboxSeries.BootShell\logs\<stamp>.log`, keeping the newest 30. | `SignInLog` | The Dashboard logs to stderr. |
| 14 | **A last-resort catch**: any unhandled error takes the black down and starts the desktop. | `Program.Main`'s `catch` | n/a |

**Outside a sign-in** [Bundle code: `Setup.cs`, `Sentinel.cs`, README]:

- `--install` copies the exe to `%LOCALAPPDATA%\XboxSeries.BootShell\`. It then writes, recording each value as it was before in `setup-record.json`:
  - the safety-net logon task, first;
  - Startup-apps switch-offs, Teams' startup task, the Startup-folder shortcuts moved aside, SCOOBE off;
  - Playnite's settings;
  - the sentinel's HKCU Run entry;
  - `Shell`, last.
- `--uninstall` undoes them in reverse, but takes the sentinel off **before** `Shell`. A value that changed since setup is left alone and said.
- **The sentinel** (`--sentinel`) is started by explorer's startup run. If no Boot shell holds the mutex, explorer was the shell from the start, which happens when a full Windows upgrade drops the HKCU value [Bundle research: boot-shell-updates §1]. It then writes `Shell` back and starts Fullscreen. Seen working on the TV: run R5 [TV, #107].
- **The safety net** is a logon task 90 s after sign-in, at normal rights. It runs `conhost.exe --headless powershell.exe` and never the Boot shell's exe, and starts explorer if none runs in the session. Registering it needed no admin on the TV PC (2026-10-23) [Bundle code: README].

**What carries over unchanged:** the order of the setup writes, the record, the sentinel, the safety net, explorer's moment, the crash rule (3 in 5 min), the 60 s black limit and the pad's two gestures. **What doesn't:** everything Playnite-specific (`safestart.flag`, `update.exe`, `PlayniteInstaceMutex`, Playnite's crash window, its settings and `Playnite.lnk`). That leaves the port's version of the job much smaller than the Bundle's [inference].

**Exit to desktop today** [here: `crates/app/src/window.rs` `Effect::Exit`; guide.md 13]: the Dashboard plays `PowerOff.wav` to its end (4 s at most), then the event loop exits and the process ends with code 0. It starts nothing. On a PC where explorer is the shell, that is the desktop. Under shell mode it is too, because explorer was started at the Boot video's end (row 6). Only an exit before that moment needs explorer started, and that is the `--shell` process's job (section 4.3).

**Power today** [here: `shell::power`]: `shutdown.exe /s /hybrid /t 0`, `shutdown.exe -r -t 0` and `SetSuspendState(false, true, false)`. None of them needs explorer.

**There is no "Settings > System" in the Bundle.** #154 asks for "Start with Windows as the shell … with the Bundle's wording and warnings". The Bundle has no such page and no such wording. Its Settings are Theme settings, Playnite settings and More Playnite options (settings.md 6.10, 6.11). The Boot shell is set up from a terminal with `--install` and `--uninstall`. Its only user-facing words are those commands' lines and the fallback line "Playnite closed 3 times in 5 minutes. Press the Guide button to try again." So #154 writes new wording. Section 4.5 lists what the warnings have to say.

## 2. Two facts that bound every choice

1. **Explorer becomes the full shell only while HKLM `Shell` names `explorer.exe` and no shell window exists** [Bundle research: per-user-shell §1, from ReactOS's explorer and two Windows reports; seen on the TV in runs R1–R6]. The per-user HKCU value only changes what userinit starts. So the route stays HKCU-only and needs no admin, and anything that changes HKLM `Shell` breaks explorer, Game Pass launches and the fall back to the desktop.
2. **Nothing restarts a non-explorer shell.** `AutoRestartShell` brings explorer back. Microsoft's Server Core page suggests nothing brings a `cmd.exe` shell back [Bundle research: per-user-shell §5]. The Bundle's T6 (kill the harness, see what Windows does) was **never run** [TV, #107: "Not tested: T6"]. Whatever process Winlogon starts has to be the one least likely to crash. That is the main reason the shell can't be the Dashboard's own GPU process.

## 3. Fold it in, or keep a launcher

The options:

| | A. The Dashboard's process is the shell | B. A separate small launcher exe | **C. One exe, two processes (recommended)** |
|---|---|---|---|
| What `Shell` names | `dashboard.exe` | `dashboard-shell.exe` | `dashboard.exe --shell` |
| Who keeps the Dashboard up after a crash | Nobody: a crash takes the shell down. The safety net brings explorer up at 90 s, and nothing brings the Dashboard back. | The launcher | The `--shell` process |
| Guide button after Exit to desktop | The Dashboard would have to stay running and hidden. That keeps a GPU device, 150 MB and the library alive to wait for one button. | The launcher reads the pad | The `--shell` process reads the pad, with the same `input` code |
| Black at sign-in | The Dashboard's first frame, about 0.4 s warm and slower cold [plan.md]: the gap after Welcome is longer than the Boot shell's 0.25 s | GDI black in milliseconds | Same as B: `--shell` makes a GDI window and never starts the GPU |
| Files to install, version and update | 1 | 2, with two versions to keep in step | 1 |
| Update while it's the shell | The exe is in use all session: rename it aside | The launcher's exe is in use all session: rename it aside. The Dashboard's isn't in use while it's down. | The exe is in use all session by `--shell`: rename it aside (section 4.6) |
| A crash in code they share | n/a | None shared | `--shell` shares the binary but runs only its own path: argument parsing, Win32 windows, XInput and the log. No wgpu, stores, Media Foundation or Discord. |

**C, because:**

- It keeps B's two-process robustness. That is what the Bundle built on, and the TV runs showed it working: the watchdog, crash relaunch and fall back in R4, Guide to come back in W3 [TV, #107].
- It keeps A's single file. The `Shell` value, the sentinel's Run entry and the uninstall key all name one path that never changes, and one version number covers both halves (#155).
- **Version skew is bounded.** Within one sign-in, the `--shell` process keeps running the build it started with, even after an update has renamed it aside. The contract between the two halves has to stay compatible across a version: the event names, the exit codes, the `--shell`, `--sentinel` and `--handover` arguments, and "watch by process image path, not by child handle". Put a contract number in the event names (`Local\XboxSeriesDashboard.v1.FirstFrame`) so a future break is explicit [inference].

**Against C:** a 15 MB exe for a process that needs a fraction of it. The pages are mapped on demand, so the cost is address space, not memory or start time **[inference; measure the `--shell` black's time on the VM]**. And a bug in startup code shared by both halves (`main`'s argument parsing) would take both down. Keep `--shell`'s branch at the very top of `main`, before logging and settings are set up.

## 4. What C needs

### 4.1 The `--shell` process

The Boot shell's steps 1–14 in section 1, with these changes:

- **It starts `dashboard.exe` with `--from-shell`** from its own folder: `GetModuleFileNameW` of itself, with no argument, so a renamed-aside old build still starts the current file (section 4.6). `--from-shell` tells the Dashboard to signal the events and to treat Exit to desktop as below.
- **The black window runs on a thread of its own, with its own 60 s timer** that takes the black down unless the main loop renews it. If the main loop hangs before explorer, a topmost black window would otherwise cover the desktop that the safety net brings up at 90 s. The Bundle's black shares the main thread **[inference; the Bundle never saw this case]**.
- **The audio wait** (row 3) stays until a TV run shows the Boot video's sound survives a device change at sign-in. It costs up to 6 s only when the device is still changing, and nothing when it has settled.
- **The watchdog watches `dashboard.exe` processes in the session by image path**, as the Bundle watches Playnite by name. After an update's handover, the running Dashboard isn't its child (section 4.6). The exit rules:
  - exit 0 with no Dashboard left: Exit to desktop; it stays down;
  - exit 0 with another Dashboard left: a handover; it watches the new one;
  - any other code, or any exit before that start's FirstFrame: a crash.

  The Dashboard has no `safestart.flag` and no crash window, so those rules go.
- **No hang detection**, as in the Bundle: View+Menu held 5 s is the way out. Windows' `IsHungAppWindow` could be a later addition, judged on the TV.
- **No Steam, Discord or Spotify list** (section 4.4).
- **A mutex `Local\XboxSeriesDashboard.Shell`** for the sentinel. Its log goes to `<data dir>\shell\logs\<stamp>.log`.

### 4.2 What the Dashboard adds

- **The events.** With `--from-shell`:
  - set `…FirstFrame` when Start-up's first frame is presented (the `startup: first video frame` line, or Who's playing today?'s first frame with the video Off or not found);
  - set `…BootVideoEnded` at the video's end, skip, failure or 20 s stop, or at once with no video.

  With `--no-startup`, set both with Home's first frame.
- **A single-instance guard**, `Local\XboxSeriesDashboard.Running`. Today nothing stops two Dashboards: the Start menu while one runs, or the Guide button racing a relaunch [here: no mutex in `crates/app`]. A second start should bring the first to the front and exit. The exception is `--handover`, which waits for the mutex instead (section 4.6).
- **Start-up topmost while it shows, under `--from-shell`.** On the TV, what kept explorer, Discord, Steam's updater and a Game Pass splash from showing over Who's playing today? was the Bundle's Start-up window being topmost. Every one of them took the foreground [TV, #107]. The port's window isn't topmost [here: `window.rs`; only the Overlay and the Toast strip are]. The pad survives losing the foreground, because the Dashboard reads XInput, which #107 had to switch to (#115 there). Without topmost, explorer's startup run at BootVideoEnded can draw over Who's playing today? **[inference]**.
- **Exit to desktop under `--from-shell`**: unchanged (exit 0). The `--shell` process makes sure there is a desktop.
- **The Xbox source activates packaged games through `IApplicationActivationManager::ActivateApplication`** rather than `ShellExecuteW("shell:AppsFolder\…")` (section 5).

### 4.3 Exit to desktop, and coming back

1. The Dashboard exits 0 after `PowerOff.wav`.
2. The `--shell` process sees no Dashboard left and leaves it down.
3. If no shell window exists (an exit before BootVideoEnded, or explorer dead), it starts explorer, as the Bundle's fallback does.
4. While no Dashboard runs, the Guide button starts one, and so does the Start menu shortcut (explorer is up).
5. View+Menu held 5 s closes the Dashboard to the desktop from anywhere, over a game included, as the Bundle's does. The Dashboard has its own Power hold on the Guide button, which is separate.

### 4.4 Startup apps

The Bundle switched off Steam's, Discord's and Spotify's HKCU Run entries and started them itself, so those three came first. That was this one PC's preference [#104][i104]. For a program others install, it means writing other programs' `StartupApproved` bytes, and Discord deletes the switch when it is reinstalled or "Open Discord" is turned on [Bundle research: boot-shell-updates §2].

**Recommended:** shell mode touches no other program's startup entry. Explorer starts every switched-on app after its startup delay, about 10 s [Bundle research: per-user-shell §2]. Explorer itself starts at the Boot video's end, so every app still trails Start-up [inference from the TV order in #104/#107]. A "start these first" list can come later as a setting if the TV shows it's wanted.

SCOOBE ("Let's finish setting up your device") is different. It is explorer's own logon task, and after an update it comes up over the Dashboard. Switching it off is per-user and needs no admin [Bundle research: boot-shell-updates §4]. Shell mode should switch it off and record the value it had.

### 4.5 What Settings has to say (#154 writes the words)

- It takes effect at the next sign-in, and turning it off does too.
- The way back from the sofa: Exit to desktop, or View+Menu held 5 s. The Guide button brings the Dashboard back.
- The way back with a keyboard if the Dashboard won't start: Ctrl+Shift+Esc, Run new task, `explorer.exe`, then turn it off (section 6).
- It is refused, and the page says why, when:
  - the Bundle's Boot shell is set up (section 6, F11);
  - HKLM `Shell` isn't `explorer.exe`;
  - this copy isn't the installed one: a portable zip would leave `Shell` naming a folder the user may move.

### 4.6 Update while it's the shell

The `--shell` process runs from `dashboard.exe` all session, so the file can't be overwritten or deleted, but it can be renamed:

- The Bundle's `--install` renames the running Boot shell exe aside and copies the new one in [Bundle code: `Setup.CopyExe`]. That path shipped in bootshell-v1.0.0, but no TV run is recorded doing it while the Boot shell was running **[unverified]**.
- Microsoft's file docs say only that "a file must also be closed before an application can move it" ([Moving and replacing files][mvf]).

Nobody has seen this repository rename its own running exe. The VM payload must prove it (section 9, P8).

The sequence, the same with or without shell mode:

1. Download to `dashboard.exe.new` beside the exe, and check its SHA-256 against the release manifest (#156).
2. Rename `dashboard.exe` → `dashboard.<old version>.old`, then `dashboard.exe.new` → `dashboard.exe`. Same folder, same volume, so each rename is atomic (`MoveFileExW` without `MOVEFILE_COPY_ALLOWED`) [MS: MoveFileEx]. Never `MOVEFILE_DELAY_UNTIL_REBOOT`, which writes HKLM `PendingFileRenameOperations` and needs admin [MS: MoveFileEx remarks].
3. **The handover.** The running Dashboard starts the new `dashboard.exe --handover <pid> [--from-shell]` and waits up to 30 s for the new one's `…FirstFrame`, or a dedicated `…Ready` event.
   - **If it comes**, the old Dashboard exits 0. The watchdog sees another Dashboard left: a restart, not a quit.
   - **If it doesn't** (the new one exits, crashes or times out), the old one ends it and renames back: `dashboard.exe` → `dashboard.<new version>.bad`, `.old` → `dashboard.exe`. Then it carries on and says so, with no error Toast, as #156 asks for a 404.

   This is "the old one kept until the new one has started once", with the old process itself as the judge. So it works without the `--shell` process too [inference].
4. **Removing `.old`.** It is removed at the next sign-in by `--shell` before it starts the Dashboard, or by any Dashboard start that finds no other process using it. Under shell mode it can't be removed sooner, because the `--shell` process is still running from it.

**What it rules out:** an updater that waits for the exe to be "not running". Under shell mode it never is. And MSIX for this build, whose updates replace the package at a new versioned path (section 7).

## 5. Store games and Game Pass with no explorer

- **What the TV showed** [TV, #107, R3]:
  - With no shell, `IApplicationActivationManager::ActivateApplication("ROBLOXCorporation.RobloxGDK_…!Game")` returned S_OK in 88 ms, the Game Pass splash showed at once, and Roblox came up 15 s later from cold.
  - With no shell, Playnite's `explorer.exe shell:AppsFolder\…` failed with "Class not registered" from a `/factory` explorer, and no shell appeared.

  This corrected the Bundle's own desk research, which had said activation itself needs the shell [Bundle research: game-pass-without-explorer §3].
- **What the port does** [here]:
  - `source-xbox` gives a game's start as the address `shell:AppsFolder\<AUMID>`.
  - `shell::shell_open` passes it to `ShellExecuteW` in the Dashboard's own process.
  - That isn't the explorer.exe route that failed. Whether the shell namespace resolves `shell:AppsFolder` in-process and activates through the same manager with no shell window is **[unverified]**.
- **Recommended:**
  - `source-xbox`'s launch calls `ActivateApplication` with the AUMID it already has, and the address stays for the log and a fallback. This is the one route seen working with no shell.
  - Explorer still starts every boot behind Start-up (section 1, row 6), so a launch with no shell can only happen in the seconds before the Boot video ends, which Start-up keeps for itself, or after explorer has died and before Windows or the watchdog restarts it.
- **Other stores' addresses** (`steam://`, `com.epicgames.launcher://`, `goggalaxy://`, …) are handled by `ShellExecuteW` through their registered handlers in HKCR. That is shell32 in our process plus the registry, not explorer, so it should work with no shell **[inference; unverified]**.
- **What else goes missing without explorer**, from the Bundle's T4 and #107, in Rust terms:
  - Gone: Win+keys, the taskbar and tray, explorer-hosted toasts, and Startup apps [Bundle research: per-user-shell §3; TV, #107: "only Ctrl+Shift+Esc works"].
  - Unaffected:
    - the Dashboard's own Toasts and Overlay, which are its own windows;
    - Power, which is `shutdown.exe` and `SetSuspendState`;
    - wake from sleep, which is `PowerRegisterSuspendResumeNotification` and needs no window (start-up.md 8);
    - the pad, which is XInput.
  - Now Playing reads Windows' media sessions through WinRT. Whether that works with no shell is **[unverified]**. It only matters before the Boot video ends.
- **The taskbar over the Dashboard once explorer is up**: #107's R6 showed no taskbar over a fullscreen, foreground Playnite with `MarkFullscreenWindow` off. The port's window is fullscreen-borderless, which the taskbar detects on its own [MS: `ITaskbarList2::MarkFullscreenWindow` remarks, via per-user-shell §4]. Call `MarkFullscreenWindow` on `TaskbarCreated` only if the VM or TV shows a taskbar [inference].

## 6. Failures, and the way back from each

**The rule the Bundle kept, and the port keeps:**

- `Shell` is written last and removed first. Before removing it, the sentinel is removed, because a sentinel left behind would write `Shell` back.
- Explorer starts every sign-in.
- Every path through the `--shell` process ends in "a shell window exists": explorer, started by `--shell` or by the safety net.

| # | Failure | What happens | The way back | Basis |
|---|---|---|---|---|
| F1 | The Dashboard crashes (a panic, a lost GPU device) | `--shell` sees an exit other than 0 and relaunches it under the black. The third crash in 5 min shows the line for 5 s, then the desktop, and relaunches nothing more on its own. | The Guide button starts it again and resets the count | Bundle code; TV R4 |
| F2 | The Dashboard hangs | Nothing notices on its own | View+Menu held 5 s: `WM_CLOSE`, then ended after 10 s, then the desktop | Bundle code; TV W1–W3 (not W4, the hold over a game) |
| F3 | The Dashboard can't start (no GPU adapter, a broken build) | Each failed launch counts as a crash, then F1's fall back | As F1. A broken build after an update is rolled back by the handover (section 4.6). | Bundle code (`LaunchFailed`); TV R4 |
| F4 | `--shell` crashes after explorer is up | Explorer is the shell. The Dashboard runs on with no watchdog until sign-out. | The next sign-in. Nothing to do. | inference |
| F5 | `--shell` crashes or hangs before explorer | A crash takes its black with it. A hang leaves the black up until its own 60 s timer (section 4.1). The safety net starts explorer at 90 s. Explorer's startup run then starts the sentinel, which finds no `--shell` mutex (crash) and starts the Dashboard, or finds it held (hang) and starts nothing. | Automatic within about 100 s. The pad can't help, so wait or use a keyboard. | Bundle code (safety net, sentinel); the hang case is **[unverified]** |
| F6 | `Shell` names an exe that isn't there (a hand-deleted folder, power lost between the two renames of an update) | What userinit shows is **[unverified]** (Bundle T8, never run): most likely a black screen with a cursor. The safety net doesn't use the exe, so it starts explorer at 90 s. The sentinel can't run. | Reinstall, which puts the exe back, or, from the desktop the safety net brought up, `reg delete "HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon" /v Shell /f` | Bundle research per-user-shell §5; inference |
| F7 | `Shell` is lost (a full Windows upgrade) | Explorer is the shell from the start. The sentinel writes `Shell` back and starts the Dashboard. | Automatic | Bundle research boot-shell-updates §1, §6; TV R5 |
| F8 | HKLM `Shell` isn't `explorer.exe` (another tool or an admin changed it) | Explorer won't become the shell, so there is no desktop, no fall back and no Game Pass by the explorer route | Turning shell mode on refuses. At sign-in, `--shell` logs it and falls back to running the Dashboard with no desktop. The user fixes HKLM as admin. | Bundle research per-user-shell §1; boot-shell-updates C1 |
| F9 | `WindowsLogon/OverrideShellProgram` is set (Windows 365 Boot, MDM) | Winlogon starts `ShellAppRuntime.exe` instead of any `Shell` value | Turning shell mode on refuses and says why. Only an admin or MDM can clear it. | [MS: Policy CSP WindowsLogon]; Bundle research boot-shell-updates §3 |
| F10 | Uninstalled while shell mode is on | The uninstaller runs `dashboard.exe --shell-off` first and stops if it fails. Without that, it's F6. | Section 8 | inference |
| F11 | **The Bundle's Boot shell is still set up.** Its sentinel `HKCU\…\Run\XboxSeriesBootShellSentinel` runs at every sign-in where it finds no Boot shell mutex, writes `Shell` back to `XboxSeries.BootShell.exe` and starts Playnite [Bundle code: `Sentinel.Check`]. With ours on, it would take the PC back at the very next sign-in. | Turning shell mode on refuses while that Run entry, the task `XboxSeries.BootShell.SafetyNet`, or a `Shell` value naming another program exists | `XboxSeries.BootShell.exe --uninstall` from a terminal, then turn ours on | Bundle code; inference |
| F12 | Explorer dies after it has become the shell | Windows restarts it (`AutoRestartShell`). If no shell window exists for 10 s, `--shell` starts it. Startup apps don't run again. | Automatic | Bundle code (`ShellWatch`); Bundle research per-user-shell §2 |
| F13 | Windows is shutting down or restarting (Power's Turn off or Restart, an update) | The Dashboard is closed by Windows. `--shell` must have heard `WM_QUERYENDSESSION` first and not relaunch. | n/a | Bundle code (`SessionEnding`) |
| F14 | Exit to desktop before explorer is up | `--shell` starts explorer on the exit (section 4.3) | Automatic | Bundle code (fallback's `StartDesktop`) |
| F15 | Two Dashboards (Start menu while one runs, or Guide racing a relaunch) | The single-instance guard brings the first to the front | n/a | section 4.2 |
| F16 | An update's new build won't start | The handover rolls back (section 4.6) | Automatic | inference |

**With a keyboard, when nothing else works**, in order [Bundle research: per-user-shell §5]:

1. Ctrl+Shift+Esc → Run new task → `explorer.exe`. It becomes the full shell, because HKLM is explorer and no shell window exists [MS: Windows 11 release health workaround, via per-user-shell §3].
2. From it, `"%LOCALAPPDATA%\Programs\XboxSeriesDashboard\dashboard.exe" --shell-off`, or `reg delete "HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon" /v Shell /f` plus `reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v XboxSeriesDashboardSentinel /f`.
3. Another local account signs in to a normal desktop, because the value is per-user. From there, `reg load` the user's `NTUSER.DAT` and delete the value.
4. Safe Mode with Command Prompt uses `SafeBoot\AlternateShell` (`cmd.exe`). Whether plain Safe Mode honours the HKCU `Shell` value is **[unverified]**.
5. Holding Shift at boot only skips autologon. Signing in as the same user still runs our shell [MS: Turn on automatic logon, via per-user-shell §5].

**Tested:** the ticket asks for the recovery path to be tested. On the VM, P5–P7 in section 9 break the Dashboard, `--shell` and the `Shell` value on purpose. The keyboard routes (1–4) and a real cold boot need the TV and a person (section 10).

## 7. The Xbox mode home-app route

**What it needs** [Bundle research: xbox-mode-home-app §1, §4; standalone-rust-xbox-mode.md §3]:

- **An MSIX** declaring the `windows.gamingApp` app extension, the `Microsoft.appCategory.gamingHome_8wekyb3d8bbwe` custom capability and `runFullTrust`. The Rust exe sits inside it like any Win32 exe.
- **The capability.** It is granted only through an SCCD with the `FFFF` development catalog under **Developer Mode**. Microsoft says there is "no public GDK API or registration flow for non-OEM apps".
- **Trust.** A certificate the PC trusts in `LocalMachine\TrustedPeople` (admin, once), or a loose-file `Add-AppxPackage -Register` under Developer Mode.
- **A handheld.** Only gaming handhelds get *Choose home app* and *Enter Xbox mode on startup*. A desktop needs the `DeviceForm` 46 fake, which ADR 0002 rejected and #109 there removed from the TV PC.

**What it loses:**

- **The Guide button.** "When FSE is active, Windows has exclusive access to the **Guide** button." A short press opens Game Bar. An app gets a Guide press only through View+Menu, or View in the long-press Task Switcher [MS: GDK Game launchers and handhelds, via xbox-mode-home-app §3]. So the Guide would open on View+Menu there.
- **Shell mode**, by construction. By default an MSIX copies HKCU writes "on write to a private per-user, per-app location" [MS: How packaged desktop apps run, Registry], so a packaged `Shell` value wouldn't be the one Winlogon reads. Opting out takes `unvirtualizedResources` with `RegistryWriteVirtualization=disabled`, a restricted capability that "is not intended to be used for other scenarios, because it could compromise the system's ability to uninstall cleanly" [MS: App capability declarations]. Microsoft's own pages disagree, though: the MSIX troubleshooting guide says HKCU writes "are **not** virtualized and persist" [MS: MSIX troubleshooting guide]. Which one holds for a full-trust Win32 app on 26300 is **[unverified]**. Either way:
  - a packaged app lives at a versioned `WindowsApps\<full name>` path that changes with each update;
  - its uninstall runs none of our code, so it couldn't take `Shell` off, which is F6.

  The packaged build and the shell-mode build are two products.
- **The updater (#156)**: a package is updated as a package, by the Store or App Installer, not by renaming an exe.

**The recommendation: leave it as a documented spike. M7 doesn't build it.**

- It serves a person on a handheld, and nobody on this project has one.
- The TV PC can't use it without reopening ADR 0002.
- The win11-e2e VM is a desktop. Turning it into a handheld means the `DeviceForm` fake, ViVe flags and Developer Mode, and every human-only step (a certificate, Developer Mode on a real PC) sits at the end.

plan.md's "a packaged home app for Xbox mode comes last" stays true. Its ticket is opened when someone with a handheld wants it, starting from standalone-rust-xbox-mode.md's spike and this section.

## 8. Install, update and uninstall: exactly what each touches

Paths:

- **The installed exe**: `%LOCALAPPDATA%\Programs\XboxSeriesDashboard\dashboard.exe`. That is the per-user install folder, kept apart from the data folder so an uninstall can keep data (#155) [inference].
- **The data folder**: `%LOCALAPPDATA%\XboxSeriesDashboard\` (`library::data_dir`) [here].

| When | Registry | Files and tasks |
|---|---|---|
| **Install (#155)** | `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\XboxSeriesDashboard`: `DisplayName`, `DisplayVersion`, `Publisher`, `InstallLocation`, `UninstallString`, `DisplayIcon`, `NoModify`=1, `NoRepair`=1. **Nothing of Winlogon, Run or Task Scheduler.** | `…\Programs\XboxSeriesDashboard\dashboard.exe`, the licences file, the README. Start menu: `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Xbox Series Dashboard.lnk`. Installing over a running copy renames it aside as in section 4.6 rather than asking it to close. |
| **Shell mode on (#154)**, in this order, each value's before-state saved to the record first | 1. Task Scheduler root task `XboxSeriesDashboard.SafetyNet` (logon trigger for this user, `Delay` `PT90S`, `LeastPrivilege`; action `%SystemRoot%\System32\conhost.exe --headless …\powershell.exe -NoProfile -NonInteractive -Command "<if no explorer in this session: start C:\Windows\explorer.exe; append a line to safety-net.log>"`). 2. `HKCU\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement\ScoobeSystemSettingEnabled` = DWORD 0. 3. `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\XboxSeriesDashboardSentinel` = `"<install>\dashboard.exe" --sentinel`. 4. **Last**: `HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell` = `"<install>\dashboard.exe" --shell` (quoted, as the Bundle learned: a profile path with a space splits otherwise). | Record: `<data>\shell\record.json`, saved after each step. Logs: `<data>\shell\logs\`. |
| **Every sign-in under shell mode** | Reads HKLM `Winlogon\Shell` (contains `explorer.exe`), HKCU `Shell` (names this exe) and `OverrideShellProgram`. Writes nothing but logs. | Writes `<data>\shell\logs\<stamp>.log` and keeps the newest 30. Deletes `…\Programs\XboxSeriesDashboard\dashboard.*.old`. |
| **Sentinel (explorer's startup run)** | With no `--shell` mutex in the session: writes `Shell` back if it differs | `<data>\shell\logs\sentinel.log` |
| **Update (#156)** | `HKCU\…\Uninstall\XboxSeriesDashboard\DisplayVersion`. **`Shell`, the Run entry and the task are untouched: the path doesn't change.** | `dashboard.exe.new` (download), then `dashboard.<old>.old` and `dashboard.exe` by rename, or `dashboard.<new>.bad` on a rollback. A marker `<data>\update\pending.json` (from, to, SHA-256) until the handover ends. |
| **Shell mode off (#154)**, the reverse, but the sentinel before `Shell` | 1. Delete `Run\XboxSeriesDashboardSentinel`. 2. `Winlogon\Shell` back to the recorded value (deleted if there was none). 3. `ScoobeSystemSettingEnabled` back. 4. Delete the task. A value that no longer holds what shell mode wrote is left alone and said. | The record is renamed `record.<stamp>.json`. The running `--shell` process runs on until sign-out. |
| **Uninstall (#155)** | `dashboard.exe --shell-off` first, and stop with a message if it fails. Then delete the `Uninstall\XboxSeriesDashboard` key. If the exe is in use (`--shell` still running this session), add `HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce\XboxSeriesDashboardCleanup` = `cmd.exe /c rmdir /s /q "<install>"`. Explorer runs RunOnce at the next sign-in, when it is the shell again [MS: Run and RunOnce keys, via per-user-shell §2]. | Delete the Start menu shortcut and the install folder (or rename the running exe aside within it for RunOnce to remove). Keep `<data>` unless the user asks. |

**No step needs admin.**

- HKCU `Winlogon` was user-writable on the TV PC [Bundle research: per-user-shell §1].
- A user logon task registered from a medium-integrity terminal there [Bundle code: README, tried 2026-10-23].
- Nothing writes HKLM.

Whether the win11-e2e template's user can do the same is to be seen in P1.

## 9. What a VM payload must prove (#154's)

**The harness can't do it yet.**

- `vm.sh desktop` runs one `run.ps1` from a scheduled task and collects `out.zip` when it ends [here: `e2e/proxmox/vm.sh`]. A restart kills it.
- Shell mode's payload needs two or three phases across restarts: each phase registers the next as an interactive logon task for the user, writes its state under `C:\e2e`, then restarts with `shutdown /r /t 0`, and only the last phase uploads `out.zip`.
- The template signs in by autologon (`vm.sh up` waits for it), so a restart signs in again **[inference]**.
- Task Scheduler is a service, so the logon task runs with any shell [Bundle research: per-user-shell §2].

`e2e/payloads/lib/E2E.psm1`'s safe mode must keep refusing every write here, so the TV runs only `--dry-shell`.

The payload, judged by log lines, never by sleeps:

- **P1 Turn on.**
  - Through Settings with the virtual pad, or `--shell-on`.
  - Read back the four writes and the record, byte for byte, and that no HKLM value changed.
  - `--dry-shell` prints the same writes and writes nothing.
- **P2 Refusals.** Each must be refused, with nothing written:
  - a fake Bundle sentinel `XboxSeriesBootShellSentinel`;
  - `Shell` naming another exe;
  - running from a portable folder;
  - HKLM `Shell` not `explorer.exe`. The payload runs elevated, so it can set and restore HKLM.
- **P3 Restart into the Dashboard.** After the restart:
  - `--shell`'s log has black up, `ShellDesktopSwitchEvent` set, the Dashboard started, FirstFrame, BootVideoEnded (use `--media` with a short generated video or Off), and explorer started;
  - `GetShellWindow()` is owned by that explorer;
  - there is no File Explorer window (no `CabinetWClass`);
  - `SessionInfo\<session>\RunStuffHasBeenRun` appears only after explorer starts;
  - screenshots at sign-in show black, never the desktop, as far as WARP's timing allows.
- **P4 Exit to desktop.**
  - Power, Exit to desktop: the Dashboard exits 0 and `--shell` logs a quit.
  - The desktop is up and no Dashboard runs.
  - A Guide press on the ViGEm pad brings it back.
  - View+Menu held 5 s closes it again.
- **P5 Crash on purpose**, three times within 5 minutes. Use a test-only `--crash-after-first-frame` that only `--allow-toast-posts`-style test runs honour.
  - Each relaunch comes under the black.
  - The third shows the line, then the desktop, and nothing more.
  - Guide brings it back.
- **P6 Break `--shell`.**
  - Kill it before explorer: the safety net's line at about 90 s, then the sentinel starts the Dashboard.
  - Kill it after explorer: nothing changes until sign-out.
- **P7 Lose `Shell`.** Delete the value, then restart:
  - explorer is the shell;
  - the sentinel writes `Shell` back and starts the Dashboard;
  - the next restart is `--shell`'s again.
- **P8 Update under shell mode.**
  - `standin serve-updates` serves a newer build. Install: `dashboard.exe` is renamed aside while `--shell` runs from it, which proves the rename on a running image.
  - The handover finishes, and the new version is in the log and in Settings > About.
  - A bad SHA-256 is refused and nothing is renamed.
  - A new build that crashes at start is rolled back within 30 s.
  - The next restart deletes `.old`.
- **P9 Turn off, then restart.**
  - Every value is back as the record had it (`Shell` absent on the template).
  - The task is gone.
  - Explorer is the shell from the start and no Dashboard runs.
- **P10 Uninstall with shell mode on.**
  - `--shell-off` runs first.
  - After the restart, explorer is the shell and the RunOnce cleanup has removed the folder.
  - `<data>` is still there.

Not provable on the VM:

- a Game Pass launch (no Game Pass, no account);
- the real black gap and cold-boot times (WARP, no real GPU);
- the audio device settling on HDMI;
- F6, a shell exe that isn't there. It can be tried on the VM, but what userinit shows has to be looked at in the Proxmox console. Keep it as a `--keep` run for the lead.

## 10. What needs a human

- **Turning shell mode on on the TV PC** is the user's decision (#154).
  - Before it, **take the Bundle's Boot shell off the TV PC** if it is set up there (`XboxSeries.BootShell.exe --uninstall`). Otherwise its sentinel takes the PC back at the next sign-in (F11). Whether it is set up there wasn't checked: this research doesn't touch the TV PC.
  - Keep a keyboard at hand the first time.
- **On the TV, once on:**
  - the black from Welcome to the Dashboard (the Bundle's T3);
  - cold-boot time against the Bundle's 20 s target;
  - the Boot video's sound on HDMI with and without the audio wait;
  - nothing over Who's playing today? when explorer comes up;
  - a Game Pass launch from Home, and one in the seconds before explorer if possible;
  - T6, killing `--shell` and then explorer;
  - plain Safe Mode.
- **A full Windows upgrade** whenever one comes (the Bundle's U2): does `Shell` survive, and did the sentinel fire?
- **A code-signing certificate** and the decision to publish (#155). An unsigned `dashboard.exe` named in `Shell` may draw SmartScreen or antivirus attention at sign-in **[unverified]**. The Bundle's Boot shell was unsigned on the TV PC with no report of trouble.
- **Xbox mode** (section 7): a handheld, Developer Mode, a trusted certificate, and a new ADR if it is ever wanted on the TV PC.

## Open questions

1. Does `ShellExecuteW("shell:AppsFolder\<AUMID>")` in our own process activate a packaged game with no shell window? If it does, `ActivateApplication` is only a nicety.
2. What does userinit do with a `Shell` value naming a missing exe (T8)? It decides how bad F6 is.
3. Does Winlogon restart, or react at all to, a non-explorer shell that exits (T6)?
4. Does the Boot video's Media Foundation sound survive the default device changing at sign-in, so `--shell`'s audio wait can go?
5. Is the Dashboard's window, made topmost for Start-up under `--from-shell`, enough to keep explorer's startup burst from drawing over Who's playing today?, as the Bundle's was?
6. MSIX HKCU virtualisation for a full-trust app on 26300: Microsoft's two pages disagree (section 7). It only matters if the home-app spike is ever built.

## Sources

- **The Bundle** at [`f11e4f9`][b]: the research named at the top; [ADR 0002][adr]; [`bootshell/XboxSeries.BootShell/`][bs] (`Program.cs`, `Watchdog.cs`, `Launcher.cs`, `SignInSchedule.cs`, `ShellWatch.cs`, `Sentinel.cs`, `Setup.cs`, `UserTasks.cs`, `README.md`) and its tests (`WatchdogTests`, `SentinelTests`, `SetupTests`, `ShellWatchTests`, `SignInScheduleTests`); `CONTEXT.md`'s **Boot shell** and its relationships. Issues [#107][i107] (the TV runs), [#104][i104], [#112][i112], [#103](https://github.com/snrgarb/xbox-series-theme/issues/103), [#122](https://github.com/snrgarb/xbox-series-theme/issues/122).
- **This repository** at `44974ac`: [standalone-rust-xbox-mode.md](standalone-rust-xbox-mode.md), [../plan.md](../plan.md), [../HANDOVER.md](../HANDOVER.md), [../spec/start-up.md](../spec/start-up.md), [../spec/guide.md](../spec/guide.md) 13, [../spec/settings.md](../spec/settings.md) 6.10–6.11; `crates/shell/src/power.rs`, `crates/shell/src/lib.rs` (`shell_open`), `crates/app/src/window.rs` (`Effect::Exit`, the window's attributes), `crates/audio/src/engine.rs`, `crates/source-xbox/src/lib.rs`, `crates/library/src/lib.rs` (`data_dir`), `e2e/proxmox/vm.sh`, `e2e/htpc/run.sh`.
- **Microsoft**, read 2026-11-17:
  - [Understanding how packaged desktop apps run on Windows: Registry](https://learn.microsoft.com/windows/msix/desktop/desktop-to-uwp-behind-the-scenes#registry)
  - [Flexible virtualization](https://learn.microsoft.com/windows/msix/desktop/flexible-virtualization)
  - [App capability declarations: restricted capabilities](https://learn.microsoft.com/windows/apps/package-and-deploy/app-capability-declarations#restricted-capabilities)
  - [desktop6:RegistryWriteVirtualization](https://learn.microsoft.com/uwp/schemas/appxpackage/uapmanifestschema/element-desktop6-registrywritevirtualization)
  - [MSIX troubleshooting guide: runtime and virtualization](https://learn.microsoft.com/windows/msix/msix-troubleshooting-guide#runtime-and-virtualization-behavior)
  - [MoveFileExW](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-movefileexw)
  - [Moving and replacing files][mvf]
  - [Policy CSP: WindowsLogon](https://learn.microsoft.com/windows/client-management/mdm/policy-csp-windowslogon)
  - Through the Bundle's research, which cites them: Shell Launcher overview, Run and RunOnce registry keys, GetShellWindow, ITaskbarList2::MarkFullscreenWindow, Turn on automatic logon, the GDK's Game launchers and handhelds, KB5070297.

[mvf]: https://learn.microsoft.com/windows/win32/fileio/moving-and-replacing-files
