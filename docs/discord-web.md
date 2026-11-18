# Discord messages: the web session (#197)

The Dashboard's own discord.com, in a WebView2, signed in by the user on Discord's own page: what the Guide's Party
will read DMs and notifications through (#198 to #202; reading: below). Voice stays on the RPC connection (docs/discord.md), which
this does not touch. The decisions are #192's (D1 opt-in, D2 our own web session, D6 messages in memory only, D7 our
own code); what the spike found on the TV PC is docs/research/discord-web-spike.md.

Code: `crates/discord_web` (the session and its host thread), `crates/model/src/discord_web.rs` (what the screens see),
`crates/screens/src/settings/discord_web.rs` (Settings > Account > Discord messages, docs/spec/settings.md 16.12),
`crates/screens/src/discord_sign_in.rs` (the sign-in page), `crates/standin/src/discord_web.rs` (the stand-in
discord.com), `e2e/payloads/discord-web` (the payload).

## When anything runs

Nothing, until Settings > Account > Discord messages' switch is on (`discord.messages`, off by default), beside the
terms said plainly (below). On:

| Kept session for this Profile | Sign-in page | What runs |
|---|---|---|
| no | closed | nothing: the status is Not signed in, or no runtime when the loader or WebView2 is missing |
| no | up | the host thread and its web view, on the sign-in page |
| yes | either | the host thread, its web view hidden, on discord.com/app: signed in or not is read from the page |

The host thread is the web view's own (WebView2's rule: one thread makes the window and makes every call). Its window
is never activated and never shown but for the sign-in page, so the Dashboard keeps the foreground and the pad. The
Dashboard's thread sends it commands over a channel and wakes it with a thread message; it answers with how the
session stands and wakes the Dashboard's event loop. As the spike measured it, a hidden discord.com costs about
400 MB and under 1% of a core at rest: the price of turning it on.

## A failure rests

The first build of this (#291) froze the TV PC. Its host thread never initialised COM, so every web view failed to
open. When a failed host thread ended, the session started another at once, because the sign-in page was still up.
That made about 130 attempts a second, and each left its window, thread and loader behind until Windows ran out.
Now:

- The host thread calls `CoInitializeEx(COINIT_APARTMENTTHREADED)` before making the environment, and
  `CoUninitialize` after. Its window is destroyed on a failed open, and its loader (`FreeLibrary`) let go, before
  its last report.
- **One host thread at most**, joined before another may start.
- **Failed is a resting state.** Only the user starts another attempt: the switch turned on, or Sign in. The
  session never retries on its own, from a frame, a report or a host ending. Only a host ended on purpose (another
  Profile, a sign-out) is followed by a new one.
- **A cap**: after `MOST_FAILURES` (3) failed opens in a run, nothing more is tried until the Dashboard starts again.
  The log says so once.
- The runtime check (no runtime) is asked once a run, and again only when the switch is turned on.
- The log has one `discord web: opening the web view (attempt N)` line an attempt. The payload kills the Dashboard,
  and the web views of its own data folder, if more than 5 attempts show.

## The states

`model::discord_web::WebState`, shown on Settings' status line and logged as `discord web: <word>`:

- **Off**: the switch is off.
- **No runtime**: `WebView2Loader.dll` is not beside `dashboard.exe`, or it finds no WebView2 runtime
  (`GetAvailableCoreWebView2BrowserVersionString`). Asked without starting anything.
- **Checking**: a kept session's web view is up and the page has not said yet.
- **Signed out** / **Signing in** (the sign-in page is up) / **Signed in**: read from the page by `js/state.js`, as the
  spike found them: signed in is no login or register route, no password field, and the server bar
  (`[data-list-id="guildsnav"]`) shown; signed out is the login or register route, or a password field without it.
- **Failed**: the web view could not be opened or its window went; the log says why.

## The session: one per Profile, in the data folder

Decided: each Profile has its own session, the web view's whole user data folder at
`%LOCALAPPDATA%\XboxSeriesDashboard\discord-web\profile-<id>` (`discord_web::folder`). A Profile signs in to its own
person's Discord, and another Profile's sign-in never shows it. That is what the account-switching research expects:
a Profile is bound to its own person's accounts (docs/research/account-switching.md, Summary, 1), and binding
Discord's own client to a Profile is out of its scope (Decisions, 6), so the Dashboard's own session is the one kept
per Profile. Switching Profile ends
one host thread and starts the other's; nothing is copied between them. The folder is WebView2's own, as Edge keeps a
profile; our code never opens a file in it.

**Sign out** closes the web view, waits for its browser processes to let go, and removes the folder: nothing of it is
left (`discord web: signed out: nothing is left, ... is gone`). A sign-in page closed before signing in removes what
it made the same way. Discord's own record of the session (User Settings > Devices) stays until it is logged out
there, which Settings says.

## Nothing secret is read, logged or kept

- The page is asked for flags only (`js/state.js`): the route's kind, whether the app's shell or a password field is
  there, and the kind of field focus is on (text or password). Never a value, a name, the page's text or a cookie.
- Text the Console keyboard keeps goes to the focused field by DevTools' `Input.insertText` and is dropped. It is in no
  log line, no focus line (`WebAsk`'s `Debug` says `Text(7 characters)`), nothing saved.
- The log has addresses as scheme, host and path only, ids and long segments hidden (`redact::address`).
- WebView2's own password saving and autofill are off; DevTools, the context menu, new windows and permission
  requests are refused.

## The sign-in page, and which keyboard

Discord's own discord.com/login, over the whole screen but a band 110 high at its foot that is the Dashboard's
(`discord_sign_in::BAND`): its first line says to scan the QR code, the second what the pad does.

### The QR code first

Scanning Discord's QR code with the Discord mobile app is the first way to sign in; typing is the second.

- The page opens with **no field focused and no keyboard up**. Discord focuses the e-mail field by itself; until the
  pad sends a key, the session lets go of any field focused (`blur()`, looked at four times a second), and the first
  Down puts focus in the form's first field, where Tab would have begun.
- The page is **zoomed, never cropped or restyled** (WebView2's `ZoomFactor`, kept across navigations, set back to 1
  when the page is put away): as far as leaves the page 860 x 440 CSS pixels at least (`discord_web::VIEW_MIN`,
  `zoom`). Discord drops its QR panel from a narrow login page, so the width is kept above that; within it the QR
  code is as large as it goes: 2.2 times on a 1080-line screen at any DPI scale, so Discord's QR panel (176 CSS pixels
  across, as the panel is laid out) is about 390 pixels at the 1080 scale. The breakpoint and the panel's size are
  Discord's as seen in its stylesheet and UNVERIFIED against today's discord.com; the log line `discord web: page zoom
  2.20 at scale 2.00, 873x441 CSS pixels` and `discord web: QR code shown yes` say what the TV got.
- The band's first line, in Discord's words where it has them: "Scan the QR code with the Discord mobile app to log
  in instantly. To type instead, move to a field and press A."
- The QR code is Discord's: it expires and refreshes itself, and nothing of ours reads it or the page's token. After
  the scan the phone asks to confirm; then Discord's page goes to the app by itself, which the session's usual look at
  the page (`app`: the server bar, no login route) takes as signed in: no input on our side.
- The log says only whether the code is shown, each time that changes while the page is up: `discord web: QR code
  shown yes` or `no` (the `qr` flag of `js/state.js`: a QR panel laid out with a size).

### Typing

The pad drives the page by DevTools key events, which reach it
whether or not its window has the foreground (the spike's lesson, with focus emulation on): Up and Down are
Shift+Tab and Tab, A presses Enter (the full rawKeyDown, char, keyUp sequence a form needs), X is Space, View is
Escape, B closes the page.

**The Dashboard's own Console keyboard is given to the page**, not the spike's pad keyboard: A on a text field puts
the web view away and opens text entry (docs/spec/settings.md 9.6) over the sign-in page, as dots for a password
field; what it keeps goes into the field. So typing on the sign-in page is the same as everywhere else in the
Dashboard.

## The loader: WebView2Loader.dll beside dashboard.exe

Decided: **shipped beside `dashboard.exe`**, not linked statically, and loaded at run time.

- webview2-com-sys 0.39.1 (`build.rs`) puts Microsoft's three loader files in its build output: `WebView2Loader.dll`,
  its import library, and `WebView2LoaderStatic.lib`. The static library is an MSVC build that links against the
  MSVC C runtime; this workspace builds for `x86_64-pc-windows-gnullvm`, whose runtime is not MSVC's. Microsoft
  documents both ways, "statically linking `WebView2Loader.lib` into the app binaries, or by including the
  `WebView2Loader.dll` that matches the app's architecture" (Microsoft Learn, *Distribute your app and the WebView2
  Runtime*, "Files to ship with the app"), and the static one only through Visual Studio's project files
  (`<WebView2LoaderPreference>Static</WebView2LoaderPreference>` in a `.vcxproj`; *Distribute a WebView2 app as a
  single executable file*). So the DLL.
- Loaded with `LoadLibraryW` from the exe's own folder, never linked: `dashboard.exe` starts, and Settings says no
  runtime, where the DLL or the runtime is missing.
- CI copies it from webview2-com-sys's build next to the release exes (`.github/workflows/ci.yml`, "WebView2Loader.dll
  beside dashboard.exe"); `tools/package.sh` takes it from beside the exe into the zip and the installer, and the
  installer removes it on uninstall (docs/release.md). It is never committed. The runtime itself is Windows 11's own
  Evergreen WebView2: nothing of it is shipped.

## Reading: Discord's own stores, pushed (#198)

**Decided by the user on 2026-11-18, over the text of #198:** the Direct Messages are read from Discord's in-page Flux
stores only, with no DOM selector and no DOM fallback. The voice spike (#292) found that this works on the real
discord.com (docs/research/discord-web-voice.md, 0 and 4b); selectors would have been the brittle part.

Code: `crates/discord_web/js/stores.js` (the search), `js/direct.js` (the reader), `src/direct.rs` (their JSON to the
model, and the events), `src/pictures.rs` (avatars), `src/win/host.rs` (`reading`, `page`), `Session::direct`,
`ask_direct`, `take_events`. Tests: `tests/scripts.rs` and `tests/js/run.js` (node, against the made-up Discord of
`crates/standin/js/fake_discord.js`), the Rust unit tests, the stand-in (below).

### The script

`discord_web::direct_script()` is `stores.js`, `direct.js` and `act.js` (see "Acting") in one function. The host installs it with
`AddScriptToExecuteOnDocumentCreated`, so it runs in every new document of the web view before Discord's own scripts
(not in frames: `window.top` only), and twice is once (`window.__xsdDirect`). It:

1. **Waits for webpack to settle**: the module cache of every runtime stops growing for two looks half a second apart,
   capped at 15 s.
2. **Finds the stores** (the spike's search, ported): Discord's chunk global `webpackChunkdiscord_app` takes one probe
   chunk, which several runtimes answer with their `require`; every loaded module's exports and their own properties are
   candidates; a store is found by its Flux name (`getName()` or `constructor.displayName`), else by its methods, and
   Flux's registry (`getAll()` on a store's class) adds the rest. No factory is ever run.
3. **Retries**: up to 5 times two seconds apart, then reports; then every 30 s for as long as the page lives (it may
   become the app later). On `/login` and `/register` it stays quiet.
4. **Subscribes** to `addChangeListener` on each store, **coalesces** (at most one snapshot a 250 ms window,
   trailing) and **pushes** JSON with `window.chrome.webview.postMessage`. The host takes it in
   `add_WebMessageReceived` (`WebMessageAsJson`, only from the page's own address) as `Report::Page`.
5. Sends **only what changed**: each kind is kept as the last JSON sent and not sent again if equal. A new message
   resends the DM list (its last line and unread moved) but not the conversation of another DM, and a conversation
   change does not resend the list. The whole list is the unit, not a diff: 169 DMs is about 40 KB, at most four a
   second, and a diff would be more to get wrong.

Read-only: it calls store getters, nothing else, except the page's route (`history.pushState` and a `popstate`
event, which is how the first spike made Discord open a conversation, where a click did not) and Discord's own
older-messages action on the host's ask. It never touches a token, anything the browser stores, a cookie, a password, a
request header, and it makes no request: a test fails the build if either file holds the words for them or for a
network call.

### What it reads

| Our key | Discord's store (by Flux name, then by shape) | Methods called | Required |
|---|---|---|---|
| `channels` | `ChannelStore` | `getChannel` | yes |
| `privateSort` | `PrivateChannelSortStore` (or `ChannelListStore`) | `getPrivateChannelIds` or `getSortedPrivateChannels` | yes |
| `users` | `UserStore` | `getUser`, `getCurrentUser` | yes |
| `readState` | `ReadStateStore` | `getUnreadCount`; optional `getMentionCount`, `hasUnread`, `getOldestUnreadMessageId`, `ackMessageId` | yes |
| `messages` | `MessageStore` | `getMessages` (its `toArray()` or `_array`, `ready`, `hasMoreBefore`); optional `getMessage`, `getLastMessage` | yes |
| `calls` | `CallStore` | `getCall` or `getCalls` (`ringing`) | no: no rings without it |
| `older` | an exports object with `fetchMessages` (found by shape) | `fetchMessages({channelId, before, limit: 50})` | no: `Older` does nothing |
| `reply`, `react`, `ack`, `forget` | exports objects found by shape: `createPendingReply`; `addReaction` + `removeReaction`; `ack`; `deleteMessage` | see "Acting" | no: the ask each serves fails plainly |

A required key or method missing, or a required method that throws, is the **health** `ok: false`: the session's
`DirectState::PageChanged`, and the log says which, once (`discord web: store "readState" no longer answers
(getUnreadCount)`, or `discord web: the page's scripts: no chunk global` when no runtime answered). Nothing is read
until all required ones are there. The optional ones failing is not a break.

### The messages

Each is `{"k": ...}`; every id is a string.

- `{"k":"health","ok":bool,"found":{<our key>:[<Discord's store name>, <methods found>...]},"missing":["readState.getUnreadCount",...],"older":bool,"tries":n}`
- `{"k":"dms","dms":[{"id","name","group","members","pic","unread","mentions","last":{"at","text"?,"who","names"}|null}]}`
  in Discord's order. `pic` is a CDN path (`avatars/<user>/<hash>.png?size=128`, `channel-icons/<channel>/<hash>.png?size=128`),
  empty where there is no hash (the default avatar: no picture). `last.text` is there only where `MessageStore` holds
  the channel's last message (it holds only what was loaded); `last.at` is its time, or the time of the channel's
  `lastMessageId` snowflake, `(id >> 22) + 1420070400000`. `names` resolves the mentions in the text.
- `{"k":"conv","id","loaded","more","older","unread_from","msgs":[...]}`, the conversation the host opened.
  `more` is `hasMoreBefore`; `older` is Discord's fetch running; `unread_from` is the first message that was unread
  when it was opened, fixed from `getOldestUnreadMessageId` (else the message after `ackMessageId`) **before** the
  route changes, since the page marks it read on opening. A message: `{"id","author":{"id","name","pic"},"own","at",
  "edited","type","text","names","state","reply":{"s":"to|deleted|none","who","tx","names"}|null,"mentioned":[names],
  "files":[{"ct","name","voice"}],"stickers":[names],"embeds":[{"type","title","site"}],"poll":question|null,
  "reactions":[{"id","name","count","me"}]}`.
- `{"k":"rings","rings":[{"channel","caller","pic"}]}`: DM calls whose `ringing` holds the user's id.
- `{"k":"reset"}`: the user is gone from the page (signed out there).

`src/direct.rs` maps them: text to spans (`<@id>` to `@name` by the names the page resolved, links, `<:name:id>` to a
server emoji); attachments by content type (all `image/*` but GIF together as `Images(n)`, `image/gif` a GIF, `video/*`
a video, a voice-message flag a voice message, else a file by name); a `gifv` embed a GIF, an embed by its title, else
its site; stickers, polls; types 0, 19, 20 and 23 are bubbles, 1 (added), 3 (call) and 6 (pin) are Discord's own
lines, any other type a line with its text; a reply as the other's author and first line, or "deleted" (a reply to a
message not loaded shows no line); the user's own messages by `state` (`SENDING`, `SEND_FAILED`).

### Opening, closing, older

`DirectAsk::Open(id)` (digits only: anything else is dropped, and the host puts nothing but digits in a script) calls
`__xsdDirect.open(id)`: the unread line is fixed, then the page is routed to `/channels/@me/<id>` by `pushState` and
`popstate`, Discord loads the channel and `MessageStore` fills, and the change listener pushes the conversation. The
session shows it (`Direct::open`) once its messages have come, and takes no other. `Close` forgets the conversation in
the session and routes the page to `/channels/@me`. `Older` calls Discord's own `fetchMessages` (the page has no button
to press; scrolling a hidden page is brittle), `before` the oldest loaded message, and `loading_older` follows until
the call settles; `at_start` is `hasMoreBefore === false`. If that action was not found, `Older` does nothing and the
log says so once (`discord web: older messages: Discord's action was not found; reading goes on without it`).

Send, Forget, React and MarkRead are acted (below, "Acting"); Call, Answer and Decline are the voice ticket's
(#297): `Session::ask_direct` logs `discord web: <kind> not built yet` once each and does nothing.

### The state, events, memory

`Direct::state` follows `WebState`: Off, NoRuntime, Checking is Loading, SignedOut and SigningIn are SignedOut, Failed
is Unreachable, SignedIn is Loading until the first list and then Ready, or PageChanged when the health says so. When
the session turns SignedIn the page is asked to send everything again (`resend`), so nothing it pushed before the
page was known to be signed in is lost.

**Everything is in memory.** Turning the switch off, signing out, the page signing out (`reset`), a failure and the
sign-in page opening forget every DM, message and picture. Nothing is written to a file; no name, id, text or address is
logged (tests capture the log of a whole made-up session and look). The log lines are counts, our keys and Discord's
method names: `discord web: stores found: channels, privateSort, ...`, `discord web: 25 DMs, 5 unread in 2`,
`discord web: conversation open, 50 messages`, `discord web: 3 pictures could not be fetched (HTTP 404)`.

**Events** (`Session::take_events`), found by comparing successive lists: a DM whose unread count rises while it is not
the open conversation is a `Message` (its first line and, in a Group DM, the sender and the group, where the store
holds the message; else an empty line, which the screens say as "sent you a message"); a call ringing the user that
was not before is a `Ring`, one that is gone a `RingEnded`. Nothing is raised for what was unread when the session
began (the first list is the baseline). **`Mention`** (a mention in a server's channel) is not raised yet: the host takes
Discord's web notifications (`NotificationReceived`, `ICoreWebView2_24`, marked handled so Windows shows nothing; the
notifications permission is granted for Discord's own address only, every other permission refused), and logs the
first one's lengths, but what Discord puts in a notification's title, body and tag has not been seen, so none is
guessed at: `Session::notification_event` is the hook, and returns nothing.

**Pictures** (docs/spec/party-chats.md, 9): fetched on three threads of their own from the CDN
(`discord_web::CDN`, or `Session::set_cdn`, the Dashboard's `--discord-cdn-url`; with the session pointed at a
stand-in Discord and no CDN given, none), by the path the page gave and only if it is one of the CDN's two shapes, into
memory, once each, the first 40 DMs' first, the open conversation's authors and a ringing caller's when they come.
Never logged by address.

### What breaks it

Discord renaming or reshaping a store or a method, or the chunk global (`webpackChunkdiscord_app`): the log names the
key and method, the Guide says "Discord has changed its page", and the names above are what to update. What the
fake Discord cannot show, and the TV PC must: the real shapes of `referencedMessage`, of a message's `timestamp`,
whether `getOldestUnreadMessageId` and `ackMessageId` are getters, whether `MessageStore` holds a last message of a
DM nobody opened (the list then has times only), what `CallStore`'s calls hold, whether `fetchMessages` takes that
argument, and whether the hidden page marks a conversation read on opening.

### The stand-in

`standin serve-discord-web`'s app page carries the same fake webpack chunk global and stores (five DMs, one Group DM,
unread counts, a conversation of varied messages in the first), and controls for a test (and, for acting, the ones in "Acting"): `POST
/standin/dm-message?channel=<id>&text=<words>`, `POST /standin/unread?channel=<id>&count=<n>`, `POST
/standin/ring?channel=<id>&on=1|0`; plain pictures at `/avatars/..` and `/channel-icons/..`, so `--discord-cdn-url`
may name the same address. The channels are 100000000000000001 to 03 (the list's links), 100000000000000010 and
100000000000000011. Its page may call the network (to poll the controls); only our injected scripts may not.

## Acting: send, reply, react, mark read (#201)

Code: `crates/discord_web/js/act.js` (the page's half, the last file of the page script), `src/act.rs` (the queue),
`src/win/host.rs` (`act`, `type_message`, `enter`), `Session::ask_direct`. Tests: `tests/js/run.js` (the page's half
in node against the fake Discord), `src/act.rs` and `src/direct_tests.rs` (the queue and the session), the stand-in.

**The rule.** Reading is from Discord's Flux stores only. Acting goes **by id**: the route, then **the page's own control
by trusted input** (DevTools' `Input.*`); one of **Discord's own functions** only where the page has no control that can
be reached without hovering (a reply, a reaction, marking read). It never calls Discord's web interface itself and
reads nothing that identifies the session; `tests/scripts.rs` fails the build if `act.js` holds the words for one.
Each ask is **done only when the stores show it**.

### One at a time, paced

`act::Acts` is a queue. At most one action runs; the next starts no sooner than **0.7 s** after the last ended; each
has **15 s** to finish (done, failed, or `no answer in 15 s`, when the page is told to let go: `cancel()`). Several
sends queued are each reported Sending. Switching the session off, signing out, the page's user going and a failure
**empty the queue**: each send that had not gone is `NotSent` (`discord web: not sent: the session ended`) and what
was running is cancelled. The queue is driven by `Session::pump`, each frame. A send while the session is not up
(off, signed out, the stores not read) is not sent at once (`discord web: not sent: not signed in`).
**Forget** is local and not queued.

### Send (`DirectAsk::Send { channel, text, reply, nonce }`)

1. The page is routed to `/channels/@me/<channel>` (`history.pushState` + `popstate`, as `open` does) if it is not, and
   the script waits (8 s) for the channel's messages (`MessageStore` `ready`) and **the message box**.
2. **The one DOM lookup**: the box, `[role="textbox"][contenteditable="true"]` inside the chat form (else anywhere), as
   the spike found it. It is focused by a script `.focus()` (focus emulation is on: `Emulation.setFocusEmulationEnabled`,
   the hidden page believes it has focus) and must be the `document.activeElement`; else `not sent: composer not focused`.
   The health record's `composer` key is yes once a lookup found it (the log's `stores found` line then lists it) and
   no when one did not (`not sent: composer not found`); never a break.
3. A **reply** (`reply: Some(message id)`): Discord's own `createPendingReply({ channel, message, shouldMention: true })`
   with the `ChannelStore` channel object and the `MessageStore` message object, from the exports object that has that
   function (health key `reply`). The page's Reply button needs a hover, so this is the one place a function is used.
   Not found, or the message not loaded: nothing is sent, `not sent: reply action not found` / `reply target not found`.
   A plain send first clears a reply the Dashboard did not ask for: `deletePendingReply(channel)` (found by shape, only
   if the `PendingReplyStore` says one is set, or called blind where that store is not found).
4. The page says `typing` (and whether the box holds a **draft**, which Discord keeps). The host then types, as the spike
   did on the TV: a draft is cleared (Ctrl+A, Backspace), the text is inserted with `Input.insertText` one line at a
   time with **Shift+Enter** between lines (a plain Enter sends; the Console keyboard makes single lines), and **400 ms
   later one Enter**: `rawKeyDown`, `char "\r"`, `keyUp`.
5. The page watches the store for a message that is **new since the box was focused, the user's own, with the text typed
   (trimmed)**. The first is Discord's own optimistic message (`state: "SENDING"`, an id of its own): reported as
   `sending`. When it is `SENT` (the store replaces it by the real one under another id), it reports `sent` with the real
   id. `SEND_FAILED` is `failed`.
6. **Failure**: `SEND_FAILED`, or nothing in 15 s: `Failed`. A **captcha** is detected only by a captcha frame appearing
   in the page after the send began (`iframe[src*="captcha"]`, `hcaptcha`, `recaptcha`): `DirectEvent::Captcha`. A
   **refusal** whose reason Discord's own dialog (`[role="dialog"]`) shows after `SEND_FAILED`: `DirectEvent::Refused`
   with that text (`Private`; the log says `rate limited` if it says "slow down", "rate limit", "too fast" or "too
   many", else `refused`). Without a reason it is just `NotSent` (`not sent: refused`). The log: `discord web: sent in
   769 ms`, `discord web: not sent: captcha | rate limited | refused | no answer in 15 s | reply action not found | ...`.

**What the screens get.** While a send is queued or in flight, `Direct::open` carries it as a message with
`Delivery::Sending`, **id = the nonce as a decimal string**, own, author id empty, the text as one `Text` span, `at` the
time it was asked: made by the session at once, and replaced by **Discord's own optimistic message** (the first new own
`SENDING`/`SENT` message with equal text) as soon as the page has tied the send to it; that message is reported under
the nonce id, never alongside. `DirectEvent::Sent { channel, nonce, message }` follows once the store shows it sent:
from then the message is Discord's, under its real id (the conversation snapshot with it is always processed first, so
the screens never see the real message after the event: **re-key the bubble from the nonce to `message` on the event**).
`DirectEvent::NotSent { channel, nonce }` follows any failure (after `Captcha` or `Refused`, when those apply): the
message stays in `Direct::open` under the nonce as `Delivery::Failed` (the store's own failed message where the page
holds it, else made from what was sent) until **`DirectAsk::Forget { channel, message: nonce id }`**: the session drops it
and asks the page to remove Discord's own failed message from its store locally (`deleteMessage(channel, id, true)`,
found by shape, health key `forget`; if it is not found only the session's view drops it). Retry is the screens': a
Forget and a new Send. A message's `state` is mapped for every own message: `SENDING` Sending, `SEND_FAILED` Failed,
anything else Sent.

### React (`DirectAsk::React { channel, message, emoji, on }`)

Discord's own reaction action, from the exports object with both `addReaction` and `removeReaction` (health key
`react`), for the message in the open conversation (it must be loaded). Called with
`{ id: null, name: "<emoji>", animated: false }`. **The argument shape is checked by the function's `length`**: positional
`(channelId, messageId, emoji)` (the builds seen), or one object `{ channelId, messageId, emoji }` where `length` is 1.
Done when the store's message shows the reaction with `me` true (or no such reaction with `me`, for `on: false`); if it
already does, it is done without asking Discord. Not shown within 10 s: `reaction not done: not shown`.

### MarkRead (`DirectAsk::MarkRead(channel)`)

Discord's own `ack(channelId)` from the exports object with a function `ack` (preferring one that also has `bulkAck`,
`ackCategory`, `localAck` or `ackChannel`, since `ack` alone is on more than one object; health key `ack`). Done when
`ReadStateStore`'s unread count for the channel is 0 (10 s). **A conversation opened by `DirectAsk::Open`** is expected to
be marked read by the page itself (it believes it is visible and focused): when it had unread messages the page waits 3 s
for the count to reach 0 and the log says once per open `discord web: conversation open, read by the page yes|no`; on
`no` it calls the same `ack` once (the user opening a conversation in the Guide has read it). Nothing is logged for a
conversation that had nothing unread.

### Required and optional, and what breaks it

All the actions' keys are **optional**: a missing one never makes `PageChanged`; the ask it serves fails with one log
line and nothing is sent. The health record's `found` lists `reply`, `react`, `ack`, `forget` (Discord's names, by
shape) and `composer` (yes once seen); the `stores found` log line lists those found.

| Depends on | Breaks when | Shows as |
|---|---|---|
| the message box (`[role="textbox"][contenteditable="true"]`, in a form) | Discord renames its role or makes the box non-editable by that shape | `not sent: composer not found` / `composer not focused` |
| `Input.insertText`, the Enter sequence, focus emulation | the page ignores input | `no answer in 15 s` |
| the route `/channels/@me/<id>` and `MessageStore.getMessages(id).ready` | the router changes | `composer not found` |
| `createPendingReply`, `deletePendingReply`, `PendingReplyStore.getPendingReply` (shape) | renamed or re-shaped | `not sent: reply action not found` |
| `addReaction` / `removeReaction` (shape, argument form by `length`) | renamed, a third argument form | `reaction not done: ...` |
| `ack` (shape) | renamed; another object with `ack` found first | `mark read not done: ...` |
| `deleteMessage(channel, id, true)` (shape) | renamed | a failed message only leaves the session's view |
| the message's `state`, `author.id`, `content` in `MessageStore` | reshaped | a send never ties: `no answer in 15 s` |

### The terms

This is the part Discord's terms are about: **it is the user's account sending, reacting and reading through a program
that is not Discord's own client**. Discord's Terms of Service and its self-bot article do not allow other programs to
use an account, and Discord says it can close an account for it (see "The terms, said plainly"). Acting by trusted input
into the page's own box and by Discord's own functions, one at a time and paced at 0.7 s, keeps it to what a person at
the keyboard would do; it is not a way round the terms, and the switch's line under it says so.

### The stand-in

The fake Discord changes as the real one does: a message box while a conversation is the route; Enter adds Discord's
own optimistic message (`SENDING`, an id of its own) to the fake `MessageStore` and ~300 ms later makes it `SENT` under
a real id; fake `createPendingReply`/`deletePendingReply`, `addReaction`/`removeReaction`, `ack` (and a decoy with only
`ack`), `deleteMessage`, exported by the same shapes. Controls: `POST /standin/send-fails?on=1|0[&reason=<words>]`,
`POST /standin/captcha?on=1|0`, `POST /standin/slow?ms=<n>`; `GET /standin/sent` lists what it received (`message` with
channel, text and reply target; `react`; `read`), for an end-to-end payload.

## The terms, said plainly

The line Settings keeps under the switch (docs/spec/settings.md 16.11, the kept lines), from Discord's Terms of
Service and its self-bot article as docs/research/discord-bridge.md (4) quotes them: "Discord's terms don't allow
other programs to use your account, and Discord says it can close an account for it. With this on, the Dashboard
signs in to discord.com as you and reads the page for you."

## Tests

- Unit: `crates/discord_web` (the folder per Profile, nothing made while off, sign-out leaves nothing, another
  Profile's folder untouched), the model (`WebAsk` never shows text), the screens (Settings' state line; text entry's
  text goes to the session and nowhere else), the stand-in (a filled form signs in; the app page has Discord's shape;
  the phone's scan signs the page in, once), the zoom (`VIEW_MIN` kept at every scale).
- Screenshots: Settings > Account with the new box, Discord messages off and signed in, the sign-in page's band.
- `e2e/payloads/discord-web` on the TV PC, against `standin serve-discord-web`, whose login page has a made-up QR
  block (hidden under 830 CSS pixels, as Discord's) and an e-mail field that focuses itself: the switch on; **signed
  in by the QR code with no input at all** (the page opens with the QR shown and no field focused, the test plays the
  phone with `POST /standin/qr-confirm`); closed and started again still signed in; signed out with nothing left;
  then signed in again by typing on the stand-in's form with the pad and the Console keyboard, and signed out again.
