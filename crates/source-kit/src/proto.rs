//! Protobuf's wire format, as much of it as Ubisoft's cache and Battle.net's `product.db` need: a message read as
//! its fields, in order, each a number and a value, with no schema; and a writer for the fixtures. A message nested
//! in another is a field of bytes, read again as a message.
use anyhow::{bail, Context, Result};

/// One field's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value<'a> {
    /// A varint: an integer, a bool, an enum.
    Varint(u64),
    /// Eight bytes: a fixed64, a double.
    Fixed64(u64),
    /// Length-delimited: a string, bytes, a message, a packed list.
    Bytes(&'a [u8]),
    /// Four bytes: a fixed32, a float.
    Fixed32(u32),
}

impl<'a> Value<'a> {
    pub fn varint(self) -> Option<u64> {
        if let Value::Varint(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn bytes(self) -> Option<&'a [u8]> {
        if let Value::Bytes(b) = self {
            Some(b)
        } else {
            None
        }
    }

    /// The bytes as text, when they are UTF-8.
    pub fn text(self) -> Option<&'a str> {
        std::str::from_utf8(self.bytes()?).ok()
    }
}

/// A message's fields, each its number and value, in the order they come.
pub fn fields(mut buf: &[u8]) -> Result<Vec<(u32, Value<'_>)>> {
    let mut out = Vec::new();
    while !buf.is_empty() {
        let key = varint(&mut buf).context("a field's key")?;
        let number = u32::try_from(key >> 3).ok().filter(|&n| n > 0).context("a field numbered 0")?;
        let value = match key & 7 {
            0 => Value::Varint(varint(&mut buf).with_context(|| format!("field {number}'s varint"))?),
            1 => Value::Fixed64(u64::from_le_bytes(take(&mut buf, 8)?.try_into()?)),
            2 => {
                let len = varint(&mut buf).with_context(|| format!("field {number}'s length"))?;
                Value::Bytes(take(&mut buf, usize::try_from(len)?).with_context(|| format!("field {number}"))?)
            }
            5 => Value::Fixed32(u32::from_le_bytes(take(&mut buf, 4)?.try_into()?)),
            wire => bail!("field {number} has wire type {wire}, which is not read"),
        };
        out.push((number, value));
    }
    Ok(out)
}

/// The first field numbered `number`, if there is one.
pub fn first<'a>(fields: &[(u32, Value<'a>)], number: u32) -> Option<Value<'a>> {
    fields.iter().find(|(n, _)| *n == number).map(|(_, v)| *v)
}

/// Every field numbered `number`, in order: a repeated one.
pub fn all<'a, 'b>(fields: &'b [(u32, Value<'a>)], number: u32) -> impl Iterator<Item = Value<'a>> + 'b {
    fields.iter().filter(move |(n, _)| *n == number).map(|(_, v)| *v)
}

fn varint(buf: &mut &[u8]) -> Result<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = buf.split_first().context("the message ends inside a varint")?;
        *buf = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    bail!("a varint longer than ten bytes")
}

fn take<'a>(buf: &mut &'a [u8], len: usize) -> Result<&'a [u8]> {
    if buf.len() < len {
        bail!("the message ends {} bytes short", len - buf.len());
    }
    let (taken, rest) = buf.split_at(len);
    *buf = rest;
    Ok(taken)
}

/// A message written field by field, for a fixture.
#[derive(Debug, Default, Clone)]
pub struct Writer(Vec<u8>);

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn varint(mut self, number: u32, value: u64) -> Self {
        self.key(number, 0);
        self.raw_varint(value);
        self
    }

    pub fn bytes(mut self, number: u32, value: &[u8]) -> Self {
        self.key(number, 2);
        self.raw_varint(value.len() as u64);
        self.0.extend_from_slice(value);
        self
    }

    pub fn text(self, number: u32, value: &str) -> Self {
        self.bytes(number, value.as_bytes())
    }

    pub fn message(self, number: u32, value: Writer) -> Self {
        self.bytes(number, &value.0)
    }

    pub fn finish(self) -> Vec<u8> {
        self.0
    }

    fn key(&mut self, number: u32, wire: u64) {
        self.raw_varint(u64::from(number) << 3 | wire);
    }

    fn raw_varint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.0.push(value as u8 | 0x80);
            value >>= 7;
        }
        self.0.push(value as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_reads_back_field_by_field() {
        let inner = Writer::new().text(1, "D:/Battle.net/Emberwatch");
        let bytes = Writer::new().varint(1, 90210).varint(2, 300).message(3, inner).text(4, "émber").finish();
        let read = fields(&bytes).unwrap();
        assert_eq!(first(&read, 1).and_then(Value::varint), Some(90210));
        assert_eq!(first(&read, 2).and_then(Value::varint), Some(300));
        let inner = fields(first(&read, 3).and_then(Value::bytes).unwrap()).unwrap();
        assert_eq!(first(&inner, 1).and_then(Value::text), Some("D:/Battle.net/Emberwatch"));
        assert_eq!(first(&read, 4).and_then(Value::text), Some("émber"));
        assert_eq!(first(&read, 9), None);
        let repeated = Writer::new().text(1, "a").varint(2, 1).text(1, "b").finish();
        let repeated = fields(&repeated).unwrap();
        assert_eq!(all(&repeated, 1).filter_map(Value::text).collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn a_message_cut_short_or_of_an_unknown_wire_type_is_an_error() {
        let bytes = Writer::new().text(1, "Harborlight").finish();
        assert!(fields(&bytes[..bytes.len() - 2]).is_err());
        assert!(fields(&[0x0b]).is_err()); // field 1, wire type 3 (a group)
        assert!(fields(&[0x08, 0x80]).is_err()); // a varint with no end
        assert!(fields(&[0x00, 0x01]).is_err()); // field 0
        assert_eq!(fields(&[]).unwrap(), []);
    }
}
