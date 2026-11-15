//! The system's sound, for the Guide's Audio Flyout (docs/spec/guide.md, section 5.3): which devices play and
//! record and which are Windows' defaults, and the default output's master volume and mute. Read and changed
//! through Windows' own Core Audio, as the Bundle does (its `WindowsAudio.cs`), and only by [`SystemAudio`];
//! a test, and any machine that is not Windows, has a [`FakeAudio`], which is a system of its own that keeps what
//! it was asked.
pub use model::{Audio, AudioDevice, Flow};

/// Whoever has the system's sound.
pub trait AudioControl {
    /// The system's sound as it is now.
    fn read(&mut self) -> Audio;
    /// The default output's master volume, 0 to 100.
    fn set_volume(&mut self, level: u8);
    /// The default output's master mute.
    fn set_muted(&mut self, muted: bool);
    /// Makes the device with this id the default of its flow.
    fn set_default(&mut self, flow: Flow, id: &str);
}

/// A system made of what it is given: it changes as it is asked, and keeps what it was asked for a test to read.
#[derive(Debug, Default)]
pub struct FakeAudio {
    pub audio: Audio,
    pub asked: Vec<String>,
}

impl FakeAudio {
    pub fn new(audio: Audio) -> Self {
        Self { audio, asked: Vec::new() }
    }
}

impl AudioControl for FakeAudio {
    fn read(&mut self) -> Audio {
        self.audio.clone()
    }

    fn set_volume(&mut self, level: u8) {
        self.asked.push(format!("volume {level}"));
        if self.audio.output.is_some() {
            self.audio.volume = level.min(100);
        }
    }

    fn set_muted(&mut self, muted: bool) {
        self.asked.push(format!("muted {muted}"));
        if self.audio.output.is_some() {
            self.audio.muted = muted;
        }
    }

    fn set_default(&mut self, flow: Flow, id: &str) {
        self.asked.push(format!("default {flow:?} {id}"));
        let (devices, default) = match flow {
            Flow::Output => (&self.audio.outputs, &mut self.audio.output),
            Flow::Input => (&self.audio.inputs, &mut self.audio.input),
        };
        if let Some(at) = devices.iter().position(|device| device.id == id) {
            *default = Some(at);
        }
    }
}

/// One line for the log saying everything that was read: `volume 67, muted false, output "Speakers" of 2, input
/// None of 0`.
pub fn describe(audio: &Audio) -> String {
    let side = |flow: Flow| {
        let (devices, default) = audio.devices(flow);
        match default.and_then(|at| devices.get(at)) {
            Some(device) => format!("{:?} of {}", device.name, devices.len()),
            None => format!("None of {}", devices.len()),
        }
    };
    format!(
        "volume {}, muted {}, output {}, input {}",
        audio.volume,
        audio.muted,
        side(Flow::Output),
        side(Flow::Input)
    )
}

/// Windows' own sound. Anywhere else there are no devices, and nothing to change.
#[derive(Debug, Default)]
pub struct SystemAudio;

#[cfg(not(windows))]
impl AudioControl for SystemAudio {
    fn read(&mut self) -> Audio {
        Audio::default()
    }

    fn set_volume(&mut self, _level: u8) {}

    fn set_muted(&mut self, _muted: bool) {}

    fn set_default(&mut self, _flow: Flow, _id: &str) {}
}

#[cfg(windows)]
impl AudioControl for SystemAudio {
    fn read(&mut self) -> Audio {
        unsafe { system::read() }.unwrap_or_else(|e| {
            eprintln!("audio: the system's sound could not be read: {e}");
            Audio::default()
        })
    }

    fn set_volume(&mut self, level: u8) {
        if let Err(e) = unsafe { system::set_volume(level) } {
            eprintln!("audio: the volume could not be set to {level}: {e}");
        }
    }

    fn set_muted(&mut self, muted: bool) {
        if let Err(e) = unsafe { system::set_muted(muted) } {
            eprintln!("audio: mute could not be set to {muted}: {e}");
        }
    }

    fn set_default(&mut self, flow: Flow, id: &str) {
        match unsafe { system::set_default(id) } {
            Ok(()) => eprintln!("audio: default {flow:?} is now {id}"),
            Err(e) => eprintln!("audio: default {flow:?} could not be set to {id}: {e}"),
        }
    }
}

#[cfg(windows)]
mod system {
    use std::ffi::c_void;

    use windows::core::{IUnknown, Interface, Result, GUID, HRESULT, PCWSTR};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eCapture, eConsole, eMultimedia, eRender, EDataFlow, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
        DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED, STGM_READ,
    };

    use super::{Audio, AudioDevice};

    /// The devices are asked for anew each time: they come and go, and so do the defaults.
    unsafe fn enumerator() -> Result<IMMDeviceEnumerator> {
        // The window's thread has COM already; a thread that has not gets it here. Either way the answer is fine.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
    }

    unsafe fn id_of(device: &IMMDevice) -> Result<String> {
        let id = device.GetId()?;
        let text = id.to_string().unwrap_or_default();
        CoTaskMemFree(Some(id.0 as *const c_void));
        Ok(text)
    }

    unsafe fn name_of(device: &IMMDevice) -> Result<String> {
        let store = device.OpenPropertyStore(STGM_READ)?;
        Ok(store.GetValue(&PKEY_Device_FriendlyName)?.to_string())
    }

    /// The active devices of one flow, and the place among them of the default for music and games.
    unsafe fn devices(enumerator: &IMMDeviceEnumerator, flow: EDataFlow) -> Result<(Vec<AudioDevice>, Option<usize>)> {
        let all = enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
        let mut devices = Vec::new();
        for at in 0..all.GetCount()? {
            let device = all.Item(at)?;
            let id = id_of(&device)?;
            // A device whose name can't be read is still a device.
            let name = name_of(&device).unwrap_or_else(|_| id.clone());
            devices.push(AudioDevice { id, name });
        }
        // No default is no error: a machine with no such device.
        let default = match enumerator.GetDefaultAudioEndpoint(flow, eMultimedia) {
            Ok(device) => id_of(&device).ok().and_then(|id| devices.iter().position(|device| device.id == id)),
            Err(_) => None,
        };
        Ok((devices, default))
    }

    /// The default output's volume control.
    unsafe fn endpoint(enumerator: &IMMDeviceEnumerator) -> Result<IAudioEndpointVolume> {
        enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?.Activate(CLSCTX_ALL, None)
    }

    pub unsafe fn read() -> Result<Audio> {
        let enumerator = enumerator()?;
        let (outputs, output) = devices(&enumerator, eRender)?;
        let (inputs, input) = devices(&enumerator, eCapture)?;
        let (mut volume, mut muted) = (0, false);
        if output.is_some() {
            if let Ok(endpoint) = endpoint(&enumerator) {
                volume = (endpoint.GetMasterVolumeLevelScalar().unwrap_or(0.0) * 100.0).round().clamp(0.0, 100.0) as u8;
                muted = endpoint.GetMute().map(|muted| muted.as_bool()).unwrap_or(false);
            }
        }
        Ok(Audio { outputs, output, inputs, input, volume, muted })
    }

    pub unsafe fn set_volume(level: u8) -> Result<()> {
        endpoint(&enumerator()?)?.SetMasterVolumeLevelScalar(level.min(100) as f32 / 100.0, std::ptr::null())
    }

    pub unsafe fn set_muted(muted: bool) -> Result<()> {
        endpoint(&enumerator()?)?.SetMute(muted, std::ptr::null())
    }

    /// Windows has no public call that sets the default device. This is the one its own Settings uses, and the
    /// Bundle: `IPolicyConfig::SetDefaultEndpoint` of the policy config client, undocumented and unchanged since
    /// Windows 7. Only that method is called, by its place in the interface: the eleventh after IUnknown's three.
    const POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
    const POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
    const RELEASE: usize = 2;
    const SET_DEFAULT_ENDPOINT: usize = 13;

    /// Makes the device the default for everything but calls, as the Bundle does: the console's role and
    /// multimedia's.
    pub unsafe fn set_default(id: &str) -> Result<()> {
        type SetDefaultEndpoint = unsafe extern "system" fn(*mut c_void, PCWSTR, i32) -> HRESULT;
        type Release = unsafe extern "system" fn(*mut c_void) -> u32;
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let client: IUnknown = CoCreateInstance(&POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)?;
        let mut policy = std::ptr::null_mut();
        client.query(&POLICY_CONFIG, &mut policy).ok()?;
        let methods = *(policy as *const *const *const c_void);
        let set: SetDefaultEndpoint = std::mem::transmute(*methods.add(SET_DEFAULT_ENDPOINT));
        let release: Release = std::mem::transmute(*methods.add(RELEASE));
        let id = crate::wide(id);
        let mut result = Ok(());
        for role in [eConsole, eMultimedia] {
            let set = set(policy, PCWSTR(id.as_ptr()), role.0).ok();
            result = result.and(set);
        }
        release(policy);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_speakers() -> Audio {
        Audio {
            outputs: vec![AudioDevice::new("out-1", "Speakers"), AudioDevice::new("out-2", "TV")],
            output: Some(0),
            inputs: vec![AudioDevice::new("in-1", "Microphone")],
            input: Some(0),
            volume: 67,
            muted: false,
        }
    }

    #[test]
    fn the_fake_changes_as_it_is_asked_and_keeps_what_it_was_asked() {
        let mut audio = FakeAudio::new(two_speakers());
        audio.set_volume(69);
        audio.set_muted(true);
        audio.set_default(Flow::Output, "out-2");
        audio.set_default(Flow::Input, "no such device");
        let read = audio.read();
        assert_eq!((read.volume, read.muted, read.output, read.input), (69, true, Some(1), Some(0)));
        assert_eq!(audio.asked, ["volume 69", "muted true", "default Output out-2", "default Input no such device"]);
    }

    #[test]
    fn with_no_output_there_is_nothing_to_turn_up() {
        let mut audio = FakeAudio::default();
        audio.set_volume(50);
        audio.set_muted(true);
        assert_eq!(audio.read(), Audio::default());
    }

    #[test]
    fn the_log_line_says_everything_read() {
        assert_eq!(
            describe(&two_speakers()),
            "volume 67, muted false, output \"Speakers\" of 2, input \"Microphone\" of 1"
        );
        assert_eq!(describe(&Audio::default()), "volume 0, muted false, output None of 0, input None of 0");
    }
}
