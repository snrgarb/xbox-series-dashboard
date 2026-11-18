# Discord voice in the web session: the spike, on the TV PC

Spike for [#292](https://github.com/snrgarb/xbox-series-dashboard/issues/292), milestone M9 Full Discord. The user,
2026-11-18: "Ideally the entire integration should be 1 thing, not split". If voice works in the web session of
[#197](https://github.com/snrgarb/xbox-series-dashboard/issues/197), hidden, with a game in front, D1 of
[#192](https://github.com/snrgarb/xbox-series-dashboard/issues/192) (RPC kept for voice) is overturned and the
Discord app and the RPC route (`crates/discord`, [`docs/discord.md`](../discord.md)) go.

It builds on the first spike, [`discord-web-spike.md`](discord-web-spike.md) (#196): hidden hosting works; a
conversation opens by `history.pushState` + `popstate`, not a click; input needs
`Emulation.setFocusEmulationEnabled` while hidden; trusted input goes through DevTools `Input.*`; signed in is the
app's shell (the server bar), not a route. How the Discord Bridge's Web mode does voice is in
[`discord-bridge.md`](discord-bridge.md), sections 2.3 and 2.5: it grants the microphone in `PermissionRequested`,
presses Discord's own buttons, reads `usernameSpeaking_` for who speaks and waits for the "Voice Connected" panel.

**Written on Linux on 18 November 2026.** The "TV result" column of section 5 is for the lead to fill from the logs.

**First TV run, 18 November 2026** (PR #293's first build, `--hide controller`, set `a`): signed in by QR. The probe:
3 inputs and 4 outputs, labels shown; the microphone granted; `getUserMedia` hidden in 43 ms (48 kHz, live, not
muted); `setSinkId` on both; the AudioContext was born running and its clock kept 60.0 s over 60.0 s, 6 of 6
samples. The timer read 0 ticks with a 0 ms gap every time: **the hooks never ran** (DevTools
`Page.addScriptToEvaluateOnNewDocument` answered ok but its script did not run in Discord's page), the spike's
fault, not throttling, so the run said FAIL wrongly. Fixed in the next build: the hooks go in by WebView2's own
`AddScriptToExecuteOnDocumentCreated`, are checked after the app shows (and put in late if missing), and the verdict
says `NEVER STARTED` and `INCOMPLETE` (exit 12) for a timer that never ran. The mic level was 0.0000 throughout
(the default input is a wireless headset that may have been off): the next build says whether the analyser saw any
non-zero sample. Cost during the probe: 585-730 MB private, 3-8% of one core. `--voice-list`: 7 servers (names from
`data-dnd-name` 7), 2 folders, 19 DM rows on the first screen.

**The page believes it is visible.** With focus emulation on, Discord's page reports `visibility visible`,
`document.hidden false`, `focus true` even with the controller told it is not visible (the first spike, without
focus emulation, read `hidden`). That is likely why nothing throttles; it also means the product's session, which
turns focus emulation on (#197), runs as a visible page would.

## 1. What the spike adds

Five steps in `spikes/discord` (`discord-spike.exe`), one per run, each appending to `discord-spike.log` beside the
exe and exiting, as #196's steps do. `--sign-in` is kept as it was (the QR code worked on the TV).

| Step | What it does | Needs the user |
| --- | --- | --- |
| `--voice-probe` | Hidden, signed in: the microphone permission, devices counted, `getUserMedia({audio:true})` hidden, then an AudioContext of the spike's own (a silent tone, the microphone into an analyser) and a 1 s timer watched for `--hold` s; one verdict line. One hiding (`--hide`) and one set of browser switches (`--args-set`) per run | No (a voice near the microphone shows a level, optional) |
| `--voice-list` | Hidden: DM rows and servers counted; with `--server-file`, that server opened by route and its voice channels read. Names to `voice-names.txt`, never the log | No; the user then names the server and channel |
| `--voice-join --server-file f --channel-file f` | Joins that voice channel once by trusted input, waits (20 s at most) for the connected panel, logs the call once a second for `--hold` s, acts on the control files, always leaves | **Yes**: a second person (another account) in the channel, talking; the user talking; a game started full screen |
| `--call --to-file f` | Opens the DM named by route, presses Start Voice Call once, the same as above | **Yes**: the other account answers on its own device |
| `--voice-incoming` | Waits `--hold` s for a call to come in; the `answer` / `decline` files press the ring's buttons; an answered call is held and left like the others | **Yes**: the other account calls the user |

Every step, old and new, now takes Discord's web notifications (`NotificationReceived`): counted, lengths logged,
marked handled so Windows shows nothing.

### How it works, and its rules

- **No retry loops.** Each step acts once. A press tries its ways in order (`--way auto`: DevTools key, then
  DevTools click, then for a voice channel its route and Join Voice button), and a later way only if the one before
  showed **no sign at all** within 5 s (no voice panel, the switch unchanged): never the same press twice, and joining
  a channel already joined does nothing. `--way key|click|route` keeps it to one way. Any failure is logged once and
  the step exits non-zero. Every wait is capped (app 90 s, a way's sign 5 s, connected 20 s, leave 10 s, each script's
  own promises 2-10 s) and the watchdog ends the process `--minutes` + 2 after the start. A hold must fit:
  `--hold` + 240 s within `--minutes` (`--hold 600` needs `--minutes 14`), or the command line is refused.
- **Always leaves.** The end of `--voice-join`, `--call` and `--voice-incoming` (time up, or the `leave` file)
  presses Discord's Disconnect and waits for the voice panel to go. If it cannot see that, the log says
  `LEAVE NOT CONFIRMED` and the step exits 13; closing the web view right after drops the connection in any case
  (Discord's server times the client out), but look in Discord itself.
- **COM** is initialised single-threaded on the web view's own thread, as before and as `crates/discord_web` does.
- **Permissions.** The voice steps grant the microphone and autoplay when asked; every step grants notifications
  (so they reach `NotificationReceived`); the camera and everything else is refused (D5, no video). Each answer is
  `SavesInProfile = false`, so every run's log shows the request again. The same kinds are also allowed ahead for
  `https://discord.com` through `ICoreWebView2Profile4::SetPermissionState`, so the page never waits on a prompt.
- **Watching the audio.** Before Discord's scripts run, `js/voice_hooks.js` (WebView2's
  `AddScriptToExecuteOnDocumentCreated`; checked once the app shows, and put in late if it is missing) keeps a reference to each `RTCPeerConnection`, `AudioContext`,
  microphone track and played media element Discord makes, each made and returned unchanged, and runs a 1 s
  `setInterval`. Once a second `js/voice_sample.js` reads them: the peer connections' `getStats()` (inbound and
  outbound audio bytes, the remote level and energy, the microphone's level), the contexts' state and clock, the
  media elements' state and time, the timer's ticks and largest gap; and the page's own signs (voice panel, its status
  class and words, Disconnect, the mute and deafen switches, who is in the channel or the call's tiles, who is
  speaking, an incoming call's ring).
- **Pressing** is #196's way: the button found by a script and marked `data-xsd-target`, then DevTools `DOM.focus`
  on it and Enter as a keyboard makes it (`rawKeyDown`, `char "\r"`, `keyUp`), or a DevTools mouse click at its
  centre; focus emulation is on for the whole step.
- **Nothing personal in the log**: counts, states, booleans, durations, selector hit counts, and Discord's own status
  words only when they are one of a fixed list (`Voice Connected`, `RTC Connecting`, ...; anything else is logged as
  its length). Names of servers, channels and conversations come from files the lead writes (logged as lengths) or go
  to `voice-names.txt` (`--voice-names`) for the lead to pick from. **`voice-names.txt` stays on the TV or in
  `target/tmp`, never in the repository.**
- **Devices**: Windows' active output and input endpoints are counted, and whether each has a default (console and
  communications roles); the page's own devices are counted with whether their labels show. No names.

## 2. Getting it onto the TV

As in [`discord-web-spike.md`](discord-web-spike.md) section 2: CI's `windows` artifact of this PR's run holds
`discord-spike.exe` and `WebView2Loader.dll`; copy both to `%LOCALAPPDATA%\xsd-discord-spike`, and run each step as
a scheduled task in the signed-in session with that section's `step`, `log` and `done_` helpers. A control file is
made over ssh while a step holds:

```sh
ctl() {  # ctl mute | deafen | leave | answer | decline
  ssh htpc-win "type nul > \"%LOCALAPPDATA%\\xsd-discord-spike\\$1\""
}
```

The files the lead writes (one name, one line, UTF-8, via `scp`): `server.txt` and `channel.txt` (as
`voice-names.txt` shows them), `to.txt` (a DM's name as `dm-names.txt` of `--dms` shows it). Delete them, and
`voice-names.txt`, after.

## 3. The commands, in order

The session of #196 was removed by its sign-out, so the spike signs in again first.

| # | Run | Who | Ends |
| --- | --- | --- | --- |
| 1 | `step '--sign-in'` | **the user** (QR code with the phone) | `ssh htpc-win 'type nul > "%LOCALAPPDATA%\xsd-discord-spike\quit"'` once the log says signed in |
| 2 | `step '--voice-probe'` | nobody (a word near the microphone shows a level) | by itself, about 2 min |
| 3 | `step '--voice-probe --args-set b'` | | |
| 4 | `step '--voice-probe --hide offscreen'`, then `'--voice-probe --hide offscreen --args-set b'` | | |
| 5 | `step '--voice-probe --hide visible-unshown'`, then `'... --args-set b'` | | |
| 6 | `step '--voice-probe --hold 420 --minutes 12'` with the hiding and set that passed with the least (prefer `controller`, `a`) | | about 8 min: past Chromium's 5 min of intensive throttling |
| 7 | `step '--voice-list'`; the user picks a server from `voice-names.txt`; the lead writes `server.txt`; `step '--voice-list --server-file server.txt'`; the user picks a voice channel; the lead writes `channel.txt` | **the user** names them | by itself |
| 8 | `step '--voice-join --server-file server.txt --channel-file channel.txt --hold 120'`; during the hold `ctl mute`, `ctl mute`, `ctl deafen`, `ctl deafen` | **the user** talking; **a second account** in the channel, talking | the hold, then it leaves |
| 9 | the same with `--hold 420 --minutes 12`, **a game started full screen on the TV** after the log says `connected: yes`; both people say whether they heard each other throughout | **the user, a second account, a game** | the hold or `ctl leave` |
| 10 | `step '--call --to-file to.txt --hold 90'` | **the second account answers** on its device | the hold or `ctl leave` |
| 11 | `step '--voice-incoming --hold 180'`; the second account calls the user; when the log says `ring: shown`, `ctl answer`; after a minute `ctl leave` | **the second account calls** | `leave` |
| 12 | `step '--voice-incoming --hold 120'`; a second call; `ctl decline` | **the second account calls** | the hold |
| 13 | `step '--sign-out'`, then the folder removed (as #196) | | |

One Discord account is in one voice connection at a time: joining from the spike moves the user's account out of a
call it is in elsewhere (the Discord app on the TV PC, the phone). The second person must be another account.

## 4. The log lines

Lines of #196's steps are as [`discord-web-spike.md`](discord-web-spike.md) section 3 says, except that
`permission asked and refused: kind n` is now `permission: kind n (<name>) granted/refused, asked by a user gesture
yes/no`.

### Every voice step

| Line | Means |
| --- | --- |
| `voice: hidden as controller/offscreen/visible-unshown, browser switches set a/b` | The combination this run tries. `controller`: the window never shown, the web view told it is hidden. `offscreen`: a window shown at -20000,-20000, never activated, the web view told it is visible. `visible-unshown`: the window never shown, the web view told it IS visible. Set `b` adds `--disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows --autoplay-policy=no-user-gesture-required` |
| `devices (Windows): n active outputs (a default yes, a communications default yes), m active inputs (...)` | Windows' audio endpoints, counted. No input: no microphone to test |
| `environment: extra browser switches: ...` | Set `b` (or `--browser-args`) reached the browser |
| `controller: IsVisible reads yes/no` | What the controller says after it was set |
| `notifications: NotificationReceived taken: ...` / `this runtime has no NotificationReceived` | Whether Discord's web notifications can be taken (WebView2 1.0.2739.15 and later) |
| `permissions: microphone allowed ahead for https://discord.com` | Set on the profile before the page loads (also `notifications`, `autoplay`) |
| `permission: kind 1 (microphone) granted, asked by a user gesture no` | The page asked (getUserMedia); the spike answered. 4 notifications, 9 autoplay, 2 camera (always refused) |
| `voice: the hooks are set to run at the page's start (AddScriptToExecuteOnDocumentCreated)` | Asked for |
| `voice: the hooks are in the page from its start: timer ticks so far n, peer connections p, audio contexts c` | **The hooks ran**: ticks above 0 |
| `voice: the hooks were NOT in the page at its start; put in now: in yes` | They did not: put in late, so only what Discord makes from then on is seen. A hold line with `[HOOKS MISSING ...]` reads audio, contexts and the timer as none |
| `voice: the page says visibility visible, document.hidden false, focus true` | What Discord believes. With focus emulation on, the TV's page said `visible` though hidden |
| `notification: n (title a chars, body b chars, tag t chars, requires interaction yes/no, silent yes/no): handled, Windows shows nothing` | **A Discord web notification** (a message, a call) taken; nothing was shown by Windows |
| `cost at n s: web view k processes, x% of one core over 10 s, private m MB, working set w MB; playing audio (Chromium) yes/no; in front: another program's window, full screen yes` | Every 10 s: **the processor and memory cost**, whether Chromium itself says the page is making sound (`IsDocumentPlayingAudio`), and **whether a full-screen program (the game) is in front** |
| `voice: notifications taken this run: n` | The count at the end |

### `--voice-probe`

| Line | Means |
| --- | --- |
| `probe: devices before getUserMedia: i inputs, o outputs, c cameras; labels shown no; ...` | `enumerateDevices()` before the microphone is allowed (labels hidden until it is) |
| `probe: the microphone permission as the page reads it: granted/prompt/denied` | `navigator.permissions.query` |
| `probe: getUserMedia({audio: true}) hidden: ok after n ms; tracks 1, readyState live, enabled true, muted false, sample rate 48000, channels 1; the browser's echo cancellation true, noise suppression true, auto gain true` | **The microphone opens hidden**, and the browser's own processing on it. `error NotAllowedError`: the permission was not granted; `timeout`: no answer in 10 s |
| `probe: devices after getUserMedia: ...; labels shown yes; a "default" input yes, a "communications" input yes` | Chromium's two inputs that follow Windows' default and communications devices |
| `probe: an output device can be chosen by the page: setSinkId on media elements yes, on AudioContext yes` | Whether Discord's output-device setting can work here |
| `probe: AudioContext made hidden: born suspended/running, resume() ok, now running; ...` | **Whether audio starts without a gesture** (set `b`'s autoplay switch, or the autoplay permission) |
| `probe: resume() again as a user gesture (DevTools userGesture): ok, now running` | Only if it did not: whether a trusted gesture starts it |
| `probe: the input getUserMedia chose: its label n chars, Windows' default input yes/no/unknown` | Which input, without its name (0 chars: no label) |
| `probe: the hooks (and their 1 s timer) in the page: yes` | No: the timer cannot be measured |
| `probe n s: AudioContext running, its clock +10.00 s over 10.00 s; timer (hooks in) ticks 10 (max gap 1003 ms); mic track live muted false, level now 0.0123, analyser 2048 of 2048 samples non-zero (peak 0.0400); page visible focus true` | Every 10 s: **whether audio's clock and timers keep pace hidden**, and whether the analyser gets any signal (all zeros: none at all; small non-zero: an open, quiet input) |
| `probe verdict (hide h, switches s): AudioContext running yes (6 of 6 samples); its clock 60.0 s over 60.0 s (kept pace); timer 60 ticks of 60 expected, max gap 1010 ms (on time); loudest mic level 0.0300, peak sample 0.0800, the analyser saw sound: PASS` | **This combination's answer.** PASS: running at every sample, its clock at least 95% of wall time, at least 90% of the timer's ticks and no gap over 1.5 s. A timer with no tick and no gap is `NEVER STARTED` (the spike's fault: a throttled timer still ticks once a second or once a minute) and the verdict `INCOMPLETE`. Exit 0 PASS, 11 FAIL, 12 INCOMPLETE |
| `probe: the microphone stopped, the AudioContext closed: ok` | Cleaned up |

### `--voice-list`

| Line | Means |
| --- | --- |
| `servers: selectors matched: guildsnav=1 items=n servers=s name_from_dnd=a name_from_aria=b folders=f; DM rows on the first screen d` | The server bar: `[data-list-item-id^="guildsnav___"]` items whose id is a number are servers; names from the item's `data-dnd-name` parent or its `aria-label`. DM rows are the first screen's only (the list is virtualised; `--dms` reads it whole) |
| `server: found (name matched exactly), opened by history.pushState + popstate: its channel list shown yes after n ms` | The server opened by its route, `/channels/<id>` |
| `channels: r rows read over p pages (scroller found yes): voice by label v, stage by label s, name_ class n, users shown in voice channels u` | The channel list read whole; voice channels told by their accessible label "(voice channel)" (an English page) |
| `voice-list: n voice channels in that server (stage channels among them s)` | |
| `voice-list: the names written to <file> (not to this log)` | `voice-names.txt`: `server<TAB>name` lines, and `channel<TAB>name<TAB>people in it<TAB>voice/stage` |

### `--voice-join`, `--call`, `--voice-incoming`

| Line | Means |
| --- | --- |
| `voice-join: a server named in n characters, a channel in m` / `call: a conversation named in n characters` | The lead's files were read |
| `voice-join: the channel found (name matched exactly); already in a call: no` | One voice channel of that name (a stage channel is not tried) |
| `press: channel by key: a 240x34 css px, covered by something else no, aria-checked before (none)` | One press: what was pressed, by which way |
| `voice-join: by key: a voice panel shown yes after n ms (status class connecting)` | The sign that the press worked; `no`: the next way, if `--way auto` |
| `voice-join: route: the channel's Join Voice button shown yes after n ms` | The third way: the channel's own page, its Join Voice button |
| `call: ... its Start Voice Call button shown yes after n ms` | The DM opened by route |
| `press: call by key: its effect seen yes after n ms` | A call view or voice panel appeared |
| `connected: yes n ms after the press: panel yes, status class connected "Voice Connected", Disconnect yes, mute switch mute (aria-checked false, in the user panel), deafen switch deafen (...), peer connections connected 1` | **Joined**: the page's evidence (`rtcConnectionStatusConnected_` class, the status words, the Disconnect button, the switches' accessible labels) beside the peer connection's own state. `no` within 20 s: exit 6, after leaving |
| `hold: n s; control files in <dir>: mute, deafen, leave` | |
| `hold t s: rtc connected "Voice Connected", disconnect yes; participants 2 (channel users 2, page users 3, call tiles 0), speaking 1 (names 1, avatars 0, tiles 0); audio in +3.4 kB over 1 streams, level 0.041, energy +0.0123; out +3.1 kB, mic level 0.020; peer connections 1 (connected 1); mic tracks 1 (enabled 1, muted 0); audio contexts 2 (running 2), clock +1.00 s; media elements 0 (with a stream 0, playing 0), time +0.00 s; timer ticks 1 (max gap 1001 ms); page hidden focus true` | **Once a second.** Participants: `voiceUser_` rows under the channel (a server) or the call's tiles (a DM). Speaking: `usernameSpeaking_`, `avatarSpeaking_`, a tile's `speaking_` class. Audio in/out: bytes received and sent by the peer connections in that second; levels 0 to 1. A remote that is silent sends little or nothing (Opus DTX), so judge flow while someone talks |
| `control: mute file seen (removed)`, `mute: before: mute (aria-checked false, in the user panel)`, `press: mute by key: ...`, `mute: after: unmute (aria-checked true, ...) (changed yes)` | **Mute and deafen** pressed on the lead's file, the switch before and after |
| `ring: shown at n s (accept buttons a, decline buttons d); notifications so far k` | `--voice-incoming`: **an incoming call seen in the page**; the buttons are counted only inside the ring's own element (`incomingCall`/`ringing` classes or an "incoming call" label) so an Accept elsewhere is never pressed |
| `answer: the voice panel shows yes` / `decline: the ring is gone yes` | The lead's file pressed the ring's button |
| `leave: the voice panel is gone after n ms; peer connections open 0, connected 0` | **Left** |
| `LEAVE NOT CONFIRMED: ...` | Could not see it leave: exit 13; look in Discord |
| `incoming: a ring is still shown at the end: left as it is (not declined)` | |

### Exit codes

0 done; 3 not signed in (run `--sign-in`); 4 the server, channel or conversation not found as exactly one; 5 no
way showed a voice panel or call; 6 a panel but never connected in 20 s; 7 a script failed; 8 no web view or the app
did not show; 9 the watchdog; 10 a name file missing or not one line; 11 `--voice-probe`'s verdict FAIL; 12 its verdict INCOMPLETE (the timer never ran); 13 leave not
confirmed.

## 5. The questions of #292

| Question | Step and line | TV result |
| --- | --- | --- |
| Does a hidden web view get the microphone without a prompt (permission granted by our host)? | 2: `permission: kind 1 (microphone) granted`, `getUserMedia ... ok` | |
| Which devices does the page see; do labels show; does the page have `setSinkId`? | 2: `probe: devices ...`, `setSinkId ...`; every voice step: `devices (Windows)` | |
| Does an AudioContext run hidden, with and without a gesture? | 2-5: `AudioContext made hidden`, `resume() again` | |
| Which hiding and switch set keep audio and a 1 s timer running for 60 s; and past 5 min? | 2-6: `probe verdict ... PASS/FAIL` | |
| Does the web session join a server voice channel, hidden, by trusted input; which way? | 8: `press: channel by ...`, `voice-join: by ...: a voice panel shown yes` | |
| What shows it is connected (panel, status words, class, switches)? | 8: `connected:` | |
| Who is in the call and who is speaking, read from the page? | 8, 10: `hold t s: participants n ..., speaking k (names, avatars, tiles)` against what the people did | |
| Do mute and deafen work by Discord's own switches, and can their state be read? | 8: `mute: before/after`, `deafen: before/after` | |
| Does leave work and is it confirmed? | 8-11: `leave: the voice panel is gone` | |
| Does a DM call start, and is it answered? | 10: `call: ... effect seen yes`, `connected: yes` | |
| Is an incoming call seen in the page; does `NotificationReceived` fire for it; does answer work; decline? | 11, 12: `ring: shown`, `notification: n (...)`, `answer:`, `decline:` | |
| Does audio keep running hidden with a game in front (section 6)? | 9: the hold lines with `in front: another program's window, full screen yes`, and both people | |
| Processor and memory during a call, during a game | 8, 9: `cost at n s` | |
| Is Krisp offered in the page's Voice & Video settings under WebView2's user agent? | Not logged: the lead or user looks once (`--sign-in` window, User Settings > Voice & Video > Noise Suppression) | |
| Do Discord's web notifications reach the host at all (messages too)? | Any step: `notification: n` while someone messages the user | |

## 6. "Audio keeps running hidden with a game in front": pass and fail

Run 9 of section 3: `--voice-join ... --hold 420 --minutes 12`, hidden as the probe chose, a game started full
screen once the log says `connected: yes`, both people talking now and then. Chromium throttles a hidden page's
timers harder after 5 minutes hidden, unless it has made sound in the last 30 seconds or uses WebRTC
([Chrome's timer throttling](https://developer.chrome.com/blog/timer-throttling-in-chrome-88)); WebView2 throttles
and purges caches for a web view whose `IsVisible` is false
([CoreWebView2Controller.IsVisible](https://learn.microsoft.com/dotnet/api/microsoft.web.webview2.core.corewebview2controller.isvisible)).
A call is WebRTC, so it should be exempt: the run is long enough to show it.

**Pass**, all of:

- every `cost` line after the game starts says `in front: another program's window, full screen yes` (the game
  really was in front);
- every `hold` line from `connected` to the end says `rtc connected` and `peer connections 1 (connected 1)`;
- while the other person talks, `audio in` is above 0 kB and `level` above 0 every second (no run of 3 seconds at
  `+0.0 kB` while they talk); while the user talks, `out` above 0 kB and `mic level` above 0;
- `mic tracks 1 (enabled 1, muted 0)` throughout (unless the lead muted), `audio contexts ... (running n)` with
  running equal to the count;
- the timer's `max gap` stays under 2000 ms (Discord's own heartbeats and speaking indicators run on timers);
- **both people say they heard each other throughout, without gaps or robot voice**: the logs cannot hear;
- no `process failed` line; the web view under about 10% of one core on average in `cost` (the game's own cost
  is the user's to judge).

**Fail**, any of: the panel leaves `connected`; incoming bytes stall while the other talks; a context goes
`suspended` or the mic track `muted` or `ended` with nobody pressing mute; a timer gap of 5 s or more; either person
hears gaps. A fail with `--hide controller` is tried again with the combination the probe passed with
(`offscreen`, `visible-unshown`, set `b`) before it counts against the route.

## 7. Devices: how the product would choose them

Windows' default devices are counted only; the spike chooses none. Chromium exposes a `default` and a
`communications` input that follow Windows' own defaults (the probe logs whether both are there), and a page can
send audio to any output by `setSinkId`
([HTMLMediaElement.setSinkId](https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement/setSinkId)). Discord's
web client has its own input and output device menus (User Settings > Voice & Video), kept in its own session.
So the product would:

- by default follow Windows: Discord's `Default` input and output, which become whatever Windows' defaults are
  (the TV's HDMI output, the user's headset or pad headset when it is plugged in);
- for a choice, open Discord's own Voice & Video settings in the session's shown window (the sign-in window of
  #197) rather than build a device menu of ours: Discord applies it through `getUserMedia({deviceId})` and
  `setSinkId` and keeps it. A menu of our own in Settings, writing Discord's choice by script, is possible but is
  more of Discord's page to depend on.

## 8. What is lost against the Discord app

| | The app | The web session | Source |
| --- | --- | --- | --- |
| Push to talk | System-wide | **Only while the browser window is focused**: "Push to Talk only works in the browser app when the window is in focus. If you want to use PTT system wide, you'll have to download the desktop app!" A hidden web view is never focused; the product would have to send Discord's PTT key by DevTools while the pad's button is held (focus emulation makes the page believe it has focus: untested), or use voice activity and mute | [Voice Input Modes 101](https://support.discord.com/hc/en-us/articles/211376518-Voice-Input-Modes-101-Push-to-Talk-Voice-Activated); Discord's engineering blog lists "system-wide push to talk" among what only the native apps do ([how Discord handles 2.5 million voice users](https://discord.com/blog/how-discord-handles-two-and-half-million-concurrent-voice-users-using-webrtc)) |
| Noise suppression (Krisp) | Yes | **Krisp is offered in desktop browsers**: "As of October 30, 2023, Krisp is available on Chrome and Firefox desktop browsers." Whether Discord offers it under WebView2's Edge user agent is a question for the run (section 5). The browser's own noise suppression is on the track either way (the probe logs it) | [Krisp FAQ](https://support.discord.com/hc/en-us/articles/360040843952-Krisp-FAQ) |
| Echo cancellation, gain control | Discord's own media engine | **The browser's**: "our browser app relies on the WebRTC implementation offered by the browser", while the desktop and phone apps use "a single C++ media engine"; "certain features work better in the installed application than in the browser". No Discord page says how its echo cancellation differs; the probe logs that the browser's echo cancellation, noise suppression and auto gain are on | [Discord's engineering blog](https://discord.com/blog/how-discord-handles-two-and-half-million-concurrent-voice-users-using-webrtc) |
| Other native-only parts | Avoiding Windows' auto-ducking, its own volume control, raw audio for voice activity, less bandwidth and CPU in silence | Lost | The same blog |
| The in-game overlay | Windows only, the app's | None. The Dashboard has its own Guide over games; the overlay was never part of the plan | [Game Overlay 101](https://support.discord.com/hc/en-us/articles/217659737-Game-Overlay-101) |
| Per-app audio | "Discord" in Windows' volume mixer and its own output device | The sound comes from WebView2's processes (`msedgewebview2.exe`, its audio service in a utility process), not "Discord"; another WebView2 program's sound is not mixed with it but may look alike in the mixer (not documented; look during the run) | [WebView2 process model](https://learn.microsoft.com/microsoft-edge/webview2/concepts/process-model), [process-related events](https://learn.microsoft.com/microsoft-edge/webview2/concepts/process-related-events) |
| End-to-end encryption (DAVE) | Yes | Required for voice since 2 March 2026; Edge 86 and later are supported ("Browser support for end-to-end encryption requires additional underlying APIs"); the TV's WebView2 is Edge 157 | [Minimum client version for voice](https://support.discord.com/hc/en-us/articles/38025123604631-Minimum-Client-Version-Requirements-for-Voice-Chat) |
| Video, screen share | Yes | Out of scope (D5) | |

## 9. What follows

**If it passes** (voice joins, connects, mutes, leaves, the call and the ring work, and audio holds with a game in
front):

- Voice moves to the web session; the RPC route (`crates/discord`, [`docs/discord.md`](../discord.md)) and the
  user's own Discord developer application (its client id, secret and redirect, the sign-in through the Discord app)
  are retired, and **the Discord app is no longer needed** on the TV PC. D1 of #192 is overturned.
- The Discord tab's Call is read from the page: the voice panel's status, the channel's or call's participants,
  `usernameSpeaking_` and the tiles for who speaks, the mute and deafen switches' `aria-checked`; its actions press
  Discord's own buttons as the spike does.
- The incoming-call Toast comes from the page: the ring's element (and `NotificationReceived` if it fires for calls),
  with Answer and Decline pressing the ring's buttons.
- The session must then be up whenever voice may be wanted: about 400 MB private and under 1% of a core at idle
  (#196), more in a call (section 5's `cost` lines). Settings says so.
- Push to talk becomes the pad's button sending Discord's PTT key by DevTools, if that works with focus emulation (a
  follow-up), or mute on a button.

**If it fails** (audio stalls with a game in front under every hiding and switch set, or joining cannot be made to
work hidden): D1 stands. The web session keeps text, the DM list and notifications (#197 to #202), and voice stays
with the Discord app over RPC (`crates/discord`), as the Discord tab does today; the user is told the integration
is two things because the web page's voice does not survive in the background. A partial pass (voice works only with
`--hide offscreen` or set `b`) is a pass with that setting made the product's.

## 10. Discord's terms

Unchanged from [`discord-web-spike.md`](discord-web-spike.md) section 5: Discord forbids automating a user account
outside its OAuth2 and bot API ("Automating normal user accounts (generally called 'self-bots') outside of the
OAuth2/bot API is forbidden, and can result in an account termination if found",
[Automated User Accounts](https://support.discord.com/hc/en-us/articles/115002192352-Automated-User-Accounts-Self-Bots)).
This spike adds pressing join, call, mute, deafen, answer, decline and disconnect, each once and each started by the
lead with the user there, and observes the page's own WebRTC objects (references kept, nothing changed). It calls no
Discord API and reads no token or storage. The risk to the account stays the user's.

## 11. What could not be checked without Windows

It compiles and passes clippy for `x86_64-pc-windows-gnullvm`; the command line is tested on Linux; the page scripts
pass `node --check`. Not yet seen working, every one of them a question for the TV:

- every selector: `section[aria-label="User area"]`, `[class*="rtcConnectionStatus_"]` and its `Connected_`,
  `Connecting_`, `Error_` forms, `button[aria-label="Disconnect"]`, the switches' `Mute`/`Unmute` and
  `Deafen`/`Undeafen` labels, `[data-list-item-id^="channels___"]` with "(voice channel)" in its label,
  `[data-dnd-name]`, `[class*="voiceUser_"]`, `[class*="usernameSpeaking_"]`, `[class*="avatarSpeaking_"]`,
  `[class*="callContainer_"]` and its tiles, `Start Voice Call`, `Join Voice`, the ring's `incomingCall`/`ringing`
  classes and its Accept/Decline buttons. Written from what is known of Discord's markup and the Bridge's notes, never
  from a saved page; the logs' hit counts say which match. English labels are assumed;
- that wrapping `RTCPeerConnection` and `AudioContext` in a `Proxy` before Discord's scripts leaves Discord working
  (it should: the objects are Discord's own, made the same way);
- that `ICoreWebView2Profile4::SetPermissionState` and `NotificationReceived` exist in the TV's runtime (both are
  logged if not);
- whether a DevTools key or click presses Discord's buttons in a hidden page (#196's click did not open a DM row;
  keys did type).
