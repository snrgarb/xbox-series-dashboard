//! `--make-clip`: writes the test clip with Media Foundation's Sink Writer, so no video file need be committed or
//! fetched. H.264 and AAC in an `.mp4`; WMV3 and WMA in a `.wmv`, for a Windows without the H.264 encoder.
use windows::core::{Interface, GUID, PCWSTR};
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFMediaType, IMFSample, IMFSinkWriter, MFAudioFormat_AAC, MFAudioFormat_PCM,
    MFAudioFormat_WMAudioV8, MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFMediaType_Audio, MFMediaType_Video, MFNominalRange_16_235,
    MFTranscodeGetAudioOutputAvailableTypes, MFVideoFormat_H264, MFVideoFormat_NV12, MFVideoFormat_WMV3,
    MFVideoInterlace_Progressive, MFVideoTransferMatrix_BT709, MFT_ENUM_FLAG_ALL, MF_MT_ALL_SAMPLES_INDEPENDENT,
    MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE, MF_MT_VIDEO_NOMINAL_RANGE,
    MF_MT_YUV_MATRIX, MF_SINK_WRITER_DISABLE_THROTTLING,
};

use super::{set_pair, subtype_name, wide, MediaFoundation, Step};
use crate::pattern::{draw_frame, frame_len, tone};
use crate::{log, MakeClip};

const FPS: u32 = 30;
const CHANNELS: u16 = 2;
/// The AAC encoder's rates are 44100 and 48000; the WMA encoder is asked which of its formats has this one.
const RATE: u32 = 48_000;
/// 160 kbit/s, one of the four bit rates the AAC encoder takes.
const AAC_BYTES_PER_SECOND: u32 = 20_000;

pub fn run(args: &MakeClip) -> Result<(), String> {
    let wmv = args.out.extension().is_some_and(|e| e.eq_ignore_ascii_case("wmv"));
    let codecs =
        if wmv { (MFVideoFormat_WMV3, MFAudioFormat_WMAudioV8) } else { (MFVideoFormat_H264, MFAudioFormat_AAC) };
    write(args, codecs).map_err(|e| {
        let hint = if wmv {
            ""
        } else {
            " (if this Windows has no H.264 or AAC encoder, as an N edition without the Media Feature Pack hasn't, \
             name the clip .wmv to write WMV3 and WMA instead)"
        };
        format!("make-clip: {e}{hint}")
    })
}

fn write(args: &MakeClip, (video_codec, audio_codec): (GUID, GUID)) -> Result<(), String> {
    let _media_foundation = MediaFoundation::start()?;
    let (width, height) = args.size;
    let frames = args.seconds * FPS;
    unsafe {
        let mut attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attributes, 1).step("MFCreateAttributes")?;
        let attributes = attributes.ok_or("MFCreateAttributes gave nothing")?;
        // Written as fast as the encoders go, not at the clip's own pace.
        attributes.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1).step("SetUINT32")?;
        let out = std::path::absolute(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
        let url = wide(&out);
        let writer: IMFSinkWriter = MFCreateSinkWriterFromURL(PCWSTR(url.as_ptr()), None, &attributes)
            .step(&format!("creating {}", args.out.display()))?;

        // The picture. NV12 in, which both encoders take as it is and which, unlike RGB, has no bottom-up form.
        let video_name = subtype_name(&video_codec);
        let encoded = video_type(&video_codec, width, height)?;
        encoded.SetUINT32(&MF_MT_AVG_BITRATE, (width * height * 4).max(1_000_000)).step("SetUINT32")?;
        let video = writer.AddStream(&encoded).step(&format!("adding the {video_name} video stream"))?;
        let raw = video_type(&MFVideoFormat_NV12, width, height)?;
        raw.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32).step("SetUINT32")?;
        raw.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32).step("SetUINT32")?;
        writer.SetInputMediaType(video, &raw, None).step(&format!("no {video_name} encoder taking NV12"))?;

        // The sound.
        let audio = if args.no_audio { None } else { Some(add_audio(&writer, &audio_codec)?) };

        writer.BeginWriting().step("IMFSinkWriter::BeginWriting")?;
        let time = |index: u64, per_second: u32| (index * 10_000_000 / per_second as u64) as i64;
        let mut samples = Vec::new();
        for frame in 0..frames as u64 {
            if let Some(audio) = audio {
                // This frame's share of the sound, written first so the file's two streams stay interleaved.
                let (from, to) = (frame * RATE as u64 / FPS as u64, (frame + 1) * RATE as u64 / FPS as u64);
                samples.clear();
                tone(from, (to - from) as usize, RATE, CHANNELS, &mut samples);
                let (start, end) = (time(from, RATE), time(to, RATE));
                let sample = sample_of(samples.len() * 2, start, end - start, |bytes| {
                    for (out, sample) in bytes.as_chunks_mut::<2>().0.iter_mut().zip(&samples) {
                        *out = sample.to_le_bytes();
                    }
                })?;
                writer.WriteSample(audio, &sample).step("writing sound")?;
            }
            let (start, end) = (time(frame, FPS), time(frame + 1, FPS));
            let sample = sample_of(frame_len(width, height), start, end - start, |pixels| {
                draw_frame(width, height, frame as u32, FPS, pixels);
            })?;
            writer.WriteSample(video, &sample).step("writing a frame")?;
        }
        writer.Finalize().step("IMFSinkWriter::Finalize")?;
    }
    let sound =
        if args.no_audio { "no sound".to_string() } else { format!("{} {RATE} Hz", subtype_name(&audio_codec)) };
    log!(
        "make-clip: wrote {} ({width}x{height}, {frames} frames at {FPS} fps, {}, {sound})",
        args.out.display(),
        subtype_name(&video_codec)
    );
    Ok(())
}

/// A progressive video type of square pixels at the clip's frame rate.
fn video_type(subtype: &GUID, width: u32, height: u32) -> Result<IMFMediaType, String> {
    unsafe {
        let media_type = MFCreateMediaType().step("MFCreateMediaType")?;
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).step("SetGUID")?;
        media_type.SetGUID(&MF_MT_SUBTYPE, subtype).step("SetGUID")?;
        media_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).step("SetUINT32")?;
        set_pair(&media_type, &MF_MT_FRAME_SIZE, width, height).step("SetUINT64")?;
        set_pair(&media_type, &MF_MT_FRAME_RATE, FPS, 1).step("SetUINT64")?;
        set_pair(&media_type, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1).step("SetUINT64")?;
        Ok(media_type)
    }
}

/// Adds the sound's stream, encoded as `codec` from 16-bit PCM. Returns the stream's index.
fn add_audio(writer: &IMFSinkWriter, codec: &GUID) -> Result<u32, String> {
    let name = subtype_name(codec);
    unsafe {
        let encoded = if *codec == MFAudioFormat_AAC {
            let encoded = audio_type(codec)?;
            encoded.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, AAC_BYTES_PER_SECOND).step("SetUINT32")?;
            encoded
        } else {
            // A WMA type carries codec data only the encoder can fill in: take one of the types it offers.
            let offered = MFTranscodeGetAudioOutputAvailableTypes(codec, MFT_ENUM_FLAG_ALL.0 as u32, None)
                .step(&format!("no {name} encoder"))?;
            let count = offered.GetElementCount().step("IMFCollection::GetElementCount")?;
            let fits = |candidate: &IMFMediaType| {
                candidate.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).ok() == Some(RATE)
                    && candidate.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).ok() == Some(CHANNELS as u32)
                    && candidate.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE).ok() == Some(16)
            };
            (0..count)
                .filter_map(|index| offered.GetElement(index).ok()?.cast::<IMFMediaType>().ok())
                .filter(fits)
                // The best the encoder offers at this rate.
                .max_by_key(|candidate| candidate.GetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND).unwrap_or(0))
                .ok_or(format!("the {name} encoder offers no {RATE} Hz stereo 16-bit format"))?
        };
        let stream = writer.AddStream(&encoded).step(&format!("adding the {name} audio stream"))?;
        let raw = audio_type(&MFAudioFormat_PCM)?;
        raw.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, CHANNELS as u32 * 2).step("SetUINT32")?;
        raw.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, RATE * CHANNELS as u32 * 2).step("SetUINT32")?;
        raw.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1).step("SetUINT32")?;
        writer.SetInputMediaType(stream, &raw, None).step(&format!("no {name} encoder taking PCM"))?;
        Ok(stream)
    }
}

/// A 16-bit stereo audio type at the clip's rate.
fn audio_type(subtype: &GUID) -> Result<IMFMediaType, String> {
    unsafe {
        let media_type = MFCreateMediaType().step("MFCreateMediaType")?;
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).step("SetGUID")?;
        media_type.SetGUID(&MF_MT_SUBTYPE, subtype).step("SetGUID")?;
        media_type.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).step("SetUINT32")?;
        media_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, RATE).step("SetUINT32")?;
        media_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, CHANNELS as u32).step("SetUINT32")?;
        Ok(media_type)
    }
}

/// A sample of `len` bytes, filled by `fill`, at `time` for `duration` (both in 100 ns units).
fn sample_of(len: usize, time: i64, duration: i64, fill: impl FnOnce(&mut [u8])) -> Result<IMFSample, String> {
    unsafe {
        let buffer = MFCreateMemoryBuffer(len as u32).step("MFCreateMemoryBuffer")?;
        let mut data = std::ptr::null_mut();
        buffer.Lock(&mut data, None, None).step("IMFMediaBuffer::Lock")?;
        // SAFETY: the buffer was made `len` bytes long and is locked until Unlock.
        fill(std::slice::from_raw_parts_mut(data, len));
        buffer.Unlock().step("IMFMediaBuffer::Unlock")?;
        buffer.SetCurrentLength(len as u32).step("IMFMediaBuffer::SetCurrentLength")?;
        let sample = MFCreateSample().step("MFCreateSample")?;
        sample.AddBuffer(&buffer).step("IMFSample::AddBuffer")?;
        sample.SetSampleTime(time).step("IMFSample::SetSampleTime")?;
        sample.SetSampleDuration(duration).step("IMFSample::SetSampleDuration")?;
        Ok(sample)
    }
}
