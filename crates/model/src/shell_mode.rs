//! Shell mode as Settings > System shows it (docs/spec/shell-mode.md): whether the Dashboard starts in place of the
//! Windows desktop at sign-in, or why it can't. Working it out and changing it is the `shell-mode` crate's; the core
//! only shows it and asks for a change.

/// Where shell mode stands on this PC, for this user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShellMode {
    /// Not looked at yet, or nowhere to look: anywhere but Windows, and in a picture taken offscreen.
    #[default]
    Unknown,
    /// Off: Windows starts its desktop at sign-in. It may be turned on.
    Off,
    /// On: the Dashboard starts in place of the desktop at sign-in.
    On,
    /// Off, and it can't be turned on, for this reason.
    Refused(Refusal),
    /// The last change couldn't be made, and what it had done was put back. The log says why.
    Failed,
}

/// Why shell mode can't be turned on (docs/research/shell.md, 4.5 and 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The Xbox Series Theme's Boot shell is set up: its sentinel would take the PC back at the next sign-in (F11).
    BundleBootShell,
    /// The user's Winlogon `Shell` already names another program.
    OtherShell,
    /// The PC's own (HKLM) `Shell` isn't explorer.exe, so explorer couldn't become the desktop behind the Dashboard
    /// (F8).
    MachineShell,
    /// The PC's shell is set by policy (`WindowsLogon/OverrideShellProgram`), which no user value overrides (F9).
    ShellOverridden,
    /// This copy isn't the installed one: `Shell` would name a folder the user may move or delete.
    NotInstalled,
}

impl Refusal {
    /// Every reason, in the order they are looked for and said.
    pub const ALL: [Refusal; 5] = [
        Refusal::BundleBootShell,
        Refusal::OtherShell,
        Refusal::MachineShell,
        Refusal::ShellOverridden,
        Refusal::NotInstalled,
    ];

    /// A short name, for the log and the focus line.
    pub fn key(self) -> &'static str {
        match self {
            Refusal::BundleBootShell => "bundle-boot-shell",
            Refusal::OtherShell => "other-shell",
            Refusal::MachineShell => "machine-shell",
            Refusal::ShellOverridden => "shell-overridden",
            Refusal::NotInstalled => "not-installed",
        }
    }

    /// What Settings > System says, and the log after `shell: refused: `.
    pub fn says(self) -> &'static str {
        match self {
            Refusal::BundleBootShell => {
                "The Xbox Series Theme's Boot shell is set up on this PC, and would take it back at the next sign-in. \
                 Remove it first with XboxSeries.BootShell.exe --uninstall."
            }
            Refusal::OtherShell => {
                "Windows already starts another program in place of the desktop for your account. Turn that off first."
            }
            Refusal::MachineShell => {
                "This PC's own shell isn't the Windows desktop, so there would be no desktop to come back to. An \
                 administrator has to set it back to explorer.exe first."
            }
            Refusal::ShellOverridden => {
                "Your organisation sets what this PC starts at sign-in, so it can't be changed here."
            }
            Refusal::NotInstalled => {
                "This copy of the Dashboard isn't the installed one. Install it, then turn this on from there."
            }
        }
    }
}
