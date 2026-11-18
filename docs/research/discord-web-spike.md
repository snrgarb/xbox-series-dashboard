# The Discord web session spike: discord.com in a WebView2 hosted from Rust, on the TV PC

Spike for [#196](https://github.com/snrgarb/xbox-series-dashboard/issues/196), milestone M9 Full Discord. The
decisions it works under are on [#192](https://github.com/snrgarb/xbox-series-dashboard/issues/192): D1 opt-in with
RPC kept for voice; D2 our own web session; D3 our own screens under Party in the Guide; D4 DMs and notifications
first; D5 no video; D6 messages in memory only; D7 our own code. The route and its risks are in
[`discord-bridge.md`](discord-bridge.md), sections 4, 6 and 8: this is its stage S0.

Written on Linux on 18 November 2026. **Nothing here has run on Windows yet**: the code is checked and linted for
`x86_64-pc-windows-gnullvm`, CI builds it, and the lead runs it on the TV PC. The results table (section 7) is empty
for that run.

## 1. What it is

`spikes/discord` builds `discord-spike.exe`, a throwaway program of five steps, one per run. Each step appends to a
log beside the exe and exits; the lead starts each one as a scheduled task in the signed-in desktop and reads the log.

| Step | What it does | Needs the user |
| --- | --- | --- |
| `--probe` | The PC (Windows build, session, input desktop, shell mode, Explorer), the WebView2 runtime (registry and loader), then an environment and a controller made on a hidden window with `--data` as the user data folder; a blank page's user agent; memory. `--online` also loads discord.com/login hidden | No |
| `--sign-in` | discord.com/login full screen, topmost; the pad mapped to keys, a keyboard for text, the pointer on the left stick. Logs where the page went and what Discord showed; waits for the quit file or `--minutes` | **Yes**: the user signs in |
| `--dms` | The saved session, hidden; one script reads the DM list as the page shows it; counts to the log, names to a file of their own; memory, and CPU at idle after 60 s | No (signed in first) |
| `--send` | Opens the conversation named in `--to-file`, puts the text from `--text-file` in by `Input.insertText`, presses Enter once, looks for the message in the page | **Yes**: the user names the conversation and the words |
| `--sign-out` | Removes the session folder; says whether anything is left | No |

**What it never does**: type, read, print or keep a password, a code, the token or a cookie. The sign-in is the
user's, in Discord's own page; the pad's presses become key events, and nothing typed is logged. WebView2's own
password saving and autofill are turned off. No script calls Discord's API or reads its storage; the DM list is read
from the page's DOM and its accessible attributes. Addresses are logged as scheme, host and path only, with long
numbers as `<id>` and long opaque segments as `<long>` (`spikes/discord/src/redact.rs`). Conversation names go to
`dm-names.txt`, never the log; the message and its conversation are logged as lengths. `--send` refuses to run
without both files.

### How it is built

- **`webview2-com` 0.39** (MIT; the COM bindings Tauri's `wry` uses), on `windows` 0.62, the version the workspace
  already builds. The repository has no `deny.toml`; the licence rule is `packaging/about.toml`'s list, which has MIT.
  The spike is not part of `dashboard.exe` or its package.
- **WebView2Loader.dll is loaded at run time** from beside the exe (`LoadLibraryW`, then
  `CreateCoreWebView2EnvironmentWithOptions` and `GetAvailableCoreWebView2BrowserVersionString` by
  `GetProcAddress`), not linked: on `-gnullvm` the crate can only link the DLL's import library, and a missing DLL
  would stop the exe before it could log. CI copies the DLL that `webview2-com-sys` ships (Microsoft's
  redistributable loader) into the `windows` artifact beside the exes (`.github/workflows/ci.yml`). It is never
  committed. **For #197**: `dashboard.exe` would need the same DLL beside it in the package, or a loader of our own.
- **The host thread**: one thread made for the web view, COM single-threaded, owning the window, the environment, the
  controller and every callback, its messages pumped by the steps' own waits. A watchdog ends the process
  `--minutes` + 2 after it starts if a step hangs.
- **A Windows-subsystem exe**: no console window when a task starts it; everything is in the log.
- **Hidden** means one of two things (`--hide`): `controller` (the default: the window is never shown and the
  controller is told `IsVisible = false`) or `offscreen` (the window is shown, never activated, at -20000,-20000,
  and the controller thinks it is visible). A hidden web view is laid out at 1600 x 900 CSS pixels (scale 1),
  wide enough for the DM list. `--browser-args` passes Chromium switches through, for trying
  `--disable-features=CalculateNativeWinOcclusion` or `--disable-backgrounding-occluded-windows` if a hidden page
  stops working.
- **Input to the page** is DevTools' `Input.dispatchKeyEvent`, `Input.insertText` and `Input.dispatchMouseEvent`
  through `CallDevToolsProtocolMethod`, as the research chose: trusted events, delivered whether or not the window
  has the foreground. The left stick's pointer and the trigger's click are Windows' own `SendInput`, for what keys
  cannot reach.

### The keyboard: the spike's own pad keyboard, and why

`--keyboard pad` (the default) is a 10 x 4 grid in three layers (`abc`, `ABC`, `#+=`) drawn by the spike with GDI
along the bottom of its window, the web view shrunk above it. It opens by itself when a text field takes focus, and
with Y. The D-pad moves over it, A types the key (`Input.insertText`), X deletes, Y is a space, LB the next layer,
View Tab, Menu Enter, B closes it.

- **Not the Dashboard's Console keyboard**: it is drawn by the `ui` and `render` crates through wgpu with the fonts
  loaded from Windows, and steered by the `screens` crate's page stack. Bringing that into a throwaway window is
  most of a renderer; the spike only has to answer whether text from the pad reaches Discord's fields, which the
  stand-in answers. The real one comes in #197 and #201, inside the Dashboard, where it already is.
- **Windows' touch keyboard** (`--keyboard tabtip`) is there to try, not the default: it is started
  (`TabTip.exe`) and toggled through `ITipInvocation` (undocumented, as every program that shows it does). Whether it
  comes up in shell mode, and whether its gamepad layout drives it, is a question for the TV run; the probe logs
  whether `TextInputHost.exe` and `TabTip.exe` are running.
- **The on-screen keyboard** (`--keyboard osk`) is started with Y; it needs a pointer, which the left stick and the
  right trigger give.
- **A real keyboard and mouse** work in every mode, if the window has the foreground.

## 2. Getting it onto the TV

From CI's `windows` artifact of the PR's run (it holds `discord-spike.exe` and `WebView2Loader.dll` beside the
other exes):

```sh
gh run download <run id> -n windows -D target/tmp/discord-spike
ssh htpc-win mkdir "%LOCALAPPDATA%\xsd-discord-spike"
scp target/tmp/discord-spike/discord-spike.exe target/tmp/discord-spike/WebView2Loader.dll \
    htpc-win:AppData/Local/xsd-discord-spike/
```

Every file is beside the exe by default: the session `session\`, the log `discord-spike.log`, the names
`dm-names.txt`, the quit file `quit`; a relative path on the command line is taken from the exe's folder too (a task
starts in System32, and `schtasks /tr` takes 261 characters at most). The folder is outside `%TEMP%` so the session outlives a cleanup.

Each step is one scheduled task in the signed-in session (`/it`), as `e2e/htpc/run.sh` starts the Dashboard, run
now and deleted after; the time `/st 00:00` is never reached. Over ssh (cmd expands `%LOCALAPPDATA%` when the task
is made; the path must have no spaces):

```sh
step() {  # step '<arguments>'
  ssh htpc-win "schtasks /create /tn xsd-discord-spike /sc once /st 00:00 /it /f /tr \"%LOCALAPPDATA%\\xsd-discord-spike\\discord-spike.exe $1\" >nul && schtasks /run /tn xsd-discord-spike"
}
log() { ssh htpc-win type "%LOCALAPPDATA%\\xsd-discord-spike\\discord-spike.log"; }
done_() { ssh htpc-win schtasks /delete /tn xsd-discord-spike /f; }
```

| Step | Run | Ends |
| --- | --- | --- |
| 1 | `step '--probe --online'` | by itself, in about a minute |
| 2 | `step '--sign-in'` (or `'--sign-in --keyboard tabtip'`, `'--sign-in --keyboard osk'`) | when the lead runs `ssh htpc-win 'type nul > "%LOCALAPPDATA%\xsd-discord-spike\quit"'`, the window is closed (Alt+F4), or after 20 min (`--minutes`) |
| 3 | `step '--dms'`, then `step '--dms --hide offscreen'` | by itself, in about two minutes (90 s at most to load, 60 s idle, 30 s measured) |
| 4 | the lead writes the user's two lines (UTF-8, one line each) to `to.txt` and `text.txt` in the folder (`scp`), then `step '--send --to-file to.txt --text-file text.txt'` | by itself; delete both files after |
| 5 | `step '--sign-out'` | by itself |

Read the log after each (`log`), then `done_`. The log is appended to, each step starting with
`discord-spike 0.1.0 started: <step> (pid n)` and ending `done: exit <code>`. **`dm-names.txt` holds the user's
conversations' names: it stays on the TV or in `target/tmp`, never in the repository**, and is removed with the
folder at the end (`ssh htpc-win rmdir /s /q "%LOCALAPPDATA%\xsd-discord-spike"`).

While `--sign-in` runs, the Dashboard behind it (if it is running) also reads the pad through XInput: leave it on a
screen where presses do no harm, or out of the way.

## 3. The steps, their log lines and what they mean

Every line starts with the UTC time. Lines any step may write:

| Line | Means |
| --- | --- |
| `window: full screen 3840x2160 px, topmost; foreground given: yes/no` | The sign-in window (or `--visible`). "no": Windows refused the foreground to a task's process; the pad still works (DevTools input), a real keyboard may not until the window is clicked |
| `window: never shown, 1600x900 px; the web view is told it is not visible` / `window: shown off every screen ...` | The two hidden modes |
| `environment: made in <ms> ms, browser <version>, user data folder <dir>` | WebView2 started a browser for our folder: an unpackaged exe can host it |
| `controller: made in <ms> ms` | The web view exists on our window |
| `settings: password saving and autofill off` | WebView2 will not offer to keep the password |
| `navigation: start <scheme://host/path>`, `navigation: done <address> ok yes http 200` | A page load and how it ended (`web error n` is `COREWEBVIEW2_WEB_ERROR_STATUS`) |
| `route: <address>` | Discord's own in-page navigation (it is one page that changes its path) |
| `new window refused: <address>` | The page asked for a new window (a help link): refused |
| `permission asked and refused: kind n` | `COREWEBVIEW2_PERMISSION_KIND`: 1 microphone, 2 camera, 4 notifications, ... All refused |
| `process failed: kind n` | A web view process died (`COREWEBVIEW2_PROCESS_FAILED_KIND`) |
| `memory (<when>): <kind> pid n: working set, private` and `... web view, n processes: ...` | Each WebView2 process (browser, renderer, gpu, utility) and their sum; `this spike` is our own |
| `page: the app is shown after <ms> ms` / `page: Discord shows its login page: this session is not signed in` | `--dms` and `--send` reaching the app, or finding no session (exit 3) |
| `watchdog: ... ended here` | The step hung: exit 9 |

### Step 1, `--probe [--online]`

| Line | Means |
| --- | --- |
| `windows: <ProductName> <DisplayVersion> build <n>.<ubr>` | Build 22000 and up is Windows 11 (ProductName says 10 on 11) |
| `session: this process in session n, the console's is m; input desktop Default` | The task ran in the signed-in desktop if n = m and the desktop is `Default` |
| `shell: dashboard.exe (this user's Winlogon Shell: shell mode yes)` | Shell mode is set |
| `shell: explorer.exe processes n, a shell window yes/no; dashboard.exe processes n` | **Whether Explorer runs** (the Dashboard's shell mode starts it behind Start-up, `shell.md`), so whether this run answers "WebView2 without Explorer" |
| `processes before: msedgewebview2.exe n ..., TextInputHost.exe n, TabTip.exe n` | Other programs' web views already running; the touch keyboard's hosts |
| `runtime: registry HKLM\...\Clients\{F3017226-...}: Evergreen version <v>` | The runtime installed for the machine (or `HKCU` for the user) |
| `runtime: no fixed version ...` | No fixed-version runtime is in play |
| `loader: <path> loaded`, `runtime: the loader finds Evergreen <v>` | WebView2Loader.dll works and finds the runtime. `loader: WebView2Loader.dll is not beside the exe` is exit 2 |
| `probe: environment and controller made from this unpackaged exe in <ms> ms` | **The hosting question answered: yes** |
| `probe: a script runs (2); page visibility hidden; user agent <ua>` | ExecuteScript works; what a hidden page believes; the user agent Discord will see (an Edge one) |
| `probe: DevTools protocol answers: <product>` | `CallDevToolsProtocolMethod` works |
| `online: after <ms> ms: route login, login form yes, captcha no, QR yes, app no` | discord.com/login loads in our web view and shows its form (`--online`) |
| `session folder <dir>: n files, m MB` | What the session left on disk |

Exit 0, else 2 (no loader or runtime) or 4 (no web view).

### Step 2, `--sign-in`

The window covers the screen; a bar along the bottom says the pad's keys.

| The pad | Keyboard closed | Pad keyboard open |
| --- | --- | --- |
| D-pad | Up Shift+Tab, Down Tab, Left/Right the arrow keys | moves over the keys (a held direction repeats) |
| A | Enter | types the key |
| B | Escape | closes the keyboard |
| X | Space (a checkbox) | Backspace |
| Y | the keyboard (`--keyboard`) | a space |
| LB / RB | arrow up / down | LB the next layer |
| View | Backspace | Tab |
| Menu | | Enter |
| Left stick, right trigger, right stick | the mouse pointer, its left button (held for a drag), the wheel | the same |

| Line | Means |
| --- | --- |
| `pad: one is connected (XInput slot n)` / `pad: none connected` | The pad the step reads |
| `page: route login, login form yes, password field yes, captcha no (0 hCaptcha frames), two-factor no, QR code yes, new-location check no, app no` | **What Discord shows**, written each time it changes (looked at every 2 s). `captcha`: an hCaptcha frame or a captcha element. `two-factor`: a one-time-code field or its words. `new-location check`: the words of Discord's "new login location" or e-mail check (English page only). `QR code`: the QR login panel |
| `focus: a text field (email / password / text / contenteditable)` / `focus: not a text field` | Where the pad's Tab put focus: the kind of field, never its value |
| `keyboard: the spike's pad keyboard opened (a text field took focus)` | The pad keyboard came up by itself |
| `keyboard: touch keyboard toggled (Y): HRESULT(0x0); TabTip.exe was running ..., TextInputHost.exe ...` | `--keyboard tabtip`: whether the toggle worked (does it show?) |
| `start: osk.exe: started / refused (n)` | `--keyboard osk` |
| `pointer: SendInput refused (...)` | The stick's pointer cannot move (once) |
| `sign-in: the app page is reached after n s: signed in; the session is kept in <dir>` | **Signed in** |
| `sign-in: the quit file is there` / `the window was closed` / `20 min are up` | Why it ended |
| `sign-in: closing; signed in yes/no; the session stays in <dir>` | The end |

### Step 3, `--dms [--hide offscreen] [--visible]`

| Line | Means |
| --- | --- |
| `dms: the page says visibility hidden/visible, focus false` | What Discord's page believes about being hidden |
| `dms: the script took <ms> ms` | One `Runtime.evaluate` of `js/dms.js`, scrolling Discord's virtualised list a page at a time |
| `dms: selectors matched: dm_links=n dm_links_in_guild_bar=n guild_bar_items=n guild_bar_badges=n listitem_role_dm=n dm_name_class=n dm_aria_label=n dm_number_badge=n unread_class=n private_channels_list=n` | **Which selectors work today**: each one's match count on the first screen (-1: refused by the browser). `dm_links` (`a[href^="/channels/@me/"]`) is the anchor: a route, not a class |
| `dms: n conversations (list scroller found yes, read over p pages); names from name_ class a, aria-label b, text c` | The list's length, read whole; where names came from |
| `dms: unread: n by any sign; in the server bar a, by an unread class b, by a heavier name c; server bar entries d` | Three ways to tell unread, side by side: a DM in the server bar (Discord shows unread DMs there), a class with "unread" in it, a bolder name than most |
| `dms: with a count: n (row badge a, aria-label b, server bar badge c); counts sum to s` | Mention counts and where they came from |
| `dms: n names written to <file> (not to this log)` | `dm-names.txt`: each line `name<TAB>unread flags (bar/class/weight, y or -)<TAB>count`, for the lead to compare with Discord itself |
| `memory (app page, hidden): ...` | **Memory of the web view hidden** with Discord loaded |
| `cpu: web view at idle over 30 s: x s of processor time, y% of one core, z% of all n threads` | **CPU at idle**, measured after 60 s of nothing |

Exit 0; 3 not signed in; 5 the script failed; 8 the app did not show.

### Step 4, `--send --to-file <f> --text-file <f>`

Refused (exit 10, nothing sent) unless both files exist, the text is one line of at most 2,000 characters, and the
name one line. Never retried.

| Line | Means |
| --- | --- |
| `send: a conversation named in n characters, a message of m characters` | The files were read |
| `send: k conversations of that name among r rows ...: nothing sent` | Not found, or not one (exit 4). The name must be as the DM list shows it; case is ignored if no exact match |
| `send: the conversation found (name matched exactly, over p pages); covered by something else: no` | Found; nothing lay over the point to click |
| `send: opened by the click in <ms> ms; it shows n messages` | A DevTools mouse click on the row opened the conversation and its message box |
| `send: the message box already holds a draft: nothing sent` | Exit 6: never mixes with the user's own draft |
| `send: the message box has focus after a click: yes` | The box takes a DevTools click; "no" stops it (exit 5) |
| `send: Enter pressed (once)` | `Input.insertText` put the text in and Enter went |
| `send: after <ms> ms: the newest message is the text yes, still marked sending no, marked failed no, box empty yes; ...` | **Whether the message appeared in the page** |
| `send: the message appeared in the page` | Exit 0. Otherwise exit 7: look in Discord itself |

### Step 5, `--sign-out`

| Line | Means |
| --- | --- |
| `sign-out: <dir> holds n files, m MB` | What the session was |
| `sign-out: attempt n: <error>` | A file held (a web view still running): tried 5 times, 2 s apart |
| `sign-out: nothing is left: <dir> is gone` | **The PC is signed out** |
| `sign-out: Discord still lists this session under User Settings > Devices until it is logged out there` | Removing the folder does not end the session on Discord's side |

## 4. The questions each step answers, for #197 to #202

| Question | Step and line | For |
| --- | --- | --- |
| Is the WebView2 runtime on the TV, which version, Evergreen or fixed? | 1: `runtime:` lines | #197 (no-runtime state) |
| Does it host from an unpackaged Rust exe, with a user data folder of ours? | 1: `environment:`, `controller:` | #197 |
| Does it work in shell mode? Is Explorer running when it does? | 1: `shell:` lines beside `probe: environment and controller made` | #197 |
| How long do the environment and controller take to make? | 1, 2, 3: `made in` | #197 (when to start it) |
| Does WebView2Loader.dll need shipping? | 1: `loader:` | #197 (packaging) |
| Does discord.com/login load in our web view, and does Discord accept it (user agent)? | 1 `--online`, 2: `page:` | #197 |
| Can the user sign in with the pad: focus by Tab, Enter, the keyboard's text in the fields? | 2: `focus:`, `page:`, the user's own account of it | #197, #201 |
| Which keyboard: the pad keyboard (stand-in for the Console keyboard), the touch keyboard in shell mode, osk? | 2: `keyboard:` lines, and what the user saw | #201 |
| What does Discord put in the way: captcha, new-location e-mail, two-factor, QR? | 2: `page:` lines | #197 (what the sign-in page must allow), #201 (captcha said plainly) |
| Does a captcha frame take the pad (Tab into it) or need the pointer? | 2: `page:` and the user | #197 |
| Does the sign-in window steal or need the foreground? | 2: `window: ... foreground given` | #197 |
| Is the session kept, and what is in the folder? | 2: `sign-in: closing`, `session folder`; 3 finding the app | #197 |
| Does a hidden web view keep Discord working: does the page load, does the list render, which hidden mode? | 3: `dms:` with `--hide controller` and `offscreen`; `visibility` | #198 |
| Which selectors match Discord's DM list today; how is unread best told; how are counts read? | 3: `selectors matched`, `unread`, `with a count` | #198 (our selectors file and its health probe) |
| Is the list virtualised, and does paging through it by scrolling work hidden? | 3: `scroller found`, `pages` | #198 |
| What does it cost hidden: memory, CPU at idle? | 3: `memory`, `cpu` | #197 (always on or on demand), #202 (a session kept for notifications) |
| Do DevTools input events open a conversation and send, hidden, with no foreground? | 4: `opened by the click`, `has focus`, `appeared` | #201 |
| Does Discord accept a message sent that way, or answer with a captcha or refusal? | 4: `marked failed`, and Discord itself | #201 |
| Does sign-out leave nothing, and what stays on Discord's side? | 5 | #197 |
| Do Discord's web notifications reach the web view? | **Not tested**: every permission is refused here. A follow-up for #202 if wanted | #202 |
| How do the screens look? | Not this spike: our own screens (D3), #199 and #200 | |

## 5. Discord's terms: the risk, plainly

Discord's [Terms of Service](https://discord.com/terms) (effective 29 September 2025; quoted in
[`discord-bridge.md`](discord-bridge.md) section 4, read 2026-11-17), under "Don't use the services to do harm to
Discord", forbid "scraping our services without our written consent, including by using any robot, spider,
crawler, scraper, or other automatic device, process, or software", and "using any unauthorized software designed
to modify the services". Discord's support article
[Automated user accounts (self-bots)](https://support.discord.com/hc/articles/115002192352) says that automating a
normal user account outside the OAuth2/bot API is forbidden and can get the account terminated (that page refuses
automated fetches; its wording is from its indexed copies, as the research says).

What this spike does against that:

- It **reads Discord's page by script** (the DM list) and **sends a message by program** (DevTools input events, the
  user's words, once, to a conversation the user names). That is a program acting on a user account: what the
  self-bot article and the "automatic ... software" clause speak to. No document of Discord's allows it.
- It does not modify Discord's software, call its API, read its token or storage, or act in bulk: one page, one
  list read, one message, each started by the lead with the user's go-ahead.
- Discord sees a second signed-in client (a new device, an Edge user agent) and may answer with a captcha, a
  new-location e-mail, or worse. The sign-in and the one message are the user's own; **the risk to the account is
  the user's, and running steps 2 to 4 is the user's decision** (D1 was answered "opt-in"; this spike is the first
  time it touches the real account).

## 6. What could not be checked without Windows

Everything that runs: the code compiles and passes clippy for `x86_64-pc-windows-gnullvm` and its pure parts are
tested on Linux (`cargo test -p discord-spike`: the command line, redaction, the pad's presses and pointer, the
keyboard's grid, the log's time). In particular, not yet seen working:

- that `-gnullvm` links `webview2-com` (CI's Windows job builds it) and the loaded DLL makes an environment;
- that DevTools' `Input.dispatchKeyEvent` Tab moves focus in Discord's form, and `Input.insertText` fills its fields
  and its message box (a Slate editor);
- that a controller told it is hidden still runs Discord's page (Chromium may throttle it), and that the off-screen
  window is not treated as occluded;
- every selector in `js/state.js`, `js/dms.js`, `js/find.js`, `js/composer.js` and `js/sent.js`: written from
  Discord's page as the research describes it and what is known of its markup (`data-list-id="guildsnav"`,
  `a[href^="/channels/@me/"]`, `[class*="name_"]`, `[class*="numberBadge"]`, `li[id^="chat-messages-"]`,
  `[id^="message-content-"]`, `[role="textbox"][data-slate-editor="true"]`), never a saved real page. The `--dms`
  log is there to say which match;
- the touch keyboard's toggle under shell mode, and whether `SendInput` (the stick's pointer) is allowed to a task's
  process;
- the GDI help bar and keyboard's look at 4K.

## 7. Results (the lead fills this from the TV run)

| Step | Run on (date, build) | Exit | What the log said | What the user saw | Answer |
| --- | --- | --- | --- | --- | --- |
| 1 `--probe --online` | | | | | |
| 2 `--sign-in` (pad keyboard) | | | | | |
| 2 `--sign-in --keyboard tabtip` | | | | | |
| 3 `--dms` (controller) | | | | | |
| 3 `--dms --hide offscreen` | | | | | |
| 4 `--send` | | | | | |
| 5 `--sign-out` | | | | | |

| Question | Answer |
| --- | --- |
| WebView2 runtime present, version, Evergreen or fixed | |
| Present and working under shell mode (Explorer running?) | |
| Environment + controller from the unpackaged exe | |
| Sign-in with the pad: worked? | |
| Keyboard that worked | |
| What Discord showed (captcha, new-location mail, two-factor, QR) | |
| DM list read hidden: conversations, unread, counts right? | |
| Selectors that matched | |
| Memory hidden (web view total) | |
| CPU at idle | |
| Message sent and seen in the page | |
| Sign-out left nothing | |
