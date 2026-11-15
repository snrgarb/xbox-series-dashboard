//! Steam's binary caches: `appcache/appinfo.vdf` and `appcache/packageinfo.vdf`.
use crate::vdf::{parse_binary, Reader, Value};
use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};

/// appid -> the app's KeyValues (root has the `appinfo` sections inline: `common`, ...).
///
/// Layout: u32 magic (0x07564427/28/29), u32 universe, [v29: i64 offset of the
/// key string table], then entries until appid 0:
///   u32 appid, u32 size (of everything after this field), u32 info state,
///   u32 last updated, u64 PICS token, 20-byte SHA-1, u32 change number,
///   [v28+: 20-byte SHA-1 of the binary data], binary KeyValues.
/// v29 string table: u32 count, then that many NUL-terminated strings.
pub fn parse_appinfo(buf: &[u8]) -> Result<HashMap<u32, Value>> {
    let mut r = Reader::new(buf);
    let magic = r.u32()?;
    let version = magic & 0xff;
    if magic >> 8 != 0x075644 || !(0x27..=0x29).contains(&version) {
        bail!("appinfo.vdf: unsupported magic {magic:#010x}");
    }
    let _universe = r.u32()?;
    let strings = if version >= 0x29 {
        let off = usize::try_from(r.u64()?).context("appinfo.vdf: bad string table offset")?;
        let mut t = Reader::new(buf);
        t.pos = off;
        let count = t.u32()?;
        let mut table = Vec::with_capacity(count as usize);
        for _ in 0..count {
            table.push(t.cstr()?);
        }
        Some(table)
    } else {
        None
    };
    let header_len = if version >= 0x28 { 60 } else { 40 };

    let mut apps = HashMap::new();
    loop {
        let appid = r.u32()?;
        if appid == 0 {
            break;
        }
        let size = r.u32()? as usize;
        let entry = r.bytes(size).with_context(|| format!("appinfo.vdf: app {appid} truncated"))?;
        let mut er = Reader::new(entry);
        er.bytes(header_len)?;
        let kv = parse_binary(&mut er, strings.as_deref()).with_context(|| format!("appinfo.vdf: app {appid}"))?;
        // The body is a single root key ("appinfo") wrapping the sections.
        let inner = match kv {
            Value::Map(mut m) if m.len() == 1 && matches!(m[0].1, Value::Map(_)) => m.remove(0).1,
            other => other,
        };
        apps.insert(appid, inner);
    }
    Ok(apps)
}

/// Every appid granted by a cached package, except package 0 (the implicit
/// "everyone has it" package: Spacewar, dedicated servers, ...).
///
/// Layout: u32 magic (0x06565527/28), u32 universe, then entries until
/// package id 0xFFFFFFFF: u32 package id, 20-byte SHA-1, u32 change number,
/// [v28: u64 PICS token], binary KeyValues with inline keys.
pub fn parse_package_apps(buf: &[u8]) -> Result<HashSet<u32>> {
    let mut r = Reader::new(buf);
    let magic = r.u32()?;
    let version = magic & 0xff;
    if magic >> 8 != 0x065655 || !(0x27..=0x28).contains(&version) {
        bail!("packageinfo.vdf: unsupported magic {magic:#010x}");
    }
    let _universe = r.u32()?;
    let mut apps = HashSet::new();
    loop {
        let pkg = r.u32()?;
        if pkg == u32::MAX {
            break;
        }
        r.bytes(if version >= 0x28 { 32 } else { 24 })?;
        let kv = parse_binary(&mut r, None).with_context(|| format!("packageinfo.vdf: package {pkg}"))?;
        if pkg == 0 {
            continue;
        }
        for (_, body) in kv.entries() {
            if let Some(ids) = body.get("appids") {
                apps.extend(ids.entries().iter().filter_map(|(_, v)| v.as_str()?.parse::<u32>().ok()));
            }
        }
    }
    Ok(apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_entry(appid: u32, body: &[u8], header_len: usize) -> Vec<u8> {
        let mut e = appid.to_le_bytes().to_vec();
        e.extend(((header_len + body.len()) as u32).to_le_bytes());
        e.extend(vec![0xAA; header_len]);
        e.extend(body);
        e
    }

    #[test]
    fn appinfo_v29_string_table() {
        // strings: 0=appinfo 1=common 2=name 3=type
        let mut body = Vec::new();
        body.push(0);
        body.extend(0u32.to_le_bytes());
        body.push(0);
        body.extend(1u32.to_le_bytes());
        body.push(1);
        body.extend(2u32.to_le_bytes());
        body.extend(b"Test Game\0");
        body.push(1);
        body.extend(3u32.to_le_bytes());
        body.extend(b"game\0");
        body.extend([8, 8, 8]);

        let entries = [app_entry(70, &body, 60), app_entry(80, &body, 60)].concat();
        let mut file = 0x07564429u32.to_le_bytes().to_vec();
        file.extend(1u32.to_le_bytes());
        file.extend(((16 + entries.len() + 4) as u64).to_le_bytes());
        file.extend(&entries);
        file.extend(0u32.to_le_bytes());
        file.extend(4u32.to_le_bytes());
        file.extend(b"appinfo\0common\0name\0type\0");

        let apps = parse_appinfo(&file).unwrap();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[&70].str_at(&["common", "name"]), Some("Test Game"));
        assert_eq!(apps[&80].str_at(&["common", "type"]), Some("game"));
    }

    #[test]
    fn appinfo_v28_inline_keys_and_bad_magic() {
        let body = b"\x00appinfo\0\x00common\0\x01name\0Old\0\x08\x08\x08";
        let mut file = 0x07564428u32.to_le_bytes().to_vec();
        file.extend(1u32.to_le_bytes());
        file.extend(app_entry(5, body, 60));
        file.extend(0u32.to_le_bytes());
        let apps = parse_appinfo(&file).unwrap();
        assert_eq!(apps[&5].str_at(&["common", "name"]), Some("Old"));

        assert!(parse_appinfo(&[1, 2, 3, 4, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn packageinfo_collects_appids_and_skips_package_zero() {
        fn pkg(id: u32, appids: &[u32]) -> Vec<u8> {
            let mut e = id.to_le_bytes().to_vec();
            e.extend([0u8; 32]);
            e.push(0);
            e.extend(format!("{id}\0").as_bytes());
            e.extend(b"\x02packageid\0");
            e.extend(id.to_le_bytes());
            e.extend(b"\x00appids\0");
            for (i, a) in appids.iter().enumerate() {
                e.push(2);
                e.extend(format!("{i}\0").as_bytes());
                e.extend(a.to_le_bytes());
            }
            e.extend([8, 8, 8]);
            e
        }
        let mut file = 0x06565528u32.to_le_bytes().to_vec();
        file.extend(1u32.to_le_bytes());
        file.extend(pkg(0, &[480]));
        file.extend(pkg(11, &[70, 80]));
        file.extend(pkg(12, &[80, 90]));
        file.extend(u32::MAX.to_le_bytes());
        let apps = parse_package_apps(&file).unwrap();
        assert_eq!(apps, HashSet::from([70, 80, 90]));
    }
}
