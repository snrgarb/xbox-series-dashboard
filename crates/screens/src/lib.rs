//! The screens: Home, with its Launch screen and Guide, as the proof of concept draws them.
pub mod art;
pub mod scene;

use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;

use model::Library;
use render::gpu::Gpu;
use render::Renderer;

use crate::scene::{GuideAction, GuideEntry, Scene, RECENT_COUNT};

static CLOCK: OnceLock<String> = OnceLock::new();

/// Fixes what the top bar's clock reads, in place of the time: a screenshot test's is the same every run.
pub fn set_clock(text: String) {
    let _ = CLOCK.set(text);
}

/// What the top bar's clock reads.
pub fn clock() -> String {
    CLOCK.get().cloned().unwrap_or_else(|| chrono::Local::now().format("%-I:%M %p").to_string())
}

/// Home on a device: the scene, its renderer and the art still on its way from the decoding threads.
pub struct Home {
    pub scene: Scene,
    renderer: Renderer,
    art: Receiver<art::Loaded>,
}

impl Home {
    /// Starts decoding the art at once; `wake` is called from a decoding thread as each piece is ready.
    pub fn new(
        gpu: &Gpu,
        format: wgpu::TextureFormat,
        output: (u32, u32),
        library: &Library,
        focus: usize,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let (scene, jobs) = Scene::new(library, focus, output);
        let (tx, rx) = channel();
        art::spawn(jobs, tx, wake);
        Self { scene, renderer: Renderer::new(&gpu.device, &gpu.queue, format), art: rx }
    }

    fn take(&mut self, gpu: &Gpu, loaded: art::Loaded) {
        let tex = (!loaded.mips.is_empty()).then(|| self.renderer.add_texture(&gpu.device, &gpu.queue, &loaded.mips));
        self.scene.set_art(loaded.slot, tex);
    }

    /// Uploads whatever art has been decoded since the last call.
    pub fn take_ready_art(&mut self, gpu: &Gpu) {
        while let Ok(loaded) = self.art.try_recv() {
            self.take(gpu, loaded);
        }
    }

    /// Waits for the next piece of art and uploads it.
    pub fn wait_for_art(&mut self, gpu: &Gpu) {
        if let Ok(loaded) = self.art.recv() {
            self.take(gpu, loaded);
        }
    }

    pub fn draw(&mut self, gpu: &Gpu, target: &wgpu::TextureView, output: (u32, u32)) -> wgpu::SubmissionIndex {
        let frame = self.scene.build(&mut self.renderer.text, output);
        self.renderer.render(&gpu.device, &gpu.queue, target, output, &frame)
    }
}

/// The Guide's landing tab. Over a game (`playing`, its place in the Recent row): the game, to go back to it, and
/// Quit. Over Home: Home, the Recent row's first games, and the way out to the desktop.
pub fn guide_entries(library: &Library, playing: Option<usize>) -> Vec<GuideEntry> {
    let recent = library.recent(RECENT_COUNT);
    let entry = |title: &str, detail: Option<&str>, game, action| GuideEntry {
        title: title.into(),
        detail: detail.map(Into::into),
        game,
        action,
    };
    if let Some(playing) = playing {
        let name = recent.get(playing).map_or("Game", |g| g.name.as_str());
        return vec![
            entry(name, Some("Running"), Some(playing), GuideAction::Close),
            entry("Quit", None, None, GuideAction::QuitGame),
        ];
    }
    let mut entries = vec![entry("Home", None, None, GuideAction::Close)];
    entries.extend(recent.iter().take(4).enumerate().map(|(i, game)| {
        let detail = (!game.installed).then_some("Not installed");
        entry(&game.name, detail, Some(i), GuideAction::Launch(i))
    }));
    entries.push(entry("Exit to desktop", None, None, GuideAction::Exit));
    entries
}
