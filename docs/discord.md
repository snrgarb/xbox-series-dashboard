# Discord

How the Dashboard connects to the Discord running on the same PC (#143): the user's own Discord application, the
local RPC client, its secrets, and Settings > Theme settings > Discord. The Guide's Discord tab, the Call, Shortcuts
and Servers (#144, #145) are built on what is here. The behaviour is the Bundle's, ported from its C#
(`XboxSeries.Core/ExtensionCore.Discord*.cs`, `extension/XboxSeries.Extension/DiscordPipe.cs` and `DiscordOAuth.cs`;
the research is the Bundle's `docs/research/discord-rpc-dotnet.md`); the page is
[spec/settings.md](spec/settings.md), 6.9.

## Why the user's own application

Discord's local RPC answers only an application whose owner, or one of its App Testers, is the signed-in user. Nobody
can ship one application for everyone to use, so each user makes their own at the developer portal, adds the redirect
`http://127.0.0.1` under OAuth2 and themselves under App Testers, and gives the Dashboard its Client ID and Client
Secret. The page's four steps say so.

## The pieces

| Where | What |
|---|---|
| `crates/model/src/discord.rs` | `Discord`, what the screens see of the connection (no secret in it); `Ask`, what Settings asks of it; the page's words. |
| `crates/discord/src/client.rs` | `Client`, the connection's logic: the handshake, AUTHORIZE, the code exchange, AUTHENTICATE, the voice subscriptions, refreshing the tokens, trying again. Pure: no I/O and no clock. It talks through three traits (`Transport`, `Tokens`, `Secrets`) and is told the time (`advance`). |
| `crates/discord/src/frame.rs` | The pipe's framing: an opcode (i32 LE), a length (i32 LE), then that much JSON. |
| `crates/discord/src/ipc.rs` | The real `Transport`: `\\.\pipe\discord-ipc-0` to `-9` on Windows, a Unix socket of the same name in `$XDG_RUNTIME_DIR` elsewhere, or one pipe a test names. A thread reads each connection and hands the frames to the loop. |
| `crates/discord/src/win.rs` | The Windows named pipe (overlapped, so a read waiting on its thread never holds up a write) and DPAPI. |
| `crates/discord/src/tokens.rs` | The real `Tokens`: Discord's OAuth2 token endpoint, asked on a thread of its own. |
| `crates/discord/src/secrets.rs` | The real `Secrets`: `discord-secrets.dat` in the data folder, the whole file encrypted with DPAPI for the Windows user (a stand-in that keeps it as it is elsewhere). |
| `crates/discord/src/lib.rs` | `Connection`: the client over the real pieces, as the Dashboard's loop drives it. |
| `crates/discord/src/voice.rs`, `shortcuts.rs`, `servers.rs`, `following.rs` | The Call, the Known channels, the Servers Flyouts, and the three followed together from the client's events. Pure, as the client is. |
| `crates/discord/src/avatars.rs` | The avatars and server icons, fetched off the frame into the data folder. |
| `crates/discord/src/fake.rs` | `FakeDiscord`: a fake Discord on a pipe of the test's naming, with its own token endpoint (`standin discord`). |
| `crates/screens/src/settings/discord.rs` | The Settings page. |
| `crates/app/src/window.rs` | Makes the `Connection`, starts it, pumps it every turn, carries out `Effect::Discord`, hands `Discord` to the screens. |

## The connection, step by step

1. **Connect** (both the Client ID and the Secret saved): the pipe is opened and the handshake sent,
   `{"v":1,"client_id":...}`. Status `Connecting...`. Discord not running: `Discord isn't running`.
2. Discord's READY: AUTHORIZE with the client ID and the scopes `rpc identify rpc.voice.read rpc.voice.write
   rpc.notifications.read` (no `redirect_uri`: Discord refuses one over RPC and takes the application's own). Status
   `Approve the request in Discord`: Discord shows the user a prompt.
3. Approved: Discord answers with a code, which is exchanged at `https://discord.com/api/oauth2/token` (HTTP Basic
   `client_id:secret`; `grant_type=authorization_code`, the code, `redirect_uri=http://127.0.0.1`). The tokens and
   when they expire are saved.
4. AUTHENTICATE with the access token. Discord answers with the user: status `Connected as <display name, else
   username>`. Then SUBSCRIBE to VOICE_CHANNEL_SELECT, VOICE_CONNECTION_STATUS and VOICE_SETTINGS_UPDATE.
5. **Start-up**, with tokens saved: steps 1 and 4 alone, nothing asked of the user. The tokens a day or less from
   expiring are refreshed first.
6. **Disconnect**: the connection closed, the tokens forgotten; the Client ID and Secret stay.

### Refreshing the tokens

- A day before they expire (`REFRESH_MARGIN`), over the open connection, and never sooner than 15 minutes after the
  last refresh (`MIN_REFRESH_INTERVAL`): tokens granted with no expiry or less than a day to run are refreshed again
  after 15 minutes.
- An unanswered refresh (no answer, 408, 429, 5xx) is tried again after 60 s (`REFRESH_RETRY`), whatever the state.
- A refused one: AUTHORIZE again (the prompt in Discord). If that fails too, the user has to reconnect: status and
  the Discord tab's line `Reconnect Discord in settings`, no more tries until Connect.
- A token Discord refuses at start-up is refreshed once; a refreshed one it refuses again goes to AUTHORIZE.
- An answer that comes after Discord closed the pipe is kept if it was a refresh (the old refresh token is dead once
  a new one is granted), dropped if it was a code exchange.

### Trying again

With tokens saved, a pipe that can't be opened, or that closes, is tried again after 2, 4, 8, 16, 30 and 60 s, then
every 60 s (`PROBE_DELAYS`), and signed in with the saved tokens once it opens. A pipe that opens and gives no READY
within 10 s (`READY_TIMEOUT`) is closed and counts as a try; Connect waits for READY however long it takes. Signing
in starts the back-off afresh. Without tokens nothing is tried by itself. Disconnect, and needing to reconnect, stop
the tries.

## The secrets

The Client ID, the Client Secret, the access and refresh tokens and when they expire are kept under the Bundle's
names (`Discord.ClientId`, `Discord.ClientSecret`, `Discord.AccessToken`, `Discord.RefreshToken`,
`Discord.ExpiresAt`) in `discord-secrets.dat` in the data folder: a JSON map, the whole file encrypted with DPAPI
(`CryptProtectData`, the current Windows user, no prompt) and written whole to `.dat.new` and renamed. The expiry is
kept in Unix milliseconds. A file that can't be read is no secrets: the user enters the application again. Off
Windows the file is kept as it is (a stand-in behind the `Protect` trait; nothing ships there).

Nothing secret is ever in a log line, a `Debug`, the screens or the focus log: the page shows the Client ID (it is
public) and only `Saved` or `Not set` for the Secret; `Ask`'s `Debug` hides what was typed; the token endpoint's
answer bodies are never logged. The library database is not used: no migration.

## The Call (#144)

The Discord tab itself is docs/spec/discord-tab.md. What it shows comes from `voice::Follower`
(`crates/discord/src/voice.rs`, ported from the Bundle's `ExtensionCore.DiscordCall.cs` and `DiscordCall.cs`),
which the `Connection` hands every event the client takes in:

- Signed in, it asks GET_SELECTED_VOICE_CHANNEL and GET_VOICE_SETTINGS. VOICE_CHANNEL_SELECT names the channel the
  user is in (null: none); a new one is fetched with GET_CHANNEL, and the Call begins once it arrives, if it is still
  the one selected. A GET_SELECTED_VOICE_CHANNEL answer after a VOICE_CHANNEL_SELECT is ignored.
- Each Call's channel is followed with SUBSCRIBE (VOICE_STATE_CREATE, _UPDATE, _DELETE, SPEAKING_START, _STOP) and
  given up with UNSUBSCRIBE as the Call ends or moves. The same channel fetched afresh keeps who was speaking; a
  refused GET_CHANNEL for another channel drops the old Call rather than show it as the current one.
- A person is named by nickname, else display name, else username; is muted or deafened whether they or the server
  did it; comes in Discord's order with the user first. A DM or group with no name is named after the other people
  in it, else the last seen, else "Call".
- Mute and Deafen send SET_VOICE_SETTINGS `{mute|deaf: !now}`, Leave SELECT_VOICE_CHANNEL `{channel_id: null}`. What
  shows is only ever what Discord said: VOICE_SETTINGS_UPDATE for the switches, VOICE_CHANNEL_SELECT for Leave. A
  refused command changes nothing and leaves the connection up.
- Discord closing ends the Call and forgets the voice settings; signed in again, it is all asked afresh.

`Discord.voice` (`Voice { call, mute, deaf }`, `Call { id, title, people }`, `Person { id, name, avatar, speaking,
muted, deafened, me }`) is the screens' copy. `Ask::Call(CallAsk::Mute|Deafen|Leave)` is what the tab asks.

### Avatars

`https://cdn.discordapp.com/avatars/<id>/<hash>.png?size=64`, else Discord's default
`embed/avatars/<(id >> 22) % 6>.png`, fetched off the frame by `avatars::Avatars` (a thread of its own, lazily
started) into `<data>/art/discord/avatars/<id>/<hash>.png` and `.../embed/<n>.png`, and kept: a kept file is used
at once ever after, as art is. Until one is there the tab draws a grey disc with a figure. At most 1 MB, written
beside its place then moved there. With a test's pipe there is no CDN at all unless `--discord-cdn-url` names one.

## Following it all (#145)

The client's events belong to the `Connection` (`Connection::follow`), which hands each to `following::Following`:
the Call (`voice::Follower`), the Known channels (`shortcuts::Known`) and the Servers Flyouts (`servers::Browser`),
told apart by command and nonce. A GET_CHANNEL sent for who is in a server's voice channel is known by its nonce and
never becomes the Call; every channel Discord sends (the Call's too) fills in who is in it in a server's Flyout.
GET_GUILD answers go to the Known channels, GET_GUILDS and GET_CHANNELS (and their refusals) to the browser.
`Following` is pure, as its parts are; the `Connection` gives it the time, sends what it asks through the client,
saves what it hands back in the secret store, and wakes the loop for the Flyout's spinner (`next_timer`).

`Discord::has_tab()`: the Guide has a Discord tab (signed in, or tokens saved). `Discord::tab_line()`: what the tab
says in Discord's place while it can't show it, None while signed in. `Dashboard::set_discord` sets
`status.discord` from `has_tab()`: Home's Discord tile opens the tab then, Settings otherwise.

### Known channels

Ported from the Bundle's `ExtensionCore.DiscordShortcuts.cs` and its tests, in `crates/discord/src/shortcuts.rs`:

- Each Call begun is learned, at the front: its channel's id, server, name (the Call's heading: the channel's name,
  else the people in it) and picture (a DM's first other person's avatar). A voice channel's server is asked for its
  name and icon (GET_GUILD) and becomes the most recent server; the answer lands on every Known channel of that server.
  The Call's own Known channel takes the name and picture it lacked as people join, without a save for an unchanged
  one; joined again, it is named afresh.
- **DMs are kept until removed; a server's voice channel only while among the six most recent.** The first six are
  the Shortcuts. Remove shortcut forgets one for good, and a removed one is not brought back by its Call going on;
  removing one leaves the servers' order alone.
- Kept in the secret store (`discord-secrets.dat`, encrypted) under the Bundle's names and in its JSON:
  `Discord.Shortcuts`, an array of `{Id, GuildId, Name, Image, ServerName}` (`Image` the picture's address on
  Discord's CDN), and `Discord.RecentServers`, the servers' ids, most recent first. Unreadable data reads as none.
  They are read as the Dashboard starts, so the Shortcuts show as soon as Discord signs in, and kept across Disconnect.
- A Shortcut's picture is fetched as an avatar is, its address mapped onto the CDN the Dashboard was given (so a test
  never reaches Discord's); a server's icon is kept in `<data>/art/discord/icons/<guild>/<hash>.png`.
- A on one sends SELECT_VOICE_CHANNEL `{channel_id, force: true}`, unless it is the Call's already.

### Servers

Ported from `ExtensionCore.DiscordServers.cs` and its tests, in `crates/discord/src/servers.rs`. What the Flyout is
for, and its looks, are docs/spec/discord-tab.md, 5.

- Servers sends GET_GUILDS; a server, GET_CHANNELS `{guild_id}`, keeping its voice (2) and stage (13) channels in
  Discord's order, then GET_CHANNEL for each to learn who is in it. Only the answer to the request still awaited is
  shown (by nonce): one for a Flyout backed out of, or a server left for another, is dropped.
- The servers come in the Bundle's order: the Known servers most recent first, then the rest by name ignoring case.
- The spinner shows a second at least: an answer that comes sooner is held until then, and dropped if the Flyout has
  closed meanwhile.
- What was answered is kept while signed in: Servers or a server opened again shows it at once and asks afresh; a
  refusal then keeps it. B back to the servers asks nothing. Discord going forgets it all.
- A on a channel sends SELECT_VOICE_CHANNEL as a Shortcut does, and closes the Flyout.

`Discord.shortcuts` (`Shortcut { id, name, server, picture, dm }`) and `Discord.browse` (`Browse::Closed | Servers {
loading, servers } | Server { id, name, loading, channels }`) are the screens' copies. `Ask::Join(id)`,
`Ask::RemoveShortcut(id)` and `Ask::Browse(Servers | Server { id, name } | Back | Close)` are what the tab asks; the
screens hold the Flyout itself (open, focus, B), and close it as `browse` closes or Discord goes.

## Testing

- `crates/discord/tests/client.rs`: the Bundle's DiscordTests, DiscordTokenTests and DiscordReconnectTests against a
  fake transport, token endpoint and secret store, on a clock the test moves.
- `crates/discord/tests/fake.rs`: the real `Connection` (pipe, threads, HTTP) against `FakeDiscord`: connects, is
  lost as the fake goes away, tries again and signs in by itself as it comes back; starting again signs in with the
  saved tokens. A Call arrives with everyone's avatar from the fake's stand-in CDN; people join, speak and leave;
  Mute and Leave, held by the fake, show only once it confirms; a refused one changes nothing. The servers listed
  with their icons, a server's channels with who is in them, one joined becoming a Shortcut, a DM's Shortcut, both
  there as the Dashboard starts again, one rejoined in one ask and removed.
- `crates/discord/tests/voice.rs`: the Bundle's DiscordCallTests, the follower against a recording of what it sends.
- `crates/discord/tests/shortcuts.rs` and `servers.rs`: the Bundle's DiscordShortcutsTests and DiscordServersTests,
  `Following` against a recording of what it sends, a secret store and a clock the test moves.
- `standin discord --pipe <name> [--port <n>]` runs the fake; `standin discord-say <name> go-away | come-back | close
  <message> | call <channel> <person>... | in <channel> [person]... | select <channel> | dm <person> | group
  <person>... | join <person> | leave <person> | speak|hush <who> | mute|unmute|deafen|undeafen <who> | hold | confirm
  | refuse [message] | hang-up` steers it (`<who>` a name, or `me`; `<channel>` General, Late_night, Lobby or
  Town_hall). `in` puts people in a voice channel without the user; `select` joins one as another device would; `dm`
  and `group` put the user in a DM or group DM with those people. `hold` keeps SET_VOICE_SETTINGS and
  SELECT_VOICE_CHANNEL waiting until `confirm` carries them out or `refuse` errors them. It has three servers
  (`fake::SERVERS`): Stand-in Server (with an icon: a text channel, General, Late night), Another Server (no icon:
  Lobby, a text channel, the stage Town hall) and art club (an icon, text only). Its HTTP port serves every avatar and
  icon asked as a plain picture of one colour: nobody's real one.
- `dashboard --discord-pipe <name> [--discord-token-url <url>] [--discord-cdn-url <url>]` points the Dashboard at
  it: only that pipe is tried, and with no token URL or CDN URL there is none at all, so a test never reaches
  discord.com. The e2e helpers add
  `--discord-pipe e2e-no-discord` to every safe-mode run that names none, so the user's own Discord on the TV PC is
  never touched.
- `e2e/payloads/discord/run.ps1`: Settings > Discord with the pad, against `standin discord`.
- `e2e/payloads/discord-tab/run.ps1`: the Guide's Discord tab against it: the Call arriving, a speaker's ring, Mute
  and Leave only once the fake confirms, and the tab's line as the fake goes away.
- `e2e/payloads/discord-servers/run.ps1`: Servers against it: the spinner then the servers, a server's channels with
  who is in them, B back onto the server, a channel joined, its Shortcut after leaving, rejoined in one press, and
  Remove shortcut from its menu.

The log, prefixed `discord: `: `client id set|forgotten`, `client secret saved|forgotten`, `connecting`, `not
running`, `retry in <n> s`, `asking Discord to authorize: approve the request in Discord`, `approved in Discord:
exchanging the code for tokens`, `tokens saved`, `connected as "<name>"`, `lost (<status>)`, `no READY in 10 s`,
`disconnected: the tokens are forgotten`, the refresh's lines, `the saved tokens won't do and Discord won't authorize
again: reconnect in Settings`; and `pipe <name> open`, `asking the token endpoint (<grant>)`, `the token endpoint
answered HTTP <n>`. The focus log on the page: `Settings: Discord: <Client ID Type|Client ID Paste|Client Secret
Type|Client Secret Paste|Connect|Disconnect>; client id <set|not set>, secret <saved|not set>, "<status>"`.

The Call's lines: `in a call in "<title>" with <people>`, `<name> joined the call`, `<name> left the call`, `out of
the call` (`: Discord has gone` as it closes), `Discord says mute on|off, deafen on|off` (as they change), `asking
Discord to mute|unmute|deafen|undeafen|leave the call` (as sent), `Discord refused <CMD>: <message>`, `avatar of <name>
fetched`, `avatar of <name> couldn't be fetched: <why>`. With `--log-focus` the Guide's line on the tab says the
entry focus is on and what Discord says: `guide: Discord Entry(5) Mute; in "General" with E2E Tester, Ana
(speaking), Bo (muted, deafened); mute off, deafen off`, `guide: Discord Entry(2) Servers; not in a call; mute off,
deafen off`, `guide: Discord Entry(0) Unavailable; "Discord isn't running"`; with the Flyout open what it shows
(docs/spec/discord-tab.md, 7).

The Shortcuts' and Servers' lines: `asking Discord for the servers`, `servers: "<name>", ...` (or `none`), `asking
Discord for "<server>"'s voice channels`, `voice channels of "<server>": "<name>", ...`, `in "<channel>": <people>`
(or `nobody`, as who is in it changes), `asking Discord to join "<name>"`, `"<name>" is the call's already: nothing
asked`, `shortcut "<name>" removed`, `icon of <server> fetched`.

## Left for a human

- A real Discord application (Client ID and Secret, the redirect, the user as App Tester) and a signed-in Discord on
  the TV PC: the fake stands in for both in every test here. Real avatars from Discord's CDN, and a real Call's
  Mute, Deafen and Leave, are only seen there.
- Discord's mark on the account card, the Status area, a DM's Shortcut and a server without an icon is a plain
  stand-in shape (`marks::discord_mark`), as on the tab strip.
- Rings (an incoming DM call) are not built; their hooks are marked "Rings" in `voice.rs`.
- The QR code is made at run time from `PORTAL_URL` (`portal_qr` in the page, with `qrcodegen`, a pure-Rust encoder
  with no dependencies): version 4, error correction M, a quiet zone of 4 modules, 41 across, drawn 200 square at x
  1540 as the Bundle's 328-pixel picture is shown, with no smoothing. The Bundle's `DiscordPortalQr.png` is not
  copied. Whether a phone reads it off the TV is for a human to try.
