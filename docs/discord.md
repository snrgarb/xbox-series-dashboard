# Discord

How the Guide's Discord tab (the Call, Shortcuts and Servers) behaves, and which rules the web session keeps. The
behaviour is the Bundle's (`XboxSeries.Core/ExtensionCore.Discord*.cs`, `DiscordCall.cs`), ported first over Discord's
local RPC, which **was retired by #299**: the Dashboard has one Discord integration, the web session
(docs/discord-web.md; `crates/discord_web`), and the TV PC needs no Discord app. Gone with it: `crates/discord`, the
user's own developer application (Client ID, Client Secret, tokens, `discord-secrets.dat`), its Settings page and its
stand-in (`standin discord`, `--discord-pipe`, `--discord-token-url`). Settings > Account > Discord is the web
session's page (docs/spec/settings.md, 16.12); how the session runs, reads and acts is docs/discord-web.md.

## The pieces

| Where | What |
|---|---|
| `crates/model/src/discord.rs` | `Discord`, what the screens see of the voice (ready or not, who is signed in, the Call, Shortcuts, the Servers Flyout); `Ask`, what the tab asks. |
| `crates/discord_web` | The session; its voice (`voice.js`, `src/voice.rs`), Shortcuts (`src/shortcuts.rs`) and pictures (`src/art.rs`). |
| `crates/screens/src/guide/discord.rs`, `servers.rs` | The Discord tab and the Servers Flyout (docs/spec/discord-tab.md). |
| `crates/app/src/window.rs` | `discord_for_screens` hands `Discord` to the screens while the session's voice is ready; carries out `Effect::Discord` through `Session::ask_voice`. |

`Discord::has_tab()` is whether the session's voice is ready; `Dashboard::set_discord` sets `status.discord` from it or
from Direct Messages being on: Home's Discord tile opens the tab then, Settings > Account > Discord otherwise.
`Discord::tab_line()` is what the tab says while the voice is not ready ("Discord isn't connected").

## The Call (#144)

The Discord tab itself is docs/spec/discord-tab.md. What it shows comes from the web session's voice
(docs/discord-web.md, "Voice"); the rules, as the Bundle's `ExtensionCore.DiscordCall.cs` has them and as the web session
keeps them, are:

- The Call is the voice channel the user is in, once its channel and people are known; each Call's people follow it
  as they join, leave, speak, mute and deafen.
- A person is named by nickname, else display name, else username; is muted or deafened whether they or the server
  did it; comes in Discord's order with the user first. A DM or group with no name is named after the other people
  in it, else the last seen, else "Call".
- Mute, Deafen and Leave are asked of Discord's own page. What shows is only ever what Discord said: a switch or a
  Leave is shown done once the session's stores say so. A refused ask changes nothing.
- Signing out or the session going ends the Call; ready again, it is all read afresh.

`Discord.voice` (`Voice { call, mute, deaf }`, `Call { id, title, people }`, `Person { id, name, avatar, speaking,
muted, deafened, me }`) is the screens' copy. `Ask::Call(CallAsk::Mute|Deafen|Leave)` is what the tab asks.

### Avatars

`https://cdn.discordapp.com/avatars/<id>/<hash>.png?size=64`, else Discord's default
`embed/avatars/<(id >> 22) % 6>.png`, fetched off the frame by the session (`crates/discord_web/src/art.rs`) into
`<data>/art/discord/avatars/<id>/<hash>.png` and `.../embed/<n>.png`, and kept: a kept file is used at once ever
after, as art is. Until one is there the tab draws a grey disc with a figure. At most 1 MB, written beside its place
then moved there. A test's CDN is `--discord-cdn-url`.

### Known channels

Ported from the Bundle's `ExtensionCore.DiscordShortcuts.cs` and its tests, kept by `crates/discord_web/src/shortcuts.rs`:

- Each Call begun is learned, at the front: its channel's id, server, name (the Call's heading: the channel's name,
  else the people in it) and picture (a DM's first other person's avatar). A voice channel's server is asked for its
  name and icon and becomes the most recent server; the answer lands on every Known channel of that server.
  The Call's own Known channel takes the name and picture it lacked as people join, without a save for an unchanged
  one; joined again, it is named afresh.
- **DMs are kept until removed; a server's voice channel only while among the six most recent.** The first six are
  the Shortcuts. Remove shortcut forgets one for good, and a removed one is not brought back by its Call going on;
  removing one leaves the servers' order alone.
- Kept in a file in the data folder under the Bundle's names and in its JSON (what the RPC port kept in its encrypted
  store, which is gone): `Discord.Shortcuts`, an array of `{Id, GuildId, Name, Image, ServerName}` (`Image` the
  picture's address on Discord's CDN), and `Discord.RecentServers`, the servers' ids, most recent first. Unreadable
  data reads as none. They are read as the session starts, and kept across signing out.
- A Shortcut's picture is fetched as an avatar is, its address mapped onto the CDN the Dashboard was given (so a test
  never reaches Discord's); a server's icon is kept in `<data>/art/discord/icons/<guild>/<hash>.png`.
- A on one joins that channel, unless it is the Call's already.

### Servers

Ported from `ExtensionCore.DiscordServers.cs` and its tests. What the Flyout is
for, and its looks, are docs/spec/discord-tab.md, 5.

- Servers lists the user's servers; a server, its voice (2) and stage (13) channels in Discord's order and who is in
  each. Only the answer to the request still awaited is shown: one for a Flyout backed out of, or a server left for
  another, is dropped.
- The servers come in the Bundle's order: the Known servers most recent first, then the rest by name ignoring case.
- The spinner shows a second at least: an answer that comes sooner is held until then, and dropped if the Flyout has
  closed meanwhile.
- What was answered is kept while signed in: Servers or a server opened again shows it at once and asks afresh; a
  refusal then keeps it. B back to the servers asks nothing. Discord going forgets it all.
- A on a channel joins it as a Shortcut does, and closes the Flyout.

`Discord.shortcuts` (`Shortcut { id, name, server, picture, dm }`) and `Discord.browse` (`Browse::Closed | Servers {
loading, servers } | Server { id, name, loading, channels }`) are the screens' copies. `Ask::Join(id)`,
`Ask::RemoveShortcut(id)` and `Ask::Browse(Servers | Server { id, name } | Back | Close)` are what the tab asks; the
screens hold the Flyout itself (open, focus, B), and close it as `browse` closes or Discord goes.

## Testing

The tab's screens are `crates/screens/tests/discord.rs` and `direct.rs`; the session's voice is
`crates/discord_web/tests` and its unit tests against the stand-in discord.com (`standin serve-discord-web`,
docs/discord-web.md), and the payloads `e2e/payloads/discord-web`, `discord-dms` and `discord-web-voice`.
