//! A package's family name, `<Identity Name>_<publisher id>`, for a package the package manager has not registered:
//! the fake's, on a test machine and in the tests. A registered package's comes from the package manager, as
//! docs/research/stores.md says it should: this is only what Windows' `PackageFamilyNameFromId` computes, so that a
//! fake's address has the same shape as a real one's.
//!
//! The publisher id is the first 8 bytes of the SHA-256 of `Publisher` in UTF-16LE, read as 65 bits (a zero bit
//! after them) in thirteen groups of five, each a letter of Crockford's base 32 in lower case.
use sha2::{Digest, Sha256};

/// Crockford's base 32: no i, l, o or u.
const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// The 13-character id Windows gives a publisher.
pub fn publisher_id(publisher: &str) -> String {
    let utf16: Vec<u8> = publisher.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let hash = Sha256::digest(&utf16);
    let first = u64::from_be_bytes(hash[..8].try_into().expect("eight bytes"));
    // 65 bits: the 64 of the hash, then a zero.
    let bits = (first as u128) << 1;
    (0..13).rev().map(|group| ALPHABET[((bits >> (group * 5)) & 31) as usize] as char).collect()
}

/// `<name>_<publisher id>`.
pub fn family_name(name: &str, publisher: &str) -> String {
    format!("{name}_{}", publisher_id(publisher))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_publishers_id_is_the_one_windows_gives_it() {
        // The publisher of Windows' own packages, whose id every Windows has in its package family names.
        let microsoft = "CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US";
        assert_eq!(publisher_id(microsoft), "8wekyb3d8bbwe");
        assert_eq!(family_name("Made.Up", microsoft), "Made.Up_8wekyb3d8bbwe");
        let made_up = publisher_id("CN=Made Up Studio");
        assert_eq!(made_up.len(), 13);
        assert!(made_up.bytes().all(|b| ALPHABET.contains(&b)), "{made_up}");
        assert_ne!(made_up, publisher_id("CN=Made Up Studio2"));
    }
}
