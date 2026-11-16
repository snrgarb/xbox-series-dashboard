//! The sources the Dashboard reads its games from, in order (docs/sources.md). Adding a store is its own crate,
//! which implements `model::Source`, and one line here.
use std::sync::Arc;

use model::{Game, Source};

use crate::Args;

/// Every source, each made once for the whole run.
pub fn all(args: &Args) -> Vec<Arc<dyn Source>> {
    vec![
        // `--steam` names the install; else the registry does.
        Arc::new(source_steam::Steam::at(args.steam.clone())),
        // `--epic` names a folder standing for the launcher's; else `%ProgramData%\Epic` and the new launcher's.
        Arc::new(source_epic::Epic::at(args.epic.clone())),
        // `--gog` names a folder standing for GOG's; else the registry and `%ProgramData%\GOG.com`.
        Arc::new(source_gog::Gog::at(args.gog.clone())),
        // `--ea`, `--ubisoft` and `--battlenet` name a fake store; else this machine's are read.
        Arc::new(source_ea::Ea::at(args.ea.clone())),
        Arc::new(source_ubisoft::Ubisoft::at(args.ubisoft.clone())),
        Arc::new(source_battlenet::BattleNet::at(args.battlenet.clone())),
        // The games added by hand, which are the database's own (docs/spec/apps.md).
        Arc::new(library::by_hand::Manual),
    ]
}

/// The source the game is from, if the Dashboard has it.
pub fn of<'a>(sources: &'a [Arc<dyn Source>], game: &Game) -> Option<&'a Arc<dyn Source>> {
    sources.iter().find(|source| source.key() == game.source)
}
