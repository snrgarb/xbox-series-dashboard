//! Adapter and device set-up, shared by the window and the offscreen mode.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use wgpu::{Backends, Instance, InstanceDescriptor, PowerPreference, RequestAdapterOptions};

pub struct Gpu {
    pub instance: Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Says whether this device has been lost (a driver reset, a GPU removed): see [`DeviceLost`].
    pub lost: DeviceLost,
}

/// Whether a device has been lost, as wgpu's device-lost callback told. Shared with that callback, which runs on
/// whatever thread wgpu likes; the draw loop asks it.
#[derive(Clone, Default)]
pub struct DeviceLost(Arc<Mutex<Option<String>>>);

impl DeviceLost {
    /// Records the loss (the first reason stays).
    pub fn set(&self, reason: String) {
        if let Ok(mut lost) = self.0.lock() {
            lost.get_or_insert(reason);
        }
    }

    /// Why the device was lost, if it has been.
    pub fn reason(&self) -> Option<String> {
        self.0.lock().ok().and_then(|lost| lost.clone())
    }
}

/// DX12 on Windows, Vulkan elsewhere.
fn preferred_backends() -> Backends {
    if cfg!(windows) {
        Backends::DX12
    } else {
        Backends::VULKAN
    }
}

pub fn instance(display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>) -> Instance {
    let mut desc = InstanceDescriptor::new_without_display_handle();
    desc.backends = preferred_backends();
    desc.display = display;
    Instance::new(desc)
}

/// The best adapter for `surface` (none when offscreen). Where the real GPU can't be had, as in a session with no
/// desktop, falls back to a software adapter (WARP, lavapipe) and then to any adapter of any backend.
pub fn init(instance: Instance, surface: Option<&wgpu::Surface<'_>>) -> Gpu {
    try_init(instance, surface, true).unwrap_or_else(|why| panic!("{why}"))
}

/// As [`init`], but the failure comes back, and `software` says whether a software adapter will do: not after a
/// device was lost, when WARP would be far too slow for the screen and the real GPU is worth waiting for. With a
/// surface (a window) wgpu's uncaptured errors are logged, not panicked on, and the device-lost callback is set:
/// the window rebuilds the GPU when `Gpu::lost` says so.
pub fn try_init(instance: Instance, surface: Option<&wgpu::Surface<'_>>, software: bool) -> Result<Gpu, String> {
    let request = |instance: &Instance, fallback: bool| {
        pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: fallback,
            compatible_surface: surface,
            ..Default::default()
        }))
    };
    let (instance, adapter) = match request(&instance, false) {
        Ok(a) => (instance, a),
        Err(e) if !software => return Err(format!("no hardware adapter: {e}")),
        Err(e) => {
            eprintln!("no hardware adapter ({e}); trying a software one");
            match request(&instance, true) {
                Ok(a) => (instance, a),
                Err(e) => {
                    eprintln!("no software adapter on the preferred backend ({e}); trying any backend");
                    let mut desc = InstanceDescriptor::new_without_display_handle();
                    desc.backends = Backends::all();
                    let any = Instance::new(desc);
                    let adapter = pollster::block_on(any.enumerate_adapters(Backends::all()))
                        .into_iter()
                        .next()
                        .ok_or("no GPU adapter of any kind")?;
                    (any, adapter)
                }
            }
        }
    };
    let info = adapter.get_info();
    eprintln!(
        "adapter: {} ({:?}, {:?}, driver {} {})",
        info.name, info.backend, info.device_type, info.driver, info.driver_info
    );
    let (device, queue) = pollster::block_on(
        adapter.request_device(&wgpu::DeviceDescriptor { label: Some("home"), ..Default::default() }),
    )
    .map_err(|e| format!("request device: {e}"))?;
    let lost = DeviceLost::default();
    if surface.is_some() {
        let flag = lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            eprintln!("gpu: wgpu says the device was lost: {reason:?}: {message}");
            flag.set(format!("{reason:?}: {message}"));
        });
        // wgpu's default is to panic on an error nobody caught, which is what every call on a lost device becomes.
        let errors = AtomicUsize::new(0);
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            // The first few say what happened; a lost device would say it every frame.
            if errors.fetch_add(1, Ordering::Relaxed) < 20 {
                eprintln!("gpu: wgpu error: {error}");
            }
        }));
    }
    Ok(Gpu { instance, adapter, device, queue, lost })
}

/// DXGI's name for the HRESULT `ID3D12Device::GetDeviceRemovedReason` gave, and who is likely behind it.
pub fn removed_reason_name(hresult: u32) -> String {
    let (name, who) = match hresult {
        0 => ("S_OK", "the device is not removed"),
        0x887A_0005 => ("DXGI_ERROR_DEVICE_REMOVED", "the GPU was removed or suspended: from outside the Dashboard"),
        0x887A_0006 => {
            ("DXGI_ERROR_DEVICE_HUNG", "the GPU stopped answering: the Dashboard's commands may be at fault")
        }
        0x887A_0007 => {
            ("DXGI_ERROR_DEVICE_RESET", "the driver reset the GPU: from outside, unless a bad command caused it")
        }
        0x887A_0020 => ("DXGI_ERROR_DRIVER_INTERNAL_ERROR", "the driver failed: from outside the Dashboard"),
        0x887A_0001 => ("DXGI_ERROR_INVALID_CALL", "the Dashboard made a call the API refuses: its own fault"),
        _ => ("an unknown HRESULT", "who is behind it is not known"),
    };
    format!("{name} ({hresult:#010x}): {who}")
}

/// Why Direct3D 12 says `device` was removed, named (`removed_reason_name`). None where the device isn't a D3D12 one.
#[cfg(windows)]
pub fn removed_reason(device: &wgpu::Device) -> Option<String> {
    // SAFETY: the raw device is only asked a question, and the guard is dropped at the end of the statement.
    let hal = unsafe { device.as_hal::<wgpu::hal::api::Dx12>() }?;
    let reason = match unsafe { hal.raw_device().GetDeviceRemovedReason() } {
        Ok(()) => 0,
        Err(error) => error.code().0 as u32,
    };
    Some(removed_reason_name(reason))
}

#[cfg(not(windows))]
pub fn removed_reason(_: &wgpu::Device) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_removed_reasons_are_named_with_who_is_behind_them() {
        assert!(removed_reason_name(0x887A_0005).starts_with("DXGI_ERROR_DEVICE_REMOVED (0x887a0005)"));
        assert!(removed_reason_name(0x887A_0006).contains("DEVICE_HUNG"));
        assert!(removed_reason_name(0x887A_0007).contains("DEVICE_RESET"));
        assert!(removed_reason_name(0x887A_0020).contains("DRIVER_INTERNAL_ERROR"));
        assert!(removed_reason_name(0x887A_0001).contains("its own fault"));
        assert!(removed_reason_name(0x1234).contains("unknown"));
    }
}
