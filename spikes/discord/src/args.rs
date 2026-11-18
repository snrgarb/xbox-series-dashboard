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
  --hide <how>            --dms, --send: controller (the web view told it is hidden) or
                          offscreen (a shown window off every screen)  [controller]
  --visible               --dms, --send: full screen instead of hidden
  --browser-args <s>      extra Chromium switches for the session's browser";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Probe,
    SignIn,
    Dms,
    Send,
    SignOut,
}

impl Step {
    pub fn name(self) -> &'static str {
        match self {
            Step::Probe => "probe",
            Step::SignIn => "sign-in",
            Step::Dms => "dms",
            Step::Send => "send",
            Step::SignOut => "sign-out",
        }
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
}

pub fn parse(args: &[String]) -> Result<Options, String> {
    let exe = std::env::current_exe().map_err(|e| format!("where is this exe? {e}"))?;
    parse_in(args, exe.parent().unwrap_or(Path::new(".")))
}

pub fn parse_in(args: &[String], base: &Path) -> Result<Options, String> {
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
            "--data" => options.data = base.join(value("--data")?),
            "--log" => options.log = base.join(value("--log")?),
            "--quit" => options.quit = base.join(value("--quit")?),
            "--names" => options.names = base.join(value("--names")?),
            "--to-file" => options.to_file = Some(base.join(value("--to-file")?)),
            "--text-file" => options.text_file = Some(base.join(value("--text-file")?)),
            "--browser-args" => options.browser_args = Some(value("--browser-args")?),
            "--minutes" => options.minutes = number("--minutes", &value("--minutes")?)?,
            "--wait-seconds" => options.wait_seconds = number("--wait-seconds", &value("--wait-seconds")?)?,
            "--idle-seconds" => options.idle_seconds = number("--idle-seconds", &value("--idle-seconds")?)?,
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
                    other => return Err(format!("--hide {other}: controller or offscreen")),
                }
            }
            other => return Err(format!("not known: {other}")),
        }
    }
    options.step = step.ok_or("no step given")?;
    if options.step == Step::Send && (options.to_file.is_none() || options.text_file.is_none()) {
        return Err("--send needs both --to-file and --text-file".into());
    }
    if options.step != Step::Send && (options.to_file.is_some() || options.text_file.is_some()) {
        return Err("--to-file and --text-file are for --send only".into());
    }
    if options.minutes == 0 {
        return Err("--minutes must be at least 1".into());
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
    fn bad_values_are_refused() {
        assert!(parse("--dms --minutes").is_err());
        assert!(parse("--dms --minutes ten").is_err());
        assert!(parse("--dms --minutes 0").is_err());
        assert!(parse("--sign-in --keyboard swype").is_err());
        assert!(parse("--dms --password x").is_err());
    }
}
