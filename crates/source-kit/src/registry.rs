//! The registry as a store's source reads it: a key's values, and a key's subkeys with theirs. Behind one type so
//! a source reads the same way from Windows' registry and from a file standing in for it: the tests on Linux, and
//! the fake stores on the test VM (`--ea <dir>` and the others: the fake's `registry.json`).
//!
//! Keys are named in full from their hive, `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher` (or `HKCU\...`), and read
//! as named: a 32-bit program's keys are asked for under `WOW6432Node` by that name, never by redirection. Only
//! values that are text (`REG_SZ`, `REG_EXPAND_SZ`, unexpanded) or a `REG_DWORD` (as its decimal) are read.
//!
//! The file is a JSON object of keys, each an object of its values as text:
//!
//! ```json
//! { "HKLM\\SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher\\Installs\\90210": { "InstallDir": "D:/Ubisoft/Harborlight/" } }
//! ```
use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};

/// A key's values by name.
pub type Values = BTreeMap<String, String>;

/// Where the Uninstall keys are, in both hives and both views: the list of what is installed that stores (and
/// Windows' own Settings) read.
pub const UNINSTALL: [&str; 4] = [
    r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKCU\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
];

/// The registry a source reads.
#[derive(Debug, Clone)]
pub enum Registry {
    /// This machine's (on Windows; elsewhere, one with nothing in it).
    Windows,
    /// Keys from a file, by their full names.
    File(BTreeMap<String, Values>),
}

impl Registry {
    /// The keys in the JSON file at `path`.
    pub fn load(path: &Path) -> Result<Registry> {
        let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
        let keys: BTreeMap<String, Values> =
            serde_json::from_str(&text).with_context(|| format!("{}: not a registry file", path.display()))?;
        Ok(Registry::File(keys))
    }

    /// Writes `keys` as a file `load` reads: a fixture's registry.
    pub fn write(path: &Path, keys: &BTreeMap<String, Values>) -> std::io::Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(keys).map_err(std::io::Error::other)?)
    }

    /// The key's values; none when there is no such key.
    pub fn values(&self, key: &str) -> Option<Values> {
        match self {
            Registry::Windows => windows::values(key),
            Registry::File(keys) => {
                let wanted = normal(key);
                keys.iter().find(|(name, _)| normal(name) == wanted).map(|(_, values)| values.clone())
            }
        }
    }

    /// One value of a key, when it is there and not empty.
    pub fn value(&self, key: &str, name: &str) -> Option<String> {
        let values = self.values(key)?;
        let (_, value) = values.iter().find(|(n, _)| n.eq_ignore_ascii_case(name))?;
        Some(value.clone()).filter(|value| !value.trim().is_empty())
    }

    /// The key's subkeys, each by its own name with its values, by name; none when there is no such key.
    pub fn subkeys(&self, key: &str) -> Vec<(String, Values)> {
        match self {
            Registry::Windows => windows::subkeys(key),
            Registry::File(keys) => {
                let parent = format!("{}\\", normal(key));
                keys.iter()
                    .filter_map(|(name, values)| {
                        normal(name).strip_prefix(&parent).filter(|rest| !rest.contains('\\'))?;
                        let child = name.trim().trim_end_matches('\\').rsplit('\\').next()?;
                        Some((child.to_string(), values.clone()))
                    })
                    .collect()
            }
        }
    }
}

/// A key's name to compare: its hive by its short name, lower case, single back slashes, none at the end.
fn normal(key: &str) -> String {
    let key = key.trim().trim_end_matches('\\').to_lowercase();
    let key = key.replace("hkey_local_machine\\", "hklm\\").replace("hkey_current_user\\", "hkcu\\");
    key.split('\\').filter(|part| !part.is_empty()).collect::<Vec<_>>().join("\\")
}

/// A value of an Uninstall key: the same as `Registry::value`, on values already read.
pub fn get<'a>(values: &'a Values, name: &str) -> Option<&'a str> {
    let (_, value) = values.iter().find(|(n, _)| n.eq_ignore_ascii_case(name))?;
    Some(value.as_str()).filter(|value| !value.trim().is_empty())
}

#[cfg(windows)]
mod windows {
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
        KEY_READ, KEY_WOW64_64KEY, REG_DWORD, REG_EXPAND_SZ, REG_SZ,
    };

    use super::Values;

    /// An open key, closed when dropped.
    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the key was opened by RegOpenKeyExW and is closed once.
            unsafe { RegCloseKey(self.0) };
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn open_in(parent: HKEY, path: &str) -> Option<Key> {
        let path = wide(path);
        let mut key: HKEY = null_mut();
        // SAFETY: the path is a NUL-terminated wide string and `key` is written only on success.
        let status = unsafe { RegOpenKeyExW(parent, path.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut key) };
        (status == 0).then_some(Key(key))
    }

    fn open(key: &str) -> Option<Key> {
        let (hive, path) = key.split_once('\\')?;
        let hive = match hive.to_ascii_uppercase().as_str() {
            "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
            "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
            _ => return None,
        };
        open_in(hive, path)
    }

    fn read_values(key: &Key) -> Values {
        let mut values = Values::new();
        for index in 0.. {
            let mut name = [0u16; 512];
            let mut name_len = name.len() as u32;
            let mut data = vec![0u8; 8192];
            let mut data_len = data.len() as u32;
            let mut kind = 0u32;
            // SAFETY: every buffer is as long as the length given with it.
            let status = unsafe {
                RegEnumValueW(
                    key.0,
                    index,
                    name.as_mut_ptr(),
                    &mut name_len,
                    null(),
                    &mut kind,
                    data.as_mut_ptr(),
                    &mut data_len,
                )
            };
            match status {
                0 => {}
                // ERROR_MORE_DATA: a value longer than any a store keeps; passed over.
                234 => continue,
                _ => break,
            }
            let name = String::from_utf16_lossy(&name[..name_len as usize]);
            let data = &data[..(data_len as usize).min(data.len())];
            let text = match kind {
                REG_SZ | REG_EXPAND_SZ => {
                    let units: Vec<u16> = data.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).collect();
                    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
                    String::from_utf16_lossy(&units[..end])
                }
                REG_DWORD if data.len() >= 4 => u32::from_le_bytes([data[0], data[1], data[2], data[3]]).to_string(),
                _ => continue,
            };
            values.insert(name, text);
        }
        values
    }

    pub fn values(key: &str) -> Option<Values> {
        open(key).map(|key| read_values(&key))
    }

    pub fn subkeys(key: &str) -> Vec<(String, Values)> {
        let Some(parent) = open(key) else { return Vec::new() };
        let mut out = Vec::new();
        for index in 0.. {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            // SAFETY: the name buffer is as long as the length given with it; nothing else is asked for.
            let status = unsafe {
                RegEnumKeyExW(parent.0, index, name.as_mut_ptr(), &mut len, null(), null_mut(), null_mut(), null_mut())
            };
            if status != 0 {
                break;
            }
            let name = String::from_utf16_lossy(&name[..len as usize]);
            if let Some(child) = open_in(parent.0, &name) {
                out.push((name, read_values(&child)));
            }
        }
        out
    }
}

#[cfg(not(windows))]
mod windows {
    use super::Values;

    pub fn values(_key: &str) -> Option<Values> {
        None
    }

    pub fn subkeys(_key: &str) -> Vec<(String, Values)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(pairs: &[(&str, &str)]) -> Values {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn a_file_is_read_as_the_registry_its_keys_are_named_for() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        let installs = r"HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs";
        let keys = BTreeMap::from([
            (r"HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher".to_string(), values(&[("InstallDir", "C:/Ubisoft/")])),
            (format!(r"{installs}\90210"), values(&[("InstallDir", "D:/Ubisoft/Harborlight/")])),
            (format!(r"{installs}\90311"), values(&[("InstallDir", "D:/Ubisoft/Oxbow Frontier/"), ("Empty", " ")])),
            (format!(r"{installs}\90311\Deeper"), values(&[])),
        ]);
        Registry::write(&path, &keys).unwrap();
        let registry = Registry::load(&path).unwrap();

        // The hive by either name, any case, a slash at the end or not.
        let launcher = r"HKEY_LOCAL_MACHINE\Software\WOW6432Node\ubisoft\launcher\";
        assert_eq!(registry.value(launcher, "installdir").as_deref(), Some("C:/Ubisoft/"));
        let names: Vec<String> = registry.subkeys(installs).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["90210", "90311"]);
        assert_eq!(registry.value(&format!(r"{installs}\90311"), "Empty"), None);
        assert_eq!(registry.values(r"HKCU\SOFTWARE\Nothing"), None);
        assert!(registry.subkeys(r"HKCU\SOFTWARE").is_empty());

        assert!(Registry::load(&dir.path().join("none.json")).is_err());
        std::fs::write(&path, "[1, 2]").unwrap();
        assert!(Registry::load(&path).is_err());
        // Off Windows the machine's own has nothing in it.
        if cfg!(not(windows)) {
            assert_eq!(Registry::Windows.values(launcher), None);
            assert!(Registry::Windows.subkeys(UNINSTALL[0]).is_empty());
        }
    }
}
