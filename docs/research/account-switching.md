# Profiles bound to store accounts, switched with the Profile

Research for [#265](https://github.com/snrgarb/xbox-series-dashboard/issues/265), milestone **M10 Account switching**. The user's request, 2026-11-18: "I'd like to have the profile system be bound to specific steam, epic, battle.net and xbox accounts (i know xbox is harder because of credentials, maybe there could be a middleman executable that injects the credentials at runtime? take a look at Xodeus Nexus)".

The question, then: when a Profile is chosen at Who's playing today? or at Switch profile, can each launcher on the TV PC be moved to that Profile's account, on the user's own PC, with accounts that have each been signed in there before, and without the Dashboard ever holding a password?

Read on 18 November 2026, on Linux. **No store client was run, nothing was signed in to, and nothing was read from `data/` or the TV PC.** What follows is other programs' source code, Microsoft's documentation, the stores' own help pages and terms. It builds on [stores.md](stores.md) (where each store keeps its library), [installs.md](installs.md) (how the Dashboard drives each client) and [../sources.md](../sources.md) (what the Dashboard does today), and does not repeat them. Tags, as there:

- **[code]** read in the source of a program that does it, linked at the commit read.
- **[MS]** Microsoft Learn or Microsoft's own pages.
- **[docs]** a store's own help or terms, or a project's README about itself. "(excerpt)" where only a search engine's excerpt of the page could be read, because the page refused automated reading.
- **[reported]** one forum post, one project's notes, one article: not confirmed by a second.
- **[inferred]** my reasoning. A guess until the TV PC tests it.

Whether something breaks a store's terms is **my reading, not legal advice**: every such line is [inferred].

## Summary

| Client | Switch between accounts it already remembers, with no password? | How | Client restart | Confidence |
|---|---|---|---|---|
| **Steam** | **Yes** | `config\loginusers.vdf`: the target's `MostRecent`, `AutoLogin`, `RememberPassword` to 1 and every other row's to 0; `HKCU\Software\Valve\Steam` `AutoLoginUser` = its account name, `RememberPassword` = 1; Steam closed with `steam.exe -shutdown` first and started after. Steam keeps each remembered account's own session token, so nothing is copied | Yes | [code: TcNo]; Steam's own Change Account does the same with a click [reported] |
| **Epic Games Launcher** (current, `com.epicgames.eglv2`) | **Yes, by swapping a snapshot** | The launcher remembers **one** account. Its session is a Windows Credential Manager entry `credentials.com.epicgames.eglv2.prod` plus the folder `%LocalAppData%\com.epicgames.eglv2\auth` (DPAPI files). A snapshot of both is taken while each account is signed in, and put back to switch | Yes, killed | [code: TcNo]; single source |
| Epic, older launcher | Yes, by swap | `GameUserSettings.ini` `[RememberMe] Data` and `HKCU\Software\Epic Games\Unreal Engine\Identifiers` `AccountId` | Yes | [code: TcNo, legendary] |
| **Battle.net** | **Probably yes** | `%AppData%\Battle.net\Battle.net.config` `Client.SavedAccountNames`: the target's email put first; each account must have been signed in with the launcher's "remember" ticked | Yes, killed | [code: TcNo]; where the token itself lives is not known |
| **Xbox app / Microsoft Store / Gaming Services** | **Not by files. Only through Microsoft's own sign-in UI** | Two separate accounts: the **Xbox profile** (gamertag, achievements, friends) and the **Microsoft Store account** (what is owned and Game Pass). Both are held by Windows' Web Account Manager, not in files a program can swap. The Xbox app can switch between them through its own UI; whether a second account added to Windows switches without a password or PIN is untested | The Xbox app, perhaps; the games yes | [MS] for the two-account model; the rest [reported] / untested |
| GOG Galaxy | Yes, by swap | `HKCU\Software\GOG.com\Galaxy` `refreshToken`, `settings\userId`, `settings\username`, `%LocalAppData%\GOG.com\Galaxy\Configuration\config.json`, and much of `%ProgramData%\GOG.com\Galaxy` (its `galaxy-2.0.db` is the library) | Yes | [code: TcNo] |
| EA app | Yes, by swap, heavy | The whole of `%LocalAppData%\Electronic Arts\EA Desktop\` and `%ProgramData%\EA Desktop\` | Yes | [code: TcNo] |
| Ubisoft Connect | Yes, by swap | `%LocalAppData%\Ubisoft Game Launcher\` `ConnectSecureStorage.dat`, `user.dat`, `user.vdat`, `settings.yaml` | Yes | [code: TcNo] |

**Xodus Nexus** (the user's "Xodeus Nexus") is **Nexus Launcher**, formerly Xodus Nexus: an unofficial, MIT-licensed launcher that runs Microsoft Store and PC Game Pass games **on Linux**, on top of the GPL-3.0 **Xodus** project, which reimplements Microsoft account sign-in by **impersonating Windows' own sign-in host** and a Windows device. It does keep several Microsoft accounts and switch between them without a password, but only because it *is* its own Microsoft Store client holding its own refresh tokens in the Linux keyring. **It does not inject credentials into the Windows Xbox app, and nothing in it can.** It is worth reading to learn how Microsoft's sign-in works; it is not a model for the Dashboard (section 2.3).

**The user's middleman idea** (a helper that supplies Xbox credentials at run time): **do not build it.** Microsoft gives no API that signs an account into Windows or the Xbox app without its own UI; a password typed by a program is defeated by the second factor, by passkeys, and by accounts with no password at all (new Microsoft accounts are passwordless by default since May 2025); the only way around that is what Xodus does, impersonating Windows to Microsoft's servers, which is fragile, against the Microsoft Services Agreement's spirit if not letter, and puts every Profile's Microsoft account at risk (section 3.5).

**Recommended**, in five lines:

1. A Profile is bound, per launcher, to **one account the client already remembers**, picked from a list the Dashboard reads from the client's own files (Steam's `loginusers.vdf`, Battle.net's `SavedAccountNames`, a snapshot taken of Epic's session); never typed into the Dashboard.
2. At Signing you in and at Switch profile, the Dashboard switches only the launchers whose account differs: closes each cleanly, writes the client's own remembered-account files, starts it again silent, one at a time, Steam first, with "Signing you in" held until each says it is signed in (or 60 s).
3. A game running or a download under way stops the switch for that launcher and asks; the Profile still changes in the Dashboard.
4. Play time, achievements and owned games then follow the account: each source reads the bound account's files, not "the newest", and the database keeps per-account rows.
5. **Xbox is not switched by the Dashboard.** It is told which Microsoft account the Profile uses and, when the Xbox app shows another, offers to open the Xbox app's own account page. One Windows user per Profile is the only full answer for Xbox, and it is a different product (section 3.4).

## 1. Each client

What the Dashboard already does with each client, and so what a switch can break, is in [../sources.md](../sources.md) and [installs.md](installs.md). The parts that matter here:

- **Steam**: the library, play time and achievements are read from the **most recently written** `userdata\<account id>\config\localconfig.vdf` (`source_steam::signed_in_account`), and achievements from `appcache\stats\UserGameStats_<account id>_<appid>.bin` [code: this repository, `crates/source-steam/src/lib.rs`, `achievements.rs`]. Installs go through Steam's CEF debugger (port 8080 or 8081) where the user has turned it on, which Steam reopens at each start while `.cef-enable-remote-debugging` is in its folder [code: `crates/source-steam/src/cef.rs`].
- **Epic**: the library comes from the **most recently written** `cache\prod\account\<account>\library-cache.json` and from `catcache.bin` [code: `crates/source-epic/src/modern.rs`]. legendary, where the user added it, has its **own** sign-in in its own `user.json` [code: `crates/source-epic/src/legendary/mod.rs`].
- **Battle.net**: installs from the Agent's local HTTP API; installed games from `product.db`, which is the machine's, not an account's [installs.md 7, stores.md].
- **Xbox**: installed packages only, from the machine; nothing of an account's is read [sources.md].
- **The database**: `games.playtime_minutes`, `games.last_played`, `achievements.unlocked` and `play_sessions` are one per game, with **no account or Profile column** [code: `crates/library/src/lib.rs`, `achievements.rs`].

### 1.1 Steam

**Where the accounts are.**

- `<Steam>\config\loginusers.vdf`: one block per account Steam remembers, keyed by SteamID64, with `AccountName`, `PersonaName`, `Timestamp`, `WantsOfflineMode`, `MostRecent`, `AutoLogin`, `RememberPassword`, `SkipOfflineModeWarning`. TcNo reads `AutoLogin` as "current Steam's" marker of the active session and `MostRecent` as the older one [code: TcNo [`loginusers.go`][tc-lu] `ActiveSessionSteamID64`, [`vdf_write.go`][tc-vw]]. The ticket names the key `AllowAutoLogin`; TcNo's code reads and writes `AutoLogin`. Which one today's Steam writes is a TV PC look (S1).
- `HKCU\Software\Valve\Steam`: `AutoLoginUser` (the account **name**, not the id) and `RememberPassword` (DWORD) [code: TcNo [`autologin_windows.go`][tc-al]]. TcNo's comment: the account name "is the one thing Steam cannot sign in without, because AutoLoginUser is it" [code: [`known_accounts.go`][tc-ka]].
- **The session token itself** is not in either. Steam stores it encrypted, and "the way to decrypt it hasn't been published by anyone" (Dr. McKay, author of the node `steam-user` library, 2023) [reported: [dev.doctormckay.com][mckay]]. Where (a `ConnectCache` in a `config` or `local.vdf` file is the usual guess) and whether it is bound to the machine or the Windows user is **not known** [UNVERIFIED]. It does not matter for the design: nothing copies it; Steam keeps one per remembered account itself.

**Switching without a password: yes.** TcNo's Steam switch, in order [code: TcNo [`switcher.go`][tc-sw] `SwapToAccount`, `writeLoginUsersAndAutoLogin`]:

1. If the target already holds the session markers, do nothing.
2. Close Steam: `steam.exe -shutdown` ("Steam treats WM_CLOSE on its main window as minimise-to-tray ... The shutdown argument drains steam.exe and its steamwebhelper children") [code: [`native_quit_windows.go`][tc-nq]], then kill what is left: `steam.exe`, `steamwebhelper.exe`, `GameOverlayUI.exe` and the `Steam Client Service`.
3. In `loginusers.vdf`, for every row `MostRecent`, `AutoLogin`, `RememberPassword` = 0; for the target, all three = 1 (and `WantsOfflineMode`, `SkipOfflineModeWarning` to the offline choice). The old file is copied to `loginusers.vdf_last` first.
4. `AutoLoginUser` = the target's `AccountName`; `RememberPassword` = 1.
5. Optionally `AlwaysShowUserChooser` in `config\config.vdf`, and the persona state in the target's `localconfig.vdf`.
6. Start `steam.exe` with the user's arguments.

TcNo's own FAQ: "Account switching swaps local login state and asks for no password at all", and "Do not use the launcher's sign-out button" [docs: [TcNo FAQ][tc-faq]]: signing out from Steam drops that account's remembered session, and it then needs its password and Steam Guard again [reported: [Steam Community, 2023][steam-forum]; [TcNo wiki][tc-faq]].

Steam's own **Change Account** does the same from inside the client: "you don't have to provide anything when changing accounts; you just click the target account's avatar" [reported: Steam Community, 2023, as quoted by search], and a Steam Deck beta's change account dialog "shows a warning that proceeding will close active games" [reported: [SteamDeckHQ][deck-beta]].

**What it costs.** A full close and start of Steam. No measurement was found; my guess is 10 to 30 s on the TV PC to a signed-in, windowless Steam [inferred; T1 measures it]. What breaks:

- **Downloads** under way stop with Steam and resume when it next runs, for whichever account owns the game [inferred]. Another account's game installed on the PC stays installed and updates under any account that owns it [inferred].
- **Cloud saves**: a game closed by Steam's shutdown syncs on exit like any close; a game killed does not [inferred]. Hence: never switch while a game runs (section 5).
- **The Dashboard's Steam reads**: `signed_in_account` already follows the newest `localconfig.vdf`, which Steam writes at sign-in, so it follows a switch after Steam has signed in, not before [inferred]. Better to read the bound account's folder by id (section 5.5).
- **The CEF debugger**: dropped with Steam, reopened at Steam's next start while the file is there [code: this repository]. The Dashboard must reconnect, and fall back to `steam.exe` routes until it can.
- **Family sharing**: a game lent through Steam Families plays under the borrower's own account; switching accounts is not needed for it [inferred].

**Bound to the machine?** The remembered session is used only by the Steam on this PC; nothing is moved. TcNo backs up `config` and `userdata` but never copies a token between machines [code]. Whether a copied `loginusers.vdf` works on another PC was not looked at and does not matter here.

### 1.2 Epic Games Launcher

**The current launcher** (`com.epicgames.eglv2`, the one on the TV PC per stores.md) remembers **one** account at a time. TcNo's definition for it [code: TcNo [`Platforms.json`][tc-pj] "Epic Games", [`epic_new_launcher.go`][tc-epic]]:

- The session is **two things together**: a Windows Credential Manager generic credential with target `LegacyGeneric:target=credentials.com.epicgames.eglv2.prod`, and every file in `%LocalAppData%\com.epicgames.eglv2\auth` (it requires `auth.json`; the account id is read from `access-token-prod.dpapi`, which TcNo decrypts with DPAPI: `UniqueIdMethod: DPAPI_JSON_SELECT`). The display name is `auth.json`'s `prod.account_info.display_name`.
- **To save an account**: read the credential (`CredRead`) and the `auth` files, and write them as one snapshot, itself DPAPI-protected (`writeProtectedSession` → `DPAPIProtect`), keeping 3 recovery copies.
- **To switch**: kill `EpicGamesLauncher.exe` and `EpicWebHelper.exe` (`ClosingMethod: TaskKill`, `ExitBeforeInteract`), write the credential back (`CredWrite`) and the `auth` files, and clear `%LocalAppData%\com.epicgames.eglv2\EBWebView` (the launcher's WebView2 profile).
- TcNo validates the credential's size (at most 2560 bytes) and persistence (1 to 3) before writing it, and refuses links in the path [code].

The files and the credential are DPAPI-protected for this Windows user on this machine ([MS: `CryptProtectData`][dpapi]: "only a user with the same logon credential as the user who encrypted the data can decrypt the data. In addition, the encryption and decryption usually must be done on the same computer"). So a snapshot works only where it was taken, which is what is wanted.

**The older launcher** keeps its session in `%LocalAppData%\EpicGamesLauncher\Saved\Config\WindowsEditor\GameUserSettings.ini` `[RememberMe] Data` and the account id in `HKCU\Software\Epic Games\Unreal Engine\Identifiers` `AccountId` [code: TcNo "Epic Games (Legacy)"]. `Data` is base64 of a JSON array whose first item has a `Token` (a refresh token), AES-encrypted with keys legendary knows [code: [legendary `core.py` `auth_import`][lg-auth]]. That is why `legendary auth --import` works only with the old launcher (installs.md P3 asks whether it works with the new one: by this, it cannot).

**What it costs**: the launcher killed and started again; no figure found [inferred: 10 to 20 s]. What breaks:

- **Downloads** in the launcher stop; it resumes them [inferred].
- **Epic Online Services** in a running game: the game is signed in through the launcher's session; killing the launcher mid-game may sign the game out of its online features [inferred]. Never switch while an Epic game runs.
- **The Dashboard's Epic reads**: `library-cache.json` is per account folder already; the newest is read. After a switch, the new account's folder becomes newest only once the launcher has written it [inferred]. `catcache.bin` is one file for whoever signed in last [stores.md].
- **legendary is separate.** Its own sign-in (`user.json` under `LEGENDARY_CONFIG_PATH`) is not touched by the launcher's switch. Per Profile, the Dashboard can give legendary **its own folder per account** (`LEGENDARY_CONFIG_PATH=<data>\legendary\<account id>`), each signed in once through legendary's own page; then switching legendary costs nothing [inferred from [legendary `lgndry.py`][lg-cfg]]. Its `installed.json` would then also be per account, which is wrong for a machine's installs: keep one installs folder and point only the sign-in at the account, or move `installed.json` between them [inferred; a ticket of its own].

### 1.3 Battle.net

TcNo's definition [code: TcNo [`Platforms.json`][tc-pj] "BattleNet"]:

- The remembered accounts are `%AppData%\Battle.net\Battle.net.config`'s `Client.SavedAccountNames`, a comma-separated list of emails. The **first** is the one the client signs in as (`JSON_SELECT_FIRST,`).
- **To switch**: kill `Battle.net` (`ClosingMethod: TaskKill`), put the target's email first, start the client. When adding a new account, TcNo empties `SavedAccountNames` and deletes `%LocalAppData%\Battle.net\BrowserCaches\common\Network\Cookies*`.
- The BattleTag for the list is read from `%LocalAppData%\Battle.net\CachedData.db` (SQLite: `SELECT battle_tag FROM login_cache WHERE account_id_lo = ...`), the account id from the newest `Logs\battle.net-*.log`.

TcNo's FAQ: Battle.net "can detect saved accounts, but each account still needs the launcher's remember-password option enabled" [docs: [TcNo FAQ][tc-faq]]. So the client keeps a token per remembered email somewhere TcNo does not touch [inferred]; where (a file under `%LocalAppData%\Battle.net`, or Credential Manager) is **unknown** [UNVERIFIED; B1]. Blizzard's own forums say an update once stopped remembering emails [reported: [Blizzard forums][bn-regress]], which is the kind of change that would break this silently.

**What it costs**: the client killed and started again [inferred: 10 to 20 s]. **The Agent** (`Agent.exe`, the downloader the Dashboard talks to) is a separate process. TcNo does not kill it; whether a download through the Agent survives the client's restart, and whether the Agent follows the new account, is **unknown** [B2]. Installed games are the machine's (`product.db`); what an account may launch is checked by the client at launch [inferred].

### 1.4 Xbox app, Microsoft Store and Gaming Services

Section 3 has this in depth. In short: two Microsoft accounts are in play, the Store account (purchases and Game Pass) and the Xbox profile (identity) [MS: [mismatched store accounts][ms-mismatch]]. Both are Web Account Manager accounts, held by Windows' token broker for the Windows user, not in files. **There is no file to swap.**

### 1.5 GOG Galaxy, EA app, Ubisoft Connect (briefly)

From TcNo's definitions [code: TcNo [`Platforms.json`][tc-pj]]; each is one source, untested here:

- **GOG Galaxy**: `HKCU\Software\GOG.com\Galaxy` `refreshToken`; `...\Galaxy\settings` `userId` and `username`; `%LocalAppData%\GOG.com\Galaxy\Configuration\config.json`; and from `%ProgramData%\GOG.com\Galaxy`: `config.json`, `storage\galaxy-2.0.db*`, `storage\etags.db` and the web cache's cookies, local and session storage. Closes `GalaxyClient.exe`, `GalaxyClientService.exe`, `GalaxyCommunication.exe` and the notifications renderer. **Note**: `galaxy-2.0.db` is where the Dashboard reads GOG's owned games [stores.md], so swapping it swaps the library too, and it is in `%ProgramData%`, shared by every Windows user [inferred]. gogdl, like legendary, has its own sign-in file per `--auth-config-path` [installs.md].
- **EA app**: the whole of `%LocalAppData%\Electronic Arts\EA Desktop\` and `%ProgramData%\EA Desktop\` (logs left out), `cookie.ini` required; closes `EADesktop.exe` and `EABackgroundService.exe`. Heavy and brittle: a folder that holds settings and caches as well as the session [inferred].
- **Ubisoft Connect**: `%LocalAppData%\Ubisoft Game Launcher\` `ConnectSecureStorage.dat`, `user.dat`, `user.vdat`, `settings.yaml`, and the launcher log (from which the account id is read); closes `upc.exe`, `UplayService.exe`, `UplayWebCore.exe`; sets `user.closebehavior` to close, not to tray.

Whether any of these files is DPAPI-bound was not looked at; GOG's `refreshToken` in the registry reads as plain [UNVERIFIED].

## 2. How existing tools do it

### 2.1 TcNo Account Switcher

The best witness: open source (**GPL-3.0**), in active development, a definition per platform in one JSON file. Read at [`6f1ad81`][tc] (master, 16 Nov 2026; `Platforms.json` version 4.0.9, now in Go with Wails). The newest **published** release is `2025-11-20_03` [code: GitHub API], so master may be ahead of what users run.

- **Model**: for every platform but Steam, a "basic" flow driven by `Platforms.json`: `ExesToEnd`, `LoginFiles` (files, registry values `REG:...`, or a JSON path `JSON_SELECT...`), `PathListToClear`, `UniqueIdFile`/`UniqueIdMethod`, `ClosingMethod`. **Save current** copies the login files into TcNo's own folder for that account; **switch** closes the client, writes them back, starts the client. Steam has its own code (section 1.1), as does Epic's credential session (1.2).
- **No passwords** are kept for switching. Its optional Steam Guard feature does take the Steam password, which "is encrypted with a key fetched from Steam, sent to Steam, and never written to disk", and keeps Steam Guard secrets in a vault behind a password of the user's own (Argon2id) [docs: [TcNo FAQ][tc-faq]; [`docs/steam-guard-passwords-and-factors.md`][tc-sg]].
- **No Xbox or Microsoft Store** platform at all; the nearest is "Spotify (Windows Store)", a file swap in a Store app's `LocalState` [code].
- **A CLI**: `+s:<steamId64>` swaps Steam, `+<platformShort>:<uniqueId>` any other (`+b:`, `+eg:`...), `--list-accounts --json` lists them [code: [`internal/cli/help.go`][tc-cli]]. So the Dashboard **could** drive TcNo as a helper the user adds, as it drives legendary, instead of doing the file work itself. Against: TcNo is a whole desktop app with its own window, update checker and Discord presence; its saved accounts live in its own folder under its own lock; and its released build lags master [inferred].
- **Trust**: the code is careful (atomic writes, `.vdf_last` backups, recovery copies, link checks, logs that never record arguments "they can contain passwords or tokens"). It is a fair reference for *what* to swap [inferred]. Its licence means **read it, do not copy it** into the Dashboard (whose own licence is not decided, plan.md open question 5).

### 2.2 Others

- **accshift** (SourceForge): "captures each launcher's own remember-me session and restores the right one on switch" [reported: search excerpt only].
- **steam-acolyte** (PyPI): Steam only; "only stores login tokens, but never any passwords" [reported].
- Steam's own **Change Account**, Battle.net's **Allow multiple instances** (a second client window signed in to another account [reported: Blizzard forums]); Epic and the Xbox app have nothing like it beyond signing out and in.

None of these does Xbox.

### 2.3 Xodus Nexus

Searched as "Xodeus Nexus" (no results), then "Xodus Nexus", "Xodus" and "Nexus Launcher". **It is Nexus Launcher, called Xodus Nexus until 0.5.0** (9 Nov 2026) [docs: [GitHub releases][nx-rel]; [xodusnexus.com][nx-site], a fan guide that hosts no files].

- **What**: "Your Microsoft Store PC games, on Linux." An Electron launcher (TypeScript) with an Xbox-like look, **MIT**, by one developer (`domsvoboda/nexus-launcher`, 1.3k stars, created 11 Oct 2026, read at [`c239c3f`][nx], 17 Nov 2026). **Linux only**; Steam Deck experimental [docs: [README][nx]].
- **How**: three parts. A **backend**, "our patched build of Xodus, handling Microsoft account sign-in, genuine license checks, package downloads and launch preparation"; a **runtime** (forks of Proton, Wine and an open `xgameruntime.dll`); and the launcher [docs: README]. It says itself: "an independent, unofficial project built on Xodus. It is not an official project or release from the upstream Xodus developers."
- **Xodus** (`xodus-gaming/xodus`, **GPL-3.0**, Rust, read at [`49453a4`][xo], 8 Nov 2026): "The project can now login, download packages and obtain licenses for games." Its sign-in [docs: [`docs/xodus/login.md`][xo-login], [`device.md`][xo-dev]]:
  - It opens Microsoft's `InlineLogin.srf`, "the same hosted login page Windows loads inside `CloudExperienceHost` (the native shell that embeds sign-in webviews for TokenBroker)", in its own web view, **presenting as `CloudExperienceHost`**: it sends the `cxh-*` headers a real one would, and "re-implements just enough of it ... for the page to behave as if it were signing in through Windows".
  - It **registers a fake Windows device** with `deviceaddcredential.srf`, sending device components (MAC address, disk serials, board and SMBIOS details) and obfuscated "CLEP" blobs, and recovers the device's RSA key from the server's reply, then trades tokens at `RST2.srf` for Xbox Live tokens.
  - Tokens are kept in the OS keyring (`keyring_core`) [code: [`tokens/backend/keychain.rs`][xo-kc]].
- **Accounts**: since 0.5.0, optional **saved accounts**: "Makes a saved account the active Store account, no sign-in and no device reset" (`saved-accounts --use <name>`), and a separate **player** account from the **licence** account [code: [`src/main/saved-accounts.ts`][nx-sa]; docs: [xodusnexus.com][nx-site]: "manage several Store accounts and a separate Xbox player account"]. "Your credentials are stored in your system keyring" (KWallet or GNOME Keyring) [docs: site]. "Xodus owns sign-in credentials; the launcher should not copy them" [docs: [`SECURITY.md`][nx]].
- **Xbox on Windows**: nothing. It never touches Windows' Xbox app, Store or Web Account Manager; it replaces them.
- **Trustworthy to learn from?** As documentation of Microsoft's sign-in protocol: yes, it is the most detailed public account, and it says plainly which parts are reverse-engineered [inferred]. As something to build on: **no**. It impersonates a Windows component and a Windows device to Microsoft's servers, which Microsoft can detect and stop at any time; its README warns of the Microsoft Store's 10-device limit ("licensing code 501"), which a fake device counts against; xodusnexus.com warns "Microsoft does not support or endorse third-party launchers"; and on Windows the real Xbox app, Gaming Services and the games' own `XUser` calls would not use its tokens anyway [inferred]. The idea worth taking from it is only the split it makes visible: **the player and the licence holder are two accounts**, which Windows has too (section 3.1).

## 3. Xbox in depth

### 3.1 Who decides the signed-in account

- **Two accounts.** "The `XStoreContext` on PC titles in production is linked to the Microsoft Account (MSA) that is currently signed into the Microsoft Store app ... This account can sign out or switch with another account at any time without notice. Additionally, the Microsoft Store account can be switched from the Xbox app based on user settings." And: "the Xbox services account can be switched with the sign-in flow in-game to create an account mis-match between the Xbox and Microsoft Store apps. The mismatched account status can be intentional for family or shared PC scenarios." Microsoft's own figure: "10-15% of Xbox services users have a different MSA signed into the Microsoft Store app than Xbox services in a Windows session" [MS: [Handling mismatched store account scenarios on PC][ms-mismatch]].
- **What each does**: the Store account holds what is owned and the Game Pass membership; the Xbox profile is the gamertag, achievements, friends and cloud saves [docs (excerpt): [support.xbox.com, "How your PC gaming account relates to your Microsoft Store account"][xb-store]]. In the Xbox app: gamerpic > Settings > Account > "Change my Microsoft Store account" [docs (excerpt): same]. "You can only sign in to the Xbox app for Windows PC with one Microsoft account at a time, but you can switch between accounts" [docs (excerpt): [support.xbox.com, "Switch between Microsoft accounts for gaming on your Windows device"][xb-switch]]. Both pages are rendered by script and could not be read whole.
- **Installed games and licences**: if the PC plays games bought by another account, "their account must be set as the Store account. Otherwise, games already installed using that account won't launch because they weren't purchased using your Microsoft account" [docs (excerpt): [xb-store]]. So **switching the Store account changes which installed games start**, and a Game Pass game starts only while a Store account with Game Pass is signed in [inferred from the same].
- **In a game**: a GDK game asks Windows for its player with `XUserAddAsync`; `AddDefaultUserSilently` returns "the user who launched the game", and with more than one user signed in and no default, it fails and the game must show Microsoft's account picker [MS: [`XUserAddAsync`][ms-xuser], [`XUserAddOptions`][ms-xuseropt]].
- **Underneath**: Windows' **Web Account Manager** (WAM) holds the Microsoft accounts added to the Windows user ("Accounts used by other apps" in Settings > Accounts > Email & accounts); an app finds them with `WebAuthenticationCoreManager.FindAllAccountsAsync` and asks for a token silently, showing `AccountsSettingsPane` only when it must [MS: [Web Account Manager][ms-wam], [`FindAllAccountsAsync`][ms-findall]]. The **Xbox Live Auth Manager** service (`XblAuthManager`) "provides authentication and authorization services for interacting with Xbox Live" [MS: [services list][ms-svc]]; its COM interface `IXblIdpAuthManager` is documented only as "Reserved for Microsoft use" [MS: [xblidpauthmanager.h][ms-xblidp]].

### 3.2 Can two Microsoft accounts be remembered and switched without a password?

- **Remembered: yes.** WAM keeps several accounts per Windows user [MS: [ms-wam]], and Microsoft's own GDK page treats a different Store and Xbox account on one Windows session as normal [MS: [ms-mismatch]].
- **Switched without the password or PIN: unknown.** No Microsoft page says what the Xbox app's account switch asks for when the target is already in WAM. Community answers only describe signing out, "Use a different account", and removing and re-adding the account in Email & accounts [reported: Microsoft Q&A threads]. One 2021 Steam forum post says changing "allow apps to sign me in" under that list made a game ask for email and password instead [reported, single]. My guess: an account already added to Windows for apps switches with a click, as WAM's silent token request is meant to allow; one that is not asks for its full sign-in [inferred]. **X1 settles it.**
- **What a switch does**: the Xbox profile change signs the old gamertag out of the Xbox app and Game Bar; a running game keeps its `XUser` until it asks again [inferred]. A Store account change changes which installed games launch, at once (3.1).

### 3.3 The Xbox app's own switcher, driven by UI Automation

The Xbox app is a packaged app whose controls UI Automation can see [inferred]. The Dashboard could open the Xbox app, walk to gamerpic > Switch account (or Settings > Change my Microsoft Store account), and pick the Profile's account by name. **Fragile**: the app's layout changes with its updates (monthly or faster), names are localised, and any prompt (password, PIN, consent, "keep me signed in") stops it [inferred]. It also shows the Xbox app's window over the Dashboard, which shell mode exists to avoid. **Do not build it** as the main route; at most, "Open the Xbox app's account page" as a button, leaving the clicks to the user (section 5).

### 3.4 One Windows user per Profile

The only route that gives each Profile its own Xbox app, Store, WAM and every launcher's session with nothing swapped: each family member has their own Windows user, and choosing a Profile is a Windows **fast user switch** [inferred].

- **Shell mode is per user**: the Dashboard's shell is `HKCU\...\Winlogon\Shell` [docs: this repository, [../spec/shell-mode.md](../spec/shell-mode.md)], so each Windows user needs it installed and set separately, and each has its own Dashboard, database, Profiles and logs.
- **A switch** goes through Windows' lock screen and sign-in (password, PIN, Windows Hello) for the target user, and the first user's session, Dashboard, launchers and any game keep running behind, holding memory and the GPU's attention [inferred]. Signing in automatically without Windows' prompt needs the target's Windows password stored for auto-logon (`AutoAdminLogon`), the very thing to avoid.
- **Launchers**: Steam, Epic, Battle.net and the rest are installed once for the machine but run and sign in per user; whether two users' Steams can run at once (one per session) is **not known** [UNVERIFIED]. Store packages are installed per user, so each Windows user installs (registers) an Xbox game again, reportedly without a second download [reported, not checked].
- **For a TV**: a Windows lock screen between "Who's playing today?" and Home is not a console. **Not recommended** as the default; worth saying in Settings as the way to give an Xbox account its own seat [inferred].

### 3.5 The middleman executable that supplies credentials

The user's idea, assessed squarely.

**What it would have to store.** Either the Microsoft account **password** (and something for the second factor), or a **refresh token**. There is no third thing.

**Where.** At best Windows Credential Manager or a DPAPI blob, both protected by the Windows user's own key: "only a user with the same logon credential ... usually ... on the same computer" [MS: [dpapi]]. That protects it from other users and other PCs, **not from any program running as this user**, malware included, which can call `CredRead` or `CryptUnprotectData` as TcNo does [code: TcNo]. Since the Dashboard runs as the one Windows user every family member uses, every Profile's stored secret is readable by every other Profile's games and programs [inferred].

**How it would hand them over.** This is where it fails:

- **No API signs a Microsoft account into Windows, the Xbox app or the Store from a program.** WAM's account add goes through `AccountsSettingsPane` or `RequestTokenAsync` with UI; Microsoft's guidance is that users "should not type in credentials in windows that pop up within the operating system with no attached context" [MS: [MSAL.NET with WAM][ms-msal-wam]]. `IXblIdpAuthManager` is "Reserved for Microsoft use" [MS: [ms-xblidp]]. So the helper would have to **type into Microsoft's sign-in window** (UI Automation or `SendInput`) [inferred].
- **Second factors** stop that: an account with two-step verification asks for an Authenticator approval, a code or a security key after the password [inferred].
- **Passwordless accounts have no password to type.** "Brand new Microsoft accounts will now be "passwordless by default."" and "Existing users can visit their account settings to delete their password." [MS: [Microsoft Security blog, 1 May 2025][ms-passkey]]. A passkey or Windows Hello cannot be replayed by a program by design [inferred].
- **Storing a refresh token instead** means getting one, which means being a Microsoft sign-in client: Xodus's route of impersonating `CloudExperienceHost` and registering a fake device (2.3). Those tokens are then the helper's, not Windows'; the Xbox app, the Store and the games' `XUserAddAsync` would never see them [inferred].
- **Terms**: "You cannot transfer your Microsoft account credentials to another user or entity" and "To protect your account, keep your account details and password confidential" [docs: [Microsoft Services Agreement][msa], section 4.a.i, published 30 July 2026]. A program holding family members' passwords to type them is at least against the second [inferred].

**Verdict.** What is workable: nothing that injects credentials. What is fragile: driving the Xbox app's own switcher by UI Automation (3.3). What I should not build: **a credential store, a password typer, or a token client of our own for Microsoft.** The workable Xbox story is: the Profile knows which Microsoft account is theirs; the Dashboard says when the Xbox app's account is another and opens the Xbox app's own account page for them; and Windows' own "Accounts used by other apps" is where the second account lives, remembered by Windows, not by the Dashboard (X1 says how smooth that is).

## 4. What the Dashboard must never keep or do

- **Never ask for, keep or log a password, a Steam Guard code, a token or a cookie** of any store. Not in the database, not in settings, not in logs, not in a crash report. Log lines name the launcher, the account's public name and id, and what was done.
- **Never copy a session to another PC or another Windows user.** Only the clients' own remembered-session files that already live on this PC under this Windows user are touched. Most are DPAPI-bound anyway (1.2).
- **At most, swap the clients' own files back and forth.** Where a client remembers several accounts itself (Steam, Battle.net), only its own "which one" markers are changed. Where it remembers one (Epic), a snapshot of its own session files is kept, **DPAPI-protected for this Windows user, in the Dashboard's data folder**, as Discord's tokens already are (docs/discord.md), with the user's knowledge, removable per account.
- **Back up before writing**, write atomically, and restore the backup if the client does not come up signed in (TcNo's `.vdf_last` and recovery copies) [code: TcNo].
- **Never sign anyone out** of a client; that drops the remembered session (1.1).
- **Never switch under a running game or a running download**; ask.
- **Never drive a Microsoft sign-in**, type into any store's sign-in window, or impersonate a client to a store's servers.
- **Never use one account for another person.** The stores' terms, quoted:
  - **Steam**: "You may not reveal, share or otherwise allow others to use your password or Account except as otherwise specifically authorized by Valve" (section 1.C); and "You may not use Cheats, mods, hacks, or any other unauthorized third-party software ... in interacting with or controlling the processes or user interface of Steam Content and Services" (4.B) [docs: [Steam Subscriber Agreement][ssa], revision of 10 September 2026, read through a summarising fetch].
  - **Epic**: "You may not share your Epic Games Account credentials with others. You can only use your own Epic Games Account that is registered to you. You can't sell it, give it away, trade it, or use someone else's Epic Games Account, even if they let you" [docs: [Epic Games Terms of Service][epic-tos], last updated 20 October 2025, effective 28 November 2025].
  - **Blizzard**: "you cannot share the Account or the Login Information with anyone, unless the terms of this Agreement allow it" (1.A.iii.2); prohibited is "any code and/or software, not expressly authorized by Blizzard, that can be used in connection with the Platform" (1.C.ii.4) and "any connection using third-party programs or tools not expressly authorized by Blizzard" (1.C.ix) [docs: [Blizzard EULA][bn-eula], last revised 21 March 2024].
  - **Microsoft**: section 3.5 above.

  My reading [inferred]: each Profile using **its own person's** account on a shared PC is what these terms expect; the switcher only changes which remembered account a client starts as, which the client's own Change Account does too. Blizzard's 1.C.ii.4 is broad enough to cover any outside program that edits its files; the Dashboard already reads Battle.net's files and drives its Agent (installs.md), so this adds no new kind of risk, but it is one. Steam's 4.B is about controlling Steam's processes or UI; writing `loginusers.vdf` while Steam is closed does neither, but closing Steam arguably does. No report was found of an account banned for using TcNo's file swaps [inferred from the absence of reports in what was searched; weak].

## 5. Recommended design

### 5.1 Binding a Profile to accounts

- A Profile's page in Settings gains **Accounts**: a row per launcher installed (Steam, Epic, Battle.net, GOG, EA, Ubisoft, Xbox), each showing the bound account or "Any" (the default: nothing switched).
- **Choosing an account** lists those the client already remembers, read from its own files: Steam's `loginusers.vdf` rows (persona name and avatar from `userdata`), Battle.net's `SavedAccountNames` (BattleTag from `CachedData.db`), Epic's **saved snapshots** plus "the account signed in now". Nothing is typed. An account not in the list: "Sign in to it in <launcher> once, with Remember me, then come back."
- **Epic** (one remembered account): "Save this sign-in for <Profile>" takes the snapshot of the launcher's session as it is now (1.2). To add a second account the user signs in to it in the launcher and saves again.
- **Xbox**: the row records which Microsoft account the Profile uses, read from what the Xbox app is signed in as when the user picks "This one", by its gamertag; nothing is switched (5.3).
- Stored in the database: `profile_accounts (profile, source, account_id, display_name)`; for Epic, the snapshot file's name. No secret is in the database.

### 5.2 At "Who's playing today?" and at Switch profile

1. The Profile is signed in at once, as today (start-up.md 6). **Signing you in** stays up, its spinner turning, past its 3 s **while any launcher is being switched**, with one line under it: "Switching Steam to <persona>..." then "Switching Epic Games...".
2. For each launcher whose bound account is not "Any" and differs from the one signed in now, **one at a time**, Steam first (the Dashboard's main source), then Epic, Battle.net, the rest:
   - running and busy (a game of its own running, a download in the Queue): **skip it** and say so later in a Toast ("Steam stays on <old> while <game> runs");
   - close it cleanly (`steam.exe -shutdown`; the others by their window close, then killed after 10 s), wait for its processes to be gone;
   - back up and write its files (1.1 to 1.5);
   - start it silent (`steam.exe -silent`; the others minimised to tray where they can be);
   - wait for "signed in": Steam's `loginusers.vdf` target row with the new `Timestamp`, or the CEF debugger answering with that account; Epic's `auth` folder rewritten; Battle.net's newest log naming the account. **Limit 60 s** per launcher.
3. Home then shows. A launcher that did not come up signed in gets a Toast: "<launcher> needs you to sign in" and stays on its own sign-in window, as today when a session has expired; the backup is put back if the client failed to start at all.
4. **Same account already, or only one account remembered, or "Any"**: nothing is done; no delay.
5. **Waking from sleep** signs the Profile in again (start-up.md 8); nothing is switched unless the Profile changed.

**How long**: 0 s when nothing differs; about 15 to 40 s per launcher that switches [inferred; T1 to T3 measure it]. The order and the "skip it" rule keep the worst case from blocking Home: after **30 s in total**, Home shows and the rest finish behind it, each with its Toast.

### 5.3 Xbox

- At Signing you in, if the Profile has an Xbox account and the Xbox app's signed-in gamertag (read how? see X2; else not read at all) is another, a Toast: "The Xbox app is signed in as <other>" with A = **Open the Xbox app's account page** (`ms-xbox://` or the Xbox app's Settings, X3), leaving the switch to the user and to Microsoft's UI.
- No Microsoft account is switched by the Dashboard, no credential is kept.
- Settings says once, plainly: "Xbox accounts are Windows' to switch. For each person to have their own Xbox account without switching, give them their own Windows user."

### 5.4 A game running, or a download under way

- **A game of that launcher running**: that launcher is not switched; the Profile changes in the Dashboard; the Toast says why; the switch is offered again when the game ends ("Switch Steam to <persona> now?").
- **A download in the Queue for that launcher**: asked once: "Steam is downloading <game>. Switch now and pause it, or later?" Later = when the Queue for that launcher is empty.
- **The Steam CEF connection** is dropped and remade after the restart; installs fall back to `steam.exe` routes until it answers (sources.md already does this when the debugger is off).

### 5.5 The library, play time and achievements follow the account

- **Sources read the bound account, not "the newest"**: `source-steam` takes the account id to read (`userdata\<id>`, `UserGameStats_<id>_*`) from the signed-in Profile's binding, falling back to today's newest-`localconfig` rule for "Any"; `source-epic` reads `cache\prod\account\<bound>\library-cache.json`; GOG's database is whatever Galaxy holds (it is swapped with the account).
- **The database** keeps what is per account per account: owned-not-installed rows, `playtime_minutes`, `last_played` and `achievements.unlocked` gain an `account` key (or move to per-account tables), and `play_sessions` gain the Profile that played. Installed games stay the machine's.
- After a switch, the source re-reads (as at start), and Home and My games show the new account's library and play time; the greeting Toast waits for that, so "Hi Sam!" and Sam's Home come together [inferred: a choice for the user, question 4].

### 5.6 Tickets for M10, smallest first

M10 depends on nothing in **M9 Full Discord**. It depends on what is built: Profiles and Signing you in (M5: #125, #127), the Queue and installs (M8, for "a download under way"), Steam's CEF route (#132/#171), legendary (#175). If the user wants a Profile bound to a Discord account too (question 6), that ticket would come after M9.

| # | Ticket | Done when | Stand-in or real |
|---|---|---|---|
| A1 | **Profile accounts in the database**: `profile_accounts`, the `ProfileChange` to bind and unbind, the log lines | A Profile can be bound to "Any" or an account per source; the binding survives a restart; tests on the library alone | Stand-in |
| A2 | **Steam: list the remembered accounts** from `loginusers.vdf` (parser of made-up files: `AccountName`, `PersonaName`, `MostRecent`, `AutoLogin`, `RememberPassword`, `Timestamp`) and which is active | `source_steam::accounts(root)` returns them; tested on made-up files, including a broken one | Stand-in |
| A3 | **Settings > Profile > Accounts** page: a row per installed launcher, the list from A2 (others "Any" only for now), `--log-focus` lines | Binding Steam from the pad works on the stand-in Steam; screenshot test | Stand-in |
| A4 | **Steam reads by account**: `signed_in_account` replaced by the bound id where there is one; play time, achievements, owned games per account in the database; `play_sessions` with the Profile | Two made-up `userdata` accounts in the fixture show two libraries and two play times by Profile | Stand-in |
| A5 | **Steam switch**: close, back up, write `loginusers.vdf` and `AutoLoginUser`, start silent, wait for signed in, put the backup back on failure; skipped while a game runs or the Queue is busy; Signing you in held with its line | On the stand-in Steam (a fake `steam.exe` honouring `-shutdown` and reading the files), the right account comes up; **then T1 on the TV PC** | Stand-in, then **real Steam** |
| A6 | **Battle.net**: list `SavedAccountNames` (+ BattleTag from `CachedData.db`), switch by putting one first | Made-up config files; **then B1, B2 on the TV PC** | Stand-in, then **real client** |
| A7 | **Epic snapshot**: "Save this sign-in" (Credential Manager entry + `auth` folder, DPAPI-wrapped in the data folder), restore to switch, remove | DPAPI and Credential Manager behind a trait, tested with a fake on Linux; **E1, E2 on the TV PC** | Stand-in for the logic, **real launcher** for the rest |
| A8 | **legendary per account**: its sign-in file per bound Epic account, its installs shared | Two made-up `user.json` folders; the right one used per Profile | Stand-in |
| A9 | **Xbox: name the account and say when it differs**, with the button to the Xbox app's account page | Needs X2 (how to read the Xbox app's gamertag) first; Toast tested on the stand-in | **Real Xbox app** for X1 to X3 |
| A10 | **GOG, EA, Ubisoft** switches by snapshot, one ticket each, only if the user asks (question 5) | As A7, per client | **Real clients** |
| A11 | **Settings > Accounts help** and the "own Windows user" note | Text in, screenshot | Stand-in |

A1 to A4 change nothing on the TV PC and can be built now. A5 is the first that writes a real client's files.

## 6. What needs trying on the TV PC

Each by the lead, by hand, with the user's own accounts, **after a backup** of the files named (copy them aside first; never into the repository or a ticket). Two accounts per launcher are needed; a family member's own, signed in on the PC once with Remember me.

**Steam**

- **S1. Which keys.** With Steam signed in to account A and B remembered, open `<Steam>\config\loginusers.vdf` and note **the key names only** of A's block (is it `AutoLogin` or `AllowAutoLogin`? is `MostRecent` there?), and `reg query HKCU\Software\Valve\Steam /v AutoLoginUser` (note only that it is A's account name). Look for: the key names TcNo uses.
- **T1. A switch by hand, timed.** `steam.exe -shutdown`; wait until `steam.exe` and `steamwebhelper.exe` are gone (time it); edit as 1.1 steps 3 and 4 for B; `steam.exe -silent`; time until B is signed in (the tray menu, or `userdata\<B>\config\localconfig.vdf` rewritten). Look for: no password asked, no Steam Guard, the time from shutdown to signed in, whether a window showed.
- **T2. Steam's own Change Account.** Steam > Change Account, pick A. Look for: whether it asks for anything; whether `loginusers.vdf` changes the same way as T1 (key names only).
- **T3. The Dashboard after a switch.** With the Dashboard running and the CEF debugger on, do T1. Read the log: does the CEF connection come back, and does the Steam source (at its next read) show B's play time? Look for: the `store:` lines and the library count.
- **T4. A download.** Start a small download as A, switch to B as T1. Look for: does it resume under B, is it paused, is it gone.

**Epic**

- **E1. What the session is.** Signed in as A: `cmdkey /list | findstr epicgames` (note the target name only), and list the file names in `%LocalAppData%\com.epicgames.eglv2\auth`. Look for: the target `credentials.com.epicgames.eglv2.prod` and `auth.json`, `access-token-prod.dpapi`.
- **E2. A swap by hand.** Close the launcher (tray > Exit; then confirm `EpicGamesLauncher.exe` and `EpicWebHelper.exe` are gone); copy `auth` aside as A; sign in to B in the launcher with Remember me; close it; copy `auth` aside as B. Credential Manager's entry cannot be copied with built-in tools, so this test tries **files only** first: put A's `auth` back, start the launcher. Look for: signed in as A, as B, or signed out. If signed out, the credential is needed too (A7 must snapshot it, as TcNo does); that is expected.
- **E3. legendary.** `legendary status` before and after E2. Look for: legendary stays on its own account (it should).

**Battle.net**

- **B1. Remembered accounts.** Signed in as A with "Keep me logged in", sign out to the account picker, sign in as B the same way. Then: the client closed, `%AppData%\Battle.net\Battle.net.config`'s `SavedAccountNames` (note the order only), put A first, start the client. Look for: signed in as A with no password. Also `cmdkey /list | findstr -i battle` (names only) to see whether its token is in Credential Manager.
- **B2. The Agent.** Start a small install through the Dashboard (the Agent route), then do B1. Look for: does `Agent.exe` keep running, does the install continue, does `GET /install/<uid>` still answer.

**Xbox**

- **X1. Two accounts in the Xbox app.** Settings > Accounts > Email & accounts > "Accounts used by other apps": is the second Microsoft account there? If not, add it there (its owner signs in). Then in the Xbox app: gamerpic > Switch account (or Sign out > sign in) to B. Look for: does it ask for B's password, PIN or an approval, or switch with a click? Then the same for Settings > Account > Change my Microsoft Store account. Then start a Game Pass game under each. Look for: which starts, which says it is not owned.
- **X2. Reading the Xbox app's gamertag.** Is there a file or setting naming the signed-in gamertag without a sign-in of our own (look in `%LocalAppData%\Packages\Microsoft.GamingApp_8wekyb3d8bbwe\LocalState` and `\Settings` for a gamertag or XUID, names only)? Look for: anything stable. If none, A9 shows the account it was told, and cannot say when it differs.
- **X3. Opening the account page.** `start ms-xbox:` and `start ms-windows-store://settings`. Look for: which link opens the Xbox app's or Store's account settings directly.

## Open questions for the user

1. **Xbox**: is "the Dashboard names the account and opens the Xbox app's account page; you switch it there" enough? The alternative that truly switches is one Windows user per person (3.4), with Windows' lock screen in between.
2. **The middleman executable**: this research recommends **not** building one (3.5). Agree?
3. **Epic's snapshot**: may the Dashboard keep a DPAPI-protected copy of the Epic launcher's session per account in its data folder (as TcNo does), or only switch launchers that remember several accounts themselves (Steam, Battle.net)?
4. **Play time and achievements**: per account (what the store says for that account, the recommendation) or per Profile (what the Dashboard saw that person play, across accounts)?
5. **GOG, EA, Ubisoft**: switch them too (whole-folder snapshots, brittle), or leave them on "Any"?
6. **Discord**: should a Profile also be bound to its own Discord account once M9 is in?
7. **A download under way**: ask each time (recommended), or always wait, or always switch and pause?
8. **TcNo as a helper**: drive TcNo's CLI for the file work, as legendary is driven, rather than the Dashboard doing it itself? Recommended: no (a whole desktop app; GPL; release lags master), but it would cover more launchers sooner.

[tc]: https://github.com/TCNOco/TcNo-Acc-Switcher/tree/6f1ad81e80703da600c8f6d3263a288a23bf698f
[tc-pj]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/Platforms.json
[tc-sw]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/switcher.go
[tc-lu]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/loginusers.go
[tc-vw]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/vdf_write.go
[tc-al]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/autologin_windows.go
[tc-ka]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/known_accounts.go
[tc-nq]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/native_quit_windows.go
[tc-epic]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/basic/epic_new_launcher.go
[tc-cli]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/cli/help.go
[tc-sg]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/docs/steam-guard-passwords-and-factors.md
[tc-faq]: https://github.com/TCNOco/TcNo-Acc-Switcher/wiki/FAQ---More-Info
[mckay]: https://dev.doctormckay.com/topic/4380-is-there-way-to-extract-refreshtoken-from-steam-client/
[steam-forum]: https://steamcommunity.com/discussions/forum/1/3875966763780751664
[deck-beta]: https://steamdeckhq.com/news/steam-deck-beta-client-update-account-switching/
[lg-auth]: https://github.com/legendary-gl/legendary/blob/170964e/legendary/core.py
[lg-cfg]: https://github.com/legendary-gl/legendary/blob/170964e/legendary/lfs/lgndry.py
[bn-regress]: https://us.forums.blizzard.com/en/blizzard/t/regression-launcher-cannot-remember-usernameemail-anymore/56298
[nx]: https://github.com/domsvoboda/nexus-launcher/tree/c239c3fd4c38582d66fb148ccb13317581b40af4
[nx-sa]: https://github.com/domsvoboda/nexus-launcher/blob/c239c3fd4c38582d66fb148ccb13317581b40af4/src/main/saved-accounts.ts
[nx-rel]: https://github.com/domsvoboda/nexus-launcher/releases
[nx-site]: https://xodusnexus.com/
[xo]: https://github.com/xodus-gaming/xodus/tree/49453a4a458f8d9d0e8c93de6319d60b4291fa59
[xo-login]: https://github.com/xodus-gaming/xodus/blob/49453a4a458f8d9d0e8c93de6319d60b4291fa59/docs/xodus/login.md
[xo-dev]: https://github.com/xodus-gaming/xodus/blob/49453a4a458f8d9d0e8c93de6319d60b4291fa59/docs/xodus/device.md
[xo-kc]: https://github.com/xodus-gaming/xodus/blob/49453a4a458f8d9d0e8c93de6319d60b4291fa59/crates/xodus/src/tokens/backend/keychain.rs
[ms-mismatch]: https://learn.microsoft.com/en-us/gaming/gdk/docs/store/commerce/pc-specific-considerations/xstore-handling-mismatched-store-accounts
[ms-xuser]: https://learn.microsoft.com/gaming/gdk/docs/reference/system/xuser/functions/xuseraddasync
[ms-xuseropt]: https://learn.microsoft.com/gaming/gdk/docs/reference/system/xuser/enums/xuseraddoptions
[ms-wam]: https://learn.microsoft.com/windows/uwp/security/web-account-manager
[ms-findall]: https://learn.microsoft.com/uwp/api/windows.security.authentication.web.core.webauthenticationcoremanager.findallaccountsasync
[ms-msal-wam]: https://learn.microsoft.com/entra/msal/dotnet/acquiring-tokens/desktop-mobile/wam
[ms-svc]: https://learn.microsoft.com/windows-server/security/windows-services/security-guidelines-for-disabling-system-services-in-windows-server
[ms-xblidp]: https://learn.microsoft.com/windows/win32/api/_xblidp/
[ms-passkey]: https://www.microsoft.com/en-us/security/blog/2025/05/01/pushing-passkeys-forward-microsofts-latest-updates-for-simpler-safer-sign-ins/
[dpapi]: https://learn.microsoft.com/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata
[xb-store]: https://support.xbox.com/en-US/help/account-profile/manage-account/gaming-and-store-accounts
[xb-switch]: https://support.xbox.com/en-US/help/account-profile/manage-account/switch-accounts-xbox-app-windows
[msa]: https://www.microsoft.com/en-us/servicesagreement
[ssa]: https://store.steampowered.com/subscriber_agreement/
[epic-tos]: https://cdn1.epicgames.com/eulatracking-download/epicgames_tos/en/v2/r1/45d4dd9e67fab2855ba409cd77569499.pdf
[bn-eula]: https://www.blizzard.com/legal/2c72a35c-bf1b-4ae6-99ec-80624e1b429c
