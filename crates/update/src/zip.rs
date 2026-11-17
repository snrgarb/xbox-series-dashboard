//! The little of the zip format the update needs: one file read out of the portable zip (`zip -X -D` by
//! `tools/package.sh`: stored or deflated, no zip64, no encryption), and a zip of one stored file written, for the
//! stand-in release.

/// The file in the zip whose name, after its last `/`, is `name`: exactly one must be. Its bytes, their CRC-32
/// checked, at most `limit` of them.
pub fn read_one(zip: &[u8], name: &str, limit: usize) -> Result<Vec<u8>, String> {
    let end = find_end(zip).ok_or("not a zip: no end of central directory")?;
    let count = u16_at(zip, end + 10)? as usize;
    let mut at = u32_at(zip, end + 16)? as usize;
    let mut found = None;
    for _ in 0..count {
        if u32_at(zip, at)? != 0x0201_4b50 {
            return Err("a bad central directory entry".into());
        }
        let method = u16_at(zip, at + 10)?;
        let crc = u32_at(zip, at + 16)?;
        let packed = u32_at(zip, at + 20)? as usize;
        let size = u32_at(zip, at + 24)? as usize;
        let (name_len, extra_len, comment_len) =
            (u16_at(zip, at + 28)? as usize, u16_at(zip, at + 30)? as usize, u16_at(zip, at + 32)? as usize);
        let local = u32_at(zip, at + 42)? as usize;
        let entry = zip.get(at + 46..at + 46 + name_len).ok_or("a name past the end")?;
        let entry = String::from_utf8_lossy(entry);
        if entry.rsplit('/').next() == Some(name) {
            if found.is_some() {
                return Err(format!("more than one {name} in the zip"));
            }
            found = Some((method, crc, packed, size, local));
        }
        at += 46 + name_len + extra_len + comment_len;
    }
    let (method, crc, packed, size, local) = found.ok_or(format!("no {name} in the zip"))?;
    if size > limit {
        return Err(format!("{name} is {size} bytes, more than {limit}"));
    }
    if u32_at(zip, local)? != 0x0403_4b50 {
        return Err("a bad local header".into());
    }
    let start = local + 30 + u16_at(zip, local + 26)? as usize + u16_at(zip, local + 28)? as usize;
    let data = zip.get(start..start + packed).ok_or("data past the end")?;
    let bytes = match method {
        0 => data.to_vec(),
        8 => miniz_oxide::inflate::decompress_to_vec_with_limit(data, limit)
            .map_err(|e| format!("{name} could not be inflated: {e:?}"))?,
        other => return Err(format!("{name} is packed by method {other}, which is not read")),
    };
    if bytes.len() != size || crc32fast::hash(&bytes) != crc {
        return Err(format!("{name} is not what the zip says it is (size or CRC)"));
    }
    Ok(bytes)
}

/// A zip of one file, stored as it is, at `path` in it.
pub fn write_one(path: &str, bytes: &[u8]) -> Vec<u8> {
    let crc = crc32fast::hash(bytes);
    let (size, name) = (bytes.len() as u32, path.as_bytes());
    let mut zip = Vec::with_capacity(bytes.len() + 200);
    // Local header: version 20, no flags, stored, no time.
    zip.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    zip.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    for value in [crc, size, size] {
        zip.extend_from_slice(&value.to_le_bytes());
    }
    zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip.extend_from_slice(name);
    zip.extend_from_slice(bytes);
    let directory = zip.len() as u32;
    zip.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    zip.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    for value in [crc, size, size] {
        zip.extend_from_slice(&value.to_le_bytes());
    }
    zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
    // Extra, comment, disk, internal attributes, external attributes, the local header's offset (0).
    zip.extend_from_slice(&[0; 16]);
    zip.extend_from_slice(name);
    let directory_len = zip.len() as u32 - directory;
    zip.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    zip.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
    zip.extend_from_slice(&directory_len.to_le_bytes());
    zip.extend_from_slice(&directory.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip
}

/// Where the end of central directory record starts: looked for back from the end, past a comment of any length.
fn find_end(zip: &[u8]) -> Option<usize> {
    let last = zip.len().checked_sub(22)?;
    let first = last.saturating_sub(0xFFFF);
    (first..=last).rev().find(|&at| zip[at..at + 4] == 0x0605_4b50u32.to_le_bytes())
}

fn u16_at(zip: &[u8], at: usize) -> Result<u16, String> {
    let bytes = zip.get(at..at + 2).ok_or("the zip ends early")?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(zip: &[u8], at: usize) -> Result<u32, String> {
    let bytes = zip.get(at..at + 4).ok_or("the zip ends early")?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_file_is_read_back() {
        let zip = write_one("XboxSeriesDashboard-1.2.3/dashboard.exe", b"MZ the program");
        assert_eq!(read_one(&zip, "dashboard.exe", 1000).unwrap(), b"MZ the program");
        assert!(read_one(&zip, "README.txt", 1000).unwrap_err().contains("no README.txt"));
        assert!(read_one(&zip, "dashboard.exe", 4).unwrap_err().contains("more than 4"));
    }

    #[test]
    fn a_deflated_file_is_inflated_and_a_damaged_one_refused() {
        // A zip as Info-ZIP writes one: the file deflated.
        let text = b"the same words again and again and again and again and again".repeat(20);
        let packed = miniz_oxide::deflate::compress_to_vec(&text, 6);
        let mut zip = write_one("d/dashboard.exe", &packed);
        // Method 8, and the sizes and CRC of what it inflates to, in both headers.
        let size = (text.len() as u32).to_le_bytes();
        let crc = crc32fast::hash(&text).to_le_bytes();
        let directory = zip.len() - 22 - (46 + 15);
        for (header, offset) in [(0, 0), (directory, 2)] {
            zip[header + 8 + offset] = 8;
            zip[header + 14 + offset..header + 18 + offset].copy_from_slice(&crc);
            zip[header + 22 + offset..header + 26 + offset].copy_from_slice(&size);
        }
        assert_eq!(read_one(&zip, "dashboard.exe", 1 << 20).unwrap(), text);
        // A byte of it changed: the CRC says so.
        let at = 30 + 15 + packed.len() / 2;
        zip[at] ^= 0x55;
        assert!(read_one(&zip, "dashboard.exe", 1 << 20).is_err());
        assert!(read_one(b"not a zip at all, not even close to one", "dashboard.exe", 10).is_err());
    }
}
