# Could the Dashboard become its own Rust executable that works with Xbox mode?

Exploration, not a decision. Researched 2026-11-15. It builds on [xbox-mode-home-app.md](xbox-mode-home-app.md) (#98), [ADR 0002](../adr/0002-boot-shell-not-xbox-mode.md), [playnite-cold-start.md](playnite-cold-start.md) (#101) and [fullscreen-theming-limits.md](fullscreen-theming-limits.md) (#6).

Each claim is tagged with its source. **[inference]** marks reasoning that nobody has tested yet.

## Answer

**It's possible, but the two halves of the idea pull in different directions:**

1. **Rust instead of Playnite + WPF** is a rewrite of the whole product, not a port of the theme. The theme is the small part. The big part is what Playnite does for us today: library imports, launching and tracking games, installs, metadata and filter presets.
2. **Working with Xbox mode** brings back both of the blockers that ADR 0002 is built on:
   - **Home app.** Only gaming handhelds can choose a home app. Elsewhere it takes the `DeviceForm` 46 fake.
   - **The Guide button.** In Xbox mode, Windows owns it.

   Neither has changed in Microsoft's current docs (GDK 2610). A Rust executable doesn't change either one, because both are decisions in the OS.

**Recommended next step.** Build a small, throwaway **spike** before deciding anything:

- A Rust Home that shows the **Recent row** from Playnite's library and starts a game.
- Packaged as an MSIX home app.
- Run in the e2e VM, in both Xbox mode and the Boot shell.

The spike answers the two questions that decide this: how much faster the first frame is, and whether a Dashboard without the Guide button is acceptable.

## 1. Where the speed would come from

**Playnite's start-up is where a native app would win.** From a cold start, Start-up's first frame takes **7.9 s** (3.8 s warm). [playnite-cold-start.md] That time breaks down as:

- **3.8 s of Playnite itself:**
  - The DesktopApp hop: 1.2 s.
  - WPF, the WCF pipe, settings and the theme: 1.4 s.
  - CEF, SDL and audio: 1.2 s.
- **2.9 s of extension constructors** (16 extensions).

A native Rust executable has no CLR, no JIT, no WPF and no CEF. It could plausibly put its first frame up in well under a second **[inference; not measured]**. That would cut 5–7 s from a cold boot of about 37 s. [boot-baseline.md]

**Cheaper ways to win back much of that time are already on the table**, all from #101 and all inside Playnite:

- Start Start-up in the extension's constructor and load the extension first: 3.5–4.2 s.
- Skip the DesktopApp hop: about 1 s.
- Drop the slow extensions: 0.5–0.9 s.

Together they recover a large part of what a rewrite would. So any case for Rust has to rest on what's left after those cuts, plus steady-state smoothness (scrolling, animation, memory), which nobody has measured yet. **[inference]**

## 2. What "independent of Playnite" costs

Today the Bundle is about **53,600 lines** of C# and XAML (counted on `main` @ `f11e4f9`):

| Part | Lines |
|---|---|
| `extension/XboxSeries.Extension` (WPF windows, Windows interop) | 30,170 |
| `extension/XboxSeries.Core` (logic, already free of Playnite UI) | 15,227 |
| `theme/XboxSeries.Dashboard` (XAML) | 5,087 |
| `bootshell/XboxSeries.BootShell` | 3,150 |

**What Playnite supplies that a standalone app would have to rebuild:**

- **Library imports:**
  - Steam: `libraryfolders.vdf` and `appmanifest_*.acf`.
  - Epic: the `Manifests\*.item` JSON files.
  - GOG, EA, Ubisoft, Battle.net and itch.
  - Xbox and Game Pass: `PackageManager` and `MicrosoftGame.config`.
- **Launching and tracking:** starting games, tracking processes, play time and *Last played*. *Last played* is what the **Recent row** sorts by.
- **Installs and uninstalls** through each client. The **Queue** already reads Steam on its own.
- **Metadata and art:** covers, backgrounds and descriptions, with Playnite's metadata sources behind them.
- **Filter presets:** **Groups** and **My games & apps**' tabs are Playnite filter presets.
- **Add-ons:** PlayniteAchievements (**Score**, achievement Toasts) and the rest of Playnite's add-on ecosystem.

**Three levels of independence**, in order of cost:

| Option | What runs | Cost | Risk |
|---|---|---|---|
| **A. Rust front-end over Playnite's library** | Playnite stays installed as the library and launcher, with no UI shown. The extension exports the library and takes commands over a pipe. The Rust executable draws everything. | Low to medium | Playnite still starts, so part of the speed gain is lost. We'd depend on a headless mode that Playnite doesn't officially support. |
| **B. Rust reads Playnite's database directly** | Playnite isn't running. Rust reads Playnite's LiteDB files and launches games itself. | Medium | No maintained LiteDB reader in Rust **[not checked in depth]**. The format is Playnite's internal one. Writing to it (play time, favourites) risks corrupting the user's library. |
| **C. Fully standalone** | Rust has its own library, importers, metadata and launching. | High: a second Playnite | The long tail of launchers, plus metadata API keys and their terms. |

Option A is the only one that keeps the Bundle's features while the front-end is proven. **[inference]**

## 3. Xbox mode in November 2026

### A home app has to be packaged, and only handhelds can choose one

**What a home app needs** (unchanged since #98):

- An MSIX package with the `windows.gamingApp` app extension.
- The `Microsoft.appCategory.gamingHome_8wekyb3d8bbwe` custom capability.
- `runFullTrust`.

A Rust `.exe` can sit inside such a package like any other Win32 executable. Packaging it is ordinary MSIX work. [xbox-mode-home-app.md §4]

**Who can choose a home app.** Only gaming handhelds get *Choose home app* and *Enter Xbox mode on startup*. Other PCs get *Enable Xbox mode*, which enters the Xbox app. [MS Support KB5070297] No 2026 source shows this widened to desktops. Guides written since the April 2026 rollout still describe the split, and still use ViVeTool or XFSET to force it. [Windows Latest, 2 May 2026; Pureinfotech]

**Third-party registration is OEM-only.** On Microsoft Q&A, the answer to a request to let third parties register a home app says:

- *"only OEMs can configure boot directly into a dedicated full screen gaming app"*
- there is *"no public GDK API or registration flow for non‑OEM apps"*, and no certification path.

[Microsoft Q&A](https://learn.microsoft.com/answers/a/12317749)

**Third-party home apps do ship on handhelds.** Winhanced is one, chosen in Settings › Gaming › *Choose home app*. [XDA] Like AnyFSE and OmniConsole, they rely on the custom capability being granted under Developer Mode, through an SCCD with the `FFFF` catalog. [xbox-mode-home-app.md §4]

**What this means for distribution.** A home-app build for other people would need:

- Developer Mode turned on.
- A sideloading certificate trusted on their PC.
- A handheld, or the `DeviceForm` fake.

The Microsoft Store can't ship that capability to us. **[inference from the Q&A answer]**

### Windows keeps the Guide button

The GDK 2610 docs still say:

> When FSE is active, Windows has exclusive access to the **Guide** button on controllers. When a player short-presses the **Guide** button in the Full Screen Experience, it brings up Xbox Game Bar. … a player can press **View+Menu** to send a **Guide** button to the focused application.

[MS Learn: Game launchers and handhelds (GDK 2610)](https://learn.microsoft.com/gaming/gdk/docs/gdk-dev/pc-dev/handheld/handheld-launchers?view=gdk-2610#game-overlays-and-guide-button)

So in Xbox mode, the Bundle's **Guide** can't open on the Guide button, however the app is written. There are two ways to adapt:

- **Open the Guide on View+Menu.** It works, but the console doesn't do it that way.
- **Move the Guide into Game Bar as a widget.** Game Bar is extensible through the Game Bar Widget SDK. [same page] Widgets are packaged UWP XAML apps (C# or C++/WinRT). There's no sensible way to write one in Rust **[inference]**, so a Guide widget would stay C# (or C++) even if the Home moved to Rust. It would also look like Game Bar's own panel, not the console's Guide.

### The Xbox app is the competition there

In Xbox mode, the Xbox app's own library already pulls in games from Steam, Battle.net, Epic, Ubisoft Connect and Humble. [Pureinfotech, citing Engadget] A third-party home app has to be clearly better than that to be worth Developer Mode and sideloading. **[inference]**

### APIs a Rust app would use

- **Detecting Xbox mode:** `IsGamingFullScreenExperienceActive` and `RegisterGamingFullScreenExperienceChangeNotification`, from `api-ms-win-gaming-experience-l1-1-0` (Windows SDK 10.0.26100.3916+). [MS Learn] A Rust app can call them through `windows-rs` or a small `extern` block. **[not checked: whether `windows-rs` generates them]**
- **Reading the pad:** the Boot shell already polls XInput ([`XInputPad.cs`](../../bootshell/XboxSeries.BootShell/XInputPad.cs)). The same calls work from Rust. In Xbox mode, the Guide bit would show only through View+Menu.

## 4. A Rust stack that could do this

What matters for the Dashboard: GPU-composited 1080p/4K at a steady 60+ fps, blur and acrylic, video (the Boot video and Idle screen), lots of art images, controller-only focus navigation, and a fast first frame.

| Layer | Candidate | Notes |
|---|---|---|
| Window and loop | `winit`, or plain Win32 through `windows-rs` | Plain Win32 gives full control of borderless fullscreen, topmost and DPI. The Boot shell and Toasts already do this in C#. |
| Rendering | `wgpu` (D3D12) with our own scene, or **Slint** on its wgpu/D3D12 back end | Slint is a declarative UI language, close to how XAML is used today. Gamepad launchers already exist in Slint, such as GPCL and Anubis. [GPCL; Anubis] Neither does controller focus or video out of the box. |
| Video | Media Foundation through `windows-rs`, decoding into a D3D texture | No Rust UI toolkit plays video on its own. |
| Input | XInput (ordinal 100 for Guide, as today), or Windows.Gaming.Input through `windows-rs` | GameInput is the newer API. **[not checked: whether it exposes Guide]** |
| Packaging | MSIX holding the `.exe` (home app), or the bare `.exe` (Boot shell route) | Both can come out of one build. |

## 5. Two routes, so the Guide button isn't lost

ADR 0002's facts haven't changed, so a standalone app would only make sense with **both** routes:

- **Boot shell route** (this TV PC, today's design): the Boot shell starts the Rust app instead of Playnite. The Guide button stays ours, and nothing about the OS changes.
- **Xbox mode route** (handhelds, or anyone who chooses it): the Rust app is the packaged home app. Game Bar takes the Guide button, and our Guide opens on View+Menu or as a widget.

Supporting the Xbox mode route on this PC would mean writing a new ADR to supersede 0002, and accepting the `DeviceForm` fake that #109 removed. **[inference]**

## Open questions for a map

1. **Why Rust?** Is it boot time, smoothness in steady state, memory, or independence from Playnite's release cycle? Each points to a different first step.
2. **How much Playnite goes?** Option A, B or C in §2.
3. **Who is Xbox mode for?** This TV PC (that means reopening ADR 0002) or other people's handhelds (that means distributing under Developer Mode).
4. **The Guide in Xbox mode:** View+Menu, a Game Bar widget, or no Guide there at all.
5. **Is the gain real?** Measure a Rust Home's cold first frame in the e2e VM against #101's numbers after its cuts.

## Not verified

- A Rust Home's cold first frame. No spike has been built.
- Whether `windows-rs` exposes the `gamingexperience.h` functions.
- Whether GameInput reports the Guide button outside Xbox mode.
- A maintained LiteDB reader for Rust.
- Whether any build of Windows 11 after May 2026 offers *Choose home app* on a stock desktop. No source found says so.

## Sources

- **Microsoft:**
  - [GDK: Game launchers and handhelds (2610)](https://learn.microsoft.com/gaming/gdk/docs/gdk-dev/pc-dev/handheld/handheld-launchers?view=gdk-2610)
  - [Unattend: GamingHomeApp](https://learn.microsoft.com/windows-hardware/customize/desktop/unattend/microsoft-windows-gaming-configuration-gaminghomeapp)
  - [Microsoft Q&A: registering an app as an Xbox FSE home app](https://learn.microsoft.com/answers/a/12317749)
  - [Windows Gaming: Full screen experience (KB5070297)](https://support.microsoft.com/en-us/topic/windows-gaming-full-screen-experience-67fb8d12-5467-4a95-8adf-0a10789576ab)
- **Press and guides:**
  - [Windows Latest, 2 May 2026](https://www.windowslatest.com/2026/05/02/how-to-force-enable-xbox-mode-in-windows-11-and-why-microsoft-hides-it-explained/)
  - [Pureinfotech: Xbox mode on PCs](https://pureinfotech.com/enable-xbox-mode-pc-windows-11/)
  - [Engadget: How to use Xbox mode](https://www.engadget.com/2260213/how-to-use-xbox-mode-windows-pc/)
  - [XDA: Winhanced and Xbox mode](https://www.xda-developers.com/winhanced-getting-better-i-still-prefer-using-xbox-mode-with-windows-11-handheld/)
- **Rust:**
  - [GPCL (Slint gamepad launcher)](https://github.com/Quackdoc/gpcl)
  - [Anubis (Slint handheld launcher)](https://rust-digger.code-maven.com/src/anubis-0.0.2/README.md)
  - [Slint wgpu/D3D12 update, May 2026](https://slint.dev/blog/servo-with-slint-update)
  - [Are we GUI yet?](https://areweguiyet.com/)
- **This repo:** ADR 0002, #98, #101, #109, and line counts on `main` @ `f11e4f9`.
