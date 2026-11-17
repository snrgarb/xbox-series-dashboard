# How the Discord Bridge extension gives Playnite "full Discord", and what the Dashboard would take from it

Research for [#192](https://github.com/snrgarb/xbox-series-dashboard/issues/192), milestone M9 Full Discord. The
user's words (2026-11-17), handing over the extension's package: "read how this works because this has FULL DISCORD
support instead our current solution. This can be it's own milestone after m8."

Read on 17 November 2026, on Linux. **Nothing in the package was run or loaded**: no .NET runtime, no script from it,
no VM, no TV PC, no Discord account. The DLL was read as data: its .NET metadata tables (types, methods, fields,
type and member references, P/Invoke imports) and its user-string heap (every string literal the code holds,
including about 250 KB of JavaScript), with [`dnfile`](https://github.com/malwarefrank/dnfile) from a script kept
outside the package. No decompiler was used, so method bodies (the IL) were not read: where a step's order or a
number lives only in code, it is marked as inference. Nothing of the package is copied here beyond short
identifiers. The package itself stays local and gitignored.

Tags:

- **[package]**: read in Discord Bridge 0.9.7 (`extension.yaml`: Id `DiscordBridge`, author Justin Delano, a Playnite
  `GenericPlugin`; `DiscordBridge.dll` built from commit `67e75a2`), in its metadata, its strings, `selectors.json`,
  `lang/labels.*.json`, `Localization/en_US.xaml` or `ThemeSample.xaml`.
- **[docs]**: a vendor's own page, linked.
- **[here]**: this repository at `0ffb4ae`.
- **[Bundle]**: `~/projects/xbox-series-theme` (`CONTEXT.md`, `docs/research/discord-rpc-dotnet.md`).
- **[inference]**: my reasoning from the above. Nobody has tested it.

## Summary

**Discord Bridge does not talk to Discord's servers or its API at all. It drives Discord's own client, the way a
person or a screen reader would, and reads what that client draws.** It has two ways in, and an Auto mode that uses
both:

1. **Desktop mode** attaches to the **Discord app already on the PC**. It finds Discord's Chromium window, reads the
   page through Windows' accessibility interfaces (UI Automation through FlaUI, and Chromium's IAccessible2 tree,
   which carries the page's element classes), and acts by posting mouse and key messages straight to Discord's
   render window, so Discord never has to come to the front. To keep Discord drawing while it is hidden it parks the
   window off-screen as an invisible sliver, and adds Chromium's `disable-backgrounding-occluded-windows` to
   Discord's `settings.json`, restarting Discord with `--start-inactive` to make it take. No sign-in is needed:
   whoever Discord is signed in as. Video tiles are captured from Discord's window on the GPU (Windows.Graphics.Capture
   into Direct3D 11, then Direct3D 9 for WPF).
2. **Web mode** opens **discord.com in a hidden WebView2** that the user signs in to once, in Discord's own login
   page. It reads the page's DOM with injected JavaScript and a file of CSS selectors, and acts with the Chrome
   DevTools Protocol's trusted input (`Input.dispatchMouseEvent`, `Input.insertText`, `Input.dispatchKeyEvent`,
   `DOM.setFileInputFiles`). Voice can run in that web view too, as a second Discord client of the same account.
3. Both modes also read files Discord itself wrote: Chromium's HTTP disk cache (the API answers and pictures
   Discord already fetched) and Local Storage (voice devices and volumes, status, favourites).

Everything it reads is drawn by the extension in its own WPF controls, which a Playnite theme lays out through named
slots and bindings. Message history it has seen is kept in a local SQLite database with full-text search.

**What it gives** is close to the whole Discord client: servers, channels, DMs, friends, messages with replies,
reactions, edits, pins, threads, forums, polls, emoji, stickers, GIFs, uploads and voice messages; voice with mute,
deafen, per-person volume, devices, soundboard, camera, screen share and watching streams; notifications and
incoming calls. **What it costs** is fragility (every read depends on Discord's class names and button labels in 31
locales), a hidden Discord window being moved and re-launched, and Discord's terms, which forbid automating a user
account; the extension's own settings page says so.

**For the Dashboard**, the recommendation (section 7) is: keep the RPC connection for the Call; add, as an opt-in
the user turns on, **our own signed-in WebView2 session of discord.com** read through the DevTools protocol, for
text (DMs, notifications, messages, sending with the pad); draw it all in our own renderer in the console's look;
leave Desktop-mode attaching and video capture out unless a later need forces them.

## 1. What it gives the user

From `Localization/en_US.xaml` (666 strings), the theme-facing descriptions in the DLL, and the backend interface
`IDiscordBackend` (about 90 operations, every one implemented by both `DesktopBackend` and `WebBackend`) [package].

| Area | What the user can do |
| --- | --- |
| **Servers** | All servers in Discord's order, with folders (open, close), unread pills and mention badges; mute a server for 15 min to forever; notification level and Discord's notification switches; mark a server read |
| **Channels** | The open server's channels in Discord's order with categories (expand, collapse), text, voice (with who is in it), announcement, stage, forum (posts as cards, a post opened beside its forum), media, followed threads; mute, notification settings, copy link or ID |
| **DMs and friends** | The DM list with status, unread and mentions; start a DM or a group DM from friends, add or remove people, leave a group, close a DM; the Friends page's Online, All, Pending, Blocked; send a friend request (a captcha sends the user to Discord itself) |
| **Messages** | The open conversation's messages as Discord draws them: day and "new" dividers, replies with the quoted message, author in the role colour, role icon, APP and OP tags, markdown (bold, code, spoilers, quotes, mentions as chips), attachments, embeds, stickers, polls, system rows, reactions; older pages; jump to present; typing indicator |
| **Acting on messages** | Send (with a formatting bar), reply with or without a ping, edit, edit last, delete, pin, react, quick react, forward, mark read or unread, vote or unvote in a poll and see voters, see who reacted, reveal spoilers, remove an attachment, upload files, record and send a voice message |
| **Pickers** | Emoji (Discord's categories, server emoji, favourites, frequently used; Nitro-only ones dimmed), stickers, GIFs (searched on Tenor or Klipy in a separate web view, plus Discord favourites), the soundboard |
| **History** | Every message the extension has shown kept locally and searchable, readable offline; clear history |
| **Voice** | Who is in the call, who is speaking, muted, deafened, streaming; join a voice channel, leave, call or answer a DM, decline; mute, deafen, camera; per-person mute and volume; input, output and camera device; mic and output volume; soundboard and its volume; "join here" to move a call from another device |
| **Video** | Screen share (Discord's picker driven hidden: windows and screens, with or without sound), stop sharing; the call's videos (streams and cameras) live at 30 fps, one focused with a strip of the others, watch a stream, the call's chat |
| **Status** | Online, idle, do not disturb, invisible; custom status with emoji |
| **Notifications** | New DMs, mentions, unread, incoming calls and video calls, into a list and Playnite's notifications; a toast-ready "newest" for themes |
| **Upkeep** | A health check that notifies "when Discord changed and something stopped working"; a diagnostics zip with names, text and links left out |

The extension draws none of this in a fixed layout of its own beyond defaults: a theme puts `DiscordBridge_<Element>`
slots (the server rail, channel column, chat, composer, call view and so on) where it wants them, binds the lists
through `{PluginSettings Plugin=DiscordBridge, Path=…}`, and restyles with `DiscordBridge_…` resources
(`ThemeSample.xaml`) [package]. It is built for a mouse and a controller alike: every list item has its "Actions"
strip because "for controllers … there is no right-click" [package].

**The Bundle did not use it.** Neither the Bundle's code nor its research mentions Discord Bridge; its Discord tab is
its own RPC client ([Bundle] `CONTEXT.md` "Discord tab", `docs/research/discord-rpc-dotnet.md`), which this project
ported (section 5). So no Bundle value is bound to it and nothing in the port depends on it.

## 2. How it works, layer by layer

### 2.1 The parts

| Layer | Types (from the metadata) | What it does |
| --- | --- | --- |
| Plugin and view model | `DiscordBridgePlugin`, `BridgeViewModel`, `DiscordSidebarView`, `BridgeElements`, `ElementCatalog` | Playnite plugin, the WPF view model and the slots a theme fills |
| Backend choice | `IDiscordBackend`, `DesktopBackend`, `WebBackend`, `BackendMode`, `Caps` | One interface; Auto, Web or Desktop. In Auto, voice through the app and text through the web session once signed in; two different accounts in the two make Auto use the app only |
| Desktop: finding and keeping Discord | `DesktopDiscord` (the largest type, several hundred methods), `DiscordWindow` | Finds `Discord.exe`'s windows, attaches, parks, launches, restarts |
| Desktop: reading | `Ia2`, `IaNode`, FlaUI (`UIA3Automation`, conditions, tree walker, Invoke, Toggle, RangeValue, Text, Scroll, Selection patterns), `oleacc.dll` `AccessibleObjectFromWindow`/`AccessibleChildren` | The page as Chromium's accessibility tree |
| Desktop: acting | P/Invoke `PostMessage`, `SendMessageTimeout`, `SetLayeredWindowAttributes`, `SetWindowPos`, `LockSetForegroundWindow`, `GetAsyncKeyState`; methods `GhostClick`, `PostClick`, `PostKey`, `PostChars`, `Hover`, `ClipboardLoan` | Clicks and keys sent to `Chrome_RenderWidgetHostHWND` without focus; paste through the clipboard |
| Web: hosting | `DiscordHost` (WebView2 `CoreWebView2Environment`, `Profile`, `NotificationReceived`, `PermissionRequested`, `ScreenCaptureStarting`, `ProcessFailed`), "DiscordBridge (offscreen host)" | discord.com in a hidden window, recreated if it crashes |
| Web: reading | `DiscordReader`, `Scripts` (about 250 JavaScript snippets), `Selectors` + `selectors.json`, `SelectorProbe`, `ChannelKindLearn` | DOM queries by CSS selector; paging through Discord's virtualised lists |
| Web: acting | `DiscordActions`, `DiscordHost.CdpAsync` | Element centres found by script, then DevTools `Input.*` events at those points |
| Files Discord wrote | `ChromiumCache`, `LocalStorage` (a LevelDB and Snappy reader), `PresenceStore`, `VoiceStore` | Cached API answers and pictures; settings stores |
| Video and audio | `LiveCapture`, `WgcOptions`, `VideoFeed`, `VideoGrid`, `SpeakingRings`, `AppVolume` (`IAudioSessionManager2`, `ISimpleAudioVolume`) | Window capture, tiles, speaking rings, Discord's per-app volume |
| Store | `CacheStore` over SqlNado (SQLite), FTS5 | `discord_cache.db`: messages, conversations, users, servers, channels, emoji, stickers, pins |
| Media | `MediaPlayer`, `GifSite`, `Lottie`, `Apng`, `Waveforms` | A second web view for video and YouTube; a third for GIF search; animated stickers |

The DLL holds 2,092 types and 12,653 methods [package]: it is a large program, not a thin wrapper.

### 2.2 Desktop mode: attaching to the Discord app

- **Finding it.** `EnumWindows`, `GetClassName` (`Chrome_WidgetWin_1`), `GetWindowThreadProcessId` and the process
  image name pick Discord's main window; `EnumChildWindows` finds the render widget `Chrome_RenderWidgetHostHWND`
  [package: P/Invoke table, strings]. Log lines: "attached: pid", "no Discord window answers UIA yet", "attached to
  Discord's HIDDEN (tray) window — no focus change" [package].
- **Starting it.** If Discord is not running, any control starts it: `Update.exe --processStart Discord.exe
  --process-start-args --start-inactive` (Squirrel's launcher, so Discord starts in the tray) [package: the string
  verbatim]. "Discord not installed (Update.exe missing)" otherwise. A launch record `discord_launch.json` and a
  cooldown stop it from starting Discord again and again [package].
- **Keeping it drawing.** Chromium stops painting a window that is covered or minimised, and a page that is not
  painted has a stale accessibility tree. The extension:
  - adds `disable-backgrounding-occluded-windows` to the `chromiumSwitches` of Discord's `settings.json` ("added
    Chromium's disable-backgrounding-occluded-windows to Discord's settings.json") and restarts Discord for it to
    take, but never while Discord is in front or in a call ("Discord needs a restart … but is in a call: not now")
    [package];
  - **parks** Discord's window: off-screen, or "as an invisible sliver" one pixel column at a monitor's edge that it
    moves if covered, made transparent through a layered window ("ghost"), and puts it back where Discord saved it
    (`WINDOW_BOUNDS`, `IS_MAXIMIZED` in `settings.json`) when the user opens Discord [package: strings "parked
    Discord", "its pixel column was covered, moved to", "unpark", "ghost show", "ghost restore"];
  - watches foreground changes (`SetWinEventHook`) so Discord coming to the front unparks it [package].
- **Reading.** Chromium exposes the page through UI Automation and through IAccessible2, whose object attributes
  include each element's HTML tag and class. The extension walks the IA2 tree (`Ia2.Walk`, `Attributes`,
  `HasClass`) and matches Discord's hashed class prefixes (`name_`, `usernameSpeaking_`, `buttonActive_`, …) and
  its `data-list-item-id` values, as the web mode's selectors do [package; that Chromium's IA2 attributes carry the
  class is inference from `Ia2.HasClass` and the class prefixes it looks for]. Buttons are found by their
  accessible names in the user's Discord language: `lang/labels.<locale>.json` maps actions to Discord's labels in
  31 locales ("Mute", "Deafen", "Start Voice Call", …), the locale read from Discord's `settings.json`; a developer
  tool (`LanguagePackGenerator`) switches Discord's language to harvest them and restores it [package].
- **Acting.** A click is a `PostMessage` of mouse messages to the render widget at the element's point
  (`PostClick`, `GhostClick`), keys and text by posted key and char messages (`PostKey`, `PostChars`), or the UIA
  Invoke, Toggle and RangeValue patterns where Discord exposes them (sliders for volumes). It refuses to act while
  Shift, Ctrl, Alt or Windows is held ("stopped: Shift, Ctrl, Alt or Windows was held down") because those would
  change what the posted click does [package]. Files go through a "clipboard loan" with
  `ExcludeClipboardContentFromMonitorProcessing`, so Windows' clipboard history does not keep them [package].
- **Lists.** Discord's lists are virtualised (only what is on screen exists), so long lists are read page by page by
  scrolling Discord's own list ("DmsAbove", "ChannelsAbove", "MembersAbove", "Older") [package]. The DM list is read
  live only while Discord shows Home, and Friends only while it shows the Friends page: "showing the list never
  moves the app" [package].

### 2.3 Web mode: its own signed-in discord.com

- **Hosting.** WebView2 (the Edge runtime) in a hidden window titled "DiscordBridge (offscreen host)", its user
  data folder `webview2` in the plugin's data folder, navigated to `https://discord.com/channels/@me` [package]. The
  user signs in through Discord's own page in a visible window ("the window closes by itself once you are in");
  "Log out" clears that profile and does not touch the Discord app [package]. The extension's local pages are served
  on virtual host names (`fixtures.discordbridge.local`, `player.discordbridge.local`) mapped to folders, not on a
  port [package: strings and `CoreWebView2HostResourceAccessKind`].
- **Reading.** `ExecuteScript`/`Runtime.evaluate` runs snippets that query the DOM with the selectors in
  `selectors.json` (59 entries: `GuildItems` `[data-list-item-id^="guildsnav___"]`, `ChannelRows`,
  `MessageItems` `li[id^="chat-messages-"]`, `UnreadBadge`, `VoicePanel`, `CallParticipants`, …, with a
  `SignatureVersion`) [package]. The selectors are pushed into the page (`PushSelectorsAsync`) so one file serves
  every script; channel kinds and icons that the selectors cannot tell apart are learned from the accessible labels
  ("(voice channel)", "(forum channel)") and saved (`ChannelKindLearn`, `Selectors.AddChannelIcon`) [package]. No
  address for fetching new selectors from the internet is in the DLL: updates come with new versions of the
  extension, or by editing the local file [package: the full list of URLs in the strings].
- **Acting.** A script finds an element's centre and checks nothing covers it (`document.elementFromPoint`); the C#
  side then sends DevTools `Input.dispatchMouseEvent` (move, press, release) and `Input.insertText` or
  `Input.dispatchKeyEvent` at that point, events the page sees as trusted, not synthetic [package]. Uploads use
  `DOM.setFileInputFiles` on Discord's hidden file input [package]. Every action waits for its effect ("pressed, no
  change seen within {1} ms") rather than assuming it [package].
- **Notifications.** WebView2's `NotificationReceived` takes Discord's own web notifications (a new DM, a mention, a
  call) instead of showing them [package: the typeref; that this is what feeds "Notify_Message" is inference].
- **Voice in the web view.** The web session can be the voice client: the extension grants the microphone and
  camera (`PermissionRequested`), and screen share goes through Chromium's picker (`ScreenCaptureStarting`) that it
  drives hidden. The "page muted / permission mic / devices in, out / rtc / mute / deafen" diagnostics line is the
  web session's [package]. In Auto mode the app does voice and the web session does text [package: settings text].

### 2.4 Files Discord wrote

- **The HTTP disk cache.** `ChromiumCache` scans `Cache/Cache_Data` of both Discord's own folder and the web view's
  `EBWebView/Default`, parses the cached JSON answers of Discord's API (`/api/v*/channels/…/messages`, users,
  stickers, soundboard) and the cached pictures, and fills messages, authors, avatars, embeds and attachments from
  them [package: regexes on API paths, "cache: {0} entries, {1} images, … {5} messages"]. This is how Desktop mode
  gets full message data and pictures without asking Discord for anything: it reads what Discord already downloaded.
- **Local Storage.** A LevelDB reader of its own (`.ldb`, `.log`, Snappy) reads `MediaEngineStore` (input, output and
  camera device, volumes, `localMutes`, `localVolumes`) and `UserSettingsProtoStore` (status, custom status,
  favourite and frequent emoji, stickers, GIFs) and follows them as they change [package]. No string in the DLL
  names Discord's token, and no snippet calls Discord's API with `fetch` [package: searched the whole string heap;
  the one `fetch` is for an audio waveform].
- **Its own store.** `discord_cache.db` in the plugin's data folder: SQLite with `messages`, `conversations`, users,
  servers, channels, emoji, stickers, pins, and an FTS5 index for search ("Everything the bridge shows is cached
  locally … Nothing is fetched to fill it") [package]. Also `discordbridge.log`, `selectors.json`,
  `discord_launch.json`. The plugin's data folder is Playnite's per-extension folder [inference].

### 2.5 Voice, speaking, video

- **Join, leave, mute, deafen, camera, answer, decline**: pressing Discord's own buttons (the user panel's
  switches, the call bar, a voice channel's row) and waiting for the "Voice Connected" panel or the switch's new
  state [package: `JoinVoice`, `JoinEvidence`, `SwitchesIn`, `CallBar`, `WaitConnectedAsync`].
- **Who is speaking**: in the web page, Discord's `usernameSpeaking_` class; in the app, the call view's green ring,
  looked for in the captured picture of Discord's window (`SpeakingRings`: `Lit`, `LitRadii`, `Green`; log
  "[speaking] ring … : no capture") [package; that the app's ring is sampled from captured pixels is inference from
  those names].
- **Per-person mute and volume**: Discord's user context menu, its slider set through UIA RangeValue or by script
  [package: `OpenUserVoice`, `SetSlider`].
- **Devices and volumes**: read from `MediaEngineStore`; set through Discord's audio menu or its Voice & Video
  settings; Discord's whole output through Windows' per-app audio session (`AppVolume`) [package].
- **Video**: Windows.Graphics.Capture of Discord's window (`IGraphicsCaptureItemInterop`,
  `CreateDirect3D11DeviceFromDXGIDevice`, no cursor, no border, a minimum interval), each video tile's rectangle
  (read from the tree) cut out and drawn with SharpDX Direct3D 11 shaders, then handed to WPF through a Direct3D 9Ex
  surface (`D3DImage`) or a bitmap if there is none; HDR monitors handled by SDR white level ("captured from
  Discord's window on the GPU at the size it is drawn, 30 fps") [package]. So video is never decoded by the
  extension: it is Discord's own drawing, copied.
- **Screen share**: Discord's share dialog (app) or Chromium's picker (web), driven while hidden; Windows' own list of
  windows and screens is shown first with live thumbnails [package].

## 3. What it needs, and what breaks it

| Need or risk | What happens, and how it copes |
| --- | --- |
| **Discord changes its page** (class names, `data-list-item-id`s, layout) | The main risk, and both modes share it: reads are CSS selectors and class prefixes. It copes by matching prefixes, not full hashed classes (`[class*="name_"]`); several alternatives per selector; learning icons and channel kinds; a `SignatureVersion`; a health check that probes every selector and notifies the user "When Discord changed and something stopped working"; diagnostics with the page's structure for the author. A fix still needs a new `selectors.json` or a new version [package] |
| **Discord changes its button labels** or the user's language | Desktop mode finds buttons by accessible name: 31 language packs of labels, English as the fallback [package] |
| **The app covered, minimised or in the tray** | Chromium stops painting, reads freeze: the occlusion switch in `settings.json`, the parked sliver, waking a page that "sleeps (closed to the tray)" by a second start [package] |
| **Discord ignores the switch** | Logged ("settings.json could not take it"); reads may freeze while covered [package] |
| **Modifier keys held** | Posted clicks are refused until released [package] |
| **Sign-in** | Desktop: none, it is whoever Discord is. Web: Discord's own login page in the web view, so two-factor, QR login and captchas are Discord's own screens the user completes; a logged-out session notifies "logged out — open Discord to log in again" [package] |
| **Captchas mid-use** | Friend requests can raise one: "send this request from Discord itself" [package] |
| **Rate limits and detection** | It acts "only when you click something, at most once every 0.7 seconds, and loads lists page by page as you scroll. It never collects servers, members or messages in bulk and never sends anything on its own" [package: settings text]. Discord sees an ordinary client making the requests its UI makes [inference] |
| **WebView2 runtime missing** | Web mode, GIFs and videos are off; Desktop mode works [package] |
| **The web view crashes** | Recreated; after repeated crashes Desktop mode continues [package] |
| **Two accounts** | Web signed in as one, the app as another: Auto uses the app only and says so [package] |
| **Discord not installed** | Desktop mode is off ("Update.exe missing") [package] |
| **Discord's own updates** | Restarting Discord for the switch, and Squirrel updates, are timed around calls and the foreground [package] |
| **Nitro and permissions** | Shown dimmed and refused, as Discord would ("works only on {1} (or with Nitro)") [package] |

The author's own assessment, in the settings page: "Desktop mode may occasionally have issues with data refreshing
or certain features not working as expected. Web mode is recommended" [package].

## 4. Discord's terms, against both routes

Discord's [Terms of Service](https://discord.com/terms) (effective 29 September 2025, read 2026-11-17) [docs], under
"Don't use the services to do harm to Discord", forbid among other things:

- "scraping our services without our written consent, including by using any robot, spider, crawler, scraper, or
  other automatic device, process, or software";
- "using any unauthorized software designed to modify the services";
- "copying, dismantling, or reverse engineering any of our services".

and under "Software in Discord's services": "You may not copy, modify, create derivative works based upon, distribute,
sell, lease, or sublicense any of our software", and "You also may not reverse engineer or decompile our software or
services, attempt to do so, or assist anyone in doing so."

Discord's support article [Automated user accounts (self-bots)](https://support.discord.com/hc/articles/115002192352)
states that automating a normal user account outside the OAuth2/bot API is forbidden and can lead to the account
being terminated [docs; the page refused an automated fetch (403) on 2026-11-17, so this is its indexed summary and
its localised versions' wording, not a fresh quote].

How each route sits against these (facts; the user decides):

| | The RPC route (today) | The client-driving route (Discord Bridge) |
| --- | --- | --- |
| How Discord is reached | Discord's documented local RPC, signed in with OAuth2 through an application the user owns and is a tester of ([RPC docs](https://github.com/discord/discord-api-docs/blob/main/developers/topics/rpc.mdx)) | Discord's client, read and pressed by another program; no API, no token, no application |
| What Discord's documents say about it | The documented, permitted way for a local program to read and set voice; RPC itself is a private beta limited to an app's testers [docs, as quoted in the Bundle's research] | No document allows it. It is a program acting on a user account, which the self-bot article and the "automatic device, process, or software" clause speak to [docs]. The author: "Discord's terms forbid automating user accounts, so a small risk remains" [package] |
| Modifying the client | No | Desktop mode writes a Chromium switch into Discord's `settings.json` and restarts Discord; it does not change Discord's code ("it adds no code to Discord") [package] |
| Reading Discord's files | No | Reads Discord's HTTP cache and Local Storage [package] |
| Volume of activity | Events Discord pushes | Paced to user gestures, one action per 0.7 s at most, pages loaded as scrolled [package] |
| Enforcement seen | None needed | The author cites Ferdium, Rambox and screen readers as tools working the same way "without reported bans" [package]. This research found no report either way and did not search Discord's enforcement history |

## 5. What the Dashboard has today, side by side

The Dashboard ports the Bundle's RPC client ([here] `docs/discord.md`, `docs/spec/discord-tab.md`, `crates/discord`,
4,339 lines; `crates/screens/src/guide/discord.rs`, `servers.rs`, `settings/discord.rs`): the user makes their own
Discord application, the Dashboard signs in over `\\.\pipe\discord-ipc-N` with scopes `rpc identify rpc.voice.read
rpc.voice.write rpc.notifications.read`, keeps tokens with DPAPI, and follows voice events.

| | Dashboard today (RPC) | Discord Bridge |
| --- | --- | --- |
| Set-up | A developer-portal application per user: Client ID, Secret, redirect, App Tester; approve once in Discord | Desktop: none. Web: sign in once in a web view |
| The Call: who, speaking, muted, deafened | Yes, pushed by Discord as events | Yes, read from the page |
| Mute, deafen, leave | Yes, confirmed by Discord's events | Yes, by pressing Discord's buttons |
| Join a voice channel or DM call | Yes (`SELECT_VOICE_CHANNEL`), Shortcuts, Servers Flyout | Yes |
| Servers and voice channels | Yes, voice and stage only | Yes, every kind |
| Text channels, DMs, messages | **No** | Yes, read and send |
| Notifications, incoming calls | The scope is asked for; nothing follows them yet, and the Bundle's ring is not built (spec 3) | Yes |
| Friends, status, custom status | No | Yes |
| Per-person volume, devices | No (RPC has commands for some; not built) | Yes |
| Camera, screen share, watching streams, video | No; RPC offers none of these | Yes (video only as a capture of the app's window) |
| Soundboard, emoji, stickers, GIFs | No | Yes |
| Needs Discord running | Yes, the app | Desktop: yes, and it moves and restarts it. Web: no |
| Breaks when Discord changes its UI | No: a documented protocol | Yes: selectors and labels |
| Terms | Permitted route | Forbidden in letter (section 4) |
| Testable off Windows | Yes: `FakeDiscord` over a pipe, e2e payloads | Only against recordings of Discord's page |

## 6. How the same route would be built here, in Rust

### 6.1 Which Discord to drive

| | A. Our own WebView2 signed in to discord.com | B. Attach to the Discord app (Desktop mode) | C. The app's DevTools port |
| --- | --- | --- | --- |
| How | `webview2-com` (MIT; the COM bindings Tauri's `wry` uses) in a hidden window; read with `ExecuteScript`, act with `CallDevToolsProtocolMethod` (`Input.*`, `DOM.setFileInputFiles`); `add_WebMessageReceived` for a `MutationObserver` in the page to push changes instead of polling | UI Automation from the `windows` crate (`Win32_UI_Accessibility`); IAccessible2 is not in `windows` and would need its interface declared by hand; posted mouse and key messages; parking and the `settings.json` switch | Add `remote-debugging-port` to Discord's `chromiumSwitches` and speak CDP to it, as `source_steam::cef` does to Steam |
| Sign-in | The user, once, in Discord's page (2FA and QR are Discord's) | None | None |
| Reading | The DOM, as Web mode | An accessibility tree: slower, coarser, harder | The DOM, as A |
| Effect on the user's Discord | None: a separate session, a second "device" | Moves, hides and restarts their app | Opens their signed-in client to any local program on that port; whether Discord honours that switch is not known [inference; not tested] |
| Voice | Leave it to the app over RPC; a web-view voice client is possible but would be a second device | The app | The app |
| Video | Not without voice in the web view | Window capture (Windows.Graphics.Capture, then a texture into wgpu) | Same as B |
| Size | Smallest | Largest: the bridge's `DesktopDiscord` alone is several hundred methods | Small, but see the risk |

Recommendation: **A**, with voice kept on RPC (section 7, D2). B is where most of the bridge's complexity lives and
is what its author calls less reliable; C hands the user's whole Discord session to any program on the PC.

### 6.2 Drawing

Our renderer (`ui` over tiny-skia and wgpu, [here]) would draw the lists and messages from data the page scripts
return, in the console's look, moved through with the pad; the web view is shown only for sign-in and Discord's own
settings. Showing the web view itself in a panel would be quicker to get but would look like Discord's site,
need a mouse, and need WebView2's composition hosting inside the Overlay's window [inference].

### 6.3 Stages

Rough sizes are for this codebase's style (pure core, thin Windows shell, a fake for tests) [inference].

| Stage | What | Rough size | Tested against a stand-in | Only against the real thing |
| --- | --- | --- | --- | --- |
| **S0. Spike** | A hidden WebView2 from Rust, discord.com signed in by hand on the TV PC, one script returning the DM list, one `Input.insertText` send to a DM with the user's agreement | a few days, thrown away | none | All of it: that WebView2 hosts from our window, that Discord's login and 2FA work in it, how Discord treats a second session |
| **S1. The session** | `crates/discord_web`: the web view host on its own thread, sign-in page shown full screen with the pad (as installs.md plans for stores), session kept in the data folder, signed-out detection; Settings > Discord gets "Messages" on/off | 1,000-1,500 lines Rust | The host's logic against a fake page server; the page shown | Discord's real login |
| **S2. Reading** | Page scripts (our own, written from Discord's DOM) for DMs, unread, the open conversation's messages, servers, text channels; a selectors file of our own with a health probe; `MutationObserver` pushes; pictures from Discord's CDN as avatars are now | 1,500-2,500 lines Rust and JS | The scripts in headless Chromium on Linux against hand-made fixture pages (made-up names, never a saved real page) | That the selectors match Discord today, and after each Discord update |
| **S3. Screens** | The Guide's Discord tab grows chats (the console's Parties & chats): a conversation list, a conversation view, notifications into Toasts; spec in `docs/spec/` | 1,500-2,500 lines Rust | Screenshot tests with fake data | Look against the console |
| **S4. Sending** | Send with the Console keyboard, reply, react (a small emoji set), mark read; one action at a time, each confirmed by what the page then shows | 800-1,200 lines | Fixture pages that change as a real one would | Discord accepting it, captchas |
| **S5. Notifications and rings** | `NotificationReceived` into Toasts; incoming calls answered through RPC where it can, else the page | 500-800 lines | Fake notifications | Real ones |
| Later, if wanted | Per-person volume and devices (RPC can do some of this without the page), soundboard, stickers, GIFs, uploads, voice messages, screen share, video by window capture | each its own ticket | partly | mostly |

The bridge's scope (section 1) is many months of one person's work; S0-S5 is a usable "text chat from the couch"
alongside the voice the Dashboard already has [inference].

## 7. Decisions for the user

**D1. Take the client-driving route at all, given Discord's terms?**
Options: (a) no, stay on RPC and accept no text; (b) yes, as an opt-in the user switches on, RPC kept for voice;
(c) yes, replacing RPC. **Recommend (b)**: text is the gap, RPC is the permitted and sturdier route for what it
covers, and an opt-in keeps the terms risk with the person choosing it.

**D2. Which Discord to drive for text: our own web session, the Discord app, or both?**
Options: (a) our own WebView2 signed in to discord.com; (b) attach to the Discord app as Desktop mode does;
(c) both with an Auto, as the bridge. **Recommend (a)**: no moving or restarting the user's Discord, a DOM instead of
an accessibility tree, testable against fixtures, and the bridge's own author recommends it. Cost: a second sign-in
and a second session Discord sees as another device.

**D3. Draw Discord in our own renderer, or show Discord's page?**
Options: (a) our renderer, the console's look, the pad; (b) the web view shown in a panel. **Recommend (a)**: the
Dashboard is a console port and (b) needs a mouse. The page is shown only for sign-in and Discord's settings.

**D4. How far does M9 go?**
Options: (a) text chat: DMs, notifications, read and send, reply, react, mark read (S0-S5); (b) (a) plus servers'
text channels, threads and forums; (c) the bridge's whole list including soundboard, stickers, GIFs, uploads,
screen share and video. **Recommend (a)** for M9, (b) as a follow-up once (a) has lived through a few Discord
updates.

**D5. Video, screen share and watching streams?**
Options: (a) not in M9; (b) through the app with window capture (needs D2 (b) or (c)); (c) through a voice client in
our web view. **Recommend (a)**: each needs either the Desktop route (moving and restarting the user's Discord) or
moving voice off the app into a second client, the two heaviest and most fragile parts of the bridge.

**D6. Keep a local store of messages?**
Options: (a) none: lists and the open conversation in memory, as Discord serves them; (b) a cache of what was shown,
encrypted as `discord-secrets.dat` is, for instant reopen; (c) the bridge's searchable history. **Recommend (a)**,
then (b) only if reopening is slow: the TV PC is a shared living-room screen and a message store is the most private
thing the Dashboard would hold.

**D7. Ask the author?**
Options: (a) build our own from the mechanism read here and ask nothing; (b) also ask Justin Delano for a licence to
reuse or for collaboration (he also maintains PlayniteAchievements). **Recommend (a), with (b) left to the user**:
the package has no licence (section 9), so nothing of it can be reused without his permission, and the mechanism is
general enough to write ourselves.

## 8. The tickets that would follow

In order. Each waits on the decisions named.

1. **Spike: Discord web session from Rust** (D1, D2). Done when a throwaway branch's report says whether a hidden
   WebView2 hosted from the Dashboard signs in to discord.com on the TV PC with the pad and keyboard, returns the DM
   list by script and sends one message by `Input.insertText`, with what Discord showed (new-device mail, captcha).
2. **The Discord web session** (D1, D2). Done when Settings > Discord can sign in and out of a session kept in the
   data folder, the Dashboard knows signed in, signed out and no runtime, and nothing starts unless the user turned
   it on.
3. **Reading Discord's page** (D2). Done when DMs with unread counts, the open conversation's messages and the
   servers' text channels come to the Dashboard as model types, tested in headless Chromium against hand-made
   fixture pages, with a health probe that names any selector that stopped matching.
4. **Spec: chats in the Discord tab** (D3, D4). Done when `docs/spec/` says how the console's Parties & chats shows
   conversations and messages, measured from the console's pictures, and the user has approved it.
5. **The chats screens** (D3, D4, D6). Done when the Guide's Discord tab lists conversations and shows one with
   screenshot tests, from fake data.
6. **Sending** (D4). Done when send, reply, a small reaction set and mark read work with the Console keyboard, each
   shown only once the page shows it.
7. **Discord notifications as Toasts** (D4). Done when a DM, a mention and an incoming call raise the Dashboard's
   Toast and A opens them.
8. **Servers' threads and forums** (D4 (b)). Done when they open from the channel list.
9. **Video and screen share** (D5 (b) or (c)). Only if chosen. Done when a call's streams show in the tab.

## 9. The package's licence and where it lives

- The package has no licence file, no README, an empty `LegalCopyright`, and the DLL's strings point to
  `https://github.com/justin-delano/DiscordBridge` (and its wiki), a Ko-fi page and a Playnite Discord thread
  [package]. On 2026-11-17 that repository answers 404 (private or not yet published), the author's 16 public
  repositories do not include it, and Playnite's add-on database
  ([JosefNemec/PlayniteAddonDatabase](https://github.com/JosefNemec/PlayniteAddonDatabase)) has no entry for it.
  The author's other Playnite repositories are MIT.
- **With no licence, the default is all rights reserved**: the package may be used as given, but none of its code,
  scripts, selectors, labels or themes may be copied, adapted or redistributed without the author's permission
  [inference; general copyright default]. Reading it to learn how it works and writing our own is what this document
  does, and what section 6 proposes.
- It bundles FlaUI, SharpDX (both MIT), the WebView2 SDK (Microsoft's licence) and SqlNado (compiled in), none of
  which we would need.

## What could not be established

- The method bodies were not decompiled, so exact timings, retry counts and the order of steps inside a method are
  not known beyond what the strings and names show. Where it matters it is marked inference.
- Whether Chromium's IAccessible2 attributes carry Discord's class names, and whether the app's speaking ring is
  sampled from captured pixels, are inferred from method names, not seen.
- The extension's public repository, release history and licence: not public on 2026-11-17.
- Discord's self-bot article could not be fetched (403); its wording is taken from its indexed and localised copies.
- How Discord treats a second web session, or the occlusion switch, or any of this in enforcement: no source found.
