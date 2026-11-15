//! Home, rendered natively: wgpu for the GPU, winit for the window, glyphon for text.
mod art;
mod gpu;
mod launch;
mod offscreen;
mod pad;
mod render;
mod scene;
mod window;

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;
use std::time::Instant;

use model::{Game, Library};

use crate::gpu::Gpu;
use crate::render::Renderer;
use crate::scene::Scene;

static START: OnceLock<Instant> = OnceLock::new();
static CLOCK: OnceLock<String> = OnceLock::new();

/// What the top bar's clock reads: `--clock`'s text, else the time.
pub fn clock() -> String {
    CLOCK.get().cloned().unwrap_or_else(|| chrono::Local::now().format("%-I:%M %p").to_string())
}

/// Milliseconds since the process started (since `main` was entered).
pub fn since_start() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

#[derive(Default)]
pub struct Args {
    library: Option<PathBuf>,
    steam: Option<PathBuf>,
    launch_screen: bool,
    guide: bool,
    screenshot: Option<PathBuf>,
    size: Option<(u32, u32)>,
    focus: usize,
    bench: bool,
    windowed: bool,
    /// What the clock reads, in place of the time: a screenshot test's is the same every run.
    clock: Option<String>,
}

const USAGE: &str = "usage: home [--steam <steam root> | --library <file.json>] [--windowed]
       home --screenshot <out.png> [--size WxH] [--focus N] [--bench] [--clock <text>] [--library <file.json>]";

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--library" => args.library = Some(value()?.into()),
            "--launch-screen" => args.launch_screen = true,
            "--guide" => args.guide = true,
            "--steam" => args.steam = Some(value()?.into()),
            "--screenshot" => args.screenshot = Some(value()?.into()),
            "--size" => {
                let value = value()?;
                let size = value.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
                args.size = Some(size.filter(|&(w, h)| w > 0 && h > 0).ok_or(format!("bad --size {value}"))?);
            }
            "--focus" => args.focus = value()?.parse().map_err(|e| format!("bad --focus: {e}"))?,
            "--bench" => args.bench = true,
            "--windowed" => args.windowed = true,
            "--clock" => args.clock = Some(value()?),
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(args)
}

/// The one place the Library comes from: `--library`'s JSON, `--steam`'s install, the
/// Steam install the registry names, else the built-in fake one.
fn load_library(args: &Args) -> Library {
    if let Some(path) = &args.library {
        let json = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        return serde_json::from_str(&json).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
    }
    match args.steam.clone().or_else(steam::find_steam_root) {
        Some(root) => {
            let t = std::time::Instant::now();
            let library = steam::load(&root).unwrap_or_else(|e| fail(&format!("{}: {e}", root.display())));
            eprintln!("steam: {} games from {} in {} ms", library.games.len(), root.display(), t.elapsed().as_millis());
            library
        }
        None => fake_library(),
    }
}

/// Ten games with no art, so Home always has something to show.
fn fake_library() -> Library {
    let names = [
        "Halo Infinite",
        "Forza Horizon 5",
        "Sea of Thieves",
        "Starfield",
        "Hades II",
        "Baldur's Gate 3",
        "Hollow Knight: Silksong",
        "Microsoft Flight Simulator",
        "Cyberpunk 2077",
        "Stardew Valley",
    ];
    let games = names.iter().enumerate().map(|(i, name)| Game {
        appid: 1000 + i as u32,
        name: name.to_string(),
        last_played: 1_790_000_000 - i as u64 * 86_400,
        playtime_minutes: 600 - i as u32 * 50,
        installed: true,
        install_dir: None,
        cover: None,
        hero: None,
        header: None,
    });
    Library { user: Some("Player One".into()), games: games.collect() }
}

fn fail(message: &str) -> ! {
    eprintln!("home: {message}");
    std::process::exit(2);
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

fn main() {
    since_start();
    let args = parse_args().unwrap_or_else(|e| fail(&format!("{e}\n{USAGE}")));
    if let Some(clock) = &args.clock {
        let _ = CLOCK.set(clock.clone());
    }
    let library = load_library(&args);
    if args.screenshot.is_some() {
        offscreen::run(&args, &library);
    } else if args.bench || args.size.is_some() {
        fail(&format!("--bench and --size go with --screenshot\n{USAGE}"));
    } else {
        window::run(&args, library);
    }
}
