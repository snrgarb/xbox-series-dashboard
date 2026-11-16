//! Where Discord's application and tokens are kept: `discord-secrets.dat` in the data folder, a JSON object of named
//! strings, the whole file encrypted for this Windows user with DPAPI (`CryptProtectData`), as the Bundle keeps its
//! own (`DpapiSecretStore.cs`). Only the same Windows user can read it back, usually only on the same PC. Anywhere
//! but Windows there is no DPAPI: the file is written as it is, by a stand-in that says so ([`Unprotected`]).
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::client::Secrets;

/// The file's name in the data folder.
pub const FILE: &str = "discord-secrets.dat";

/// What makes the file unreadable to anyone but this user.
pub trait Protect: Send {
    fn protect(&self, plain: &[u8]) -> std::io::Result<Vec<u8>>;
    fn unprotect(&self, protected: &[u8]) -> std::io::Result<Vec<u8>>;
    /// What the log says of it.
    fn name(&self) -> &'static str;
}

/// The stand-in where there is no DPAPI: the bytes as they are.
pub struct Unprotected;

impl Protect for Unprotected {
    fn protect(&self, plain: &[u8]) -> std::io::Result<Vec<u8>> {
        Ok(plain.to_vec())
    }

    fn unprotect(&self, protected: &[u8]) -> std::io::Result<Vec<u8>> {
        Ok(protected.to_vec())
    }

    fn name(&self) -> &'static str {
        "not encrypted: there is no DPAPI here"
    }
}

/// DPAPI for the current user.
#[cfg(windows)]
pub use crate::win::Dpapi;

/// This machine's protection: DPAPI on Windows, the stand-in anywhere else.
pub fn system() -> Box<dyn Protect> {
    #[cfg(windows)]
    return Box::new(crate::win::Dpapi);
    #[cfg(not(windows))]
    Box::new(Unprotected)
}

/// The secrets in a file, read once and written whole at each change.
pub struct FileSecrets {
    path: PathBuf,
    values: BTreeMap<String, String>,
    protect: Box<dyn Protect>,
}

impl FileSecrets {
    /// The secrets kept at `path`, or none where there is no file or it can't be read (another user's, or broken),
    /// which the log says, never what is in it.
    pub fn open(path: &Path, protect: Box<dyn Protect>) -> Self {
        let values = match std::fs::read(path) {
            Ok(bytes) => match protect.unprotect(&bytes).map(|plain| serde_json::from_slice(&plain)) {
                Ok(Ok(values)) => values,
                Ok(Err(_)) | Err(_) => {
                    eprintln!("discord: {} can't be read: starting with no application", path.display());
                    BTreeMap::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => {
                eprintln!("discord: {}: {e}", path.display());
                BTreeMap::new()
            }
        };
        eprintln!("discord: secrets at {} ({})", path.display(), protect.name());
        Self { path: path.to_path_buf(), values, protect }
    }

    fn save(&self) {
        let plain = serde_json::to_vec(&self.values).expect("strings");
        let written = self.protect.protect(&plain).and_then(|bytes| {
            if let Some(dir) = self.path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            // Written aside and moved over, so a crash mid-write leaves the old file whole.
            let aside = self.path.with_extension("dat.new");
            std::fs::write(&aside, bytes)?;
            std::fs::rename(&aside, &self.path)
        });
        if let Err(e) = written {
            eprintln!("discord: secrets not saved to {}: {e}", self.path.display());
        }
    }
}

impl Secrets for FileSecrets {
    fn read(&self, name: &str) -> Option<String> {
        self.values.get(name).cloned()
    }

    fn write(&mut self, name: &str, value: &str) {
        if self.values.get(name).map(String::as_str) != Some(value) {
            self.values.insert(name.to_string(), value.to_string());
            self.save();
        }
    }

    fn delete(&mut self, name: &str) {
        if self.values.remove(name).is_some() {
            self.save();
        }
    }
}

/// Secrets held in memory alone: a test's, or a Dashboard with no data folder.
#[derive(Debug, Default, Clone)]
pub struct MemorySecrets {
    pub values: BTreeMap<String, String>,
}

impl Secrets for MemorySecrets {
    fn read(&self, name: &str) -> Option<String> {
        self.values.get(name).cloned()
    }

    fn write(&mut self, name: &str, value: &str) {
        self.values.insert(name.to_string(), value.to_string());
    }

    fn delete(&mut self, name: &str) {
        self.values.remove(name);
    }
}

/// Either: the file in the data folder, or memory where there is none.
pub enum AnySecrets {
    File(FileSecrets),
    Memory(MemorySecrets),
}

impl Secrets for AnySecrets {
    fn read(&self, name: &str) -> Option<String> {
        match self {
            AnySecrets::File(file) => file.read(name),
            AnySecrets::Memory(memory) => memory.read(name),
        }
    }

    fn write(&mut self, name: &str, value: &str) {
        match self {
            AnySecrets::File(file) => file.write(name, value),
            AnySecrets::Memory(memory) => memory.write(name, value),
        }
    }

    fn delete(&mut self, name: &str) {
        match self {
            AnySecrets::File(file) => file.delete(name),
            AnySecrets::Memory(memory) => memory.delete(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in that does change the bytes, so a test sees the file is not the plain text.
    struct Flipped;

    impl Protect for Flipped {
        fn protect(&self, plain: &[u8]) -> std::io::Result<Vec<u8>> {
            Ok(plain.iter().map(|b| !b).collect())
        }

        fn unprotect(&self, protected: &[u8]) -> std::io::Result<Vec<u8>> {
            Ok(protected.iter().map(|b| !b).collect())
        }

        fn name(&self) -> &'static str {
            "flipped"
        }
    }

    #[test]
    fn the_secrets_come_back_next_time_and_are_never_in_the_file_as_they_are() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let mut secrets = FileSecrets::open(&path, Box::new(Flipped));
        secrets.write("Discord.ClientSecret", "s3cr3t");
        secrets.write("Discord.ClientId", "1290");
        secrets.delete("Discord.ClientId");
        let bytes = std::fs::read(&path).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("s3cr3t"));

        let again = FileSecrets::open(&path, Box::new(Flipped));
        assert_eq!(again.read("Discord.ClientSecret").as_deref(), Some("s3cr3t"));
        assert_eq!(again.read("Discord.ClientId"), None);
    }

    #[test]
    fn a_file_that_cant_be_read_is_no_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        std::fs::write(&path, b"not ours").unwrap();
        let secrets = FileSecrets::open(&path, Box::new(Flipped));
        assert_eq!(secrets.read("Discord.ClientId"), None);
    }
}
