# Set up this PC (Settings > Games & apps > Set up this PC; #178)

The page that installs the launchers' own apps on a PC that lacks them, and removes them again, with the controller:
Steam, the Epic Games Launcher, GOG Galaxy, the EA app, Ubisoft Connect and Battle.net. From
[../research/installs.md](../research/installs.md), part B (section 11) and section 13 item 10. The Bundle had
nothing of the kind: the page, its words and its layout are the port's own, made of Settings' Rows and questions.

The code: `helpers::clients` (the table, the download and check, Windows' list, the commands and how a step is
followed; none of it needs Windows but the run), `app::set_up` (the thread, the states, the log),
`shell::running` (whether an app is open), `screens::settings::set_up` (the page), the stand-ins
(`standin serve-helpers --clients`, `standin-setup-<key>.exe`).

## 1. Decisions

- **The Dashboard downloads and runs each vendor's installer itself, not through winget.** The research (11) leaned
  to winget, and the ticket's first wording says "winget installs one at a time"; what decided it the other way:
  - *What runs is fixed in the Dashboard's own source.* winget installs whatever its community catalogue says today:
    a manifest changed upstream changes the program the Dashboard runs on the user's Yes, with nothing reviewed here.
    The helpers' rule (helpers.md 1: a fixed version, address and SHA-256, every update a reviewed change) is kept.
  - *The question can name the address and the size*, which winget's CLI does not give before it starts, and the
    download's progress is the Dashboard's own (winget's CLI prints a terminal's progress bar, not lines a program can
    read: winget-cli #184; its COM API from an unpackaged or elevated program has open crashes, #4377).
  - *A fresh Windows 11 has no winget* until App Installer registers after the first sign-in (research 11): the
    page would not work on the very PC it is for.
  - *winget accepts the vendors' terms on the user's behalf* (`--accept-package-agreements`); here the user accepts
    each app's terms in its own window, at its first sign-in.
  - What is taken from winget is its knowledge: **every id, installer kind, switch, product code and SHA-256 in the
    table is the app's manifest in [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs/tree/master/manifests)**
    (read on 2026-11-17), and winget's own default switches per installer kind
    ([`ManifestCommon.cpp`](https://github.com/microsoft/winget-cli/blob/57d424f360608411e871c44c7d0ba21d848bcab6/src/AppInstallerCommonCore/Manifest/ManifestCommon.cpp),
    `GetDefaultKnownSwitches`: Nullsoft `/S`, Inno `/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART`, MSI and Burn
    `/quiet /norestart`). The addresses are the vendors' own, the ones winget's manifests point at too.
- **The Xbox app is not on the page.** It comes only from the Microsoft Store (`9MV0B5HZVK9Z`, `-s msstore`), with no
  installer of its own to fix; and removing it, or Gaming Services, stops Game Pass games (research 11). Later, if
  wanted: its Store page opened from here.
- **What is installed is what Windows' list of installed programs says**: the app's Uninstall key (either hive,
  either view), matched as winget matches it, by the key's name (the manifest's `ProductCode`) or else its display
  name; a key Windows hides (`SystemComponent` 1, an MSI inside a bundle) is passed over. The stores' sources find a
  store's *games*, which can be there without its app (games installed by legendary or gogdl, a Steam library on
  another drive), so their word is not the app's; what they read is used for the games, in the removal's warning
  (the installed games of that launcher in the library), and each launcher is read again (Launchers' update) as soon
  as its app is installed or removed, so its games come in or show as not installed.
- **Removal is the app's own registered uninstaller**: the key's `QuietUninstallString` where it has one; an MSI's
  `msiexec /x <product code> /quiet /norestart`; else its `UninstallString` with the quiet switches of the
  installer's kind (Nullsoft `/S`, Inno `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`). The EA app's and Battle.net's
  are run as their keys give them, and the question says they show a window of their own.
- **One step at a time**: while one app is being installed or removed, A on another says "Wait until <app> is done."
- **Refused**, said under the Rows with nothing asked: removing an app while a game of its launcher plays or is being
  launched ("Quit <game> before removing <app>."), or is in the Queue ("<game> is installing: let it finish, or cancel
  it, before removing <app>."); and, checked again by the executable on the Yes, while the app itself is open (a
  program under its install folder running: "Close <app> first: <program> is running."). Every refusal is a log line.
- **Windows' permission prompt.** Every one of these installers installs for the whole machine. The Dashboard starts
  an installer as it is; where Windows answers that it needs an administrator (error 740), and for an MSI always
  (`msiexec` told to be quiet never asks itself), it starts it through `powershell Start-Process -Verb RunAs`, which
  puts the prompt up. That prompt is on Windows' secure desktop, which no program's input reaches, the virtual pad's
  included (helpers.md 5): **the controller cannot answer it**, and both questions say so before anything happens.
  The ways round it are the user's to choose, as helpers.md 5 lists them. Declining the prompt fails the step
  ("It wasn't installed: its installer ended with 1.").
- **Signing in** is each app's own, in its own window, once it is installed: the question says so. The Dashboard
  does not open the app for it yet (a hook for later: the app's program is in its key's `InstallLocation`).

## 2. The table (`helpers::clients::builtin`)

Looked up on 2026-11-17. "Checked" means the file at the address was downloaded here and hashed to the manifest's
SHA-256; the others (over 250 MB each) were not, and their address answered the manifest's size and date.

| App | Vendor's page | Installer address | Size | Kind, switches | winget id, manifest | SHA-256 |
| --- | --- | --- | --- | --- | --- | --- |
| Steam | [store.steampowered.com/about](https://store.steampowered.com/about/) (links the same file on `cdn.fastly.steamstatic.com`) | `https://cdn.akamai.steamstatic.com/client/installer/SteamSetup.exe` | 2,380,800 | Nullsoft, `/S` | `Valve.Steam` [2.10.91.91](https://github.com/microsoft/winget-pkgs/tree/master/manifests/v/Valve/Steam/2.10.91.91) | `7d3654531c32d941b8cae81c4137fc542172bfa9635f169cb392f245a0a12bcb`, checked |
| Epic Games Launcher | [store.epicgames.com/download](https://store.epicgames.com/download) (its link, `launcher-public-service-prod06.ol.epicgames.com/.../EpicGamesLauncherInstaller.msi`, redirects to this address) | `https://epicgames-download1.akamaized.net/Builds/UnrealEngineLauncher/Installers/Windows/EpicInstaller-20.3.3.msi?launcherfilename=EpicInstaller-20.3.3.msi` | 264,237,056 | MSI (WiX), `msiexec /i ... /quiet /norestart` | `EpicGames.EpicGamesLauncher` [1.3.210.0](https://github.com/microsoft/winget-pkgs/tree/master/manifests/e/EpicGames/EpicGamesLauncher/1.3.210.0) | `d55d79710edfeaa107a62f2ea007bec1eeeac8b715a43c2ba3858eec7bc60337` |
| GOG Galaxy | [gog.com/galaxy](https://www.gog.com/galaxy) (its button is a web installer of no fixed version, `webinstallers.gog-statics.com/download/GOG_Galaxy_2.0.exe`; this is GOG's full installer at its own content address) | `https://content-system.gog.com/open_link/download?path=/open/galaxy/client/setup_galaxy_2.1.10.56.exe` | 342,291,968 | Inno, `/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART` | `GOG.Galaxy` [2.1.10.56](https://github.com/microsoft/winget-pkgs/tree/master/manifests/g/GOG/Galaxy/2.1.10.56) | `ec1ef3744a6ba365c022d5e114a482b74ff0496e00a91924ebdae401e2c9291b` |
| EA app | [ea.com/ea-app](https://www.ea.com/ea-app) (links the unversioned `EAappInstaller.exe` at the same address) | `https://origin-a.akamaihd.net/EA-Desktop-Client-Download/installer-releases/EAappInstaller-13.827.2.6342-16601618.exe` | 2,141,208 | Burn, `/passive /norestart` (the manifest's own: a progress window shows) | `ElectronicArts.EADesktop` [13.827.2.6342](https://github.com/microsoft/winget-pkgs/tree/master/manifests/e/ElectronicArts/EADesktop/13.827.2.6342) | `6eafd193300fdfb7be5a014a2261b8c08e51d867a0d5590cb92aa055917c5cbd`, checked |
| Ubisoft Connect | [ubisoft.com/.../ubisoft-connect/download](https://www.ubisoft.com/en-us/ubisoft-connect/download) (links `ubi.li/4vxt9`, which redirects to this address) | `https://static3.cdn.ubi.com/orbit/launcher_installer/UbisoftConnectInstaller.exe` | 263,096,496 | Nullsoft, `/S` | `Ubisoft.Connect` [175.1.0.13416](https://github.com/microsoft/winget-pkgs/tree/master/manifests/u/Ubisoft/Connect/175.1.0.13416) | `e907f33e1fc74fddd61eaf548647201ebc4baac8c439c3fa8e7043c67dc11576` |
| Battle.net | [download.battle.net/en-us/desktop](https://download.battle.net/en-us/desktop) | `https://downloader.battle.net/download/getInstaller?os=win&installer=Battle.net-Setup.exe` | 4,700,880 | exe, `--lang=enUS --installpath="%ProgramFiles(x86)%\Battle.net"` (the manifest's `InstallLocationRequired`) | `Blizzard.BattleNet` [2.0.0.4232](https://github.com/microsoft/winget-pkgs/tree/master/manifests/b/Blizzard/BattleNet/2.0.0.4232) | `f4643a2086c0cecb1ab68cc601bf38c1a0a6f7ef5914ad03375cc9449385bec6`, checked |

Windows' list (the Uninstall key) by: Steam `Steam`; Epic its product code `{0716D12C-C7A6-4C1B-A5BB-E23B7160250D}`,
else "Epic Games Launcher"; GOG `{7258BA11-600C-430E-A759-27E2C691A335}_is1`, else "GOG GALAXY"; EA "EA app"; Ubisoft
`Uplay`; Battle.net `Battle.net`. Steam's key name and the display names other than winget's product codes are
[inferred] from the vendors' installers as commonly seen, to be confirmed on the TV PC.

**Unversioned addresses.** Steam's, Ubisoft's and Battle.net's addresses always serve the vendor's newest
installer. When the vendor changes it, its SHA-256 is no longer the table's and the download is refused with nothing
run ("The download wasn't the file expected, so it wasn't kept."): the table is then brought up to date from the new
winget manifest, as a reviewed change. Steam's has been the same file since May 2024.

## 3. Settings > Games & apps > Set up this PC

**Its box**: **Set up this PC**, icon U+E977, the third of the Installs section, under Helpers (x 975, y 560).

**The page**, on the Settings pages' 892-wide column from (520, 210):

| Element | y | Notes |
| --- | --- | --- |
| The Rows | 210, no line over them (#261, settings.md 16.11) | One an app, 892 x 66 every 74 (settings.md 4.3), in the table's order: the launcher's mark (`home::source_mark`, as Launchers'), the app's name, and how it stands right-aligned: "Not installed", "Installed", "Downloading: 34%", "Installing...", "Removing..." (with the Spinner before it) |
| The line | 30 under the last Row | Of the focused app: "A installs Steam 2.10.91.91 from Valve's own address (2.4 MB)."; installed, "8 Steam games are installed." and what removing it does; while a step works, what it is doing; after a failure, why; after a refusal, why (white) |

Up and Down move between the Rows, stopping at the ends; B or Left goes back to Games & apps' boxes on Set up this PC.

**A on an app not installed asks first**, a prompt over the dimmed page (as Helpers' Add): "Install Steam?", "The
Dashboard downloads Steam's installer (2.4 MB) from Valve's own address,
cdn.akamai.steamstatic.com/client/installer/SteamSetup.exe, checks it and runs it. Windows then asks for permission
on screen: answer it with a mouse or keyboard, as the controller can't. Once it's installed, sign in to Steam in its
own window." (with " Its installer shows a window of its own." for the EA app and Battle.net), **Install** and
**Cancel**, Install focused. Only Install downloads (`Effect::SetUp(SetUpAsk::Install)`).

**A on an installed app asks too**: "Remove Steam?", "8 Steam games are installed. Steam's uninstaller deletes the
games in its own folder with it; games in Steam libraries on other drives stay, and need Steam to play. Steam's own
uninstaller runs; Windows may ask for permission on screen, which needs a mouse or keyboard.", **Remove** and
**Cancel**. What removing does, per app (research 11, [reported] there):

| App | Removing it |
| --- | --- |
| Steam | Its uninstaller deletes the games in its own folder; other libraries stay, and need Steam |
| Epic Games Launcher | Its games are reported to go with it; most can't be played without it (or legendary) |
| GOG Galaxy | Its games stay and still play; achievements and some online play need Galaxy |
| EA app | Its games stay, but can't be played until it's back |
| Ubisoft Connect | Its games are reported to stay, but most can't be played without it |
| Battle.net | Its games stay, but start and update only through it |

## 4. Installing, removing

On Install's Yes, on a thread of its own:

1. A plain GET of the vendor's address, redirects followed, User-Agent `dashboard-helpers`, nothing else (as the
   helpers'); at most the table's size and a tenth. Written as `<data>\set-up\<key>\<file>.part` while its SHA-256 is
   worked out as it comes; not the table's, the folder is deleted and nothing is run. The page shows the share
   downloaded.
2. Run with its switches (section 1 on the permission prompt).
3. Followed by Windows' list, every half second: **installed** once the app's key is there and the installer has
   ended, or once the key has been there 20 s with the installer still running (one that starts the app and waits on
   it); **not installed** if the installer ended and the key is not there a minute later; given up after 45 minutes,
   the installer left to finish on its own.
4. The installer's folder deleted (one still running is left to it, and said).
5. The launcher read again (`launchers: update: reading battlenet`), so its games come in.

On Remove's Yes: the app's key read again, refused as section 1 says, then its uninstaller run and followed the same
way until the key is gone (an NSIS uninstaller ends at once, having handed over to a copy of itself: the list is
looked at for a minute more), 20 minutes at most; then its launcher read again.

At start one line says them all: `set-up: steam installed, epic not installed, gog not installed, ea not installed,
ubisoft not installed, battlenet not installed`. Each step: `set-up: battlenet: downloading Battle.net 2.0.0.4232 from
<address> (<n> bytes)`, `checked and kept at <path>` (or `refused: its SHA-256 is <got>, not the <want> the Dashboard
expects`), `running Battle.net-Setup.exe --lang=enUS --installpath="..."`, `wants an administrator: Windows asks on
screen`, `started (pid <n>)`, `<program> ended with <code>`, `installer deleted`, `installed: <key>` (or `not
installed: its installer ended with <code>`); `running <uninstaller> <switches>`, `removed` (or `still installed: its
uninstaller ended with <code>`); `refused: <why>`.

**The focus log**: `Settings: Games & apps: Set up this PC`; on the page `Settings: Set up this PC: Steam; not
installed` (`installed`, `installed, no uninstaller`, `downloading`, `installing`, `removing`, `not installed; failed:
<why>`), and `; asking Install Steam?` (`Remove Steam?`) while a question is up, `; refused: <why>` after a refusal.
The line changes as a step goes, without a press.

## 5. What a test may give it

- `dashboard --client-table <file>`: the table, as JSON, in place of the builtin one (`standin serve-helpers
  --clients <file> --sandbox <dir>` writes one: every installer `standin-setup-<key>.exe` from the stand-in's own
  address, GOG Galaxy's other bytes, an installer told where to go told `<dir>\<key>`).
- `dashboard --setup-registry <file>`: a file standing in for Windows' list (`source_kit::registry`'s format), read
  afresh at every look: the machine's own Uninstall keys are then never read.
- The stand-in installer (`crates/standin/src/set_up.rs`) writes only in the folder `STANDIN_SETUP` names: what it was
  run with (`runs.txt`), the app's folder with its uninstaller, and the app's key in `registry.json`; Battle.net's
  folder is a whole fake Battle.net (`source_battlenet::fixture`), read with `--battlenet <dir>\battlenet`. Its
  uninstaller hands over to a copy of itself and empties the folder. Without `STANDIN_SETUP` it writes nothing.

## 6. Tests

- `helpers::clients`: the table's shape (the vendors' hosts, a SHA-256 each); Windows' list read by key name and by
  display name, a hidden key passed over, the quiet uninstall commands (an MSI's `msiexec /x`, a key's own quiet
  string, Battle.net's own); the install commands; a download kept only once it is the file expected.
- `app::set_up`: a wrong SHA-256 refused with nothing run or kept; removal refused while a game plays, while another
  step works, and when Windows lists nothing.
- `crates/standin/tests/set_up.rs`: Battle.net downloaded, checked, run with its own switches, found in the list and
  read by its source, then removed by its own uninstaller; Steam's quiet switches both ways; the stand-in writing
  nothing without its folder.
- `screens::settings::set_up` and `crates/screens/tests/settings.rs` (`settings_set_up`): the questions' words, the
  Rows, focus and the effects, and pictures (`settings-set-up-box`, `settings-set-up`, `-question`, `-installing`, `-remove-question`).
- The payload `e2e/payloads/set-up`: Battle.net installed and removed with the pad against the stand-ins, GOG
  Galaxy's refused. Safe on a PC in daily use: nothing real is downloaded, installed or listed.

## 7. What only a real run on the TV PC can say

L1 of the research, per app: whether its installer is quiet with these switches, how many permission prompts a
whole set-up takes, whether its key is named as section 2 says, whether its quiet uninstall is quiet, and whether an
installer that starts its app ends (step 3's 20 s rule). Each `set-up:` line says what was run and what Windows'
list said, so the first real run answers them.
