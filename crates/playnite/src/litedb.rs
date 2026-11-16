//! LiteDB 4's data file (its format version 7), which Playnite 10 keeps each of its library's collections in
//! (`games.db`, `filterpresets.db`, ...). Read: every document of a collection, by walking its `_id` index. Written:
//! a whole file at once, for the made-up libraries of the tests (`standin make-playnite`).
//!
//! The layout is LiteDB's own reader of this format, `FileReaderV7`, which Playnite carries to recover damaged files
//! (`source/Playnite/Database/Collections/LiteDBFileReaderV7.cs`), checked against files LiteDB 4.1.4 wrote with
//! Playnite's own models (`tools/playnite-litedb`, `tests/litedb4/`): docs/import.md.
//!
//! The file is pages of 4096 bytes, each with a 25-byte header: its id (u32), its type (u8: 1 header, 2 collection,
//! 3 index, 4 data, 5 extend, 0 empty), the previous and next page (u32, `u32::MAX` for none), how many items it
//! holds (u16), how many bytes are free (u16), and 8 reserved. All little-endian. Page 0 is the header: the
//! collections and the page each one's collection page is. A collection page names its indexes, `_id` first, each
//! with its head and tail node; an index is a skip list of nodes on index pages, ordered by key, each node pointing
//! at its document's data block. A document longer than a data page holds is in a chain of extend pages instead.
use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::bson::{self, Doc, Guid, Value};

pub const PAGE: usize = 4096;
const HEADER: usize = 25;
const INFO: &[u8; 27] = b"** This is a LiteDB file **";
const VERSION: u8 = 7;
const NONE: u32 = u32::MAX;
/// The most an index node links: the head and tail nodes have every level.
const MAX_LEVEL: u8 = 32;
/// A collection page has this many index slots.
const INDEXES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Empty,
    Header,
    Collection,
    Index,
    Data,
    Extend,
}

impl Kind {
    fn of(byte: u8) -> Option<Kind> {
        Some(match byte {
            0 => Kind::Empty,
            1 => Kind::Header,
            2 => Kind::Collection,
            3 => Kind::Index,
            4 => Kind::Data,
            5 => Kind::Extend,
            _ => return None,
        })
    }

    fn byte(self) -> u8 {
        match self {
            Kind::Empty => 0,
            Kind::Header => 1,
            Kind::Collection => 2,
            Kind::Index => 3,
            Kind::Data => 4,
            Kind::Extend => 5,
        }
    }
}

/// A page and an item on it: where a node or a data block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Address {
    page: u32,
    index: u16,
}

impl Address {
    const NONE: Address = Address { page: NONE, index: u16::MAX };

    fn is_none(self) -> bool {
        self.page == NONE
    }
}

/// A LiteDB 4 file, read whole into memory. Opened for reading only: nothing is ever written to it.
pub struct File {
    bytes: Vec<u8>,
    collections: Vec<(String, u32)>,
}

/// Why a file is not one this reader takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    NotLiteDb,
    /// Another version of LiteDB's format: LiteDB 5's is version 8.
    Version(u8),
    Encrypted,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::NotLiteDb => write!(f, "it is not a LiteDB file"),
            Refused::Version(v) => write!(f, "it is LiteDB's format version {v}, and only version 7 (LiteDB 4) is read"),
            Refused::Encrypted => write!(f, "it has a password"),
        }
    }
}

impl std::error::Error for Refused {}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], at: usize) -> Self {
        Self { bytes, at }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).filter(|&end| end <= self.bytes.len()).context("a page runs short")?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }

    fn address(&mut self) -> Result<Address> {
        Ok(Address { page: self.u32()?, index: self.u16()? })
    }

    /// A string as LiteDB 4 writes one in a page: its length in bytes (i32), then UTF-8.
    fn string(&mut self) -> Result<String> {
        let length = usize::try_from(i32::from_le_bytes(self.take(4)?.try_into()?)).context("a name's length")?;
        Ok(String::from_utf8_lossy(self.take(length)?).into_owned())
    }
}

/// An index node, as much of it as a walk needs.
struct Node {
    data: Address,
    next: Address,
}

impl File {
    /// Reads the file at `path`, and checks that it is LiteDB 4's and has no password.
    pub fn open(path: &Path) -> Result<File> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        File::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<File> {
        if bytes.len() < PAGE || Kind::of(bytes[4]) != Some(Kind::Header) || &bytes[HEADER..HEADER + 27] != INFO {
            bail!(Refused::NotLiteDb);
        }
        let mut header = Cursor::new(&bytes, HEADER + 27);
        let version = header.u8()?;
        if version != VERSION {
            bail!(Refused::Version(version));
        }
        // ChangeID, FreeEmptyPageID, LastPageID, UserVersion.
        header.take(2 + 4 + 4 + 2)?;
        let password = header.take(20)?;
        let salt = header.take(16)?;
        if password.iter().chain(salt).any(|&b| b != 0) {
            bail!(Refused::Encrypted);
        }
        let mut collections = Vec::new();
        for _ in 0..header.u8()? {
            let name = header.string()?;
            collections.push((name, header.u32()?));
        }
        Ok(File { bytes, collections })
    }

    /// The collections in the file, by name.
    pub fn collections(&self) -> impl Iterator<Item = &str> {
        self.collections.iter().map(|(name, _)| name.as_str())
    }

    fn page(&self, id: u32, kind: Kind) -> Result<&[u8]> {
        let start = (id as usize).checked_mul(PAGE).context("a page beyond the file")?;
        let page = self.bytes.get(start..start + PAGE).with_context(|| format!("page {id} is beyond the file"))?;
        if Kind::of(page[4]) != Some(kind) {
            bail!("page {id} is not the {kind:?} page it should be");
        }
        Ok(page)
    }

    fn item_count(page: &[u8]) -> usize {
        u16::from_le_bytes([page[13], page[14]]) as usize
    }

    /// Where the `_id` index of the collection starts: its head node.
    fn head(&self, collection: &str) -> Result<Option<Address>> {
        let Some(&(_, page)) = self.collections.iter().find(|(name, _)| name == collection) else {
            return Ok(None);
        };
        let mut c = Cursor::new(self.page(page, Kind::Collection)?, HEADER);
        c.string()?;
        // DocumentCount (i64), FreeDataPageID.
        c.take(8 + 4)?;
        for _ in 0..INDEXES {
            let field = c.string()?;
            let _unique = c.u8()?;
            let head = c.address()?;
            // TailNode, FreeIndexPageID.
            c.take(6 + 4)?;
            // "_id", or "_id=$._id" as LiteDB 4.1 writes it.
            if field.split('=').next() == Some("_id") {
                return Ok(Some(head));
            }
        }
        bail!("collection {collection} has no _id index")
    }

    /// The node at `at`, read from its index page.
    fn node(&self, at: Address) -> Result<Node> {
        let page = self.page(at.page, Kind::Index)?;
        let mut c = Cursor::new(page, HEADER);
        for _ in 0..Self::item_count(page) {
            let index = c.u16()?;
            let levels = c.u8()?;
            // Slot, PrevNode, NextNode: the other indexes' nodes of the same document.
            c.take(1 + 6 + 6)?;
            let key_length = c.u16()? as usize;
            // The key's type, and the key.
            c.take(1 + key_length)?;
            let data = c.address()?;
            let _prev = c.address()?;
            let next = c.address()?;
            c.take(levels.saturating_sub(1) as usize * 12)?;
            if index == at.index {
                return Ok(Node { data, next });
            }
        }
        bail!("no node {} on page {}", at.index, at.page)
    }

    /// The bytes of the data block at `at`, its extend pages' if it has them.
    fn data(&self, at: Address) -> Result<Vec<u8>> {
        let page = self.page(at.page, Kind::Data)?;
        let mut c = Cursor::new(page, HEADER);
        for _ in 0..Self::item_count(page) {
            let index = c.u16()?;
            let extend = c.u32()?;
            let length = c.u16()? as usize;
            let data = c.take(length)?;
            if index != at.index {
                continue;
            }
            if extend == NONE {
                return Ok(data.to_vec());
            }
            let mut out = Vec::new();
            let mut next = extend;
            let mut seen = 0;
            while next != NONE {
                let page = self.page(next, Kind::Extend)?;
                let length = Self::item_count(page).min(PAGE - HEADER);
                out.extend(&page[HEADER..HEADER + length]);
                next = u32::from_le_bytes(page[9..13].try_into()?);
                seen += 1;
                if seen > self.bytes.len() / PAGE {
                    bail!("extend pages that go round in a circle");
                }
            }
            return Ok(out);
        }
        bail!("no data block {} on page {}", at.index, at.page)
    }

    /// Every document of the collection, in the order of their `_id`s. A collection the file does not have has
    /// none.
    pub fn documents(&self, collection: &str) -> Result<Vec<Doc>> {
        let Some(head) = self.head(collection)? else { return Ok(Vec::new()) };
        let mut docs = Vec::new();
        let mut at = self.node(head)?.next;
        // A skip list's level 0 is every node once; more steps than the file has room for nodes is a loop.
        let most = self.bytes.len() / 20;
        while !at.is_none() {
            let node = self.node(at)?;
            if !node.data.is_none() {
                let bytes = self.data(node.data)?;
                docs.push(bson::read(&bytes).with_context(|| format!("a document of {collection}"))?);
            }
            at = node.next;
            if docs.len() > most {
                bail!("the index of {collection} goes round in a circle");
            }
        }
        Ok(docs)
    }
}

// ------------------------------------------------------------------------------------------------------- writing

/// A key as an index node holds it: LiteDB's own type numbers (`BsonType`), not BSON's.
fn key(value: &Value) -> Result<(u8, Vec<u8>)> {
    Ok(match value {
        Value::MinKey => (0, Vec::new()),
        Value::Int32(n) => (2, n.to_le_bytes().to_vec()),
        Value::Int64(n) => (3, n.to_le_bytes().to_vec()),
        Value::String(text) => (6, text.as_bytes().to_vec()),
        Value::Guid(guid) => (11, guid.0.to_vec()),
        Value::MaxKey => (14, Vec::new()),
        other => bail!("an _id LiteDB files are not written with here: {other:?}"),
    })
}

/// The order of `_id`s in the index, as LiteDB compares them: an id of each type the writer takes.
fn key_order(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Guid(a), Value::Guid(b)) => a.net_cmp(b),
        (Value::Int32(a), Value::Int32(b)) => a.cmp(b),
        (Value::Int64(a), Value::Int64(b)) => a.cmp(b),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        _ => std::cmp::Ordering::Equal,
    }
}

/// Pages as they are being made.
struct Pages(Vec<[u8; PAGE]>);

impl Pages {
    fn add(&mut self, kind: Kind) -> u32 {
        let id = self.0.len() as u32;
        let mut page = [0u8; PAGE];
        page[0..4].copy_from_slice(&id.to_le_bytes());
        page[4] = kind.byte();
        page[5..9].copy_from_slice(&NONE.to_le_bytes());
        page[9..13].copy_from_slice(&NONE.to_le_bytes());
        page[15..17].copy_from_slice(&((PAGE - HEADER) as u16).to_le_bytes());
        self.0.push(page);
        id
    }

    /// Puts an item's bytes on a page after those it has, as LiteDB counts it: one more item, that many fewer free.
    fn push(&mut self, id: u32, used: &mut usize, item: &[u8]) {
        let page = &mut self.0[id as usize];
        page[HEADER + *used..HEADER + *used + item.len()].copy_from_slice(item);
        *used += item.len();
        let count = u16::from_le_bytes([page[13], page[14]]) + 1;
        page[13..15].copy_from_slice(&count.to_le_bytes());
        page[15..17].copy_from_slice(&((PAGE - HEADER - *used) as u16).to_le_bytes());
    }
}

fn put_address(out: &mut Vec<u8>, at: Address) {
    out.extend(at.page.to_le_bytes());
    out.extend(at.index.to_le_bytes());
}

fn put_string(out: &mut Vec<u8>, text: &str) {
    out.extend((text.len() as i32).to_le_bytes());
    out.extend(text.as_bytes());
}

/// A file of these collections, each with its documents, as LiteDB 4 lays one out: what `File` reads back, and LiteDB
/// itself (`tools/playnite-litedb dump` and `touch`). Each document needs an `_id`: a Guid, a number or a string.
/// Each node of the `_id` index is on its lowest level only, which is a skip list still: LiteDB's own nodes have
/// random heights, and finds and inserts the same either way.
pub fn write(collections: &[(&str, Vec<Doc>)]) -> Result<Vec<u8>> {
    if collections.len() > u8::MAX as usize {
        bail!("too many collections");
    }
    let mut pages = Pages(Vec::new());
    pages.add(Kind::Header);
    let mut placed = Vec::new();
    for (name, docs) in collections {
        let collection = pages.add(Kind::Collection);
        placed.push((*name, collection));

        // The documents, each in a data block: on the data page being filled while it has room, else in extend
        // pages of their own, the block empty.
        let mut blocks = Vec::new();
        let mut data: Option<(u32, usize, u16)> = None;
        for doc in docs {
            let id = doc.get("_id").with_context(|| format!("a document of {name} with no _id"))?.clone();
            let bytes = bson::write(doc);
            let inline = bytes.len() + 8 <= PAGE - HEADER;
            let (extend, payload) = if inline { (NONE, &bytes[..]) } else { (extend_pages(&mut pages, &bytes), &[][..]) };
            let size = payload.len() + 8;
            let (page, used, index) = match data {
                Some((page, used, index)) if used + size <= PAGE - HEADER => (page, used, index),
                _ => (pages.add(Kind::Data), 0, 0),
            };
            let mut block = Vec::new();
            block.extend(index.to_le_bytes());
            block.extend(extend.to_le_bytes());
            block.extend((payload.len() as u16).to_le_bytes());
            block.extend(payload);
            let mut used = used;
            pages.push(page, &mut used, &block);
            blocks.push((id, Address { page, index }));
            data = Some((page, used, index + 1));
        }
        blocks.sort_by(|a, b| key_order(&a.0, &b.0));

        // The `_id` index: the head (every level, the lowest key), each document's node in key order, the tail (every
        // level, the highest key). Nodes are placed first, so each knows where its neighbours are.
        let mut nodes: Vec<(Value, Address, u8)> = vec![(Value::MinKey, Address::NONE, MAX_LEVEL)];
        nodes.extend(blocks.iter().map(|(id, at)| (id.clone(), *at, 1)));
        nodes.push((Value::MaxKey, Address::NONE, MAX_LEVEL));
        let mut sizes = Vec::new();
        for (id, _, levels) in &nodes {
            let (_, bytes) = key(id)?;
            sizes.push(2 + 1 + 1 + 6 + 6 + 2 + 1 + bytes.len() + 6 + *levels as usize * 12);
        }
        let mut places = Vec::new();
        let mut page = pages.add(Kind::Index);
        let first = page;
        let (mut used, mut index) = (0usize, 0u16);
        for size in &sizes {
            if used + size > PAGE - HEADER {
                page = pages.add(Kind::Index);
                (used, index) = (0, 0);
            }
            places.push(Address { page, index });
            used += size;
            index += 1;
        }
        let (mut page, mut used) = (first, 0usize);
        for (i, (id, data, levels)) in nodes.iter().enumerate() {
            let at = places[i];
            if at.page != page {
                (page, used) = (at.page, 0);
            }
            let (kind, bytes) = key(id)?;
            let mut node = Vec::new();
            node.extend(at.index.to_le_bytes());
            node.push(*levels);
            // The index's slot, then the other indexes' nodes of this document: none.
            node.push(0);
            put_address(&mut node, Address::NONE);
            put_address(&mut node, Address::NONE);
            node.extend((bytes.len() as u16).to_le_bytes());
            node.push(kind);
            node.extend(&bytes);
            put_address(&mut node, *data);
            let last = nodes.len() - 1;
            for level in 0..*levels {
                // Level 0 links each node to the next; above it the head links straight to the tail.
                let (prev, next) = match (i, level) {
                    (0, 0) => (Address::NONE, places[1]),
                    (0, _) => (Address::NONE, places[last]),
                    (i, 0) if i == last => (places[i - 1], Address::NONE),
                    (i, _) if i == last => (places[0], Address::NONE),
                    (i, _) => (places[i - 1], places[i + 1]),
                };
                put_address(&mut node, prev);
                put_address(&mut node, next);
            }
            pages.push(page, &mut used, &node);
        }

        // The collection page: its name, its count, no page with room for more data, and the one index.
        let mut content = Vec::new();
        put_string(&mut content, name);
        content.extend((docs.len() as i64).to_le_bytes());
        content.extend(NONE.to_le_bytes());
        for slot in 0..INDEXES {
            if slot == 0 {
                put_string(&mut content, "_id=$._id");
                content.push(1);
                put_address(&mut content, places[0]);
                put_address(&mut content, places[nodes.len() - 1]);
            } else {
                put_string(&mut content, "");
                content.push(0);
                put_address(&mut content, Address::NONE);
                put_address(&mut content, Address::NONE);
            }
            content.extend(NONE.to_le_bytes());
        }
        let page = &mut pages.0[collection as usize];
        page[HEADER..HEADER + content.len()].copy_from_slice(&content);
        page[15..17].copy_from_slice(&((PAGE - HEADER - content.len()) as u16).to_le_bytes());
    }

    // The header: what it is, its version, no change yet, no empty page, the last page, and where each collection is.
    let last = pages.0.len() as u32 - 1;
    let mut content = Vec::new();
    content.extend(INFO);
    content.push(VERSION);
    content.extend(0u16.to_le_bytes());
    content.extend(NONE.to_le_bytes());
    content.extend(last.to_le_bytes());
    content.extend(0u16.to_le_bytes());
    content.extend([0u8; 20 + 16]);
    content.push(placed.len() as u8);
    for (name, page) in &placed {
        put_string(&mut content, name);
        content.extend(page.to_le_bytes());
    }
    if content.len() > PAGE - HEADER {
        bail!("too many collections for the header");
    }
    pages.0[0][HEADER..HEADER + content.len()].copy_from_slice(&content);
    Ok(pages.0.concat())
}

/// `bytes` across as many extend pages as it takes, each linked to the next: the first one's id.
fn extend_pages(pages: &mut Pages, bytes: &[u8]) -> u32 {
    let chunks: Vec<&[u8]> = bytes.chunks(PAGE - HEADER).collect();
    let ids: Vec<u32> = chunks.iter().map(|_| pages.add(Kind::Extend)).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let page = &mut pages.0[ids[i] as usize];
        if i > 0 {
            page[5..9].copy_from_slice(&ids[i - 1].to_le_bytes());
        }
        if let Some(next) = ids.get(i + 1) {
            page[9..13].copy_from_slice(&next.to_le_bytes());
        }
        // An extend page's item count is how many of its bytes are the document's.
        page[13..15].copy_from_slice(&(chunk.len() as u16).to_le_bytes());
        page[15..17].copy_from_slice(&((PAGE - HEADER - chunk.len()) as u16).to_le_bytes());
        page[HEADER..HEADER + chunk.len()].copy_from_slice(chunk);
    }
    ids[0]
}

/// Every collection of a file with its documents' `_id`s, for a test to look at.
pub fn ids(file: &File) -> Result<HashMap<String, Vec<String>>> {
    let mut out = HashMap::new();
    for name in file.collections() {
        let ids = file.documents(name)?.iter().map(|doc| format!("{:?}", doc.get("_id"))).collect();
        out.insert(name.to_string(), ids);
    }
    Ok(out)
}

/// A Guid `_id`, for the tests' documents.
pub fn guid_id(text: &str) -> Value {
    Value::Guid(Guid::parse(text).expect("a Guid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(id: Value, name: &str) -> Doc {
        Doc::default().with("_id", id).with("Name", Value::String(name.into()))
    }

    #[test]
    fn a_written_file_reads_back_in_id_order() {
        let mut games = Vec::new();
        for n in (0..300u32).rev() {
            let id = Value::Guid(Guid::parse(&format!("{n:08x}-0000-4000-8000-000000000000")).unwrap());
            games.push(doc(id, &format!("Game {n}")));
        }
        // Longer than a page, twice over.
        let long = doc(guid_id("ffffffff-0000-4000-8000-000000000000"), "Long").with("Description", Value::String("x".repeat(9000)));
        games.push(long.clone());
        let settings = vec![Doc::default().with("_id", Value::Int32(0)).with("SortingOrder", Value::Array(Vec::new()))];
        let bytes = write(&[("Game", games), ("Settings", settings.clone())]).unwrap();
        assert_eq!(bytes.len() % PAGE, 0);

        let file = File::from_bytes(bytes).unwrap();
        assert_eq!(file.collections().collect::<Vec<_>>(), ["Game", "Settings"]);
        let read = file.documents("Game").unwrap();
        assert_eq!(read.len(), 301);
        assert_eq!(read[0].str("Name"), Some("Game 0"));
        assert_eq!(read[299].str("Name"), Some("Game 299"));
        assert_eq!(read[300], long);
        assert_eq!(file.documents("Settings").unwrap(), settings);
        assert!(file.documents("Nothing").unwrap().is_empty());
    }

    #[test]
    fn what_is_not_a_litedb_4_file_is_refused() {
        let refused = |bytes: Vec<u8>| File::from_bytes(bytes).err().and_then(|e| e.downcast::<Refused>().ok());
        assert_eq!(refused(vec![0; 100]), Some(Refused::NotLiteDb));
        let mut bytes = write(&[("Game", Vec::new())]).unwrap();
        bytes[HEADER + 27] = 8;
        assert_eq!(refused(bytes.clone()), Some(Refused::Version(8)));
        bytes[HEADER + 27] = 7;
        // The salt is past the info, the version, and 12 bytes of counts and the 20 of the password.
        bytes[HEADER + 28 + 12 + 20] = 1;
        assert_eq!(refused(bytes), Some(Refused::Encrypted));
    }
}
