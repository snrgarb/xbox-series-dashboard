# Installing and uninstalling every store's games from the Dashboard alone

Research for [#166](https://github.com/snrgarb/xbox-series-dashboard/issues/166). The user's question: "how we can manage installing/uninstalling for EACH game launcher ONLY via the dashboard. search github for alternative solutions."

It is answered two ways:

- **A. The games** (the main one): for each store, how the Dashboard installs and uninstalls one game with the pad, with no launcher window ever shown, and with bytes, speed, pause and cancel in its own Queue.
- **B. The launchers themselves**: how a new PC could get Steam, Epic, GOG Galaxy, the EA app, Ubisoft Connect, Battle.net and the Xbox app from the couch, and what removing one does to its games.

Read on 17 November 2026, on Linux. **No store client was run, nothing was installed and nothing was signed in to**; no VM and no TV PC. What follows is other projects' source code, their issues, official documentation and the stores' terms. It builds on [stores.md](stores.md) (where each store keeps its games) and [../sources.md](../sources.md) (what the Dashboard does today), and does not repeat them. Tags, as in stores.md:

- **[code]** read in the source of a program that does it, linked at the commit or date read.
- **[docs]** the store's or Microsoft's own documentation, or a project's README about itself.
- **[reported]** an issue, a forum, one project's notes: not confirmed by a second.
- **[inferred]** my reasoning. A guess until a signed-in PC tests it.

Terms of service are quoted where the text could be read; where only a search excerpt or a mirror could be, it says so. Whether something breaks a store's terms is **my reading, not legal advice**: every such line is [inferred].

## Summary

Today (sources.md, the M4 and M5 reports): Steam installs and uninstalls with no window and live progress; Epic shows the launcher's dialog and cannot uninstall from outside; GOG, EA, Ubisoft, Battle.net and Xbox open their client, a game's uninstaller or Windows' Settings, and never join the Queue.

| Store | Recommended way | Only via the Dashboard, no launcher window? |
| --- | --- | --- |
| **Steam** | Steam's own `SteamClient.Installs` and `SteamClient.Downloads` over the CEF debugger the Dashboard already uses: install to a chosen library, uninstall with no prompt, bytes, speed, pause, resume, cancel, queue order. `steam.exe -silent` stays the fallback with the debugger off | **Fully**, with the debugger on (the user's to turn on). Without it, as today: Steam's window is hidden as it comes |
| **Epic** | **legendary**, an open command-line Epic client (GPL-3.0), run as a separate program the Dashboard drives; its own sign-in. The launcher's link stays for users who decline | **Fully, outside the launcher**: the official launcher does not see games legendary installs, and those games start through legendary. Partly through the launcher (its dialog; no uninstall) |
| **GOG** | **gogdl** (Heroic's GOG downloader, GPL-3.0) as a separate program, then the game's own `scriptinterpreter`; `unins000.exe /VERYSILENT` to uninstall. GOG's offline installers run `/VERYSILENT` as the plainest alternative | **Fully**, but for one Windows permission prompt for games whose set-up writes the machine's registry. Galaxy is never needed |
| **EA app** | `origin2://game/launch?offerIds=<id>&autoDownload=1` with the app's window shown, then the folder watched; uninstall by the key's `Cleanup.exe` | **No.** Only by reimplementing EA's downloader with the EA app's own credentials (Maxima, efxlve): not recommended |
| **Ubisoft Connect** | `uplay://install/<id>` and `uplay://uninstall/<id>`, as today | **No.** The only windowless tools are alternative clients Ubisoft's terms forbid by name |
| **Battle.net** | The Battle.net **Agent's local HTTP API** (the client's own downloader, which runs without the client's window): install to a chosen folder, uninstall, bytes and speed | **Fully, if the Agent answers** as two open tools say it does. Undocumented; a signed-in PC must confirm. Pause and cancel unconfirmed |
| **Xbox / Game Pass** | Windows' `AppInstallManager` (the Store's own install queue) to install with bytes, pause, cancel and a drive; `PackageManager.RemovePackageAsync` to uninstall | **Possibly fully.** Both are Windows APIs; whether the install call takes an owned or Game Pass game from an unpackaged program, and whether removing a GDK game cleans up, only a signed-in PC can say. Otherwise partly: the Store's page to install |

**The recommendation**, in one line: drive each store through **its own client's channel** where one exists (Steam's CEF, Battle.net's Agent, Windows' Store queue), through **an open helper program** the user chooses to add where the client has none (legendary for Epic, gogdl for GOG), and **open the client** where nothing else is safe (EA, Ubisoft). Every route feeds the Queue already built (`Source::installs`, `docs/spec/manage-queue.md`).

**The biggest risks:**

1. **Every windowless route but Xbox's is undocumented.** Steam's `SteamClient` object, Battle.net's Agent, Epic's and GOG's download APIs are all reverse-engineered by others and can change with a client update. Each needs a fallback to today's route and a log line that says which was taken.
2. **Epic through legendary splits the library**: the launcher does not see those games (legendary's launcher sync knows only the old launcher's files, [legendary#797][lg-797]), and they must start through legendary. A user who also opens the Epic launcher sees them "not installed".
3. **Sign-in.** legendary and gogdl need the user's Epic or GOG account, signed in through a page of the store's in the Dashboard. Both borrow the official client's application id (as Heroic, Playnite's community plugins and Lutris do); the stores could stop that.
4. **Terms.** Steam's and Blizzard's terms speak of "unauthorized third-party software" controlling their client; Ubisoft's forbid "alternative launchers". No store is reported to have acted against a launcher of this kind (Playnite, Heroic, Decky, GameShelf), but none has said it may. Section 9.

## 1. What exists already, and what "the Queue" needs

- `model::Source` (`crates/model/src/source.rs`): `ask(game, Ask::Install | Ask::Uninstall) -> Option<Start>` gives an address or a program to open; `installs() -> Option<Vec<Install>>` gives what the client has under way, read from local files once a second; `uninstalls_by_itself()` says whether the uninstall needs the user. The Queue (`library::queue`, `model::queue`) holds `Queued`, `Downloading { done, total }` and `Paused { done, total }`, with an optional store speed.
- Steam (`shell::steam`, `source_steam::cef`, `overview`, `follow`): `app_install` and `app_uninstall` typed into Steam's console over its CEF debugger, else `steam.exe -silent`, with Steam's windows hidden as they come; progress from the manifests, the registry and Steam's download overview.
- Manage Queue has no Pause or Cancel: the Bundle's has none (`docs/spec/manage-queue.md` section 5). The console has both in a queued game's menu. Where they would go is a decision for the user (open question 1).

So "only via the Dashboard" needs, per store: a way to start an install that shows nothing, a way to read its progress, a way to pause and cancel it, and a way to uninstall that asks nothing. The sections below take each in turn.

## 2. Steam

| Approach | Install | Uninstall | Progress, pause, cancel | Needs | Steam sees it; DRM, cloud, achievements | Alive; licence | Risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **A. `SteamClient.Installs` and `.Downloads` over the CEF debugger** | Yes, to a chosen library folder, shortcuts off, without a click [code] | Yes, `OpenUninstallWizard([id], true)`: no prompt [code] | Bytes done and total per app, network and disk speed, seconds left, `PauseAppUpdate`, `ResumeAppUpdate`, `RemoveFromDownloadList`, `SetQueueIndex` [code] | Steam running and signed in; the debugger file (the user's) | Yes: Steam does the install | Steam's private web API; typed by decky-frontend-lib (LGPL-2.1, 2026-11-01) and Millennium (MIT). Nothing to ship: only calls | Medium: renamed calls with Steam updates (it already gained a client-id argument, below); Steam's install window may still flash and is hidden by the watch that exists |
| B. Steam's remote downloads (`IClientCommService`) | Yes, by app id [code: proto] | Yes [code: proto] | Bytes, rate, seconds left, paused; `SetClientAppUpdateState` likely pauses [code; inferred] | Steam running on this PC, and a Steam web token for the account | Yes | Valve's own "install on my PC" from the phone; the API undocumented | Medium: a web token to hold. May show no window at all [inferred] |
| C. `steam.exe -silent +app_install` / `+app_uninstall`, `steam://install/<id>` | Yes | Yes | Only from the manifests | Steam installed | Yes | Built in | Low; Steam's window comes and is hidden (today's route) |
| D. steamcmd `+login +force_install_dir +app_update <id>` | Yes, into a library root | Unclear | Percent lines on stdout | The password and a Steam Guard code typed into steamcmd; no QR | Only after Steam restarts [reported] | Valve's binary; bundling it not allowed [inferred] | High in practice: signing steamcmd in with the same account signs the client out [reported] |
| E. DepotDownloader / SteamKit2, Aurelia | Files only; no appmanifest (DepotDownloader); Aurelia is a whole headless Steam client | No / yes | Their own | Their own sign-in (QR in both) | No, unless the manifest is written by hand and Steam restarted | GPL-2.0 / LGPL-2.1; Aurelia GPL-3.0, created 2026-06, now a Heroic backend [code] | High: a second, unofficial Steam session. Aurelia's README carries its own ban disclaimer [docs] |

- **The calls**, from decky-frontend-lib's types [code: [`Installs.ts`, `InstallFolder.ts`][dfl]] and a Windows launcher that uses them end to end, GameShelf's [`SteamInstaller.cs`][gs] (MIT, 2026-10-02) [code]:
  - Install: `Installs.SetAppList([id])`, wait for `GetInstallManagerInfo()` to give a size (it also gives the disk space needed and free), `OpenInstallWizard([id])`, wait for the wizard's state, `SetInstallFolder(index)` (the index from `InstallFolder.GetInstallFolders()`, each with its path, free space and whether it is the default), `SetCreateShortcuts(false, false)`, `ContinueInstall()`.
  - Uninstall: `Installs.OpenUninstallWizard([id], true)`; the `true` skips Steam's prompt (found by trial in GameShelf).
  - Pause, resume, cancel: `Downloads.PauseAppUpdate(id, "0")`, `ResumeAppUpdate(id, "0")` then `EnableAllDownloads(true, "0")`; cancel is pause, `RemoveFromDownloadList(id, "0")`, then uninstall to delete what was fetched. The second argument, the client's id (`"0"` this PC), came with Steam's remote downloads management (beta April 2026, [GamingOnLinux][gol-remote]); GameShelf's comment says Steam ignores the call without it. The decky types do not have it yet.
  - Per download, `RegisterForDownloadItems` gives `downloaded_bytes`, `total_bytes`, `paused`, `queue_index`; the overview the Dashboard already reads gives the speeds and `update_seconds_remaining`.
  - The same pattern: [opensu][opensu] (MIT, 2026-11-16) and [OmniDash][omnidash] (MIT, a console-style launcher for Windows, 2026-11-05) [code, as reported by the research].
- **A window still comes.** GameShelf hides "Steam's install window and its main window" as they come, as `shell::steam` does. Whether `SetAppList` and `ContinueInstall` work without `OpenInstallWizard`, or the wizard can be kept from showing, is open question S1.
- **Seen on the TV PC (2026-11-17, #171), correcting the above:** Steam's own script (`sp.js`) passes the client id `"0"` to every one of these calls, and a call without it returns `undefined` and does nothing: the install is `OpenInstallWizard([id], "0")` (state 0 to 7), `SetInstallFolder`, `SetCreateShortcuts(false, false)`, `ContinueInstall()` (state 14), with no `SetAppList`; the uninstall `OpenUninstallWizard([id], "0", true)`; resume needs no `EnableAllDownloads`. The wizard does show Steam's Install dialog and main window. docs/sources.md ("Steam's own calls") has what is relied on now.
- **Remote downloads** ([`service_clientcomm.proto`][sp-cc]): `InstallClientApp`, `UninstallClientApp`, `GetClientAppList` (with `bytes_downloaded`, `bytes_download_rate`, `estimated_seconds_remaining`, `download_paused`), `SetClientAppUpdateState`, `EnableOrDisableDownloads`, called as `IClientCommService/<Method>/v1?access_token=...` [code: [XasYer/steam-plugin][xs-cc], MIT]. It is the official feature behind the phone's "install on my PC", so it may show nothing on the PC at all; but it needs a web token for the account, which means a sign-in of the Dashboard's own (Steam's QR sign-in, as SteamKit2's sample shows [code: [SteamKit `001_AuthenticationWithQrCode`][sk-qr]]). Worth one test (S2); not the default.
- **Steam Subscriber Agreement** (revision of 10 September 2026, [link][ssa], as quoted by the research): section 4.B, "You may not use Cheats, mods, hacks, or any other unauthorized third-party software, to modify any Subscription Marketplace process, the process of Steam account creation or otherwise in interacting with or controlling the processes or user interface of Steam Content and Services, except to the degree expressly permitted"; 4.C, "You may not use any form of scripts, bots, macros, or other non-human-controlled systems ("Automation") to interact with Content and Services on Steam"; 2.G forbids reverse engineering and to "emulate or redirect the communication protocols used by Valve". My reading [inferred]: one press of the user's becoming one call to Steam's own client is not a bot, and nothing is emulated; "controlling the ... user interface" is the grey part, and Decky, Millennium and the Bundle have done it openly for years with no enforcement reported. D and E are the exposed ones (a second client, 2.G).

**Recommendation: fully possible.** Extend `source_steam::cef` from `ExecCommand('app_install')` to the `Installs` and `Downloads` calls: a chosen library, pause, resume, cancel and queue order in the Queue, and uninstall with no prompt. Keep `steam.exe -silent` as the route with the debugger off, as now. Do not use steamcmd, DepotDownloader or Aurelia.

## 3. Epic Games

| Approach | Install | Uninstall | Progress, pause, cancel | Needs | Launcher sees it; DRM, cloud, achievements | Alive; licence | Risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **A. legendary**, a separate program (`legendary -y install <AppName> --base-path <dir> --skip-sdl`) | Yes, with update, repair, DLC, any drive [code] | Yes (`uninstall`), for the games it knows; a launcher-installed game must be `import`ed first, which verifies it whole [docs] | Bytes, speed, disk speed, ETA on stderr about once a second; pause = stop the process, resume = run `install` again (a `.resume` file); cancel = stop and delete it [code] | `legendary_windows_x64.exe`; its own sign-in (`user.json`, which `LEGENDARY_CONFIG_PATH` moves into the Dashboard's folder) [code]; no admin but for a game's prerequisites | **The current launcher does not see it** (below). The game starts through legendary, which passes an exchange code, so ownership and online checks pass; cloud saves by `sync-saves`; the EOS overlay by `eos-overlay` [code; achievements inferred] | 0.21.1 (2026-09-08), last commit 2026-11-04, about 97 open issues; moved to the `legendary-gl` organisation in April 2026 [reported]. GPL-3.0 | Uses the launcher's own application id; Epic broke its sign-in in 2022 [reported] |
| B. `egs-api` (Rust, MIT) and our own downloader | Only if we write chunks, assembly and patching | We delete the files | Ours | As A | As A | 0.14.0, 2026-02 | As A, plus the maintenance |
| C. The launcher's link, `com.epicgames.launcher://apps/...?action=install` (today) | The launcher's dialog | Nothing from outside: no link or command found [reported: [Epic help][epic-uninstall]] | Only "not done" from the `.egi` records | The launcher, signed in | Yes | - | None; not windowless |

- **Progress lines** (`downloader/mp/manager.py`) [code: [legendary `170964e`][lg]]:

  ```
  = Progress: 12.34% (123/999), Running for 00:01:02, ETA: 00:08:00
   - Downloaded: 512.00 MiB, Written: 600.00 MiB
   + Download	- 25.00 MiB/s (raw) / 40.00 MiB/s (decompressed)
   + Disk	- 38.00 MiB/s (write) / 0.00 MiB/s (read)
  ```

  The percentage counts chunks; "Download size" comes first. Heroic ([`storeManagers/legendary/games.ts`][heroic]) and Playnite's community Legendary plugin ([`LegendaryDownloadLogic.cs`][pl-leg], MIT, 1.17.1, 2026-11-15) both read these lines with patterns, and both stop the process to pause or cancel (the plugin first gently, then by force) [code].
- **The launcher does not see it.** `legendary egl-sync` writes only the old launcher's `Data\Manifests\*.item` (`lfs/egl.py`) [code]; [legendary#797][lg-797] (open, 2026-10-20): the new launcher "no longer uses `C:\ProgramData\Epic\EpicGamesLauncher\Data`". The TV PC's launcher records installs in `InstallHelper\InstalledItems\*.egi` (stores.md); writing those ourselves would be guessing at Epic's private format. So a legendary install is the Dashboard's (and legendary's) alone: `source-epic` has to read legendary's `installed.json` as one more record, and start those games through legendary (`legendary launch <AppName>`, or its `--dry-run` command line) [inferred].
- **Prerequisites.** With `-y` on Windows legendary skips a game's prerequisite installers (`cli.py`) [code]; the Dashboard would run them, and some raise a permission prompt [inferred].
- **Sign-in** (`legendary auth`) opens a web view on Windows. Without it: `auth --disable-webview --code <code>`, the code from Epic's page `https://www.epicgames.com/id/api/redirect?clientId=34a02cf8f4414e29b15921876da36f9a&responseType=code` [code]. The Dashboard can show that page in a web view of its own and catch the code (as Playnite's Legendary plugin does in Playnite's, `EpicAccountClient.cs` [code]). Epic also has a device-code flow (a short code and an `epicgames.com/activate` address: a QR code to a phone), documented by the community [reported: [Fortnite endpoints docs][epic-device]], but only under the application id of one of Epic's own programs: borrowing it is impersonation, not recommended. `auth --import` takes the launcher's sign-in and signs the launcher out [docs]; with the new launcher, unknown.
- **Epic's terms** (store EULA, last updated 2025-01-15, [link][epic-eula]; read from search excerpts only, the page asks for sign-in): a "personal, non-exclusive, non-transferable, non-sublicensable limited right and license to use the Software and its associated Services for your personal, non-commercial use"; you may not "... (f) remove, disable, circumvent, or modify any proprietary notice or label or security technology included in it; (g) create, develop, distribute, or use any unauthorized software programs to gain ...". My reading [inferred]: (g) is aimed at cheats; nothing names other launchers, and Epic is not reported to have acted against legendary or Heroic in five years.

**Recommendation: fully possible outside the launcher, partly through it.** Offer legendary as the Epic route the user turns on (section 10), with the launcher's link kept for those who do not. Say plainly in Settings that the Epic launcher will not list games installed this way.

## 4. GOG

| Approach | Install | Uninstall | Progress, pause, cancel | Needs | Galaxy sees it; DRM, cloud, achievements | Alive; licence | Risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **A. gogdl**, a separate program (`gogdl --auth-config-path <file> download <id> --platform windows --path <dir> --with-dlcs`), then the game's `scriptinterpreter.exe /VERYSILENT` | Yes, v2 and v1 builds, delta updates, `repair` [code] | No command: the game's `unins000.exe /VERYSILENT` where there is one, then the folder | legendary's line format; a `.gogdl-resume` file; pause = stop, resume = run again [code] | `gogdl_windows_x86_64.exe`; its own sign-in (`auth.json`, wherever we say); **admin for the script interpreter** (Heroic runs it `-Verb RunAs`) [code] | GOG games are DRM-free. The interpreter writes the game's registry keys, so the Dashboard's GOG source (and probably Galaxy) sees it [inferred]; cloud saves by `save-sync`; achievements need Galaxy or Comet | 1.4.0 (2026-10-28); open #103 (a failed update loses the resume file), #102 (hangs) [reported]. GPL-3.0 | Galaxy's application id and secret, from gogdl's `auth.py`; not an official API |
| B. Our own downloader, as Playnite's GOG OSS plugin has (C#, about 2,900 lines) | Yes | As A | Ours, exact | As A | As A | GOG OSS: MIT, 1.12.0, 2026-11-15 [code] | As A, plus the maintenance |
| C. GOG's offline installers, fetched with the user's sign-in, run `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR=<dir>` | Yes, the whole installer; no delta updates | `unins000.exe /VERYSILENT /ProductId=<id> /KEEPSAVES` (as GOG OSS runs it) [code] | The download ours; the install step none (the folder can be watched) | Admin very likely (`HKLM\...\GOG.com\Games`) [inferred] | As a Galaxy install [inferred] | GOG's own product | **Lowest**: the user's own installers used as GOG means them to be |
| D. `GalaxyClient.exe /gameId=<id> /command=installGame` (today) | Starts Galaxy's install; whether silent, unknown | No `uninstallGame` anywhere [reported] | None | Galaxy, signed in | Yes | - | None; probably shows Galaxy [inferred] |
| E. lgogdownloader | Installers and Galaxy builds | No | Text | Its own sign-in | As C | WTFPL, 3.20 (2026-11-11); Linux builds only | Low; no Windows build |

- gogdl's README: "**not** user friendly cli, it's meant to be used by some other application" [docs, [heroic-gogdl `6c6ef53`][gogdl]]. Heroic on Windows, after a GOG download, runs `__redist\ISI\scriptinterpreter.exe /VERYSILENT /DIR=... /ProductId=... /galaxyclient /buildId=...` through `Start-Process -Verb RunAs`, so **a permission prompt appears** [code: Heroic `src/backend/storeManagers/gog/setup.ts`].
- **Comet** ([imLinguin/comet][comet], Rust, Apache-2.0, 0.4.1, 2026-10-28; Heroic ships it) stands in for Galaxy's communication service for a game's own GOG SDK calls: achievements, stats, play time. It "can't and won't replace Communication Service in official client" [docs]. Its messages are reverse-engineered, and GOG's agreement names emulation (below). Optional; leave for later.
- **Galaxy's own interfaces** go the other way: the integrations API ([galaxy-integrations-python-api][gx-api], MIT) is Galaxy calling a plugin's `install_game`. Nothing lets an outside program make Galaxy install quietly [docs].
- **Sign-in**: GOG has no device-code flow [inferred: none found]. Its login page, `https://login.gog.com/auth?client_id=46899977096215655&layout=galaxy&redirect_uri=https%3A%2F%2Fembed.gog.com%2Fon_login_success%3Forigin%3Dclient&response_type=code`, ends at an address carrying the code [code: gogdl `auth.py`]: a web view in the Dashboard catches it, as GOG OSS does in Playnite's (`GogAccountClient.cs`) [code].
- **GOG's user agreement** (read from a [mirror][gog-eula], undated; the live page refused automated reading): 2.5 calls Galaxy "our (optional) online service"; 11.1(c), "unless you have prior GOG permission please don't modify, merge, distribute, translate, reverse engineer"; 11.1(e) lists "automation software, robots, bots, mods, hacks, spiders, spyware" and "unauthorised third party programs that intercept, emulate, or redirect any communication". My reading [inferred]: downloading the user's own DRM-free games from GOG's servers is GOG's model; Comet's emulation is the clause's target. No action against Heroic, gogdl, lgogdownloader or Minigalaxy is reported.

**Recommendation: fully possible**, but for one permission prompt per install for games that need the script interpreter (section 10 on elevation). gogdl is the quickest; the offline installers the most conservative (no updates, no progress while installing). Galaxy is not needed for any of it.

## 5. EA app

| Approach | Install | Uninstall | Progress, pause, cancel | Needs | EA app sees it | Alive; licence | Risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A. `origin2://game/launch?offerIds=<id>&autoDownload=1` | Starts the download **in the EA app's window**; "download used to be a valid action but it doesn't seem like it's implemented in EA App" [code: [Lutris `ea_app.py`][lutris-ea]] | - | None readable | The EA app, signed in | Yes | Lutris, unifideck use it | Low |
| B. The key's `UninstallString` (today) | - | `EAInstaller\<game>\Cleanup.exe uninstall_game`, its own window; no silent switch found | None | - | Yes | - | Low |
| C. Maxima, an open EA client in Rust | Yes, a queue, then EA's own touch-up step registers it [code] | None found | Bytes and cancel; no pause, no speed [code] | Its own browser sign-in with **the EA app's own client id and secret**; on Windows it rewrites `HKLM\...\Origin\ClientPath`, takes the `qrc:` protocol, and injects a DLL through its service [code] | It says so [docs] | GPL-3.0; "pre-pre-pre-alpha ... cannot yet provide any support" [docs]; last commit 2026-01, branches 2026-08 | **High**: impersonates the EA app and fights an installed one |
| D. efxlve launcher (Rust/Tauri), EA's download reimplemented | Yes [code] | `Cleanup.exe` through a permission prompt | Its own, cancel, speed cap | EA sign-in (PKCE) | Hands Javelin anti-cheat and older titles to the EA app [code] | GPL-3.0, created 2026-09 | High, very new |
| E. Origin's LSX channel / `EABackgroundService` | No: LSX is how a game talks to the launcher, not how it installs | - | - | - | - | - | No install API |

- **EA User Agreement** (updated 2026-05-14, [link][ea-ua]): section 2, "You may not reverse engineer or attempt to extract or otherwise use source code or other data from EA Services"; 7.B, "EA may require you to install and use the EA app client application"; 7.C, anti-cheat looks for "Unauthorized Third-Party Programs". C and D fall under section 2 [inferred]. Heroic declined EA for the protocol being undocumented [reported: Heroic issue #352, via unifideck].
- Questions only the PC answers: whether `autoDownload=1` starts the download with the EA app left in the tray (its window not raised), and whether the EA app writes progress anywhere readable (E1, E2).

**Recommendation: not possible without the EA app's window** on any route this project should take. Keep the link and the uninstaller; if E1 shows the download starts with the app in the tray, the window can be hidden as Steam's is (`shell::steam`'s watch), and the Queue shows "Installing..." with no share until the game's `installerdata.xml` and key appear.

## 6. Ubisoft Connect

| Approach | Install | Uninstall | Progress, pause, cancel | Needs | Client sees it | Alive; licence | Risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A. `uplay://install/<id>`, `uplay://uninstall/<id>` (today; Playnite, Lutris) [code] | The client's install dialog | The client's question | None. unifideck shows an indeterminate state and cancels by ending `upc.exe` [code] | The client | Yes | - | Low |
| B. UplayKit and its downloader (C#), Ubisoft's internal protocol | Yes, with the user's own account [code] | - | Its own | User name and password | No registry entry; games still need the client for their DRM [inferred] | MIT, 2026-10-31; **the same organisation publishes DRM and ticket tools** | **Out of bounds** |
| C. `ubisoft-demux-node` | Protocol only | - | - | - | - | 2024-02 | As B |

- **Ubisoft Terms of Use** (updated 2026-01-26, [link][ubi-tou]): 7.3.5 forbids "private servers, emulators, mirror sites, or alternative launchers without authorization" and "probing, scanning, or abusing APIs"; 7.3.4, "unauthorized third-party tools". An alternative downloader is exactly that. No silent switch for a game install was found; the client's own installer takes `/S` (part B).

**Recommendation: not possible** without the client's window. Keep the links. Whether `uplay://install` skips its dialog when a default folder is set is question U1; if it does, the client's window could be hidden as Steam's is.

## 7. Battle.net

| Approach | Install | Uninstall | Progress, pause, cancel | Needs | Client sees it | Alive; licence | Risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **A. The Agent's local HTTP API** (`Agent.exe`, the client's own downloader) | Yes: `POST /install`, then `POST /install/<uid>` with `game_dir`, so any drive [docs; code] | `POST /uninstall {uid, run_compaction}` [docs; not seen in code] | `GET /install/<uid>`: `progress`, `download_rate` (bytes a second), `download_current`, `download_total`, `download_remaining`, `paused`, `playable` [docs]; a `/download` speed cap [docs]. **No pause or cancel call found**; only a `paused` field | Battle.net installed and signed in recently; the Agent started by us with no window; no admin unless the folder needs it | Yes: it is the client's own Agent, so `product.db` and `aggregate.json` get the game and it starts through Battle.net as now [inferred] | [wowdev wiki "Agent"][wd-agent] (edited 2026-02-23); [barncastle/Battle.Net-Installer][bn-inst] (2.1, 2025-08-29, **no licence**: read, not copied); [HSCentric][hsc] used it 2026-10-02 | Undocumented. One operation at a time; the Agent's own update blocks installs [reported] |
| B. `Battle.net.exe --exec="install <code>"` | Measured to do nothing but open the game's page (client 2.52.8) [reported: unifideck] | - | - | The client window | - | - | Silent failure |
| C. `Battle.net.exe --game=<uid>` (today) | Opens the game's page | - | - | The client window | Yes | - | Low |
| D. `Blizzard Uninstaller.exe --lang=enUS --uid=<uid> --displayname=<name>` (the key's string, today) | - | Its own window; no silent switch found | - | - | Yes | - | Low |
| E. cascette-rs, a stand-in for the Agent | Yes | Yes | Yes | No Blizzard client | Not the official path | Apache-2.0 | **Out of bounds**: emulates Blizzard's platform |

- **How the Agent is driven** [code: barncastle `AgentApp.cs` and `Program.cs` at `3aa2d98`; HSCentric [`BattleNetAgentClient.cs`][hsc] at `14f79ee`]: take the version from `%ProgramData%\Battle.net\Agent\Agent.exe`; start `Agent.<version>\Agent.exe --internalclienttools`, windowless; find its port with `GetExtendedTcpTable` for that process (it is not always 1120; the wiki also names a `Global\Battle.netHelperSvcPortObject` mapping [docs]); `GET /agent` with `User-Agent: phoenix-agent/1.0`, and send the `authorization` it answers as the `Authorization` header from then on; `POST /install {uid, instructions_product: "NGDP", instructions_patch_url: "http://us.patch.battle.net:1119/<product>", priority}`; `POST /install/<uid> {game_dir, selected_locale, ...}`; poll `GET /install/<uid>` (else `/update/<uid>`); `DELETE /agent` when done.
- unifideck, under Wine, got 401 from every call, `/agent` included; it sent neither the `phoenix-agent` agent string nor started its own Agent [reported], which is likely why [inferred]. barncastle's #25 and #26 (error 2310, 2026-03 and 2026-08) are open with no fix [reported]. With the client open, the client can take the operation over ("Another application has taken over...") [code].
- **Blizzard's EULA** (updated 2024-03-21, [link][bn-eula]): 1.C.ii.3 forbids "accessing or modifying the software of the Platform in any manner not expressly authorized by Blizzard"; 1.C.ix, "any connection using third-party programs or tools not expressly authorized by Blizzard"; 1.C.viii, to "intercept, emulate or redirect the communication protocols used by Blizzard". My reading [inferred]: asking Blizzard's own Agent to do what its client asks it, with nothing emulated and no game touched, is the grey part of 1.C.ix; no enforcement against these tools is reported.

**Recommendation: fully possible if the Agent answers**: install to a chosen folder, uninstall, bytes and speed in the Queue; pause and cancel to find out (B3). Build it behind a test on a signed-in PC (B1 to B4), with `--game=<uid>` as the fallback.

## 8. Xbox and Game Pass

| Approach | Install | Uninstall | Progress, pause, cancel | Needs | Xbox app sees it; licence, saves | Alive; licence | Risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **A. `AppInstallManager.StartProductInstallAsync(storeId, ..., AppInstallOptions)`** (`Windows.ApplicationModel.Store.Preview.InstallControl`) | Yes for Store apps [docs]; owned, paid and Game Pass games untested | No | `AppInstallItem.GetCurrentStatus()`: `BytesDownloaded`, `DownloadSizeInBytes`, `PercentComplete`, `InstallState`, `ErrorCode`; `Pause`, `Restart`, `Cancel`, `MoveToFrontOfDownloadQueue`; `ItemStatusChanged` and `ItemCompleted` events; `AppInstallOptions.TargetVolume` chooses the drive; the Store's own notices can be turned off [docs: [AppInstallManager][ms-aim], [AppInstallStatus][ms-ais], [AppInstallOptions][ms-aio]] | The Store signed in with the account that owns the game; admin only for all users | Yes: the Store's own install service does it, Gaming Services and licence as usual [inferred] | In the Windows SDK; the `windows` crate covers it | Documented as "a private capability that is restricted to apps that are developed by Microsoft", yet unpackaged scripts call it (below): could be closed later |
| B. `winget install -s msstore <id>` / its COM API | Free apps rated for everyone only: Xbox titles were taken out of its catalogue [reported: [winget-cli#1833][wg-1833]]; "Support installing purchased Apps" still open ([#908][wg-908]) | Yes, for what it lists | Percent | App Installer | - | MIT | Not for games |
| C. `StoreContext` download calls | Only the calling app's own add-ons [docs] | - | - | Package identity | - | - | Not usable |
| **D. `PackageManager.RemovePackageAsync(fullName)`** | - | Yes, for the current user, no admin [docs] | Deployment percent | Nothing | Unknown whether Gaming Services cleans `XboxGames\<game>` | Windows API | Low to medium; untested on a GDK game |
| E. `ms-windows-store://pdp/?ProductId=`, `msxbox://game/?productId=`, `ms-settings:appsfeatures` (today) | A window | A window | None | - | Yes | - | None; not windowless |
| F. Fetching and decrypting packages ourselves (MsixvcPackageDownloader, XvdTool, irori's `irori-msixvc`) | Outside the Store | - | Their own | Xbox Live tokens | Not licensed on Windows [inferred] | Mixed | **Out of bounds**: decryption |

- **Who calls A outside Microsoft's packaged apps**: Fleet's [`install-microsoft-store-app.ps1`][fleet] from plain PowerShell, with the Store's notices off; it says a paid app fails with `0x803F8001` where the device has no entitlement [code]. winget uses the same queue for Store apps (`MSStore.cpp`), but winget is Microsoft's own packaged app [code]. `AppInstallManager.AppInstallItems` lists the whole Store queue: if the Xbox app's installs go through it (X2), the Queue could also show, pause and cancel installs started in the Xbox app [inferred].
- **Microsoft Services Agreement** (effective 2026-09-30, [link][msa]): Code of Conduct 3.a.vi, "Don't circumvent any restrictions on access to, usage, or availability of the Services"; 8.b.ii forbids to "decompile, decrypt, hack, emulate, exploit, or reverse engineer any software". My reading [inferred]: A, D and E are Windows' public APIs and addresses and circumvent nothing; calling A against its "restricted" note is a support risk (it can stop working), not a breach. F is what those clauses are about.

**Recommendation: possibly fully.** Uninstall by D once X3 shows it leaves no `XboxGames` folder behind and the Xbox app says "not installed"; until then, Settings' page as now. Install by A if X1 shows it takes an owned or Game Pass product from the Dashboard's process; else the Store's page, with A only to read and steer the Store's queue.

## 9. Prior art across stores

How other launchers solve "install without the launcher" (read 2026-11-17) [code unless marked]:

| Project | Stores | How it installs | Progress, pause, cancel | Platform | Licence | Last activity |
| --- | --- | --- | --- | --- | --- | --- |
| Playnite's own library plugins ([PlayniteExtensions][pe]; archived 2026-09, moved to Codeberg) | All seven and more | **Always the store's client**: `steam://install`, the Epic library page, `GalaxyClient /command=installGame`, the Store page, `Battle.net --game`, `uplay://install`; then polls the installed list | None | Windows | MIT | 2026-09-21 |
| Playnite community: [Legendary][pl-leg], [GOG OSS][pl-gog], Nile, a shared download manager (hawkeye116477) | Epic, GOG, Amazon | legendary as a child process; GOG with its own C# downloader and Comet | Bytes, speed, pause (stop, then run again), cancel, one queue | Windows | MIT | 2026-11-15 |
| [Heroic][heroic] | Epic, GOG, Amazon, **Steam (Aurelia, new)** | legendary, gogdl, nile, Aurelia as child processes; GOG's script interpreter elevated | Pause stops the process and requeues; JSON from Aurelia | Windows, Linux, macOS | GPL-3.0 | 2026-11-16 |
| [Lutris][lutris] | Many | GOG's offline installers itself; EA, Ubisoft, Battle.net by their clients under Wine | GOG only | Linux | GPL-3.0 | 2026-11-15 |
| [unifideck][ud] | Steam, Epic, GOG, Amazon, Ubisoft, Battle.net | Epic, GOG as Heroic; Ubisoft and Battle.net "manual", an indeterminate bar while the user clicks Install in the client | Partial | Steam Deck | GPL-3.0 | 2026-11-16 |
| NonSteamLaunchers (moraroy) | Launchers, not games | Vendor installers with switches (part B) | - | SteamOS | MIT | 2026-11-13 |
| Junk Store | Epic (GOG paid) | Scripts, likely legendary [inferred] | - | Steam Deck | Custom | 2026-11-10 |
| [GameShelf][gs], opensu, OmniDash | Steam (others by client) | Steam's `SteamClient.Installs` over CEF, as section 2 | Bytes, pause, cancel | Windows | MIT | 2026-10 to 2026-11 |
| [irori][irori] | Epic, GOG, Amazon (own Rust protocols), Store packages | Its own depot engine; **its `irori-msixvc` decrypts Store packages: out of bounds** | Resume, verify | Windows | MIT | 2026-11-12 |
| Heronite, efxlve_launcher | Epic, GOG, Amazon (+ EA in efxlve) | Rust wrappers round legendary, gogdl, nile; efxlve's EA path is section 5's D | One queue, pause, resume | Windows | GPL-3.0 | 2026-11 |
| GameHub, Bottles, GOG Galaxy integrations | - | Archived (GameHub); launchers in Wine prefixes (Bottles); Galaxy calls the plugin (Galaxy) | - | - | - | - |
| Hydra | - | **A torrent client over "community sources"**: not a store installer, not a reference | - | - | - | - |

- **The pattern is the same everywhere**: only Epic, GOG and Amazon (and now Steam, through Aurelia or CEF) install without the store's window; for EA, Ubisoft, Battle.net and Xbox every project hands over to the client and polls. Battle.net's Agent is the one lead nobody has built into a launcher yet (unifideck calls it its "highest-value lead") [reported].
- **"Pause" everywhere means stop the program and run the same command again**: the tool resumes from its own state file.
- **No cross-store library worth depending on exists**, in Rust or Go. Crates for parts: `egs-api` (Epic's API, MIT), `steam-vent` (Steam's protocol, MIT), `steam-cdn` (Apache-2.0), `steamlocate` (MIT), Comet (GOG's game service, Apache-2.0). Nothing named `legendary`, `gogdl` or `nile` on crates.io. Most Rust launchers above are under three months old: ideas to read, not dependencies [inferred].

## 10. One design for the Dashboard

### 10.1 Three kinds of route, one interface

Every store's install goes one of three ways, chosen per store at run time, the first that can be taken:

1. **The client's own channel**: Steam's `SteamClient` over CEF, Battle.net's Agent, Windows' Store queue and package manager. Nothing to ship; the client stays the owner of the game, which keeps DRM, cloud saves and achievements as they are.
2. **A helper program the user adds**: legendary for Epic, gogdl for GOG. Run as a separate process, its output read line by line. Never linked into the Dashboard: both are GPL-3.0 and the Dashboard's own licence is not decided (plan.md open question 5); a separate program is "mere aggregation" either way [inferred]. **Not bundled**: on the user's Yes in Settings, the Dashboard fetches the helper's release named in the Dashboard's own source (version and SHA-256 fixed in code) from that helper's GitHub releases page into its data folder, shows its licence and a link to its source, and can remove it again. That keeps the GPL's source offer theirs and every update a reviewed change of ours.
3. **The client's window**, as today (`ask` returning a `Start`): EA, Ubisoft, and any store whose better route failed. The Dashboard says so before it opens the client.

`model::Source` grows one optional method, with a default, as `installs` did (sources.md: "a new method on the trait with a default"):

```rust
pub trait Source {
    // ... as now: read, start, ask, running, installs, uninstalls_by_itself
    /// The store's own way to install, uninstall and steer installs with no window; None (the default): `ask`.
    fn installer(&self) -> Option<&dyn Installer> { None }
}

pub trait Installer: Send + Sync {
    fn can(&self) -> Can;                                            // pause, cancel, choose a folder, uninstall quietly
    fn folders(&self) -> Vec<Folder>;                                // where it may install: path, free bytes, default
    fn install(&self, game: &Game, folder: Option<&Folder>) -> anyhow::Result<()>;
    fn uninstall(&self, game: &Game) -> anyhow::Result<()>;
    fn pause(&self, source_id: &str) -> anyhow::Result<()>;
    fn resume(&self, source_id: &str) -> anyhow::Result<()>;
    fn cancel(&self, source_id: &str) -> anyhow::Result<()>;        // and delete what was fetched
}
```

- `installs()` stays the one place progress comes from, for every route: Steam's from its overview and manifests as now; Battle.net's from `GET /install/<uid>`; Xbox's from `AppInstallManager.AppInstallItems`; a helper's from the last line it printed, kept by the job that runs it. The Queue, its follower, Manage Queue and the Toasts need no change for progress; `Install::speed` takes the store's speed where it has one (Steam's, Battle.net's `download_rate`, legendary's `Download - ... MiB/s`).
- A helper's job is a small runner in `shell` (it starts and stops processes, as `shell::steam` does): one job at a time per store, its output parsed by the source's crate (`source_epic::legendary::progress(line)`, tested on made-up lines), "pause" = stop the process and mark the entry `Paused`, "resume" = run the same install again. A job that dies is said in the log and leaves the Queue as dropped, as a Steam install that Steam drops does.
- `uninstalls_by_itself` becomes true for every store whose `installer` uninstalls quietly (Steam, Epic through legendary, GOG through `/VERYSILENT`, Battle.net through the Agent, Xbox through the package manager once X3 says so).
- **Reading what helpers installed.** `source-epic` reads legendary's `installed.json` (in the folder the Dashboard gives it) as a fourth record beside the install helper's, the manifests and the eglv2 files, and `start` gives `legendary launch <AppName>` for those games; `source-gog` already reads `GOG.com\Games\<id>`, which the script interpreter is expected to write [inferred; G2].

### 10.2 Folders and drives

`Installer::folders` lets Install ask where, as the console does when more than one drive is set up: Steam's library folders (`InstallFolder.GetInstallFolders`), the Store's volumes (`TargetVolume`, the drives with a `.GamingRoot`), and for Battle.net, Epic and GOG a folder per drive the user names once in Settings > Storage (a new page; the console has one). With one folder there is no question. The storage ring under Manage's nav (manage-queue.md 7.6) can then be drawn from the same list.

### 10.3 Permission prompts

GOG's script interpreter, some Epic prerequisites and Battle.net into `Program Files` need an administrator. A Windows permission prompt is on the secure desktop: the pad cannot answer it [inferred]. Options, for the user to choose (open question 3): (a) install those games under a folder the user owns (`D:\Games`), which avoids most prompts; (b) the prompt, with the Dashboard saying "Windows will ask on screen; use the mouse or keyboard"; (c) a small helper registered once, elevated, as a scheduled task or service, that runs only those set-up steps; it must accept only a known set-up program in a folder the Dashboard chose, or anything on the PC could use it to run as administrator [inferred].

### 10.4 Signing in with a pad

Only the helper routes need a sign-in of the Dashboard's own: Epic (legendary) and GOG (gogdl). Steam, Battle.net and Xbox ride on their clients' own sign-in.

- A **sign-in page** in Settings > Accounts per store: the store's own login page in a WebView2 the Dashboard owns, full screen, the pad moving between its fields as Tab and Shift+Tab, A as Enter, and the Dashboard's own Console keyboard (docs/spec/search.md) for text. The page's last redirect carries the code (Epic's `api/redirect` JSON, GOG's `on_login_success?code=`), which the Dashboard hands to `legendary auth --code` or `gogdl auth --code`. Nothing typed is kept by the Dashboard.
- **Or a phone**: a QR code of the same login address, the user signing in on the phone and typing back the short code the page shows (Epic shows an `authorizationCode`; GOG's redirect address can be read off). Clumsier, but no password on the TV.
- Epic's device-code flow (a QR to `epicgames.com/activate`) would be the best on a TV, but only under the application id of one of Epic's own programs; **not recommended** unless Epic offers one to third parties.
- Tokens: the helpers' own files, in the Dashboard's data folder (`LEGENDARY_CONFIG_PATH`, `--auth-config-path`), protected with DPAPI at rest where the helper allows, as Discord's tokens are (docs/discord.md). Sign out deletes them.

### 10.5 What the user sees

- **Install** on a tile or the hub: the folder question when there is more than one; then the game joins the Queue as now, with a share and speed for every store but EA and Ubisoft, which read "Installing..." with no share while their client works (manage-queue.md section 3 already draws that), and say before their client opens that it will.
- **Pause, Resume and Cancel**: the Bundle's Manage Queue has none (manage-queue.md 5). The console puts them in a queued game's menu; this needs the user's decision (open question 1), and its look taken from the console since the Bundle has none.
- **Uninstall**: the Tile menu's question as now, then nothing more for any store whose `installer` uninstalls quietly; the Uninstalled Toast once it is gone, as for Steam.
- **Settings > Stores** (new): per store, which route is in use and why ("Epic: through legendary 0.21.1, signed in"; "Steam: the debugger is off, so Steam is asked through steam.exe"), the helper's add and remove, the sign-in, and the terms caveat in one plain line.

## 11. Part B: the launchers themselves

From the [winget-pkgs][wgp] manifests read on 2026-11-17, and winget's own default switches (`ManifestCommon.cpp`: MSI and Burn `/quiet /norestart`, NSIS `/S`, Inno `/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART`, a plain exe none) [code]:

| Launcher | winget id | Installer, silent switch | Admin | First run | Removing the client does to its games |
| --- | --- | --- | --- | --- | --- |
| Steam | `Valve.Steam` | NSIS, `/S` | Machine; one permission prompt | Updates itself, then sign-in (password or QR) | **Deletes the Steam folder with the games in its own `steamapps`**; libraries on other drives stay [docs: Steam Support "Uninstall Steam", as quoted by search; reported] |
| Epic Games Launcher | `EpicGames.EpicGamesLauncher` | WiX MSI, `/quiet /norestart` | Machine | Sign-in | Game files are reported to go too [reported]; most games need the launcher (or legendary) for their ownership token |
| GOG Galaxy | `GOG.Galaxy` | Inno, `/VERYSILENT ...` | Machine [inferred] | Sign-in | **Games stay and run** (DRM-free); achievements and some multiplayer need Galaxy [docs; reported] |
| EA app | `ElectronicArts.EADesktop` | Burn, but the manifest sets `/passive /norestart`: **a progress window always shows** | Machine [inferred] | Sign-in | Files stay; **games cannot be played until it is back** [docs: EA help, via excerpt] |
| Ubisoft Connect | `Ubisoft.Connect` | NSIS, `/S` | Machine | Sign-in | The uninstaller offers to keep the games; most need Connect to run [reported] |
| Battle.net | `Blizzard.BattleNet` | Plain exe, `--lang=enUS --installpath="<dir>"`, **`InstallLocationRequired: true`**: winget fails without `--location` when it may not ask; **its window shows** [code: winget `PromptFlow.cpp`; reported] | Machine [inferred] | Sign-in | Games stay but start and update only through it [reported: Blizzard support 30304] |
| Xbox app | `9MV0B5HZVK9Z` (`-s msstore`) | MSIX by the Store | Per user | Microsoft account; offers Gaming Services | - |
| Gaming Services | `9MWPM2CQNLHN` (`-s msstore`); repair tool `Microsoft.Gaming.GamingServicesRepairTool` | MSIX | Machine service [inferred] | - | Removing it: GDK and Game Pass games neither start nor install until it is back; files stay [reported] |

- **Driving winget**: `winget install --id <id> -e -s winget --silent --accept-package-agreements --accept-source-agreements --disable-interactivity` (`--location` for Battle.net; `-s msstore` for the Xbox app and Gaming Services) [docs]. Its output is a progress bar for a terminal, not JSON (winget-cli #184 is open) [reported]. Its COM API (`Microsoft.Management.Deployment`, `InstallPackageAsync`) reports `Queued`, `Downloading`, `Installing`, `PostInstall`, `Finished` with bytes, and cancels only before `Installing` [code: `PackageManager.idl`]; from an unpackaged program it needs App Installer present, and from an elevated one a manual activation that has an open crash (#4377) [code; reported]. The CLI is the plainer start.
- **A fresh Windows 11** has App Installer, but registers it some time after the first sign-in; `Add-AppxPackage -RegisterByFamilyName -MainPackage Microsoft.DesktopAppInstaller_8wekyb3d8bbwe` brings it [docs].
- **Installers start the client when done and do not exit** while it runs: wait for the client's program to appear, not for the installer (irori's notes) [code].
- **Terms**: nothing found forbids fetching a store's public installer onto the user's own PC; winget-pkgs points at the vendors' own addresses, and the user accepts each client's terms at its first sign-in [inferred]. So: through winget or the vendor's address, never bundled or mirrored.

**A "Set up this PC" page** (Settings > Stores), with the pad: a list of the seven clients, those installed ticked; for each, one line of what removing it would do (the last column above); A to add one, one at a time, through winget, the row reading Waiting, Downloading, Installing, Done (from the COM API, or the CLI's exit and the client's program appearing); a warning that Windows asks permission for each and that EA's and Battle.net's installers show a window of their own; then "Sign in to <store>", which opens that client's sign-in window, the one time a client window is the point (Steam's QR is the friendliest), and returns to the Dashboard once the client's files show a signed-in user. Removing a client: the same page, after the question "Remove <client>? <what happens to its games>".

## 12. What only a signed-in Windows PC can answer

Each to try once on the TV PC by the user, or on a spare PC, before the matching ticket is built. Codes are used by the tickets below.

**Steam**
- S1. Does `Installs.SetAppList` then `ContinueInstall` install without `OpenInstallWizard`, or can the wizard's window be kept from showing at all? Is the `"0"` client-id argument right on the stable client?
- S2. Does `IClientCommService/InstallClientApp` for this PC start a download with no window, and does `SetClientAppUpdateState` pause it? Can the running client's own web token be used?
- S3. Does any of this sign the client out, as steamcmd is reported to?

**Epic**
- P1. Does the current launcher (20.3.3) see a legendary install exported by `egl-sync` into a hand-made `Data\Manifests`? If the launcher is pointed at a folder legendary filled ("Install", same folder), does it verify rather than download, and write an `.egi`?
- P2. Does a game installed and started by legendary pass its ownership check, sync its saves and unlock achievements, against the same game from the launcher?
- P3. Does `legendary auth --import` take the new launcher's sign-in?

**GOG**
- G1. Is `GalaxyClient.exe /gameId=<id> /command=installGame` silent? Is there an uninstall command (strings in `GalaxyClient.exe`)?
- G2. After a gogdl download and the script interpreter, does Galaxy list the game as installed? Which keys does the interpreter write, and does it really need an administrator for a game in a folder the user owns?
- G3. Do the offline installers run `/VERYSILENT /DIR=` from an unelevated process without any prompt?

**EA**
- E1. Does `origin2://game/launch?offerIds=<id>&autoDownload=1` start the download with the EA app in the tray, its window not raised?
- E2. Does the EA app write its download's progress anywhere readable (its logs, its local data)?
- E3. Does `Cleanup.exe uninstall_game` take any silent switch?

**Ubisoft**
- U1. Does `uplay://install/<id>` skip its dialog when a default folder is set? Does the client write progress anywhere readable?

**Battle.net**
- B1. Does `Agent.exe --internalclienttools` then `GET /agent` (with `phoenix-agent/1.0`) still give a token, with and without the client open?
- B2. Do `POST /install/<uid>` and `POST /uninstall` work with no window, and does the client then list the game?
- B3. Can an Agent install be paused or cancelled (`DELETE /install/<uid>`, `/priorities`, `/download`)?
- B4. Do `progress` and `download_rate` move live?

**Xbox**
- X1. Does `AppInstallManager.StartProductInstallAsync` take an owned paid game, a Game Pass GDK game and an older UWP game from the Dashboard's own unelevated, unpackaged process? Where does `TargetVolume` put a GDK game against the Xbox app's `XboxGames` folder?
- X2. Do installs started in the Xbox app or the Store appear in `AppInstallManager.AppInstallItems`, and can they be paused and cancelled from there?
- X3. Does `RemovePackageAsync` on a GDK game remove `XboxGames\<game>`, keep its cloud saves, and leave the Xbox app saying "not installed"? Does it need an administrator?

**Part B**
- L1. Which of the seven clients' installers, run through winget from the Dashboard, show a window, and how many permission prompts does a whole set-up take?

## 13. Follow-up tickets (drafted, not created)

Each is one PR; the probes come first, since what they find decides the rest.

1. **Probe: installs on a signed-in PC** (research, needs a human). A small `dashboard --probe-installs <store>` mode, read-only but for the one test game the user names, that runs S1 to S3, P1 to P3, G1 to G3, E1 to E3, U1, B1 to B4, X1 to X3 and L1 and logs what each answered, so one session at the TV fills section 12. Done when every question there has an answer written into this file.
2. **Steam: install, pause, resume and cancel through `SteamClient`** (M7 or later). `source_steam::cef` gains `Installs` and `Downloads` calls; `Installer` for Steam (library folders, pause, resume, cancel, `OpenUninstallWizard(..., true)`); `steam.exe -silent` stays the route with the debugger off. Fake: the stand-in Steam's CEF page answers the calls. Depends on S1. Built in #171 (`source_steam::client`, the calls listed with how each is known in docs/sources.md, "Steam's own calls"), every call logging what it asked and what came back, so S1 and the `"0"` client id are answered by the first run on the TV PC.
3. **The `Installer` interface and Manage Queue's Pause, Resume and Cancel** (needs the user's decision on where they go: open question 1). `model::Installer`, `Source::installer`, the Queue taking pause and cancel, the menu rows, the folder question. No store but Steam uses it yet.
4. **Battle.net through its Agent.** `source-battlenet` starts the Agent, takes its token, installs to a chosen folder, uninstalls, reports bytes and speed; `--game` as the fallback. Fake: a stand-in Agent in `standin` answering the calls. Depends on B1 to B4.
5. **Xbox: uninstall by the package manager, install by the Store's queue.** `RemovePackageAsync` once X3 holds; `AppInstallManager` for install and for reading and steering the Store's queue once X1 and X2 hold; `ms-windows-store://` stays the fallback. Fake: the VM's loose-file package (removal); a stand-in queue behind a trait for the rest.
6. **Helpers: fetching, checking and removing legendary and gogdl** (Settings > Stores). Version and SHA-256 fixed in code, fetched on the user's Yes, licence and source link shown, removable. A runner in `shell` for helper jobs (start, read lines, stop).
7. **Epic through legendary.** Sign-in page (WebView2 and the pad), `installed.json` as a fourth record, install, uninstall, pause by stopping, start through legendary; parsers tested on made-up lines. Depends on 3, 6 and P1 to P2.
8. **GOG through gogdl.** Sign-in page, download, script interpreter, `unins000.exe /VERYSILENT`; the permission question decided (open question 3). Depends on 3, 6 and G2 to G3.
9. **EA and Ubisoft: the client's window, kept short.** "Installing..." in the Queue from Install until the game's files appear, the client's window hidden once the download has started where E1 and U1 say it can be. Depends on E1, U1.
10. **Set up this PC** (part B). Settings > Stores' list of clients, winget installs one at a time, the sign-in hand-over, removal with its warning. Depends on L1. Built in #178 as Settings > Games & apps > Set up this PC (docs/spec/set-up.md): each vendor's installer downloaded and run by the Dashboard itself at a version and SHA-256 fixed in its source, the ids, switches and hashes taken from winget's manifests (why not winget itself: set-up.md 1); L1 is answered by its first real run.
11. **Settings > Storage**: the drives and folders installs may use, free space, and Manage's storage ring (manage-queue.md 7.6).

## Open questions for the user

1. **Pause, Resume and Cancel** are not in the Bundle. Add them to a queued game's menu as the console has them (the port's own look, copied from the console's), or leave the Queue as the Bundle has it?
2. **Helper programs.** Is it acceptable that Epic and GOG games installed through legendary and gogdl are the Dashboard's, invisible to the Epic launcher and possibly to Galaxy, with a sign-in of the Dashboard's own? Or keep the official launchers' windows for those two?
3. **Permission prompts** for GOG's set-up steps and installs under `Program Files`: a folder the user owns, the prompt answered with a mouse, or an elevated helper registered once?
4. **Risk appetite.** Steam's and Blizzard's terms speak against "unauthorized third-party software" controlling their clients; nothing is reported enforced against tools of this kind. Go ahead per store, or keep to the documented routes (Xbox) and the clients' windows elsewhere?

[dfl]: https://github.com/SteamDeckHomebrew/decky-frontend-lib/tree/52d1173edd370524d58fe4fe703eb70930c91d53/src/globals/steam-client
[gs]: https://github.com/ArthurAugis/gameshelf/blob/896d1be244/Steam/SteamInstaller.cs
[opensu]: https://github.com/SomeoneIsWorking/opensu
[omnidash]: https://github.com/ykyritsis/OmniDash
[gol-remote]: https://www.gamingonlinux.com/2026/04/steam-beta-adds-remote-downloads-management/page=1/
[sp-cc]: https://github.com/SteamDatabase/SteamTracking/blob/master/Protobufs/webui/service_clientcomm.proto
[xs-cc]: https://github.com/XasYer/steam-plugin/blob/master/models/api/IClientCommService.js
[sk-qr]: https://github.com/SteamRE/SteamKit/tree/7c6e2f0309/Samples/001_AuthenticationWithQrCode
[ssa]: https://store.steampowered.com/subscriber_agreement/
[lg]: https://github.com/legendary-gl/legendary/tree/170964e
[lg-797]: https://github.com/legendary-gl/legendary/issues/797
[heroic]: https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/tree/4b55a4cb70
[pl-leg]: https://github.com/hawkeye116477/playnite-legendary-plugin/tree/ce6b933
[pl-gog]: https://github.com/hawkeye116477/playnite-gog-oss-plugin/tree/57d7baa
[epic-uninstall]: https://help.epicgames.com/help/en-US/c-Category_EpicGamesStore/c-EpicGamesStore_LauncherSupport/how-to-uninstall-a-game-from-the-epic-games-launcher-a000087932
[epic-device]: https://github.com/LeleDerGrasshalmi/FortniteEndpointsDocumentation/blob/main/EpicGames/AccountService/Authentication/DeviceCode/Create.md
[epic-eula]: https://legal.epicgames.com/store/eula
[gogdl]: https://github.com/Heroic-Games-Launcher/heroic-gogdl/tree/6c6ef53
[comet]: https://github.com/imLinguin/comet/tree/46c9152
[gx-api]: https://github.com/gogcom/galaxy-integrations-python-api
[gog-eula]: https://mirrors.sjtug.sjtu.edu.cn/gentoo/licenses/GOG-EULA
[lutris-ea]: https://github.com/lutris/lutris/blob/82015e004df2ce9308a168db47719d3f589d4441/lutris/services/ea_app.py
[ea-ua]: https://www.ea.com/legal/user-agreement
[ubi-tou]: https://www.ubisoft.com/legal/documents/termsofuse/en-CA
[wd-agent]: https://wowdev.wiki/Agent
[bn-inst]: https://github.com/barncastle/Battle.Net-Installer/tree/3aa2d98d86
[hsc]: https://github.com/z-henry/HSCentric/blob/14f79eecd4/HSCentric/BattleNetAgentClient.cs
[bn-eula]: https://www.blizzard.com/en-us/legal/fba4d00f-c7e4-4883-b8b9-1b4500a402ea/blizzard-end-user-license-agreement
[ms-aim]: https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.store.preview.installcontrol.appinstallmanager
[ms-ais]: https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.store.preview.installcontrol.appinstallstatus
[ms-aio]: https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.store.preview.installcontrol.appinstalloptions
[wg-1833]: https://github.com/microsoft/winget-cli/issues/1833
[wg-908]: https://github.com/microsoft/winget-cli/issues/908
[fleet]: https://github.com/fleetdm/fleet/blob/main/docs/solutions/windows/scripts/install-microsoft-store-app.ps1
[msa]: https://www.microsoft.com/servicesagreement
[pe]: https://github.com/JosefNemec/PlayniteExtensions/tree/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries
[lutris]: https://github.com/lutris/lutris/tree/82015e004df2ce9308a168db47719d3f589d4441/lutris/services
[ud]: https://github.com/mubaraknumann/unifideck/tree/2e9981d2de73db8aba7864c5f351fe630895e2d7
[irori]: https://github.com/irori-app/irori
[wgp]: https://github.com/microsoft/winget-pkgs/tree/master/manifests
