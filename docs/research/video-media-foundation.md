# Video frames in the renderer through Media Foundation

Researched and built 2026-11-15 for #6. The code is `spikes/video` (`video-spike.exe`). It has run on the win11-e2e VM (`e2e/run.sh video`, software rendering through WARP): the results are in [To be measured](#to-be-measured). The timings only a real GPU can give are still to be taken on the TV PC.

Each claim is tagged with its source. **[inference]** marks reasoning nobody has tested. **[docs]** is Microsoft Learn, **[wgpu]** is the wgpu 30.0.2 source in the cargo registry, **[spike]** is the code in this repository.

## Answer

**Decode in software with the Source Reader, copy each frame into one texture the renderer already knows how to draw, and play the sound ourselves through WASAPI, pacing the picture by the sound device's clock** (path A below). It needs no `unsafe` in `render`, no dependence on wgpu's internals, works the same on WARP in the VM as on the TV PC, and its cost is a few memory copies per frame: about 8 MB a frame at 1080p and 33 MB at 4K. [inference] puts that well inside a 60 Hz frame at 1080p and makes 4K at 60 the one case that needs a measurement before it is trusted.

For `render` this means one new thing: **a texture the app can overwrite each frame**, drawn by the same quad as any image. The spike adds exactly that (`add_video_texture`, `update_video_texture`), about 60 lines.

The hardware path (B) is possible in wgpu 30 but costs `unsafe` on both sides and a second Direct3D device or D3D12 fences; keep it in reserve for 4K. The Media Engine (C) would give sound and sync for nothing but can only hand frames to a Direct3D 11 surface, so it lands in the same interop as B.

## The three paths

### A. Source Reader, software, frames copied into a texture (built)

What the spike does [spike]:

1. **One `IMFSourceReader` on a thread of its own,** in synchronous mode, created with `MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING`. The video stream's output type is set to `MFVideoFormat_RGB32`, the audio stream's to `MFAudioFormat_PCM`; the reader loads the decoders and the video processor itself. [docs: [the attribute](https://learn.microsoft.com/windows/win32/medfound/mf-source-reader-enable-advanced-video-processing), [setting output formats](https://learn.microsoft.com/windows/win32/medfound/processing-media-data-with-the-source-reader#setting-output-formats)]
2. **Each frame is copied out of Media Foundation's buffer** (`IMF2DBuffer::Lock2D`, which gives the top row and the real distance between rows, negative when the image is stored bottom-up) into a plain `Vec<u8>`, cropped to the display aperture, and sent to the window over a queue of four frames. The decoder blocks when the queue is full, so it runs four frames ahead of the screen and no further.
3. **The window writes the due frame into the texture** with `queue.write_texture` and draws it as one letterboxed quad through `render::Renderer`.
4. **Sound:** the decoder keeps the PCM about a second ahead of the picture in a queue; a third thread feeds it to the default device through WASAPI in shared mode. `AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM` makes the audio engine convert rate and channels, so the decoder's format is passed as it is. [docs: [stream flags](https://learn.microsoft.com/windows/win32/coreaudio/audclnt-streamflags-xxx-constants)]
5. **Sync:** the clock is `IAudioClock::GetPosition`, what the device has actually played, corrected by the performance-counter time the position was taken at. A frame is shown when the clock reaches its timestamp less half a refresh interval; when several are due, the latest is shown and the others count as dropped. The Source Reader does none of this itself: it "does not manage a presentation clock, handle timing issues, or synchronize video with audio". [docs: [Source Reader](https://learn.microsoft.com/windows/win32/medfound/source-reader)]
6. **No sound device, no audio stream, `--no-audio`, or the device lost part-way:** the clock runs on the system's monotonic clock from the first frame. The sound never holds the picture hostage.

**Why RGB32 and not NV12 with a conversion in the shader.** RGB32 goes into a `Bgra8Unorm` texture and through the existing quad shader, its bytes shown as they are (the renderer's target is not an sRGB format, so nothing is converted on the way). NV12 would halve the bytes copied (12 bits a pixel, not 32) and move the colour conversion to the GPU, but needs a second pipeline with two planes and a YUV matrix. wgpu 30 has an `NV12` texture format behind `Features::TEXTURE_FORMAT_NV12` [wgpu: `wgpu-types/src/features.rs`], so that door is open if the measurement says the conversion is what costs.

**What it costs** [inference, from the sizes]:

| | Bytes a frame | At 60 fps, per copy |
|---|---|---|
| 1080p | 8.3 MB | 0.5 GB/s |
| 4K | 33.2 MB | 2.0 GB/s |

A frame is touched four times after decoding: the video processor writes it as RGB32, the decode thread copies it out, `write_texture` copies it into wgpu's staging buffer, and the GPU copies that into the texture. At a memory bandwidth of 5–10 GB/s a copy is 1–2 ms at 1080p and 3–7 ms at 4K. So 1080p60 should leave most of a 16.7 ms frame free, and 4K60 uses two cores' worth of copying on top of a software 4K decode. The Boot video and the Idle screen's clips are 1080p or less today in the Bundle [inference: not checked against its files], so A covers the product; 4K is the number to measure before promising it.

The middle copy (into the `Vec`) can go: lock the sample on the window's thread and hand `write_texture` the locked rows directly. The spike keeps it because it lets the decode thread own every Media Foundation object, and it is the cheapest of the four.

**Microsoft's own caution.** The docs call the Source Reader's conversion to RGB32 "not recommended for real-time video rendering" [docs: [Source Reader](https://learn.microsoft.com/windows/win32/medfound/source-reader)]. The older `MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING` is described the same way, "not optimized for playback" [docs: [the attribute](https://learn.microsoft.com/windows/win32/medfound/mf-source-reader-enable-video-processing)], and the advanced attribute, which uses the video processor MFT and can run on the GPU, carries no such warning; so the caution is most likely about the older one. [inference] The spike has `--basic-processing` to compare the two.

**A middle step, built as `--dxva`:** give the Source Reader a Direct3D 11 device of its own (`MF_SOURCE_READER_D3D_MANAGER`), so decoding and the conversion to RGB32 run on the GPU, and still read each frame back to memory with `Lock2D`. It keeps `render` untouched and removes the software decode, at the price of a GPU-to-CPU read-back per frame. [docs: [the attribute](https://learn.microsoft.com/windows/win32/medfound/mf-source-reader-d3d-manager)] Whether that beats software decoding at 4K is a measurement.

### B. Hardware decode into a texture wgpu samples (not built)

**What wgpu 30 offers** [wgpu]:

- `wgpu::Device::as_hal::<Dx12>()` reaches the hal device, whose `raw_device()` is the `ID3D12Device` and `raw_queue()` the `ID3D12CommandQueue` (`wgpu-hal/src/dx12/device.rs`).
- `wgpu_hal::dx12::Device::texture_from_raw(resource, format, dimension, size, mips, samples)` wraps an `ID3D12Resource`, and `wgpu::Device::create_texture_from_hal::<Dx12>(texture, &desc, initial_state)` turns that into a `wgpu::Texture` (`wgpu/src/api/device.rs`). Both are `unsafe`: the resource must belong to wgpu's device, match the descriptor, and be in the state named.

**Two ways to get a decoded frame into such a resource:**

1. **A shared texture between a D3D11 device and wgpu's D3D12 device.** Media Foundation decodes on D3D11 (the device manager above); the app creates a D3D11 texture with a shared NT handle, opens it on wgpu's `ID3D12Device` with `OpenSharedHandle`, wraps it for wgpu, and each frame has the D3D11 video processor blit the decoded NV12 surface into it, with a shared fence or a flush so D3D12 doesn't sample half a frame. [inference: assembled from the D3D11/D3D12 interop model, not tried]
2. **Media Foundation on D3D12 directly.** Windows 11 has `IMFD3D12SynchronizationObject` for handing D3D12 resources between Media Foundation and a consumer, with waits enqueued on the consumer's command queue [docs: [mfd3d12.h](https://learn.microsoft.com/windows/win32/api/mfd3d12/), [the GDK's decode walkthrough](https://learn.microsoft.com/gaming/gdk/docs/gdk-dev/overviews/mediafoundation-decode?view=gdk-2610)]. That means enqueueing waits on the queue wgpu owns, around wgpu's own submissions.

**What it brings:** `unsafe` in `render` (or a new `media` to `render` seam that passes raw resources), a hard dependency on `wgpu-hal`'s DX12 types, which change between wgpu releases, a video path that only exists on DX12 (the Linux screenshot tests could not cover it), resource-state and lifetime rules wgpu cannot check, and device-lost handling for two devices. **What it saves:** every CPU copy and the software decode. That is worth it only if A measurably fails at a resolution the product needs.

### C. `IMFMediaEngine` in frame-server mode (not built)

The Media Engine plays the file itself: it decodes, plays the sound, keeps sync, and loops. In frame-server mode the app asks each vertical blank whether a new frame is ready (`OnVideoStreamTick`) and has it blitted, scaled and letterboxed, into a surface (`TransferVideoFrame`). [docs: [OnVideoStreamTick](https://learn.microsoft.com/windows/win32/api/mfmediaengine/nf-mfmediaengine-imfmediaengine-onvideostreamtick), [TransferVideoFrame](https://learn.microsoft.com/windows/win32/api/mfmediaengine/nf-mfmediaengine-imfmediaengine-transfervideoframe)]

**The catch is the surface.** `TransferVideoFrame` copies "to a DXGI surface or WIC bitmap" [docs]. A DXGI surface here means a Direct3D 11 texture on the device given to the engine through `MF_MEDIA_ENGINE_DXGI_MANAGER`; our renderer is Direct3D 12. So either:

- the same shared D3D11-to-D3D12 texture as B.1, with all of B's `unsafe` and hal coupling, or
- a WIC bitmap, which is a CPU copy again and no better than A.

It also needs a COM callback object (`IMFMediaEngineNotify`) implemented in Rust, which pulls in the `windows` crate's `implement` feature and its macros. **C is the least code for sound and sync, and that is the part A turned out to need the least code for anyway:** the WASAPI feeder and the clock are about 300 lines [spike: `spikes/video/src/win/audio.rs`, now `crates/media/src/win/audio.rs`].

One thing C would do better: it follows the default device when the user changes it. A stops the sound and carries on silent when its device goes; following a change of device is a listed follow-up below.

## What it implies for the crates

- **`render`: a texture the app can overwrite each frame.** Nothing else. In the spike that is `Renderer::add_video_texture(device, width, height) -> TexId` and `Renderer::update_video_texture(queue, tex, pixels, stride)`, beside the `update_texture` that takes an `RgbaImage`. The texture is `Bgra8Unorm`, which is the byte order `MFVideoFormat_RGB32` has, so no channel swap is done on the CPU. RGB32's fourth byte is undefined, so quads with a video texture are drawn by a second pipeline whose fragment shader takes the texture's alpha as 1, rather than the app rewriting 8 or 33 MB of alpha per frame. Every other quad goes through the same shader code as before, and the screenshot references are unchanged.
- **The video texture has one mip level.** Drawn smaller than itself (a 4K clip on a 1080p screen, or a small Idle-screen tile) it will shimmer. The product should decode at the size it draws: the video processor resizes if the output type's `MF_MT_FRAME_SIZE` is set [docs: the advanced-processing attribute lists "video resizing"].
- **`media`** owns the decoder thread, the sound thread and the clock, and hands `screens` a stream of `(time, pixels)` and a clock to compare against. It does not need to know about wgpu.
- **The `windows` crate, not `windows-sys`.** Media Foundation and WASAPI are COM, and `windows-sys` has no COM interfaces. The spike depends on `windows` 0.62, the version `wgpu-hal` already builds for DX12, so nothing is compiled twice.
- **Windows N.** `video-spike.exe` imports `mfplat.dll`, `mfreadwrite.dll` and `mf.dll` statically (checked with `llvm-objdump -p`). An N edition without the Media Feature Pack has none of them, and the process would not start at all. In the product, `media` must load Media Foundation at run time and treat its absence as "no video", or the whole Dashboard fails to start on those machines. [inference: not tried on an N edition]

## Since the spike: the `media` crate (#126)

The spike's playback, path A, is now `crates/media`, which the Dashboard plays its Boot video with (docs/spec/start-up.md) and the spike plays through too: `media::Video` (opened on a decoding thread, its frames taken as they come due by the clock, its pixels given back to be filled again), the WASAPI feeder and clock (`media::win::audio`, as the spike's, with the stream's volume set through `ISimpleAudioVolume`), and the decoder (`media::win::decode`). What changed on the way:

- **Media Foundation is loaded at run time** (`media::win::mf`): `mfplat.dll` and `mfreadwrite.dll` by `LoadLibraryExW` from System32, each function by `GetProcAddress`. Without them a video fails to open ("no Media Foundation on this Windows") and the Dashboard starts all the same: the Windows N point above. The COM interfaces' methods need no import, only the free functions.
- **Anywhere but Windows** `Video::open` fails at once, so the core's tests and `--screenshot` run on Linux.
- `make_clip` and the test pattern stay in the spike, which is still built and run by the `video` payload, and makes the `boot-video` payload's clip.

## Not done in the spike

- **Looping** (the Idle screen): `IMFSourceReader::SetCurrentPosition` back to zero at the end of the stream, with the sound's queue and clock reset.
- **Pausing and seeking.** The Boot video needs neither.
- **Following a change of default sound device.**
- **HDR and 10-bit.** RGB32 is 8-bit SDR; an HDR clip would be tone-mapped by the video processor or look wrong. The Bundle's videos are SDR [inference].
- **The volume the Bundle plays the Boot video at,** and ducking.

## To be measured

The VM table is from `e2e/run.sh video` on 2026-11-15. The TV PC table is empty.

### On the win11-e2e VM (WARP, correctness only)

| Question | Result |
|---|---|
| Does `--make-clip clip.mp4` succeed (H.264 and AAC encoders present)? If not, the HRESULT, and does `clip.wmv`? | Yes, both: `clip.mp4` (H.264, AAC, 341 kB, written in 0.3 s) and `clip.wmv` (WMV3, WMA, 450 kB). |
| Does the clip play: `video: opened`, `first frame presented`, `end of stream`? | Yes, mp4 and wmv, to `end of stream` and exit 0. First frame 0.7 to 0.9 s after the process starts for mp4. |
| The media types logged: native subtype, frame size (is the display aperture needed?), sound format | `1280x720 30.00 fps 5.01 s, video H264 -> RGB32, audio AAC -> PCM 48000 Hz 2 ch 16-bit`; texture 1280x720, so the aperture handling gave the right size. |
| Is the picture the right way up, the right colours, and opaque? (screenshot: the frame counter reads left to right, white bar first on frame 0) | Yes: the bars in order, the counter readable, opaque, full screen and windowed, for H.264 and WMV3. |
| `decoded`, `presented`, `dropped` at the end of a 5 s clip (150 frames) | `decoded=150 presented=38 dropped=112`. The VM presents about four frames a second at 1080p, so most frames are dropped; decode itself averaged 3 to 12 ms a frame, the copy out 0.3 ms and the upload 0.3 ms. |
| Sound: is there an endpoint? `audio: device ... started` or `audio: none (...)` | There is one: `device "Speakers (High Definition Audio Device)" started, 48000 Hz 2 ch 16-bit, buffer 200 ms`. `AUTOCONVERTPCM` took the decoder's format as it is. Nobody heard it. |
| With no endpoint, or `--no-audio`, does it still play at the right speed (5 s clip ends about 5 s after the first frame)? | **Not on the VM, with or without sound:** the 5 s clip took about 10 s. The window takes at most the four queued frames per present, and a present takes 250 ms here, so the decoder is held back and the picture falls behind the clock. The product must drain every due frame before each present (or let the decoder skip), so a slow screen drops frames and never slows the film. To be confirmed at real speed on the TV PC. |
| Does a key skip it? Does `--seconds 2` stop it? | `--seconds` stops it. A key skips it too readily: a key press that arrived as the window opened ended the first run at once. Keys now count only once the picture has been up 300 ms; the product should take skips from the pad and from keys pressed while it has the foreground. |
| `--basic-processing` and `--dxva`: do they work at all on WARP? | Both work and show the same picture. `--dxva` took 1.3 s longer to the first frame on WARP. |

### On the TV PC (RX 9070 XT, DX12, real timings)

| Question | 1080p30 | 1080p60 | 4K60 |
|---|---|---|---|
| `avg_decode_ms` (decode and conversion to RGB32) | | | |
| `avg_copy_ms` (out of Media Foundation's buffer) | | | |
| `avg_upload_ms` (`write_texture`) | | | |
| Dropped frames over the clip | | | |
| CPU use of the process | | | |
| The same with `--dxva` | | | |
| The same with `--basic-processing` | | | |

| Question | Result |
|---|---|
| Process start to first frame (`first frame presented at N ms`) | |
| Do the beep and the white square arrive together, by eye and ear, over the TV's sound path? | |
| Does the clock drift from the picture over a long clip (`clock=` in the stats line against frames presented)? | |
| Does the real Boot video (the Bundle's file, not committed here) play right? | |

## Sources

- Microsoft Learn, Media Foundation: Source Reader, its attributes, `IMFMediaEngine`, `mfd3d12.h`; Core Audio: `AUDCLNT_STREAMFLAGS_XXX`. Linked where used.
- wgpu 30.0.2, wgpu-hal 30.0.2 and windows 0.62.2, read from `~/projects/.rust/cargo/registry/src`.
