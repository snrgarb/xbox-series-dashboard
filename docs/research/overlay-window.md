# Research: the Overlay window

Ticket: [#2](https://github.com/snrgarb/xbox-series-dashboard/issues/2). Status: **answered on the VM** (win11-e2e, WARP, 2026-11-15, `e2e/run.sh overlay`); the measures only a real GPU and a real game can give are still to be taken on the TV PC, and are listed at the end.

**Question.** Can a wgpu DX12 surface present through DirectComposition with premultiplied alpha on a topmost window that (a) dims a running game and draws translucent panels over it, (b) takes the keyboard when it is the Guide, (c) is click-through and never activates when it is a Toast, the Launch screen or the Idle dim, and (d) costs nothing while hidden?

## Answer

Yes, with stock wgpu and no patches: on the VM the composition swap chain is created (on WARP too), the Overlay comes out translucent over the window behind exactly as predicted, the `guide` mode takes the keyboard and gives it back, and the `passive` mode is never activated. This holds for games that run in a window, borderless, or under Fullscreen Optimizations. Not for true exclusive fullscreen, which no separate window can draw over (to be seen on the TV PC). The blending question is settled: the renderer now always blends encoded values on a non-sRGB target, as WPF does, and the DWM composes the stored bytes as they are, so the two agree exactly.

## The spike

`spikes/overlay`, binary `overlay-spike`. Windows only; elsewhere it prints "Windows only" and exits 2.

It makes one raw Win32 window over the primary monitor, puts a wgpu surface on it, and draws one static frame with `render::Renderer`: a 60% black dim over the whole screen, a 540-pixel `#1C1C1C` panel at 85% down the left with a few lines of text, and an opaque green banner at the bottom centre. It runs its own message loop: while shown it draws every vertical blank; while hidden it sleeps in `MsgWaitForMultipleObjects`.

| Argument | What it does |
|---|---|
| `--mode guide` (default) | Activatable. On show: logs the foreground window, taps Alt with `SendInput`, calls `SetForegroundWindow`, logs the foreground window again. On hide: gives the foreground back to the window that had it, then hides. Esc exits. |
| `--mode passive` | Click-through, never activated. Logs that the foreground window did not change and how many `WM_ACTIVATE`s arrived. |
| `--cycle <n>` | Shown 2 s, hidden 2 s, n times. Logs per shown stretch the frame count and min/avg/max of the present-to-present interval, the wait for a back buffer, the draw and the present call; per hidden stretch the frames presented (must be 0), wake-ups and processor time used. |
| `--seconds <n>` | Exit 0 after n seconds (default 10 without `--cycle`). |
| `--selfcheck` | Reads three screen pixels through GDI before the first show and 500 ms after the first frame, and logs them beside what composing the back buffer over the "before" colour predicts. |
| `--visual` | Fallback: the spike makes the DirectComposition device, target and visual and passes wgpu `SurfaceTargetUnsafe::CompositionVisual`. |
| `--hwnd-swapchain` | Control: an ordinary swap chain on the window. Expected to be opaque. |
| `--gamma` | Does nothing now: the renderer always draws to a plain 8-bit target with colours blended as encoded values. |
| `--no-layered` | Fallback: `passive` without `WS_EX_LAYERED`. |
| `--no-alt` | Control: `guide` without the Alt tap. Expected to fail to take the foreground from another program. |

### Why each window style is there

| Style | Why |
|---|---|
| `WS_POPUP` | No frame, no caption: the client area is the whole window. |
| `WS_EX_NOREDIRECTIONBITMAP` | "The window does not render to a redirection surface. This is for windows that do not have visible content or that use mechanisms other than surfaces to provide their visual" ([Extended Window Styles](https://learn.microsoft.com/windows/win32/winmsg/extended-window-styles)). Without it the DWM composes an opaque GDI surface for the window underneath the visual, and nothing behind shows through. It can only be set at creation, which is why the window is not winit's. |
| `WS_EX_TOPMOST` | "Above all non-topmost windows and should stay above them, even when the window is deactivated" (same page). A game's window is an ordinary desktop-band window, so this is enough; window bands and `uiAccess` are not needed (theme repo, `topmost-overlay.md` §3). Each show also calls `SetWindowPos(HWND_TOPMOST)` so it lands above a game that is itself topmost. |
| `WS_EX_TOOLWINDOW` | Out of the taskbar and Alt+Tab (same page). |
| `WS_EX_NOACTIVATE` (passive) | Showing or clicking the window never makes it the foreground window, so the game keeps the keyboard and its focus-gated pad input. The window is also always shown with `SW_SHOWNOACTIVATE`. |
| `WS_EX_LAYERED` + `WS_EX_TRANSPARENT` (passive) | "If the layered window has the WS_EX_TRANSPARENT extended window style, the shape of the layered window will be ignored and the mouse events will be passed to other windows underneath" ([Window Features § Layered Windows](https://learn.microsoft.com/windows/win32/winmsg/window-features#window-types)). A DirectComposition target window "can be a layered window" ([`CreateTargetForHwnd`](https://learn.microsoft.com/windows/win32/api/dcomp/nf-dcomp-idcompositiondevice-createtargetforhwnd#remarks)). The spike calls `SetLayeredWindowAttributes(255, LWA_ALPHA)` once, as `crates/shell` does for the Launch screen, because a layered window shows nothing until it has attributes. |

The process declares itself per-monitor DPI aware first, so the window is in real pixels on a scaled display.

### How the surface is made in wgpu 30

- `InstanceDescriptor::backend_options.dx12.presentation_system = Dx12SwapchainKind::DxgiFromVisual` (`wgpu-types/src/backend.rs`; also settable with the `WGPU_DX12_PRESENTATION_SYSTEM=visual` environment variable). The surface is then made from the HWND as usual (`SurfaceTargetUnsafe::RawHandle`).
- With that option, `wgpu-hal/src/dx12/instance.rs` makes the surface a `VisualFromWndHandle`. On the first `configure`, `dx12/mod.rs` calls `IDXGIFactory2::CreateSwapChainForComposition` on the present queue, then `dx12/dcomp.rs` calls `DCompositionCreateDevice2`, `CreateTargetForHwnd(hwnd, false)`, `CreateVisual`, `SetRoot`, and `mod.rs` calls `visual.SetContent(swap_chain)` and `device.Commit()`.
- `dx12/adapter.rs` `surface_capabilities` reports alpha modes `[Auto, Inherit, Opaque, PostMultiplied, PreMultiplied]` for every composition target and `[Opaque]` for a plain HWND one. So `PreMultiplied` should be offered with no further work, and `--hwnd-swapchain` should log only `Opaque`. `PreMultiplied` maps to `DXGI_ALPHA_MODE_PREMULTIPLIED` (`auxil/dxgi/conv.rs`).
- The swap chain is `DXGI_SWAP_EFFECT_FLIP_DISCARD`, `DXGI_SCALING_STRETCH`, three buffers at the default frame latency of 2, with a frame-latency waitable object that `get_current_texture` waits on.
- `--visual` does the DirectComposition calls itself (through the `windows` crate, which wgpu-hal already builds), passes the visual as `SurfaceTargetUnsafe::CompositionVisual`, and calls `Commit` after each `configure`, because on that path wgpu sets the visual's content but does not commit.

The adapter comes from `render::gpu::init`: hardware, then software (WARP), then anything.

## What the documentation says

### Flip-model swap chains and premultiplied alpha

- A swap chain made with `CreateSwapChainForHwnd` is not expected to be transparent: wgpu reports only `Opaque` for it (`--hwnd-swapchain` is the control). Alpha needs `CreateSwapChainForComposition`, which exists "to send Direct3D content into the DirectComposition API ... to compose in a window" and "supports only [the] flip presentation model" ([`CreateSwapChainForComposition`](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-createswapchainforcomposition)).
- Two sentences on that page do not match what wgpu does. wgpu ships this path for transparent windows, so it presumably works on real GPUs, but that is not verified here; both are things the runs check:
  - "You must specify the `DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL` value". wgpu passes `FLIP_DISCARD`, which is the later flip-model effect.
  - "Software drivers, like `D3D_DRIVER_TYPE_REFERENCE`, are not supported for composition swap chains." Whether that includes WARP is not stated. **If it does, the spike cannot run on the VM at all**, and the log will show wgpu's `SwapChain creation error` (the spike prints wgpu's warnings and errors).
- `DXGI_ALPHA_MODE_PREMULTIPLIED`: "Each color is first scaled by the alpha value ... Typically, no color channel value is greater than the alpha channel value. If a color channel value in a premultiplied format is greater than the alpha channel, the standard source-over blending math results in an additive blend" ([`DXGI_ALPHA_MODE`](https://learn.microsoft.com/windows/win32/api/dxgi1_2/ne-dxgi1_2-dxgi_alpha_mode)). So the DWM computes `screen = buffer.rgb + (1 - buffer.a) × behind`.
- The visual's content "is always composed on top of whatever is drawn directly to that window" ([`CreateTargetForHwnd`](https://learn.microsoft.com/windows/win32/api/dcomp/nf-dcomp-idcompositiondevice-createtargetforhwnd#remarks)); with no redirection bitmap there is nothing under it but the desktop behind.

### Premultiplied alpha and the sRGB target

Worked out in `spikes/overlay/src/scene.rs` and checked by a test that draws the scene offscreen and reads the target back.

- **Alpha is right.** `quad.wgsl` outputs `(rgb × a, a)` and the pipeline is `PREMULTIPLIED_ALPHA_BLENDING`; glyphon's text pipeline is `ALPHA_BLENDING`. Over a target cleared to transparent both leave coverage in the alpha channel and premultiplied colour.
- **Colour is premultiplied in linear light, then encoded.** On the sRGB target the Dashboard uses, the buffer holds `encode(rgb_linear × a)`. A DWM that composes the stored bytes as they are wants `encode(rgb_linear) × a`. They agree for `a` = 0, `a` = 1 and for black, so the dim, the banner and opaque text are exact and the near-black panel at 85% is within a count or two. They disagree for light colours at low alpha: white at 10% is stored as 89 where 26 is meant, a colour above its alpha, which the DWM adds as a glow.
- **`--gamma`** draws to `Bgra8Unorm` with colours as encoded values. The stored colour is then exactly `encoded × a`. This is also how WPF blends, so the Bundle's alpha values would mean what they mean there (the handover's "black overlays darken far less" note).
- If the DWM composes in linear light instead (plausible with HDR or Advanced Colour on, not documented for SDR), the sRGB target is the exact one. `--selfcheck` logs both predictions beside the screen's colour, so a run over a mid-grey or coloured background says which the DWM did. Over black or white the panel point cannot tell them apart; the dim point can over anything but black.

**Decided:** the whole renderer blends in encoded space on a non-sRGB target (`docs/ui.md`), so this spike has only that path left. On the VM the DWM composed the stored bytes as they are: every translucent point matched "as stored" and not "linear".

### Over games

From the theme repository's `docs/research/topmost-overlay.md` §1, which has the full table and sources:

| The game presents | Expected | Source |
|---|---|---|
| Windowed or borderless, flip model | Works. The game may be in independent flip or on an overlay plane; when other content comes on top the DWM goes back to composing, or keeps the game on its plane and composes the Overlay over it. A small cost while shown. | [For best performance, use DXGI flip model § DirectFlip](https://learn.microsoft.com/windows/win32/direct3ddxgi/for-best-performance--use-dxgi-flip-model#directflip) |
| "Fullscreen" under Fullscreen Optimizations | Works. "When an overlay such as the Game Bar is present, the DWM reassumes control of the display." | [Demystifying Full Screen Optimizations](https://devblogs.microsoft.com/directx/demystifying-full-screen-optimizations/) |
| True exclusive fullscreen, DXGI | Does not work. The DWM is not composing; a full-screen swap chain "will relinquish full-screen mode whenever its output window is occluded by another window". `guide` mode makes it worse by taking the focus. `passive` mode is the open case: a topmost window that never activates may simply not appear, or may still knock the game out of fullscreen. | [DXGI best practices § Full-screen issues](https://learn.microsoft.com/windows/win32/direct3darticles/dxgi-best-practices#full-screen-issues) |
| True exclusive fullscreen, Direct3D 9 | Does not work; the device is lost when the game loses focus. | [Lost Devices](https://learn.microsoft.com/windows/win32/direct3d9/lost-devices) |

Taking the foreground from the game needs the Alt tap: "the system automatically enables calls to SetForegroundWindow if the user presses the ALT key" ([`LockSetForegroundWindow`](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-locksetforegroundwindow)), under the rules in [`SetForegroundWindow`](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setforegroundwindow#remarks). `SendInput` does not reach a game running elevated (theme research §4). The proof of concept already does this from `crates/shell`.

### Cost while hidden

Expected to be nothing the game can notice:

- A hidden window is not in the DWM's scene, so the game can go back to independent flip. This is why the Overlay must be **hidden**, not drawn fully transparent: a visible full-screen window with alpha keeps the DWM composing (theme research §5).
- The loop presents nothing and blocks in `MsgWaitForMultipleObjects`. The spike logs frames presented, wake-ups and processor time for each hidden stretch.
- What stays allocated: the device and three back buffers, about 25 MB at 1080p and 100 MB at 4K. Dropping the swap chain while hidden would free that at the price of a slower show; not tried.

The real Overlay still has to wake for the Guide button (the proof of concept polls XInput every 30 ms); that is outside this spike.

### Reading the screen back

`--selfcheck` uses `BitBlt` from the screen's device context with `SRCCOPY | CAPTUREBLT`, which is what `Graphics.CopyFromScreen` does. Under the DWM this reads the composed desktop, DirectComposition visuals included. Caveats:

- [`BitBlt`](https://learn.microsoft.com/windows/win32/api/wingdi/nf-wingdi-bitblt) documents `CAPTUREBLT` only as including "windows that are layered on top of your window"; that it returns composed DirectComposition content is general knowledge, not a documented promise.
- It does not see a window that set `WDA_EXCLUDEFROMCAPTURE`.
- For content the DWM hands straight to the display (an independent-flip or overlay-plane game) what it returns is undocumented and may be black or stale. On the VM under WARP there are no overlay planes, so this should not arise there.
- The "before" colours are read once, before the first show. If what is behind changes in between (an animating game, a window repainting its title bar on losing focus), the comparison is meaningless at that point.

## Least sure of, and the switch for each

| Risk | How it shows | Switch |
|---|---|---|
| WARP refuses a composition swap chain | `wgpu: ERROR ... SwapChain creation error` and a panic at configure | None on the VM; `--hwnd-swapchain` shows the rest of the spike runs. The real answer then waits for the TV PC. |
| wgpu's own visual doesn't show | No panic, `selfcheck` says "unchanged, the Overlay is not on screen" | `--visual` |
| `WS_EX_LAYERED` with `WS_EX_NOREDIRECTIONBITMAP` shows nothing, or shows opaque black | `passive` fails `selfcheck` where `guide` passes | `--mode passive --no-layered`; then check a click still goes through |
| Light translucent colours glow | Panel point "matches linear" or "NO MATCH" over a light background | `--gamma` |
| `SetForegroundWindow` refused in the VM harness's session | `took foreground: false` | None; compare with `--no-alt` to see the Alt tap is what matters |
| A full-screen topmost window counts as a full-screen app to the shell (notifications suppressed, taskbar behind) | Seen by eye | Not handled in the spike |

## To be measured

### VM (win11-e2e, WARP): correctness only

| Check | Command | Result |
|---|---|---|
| Adapter and capabilities logged; `PreMultiplied` offered | any | Microsoft Basic Render Driver (DX12, CPU). Formats `Bgra8UnormSrgb, Rgba8UnormSrgb, Bgra8Unorm, Rgba8Unorm, Rgb10a2Unorm, Rgba16Float`; alpha modes `Auto, Inherit, Opaque, PostMultiplied, PreMultiplied`; present modes `Mailbox, Fifo, Immediate`. |
| Swap chain for composition is created on WARP | any | Yes: configured 1920x1080 `Bgra8Unorm`, `PreMultiplied`, `Fifo`; first frame at 525 ms. |
| `guide`: 3 of 3 `selfcheck` points; which composition (as stored / linear) | `--mode guide --selfcheck --seconds 6` | 3 of 3. Over Notepad's `[249,249,249]`: panel `[39,39,39]`, dim `[100,100,100]`, banner `[16,124,16]`: **as stored** (linear light would be 73 and 166). |
| `guide`: took the foreground; gave it back on hide | `--mode guide --cycle 2` | Yes both times: `SetForegroundWindow returned true`, and Notepad had the foreground again after each hide. |
| `guide --no-alt`: foreground refused | `--mode guide --no-alt --seconds 4` | Refused as expected: `SetForegroundWindow returned false`, Notepad kept the foreground. The Alt tap is what makes it work. |
| `passive`: 3 of 3 `selfcheck` points | `--mode passive --selfcheck --seconds 6` | 3 of 3, the same colours as `guide`. |
| `passive`: foreground unchanged, 0 activations, a click reaches the window behind | `--mode passive --seconds 10` | Foreground unchanged and 0 activations in every passive run. The click was not tried (the harness sends no mouse input): TV PC. |
| `passive --no-layered`: same checks | `--mode passive --no-layered --selfcheck --seconds 6` | The same: 3 of 3, foreground unchanged, 0 activations. `WS_EX_LAYERED` is kept because click-through is documented with it. |
| `--visual`: same as default | `--mode guide --visual --selfcheck --seconds 6` | The same, 3 of 3. Not needed: wgpu's own visual works. |
| `--hwnd-swapchain`: opaque, only `Opaque` offered | `--mode guide --hwnd-swapchain --selfcheck --seconds 6` | Opaque as expected: 1 of 3, the dim reads `[0,0,0]` and the panel `[24,24,24]`. The composition swap chain is what gives transparency. |
| `--gamma`: panel point matches "as stored" | `--mode guide --gamma --selfcheck --seconds 6` | Now the only path, and it matches "as stored" (rows above). |
| Hidden: 0 frames, wake-ups, processor time per hidden stretch | `--mode passive --cycle 3` | 0 frames and 1 wake-up per 2 s hidden stretch. Processor time was **not** nothing: 0.8 to 1.2 s per 2 s stretch, with the spike's own thread asleep. WARP renders on the processor on its own threads, so this is most likely WARP finishing or idling; it has to be read again on a real GPU before "costs nothing while hidden" is claimed. |
| Screenshot from the harness shows the dim, the panel and the banner over the desktop | | Yes: `CopyFromScreen` shows Notepad dimmed, the panel translucent and the banner opaque, in both modes. |

Shown, the Overlay drew a frame about every 210 ms on the VM: the wait for a back buffer is the DWM composing 1080p on the processor. That is the VM, not the design.

### TV PC (real GPU)

| Measure | Result |
|---|---|
| Adapter, capabilities, first frame at | |
| Processor time per hidden stretch (`--mode passive --cycle 3`): near 0? | |
| `passive`: a mouse click reaches the window behind | |
| Overlay frame interval and draw/present times while shown (`--cycle 3`) | |
| Game frame time, Overlay not running (baseline) | |
| Game frame time, Overlay hidden | |
| Game frame time, Overlay shown, `passive` | |
| Game frame time, Overlay shown, `guide` | |
| Game's presentation mode before / shown / after hide (PresentMon: independent flip restored?) | |
| Borderless game: dimmed, panel translucent | |
| Fullscreen Optimizations game: dimmed, panel translucent | |
| Exclusive fullscreen (FSO off), `passive`: appears? game stays fullscreen? | |
| Exclusive fullscreen (FSO off), `guide`: game drops out / minimises / recovers on hide? | |
| `guide`: foreground taken from the game; game stops reading the pad | |
| Time from show to first frame on screen after a hide | |
| HDR on: colours of the dim and the panel | |
