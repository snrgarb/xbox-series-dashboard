//! `.GamingRoot`, at the root of each drive the Xbox app installs games on: the bytes `RGBX`, a 32-bit count (little
//! endian), then that many folder names, each relative to the drive, in UTF-16LE and ended by a zero
//! (docs/research/stores.md, "Xbox: packaged games and Game Pass"). By default the one name is `XboxGames`.
use anyhow::{bail, Result};

/// The file's name, at a drive's root.
pub const FILE: &str = ".GamingRoot";

const MAGIC: &[u8; 4] = b"RGBX";

/// The folders the file names, as written.
pub fn parse(bytes: &[u8]) -> Result<Vec<String>> {
    let Some(rest) = bytes.strip_prefix(MAGIC) else { bail!("not a .GamingRoot: no RGBX at its start") };
    let Some((count, rest)) = rest.split_first_chunk::<4>() else { bail!("no count after RGBX") };
    let count = u32::from_le_bytes(*count) as usize;
    let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).collect();
    // Each name ends with a zero; what follows the last zero is no name.
    let mut names: Vec<&[u16]> = units.split(|&unit| unit == 0).collect();
    names.pop();
    if names.len() < count {
        bail!("says it names {count} folders, and names {}", names.len());
    }
    Ok(names[..count]
        .iter()
        .map(|name| String::from_utf16_lossy(name))
        .filter(|name| !name.trim().is_empty())
        .collect())
}

/// The file naming `folders`, as the Xbox app writes it.
pub fn write(folders: &[&str]) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend((folders.len() as u32).to_le_bytes());
    for folder in folders {
        bytes.extend(folder.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_its_folders() {
        assert_eq!(parse(&write(&["XboxGames"])).unwrap(), ["XboxGames"]);
        assert_eq!(parse(&write(&["XboxGames", r"Games\Xbox Spiele"])).unwrap(), ["XboxGames", r"Games\Xbox Spiele"]);
        assert_eq!(parse(&write(&[])).unwrap(), Vec::<String>::new());
        // As the Xbox app's: a name, its zero, nothing after.
        let bytes = [b"RGBX".as_slice(), &[1, 0, 0, 0], &[b'X', 0, b'G', 0, 0, 0]].concat();
        assert_eq!(parse(&bytes).unwrap(), ["XG"]);
    }

    #[test]
    fn a_file_that_is_not_one_is_an_error() {
        assert!(parse(b"").is_err());
        assert!(parse(b"MZ\x90\x00").is_err());
        assert!(parse(b"RGBX\x01").is_err());
        // It says three and has one.
        let bytes = [b"RGBX".as_slice(), &[3, 0, 0, 0], &[b'X', 0, 0, 0]].concat();
        assert!(parse(&bytes).is_err());
    }
}
