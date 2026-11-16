//! Profiles (docs/spec/profiles.md): a row of `profiles` each (migration 6), its picture a built-in plain one or a
//! file of its own in the data folder's `profiles` folder, and which signed in last a setting, `profile.last`.
//!
//! There is always one once the executable has opened the database: the first is made from the Windows user
//! (`first_profile`), and the last is never removed.
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use model::{Picture, Profile, ProfileChange, Profiles};
use rusqlite::{params, OptionalExtension};

use crate::Database;

/// Migration 6: the Profiles.
pub const MIGRATION: &str = "CREATE TABLE profiles (
        id INTEGER PRIMARY KEY,
        gamertag TEXT NOT NULL,
        real_name TEXT NOT NULL DEFAULT '',
        -- 'shape:<n>', a built-in plain picture; else its own file, by its name in the data folder's profiles
        -- folder, or by its full path where the database has no folder.
        picture TEXT NOT NULL DEFAULT 'shape:0',
        -- The file the picture was copied from, if it was one.
        source TEXT
    );";

/// The setting that names the Profile signed in last.
pub const LAST: &str = "profile.last";

/// The folder in the data folder that holds the Profiles' pictures.
pub const FOLDER: &str = "profiles";

impl Database {
    /// Every Profile, in the order they were made, and the one signed in last.
    pub fn profiles(&self) -> Result<Profiles> {
        let mut rows =
            self.conn.prepare("SELECT id, gamertag, real_name, picture, source FROM profiles ORDER BY id")?;
        let list = rows
            .query_map([], |row| {
                let picture = match Picture::from_text(&row.get::<_, String>(3)?) {
                    Picture::File(file) => Picture::File(self.picture_path(&file)),
                    shape => shape,
                };
                Ok(Profile {
                    id: row.get(0)?,
                    gamertag: row.get(1)?,
                    real_name: row.get(2)?,
                    picture,
                    source: row.get::<_, Option<String>>(4)?.map(PathBuf::from),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let last = self.setting(LAST)?.and_then(|id| id.trim().parse::<i64>().ok());
        let signed_in = last.filter(|id| list.iter().any(|p| p.id == *id)).or(list.first().map(|p| p.id));
        Ok(Profiles { list, signed_in, pictures: Vec::new() })
    }

    /// With no Profile yet, makes the first: of this name, wearing a copy of `picture` if there is one, else the
    /// first plain picture; and it is the one signed in. Says what it made, or None when there were Profiles.
    pub fn first_profile(&mut self, name: &str, picture: Option<&Path>) -> Result<Option<String>> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))?;
        if count > 0 {
            return Ok(None);
        }
        let name = match name.trim() {
            "" => "Player",
            name => name,
        };
        self.conn.execute("INSERT INTO profiles (gamertag) VALUES (?1)", [name])?;
        let id = self.conn.last_insert_rowid();
        self.set_setting(LAST, &id.to_string())?;
        let worn = match picture {
            Some(file) => match self.wear_file(id, file) {
                Ok(()) => "its account picture",
                Err(e) => {
                    eprintln!("profiles: the account picture: {e:#}");
                    "a plain picture"
                }
            },
            None => "a plain picture",
        };
        Ok(Some(format!("made {id} {name:?} from the Windows user, wearing {worn}")))
    }

    /// Carries out a change Settings' Profiles pages asked for. Gives what the log says of it: `added 2 "Alpha"`,
    /// `2 is now "Alpha Two"`, `removed 1 "Tester"; 2 is signed in`.
    pub fn change_profile(&mut self, change: &ProfileChange) -> Result<String> {
        let gamertag = |id: i64| -> Result<String> {
            self.conn
                .query_row("SELECT gamertag FROM profiles WHERE id = ?1", [id], |row| row.get(0))
                .optional()?
                .with_context(|| format!("there is no profile {id}"))
        };
        match change {
            ProfileChange::Add { gamertag } => {
                let gamertag = named(gamertag)?;
                self.conn.execute("INSERT INTO profiles (gamertag) VALUES (?1)", [&gamertag])?;
                Ok(format!("added {} {gamertag:?}", self.conn.last_insert_rowid()))
            }
            ProfileChange::Gamertag { profile, gamertag: new } => {
                gamertag(*profile)?;
                let new = named(new)?;
                self.conn.execute("UPDATE profiles SET gamertag = ?2 WHERE id = ?1", params![profile, new])?;
                Ok(format!("{profile} is now {new:?}"))
            }
            ProfileChange::RealName { profile, real_name } => {
                gamertag(*profile)?;
                let real_name = real_name.trim();
                self.conn.execute("UPDATE profiles SET real_name = ?2 WHERE id = ?1", params![profile, real_name])?;
                Ok(format!("{profile}'s real name is {real_name:?}"))
            }
            ProfileChange::Picture { profile, picture } => {
                gamertag(*profile)?;
                match picture {
                    Picture::Shape(_) => {
                        let old = self.own_file(*profile)?;
                        self.conn.execute(
                            "UPDATE profiles SET picture = ?2, source = NULL WHERE id = ?1",
                            params![profile, picture.to_text()],
                        )?;
                        forget(old);
                    }
                    Picture::File(file) => self.wear_file(*profile, file)?,
                }
                Ok(format!("{profile} wears {}", picture.describe()))
            }
            ProfileChange::Remove(profile) => {
                let name = gamertag(*profile)?;
                let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))?;
                if count < 2 {
                    bail!("{name} is the only profile, and stays");
                }
                let signed_in = self.profiles()?.signed_in;
                let old = self.own_file(*profile)?;
                self.conn.execute("DELETE FROM profiles WHERE id = ?1", [profile])?;
                forget(old);
                if signed_in == Some(*profile) {
                    let first: i64 =
                        self.conn.query_row("SELECT id FROM profiles ORDER BY id LIMIT 1", [], |row| row.get(0))?;
                    self.set_setting(LAST, &first.to_string())?;
                    return Ok(format!("removed {profile} {name:?}; {first} is signed in"));
                }
                Ok(format!("removed {profile} {name:?}"))
            }
            ProfileChange::SignIn(profile) => {
                let name = gamertag(*profile)?;
                self.set_setting(LAST, &profile.to_string())?;
                Ok(format!("{profile} {name:?} signed in"))
            }
        }
    }

    /// Copies `file` into the data folder as the Profile's picture, under a name of its own each time (so a picture
    /// changed is never mistaken for the one decoded before), and forgets the copy it had. With no data folder the
    /// file is worn where it is.
    fn wear_file(&mut self, profile: i64, file: &Path) -> Result<()> {
        if !file.is_file() {
            bail!("there's no picture at {}", file.display());
        }
        let kept = match self.profiles_dir() {
            Some(dir) => {
                std::fs::create_dir_all(&dir).with_context(|| format!("making {}", dir.display()))?;
                let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
                let stamp =
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
                let name = format!("{profile}-{stamp}.{extension}");
                std::fs::copy(file, dir.join(&name))
                    .with_context(|| format!("copying {} into {}", file.display(), dir.display()))?;
                name
            }
            None => crate::text(file),
        };
        let old = self.own_file(profile)?;
        self.conn.execute(
            "UPDATE profiles SET picture = ?2, source = ?3 WHERE id = ?1",
            params![profile, kept, crate::text(file)],
        )?;
        forget(old);
        Ok(())
    }

    /// The Profile's own copy of its picture in the data folder, if it has one.
    fn own_file(&self, profile: i64) -> Result<Option<PathBuf>> {
        let picture: Option<String> = self
            .conn
            .query_row("SELECT picture FROM profiles WHERE id = ?1", [profile], |row| row.get(0))
            .optional()?;
        let Some(Picture::File(file)) = picture.as_deref().map(Picture::from_text) else { return Ok(None) };
        let dir = self.profiles_dir();
        Ok(dir.filter(|_| file.is_relative()).map(|dir| dir.join(file)))
    }

    /// Where a picture the database names is: in the profiles folder, unless it is a full path.
    fn picture_path(&self, file: &Path) -> PathBuf {
        match self.profiles_dir() {
            Some(dir) if file.is_relative() => dir.join(file),
            _ => file.to_path_buf(),
        }
    }

    fn profiles_dir(&self) -> Option<PathBuf> {
        self.folder.as_ref().map(|folder| folder.join(FOLDER))
    }
}

/// A gamertag trimmed; an empty one is refused.
fn named(gamertag: &str) -> Result<String> {
    match gamertag.trim() {
        "" => bail!("a profile's gamertag can't be empty"),
        name => Ok(name.to_string()),
    }
}

/// Deletes a copy no Profile wears now. One that can't be is left, and said.
fn forget(file: Option<PathBuf>) {
    if let Some(file) = file {
        if let Err(e) = std::fs::remove_file(&file) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("profiles: {}: {e}", file.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"not really a picture").unwrap();
        path
    }

    fn names(db: &Database) -> Vec<String> {
        db.profiles().unwrap().list.into_iter().map(|p| p.gamertag).collect()
    }

    #[test]
    fn the_first_profile_is_made_once_from_the_windows_user_and_signed_in() {
        let mut db = Database::in_memory().unwrap();
        assert_eq!(db.profiles().unwrap(), Profiles::default());
        let made = db.first_profile("  tester ", None).unwrap().unwrap();
        assert!(made.contains("\"tester\"") && made.ends_with("a plain picture"), "{made}");
        assert_eq!(db.first_profile("someone else", None).unwrap(), None);
        let profiles = db.profiles().unwrap();
        assert_eq!(names(&db), ["tester"]);
        assert_eq!(profiles.signed_in, Some(profiles.list[0].id));
        assert_eq!(profiles.list[0].picture, Picture::Shape(0));
        // No name at all is a Player.
        let mut db = Database::in_memory().unwrap();
        db.first_profile(" ", None).unwrap();
        assert_eq!(names(&db), ["Player"]);
    }

    #[test]
    fn the_account_picture_is_copied_into_the_data_folder() {
        let dir = tempfile::tempdir().unwrap();
        let account = picture(dir.path(), "user-192.png");
        let mut db = Database::open(&dir.path().join("data").join(crate::FILE)).unwrap();
        db.first_profile("tester", Some(&account)).unwrap();
        let profile = db.profiles().unwrap().list.remove(0);
        let Picture::File(copy) = &profile.picture else { panic!("{:?}", profile.picture) };
        assert!(copy.starts_with(dir.path().join("data").join(FOLDER)) && copy.is_file(), "{}", copy.display());
        assert_eq!(profile.source.as_deref(), Some(account.as_path()));
        assert!(profile.wears(&Picture::File(account.clone())));
        // An account picture that isn't there: the plain one.
        let mut db = Database::open(&dir.path().join("other").join(crate::FILE)).unwrap();
        let made = db.first_profile("tester", Some(&dir.path().join("none.png"))).unwrap().unwrap();
        assert!(made.ends_with("a plain picture"), "{made}");
    }

    #[test]
    fn profiles_are_added_renamed_named_and_removed_but_never_the_last() {
        let mut db = Database::in_memory().unwrap();
        db.first_profile("tester", None).unwrap();
        let first = db.profiles().unwrap().list[0].id;
        let added = db.change_profile(&ProfileChange::Add { gamertag: " Alpha ".into() }).unwrap();
        let alpha = db.profiles().unwrap().list[1].id;
        assert_eq!(added, format!("added {alpha} \"Alpha\""));
        assert!(db.change_profile(&ProfileChange::Add { gamertag: "  ".into() }).is_err());
        let renamed = ProfileChange::Gamertag { profile: alpha, gamertag: "Alpha Two".into() };
        assert_eq!(db.change_profile(&renamed).unwrap(), format!("{alpha} is now \"Alpha Two\""));
        assert!(db.change_profile(&ProfileChange::Gamertag { profile: alpha, gamertag: "".into() }).is_err());
        db.change_profile(&ProfileChange::RealName { profile: alpha, real_name: " Ann Smith ".into() }).unwrap();
        assert_eq!(names(&db), ["tester", "Alpha Two"]);
        assert_eq!(db.profiles().unwrap().get(alpha).unwrap().real_name, "Ann Smith");
        // Removing one that is not signed in leaves the one that is.
        db.change_profile(&ProfileChange::Remove(alpha)).unwrap();
        assert_eq!((names(&db), db.profiles().unwrap().signed_in), (vec!["tester".to_string()], Some(first)));
        assert!(db.change_profile(&ProfileChange::Remove(first)).is_err(), "the last one stays");
        assert!(db.change_profile(&ProfileChange::Remove(987)).is_err());
    }

    #[test]
    fn the_one_signed_in_last_is_remembered_and_the_first_signs_in_when_it_goes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(crate::FILE);
        let (first, beta) = {
            let mut db = Database::open(&path).unwrap();
            db.first_profile("tester", None).unwrap();
            db.change_profile(&ProfileChange::Add { gamertag: "Alpha".into() }).unwrap();
            db.change_profile(&ProfileChange::Add { gamertag: "Beta".into() }).unwrap();
            let ids: Vec<i64> = db.profiles().unwrap().list.iter().map(|p| p.id).collect();
            let signed = db.change_profile(&ProfileChange::SignIn(ids[2])).unwrap();
            assert_eq!(signed, format!("{} \"Beta\" signed in", ids[2]));
            (ids[0], ids[2])
        };
        // Opened again, Beta is still the one.
        let mut db = Database::open(&path).unwrap();
        assert_eq!(db.profiles().unwrap().signed_in, Some(beta));
        let removed = db.change_profile(&ProfileChange::Remove(beta)).unwrap();
        assert_eq!(removed, format!("removed {beta} \"Beta\"; {first} is signed in"));
        assert_eq!(db.profiles().unwrap().signed_in, Some(first));
        assert!(db.change_profile(&ProfileChange::SignIn(beta)).is_err());
    }

    #[test]
    fn a_picture_file_is_copied_and_its_old_copy_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let mut db = Database::open(&data.join(crate::FILE)).unwrap();
        db.first_profile("tester", None).unwrap();
        let id = db.profiles().unwrap().list[0].id;
        let file = |db: &Database| match &db.profiles().unwrap().list[0].picture {
            Picture::File(path) => Some(path.clone()),
            Picture::Shape(_) => None,
        };
        let one = picture(dir.path(), "One.JPG");
        let said = db.change_profile(&ProfileChange::Picture { profile: id, picture: Picture::File(one) }).unwrap();
        assert_eq!(said, format!("{id} wears One.JPG"));
        let first = file(&db).unwrap();
        assert!(first.is_file() && first.extension().unwrap() == "jpg");
        let two = picture(dir.path(), "two.png");
        db.change_profile(&ProfileChange::Picture { profile: id, picture: Picture::File(two.clone()) }).unwrap();
        let second = file(&db).unwrap();
        assert!(second.is_file() && !first.exists() && second != first);
        assert!(db.profiles().unwrap().list[0].wears(&Picture::File(two)));
        // A plain one: the copy goes.
        db.change_profile(&ProfileChange::Picture { profile: id, picture: Picture::Shape(3) }).unwrap();
        assert!(file(&db).is_none() && !second.exists());
        assert!(db.profiles().unwrap().list[0].wears(&Picture::Shape(3)));
        // A file that isn't there changes nothing.
        let none = Picture::File(dir.path().join("none.png"));
        assert!(db.change_profile(&ProfileChange::Picture { profile: id, picture: none }).is_err());
        assert_eq!(db.profiles().unwrap().list[0].picture, Picture::Shape(3));
        // A Profile removed takes its copy with it.
        db.change_profile(&ProfileChange::Add { gamertag: "Alpha".into() }).unwrap();
        let alpha = db.profiles().unwrap().list[1].id;
        let three = picture(dir.path(), "three.png");
        db.change_profile(&ProfileChange::Picture { profile: alpha, picture: Picture::File(three) }).unwrap();
        let Picture::File(copy) = db.profiles().unwrap().list[1].picture.clone() else { panic!() };
        db.change_profile(&ProfileChange::Remove(alpha)).unwrap();
        assert!(!copy.exists());
    }

    #[test]
    fn a_snapshot_carries_the_profiles() {
        let mut db = Database::in_memory().unwrap();
        db.first_profile("tester", None).unwrap();
        let snapshot = db.snapshot().unwrap();
        assert_eq!(snapshot.profiles.signed_in().unwrap().gamertag, "tester");
    }
}
