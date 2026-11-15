//! The command line.

pub const USAGE: &str = "\
overlay-spike [--mode guide|passive] [--cycle <n>] [--seconds <n>] [--selfcheck]
              [--visual | --hwnd-swapchain] [--gamma] [--no-layered] [--no-alt]

  --mode guide       Activatable: on show, tap Alt, then take the foreground. Esc exits. The default.
  --mode passive     Click-through and never activated: a Toast, the Launch screen, the Idle dim.
  --cycle <n>        Show for 2 s, hide for 2 s, n times, then exit. Without it the Overlay stays shown.
  --seconds <n>      Exit 0 after n seconds. The default is 10, or with --cycle as long as the cycles take.
  --selfcheck        Read three screen pixels back through GDI before showing and after the first frames.
  --visual           Make the DirectComposition device, target and visual here and hand wgpu the visual,
                     in place of wgpu's own (Dx12SwapchainKind::DxgiFromVisual).
  --hwnd-swapchain   The control: an ordinary swap chain on the window. Expected to come out opaque.
  --gamma            Does nothing now: the renderer always blends colours as encoded on a plain 8-bit target.
  --no-layered       passive without WS_EX_LAYERED: WS_EX_TRANSPARENT | WS_EX_NOACTIVATE alone.
  --no-alt           guide without the Alt tap: SetForegroundWindow alone.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Guide,
    Passive,
}

/// Where the swap chain goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    /// wgpu makes the DirectComposition visual from the window.
    WgpuVisual,
    /// The spike makes the visual and hands it to wgpu.
    OwnVisual,
    /// A swap chain on the window itself, with no DirectComposition.
    Hwnd,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub mode: Mode,
    pub cycle: u32,
    pub seconds: Option<f64>,
    pub selfcheck: bool,
    pub surface: SurfaceKind,
    pub gamma: bool,
    pub layered: bool,
    pub alt: bool,
}

impl Args {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut out = Self {
            mode: Mode::Guide,
            cycle: 0,
            seconds: None,
            selfcheck: false,
            surface: SurfaceKind::WgpuVisual,
            gamma: false,
            layered: true,
            alt: true,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
            match arg.as_str() {
                "--mode" => {
                    out.mode = match value()?.as_str() {
                        "guide" => Mode::Guide,
                        "passive" => Mode::Passive,
                        other => return Err(format!("unknown mode {other}")),
                    }
                }
                "--cycle" => out.cycle = value()?.parse().map_err(|e| format!("--cycle: {e}"))?,
                "--seconds" => {
                    let seconds: f64 = value()?.parse().map_err(|e| format!("--seconds: {e}"))?;
                    if !(seconds > 0.0 && seconds.is_finite()) {
                        return Err("--seconds must be more than 0".into());
                    }
                    out.seconds = Some(seconds);
                }
                "--selfcheck" => out.selfcheck = true,
                "--visual" => out.surface = SurfaceKind::OwnVisual,
                "--hwnd-swapchain" => out.surface = SurfaceKind::Hwnd,
                "--gamma" => out.gamma = true,
                "--no-layered" => out.layered = false,
                "--no-alt" => out.alt = false,
                other => return Err(format!("unknown argument {other}")),
            }
        }
        Ok(out)
    }

    /// How long to run when nothing else ends it: `--seconds`, or 10 s when there are no cycles to wait for.
    pub fn run_seconds(&self) -> Option<f64> {
        self.seconds.or((self.cycle == 0).then_some(10.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Args, String> {
        Args::parse(line.split_whitespace().map(String::from))
    }

    #[test]
    fn the_default_is_an_activatable_guide_for_ten_seconds() {
        let args = parse("").unwrap();
        assert_eq!(args.mode, Mode::Guide);
        assert_eq!(args.surface, SurfaceKind::WgpuVisual);
        assert!(args.layered && args.alt && !args.gamma && !args.selfcheck);
        assert_eq!(args.run_seconds(), Some(10.0));
    }

    #[test]
    fn cycles_run_until_they_are_done_unless_seconds_cuts_them_short() {
        assert_eq!(parse("--cycle 3").unwrap().run_seconds(), None);
        assert_eq!(parse("--cycle 3 --seconds 5").unwrap().run_seconds(), Some(5.0));
    }

    #[test]
    fn every_switch_is_read() {
        let args = parse("--mode passive --cycle 2 --selfcheck --visual --gamma --no-layered --no-alt").unwrap();
        assert_eq!(args.mode, Mode::Passive);
        assert_eq!(args.cycle, 2);
        assert_eq!(args.surface, SurfaceKind::OwnVisual);
        assert!(args.selfcheck && args.gamma && !args.layered && !args.alt);
        assert_eq!(parse("--hwnd-swapchain").unwrap().surface, SurfaceKind::Hwnd);
    }

    #[test]
    fn mistakes_are_refused() {
        assert!(parse("--mode toast").is_err());
        assert!(parse("--cycle").is_err());
        assert!(parse("--seconds 0").is_err());
        assert!(parse("--screenshot").is_err());
    }
}
