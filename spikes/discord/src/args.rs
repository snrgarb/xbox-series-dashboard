//! The command line: one step, and where its files are. Every file defaults to the exe's own folder, and a relative
//! path is taken from there too (a scheduled task starts in System32), so a task on the TV needs no more than the
//! step and short names.
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: discord-spike <step> [options]
steps (exactly one):
  --probe                 is the WebView2 runtime here; can a session be made in --data
  --sign-in               discord.com/login full screen, to sign in with the pad; ends on --quit or --minutes
  --dms                   hidden: the DM list as the page shows it; memory and idle CPU
  --send --to-file <f> --text-file <f>
                          opens the conversation named in <f> and sends the text in the other, once
  --sign-out              removes the session folder
  --voice-probe           hidden: microphone permission, devices, getUserMedia, an AudioContext and a 1 s timer
                          kept for --hold s; one hiding (--hide) and one switch set (--args-set) per run
  --voice-list            hidden: DM rows and servers counted; with --server-file, that server's voice channels
                          (names to --voice-names, never the log)
  --voice-join --server-file <f> --channel-file <f>
                          joins that server's voice channel named in <f>, once; holds --hold s; always leaves
  --call --to-file <f>    starts a voice call in the DM named in <f>, once; holds --hold s; always leaves
  --voice-incoming        waits --hold s for an incoming call; the answer / decline control files press its buttons
  --dump-shape [--server-file <f>]
                          writes the page's shape (tags, roles, classes, Discord UI words of labels, icon hashes;
                          no names) of the channel list, user panel, voice panel and call area to --shape
options (a relative path is from the exe's folder):
  --data <dir>            the session (WebView2's user data folder)  [<exe dir>\\session]
  --log <file>            appended to                                  [<exe dir>\\discord-spike.log]
  --quit <file>           --sign-in ends when this file appears        [<exe dir>\\quit]
  --minutes <n>           --sign-in closes after n minutes; any step gives up after it   [20]
  --online                --probe also loads discord.com/login, hidden
  --keyboard <k>          --sign-in's keyboard: pad (the spike's own), tabtip, osk, none  [pad]
  --names <file>          --dms writes the conversations' names here   [<exe dir>\\dm-names.txt]
  --wait-seconds <n>      --dms waits this long before measuring idle CPU  [60]
  --idle-seconds <n>      --dms measures idle CPU over this long          [30]
  --hide <how>            hidden steps: controller (the window never shown, the web view told it is hidden),
                          offscreen (a shown window off every screen, the web view told it is visible), or
                          visible-unshown (the window never shown, the web view told it is visible)  [controller]
  --visible               --dms, --send: full screen instead of hidden
  --browser-args <s>      extra Chromium switches for the session's browser
  --args-set <a|b>        a: no extra switches; b: the four that keep a background page running  [a]
  --hold <n>              the voice steps hold, logging once a second, this many seconds  [60]
  --way <w>               how the voice steps press Discord's buttons: auto (key, then click, then route, each
                          only if the one before showed nothing), key, click, route  [auto]
  --voice-names <file>    --voice-list writes server and voice channel names here  [<exe dir>\\voice-names.txt]
  --index                 builds the index (servers, channels, DMs, voice states) from Discord's in-page stores,
                          read-only; counts and timings to the log, names and ids to --index-file
  --shape <file>          where the shape goes                          [<exe dir>\\shape.txt]
  --index-file <file>     the index's names and ids                     [<exe dir>\\index.txt]
  --dir <dir>             the folder every relative name is taken from  [the exe's folder]
  --args-file <f>         more arguments from <f>, one a line (# lines skipped): for long command lines
control files, made in --dir while a voice step holds: mute, deafen, leave, answer, decline, shape (each acted on
once, then removed)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Probe,
    SignIn,
    Dms,
    Send,
    SignOut,
    VoiceProbe,
    VoiceList,
    VoiceJoin,
    Call,
    VoiceIncoming,
    DumpShape,
    Index,
}

impl Step {
    pub fn name(self) -> &'static str {
        match self {
            Step::Probe => "probe",
            Step::SignIn => "sign-in",
            Step::Dms => "dms",
            Step::Send => "send",
            Step::SignOut => "sign-out",
            Step::VoiceProbe => "voice-probe",
            Step::VoiceList => "voice-list",
            Step::VoiceJoin => "voice-join",
            Step::Call => "call",
            Step::VoiceIncoming => "voice-incoming",
            Step::DumpShape => "dump-shape",
            Step::Index => "index",
        }
    }

    /// The voice steps (#292) that hold for `--hold` seconds.
    pub fn holds(self) -> bool {
        matches!(self, Step::VoiceProbe | Step::VoiceJoin | Step::Call | Step::VoiceIncoming)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyboard {
    Pad,
    TabTip,
    Osk,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hide {
    /// The window is never shown and the controller is told it is not visible.
    Controller,
    /// The window is shown, without activating, at a point off every screen; the controller thinks it is visible.
    Offscreen,
    /// The window is never shown, but the controller is told it is visible (`put_IsVisible(true)`).
    VisibleUnshown,
}

impl Hide {
    pub fn name(self) -> &'static str {
        match self {
            Hide::Controller => "controller",
            Hide::Offscreen => "offscreen",
            Hide::VisibleUnshown => "visible-unshown",
        }
    }
}

/// `--args-set b`: the Chromium switches that keep a page in the background from being slowed down or its audio
/// from waiting for a gesture.
pub const ARGS_SET_B: &str = "--disable-background-timer-throttling --disable-renderer-backgrounding \
--disable-backgrounding-occluded-windows --autoplay-policy=no-user-gesture-required";

/// How a voice step presses one of Discord's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// Key, then click, then route: each only if the one before showed nothing.
    Auto,
    /// DevTools' `DOM.focus` on the button, then Enter as a keyboard makes it.
    Key,
    /// A DevTools mouse click at the button's centre.
    Click,
    /// A voice channel's own address by `history.pushState`, then its Join Voice button by key.
    Route,
}

impl Way {
    pub fn name(self) -> &'static str {
        match self {
            Way::Auto => "auto",
            Way::Key => "key",
            Way::Click => "click",
            Way::Route => "route",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub step: Step,
    pub data: PathBuf,
    pub log: PathBuf,
    pub quit: PathBuf,
    pub minutes: u64,
    pub online: bool,
    pub keyboard: Keyboard,
    pub names: PathBuf,
    pub wait_seconds: u64,
    pub idle_seconds: u64,
    pub hide: Hide,
    pub visible: bool,
    pub to_file: Option<PathBuf>,
    pub text_file: Option<PathBuf>,
    pub browser_args: Option<String>,
    /// `--args-set b`: [`ARGS_SET_B`] added to the browser's switches.
    pub args_set_b: bool,
    pub server_file: Option<PathBuf>,
    pub channel_file: Option<PathBuf>,
    pub hold: u64,
    pub way: Way,
    pub voice_names: PathBuf,
    /// Where the control files (`mute`, `deafen`, `leave`, `answer`, `decline`) appear: the exe's folder.
    pub control: PathBuf,
    /// `--dump-shape` and the `shape` control file write the page's shape here.
    pub shape: PathBuf,
    /// `--index` (and the voice steps that resolve by it) write the index's names and ids here.
    pub index_file: PathBuf,
}

impl Options {
    /// The switches the browser starts with: `--browser-args`, then `--args-set b`'s.
    pub fn switches(&self) -> Option<String> {
        let set = self.args_set_b.then_some(ARGS_SET_B);
        match (self.browser_args.as_deref(), set) {
            (Some(a), Some(b)) => Some(format!("{a} {b}")),
            (Some(a), None) => Some(a.to_string()),
            (None, Some(b)) => Some(b.to_string()),
            (None, None) => None,
        }
    }
}

pub fn parse(args: &[String]) -> Result<Options, String> {
    let exe = std::env::current_exe().map_err(|e| format!("where is this exe? {e}"))?;
    parse_in(args, exe.parent().unwrap_or(Path::new(".")))
}

/// `--dir <d>` (the folder relative names are taken from, itself from the exe's folder) and `--args-file <f>` (more
/// arguments, one a line, `#` lines skipped: for command lines longer than schtasks' 261 characters), resolved first.
fn expand(args: &[String], exe_dir: &Path) -> Result<(Vec<String>, PathBuf), String> {
    let mut base = exe_dir.to_path_buf();
    if let Some(i) = args.iter().position(|a| a == "--dir") {
        let dir = args.get(i + 1).ok_or("--dir needs a value")?;
        base = exe_dir.join(dir);
    }
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dir" => {
                it.next();
            }
            "--args-file" => {
                let path = base.join(it.next().ok_or("--args-file needs a value")?);
                let text =
                    std::fs::read_to_string(&path).map_err(|e| format!("--args-file {}: {e}", path.display()))?;
                let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
                for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                    if line == "--args-file" || line == "--dir" {
                        return Err(format!("{line} cannot come from an --args-file"));
                    }
                    out.push(line.to_string());
                }
            }
            _ => out.push(arg.clone()),
        }
    }
    Ok((out, base))
}

pub fn parse_in(args: &[String], exe_dir: &Path) -> Result<Options, String> {
    let (args, base) = expand(args, exe_dir)?;
    let (args, base) = (args.as_slice(), base.as_path());
    let mut step = None;
    let mut options = Options {
        step: Step::Probe,
        data: base.join("session"),
        log: base.join("discord-spike.log"),
        quit: base.join("quit"),
        minutes: 20,
        online: false,
        keyboard: Keyboard::Pad,
        names: base.join("dm-names.txt"),
        wait_seconds: 60,
        idle_seconds: 30,
        hide: Hide::Controller,
        visible: false,
        to_file: None,
        text_file: None,
        browser_args: None,
        args_set_b: false,
        server_file: None,
        channel_file: None,
        hold: 60,
        way: Way::Auto,
        voice_names: base.join("voice-names.txt"),
        control: base.to_path_buf(),
        shape: base.join("shape.txt"),
        index_file: base.join("index.txt"),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or_else(|| format!("{name} needs a value"));
        let mut set_step = |s: Step| match step.replace(s) {
            Some(other) => Err(format!("one step at a time: --{} and --{}", other.name(), s.name())),
            None => Ok(()),
        };
        match arg.as_str() {
            "--probe" => set_step(Step::Probe)?,
            "--sign-in" => set_step(Step::SignIn)?,
            "--dms" => set_step(Step::Dms)?,
            "--send" => set_step(Step::Send)?,
            "--sign-out" => set_step(Step::SignOut)?,
            "--voice-probe" => set_step(Step::VoiceProbe)?,
            "--voice-list" => set_step(Step::VoiceList)?,
            "--voice-join" => set_step(Step::VoiceJoin)?,
            "--call" => set_step(Step::Call)?,
            "--voice-incoming" => set_step(Step::VoiceIncoming)?,
            "--dump-shape" => set_step(Step::DumpShape)?,
            "--index" => set_step(Step::Index)?,
            "--index-file" => options.index_file = base.join(value("--index-file")?),
            "--shape" => options.shape = base.join(value("--shape")?),
            "--data" => options.data = base.join(value("--data")?),
            "--log" => options.log = base.join(value("--log")?),
            "--quit" => options.quit = base.join(value("--quit")?),
            "--names" => options.names = base.join(value("--names")?),
            "--voice-names" => options.voice_names = base.join(value("--voice-names")?),
            "--to-file" => options.to_file = Some(base.join(value("--to-file")?)),
            "--text-file" => options.text_file = Some(base.join(value("--text-file")?)),
            "--server-file" => options.server_file = Some(base.join(value("--server-file")?)),
            "--channel-file" => options.channel_file = Some(base.join(value("--channel-file")?)),
            "--browser-args" => options.browser_args = Some(value("--browser-args")?),
            "--minutes" => options.minutes = number("--minutes", &value("--minutes")?)?,
            "--wait-seconds" => options.wait_seconds = number("--wait-seconds", &value("--wait-seconds")?)?,
            "--idle-seconds" => options.idle_seconds = number("--idle-seconds", &value("--idle-seconds")?)?,
            "--hold" => options.hold = number("--hold", &value("--hold")?)?,
            "--online" => options.online = true,
            "--visible" => options.visible = true,
            "--keyboard" => {
                options.keyboard = match value("--keyboard")?.as_str() {
                    "pad" => Keyboard::Pad,
                    "tabtip" => Keyboard::TabTip,
                    "osk" => Keyboard::Osk,
                    "none" => Keyboard::None,
                    other => return Err(format!("--keyboard {other}: pad, tabtip, osk or none")),
                }
            }
            "--hide" => {
                options.hide = match value("--hide")?.as_str() {
                    "controller" => Hide::Controller,
                    "offscreen" => Hide::Offscreen,
                    "visible-unshown" => Hide::VisibleUnshown,
                    other => return Err(format!("--hide {other}: controller, offscreen or visible-unshown")),
                }
            }
            "--args-set" => {
                options.args_set_b = match value("--args-set")?.as_str() {
                    "a" => false,
                    "b" => true,
                    other => return Err(format!("--args-set {other}: a or b")),
                }
            }
            "--way" => {
                options.way = match value("--way")?.as_str() {
                    "auto" => Way::Auto,
                    "key" => Way::Key,
                    "click" => Way::Click,
                    "route" => Way::Route,
                    other => return Err(format!("--way {other}: auto, key, click or route")),
                }
            }
            other => return Err(format!("not known: {other}")),
        }
    }
    options.step = step.ok_or("no step given")?;
    let step = options.step;
    if step == Step::Send && (options.to_file.is_none() || options.text_file.is_none()) {
        return Err("--send needs both --to-file and --text-file".into());
    }
    if step == Step::Call && (options.to_file.is_none() || options.text_file.is_some()) {
        return Err("--call needs --to-file, and no --text-file".into());
    }
    if !matches!(step, Step::Send | Step::Call) && (options.to_file.is_some() || options.text_file.is_some()) {
        return Err("--to-file and --text-file are for --send and --call only".into());
    }
    if step == Step::VoiceJoin && (options.server_file.is_none() || options.channel_file.is_none()) {
        return Err("--voice-join needs both --server-file and --channel-file".into());
    }
    if !matches!(step, Step::VoiceJoin | Step::VoiceList | Step::DumpShape | Step::Index)
        && options.server_file.is_some()
    {
        return Err("--server-file is for --voice-list, --voice-join, --dump-shape and --index only".into());
    }
    if step != Step::VoiceJoin && options.channel_file.is_some() {
        return Err("--channel-file is for --voice-join only".into());
    }
    if options.minutes == 0 {
        return Err("--minutes must be at least 1".into());
    }
    if options.hold == 0 {
        return Err("--hold must be at least 1".into());
    }
    // The watchdog ends the process --minutes + 2 after the start: a hold must end, and leave, well before it.
    if step.holds() && options.hold + 240 > options.minutes * 60 {
        return Err(format!(
            "--hold {} needs --minutes {} at least (the hold, and 4 min to load, join and leave)",
            options.hold,
            (options.hold + 240).div_ceil(60)
        ));
    }
    Ok(options)
}

fn number(name: &str, value: &str) -> Result<u64, String> {
    value.parse().map_err(|_| format!("{name} {value}: a whole number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Options, String> {
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        parse_in(&args, Path::new("/spike"))
    }

    #[test]
    fn a_step_alone_takes_every_file_beside_the_exe() {
        let o = parse("--dms").unwrap();
        assert_eq!(o.step, Step::Dms);
        assert_eq!(o.data, Path::new("/spike/session"));
        assert_eq!(o.log, Path::new("/spike/discord-spike.log"));
        assert_eq!(o.names, Path::new("/spike/dm-names.txt"));
        assert_eq!((o.wait_seconds, o.idle_seconds, o.minutes), (60, 30, 20));
        assert_eq!(o.hide, Hide::Controller);
    }

    #[test]
    fn options_are_read() {
        let o = parse("--sign-in --data D --keyboard tabtip --minutes 5 --quit Q").unwrap();
        assert_eq!((o.step, o.keyboard, o.minutes), (Step::SignIn, Keyboard::TabTip, 5));
        assert_eq!((o.data.as_path(), o.quit.as_path()), (Path::new("/spike/D"), Path::new("/spike/Q")));
        assert_eq!(parse("--dms --data /elsewhere").unwrap().data, Path::new("/elsewhere"), "absolute stays");
        assert_eq!(parse("--dms --hide offscreen --visible").unwrap().hide, Hide::Offscreen);
    }

    #[test]
    fn one_step_exactly() {
        assert!(parse("").is_err());
        assert!(parse("--dms --probe").is_err());
        assert!(parse("--data D").is_err());
        assert!(parse("--voice-probe --voice-list").is_err());
    }

    #[test]
    fn send_only_with_both_files() {
        assert!(parse("--send").is_err());
        assert!(parse("--send --to-file T").is_err());
        let o = parse("--send --to-file T --text-file X").unwrap();
        assert_eq!(o.to_file.as_deref(), Some(Path::new("/spike/T")));
        assert!(parse("--dms --to-file T --text-file X").is_err(), "the files never come with another step");
    }

    #[test]
    fn voice_steps_take_their_files() {
        let o = parse("--voice-join --server-file S --channel-file C").unwrap();
        assert_eq!(o.step, Step::VoiceJoin);
        assert_eq!(o.server_file.as_deref(), Some(Path::new("/spike/S")));
        assert_eq!(o.channel_file.as_deref(), Some(Path::new("/spike/C")));
        assert_eq!((o.hold, o.way, o.control.as_path()), (60, Way::Auto, Path::new("/spike")));
        assert!(parse("--voice-join --server-file S").is_err());
        assert!(parse("--voice-join --channel-file C").is_err());
        assert!(parse("--call").is_err());
        assert_eq!(parse("--call --to-file T").unwrap().to_file.as_deref(), Some(Path::new("/spike/T")));
        assert!(parse("--call --to-file T --text-file X").is_err());
        assert!(parse("--voice-list --server-file S").is_ok());
        assert!(parse("--voice-list --channel-file C").is_err());
        assert!(parse("--dms --server-file S").is_err());
        assert_eq!(parse("--voice-list").unwrap().voice_names, Path::new("/spike/voice-names.txt"));
    }

    #[test]
    fn the_hold_fits_inside_the_watchdog() {
        assert!(parse("--voice-incoming --hold 900").is_ok(), "900 s and 4 min fit in 20 min");
        assert!(parse("--voice-incoming --hold 1000").is_err(), "1000 s and 4 min do not");
        assert!(parse("--voice-incoming --hold 1000 --minutes 21").is_ok());
        assert!(parse("--voice-probe --hold 0").is_err());
        assert!(parse("--voice-list --hold 5000").is_ok(), "the list does not hold");
    }

    #[test]
    fn switch_sets_ways_and_hidings() {
        assert_eq!(parse("--voice-probe").unwrap().switches(), None);
        let b = parse("--voice-probe --args-set b").unwrap();
        assert_eq!(b.switches().as_deref(), Some(ARGS_SET_B));
        assert!(ARGS_SET_B.contains("--disable-renderer-backgrounding --disable-backgrounding-occluded-windows"));
        let both = parse("--voice-probe --args-set b --browser-args --x").unwrap();
        assert_eq!(both.switches(), Some(format!("--x {ARGS_SET_B}")));
        assert!(parse("--voice-probe --args-set c").is_err());
        assert_eq!(parse("--voice-probe --hide visible-unshown").unwrap().hide, Hide::VisibleUnshown);
        assert_eq!(parse("--voice-join --server-file S --channel-file C --way click").unwrap().way, Way::Click);
        assert!(parse("--voice-probe --way swipe").is_err());
    }

    #[test]
    fn dir_and_args_file() {
        let o = parse("--dms --dir D").unwrap();
        assert_eq!((o.data.as_path(), o.control.as_path()), (Path::new("/spike/D/session"), Path::new("/spike/D")));
        let dir = std::env::temp_dir().join(format!("discord-spike-args-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.txt"),
            "\u{feff}# the join\n--voice-join\n--server-file\nmy server.txt\n\n--hold\n120\n",
        )
        .unwrap();
        let args = ["--args-file".to_string(), "a.txt".into(), "--channel-file".into(), "c.txt".into()];
        let o = parse_in(&args, &dir).unwrap();
        assert_eq!(o.step, Step::VoiceJoin);
        assert_eq!(o.server_file, Some(dir.join("my server.txt")), "a line is one argument, spaces and all");
        assert_eq!((o.hold, o.channel_file), (120, Some(dir.join("c.txt"))));
        std::fs::write(dir.join("b.txt"), "--args-file\nb.txt\n").unwrap();
        assert!(parse_in(&["--args-file".to_string(), "b.txt".into()], &dir).is_err(), "no nesting");
        assert!(parse("--dms --args-file missing.txt").is_err());
        assert!(parse("--dms --dir").is_err());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(parse("--dump-shape --server-file S").unwrap().shape, Path::new("/spike/shape.txt"));
    }

    #[test]
    fn bad_values_are_refused() {
        assert!(parse("--dms --minutes").is_err());
        assert!(parse("--dms --minutes ten").is_err());
        assert!(parse("--dms --minutes 0").is_err());
        assert!(parse("--sign-in --keyboard swype").is_err());
        assert!(parse("--dms --password x").is_err());
    }
}
