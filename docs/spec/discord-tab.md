# The Discord tab

The Guide's tab in the console's Parties & chats slot, between the landing tab and Profile & system: Discord's
account, the Call the user is in, and Mute, Deafen and Leave (#144); the Shortcuts to the channels joined before, and
Servers, a Flyout of the user's servers and their voice channels (#145). How it connects to Discord, and what the
Call, the Known channels and the Servers follow, is [../discord.md](../discord.md).

Sources: the Bundle's C/ExtensionCore.DiscordCall.cs (the Call's items, the focus, Mute, Deafen and Leave),
C/DiscordCall.cs (people, names, avatars, titles), C/ExtensionCore.DiscordShortcuts.cs (Known channels, the tiles,
moving among them, Remove shortcut), C/ExtensionCore.DiscordServers.cs (the Flyout), their tests
(DiscordShortcutsTests.cs, DiscordServersTests.cs), and the Guide's XAML (E/OverlayWindow.xaml, `GuideEntry`'s
Discord kinds, 663-875, and the Flyout's). The Bundle has **no picture of the tab** among its reference pictures:
what follows is measured from the XAML alone, and waits for a picture of a real Call to be checked against.

Code: `screens::guide::discord` (the entries, their faces, where focus goes, the tiles' grid),
`screens::guide::servers` (the Flyout), `screens::guide::status` (the Status area's mark), `discord::voice` (the
Call), `discord::shortcuts` (Known channels), `discord::servers` (the Flyout's data), `discord::following` (the three
together), `discord::avatars` (the pictures), `app` (`follow_discord`).

## 1. When it is there

From the first Connect until Disconnect (`Discord::has_tab()`: signed in, or tokens saved), whether Discord is
running or not. Home's Discord tile opens the Guide on it then, and Settings > Discord otherwise, or with the Guide
off. Disconnect while it shows: the Guide goes back to the landing tab.

## 2. What it lists

Under the heading "Discord" (as Profile & system's), from the top:

| Discord | Entries |
| --- | --- |
| Can't show it | One line saying why: `Connecting to Discord`, `Discord isn't running` (tried again by itself), `Reconnect Discord in settings`. Nothing else |
| Signed in, out of a Call | The account: Discord's mark on its blurple, who is signed in. Then "You're not in a voice channel" |
| In a Call | The Call's heading: its channel's name, else the other people in it, else "Call". Then everyone in it, the user first, then Discord's order. Then, under a divider, Mute, Deafen and Leave |
| Signed in, either way | Then the Shortcuts, tiles three to a row, the first row under a divider; then Servers under a divider |

**Shortcuts** are the six most recently joined Known channels, most recently first (discord.md, "Known channels"):
a voice channel joined, a DM or a group DM. A on one asks Discord to join it (`Ask::Join`; nothing is asked when it
is the Call's already); the Menu button opens its menu, titled with its name, with one option, Remove shortcut.
Remove shortcut forgets the channel for good. Shortcuts are kept while Discord is away and across Disconnect: they
show again once signed in.

**Servers** opens the Servers Flyout (section 5).

A on Mute, Deafen, Leave, a Shortcut or a channel asks Discord (`GuideAction::Discord`, `Effect::Discord(..)`) and
sounds as a choice; the Guide stays open. **Nothing shows done until Discord says so**: the switches are Discord's own
voice settings as it last told them, the Call comes and goes when Discord says, and a Shortcut joined shows as the
Call only once Discord has put the user in it. A on anything else does nothing.

## 3. Where focus goes

- **Showing the tab**: the first entry A does something on: in a Call, Mute; else the first Shortcut; else Servers;
  else the first entry. (The Bundle's Answer, first while a Call rings, is not built: rings are not.)
- **As Discord changes** while it shows: going into a Call or out of one, as on showing it. Otherwise it stays on the
  same entry: the same person (by id) as people come and go, the same Shortcut (by channel) as others are added or
  removed, the same Mute, Deafen, Leave or Servers; else the entry now at its place. Someone speaking or muting moves
  nothing.
- **Up and down** move along the entries and stop at either end; the tab has no Utilities.
- **Among the tiles**, as the Bundle's `MoveAmongShortcuts`: left and right go along a row, held at its ends. Down
  goes a row down, else onto the last tile of a shorter row below, else on past the tiles to Servers. Up goes a row
  up, else to the entry above the tiles. Down onto the tiles from above lands on the first; up onto them from Servers
  on the last row's first.
- **The Flyout** takes the pad while it is open; when it closes focus is on Servers, where it was.

## 4. What each entry looks like

Every entry is 518 wide from x 87 (the panel's x + 9), 3 under the one above; focus is the Guide's fill and ring.
Glyphs are Segoe Fluent Icons, the text the Guide's body (Selawik 22 white).

| Entry | Height | At its left | Text | At its right |
| --- | --- | --- | --- | --- |
| Can't show it | 75 | U+E7BA at 26, dimmed (#B3FFFFFF), its middle 36 in | at 69, dimmed, up to two lines | |
| Account | 105 | A 60 disc of #5865F2, 18 in, Discord's mark 32 across in white on it | at 96, one line | |
| Not in a voice channel | 75 | U+E7F6 at 26, dimmed | at 69, dimmed | |
| The Call's heading | 60 | U+E767 at 24, dimmed | at 69, one line | |
| Someone in it | 72 | Their avatar, 56 across 18 in: a 48 disc of #4A4A4A with U+E77B at 22 dimmed, the picture over it cut round; ringed 3 wide in the accent while they speak | at 88, one line, 96 clear of the right | Muted: U+EC54 at 22 dimmed. Deafened: U+E7F6 at 22 with a line (3,3)-(23,23) 2 wide across it. 26 each, 10 between, 24 in from the right |
| Mute | 75 (a divider above) | U+E720 at 26, white | "Mute" at 69 | The Guide's switch, 48 by 24, 24 in from the right: on is the accent with its knob right |
| Deafen | 75 | U+E7F6 at 26, white | "Deafen" at 69 | The switch |
| Leave | 75 | U+E778 at 26 in #ED4245 | "Leave" at 69 | |
| Servers | 75 (a divider above) | U+E716 at 26, white | "Servers" at 69 | |

**A Shortcut's tile** is 160 square in a column 178 wide (its margin 9, 0, 9, 18), three columns from the panel's
x; the first row's divider spans the panel's 476 as every divider does (its margin 30, 14, -328, 26 on the first
tile). In the tile, in a column 10 in from its sides and in its middle: the picture, 72 across, then 10 under it the
name on one line, centred, then the server's name dimmed, centred. A voice channel's picture is its server's icon on
a square rounded by 16, #4A4A4A with U+E767 at 28 dimmed under it while there is none; a DM's is the first other
person's avatar on a disc, #4A4A4A with Discord's mark (34 across, dimmed) under it while there is none. Pictures are
decoded at 144.

Text that does not fit is cut short with an ellipsis. **Pictures** are fetched off the frame and kept (discord.md,
"Avatars"); until one is there the stand-in shows. Discord's mark is a plain stand-in shape, as on the tab strip: no
Discord logo is copied.

## 5. The Servers Flyout

A on Servers opens a Flyout over the panel at once (the Guide's Flyout: x 155, 536 wide, #1F1F1F, its title at the
heading's size), titled "Servers", and asks Discord for the user's servers. While Discord is asked, a spinner 102
across in the middle of the Flyout's list; Discord's answer shows once the spinner has shown a second at least (an
answer that comes sooner is held until then). Opened again, the servers answered before show at once and are asked
afresh; a refusal then keeps those shown.

- **The servers**, in the Bundle's order: the servers of the channels joined most recently first (the Known servers,
  kept with the Known channels), then the rest by name ignoring case. A row each, 96 high: the icon 60 across, 18 in,
  on a square rounded by 16 (#4A4A4A with Discord's mark 30 across dimmed under it while there is none), the name at
  96, one line. None: "No servers", dimmed, at 28.
- **A server**: A on one shows its voice and stage channels (types 2 and 13) in Discord's order in the same Flyout,
  titled with its name. A row each, as tall as it needs: U+E767 at 26 at its top left (14 down), the name at 69 (16
  in from the top and the foot, 16 from the right); under the name who is in it, once Discord has said: their avatars
  on discs 34 across, each over the last by 8 (#4A4A4A, edged #1F1F1F 2 wide, the picture 2 in), 8 under the name, or
  "Empty" dimmed 4 under it. Nothing there until Discord has said. None: "No voice channels". Who is in each is asked
  of Discord channel by channel (GET_CHANNEL), and is never taken for the Call.
- **A on a channel** asks Discord to join it (nothing is asked when it is the Call's already) and the Flyout closes,
  focus on Servers. The Call shows once Discord says the user is in it.
- **B** in a server's goes back to the servers, focus on that server, without asking Discord again; in the servers' it
  closes the Flyout. The Guide button closes the Guide.
- After A on a server or B, focus stays on the row pressed until Discord shows where it went: the server's channels
  (focus on the first) or the servers (focus on the server B came from). It never stands on the first row meanwhile.
- An answer for a Flyout no longer showing (backed out of, or another server opened) is not shown. Discord going
  closes the Flyout and forgets what it answered.

## 6. Elsewhere in a Call

- **The Status area**: Discord's mark, 30 by 24: the mark 24 by 18, dimmed (#B3FFFFFF), in the accent while the user
  speaks; at its foot right a badge 14 across (#1A1A1A, edged white 1) with U+E720 (muted) or U+E7F6 (deafened, which
  wins) at 8 and a stroke (3.5, 3.5)-(10.5, 10.5) 1.25 wide across it. In the Guide 12 left of the PC's battery and the
  pad's; on Home's top bar 16 left of the pad, or of the clock without one.
- **Audio's Flyout** (guide.md 5.2): after Mute, "Mute in Discord" and "Deafen in Discord", 75 high, U+E720 and
  U+E7F6 at 26 and the Guide's switch, Discord's own voice settings. A asks Discord, as on the tab. The Call over, focus
  on either goes to Mute.

## 7. The log

`--log-focus`'s Guide line on the tab: the entry focus is on, then what Discord says (`Discord Entry(2) Shortcut
"Late night" (Stand-in Server); not in a call; mute off, deafen off`), and with the Flyout open what it shows, after
`, flyout Discord <focus>`: its title and the row focus is on (`"Servers" "Stand-in Server" of 3`), `"Servers"
loading`, `"art club" "No voice channels"`, or a channel with who is in it (`"Stand-in Server" "Late night" of 2: 2 in
it`, `: Empty`, `: not known yet`). The line is said after a press and again after what it asked is carried out, so
the Flyout loading, B's servers and a Shortcut removed each get their own line. A removed Shortcut leaves the tab as
Discord confirms it, focus on the next Shortcut, else on Servers.

## 8. Pictures

`crates/screens/tests/reference/`: `guide-discord.png` (signed in, out of a Call), `discord-tab-call.png` (four
people: the user's default avatar, Ana speaking, Bo with no avatar yet and muted, a long name muted and deafened; focus
on Mute; the Status area's mark), `discord-tab-call-muted-deafened.png` (both switches on, focus on Leave, the mark
badged deafened), `discord-tab-not-running.png`, `discord-tab-reconnect.png`, `discord-tab-shortcuts.png` (five
Shortcuts, two of them DMs, three with made-up pictures), `discord-servers-loading.png`, `discord-servers.png`,
`discord-server-channels.png` (three people in one channel, one Empty, one not known yet), `discord-audio-in-call.png`
(Mute in Discord on, focus on it), `discord-home-mark.png` (Home's top bar, the user speaking). Every picture in them
is made up; none is Discord's or anyone's.

## 9. Not here, and how it differs from the Bundle

- **Rings** (an incoming DM call, Answer and Decline, `ExtensionCore.DiscordRing.cs`) are not built: the places they
  hook into are marked "Rings" in `discord::voice`.
- **Closing the Guide with the Flyout open** does not tell the browser, which the Bundle's closing does. Answers that
  come after are kept rather than dropped: the next Servers shows them at once (and asks afresh) instead of the
  spinner.
- **Discord's mark** everywhere is the plain stand-in shape (`marks::discord_mark`); a server's icon and everyone's
  avatar are fetched from Discord's CDN at run time, never shipped.
