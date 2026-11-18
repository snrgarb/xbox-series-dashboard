//! Writes a fake drive of Xbox games: what `Xbox` reads, with made-up games, for the tests here and for the
//! end-to-end tests' stand-in (`standin make-xbox <dir>`), read with `--xbox <dir>`.
//!
//! At `<dir>`, as at a drive's root: `.GamingRoot` naming `XboxGames` (and a folder that is not there), and under
//! `XboxGames\<game>\Content` each package as the Xbox app installs one, its `appxmanifest.xml`, its
//! `MicrosoftGame.config` (an app's has none), its logos under `Assets` (plain-colour pictures) and, given a program,
//! that program as `game.exe`, the application's executable, titled with the game's name. One game is under
//! `Program Files\ModifiableWindowsApps` instead. Every name, id and publisher is made up.
//!
//! Tidewater Trials' package is whole, as Windows registers a folder of loose files (`Add-AppxPackage -Register`):
//! the end-to-end test registers it on the VM, so that it is a real package that starts by its app user model ID.
use std::io;
use std::path::{Path, PathBuf};

use image::{ImageFormat, Rgb, RgbImage};

use crate::gaming_root;

/// Every fake package's publisher.
pub const PUBLISHER: &str = "CN=Made Up Studio";

/// A fake drive to write.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub games: Vec<FixtureGame>,
    /// A program copied into every game's package as `game.exe`. Without one the packages have none.
    pub game_exe: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct FixtureGame {
    pub name: String,
    /// The package's identity name.
    pub identity: String,
    pub kind: Kind,
}

/// What the package is, and what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A game with every logo, a splash screen, a wide tile and a product in the Store.
    Whole,
    /// A game whose logos are there only at their scales, whose manifest's name is an `ms-resource:` one, with no
    /// splash screen or wide tile.
    Scaled,
    /// A game under `Program Files\ModifiableWindowsApps`, with no product in the Store.
    Modifiable,
    /// An app: a package with no `MicrosoftGame.config`.
    App,
    /// A game whose manifest is not XML.
    Broken,
}

impl Fixture {
    /// Three games, Tidewater Trials (whole), Lumen Hollow (its logos at scales) and Pebble Courier (under
    /// ModifiableWindowsApps); an app, Made Up Notes, and a game whose manifest is broken, Quillmoor, which `Xbox`
    /// passes over.
    pub fn standard() -> Fixture {
        let game =
            |name: &str, identity: &str, kind| FixtureGame { name: name.into(), identity: identity.into(), kind };
        Fixture {
            games: vec![
                game("Tidewater Trials", "MadeUpStudio.TidewaterTrials", Kind::Whole),
                game("Lumen Hollow", "MadeUpStudio.LumenHollow", Kind::Scaled),
                game("Pebble Courier", "MadeUpStudio.PebbleCourier", Kind::Modifiable),
                game("Made Up Notes", "MadeUpStudio.Notes", Kind::App),
                game("Quillmoor", "MadeUpStudio.Quillmoor", Kind::Broken),
            ],
            game_exe: None,
        }
    }

    /// The names of the games `Xbox` reads from it, as it lists them.
    pub fn expected(&self) -> Vec<&str> {
        let listed = [Kind::Whole, Kind::Scaled, Kind::Modifiable];
        let mut names: Vec<&str> =
            self.games.iter().filter(|g| listed.contains(&g.kind)).map(|g| g.name.as_str()).collect();
        names.sort_by_key(|name| name.to_lowercase());
        names
    }
}

impl FixtureGame {
    /// Its package's root under the fake drive `root`.
    pub fn package_root(&self, root: &Path) -> PathBuf {
        match self.kind {
            Kind::Modifiable => root.join("Program Files").join("ModifiableWindowsApps").join(&self.name),
            _ => root.join("XboxGames").join(&self.name).join("Content"),
        }
    }
}

/// Writes `spec` at `root`.
pub fn write(root: &Path, spec: &Fixture) -> io::Result<()> {
    let root = std::path::absolute(root)?;
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join(gaming_root::FILE), gaming_root::write(&["XboxGames", "Not There"]))?;
    for game in &spec.games {
        let package = game.package_root(&root);
        source_kit::write_game_folder(&package, &game.name, spec.game_exe.as_deref())?;
        let assets = package.join("Assets");
        std::fs::create_dir_all(&assets)?;
        let colour = colour(&game.identity);
        let logos: &[(&str, u32, u32)] = match game.kind {
            Kind::Scaled => &[
                ("Logo150.scale-100.png", 150, 150),
                ("Logo150.scale-200.png", 300, 300),
                ("Logo44.scale-100.png", 44, 44),
                ("StoreLogo.scale-100.png", 50, 50),
                ("Splash620.scale-100.png", 620, 300),
            ],
            _ => &[
                ("Logo480.png", 480, 480),
                ("Logo150.png", 150, 150),
                ("Logo44.png", 44, 44),
                ("StoreLogo.png", 50, 50),
                ("Splash.png", 960, 540),
                ("Splash620.png", 620, 300),
                ("Wide310.png", 310, 150),
            ],
        };
        for &(name, w, h) in logos {
            logo(colour, w, h).save_with_format(assets.join(name), ImageFormat::Png).map_err(io::Error::other)?;
        }
        let manifest = if game.kind == Kind::Broken { "<Package><Identity".to_string() } else { manifest(game) };
        std::fs::write(package.join(crate::manifest::MANIFEST), manifest)?;
        if game.kind != Kind::App {
            std::fs::write(package.join(crate::manifest::CONFIG), config(game))?;
        }
    }
    Ok(())
}

/// A colour of the package's own, from its name: blues to greens, unlike the other stores' fixtures.
fn colour(identity: &str) -> [u8; 3] {
    let n = identity.bytes().fold(0u32, |n, b| n.wrapping_mul(31).wrapping_add(b as u32));
    [30 + (n % 60) as u8, 120 + (n / 7 % 100) as u8, 90 + (n / 13 % 120) as u8]
}

/// A logo: the colour, darker towards the bottom right.
fn logo(colour: [u8; 3], w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        let t = (x as f32 / w as f32 + y as f32 / h as f32) / 2.0;
        Rgb(colour.map(|c| (c as f32 * (1.0 - 0.6 * t)) as u8))
    })
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
}

/// The package's manifest: whole, as Windows registers it from loose files, its one application the stand-in game.
fn manifest(game: &FixtureGame) -> String {
    let name = escape(&game.name);
    let scaled = game.kind == Kind::Scaled;
    // Lumen Hollow's names are in its resources, as many a Store package's are: its config has the plain one.
    let display = if scaled { "ms-resource:AppDisplayName".to_string() } else { name.clone() };
    let tile = if scaled {
        String::new()
    } else {
        "\n        <uap:DefaultTile Wide310x150Logo=\"Assets\\Wide310.png\" />".into()
    };
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap rescap">
  <Identity Name="{identity}" Publisher="{publisher}" Version="1.0.0.0" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>{display}</DisplayName>
    <PublisherDisplayName>Made Up Studio</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Applications>
    <Application Id="Game" Executable="game.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="{display}" Description="{name}" BackgroundColor="transparent"
                          Square150x150Logo="Assets\Logo150.png" Square44x44Logo="Assets\Logo44.png">{tile}
        <uap:SplashScreen Image="Assets\Splash620.png" />
      </uap:VisualElements>
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"#,
        identity = game.identity,
        publisher = PUBLISHER,
    )
}

/// The game's `MicrosoftGame.config`.
fn config(game: &FixtureGame) -> String {
    let name = escape(&game.name);
    // Twelve letters and digits, as a product's is: made up.
    let store_id = match game.kind {
        Kind::Modifiable => "",
        Kind::Whole => "\n  <StoreId>9ZZZMADEUP01</StoreId>",
        _ => "\n  <StoreId>9ZZZMADEUP02</StoreId>",
    };
    let visuals = match game.kind {
        Kind::Scaled => {
            r#"Square150x150Logo="Assets\Logo150.png"
                Square44x44Logo="Assets\Logo44.png"
                StoreLogo="Assets\StoreLogo.png""#
        }
        _ => {
            r#"Square480x480Logo="Assets\Logo480.png"
                Square150x150Logo="Assets\Logo150.png"
                Square44x44Logo="Assets\Logo44.png"
                StoreLogo="Assets\StoreLogo.png"
                SplashScreenImage="Assets\Splash.png""#
        }
    };
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Game configVersion="1">
  <Identity Name="{identity}" Publisher="{publisher}" Version="1.0.0.0" />
  <ExecutableList>
    <Executable Name="game.exe" Id="Game" TargetDeviceFamily="PC" />
  </ExecutableList>{store_id}
  <ShellVisuals DefaultDisplayName="{name}"
                PublisherDisplayName="Made Up Studio"
                {visuals} />
</Game>
"#,
        identity = game.identity,
        publisher = PUBLISHER,
    )
}

/// A market's games: product id, name and package identity each.
pub type Market = (&'static str, &'static [(&'static str, &'static str, &'static str)]);

/// The fake PC Game Pass catalogue's games, by market: (product id, name, the identity its family name is made of).
/// Tidewater Trials is the fake drive's own installed game (`9ZZZMADEUP01`), so it is listed once; GB's list has one
/// game more than US's, so a change of region shows. Every name and id is made up.
pub const GAME_PASS: &[Market] = &[
    (
        "US",
        &[
            ("9ZZZMADEUP01", "Tidewater Trials", "MadeUpStudio.TidewaterTrials"),
            ("9ZZZGPASS101", "Saltmarsh Run", "MadeUpStudio.SaltmarshRun"),
            ("9ZZZGPASS102", "Ember Orchard", "MadeUpStudio.EmberOrchard"),
        ],
    ),
    (
        "GB",
        &[
            ("9ZZZMADEUP01", "Tidewater Trials", "MadeUpStudio.TidewaterTrials"),
            ("9ZZZGPASS101", "Saltmarsh Run", "MadeUpStudio.SaltmarshRun"),
            ("9ZZZGPASS102", "Ember Orchard", "MadeUpStudio.EmberOrchard"),
            ("9ZZZGPASS103", "Kestrel Line", "MadeUpStudio.KestrelLine"),
        ],
    ),
];

/// Writes a local PC Game Pass catalogue at `dir` (`--gamepass <dir>`, `standin make-gamepass`): for each market of
/// [`GAME_PASS`], `<dir>/<CC>/sigl.json` and `products.json`, the two answers in the real ones' shapes
/// (`library::game_pass`), with no pictures.
pub fn write_game_pass(dir: &Path) -> io::Result<()> {
    for (market, games) in GAME_PASS {
        let at = dir.join(market);
        std::fs::create_dir_all(&at)?;
        let mut sigl = vec![
            r#"{"siglId":"made-up","title":"All PC Games","description":"","requiresShuffling":"False","imageUrl":""}"#
                .to_string(),
        ];
        let mut products = Vec::new();
        for (id, name, identity) in games.iter() {
            sigl.push(format!(r#"{{"id":"{id}"}}"#));
            let family = crate::family::family_name(identity, PUBLISHER);
            products.push(format!(
                r#"{{"ProductId":"{id}","ProductKind":"Game","LocalizedProperties":[{{"ProductTitle":"{name}","DeveloperName":"Made Up Studio","PublisherName":"Made Up Studio","Images":[]}}],"MarketProperties":[{{"OriginalReleaseDate":"2024-05-01T00:00:00.0000000Z"}}],"Properties":{{"PackageFamilyName":"{family}"}},"DisplaySkuAvailabilities":[]}}"#
            ));
        }
        std::fs::write(at.join("sigl.json"), format!("[{}]", sigl.join(",")))?;
        std::fs::write(at.join("products.json"), format!(r#"{{"Products":[{}]}}"#, products.join(",")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_each_package_with_its_logos_and_the_program_as_its_game() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("xbox");
        let exe = dir.path().join("stand-in.exe");
        std::fs::write(&exe, b"stand-in").unwrap();
        let spec = Fixture { game_exe: Some(exe), ..Fixture::standard() };
        write(&root, &spec).unwrap();
        let tidewater = spec.games[0].package_root(&root);
        assert_eq!(std::fs::read(tidewater.join("game.exe")).unwrap(), b"stand-in");
        assert_eq!(std::fs::read_to_string(tidewater.join("game-args.txt")).unwrap(), "--title\nTidewater Trials\n");
        let logo = image::open(tidewater.join("Assets").join("Logo480.png")).unwrap();
        assert_eq!((logo.width(), logo.height()), (480, 480));
        // Every file the manifest names is there, as Windows checks when it registers the package.
        let manifest = crate::manifest::read_manifest(&tidewater).unwrap();
        let app = &manifest.applications[0];
        for file in [&manifest.logo, &app.square150, &app.square44, &app.wide310, &app.splash, &app.executable] {
            let file = file.as_deref().unwrap().replace('\\', "/");
            assert!(tidewater.join(&file).is_file(), "{file}");
        }
        let notes = spec.games[3].package_root(&root);
        assert!(notes.join(crate::manifest::MANIFEST).is_file());
        assert!(!notes.join(crate::manifest::CONFIG).exists());
    }
}
