//! Clips brought to the device's rate, and the ones playing summed into what the device is given.
use std::sync::Arc;

use crate::wav::Clip;

/// A clip's frames at the device's rate, shared by every playing of it.
pub type Frames = Arc<[[f32; 2]]>;

/// The clip at `rate` frames a second, each new frame a straight line between the two it falls between.
pub fn resample(clip: &Clip, rate: u32) -> Vec<[f32; 2]> {
    if clip.rate == rate || clip.frames.is_empty() || clip.rate == 0 || rate == 0 {
        return clip.frames.clone();
    }
    let last = clip.frames.len() - 1;
    let count = (clip.frames.len() as u64 * rate as u64).div_ceil(clip.rate as u64) as usize;
    let step = clip.rate as f64 / rate as f64;
    (0..count)
        .map(|i| {
            let at = i as f64 * step;
            let before = (at as usize).min(last);
            let (a, b, t) = (clip.frames[before], clip.frames[(before + 1).min(last)], (at - before as f64) as f32);
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        })
        .collect()
}

/// A sample from -1 to 1 as 16 bits. What is louder than that is held at the loudest there is.
pub fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * 32_767.0).round() as i16
}

struct Voice {
    frames: Frames,
    /// The next frame to play.
    at: usize,
}

/// The clips playing now. Any number at once, the same one several times over.
pub struct Mixer {
    voices: Vec<Voice>,
    /// What every sample is multiplied by, 0 to 1.
    pub volume: f32,
}

impl Mixer {
    pub fn new(volume: f32) -> Mixer {
        Mixer { voices: Vec::new(), volume }
    }

    /// Starts `frames` from its beginning, over whatever else is playing.
    pub fn play(&mut self, frames: Frames) {
        if !frames.is_empty() {
            self.voices.push(Voice { frames, at: 0 });
        }
    }

    /// How many clips are still playing.
    pub fn playing(&self) -> usize {
        self.voices.len()
    }

    pub fn clear(&mut self) {
        self.voices.clear();
    }

    /// Fills `out` with the next frames: every playing clip summed, at the volume, held within -1 to 1. A clip
    /// that ends part-way leaves silence after it and is forgotten. Returns how many frames from the start of
    /// `out` have a clip in them: the rest is the silence after the last one ended.
    pub fn mix(&mut self, out: &mut [[f32; 2]]) -> usize {
        out.fill([0.0; 2]);
        let mut filled = 0;
        for voice in &mut self.voices {
            let rest = &voice.frames[voice.at..];
            for (sum, frame) in out.iter_mut().zip(rest) {
                sum[0] += frame[0];
                sum[1] += frame[1];
            }
            let taken = out.len().min(rest.len());
            voice.at += taken;
            filled = filled.max(taken);
        }
        self.voices.retain(|voice| voice.at < voice.frames.len());
        for sample in out.iter_mut().flatten() {
            *sample = (*sample * self.volume).clamp(-1.0, 1.0);
        }
        filled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tones::tone;

    fn frames(samples: &[f32]) -> Frames {
        samples.iter().map(|&s| [s, -s]).collect()
    }

    #[test]
    fn two_clips_are_summed_and_each_ends_where_it_ends() {
        let mut mixer = Mixer::new(1.0);
        mixer.play(frames(&[0.1, 0.2, 0.3, 0.4, 0.5]));
        mixer.play(frames(&[0.25, 0.25]));
        let mut out = [[9.0; 2]; 3];
        mixer.mix(&mut out);
        assert_eq!(out, [[0.35, -0.35], [0.45, -0.45], [0.3, -0.3]]);
        assert_eq!(mixer.playing(), 1, "the short one is over");
        // The long one's last two frames, then silence.
        assert_eq!(mixer.mix(&mut out), 2);
        assert_eq!(out, [[0.4, -0.4], [0.5, -0.5], [0.0, 0.0]]);
        assert_eq!(mixer.playing(), 0);
        assert_eq!(mixer.mix(&mut out), 0);
        assert_eq!(out, [[0.0; 2]; 3]);
    }

    #[test]
    fn a_clip_plays_over_itself() {
        let clip = frames(&[0.2, 0.4, 0.1]);
        let mut mixer = Mixer::new(1.0);
        mixer.play(clip.clone());
        let mut out = [[0.0; 2]; 1];
        mixer.mix(&mut out);
        assert_eq!(out, [[0.2, -0.2]]);
        // Started again a frame later: its second frame and its first together.
        mixer.play(clip);
        mixer.mix(&mut out);
        assert!((out[0][0] - 0.6).abs() < 1e-6 && (out[0][1] + 0.6).abs() < 1e-6, "{out:?}");
        assert_eq!(mixer.playing(), 2);
    }

    #[test]
    fn what_sums_past_full_scale_is_held_there() {
        let mut mixer = Mixer::new(1.0);
        for _ in 0..3 {
            mixer.play(frames(&[0.5, -0.5]));
        }
        let mut out = [[0.0; 2]; 2];
        mixer.mix(&mut out);
        assert_eq!(out, [[1.0, -1.0], [-1.0, 1.0]]);
        assert_eq!((to_i16(out[0][0]), to_i16(out[0][1])), (32_767, -32_767));
        assert_eq!((to_i16(3.0), to_i16(-3.0), to_i16(0.0)), (32_767, -32_767, 0));
    }

    #[test]
    fn the_volume_scales_and_zero_is_silence() {
        let mut out = [[0.0; 2]; 1];
        let mut mixer = Mixer::new(0.5);
        mixer.play(frames(&[0.8]));
        mixer.mix(&mut out);
        assert_eq!(out, [[0.4, -0.4]]);
        mixer.volume = 0.0;
        mixer.play(frames(&[0.8]));
        mixer.mix(&mut out);
        assert_eq!(out, [[0.0, 0.0]]);
    }

    #[test]
    fn two_tones_mixed_are_their_sum() {
        let (low, high) = (tone(220.0, 0.05, 48_000), tone(330.0, 0.02, 48_000));
        let mut mixer = Mixer::new(1.0);
        mixer.play(low.frames.clone().into());
        mixer.play(high.frames.clone().into());
        let mut out = vec![[0.0; 2]; low.frames.len()];
        mixer.mix(&mut out);
        for (i, frame) in out.iter().enumerate() {
            let want = low.frames[i][0] + high.frames.get(i).map_or(0.0, |f| f[0]);
            assert!((frame[0] - want).abs() < 1e-6, "frame {i}: {} for {want}", frame[0]);
        }
        assert_eq!(mixer.playing(), 0);
    }

    #[test]
    fn resampling_keeps_the_length_in_time_and_the_shape() {
        let clip = tone(440.0, 0.1, 44_100);
        let at_48k = resample(&clip, 48_000);
        assert_eq!(at_48k.len(), 4_800);
        // The same moment in both: a quarter of the way through a 440 Hz tone's 44th cycle.
        let (a, b) = (clip.frames[2_205][0], at_48k[2_400][0]);
        assert!((a - b).abs() < 0.01, "{a} at 44.1 kHz, {b} at 48 kHz");
        // A ramp halved in rate keeps every other frame; doubled, gains the frames half-way between.
        let ramp = Clip { rate: 4, frames: (0..5).map(|i| [i as f32, -(i as f32)]).collect() };
        assert_eq!(resample(&ramp, 2), [[0.0, 0.0], [2.0, -2.0], [4.0, -4.0]]);
        assert_eq!(resample(&ramp, 8)[..4], [[0.0, 0.0], [0.5, -0.5], [1.0, -1.0], [1.5, -1.5]]);
        assert_eq!(resample(&ramp, 8).len(), 10);
        assert_eq!(resample(&ramp, 4), ramp.frames);
    }
}
