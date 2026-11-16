# The Dashboard as Xbox mode's home app through AnyFSE

Research for [#180](https://github.com/snrgarb/xbox-series-dashboard/issues/180). The user's words (2026-11-17): "I am keen to get this running in XBOX mode using a 3rd party AnyFSE tool to have it become the home app." The PC is the TV PC: a desktop, Windows 11, an RX 9070 XT, a 4K TV and Xbox pads. Not a handheld.

Researched 2026-11-17, on Linux. **Nothing was installed, run or changed on any machine**: no VM, no TV PC. AnyFSE was read as source, not run. This builds on [shell.md](shell.md) (shell mode, #153/#154, section 7 on the home-app route) and the Bundle's [xbox-mode-home-app.md][xm] and [ADR 0002][adr], and does not repeat them. Tags:

- **[AnyFSE code]**: [ashpynov/AnyFSE][any] at [`0e94178`][anyc] (2026-11-14, the v0.90.24 release), read file by file.
- **[AnyFSE issue]**: its GitHub issues, by number. The author's answers are his, not Microsoft's.
- **[MS]**: Microsoft Learn and Support, read 2026-11-17.
- **[Bundle]**: the Bundle's research and its TV runs (`~/projects/xbox-series-theme`), as cited.
- **[here]**: this repository at `5d22228`, and PR #165 (shell mode, not merged yet).
- **[press]**: news sites and guides. Secondary.
- **[inference]**: my reasoning. Nobody has tested it.
- **[unverified]**: nobody has seen it on a machine; sections 10 and 11 say how to find out.

[any]: https://github.com/ashpynov/AnyFSE
[anyc]: https://github.com/ashpynov/AnyFSE/tree/0e94178e1068526b12a9a3d2dd759c68c0be0b7b
[xm]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/research/xbox-mode-home-app.md
[adr]: https://github.com/snrgarb/xbox-series-theme/blob/f11e4f9/docs/adr/0002-boot-shell-not-xbox-mode.md
[gdk]: https://learn.microsoft.com/gaming/gdk/docs/gdk-dev/pc-dev/handheld/handheld-launchers?view=gdk-2610
[kb]: https://support.microsoft.com/en-us/topic/windows-gaming-full-screen-experience-67fb8d12-5467-4a95-8adf-0a10789576ab

## Summary

**It can be done on the TV PC, and the Dashboard needs only small changes to be a good home app. It still costs the handheld fake, and the Guide button is only partly ours.**

- **What AnyFSE is.** A small C++ program, MIT, one maintainer, released every few weeks (v0.90.24 on 2026-11-14), also listed on the Microsoft Store. It registers a signed identity package that Windows accepts as a gaming home app. When Windows starts the home app, AnyFSE starts the program you chose (any exe, with arguments), shows a splash until that program's window appears, and exits. It is not a shell and does not stay resident unless you ask it to.
- **What it needs.** Windows' Xbox mode API (24H2/25H2/26H2 builds that have Xbox mode), admin once, and **`DeviceForm` = 46 in HKLM**: AnyFSE's installer writes it itself and will not install without it. Microsoft still gives *Choose home app* and *Enter Xbox mode on startup* to gaming handhelds only. Nothing published as of 2026-11-17 widens that to desktops.
- **What has changed since ADR 0002:**
  1. AnyFSE now writes only `DeviceForm`, with no ViVe flags and no boot-time SYSTEM tasks. XFSET, which the Bundle removed, did both.
  2. AnyFSE 0.90.24 has a switch that stops a short Guide press opening Game Bar in Xbox mode (`HKCU\Software\Microsoft\GameBar\LegacyGamepadXboxGuideButtonEnabled`). If that lets `XInputGetStateEx` see the Guide bit, the Dashboard keeps its Guide on a tap. That is **[unverified]** and undocumented by Microsoft. A long press still opens Windows' Task Switcher, so the Guide's 0.7 s hold for Power cannot stay.
  3. Xbox mode now has its own Exit to Desktop and Power in Gaming Task View (October 2026 Insider and Release Preview builds) [press].
- **The objection that still holds:** the device form fake, admin, a third party's signed package, and Windows owning the Guide button by design. The fake is machine-wide, and anything that asks Windows "is this a handheld" will be told yes. That includes games that pick handheld presets **[inference]**.

## Recommendation

1. **Shell mode (#154) stays the default route for the TV PC.** It keeps the Guide button, needs no admin and no fake, and its way back is tested on the VM.
2. **Make the Dashboard a good home app anyway.** It is cheap: one start-up argument, Exit to desktop that leaves Xbox mode, a crash limit, a fixed window class, and the Guide on View+Menu while Xbox mode is active (section 6). Draft tickets X1 and X2.
3. **Settings detects Xbox mode, and never sets it up.** It shows which route is on and refuses shell mode while Xbox mode is set to start at sign-in. Setting it up means admin, a machine-wide fake and another program's configuration (section 7).
4. **Try it on the win11-e2e VM first** (section 10). Then it is the user's call to try it on the TV PC, by the steps in section 9, with the way back written down. If the user then keeps it, write an ADR in this repository saying the port supports it as an opt-in route. The Bundle's ADR 0002 decided for the Bundle's PC, not for the port.

## 1. What AnyFSE is and what it writes

**The home-app trick.** A *package with external location* (a sparse identity package) named `ArtemShpynov.AnyFSE`, AUMID `ArtemShpynov.AnyFSE_by4wjhxmygwn4!App` [AnyFSE code: `AppxManifest.xml`, `src/App/Constants.hpp`]. The manifest declares:

- `windows.gamingApp` as an app extension;
- the custom capability `Microsoft.appCategory.gamingHome_8wekyb3d8bbwe`, granted by a `CustomCapability.SCCD` with `AllowAny` and the `FFFF` development catalog;
- `runFullTrust`, `unvirtualizedResources` and `desktop6:RegistryWriteVirtualization` = `disabled`;
- `MinVersion` 10.0.26100.8039.

The exe itself lives in `C:\Program Files\AnyFSE`, outside the package [AnyFSE code: `AppInstaller_Install.cpp`; README]. The idea came from [driver1998/FullScreenExperienceShell](https://github.com/driver1998/FullScreenExperienceShell) (MIT, a proof of concept) [AnyFSE README].

**The installer** (`AnyFSE.Installer.exe`, admin, `/s` for silent) does this, in order [AnyFSE code: `AppInstaller_Install.cpp`, `GamingExperience.cpp`]:

| # | Writes | Where | Kept? |
|---|---|---|---|
| 1 | `DeviceForm` = 46, after saving the old value as `DeviceForm_AnyFSE` | `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\OEM` | Yes, until uninstall |
| 2 | Developer Mode on, **only if it was off** | `HKLM\…\AppModelUnlock` | No: turned off again at the end |
| 3 | A temporary certificate | `LocalMachine\TrustedPeople` | No: removed after the package is registered |
| 4 | Files | `%ProgramFiles%\AnyFSE` | Yes |
| 5 | The identity package, registered with that folder as its external location | the user's package store | Yes |
| 6 | The uninstall entry | `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AnyFSE` | Yes |
| 7 | A task `\AnyFSE`: no trigger, run on demand, highest rights, the user's interactive token, running `AnyFSE.exe /task`. It is how AnyFSE does its few admin jobs (start as admin, startup apps as admin, the device form) with no UAC prompt. | Task Scheduler root | Yes |
| 8 | Only if hotkeys or ROG Ally buttons are switched on: `AnyFSE Hotkeys` | `HKCU\…\CurrentVersion\Run` | Yes |
| 9 | Only on ROG Ally hardware: the `ACSEFilterInjector` service, which injects a DLL into ASUS's service | Services | Not on the TV PC |

**Its own settings** (Start menu, AnyFSE, right-click, *Configure*) write [AnyFSE code: `LauncherPage.cpp`, `Config.cpp`]:

- `HKCU\Software\Microsoft\Windows\CurrentVersion\GamingConfiguration`: `GamingHomeApp` = AnyFSE's AUMID and `StartupToGamingHome` = 0 or 1. These are the same values Windows' own *Choose home app* and *Enter Xbox mode on startup* write [Bundle: xbox-mode-home-app §2].
- `HKCU\Software\Microsoft\GameBar\LegacyGamepadXboxGuideButtonEnabled`, from the switch "Allow the gamepad to open Game Bar in Xbox mode". Off writes 1.
- Its JSON (`AnyFSE.json`) in the package's per-user `LocalState` folder: the launcher's `StartCommand`, `StartArg`, `ProcessName`, `ClassName`, `WindowTitle`, the splash, the startup apps, and `Extra/ExitFSEOnHomeExit`.
- To enter Xbox mode with a restart, it briefly writes `SystemDialogResults\EnterGamingPostureConfirmation` (and keeps the old value in `…_Restore`).

**No service, no boot task and no ViVe feature IDs on a desktop.** That is the difference from XFSET, whose two SYSTEM boot tasks and three feature IDs the Bundle removed from the TV PC ([xbox-series-theme#109](https://github.com/snrgarb/xbox-series-theme/issues/109)).

**Updates.** The settings app checks GitHub (or its Codeberg mirror) and runs the new installer with `/autoupdate`. The uninstaller's update path keeps the device form and the package [AnyFSE code: `Updater.cpp`, `AppUninstaller.cpp`, `Constants.hpp`]. How often it checks (`Update/CheckInterval`) wasn't followed through the code. The Microsoft Store listing (`XPFM6KZ5SD704G`, "MS content published" 2026-10-16) is a listing for the same installer **[inference from the `XP` id, which is the Store's id form for Win32 installers; the listing page could not be read]**.

**Uninstall** (Settings, Apps, or `unins000.exe`) [AnyFSE code: `AppUninstaller.cpp`]:

1. deletes the task and stops the ACSE service;
2. kills `AnyFSE.exe` and deletes the Run value;
3. puts `DeviceForm` back from `DeviceForm_AnyFSE`, or deletes it if that was 0;
4. removes the package, the certificates and the HKLM uninstall key, and the folder.

It does **not** reset `GamingHomeApp` or `StartupToGamingHome`. Choose the home app back to Xbox or None first, as OmniConsole's README warns for its own package [Bundle: xbox-mode-home-app, "What this changes elsewhere"]. If uninstall breaks, the README's manual clean-up is `Get-AppxPackage *AnyFSE* | Remove-AppxPackage`, `sc delete ACSEFilterInjector`, `rmdir "%ProgramFiles%\AnyFSE"` and `reg delete HKLM\…\Uninstall\AnyFSE`. It does not mention `DeviceForm`.

**Defender.** The README warns that Defender may flag it as `Trojan.Wacatac!ml`. It names the ACSE DLL injection (Ally only) and the installer's Developer Mode and certificate steps as the likely reasons [AnyFSE README; issue #82].

**Licence and how alive it is.** MIT, copyright Artem Shpynov [AnyFSE `LICENSE`].

- 360 stars, 1 open issue (a feature request, #109), and 120 closed.
- Releases: 0.90.15 on 2026-06-20, then 0.90.16, .17, .18, .19, .20 and .21, and 0.90.24 on 2026-11-14.
- Last commit 2026-11-14.
- The maintainer answers issues within days.

It is one person's project, though. When Windows renamed the feature in 26300.8170, the home-app picker vanished and the answer was "use XFSET" (#83).

## 2. What it needs

- **Windows with the Xbox mode API.** At start it checks `api-ms-win-gaming-experience-l1-1-0` and refuses with "supported since Windows 25H2 version for Handheld Devices" if missing [AnyFSE code: `App.cpp`]. The package needs 10.0.26100.8039 or later. Microsoft rolled Xbox mode out to PCs from late April 2026 (26100/26200 `.8328`+; 26300 for 26H2) [Bundle: xbox-mode-home-app §1; press: [Pureinfotech, Aug 2026](https://pureinfotech.com/windows-11-enable-xbox-full-screen-experience-any-pc/)]. On 2026-10-22 the TV PC offered *Enable XBOX mode* (Off), *Show XBOX mode in Task View* and no home-app choice [Bundle: #109's resolution].
- **The handheld device form.** Microsoft's own page still splits it: "On gaming handheld devices: … **Choose home app** … **Enter Xbox mode on startup**"; "On other PC form factors: Turn on **Enable Xbox mode**" [MS: [KB5070297][kb], read 2026-11-17]. No Microsoft source found says a desktop gets the home-app choice. The October 2026 Insider and Release Preview builds (26220.9739, 26340.9750, 26200/26300.9807) added a controller bar in Gaming Task View with Exit to Desktop and Power. Nothing about home apps [press: [HelpDeskGeek, 2026-10-25](https://helpdeskgeek.com/microsoft-tests-controller-friendly-gaming-task-view-windows-11/)]. So:
  - AnyFSE's installer sets `DeviceForm` = 46 itself and stops if it can't. Its own words: "Custom Home app selection is disabled. AnyFSE requires the gaming handheld device mode to be enabled." [AnyFSE code: `AppInstaller_Install.cpp`, `localization/en_US.json`].
  - Its README still says it "is not implement enabling FSE mode support in windows" and points at XFSET for that. That is for PCs where Xbox mode itself is missing, which the TV PC is not.
  - **Whether `DeviceForm` 46 alone, with no feature IDs, shows *Choose home app* on the TV PC's build is [unverified].** XFSET still turns on three feature IDs on "native" builds [Bundle: xbox-mode-home-app §1]. AnyFSE's code assumes the device form is enough, and one issue shows it working on a laptop on 26H2 26300.9711 (#120). Whether that laptop also had XFSET isn't said.
- **The device form is read machine-wide.** The GDK tells games to read it through `RtlGetDeviceFamilyInfoEnum` (`DEVICEFAMILYDEVICEFORM_GAMING_HANDHELD`) "if they want the device to be identified as a Windows gaming handheld" [MS: Handheld guidance, via Bundle research]. A game written to that guidance may choose handheld defaults on the RX 9070 XT **[inference; which games do is unknown]**.
- **Developer Mode.** Only during install. It is put back as it was. The TV PC had it on in October [Bundle: xbox-mode-home-app §0]. If it is still on, nothing changes there.
- **The Xbox app and Gaming Services.** AnyFSE doesn't need the Xbox app. It kills it if Windows starts it as the home app in its place (#53, #23). Game Bar is needed, because Game Bar's Home button and the Guide press go through it [MS: KB5070297]. Whether Xbox mode needs Gaming Services is **undocumented**.

## 3. How it starts the program, and what it expects of it

From `src/App/Main.cpp`, `Launchers.cpp`, `ExitFSE.cpp`, `MainWindow*.cpp` and `Tools/Process.cpp` [AnyFSE code]:

1. Windows starts AnyFSE as the home app at sign-in (with *Enter Xbox mode on startup*), on Win+F11 or Game Bar, and again whenever the player presses Home in Game Bar.
2. AnyFSE signals `ShellDesktopSwitchEvent`, so Windows' sign-in screen gives way at once, and shows a topmost splash. The splash can be text, logo, animation or a video, each switchable.
3. On the first launch of a sign-in (a global atom) in Xbox mode, it starts its own startup-apps list.
4. **If the chosen program already has a window** (by process name, class and title; minimised counts), it brings that window forward: `ShowWindow(SW_SHOWMAXIMIZED)` if it's minimised or hidden, then `AttachThreadInput` and `SetForegroundWindow`. Then it exits. This is the Game Bar Home path.
5. **Otherwise it starts the program** by `CreateProcessW("<StartCommand>" <StartArg>)`: no handle inheritance, the default working directory (AnyFSE's), and a 260-character limit on the whole command line. A `://` command goes to the shell as a protocol.
6. It polls every 500 ms for the program's window. When the window appears, the splash fades out. After 60 s with no window, the splash goes anyway.
7. If *Leave full screen experience on home app exit* is on, it stays and waits on the program's process. When the process ends and no new one appears, it calls `SetGamingFullScreenExperience(FALSE)` and Windows returns to the desktop. Otherwise AnyFSE exits with the splash.

**What it expects of the program:**

- **One window it can find.** The process name, class name and title are set per launcher. For a custom exe they are typed into *Custom settings*.
- **A window that comes up within 60 s.**
- **One instance.** A second start would come only from step 5, and step 4 runs first.
- **No exit codes.** Nothing reads the program's exit code. An exit is an exit.
- **No arguments** of its own: it passes `StartArg` through.
- **No package identity.** A child of a sparse-package app gets identity only through its own exe's manifest, so `dashboard.exe` runs unpackaged. Its registry and AppData writes are real [inference from MS: [packaging with external location](https://learn.microsoft.com/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps), and from AnyFSE disabling registry virtualisation anyway].
- **Its own rights.** It runs at the user's ordinary rights unless *Run as administrator* is set, which goes through the task.

**What Windows itself does with a home app** [MS: [GDK, Game launchers and handhelds][gdk]]:

- It suppresses the taskbar, "some startup apps, and other services".
- It returns to the game or app after sleep, "if the games and apps are closed … to the default gaming app".
- It starts the home app at power-on.
- It launches games and apps full screen.

**Undocumented, reported:** when the last visible window closes, Windows starts the home app again. The author calls this the "Explorer.exe watchdog" (#23). That is how a crash or a plain exit of the Dashboard comes back, through AnyFSE. It is also why AnyFSE needs its *Leave … on home app exit* option to get to the desktop at all. That option was broken from 0.90.18 to 0.90.23, which left a Steam user stuck on the splash. 0.90.24 says it is fixed (#122).

## 4. The Guide button, Game Bar, Task view and the Xbox app

| | In Xbox mode, any home app | What it means for the Dashboard |
|---|---|---|
| Guide, short press | "Windows has exclusive access to the Guide button." A short press opens Game Bar [MS: GDK]. | The Dashboard's Guide doesn't open on a tap, unless the switch below works. |
| Guide, short press, with AnyFSE's switch off ("Allow the gamepad to open Game Bar in Xbox mode") | Writes `LegacyGamepadXboxGuideButtonEnabled` = 1. The changelog says a short press then no longer opens Game Bar, and Steam can use it "in Steam Guide button chords or focus it" [AnyFSE 0.90.24 changelog, commit `91aa85c`]. | **[unverified]**: whether `XInputGetStateEx` sees bit `0x0400` then. Steam reads the pad through its own driver, so Steam working doesn't prove XInput does. Microsoft documents neither the value nor that behaviour. |
| View+Menu | Sends a Guide press to the focused app, as View, Menu and Guide together [MS: GDK]. | Works whatever the switch. The Dashboard has to read Guide-with-View-and-Menu as the Guide and ignore View and Menu in that press. |
| Guide, long press | Windows' Gaming Task View (task switcher); View there sends a Guide press to the chosen app [MS: GDK]. Since October 2026's builds it has Exit to Desktop and Power [press]. | It **collides with the Guide's 0.7 s hold for Power** (guide.md 7.1). In Xbox mode the hold must go: Power stays reachable from the Guide's Profile & system tab and from Task View's own Power. |
| Game Bar's Home button | Returns to the home app [MS: KB5070297]. That starts AnyFSE, which brings the Dashboard's window forward. | The Dashboard must be findable by AnyFSE (section 6). If the Xbox app is open, Home has jumped to it instead (#31, reported, closed). |
| Task view (Win+Tab, long Guide) | Shell UI from explorer [Bundle: xbox-mode-home-app §5]. | The Dashboard's window shows there as a card. Nothing to do. |
| Win+F11 | Enters or leaves Xbox mode [MS: KB5070297]. | A keyboard way back. |
| The Xbox app | Not started, as AnyFSE is the home app. AnyFSE kills it if Windows starts it in its place (#53). | Game Pass games still start through `ActivateApplication` (#165) **[inference]**. |

**Before Xbox mode is entered** (while it's off, at the desktop), the Guide behaves as today. The Dashboard reads the pad in the background either way [here: `crates/input`].

## 5. What Xbox mode gives beyond shell mode, and what it costs

**Gives:**

- Windows' own boot path into a full-screen app, maintained by Microsoft. There is no `Shell` value, sentinel, safety net or watchdog of ours, and a Windows upgrade can't drop it the way it drops HKCU `Shell` [Bundle: boot-shell-updates §1].
- The taskbar suppressed and some startup apps held back. Each app gets *Start at log in*, *Start when exiting to desktop* or *Off* [MS: KB5070297].
- "Windows limits background processes to improve performance while gaming" with *Enter Xbox mode on startup* on, and a restart offered to get it [MS: KB5070297].
- Back to the game or home app after sleep [MS: GDK].
- Gaming Task View with Exit to Desktop and Power, and Game Bar [press; MS].

**Costs:**

- **Explorer still runs.** Task View is its UI, and AnyFSE waits for `TaskbarCreated` [Bundle: xbox-mode-home-app §5; AnyFSE code: `MainWindow.cpp`]. So "explorer and startup apps not loaded" is half true: startup apps yes, explorer no. Microsoft publishes no memory figure. One user reported AnyFSE never got the "optimised" boot (#80). The answer was "totally redesigned", with no measurement since **[unverified]**.
- **The device form fake**, machine-wide, admin, and kept by AnyFSE until uninstall (section 2).
- **The Guide button**: a tap only if the switch works (section 4); the hold for Power gone.
- **A third party in the boot path**: AnyFSE's splash, its 60 s timeout, its window matching and its update cadence. A Windows build has broken the home-app picker before (#83).
- **Windows' Xbox logo** at the start of Xbox mode can't be replaced. Users asked to remove it (#74), so the Dashboard's Boot video comes after an Xbox loading screen **[reported]**.
- **Restarts.** "Restart for better performance", Win+Shift+F11 (AnyFSE's `/FSEReboot`) and Xbox mode's own Restart are ordinary restarts. On the TV PC they land in Limine's default, CachyOS, unless the boot is pointed at Windows (section 9).

## 6. What the Dashboard must do to be a good home app

Proposed. Nothing here is built.

1. **One start-up argument, `--xbox-mode`**, put in AnyFSE's *Custom settings* as `StartArg`. It means: AnyFSE started me. With it:
   - none of shell mode's duties: no events, no `--from-shell` log path, no topmost Start-up. AnyFSE's splash is topmost until it finds the window, then fades. Topmost Start-up would only fight Game Bar and Task View **[inference]**.
   - **AnyFSE's splash off** (logo, text, animation and video all off: a black window), so the Dashboard's own Boot video is the start-up. Start-up and Who's playing today? run as they do today.
   - The flag is a statement of intent. The Dashboard also asks Windows (`IsGamingFullScreenExperienceActive`, below) and acts on the answer, so a Win+F11 out of Xbox mode with the Dashboard still running changes its behaviour at once.
2. **Ask Windows, by `api-ms-win-gaming-experience-l1-1-0`**:
   - `IsGamingFullScreenExperienceActive`;
   - `RegisterGamingFullScreenExperienceChangeNotification`, documented [MS: GDK];
   - `SetGamingFullScreenExperience`, in the SDK's `gamingexperience.h` and used by AnyFSE, but **not documented on Learn**.

   Load them with `LoadLibrary`/`GetProcAddress` behind `IsApiSetImplemented`, as AnyFSE does. That way the gnullvm build needs no import library, and an older Windows simply has no Xbox mode. Whether `windows-rs` has bindings is **[not checked]** and doesn't matter this way.
3. **Exit to desktop leaves Xbox mode.** While Xbox mode is active, the Guide's Exit to desktop calls `SetGamingFullScreenExperience(FALSE)`, waits for the change notification, plays `PowerOff.wav`, and exits 0. Without the call, Windows starts the home app again when the window goes (section 3), and AnyFSE starts the Dashboard again. So AnyFSE's *Leave full screen experience on home app exit* stays **off**, and the Dashboard owns the decision. Whether Windows asks for confirmation here (`ExitGamingPostureConfirmation`) is **[unverified]**.
4. **A crash limit.** A panic or a lost device leaves no window, so Windows starts AnyFSE, and AnyFSE starts the Dashboard: a relaunch for free, but with no limit. The Dashboard counts starts under `--xbox-mode` that never reach Home's first frame (a file in its data folder). On the third within 5 minutes, it leaves Xbox mode at once instead of drawing anything. That is shell mode's rule (shell.md F1), with the desktop as the fall-back.
5. **Findable by AnyFSE:**
   - process `dashboard.exe`;
   - title `Xbox Series Dashboard` [here: `window.rs`];
   - a fixed window class of our own (`XboxSeriesDashboard`, by winit's Windows `with_class_name`), never winit's default, which other winit programs share.

   AnyFSE calls `ShowWindow(SW_SHOWMAXIMIZED)` only on a minimised or hidden window. The Dashboard's borderless full-screen window has to come back from that whole **[unverified on winit]**.
6. **One instance.** #165's `Local\XboxSeriesDashboard.Running` is right as it is. A second start brings the first forward. Under Xbox mode the foreground rule is stricter ("background apps can't take the foreground") [MS: GDK]. The second process has to `AllowSetForegroundWindow` the first before it exits, which #165 should already do. Check it there.
7. **The Guide in Xbox mode:**
   - Guide with View and Menu down in the same report opens the Guide, and that press's View and Menu do nothing else;
   - a plain Guide tap still opens it, if the switch lets it through;
   - the 0.7 s hold for Power is off while Xbox mode is active;
   - nothing else changes outside Xbox mode.
8. **The Launch screen (#164) is unchanged and still needed.** Xbox mode hides the taskbar but not the wallpaper or a launcher's window, and between a launcher closing and a game opening there is still a gap. Windows launches games full screen in Xbox mode [MS: GDK], which may make the backdrop show less, not more **[inference]**. #164's payload can run with Xbox mode on as one more case.
9. **Power.** Turn off and Sleep are unchanged. Restart is unchanged too, and on the TV PC it has the dual-boot problem whatever the route (section 9). Xbox mode's own Power in Task View sits beside ours.
10. **Startup apps.** Steam, Discord and the rest are set by the user in Settings, Apps, Startup (*Start at log in* to have them in Xbox mode). Nothing of ours. AnyFSE's own startup list stays empty.

## 7. Beside shell mode: one or the other

**Never both** **[inference]**:

- Xbox mode needs explorer as the shell: Task View, Game Bar's Home and the taskbar it suppresses are explorer's.
- Shell mode starts explorer late, and its watchdog would treat Windows' own relaunch of the home app as a second Dashboard.
- At a sign-in with both on, Winlogon starts `dashboard.exe --shell` before explorer. Xbox mode starts later, if at all, from an explorer that was started as a plain process.

**What Settings > System shows** (it reads, never writes):

| State | Read from | Line |
|---|---|---|
| Xbox mode at sign-in, through AnyFSE | `GamingConfiguration\GamingHomeApp` = `ArtemShpynov.AnyFSE_by4wjhxmygwn4!App` and `StartupToGamingHome` = 1 | "Xbox mode starts the Dashboard when you sign in, through AnyFSE. To change it, use Settings, Gaming, Xbox mode." |
| Xbox mode on now | `IsGamingFullScreenExperienceActive` | the line above, plus "Exit to desktop leaves Xbox mode." |
| Another home app | `GamingHomeApp` names something else, with `StartupToGamingHome` = 1 | "Xbox mode starts another app when you sign in." |

Words to be settled in X2. **Shell mode refuses** to turn on while `StartupToGamingHome` = 1, with a new refusal key `xbox-mode-startup`: "Windows starts Xbox mode when you sign in. Turn off Enter Xbox mode on startup first." Xbox mode can't refuse shell mode, because it isn't ours. If both are found on at a sign-in, `--shell` logs it and carries on as shell mode, which wins because Winlogon runs it first.

**Set up or detect?** **Detect only.**

- **Possible but not recommended:** the two `GamingConfiguration` values are HKCU and need no admin. Settings *could* write them, and AnyFSE's JSON, which is per-user.
- **Why not:**
  - The device form is HKLM and needs admin.
  - The package needs AnyFSE's installer.
  - Writing another program's configuration breaks when it changes its format.
  - Windows' own page already owns the two values.
- **What Settings offers instead:** detection, and a line pointing at `docs/research/xbox-mode-anyfse.md`'s steps (or the user guide, when there is one).

## 8. The three routes

| | Plain start-up app | Shell mode (#154) | Xbox mode through AnyFSE |
|---|---|---|---|
| What starts it | Explorer's startup run, after the desktop | Winlogon, in place of explorer | Windows' Xbox mode starts AnyFSE, which starts the Dashboard |
| Desktop seen at sign-in | Yes: the wallpaper, the taskbar and startup apps first | No: a black window, then the Boot video | No: Windows' Xbox loading screen, then AnyFSE's black, then the Boot video [reported] |
| Explorer | Runs from the start | Started behind the Boot video | Runs, with the taskbar hidden |
| Startup apps | All, about 10 s after sign-in | All, when explorer starts | Only those set to *Start at log in* |
| Guide button | Ours | Ours, and it brings the Dashboard back after Exit to desktop | Game Bar's. Ours by View+Menu, and by a tap only if the Game Bar switch works [unverified]. No Power hold. |
| Crash | Nothing restarts it | Our watchdog: relaunch, then the desktop after 3 in 5 min | Windows and AnyFSE relaunch it. Our own count leaves Xbox mode after 3 (section 6). |
| Exit to desktop | Exits | Exits; explorer is up | Leaves Xbox mode, then exits |
| Admin | No | No | Yes, once (`DeviceForm`, the package) |
| Machine-wide change | None | None (HKCU only) | `DeviceForm` = 46: the PC claims to be a handheld |
| Third party | None | None | AnyFSE (MIT, one maintainer) |
| Survives a Windows upgrade | Yes | `Shell` may drop; the sentinel puts it back | Unknown: the home-app picker vanished once (#83) |
| Way back | Untick it | Settings, or `--shell-off`, or Ctrl+Shift+Esc | Win+F11, then Settings, Gaming; uninstall AnyFSE to undo `DeviceForm` |
| Built | Nothing to build | PR #165 | Section 6, small (X1, X2) |
| Tested | — | VM payload P1–P9 | Nothing yet (section 10) |

## 9. The user's steps on the TV PC

**These are the user's steps, to take only if they choose to.** Nothing in this ticket does them. Marks:

- **[REG]** writes the registry;
- **[ADMIN]** needs an administrator;
- **[RESTART]** needs a restart. The TV PC dual-boots through Limine, which defaults to CachyOS, so every restart must be pointed at Windows: pick Windows in Limine at the TV, or set the firmware's next boot first. `bcdedit /enum firmware` lists the entries, then `bcdedit /set {fwbootmgr} bootsequence {<Windows Boot Manager id>}` as admin. That route is untested on this PC.

Have a keyboard at hand the first time.

**Before:**

1. Make sure the Dashboard is installed where it will stay (the install folder from #155, or a folder you won't move). AnyFSE stores the full path.
2. If shell mode is on, turn it off in Settings > System (or `dashboard.exe --shell-off`) **[REG]**. If the Bundle's Boot shell is set up, `XboxSeries.BootShell.exe --uninstall` **[REG]**. Restart once into Windows and check you get the normal desktop **[RESTART]**.
3. Note what's there now, so the way back is exact. In a terminal, run `reg query "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\OEM"`, `reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\GamingConfiguration"` and `reg query "HKCU\Software\Microsoft\GameBar"`. Save the output.

**Turning it on:**

4. Settings, Gaming, Xbox mode: if *Enable Xbox mode* is there and off, turn it on **[REG]**.
5. Get AnyFSE 0.90.24 or later, from its [GitHub releases](https://github.com/ashpynov/AnyFSE/releases) or the Microsoft Store. Run `AnyFSE.Installer.exe` **[ADMIN] [REG]**. It writes `DeviceForm` = 46 and the rest of section 1's table.
6. Restart into Windows **[RESTART]**. Whether the device form needs it before *Choose home app* shows is **[unverified]**; assume yes.
7. Settings, Gaming, Xbox mode: *Choose home app* = **AnyFSE**; *Enter Xbox mode on startup* **on** **[REG]**. If *Choose home app* isn't there, stop here and undo (steps 13–15): the device form alone isn't enough on this build.
8. Start menu, AnyFSE, right-click, *Configure*:
   - Launcher: a custom executable, `dashboard.exe` in the install folder.
   - *Custom settings*: `StartArg` empty until X1 adds `--xbox-mode`, then `--xbox-mode`. `ProcessName` `dashboard.exe`. `WindowTitle` `Xbox Series Dashboard`. `ClassName` empty until X1 sets `XboxSeriesDashboard`.
   - *Run as administrator*: off.
   - Splash: logo, text, animation and video all off.
   - *Leave full screen experience on home app exit*: **on** until X1 is built (otherwise Exit to desktop just comes back), then **off**.
   - *Allow the gamepad to open Game Bar in Xbox mode*: try both. Off writes `HKCU\Software\Microsoft\GameBar\LegacyGamepadXboxGuideButtonEnabled` = 1 **[REG]**.
   - Startup apps: none. Updates: as you like.
9. Settings, Apps, Startup: for Steam, Discord and anything else, choose *Start at log in* (in Xbox mode) or *Start when exiting to desktop* **[REG]**.
10. Restart into Windows **[RESTART]**. Expect the sign-in, Windows' Xbox loading screen, black, then the Boot video.

**Checking it:** a Guide tap; View+Menu; a long Guide press (Task View); Game Bar's Home from a game; a Steam game and a Game Pass game; sleep and wake; Exit to desktop; the Dashboard's Restart (it will land in Limine).

**The way back:**

11. Win+F11 (or Task View, Exit to Desktop) leaves Xbox mode now.
12. Settings, Gaming, Xbox mode: *Enter Xbox mode on startup* off; *Choose home app* Xbox or None **[REG]**. Do this before uninstalling.
13. Settings, Apps, AnyFSE, Uninstall **[ADMIN] [REG]**. It puts `DeviceForm` back from its saved copy. Check with step 3's `reg query`. If `DeviceForm` is still 46 and wasn't before: `reg delete "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\OEM" /v DeviceForm /f` **[ADMIN] [REG]**.
14. If the uninstall breaks, use the README's clean-up (section 1) **[ADMIN] [REG]**.
15. Set `LegacyGamepadXboxGuideButtonEnabled` back as step 3 found it, or delete it **[REG]**. Restart into Windows **[RESTART]**.
16. **If it goes wrong at sign-in** (a black screen, a stuck splash): Win+F11. If that does nothing, Ctrl+Shift+Esc, Run new task, `ms-settings:gaming-xboxmode`. The URI is a guess; Settings, Gaming works too. Turn off *Enter Xbox mode on startup*. Signing in with another account is unaffected by the per-user values, though the device form still applies.

## 10. What can be tried on the win11-e2e VM first

The lead runs the VM, one at a time. These are for a payload or a `--keep` run. Downloading and installing AnyFSE on the VM is the lead's decision. It is a third-party binary from the internet.

- **V1 Unlock.** On the VM's build, does `DeviceForm` = 46 alone (AnyFSE's installer) bring *Choose home app* and *Enter Xbox mode on startup*, with or without a restart? Read `GamingConfiguration` before and after.
- **V2 Install footprint.** Diff the registry, tasks, services, certificate stores and `Program Files` before and after against section 1's table. Then uninstall and diff again: is `DeviceForm` back, and are `GamingHomeApp` and `StartupToGamingHome` left behind?
- **V3 Start.** With the Dashboard as the custom launcher and `--log-focus`: restart; Xbox mode at sign-in; AnyFSE starts `dashboard.exe`; the splash goes when the window shows; time from sign-in to first frame. Compare with shell mode's P3. WARP, so only the order counts, not the times.
- **V4 Home.** From a stand-in game, Game Bar's Home (by keyboard, Win+G) brings the Dashboard forward, with no second process.
- **V5 Exit and crash.** Exit to desktop with and without AnyFSE's *Leave … on exit*. Kill the Dashboard: does Windows relaunch AnyFSE, how fast, and does it loop? (#23 says it relaunches.)
- **V6 The Guide.** With the ViGEm virtual pad, which `e2e` already uses: what `XInputGetStateEx` reports for Guide in Xbox mode, with `LegacyGamepadXboxGuideButtonEnabled` 0 and 1, and for View+Menu. A virtual pad may not be treated as a real one by Game Bar **[unverified]**, so V6 is a hint, not an answer.
- **V7 Both routes.** With shell mode on and `StartupToGamingHome` = 1, what happens at sign-in. This proves the refusal is needed.

Not provable on the VM: real Xbox pads (USB and wireless), the 4K TV and HDR, the RX 9070 XT, Game Pass with an account, the dual-boot restart, and whether a real game picks handheld settings.

## 11. Open questions only the TV PC can answer

1. Does `DeviceForm` = 46 alone bring *Choose home app* on the TV PC's current build (26300.x, 26H2, Pro)?
2. With *Allow the gamepad to open Game Bar* off, does a Guide tap from a real Xbox pad reach `XInputGetStateEx`? Wired, and over the Xbox wireless adapter or Bluetooth.
3. Does any installed game, or Steam Big Picture, change its behaviour because the PC now says it's a handheld?
4. Is the cold boot to the Dashboard's first frame faster or slower than shell mode's? And what does memory look like at Home in each (Task Manager, committed)?
5. Do Discord and Steam behave when started as *Start at log in* in Xbox mode? Does the Dashboard's Discord tab still see Discord?
6. Does Xbox mode's "Restart for better performance" (or Win+Shift+F11) land in CachyOS, and can Windows' own restart be pointed at Windows at all from the sofa?
7. HDR and 4K: does entering or leaving Xbox mode change the display mode or HDR state under the Dashboard?
8. Does Windows ask before leaving Xbox mode when `SetGamingFullScreenExperience(FALSE)` is called (`ExitGamingPostureConfirmation`)?

## Sources

- **AnyFSE** at [`0e94178`][anyc]:
  - `README.md`, `LICENSE`, `AppxManifest.xml`, `CustomCapability.SCCD`
  - `src/App/`: `Main.cpp`, `App.cpp`, `CmdLine.cpp`, `Constants.hpp`, `GamingExperience.cpp`, `ExitFSE.cpp`, `Launchers.cpp`, `MainWindow.cpp`, `MainWindow_Timer.cpp`, `MainWindow.hpp`
  - `src/Tools/`: `Process.cpp`, `Paths.cpp`, `ShellWindow.cpp`
  - `src/AppInstaller/`: `AppInstaller_Install.cpp`, `AppInstaller_Page.cpp`, `AppUninstaller.cpp`, `ScheduledTask.cpp`
  - `src/AppSettings/`: `SettingsPages/LauncherPage.cpp`, `DetectLaunchers.cpp`
  - `src/Configuration/Config.cpp`
  - `localization/en_US.json` and `media/Store/Store listing AnyFSE.csv` (the 0.90.24 changelog); commit `91aa85c`
  - Issues [#23](https://github.com/ashpynov/AnyFSE/issues/23), [#31](https://github.com/ashpynov/AnyFSE/issues/31), [#44](https://github.com/ashpynov/AnyFSE/issues/44), [#53](https://github.com/ashpynov/AnyFSE/issues/53), [#74](https://github.com/ashpynov/AnyFSE/issues/74), [#78](https://github.com/ashpynov/AnyFSE/issues/78), [#80](https://github.com/ashpynov/AnyFSE/issues/80), [#82](https://github.com/ashpynov/AnyFSE/issues/82), [#83](https://github.com/ashpynov/AnyFSE/issues/83), [#101](https://github.com/ashpynov/AnyFSE/issues/101), [#107](https://github.com/ashpynov/AnyFSE/issues/107), [#109](https://github.com/ashpynov/AnyFSE/issues/109), [#120](https://github.com/ashpynov/AnyFSE/issues/120), [#122](https://github.com/ashpynov/AnyFSE/issues/122)
  - Releases v0.90.15 to v0.90.24
- **Related tools:** [driver1998/FullScreenExperienceShell](https://github.com/driver1998/FullScreenExperienceShell) (MIT, proof of concept, last push 2026-02-25); [8bit2qubit/XboxFullScreenExperienceTool](https://github.com/8bit2qubit/XboxFullScreenExperienceTool) (GPL-3.0, v0.9.4, 2026-05-14, release notes on the "Limited PC edition"); [8bit2qubit/OmniConsole](https://github.com/8bit2qubit/OmniConsole) (the same idea as a full home app, with its own Game Bar widget).
- **Microsoft**, read 2026-11-17:
  - [Windows Gaming: Full screen experience (KB5070297)][kb]
  - [GDK: Game launchers and handhelds (2610)][gdk]
  - [Grant package identity by packaging with external location](https://learn.microsoft.com/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps)
  - [Policy CSP, Games: DisableGamingFullScreenExperience](https://learn.microsoft.com/windows/client-management/mdm/policy-csp-games#disablegamingfullscreenexperience)
  - [Unattend: StartupToGamingHome](https://learn.microsoft.com/windows-hardware/customize/desktop/unattend/microsoft-windows-gaming-configuration-startuptogaminghome)
- **Press:**
  - [Pureinfotech: force-enable Xbox mode on any PC (2026-08-13)](https://pureinfotech.com/windows-11-enable-xbox-full-screen-experience-any-pc/)
  - [HelpDeskGeek: controller-friendly Gaming Task View (2026-10-25)](https://helpdeskgeek.com/microsoft-tests-controller-friendly-gaming-task-view-windows-11/)
  - [XDA: Winhanced and Xbox mode](https://www.xda-developers.com/winhanced-getting-better-i-still-prefer-using-xbox-mode-with-windows-11-handheld/)
- **The Bundle:** [xbox-mode-home-app.md][xm], [ADR 0002][adr], [#109](https://github.com/snrgarb/xbox-series-theme/issues/109) (XFSET removed; Settings > Gaming on the stock form).
- **This repository:** [shell.md](shell.md), [standalone-rust-xbox-mode.md](standalone-rust-xbox-mode.md), [../spec/guide.md](../spec/guide.md) 7.1 and 13, `crates/input/src/lib.rs`, `crates/app/src/window.rs`; PR #165's `docs/spec/shell-mode.md`; #154, #164.

## Follow-up tickets (drafted, not created)

**X1. The Dashboard as Xbox mode's home app** (`wayfinder:task`, M7)

> After #180's research (`docs/research/xbox-mode-anyfse.md`, section 6). Under Xbox mode, started by AnyFSE:
>
> - `--xbox-mode`: none of shell mode's duties; Start-up as today.
> - The Xbox mode API through `api-ms-win-gaming-experience-l1-1-0`, loaded at run time: is it active, and the change notification.
> - Exit to desktop calls `SetGamingFullScreenExperience(FALSE)` while Xbox mode is active, then exits 0.
> - Three starts that never reach Home within 5 minutes: leave Xbox mode.
> - A fixed window class `XboxSeriesDashboard`.
> - While Xbox mode is active: the Guide on Guide-with-View+Menu, and no Power hold.
> - `--log-focus` and a `xbox-mode:` log line for each of these.
>
> Tested on the win11-e2e VM with AnyFSE installed by the lead (V3–V6). **Needs a human:** the TV PC trial (section 9).
>
> **Done when:** a VM payload starts the Dashboard through AnyFSE at sign-in, Exit to desktop lands on the desktop with Xbox mode off, a crash on purpose three times leaves Xbox mode, and guide.md and start-up.md say what changes in Xbox mode.

**X2. Settings shows Xbox mode, and shell mode refuses beside it** (`wayfinder:task`, M7, after #154)

> - Settings > System reads `GamingConfiguration` and the Xbox mode API and shows the lines in section 7. It writes nothing.
> - Shell mode gets the refusal `xbox-mode-startup`.
> - `--shell` logs it if both are found on at a sign-in.
>
> **Done when:** screenshot tests for each line, a `shell-mode` unit test for the refusal through `system::Fake`, and the spec.

**X3. VM spike: AnyFSE on win11-e2e** (`wayfinder:research`, for the lead)

> Section 10's V1–V7 on the VM, with AnyFSE 0.90.24, and the diffs kept as artefacts. Answers whether the device form alone unlocks the home app on the template's build and what the install really writes. Run before X1's payload.

**X4. ADR: Xbox mode through AnyFSE is an opt-in route for the port** (`wayfinder:decision`, only if the user keeps it after trying it)

> Records that the port supports Xbox mode as a second, user-chosen route beside shell mode, and what it gives up: the handheld fake, the Guide hold, and a third-party home app. It does not change the Bundle's ADR 0002, which was about the Bundle's own PC.
