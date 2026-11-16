# The Discord tab

The Guide's tab in the console's Parties & chats slot, between the landing tab and Profile & system: Discord's
account, the Call the user is in, and Mute, Deafen and Leave. Built in #144; the Shortcuts and Servers under the
Call come with #145. How it connects to Discord, and what the Call follows, is [../discord.md](../discord.md).

Sources: the Bundle's C/ExtensionCore.DiscordCall.cs (the Call's items, the focus, Mute, Deafen and Leave),
C/DiscordCall.cs (people, names, avatars, titles), and the Guide's XAML
(E/OverlayWindow.xaml, `GuideEntry`'s Discord kinds, 663-875). The Bundle has **no picture of the tab** among its
reference pictures: what follows is measured from the XAML alone, and waits for a picture of a real Call to be
checked against.

Code: `screens::guide::discord` (the entries, their faces, where focus goes), `discord::voice` (the Call),
`discord::avatars` (the pictures), `app` (`follow_discord`).

## 1. When it is there

From the first Connect until Disconnect (`Discord::has_tab()`: signed in, or tokens saved), whether Discord is
running or not. Home's Discord tile opens the Guide on it then, and Settings > Discord otherwise, or with the Guide
off. Disconnect while it shows: the Guide goes back to the landing tab.

## 2. What it lists

Under the heading "Discord" (as Profile & system's), from the top:

| Discord | Entries |
| --- | --- |
| Can't show it | One line saying why: `Connecting to Discord`, `Discord isn't running` (tried again by itself), `Reconnect Discord in settings` |
| Signed in, out of a Call | The account: Discord's mark on its blurple, who is signed in. Then "You're not in a voice channel" |
| In a Call | The Call's heading: its channel's name, else the other people in it, else "Call". Then everyone in it, the user first, then Discord's order. Then, under a divider, Mute, Deafen and Leave |
| (#145) | The Shortcuts, then Servers under a divider |

A on Mute, Deafen or Leave asks Discord (`GuideAction::Discord`, `Effect::Discord(Ask::Call(..))`) and sounds as a
choice; the Guide stays open. **Nothing shows done until Discord says so**: the switches are Discord's own voice
settings as it last told them, and the Call goes when Discord says the user has left it. A on anything else does
nothing.

## 3. Where focus goes

- **Showing the tab**: in a Call, Mute; else (#145: the first Shortcut, else Servers) the first entry.
- **As Discord changes** while it shows: going into a Call or out of one, as on showing it. Otherwise it stays on the
  same entry: the same person (by id) as people come and go, the same Mute, Deafen or Leave; else the entry now at
  its place. Someone speaking or muting moves nothing.
- Up and down move along the entries and stop at either end; the tab has no Utilities.

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

Text that does not fit is cut short with an ellipsis. **Avatars** are fetched off the frame and kept (discord.md,
"Avatars"); until one is there the grey disc and figure show. Discord's mark is a plain stand-in shape, as on the tab
strip: no Discord logo is copied.

Pictures: `crates/screens/tests/reference/guide-discord.png` (signed in, out of a Call), `discord-tab-call.png`
(four people: the user's default avatar, Ana speaking, Bo with no avatar yet and muted, a long name muted and
deafened; focus on Mute), `discord-tab-call-muted-deafened.png` (both switches on, focus on Leave),
`discord-tab-not-running.png`, `discord-tab-reconnect.png`.

## 5. Not here yet

- The Shortcuts, Servers, known channels and rings (#145).
- The Status area's Discord mark in a Call, and the Audio Flyout's "Mute in Discord" and "Deafen in Discord"
  (guide.md 4, 5.2): the Bundle shows both in a Call; not built in #144.
