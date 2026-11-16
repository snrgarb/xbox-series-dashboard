//! The two files at a packaged game's root (docs/research/stores.md, "Xbox: packaged games and Game Pass"):
//!
//! - `AppxManifest.xml`, the package's: its identity (name and publisher, which make its family name), its
//!   applications (each `Id` makes an app user model ID), names and logos.
//! - `MicrosoftGame.config`, which every GDK game has and no other package does: what tells a game from an app. Its
//!   `ShellVisuals` name the game's own logos and splash screen; `StoreId` is its product in the Store.
//!
//! Elements are matched by their local names, whatever their namespace: the manifest's schema has grown many.
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use roxmltree::{Document, Node};

/// The manifest's name at a package's root, as the Xbox app writes it; Windows' own folders write
/// `AppxManifest.xml`, which on Windows is the same file.
pub const MANIFEST: &str = "appxmanifest.xml";
/// The game's config, at the same root.
pub const CONFIG: &str = "MicrosoftGame.config";

/// What a package's `AppxManifest.xml` says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// `Identity`'s `Name` and `Publisher`.
    pub name: String,
    pub publisher: String,
    /// `Properties`.
    pub display_name: Option<String>,
    pub publisher_display_name: Option<String>,
    pub logo: Option<String>,
    /// `Properties/Framework`: a framework other packages use, never a game.
    pub framework: bool,
    pub applications: Vec<Application>,
}

/// One `Application`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Application {
    pub id: String,
    pub executable: Option<String>,
    /// From its `VisualElements`, `DefaultTile` and `SplashScreen`.
    pub display_name: Option<String>,
    pub square150: Option<String>,
    pub square44: Option<String>,
    pub square310: Option<String>,
    pub wide310: Option<String>,
    pub splash: Option<String>,
}

/// What a `MicrosoftGame.config` says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameConfig {
    pub name: String,
    pub publisher: String,
    /// `ExecutableList`: each `Executable`'s `Name` and application `Id`.
    pub executables: Vec<(String, String)>,
    /// Twelve letters and digits: the product in the Store.
    pub store_id: Option<String>,
    pub title_id: Option<String>,
    /// `ShellVisuals`.
    pub display_name: Option<String>,
    pub publisher_display_name: Option<String>,
    pub square480: Option<String>,
    pub square150: Option<String>,
    pub square44: Option<String>,
    pub store_logo: Option<String>,
    pub splash: Option<String>,
}

fn named(node: &Node, name: &str) -> bool {
    node.is_element() && node.tag_name().name().eq_ignore_ascii_case(name)
}

/// The first element under `node` named `name`.
fn find<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.descendants().find(|n| named(n, name))
}

/// An attribute, trimmed, if it says anything.
fn attr(node: Option<Node>, name: &str) -> Option<String> {
    let node = node?;
    let value = node.attributes().find(|a| a.name().eq_ignore_ascii_case(name))?.value().trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// An element's text, trimmed, if it says anything.
fn text(node: Option<Node>) -> Option<String> {
    let value = node?.text()?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// The file's text, without UTF-8's or UTF-16's mark: the Xbox app writes UTF-8, a tool may write UTF-16.
fn decode(bytes: &[u8]) -> String {
    let utf16 = |bytes: &[u8], unit: fn([u8; 2]) -> u16| {
        String::from_utf16_lossy(&bytes.as_chunks::<2>().0.iter().map(|&c| unit(c)).collect::<Vec<_>>())
    };
    match bytes {
        [0xff, 0xfe, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xfe, 0xff, rest @ ..] => utf16(rest, u16::from_be_bytes),
        [0xef, 0xbb, 0xbf, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest> {
    let source = decode(bytes);
    let document = Document::parse(&source).context("not XML")?;
    let root = document.root_element();
    if !named(&root, "Package") {
        bail!("not a package manifest: its root is <{}>", root.tag_name().name());
    }
    let identity = root.children().find(|n| named(n, "Identity"));
    let (Some(name), Some(publisher)) = (attr(identity, "Name"), attr(identity, "Publisher")) else {
        bail!("no Identity with a Name and a Publisher");
    };
    let properties = root.children().find(|n| named(n, "Properties"));
    let property = |name: &str| text(properties.and_then(|p| p.children().find(|n| named(n, name))));
    let applications = root
        .children()
        .filter(|n| named(n, "Applications"))
        .flat_map(|apps| apps.children().filter(|n| named(n, "Application")))
        .filter_map(|app| {
            let visuals = find(app, "VisualElements");
            let tile = visuals.and_then(|v| find(v, "DefaultTile"));
            Some(Application {
                id: attr(Some(app), "Id")?,
                executable: attr(Some(app), "Executable"),
                display_name: attr(visuals, "DisplayName"),
                square150: attr(visuals, "Square150x150Logo"),
                square44: attr(visuals, "Square44x44Logo"),
                square310: attr(tile, "Square310x310Logo"),
                wide310: attr(tile, "Wide310x150Logo"),
                splash: attr(visuals.and_then(|v| find(v, "SplashScreen")), "Image"),
            })
        })
        .collect();
    Ok(Manifest {
        name,
        publisher,
        display_name: property("DisplayName"),
        publisher_display_name: property("PublisherDisplayName"),
        logo: property("Logo"),
        framework: property("Framework").is_some_and(|f| f.eq_ignore_ascii_case("true")),
        applications,
    })
}

pub fn parse_config(bytes: &[u8]) -> Result<GameConfig> {
    let source = decode(bytes);
    let document = Document::parse(&source).context("not XML")?;
    let root = document.root_element();
    if !named(&root, "Game") {
        bail!("not a game config: its root is <{}>", root.tag_name().name());
    }
    // As DLSS Swapper reads it: versions 0 and 1 are the ones there are.
    let version = attr(Some(root), "configVersion").unwrap_or_else(|| "0".into());
    if !matches!(version.as_str(), "0" | "1") {
        bail!("configVersion {version}, which is none known");
    }
    let identity = root.children().find(|n| named(n, "Identity"));
    let (Some(name), Some(publisher)) = (attr(identity, "Name"), attr(identity, "Publisher")) else {
        bail!("no Identity with a Name and a Publisher");
    };
    let executables = find(root, "ExecutableList")
        .into_iter()
        .flat_map(|list| list.children().filter(|n| named(n, "Executable")))
        .filter_map(|exe| Some((attr(Some(exe), "Name")?, attr(Some(exe), "Id")?)))
        .collect();
    let element = |name: &str| text(root.children().find(|n| named(n, name)));
    let visuals = root.children().find(|n| named(n, "ShellVisuals"));
    Ok(GameConfig {
        name,
        publisher,
        executables,
        store_id: element("StoreId"),
        title_id: element("TitleId"),
        display_name: attr(visuals, "DefaultDisplayName"),
        publisher_display_name: attr(visuals, "PublisherDisplayName"),
        square480: attr(visuals, "Square480x480Logo"),
        square150: attr(visuals, "Square150x150Logo"),
        square44: attr(visuals, "Square44x44Logo"),
        store_logo: attr(visuals, "StoreLogo"),
        splash: attr(visuals, "SplashScreenImage"),
    })
}

/// The package root's manifest.
pub fn read_manifest(root: &Path) -> Result<Manifest> {
    let path = root.join(MANIFEST);
    let bytes = std::fs::read(&path).with_context(|| path.display().to_string())?;
    parse_manifest(&bytes).with_context(|| path.display().to_string())
}

/// The package root's game config; `Ok(None)` when it has none, which is to say the package is no game.
pub fn read_config(root: &Path) -> Result<Option<GameConfig>> {
    let path = root.join(CONFIG);
    match std::fs::read(&path) {
        Ok(bytes) => parse_config(&bytes).map(Some).with_context(|| path.display().to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| path.display().to_string()),
    }
}

impl Manifest {
    /// The application the game starts as: the one the config's first executable names, else the first.
    pub fn game_application(&self, config: &GameConfig) -> Option<&Application> {
        let wanted = config.executables.iter().map(|(_, id)| id);
        wanted
            .filter_map(|id| self.applications.iter().find(|app| app.id.eq_ignore_ascii_case(id)))
            .next()
            .or(self.applications.first())
    }
}

/// A name that is the name and not a reference into the package's resources (`ms-resource:AppName`), which only
/// Windows can look up.
pub fn plain(name: &Option<String>) -> Option<&str> {
    name.as_deref().filter(|name| !name.trim_start().to_ascii_lowercase().starts_with("ms-resource:"))
}

/// A logo the package names, from its root: the file itself where it is there, else the largest of the variants
/// Windows picks among by scale or size (`Logo.scale-200.png`, `Logo.targetsize-256.png`), contrast and
/// light-theme variants aside.
pub fn asset(root: &Path, relative: &str) -> Option<PathBuf> {
    let path = relative.split(['\\', '/']).filter(|part| !part.is_empty()).fold(root.to_path_buf(), |p, s| p.join(s));
    if path.is_file() {
        return Some(path);
    }
    let folder = path.parent()?;
    let stem = path.file_stem()?.to_str()?.to_ascii_lowercase();
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let size = |name: &str| -> Option<u32> {
        let name = name.to_ascii_lowercase();
        let qualifiers = name.strip_prefix(&format!("{stem}."))?.strip_suffix(&format!(".{extension}"))?;
        if qualifiers.contains("contrast") || qualifiers.contains("altform") {
            return None;
        }
        qualifiers.split('_').find_map(|q| {
            let n = q.strip_prefix("scale-").or_else(|| q.strip_prefix("targetsize-"))?;
            n.parse().ok()
        })
    };
    std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .filter_map(|entry| Some((size(entry.file_name().to_str()?)?, entry.path())))
        .max_by_key(|(size, _)| *size)
        .map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As a GDK game's, every name made up.
    const MANIFEST_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10">
  <Identity Name="MadeUpStudio.TidewaterTrials" Publisher="CN=00000000-1111-2222-3333-444444444444" Version="1.2.0.0" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>ms-resource:AppDisplayName</DisplayName>
    <PublisherDisplayName>Made Up Studio</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Applications>
    <Application Id="Editor" Executable="Editor.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="Tidewater Editor" Square150x150Logo="Assets\Editor150.png" Square44x44Logo="Assets\Editor44.png" />
    </Application>
    <Application Id="Game" Executable="GameLaunchHelper.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="Tidewater Trials" Square150x150Logo="Assets\Logo150.png" Square44x44Logo="Assets\Logo44.png">
        <uap:DefaultTile Wide310x150Logo="Assets\Wide310.png" Square310x310Logo="Assets\Logo310.png" />
        <uap:SplashScreen Image="Assets\Splash620.png" />
      </uap:VisualElements>
    </Application>
  </Applications>
</Package>
"#;

    const CONFIG_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Game configVersion="1">
  <Identity Name="MadeUpStudio.TidewaterTrials" Publisher="CN=00000000-1111-2222-3333-444444444444" Version="1.2.0.0" />
  <ExecutableList>
    <Executable Name="TidewaterTrials.exe" Id="Game" TargetDeviceFamily="PC" />
  </ExecutableList>
  <StoreId>9ZZZMADEUP01</StoreId>
  <TitleId>7F00ABCD</TitleId>
  <ShellVisuals DefaultDisplayName="Tidewater Trials"
                PublisherDisplayName="Made Up Studio"
                Square480x480Logo="Assets\Logo480.png"
                Square150x150Logo="Assets\Logo150.png"
                Square44x44Logo="Assets\Logo44.png"
                StoreLogo="Assets\StoreLogo.png"
                SplashScreenImage="Assets\Splash.png" />
</Game>
"#;

    #[test]
    fn a_manifest_gives_the_identity_names_logos_and_applications() {
        let manifest = parse_manifest(MANIFEST_XML.as_bytes()).unwrap();
        assert_eq!(manifest.name, "MadeUpStudio.TidewaterTrials");
        assert_eq!(manifest.publisher, "CN=00000000-1111-2222-3333-444444444444");
        assert_eq!(manifest.display_name.as_deref(), Some("ms-resource:AppDisplayName"));
        assert_eq!(plain(&manifest.display_name), None);
        assert_eq!(manifest.publisher_display_name.as_deref(), Some("Made Up Studio"));
        assert_eq!(manifest.logo.as_deref(), Some(r"Assets\StoreLogo.png"));
        assert!(!manifest.framework);
        let ids: Vec<&str> = manifest.applications.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["Editor", "Game"]);
        let game = &manifest.applications[1];
        assert_eq!(game.executable.as_deref(), Some("GameLaunchHelper.exe"));
        assert_eq!(game.display_name.as_deref(), Some("Tidewater Trials"));
        assert_eq!(game.wide310.as_deref(), Some(r"Assets\Wide310.png"));
        assert_eq!(game.square310.as_deref(), Some(r"Assets\Logo310.png"));
        assert_eq!(game.splash.as_deref(), Some(r"Assets\Splash620.png"));
    }

    #[test]
    fn a_config_gives_the_game_its_store_id_and_shell_visuals() {
        let config = parse_config(CONFIG_XML.as_bytes()).unwrap();
        assert_eq!(config.name, "MadeUpStudio.TidewaterTrials");
        assert_eq!(config.executables, [("TidewaterTrials.exe".to_string(), "Game".to_string())]);
        assert_eq!(config.store_id.as_deref(), Some("9ZZZMADEUP01"));
        assert_eq!(config.title_id.as_deref(), Some("7F00ABCD"));
        assert_eq!(plain(&config.display_name), Some("Tidewater Trials"));
        assert_eq!(config.square480.as_deref(), Some(r"Assets\Logo480.png"));
        assert_eq!(config.splash.as_deref(), Some(r"Assets\Splash.png"));
        // The application it starts as is the one its executable names, not the manifest's first.
        let manifest = parse_manifest(MANIFEST_XML.as_bytes()).unwrap();
        assert_eq!(manifest.game_application(&config).unwrap().id, "Game");
        let none = GameConfig { executables: Vec::new(), ..config };
        assert_eq!(manifest.game_application(&none).unwrap().id, "Editor");
    }

    #[test]
    fn files_that_are_not_these_are_errors() {
        assert!(parse_manifest(b"not xml").is_err());
        assert!(parse_manifest(CONFIG_XML.as_bytes()).is_err());
        assert!(parse_manifest(br#"<Package><Identity Name="A" /></Package>"#).is_err());
        assert!(parse_config(MANIFEST_XML.as_bytes()).is_err());
        assert!(parse_config(CONFIG_XML.replace(r#"configVersion="1""#, r#"configVersion="7""#).as_bytes()).is_err());
        // UTF-16, with its mark, reads as UTF-8 does.
        let utf16: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain(CONFIG_XML.replace("utf-8", "utf-16").encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(parse_config(&utf16).unwrap().store_id.as_deref(), Some("9ZZZMADEUP01"));
    }

    #[test]
    fn a_logo_is_the_file_or_its_largest_scale() {
        let dir = tempfile::tempdir().unwrap();
        let assets = dir.path().join("Assets");
        std::fs::create_dir_all(&assets).unwrap();
        for name in [
            "Logo150.png",
            "Logo44.scale-100.png",
            "Logo44.scale-200.png",
            "Logo44.targetsize-48.png",
            "Logo44.scale-400_contrast-black.png",
            "Logo44.scale-200.jpg",
        ] {
            std::fs::write(assets.join(name), b"").unwrap();
        }
        assert_eq!(asset(dir.path(), r"Assets\Logo150.png"), Some(assets.join("Logo150.png")));
        assert_eq!(asset(dir.path(), "Assets/Logo44.png"), Some(assets.join("Logo44.scale-200.png")));
        assert_eq!(asset(dir.path(), r"Assets\Splash.png"), None);
        assert_eq!(asset(dir.path(), r"Missing\Logo.png"), None);
    }
}
