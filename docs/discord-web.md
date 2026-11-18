# Discord messages: the web session (#197)

The Dashboard's own discord.com, in a WebView2, signed in by the user on Discord's own page: what the Guide's Party
will read DMs and notifications through (#198 to #202). Voice stays on the RPC connection (docs/discord.md), which
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
