//! Discord's IPC frames: an opcode and a length, each a little-endian i32, then that many bytes of UTF-8 JSON
//! (docs/research/discord-rpc.md in the Bundle; the same on every OS, only the pipe's path differs).

/// A frame's opcode, its header's first little-endian i32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Handshake = 0,
    Frame = 1,
    Close = 2,
    Ping = 3,
    Pong = 4,
}

impl Opcode {
    pub fn from_i32(value: i32) -> Option<Opcode> {
        Some(match value {
            0 => Opcode::Handshake,
            1 => Opcode::Frame,
            2 => Opcode::Close,
            3 => Opcode::Ping,
            4 => Opcode::Pong,
            _ => return None,
        })
    }
}

/// A frame longer than this means the stream is out of step, not that Discord sent a huge payload.
pub const MAX_PAYLOAD: usize = 16 * 1024 * 1024;

/// One frame's bytes: the header, then `json` as UTF-8.
pub fn encode(opcode: Opcode, json: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + json.len());
    bytes.extend_from_slice(&(opcode as i32).to_le_bytes());
    bytes.extend_from_slice(&(json.len() as i32).to_le_bytes());
    bytes.extend_from_slice(json.as_bytes());
    bytes
}

/// What a stream of bytes held, frame by frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    Frame(Opcode, String),
    /// The stream is out of step (an unknown opcode, a length out of bounds): the connection is dropped.
    Broken(String),
}

/// Gathers bytes as they come, split across reads or several frames to a read, and gives back whole frames.
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}

impl Decoder {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole frame, if one has come.
    pub fn take_frame(&mut self) -> Option<Decoded> {
        if self.buffer.len() < 8 {
            return None;
        }
        let word = |at: usize| i32::from_le_bytes(self.buffer[at..at + 4].try_into().expect("four bytes"));
        let (opcode, length) = (word(0), word(4));
        let Some(opcode) = Opcode::from_i32(opcode) else {
            return Some(Decoded::Broken(format!("a frame with opcode {opcode}")));
        };
        if length < 0 || length as usize > MAX_PAYLOAD {
            return Some(Decoded::Broken(format!("a {opcode:?} frame {length} bytes long")));
        }
        let end = 8 + length as usize;
        if self.buffer.len() < end {
            return None;
        }
        let json = String::from_utf8_lossy(&self.buffer[8..end]).into_owned();
        self.buffer.drain(..end);
        Some(Decoded::Frame(opcode, json))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_split_and_merged_come_out_whole() {
        let mut bytes = encode(Opcode::Frame, "{\"a\":1}");
        bytes.extend(encode(Opcode::Ping, "{}"));
        let mut decoder = Decoder::default();
        decoder.push(&bytes[..5]);
        assert_eq!(decoder.take_frame(), None);
        decoder.push(&bytes[5..12]);
        assert_eq!(decoder.take_frame(), None);
        decoder.push(&bytes[12..]);
        assert_eq!(decoder.take_frame(), Some(Decoded::Frame(Opcode::Frame, "{\"a\":1}".into())));
        assert_eq!(decoder.take_frame(), Some(Decoded::Frame(Opcode::Ping, "{}".into())));
        assert_eq!(decoder.take_frame(), None);
    }

    #[test]
    fn a_header_out_of_step_breaks_the_stream() {
        let mut decoder = Decoder::default();
        decoder.push(&[9, 0, 0, 0, 0, 0, 0, 0]);
        assert!(matches!(decoder.take_frame(), Some(Decoded::Broken(_))));
        let mut decoder = Decoder::default();
        decoder.push(&[1, 0, 0, 0, 0xff, 0xff, 0xff, 0x7f]);
        assert!(matches!(decoder.take_frame(), Some(Decoded::Broken(_))));
    }

    #[test]
    fn the_header_is_little_endian() {
        assert_eq!(&encode(Opcode::Handshake, "ab")[..8], &[0, 0, 0, 0, 2, 0, 0, 0]);
    }
}
