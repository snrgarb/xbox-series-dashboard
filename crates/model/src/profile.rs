//! Profiles (docs/spec/profiles.md): who plays on this PC. A Profile is a gamertag, a real name and a picture, and
//! nothing more: every option is shared by every Profile. The library database keeps them; a snapshot carries them
//! (`Library::profiles`), with the one signed in.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many plain pictures are built in: `Picture::Shape(0)` to `Picture::Shape(SHAPES - 1)`.
pub const SHAPES: u8 = 8;

/// What a Profile wears.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Picture {
    /// One of the built-in plain pictures, drawn rather than read: 0 is Windows' look for an account with no
    /// picture of its own, a grey figure on a pale disc.
    Shape(u8),
    /// A picture file: in a snapshot, the Profile's own copy in the data folder; in a change, the file to copy.
    File(PathBuf),
}

impl Default for Picture {
    fn default() -> Self {
        Picture::Shape(0)
    }
}

impl Picture {
    /// As the library database keeps it: `shape:<n>`, or the file's path.
    pub fn to_text(&self) -> String {
        match self {
            Picture::Shape(n) => format!("shape:{n}"),
            Picture::File(path) => path.to_string_lossy().into_owned(),
        }
    }

    /// What `to_text` wrote. A shape out of range is the first; an empty text is no file but the first shape.
    pub fn from_text(text: &str) -> Picture {
        match text.strip_prefix("shape:") {
            Some(n) => Picture::Shape(n.trim().parse().ok().filter(|&n| n < SHAPES).unwrap_or(0)),
            None if text.trim().is_empty() => Picture::Shape(0),
            None => Picture::File(PathBuf::from(text)),
        }
    }

    /// What the log says of it: `shape 3`, or the file's name.
    pub fn describe(&self) -> String {
        match self {
            Picture::Shape(n) => format!("shape {n}"),
            Picture::File(path) => {
                path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into())
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Its row in the library database.
    pub id: i64,
    pub gamertag: String,
    /// The person's own name; may be empty.
    #[serde(default)]
    pub real_name: String,
    #[serde(default)]
    pub picture: Picture,
    /// The file its picture was copied from, if it was: what the Picture page ticks.
    #[serde(default)]
    pub source: Option<PathBuf>,
}

impl Profile {
    /// Whether it has a real name: only then does Home's top bar take turns between it and the gamertag.
    pub fn has_real_name(&self) -> bool {
        !self.real_name.trim().is_empty()
    }

    /// The name "Hi X!" greets: the real name's first word, or the gamertag without one.
    pub fn first_name(&self) -> &str {
        self.real_name.split_whitespace().next().unwrap_or(&self.gamertag)
    }

    /// Whether it wears this picture, as the Picture page offers it: the same shape, or a copy of the same file.
    pub fn wears(&self, offered: &Picture) -> bool {
        match (offered, &self.picture) {
            (Picture::Shape(a), Picture::Shape(b)) => a == b,
            (Picture::File(path), _) => self.source.as_deref() == Some(path.as_path()),
            _ => false,
        }
    }
}

/// The Profiles, as a snapshot of the library database has them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profiles {
    /// Every Profile, in the order they were made.
    pub list: Vec<Profile>,
    /// The one signed in, which is the one signed in last: the database remembers it.
    pub signed_in: Option<i64>,
    /// The picture files a Profile may wear besides the plain ones: the Windows account picture, then the
    /// Gamerpics folder's, by name. Not the database's: the executable lists them for every snapshot.
    #[serde(default)]
    pub pictures: Vec<PathBuf>,
}

impl Profiles {
    pub fn get(&self, id: i64) -> Option<&Profile> {
        self.list.iter().find(|p| p.id == id)
    }

    /// The Profile signed in: the one the database names, or the first if it names none there is.
    pub fn signed_in(&self) -> Option<&Profile> {
        self.signed_in.and_then(|id| self.get(id)).or(self.list.first())
    }
}

/// A change to the Profiles, as Settings' Profiles pages ask for it. Whoever owns the library database carries it
/// out (`library::Database::change_profile`) and shows the snapshot after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileChange {
    /// A new Profile of this gamertag, last, wearing the first plain picture.
    Add { gamertag: String },
    /// Its gamertag: trimmed, and never empty.
    Gamertag { profile: i64, gamertag: String },
    /// Its real name: trimmed, and may be empty.
    RealName { profile: i64, real_name: String },
    /// Its picture: a plain one, or a file that is copied into the data folder.
    Picture { profile: i64, picture: Picture },
    /// It goes; never the last one. The one signed in gone, the first signs in.
    Remove(i64),
    /// It signs in, and is remembered as the one signed in last: Who's playing today?'s (#127).
    SignIn(i64),
}

/// The picture file extensions a Profile's picture may have.
pub const PICTURE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "bmp"];

/// Whether this file is a picture by its name.
pub fn is_picture(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PICTURE_EXTENSIONS.iter().any(|known| known.eq_ignore_ascii_case(e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_kept_as_text_and_read_back() {
        for picture in [Picture::Shape(0), Picture::Shape(7), Picture::File(PathBuf::from(r"C:\Pics\me.png"))] {
            assert_eq!(Picture::from_text(&picture.to_text()), picture);
        }
        assert_eq!(Picture::from_text("shape:99"), Picture::Shape(0));
        assert_eq!(Picture::from_text(""), Picture::Shape(0));
    }

    #[test]
    fn the_first_name_is_the_real_names_first_word_or_the_gamertag() {
        let mut profile = Profile { gamertag: "Ashfall".into(), ..Profile::default() };
        assert_eq!(profile.first_name(), "Ashfall");
        assert!(!profile.has_real_name());
        profile.real_name = "  Ann  Smith ".into();
        assert_eq!(profile.first_name(), "Ann");
        assert!(profile.has_real_name());
    }

    #[test]
    fn the_one_signed_in_is_the_one_named_or_the_first() {
        let profile = |id| Profile { id, gamertag: format!("p{id}"), ..Profile::default() };
        let mut profiles = Profiles { list: vec![profile(1), profile(2)], signed_in: Some(2), pictures: vec![] };
        assert_eq!(profiles.signed_in().unwrap().id, 2);
        profiles.signed_in = Some(9);
        assert_eq!(profiles.signed_in().unwrap().id, 1);
        assert!(Profiles::default().signed_in().is_none());
    }
}
