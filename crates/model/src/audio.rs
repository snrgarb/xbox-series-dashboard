//! The system's sound as the Guide's Audio Flyout shows it: the devices that play and record, which of them
//! Windows has as its defaults, and the master volume and mute of the one that plays. The shell reads it and
//! changes it (`shell::audio`); the Dashboard only ever sees this.

/// Which way sound goes through a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Playback: speakers, headphones, a TV.
    Output,
    /// Recording: microphones.
    Input,
}

/// A device that is there to be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    /// Windows' own name for it, which says which one to make the default.
    pub id: String,
    /// What the user knows it by: "Speakers (High Definition Audio Device)".
    pub name: String,
}

impl AudioDevice {
    pub fn new(id: &str, name: &str) -> Self {
        Self { id: id.into(), name: name.into() }
    }
}

/// The system's sound, as last read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Audio {
    /// The devices that play, and which of them is the default, by its place among them.
    pub outputs: Vec<AudioDevice>,
    pub output: Option<usize>,
    /// The devices that record, and the default among them.
    pub inputs: Vec<AudioDevice>,
    pub input: Option<usize>,
    /// The default output's master volume, 0 to 100, and whether it is muted. With no output, 0 and not muted.
    pub volume: u8,
    pub muted: bool,
}

impl Audio {
    /// The devices of one flow, and the default among them.
    pub fn devices(&self, flow: Flow) -> (&[AudioDevice], Option<usize>) {
        match flow {
            Flow::Output => (&self.outputs, self.output),
            Flow::Input => (&self.inputs, self.input),
        }
    }

    /// The default device's name, or "None" where there is no device.
    pub fn selection(&self, flow: Flow) -> &str {
        let (devices, default) = self.devices(flow);
        default.and_then(|at| devices.get(at)).map_or("None", |device| device.name.as_str())
    }
}
