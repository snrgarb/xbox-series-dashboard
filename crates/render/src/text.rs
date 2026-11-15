//! Fonts and shaped text. The fonts are Windows' own, loaded from its Fonts folder at runtime and never bundled:
//! Segoe UI for the interface, Bahnschrift for headings and the clock, Segoe Fluent Icons for glyph icons, Segoe UI
//! Symbol for the few symbols the icon font lacks. Elsewhere (development and CI on Linux) the folder
//! `DASHBOARD_FONTS` names stands in, with Selawik, which has Segoe UI's metrics (tools/test-fonts.sh), and failing
//! that whatever sans-serif the system has. There is no icon font and no symbol font there.
use std::collections::HashMap;
use std::path::PathBuf;

use glyphon::cosmic_text::{Align, Ellipsize, EllipsizeHeightLimit, Wrap};
use glyphon::fontdb::{Database, Source};
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Font {
    /// Segoe UI.
    #[default]
    Ui,
    /// Bahnschrift.
    Display,
    /// Segoe Fluent Icons, else Segoe MDL2 Assets.
    Icons,
    /// Segoe UI Symbol: where WPF finds a symbol Segoe UI has no glyph for (the trophy, U+1F3C6), drawn in one
    /// colour as WPF draws it.
    Symbols,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// What a piece of text is laid out from; its shaped buffer is kept under this.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TextKey {
    pub text: String,
    /// Font size in tenths of a pixel.
    pub size: u32,
    /// 300 light, 400 regular, 600 semibold, 700 bold.
    pub weight: u16,
    pub font: Font,
    /// The width it is laid out in, in pixels; 0 = as wide as its one line.
    pub width: u32,
    /// With a width: how many lines it may wrap to, the last ending in an ellipsis; 0 = as many as it takes.
    pub max_lines: u8,
    /// Within its width.
    pub align: HAlign,
}

/// A laid-out text's size in pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Measured {
    /// The widest line.
    pub width: f32,
    pub line_height: f32,
    pub lines: usize,
}

impl Measured {
    pub fn height(&self) -> f32 {
        self.line_height * self.lines as f32
    }
}

pub struct TextSys {
    pub(crate) fonts: FontSystem,
    pub(crate) buffers: HashMap<TextKey, Buffer>,
    families: [Option<String>; 4],
    /// Buffers shaped since the last trim, to drop the ones a frame no longer uses.
    used: std::collections::HashSet<TextKey>,
}

fn load(db: &mut Database, dir: &std::path::Path, files: &[&str]) -> Option<String> {
    let mut family = None;
    for file in files {
        let path = dir.join(file);
        if !path.is_file() {
            continue;
        }
        let ids = db.load_font_source(Source::File(path));
        let name = ids.first().and_then(|&id| db.face(id)).and_then(|f| f.families.first()).map(|f| f.0.clone());
        family = family.or(name);
    }
    family
}

impl TextSys {
    pub(crate) fn new() -> Self {
        let mut db = Database::new();
        let mut families: [Option<String>; 4] = [None, None, None, None];
        if cfg!(windows) {
            let dir = std::env::var_os("WINDIR").map(|w| PathBuf::from(w).join("Fonts")).unwrap_or_default();
            let ui = ["segoeui.ttf", "segoeuil.ttf", "segoeuisl.ttf", "seguisb.ttf", "segoeuib.ttf"];
            families[0] = load(&mut db, &dir, &ui);
            families[1] = load(&mut db, &dir, &["bahnschrift.ttf"]);
            families[2] = load(&mut db, &dir, &["SegoeIcons.ttf"]).or_else(|| load(&mut db, &dir, &["segmdl2.ttf"]));
            families[3] = load(&mut db, &dir, &["seguisym.ttf"]);
            // Emoji the interface fonts lack fall back to this.
            load(&mut db, &dir, &["seguiemj.ttf"]);
        } else if let Some(dir) = std::env::var_os("DASHBOARD_FONTS").map(PathBuf::from) {
            let ui = ["selawk.ttf", "selawkl.ttf", "selawksl.ttf", "selawksb.ttf", "selawkb.ttf"];
            families[0] = load(&mut db, &dir, &ui);
        }
        if families[0].is_none() {
            let dejavu = std::path::Path::new("/usr/share/fonts/truetype/dejavu");
            families[0] = load(&mut db, dejavu, &["DejaVuSans.ttf", "DejaVuSans-Bold.ttf"]);
        }
        if families[0].is_none() {
            db.load_system_fonts();
            families[0] = db.faces().next().and_then(|f| f.families.first()).map(|f| f.0.clone());
        }
        if let Some(family) = &families[0] {
            eprintln!("font: {family} ({} faces)", db.len());
            db.set_sans_serif_family(family.clone());
        }
        Self {
            fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
            buffers: HashMap::new(),
            families,
            used: Default::default(),
        }
    }

    /// Whether this font is here. Off Windows the icon and symbol fonts aren't, and a screen draws something plainer.
    pub fn has(&self, font: Font) -> bool {
        self.families[font as usize].is_some()
    }

    pub(crate) fn buffer(&mut self, key: &TextKey) -> &Buffer {
        if !self.used.contains(key) {
            self.used.insert(key.clone());
        }
        if !self.buffers.contains_key(key) {
            let size = key.size as f32 / 10.0;
            // Segoe UI's line is 1.33 of its size, Bahnschrift's 1.2.
            let line = if key.font == Font::Display { 1.2 } else { 1.33 };
            let line = if key.font == Font::Icons { size } else { (size * line).round() };
            let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, line));
            buffer.set_size((key.width > 0).then_some(key.width as f32), None);
            match key.max_lines {
                0 => {}
                1 => {
                    buffer.set_wrap(Wrap::None);
                    buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
                }
                n => buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(n as usize))),
            }
            let family = self.families[key.font as usize].as_deref().or(self.families[0].as_deref());
            let family = family.map_or(Family::SansSerif, Family::Name);
            let attrs = Attrs::new().family(family).weight(Weight(key.weight.max(100)));
            let align = match key.align {
                HAlign::Left => None,
                HAlign::Center => Some(Align::Center),
                HAlign::Right => Some(Align::Right),
            };
            buffer.set_text(&key.text, &attrs, Shaping::Advanced, align);
            buffer.shape_until_scroll(&mut self.fonts, false);
            self.buffers.insert(key.clone(), buffer);
        }
        &self.buffers[key]
    }

    pub fn measure(&mut self, key: &TextKey) -> Measured {
        let buffer = self.buffer(key);
        let width = buffer.layout_runs().map(|r| r.line_w).fold(0.0, f32::max);
        Measured { width, line_height: buffer.metrics().line_height, lines: buffer.layout_runs().count() }
    }

    /// Forgets the text no frame has asked for since the last call.
    pub(crate) fn trim(&mut self) {
        if self.buffers.len() > 512 {
            let used = std::mem::take(&mut self.used);
            self.buffers.retain(|key, _| used.contains(key));
        } else {
            self.used.clear();
        }
    }
}
