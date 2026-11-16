//! `<install dir>\__Installer\installerdata.xml`, which EA's installer leaves in every game's folder: the game's
//! content id (what EA's launch link takes), its title, and the program that starts it (docs/research/stores.md,
//! "EA app"). It comes in two shapes, a `DiPManifest` with `gameTitles`, and a `game` with `metadata/localeInfo`,
//! and in UTF-8 or UTF-16.
use std::path::Path;

use anyhow::{Context, Result};

/// Where the file is under a game's folder.
pub const FILE: [&str; 2] = ["__Installer", "installerdata.xml"];

/// What the file says of a game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallerData {
    /// The first `contentID`.
    pub content_id: String,
    /// Its title in US English where it has one, else its first.
    pub title: Option<String>,
    /// The program that starts the game, under its folder, and the arguments it is given.
    pub launcher: Option<Launcher>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Launcher {
    /// The program's path, from the game's folder: what follows the bracketed registry value in `filePath`.
    pub path: String,
    pub parameters: String,
}

/// The file in the game's folder `install_dir`.
pub fn read(install_dir: &Path) -> Result<InstallerData> {
    let path = FILE.iter().fold(install_dir.to_path_buf(), |path, part| path.join(part));
    let bytes = std::fs::read(&path).with_context(|| path.display().to_string())?;
    parse(&bytes).with_context(|| path.display().to_string())
}

/// The file's text, UTF-16 where it starts with UTF-16's mark, else UTF-8.
fn text(bytes: &[u8]) -> String {
    let utf16 = |bytes: &[u8], unit: fn([u8; 2]) -> u16| {
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|&c| unit(c)).collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xff, 0xfe, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xfe, 0xff, rest @ ..] => utf16(rest, u16::from_be_bytes),
        [0xef, 0xbb, 0xbf, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

pub fn parse(bytes: &[u8]) -> Result<InstallerData> {
    let text = text(bytes);
    // roxmltree reads text, not bytes: the declaration's encoding is the one just read, whatever it says.
    let text = text.trim_start_matches('\u{feff}');
    let document = roxmltree::Document::parse(text).context("not XML")?;
    let named = |node: &roxmltree::Node, name: &str| node.tag_name().name().eq_ignore_ascii_case(name);
    let root = document.root_element();
    let content_id = root
        .descendants()
        .find(|node| named(node, "contentID"))
        .and_then(|node| node.text())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .context("no contentID")?
        .to_string();

    // DiPManifest: gameTitles/gameTitle; the other shape: metadata/localeInfo/title.
    let titles: Vec<(Option<&str>, &str)> = root
        .descendants()
        .filter_map(|node| {
            if named(&node, "gameTitle") {
                Some((node.attribute("locale"), node.text()?))
            } else if named(&node, "title") && node.parent_element().is_some_and(|p| named(&p, "localeInfo")) {
                Some((node.parent_element()?.attribute("locale"), node.text()?))
            } else {
                None
            }
        })
        .map(|(locale, title)| (locale, title.trim()))
        .filter(|(_, title)| !title.is_empty())
        .collect();
    let us = titles.iter().find(|(locale, _)| locale.is_some_and(|l| l.eq_ignore_ascii_case("en_US")));
    let title = us.or(titles.first()).map(|(_, title)| title.to_string());

    // A launcher that is not a trial's; of those, the one for 64-bit Windows where there is one.
    let field = |node: &roxmltree::Node, name: &str| {
        node.children().find(|child| named(child, name)).and_then(|child| child.text()).unwrap_or("").trim().to_string()
    };
    let yes = |value: String| value == "1" || value.eq_ignore_ascii_case("true");
    let launchers: Vec<(Launcher, bool)> = root
        .descendants()
        .filter(|node| named(node, "launcher") && node.parent_element().is_some_and(|p| named(&p, "runtime")))
        .filter(|node| !yes(field(node, "trial")))
        .filter_map(|node| {
            let path = relative(&field(&node, "filePath"))?;
            Some((Launcher { path, parameters: field(&node, "parameters") }, yes(field(&node, "requires64BitOS"))))
        })
        .collect();
    let launcher = launchers.iter().find(|(_, wide)| *wide).or(launchers.first()).map(|(l, _)| l.clone());
    Ok(InstallerData { content_id, title, launcher })
}

/// A `filePath`'s path from the game's folder: `[<registry key>\<value>]Game.exe` names the folder by the registry
/// value that holds it, which is the game's folder, so what follows the bracket is the path in it.
fn relative(file_path: &str) -> Option<String> {
    let path = match file_path.strip_prefix('[') {
        Some(bracketed) => bracketed.split_once(']')?.1,
        None => file_path,
    };
    let path = path.trim().trim_start_matches(['\\', '/']);
    (!path.is_empty()).then(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIP: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<DiPManifest version="4.0">
  <contentIDs>
    <contentID>1099001</contentID>
    <contentID>1099001.pc</contentID>
  </contentIDs>
  <gameTitles>
    <gameTitle locale="fr_FR">Rallye de Cinderpeak</gameTitle>
    <gameTitle locale="en_US">Cinderpeak Rally</gameTitle>
  </gameTitles>
  <runtime>
    <launcher>
      <filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\Made Up Studio\Cinderpeak Rally\Install Dir]Trial.exe</filePath>
      <parameters></parameters>
      <trial>1</trial>
    </launcher>
    <launcher>
      <filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\Made Up Studio\Cinderpeak Rally\Install Dir]Bin\Rally32.exe</filePath>
      <parameters>-dx11</parameters>
      <requires64BitOS>0</requires64BitOS>
      <trial>0</trial>
    </launcher>
    <launcher>
      <filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\Made Up Studio\Cinderpeak Rally\Install Dir]CinderpeakRally.exe</filePath>
      <parameters>-windowed</parameters>
      <executeElevated>0</executeElevated>
      <requires64BitOS>1</requires64BitOS>
      <trial>0</trial>
    </launcher>
  </runtime>
</DiPManifest>"#;

    #[test]
    fn a_dip_manifest_gives_the_content_id_the_us_title_and_the_64_bit_launcher() {
        let data = parse(DIP.as_bytes()).unwrap();
        assert_eq!(data.content_id, "1099001");
        assert_eq!(data.title.as_deref(), Some("Cinderpeak Rally"));
        let launcher = Launcher { path: "CinderpeakRally.exe".into(), parameters: "-windowed".into() };
        assert_eq!(data.launcher, Some(launcher));
    }

    #[test]
    fn the_other_shape_and_utf_16_read_the_same() {
        let game = r#"<?xml version="1.0" encoding="utf-16"?>
<game manifestVersion="3.0">
  <metadata>
    <localeInfo locale="de_DE"><title>Vantage-Riff</title></localeInfo>
    <localeInfo locale="en_US"><title>Vantage Reef</title></localeInfo>
  </metadata>
  <contentIDs><contentID>1099002</contentID></contentIDs>
  <runtime>
    <launcher><filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\Made Up\Vantage Reef\Install Dir]game.exe</filePath></launcher>
  </runtime>
</game>"#;
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend(game.encode_utf16().flat_map(u16::to_le_bytes));
        let data = parse(&bytes).unwrap();
        assert_eq!(data.content_id, "1099002");
        assert_eq!(data.title.as_deref(), Some("Vantage Reef"));
        assert_eq!(data.launcher, Some(Launcher { path: "game.exe".into(), parameters: String::new() }));
    }

    #[test]
    fn a_file_with_no_content_id_or_not_xml_is_an_error_and_a_launcher_may_be_missing() {
        assert!(parse(b"<DiPManifest><gameTitles/></DiPManifest>").is_err());
        assert!(parse(b"not xml at all").is_err());
        let bare = parse(b"<DiPManifest><contentIDs><contentID> 77 </contentID></contentIDs></DiPManifest>").unwrap();
        assert_eq!(bare, InstallerData { content_id: "77".into(), title: None, launcher: None });
        assert_eq!(relative("[HKEY_LOCAL_MACHINE\\SOFTWARE\\X\\Install Dir]"), None);
        assert_eq!(relative("Bin/Game.exe").as_deref(), Some("Bin/Game.exe"));
    }
}
