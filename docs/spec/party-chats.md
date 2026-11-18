# Parties & chats: Discord's conversations in the Guide

**Draft for the user's approval (#199, M9 Full Discord).** How Discord's direct messages show in the Guide, in the
manner of the console's Parties & chats tab, drawn with the Guide's own measures and parts. The decisions it rests
on are #192's (D1 opt-in with RPC kept for voice; D2 our own web session; D3 our own screens under Party in the
Guide; D4 DMs and notifications first; D5 no video; D6 memory only; D7 our own code). What the web session can do is
#196's spike: read the DM list by name with unread marks and counts, open a conversation, read its messages, send
text. Section 12 lists what is the user's to approve; section 13 has sketches of the three views.

Read with: [guide.md](guide.md) (the panel, entries, Flyouts, dialogs, Toasts: every measure here not given is
its), [discord-tab.md](discord-tab.md) (the tab as built: voice, the Call, Shortcuts, Servers),
[search.md](search.md) 7 to 10 (the Console keyboard and text entry), [settings.md](settings.md) 16,
[../research/discord-bridge.md](../research/discord-bridge.md) sections 4, 6 and 8, and the Bundle's `CONTEXT.md`
(**Discord tab**, **Call**, **Toast**).

Conventions:

- All numbers are px on the Guide's 1920 x 1080 canvas, as guide.md.
- **(est.)** marks a measure taken from Microsoft's pictures of the console (section 1), read off a 1920 x 1080 JPEG
  by eye and rounded. Everything else is the Guide's own value, cited by guide.md's section, or this spec's own
  choice, said as such.
- Colours are the Guide's tokens: body `#FFFFFFFF` (Segoe UI 26, `theme::BODY`; semibold 600 `theme::BODY_SEMIBOLD`),
  dim `#B3FFFFFF` (`theme::TEXT_DIM`), heading Bahnschrift 33, accent `#FF2CA243` (`theme::ACCENT_LIGHT`), the
  console's green `#FF107C10` (`theme::ACCENT`, the Toast's card), focused fill `#26FFFFFF`, card at rest
  `#14FFFFFF`, divider `#33FFFFFF`, disc `#FF4A4A4A`, Flyout `#FF1F1F1F`, list box `#FF2E2E2E` (the Audio device
  list's), Search's pill `#FF2E3136`, Discord's red `#FFED4245` (Leave's). No new colour is introduced.
- Names in examples are made up. No picture of Microsoft's or Discord's is in the repository: they are linked.

---

## 1. The console's Parties & chats, from public sources

Read 2026-11-18. **Stated** = Microsoft's text says it; **seen** = one of Microsoft's own pictures shows it (linked,
not copied); **recollection** = nobody's source, said so.

### 1.1 What Microsoft says

- The Guide's tabs are "Guide, People, Parties & chats, Game activity, Capture & share, and Profile & system",
  reorderable; LB and RB move between them (stated:
  [Customize Guide tabs](https://support.xbox.com/en-US/help/hardware-network/getting-started-set-up/customize-guide-tabs),
  [Using the Guide](https://support.xbox.com/en-US/help/hardware-network/getting-started-set-up/using-the-guide)).
- The tab "combines Party, messaging and invites into a single location" (stated:
  [Xbox Wire, May 2020 update](https://news.xbox.com/en-us/2020/05/29/may-2020-xbox-one-update/)) and "previews the
  text of new messages" (stated:
  [Xbox Wire, August 2020](https://news.xbox.com/en-us/2020/08/25/xbox-one-and-xbox-series-x-new-digital-experience/)).
- Its entries, as the help pages name them: **Start a party**
  ([How to start a party](https://support.xbox.com/en-US/help/friends-social-activity/parties-chat/how-to-start-a-party)),
  **New chat**, **Chats**, **Chat requests**
  ([Send and receive messages](https://support.xbox.com/en-US/help/friends-social-activity/share-socialize/send-receive-messages-xbox-one)),
  **Options** ([party noise suppression, 2022](https://news.xbox.com/en-us/2022/09/07/rolling-out-today-party-chat-noise-suppression/)),
  **Discord** ([November 2022 update](https://news.xbox.com/en-us/2022/11/16/xbox-november-2022-update-rolls-out-today/)),
  **Party history** ([February 2023 update](https://news.xbox.com/en-us/2023/02/15/xbox-february-update-out-now/)).
- Writing: type in the box, select Enter, then "Send message"; with a headset, View records a voice message (stated,
  Send and receive messages). New chat picks "someone (or several people)" then Done: group chats (same page).
  A chat's row: X, then Clear, deletes it (same page).
- Report: in the conversation, More, "Report person" (stated:
  [File a complaint](https://support.xbox.com/en-US/help/family-online-safety/enforcement/file-a-complaint)).
- Toasts: "a new message or an incoming video call" pops up; "Press and hold the Xbox button" to open it; missed ones
  are in Notifications at the Guide's foot. Settings > Preferences > Notifications has banners on and off, Xbox and
  app notifications with per-kind choices, the position (six places) and the time a pop-up stays (Default, 15 s,
  30 s, 1 min, 5 min) (stated:
  [Pop-up notifications](https://support.xbox.com/en-US/help/hardware-network/settings-updates/pop-up-notifications)).
  Do not disturb is a profile status that holds notifications back (stated:
  [Xbox Wire, January 2018](https://news.xbox.com/en-us/2018/01/08/new-year-new-xbox-one-features/)).
- A party invite's Toast, held open, offers "Accept & play" or "Accept chat" (stated, How to start a party).

Nothing Microsoft published from 2023 to 2026 describes a change to the tab's look; the 2026 Guide can open on the
right ([Xbox Wire, 2026-11-12](https://news.xbox.com/en-us/2026/11/12/xbox-insiders-january-2027-console-features-1/)),
which the Dashboard does not take up. **No source describes reactions, GIFs, Mark as read, the conversation's
bubbles, its timestamps, or whether it widens the Guide.**

### 1.2 What Microsoft's pictures show

**The tab, in a party** ([Parties-chats.jpg](https://xboxwire.thesourcemediaassets.com/sites/2/2020/05/Parties-chats.jpg),
Xbox Wire May 2020, 1920 x 1080; seen, measures (est.)):

| Part | What and where |
|---|---|
| Panel | x 77..617, y 32..1047 (540 x 1015): the Bundle's 78..614 x 33..1045 to within 3 |
| Tab strip | Six marks across the top at y about 82; the Parties & chats mark is two overlapping speech bubbles, a small white figure ("1") at its top right; the selected tab's white bar about 64 wide at y 117 |
| Heading | "Parties & chats", bold, about 36, at x 109, y about 132..168 |
| The party | First entry, the whole panel's width, y 186..321 (135 tall), focused (the 2020 focus was a green fill): "Sasquatch's party - 1 person" at x 111, under it the members' gamerpics, 64 across, from x 111 |
| Invitations | One line at y about 359; a divider under it, x 109..581 at y 411 |
| Chats | "Chats" at x 109, y about 446..470; under it **a row of gamerpics, 64 across** (x 111..175, y 482..546), one per recent chat |
| New chat | One line, y about 600 |
| Message requests | Two lines, y about 669 and 704: "Message requests", then dim "0 unread, 39 total" |
| Messages from Xbox Live | One line, y about 774 |
| Type | Body about 26 (cap height 19); entries' pitch about 70 for one line |
| Beside the panel | The clock, right-aligned to about x 1770, and two status glyphs over it: the Bundle's Status area |

**Out of a party** ([Partieschats-940x528-1.jpg](https://xboxwire.thesourcemediaassets.com/sites/2/2020/04/Partieschats-940x528-1.jpg),
Xbox Wire April 2020, 940 x 528, so doubled for (est.)): the first entry is **Start a party**, one line, about 65
tall, then Invitations, the divider, Chats with its gamerpic row, New chat, Message requests, Messages from Xbox
Live: the same list with the party's entry one line.

**The conversation list and its rows** ([Message-Requests.png](https://xboxwire.thesourcemediaassets.com/sites/2/Message-Requests.png),
Xbox Wire May 2019, the tab as "Communication", a crop at an unknown scale; seen, proportions only):

- On the tab, "Conversations" over a row of five gamerpics, about 8 apart, and a sixth **circle, outlined, reading
  "+33"** for the rest.
- A list of conversations (Message requests'): **a row each, the gamerpic at the left (about the row's height less a
  third), the name, the date right-aligned and dim on the name's line, the last message's text dim on a second line,
  cut short**; a small green dot at the gamerpic's top left for someone online. Over the list: "Notifications" with
  a switch, "Mark all as read", "Delete all".

**Recollection, not sourced** (the research for this spec found no picture of these): a conversation opens in a
panel of its own; one's own messages stand at the right in green, the other person's at the left in grey; the box
at the foot opens the on-screen keyboard; a message's Toast has the gamerpic, the name, the text and "Hold (Xbox) to
open". This spec takes these as its starting point and marks every choice that hangs on them.

---

## 2. Where it sits

### 2.1 Recommended: one tab, "Parties & chats", in the Discord tab's place

The console has one tab for parties and chats; the Dashboard already has the tab "in the console's Parties & chats
slot" (discord-tab.md, CONTEXT.md **Discord tab**). **The chats join that tab** as its own section, between the Call
and the voice parts the tab has now:

| | Value |
|---|---|
| Place in the strip | Second, between the landing tab and Profile & system, as now (guide.md 2.2): x 183..267 |
| Mark | Discord's mark, 36 wide, `#FF8C8C8C` or white, as now (guide.md 10.1). With any unread chat, a dot 12 across in the accent at the mark's top right, its centre 20 right of and 20 above the tab's centre (the console's figure there (est.) is a count; the Guide has no type under 26, so a dot) |
| Heading | **"Parties & chats"** (Bahnschrift 33, guide.md 2.4), the console's name and the user's word (D3: "Party"). Decision 2 |
| When it is there | Signed in to the voice connection or tokens saved (as now, `Discord::has_tab()`), **or** Messages switched on (section 8). Either alone is enough; each part of the tab says for itself when it can't show (section 3.5) |
| Code name | The tab stays `Tab::Discord`, and `--log-focus` keeps `Discord` (section 10), so the payloads written for the tab stand |

**Why one tab** (D1): voice and text come over two connections, RPC and the web session, which are up and down on
their own. In one tab each section says its own state and the other goes on working, as the console's tab holds the
party and the chats together. Two tabs would show or hide a whole tab as a connection comes and goes, add a bumper
press between the landing tab and Profile & system, and split what the console keeps together.

### 2.2 The alternative: two tabs

"Discord" (voice, exactly as discord-tab.md) then **"Chats"** (mark: `theme::glyph::CHAT`, U+E8BD at 33, the
console's speech bubbles in the icon font), at x 267..351, Profile & system moving to x 351..435. The Chats tab
holds sections 3.2 to 3.5 alone, with the conversation rows of section 4 straight on the tab instead of the avatar
row. Simpler drawing; the cost is above.

---

## 3. The tab, with chats

### 3.1 What it lists, top to bottom

With Messages off the tab is exactly discord-tab.md's. With Messages on:

| # | Section | When | Entries |
|---|---|---|---|
| 1 | **Answer** | A DM call is ringing (section 7.3) | One entry, as the Bundle's ring (CONTEXT.md: Answer atop the tab): 96 tall, the caller's avatar 60 across 18 in, "Answer" at 96 in, under it dim "{caller} is calling" |
| 2 | **The Call** | In a Call | discord-tab.md 2 and 4, unchanged: its heading, the people, a divider, Mute, Deafen, Leave. The console's party entry |
| 2' | Voice's line | The voice connection can't show (`Connecting to Discord`, `Discord isn't running`, `Reconnect Discord in settings`) | discord-tab.md's line, unchanged |
| 3 | **Chats** | Always, with Messages on | A divider (only with something above), the "Chats" line, the avatar row, the name line (3.2) |
| 4 | **Shortcuts** and **Servers** | Signed in to the voice connection | As built (discord-tab.md 2), under a divider |

Out of a Call, the account entry and "You're not in a voice channel" are not shown while Messages is on: the
console's tab has neither, and the Chats section is what stands first. Decision 3 has the other order.

### 3.2 The Chats section (the console's gamerpic row)

| Part | Value |
|---|---|
| Divider | guide.md 2.5's: 1 high `#33FFFFFF`, x 108..584, 14 above and 26 below. Not drawn when Chats is the first section |
| "Chats" | Segoe UI 26 white, at x 106 (the heading's x), one line, 34.6 tall; 12 under it the row. The console's "Chats" (est. x 109) |
| Avatar row | Up to **5 chats**, Discord's DM list order (most recent first, as Discord orders it), then a sixth circle. Each a disc **64 across** (est., the console's), from x 106, **80 apart** (x 106, 186, 266, 346, 426, 506; the last ends at 570, inside the panel's 605): 16 between discs so two neighbours' rings (8 outside each, guide.md 2.5) meet and do not cross |
| A chat's disc | The avatar, cut round, over a `#4A4A4A` disc; without one yet, the first letter of the name (Segoe UI semibold 26, white, centred); a group DM with no icon of its own: U+E716 (People) at 26, dim |
| Unread | A dot 16 across, the accent, ringed 3 in the panel's colour, at the disc's top right (its centre 27 right of and 27 above the disc's centre). The count is on the name line. Discord's DMs carry a count for every unread conversation (#198 reads it) |
| The sixth circle | A ring 2 wide `#80FFFFFF` (the drop box's border, guide.md 5.3), 64 across, no fill. With more than 5 chats it reads "+{n}" (the rest; Segoe UI 26 white, centred), as the console's "+33" (seen, 2019); with 5 or fewer, U+E8BD (Chat) at 26. A on it opens **All chats** (section 4) |
| Name line | 20 under the discs (clear of a ring), x 106..586, one line, ellipsis. With focus on a chat: its name, white, then dim " · {n} unread" when unread ("· 1 unread"). With focus on the sixth circle: "All chats" white, then dim " · {n} unread in {m}" when any. With focus elsewhere: dim "{n} unread" over all chats, or nothing at none |
| Focus | The Guide's ring round the disc (4 thick, 4 clear, so its outer edge 8 outside the disc), with the glow; no fill (a disc is not a box) |
| Height | 34.6 + 12 + 64 + 20 + 34.6 = **about 165** from the "Chats" line's top to the name line's foot |

**Why a row of pictures and not rows with names**: it is what the console's tab draws (seen, 2019 and 2020), and it
keeps the tab short enough that Shortcuts and Servers are still on it. The names, the last messages and the times
are one press away, in All chats. Decision 4 is the other way.

### 3.3 Where focus goes

guide.md 3.2 and discord-tab.md 3, with:

- **Showing the tab**: ringing, Answer; in a Call, Mute (as now); else the **first chat** (the first disc); with no
  chats, the sixth circle.
- **Up and Down** move between sections as between entries: Down from the last entry above the row lands on the
  first disc; Down from anywhere on the row goes to the entry under the section (the first Shortcut); Up from the row
  to the entry above it (Leave, or nothing at the tab's top); Up from the first Shortcut lands on the **disc focus
  last left** in the row (else the first).
- **Left and Right** move along the row, held at its ends; on the other entries they do as now.
- **As chats change** while the tab shows: focus stays on the same conversation (by Discord's channel id) as it
  moves along the row, even off it (then onto the sixth circle); it never jumps because a message came.

### 3.4 Buttons on the tab

| Button | On a chat's disc | On the sixth circle | Elsewhere |
|---|---|---|---|
| A | Opens the conversation (section 5), the Guide's select sound | Opens All chats (section 4) | As now |
| Menu | The chat's menu (3.6) | Nothing | As now (a Shortcut's menu) |
| Left / Right | Along the row | Left back along the row | As now |
| X, Y, View, triggers | Nothing | Nothing | Nothing |
| B, Guide, LB/RB | As the Guide everywhere (guide.md 3.1) | | |

The hints beside the panel add "More options" (the Menu glyph) with focus on a chat, as on a Running game
(guide.md 3.6).

### 3.5 Section states

Each is one entry in the Chats section in the row's place, under the "Chats" line, in the "can't show it" entry's
form (discord-tab.md 4: 75 tall, its glyph at 26 dim, its text at 69, dim, up to two lines):

| State | Glyph | Text | A |
|---|---|---|---|
| Starting (the web session coming up, the list not yet read) | the Guide's spinner, 36 across, its middle 36 in | "Loading chats" | Nothing |
| Signed out (never signed in, or Discord ended the session) | U+E77B (Contact) | "Sign in to Discord in Settings > Account > Discord messages" | Closes the Guide and opens Settings there, as the Guide's Settings does (guide.md 3.3), focus on Sign in |
| No WebView2 runtime | U+E7BA | "Messages need Microsoft Edge WebView2, which isn't on this PC" | Nothing |
| Can't reach Discord (no network, Discord down) | U+E7BA | "Can't reach Discord. Trying again" | Nothing; it tries again by itself |
| Discord's page changed (the health probe, #198, finds a selector that no longer matches) | U+E7BA | "Discord has changed its page, so chats can't be read until the Dashboard is updated" | Nothing |
| Signed in, no DMs | U+E8BD | "No chats yet" | Nothing |

All are focusable, as the tab's other lines are. The spinner shows only once a second has passed with nothing to
show (the Servers Flyout's rule, discord-tab.md 5), so a quick start never flashes it.

### 3.6 A chat's menu

The Guide's menu panel (guide.md 6.1), titled with the chat's name:

| Option | Icon | When | Does |
|---|---|---|---|
| Mark as read | U+E8FB (Accept) | Unread | Asks the page to mark it read (#201); the dot goes when the page shows it read |
| Call | U+E717 (Phone) | Signed in to the voice connection | Asks Discord over RPC to join the DM's call (`Ask::Join`, as a DM Shortcut does); the Call shows when Discord says so |
| Cancel | U+E72B | Always | Closes the menu |

**Not offered** (decision 7): Open in the Discord app, which would bring the app in front of a game.

---

## 4. All chats (a Flyout)

The Guide's Flyout (guide.md 5.1: x 155..691, y 33..1080, `#1F1F1F`, the panel dimmed under it), titled **"Chats"**.
Its rows are the console's conversation list (seen, 2019) in the Guide's entry:

| Part | Value |
|---|---|
| Row | Box x 164..682 (518), **96 high**, 3 below; the Guide's fill and ring with focus (guide.md 2.5) |
| Avatar | 60 across, 18 in, centred, as the Profile entry (guide.md 2.5); the stand-ins of 3.2 |
| Name | From 96 in to the time's left less 16, Segoe UI 26 white, one line, ellipsis; **semibold** (`BODY_SEMIBOLD`) while unread |
| Time | The last message's time when the page gives it (#198 to find out; Discord's DM list shows none today): dim, right-aligned 24 in from the box's right, on the name's line. "21:40" today, "Yesterday", a weekday within 7 days, else the short date, in the user's locale (as the clock, guide.md 2.3) |
| Second line | Dim, one line, ellipsis: the last message's text when the page gives it (the console's preview); else for a group DM "{n} members"; else none, and the name is centred in the row |
| Unread | A pill at the right of the second line: at least 34 wide, **34 high**, radius 17, the accent, the count in Segoe UI 26 white, padding 10 each side, right edge 24 in from the box's right. (The Utility's badge is 24 high and its 26 figure overflows it, guide.md open question 14: this one is sized to fit.) With unread but no count, a 12 dot in the accent there |
| Order | Discord's, as the page lists it |
| More | The page loads more as it is scrolled (#196: 25 over 12 pages). With focus within 3 rows of the end and more to load, a row "Loading" with the spinner (36) is added at the end until they come; focus stays where it was |

Buttons: Up and Down along the rows (held at the ends, repeating at the Guide's pace); **A opens the conversation in
this Flyout's place** (as a server opens onto its channels, discord-tab.md 5); Menu opens the chat's menu (3.6); B
closes the Flyout, focus back on the sixth circle. Focus starts on the first row, or on the chat that had focus on
the tab if it is in the list. States: section 3.5's lines at x 183 under the title.

---

## 5. The conversation

### 5.1 Where it opens: a Flyout, the Guide's width (recommended)

A conversation opens as the Guide's Flyout (x 155..691, y 33..1080, 536 wide; guide.md 5.1), from the tab or in All
chats' place. B goes back to where it was opened from, focus on that chat.

**Why the Guide's width**: the Flyout is the Guide's one way to open a level beside the panel (Audio, Notifications,
Servers), Microsoft's picture of the tab has the console's panel at this width, and a 480 text column holds about 30
characters a line at 26, enough for chat. No source says whether the console widens (section 1.2). **The
alternative** (decision 5): a wide panel x 78..1062 (984 wide, the panel's left and radius, over the hints), with
bubbles up to 640.

### 5.2 Layout

| Part | Value |
|---|---|
| Title | The chat's avatar, 48 across, at x 183, its middle level with the title's; the name 16 after it (x 247), Bahnschrift 33 white, top y 69, one line, ellipsis, to x 663. A group DM: its name, else its members' names joined by ", " |
| Divider | 1 high `#33FFFFFF`, x 183..663, at y 133 |
| Message view | y 134 to 970, clipped. Content column x 183..663 (480). The ring of a focused bubble is never cut: the view is drawn 8 wider each side and keeps a focused bubble 12 inside its top and foot (guide.md 2.4's room for a ring) |
| Reply box | x 183..663, y 986..1040: Search's pill (`#2E3136`, radius 27, 54 high; search.md 10.2). From 28 in, Segoe UI 26: the draft, white, one line, showing its end with an ellipsis at the start when long; with no draft, dim "Message {name}". Focus: the Guide's ring round the pill (radius 35). While a reply is set (6.2), a line over it, y 948..982: a bar 3 wide `#80FFFFFF` at x 183, then dim "Replying to {name}" at x 199, ellipsis; the view ends over it (y 940) |

### 5.3 A message

**Theirs** stand at the left, **one's own** at the right, as the console (recollection) and every chat program.

| Part | Theirs | Own |
|---|---|---|
| Bubble | From x 183 (a group DM: x 231, after a 36 avatar at x 183 level with the run's first bubble's top) | Its right edge at x 663 |
| Fill | `#2E2E2E` (the Guide's list box) | `#107C10` (the console's green, the Toast's card) |
| Shape | Radius 12, padding 16 left and right, 10 top and bottom | The same |
| Width | As wide as its text, at most **400** (a group DM's theirs: 384) | At most 400 |
| Text | Segoe UI 26 white, wraps at the bubble's width, 34.6 a line, every line shown (a long message is long; the view scrolls) | The same |

- **A run** is one author's messages within 7 minutes of each other (Discord's grouping): 4 between its bubbles, 16
  between runs. In a group DM the name stands over a run of theirs, dim, at the bubble's x, 4 above it.
- **The time** under a run's last bubble, dim, 4 under it, at the bubble's side ("21:40", the short time format).
  "Edited" follows it, dim, after " · ", on an edited message's run (Discord marks edits per message: on the message,
  it shows on the last of the run with any edited).
- **A day divider** before the first message of each day: the day centred, dim ("Today", "Yesterday", a weekday
  within 7 days, else the short date), with a 1-high `#33FFFFFF` line either side out to x 183 and x 663, 24 above
  and below.
- **The unread line**, before the first message that was unread when the conversation opened: a 2-high line in the
  accent from x 183 to x 663, with "New" in the accent, Segoe UI semibold 26, at its right end over a 12 gap. It
  stays where it is while the conversation is open, as Discord's does, and is gone when it is opened again.
- **A reply** (a message answering another): inside the bubble, over its text, a line: a bar 3 wide `#80FFFFFF`,
  then 8 after it the other message's author and first line, dim, one line, ellipsis ("Ana: are you on tonight?"),
  6 above the text. Replied-to message gone: "Original message was deleted", dim.
- **Mentions** (`@name`) in the accent, semibold. **Links** as text, white, underlined 1; never opened.
- **Emoji** in text: Segoe UI Emoji at 26, drawn as the renderer can (colour where it can, as a glyph where not; a
  square where the font is missing, as `ui.icon`). A server's own emoji as `:name:`, dim.
- **Reactions**, under the bubble, 6 below it, from its side: pills 36 high, radius 18, `#14FFFFFF`, 8 apart,
  wrapping; each the emoji at 24 then the count in Segoe UI 26 white, padding 10 each side. One the user made: ringed
  2 in the accent. A server emoji's: `:name:` dim. At most 8 shown, then "+{n}" dim.
- **What isn't text** (D5, D6): one line each inside the bubble, a glyph at 26 dim then the word, dim, in place of
  the thing; nothing is fetched, played or kept:

  | Kind | Glyph | Line |
  |---|---|---|
  | Image | U+EB9F | "Image" ("3 images") |
  | Video | U+E714 | "Video" |
  | GIF | U+EB9F | "GIF" |
  | File | U+E8A5 | "File: {name}" |
  | Voice message | U+E720 | "Voice message" |
  | Sticker | U+E76E | "Sticker: {name}" |
  | Link preview (embed) | U+E71B | "{title}", else "{site}" |
  | Poll | U+E9D9 | "Poll: {question}" |
  | Anything else the page shows | U+E946 | "Something the Dashboard can't show" |

  Glyph code points are to be checked against Microsoft's list in #200 (guide.md 10.1's link).
- **Discord's own lines** (a call started, someone added to a group, a pin): centred, dim, no bubble, with U+E717,
  U+E8FA or U+E718 at 26 before them ("Ana started a call", "Bo added Cy", "Ana pinned a message").
- **One's own message going out**: drawn at once, opacity 0.6, with "Sending" dim as its time; as the page shows it,
  it is drawn whole with its time. **Not sent**: U+E783 at 26 in Discord's red, then "Not sent" white, as its time
  (6.4).

### 5.4 Focus, scroll and paging

- **Opening**: with no unread message, or all the unread in view with the newest at the foot, focus is on the
  **reply box** and the view shows the newest at its foot. With unread messages that don't all fit, focus is on the
  **first unread message** and the view has the unread line 12 under its top.
- **Up and Down** move a message at a time (each message is its own stop, a run's too; Discord's lines are not
  stops); **Down from the newest message is the reply box**; Down on the reply box does nothing. Held, they repeat at
  the Guide's pace (guide.md 3.1).
- **LT and RT** page: focus moves up or down by as many messages as the view shows, then to the oldest or newest
  loaded; RT on the newest goes to the reply box. (Triggers do nothing elsewhere in the Guide; here they are this
  spec's own.)
- **The view scrolls by the least movement** that shows the focused bubble whole with its ring, as the Guide's lists
  (guide.md 2.4), not animated. A bubble taller than the view is shown from its top.
- **Older messages**: with focus on the oldest loaded message, the page is asked for older ones (#198); meanwhile a
  row at the view's top, 60 tall, the spinner 36 centred. When they come, focus stays on the same message and the
  view stays where it was. At the start of the conversation, a dim centred line over the first message: "This is the
  start of your messages with {name}".
- **New messages while open**: added at the foot. With focus on the reply box (the newest in view) the view follows
  them. With focus higher up, the view stays and a pill shows at the view's foot, 12 over the reply box, centred:
  36 high, radius 18, the accent, "{n} new" white with U+E74B (Down) at 20; RT or Down to the reply box takes it
  away. Messages edited or deleted change in place; focus on a deleted one moves to the next newer.
- **Read**: the conversation open in the page is read as Discord counts it (the page marks what it shows); whether
  the hidden page does so is for #198 and #201 to establish, and Mark as read (3.6) covers it if not.

### 5.5 Buttons in the conversation

| Button | On a message | On the reply box |
|---|---|---|
| A | The message's menu (6.3) | Opens the keyboard (6.1) |
| Menu | The message's menu | **Sends the draft**, if any (the console's Menu to send; with none, nothing) |
| X | Reply to it (6.2) | Clears the reply being set, if any |
| Y | React to it (6.3's React) | Nothing |
| Up / Down, LT / RT | 5.4 | 5.4 |
| B | Closes the conversation, the draft kept (6.1), back to where it was opened, focus on its chat | The same |
| View, LB / RB, stick clicks | Nothing | Nothing |
| Guide | Closes the Guide; the conversation and its draft are not reopened next time (guide.md 3.2: the Guide opens on the landing tab), but the draft is kept for when it is opened again | |

Hints beside the panel are hidden while a Flyout is open (guide.md 5.1); X and Y here have no hint. The keyboard's
keys carry their own badges (search.md 7.3).

---

## 6. Writing (#201)

### 6.1 The Console keyboard

A on the reply box puts up **text entry** (search.md 10) over the whole screen, the Guide and the conversation under
its dim, unchanged in its layout: caption at y 570, field 620..674, keyboard 710..970, all x 496..1424. While it is
up the conversation's view ends at **y 556** and shows the newest messages at its foot, so what is being answered is
readable over the caption through the dim's lighter part (35% black to y 432).

| | Value |
|---|---|
| Caption | "Message {name}", or "Reply to {name}" while a reply is set |
| Starts with | The draft |
| Limit | 2000 characters (Discord's for a message) |
| Typing | Each change is the draft at once (`Event::Typed`) |
| **Menu, or Enter** (the key or a keyboard's) | **Sends** (`Event::Kept`): text entry goes, the message goes out (6.4), the draft is emptied, focus on the reply box |
| **B**, Escape | Text entry goes **and the draft is kept as typed**, not put back as other text entries do (decision 6): focus on the reply box, which shows it |
| Blank text sent | Nothing goes; the draft is emptied |

The keyboard is the Board as it is (search.md 7 to 9): X backspace, Y space, LB and RB the cursor, the left stick
click Shift, LT symbols, Menu Enter. Nothing new is drawn for it. A physical keyboard's text goes in as everywhere.

**The draft** is kept in memory for each conversation while the Dashboard runs (decision 8), and shows in the reply
box when it is opened again; closing the Guide keeps it; it is never written anywhere (section 9).

### 6.2 Replies

X on a message, or Reply in its menu: the reply is set (the line over the reply box, 5.2), focus moves to the reply
box and the keyboard comes up at once with "Reply to {name}". Sent, the message goes as a reply to that one. B on the
keyboard keeps the reply set with the draft; X on the reply box clears it.

### 6.3 A message's menu, and reactions

The menu panel (guide.md 6.1), titled with the message's first line, cut short:

| Option | Icon | When |
|---|---|---|
| Reply | U+E97A (Reply) | Always |
| React | U+E76E (Emoji) | Always |
| Try again | U+E72C (Refresh) | One's own, Not sent |
| Delete | U+E74D | One's own, Not sent: drops it from the view (it was never sent) |
| Cancel | U+E72B | Always |

**React** (and Y on a message) opens a second menu panel in the first's place, titled "React", with **six
reactions** (decision 9): 👍 Thumbs up, ❤️ Heart, 😂 Laughing, 😮 Surprised, 😢 Sad, 🔥 Fire, each a row with the
emoji at 22 in the icon's box and its name; a reaction the user has already made reads "Remove {name}". A asks the
page; the pill shows (or goes) once the page shows it (#201: shown done only when Discord's page shows it). B goes
back to the message's menu.

### 6.4 Sending, one at a time

- A message sent shows at once as Sending (5.3). The web session sends **one action at a time**, each paced (#201;
  discord-bridge.md 4: at most one action per 0.7 s, as the bridge paces itself) and counted done only when the
  page shows it. Several sent quickly wait their turn, each shown Sending.
- **Not sent** (the page shows it failed, or nothing shows within 15 s): the bubble stays, "Not sent" as its time;
  its menu has Try again and Delete. The draft is not put back: the text is in the bubble.
- **Discord asks for a check** (a captcha) or **refuses** (rate limit, blocked, not allowed): the message is Not sent
  and a prompt (guide.md 6.1, 6.4) says so plainly, once: "Discord wants to check it's you" / "Sign in again in
  Settings > Account > Discord messages to carry on.", or "Discord didn't send it" / "{Discord's own reason, as the
  page shows it}", with one option, OK.
- **Mark as read** is 3.6's; a conversation is otherwise read as the page counts it (5.4).

---

## 7. Toasts (#202)

guide.md 9: the banner, its queue, its 5 s, the text cycle, the Guide button's tap. Three new kinds:

### 7.1 A direct message

| Part | Value |
|---|---|
| Tile | The sender's avatar, round, 80 across, bare on the card (as the greeting's picture, guide.md 9.2); without one, Discord's mark on the lighter square (the Call kind's) |
| Line | "{name}: {text}" (the message's first line), one line, ellipsis at the card's 640. A group DM: "{name} in {group}: {text}". With Show what they say off (8.2): "{name} sent you a message" / "{name} in {group}" |
| Not text | The line uses 5.3's word: "Ana: Image" |
| Prompt | "Press (Guide) to view", as every Toast (guide.md 9.2) |
| Tap of the Guide button | Opens the Guide on Parties & chats **with that conversation open**, focus on the reply box (the console's "hold to open"; ours is the Guide's tap, guide.md 9.5) |
| Several | A DM Toast waiting in the queue for the same conversation is replaced by the newer one, which says "{name}: {n} new messages" when more than one is waiting: the user sees one Toast for a burst |
| None | While that conversation is open in the Guide; while Messages' Toasts are off; per 7.4 in a game |

### 7.2 A mention

A mention of the user in a server channel (the page raises it; channels themselves are #203): "{name} mentioned you
in #{channel}", Discord's mark on the lighter square. The tap opens the Guide on Parties & chats (no channel view
until #203). Decision 10 has the other way: no mention Toasts until #203.

### 7.3 An incoming call

The Bundle's ring, not yet built in the port (discord-tab.md 9): **"{caller} is calling"**, the caller's avatar (else
Discord's mark at 48, the Call kind, guide.md 9.6), shown up to **30 s** (`RingToastDuration`, the Bundle's
`ExtensionCore.DiscordRing.cs`), cutting in before waiting notifications, with the notification sound; the Answer
entry atop the tab while it rings (3.1). The tap opens the Guide on the tab, focus on Answer. A on Answer joins the
DM's call **through the voice connection** (RPC, D1), asking "Leave {X} and answer?" first when in another Call, as
the Bundle. **Without the voice connection** nothing can answer: the Toast says "{caller} is calling", the tap opens
the conversation, and there is no Answer. A ring seen by both connections is one ring (by channel).

### 7.4 In a game, and do not disturb

- A setting, **While playing** (8.2): "Show all" / "Calls only" / "None", for while a game is running; default
  **Show all** (decision 11). Toasts never take the pad from a game (guide.md 9.1), so showing is safe.
- Discord's own **Do Not Disturb** status: the page raises no notification then, and so no Toast shows (Discord's
  rule, to be checked in #202).
- What the log says of these Toasts: section 10.
- **Not added to the Notifications Flyout's list** (guide.md 9.7): the chats have their own unread marks, and the
  list would hold message text for as long as the Dashboard runs (decision 12).

---

## 8. Settings > Account > Discord messages

### 8.1 Where (recommended: a box of its own)

Account (settings.md 16.1) gets a fourth box, **Discord messages**, at (1, 1) beside Discord; its glyph U+E8BD. The
Discord box and its page stay as built (the voice connection). #197 says "Settings > Discord gets the switch";
**the recommendation is a page of its own** (decision 13), since the Discord page is full (its status line ends near
y 830) and the two connections are separate things for the user to set up. The alternative: the same rows under the
Discord page's, the page scrolling as 16.2 does.

### 8.2 The page

Rows from y 190 at x 520, the Guide page's controls (settings.md 4.4, 4.5, 4.7), the header "**Account** Discord
messages" (16.1):

| # | Row | Control | Default |
|---|---|---|---|
| 1 | **Messages from discord.com** | Checkbox row | **Off** (D1: opt-in) |
| | Under it, 14 down, MutedText, wrapping to x 1750: **"This signs in to discord.com as you, then reads and sends your messages for you. Discord's terms don't allow programs to do that with a user account, and Discord could close your account for it."** | text | Always shown (settings.md 16.11 keeps text that says what the user cannot do without) |
| 2 | **Sign in** / **Sign out** | Pane buttons, 20 apart | Dimmed while row 1 is off |
| | The status line under them (Text): "Not signed in", "Starting...", "Signed in as {name}", "Discord signed you out", "Microsoft Edge WebView2 isn't on this PC", "Can't reach Discord", "Discord has changed its page" | text | |
| 3 | "Notifications" (the section title, Bahnschrift 30) | | |
| 4 | **Messages** | Checkbox row | On |
| 5 | **Show what they say** | Checkbox row | On (decision 11: off for a shared living room) |
| 6 | **Mentions** | Checkbox row | On |
| 7 | **Calls** | Checkbox row | On |
| 8 | **While playing** | Dropdown: Show all, Calls only, None | Show all |

- **Sign in** shows Discord's own login page full screen with the pad (#197); **Sign out** removes the session folder
  and says, on the status line, "Signed out. Discord lists this PC under Devices until you log it out there" (#196:
  the spike's sign-out left the session listed). Turning row 1 off stops the web session and forgets every chat,
  message and draft in memory, and **keeps the sign-in** (decision 14): on again, no sign-in is asked.
- The risk line's sources: Discord's [Terms of Service](https://discord.com/terms) ("scraping our services ...
  including by using any robot, spider, crawler, scraper, or other automatic device, process, or software") and the
  support article [Automated user accounts (self-bots)](https://support.discord.com/hc/articles/115002192352), as
  read in discord-bridge.md section 4.
- Focus and B as settings.md 9.4 and 16.1; the log: `Settings: Discord messages: <row>; messages <on|off>,
  <signed in|signed out|starting|no runtime|unreachable|page changed>` (no name, section 10).

---

## 9. Privacy: memory only (D6)

1. **Nothing of a message is written to disk by the Dashboard**: not text, names, avatars of people chatted with,
   drafts, unread counts, conversation lists or times. Not in the library database, the settings table, the art
   folder or any cache. The voice part's avatars stay as built (`<data>/art/discord/avatars`); a chat's avatars are
   fetched into memory only and dropped with the web session (decision 15).
2. **Nothing of a message or a name reaches the log**, `--log-focus` included: counts, positions and states only
   (section 10). Types that carry message text or names write themselves in `Debug` as their size (`Text(24 chars)`),
   so a panic or a `{:?}` cannot print them.
3. **Text entry for a message** logs its state without its text (`TextEntry: A; 12 chars, lower letters`, not
   `"Ha|"`; ui.md's `Page::state`): a flag on the entry, as `masked` is, that the keyboard's state line honours.
4. **The web session's own disk** (WebView2's user data folder, #197) keeps Discord's sign-in, which it must; its
   HTTP cache, where Discord's page keeps what it fetched, is cleared each time the web session stops
   (`ICoreWebView2Profile2::ClearBrowsingData` with the disk cache kind,
   [Microsoft's reference](https://learn.microsoft.com/microsoft-edge/webview2/reference/win32/icorewebview2profile2)).
   What Discord's page keeps in its own local storage is Discord's; #197 is to list it.
5. **Toasts** follow 7.1's Show what they say. Their log line names the kind, never the line (section 10).
6. **Tests and payloads** use made-up names and fixture pages only (#198); no screenshot of a real conversation is
   ever taken or committed.

---

## 10. The log

`--log-focus`'s `guide:` line on the tab (discord-tab.md 7), with chats: the entry, then what Discord says, then the
chats' counts. **Places and counts, never names.**

| Where | Line |
|---|---|
| A chat on the row | `guide: Discord Chat(1) of 25, unread 3; not in a call; mute off, deafen off; chats 25, unread 5 in 2` |
| The sixth circle | `guide: Discord AllChats; ...; chats 25, unread 5 in 2` |
| A section state | `guide: Discord ChatsState(SignedOut)` (`Loading`, `NoRuntime`, `Unreachable`, `PageChanged`, `Empty`) |
| Answer | `guide: Discord Answer; ringing` |
| All chats | `..., flyout Chats Chat(7) of 25` (`, unread 2`, `, group`), `..., flyout Chats loading` |
| The conversation | `..., flyout Conversation Message(3) of 50, theirs` (counted from the newest, 0; `, own`, `, unread line above`, `, sending`, `, not sent`), `..., flyout Conversation ReplyBox; draft 12 chars` (`, replying`), `..., flyout Conversation loading`, `..., 2 new below` |
| A menu over it | guide.md 14's form with the title left out: `dialog Menu { prompt: false, row: 1 } (chat)`, `(message)`, `(react)` |
| Text entry | `focus: TextEntry: A; 12 chars, lower letters` |
| Toasts | `toasts: shown (DiscordMessage)`, `(DiscordMention)`, `(DiscordCall)`: the kind and the queue's figures, no line |
| The web session | `discord web: signed in`, `discord web: 25 chats, 5 unread in 2`, `discord web: conversation open, 50 messages`, `discord web: sent in 769 ms`, `discord web: not sent: captcha` (`rate limited`, `refused`, `no answer in 15 s`), `discord web: selector "dm-row" no longer matches` (selector names are ours, not Discord's text) |

Never in the log: names, channel or user ids, message text, emoji used, times of messages.

---

## 11. Not in this version

- **Servers' text channels, threads and forums**: #203, after DMs have lived through a Discord page change (D4).
- **New chat** and **message requests**: the console has both; they need Discord's friend list and requests read,
  which #198 does not. Decision 16.
- Video, screen share, streams (D5); voice messages, GIFs, stickers, uploads sent; editing or deleting a sent
  message; pins; search; typing ("Ana is typing"); presence dots; profiles; blocking and reporting (Discord's page
  does these).
- Showing Discord's page in the Guide (D3): it is shown only for sign-in.

---

## 12. Decisions for the user

Each with the recommendation first, then the other way in a line.

1. **One tab.** Chats join the Discord tab as a section. *Or:* a second tab, "Chats", after Discord (2.2).
2. **The tab's heading reads "Parties & chats"**, the mark stays Discord's. *Or:* the heading stays "Discord".
3. **Order on the tab: Answer, the Call, Chats, then Shortcuts and Servers**; out of a Call, Chats first and the
   account entry hidden. *Or:* voice first as now (account, Shortcuts, Servers), Chats under them.
4. **On the tab, a row of 5 avatars and a "+n" circle** (the console's), names in All chats. *Or:* rows with name,
   last message and unread count straight on the tab (All chats' rows), 5 of them.
5. **A conversation opens as a Flyout, the Guide's width (536).** *Or:* a wide panel, 984, over the hints.
6. **B on the keyboard keeps the draft**; Menu sends. *Or:* B drops what was typed, as every other text entry.
7. **No "Open in the Discord app"** in a chat's menu. *Or:* offer it (it brings the app in front of a game).
8. **A draft per conversation, kept while the Dashboard runs**, in memory. *Or:* one draft, dropped when the
   conversation closes.
9. **Six reactions**: 👍 ❤️ 😂 😮 😢 🔥. *Or:* another six, or Discord's frequently used if the page gives them.
10. **Mention Toasts now**, opening the tab. *Or:* none until server channels (#203).
11. **Toasts show the message's text, and show during games**, both switchable. *Or:* names only and Calls only
    while playing by default, for a shared living-room screen.
12. **Discord Toasts are not added to the Notifications list.** *Or:* added, text and all, for the Dashboard's run.
13. **Settings > Account > Discord messages, a page of its own.** *Or:* the same rows under Settings > Account >
    Discord (as #197's wording), the page scrolling.
14. **Turning Messages off keeps the sign-in.** *Or:* off also signs out.
15. **Avatars of people chatted with are kept in memory only.** *Or:* on disk with the voice avatars.
16. **No New chat or message requests in M9.** *Or:* a ticket for them after #198, reading Discord's friend list.

---

## 13. Sketches

At the Guide's proportions: one column is 16 px, so the panel (x 78..614) is columns 5 to 38; the y of each line on
the left. `#` marks focus, `( )` a round picture, `[ ]` a glyph, `.` dim text.

### 13.1 The tab, out of a Call, focus on the first chat

```
 y      0    5    10        20        30   38  42        52        62
   33      .--------------------------------.
           |  [Xbox]  [Disc]• (pic)          |
  133      |           =====                 |
  161      |  Parties & chats                |    [X] Hold for power options
  213      |  Chats                          |    [=] More options
  260      | #(A)#  (B)  (C)  (G)  (E)  (+20)|
  344      |  Ana  . 3 unread                |
  393      |  ------------------------------ |
  420      |  .------.  .------.  .------.   |
           |  | (sv) |  | (Bo) |  | (sv) |   |
           |  | Late |  |  Bo  |  | Raid |   |
  580      |  '------'  '------'  '------'   |
  772      |  ------------------------------ |
  800      |  [#]  Servers                   |
 1045      '--------------------------------'
```

### 13.2 A conversation (a Flyout over the dimmed panel)

```
 y      0    5    10        20        30        40   44
   33      .----.--------------------------------.
           |////|  (A) Ana                        |
  133      |////|  ------------------------------ |
           |////|  ---------- Yesterday --------- |
           |////|  .----------------------.       |
           |////|  | are you on tonight?  |       |
           |////|  '----------------------'       |
           |////|  .21:40                         |
           |////|          .--------------------. |
           |////|          | after food, 20 min | |
           |////|          '--------------------' |
           |////|                         .21:42 |
           |////|  ======================== New = |
           |////| #.----------------------.#     |
           |////| #| [img] Image          |#     |
           |////| #| look at this          |#     |
           |////| #'----------------------'#     |
           |////|  (👍 2) (😂 1)                  |
  940      |////|  .09:38                         |
  986      |////| ( Message Ana                 ) |
 1080      '----'--------------------------------'
```

### 13.3 Writing a reply (text entry over the Guide)

```
 y      0    5    10        20        30   35        45        55        65        75        85   90
   33      .----.--------------------------------.
           |////|  (A) Ana                        |
  133      |////|  ------------------------------ |
           |////|  .----------------------.       |
           |////|  | look at this          |       |
           |////|  '----------------------'       |
  556      |////|  .09:38                         |
  570      |////|                 Reply to Ana
  620      |////|              ( nice, where is that|                                         )
  710      |////|              [?123][1][2][3][4][5][6][7][8][9][0][<X ]
           |////|              [ <  ][q][w][e][r][t][y][u][i][o][p][ > ]
           |////|              [    ][a][s][d][f][g][h][j][k][l][']    ]
           |////|              [Shft][z][x][c][v][b][n][m][,][.][?][Entr]
  970      |////|              [    ][        space         ][    ]
 1080      '----'--------------------------------'
```

(Text entry's field and keyboard are x 496..1424, columns 31 to 89; drawn here narrower to fit. The dim of text
entry lies over all of it.)

---

## 14. What could not be sourced

- **The conversation view** (bubbles, colours, sides, times, the reply box, the keyboard's place) and **the message
  Toast's look**: no Microsoft text or picture found; 5 and 7.1 rest on recollection and the Guide's own parts.
- **Reactions, GIFs, Mark as read** on the console: none found; the reaction set is this spec's.
- **2023 to 2026 changes to the tab's look**: none published; the pictures used are of 2019 and 2020.
- **Measures**: only the May 2020 picture is at 1920 x 1080; the rest are crops or half size. Every console measure
  here is (est.).
- **What Discord's page gives** (a last message or a time per DM, read marks when hidden, Do Not Disturb's effect,
  what its local storage keeps): for #197, #198 and #201 to establish; the spec says what shows when it does not.
- Discord's self-bot article refused an automated fetch (discord-bridge.md 4); its wording is from its indexed copies.
