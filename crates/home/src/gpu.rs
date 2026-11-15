//! Adapter and device set-up, shared by the window and the offscreen mode.
use wgpu::{Backends, Instance, InstanceDescriptor, PowerPreference, RequestAdapterOptions};

pub struct Gpu {
    pub instance: Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
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
                        .expect("no GPU adapter of any kind");
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
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("home"),
        ..Default::default()
    }))
    .expect("request device");
    Gpu { instance, adapter, device, queue }
}
