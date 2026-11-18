//! The PC Game Pass catalogue the Xbox source lists (#277; docs/sources.md "Xbox"): which region, whether at all,
//! and where it comes from. One for the run, shared by the Xbox source (which asks it for the catalogue at each read)
//! and the window (which changes it as Settings does, then has Xbox read again).
//!
//! Where it comes from: `--gamepass <dir>` (or `DASHBOARD_GAMEPASS`) names a local catalogue, a test's; else
//! Microsoft's, kept in the data folder's `gamepass`. A picture offscreen (`--screenshot`) with neither asks nothing.
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use library::game_pass::{From, Hosts};
use model::game_pass::{Catalogue, Locale, Region};

use crate::Args;

/// The environment variable that names a local catalogue, as `--gamepass` does.
pub const ENV: &str = "DASHBOARD_GAMEPASS";

/// What is listed, as Settings has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Choice {
    shown: bool,
    /// The region chosen; None is Windows'.
    chosen: Option<Region>,
}

pub struct GamePass {
    from: Option<From>,
    choice: Mutex<Choice>,
    /// Windows' region and language, read once.
    windows: (Option<Region>, Option<String>),
}

static SHARED: OnceLock<Arc<GamePass>> = OnceLock::new();

/// The run's, made from `args` the first time.
pub fn shared(args: &Args) -> Arc<GamePass> {
    SHARED.get_or_init(|| Arc::new(GamePass::new(args))).clone()
}

/// The run's, once made.
pub fn get() -> Option<Arc<GamePass>> {
    SHARED.get().cloned()
}

impl GamePass {
    fn new(args: &Args) -> Self {
        let folder =
            args.gamepass.clone().or_else(|| std::env::var_os(ENV).filter(|v| !v.is_empty()).map(PathBuf::from));
        let data = args.data_dir.clone().or_else(|| args.screenshot.is_none().then(library::data_dir).flatten());
        let from = match (folder, data) {
            (Some(dir), _) => Some(From::Folder(dir)),
            (None, Some(data)) => {
                Some(From::Online { hosts: Hosts::default(), kept: Some(data.join(library::game_pass::FOLDER)) })
            }
            (None, None) => None,
        };
        let windows = source_xbox::region::windows();
        let region = windows.region.as_deref().and_then(Region::parse);
        eprintln!(
            "gamepass: Windows' region is {}, its language {}",
            region.map_or("not known".to_string(), |r| r.to_string()),
            windows.language.as_deref().unwrap_or("not known")
        );
        Self { from, choice: Mutex::new(Choice { shown: true, chosen: None }), windows: (region, windows.language) }
    }

    /// Windows' own region, for Settings' list.
    pub fn windows_region(&self) -> Option<Region> {
        self.windows.0
    }

    /// As Settings has it now.
    pub fn set(&self, shown: bool, chosen: Option<Region>) {
        if let Ok(mut choice) = self.choice.lock() {
            *choice = Choice { shown, chosen };
        }
    }

    /// The region and language the catalogue is asked in: the region chosen, else Windows', else the United States;
    /// the language Windows' display language, whatever the region (the catalogue answers in it where it can), else
    /// English for the region (`en-<CC>`).
    pub fn locale(&self) -> Locale {
        let chosen = self.choice.lock().map(|choice| choice.chosen).unwrap_or(None);
        let region = chosen.or(self.windows.0).unwrap_or(Region::US);
        let language = self.windows.1.clone().unwrap_or_else(|| format!("en-{region}"));
        Locale { market: region.to_string(), language }
    }

    /// The catalogue to list: none while it is switched off, or with nowhere to have it from.
    pub fn catalogue(&self) -> Option<Catalogue> {
        let shown = self.choice.lock().map(|choice| choice.shown).unwrap_or(true);
        if !shown {
            eprintln!("gamepass: Show Game Pass games is off: none listed");
            return None;
        }
        library::game_pass::catalogue(self.from.as_ref()?, &self.locale(), library::now())
    }
}
