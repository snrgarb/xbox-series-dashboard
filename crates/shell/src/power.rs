//! Power's options that are the machine's to do: turn off, restart, sleep (docs/spec/guide.md, section 6.2). They
//! are done as the Bundle does them, and only by [`SystemPower`], which the executable makes only when it runs for
//! real in its window. Everything else (a test, an offscreen frame, a run with `--dry-power`, any machine that is
//! not Windows) has a [`Power`] that says what it would have done and does nothing.
pub use model::PowerCommand;

use crate::since_start;

/// Whoever carries out what Power asks of the machine.
pub trait Power {
    fn carry_out(&mut self, command: PowerCommand);
}

/// The program the Bundle runs for a command, and its arguments: a hybrid shutdown, which is what Windows' own
/// Start menu does, and a plain restart. Sleep is a call, not a program.
pub fn command_line(command: PowerCommand) -> Option<(&'static str, &'static [&'static str])> {
    match command {
        PowerCommand::TurnOff => Some(("shutdown.exe", &["/s", "/hybrid", "/t", "0"])),
        PowerCommand::Restart => Some(("shutdown.exe", &["-r", "-t", "0"])),
        PowerCommand::Sleep => None,
    }
}

/// What a command does, for the log.
fn describe(command: PowerCommand) -> String {
    match command_line(command) {
        Some((program, arguments)) => format!("{program} {}", arguments.join(" ")),
        None => "SetSuspendState(sleep)".into(),
    }
}

/// Says what it was asked and does nothing: `power: would run ...`. It keeps what it was asked, for a test to read.
#[derive(Debug, Default)]
pub struct DryPower {
    pub asked: Vec<PowerCommand>,
}

impl Power for DryPower {
    fn carry_out(&mut self, command: PowerCommand) {
        eprintln!("power: would run {} ({command:?}): a dry run at {:.0} ms", describe(command), since_start());
        self.asked.push(command);
    }
}

/// The machine itself. On Windows it turns off, restarts and sleeps; anywhere else it says so and does nothing.
#[derive(Debug, Default)]
pub struct SystemPower;

impl Power for SystemPower {
    #[cfg(windows)]
    fn carry_out(&mut self, command: PowerCommand) {
        eprintln!("power: {} ({command:?}) at {:.0} ms", describe(command), since_start());
        match command_line(command) {
            Some((program, arguments)) => {
                if let Err(e) = std::process::Command::new(program).args(arguments).spawn() {
                    eprintln!("power: {program} did not start: {e}");
                }
            }
            // Sleep, not hibernate; forced, as the Bundle asks; wake events left on.
            None => unsafe {
                if !windows_sys::Win32::System::Power::SetSuspendState(false, true, false) {
                    eprintln!("power: SetSuspendState failed: {}", std::io::Error::last_os_error());
                }
            },
        }
    }

    #[cfg(not(windows))]
    fn carry_out(&mut self, command: PowerCommand) {
        eprintln!("power: {} ({command:?}) is Windows' to do: nothing here", describe(command));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_commands_are_the_bundles() {
        let line =
            |command| command_line(command).map(|(program, arguments)| format!("{program} {}", arguments.join(" ")));
        assert_eq!(line(PowerCommand::TurnOff).as_deref(), Some("shutdown.exe /s /hybrid /t 0"));
        assert_eq!(line(PowerCommand::Restart).as_deref(), Some("shutdown.exe -r -t 0"));
        assert_eq!(line(PowerCommand::Sleep), None);
    }

    #[test]
    fn a_dry_run_keeps_what_it_was_asked_and_does_nothing() {
        let mut power = DryPower::default();
        for command in [PowerCommand::TurnOff, PowerCommand::Restart, PowerCommand::Sleep] {
            power.carry_out(command);
        }
        assert_eq!(power.asked, [PowerCommand::TurnOff, PowerCommand::Restart, PowerCommand::Sleep]);
    }
}
