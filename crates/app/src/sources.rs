//! The sources the Dashboard reads its games from, in order (docs/sources.md). Adding a store is its own crate,
//! which implements `model::Source`, and one line here.
use std::path::PathBuf;
use std::sync::Arc;

use model::{Game, Source};
use source_kit::HelperRoute;

use crate::Args;

/// Every source, each made once for the whole run.
pub fn all(args: &Args) -> Vec<Arc<dyn Source>> {
    vec![
        // `--steam` names the install; else the registry does.
        Arc::new(source_steam::Steam::at(args.steam.clone())),
        // `--epic` names a folder standing for the launcher's; else `%ProgramData%\Epic` and the new launcher's.
        // legendary too, where there is a data folder to keep it in (#175).
        Arc::new(match legendary_route(args) {
            Some(route) => source_epic::Epic::at(args.epic.clone()).with_legendary(route),
            None => source_epic::Epic::at(args.epic.clone()),
        }),
        // `--gog` names a folder standing for GOG's; else the registry and `%ProgramData%\GOG.com`. gogdl too (#176).
        Arc::new(match gogdl_route(args) {
            Some(route) => source_gog::Gog::at(args.gog.clone()).with_gogdl(route),
            None => source_gog::Gog::at(args.gog.clone()),
        }),
        // `--ea`, `--ubisoft` and `--battlenet` name a fake store; else this machine's are read.
        Arc::new(source_ea::Ea::at(args.ea.clone())),
        Arc::new(source_ubisoft::Ubisoft::at(args.ubisoft.clone())),
        Arc::new(source_battlenet::BattleNet::at(args.battlenet.clone())),
        // `--xbox` names a fake drive of Xbox games; else this machine's drives and the user's packages. With the PC
        // Game Pass catalogue of the region Settings names (#277).
        Arc::new(source_xbox::Xbox::at(args.xbox.clone()).with_game_pass({
            let game_pass = crate::game_pass::shared(args);
            Arc::new(move || game_pass.catalogue())
        })),
        // The games added by hand, which are the database's own (docs/spec/apps.md).
        Arc::new(library::by_hand::Manual),
    ]
}

/// The data folder helpers are kept in: `--data-dir`, else, with a window, the user's.
fn data(args: &Args) -> Option<PathBuf> {
    args.data_dir.clone().or_else(|| args.screenshot.is_none().then(library::data_dir).flatten())
}

/// legendary's own folder for its records: `--helper-home`'s `legendary` (a test's), else legendary's own default,
/// so that a sign-in made with legendary itself is the Dashboard's (`source_epic::legendary::default_home`).
pub fn legendary_home(args: &Args) -> Option<PathBuf> {
    match &args.helper_home {
        Some(dir) => Some(dir.join("legendary")),
        None => source_epic::legendary::default_home(),
    }
}

/// The Dashboard's own folder for gogdl (its sign-in, manifests and records): `--helper-home`'s `gogdl` (a test's),
/// else the data folder's `gogdl`. gogdl keeps nothing a user would have made with it by hand.
pub fn gogdl_home(args: &Args) -> Option<PathBuf> {
    match &args.helper_home {
        Some(dir) => Some(dir.join("gogdl")),
        None => data(args).map(|data| data.join("gogdl")),
    }
}

/// Where a helper installs: `--helper-home`'s `games`, else the user's `Games`.
fn games(args: &Args) -> Option<PathBuf> {
    match &args.helper_home {
        Some(dir) => Some(dir.join("games")),
        None => source_epic::legendary::default_games(),
    }
}

/// The helper `key`'s program in the data folder's `helpers\<key>`, while it is added at the table's version.
fn kept_program(args: &Args, key: &str) -> Option<Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>> {
    let data = data(args)?;
    let helper = crate::helper_programs::table(args).into_iter().find(|helper| helper.key == key)?;
    Some(Arc::new(move || {
        let kept = helpers::kept(&data, &helper).filter(|kept| kept.version == helper.version);
        kept.map(|_| helpers::program(&data, &helper))
    }))
}

/// Where legendary is run from, where its records are, and where it installs.
fn legendary_route(args: &Args) -> Option<HelperRoute> {
    let (program, home, games) = (kept_program(args, "legendary")?, legendary_home(args)?, games(args)?);
    // A test's folder is legendary's for every run of it, `launch` too (`main`): legendary reads it from there.
    let env = vec![("LEGENDARY_CONFIG_PATH".to_string(), home.clone().into_os_string())];
    let env = if args.helper_home.is_some() { env } else { Vec::new() };
    Some(HelperRoute { program, home, env, games })
}

/// Where gogdl is run from, the Dashboard's folder for it, and where it installs.
fn gogdl_route(args: &Args) -> Option<HelperRoute> {
    let (program, home, games) = (kept_program(args, "gogdl")?, gogdl_home(args)?, games(args)?);
    Some(HelperRoute { program, home, env: Vec::new(), games })
}

/// Every store's achievements (docs/achievements.md): Steam's alone, from its own files.
pub fn achievements(args: &Args) -> Vec<Arc<dyn model::AchievementSource>> {
    vec![Arc::new(source_steam::achievements::SteamAchievements::at(args.steam.clone()))]
}

/// The source the game is from, if the Dashboard has it.
pub fn of<'a>(sources: &'a [Arc<dyn Source>], game: &Game) -> Option<&'a Arc<dyn Source>> {
    sources.iter().find(|source| source.key() == game.source)
}
