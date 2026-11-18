//! Games & apps > Storage (docs/spec/storage.md; #179): the drives of this PC with their free and total space,
//! and each store's install folders, the one its installs go to ticked. The Bundle has no such page (Playnite left
//! it to the stores): it is the port's, after the console's Storage, made of Settings' Rows in the Bundle's look.
//!
//! A on a folder of a store that can be told which folder makes it the one its installs go to unless the user says
//! otherwise (`Effect::DefaultFolder`): Install then asks "Where do you want to install X?" starting on it. A store
//! that cannot be told has its folders listed, its own default ticked, and A does nothing there. What is shown is
//! the library's [`model::Storage`], which the shell reads off the UI thread and is asked to read again as the page
//! opens (`Effect::LookAtStorage`).
use model::storage::{Drive, Place, StoreFolders};
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, glyph, BOXES_TOP, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH};
use super::{Pane, Step};
use crate::pages::{Around, PageAction};
use crate::theme::{ACCENT, BODY, TEXT_DIM};
use crate::Effect;

/// The box on Games & apps, and the page's header.
pub const TITLE: &str = "Storage";
const DRIVES: &str = "Drives";
const NO_DRIVES: &str = "This PC's drives couldn't be read.";
const NO_STORES: &str = "No launcher here says where it installs games.";
/// The line under a store that chooses its own folder, so A does nothing on its Rows: one that can be told has none.
const OWN: &str = "Installs go to the folder ticked: this launcher chooses it for itself.";
/// The bar under a drive: its track and how much is used.
const TRACK: Color = Color::rgb(0x262626);
const BAR: f32 = 6.0;

/// Where focus is: a store's folder, by the store's place and the folder's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct At {
    store: usize,
    folder: usize,
}

/// What is drawn where, top to bottom, as laid out for the page's width.
struct Layout {
    /// Each drive's top.
    drives: Vec<f32>,
    /// Each store's label's top, its note's if it has one, then each of its folders' Row.
    stores: Vec<(f32, Option<f32>, Vec<Rect>)>,
    /// Where the line said in place of the stores is, with none.
    none: f32,
}

pub struct Storage {
    storage: model::Storage,
    at: Option<At>,
    /// The page has yet to ask for the drives to be read again.
    look: bool,
    scroll: f32,
}

impl Storage {
    /// The page as the library has it now, focus on the first store's ticked folder.
    pub fn open(around: Around) -> Self {
        let storage = around.library.storage.clone();
        let at = first(&storage);
        Self { storage, at, look: true, scroll: 0.0 }
    }

    fn store(&self) -> Option<&StoreFolders> {
        self.storage.stores.get(self.at?.store)
    }

    /// Every place focus can be, top to bottom.
    fn places(&self) -> Vec<At> {
        let stores = self.storage.stores.iter().enumerate();
        stores.flat_map(|(store, folders)| (0..folders.folders.len()).map(move |folder| At { store, folder })).collect()
    }

    fn layout(&self, ui: &mut Ui) -> Layout {
        let line = ui.measure(&BODY, "0").height;
        let mut y = BOXES_TOP + line + 12.0;
        let mut drives = Vec::new();
        for _ in &self.storage.drives {
            drives.push(y);
            y += line + 8.0 + BAR + 22.0;
        }
        if self.storage.drives.is_empty() {
            y += line + 22.0;
        }
        y += 18.0;
        let none = y;
        let mut stores = Vec::new();
        for store in &self.storage.stores {
            let label = y;
            let note = (!store.choose).then_some(label + line + 2.0);
            y = note.unwrap_or(label) + line + 14.0;
            let rows = store
                .folders
                .iter()
                .map(|_| {
                    let row = Rect::new(LEFT, y, ROW_WIDTH, ROW_HEIGHT);
                    y += ROW_PITCH;
                    row
                })
                .collect();
            stores.push((label, note, rows));
            y += 26.0;
        }
        Layout { drives, stores, none }
    }
}

/// Focus as the page opens: the first store's ticked folder.
fn first(storage: &model::Storage) -> Option<At> {
    let store = storage.stores.iter().position(|store| !store.folders.is_empty())?;
    Some(At { store, folder: storage.stores[store].default })
}

impl Pane for Storage {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `Storage: Steam D:\SteamLibrary`, `Storage: Nothing` with no store to show.
    fn focus(&self) -> String {
        match (self.store(), self.at) {
            (Some(store), Some(at)) => {
                let path = store.folders.get(at.folder).map(|place| place.path.display().to_string());
                format!("{TITLE}: {} {}", store.name, path.unwrap_or_default())
            }
            _ => format!("{TITLE}: Nothing"),
        }
    }

    /// Whether the focused folder is its store's ticked one, and how many drives there are: `the default; 2 drives`.
    fn state(&self) -> String {
        let ticked = match (self.store(), self.at) {
            (Some(store), Some(at)) if store.default == at.folder => "the default; ",
            (Some(_), Some(_)) => "not the default; ",
            _ => "",
        };
        let drives = match self.storage.drives.len() {
            1 => "1 drive".to_string(),
            n => format!("{n} drives"),
        };
        format!("{ticked}{drives}")
    }

    fn press(&mut self, press: Press, _around: Around) -> Step {
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        let places = self.places();
        let here = self.at.and_then(|at| places.iter().position(|place| *place == at));
        match (press.button, here) {
            (Button::B | Button::Left, _) => return Step::Back,
            (Button::Up, Some(here)) => self.at = Some(places[here.saturating_sub(1)]),
            (Button::Down, Some(here)) => self.at = Some(places[(here + 1).min(places.len() - 1)]),
            (Button::A, Some(_)) => {
                let (Some(store), Some(at)) = (self.store(), self.at) else { return Step::Stay };
                if !store.choose || store.default == at.folder {
                    return Step::Stay;
                }
                let Some(place) = store.folders.get(at.folder) else { return Step::Stay };
                let effect = Effect::DefaultFolder { store: store.store.clone(), folder: place.path.clone() };
                // Ticked at once; the snapshot that follows says the same.
                self.storage.stores[at.store].default = at.folder;
                return Step::Do(PageAction::Effect(effect));
            }
            _ => {}
        }
        Step::Stay
    }

    /// The new snapshot's drives and folders; focus stays on the same folder of the same store where it still is,
    /// else on the first store's ticked one.
    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        let was = self
            .store()
            .zip(self.at)
            .and_then(|(store, at)| Some((store.store.clone(), store.folders.get(at.folder)?.path.clone())));
        self.storage = around.library.storage.clone();
        self.at = was
            .and_then(|(key, path)| {
                let store = self.storage.stores.iter().position(|store| store.store == key)?;
                let folders = &self.storage.stores[store].folders;
                let folder = folders.iter().position(|place| model::storage::same_folder(&place.path, &path))?;
                Some(At { store, folder })
            })
            .or_else(|| first(&self.storage));
        None
    }

    fn asks(&mut self) -> Vec<Effect> {
        if std::mem::take(&mut self.look) {
            vec![Effect::LookAtStorage]
        } else {
            Vec::new()
        }
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, focused: bool) {
        let layout = self.layout(ui);
        let ring = self.at.and_then(|at| layout.stores.get(at.store)?.2.get(at.folder).copied());
        let (top, bottom) = ring.map_or((BOXES_TOP, BOXES_TOP), |rect| (rect.y, rect.bottom()));
        let Self { storage, at, scroll, .. } = self;
        let focus = at.filter(|_| focused);
        controls::scrolled(ui, scroll, BOXES_TOP, top, bottom, |ui| {
            let drives_top =
                layout.drives.first().copied().unwrap_or(layout.none) - ui.measure(&BODY, "0").height - 12.0;
            controls::label(ui, LEFT, drives_top, DRIVES);
            if storage.drives.is_empty() {
                let y = drives_top + ui.measure(&BODY, "0").height + 12.0;
                controls::description(ui, LEFT, y, ROW_WIDTH, NO_DRIVES);
            }
            for (drive, &y) in storage.drives.iter().zip(&layout.drives) {
                drive_line(ui, y, drive);
            }
            if storage.stores.is_empty() {
                controls::description(ui, LEFT, layout.none, ROW_WIDTH, NO_STORES);
            }
            let mut focused_row = None;
            for (s, (store, (label, note, rows))) in storage.stores.iter().zip(&layout.stores).enumerate() {
                controls::label(ui, LEFT, *label, &store.name);
                if let Some(note) = note {
                    controls::description(ui, LEFT, *note, ROW_WIDTH, OWN);
                }
                for (f, (place, rect)) in store.folders.iter().zip(rows).enumerate() {
                    let row = (*rect, place, f == store.default);
                    if focus == Some(At { store: s, folder: f }) {
                        focused_row = Some(row);
                    } else {
                        folder_row(ui, row, false);
                    }
                }
            }
            // The focused Row last: its ring and glow lie over its neighbours.
            if let Some(row) = focused_row {
                folder_row(ui, row, true);
            }
        });
    }
}

/// A drive: its name, its space at the right, and a bar under them filled as far as it is used.
fn drive_line(ui: &mut Ui, y: f32, drive: &Drive) {
    let line = ui.measure(&BODY, "0").height;
    let text = Rect::new(LEFT, y, ROW_WIDTH, line);
    ui.paragraph(text, &drive.name(), &BODY, HAlign::Left, VAlign::Top, 1);
    ui.paragraph(text, &drive.space_text(), &BODY.color(TEXT_DIM), HAlign::Right, VAlign::Top, 1);
    let bar = Rect::new(LEFT, y + line + 8.0, ROW_WIDTH, BAR);
    ui.fill(bar, TRACK, BAR / 2.0);
    if let Some(used) = drive.used().filter(|used| *used > 0.0) {
        ui.fill(Rect::new(bar.x, bar.y, (bar.w * used as f32).max(BAR), BAR), ACCENT, BAR / 2.0);
    }
}

/// A folder's Row: the tick on its store's default, its path, and its drive's free space at the right.
fn folder_row(ui: &mut Ui, (rect, place, ticked): (Rect, &Place, bool), focused: bool) {
    let free = place.drive.as_ref().map(Drive::free_text).unwrap_or_default();
    let free_width = if free.is_empty() { 0.0 } else { ui.measure(&BODY, &free).width + 24.0 };
    controls::row_marked(ui, rect, "", ticked.then_some(glyph::TICK), focused);
    let path = place.path.display().to_string();
    let name = Rect::new(rect.x + 84.0, rect.y, rect.w - free_width - 108.0, rect.h);
    ui.paragraph(name, &path, &BODY, HAlign::Left, VAlign::Center, 1);
    if !free.is_empty() {
        let right = Rect::new(rect.x, rect.y, rect.w - 24.0, rect.h);
        ui.paragraph(right, &free, &BODY.color(TEXT_DIM), HAlign::Right, VAlign::Center, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_opens_on_the_first_stores_ticked_folder() {
        let place = |path: &str| Place { path: path.into(), drive: None };
        let store = StoreFolders {
            store: "steam".into(),
            name: "Steam".into(),
            folders: vec![place("D:/a"), place("C:/b")],
            choose: true,
            default: 1,
        };
        let storage = model::Storage { stores: vec![store], ..model::Storage::default() };
        assert_eq!(first(&storage), Some(At { store: 0, folder: 1 }));
        assert_eq!(first(&model::Storage::default()), None);
    }
}
