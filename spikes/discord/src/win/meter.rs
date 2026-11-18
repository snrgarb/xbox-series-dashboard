//! Sound as Windows sees it, whatever the page says: the default output's and input's peak meters, and the audio
//! sessions of the web view's own processes on them (`IAudioSessionManager2`, `IAudioMeterInformation`). A real,
//! page-independent measure that the web view plays sound and takes the microphone. Peaks are 0 to 1.
use windows::core::Interface;
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, AudioSessionStateActive, EDataFlow, IAudioSessionControl2, IAudioSessionManager2,
    IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

/// One reading.
#[derive(Default)]
pub struct Reading {
    /// The default output's peak (everything playing on it).
    pub out_peak: Option<f32>,
    /// The default input's peak (the microphone, whoever listens).
    pub in_peak: Option<f32>,
    /// The web view's sessions on the default output: how many, active, and their highest peak.
    pub out_sessions: (u32, u32, f32),
    /// The web view's sessions on the default input.
    pub in_sessions: (u32, u32, f32),
}

impl Reading {
    pub fn line(&self) -> String {
        let peak = |p: Option<f32>| p.map_or("-".to_string(), |p| format!("{p:.3}"));
        format!(
            "Windows: web view output sessions {} (active {}, peak {:.3}), input sessions {} (active {}, peak {:.3}); default output peak {}, default input peak {}",
            self.out_sessions.0,
            self.out_sessions.1,
            self.out_sessions.2,
            self.in_sessions.0,
            self.in_sessions.1,
            self.in_sessions.2,
            peak(self.out_peak),
            peak(self.in_peak)
        )
    }
}

fn default(flow: EDataFlow) -> Option<IMMDevice> {
    unsafe {
        let e: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        e.GetDefaultAudioEndpoint(flow, eConsole).ok()
    }
}

fn endpoint_peak(device: &IMMDevice) -> Option<f32> {
    unsafe { device.Activate::<IAudioMeterInformation>(CLSCTX_ALL, None).ok()?.GetPeakValue().ok() }
}

/// The sessions on `device` whose process is one of `pids`: (count, active, highest peak).
fn sessions(device: &IMMDevice, pids: &[u32]) -> (u32, u32, f32) {
    let mut out = (0, 0, 0f32);
    unsafe {
        let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return out };
        let Ok(list) = manager.GetSessionEnumerator() else { return out };
        let count = list.GetCount().unwrap_or(0);
        for i in 0..count {
            let Ok(control) = list.GetSession(i) else { continue };
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
            let Ok(pid) = control2.GetProcessId() else { continue };
            if !pids.contains(&pid) {
                continue;
            }
            out.0 += 1;
            if control.GetState().map(|s| s == AudioSessionStateActive).unwrap_or(false) {
                out.1 += 1;
            }
            if let Ok(peak) = control.cast::<IAudioMeterInformation>().and_then(|m| m.GetPeakValue()) {
                out.2 = out.2.max(peak);
            }
        }
    }
    out
}

/// Reads now, for the web view's processes `pids`.
pub fn read(pids: &[u32]) -> Reading {
    let mut r = Reading::default();
    if let Some(out) = default(eRender) {
        r.out_peak = endpoint_peak(&out);
        r.out_sessions = sessions(&out, pids);
    }
    if let Some(input) = default(eCapture) {
        r.in_peak = endpoint_peak(&input);
        r.in_sessions = sessions(&input, pids);
    }
    r
}
