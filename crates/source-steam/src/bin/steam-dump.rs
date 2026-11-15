//! `steam-dump [steam_root]`: print the Steam library as JSON.
use anyhow::{Context, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let root = match std::env::args_os().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => {
            source_steam::find_steam_root().context("Steam not found; pass the Steam folder as the first argument")?
        }
    };
    let lib = source_steam::load(&root).with_context(|| format!("reading Steam at {}", root.display()))?;
    println!("{}", serde_json::to_string_pretty(&lib)?);
    let g = &lib.games;
    eprintln!(
        "user={} games={} played={} installed={} cover={} hero={} header={}",
        lib.user.as_deref().unwrap_or("?"),
        g.len(),
        g.iter().filter(|x| x.last_played > 0).count(),
        g.iter().filter(|x| x.installed).count(),
        g.iter().filter(|x| x.cover.is_some()).count(),
        g.iter().filter(|x| x.hero.is_some()).count(),
        g.iter().filter(|x| x.header.is_some()).count(),
    );
    Ok(())
}
