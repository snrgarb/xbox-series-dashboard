//! The Microsoft Store apps installed on this PC, for Add an app's list (docs/spec/apps.md, 4.3).
//!
//! Windows' own list of what its Start menu starts is `Get-StartApps`; a packaged app is one whose AppID is an
//! Application User Model ID (`<package family>!<app id>`), which the shell's Apps folder starts. Its tile art is in
//! its package: the manifest's `Square150x150Logo`, a path in the package's folder with no scale in it, of which
//! the files on disk are the scaled ones (`Square150x150Logo.scale-200.png`). Windows PowerShell lists them, its
//! output read as JSON ([`from_json`]), and the art is found on disk here ([`logo`]); both are tested on made-up
//! data. **What is not tested** is the PowerShell itself on a real machine: if it lists nothing there, this is the
//! one place to change.
use std::path::{Path, PathBuf};

use model::StoreApp;
use serde_json::Value;

/// Lists the Store apps installed, by name. None anywhere but Windows, and where they could not be listed, which is
/// said in the log.
pub fn list() -> Option<Vec<StoreApp>> {
    match listed() {
        Ok(json) => match from_json(&json) {
            Ok(apps) => Some(apps),
            Err(e) => {
                eprintln!("apps: the Store apps' list could not be read: {e:#}");
                None
            }
        },
        Err(e) => {
            if cfg!(windows) {
                eprintln!("apps: the Store apps could not be listed: {e:#}");
            }
            None
        }
    }
}

/// Windows PowerShell's script: every app of the Start menu that is a package's, with its package's folder and the
/// manifest's tile art (or the package's logo where the app names none), as one JSON array.
#[cfg(windows)]
const SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$packages = @{}
foreach ($p in Get-AppxPackage) { $packages[$p.PackageFamilyName] = $p }
$apps = foreach ($a in Get-StartApps) {
    if ($a.AppID -notlike '*!*') { continue }
    $family, $id = $a.AppID.Split('!', 2)
    $dir = $null; $logo = $null
    $p = $packages[$family]
    if ($p) {
        $dir = $p.InstallLocation
        $manifest = Get-AppxPackageManifest -Package $p.PackageFullName
        $entry = @($manifest.Package.Applications.Application) | Where-Object { $_.Id -eq $id } | Select-Object -First 1
        if ($entry) { $logo = $entry.VisualElements.Square150x150Logo }
        if (-not $logo) { $logo = $manifest.Package.Properties.Logo }
    }
    [pscustomobject]@{ Name = $a.Name; AppID = $a.AppID; Dir = $dir; Logo = $logo }
}
ConvertTo-Json -InputObject @($apps) -Compress
"#;

#[cfg(windows)]
fn listed() -> anyhow::Result<String> {
    use std::os::windows::process::CommandExt;

    use anyhow::{bail, Context};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // The script as PowerShell takes one whole: UTF-16 in Base64, so nothing in it is quoted twice.
    let utf16: Vec<u8> = SCRIPT.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &base64(&utf16)])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("running Windows PowerShell")?;
    if !output.status.success() {
        bail!("Windows PowerShell said {}: {}", output.status, String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(not(windows))]
fn listed() -> anyhow::Result<String> {
    anyhow::bail!("only Windows has Store apps")
}

/// Base64, as `-EncodedCommand` takes it.
#[cfg_attr(not(windows), allow(dead_code))]
fn base64(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            text.push(if i <= chunk.len() { DIGITS[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    text
}

/// The apps in PowerShell's JSON: an array of `{Name, AppID, Dir, Logo}` (or one such object alone, as
/// `ConvertTo-Json` writes a single one), each with its tile art found on disk. Those that are not a package's
/// (no `!` in the AppID) or have no name are left out, as is a second of the same AppID; the rest by name.
pub fn from_json(json: &str) -> anyhow::Result<Vec<StoreApp>> {
    let json = json.trim();
    if json.is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(json)?;
    let items = match value {
        Value::Array(items) => items,
        Value::Null => Vec::new(),
        item => vec![item],
    };
    let text =
        |item: &Value, key: &str| item.get(key).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_string();
    let mut apps: Vec<StoreApp> = Vec::new();
    for item in &items {
        let (name, aumid, dir, art) = (text(item, "Name"), text(item, "AppID"), text(item, "Dir"), text(item, "Logo"));
        if name.is_empty() || !aumid.contains('!') || apps.iter().any(|app| app.aumid == aumid) {
            continue;
        }
        let logo = (!dir.is_empty() && !art.is_empty()).then(|| logo(Path::new(&dir), &art)).flatten();
        apps.push(StoreApp { name, aumid, logo });
    }
    apps.sort_by_cached_key(|app| (app.name.to_lowercase(), app.aumid.clone()));
    Ok(apps)
}

/// The file of a package's art named `relative` (as its manifest names it: `Assets\Square150x150Logo.png`) in the
/// package's folder `dir`: the file itself if it is there, else the largest of its scales (`.scale-400` over
/// `.scale-200`), else of its target sizes (`.targetsize-256`); never a high-contrast one. None if there is none.
pub fn logo(dir: &Path, relative: &str) -> Option<PathBuf> {
    let path = relative
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .fold(dir.to_path_buf(), |path, part| path.join(part));
    if path.is_file() {
        return Some(path);
    }
    let (folder, file) = (path.parent()?, path.file_name()?.to_str()?.to_lowercase());
    let (stem, extension) = file.rsplit_once('.')?;
    let mut best: Option<((u8, u32), PathBuf)> = None;
    for entry in std::fs::read_dir(folder).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let Some(qualifiers) =
            name.strip_prefix(&format!("{stem}.")).and_then(|rest| rest.strip_suffix(&format!(".{extension}")))
        else {
            continue;
        };
        if qualifiers.contains("contrast") {
            continue;
        }
        let number = |key: &str| {
            qualifiers.split(['_', '.']).find_map(|q| q.strip_prefix(key)).and_then(|n| n.parse::<u32>().ok())
        };
        let rank = match (number("scale-"), number("targetsize-")) {
            (Some(scale), _) => (2, scale),
            (None, Some(size)) => (1, size),
            (None, None) => (0, 0),
        };
        if best.as_ref().is_none_or(|(was, _)| rank > *was) {
            best = Some((rank, entry.path()));
        }
    }
    best.map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_is_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        // "dir" as -EncodedCommand has it.
        assert_eq!(base64(&[0x64, 0, 0x69, 0, 0x72, 0]), "ZABpAHIA");
    }

    #[test]
    fn the_largest_scale_of_a_packages_art_is_its_logo() {
        let dir = tempfile::tempdir().unwrap();
        let assets = dir.path().join("Assets");
        std::fs::create_dir(&assets).unwrap();
        for file in [
            "Square150x150Logo.scale-100.png",
            "Square150x150Logo.scale-200.png",
            "Square150x150Logo.scale-200_contrast-black.png",
            "Square150x150Logo.targetsize-256.png",
            "StoreLogo.png",
            "Wide310x150Logo.scale-400.png",
        ] {
            std::fs::write(assets.join(file), b"png").unwrap();
        }
        let found = logo(dir.path(), r"Assets\Square150x150Logo.png").unwrap();
        assert_eq!(found.file_name().unwrap(), "Square150x150Logo.scale-200.png");
        // The file itself, where it is there.
        assert_eq!(logo(dir.path(), "Assets/StoreLogo.png").unwrap(), assets.join("StoreLogo.png"));
        assert_eq!(logo(dir.path(), r"Assets\Missing.png"), None);
        assert_eq!(logo(&dir.path().join("nowhere"), r"Assets\StoreLogo.png"), None);
    }

    #[test]
    fn powershells_list_is_the_packaged_apps_by_name_with_their_art() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Assets")).unwrap();
        std::fs::write(dir.path().join("Assets").join("CalculatorSquare150x150Logo.scale-200.png"), b"png").unwrap();
        let package = dir.path().to_string_lossy().replace('\\', "\\\\");
        let json = format!(
            r#"[
                {{"Name":"Calculator","AppID":"Microsoft.WindowsCalculator_8wekyb3d8bbwe!App","Dir":"{package}","Logo":"Assets\\CalculatorSquare150x150Logo.png"}},
                {{"Name":"Notepad (desktop)","AppID":"C:\\Windows\\notepad.exe","Dir":null,"Logo":null}},
                {{"Name":"alarms & Clock","AppID":"Microsoft.WindowsAlarms_8wekyb3d8bbwe!App","Dir":"C:\\Nowhere","Logo":"Assets\\AlarmClock.png"}},
                {{"Name":"Calculator","AppID":"Microsoft.WindowsCalculator_8wekyb3d8bbwe!App","Dir":null,"Logo":null}},
                {{"Name":"","AppID":"Nameless_1!App"}}
            ]"#
        );
        let apps = from_json(&json).unwrap();
        let listed: Vec<(&str, &str, bool)> =
            apps.iter().map(|app| (app.name.as_str(), app.aumid.as_str(), app.logo.is_some())).collect();
        assert_eq!(
            listed,
            [
                ("alarms & Clock", "Microsoft.WindowsAlarms_8wekyb3d8bbwe!App", false),
                ("Calculator", "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", true),
            ]
        );
        // One alone is an object, not an array; none is nothing at all.
        let one = r#"{"Name":"Photos","AppID":"Microsoft.Windows.Photos_8wekyb3d8bbwe!App"}"#;
        assert_eq!(from_json(one).unwrap().len(), 1);
        assert!(from_json("").unwrap().is_empty() && from_json("null").unwrap().is_empty());
        assert!(from_json("not json").is_err());
        if !cfg!(windows) {
            assert_eq!(list(), None);
        }
    }
}
