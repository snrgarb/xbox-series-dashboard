//! Our own art cache: the pictures downloaded for games their store gives none for, in the data folder, one folder a
//! game: `art/<source>/<source_id>/{cover,hero,header}.{png,jpg,webp}`. A picture that was looked for and not found
//! leaves `<kind>.none` in its place, holding when (Unix seconds), so that it is not asked for again until
//! [`MISS_DAYS`] have passed. Nothing here is a store's: clearing the cache removes this folder and nothing else.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// How long a picture that was not found is not asked for again.
pub const MISS_DAYS: u64 = 7;

/// A game's three pictures, as a tile, Home's background and the Game hub use them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Portrait, 600x900: a tile's face.
    Cover,
    /// Wide background art.
    Hero,
    /// Landscape, 920x430 or 460x215.
    Header,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Cover, Kind::Hero, Kind::Header];

    /// Its file's name without the extension.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Cover => "cover",
            Kind::Hero => "hero",
            Kind::Header => "header",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// What a picture is, by its first bytes: its extension, for PNG, JPEG and WebP alone.
pub fn extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// The cache's folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtDir {
    root: PathBuf,
}

/// The pictures the cache has of each game, by `(source, source_id)`: cover, hero, header.
pub type Index = HashMap<(String, String), [Option<PathBuf>; 3]>;

impl ArtDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The cache in a data folder: its `art` folder.
    pub fn under(data_dir: &Path) -> Self {
        Self::new(data_dir.join("art"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn game_dir(&self, source: &str, source_id: &str) -> PathBuf {
        self.root.join(escape(source)).join(escape(source_id))
    }

    /// The picture the cache has of a game, if it has one.
    pub fn find(&self, source: &str, source_id: &str, kind: Kind) -> Option<PathBuf> {
        let dir = self.game_dir(source, source_id);
        ["png", "jpg", "webp"].into_iter().map(|ext| dir.join(format!("{}.{ext}", kind.name()))).find(|p| p.is_file())
    }

    /// Every picture the cache has, read in one walk of its folder: what a snapshot of the library fills its games'
    /// missing art from. An empty or missing folder is an empty index.
    pub fn index(&self) -> Index {
        let mut index = Index::new();
        let Ok(sources) = std::fs::read_dir(&self.root) else { return index };
        for source in sources.flatten() {
            let Some(source_key) = source.file_name().to_str().and_then(unescape) else { continue };
            let Ok(games) = std::fs::read_dir(source.path()) else { continue };
            for game in games.flatten() {
                let Some(id) = game.file_name().to_str().and_then(unescape) else { continue };
                let Ok(files) = std::fs::read_dir(game.path()) else { continue };
                let mut found: [Option<PathBuf>; 3] = Default::default();
                for file in files.flatten() {
                    let path = file.path();
                    let (Some(stem), Some(ext)) = (path.file_stem().and_then(|s| s.to_str()), path.extension()) else {
                        continue;
                    };
                    if !matches!(ext.to_str(), Some("png" | "jpg" | "webp")) {
                        continue;
                    }
                    if let Some(kind) = Kind::ALL.into_iter().find(|kind| kind.name() == stem) {
                        found[kind.index()] = Some(path);
                    }
                }
                if found.iter().any(Option::is_some) {
                    index.insert((source_key.clone(), id), found);
                }
            }
        }
        index
    }

    /// Whether the picture was looked for and not found less than [`MISS_DAYS`] before `now`.
    pub fn missed(&self, source: &str, source_id: &str, kind: Kind, now: u64) -> bool {
        let file = self.game_dir(source, source_id).join(format!("{}.none", kind.name()));
        let Ok(text) = std::fs::read_to_string(file) else { return false };
        let Ok(when) = text.trim().parse::<u64>() else { return false };
        now < when.saturating_add(MISS_DAYS * 24 * 60 * 60)
    }

    /// Remembers that the picture was looked for at `now` and not found.
    pub fn miss(&self, source: &str, source_id: &str, kind: Kind, now: u64) -> Result<()> {
        let dir = self.game_dir(source, source_id);
        std::fs::create_dir_all(&dir).with_context(|| format!("making {}", dir.display()))?;
        std::fs::write(dir.join(format!("{}.none", kind.name())), now.to_string())?;
        Ok(())
    }

    /// Keeps a downloaded picture, written whole or not at all; gives where. Bytes that are not a PNG, a JPEG or a
    /// WebP are refused.
    pub fn save(&self, source: &str, source_id: &str, kind: Kind, bytes: &[u8]) -> Result<PathBuf> {
        let ext = extension(bytes).context("not a PNG, JPEG or WebP picture")?;
        let dir = self.game_dir(source, source_id);
        std::fs::create_dir_all(&dir).with_context(|| format!("making {}", dir.display()))?;
        let path = dir.join(format!("{}.{ext}", kind.name()));
        let part = dir.join(format!("{}.part", kind.name()));
        std::fs::write(&part, bytes)?;
        std::fs::rename(&part, &path)?;
        // One picture of a kind: an older one of another format, and the remembered miss, go.
        for other in ["png", "jpg", "webp", "none"].into_iter().filter(|&other| other != ext) {
            let _ = std::fs::remove_file(dir.join(format!("{}.{other}", kind.name())));
        }
        Ok(path)
    }

    /// Removes the whole cache, pictures and remembered misses; gives how many pictures went.
    pub fn clear(&self) -> Result<usize> {
        let pictures = self.index().values().map(|found| found.iter().flatten().count()).sum();
        match std::fs::remove_dir_all(&self.root) {
            Ok(()) => Ok(pictures),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(e).with_context(|| format!("removing {}", self.root.display())),
        }
    }
}

/// A store's key or id as a folder's name: letters, digits, `-` and `_` as they are, every other byte `%XX`, so any
/// id is a name every file system takes and two ids are never one folder.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    if out.is_empty() {
        out.push('%');
    }
    out
}

fn unescape(name: &str) -> Option<String> {
    if name == "%" {
        return Some(String::new());
    }
    let mut bytes = Vec::with_capacity(name.len());
    let mut rest = name.as_bytes();
    while let Some((&first, tail)) = rest.split_first() {
        if first == b'%' {
            let hex = std::str::from_utf8(tail.get(..2)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = &tail[2..];
        } else {
            bytes.push(first);
            rest = tail;
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";

    #[test]
    fn any_id_is_a_folder_of_its_own_and_comes_back_as_it_was() {
        for id in ["440", "Fortnite", "Origin.OFR.50.0001", "a/b\\c:d", "..", "", "café"] {
            let name = escape(id);
            assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || "-_%".contains(c)), "{name}");
            assert_eq!(unescape(&name).as_deref(), Some(id));
        }
        assert_ne!(escape("a.b"), escape("a_b"));
    }

    #[test]
    fn a_picture_is_kept_found_indexed_and_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        assert!(art.index().is_empty() && art.find("epic", "Moss", Kind::Cover).is_none());
        assert!(art.save("epic", "Moss", Kind::Cover, b"not a picture").is_err());
        let saved = art.save("epic", "Moss", Kind::Cover, PNG).unwrap();
        assert!(saved.ends_with("art/epic/Moss/cover.png"));
        assert_eq!(art.find("epic", "Moss", Kind::Cover), Some(saved.clone()));
        let index = art.index();
        assert_eq!(index[&("epic".into(), "Moss".into())], [Some(saved), None, None]);
        assert_eq!(art.clear().unwrap(), 1);
        assert!(!art.root().exists() && art.index().is_empty());
        assert_eq!(art.clear().unwrap(), 0);
    }

    #[test]
    fn a_miss_is_remembered_for_a_week() {
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let now = 1_800_000_000;
        assert!(!art.missed("gog", "1", Kind::Hero, now));
        art.miss("gog", "1", Kind::Hero, now).unwrap();
        assert!(art.missed("gog", "1", Kind::Hero, now + 6 * 86_400));
        assert!(!art.missed("gog", "1", Kind::Hero, now + 7 * 86_400));
        assert!(!art.missed("gog", "1", Kind::Cover, now));
        // A picture found later takes the miss's place.
        art.save("gog", "1", Kind::Hero, PNG).unwrap();
        assert!(!art.missed("gog", "1", Kind::Hero, now));
    }
}
