//! Reads a Steam install into a `model::Library`.
//!
//! Sources, all under the Steam root (same layout on Windows and in the fixture):
//! - `appcache/appinfo.vdf`: name, app type and art file names per app.
//! - `appcache/packageinfo.vdf`: which apps the account's packages grant (ownership).
//! - `userdata/<id>/config/localconfig.vdf`: persona name, last played, playtime.
//! - `steamapps/appmanifest_*.acf`: what is installed.
//! - `appcache/librarycache/<appid>/`: cover, hero and header art.
pub mod appcache;
pub mod fixture;
pub mod vdf;

use anyhow::{Context, Result};
use model::{Game, Library};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use vdf::Value;

/// Read the Steam install at `steam_root`.
pub fn load(steam_root: &Path) -> Result<Library> {
    let appcache = child(steam_root, "appcache").context("no appcache folder under the Steam root")?;
    let appinfo_path = child(&appcache, "appinfo.vdf").context("no appcache/appinfo.vdf")?;
    let appinfo = appcache::parse_appinfo(&fs::read(&appinfo_path)?)?;

    let local = load_localconfig(steam_root);
    let installed = installed_apps(steam_root);

    // Ownership: apps granted by a cached package. Without packageinfo.vdf,
    // fall back to apps the user has local config for (i.e. has launched).
    let packaged = child(&appcache, "packageinfo.vdf")
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| appcache::parse_package_apps(&b).ok());
    let mut owned: HashSet<u32> = match packaged {
        Some(apps) => apps,
        None => local.apps.keys().copied().collect(),
    };
    owned.extend(installed.keys().copied());

    let art_root = child(&appcache, "librarycache");
    let mut games: Vec<Game> = owned
        .into_iter()
        .filter_map(|appid| {
            let common = appinfo.get(&appid)?.get("common")?;
            if !common.str_at(&["type"])?.eq_ignore_ascii_case("game") {
                return None;
            }
            let name = common.str_at(&["name"])?.to_string();
            let (played, minutes) = local.apps.get(&appid).copied().unwrap_or((0, 0));
            let manifest = installed.get(&appid);
            let manifest_played = manifest.map(|m| m.0);
            let art = art_root.as_deref().and_then(|r| child(r, &appid.to_string())).map(|d| ArtDir::scan(&d));
            let find =
                |asset: &str, legacy: Option<&str>, names: &[&str]| art.as_ref()?.find(common, asset, legacy, names);
            Some(Game {
                appid,
                name,
                last_played: played.max(manifest_played.unwrap_or(0)),
                playtime_minutes: minutes,
                installed: manifest_played.is_some(),
                install_dir: manifest.and_then(|m| m.1.clone()),
                cover: find("library_capsule", None, &["library_600x900.jpg", "library_capsule.jpg"]),
                hero: find("library_hero", None, &["library_hero.jpg"]),
                header: find("library_header", Some("header_image"), &["header.jpg", "library_header.jpg"]),
            })
        })
        .collect();
    games.sort_by(|a, b| {
        b.last_played
            .cmp(&a.last_played)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a.appid.cmp(&b.appid))
    });
    Ok(Library { user: local.persona, games })
}

/// The Steam install folder from the registry (Windows only).
pub fn find_steam_root() -> Option<PathBuf> {
    // The registry value uses forward slashes ("c:/program files (x86)/steam").
    registry::steam_path().map(|s| PathBuf::from(s.replace('/', "\\"))).filter(|p| p.is_dir())
}

#[derive(Default)]
struct LocalConfig {
    persona: Option<String>,
    /// appid -> (last played unix seconds, playtime minutes)
    apps: HashMap<u32, (u64, u32)>,
}

/// The most recently written `userdata/*/config/localconfig.vdf`.
fn load_localconfig(steam_root: &Path) -> LocalConfig {
    let Some(userdata) = child(steam_root, "userdata") else { return LocalConfig::default() };
    let newest = fs::read_dir(userdata)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().parse::<u64>().is_ok_and(|id| id != 0))
        .filter_map(|e| child(&child(&e.path(), "config")?, "localconfig.vdf"))
        .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    let parsed = newest.and_then(|p| fs::read(p).ok()).and_then(|b| vdf::parse_text(&String::from_utf8_lossy(&b)).ok());
    match parsed {
        Some(v) => parse_localconfig(&v),
        None => LocalConfig::default(),
    }
}

fn parse_localconfig(v: &Value) -> LocalConfig {
    let mut out = LocalConfig::default();
    let Some(root) = v.get("UserLocalConfigStore") else { return out };
    out.persona = root.str_at(&["friends", "PersonaName"]).filter(|s| !s.is_empty()).map(str::to_string);
    if let Some(apps) = root.path(&["Software", "Valve", "Steam", "apps"]) {
        for (id, app) in apps.entries() {
            let Ok(id) = id.parse::<u32>() else { continue };
            let played = app.u64_at(&["LastPlayed"]).unwrap_or(0);
            let minutes = app.u64_at(&["Playtime"]).unwrap_or(0).min(u32::MAX as u64) as u32;
            out.apps.insert(id, (played, minutes));
        }
    }
    out
}

/// appid -> the manifest's LastPlayed and the game's folder, for every `appmanifest_*.acf` in the
/// Steam root's `steamapps` and in any other library folder that exists here.
fn installed_apps(steam_root: &Path) -> HashMap<u32, (u64, Option<PathBuf>)> {
    let mut dirs = Vec::new();
    if let Some(main) = child(steam_root, "steamapps") {
        let folders = child(&main, "libraryfolders.vdf")
            .and_then(|p| fs::read(p).ok())
            .and_then(|b| vdf::parse_text(&String::from_utf8_lossy(&b)).ok());
        if let Some(list) = folders.as_ref().and_then(|v| v.get("libraryfolders")) {
            for (_, folder) in list.entries() {
                let Some(path) = folder.str_at(&["path"]) else { continue };
                if let Some(dir) = child(Path::new(path), "steamapps") {
                    if !same_dir(&dir, &main) {
                        dirs.push(dir);
                    }
                }
            }
        }
        dirs.push(main);
    }
    let mut out = HashMap::new();
    for dir in dirs {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let id = name.strip_prefix("appmanifest_").and_then(|s| s.strip_suffix(".acf"));
            let Some(Ok(id)) = id.map(str::parse::<u32>) else { continue };
            let manifest = fs::read(entry.path()).ok().and_then(|b| vdf::parse_text(&String::from_utf8_lossy(&b)).ok());
            let played = manifest.as_ref().and_then(|v| v.u64_at(&["AppState", "LastPlayed"])).unwrap_or(0);
            let folder = manifest
                .as_ref()
                .and_then(|v| v.str_at(&["AppState", "installdir"]))
                .map(|name| dir.join("common").join(name));
            out.insert(id, (played, folder));
        }
    }
    out
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// `dir/name`, matching `name` case-insensitively if the exact name is absent
/// (Steam's own casing varies, and the Linux fixture is case-sensitive).
fn child(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.exists() {
        return Some(exact);
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
}

/// The files of one `librarycache/<appid>` folder: flat (older clients) and one
/// level of hash-named subfolders (newer clients).
struct ArtDir {
    dir: PathBuf,
    /// Paths relative to `dir`, '/'-separated, as named on disk.
    files: Vec<String>,
}

impl ArtDir {
    fn scan(dir: &Path) -> ArtDir {
        let mut files = Vec::new();
        for e in fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() {
                for sub in fs::read_dir(e.path()).into_iter().flatten().flatten() {
                    files.push(format!("{name}/{}", sub.file_name().to_string_lossy()));
                }
            } else {
                files.push(name);
            }
        }
        files.sort();
        ArtDir { dir: dir.to_path_buf(), files }
    }

    /// Prefer the file appinfo names for `asset` (English, else any language),
    /// then fall back to the well-known file names anywhere in the folder.
    fn find(&self, common: &Value, asset: &str, legacy: Option<&str>, names: &[&str]) -> Option<PathBuf> {
        let mut wanted: Vec<&str> = Vec::new();
        let by_lang = [common.path(&["library_assets_full", asset, "image"]), legacy.and_then(|k| common.get(k))];
        for langs in by_lang.into_iter().flatten() {
            wanted.extend(langs.str_at(&["english"]));
            wanted.extend(langs.entries().iter().filter_map(|(_, v)| v.as_str()));
        }
        let hit = wanted
            .iter()
            .find_map(|w| {
                // appinfo says "<hash>/<file>"; on disk that is either that
                // path or, in the older flat layout, just "<file>".
                let base = w.rsplit('/').next().unwrap_or(w);
                self.files.iter().find(|f| f.eq_ignore_ascii_case(w) || f.eq_ignore_ascii_case(base))
            })
            .or_else(|| {
                names.iter().find_map(|n| {
                    self.files.iter().find(|f| f.rsplit('/').next().is_some_and(|b| b.eq_ignore_ascii_case(n)))
                })
            })?;
        Some(hit.split('/').fold(self.dir.clone(), |p, part| p.join(part)))
    }
}

#[cfg(windows)]
mod registry {
    use std::ffi::c_void;

    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            hkey: isize,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            ty: *mut u32,
            data: *mut c_void,
            len: *mut u32,
        ) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// `HKCU\Software\Valve\Steam\SteamPath`
    pub fn steam_path() -> Option<String> {
        let (key, value) = (wide("Software\\Valve\\Steam"), wide("SteamPath"));
        let mut buf = [0u16; 1024];
        let mut len = std::mem::size_of_val(&buf) as u32;
        // SAFETY: all pointers are valid for the call; `len` is the buffer size in bytes.
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if rc != 0 {
            return None;
        }
        let chars = &buf[..(len as usize / 2).min(buf.len())];
        let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
        Some(String::from_utf16_lossy(&chars[..end]))
    }
}

#[cfg(not(windows))]
mod registry {
    pub fn steam_path() -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localconfig_persona_and_apps() {
        let v = vdf::parse_text(
            r#""UserLocalConfigStore" {
                "friends" { "1" { "name" "Someone" } "PersonaName" "tester" }
                "Software" { "Valve" { "Steam" { "apps" {
                    "70" { "LastPlayed" "1700000000" "Playtime" "90" }
                    "80" { "cloud" { "last_sync_state" "synchronized" } }
                } } } }
            }"#,
        )
        .unwrap();
        let lc = parse_localconfig(&v);
        assert_eq!(lc.persona.as_deref(), Some("tester"));
        assert_eq!(lc.apps[&70], (1_700_000_000, 90));
        assert_eq!(lc.apps[&80], (0, 0));
    }

    #[test]
    fn art_prefers_appinfo_name_then_falls_back() {
        let art = ArtDir {
            dir: PathBuf::from("lc/70"),
            files: vec!["abc/library_600x900.jpg".into(), "header.jpg".into(), "Library_Hero.jpg".into()],
        };
        let common = vdf::parse_text(
            r#""library_assets_full" { "library_capsule" { "image" { "english" "abc/library_600x900.jpg" } }
                                       "library_hero" { "image" { "english" "gone/hero_v2.jpg" } } }
               "header_image" { "english" "header.jpg" }"#,
        )
        .unwrap();
        let cover = art.find(&common, "library_capsule", None, &["library_600x900.jpg"]);
        assert_eq!(cover, Some(PathBuf::from("lc/70").join("abc").join("library_600x900.jpg")));
        let hero = art.find(&common, "library_hero", None, &["library_hero.jpg"]);
        assert_eq!(hero, Some(PathBuf::from("lc/70").join("Library_Hero.jpg")));
        let header = art.find(&common, "library_header", Some("header_image"), &["header.jpg"]);
        assert_eq!(header, Some(PathBuf::from("lc/70").join("header.jpg")));
        assert_eq!(art.find(&common, "library_logo", None, &["logo.png"]), None);
    }
}
