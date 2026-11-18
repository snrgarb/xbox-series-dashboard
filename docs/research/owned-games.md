# Every library's owned games, not only the installed ones

Research for [#276](https://github.com/snrgarb/xbox-series-dashboard/issues/276), milestone **M8 Installs**. The user's question, 2026-11-18: "Can you import the games list to install for not just battle.net but for ALL libraries?"

So, for each of Steam, Epic, GOG, Battle.net, EA, Ubisoft Connect and Xbox: how the Dashboard can list the games the account owns (and, where it differs, those a subscription gives), so that a game not installed is on Home and in My games with Install on it.

Read on 18 November 2026, on Linux. **No store client was run, nothing was signed in to, nothing was read from `data/` or the TV PC.** What follows is this repository's own crates, other programs' source code (each at the commit read), Microsoft's documentation and one wiki. It builds on [stores.md](stores.md) (where each store keeps its library), [installs.md](installs.md) (how each installs), [account-switching.md](account-switching.md) (what the Dashboard may keep) and [../sources.md](../sources.md) (what the Dashboard does today), and does not repeat them. Tags, as there:

- **[code]** read in the source of a program that does it, linked at the commit read.
- **[docs]** the store's or Microsoft's own documentation, or a project's README about itself.
- **[reported]** one wiki, one issue, one project's notes: not confirmed by a second.
- **[inferred]** my reasoning. A guess until a signed-in PC tests it.
- **[UNVERIFIED]** looked for and not settled.

**Since (2026-11-18, the user: "the big one to finish building is 276").** B-O1 is answered from a read-only copy of the TV PC's Battle.net files: `CachedData.db` holds the account's licences, and the client's `Cache` its whole catalogue as plain JSON with the rules from licences to products, so **Battle.net's owned games need no sign-in** (`source_battlenet::cache`, which says what was proven and what is inferred; docs/sources.md "Which games can be installed"). Built for #276: ticket 1 (the owned list as a file, `source_kit::owned`, refreshed at start and on Update now by `Source::refresh_owned`), ticket 6 from the cache (decision 4 taken as recommended: unowned catalogue games hidden once the list is known), ticket 4's B-O1 counts in `e2e/htpc/probes/battlenet.ps1` (B-O2 not run: not needed now), and, in a pull request of its own, ticket 2 (GOG through gogdl, decision 6 as recommended: `source_gog::gogdl::library`, its stand-in `standin serve-gog-web`, the GOG part of section 10's `store-web`, under `DASHBOARD_STORE_WEB`). Ticket 3 is #277/#282. **Tickets 5, 7 and 8 (the store sign-in page, EA's owned games and EA's Install) wait for M9's web view host** and are as written below; with Battle.net's list local, ticket 5 is now for EA alone.

## Summary

**What the Dashboard lists today**, from each crate's own header (`crates/source-*/src/lib.rs`):

| Store | Not installed, listed today | From |
| --- | --- | --- |
| Steam | **Owned** | `appcache/packageinfo.vdf`: the apps the cached packages grant, with no sign-in of the Dashboard's |
| Epic | **Owned** | `Data\Catalog\catcache.bin` (the launcher signed in once), the eglv2 launcher's `library-cache.json`, and legendary's `metadata\` where the user added legendary |
| GOG | **Owned, with Galaxy** | `galaxy-2.0.db` (Galaxy signed in once). gogdl lists none |
| Ubisoft Connect | **Owned** | the client's `cache\configuration\configurations` (signed in once) |
| Battle.net | A built-in catalogue of 27, **ownership unknown** | `source_battlenet::catalogue`; the Agent refuses an unowned one at install (2221) |
| EA app | Nothing | No local file lists them |
| Xbox | Nothing | No local file lists them |

The issue's own line says Ubisoft lists installed games only: that is out of date. `source-ubisoft` has read the client's owned list since M4 ("Owned: the client's cache ... A game with no `Installs` key is owned and not installed"). And Epic lists owned games from the launcher's cache without legendary; legendary only adds its own list beside it.

**So the work is three stores: Battle.net, EA and Xbox**, plus GOG for a PC with gogdl and no Galaxy.

| Store | Recommended route | What the user does once | Tickets |
| --- | --- | --- | --- |
| Steam | Nothing new: `packageinfo.vdf`, as today | Nothing | 0 |
| Epic | Nothing new: the launcher's caches, legendary's list | Nothing (legendary's sign-in where they use it) | 0 |
| GOG | Galaxy's database as today; **with gogdl and no Galaxy, GOG's own library service with gogdl's token** | Nothing new: gogdl's sign-in, already there | 1 |
| Ubisoft | Nothing new: the client's cache | Nothing | 0 |
| Battle.net | First a read-only look at the client's own files on the TV PC; then **Blizzard's account site in a web view the Dashboard hosts** (`account.battle.net/api/games-and-subs`), as Playnite does | Sign in to Battle.net once in that page, pad and on-screen keyboard (a keyboard is quicker) | 2 (+1 shared) |
| EA | **EA's site in the same web view**, then EA's own GraphQL service for `ownedGameProducts`, as Lutris does; and an Install that asks the EA app | Sign in to EA once in that page | 2 (+1 shared) |
| Xbox | **No owned list** (no route inside this project's rules); optionally **the public Game Pass catalogue**, listed only when the user says they have Game Pass | Tick "I have PC Game Pass" in Settings | 1 |

**Can be done with no sign-in at all:** Steam, Epic, GOG with Galaxy, Ubisoft (all already), Xbox's Game Pass catalogue (public). Battle.net only if the TV PC's `CachedData.db` turns out to hold the account's products (unknown, B-O1 below). EA's purchased games and Xbox's purchased games: not without a sign-in.

**The one new thing to build is a store sign-in page in the Dashboard**: the web view host M9 is building for Discord ([#196](https://github.com/snrgarb/xbox-series-dashboard/issues/196), [#197](https://github.com/snrgarb/xbox-series-dashboard/issues/197); docs/research/discord-bridge.md) carries a store's page as well as Discord's, one profile folder per store. The session stays in that profile, as the browser's own cookies: the Dashboard's code never reads or stores the cookie or a long-lived token itself. That is still a saved sign-in at rest, the question account-switching.md section 7 put to the user (decision 3): here it is decision 1 below.

## 1. What "owned" means here, and where the list goes

- **Owned** is the account's licence: a game bought, claimed free, or granted. **Entitled** adds what a subscription gives while it runs: PC Game Pass (Xbox), EA Play, Ubisoft+, Battle.net's subscriptions (World of Warcraft). The two differ only for those; each section says.
- **A source reads files, never the network** (docs/sources.md: "Keep it to the client's local files: no network, no sign-in"; docs/spec/helpers.md 5: "a source reads files, never the network"). legendary already shows how an online list fits that rule: the helper is asked off the window's thread (`legendary status --json`, `Installer::refresh`), writes its own records, and `source-epic` reads them. Every online route below does the same: **a refresh** (at start, on Update now, after a sign-in) asks the store's service off the window's thread and writes `<data>\owned\<store>.json`, the store's own ids and names and when it was asked; the source's `read` takes it as one more file, with the store's local files winning for anything installed. No answer (offline, signed out, changed API) leaves the last file as it was and says why in the log: the library never loses games because a refresh failed.
- **Not owned any more** (a refund, a subscription ended): the next refresh leaves it out, and the library marks it gone as for any game a source no longer lists (sources.md, "How the library merges them"). An installed game stays listed whatever the list says: installed games are the machine's.
- **Profiles** (account-switching.md 5.5): owned-not-installed rows follow the bound account. A refresh writes the account's id into the file, so the M10 work can show each Profile its own.

## 2. Steam

| Route | Lists owned-not-installed | Fresh | Needs |
| --- | --- | --- | --- |
| **(a) `appcache/packageinfo.vdf`** (today) | Yes: the apps every cached package grants | As Steam last ran; Steam rewrites it as licences change [inferred] | Steam signed in once |
| (a) `userdata/<id>/config/localconfig.vdf` | Only apps played (today's fallback without `packageinfo.vdf`) | As Steam last ran | - |
| (b) Steam's `SteamClient` over the CEF debugger | Very likely (the client's own library page) [inferred]; no call read for it | Live | The debugger on (the user's), Steam running |
| (c) `IPlayerService/GetOwnedGames/v1` with the store page's `webapi_token` | Yes, and `IFamilyGroupsService/GetSharedLibraryApps/v1` the family's shared games | Live | A web view signed in to `store.steampowered.com` |

- **(c), how Playnite does it** [code: [`SteamStoreService.cs`][pe-steam-store], [`SteamServiceAggregator.cs`][pe-steam-agg], [`PlayerService.cs`][pe-steam-player]]: an offscreen web view opens `https://store.steampowered.com/explore/`, reads `data-store_user_config` on `#application_config` for `webapi_token` and `data-userinfo` for the SteamID, then calls `api.steampowered.com/IPlayerService/GetOwnedGames/v1/?access_token=...`; `IClientCommService/GetClientAppList/v1` and `/dynamicstore/userdata/` as fallbacks; `IParentalService` to drop what parental controls hide. The token lives as long as the web view's Steam cookies [inferred]. The old route, a personal Web API key, is still there for private profiles.
- The FriendsOfGalaxy Steam plugin signs in as a whole Steam network client (CM websocket, its own credential storage) [code: [galaxy-integration-steam `steam_network/`][gx-steam]]: a second Steam session, which installs.md section 2 (D, E) already ruled out.
- **Caveat on (a)** [inferred]: `packageinfo.vdf` is Steam's cache of every package it has looked up on this PC, so with two Steam accounts used here it may grant one account's games to the other, and it may hold packages of family-shared or free-weekend licences. M10 (account switching) is where that would matter; nothing reported says it does.

**Recommended:** nothing new. (a) as today. **Once:** nothing. **Kept:** nothing. **Install:** works (installs.md 2, `source_steam::client`). **Effort:** 0. **Tests:** as today (`source_steam::fixture`).

## 3. Epic Games

| Route | Lists owned-not-installed | Fresh | Needs |
| --- | --- | --- | --- |
| **(a) `%ProgramData%\Epic\EpicGamesLauncher\Data\Catalog\catcache.bin`** (today) | Yes, base64 JSON of the account's catalogue items | As the launcher last cached it | The launcher signed in once |
| **(a) `%LOCALAPPDATA%\com.epicgames.eglv2\cache\prod\account\<account>\library-cache.json`** (today) | Yes, with `owned` | As the eglv2 launcher last ran | That launcher |
| **(b) legendary's `metadata\<AppName>.json`**, refreshed by `legendary status --json` (today, #175) | Yes | At each Dashboard start and when added | legendary added and signed in |
| (c) `library-service.live.use1a.on.epicgames.com/library/api/public/items?includeMetadata=true&platform=Windows` | Yes | Live | An Epic OAuth token |

- **(c), how it is done** [code: Playnite [`EpicAccountClient.cs`][pe-epic-acct]; legendary [`api/egs.py`][lg-egs] lines 24-34, 64, 241]: a web view opens `https://www.epicgames.com/id/login?responseType=code` (Playnite sets the user agent `... EpicGamesLauncher`, "needed otherwise captcha won't pass"), catches `localhost/launcher/authorized?code=...` or the JSON of `https://www.epicgames.com/id/api/redirect?clientId=34a02cf8f4414e29b15921876da36f9a&responseType=code`, and trades the code at `account-public-service-prod03.ol.epicgames.com/account/api/oauth/token` with the launcher's own client id and secret (`grant_type=authorization_code`, `token_type=eg1`). Kept: the token answer, access and refresh token; Playnite AES-encrypts it with a key derived from the Windows user's SID [code: [`Encryption.cs`][pe-encryption]], which is obfuscation (the SID is public), not DPAPI; legendary keeps `user.json` in plain JSON with `expires_at` and refreshes it [code: [legendary `core.py`][lg-core] 219-247]. Lifetimes are Epic's and undocumented (installs.md, account-switching.md 7.3: 2 h access, 8 h refresh for another Epic client [reported]).
- **What breaks it**: Epic's captcha on sign-in (the user agent above), Epic stopping the launcher's client id for others (it broke legendary's sign-in once in 2022 [reported, installs.md 3]).

**Recommended:** nothing new. (a) and (b) as today; the Dashboard never needs Epic's token itself. **Once:** nothing (legendary's sign-in where they use legendary, already in Settings > Helpers). **Kept:** nothing new. **Install:** works (launcher link, or legendary: installs.md 3, helpers.md 5). **Effort:** 0. **Tests:** as today (`standin make-epic`, the stand-in legendary).

## 4. GOG

| Route | Lists owned-not-installed | Fresh | Needs |
| --- | --- | --- | --- |
| **(a) `%ProgramData%\GOG.com\Galaxy\storage\galaxy-2.0.db`** (today) | Yes: `ProductPurchaseDates` `gog_` release keys | As Galaxy last synced | Galaxy signed in once |
| (b) gogdl | No: gogdl lists no library (helpers.md 5) | - | - |
| **(c) `https://galaxy-library.gog.com/users/<user_id>/releases`** with gogdl's token | Yes: release entries, `platform_id` `gog` and `external_id` the product id, paged by `next_page_token` | Live | gogdl signed in (already, #176) |
| (c) `https://www.gog.com/account/getFilteredProducts?mediaType=1&page=<n>` | Yes | Live | A web view signed in to gog.com (cookies) |

- **(c) with gogdl's token, as Heroic does** [code: Heroic [`gog/library.ts`][heroic-gog-lib] `getGalaxyLibrary`, 328-371, and `refresh`, 459-560; [`gog/user.ts`][heroic-gog-user]]: `gogdl auth` (no `--code`) prints the current credentials, refreshing them first where expired [code: gogdl [`auth.py`][gogdl-auth] 42-122: `loginTime + expires_in`, refreshed at `auth.gog.com/token` with Galaxy's client id `46899977096215655` and its secret]. `GET galaxy-library.gog.com/users/<user_id>/releases` with `Authorization: Bearer <access_token>`, every page; keep `platform_id == "gog"`; names from `gamesdb.gog.com/platforms/gog/external_releases/<id>`, else `api.gog.com/products/<id>`. The Dashboard already has gogdl's `auth.json` in `<data>\gogdl` (helpers.md 5): it would hold the access token in memory for the call only, nothing new on disk.
- Playnite's way is the website's cookies in its own web view [code: [`GogAccountClient.cs`][pe-gog-acct]]: no better than gogdl's token for a Dashboard that already has one.
- **What breaks it**: GOG ending Galaxy's client id for others (gogdl, Heroic, Playnite's GOG OSS plugin all borrow it); a refresh token GOG has expired (signed out by a password change), which `gogdl auth` reports and Settings > Helpers then shows as "Not signed in".

**Recommended:** (a) as today; and where Galaxy's database is not there and gogdl is signed in, (c) with gogdl's token, written to `<data>\owned\gog.json`. **Once:** nothing new. **Kept:** nothing new (gogdl's own `auth.json`). **Install:** works (gogdl, or Galaxy's `/command=installGame`: installs.md 4). **Effort:** 1 ticket. **Tests:** the stand-in gogdl prints made-up credentials for `auth`; a stand-in library server (section 10) answers `/users/<id>/releases` in two pages, one entry another platform's; the Dashboard is pointed at it as in section 10.

## 5. Ubisoft Connect

| Route | Lists owned-not-installed | Fresh | Needs |
| --- | --- | --- | --- |
| **(a) `%LOCALAPPDATA%\Ubisoft Game Launcher\cache\configuration\configurations`** (today) | Yes: every product the account has had, protobuf around YAML | As the client last ran | The client signed in once |
| (c) `public-ubiservices.ubi.com/v3/profiles/sessions`, then the club and `uplay/graphql` services | Yes | Live | Ubisoft's site in a web view, a `ticket` and a `rememberMeTicket` |
| (c) `api-uplayplusvault.ubi.com/v1/games` | Ubisoft+ games | Live | As above |

- Playnite reads (a) only [code: [`Uplay.cs`][pe-uplay] 46-97, `UplayLibrary.cs` `GetLibraryGames`]; Lutris signs in on `connect.ubisoft.com` in a web view and reads (a) beside it [code: [Lutris `services/ubisoft.py`][lutris-ubi] 102-202]; the FriendsOfGalaxy plugin holds Ubisoft's `ticket` and `rememberMeTicket` (`Ubi-AppId` the club's) and refreshes them [code: [galaxy-integration-uplay `backend.py`][gx-uplay] 105-215, 276-312].
- **Ubisoft+**: whether its games are in the client's cache while subscribed is **[UNVERIFIED]**. Ubisoft's terms forbid "probing, scanning, or abusing APIs" (installs.md 6), which (c) is close to.

**Recommended:** nothing new. (a) as today. **Once:** nothing. **Kept:** nothing. **Install:** `uplay://install/<id>`, the client's dialog (installs.md 6). **Effort:** 0. **Tests:** as today (`standin make-ubisoft`). One look on a signed-in PC: does the cache list Ubisoft+ games (U-O1)?

## 6. Battle.net

| Route | Lists owned-not-installed | Fresh | Needs |
| --- | --- | --- | --- |
| (a) `%PROGRAMDATA%\Battle.net\Agent\product.db` | No: installed products only (today) | Live | - |
| (a) `%APPDATA%\Battle.net\Battle.net.config`, `Games` | Products the user has touched on this PC (the TV PC: `battle_net`, `lbra`) | As the client last ran | - |
| (a) `%LOCALAPPDATA%\Battle.net\CachedData.db` (SQLite) beside `Account\<id>\` | **[UNVERIFIED]**: contents not yet read | As the client last ran | The client signed in once |
| (b) The Agent | **No read-only call.** `POST /install` answers 2221 for a product the account does not own | Live | Battle.net closed (sources.md, "Which Agent is asked") |
| **(c) `https://account.battle.net/api/games-and-subs`** and **`/api/classic-games`** | Yes: `gameAccounts[]` with `titleId` and `localizedGameName`; subscriptions in the same answer | Live | Blizzard's account site signed in, in a web view |

- **The client's files.** `Battle.net.config`'s `Games` names what was installed or opened, not what is owned: a game bought and never opened is not there [inferred from the TV PC's two entries]. `CachedData.db` is the one local file left that might hold the account's products; nobody whose code was read reads it (Playnite, Lutris, the Galaxy plugin, DroidDeck, bnetswitch). One read-only look on the TV PC settles it (B-O1): its tables and columns, and whether a product the user owns and never installed appears (a row count and product codes, no values copied out).
- **The Agent** [reported: [wowdev.wiki "Agent"][wd-agent], oldid 36960, edited 2026-02-23]: `GET /game` is "a list of currently installed/installing products known to Agent"; `GET /game/<product>` a product's install path, version, locale and `account_country`; `GET /version/<product>` the patch server's versions. **Nothing lists entitlements or answers "owned?"** without a form: `POST /install` "registers a product without starting the install", and it is in that answer that 2221 comes (installs.md 7). The Dashboard can already fill a form and abandon it (`DASHBOARD_BATTLENET_NO_DOWNLOAD=1`, `DELETE /install/<uid>`), so **an ownership check per product without installing is possible**: `POST /install`, read `error`, `DELETE /install/<uid>`. Not read-only: it registers the product with the Agent for a moment, and whether `DELETE /install/<uid>` takes it back cleanly is a guess (sources.md). Before it is used at all, B-O2 on the TV PC: with Battle.net closed, the check for one owned and one unowned product, `product.db` compared before and after. Even then it is a check on a press (as Install already does), never a scan of the catalogue at start: 27 registrations every start is not a "read".
- **(c), how Playnite does it** [code: [`BattleNetAccountClient.cs`][pe-bn-acct]; [`BattleNetLibrary.cs`][pe-bn-lib] 200-203; [`GamesAndSubs.cs`][pe-bn-model]]: a web view opens `https://account.battle.net:443/oauth2/authorization/account-settings` (which refreshes the site's session cookie), then `https://account.battle.net/api/games-and-subs` and `/api/classic-games`, and reads the page's text as JSON. Each `titleId` is matched to a table of known products (`BattleNetGames.cs`, `ApiId` beside `ProductId` and `InternalId`); an unknown one is skipped with a warning. Sign-in: Blizzard's own login page in that web view, ended when the address reaches `account.battle.net/overview`. Kept: nothing of Playnite's; the cookies are the web view's profile's. The melcom Galaxy plugin (2026-11-16) does the same request with the site's cookies, and renews the site session from the stored login cookies when it answers 401 [code: [galaxy-integration-battlenet `backend.py`][gx-bnet-backend] 106-135, [`http_client.py`][gx-bnet-http] 151-243]; its own OAuth route needs every user to register a personal client at develop.battle.net, because "the original shared OAuth credentials were revoked by Blizzard" [docs: [its README][gx-bnet-readme]]. So **the account site's cookies are the route that still works for others**, and Blizzard has shown it will revoke a shared client.
- **For the Dashboard**: the store's page in the web view host (section 9), profile `<data>\webview2\battlenet`; the refresh navigates that hidden view to the two addresses and reads the page text, as Playnite does; the catalogue (`source_battlenet::catalogue`) gains each product's `titleId` (Playnite's table has them for the older games; the newer ones, Tony Hawk's Pro Skater 3 + 4 among them, **[UNVERIFIED]**: read from the user's own answer once signed in, by name). A product the account owns that the catalogue does not know is logged and not listed: without its uid it cannot be installed.
- **What it holds**: Blizzard's site session in the web view's cookie store (WebView2 keeps cookies encrypted with a key protected by DPAPI for the Windows user, as Edge does [inferred: Chromium's behaviour on Windows; not read in WebView2's documentation]). Lifetime: Blizzard's, undocumented **[UNVERIFIED]**; the Galaxy plugin's renewal says the login cookie outlives the session cookie. **What breaks it**: Blizzard's authenticator or an emailed code (typed once in the page, fine); a captcha (fine in a real browser engine); Blizzard changing `/api/games-and-subs`; Blizzard blocking embedded browsers **[UNVERIFIED]**.
- **Subscriptions**: `games-and-subs` carries the account's subscriptions (World of Warcraft's game time) beside its games [code: `GamesAndSubs.cs`]. A game whose licence is a subscription is listed while it runs.

**Recommended:** B-O1 first (no sign-in if `CachedData.db` has it). Else (c) in the web view host, opt-in in Settings > Accounts; the built-in catalogue's unowned games then leave the library (decision 4). The Agent's check stays what it is now: at Install. **Once:** sign in to Battle.net in the page, with the pad and the Dashboard's on-screen keyboard; a keyboard is quicker and fine; Blizzard's authenticator app on a phone answers the second step. **Kept:** the web view's own Battle.net profile; `<data>\owned\battlenet.json` (product codes, names, the account's public battletag). **Install:** works through the Agent where Battle.net is closed, else the client's game page (installs.md 7, #172). **Effort:** 2 tickets (B-O1 probe, then the route), after the shared web view ticket. **Tests:** the stand-in page server (section 10) serves a made-up Blizzard sign-in form and both addresses (a known `titleId`, an unknown one, a subscription); the stand-in Agent already refuses unowned products (2221).

## 7. EA app

| Route | Lists owned-not-installed | Fresh | Needs |
| --- | --- | --- | --- |
| (a) The Uninstall keys and `installerdata.xml` | No (today) | - | - |
| (a) `%ProgramData%\EA Desktop\<hash>\IS`, `%LOCALAPPDATA%\Electronic Arts\EA Desktop\` | **[UNVERIFIED]**: `IS` is encrypted (sources.md); what the app caches of the library was never looked at | - | - |
| (b) The EA app | No local service for a library found (installs.md 5, E) | - | - |
| **(c) `https://service-aggregation-layer.juno.ea.com/graphql`, `me { ownedGameProducts(...) }`** | Yes: `originOfferId`, and through `legacyOffers` the `contentId` | Live | An EA access token |

- **(c), how Lutris does it** [code: [Lutris `services/ea_app.py`][lutris-ea] 239-475]: a web view signs in at `https://www.ea.com/login`; its cookies then get a token with no prompt from `https://accounts.ea.com/connect/auth?client_id=ORIGIN_JS_SDK&response_type=token&redirect_uri=nucleus:rest&prompt=none` (the web store's own client, the one EA's site uses); the token goes as `Authorization: Bearer` to the GraphQL service: `me{player{pd psd displayName}}` for the account, `ownedGameProducts(locale: "DEFAULT", entitlementEnabled: true, storefronts: [EA], type: [DIGITAL_FULL_GAME, PACKAGED_FULL_GAME], platforms: [PC], paging: {limit, next})` for the games (`baseItem.gameType == "BASE_GAME"`), then `legacyOffers(offerIds){contentId}` and `gameProducts(offerIds){baseItem{title, keyArt, packArt}}` a hundred at a time. Lutris writes the token to a file; it is short-lived, and fetched again from the cookies when the service says `invalid_access_token`.
- The melcom Galaxy plugin (2026-10-13) instead signs in as the EA app itself: `client_id=JUNO_PC_CLIENT` with the app's secret and a `pc_sign` device signature, a refresh token kept, and reads `me{subscriptions{...}}` for EA Play [code: [galaxy-integration-ea `plugin.py`][gx-ea-plugin] 160-185, [`http_client.py`][gx-ea-http] 47-200, [`backend.py`][gx-ea-backend] 51-130, 338-371]. That is impersonating EA's client (installs.md 5, C): not for the Dashboard. Playnite's old Origin plugin used `api1.origin.com/ecommerce2/consolidatedentitlements/<user id>`, gone with Origin (stores.md).
- **Ids match**: `legacyOffers.contentId` is the content id `source-ea` already takes from `installerdata.xml` as `source_id`, so an owned game and its installed copy are one game [inferred: Lutris matches the two the same way, `get_installed_games_content_ids`].
- **EA Play**: games from the subscription are not in `ownedGameProducts` with `entitlementEnabled` [inferred]; the subscription's catalogue is another query, not read. Out of scope until asked.
- **For the Dashboard**: EA's page in the web view host, profile `<data>\webview2\ea`; the refresh runs Lutris's two steps **inside the page** (a `fetch` from the signed-in `accounts.ea.com` origin, `ExecuteScript`), so the token stays in the page for the call and never reaches the Dashboard's files [inferred: whether `connect/auth` answers a script in the page as it answers Lutris's cookie-carrying request, to be seen on the stand-in's real counterpart, E-O1].
- **What breaks it**: EA ending `ORIGIN_JS_SDK` with `prompt=none` (it is the web store's own, so less likely than a desktop client's); GraphQL changes (Lutris and the Galaxy plugin both follow them); EA's sign-in asking a code by email (typed once). EA's terms (installs.md 5): reading the user's own library from EA's own web service is what EA's site does; nothing is emulated [inferred].
- **Install of a game not installed**: today EA's `ask` gives nothing for Install (sources.md). It becomes `origin2://game/launch?offerIds=<offerId>&autoDownload=1`, the EA app's window (installs.md 5, A); the game reads installed once its key and `installerdata.xml` appear.

**Recommended:** (c) through the web view, opt-in. **Once:** sign in to EA in the page (pad and on-screen keyboard; keyboard fine). **Kept:** the web view's own EA profile; `<data>\owned\ea.json` (content ids, offer ids, names, art addresses). **Install:** the EA app's own window through `origin2://` (no Queue progress: installs.md 5). **Effort:** 2 tickets (the owned list; Install for not-installed games), after the shared web view ticket. **Tests:** the stand-in page server answers a made-up `connect/auth` (from its cookie) and the three GraphQL queries, paged; `standin make-ea` gets one owned game not installed; the stand-in `origin2://` handler records the link it was opened with.

## 8. Xbox (PC Game Pass, Microsoft Store)

| Route | Lists owned-not-installed | Fresh | Needs |
| --- | --- | --- | --- |
| (a) The packages registered for the user (today) | No: installed only | Live | - |
| (a) The Xbox app's and Store's own caches (`%LOCALAPPDATA%\Packages\Microsoft.GamingApp_*`, `Microsoft.WindowsStore_*`) | **[UNVERIFIED]**: never looked at; not readable as a documented format | - | - |
| (b) Windows APIs: `StoreContext`, `AppInstallManager` | No: `StoreContext` lists only the calling app's own add-ons [docs: installs.md 8, C]; no API lists the user's Store library | - | - |
| (c) `collections.mp.microsoft.com` (Collections v8 `b2bLicensePreview`, v9 `publisherQuery`) | The user's entitlements, Game Pass included | Live | **Partner services only**: "Client apps or games aren't able to call this service directly" [docs: [xstore v9 query][ms-v9], [detecting Game Pass][ms-gp]] |
| (c) `titlehub.xboxlive.com/users/xuid(<id>)/titles/titlehistory/decoration/...` | **Only games played** ("not started at least once ... won't appear") | Live | A Microsoft account OAuth token for Xbox Live (`Xboxlive.signin Xboxlive.offline_access`), traded for an XSTS token |
| **(c) `catalog.gamepass.com/sigls/v2?id=<list>&language=&market=`** | The **Game Pass catalogue**: what Game Pass has, not what this user may play | As Microsoft updates it | **Nothing**: public |

- **titlehub, how Playnite does it** [code: [`XboxAccountClient.cs`][pe-xbox-acct] 24-282; `XboxLibrary.cs` 239]: `login.live.com/oauth20_authorize.srf` with its own client id `38cd2fa8-...` and `https://login.live.com/oauth20_desktop.srf` as redirect, the code traded at `oauth20_token.srf`, then `user.auth.xboxlive.com/user/authenticate` and `xsts.auth.xboxlive.com/xsts/authorize`; both tokens kept, encrypted as Epic's. It lists **played** titles, so it never answers "what can I install that I have not played", the user's question. And it is a Microsoft sign-in driven by the Dashboard, which account-switching.md section 4 ruled out ("Never drive a Microsoft sign-in"). Not recommended.
- **The Game Pass catalogue** (stores.md, "Xbox ... Owned but not installed"; unifideck's `game_pass.py`, measured 2026-10-04 [reported]): public, product ids for a list ("all PC games" among five list ids), details and art from Microsoft's display catalogue by product id, as the art lookup already asks it (`library::art`, `Store::Xbox`). With no sign-in the Dashboard cannot know whether the user subscribes; **the user saying so in Settings** is the only no-sign-in answer. Each such game is then listed not installed, marked as Game Pass's, its Install the Store's page (`ms-windows-store://pdp/?ProductId=<id>`, as today's Install for an installed package's StoreId), and later `AppInstallManager` where X1 shows it takes a Game Pass product (installs.md 8). A game leaving Game Pass leaves the list at the next refresh; one the user bought stays only if installed.
- **Purchased Store games not installed: no route** inside this project's rules. Xodus (account-switching.md 2.3) lists them by impersonating Windows' own sign-in to the Store's private services: out of bounds.

**Recommended:** no owned list. Optionally the public Game Pass catalogue, only when the user ticks "I have PC Game Pass" (decision 3). **Once:** that tick, with the pad. **Kept:** `<data>\owned\xbox-gamepass.json` (product ids, names, the day it was fetched). **Install:** the Store's page as today (a window); `AppInstallManager` later (installs.md 8). **Effort:** 1 ticket. **Tests:** the stand-in page server answers a made-up `sigls/v2` list and the catalogue's product details; `standin make-xbox`'s installed game is also in the list, so the test sees one game, not two.

## 9. The sign-in page: the Discord web view, carrying a store

M9 builds a WebView2 host for Discord: a hidden window on its own thread, Discord's page shown full screen and usable with the pad, the session kept in the data folder, signed-in / signed-out / no-runtime known, a sign-out that leaves nothing ([#197](https://github.com/snrgarb/xbox-series-dashboard/issues/197); the spike [#196](https://github.com/snrgarb/xbox-series-dashboard/issues/196) is still open, so none of it is proven on the TV PC yet). A store's sign-in page is an ordinary web page: **the same host can carry it**, with these differences [inferred unless marked]:

- **One profile folder per store** (`<data>\webview2\<store>`, WebView2's `CoreWebView2Environment` user data folder or a named `Profile`), so signing out of one is deleting its folder, and Discord's session never sees Blizzard's cookies.
- **Shown only to sign in**, hidden for a refresh. The refresh is a navigation and a read of the page's text (Battle.net), or a script in the page (EA); its answer goes to the source's file, never the cookies.
- **Driving it with the pad**, as #197 plans for Discord: the pad's D-pad as Tab and Shift+Tab between the page's fields, A as Enter, the Dashboard's own Console keyboard (docs/spec/search.md) for text, put into the field by the DevTools protocol's `Input.insertText` (discord-bridge.md 6). A keyboard plugged in works as in any browser and is quicker for a password.
- **What a pad cannot do** in such a page: a captcha that wants a pointer (hCaptcha's or Arkose's puzzles; Epic sets its launcher's user agent so its captcha passes [code: `EpicAccountClient.cs`]; Blizzard's and EA's are **[UNVERIFIED]**), which the pad as a mouse pointer may answer; a passkey or Windows Hello prompt, which Windows puts on its own secure desktop, where no program's input reaches (the same limit as UAC, helpers.md 5); "Sign in with Google", which Google refuses in embedded browsers **[reported; not tried in WebView2]**. A plain password and an emailed or authenticator code are fine.
- **What it is, in the terms of account-switching.md**: a store's sign-in kept at rest on the PC, in the web view's cookie store under DPAPI [inferred], used by anything running as this Windows user that can open the folder. That is section 7's "saved sign-in" for one more launcher each, so it is the user's decision (decision 1), opt-in per store, listed and forgettable in Settings, deleted on sign-out. The principle that **the Dashboard never holds a password** holds: the password is typed into the store's own page and goes to the store; the Dashboard's code reads no cookie and stores no token (EA's short token lives inside the page for the call).
- **The alternative with no web view**: none fits Battle.net or EA. Their routes are cookie sessions of a website, not codes that a phone could fetch and the user type back (Epic's and GOG's codes could, installs.md 10.4; neither is needed here).

## 10. Testing against the stand-ins

Nothing here is ever tested against a store's real service: the tests use made-up accounts on loopback.

- **A stand-in store web server**, `standin store-web <dir>` (a sibling of `standin steam-wizard` and the stand-in Agent): on a free port written in `<dir>`, it serves a made-up sign-in page per store (a form whose Submit sets a cookie and redirects where the real one ends), and the addresses above with made-up answers read from `<dir>`: Blizzard's `games-and-subs` and `classic-games` (a 401 without the cookie), EA's `connect/auth` (a token only with the cookie) and GraphQL (the three queries, two pages), GOG's `galaxy-library` releases (a bearer token), the Game Pass `sigls/v2` list. Every request a line in its log, cookies and tokens masked.
- **The Dashboard is pointed at it** by one setting a test gives (`DASHBOARD_STORE_WEB=http://127.0.0.1:<port>`), taken only for a loopback address so it can never send a real session elsewhere [inferred: the only safe shape for such an override].
- **Parsers** (`games-and-subs` to product codes, GraphQL pages to content ids, releases to product ids, the `sigls` list) are tested on made-up JSON in each source crate, as every parser here is.
- **The sign-in page with the pad**: a VM payload signs in to the stand-in's page with pad presses, restarts, is still signed in, refreshes and finds the made-up owned game not installed, signs out and leaves no profile folder (as #197's done-when for Discord). The lead runs it; this ticket only writes it.

## 11. Proposed tickets, in dependency order

1. **Owned lists as files** (shared). `<data>\owned\<store>.json`, its shape (store ids, names, art addresses, the account's public id and name, when fetched), a refresh hook beside `Installer::refresh` run at start, on Update now and after a sign-in, off the window's thread, its failure leaving the last file and a log line; the source crates read it. `standin store-web` with its first answers. Nothing online yet. *Needs nothing.*
2. **GOG's library from gogdl's token** where Galaxy's database is not there. *Needs 1.*
3. **Xbox: the Game Pass catalogue**, behind "I have PC Game Pass" in Settings > Launchers' Xbox row, listed as Game Pass's, Install the Store's page. *Needs 1; decision 3.*
4. **Battle.net probe (B-O1, B-O2)**: a read-only look at `CachedData.db`'s tables and `Battle.net.config`'s `Games`, and the Agent's ownership check for one owned and one unowned product with `product.db` compared, as a probe script under `e2e/htpc/probes/` for the lead to run. *Needs nothing.*
5. **Store web sessions**: the M9 web view host given one profile per store, its page full screen with the pad, Settings > Accounts' per-store Sign in, Sign out, and the saved-sign-in line of account-switching.md 7.4. *Needs #197 (and its spike #196); decision 1.*
6. **Battle.net's owned games** from `account.battle.net/api/games-and-subs` and `classic-games` (or from `CachedData.db` if ticket 4 finds them there, with no web view), the catalogue given `titleId`s, unowned catalogue games dropped. *Needs 1, 4, and 5 unless 4 finds a local list; decision 4.*
7. **EA's owned games** from EA's GraphQL service through the web view. *Needs 1, 5.*
8. **EA: Install for a game not installed**, `origin2://game/launch?offerIds=<id>&autoDownload=1`, the game followed until its key appears. *Needs 7.*

Eight tickets: four with no sign-in (1 to 4), four on the web view (5 to 8).

## 12. Questions for the user

1. **May the Dashboard keep a store's sign-in in its own web page, for Battle.net and EA?** It is the store's own sign-in page; your password goes to the store, not to the Dashboard. What is kept on the PC is the store's "remember me", as a browser keeps it, and anything running on the PC as you could use it, as with Discord's. *Recommended: yes, one store at a time, only when you press Sign in, with Sign out and "Forget" in Settings.*
2. **Signing in with the pad, or a keyboard?** The page works with the pad and the on-screen keyboard; a password is slow that way, and a store asking for a picture puzzle may need the pad as a mouse. *Recommended: pad first, with a keyboard allowed; it is once per store.*
3. **Xbox: show the Game Pass catalogue as games you can install?** Nothing on the PC says which Store games you bought, and Microsoft gives other programs no way to ask without impersonating Windows. The Game Pass list is public. *Recommended: yes, only after you tick "I have PC Game Pass", marked as Game Pass's.*
4. **Battle.net: once your owned games are known, hide the catalogue games you don't own?** Today all 27 show, and an unowned one opens Battle.net when you press Install. *Recommended: yes, hide them once the list is read; keep showing the catalogue only while Battle.net is not signed in here.*
5. **Battle.net without signing in: check a game you don't have installed by asking Battle.net's downloader, before you press Install?** It would start an install form and withdraw it, which leaves Battle.net's records alone only if the TV PC test says so. *Recommended: no, keep the check at Install as now, unless ticket 4 shows it leaves nothing behind.*
6. **GOG with gogdl and no Galaxy: list your GOG games from GOG's service with gogdl's existing sign-in?** No new sign-in. *Recommended: yes.*

## 13. What only a signed-in PC can answer

- **B-O1** `CachedData.db`: its tables and columns; does a product the account owns and never installed appear? **Answered** (2026-11-18, from a copy): its tables are `schema_info`, `login_cache`, `remote_objects`, `key_value_store`, `catalog_cache`, `browser_stats`, `personal_avatar`; the licences are in `key_value_store`, and with the catalogue `catalog_cache` names they give the owned products, installed or not (`source_battlenet::cache`).
- **B-O2** The Agent's check (`POST /install`, read `error`, `DELETE /install/<uid>`): does `product.db` come back unchanged, for an owned and an unowned product?
- **B-O3** Does `account.battle.net` sign in and answer `/api/games-and-subs` in WebView2? How long does its session last unused?
- **E-O1** Does `accounts.ea.com/connect/auth?client_id=ORIGIN_JS_SDK&prompt=none` answer a script in the signed-in page? How long does EA's site session last?
- **U-O1** Does the Ubisoft client's cache list Ubisoft+ games?
- **X-O1** Does the Game Pass list's "all PC games" id still answer, and do its product ids open in the Store?
- **W-O1** On the TV PC, do Blizzard's and EA's sign-in pages show a captcha or a passkey prompt in WebView2?

## Sources

Each at the commit or date read, 18 November 2026.

[pe-steam-store]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/SteamLibrary/Services/SteamStoreService.cs
[pe-steam-agg]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/SteamLibrary/Services/SteamServiceAggregator.cs
[pe-steam-player]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/SteamLibrary/Services/PlayerService.cs
[pe-epic-acct]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/Services/EpicAccountClient.cs
[pe-encryption]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Generic/PlayniteExtensions.Common/Encryption.cs
[pe-gog-acct]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/Services/GogAccountClient.cs
[pe-uplay]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/Uplay.cs
[pe-bn-acct]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/Services/BattleNetAccountClient.cs
[pe-bn-lib]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/BattleNetLibrary.cs
[pe-bn-model]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/Models/GamesAndSubs.cs
[pe-xbox-acct]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/XboxLibrary/Services/XboxAccountClient.cs
[lg-egs]: https://github.com/derrod/legendary/blob/170964e75affc0c04be467ec2b12879376738ef5/legendary/api/egs.py
[lg-core]: https://github.com/derrod/legendary/blob/170964e75affc0c04be467ec2b12879376738ef5/legendary/core.py
[gogdl-auth]: https://github.com/Heroic-Games-Launcher/heroic-gogdl/blob/6c6ef53fd4965d8d9b943c73d9400fe55eadc6ba/gogdl/auth.py
[heroic-gog-lib]: https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/2dbb80272a6463e73e9bac0f4ab20f039490bd90/src/backend/storeManagers/gog/library.ts
[heroic-gog-user]: https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/2dbb80272a6463e73e9bac0f4ab20f039490bd90/src/backend/storeManagers/gog/user.ts
[lutris-ea]: https://github.com/lutris/lutris/blob/eb1cedf4f48434fa469321d23546a706409f4498/lutris/services/ea_app.py
[lutris-ubi]: https://github.com/lutris/lutris/blob/eb1cedf4f48434fa469321d23546a706409f4498/lutris/services/ubisoft.py
[gx-steam]: https://github.com/FriendsOfGalaxy/galaxy-integration-steam/tree/1ebe11ef4bb9f1fd28d96714eb204bc4accc756e/src/steam_network
[gx-uplay]: https://github.com/FriendsOfGalaxy/galaxy-integration-uplay/blob/61d24e4ed0f1b2378252142e5ee2c92cd309612b/src/backend.py
[gx-bnet-readme]: https://github.com/melcom-creations/galaxy-integration-battlenet/blob/425aee1e8be496102a35305c65e2148435b7215a/README.md
[gx-bnet-backend]: https://github.com/melcom-creations/galaxy-integration-battlenet/blob/425aee1e8be496102a35305c65e2148435b7215a/src/backend.py
[gx-bnet-http]: https://github.com/melcom-creations/galaxy-integration-battlenet/blob/425aee1e8be496102a35305c65e2148435b7215a/src/http_client.py
[gx-ea-plugin]: https://github.com/melcom-creations/galaxy-integration-ea/blob/dccceaaa6d6c52a9728758148b2f1c2292c2140f/src/plugin.py
[gx-ea-http]: https://github.com/melcom-creations/galaxy-integration-ea/blob/dccceaaa6d6c52a9728758148b2f1c2292c2140f/src/http_client.py
[gx-ea-backend]: https://github.com/melcom-creations/galaxy-integration-ea/blob/dccceaaa6d6c52a9728758148b2f1c2292c2140f/src/backend.py
[wd-agent]: https://wowdev.wiki/index.php?title=Agent&oldid=36960
[ms-v9]: https://devdocs.xbox.com/reference/microsoft-store-apis/xstore-v9-query-for-products
[ms-gp]: https://learn.microsoft.com/en-us/gaming/gdk/docs/store/commerce/service-to-service/xstore-detecting-game-pass
