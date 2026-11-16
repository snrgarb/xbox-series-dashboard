//! BSON, as LiteDB 4 writes a document (bsonspec.org's format; LiteDB's `BsonWriter`): what Playnite keeps in each
//! of its database's records. Read and written; only the types LiteDB writes are known.
use anyhow::{bail, Context, Result};

/// A value in a document.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Double(f64),
    String(String),
    Document(Doc),
    Array(Vec<Value>),
    /// Binary of a subtype other than a Guid's.
    Binary(u8, Vec<u8>),
    /// A .NET `Guid`: binary subtype 4, in `Guid.ToByteArray`'s order (its first three fields little-endian).
    Guid(Guid),
    ObjectId([u8; 12]),
    Bool(bool),
    /// Milliseconds since 1970 in UTC, as LiteDB writes a `DateTime`.
    DateTime(i64),
    Null,
    Int32(i32),
    Int64(i64),
    Decimal([u8; 16]),
    MinKey,
    MaxKey,
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// A whole number, whichever way it was written: LiteDB writes a `ulong` as an Int64, an `int` as an Int32.
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::Int32(n) => Some(n.into()),
            Value::Int64(n) => Some(n),
            Value::Double(n) if n.is_finite() => Some(n as i64),
            _ => None,
        }
    }

    pub fn as_guid(&self) -> Option<Guid> {
        match self {
            Value::Guid(guid) => Some(*guid),
            _ => None,
        }
    }

    pub fn as_doc(&self) -> Option<&Doc> {
        match self {
            Value::Document(doc) => Some(doc),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// A document: its fields in the order they were written.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Doc(pub Vec<(String, Value)>);

impl Doc {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.iter().find(|(key, _)| key == name).map(|(_, value)| value)
    }

    pub fn with(mut self, name: &str, value: Value) -> Self {
        self.0.push((name.to_string(), value));
        self
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(Value::as_str)
    }

    pub fn bool(&self, name: &str) -> bool {
        self.get(name).and_then(Value::as_bool).unwrap_or(false)
    }

    pub fn guid(&self, name: &str) -> Option<Guid> {
        self.get(name).and_then(Value::as_guid)
    }

    /// The Guids of a list field (`CategoryIds`); none where it is missing.
    pub fn guids(&self, name: &str) -> Vec<Guid> {
        let items = self.get(name).and_then(Value::as_array).unwrap_or_default();
        items.iter().filter_map(Value::as_guid).collect()
    }
}

/// A .NET `Guid`, as its 16 bytes are on disk.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Guid(pub [u8; 16]);

impl Guid {
    pub const EMPTY: Guid = Guid([0; 16]);

    /// From its text, `cb91dfc9-b977-43bf-8e70-55f46e410fab`, in any case.
    pub fn parse(text: &str) -> Option<Guid> {
        let hex: String = text.trim_matches(|c| c == '{' || c == '}').chars().filter(|&c| c != '-').collect();
        if hex.len() != 32 || !hex.is_ascii() {
            return None;
        }
        let mut b = [0u8; 16];
        for (i, byte) in b.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
        }
        // The text is the fields in order, most significant first; on disk the first three are little-endian.
        Some(Guid([
            b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15],
        ]))
    }

    /// The order LiteDB keeps an index of Guids in: .NET's `Guid.CompareTo`, field by field, unsigned.
    pub fn net_cmp(&self, other: &Guid) -> std::cmp::Ordering {
        let key = |g: &Guid| {
            let b = g.0;
            (
                u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                u16::from_le_bytes([b[4], b[5]]),
                u16::from_le_bytes([b[6], b[7]]),
                [b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]],
            )
        };
        key(self).cmp(&key(other))
    }
}

impl std::fmt::Display for Guid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b = self.0;
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
        )
    }
}

impl std::fmt::Debug for Guid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Guid({self})")
    }
}

/// Reads one document from the front of `bytes`.
pub fn read(bytes: &[u8]) -> Result<Doc> {
    let mut reader = Reader { bytes, at: 0 };
    reader.document()
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self.at.checked_add(n).filter(|&end| end <= self.bytes.len()).context("a document runs short")?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("taken as many"))
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    fn cstring(&mut self) -> Result<String> {
        let rest = &self.bytes[self.at..];
        let end = rest.iter().position(|&b| b == 0).context("a name with no end")?;
        let text = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.at += end + 1;
        Ok(text)
    }

    /// A document or an array: its length, its elements, a 0.
    fn elements(&mut self) -> Result<Vec<(String, Value)>> {
        let start = self.at;
        let length = usize::try_from(self.i32()?).ok().filter(|&n| n >= 5).context("a document's length")?;
        let end = start.checked_add(length).filter(|&end| end <= self.bytes.len()).context("a document runs short")?;
        let mut fields = Vec::new();
        while self.at + 1 < end {
            let kind = self.take(1)?[0];
            let name = self.cstring()?;
            let value = self.value(kind)?;
            fields.push((name, value));
        }
        if self.take(1)? != [0] || self.at != end {
            bail!("a document does not end where its length says");
        }
        Ok(fields)
    }

    fn document(&mut self) -> Result<Doc> {
        Ok(Doc(self.elements()?))
    }

    fn value(&mut self, kind: u8) -> Result<Value> {
        Ok(match kind {
            0x01 => Value::Double(f64::from_le_bytes(self.array()?)),
            0x02 => {
                let length = usize::try_from(self.i32()?).ok().filter(|&n| n >= 1).context("a string's length")?;
                let bytes = self.take(length)?;
                Value::String(String::from_utf8_lossy(&bytes[..length - 1]).into_owned())
            }
            0x03 => Value::Document(self.document()?),
            0x04 => Value::Array(self.elements()?.into_iter().map(|(_, value)| value).collect()),
            0x05 => {
                let length = usize::try_from(self.i32()?).context("a binary's length")?;
                let subtype = self.take(1)?[0];
                let bytes = self.take(length)?.to_vec();
                match (subtype, <[u8; 16]>::try_from(bytes.as_slice())) {
                    (0x04, Ok(guid)) => Value::Guid(Guid(guid)),
                    _ => Value::Binary(subtype, bytes),
                }
            }
            0x07 => Value::ObjectId(self.array()?),
            0x08 => Value::Bool(self.take(1)?[0] != 0),
            0x09 => Value::DateTime(self.i64()?),
            0x0A => Value::Null,
            0x10 => Value::Int32(self.i32()?),
            0x12 => Value::Int64(self.i64()?),
            0x13 => Value::Decimal(self.array()?),
            0xFF => Value::MinKey,
            0x7F => Value::MaxKey,
            other => bail!("a BSON type LiteDB does not write: {other:#04x}"),
        })
    }
}

/// A document as LiteDB writes it.
pub fn write(doc: &Doc) -> Vec<u8> {
    let mut out = Vec::new();
    write_elements(&mut out, doc.0.iter().map(|(name, value)| (name.as_str(), value)));
    out
}

fn write_elements<'a>(out: &mut Vec<u8>, fields: impl Iterator<Item = (&'a str, &'a Value)>) {
    let start = out.len();
    out.extend([0; 4]);
    for (name, value) in fields {
        out.push(kind(value));
        out.extend(name.as_bytes());
        out.push(0);
        write_value(out, value);
    }
    out.push(0);
    let length = (out.len() - start) as i32;
    out[start..start + 4].copy_from_slice(&length.to_le_bytes());
}

fn kind(value: &Value) -> u8 {
    match value {
        Value::Double(_) => 0x01,
        Value::String(_) => 0x02,
        Value::Document(_) => 0x03,
        Value::Array(_) => 0x04,
        Value::Binary(..) | Value::Guid(_) => 0x05,
        Value::ObjectId(_) => 0x07,
        Value::Bool(_) => 0x08,
        Value::DateTime(_) => 0x09,
        Value::Null => 0x0A,
        Value::Int32(_) => 0x10,
        Value::Int64(_) => 0x12,
        Value::Decimal(_) => 0x13,
        Value::MinKey => 0xFF,
        Value::MaxKey => 0x7F,
    }
}

fn write_value(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Double(n) => out.extend(n.to_le_bytes()),
        Value::String(text) => {
            out.extend((text.len() as i32 + 1).to_le_bytes());
            out.extend(text.as_bytes());
            out.push(0);
        }
        Value::Document(doc) => write_elements(out, doc.0.iter().map(|(name, value)| (name.as_str(), value))),
        Value::Array(items) => {
            let names: Vec<String> = (0..items.len()).map(|i| i.to_string()).collect();
            write_elements(out, names.iter().map(String::as_str).zip(items));
        }
        Value::Binary(subtype, bytes) => {
            out.extend((bytes.len() as i32).to_le_bytes());
            out.push(*subtype);
            out.extend(bytes);
        }
        Value::Guid(guid) => {
            out.extend(16i32.to_le_bytes());
            out.push(0x04);
            out.extend(guid.0);
        }
        Value::ObjectId(id) => out.extend(id),
        Value::Bool(b) => out.push(u8::from(*b)),
        Value::DateTime(ms) => out.extend(ms.to_le_bytes()),
        Value::Null | Value::MinKey | Value::MaxKey => {}
        Value::Int32(n) => out.extend(n.to_le_bytes()),
        Value::Int64(n) => out.extend(n.to_le_bytes()),
        Value::Decimal(bytes) => out.extend(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guid_reads_as_dotnet_writes_it() {
        let steam = Guid::parse("CB91DFC9-B977-43BF-8E70-55F46E410FAB").unwrap();
        // Guid.ToByteArray: the first three fields little-endian.
        assert_eq!(steam.0[..8], [0xC9, 0xDF, 0x91, 0xCB, 0x77, 0xB9, 0xBF, 0x43]);
        assert_eq!(steam.to_string(), "cb91dfc9-b977-43bf-8e70-55f46e410fab");
        assert_eq!(Guid::parse("{cb91dfc9-b977-43bf-8e70-55f46e410fab}"), Some(steam));
        assert_eq!(Guid::parse("not a guid"), None);
        // .NET orders by the first field as a number, where the bytes would not.
        let low = Guid::parse("00000002-0000-0000-0000-000000000000").unwrap();
        let high = Guid::parse("00000100-0000-0000-0000-000000000000").unwrap();
        assert!(low.net_cmp(&high).is_lt() && low.0 > high.0);
    }

    #[test]
    fn a_document_goes_out_and_comes_back() {
        let doc = Doc::default()
            .with("_id", Value::Guid(Guid::parse("4a1b2c3d-5e6f-4789-9abc-def012345678").unwrap()))
            .with("Name", Value::String("Kestrel Bay".into()))
            .with("Playtime", Value::Int64(7260))
            .with("LastActivity", Value::DateTime(1_777_636_800_000))
            .with("Favorite", Value::Bool(true))
            .with("Ids", Value::Array(vec![Value::Int32(1), Value::Null, Value::Double(2.5)]))
            .with("Settings", Value::Document(Doc::default().with("Hidden", Value::Bool(false))));
        let bytes = write(&doc);
        assert_eq!(read(&bytes).unwrap(), doc);
        assert_eq!(i32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize, bytes.len());
        // Cut short, it is an error rather than a wrong answer.
        assert!(read(&bytes[..bytes.len() - 3]).is_err());
    }
}
