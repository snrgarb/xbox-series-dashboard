//! Stand-ins for what a test machine doesn't have: Steam, and a game.
//!
//!   standin make-steam <dir> [--json <file>] [--full-shelf]
//!       Writes a fake Steam install at <dir> (`source_steam::fixture`), with a copy of this program as `game.exe` in
//!       every installed game's folder and as the client's `steam.exe`, reads it back, and with --json writes the
//!       Library it read there. With --full-shelf it is `Fixture::full_shelf`, twenty installed games, where it is
//!       otherwise `Fixture::standard`.
//!   steam.exe -silent +app_install <appid> | -silent -console +app_uninstall <appid>
//!       The stand-in Steam client, when this program is named `steam.exe` in a fake Steam's folder, as the
//!       Dashboard asks the real one (`source_steam::command`): says in the log what it was asked (`steam: asked to
//!       install <appid>: ...`), and puts the game in its queue, Queued (its app manifest written as Steam writes
//!       one), or uninstalls it (its manifest removed). Nothing is downloaded or deleted.
//!   steam.exe --standin-client [--seconds <n>]
//!       The same client, running (for <n> seconds, 1800 without), as the Dashboard sees a running Steam by its
//!       process. While `.cef-enable-remote-debugging` is in its folder as it starts it opens its CEF debugger
//!       (`source_steam::debugger`, #171) on a free port of 127.0.0.1, written down in `standin-cef-port.txt` (never
//!       8080 or 8081, where a real Steam's may be): Steam's own calls to install, uninstall, pause, resume and cancel
//!       are taken there and done to the fake's files (`steam: CEF: asked to pause <appid>: ...`), and its download
//!       overview given, as the real client's are. make-steam marks its folder so (`STANDIN_CLIENT`), which is how
//!       the Dashboard knows to ask that port and no other. With its debugger open it has, on Windows, the real
//!       client's two windows (#245): its main window, titled "Steam" (class `StandInSteam`, magenta), and its
//!       Install dialog, titled "Install" (class `StandInSteamInstall`, orange), each shown, brought to the top and
//!       given the foreground if Windows lets it as the install wizard's scripts open them, and hidden or minimized as
//!       they are put back (`steam: window: ...` in its log).
//!   standin steam-install <dir> <appid> --queued | --bytes <n>/<m> | --paused <n>/<m> | --done | --drop
//!       Moves the fake Steam's install of <appid> at <dir> along, by rewriting its app manifest as Steam's client
//!       would (`fixture::write_progress`): waiting, so many bytes of so many downloaded, paused there, installed
//!       (with this program as its `game.exe`), or dropped (the manifest gone).
//!   standin steam-workshop <dir> <appid> <done>/<items>
//!       Writes the fake Steam's Workshop record of <appid> (#188, `fixture::write_workshop`): <items> subscribed
//!       items, <done> of them in their folders under `steamapps/workshop/content/<appid>`, and Steam still fetching
//!       until all are in. A test calls it again and again to fill the folder over time.
//!   standin steam-library <dir> <folder>
//!       Adds a library at <folder> to the fake Steam at <dir> (`fixture::add_library`, #179), as Steam's Storage
//!       Manager adds one on another drive: a second folder installs may go to, which the Dashboard asks about.
//!   standin steam-wizard <dir> [--agreements <appid>:<n>]... [--key <appid>]... [--free <bytes>]
//!       Sets up the stand-in client's install wizard at <dir> (#230, `fixture::WizardSetUp`): the apps whose install
//!       shows <n> licence agreements (eInstallState 8, each served by the stand-in at its own address and marked
//!       accepted only by `MarkEulaAccepted`), those that ask for a product key (4), and the bytes free that its
//!       install manager says every library has (a TiB by default). Each call writes the whole set-up anew.
//!   standin steam-change <dir> add-game | played <appid> <minutes>
//!       Changes the fake Steam at <dir>, written by make-steam without --full-shelf, as Steam's own files change
//!       while the Dashboard runs (`fixture::change`): add-game makes Paper Lighthouse (9100800) the user's,
//!       installed; played adds <minutes> to the game's play time and makes it last played now. Each is written over
//!       the standard fixture with the changes made before it, which `fixture-changes.txt` in <dir> keeps.
//!   standin make-epic <dir> [--json <file>] [--owning <name>]
//!   standin make-gog <dir> [--json <file>]
//!       As make-steam, for a fake Epic Games Launcher (`source_epic::fixture`) or a fake GOG (`source_gog::fixture`):
//!       the Dashboard reads them with `--epic <dir>` and `--gog <dir>`. A GOG game is started as its own program;
//!       an Epic one through the launcher's link, which the stand-in launcher, epic-url, takes on a test machine.
//!       With --owning, Epic's account also owns a game of that name, not installed (`Fixture::owning`): the same
//!       game in two launchers, for "Install from" (#249).
//!   standin epic-url <dir> com.epicgames.launcher://apps/...?action=launch
//!       The stand-in Epic launcher, registered as the `com.epicgames.launcher://` handler with the fake's <dir>:
//!       starts the installed game the link names, as the launcher does, and waits for it. Exits 1, having done
//!       nothing, when the game isn't installed. A link with another action (`install`) is said in the log, and
//!       nothing more is done.
//!   standin make-tones <dir>
//!       Writes a short tone into <dir> under each of the names the Dashboard's sounds have in the Bundle, for
//!       `dashboard --media <dir>`: the console's own sounds are in no repository and on no test machine.
//!   standin game [--title <t>] [--splash-ms <n>] [--seconds <n>] [--ignore-close] [--start-ms <n>] [--gap-ms <n>]
//!                [--windowed <percent>] [--crash-ms <n>] [--hand-off-ms <n>]
//!       The stand-in game: a borderless, full-screen, dark-blue window titled <t> ("Stand-in game"). It closes when
//!       asked to, on Esc, and after <seconds> (600) in case a test leaves it behind. With --splash-ms, a small window
//!       titled "<t> launcher" shows for that long first, as a game's launcher or splash screen does. With
//!       --ignore-close it does not close when asked to, as a game that has hung does not, and says so in the log.
//!       For the backdrop behind a game (docs/spec/launch-screen.md): --start-ms waits that long with no window at
//!       all before anything, as a game slow to start does; --gap-ms that long with none between its launcher's
//!       window closing and its own opening; --windowed puts its window in a frame, <percent> of the screen across
//!       and down and in its middle, rather than borderless over all of it; --crash-ms ends the process that long
//!       after its window is up, at once, as a crash does (no close asked, its window going with the process,
//!       exit code 0xC0000005); --hand-off-ms, that long after its window is up, starts this program again as the
//!       game (`standin game`, the same arguments but for --hand-off-ms, --splash-ms and --start-ms, and with
//!       --gap-ms as its --start-ms) and exits at once, as a launcher that starts the real game or a game that
//!       restarts itself does.
//!       Its pulse, a square towards the window's bottom right, changes colour four times a second while the game
//!       runs, and so stands still while the game is frozen; a beat that comes late says in the log how long the
//!       game stood still.
//!       This is also what the program does when it is named `game.exe` and given no arguments, its arguments then
//!       coming from `game-args.txt` beside it, one a line, if that exists.
//!   standin steam-url steam://rungameid/<appid>
//!       The stand-in Steam, registered as the `steam://` handler: starts the installed game's `game.exe`, and keeps
//!       `HKCU\Software\Valve\Steam\Apps\<appid>\Running` at 1 while it runs, as Steam does. Exits 1, having done
//!       nothing, when the game isn't installed.
//!   standin media [--title <t>] [--artist <a>] [--picture <file>] [--position <s>] [--length <s>] [--paused]
//!                 [--no-previous] [--seconds <n>]
//!       A media session of its own, as a music player has, which Windows lists among every app's under this
//!       program's name (`standin.exe`) and the Dashboard's Now Playing card shows as "Standin" (`media.rs`): a
//!       track <t> ("Stand-in Song") by <a> ("The Stand-ins") with <file>'s picture (a green square drawn without
//!       one), <s> seconds into a track <s> long (65 of 200; a length of 0 is none), playing or --paused. Each button
//!       Windows passes on is a line in the log (`media: pressed Play|Pause|Next|Previous`) and is answered as a
//!       player would; Next puts " (2)" and so on after the title. With --no-previous, Previous is off. It ends after
//!       <seconds> (600). Windows only.
//!   standin toast <text>
//!       Posts a notification to the running Dashboard, which shows its Toast (`model::post`). Exits 1 when no
//!       Dashboard took it: none is running, or it has no Toast strip yet.
//!   standin wake
//!       Tells the running Dashboard the machine has woken from sleep, as Windows would (`model::post::WAKE_ID`):
//!       Who's playing today? asks, if it asks on waking. Taken, as a notification is, only by a Dashboard started
//!       with --allow-toast-posts. Exits 1 when none took it.
//!   standin steam-url steam://install/<appid> | steam://uninstall/<appid>
//!       Says in the log which it was asked, the address with it, and does nothing more: a test reads the line.
//!   standin make-ea <dir> [--json <file>] | make-ubisoft <dir> [--json <file>] | make-battlenet <dir> [--json <file>]
//!       Writes that store's fake at <dir> (`source_ea::fixture`, `source_ubisoft::fixture`,
//!       `source_battlenet::fixture`: made-up games, each installed one with a copy of this program as its
//!       `game.exe`, titled with its name), reads it back as the Dashboard does with `--ea <dir>` (and the others),
//!       and with --json writes the Library it read there. Battle.net's client in the fake, `Battle.net.exe`, is a
//!       copy of this program too. make-battlenet's --owned (last) writes `source_battlenet::fixture::Fixture::owned`:
//!       the client's catalogue and the account's licences in its cache, so the games owned are known (#276).
//!   standin make-xbox <dir> [--json <file>]
//!   standin make-gamepass <dir>   (a local PC Game Pass catalogue, US and GB, for --gamepass <dir>: #277)
//!       As make-ea, for a fake drive of Xbox games (`source_xbox::fixture`): a `.GamingRoot` and made-up packages,
//!       each game's `game.exe` a copy of this program, read with `--xbox <dir>`. Tidewater Trials' package is whole,
//!       as Windows registers a folder of loose files: the end-to-end test registers it on the VM.
//!   standin uplay-url <dir> uplay://launch/<id> | uplay://install/<id> | uplay://uninstall/<id>
//!       The stand-in Ubisoft Connect, registered as the `uplay://` handler for the fake at <dir>: starts the game's
//!       `game.exe` from its install folder and waits for it; asked to install or uninstall, says so in the log and
//!       does nothing more. Exits 1, having done nothing, when the game isn't installed.
//!   Battle.net.exe [--exec="launch <code>"] | --game=<uid>
//!       The stand-in Battle.net, when this program is named `Battle.net.exe` in a fake Battle.net's folder. As the
//!       real client (#172): where none runs, it becomes the client, drops a launch given to it as it starts, and
//!       comes up `STANDIN_BATTLENET_READY_MS` (2500) later with its main window, titled "Battle.net", and two
//!       helpers (`Battle.net.exe --type=helper`); a launch given to another `Battle.net.exe` while it is up is
//!       handed to it, and it starts the game with that product code from its install folder. Asked for a game's
//!       page, says so in the log.
//!   Agent.exe [--internalclienttools] [--seconds <n>]
//!       The stand-in Battle.net Agent (#172, `source_battlenet::standin`), when this program is named `Agent.exe` in
//!       a fake Battle.net's `Agent/Agent.<build>` folder, as make-battlenet writes it and the Dashboard starts it:
//!       serves the Agent's HTTP calls on a free port of 127.0.0.1, written in the fake's `Agent/Agent.dat`, for <n>
//!       seconds (1800), installing the fake's catalogue products into the folder it is given (their bytes counted
//!       out `STANDIN_HELPER_STEP_MS` apart, 600 ms without) and uninstalling, by rewriting the fake's files; every
//!       request a line in the log (`battlenet agent: POST /install {...} -> 200 {...}`), its token never.
//!   standin open-address <address>
//!       What `dashboard --open-addresses-with standin.exe` gives every store's address to, on a test machine whose
//!       registry is left alone (the TV PC): does what the handler registered for the address's scheme would,
//!       `steam://` as steam-url, `com.epicgames.launcher://` as epic-url and `uplay://` as uplay-url, and Windows
//!       would with `shell:AppsFolder\<AUMID>`: the fake Xbox drive's game of that app user model ID started from its
//!       package's folder (its Install and Uninstall, the Store's page and Settings, only said); with each fake's
//!       folder from `handlers.json` beside this program (`{"steam": "<dir>", "epic": "<dir>", "ubisoft": "<dir>",
//!       "xbox": "<dir>"}`, written by the tests' helpers) and never from the registry. A game the stand-in Steam starts this way has no `Running` value written:
//!       the Dashboard follows it by its windows. Exits 2 for a scheme it has no stand-in for, 1 when the fake it
//!       needs is not in `handlers.json`.
//!
//!   standin serve-art [--port <n>] [--key <key>] [--candidates <n>]
//!       The stand-in SteamGridDB (`library::art::fake`), on 127.0.0.1 at <n> (any free port without one), for
//!       `dashboard --art-service http://127.0.0.1:<n>`: it takes only <key> ("e2e-key" without one), finds a game
//!       of any name searched for, and gives every game a plain picture of each kind, an orange cover, a teal hero,
//!       a violet header and a yellow square. Those are the first of each kind's list of candidates, which the art
//!       picker pages through: --candidates long (60 without), served as SteamGridDB serves them (`page` from 0,
//!       `limit` at most 50, the reply's `total`), each a lighter shade of its kind's colour, its thumbnail a
//!       smaller one. Under `/stores` it is the stores' own art online too, for
//!       `dashboard --store-art-service http://127.0.0.1:<n>/stores`: Steam's GetItems and CDN, the Microsoft
//!       Store's catalogue and GOG's API, knowing every id, with a red cover, a blue hero, a green header and (the
//!       Microsoft Store's alone) a magenta square (`STORE_ART_COLOURS`). The stores' searches by name (Steam's,
//!       the Microsoft Store's autosuggest, the PlayStation Store's) answer, and find nothing: a game that is not a
//!       store's own goes on to SteamGridDB, as the art payloads expect. Says `serve-art: listening on
//!       http://127.0.0.1:<n>` and then a line a request (its path, and whether it had the key), until it is killed.
//!   standin serve-updates --exe <dashboard.exe> --version <x.y.z> [--port <n>] [--bad-hash] [--not-public]
//!       The stand-in update address (`update::fake`), on 127.0.0.1 at <n> (any free port without one), for
//!       `dashboard --update-source http://127.0.0.1:<n>/releases/latest`: GitHub's API's newest release, version
//!       <x.y.z>, whose portable zip holds <dashboard.exe> and whose SHA256SUMS.txt gives the zip's SHA-256; with
//!       --bad-hash one that is not the zip's, so the download must be refused; with --not-public 404, as a private
//!       repository's. Says `serve-updates: listening on <the update address>` and then a line a request (its path
//!       and User-Agent, and whether it carried any Authorization), until it is killed.
//!   standin serve-helpers --table <file> [--port <n>] [--clients <file> --sandbox <dir>]
//!       The stand-in helpers' release pages (`helpers::fake`), on 127.0.0.1 at <n> (any free port without one):
//!       serves this program, and writes at <file> the helpers' table for `dashboard --helper-table <file>`, every
//!       helper's address the stand-in's and its SHA-256 this program's. legendary's address serves this program, so
//!       it is kept; gogdl's serves other bytes, so it must be refused. With --clients, the launchers' apps' table
//!       too, for `dashboard --client-table <file>` (#178): every installer this program, named
//!       `standin-setup-<key>.exe`, GOG Galaxy's other bytes; an installer told where to go told `<dir>\<key>`.
//!       Says `serve-helpers: listening on http://127.0.0.1:<n>` and then a line a request (its path and
//!       User-Agent, and whether it carried any Authorization), until it is killed.
//!   standin serve-gog-web [--port <n>]
//!       A stand-in of GOG's library service, gamesdb and product API (`source_gog::gogdl::standin::Web`), on
//!       127.0.0.1 at <n> (any free port without one), for `DASHBOARD_STORE_WEB=http://127.0.0.1:<n>` (#276): the
//!       stand-in gogdl's account owns the GOG fixture's three games and Quarrel Fen, two a page; any other sign-in
//!       is refused. Says `serve-gog-web: listening on http://127.0.0.1:<n>` and then a line a request (its path and
//!       whether it came with the stand-in's sign-in), until it is killed.
//!   standin serve-discord-web [--port <n>]
//!       A stand-in discord.com (`discord_web::Web`, #197) on 127.0.0.1 at <n>, for the Dashboard's
//!       `--discord-web-url http://127.0.0.1:<n>`: a made-up sign-in form at /login that takes any e-mail and password,
//!       a session cookie, and an app page in Discord's DOM shape at /app; beside the form a made-up QR block, and
//!       `POST /standin/qr-confirm` to play the phone that scanned it and confirmed. Says `serve-discord-web: listening on
//!       http://127.0.0.1:<n>` and then a line a request (its method, path and whether it came signed in; never what
//!       was typed), until it is killed. The app page also carries a made-up Discord world for the Direct Messages tests
//!       (#198; `crates/standin/js/fake_discord.js`: the page's webpack chunk global with fake stores, five DMs, one
//!       Group DM, a conversation of varied messages), driven by controls: `POST /standin/dm-message?channel=<id>&text=<words>`
//!       (a message arrives in that DM: its unread goes up, it moves to the top), `POST /standin/unread?channel=<id>&count=<n>`,
//!       `POST /standin/ring?channel=<id>&on=1|0` (a DM call rings the user, or stops); and plain pictures at
//!       `/avatars/..` and `/channel-icons/..`, so `--discord-cdn-url` may name the same address. The channels are
//!       100000000000000001..03 (the list's links), 100000000000000010 (a Group DM) and 100000000000000011 (a named group
//!       with 60 older messages). Acting (#201): the app page has a message box while a conversation is its route; Enter
//!       in it adds Discord's own optimistic message (SENDING) to the fake store and, ~300 ms later, makes it SENT under a
//!       real id; the fake page also has Discord's reply, reaction, mark-read and local-delete actions by their shapes.
//!       Controls: `POST /standin/send-fails?on=1|0[&reason=<words>]` (the next sends go SEND_FAILED, with Discord's own
//!       dialog showing the reason), `POST /standin/captcha?on=1|0` (the next send shows a captcha frame and sends
//!       nothing), `POST /standin/slow?ms=<n>` (how long a send stays SENDING); and `GET /standin/sent`, a JSON list of
//!       what the fake received, in order: `{"kind":"message","channel","text","reply"}`, `{"kind":"react","channel",
//!       "message","emoji","on"}`, `{"kind":"read","channel"}` (made-up test data only). Voice (#297): three made-up servers,
//!       voice channels (some occupied), a fake `selectVoiceChannel` that connects 300 ms later, mute, deafen, ring and
//!       stop-ringing actions; controls `POST /standin/voice-speak?user=<id>&on=1|0`, `POST /standin/voice-join?channel=<id>&user=<id>`,
//!       `POST /standin/voice-leave[?user=<id>]`; `GET /standin/sent` also lists `{"kind":"join","channel"}`, `leave`, `mute`,
//!       `deafen`, `{"kind":"ring","channel"}`, `{"kind":"stop-ringing","channel"}`. A ring ends when answered or declined.
//!       See crates/standin/src/discord_web.rs.
//!   standin-setup-<key>.exe | standin-uninstall-<key>.exe <switches>...
//!       A launcher's installer, and the uninstaller it leaves (`set_up`): in the test's own folder that
//!       `STANDIN_SETUP` names, and nowhere else, writes what it was run with (`runs.txt`), the app's folder and its
//!       Uninstall key in the registry file standing in for Windows' (`registry.json`, `dashboard --setup-registry`);
//!       the uninstaller takes them away. Without `STANDIN_SETUP` writes nothing and ends with 3.
//!   legendary.exe | gogdl.exe [--version | <args>...]
//!       The stand-in helper, when this program is named as one (as `serve-helpers` serves it, kept in the
//!       Dashboard's data folder): with --version says `<name> <version> (stand-in)` on standard output and a line on
//!       standard error, and exits 0. As legendary, the commands the Epic source runs (`auth --code`, `status`,
//!       `install`, `uninstall`, `launch`, `egl-sync`), answered in legendary's own lines and records in the folder
//!       `LEGENDARY_CONFIG_PATH` names, and only there (`source_epic::legendary::standin`: one game, Zinnia Gate,
//!       its bytes counted out `STANDIN_HELPER_STEP_MS` apart). As gogdl, the commands the GOG source runs (`auth
//!       --code`, `auth`, `download`, `info`, `redist --ids ISI`) in gogdl's own lines and files, with `GOGDL_CONFIG_PATH`
//!       naming the test's folder (`source_gog::gogdl::standin`: any product id, named as the GOG fixture names it).
//!       With anything else says each argument a line, then `done`.
//!   scriptinterpreter.exe | unins000.exe <args>...
//!       A GOG build's script interpreter, which writes its arguments in `/DIR=`'s `standin-set-up.txt`; a GOG game's
//!       uninstaller, which empties `/dir=` but for itself. Both say their arguments in standin.log.
//!
//! It has no console on Windows, so nothing it starts shows one: what it has to say goes to `standin.log` in
//! `%E2E_OUT%`, when that is set, and `make-steam`'s failure to `standin-error.txt` beside <dir> as well.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod discord_web;
mod media;
mod set_up;
#[cfg(windows)]
mod win;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use source_steam::fixture::{self, Fixture};

const USAGE: &str = "usage: standin make-steam <dir> [--json <file>] [--full-shelf]
       standin make-epic <dir> [--json <file>] [--owning <name>]
       standin make-gog <dir> [--json <file>]
       standin epic-url <dir> com.epicgames.launcher://apps/...
       standin make-tones <dir>
       standin game [--title <t>] [--splash-ms <n>] [--seconds <n>] [--ignore-close] [--start-ms <n>] [--gap-ms <n>]
                    [--windowed <percent>] [--crash-ms <n>] [--hand-off-ms <n>]
       standin steam-url steam://rungameid/<appid> | steam://install/<appid> | steam://uninstall/<appid>
       standin make-ea | make-ubisoft | make-battlenet | make-xbox <dir> [--json <file>] [--owned]
       standin make-gamepass <dir>
       standin uplay-url <dir> uplay://launch/<id> | uplay://install/<id> | uplay://uninstall/<id>
       Agent.exe [--internalclienttools] [--seconds <n>]
       standin open-address <address>
       standin serve-art [--port <n>] [--key <key>] [--candidates <n>]
       standin serve-updates --exe <dashboard.exe> --version <x.y.z> [--port <n>] [--bad-hash] [--not-public]
       standin serve-helpers --table <file> [--port <n>] [--clients <file> --sandbox <dir>]
       standin serve-gog-web [--port <n>]
       standin serve-discord-web [--port <n>]
       standin-setup-<key>.exe | standin-uninstall-<key>.exe <switches>...
       legendary.exe | gogdl.exe [--version | <args>...]
       standin toast <text>
       standin wake
       standin media [--title <t>] [--artist <a>] [--picture <file>] [--position <s>] [--length <s>] [--paused]
                     [--no-previous] [--seconds <n>]
       standin steam-install <dir> <appid> --queued | --bytes <n>/<m> | --paused <n>/<m> | --done | --drop
       standin steam-workshop <dir> <appid> <done>/<items>
       standin steam-library <dir> <folder>
       standin steam-wizard <dir> [--agreements <appid>:<n>]... [--key <appid>]... [--free <bytes>]
       standin unlock-achievement <dir> <key>
       standin steam-change <dir> add-game | played <appid> <minutes>
       steam.exe -silent +app_install <appid> | -silent -console +app_uninstall <appid>
       steam.exe --standin-client [--seconds <n>]";

/// The file beside this program that names the fakes `open-address` takes addresses for.
const HANDLERS_FILE: &str = "handlers.json";

/// The fakes `open-address` hands each store's addresses to: their folders, from `handlers.json`.
#[derive(Debug, Default, PartialEq)]
struct Handlers {
    steam: Option<PathBuf>,
    epic: Option<PathBuf>,
    ubisoft: Option<PathBuf>,
    xbox: Option<PathBuf>,
}

impl Handlers {
    /// `{"steam": "<dir>", "epic": "<dir>", "ubisoft": "<dir>", "xbox": "<dir>"}`, any left out. PowerShell's UTF-8 can start with a
    /// byte-order mark, which is passed over.
    fn parse(text: &str) -> Result<Handlers, String> {
        let value: serde_json::Value =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{HANDLERS_FILE}: {e}"))?;
        let dir =
            |key: &str| value.get(key).and_then(|dir| dir.as_str()).filter(|dir| !dir.is_empty()).map(PathBuf::from);
        Ok(Handlers { steam: dir("steam"), epic: dir("epic"), ubisoft: dir("ubisoft"), xbox: dir("xbox") })
    }

    /// The file beside this program; none there is no fakes.
    fn beside_this_program() -> Result<Handlers, String> {
        let exe = std::env::current_exe().map_err(|e| format!("no path to this program: {e}"))?;
        let file = exe.with_file_name(HANDLERS_FILE);
        match std::fs::read_to_string(&file) {
            Ok(text) => Handlers::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Handlers::default()),
            Err(e) => Err(format!("{}: {e}", file.display())),
        }
    }
}

/// The stand-in for whatever is registered for the address's scheme, with the fakes `handlers` names.
fn open_address(handlers: &Handlers, address: &str) -> i32 {
    let address = address.trim();
    let scheme = address.split_once("://").map(|(scheme, _)| scheme.to_ascii_lowercase()).unwrap_or_default();
    let fake = |dir: &Option<PathBuf>, store: &str| {
        let found = dir.clone();
        if found.is_none() {
            note(&format!("open-address: no fake {store} in {HANDLERS_FILE} for {address}"));
        }
        found
    };
    // A packaged game's app user model ID, which Windows would activate: the fake drive's game of that ID.
    if let Some(aumid) = source_xbox::aumid_of(address) {
        return match fake(&handlers.xbox, "Xbox") {
            Some(root) => run_game("xbox", source_xbox::fake_game(&root, aumid).as_ref(), aumid),
            None => 1,
        };
    }
    // What an Xbox game's Install and Uninstall open: the Store's page, Settings' installed apps.
    if scheme == "ms-windows-store" || address.to_ascii_lowercase().starts_with("ms-settings:") {
        note(&format!("open-address: asked to open {address}"));
        return 0;
    }
    match scheme.as_str() {
        "steam" => match fake(&handlers.steam, "Steam") {
            Some(root) => steam_address(address, Some(&root)),
            None => 1,
        },
        "com.epicgames.launcher" => match fake(&handlers.epic, "Epic") {
            Some(root) => epic_url(&root, address),
            None => 1,
        },
        "uplay" => match fake(&handlers.ubisoft, "Ubisoft Connect") {
            Some(root) => uplay_url(&root, address),
            None => 1,
        },
        _ => {
            note(&format!("open-address: no stand-in for {address}"));
            2
        }
    }
}

/// The stand-in Steam given a `steam://` address: its game started from the install at `root`, or with none from
/// the one the registry names.
fn steam_address(url: &str, root: Option<&Path>) -> i32 {
    match (appid_of(url), asked_of(url)) {
        (Some(appid), _) => steam_url(appid, root),
        // The stand-in Steam installs and uninstalls nothing: that it was asked is what a test looks for.
        (None, Some((verb, appid))) => {
            note(&format!("steam-url: asked to {verb} {appid}: {}", url.trim()));
            0
        }
        _ => {
            note(&format!("steam-url: not an address the stand-in Steam knows: {url:?}"));
            2
        }
    }
}

/// The arguments a `game.exe` run with none takes instead, one a line.
const GAME_ARGS_FILE: &str = "game-args.txt";

#[derive(Debug, PartialEq)]
struct GameArgs {
    title: String,
    /// How long the launcher's small window shows first; 0 for none.
    splash_ms: u32,
    seconds: u32,
    /// Whether it stays up when its window is asked to close, as a game that has hung does.
    ignore_close: bool,
    /// How long it waits with no window at all before anything, as a game slow to start does.
    start_ms: u32,
    /// How long it waits with no window between its launcher's closing and its own opening.
    gap_ms: u32,
    /// Its window in a frame, this percent of the screen across and down, in the screen's middle; 0 for borderless
    /// over the whole screen.
    windowed: u32,
    /// How long after its window is up the process ends at once, as a crash ends it; 0 for never.
    crash_ms: u32,
    /// How long after its window is up it starts the game again as another process and exits; 0 for never.
    hand_off_ms: u32,
}

impl Default for GameArgs {
    fn default() -> Self {
        GameArgs {
            title: "Stand-in game".into(),
            splash_ms: 0,
            seconds: 600,
            ignore_close: false,
            start_ms: 0,
            gap_ms: 0,
            windowed: 0,
            crash_ms: 0,
            hand_off_ms: 0,
        }
    }
}

impl GameArgs {
    fn parse(args: &[String]) -> Result<GameArgs, String> {
        let mut out = GameArgs::default();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let mut value = || it.next().ok_or(format!("{arg} needs a value"));
            let number = |v: &String| v.parse::<u32>().map_err(|e| format!("bad {arg} {v}: {e}"));
            match arg.as_str() {
                "--title" => out.title = value()?.clone(),
                "--splash-ms" => out.splash_ms = number(value()?)?,
                "--seconds" => out.seconds = number(value()?)?,
                "--ignore-close" => out.ignore_close = true,
                "--start-ms" => out.start_ms = number(value()?)?,
                "--gap-ms" => out.gap_ms = number(value()?)?,
                "--windowed" => match number(value()?)? {
                    percent @ 10..=100 => out.windowed = percent,
                    percent => return Err(format!("--windowed {percent}: a percent of the screen from 10 to 100")),
                },
                "--crash-ms" => out.crash_ms = number(value()?)?,
                "--hand-off-ms" => out.hand_off_ms = number(value()?)?,
                _ => return Err(format!("unknown argument {arg}")),
            }
        }
        Ok(out)
    }

    /// The arguments of the game it hands off to: the same game, its window up after `gap_ms` with no launcher, and
    /// no hand-off of its own.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn handed_off(&self) -> Vec<String> {
        let mut args = vec!["game".to_string(), "--title".into(), self.title.clone()];
        args.extend(["--seconds".into(), self.seconds.to_string()]);
        if self.ignore_close {
            args.push("--ignore-close".into());
        }
        let numbers = [("--start-ms", self.gap_ms), ("--windowed", self.windowed), ("--crash-ms", self.crash_ms)];
        for (flag, value) in numbers.into_iter().filter(|&(_, value)| value > 0) {
            args.extend([flag.to_string(), value.to_string()]);
        }
        args
    }
}

/// The appid in `steam://rungameid/<appid>`.
fn appid_of(url: &str) -> Option<u32> {
    let rest = url.trim().strip_prefix("steam://rungameid/")?;
    rest.trim_end_matches('/').parse().ok().filter(|&id| id != 0)
}

/// What `steam://install/<appid>` or `steam://uninstall/<appid>` asks for, and of which game.
fn asked_of(url: &str) -> Option<(&'static str, u32)> {
    let rest = url.trim().strip_prefix("steam://")?;
    let (verb, appid) = rest.trim_end_matches('/').split_once('/')?;
    let verb = ["install", "uninstall"].into_iter().find(|&known| known == verb)?;
    appid.parse().ok().filter(|&id| id != 0).map(|id| (verb, id))
}

/// Where the Steam install at `root` has `appid` installed, by its app manifest.
fn install_dir(root: &Path, appid: u32) -> Option<PathBuf> {
    let steamapps = root.join("steamapps");
    let manifest = std::fs::read_to_string(steamapps.join(format!("appmanifest_{appid}.acf"))).ok()?;
    let manifest = source_steam::vdf::parse_text(&manifest).ok()?;
    Some(steamapps.join("common").join(manifest.str_at(&["AppState", "installdir"])?))
}

/// Writes `spec` at `dir` with this program as every installed game's `game.exe` and as the client's `steam.exe`,
/// and reads it back.
fn make_steam(dir: &Path, json: Option<&Path>, mut spec: Fixture) -> Result<(), String> {
    let dir = std::path::absolute(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let exe = std::env::current_exe().map_err(|e| format!("no path to this program: {e}"))?;
    spec.game_exe = Some(exe.clone());
    fixture::write(&dir, &spec).map_err(|e| format!("writing {}: {e}", dir.display()))?;
    // The client, which the Dashboard asks to install and uninstall (`steam_exe`), and, as the stand-in it is, to
    // pause, resume and cancel.
    let client = dir.join(STEAM_EXE);
    std::fs::copy(&exe, &client).map_err(|e| format!("{}: {e}", client.display()))?;
    let marker = dir.join(fixture::STANDIN_CLIENT);
    std::fs::write(&marker, "the stand-in Steam: its CEF debugger's port is in standin-cef-port.txt while it is up\n")
        .map_err(|e| format!("{}: {e}", marker.display()))?;
    for game in spec.games.iter().filter(|g| g.installed) {
        // Each game's window is titled with its name, so a test can tell which one is in front.
        let args = dir.join("steamapps").join("common").join(&game.install_dir).join(GAME_ARGS_FILE);
        std::fs::write(&args, format!("--title\n{}\n", game.name)).map_err(|e| format!("{}: {e}", args.display()))?;
    }

    // Read back with the reader the Dashboard uses, on the machine it will run on: a fixture it can't read should
    // fail here, not as an empty Home.
    let library = source_steam::load(&dir).map_err(|e| format!("reading {} back: {e:#}", dir.display()))?;
    if library.games.len() != spec.games.len() {
        return Err(format!("wrote {} games, read {} back", spec.games.len(), library.games.len()));
    }
    if let Some(json) = json {
        let text = serde_json::to_string_pretty(&library).map_err(|e| e.to_string())?;
        std::fs::write(json, text).map_err(|e| format!("{}: {e}", json.display()))?;
    }
    note(&format!("make-steam: {} games at {}", library.games.len(), dir.display()));
    Ok(())
}

/// The fake Steam's client, a copy of this program in its folder.
const STEAM_EXE: &str = "steam.exe";

/// What `steam-install` moves the fake Steam's install of a game to, from its arguments: `--queued`, `--bytes
/// <done>/<total>`, `--paused <done>/<total>`, `--done` or `--drop`.
fn progress_of(args: &[String]) -> Option<fixture::Progress> {
    let bytes = |text: &str| {
        let (done, total) = text.split_once('/')?;
        Some((done.parse().ok()?, total.parse().ok()?))
    };
    match args {
        [flag] if flag == "--queued" => Some(fixture::Progress::Queued),
        [flag] if flag == "--done" => Some(fixture::Progress::Done),
        [flag] if flag == "--drop" => Some(fixture::Progress::Dropped),
        [flag, n] if flag == "--bytes" => bytes(n).map(|(done, total)| fixture::Progress::Downloading { done, total }),
        [flag, n] if flag == "--paused" => bytes(n).map(|(done, total)| fixture::Progress::Paused { done, total }),
        _ => None,
    }
}

/// Moves the fake Steam's install of `appid` at `root` to `progress`, as the real client's files would show it:
/// its app manifest rewritten (`fixture::write_progress`). Done, the game's folder has this program as its
/// `game.exe`, titled with the game's name, so that it can be played. Nothing is downloaded.
fn steam_install(root: &Path, appid: u32, progress: fixture::Progress) -> Result<(), String> {
    let root = std::path::absolute(root).map_err(|e| format!("{}: {e}", root.display()))?;
    fixture::write_progress(&root, appid, progress).map_err(|e| format!("{}: {e}", root.display()))?;
    if progress == fixture::Progress::Done {
        let folder = install_dir(&root, appid).ok_or_else(|| format!("no manifest for {appid} once done"))?;
        let exe = std::env::current_exe().map_err(|e| format!("no path to this program: {e}"))?;
        std::fs::copy(&exe, folder.join("game.exe")).map_err(|e| format!("{}: {e}", folder.display()))?;
        let manifest = std::fs::read_to_string(root.join("steamapps").join(format!("appmanifest_{appid}.acf")));
        let manifest = manifest.map_err(|e| e.to_string())?;
        let name = source_steam::vdf::parse_text(&manifest)
            .ok()
            .and_then(|vdf| vdf.str_at(&["AppState", "name"]).map(str::to_string))
            .unwrap_or_else(|| format!("App {appid}"));
        title_game(&folder.join("game.exe"), &name)?;
    }
    note(&format!("steam-install: {appid} {progress:?} at {}", root.display()));
    Ok(())
}

/// The stand-in Steam client, as this program is named `steam.exe` in a fake Steam's folder: given what the
/// Dashboard gives the real one, `-silent +app_install <appid>` or `-silent -console +app_uninstall <appid>`, it
/// puts the game in its queue, Queued (its manifest written as Steam writes one as it takes an install on), or
/// uninstalls it (its manifest removed); a test then moves the install along with `steam-install`.
/// Given `--standin-client [--seconds <n>]` it is the client running: it stays up (for <n> seconds, 1800 without),
/// and while `.cef-enable-remote-debugging` is in its folder as it starts, opens its CEF debugger
/// (`source_steam::debugger`), which takes Steam's own calls (#171).
fn steam_client(args: &[String]) -> i32 {
    let root = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.to_path_buf()));
    let asked = |command: &str| {
        let at = args.iter().position(|arg| arg.eq_ignore_ascii_case(command))?;
        args.get(at + 1)?.parse::<u32>().ok().filter(|&id| id != 0)
    };
    if args.first().is_some_and(|arg| arg == "--standin-client") {
        let Some(root) = root else { return 1 };
        let seconds = match args.get(1..) {
            Some([flag, n]) if flag == "--seconds" => n.parse().unwrap_or(1800),
            _ => 1800,
        };
        return running_client(&root, seconds);
    }
    let (verb, appid, progress) = match (asked("+app_install"), asked("+app_uninstall")) {
        (Some(appid), _) => ("install", appid, fixture::Progress::Queued),
        (None, Some(appid)) => ("uninstall", appid, fixture::Progress::Dropped),
        _ => {
            note(&format!("steam: not a command the stand-in Steam knows: {args:?}"));
            return 2;
        }
    };
    note(&format!("steam: asked to {verb} {appid}: {}", args.join(" ")));
    let Some(root) = root else { return 1 };
    match fixture::write_progress(&root, appid, progress) {
        Ok(()) => 0,
        Err(e) => {
            note(&format!("steam: {}: {e}", root.display()));
            1
        }
    }
}

/// The stand-in Steam client running from the fake at `root` for `seconds`: its debugger open while the debugger's
/// file is there as it starts, as the real client's is. The Dashboard knows it is running by its process.
fn running_client(root: &Path, seconds: u64) -> i32 {
    let debugger = if source_steam::cef::enabled(root) {
        match source_steam::debugger::Debugger::start(root, |line| note(&format!("steam: {line}"))) {
            Ok(debugger) => {
                note(&format!("steam: running, its CEF debugger on 127.0.0.1:{}", debugger.port()));
                Some(debugger)
            }
            Err(e) => {
                note(&format!("steam: running, but its CEF debugger could not be opened: {e}"));
                None
            }
        }
    } else {
        note(&format!("steam: running, its CEF debugger off (no {} in its folder)", source_steam::cef::FILE));
        None
    };
    // Its main window and Install dialog, real windows as Steam's are, shown as the debugger's scripts have them.
    match &debugger {
        Some(debugger) => steam_windows(debugger, seconds),
        None => std::thread::sleep(std::time::Duration::from_secs(seconds)),
    }
    drop(debugger);
    note("steam: the running client stops");
    0
}

/// `standin steam-workshop <dir> <appid> <done>/<items>`: the fake Steam's Workshop record of the app with that many
/// of its items in (`fixture::write_workshop`).
fn steam_workshop(root: &Path, appid: u32, done: u32, items: u32) -> Result<(), String> {
    let root = std::path::absolute(root).map_err(|e| format!("{}: {e}", root.display()))?;
    fixture::write_workshop(&root, appid, done, items).map_err(|e| format!("{}: {e}", root.display()))?;
    note(&format!("steam-workshop: {appid} {done} of {items} items at {}", root.display()));
    Ok(())
}

/// Writes the fake `store` (ea, ubisoft, battlenet or xbox) at `dir`, with this program as every installed game's
/// `game.exe`, and reads it back as the Dashboard does.
fn make_store(store: &str, dir: &Path, json: Option<&Path>, owned: bool) -> Result<(), String> {
    let dir = std::path::absolute(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let exe = std::env::current_exe().map_err(|e| format!("no path to this program: {e}"))?;
    let written = |e: std::io::Error| format!("writing {}: {e}", dir.display());
    let names = |names: Vec<&str>| names.into_iter().map(str::to_string).collect::<Vec<_>>();
    let (source, expected): (Box<dyn model::Source>, Vec<String>) = match store {
        "ea" => {
            let spec = source_ea::fixture::Fixture { game_exe: Some(exe), ..source_ea::fixture::Fixture::standard() };
            source_ea::fixture::write(&dir, &spec).map_err(written)?;
            (Box::new(source_ea::Ea::at(Some(dir.clone()))), names(spec.expected()))
        }
        "ubisoft" => {
            let standard = source_ubisoft::fixture::Fixture::standard();
            let spec = source_ubisoft::fixture::Fixture { game_exe: Some(exe), ..standard };
            source_ubisoft::fixture::write(&dir, &spec).map_err(written)?;
            (Box::new(source_ubisoft::Ubisoft::at(Some(dir.clone()))), names(spec.expected()))
        }
        "battlenet" => {
            let standard = if owned {
                source_battlenet::fixture::Fixture::owned()
            } else {
                source_battlenet::fixture::Fixture::standard()
            };
            let spec = source_battlenet::fixture::Fixture { game_exe: Some(exe), ..standard };
            source_battlenet::fixture::write(&dir, &spec).map_err(written)?;
            (Box::new(source_battlenet::BattleNet::at(Some(dir.clone()))), names(spec.expected()))
        }
        "xbox" => {
            let spec =
                source_xbox::fixture::Fixture { game_exe: Some(exe), ..source_xbox::fixture::Fixture::standard() };
            source_xbox::fixture::write(&dir, &spec).map_err(written)?;
            (Box::new(source_xbox::Xbox::at(Some(dir.clone()))), names(spec.expected()))
        }
        _ => return Err(format!("no fake store {store}")),
    };
    // Read back with the reader the Dashboard uses, on the machine it will run on.
    let found = source.read().map_err(|e| format!("reading {} back: {e:#}", dir.display()))?;
    let library = found.ok_or_else(|| format!("reading {} back: no {store} there", dir.display()))?.library;
    let read: Vec<String> = library.games.iter().map(|game| game.name.clone()).collect();
    if read != expected {
        return Err(format!("wrote {expected:?}, read {read:?} back"));
    }
    read_back(&format!("make-{store}"), &dir, library, expected.len(), json)
}

/// Writes `--title <name>` as the arguments of the stand-in game `exe`, so its window is titled with the game's
/// name and a test can tell which one is in front.
fn title_game(exe: &Path, name: &str) -> Result<(), String> {
    let args = exe.with_file_name(GAME_ARGS_FILE);
    std::fs::write(&args, format!("--title\n{name}\n")).map_err(|e| format!("{}: {e}", args.display()))
}

/// Says what was read back from a fake store at `dir` (and with `json`, writes it there), if it is the `wanted`
/// number of games.
fn read_back(
    what: &str,
    dir: &Path,
    library: model::Library,
    wanted: usize,
    json: Option<&Path>,
) -> Result<(), String> {
    if library.games.len() != wanted {
        return Err(format!("wrote {wanted} games, read {} back", library.games.len()));
    }
    if let Some(json) = json {
        let text = serde_json::to_string_pretty(&library).map_err(|e| e.to_string())?;
        std::fs::write(json, text).map_err(|e| format!("{}: {e}", json.display()))?;
    }
    note(&format!("{what}: {} games at {}", library.games.len(), dir.display()));
    Ok(())
}

/// Writes Epic's standard fixture at `dir` with this program as every installed game's, and reads it back with the
/// Dashboard's reader.
fn make_epic(dir: &Path, json: Option<&Path>, owning: Option<&str>) -> Result<(), String> {
    use source_epic::fixture::{self, Fixture, Kind};
    let dir = std::path::absolute(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut spec = owning.into_iter().fold(Fixture::standard(), Fixture::owning);
    spec.game_exe = Some(std::env::current_exe().map_err(|e| format!("no path to this program: {e}"))?);
    fixture::write(&dir, &spec).map_err(|e| format!("writing {}: {e}", dir.display()))?;
    let games: Vec<_> = spec.games.iter().filter(|game| game.kind == Kind::Game).collect();
    for game in games.iter().filter(|game| game.installed) {
        title_game(&game.exe(&dir), &game.name)?;
    }
    let library = source_epic::load(&source_epic::Places::under(&dir))
        .map_err(|e| format!("reading {} back: {e:#}", dir.display()))?;
    read_back("make-epic", &dir, library, games.len(), json)
}

/// Writes GOG's standard fixture at `dir` with this program as every installed game's, and reads it back with the
/// Dashboard's reader.
fn make_gog(dir: &Path, json: Option<&Path>) -> Result<(), String> {
    use source_gog::fixture::{self, Fixture, EXE};
    let dir = std::path::absolute(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut spec = Fixture::standard();
    spec.game_exe = Some(std::env::current_exe().map_err(|e| format!("no path to this program: {e}"))?);
    fixture::write(&dir, &spec).map_err(|e| format!("writing {}: {e}", dir.display()))?;
    for game in spec.games.iter().filter(|game| game.installed) {
        title_game(&game.install_dir(&dir).join(EXE), &game.name)?;
    }
    let places = source_gog::Places::under(&dir);
    let library = source_gog::load(&places, places.records().unwrap_or_default())
        .map_err(|e| format!("reading {} back: {e:#}", dir.display()))?;
    read_back("make-gog", &dir, library, spec.games.len(), json)
}

/// The stand-in Epic launcher: starts the installed game a launch link names, from the fake launcher's files at
/// `dir`, and waits for it.
fn epic_url(dir: &Path, url: &str) -> i32 {
    let Some((action, app)) = source_epic::parse_link(url) else {
        note(&format!("epic-url: not a link the stand-in launcher knows: {url}"));
        return 2;
    };
    if action != "launch" {
        // The stand-in launcher installs nothing: that it was asked is what a test looks for.
        note(&format!("epic-url: asked to {action} {app}: {}", url.trim()));
        return 0;
    }
    let Some(exe) = source_epic::executable(&source_epic::Places::under(dir), &app) else {
        note(&format!("epic-url: {app} isn't installed in {}", dir.display()));
        return 1;
    };
    let mut command = std::process::Command::new(&exe);
    if let Some(folder) = exe.parent() {
        command.current_dir(folder);
    }
    let mut game = match command.spawn() {
        Ok(game) => game,
        Err(e) => {
            note(&format!("epic-url: could not start {}: {e}", exe.display()));
            return 1;
        }
    };
    note(&format!("epic-url: {app} running, {} pid {}", exe.display(), game.id()));
    let status = game.wait();
    note(&format!("epic-url: {app} ended ({})", status.map_or_else(|e| e.to_string(), |s| s.to_string())));
    0
}

/// `<dir> [--json <file>]`, as the make- commands take it.
fn dir_and_json(args: &[String]) -> Option<(&Path, Option<&Path>)> {
    match args {
        [dir] => Some((Path::new(dir), None)),
        [dir, flag, json] if flag == "--json" => Some((Path::new(dir), Some(Path::new(json)))),
        _ => None,
    }
}

/// A make- command's exit code: its failure said in the log, and in `standin-error.txt` beside `dir`.
/// `steam-wizard`'s options: `--agreements <appid>:<n>`, `--key <appid>`, each as often as wanted, and `--free
/// <bytes>`. None for anything else.
fn wizard_set_up(args: &[String]) -> Option<fixture::WizardSetUp> {
    let mut set_up = fixture::WizardSetUp::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let value = args.next()?;
        match arg.as_str() {
            "--agreements" => {
                let (appid, n) = value.split_once(':')?;
                set_up.agreements.insert(appid.parse().ok()?, n.parse().ok()?);
            }
            "--key" => set_up.keys.push(value.parse().ok()?),
            "--free" => set_up.free = Some(value.parse().ok()?),
            _ => return None,
        }
    }
    Some(set_up)
}

fn made(what: &str, dir: &Path, result: Result<(), String>) -> i32 {
    let Err(e) = result else { return 0 };
    note(&format!("{what}: {e}"));
    let beside = std::path::absolute(dir).ok().and_then(|d| Some(d.parent()?.join("standin-error.txt")));
    if let Some(file) = beside {
        let _ = std::fs::write(file, format!("{e}\n"));
    }
    1
}

/// What a `uplay://` address asks: launch, install or uninstall, and of which game.
fn uplay_asked(url: &str) -> Option<(&'static str, u32)> {
    let rest = url.trim().strip_prefix("uplay://")?;
    let mut parts = rest.trim_end_matches('/').split('/');
    let verb = parts.next()?;
    let verb = ["launch", "install", "uninstall"].into_iter().find(|&known| known == verb)?;
    parts.next()?.parse().ok().map(|id| (verb, id))
}

/// What the client is asked on its command line: `--exec="launch <code>"` (which Windows hands over as one
/// argument, `--exec=launch <code>`, or as two when unquoted) or `--game=<uid>`.
fn battlenet_asked(args: &[String]) -> Option<(&'static str, String)> {
    let line = args.join(" ");
    if let Some(exec) = line.split("--exec=").nth(1) {
        let code = exec.trim().trim_matches('"').strip_prefix("launch")?.trim().trim_matches('"');
        return code.split_whitespace().next().map(|code| ("launch", code.to_string()));
    }
    let uid = args.iter().find_map(|arg| arg.strip_prefix("--game="))?;
    Some(("game", uid.to_string()))
}

/// Starts the game's `game.exe` from its install folder, as its store's client would, and waits for it. `who` is
/// what the log calls the store's stand-in, `id` the game's id in the store.
fn run_game(who: &str, game: Option<&model::Game>, id: &str) -> i32 {
    let Some(dir) = game.and_then(|game| game.install_dir.clone()) else {
        note(&format!("{who}: {id} isn't installed"));
        return 1;
    };
    let exe = dir.join("game.exe");
    let mut child = match std::process::Command::new(&exe).current_dir(&dir).spawn() {
        Ok(child) => child,
        Err(e) => {
            note(&format!("{who}: could not start {}: {e}", exe.display()));
            return 1;
        }
    };
    note(&format!("{who}: {id} running, {} pid {}", exe.display(), child.id()));
    let status = child.wait();
    note(&format!("{who}: {id} ended ({})", status.map_or_else(|e| e.to_string(), |s| s.to_string())));
    0
}

/// The installed game with this `source_id` in a store's fake.
fn game_of(source: &dyn model::Source, id: &str) -> Option<model::Game> {
    let library = source.read().ok()??.library;
    library.games.into_iter().find(|game| game.source_id == id && game.installed)
}

/// The stand-in Ubisoft Connect, for the fake at `dir`.
fn uplay_url(dir: &Path, url: &str) -> i32 {
    match uplay_asked(url) {
        Some(("launch", id)) => {
            let id = id.to_string();
            run_game("uplay-url", game_of(&source_ubisoft::Ubisoft::at(Some(dir.to_path_buf())), &id).as_ref(), &id)
        }
        Some((verb, id)) => {
            note(&format!("uplay-url: asked to {verb} {id}: {}", url.trim()));
            0
        }
        None => {
            note(&format!("uplay-url: not an address the stand-in Ubisoft Connect knows: {url:?}"));
            2
        }
    }
}

/// The stand-in Battle.net client's word in its fake's folder: `<pid> starting` or `<pid> up`, written again every
/// half second while it runs, so that one killed is seen gone a few seconds on.
const CLIENT_STATE: &str = "standin-client.txt";
/// What a second `Battle.net.exe` hands the running client, a line each (`launch <code>`).
const CLIENT_ASKS: &str = "standin-client.asks";
/// How old the client's word may be and it still run.
const CLIENT_FRESH: Duration = Duration::from_secs(3);
/// How long the stand-in client runs.
const CLIENT_SECONDS: u64 = 1800;

/// The running stand-in client of the fake at `dir`: its pid, and whether it is up.
fn client_state(dir: &Path) -> Option<(u32, bool)> {
    let path = dir.join(CLIENT_STATE);
    let age = std::fs::metadata(&path).ok()?.modified().ok()?.elapsed().unwrap_or_default();
    if age > CLIENT_FRESH {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let mut words = text.split_whitespace();
    let pid = words.next()?.parse().ok()?;
    Some((pid, words.next() == Some("up")))
}

fn say_client_state(dir: &Path, up: bool) {
    let state = format!("{} {}", std::process::id(), if up { "up" } else { "starting" });
    let _ = std::fs::write(dir.join(CLIENT_STATE), state);
}

/// Hands `launch <code>` to the running client.
fn hand_over(dir: &Path, code: &str) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(CLIENT_ASKS))?;
    writeln!(file, "launch {code}")
}

/// What was handed to the client since it last looked, each code once taken.
fn take_asks(dir: &Path) -> Vec<String> {
    let taking = dir.join(format!("{CLIENT_ASKS}.taking"));
    if std::fs::rename(dir.join(CLIENT_ASKS), &taking).is_err() {
        return Vec::new();
    }
    let text = std::fs::read_to_string(&taking).unwrap_or_default();
    let _ = std::fs::remove_file(&taking);
    text.lines().filter_map(|line| line.trim().strip_prefix("launch ")).map(|code| code.trim().to_string()).collect()
}

/// The stand-in Battle.net, as this program is named in a fake Battle.net's folder, which it reads. As the real one
/// (#172): one client at a time; started, it takes `STANDIN_BATTLENET_READY_MS` (2500) to come up, then shows its
/// main window with two helpers (`Battle.net.exe --type=helper`) beside it, and a launch given to it before then is
/// dropped; a launch given to another `Battle.net.exe` once it is up is handed to it, which starts the game.
fn battlenet(args: &[String]) -> i32 {
    let Some(dir) = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.to_path_buf())) else {
        note("battlenet: its folder is not known");
        return 2;
    };
    if args.iter().any(|arg| arg.starts_with("--type=")) {
        return battlenet_helper(&dir);
    }
    let running = client_state(&dir);
    match (battlenet_asked(args), running) {
        (Some(("launch", code)), Some((pid, up))) => {
            note(&format!("battlenet: asked to launch {code}: {}", args.join(" ")));
            if !up {
                note(&format!(
                    "battlenet: the client (pid {pid}) is not up yet: launch {code} dropped, as the real client drops it"
                ));
            } else if let Err(e) = hand_over(&dir, &code) {
                note(&format!("battlenet: launch {code} not handed to the client (pid {pid}): {e}"));
                return 1;
            } else {
                note(&format!("battlenet: launch {code} handed to the running client (pid {pid})"));
            }
            0
        }
        (Some((verb, id)), _) if verb != "launch" => {
            note(&format!("battlenet: asked for {verb} {id}: {}", args.join(" ")));
            0
        }
        (asked, None) => {
            if let Some((_, code)) = asked {
                note(&format!("battlenet: asked to launch {code}: {}", args.join(" ")));
                note(&format!(
                    "battlenet: launch {code} dropped: the client is only starting, as the real one drops a launch \
                     given to it as it starts"
                ));
            }
            battlenet_client(&dir)
        }
        (_, Some((pid, _))) => {
            note(&format!("battlenet: the client is already running (pid {pid}): {args:?}"));
            0
        }
    }
}

/// The stand-in client itself: comes up, shows its window, and starts the games handed to it.
fn battlenet_client(dir: &Path) -> i32 {
    let ready = std::env::var("STANDIN_BATTLENET_READY_MS").ok().and_then(|ms| ms.parse().ok()).unwrap_or(2500);
    note(&format!("battlenet: client starting (pid {}), up in {ready} ms", std::process::id()));
    let _ = std::fs::remove_file(dir.join(CLIENT_ASKS));
    let up = std::sync::Arc::new(AtomicBool::new(false));
    let (beat_dir, beat_up) = (dir.to_path_buf(), up.clone());
    std::thread::spawn(move || loop {
        say_client_state(&beat_dir, beat_up.load(Ordering::Relaxed));
        std::thread::sleep(Duration::from_millis(500));
    });
    say_client_state(dir, false);
    std::thread::sleep(Duration::from_millis(ready));
    let mut helpers = Vec::new();
    for _ in 0..2 {
        match std::env::current_exe().and_then(|exe| std::process::Command::new(exe).arg("--type=helper").spawn()) {
            Ok(child) => helpers.push(child.id().to_string()),
            Err(e) => note(&format!("battlenet: a helper not started: {e}")),
        }
    }
    up.store(true, Ordering::Relaxed);
    say_client_state(dir, true);
    note(&format!("battlenet: client up (pid {}), its helpers pids {}", std::process::id(), helpers.join(", ")));
    let looked = dir.to_path_buf();
    battlenet_window(CLIENT_SECONDS, move || {
        for code in take_asks(&looked) {
            note(&format!("battlenet: the client launches {code}"));
            let dir = looked.clone();
            std::thread::spawn(move || match source_battlenet::fixture::client_sees(&dir, &code) {
                // As the real client: by its own view of what is installed, product.db under the uid it knows the
                // game by, never the Dashboard's (#172: a game installed under another uid shows Install).
                Ok(folder) => {
                    let game = model::Game { install_dir: Some(folder), ..model::Game::default() };
                    run_game("battlenet", Some(&game), &code);
                }
                Err(why) => note(&format!("battlenet: {why}")),
            });
        }
    });
    let _ = std::fs::remove_file(dir.join(CLIENT_STATE));
    0
}

/// A helper of the stand-in client's: there while the client is.
fn battlenet_helper(dir: &Path) -> i32 {
    let until = std::time::Instant::now() + Duration::from_secs(CLIENT_SECONDS);
    while std::time::Instant::now() < until && client_state(dir).is_some() {
        std::thread::sleep(Duration::from_millis(500));
    }
    0
}

/// Anywhere but Windows the stand-in Battle.net has no window: `each` every 100 ms for `seconds`.
#[cfg(not(windows))]
fn battlenet_window(seconds: u64, mut each: impl FnMut()) {
    let until = std::time::Instant::now() + Duration::from_secs(seconds);
    while std::time::Instant::now() < until {
        each();
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A line in `%E2E_OUT%\standin.log`, and on stderr where there is one.
fn note(message: &str) {
    eprintln!("standin: {message}");
    let Some(out) = std::env::var_os("E2E_OUT") else { return };
    let file = std::fs::OpenOptions::new().create(true).append(true).open(Path::new(&out).join("standin.log"));
    if let Ok(mut file) = file {
        let _ = writeln!(file, "[pid {}] {message}", std::process::id());
    }
}

#[cfg(not(windows))]
fn game(_: &GameArgs) -> i32 {
    note("game: Windows only");
    2
}

#[cfg(not(windows))]
fn steam_url(_: u32, _: Option<&Path>) -> i32 {
    note("steam-url: Windows only");
    2
}

#[cfg(not(windows))]
fn toast(_: &str) -> i32 {
    note("toast: Windows only");
    2
}

#[cfg(not(windows))]
fn wake() -> i32 {
    note("wake: Windows only");
    2
}

/// Anywhere but Windows the stand-in Steam has no windows: it waits out its time.
#[cfg(not(windows))]
fn steam_windows(_: &source_steam::debugger::Debugger, seconds: u64) {
    std::thread::sleep(std::time::Duration::from_secs(seconds));
}

#[cfg(windows)]
use win::{battlenet_window, game, steam_url, steam_windows, toast, wake};

fn run(args: &[String]) -> i32 {
    let parsed_game = |args: &[String]| match GameArgs::parse(args) {
        Ok(args) => game(&args),
        Err(e) => {
            note(&format!("game: {e}\n{USAGE}"));
            2
        }
    };
    match args.first().map(String::as_str) {
        Some("make-steam") => {
            let full_shelf = args.last().is_some_and(|last| last == "--full-shelf");
            let spec = if full_shelf { Fixture::full_shelf() } else { Fixture::standard() };
            let Some((dir, json)) = dir_and_json(&args[1..args.len() - usize::from(full_shelf)]) else {
                note(USAGE);
                return 2;
            };
            made("make-steam", dir, make_steam(dir, json, spec))
        }
        Some(what @ ("make-epic" | "make-gog")) => {
            // Epic's --owning <name>, last.
            let rest = &args[1..];
            let (rest, owning) = match rest {
                [before @ .., flag, name] if what == "make-epic" && flag == "--owning" => (before, Some(name.as_str())),
                _ => (rest, None),
            };
            let Some((dir, json)) = dir_and_json(rest) else {
                note(USAGE);
                return 2;
            };
            let result = if what == "make-epic" { make_epic(dir, json, owning) } else { make_gog(dir, json) };
            made(what, dir, result)
        }
        Some("epic-url") => match &args[1..] {
            [dir, url] => epic_url(Path::new(dir), url),
            _ => {
                note(USAGE);
                2
            }
        },
        Some(what @ ("make-ea" | "make-ubisoft" | "make-battlenet" | "make-xbox")) => {
            // Battle.net's --owned, last (#276).
            let owned = what == "make-battlenet" && args.last().is_some_and(|last| last == "--owned");
            let Some((dir, json)) = dir_and_json(&args[1..args.len() - usize::from(owned)]) else {
                note(USAGE);
                return 2;
            };
            made(what, dir, make_store(&what["make-".len()..], dir, json, owned))
        }
        Some("make-gamepass") => {
            let [dir] = &args[1..] else {
                note(USAGE);
                return 2;
            };
            match source_xbox::fixture::write_game_pass(Path::new(dir)) {
                Ok(()) => {
                    note(&format!("make-gamepass: a PC Game Pass catalogue for US and GB at {dir}"));
                    0
                }
                Err(e) => {
                    note(&format!("make-gamepass: {dir}: {e}"));
                    1
                }
            }
        }
        Some("uplay-url") => match &args[1..] {
            [dir, url] => uplay_url(Path::new(dir), url),
            _ => {
                note(USAGE);
                2
            }
        },
        Some("make-tones") => {
            let [dir] = &args[1..] else {
                note(USAGE);
                return 2;
            };
            let names = model::Sound::all_files();
            match audio::tones::write(Path::new(dir), &names) {
                Ok(()) => {
                    note(&format!("make-tones: {} tones in {dir}", names.len()));
                    0
                }
                Err(e) => {
                    note(&format!("make-tones: {dir}: {e}"));
                    1
                }
            }
        }
        Some("serve-art") => serve_art(&args[1..]),
        Some("serve-updates") => serve_updates(&args[1..]),
        Some("serve-helpers") => serve_helpers(&args[1..]),
        Some("serve-gog-web") => serve_gog_web(&args[1..]),
        Some("serve-discord-web") => serve_discord_web(&args[1..]),
        Some("game") => parsed_game(&args[1..]),
        Some("steam-url") => match args.get(1) {
            Some(url) => steam_address(url, None),
            None => {
                note(USAGE);
                2
            }
        },
        Some("open-address") => match &args[1..] {
            [address] => match Handlers::beside_this_program() {
                Ok(handlers) => open_address(&handlers, address),
                Err(e) => {
                    note(&format!("open-address: {e}"));
                    1
                }
            },
            _ => {
                note(USAGE);
                2
            }
        },
        Some("media") => match media::MediaArgs::parse(&args[1..]) {
            Ok(media) => media::run(&media),
            Err(e) => {
                note(&format!("media: {e}\n{USAGE}"));
                2
            }
        },
        Some("toast") => match &args[1..] {
            [text] => toast(text),
            _ => {
                note(USAGE);
                2
            }
        },
        Some("wake") => wake(),
        Some("steam-workshop") => {
            let counts = |text: &str| {
                let (done, items) = text.split_once('/')?;
                Some((done.parse::<u32>().ok()?, items.parse::<u32>().ok()?))
            };
            let parsed = match &args[1..] {
                [dir, appid, of] => appid.parse::<u32>().ok().zip(counts(of)).map(|got| (Path::new(dir), got)),
                _ => None,
            };
            let Some((dir, (appid, (done, items)))) = parsed else {
                note(USAGE);
                return 2;
            };
            made("steam-workshop", dir, steam_workshop(dir, appid, done, items))
        }
        Some("steam-library") => match &args[1..] {
            [dir, folder] => {
                let dir = Path::new(dir);
                let added = fixture::add_library(dir, Path::new(folder)).map_err(|e| format!("{}: {e}", dir.display()));
                if added.is_ok() {
                    note(&format!("steam-library: {folder} added to {}", dir.display()));
                }
                made("steam-library", dir, added)
            }
            _ => {
                note(USAGE);
                2
            }
        },
        Some("steam-wizard") => {
            let Some((dir, rest)) = args[1..].split_first() else {
                note(USAGE);
                return 2;
            };
            let Some(set_up) = wizard_set_up(rest) else {
                note(USAGE);
                return 2;
            };
            let dir = Path::new(dir);
            let written = set_up.write(dir).map_err(|e| format!("{}: {e}", dir.display()));
            if written.is_ok() {
                note(&format!("steam-wizard: {set_up:?} at {}", dir.display()));
            }
            made("steam-wizard", dir, written)
        }
        Some("steam-install") => {
            let parsed = match &args[1..] {
                [root, appid, rest @ ..] => appid.parse::<u32>().ok().zip(progress_of(rest)).map(|got| (root, got)),
                _ => None,
            };
            let Some((root, (appid, progress))) = parsed else {
                note(USAGE);
                return 2;
            };
            match steam_install(Path::new(root), appid, progress) {
                Ok(()) => 0,
                Err(e) => {
                    note(&format!("steam-install: {e}"));
                    1
                }
            }
        }
        // Changes the fake Steam's library while the Dashboard runs, as Steam's own files change.
        Some("steam-change") => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let change = match &args[1..] {
                [root, what] if what == "add-game" => Some((root, fixture::Change::AddGame)),
                [root, what, appid, minutes] if what == "played" => appid
                    .parse()
                    .ok()
                    .zip(minutes.parse().ok())
                    .map(|(appid, minutes)| (root, fixture::Change::Played { appid, minutes, at: now })),
                _ => None,
            };
            let Some((root, change)) = change else {
                note(USAGE);
                return 2;
            };
            let exe = std::env::current_exe().ok();
            match fixture::change(Path::new(root), change, exe.as_deref()) {
                Ok(()) => {
                    note(&format!("steam-change: {change:?} at {root}"));
                    0
                }
                Err(e) => {
                    note(&format!("steam-change: {root}: {e}"));
                    1
                }
            }
        }
        // Unlocks one of the fake Steam's achievements now, as Steam writes its stats file while the game runs.
        Some("unlock-achievement") => {
            let [root, key] = &args[1..] else {
                note(USAGE);
                return 2;
            };
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            match fixture::unlock_achievement(Path::new(root), key, now) {
                Ok(true) => {
                    note(&format!("unlock-achievement: {key} of {} at {now}", fixture::ACHIEVEMENTS_GAME));
                    0
                }
                Ok(false) => {
                    note(&format!("unlock-achievement: no achievement {key}, or no signed-in user, at {root}"));
                    1
                }
                Err(e) => {
                    note(&format!("unlock-achievement: {root}: {e}"));
                    1
                }
            }
        }
        // As a launcher's installer or uninstaller, in a test's own folder (#178).
        _ if set_up::named().is_some() => {
            let (install, key) = set_up::named().expect("named");
            set_up::run(install, &key, args)
        }
        // As a fake Battle.net's client, and its Agent.
        _ if named("Battle.net") => battlenet(args),
        _ if named("Agent") => match std::env::current_exe() {
            Ok(exe) => source_battlenet::standin::run(&exe, args, note),
            Err(e) => {
                note(&format!("battlenet agent: {e}"));
                2
            }
        },
        _ if named("legendary") || named("gogdl") => helper(args),
        // As a GOG build's script interpreter and a GOG game's uninstaller, as the stand-in gogdl writes them (#176).
        _ if named("scriptinterpreter") => {
            note(&format!("scriptinterpreter: {}", args.join(" ")));
            source_gog::gogdl::standin::script_interpreter(args)
        }
        _ if named("unins000") => {
            note(&format!("unins000: {}", args.join(" ")));
            source_gog::gogdl::standin::uninstaller(args)
        }
        // As a fake Steam's client.
        _ if named("steam") => steam_client(args),
        // As a game's own program, in its install folder.
        None if named("game") => {
            let beside = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join(GAME_ARGS_FILE)));
            let lines = beside.and_then(|file| std::fs::read_to_string(file).ok()).unwrap_or_default();
            let args: Vec<String> = lines.lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
            parsed_game(&args)
        }
        _ => {
            note(USAGE);
            2
        }
    }
}

/// Whether this program is running as `<name>.exe`: `game.exe`, a game's; `Battle.net.exe`, a fake Battle.net's.
fn named(name: &str) -> bool {
    std::env::current_exe().is_ok_and(|exe| exe.file_stem().is_some_and(|s| s.eq_ignore_ascii_case(name)))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
}

/// The colour of the stand-in SteamGridDB's pictures, by kind: what a test looks for on screen.
pub const ART_COLOURS: [(library::art::Kind, [u8; 3]); 4] = [
    (library::art::Kind::Cover, [0xF0, 0x80, 0x10]),
    (library::art::Kind::Hero, [0x10, 0xA0, 0xA0]),
    (library::art::Kind::Header, [0x80, 0x30, 0xD0]),
    (library::art::Kind::Square, [0xE0, 0xD0, 0x20]),
];

/// The colour of the stores' stand-ins' pictures (their own art online), by kind: unlike SteamGridDB's, so a test
/// can tell whose a picture on screen is.
pub const STORE_ART_COLOURS: [(library::art::Kind, [u8; 3]); 4] = [
    (library::art::Kind::Cover, [0xE0, 0x30, 0x30]),
    (library::art::Kind::Hero, [0x30, 0x60, 0xE0]),
    (library::art::Kind::Header, [0x30, 0xC0, 0x40]),
    (library::art::Kind::Square, [0xD0, 0x20, 0x90]),
];

/// A plain PNG of a kind's colour in `colours`, of its shape.
fn art_picture(kind: library::art::Kind, colours: &[(library::art::Kind, [u8; 3])]) -> Vec<u8> {
    use library::art::Kind;
    let (width, height) = match kind {
        Kind::Cover => (60, 90),
        Kind::Hero => (192, 62),
        Kind::Header => (92, 43),
        Kind::Square => (64, 64),
    };
    let colour = colours.iter().find(|(k, _)| *k == kind).map_or([0; 3], |(_, colour)| *colour);
    let image = image::RgbImage::from_pixel(width, height, image::Rgb(colour));
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).expect("a PNG in memory");
    png.into_inner()
}

/// One of the stand-in SteamGridDB's candidates: the first its kind's colour (`ART_COLOURS`) as the lookup takes
/// it, each after it a lighter shade, ten shades round; a thumbnail half the size.
fn candidate_picture(kind: library::art::Kind, n: usize, thumb: bool) -> Vec<u8> {
    use library::art::Kind;
    let (width, height) = match kind {
        Kind::Cover => (60, 90),
        Kind::Hero => (192, 62),
        Kind::Header => (92, 43),
        Kind::Square => (64, 64),
    };
    let (width, height) = if thumb { (width / 2, height / 2) } else { (width, height) };
    let base = ART_COLOURS.iter().find(|(k, _)| *k == kind).map_or([0; 3], |(_, colour)| *colour);
    let share = (n % 10) as f32 / 12.0;
    let colour = base.map(|c| (c as f32 + (255.0 - c as f32) * share).round() as u8);
    let image = image::RgbImage::from_pixel(width, height, image::Rgb(colour));
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).expect("a PNG in memory");
    png.into_inner()
}

fn serve_updates(args: &[String]) -> i32 {
    let (mut exe, mut version, mut port) = (None, None, 0u16);
    let (mut bad_hash, mut not_public) = (false, false);
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--bad-hash" => bad_hash = true,
            "--not-public" => not_public = true,
            "--exe" => exe = it.next().map(PathBuf::from),
            "--version" => version = it.next().and_then(|v| update::Version::parse(v)),
            "--port" => match it.next().and_then(|v| v.parse().ok()) {
                Some(value) => port = value,
                None => {
                    note("serve-updates: bad --port");
                    return 2;
                }
            },
            _ => {
                note(USAGE);
                return 2;
            }
        }
    }
    let (Some(exe), Some(version)) = (exe, version) else {
        note(&format!("serve-updates: --exe and --version are needed\n{USAGE}"));
        return 2;
    };
    let bytes = match std::fs::read(&exe) {
        Ok(bytes) => bytes,
        Err(e) => {
            note(&format!("serve-updates: {}: {e}", exe.display()));
            return 1;
        }
    };
    let script = update::fake::Script { bad_hash, not_public, ..update::fake::Script::new(version, bytes) };
    let service = match update::fake::FakeReleases::start(port, script) {
        Ok(service) => service,
        Err(e) => {
            note(&format!("serve-updates: port {port}: {e}"));
            return 1;
        }
    };
    let how = if not_public {
        ", answering 404"
    } else if bad_hash {
        ", its manifest's SHA-256 not the zip's"
    } else {
        ""
    };
    note(&format!("serve-updates: {version} of {}{how}", exe.display()));
    note(&format!("serve-updates: listening on {}", service.url()));
    let mut said = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let requests = service.requests();
        for request in &requests[said..] {
            let agent = request.user_agent.as_deref().unwrap_or("none");
            let key = if request.authorization.is_some() { "an Authorization" } else { "no Authorization" };
            note(&format!("serve-updates: {} (User-Agent {agent}, {key})", request.path));
        }
        said = requests.len();
    }
}

fn serve_helpers(args: &[String]) -> i32 {
    let (mut table, mut port, mut clients, mut sandbox) = (None, 0u16, None, None);
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--table" => table = it.next().map(PathBuf::from),
            "--clients" => clients = it.next().map(PathBuf::from),
            "--sandbox" => sandbox = it.next().map(PathBuf::from),
            "--port" => match it.next().and_then(|v| v.parse().ok()) {
                Some(value) => port = value,
                None => {
                    note("serve-helpers: bad --port");
                    return 2;
                }
            },
            _ => {
                note(USAGE);
                return 2;
            }
        }
    }
    let Some(table) = table else {
        note(&format!("serve-helpers: --table is needed\n{USAGE}"));
        return 2;
    };
    let me = std::env::current_exe().and_then(std::fs::read);
    let service = match me.and_then(|bytes| helpers::fake::FakeHelpers::start(port, bytes)) {
        Ok(service) => service,
        Err(e) => {
            note(&format!("serve-helpers: port {port}: {e}"));
            return 1;
        }
    };
    let json = serde_json::to_string_pretty(&service.table()).expect("plain fields");
    if let Err(e) = std::fs::write(&table, json) {
        note(&format!("serve-helpers: {}: {e}", table.display()));
        return 1;
    }
    note(&format!("serve-helpers: table at {}: legendary served as itself, gogdl as other bytes", table.display()));
    // The launchers' apps' table (#178): every installer this program, GOG Galaxy's other bytes.
    if let Some(clients) = &clients {
        let Some(sandbox) = sandbox.as_deref().and_then(|dir| std::path::absolute(dir).ok()) else {
            note(&format!("serve-helpers: --clients needs --sandbox\n{USAGE}"));
            return 2;
        };
        let json = serde_json::to_string_pretty(&service.client_table(&sandbox)).expect("plain fields");
        if let Err(e) = std::fs::write(clients, json) {
            note(&format!("serve-helpers: {}: {e}", clients.display()));
            return 1;
        }
        note(&format!(
            "serve-helpers: launchers' table at {}: every installer served as itself, GOG Galaxy's as other bytes",
            clients.display()
        ));
    }
    note(&format!("serve-helpers: listening on {}", service.url("").trim_end_matches("/helpers/")));
    let mut said = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let requests = service.requests();
        for request in &requests[said..] {
            let agent = request.user_agent.as_deref().unwrap_or("none");
            let key = if request.authorization.is_some() { "an Authorization" } else { "no Authorization" };
            note(&format!("serve-helpers: {} (User-Agent {agent}, {key})", request.path));
        }
        said = requests.len();
    }
}

fn serve_gog_web(args: &[String]) -> i32 {
    let port = match args {
        [] => 0,
        [flag, value] if flag == "--port" => match value.parse() {
            Ok(port) => port,
            Err(_) => {
                note("serve-gog-web: bad --port");
                return 2;
            }
        },
        _ => {
            note(USAGE);
            return 2;
        }
    };
    let web = match source_gog::gogdl::standin::Web::start(port) {
        Ok(web) => web,
        Err(e) => {
            note(&format!("serve-gog-web: port {port}: {e}"));
            return 1;
        }
    };
    note(&format!("serve-gog-web: listening on {}", web.base()));
    let mut said = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let asked = web.asked.lock().unwrap_or_else(|p| p.into_inner()).clone();
        for (path, signed) in &asked[said..] {
            let key = if *signed { "the stand-in's sign-in" } else { "no sign-in of the stand-in's" };
            note(&format!("serve-gog-web: {path} ({key})"));
        }
        said = asked.len();
    }
}

/// `serve-discord-web`: the stand-in discord.com, until killed (#197).
fn serve_discord_web(args: &[String]) -> i32 {
    let port = match args {
        [] => 0,
        [flag, value] if flag == "--port" => match value.parse() {
            Ok(port) => port,
            Err(_) => {
                note("serve-discord-web: bad --port");
                return 2;
            }
        },
        _ => {
            note(USAGE);
            return 2;
        }
    };
    let web = match discord_web::Web::start(port) {
        Ok(web) => web,
        Err(e) => {
            note(&format!("serve-discord-web: port {port}: {e}"));
            return 1;
        }
    };
    note(&format!("serve-discord-web: listening on {}", web.base()));
    let mut said = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let asked = web.asked.lock().unwrap_or_else(|p| p.into_inner()).clone();
        for line in &asked[said..] {
            note(&format!("serve-discord-web: {line}"));
        }
        said = asked.len();
    }
}

/// The stand-in helper: legendary or gogdl, as this program is named.
fn helper(args: &[String]) -> i32 {
    let name = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.file_stem()?.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let version = helpers::builtin().into_iter().find(|h| h.key == name).map(|h| h.version).unwrap_or_default();
    note(&format!("{name}: {}", if args.is_empty() { "no arguments".into() } else { args.join(" ") }));
    if args.first().map(String::as_str) == Some("--version") {
        println!("{name} {version} (stand-in)");
        eprintln!("[cli] INFO: stand-in {name}, nothing downloaded");
        return 0;
    }
    // What the Epic source runs legendary for (#175): its own lines and records.
    if name == "legendary" {
        return source_epic::legendary::standin::run(args);
    }
    // What the GOG source runs gogdl for (#176): its own lines and files.
    if name == "gogdl" {
        return source_gog::gogdl::standin::run(args);
    }
    for arg in args {
        println!("{arg}");
    }
    println!("done");
    0
}

fn serve_art(args: &[String]) -> i32 {
    let (mut port, mut key, mut candidates) = (0u16, "e2e-key".to_string(), 60usize);
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match (arg.as_str(), it.next()) {
            ("--port", Some(value)) => match value.parse() {
                Ok(value) => port = value,
                Err(_) => {
                    note(&format!("serve-art: bad --port {value}"));
                    return 2;
                }
            },
            ("--key", Some(value)) => key = value.clone(),
            ("--candidates", Some(value)) => match value.parse() {
                Ok(value) => candidates = value,
                Err(_) => {
                    note(&format!("serve-art: bad --candidates {value}"));
                    return 2;
                }
            },
            _ => {
                note(USAGE);
                return 2;
            }
        }
    }
    use library::art::fake::{CandidatePainter, FakeService, Painter, Script, StorePainter, StoreScript};
    let picture: Painter = std::sync::Arc::new(|_, kind| art_picture(kind, &ART_COLOURS));
    let store_picture: StorePainter = std::sync::Arc::new(|_, _, kind| art_picture(kind, &STORE_ART_COLOURS));
    // How rare the fake Steam's achievements are, as Steam's global percentages would say.
    let rarity = fixture::ACHIEVEMENTS.iter().map(|a| (a.key.to_string(), a.rarity)).collect();
    let rarity = vec![(fixture::ACHIEVEMENTS_GAME.to_string(), rarity)];
    let stores = StoreScript { any_id: true, rarity, ..StoreScript::new(store_picture) };
    let candidate: CandidatePainter = std::sync::Arc::new(|_, kind, n, thumb| candidate_picture(kind, n, thumb));
    let script = Script {
        any_name: true,
        stores: Some(stores),
        candidates,
        candidate: Some(candidate),
        ..Script::new(&key, picture)
    };
    let service = match FakeService::start(port, script) {
        Ok(service) => service,
        Err(e) => {
            note(&format!("serve-art: port {port}: {e}"));
            return 1;
        }
    };
    note(&format!("serve-art: listening on {}", service.url()));
    note(&format!("serve-art: the stores' own art online at {}", service.stores_url()));
    let mut said = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let requests = service.requests();
        for request in &requests[said..] {
            let keyed = request.authorization.as_deref() == Some(format!("Bearer {key}").as_str());
            let with = if request.authorization.is_none() {
                "no key"
            } else if keyed {
                "the key"
            } else {
                "another key"
            };
            note(&format!("serve-art: {} ({with})", request.path));
        }
        said = requests.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn game_arguments() {
        assert_eq!(
            GameArgs::parse(&[]).unwrap(),
            GameArgs {
                title: "Stand-in game".into(),
                splash_ms: 0,
                seconds: 600,
                ignore_close: false,
                ..Default::default()
            }
        );
        assert_eq!(
            GameArgs::parse(&strings(&["--title", "Brass Meridian", "--splash-ms", "2500", "--seconds", "30"]))
                .unwrap(),
            GameArgs { title: "Brass Meridian".into(), splash_ms: 2500, seconds: 30, ..Default::default() }
        );
        assert!(GameArgs::parse(&strings(&["--ignore-close"])).unwrap().ignore_close);
        assert!(GameArgs::parse(&strings(&["--title"])).is_err());
        assert!(GameArgs::parse(&strings(&["--seconds", "soon"])).is_err());
        assert!(GameArgs::parse(&strings(&["--fullscreen"])).is_err());
    }

    /// What the backdrop's test asks of the stand-in game: a slow start, a gap after its launcher, a frame, a crash
    /// and a hand-off.
    #[test]
    fn game_arguments_for_the_backdrop() {
        let args = strings(&[
            "--start-ms",
            "4000",
            "--splash-ms",
            "2000",
            "--gap-ms",
            "1500",
            "--windowed",
            "85",
            "--crash-ms",
            "6000",
            "--hand-off-ms",
            "3000",
        ]);
        assert_eq!(
            GameArgs::parse(&args).unwrap(),
            GameArgs {
                start_ms: 4000,
                splash_ms: 2000,
                gap_ms: 1500,
                windowed: 85,
                crash_ms: 6000,
                hand_off_ms: 3000,
                ..Default::default()
            }
        );
        assert!(GameArgs::parse(&strings(&["--windowed", "5"])).is_err(), "too small to be a game's window");
        assert!(GameArgs::parse(&strings(&["--windowed", "150"])).is_err());
    }

    /// The game a hand-off starts is the same game, up after the gap, with no launcher and no hand-off of its own:
    /// and its arguments are ones the game takes.
    #[test]
    fn a_hand_off_starts_the_same_game_after_the_gap() {
        let args = strings(&[
            "--title",
            "Brass Meridian",
            "--start-ms",
            "4000",
            "--splash-ms",
            "2000",
            "--gap-ms",
            "1500",
            "--windowed",
            "85",
            "--hand-off-ms",
            "3000",
            "--seconds",
            "20",
        ]);
        let handed = GameArgs::parse(&args).unwrap().handed_off();
        assert_eq!(handed[0], "game");
        assert_eq!(
            GameArgs::parse(&handed[1..]).unwrap(),
            GameArgs {
                title: "Brass Meridian".into(),
                seconds: 20,
                start_ms: 1500,
                windowed: 85,
                ..Default::default()
            }
        );
    }

    #[test]
    fn make_tones_writes_a_sound_for_every_file_the_dashboard_plays() {
        let dir = tempfile::tempdir().unwrap();
        let tones = dir.path().join("tones");
        assert_eq!(run(&strings(&["make-tones", tones.to_str().unwrap()])), 0);
        for name in model::Sound::all_files() {
            let clip = audio::wav::decode(&std::fs::read(tones.join(name)).unwrap()).unwrap();
            assert!(clip.seconds() > 0.1 && clip.seconds() < 0.5, "{name} is {} s", clip.seconds());
        }
        assert_eq!(run(&strings(&["make-tones"])), 2);
    }

    #[test]
    fn steam_addresses() {
        assert_eq!(appid_of("steam://rungameid/9100030"), Some(9_100_030));
        assert_eq!(appid_of("steam://rungameid/9100030/"), Some(9_100_030));
        assert_eq!(appid_of("steam://rungameid/"), None);
        assert_eq!(appid_of("steam://rungameid/0"), None);
        assert_eq!(appid_of("steam://store/9100030"), None);
        assert_eq!(appid_of("https://example.com/rungameid/5"), None);
        assert_eq!(asked_of("steam://install/9100040"), Some(("install", 9_100_040)));
        assert_eq!(asked_of("steam://uninstall/9100010/"), Some(("uninstall", 9_100_010)));
        assert_eq!(asked_of("steam://rungameid/9100030"), None);
        assert_eq!(asked_of("steam://install/0"), None);
        assert_eq!(asked_of("steam://install/"), None);
        assert_eq!(run(&strings(&["steam-url", "steam://uninstall/9100010"])), 0);
        assert_eq!(run(&strings(&["steam-url", "steam://store/9100010"])), 2);
    }

    #[test]
    fn steam_install_moves_the_fake_steams_install_along_as_the_dashboard_reads_it() {
        use model::{Install, InstallState, Source};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let mut spec = Fixture::standard();
        spec.games.truncate(6);
        make_steam(&root, None, spec).unwrap();
        assert!(root.join(STEAM_EXE).is_file());
        let steam = source_steam::Steam::at(Some(root.clone()));
        let installs = || steam.installs().unwrap();
        let path = root.to_str().unwrap();
        // Saltwind Rally, the sixth, is not installed.
        let appid = "9100060";
        let at = |state| vec![Install::new(appid, state)];
        assert_eq!(run(&strings(&["steam-install", path, appid, "--queued"])), 0);
        assert_eq!(installs(), at(InstallState::Queued));
        assert_eq!(run(&strings(&["steam-install", path, appid, "--bytes", "25/100"])), 0);
        assert_eq!(installs(), at(InstallState::Downloading { done: 25, total: 100 }));
        assert_eq!(run(&strings(&["steam-install", path, appid, "--paused", "50/100"])), 0);
        assert_eq!(installs(), at(InstallState::Paused { done: 50, total: 100 }));
        assert_eq!(run(&strings(&["steam-install", path, appid, "--done"])), 0);
        assert_eq!(installs(), []);
        let game = |steam: &source_steam::Steam| {
            let library = steam.read().unwrap().unwrap().library;
            library.games.into_iter().find(|game| game.source_id == appid).unwrap()
        };
        assert!(game(&steam).installed);
        assert!(install_dir(&root, 9_100_060).unwrap().join("game.exe").is_file());
        assert_eq!(run(&strings(&["steam-install", path, appid, "--drop"])), 0);
        assert!(!game(&steam).installed);
        // What it can't make out.
        assert_eq!(run(&strings(&["steam-install", path, appid, "--bytes", "half"])), 2);
        assert_eq!(run(&strings(&["steam-install", path, "nine", "--done"])), 2);
        assert_eq!(progress_of(&strings(&["--paused", "1/2"])), Some(fixture::Progress::Paused { done: 1, total: 2 }));
    }

    #[test]
    fn unlock_achievement_unlocks_one_of_the_fake_steams_now() {
        use model::achievement::AchievementSource;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let mut spec = Fixture::standard();
        spec.games.truncate(1);
        spec.games[0].art = fixture::Art::None;
        fixture::write(&root, &spec).unwrap();
        let path = root.to_str().unwrap();
        assert_eq!(run(&strings(&["unlock-achievement", path, "ACH_NIGHT_ROUTE"])), 0);
        assert_eq!(run(&strings(&["unlock-achievement", path, "ACH_NONE_SUCH"])), 1);
        assert_eq!(run(&strings(&["unlock-achievement", path])), 2);
        let steam = source_steam::achievements::SteamAchievements::at(Some(root));
        let read = steam.read(&fixture::ACHIEVEMENTS_GAME.to_string()).unwrap().unwrap();
        let night = read.iter().find(|a| a.key == "ACH_NIGHT_ROUTE").unwrap();
        assert!(night.unlocked.is_some_and(|at| at > fixture::NEWEST_PLAY));
    }

    #[test]
    fn make_steam_writes_an_install_the_stand_in_steam_can_start_games_from() {
        let dir = tempfile::tempdir().unwrap();
        let (root, json) = (dir.path().join("steam"), dir.path().join("library.json"));
        // The Recent row's first four, without their art: enough to start a game from, and quick to write.
        let mut spec = Fixture::standard();
        spec.games.truncate(4);
        spec.games.iter_mut().for_each(|g| g.art = fixture::Art::None);
        make_steam(&root, Some(&json), spec).unwrap();

        // Brass Meridian, the Recent row's third: installed, with this program as its game.exe and its title to show.
        let game = install_dir(&root, 9_100_030).unwrap();
        assert_eq!(game, root.join("steamapps").join("common").join("Brass Meridian"));
        assert_eq!(
            std::fs::read(game.join("game.exe")).unwrap(),
            std::fs::read(std::env::current_exe().unwrap()).unwrap()
        );
        let args: Vec<String> =
            std::fs::read_to_string(game.join(GAME_ARGS_FILE)).unwrap().lines().map(str::to_string).collect();
        assert_eq!(GameArgs::parse(&args).unwrap().title, "Brass Meridian");
        // Quiet Orbit, the fourth, isn't installed.
        assert_eq!(install_dir(&root, 9_100_040), None);

        let library: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
        assert_eq!(library["user"], "Tester");
        assert_eq!(library["games"].as_array().unwrap().len(), 4);
        assert_eq!(library["games"][2]["name"], "Brass Meridian");
        assert_eq!(library["games"][2]["installed"], true);
    }

    #[test]
    fn make_epic_and_make_gog_write_fakes_whose_games_are_this_program_titled_with_their_names() {
        let dir = tempfile::tempdir().unwrap();
        let this = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let title = |exe: &Path| {
            let args: Vec<String> = std::fs::read_to_string(exe.with_file_name(GAME_ARGS_FILE))
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect();
            GameArgs::parse(&args).unwrap().title
        };

        let (epic, json) = (dir.path().join("epic"), dir.path().join("epic.json"));
        let epic_arg = epic.to_str().unwrap();
        assert_eq!(run(&strings(&["make-epic", epic_arg, "--json", json.to_str().unwrap()])), 0);
        let library: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
        let games = library["games"].as_array().unwrap();
        assert_eq!(games.len(), 3);
        let kite = &games[0];
        assert_eq!((kite["name"].as_str(), kite["installed"].as_bool()), (Some("Kiteborne"), Some(true)));
        let app = kite["source_id"].as_str().unwrap();
        let exe = source_epic::executable(&source_epic::Places::under(&epic), app).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), this);
        assert_eq!(title(&exe), "Kiteborne");

        // The stand-in launcher: a link it doesn't know, one for a game that isn't installed, and an install, which
        // it only says.
        assert_eq!(run(&strings(&["epic-url", epic_arg, "steam://rungameid/440"])), 2);
        let wexford = games[2]["source_id"].as_str().unwrap();
        let launch = format!("com.epicgames.launcher://apps/{wexford}?action=launch&silent=true");
        assert_eq!(run(&strings(&["epic-url", epic_arg, &launch])), 1);
        assert_eq!(run(&strings(&["epic-url", epic_arg, &launch.replace("launch&silent=true", "install")])), 0);
        assert_eq!(run(&strings(&["epic-url", epic_arg])), 2);

        let gog = dir.path().join("gog");
        assert_eq!(run(&strings(&["make-gog", gog.to_str().unwrap()])), 0);
        let juniper = gog.join("Games").join("Juniper Signal").join(source_gog::fixture::EXE);
        assert_eq!(std::fs::read(&juniper).unwrap(), this);
        assert_eq!(title(&juniper), "Juniper Signal");
        assert_eq!(run(&strings(&["make-gog"])), 2);
        assert_eq!(run(&strings(&["make-epic", "a", "--yaml", "b"])), 2);

        // Owning a game besides, not installed: the same game in two launchers.
        let (owning, json) = (dir.path().join("epic-owning"), dir.path().join("epic-owning.json"));
        let args = ["make-epic", owning.to_str().unwrap(), "--json", json.to_str().unwrap(), "--owning", "Quiet Orbit"];
        assert_eq!(run(&strings(&args)), 0);
        let library: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
        let games = library["games"].as_array().unwrap();
        let quiet = games.iter().find(|game| game["name"] == "Quiet Orbit").expect("Quiet Orbit owned");
        assert_eq!((games.len(), quiet["installed"].as_bool()), (4, Some(false)));
    }

    #[test]
    fn make_store_writes_each_fake_and_reads_it_back() {
        let dir = tempfile::tempdir().unwrap();
        for store in ["ea", "ubisoft", "battlenet", "xbox"] {
            let (root, json) = (dir.path().join(store), dir.path().join(format!("{store}.json")));
            let args = strings(&[&format!("make-{store}"), root.to_str().unwrap(), "--json", json.to_str().unwrap()]);
            let why = || std::fs::read_to_string(dir.path().join("standin-error.txt")).unwrap_or_default();
            assert_eq!(run(&args), 0, "make-{store}: {}", why());
            let library: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
            let games = library["games"].as_array().unwrap();
            assert!(games.len() >= 2, "{store}: {games:?}");
            // Every installed game has this program as its game.exe.
            for game in games.iter().filter(|g| g["installed"] == true) {
                let exe = Path::new(game["install_dir"].as_str().unwrap()).join("game.exe");
                assert!(exe.is_file(), "{}", exe.display());
            }
        }
        assert!(dir.path().join("battlenet").join("Battle.net.exe").is_file());
        assert_eq!(run(&strings(&["make-ea"])), 2);
        // Battle.net with the client's catalogue and licences in its cache (#276): its owned games alone offered.
        let (root, json) = (dir.path().join("owned"), dir.path().join("owned.json"));
        let args = strings(&["make-battlenet", root.to_str().unwrap(), "--json", json.to_str().unwrap(), "--owned"]);
        assert_eq!(run(&args), 0);
        let library: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
        let names: Vec<&str> =
            library["games"].as_array().unwrap().iter().map(|g| g["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"Moorlight Saga") && !names.contains(&"Quarry Bell"), "{names:?}");
        assert!(root.join("Cache").is_dir());
    }

    #[test]
    fn uplay_and_battlenet_commands() {
        assert_eq!(uplay_asked("uplay://launch/90310"), Some(("launch", 90_310)));
        assert_eq!(uplay_asked("uplay://launch/90310/0"), Some(("launch", 90_310)));
        assert_eq!(uplay_asked("uplay://install/90410/"), Some(("install", 90_410)));
        assert_eq!(uplay_asked("uplay://uninstall/x"), None);
        assert_eq!(uplay_asked("steam://launch/5"), None);
        assert_eq!(battlenet_asked(&strings(&["--exec=launch emb"])), Some(("launch", "emb".into())));
        assert_eq!(battlenet_asked(&strings(&["--exec=launch", "emb"])), Some(("launch", "emb".into())));
        assert_eq!(battlenet_asked(&strings(&["--exec=\"launch jnv\""])), Some(("launch", "jnv".into())));
        assert_eq!(battlenet_asked(&strings(&["--game=emberwatch"])), Some(("game", "emberwatch".into())));
        assert_eq!(battlenet_asked(&strings(&["--exec=install emb"])), None);
        assert_eq!(battlenet_asked(&[]), None);

        // A game the fake does not have installed starts nothing.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ubisoft");
        assert_eq!(run(&strings(&["make-ubisoft", root.to_str().unwrap()])), 0);
        assert_eq!(uplay_url(&root, "uplay://launch/90410"), 1);
        assert_eq!(uplay_url(&root, "uplay://install/90410"), 0);
        assert_eq!(uplay_url(&root, "uplay://store/90410"), 2);
    }

    #[test]
    fn the_stand_in_battle_net_client_is_seen_running_while_its_word_is_fresh_and_takes_what_is_handed_to_it() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(client_state(dir.path()), None, "no client");
        say_client_state(dir.path(), false);
        assert_eq!(client_state(dir.path()), Some((std::process::id(), false)), "starting: a launch is dropped");
        say_client_state(dir.path(), true);
        assert_eq!(client_state(dir.path()), Some((std::process::id(), true)), "up: a launch is handed over");
        // Word gone stale: a client killed is not running.
        let stale = std::time::SystemTime::now() - CLIENT_FRESH - Duration::from_secs(1);
        let file = std::fs::File::options().write(true).open(dir.path().join(CLIENT_STATE)).unwrap();
        file.set_modified(stale).unwrap();
        assert_eq!(client_state(dir.path()), None);

        assert!(take_asks(dir.path()).is_empty());
        hand_over(dir.path(), "emb").unwrap();
        hand_over(dir.path(), "jnv").unwrap();
        assert_eq!(take_asks(dir.path()), ["emb", "jnv"]);
        assert!(take_asks(dir.path()).is_empty(), "each taken once");
    }

    #[test]
    fn handlers_json_names_each_fake_and_may_leave_any_out() {
        let both = "\u{feff}{\"steam\": \"C:\\\\Users\\\\Test User\\\\fakes\\\\steam\", \"epic\": \"D:\\\\e\"}";
        assert_eq!(
            Handlers::parse(both).unwrap(),
            Handlers {
                steam: Some(PathBuf::from(r"C:\Users\Test User\fakes\steam")),
                epic: Some(PathBuf::from(r"D:\e")),
                ubisoft: None,
                xbox: None,
            }
        );
        assert_eq!(Handlers::parse(r#"{"steam": ""}"#).unwrap(), Handlers::default());
        assert_eq!(Handlers::parse("{}").unwrap(), Handlers::default());
        assert!(Handlers::parse("steam=C:\\").is_err());
    }

    #[test]
    fn open_address_hands_each_scheme_to_its_stand_in_with_the_fake_handlers_json_names() {
        let dir = tempfile::tempdir().unwrap();
        let epic = dir.path().join("epic");
        assert_eq!(run(&strings(&["make-epic", epic.to_str().unwrap()])), 0);
        let ubisoft = dir.path().join("ubisoft");
        assert_eq!(run(&strings(&["make-ubisoft", ubisoft.to_str().unwrap()])), 0);
        let xbox = dir.path().join("xbox");
        assert_eq!(run(&strings(&["make-xbox", xbox.to_str().unwrap()])), 0);
        let handlers = Handlers {
            steam: Some(dir.path().join("steam")),
            epic: Some(epic.clone()),
            ubisoft: Some(ubisoft),
            xbox: Some(xbox),
        };

        // Steam's install and uninstall are said and done nothing more about, as steam-url does.
        assert_eq!(open_address(&handlers, "steam://install/9100040"), 0);
        assert_eq!(open_address(&handlers, " steam://uninstall/9100010/ "), 0);
        assert_eq!(open_address(&handlers, "steam://store/9100010"), 2);
        // Epic's, as epic-url does with the fake's folder: an install said, a game that isn't installed not started.
        let wexford = "com.epicgames.launcher://apps/ns%3Aitem%3AWexford?action=launch&silent=true";
        assert_eq!(open_address(&handlers, &wexford.replace("launch&silent=true", "install")), 0);
        assert_eq!(open_address(&handlers, wexford), 1);
        // Ubisoft Connect's, as uplay-url does.
        assert_eq!(open_address(&handlers, "uplay://install/90410"), 0);
        assert_eq!(open_address(&handlers, "uplay://launch/90410"), 1);
        // An Xbox game's, as Windows would activate it: an app user model ID the fake drive has no game of starts
        // nothing; the Store's page and Settings are said.
        let unknown = r"shell:AppsFolder\MadeUpStudio.Unknown_0000000000000!Game";
        assert_eq!(open_address(&handlers, unknown), 1);
        assert_eq!(open_address(&handlers, "ms-windows-store://pdp/?ProductId=9ZZZMADEUP01"), 0);
        assert_eq!(open_address(&handlers, "ms-settings:appsfeatures"), 0);

        // No fake for the store: nothing done, and never the registry's.
        let none = Handlers::default();
        assert_eq!(open_address(&none, "steam://rungameid/9100030"), 1);
        assert_eq!(open_address(&none, wexford), 1);
        assert_eq!(open_address(&none, "uplay://install/90410"), 1);
        assert_eq!(open_address(&none, unknown), 1);
        // A scheme with no stand-in, or no address at all.
        assert_eq!(open_address(&handlers, "origin2://game/launch?offerIds=1"), 2);
        assert_eq!(open_address(&handlers, "C:\\Games\\game.exe"), 2);
        assert_eq!(run(&strings(&["open-address"])), 2);
        assert_eq!(run(&strings(&["steam-url"])), 2);
    }

    #[test]
    fn make_steam_reports_a_folder_it_cannot_write() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a-file");
        std::fs::write(&file, b"").unwrap();
        assert_eq!(run(&strings(&["make-steam", file.join("steam").to_str().unwrap()])), 1);
        assert_eq!(run(&strings(&["make-steam"])), 2);
        assert_eq!(run(&strings(&["play"])), 2);
    }
}
