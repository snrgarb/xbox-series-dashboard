//! WAV files read into a [`Clip`], and a clip written as one.

/// A sound, decoded: stereo frames of samples from -1 to 1, at its file's rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// Frames a second.
    pub rate: u32,
    /// Left and right. A mono file's sample is in both; of more than two channels the first two are kept.
    pub frames: Vec<[f32; 2]>,
}

impl Clip {
    pub fn seconds(&self) -> f32 {
        self.frames.len() as f32 / self.rate.max(1) as f32
    }
}

const PCM: u16 = 1;
const FLOAT: u16 = 3;
/// `WAVE_FORMAT_EXTENSIBLE`: the real format is the first two bytes of the sub-format that follows.
const EXTENSIBLE: u16 = 0xFFFE;

struct Format {
    tag: u16,
    channels: usize,
    rate: u32,
    /// Bytes from one frame to the next.
    stride: usize,
    bits: u16,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn format(chunk: &[u8]) -> Result<Format, String> {
    let short = || "its format chunk is cut short".to_string();
    let mut tag = u16_at(chunk, 0).ok_or_else(short)?;
    let channels = u16_at(chunk, 2).ok_or_else(short)? as usize;
    let rate = u32_at(chunk, 4).ok_or_else(short)?;
    let align = u16_at(chunk, 12).ok_or_else(short)? as usize;
    let bits = u16_at(chunk, 14).ok_or_else(short)?;
    if tag == EXTENSIBLE {
        tag = u16_at(chunk, 24).ok_or_else(short)?;
    }
    if channels == 0 || rate == 0 {
        return Err(format!("{channels} channels at {rate} Hz"));
    }
    if !matches!((tag, bits), (PCM, 8 | 16 | 24 | 32) | (FLOAT, 32 | 64)) {
        return Err(format!("format {tag:#x} at {bits} bits is not PCM or float this reads"));
    }
    let bytes = bits as usize / 8;
    // A block alignment that makes no sense is written by some tools: the samples say how far apart frames are.
    let stride = if align >= channels * bytes { align } else { channels * bytes };
    Ok(Format { tag, channels, rate, stride, bits })
}

/// One sample, from -1 to 1.
fn sample(format: &Format, bytes: &[u8]) -> f32 {
    match (format.tag, format.bits) {
        // Eight bits are unsigned, silence at 128.
        (PCM, 8) => (bytes[0] as f32 - 128.0) / 128.0,
        (PCM, 16) => i16::from_le_bytes([bytes[0], bytes[1]]) as f32 / 32_768.0,
        (PCM, 24) => (i32::from_le_bytes([0, bytes[0], bytes[1], bytes[2]]) >> 8) as f32 / 8_388_608.0,
        (PCM, _) => i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32 / 2_147_483_648.0,
        (_, 32) => f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        _ => f64::from_le_bytes(bytes[..8].try_into().unwrap()) as f32,
    }
}

/// Reads a WAV file's bytes: PCM of 8, 16, 24 or 32 bits or float of 32 or 64, any rate, any number of channels.
pub fn decode(bytes: &[u8]) -> Result<Clip, String> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let mut found = None;
    let mut at = 12usize;
    while at.saturating_add(8) <= bytes.len() {
        let (id, size) = (&bytes[at..at + 4], u32_at(bytes, at + 4).unwrap_or(0));
        // A size past the end of the file is a recording that was never closed: what is there is taken.
        let body = &bytes[at + 8..(at + 8).saturating_add(size as usize).min(bytes.len())];
        match id {
            b"fmt " => found = Some(format(body)?),
            b"data" => {
                let format: Format = found.ok_or("its samples come before its format")?;
                let width = format.bits as usize / 8;
                let frames = body.chunks_exact(format.stride).map(|frame| {
                    let left = sample(&format, frame);
                    let right = if format.channels > 1 { sample(&format, &frame[width..]) } else { left };
                    [left, right]
                });
                return Ok(Clip { rate: format.rate, frames: frames.collect() });
            }
            _ => {}
        }
        // Chunks start on even bytes.
        at = (at + 8).saturating_add(size as usize).saturating_add(size as usize & 1);
    }
    Err("it has no samples".into())
}

/// The clip as a WAV file's bytes: 16-bit stereo PCM.
pub fn encode(clip: &Clip) -> Vec<u8> {
    let data = clip.frames.len() as u32 * 4;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend(b"RIFF");
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(PCM.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(clip.rate.to_le_bytes());
    out.extend((clip.rate * 4).to_le_bytes());
    out.extend(4u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    for sample in clip.frames.iter().flatten() {
        out.extend(crate::mix::to_i16(*sample).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WAV file of `tag` and `bits` holding `samples`, which are already interleaved by channel.
    fn file(tag: u16, channels: u16, rate: u32, bits: u16, samples: &[u8], extensible: bool) -> Vec<u8> {
        let align = channels * bits / 8;
        let mut fmt = Vec::new();
        fmt.extend((if extensible { EXTENSIBLE } else { tag }).to_le_bytes());
        fmt.extend(channels.to_le_bytes());
        fmt.extend(rate.to_le_bytes());
        fmt.extend((rate * align as u32).to_le_bytes());
        fmt.extend(align.to_le_bytes());
        fmt.extend(bits.to_le_bytes());
        if extensible {
            // The extension's size, the valid bits, the speakers, then the sub-format, which starts with the tag.
            fmt.extend(22u16.to_le_bytes());
            fmt.extend(bits.to_le_bytes());
            fmt.extend(3u32.to_le_bytes());
            fmt.extend(tag.to_le_bytes());
            fmt.extend([0; 14]);
        }
        let mut out = b"RIFF\0\0\0\0WAVE".to_vec();
        // A chunk of an odd length that is nothing to do with the sound, as a tagging tool leaves.
        out.extend(b"LIST");
        out.extend(3u32.to_le_bytes());
        out.extend(b"abc\0");
        out.extend(b"fmt ");
        out.extend((fmt.len() as u32).to_le_bytes());
        out.extend(fmt);
        out.extend(b"data");
        out.extend((samples.len() as u32).to_le_bytes());
        out.extend(samples);
        out
    }

    fn close(clip: &Clip, expected: &[[f32; 2]]) {
        assert_eq!(clip.frames.len(), expected.len());
        for (got, want) in clip.frames.iter().zip(expected) {
            assert!((got[0] - want[0]).abs() < 1e-4 && (got[1] - want[1]).abs() < 1e-4, "{got:?} for {want:?}");
        }
    }

    #[test]
    fn sixteen_bit_stereo() {
        let samples: Vec<u8> = [16_384i16, -16_384, 0, 32_767].iter().flat_map(|s| s.to_le_bytes()).collect();
        let clip = decode(&file(PCM, 2, 44_100, 16, &samples, false)).unwrap();
        assert_eq!(clip.rate, 44_100);
        close(&clip, &[[0.5, -0.5], [0.0, 1.0]]);
    }

    #[test]
    fn eight_bit_mono_is_unsigned_and_goes_to_both_sides() {
        let clip = decode(&file(PCM, 1, 8_000, 8, &[128, 192, 0], false)).unwrap();
        assert_eq!(clip.rate, 8_000);
        close(&clip, &[[0.0, 0.0], [0.5, 0.5], [-1.0, -1.0]]);
    }

    #[test]
    fn twenty_four_bit_keeps_its_sign() {
        let samples = [0x00, 0x00, 0x40, 0x00, 0x00, 0xC0];
        let clip = decode(&file(PCM, 2, 48_000, 24, &samples, false)).unwrap();
        close(&clip, &[[0.5, -0.5]]);
    }

    #[test]
    fn float_and_the_extensible_header() {
        let samples: Vec<u8> = [0.25f32, -0.75].iter().flat_map(|s| s.to_le_bytes()).collect();
        for extensible in [false, true] {
            let clip = decode(&file(FLOAT, 2, 96_000, 32, &samples, extensible)).unwrap();
            assert_eq!(clip.rate, 96_000);
            close(&clip, &[[0.25, -0.75]]);
        }
    }

    #[test]
    fn of_many_channels_the_first_two_are_kept() {
        let samples: Vec<u8> = [8_192i16, -8_192, 1, 2, 3, 4].iter().flat_map(|s| s.to_le_bytes()).collect();
        let clip = decode(&file(PCM, 6, 48_000, 16, &samples, true)).unwrap();
        close(&clip, &[[0.25, -0.25]]);
    }

    #[test]
    fn a_data_chunk_longer_than_the_file_gives_what_is_there() {
        let samples: Vec<u8> = [100i16, 100, 200, 200].iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut bytes = file(PCM, 2, 44_100, 16, &samples, false);
        let size = bytes.len() - samples.len() - 4;
        bytes[size..size + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode(&bytes).unwrap().frames.len(), 2);
    }

    #[test]
    fn what_is_not_a_wav_file_is_refused_not_a_panic() {
        assert!(decode(b"").is_err());
        assert!(decode(b"RIFF\0\0\0\0WAVE").is_err());
        assert!(decode(b"OggS and so on, for a while").is_err());
        // A compressed format (ADPCM), and one cut off inside its format chunk.
        assert!(decode(&file(2, 2, 44_100, 4, &[0; 8], false)).is_err());
        let whole = file(PCM, 2, 44_100, 16, &[0; 8], false);
        for length in 0..whole.len() {
            let _ = decode(&whole[..length]);
        }
    }

    #[test]
    fn a_clip_written_reads_back() {
        let clip = Clip { rate: 22_050, frames: vec![[0.5, -0.5], [0.0, 0.25], [-1.0, 1.0]] };
        let back = decode(&encode(&clip)).unwrap();
        assert_eq!(back.rate, 22_050);
        close(&back, &clip.frames);
    }
}
