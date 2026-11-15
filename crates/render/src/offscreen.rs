//! Drawing with no window: the target's format, waiting on the GPU and reading a texture back.
use crate::gpu::Gpu;

pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub fn wait(gpu: &Gpu, submission_index: Option<wgpu::SubmissionIndex>) {
    gpu.device.poll(wgpu::PollType::Wait { submission_index, timeout: None }).expect("poll device");
}

/// The texture's pixels, RGBA, row by row.
pub fn read_back(gpu: &Gpu, texture: &wgpu::Texture, size: (u32, u32)) -> Vec<u8> {
    let row = 4 * size.0 as usize;
    let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("read back"),
        size: (padded * size.1 as usize) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded as u32), rows_per_image: None },
        },
        wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
    );
    gpu.queue.submit(Some(encoder.finish()));
    buffer.map_async(wgpu::MapMode::Read, .., |result| result.expect("map read-back buffer"));
    wait(gpu, None);
    let mapped = buffer.get_mapped_range(..).expect("mapped range");
    mapped.chunks(padded).flat_map(|line| &line[..row]).copied().collect()
}
