//! Valve KeyValues ("VDF"), text and binary, parsed into one small tree.
use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Leaf. Binary numbers are stored as their decimal text.
    Str(String),
    Map(Vec<(String, Value)>),
}

impl Value {
    /// Child by key, ASCII case-insensitive (Steam is inconsistent about key case).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Value::Str(_) => None,
        }
    }

    pub fn path(&self, keys: &[&str]) -> Option<&Value> {
        keys.iter().try_fold(self, |v, k| v.get(k))
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Map(_) => None,
        }
    }

    pub fn str_at(&self, keys: &[&str]) -> Option<&str> {
        self.path(keys)?.as_str()
    }

    pub fn u64_at(&self, keys: &[&str]) -> Option<u64> {
        self.str_at(keys)?.trim().parse().ok()
    }

    pub fn entries(&self) -> &[(String, Value)] {
        match self {
            Value::Map(m) => m,
            Value::Str(_) => &[],
        }
    }
}

// ---------------------------------------------------------------- text

enum Tok {
    Str(String),
    Open,
    Close,
}

fn tokens(src: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut it = src.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            c if c.is_whitespace() || c == '\u{feff}' => {}
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '/' if it.peek() == Some(&'/') => {
                for c in it.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '"' => {
                let mut s = String::new();
                while let Some(c) = it.next() {
                    match c {
                        '"' => break,
                        '\\' => match it.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => {}
                        },
                        c => s.push(c),
                    }
                }
                out.push(Tok::Str(s));
            }
            c => {
                let mut s = String::from(c);
                while let Some(&n) = it.peek() {
                    if n.is_whitespace() || n == '{' || n == '}' || n == '"' {
                        break;
                    }
                    s.push(n);
                    it.next();
                }
                out.push(Tok::Str(s));
            }
        }
    }
    out
}

/// Parse a text VDF file. The result is a map holding the root key(s).
pub fn parse_text(src: &str) -> Result<Value> {
    let toks = tokens(src);
    let mut pos = 0;
    let v = text_map(&toks, &mut pos, true)?;
    Ok(v)
}

fn text_map(toks: &[Tok], pos: &mut usize, top: bool) -> Result<Value> {
    let mut out = Vec::new();
    loop {
        let key = match toks.get(*pos) {
            None if top => return Ok(Value::Map(out)),
            None => bail!("text vdf: unexpected end of input"),
            Some(Tok::Close) if !top => {
                *pos += 1;
                return Ok(Value::Map(out));
            }
            Some(Tok::Str(s)) => s.clone(),
            Some(_) => bail!("text vdf: unexpected brace at token {pos}"),
        };
        *pos += 1;
        let val = match toks.get(*pos) {
            Some(Tok::Str(s)) => {
                *pos += 1;
                Value::Str(s.clone())
            }
            Some(Tok::Open) => {
                *pos += 1;
                text_map(toks, pos, false)?
            }
            _ => bail!("text vdf: key {key:?} has no value"),
        };
        out.push((key, val));
    }
}

// -------------------------------------------------------------- binary

pub struct Reader<'a> {
    pub buf: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.buf.len());
        let end = end.with_context(|| format!("binary vdf: truncated at offset {}", self.pos))?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    pub fn cstr(&mut self) -> Result<String> {
        let rest = &self.buf[self.pos.min(self.buf.len())..];
        let len = rest.iter().position(|&b| b == 0).context("binary vdf: unterminated string")?;
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.pos += len + 1;
        Ok(s)
    }
}

/// Parse one binary KeyValues map body (up to and including its end marker).
/// With `strings`, keys are u32 indexes into that table (appinfo v29+);
/// without, keys are inline NUL-terminated strings.
pub fn parse_binary(r: &mut Reader, strings: Option<&[String]>) -> Result<Value> {
    binary_map(r, strings, 0, false)
}

/// Parse a whole file of binary KeyValues with inline keys, as `appcache/stats/*.bin` are: its top level ends at
/// its end marker or at the end of the file, whichever comes first, as some writers leave the last marker off.
pub fn parse_binary_file(buf: &[u8]) -> Result<Value> {
    binary_map(&mut Reader::new(buf), None, 0, true)
}

fn binary_map(r: &mut Reader, strings: Option<&[String]>, depth: u32, top: bool) -> Result<Value> {
    if depth > 64 {
        bail!("binary vdf: nesting too deep");
    }
    let mut out = Vec::new();
    loop {
        if top && r.pos >= r.buf.len() {
            return Ok(Value::Map(out));
        }
        let ty = r.u8()?;
        if ty == 8 || ty == 11 {
            return Ok(Value::Map(out));
        }
        let key = match strings {
            Some(table) => {
                let i = r.u32()? as usize;
                table.get(i).with_context(|| format!("binary vdf: bad key index {i}"))?.clone()
            }
            None => r.cstr()?,
        };
        let val = match ty {
            0 => binary_map(r, strings, depth + 1, false)?,
            1 => Value::Str(r.cstr()?),
            2 => Value::Str((r.u32()? as i32).to_string()),
            3 => Value::Str(f32::from_le_bytes(r.bytes(4)?.try_into().unwrap()).to_string()),
            4 | 6 => Value::Str(r.u32()?.to_string()),
            5 => {
                // wide string: UTF-16LE, terminated by a 0 code unit
                let mut units = Vec::new();
                loop {
                    let b = r.bytes(2)?;
                    let u = u16::from_le_bytes([b[0], b[1]]);
                    if u == 0 {
                        break;
                    }
                    units.push(u);
                }
                Value::Str(String::from_utf16_lossy(&units))
            }
            7 => Value::Str(r.u64()?.to_string()),
            10 => Value::Str((r.u64()? as i64).to_string()),
            other => bail!("binary vdf: unknown type {other} at offset {}", r.pos - 1),
        };
        out.push((key, val));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_nested_escapes_and_comments() {
        let src = "\u{feff}// comment\n\"Root\"\n{\n\t\"path\"\t\t\"c:\\\\games\\\\steam\"\n\t\"Apps\"\n\t{\n\t\t\"10\"\t{ \"LastPlayed\" \"123\" }\n\t\tbare value\n\t}\n\t\"q\" \"say \\\"hi\\\"\"\n}\n";
        let v = parse_text(src).unwrap();
        assert_eq!(v.str_at(&["root", "path"]), Some("c:\\games\\steam"));
        assert_eq!(v.u64_at(&["Root", "apps", "10", "lastplayed"]), Some(123));
        assert_eq!(v.str_at(&["Root", "Apps", "bare"]), Some("value"));
        assert_eq!(v.str_at(&["Root", "q"]), Some("say \"hi\""));
        assert_eq!(v.path(&["Root", "Apps"]).unwrap().entries().len(), 2);
    }

    #[test]
    fn text_rejects_unbalanced() {
        assert!(parse_text("\"a\" { \"b\" \"c\"").is_err());
        assert!(parse_text("\"a\"").is_err());
    }

    #[test]
    fn binary_inline_keys() {
        let mut b = Vec::new();
        b.extend(b"\x00common\0");
        b.extend(b"\x01name\0Half-Life\0");
        b.extend(b"\x02n\0");
        b.extend((-5i32).to_le_bytes());
        b.extend(b"\x07big\0");
        b.extend(7_000_000_000u64.to_le_bytes());
        b.extend(b"\x08\x08");
        let mut r = Reader::new(&b);
        let v = parse_binary(&mut r, None).unwrap();
        assert_eq!(r.pos, b.len());
        assert_eq!(v.str_at(&["common", "name"]), Some("Half-Life"));
        assert_eq!(v.str_at(&["common", "n"]), Some("-5"));
        assert_eq!(v.u64_at(&["common", "big"]), Some(7_000_000_000));
    }

    #[test]
    fn binary_string_table_keys_and_truncation() {
        let table = vec!["type".to_string(), "name".to_string()];
        let mut b = Vec::new();
        b.push(1);
        b.extend(1u32.to_le_bytes());
        b.extend(b"Portal\0");
        b.push(1);
        b.extend(0u32.to_le_bytes());
        b.extend(b"Game\0");
        b.push(8);
        let v = parse_binary(&mut Reader::new(&b), Some(&table)).unwrap();
        assert_eq!(v.str_at(&["name"]), Some("Portal"));
        assert_eq!(v.str_at(&["type"]), Some("Game"));
        assert!(parse_binary(&mut Reader::new(&b[..b.len() - 1]), Some(&table)).is_err());
        assert!(parse_binary(&mut Reader::new(&[1, 9, 0, 0, 0, b'x', 0, 8]), Some(&table)).is_err());
    }

    #[test]
    fn a_binary_file_ends_at_its_marker_or_at_its_end() {
        let with = b"\x00620\0\x01gamename\0Made Up\0\x08\x08";
        let without = &with[..with.len() - 1];
        for buf in [&with[..], without] {
            assert_eq!(parse_binary_file(buf).unwrap().str_at(&["620", "gamename"]), Some("Made Up"));
        }
        // But a map left open inside it is not a file.
        assert!(parse_binary_file(&with[..with.len() - 2]).is_err());
    }
}
