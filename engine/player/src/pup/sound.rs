// Follows Visual Pinball X (license:GPLv3+), newer than build 5436: its PinUp Player plugin's sound bank
// (plugins/pup/PUPSound.cpp, PUPSound.h, added in 7b7f7ec0e), matching PinUP Player.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A pack's sound bank: every `.wav` straight in its `PUPSOUNDS` folder, played
//! by a script's `PuPSound`, `SendMSG` 402 and `PlaySSF` triggers as PinUP
//! Player plays them — panned from left to right by their x (-10 to 10; their
//! y and z change nothing, PinUP's 3D falloff being too small to hear), at
//! their volume (100 as made, up to 200). `ballroll0.wav` to `ballroll9.wav`
//! are ten slots that loop while the `sp` speed sent isn't 0. Heard on the
//! backglass speakers with the pack's other sound (`playback::sink`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::playback::sink;
use crate::audio::OUTPUT_RATE;

#[derive(Default)]
pub struct SoundBank {
    /// Each sound by its file's name in lower case, stereo at the mixer's rate.
    samples: HashMap<String, Arc<Vec<f32>>>,
    /// The ball-roll slots' sounds, and the loop playing in each.
    ball_roll: [Option<Arc<Vec<f32>>>; 10],
    rolling: [Option<u64>; 10],
}

impl SoundBank {
    /// The sounds of the pack at `pack` (VPX's `Load`).
    pub fn load(pack: &Path) -> SoundBank {
        let mut bank = SoundBank::default();
        let folder = std::fs::read_dir(pack)
            .into_iter()
            .flatten()
            .flatten()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("PUPSOUNDS") && e.path().is_dir())
            .map(|e| e.path());
        let Some(folder) = folder else { return bank };
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !name.ends_with(".wav") || !entry.path().is_file() {
                continue;
            }
            let decoded = std::fs::read(entry.path()).map_err(|e| e.to_string()).and_then(|bytes| crate::decode::decode_all(bytes, Some("wav")));
            let pcm = match decoded {
                Ok(pcm) => pcm,
                Err(e) => {
                    crate::script::log::once(&format!("PinUP: sound {} can't be read: {e}", entry.path().display()));
                    continue;
                }
            };
            let sample = Arc::new(stereo_at_output_rate(&pcm));
            let digit = name.as_bytes().get(8).copied();
            if name.len() == 13
                && name.starts_with("ballroll")
                && let Some(d) = digit.filter(u8::is_ascii_digit)
            {
                bank.ball_roll[usize::from(d - b'0')] = Some(sample.clone());
            }
            bank.samples.insert(name, sample);
        }
        bank
    }

    /// How many sounds the pack has.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Forgets the sounds, the ball-roll loops stopped (VPX's `Unload`).
    pub fn unload(&mut self) {
        for slot in &mut self.rolling {
            if let Some(id) = slot.take() {
                sink().stop_voice(id);
            }
        }
        *self = SoundBank::default();
    }

    /// Plays a sound (VPX's `Play`): a ball-roll slot starts looping, takes its new volume and
    /// place, or stops at a speed (`special`) of 0; another sound plays once.
    pub fn play(&mut self, name: &str, x: i32, _y: i32, _z: i32, volume: i32, special: &str) {
        let key = name.to_lowercase();
        let slot = key.as_bytes().get(8).filter(|d| key.len() >= 9 && key.starts_with("ballroll") && d.is_ascii_digit()).map(|d| usize::from(d - b'0'));
        if let Some(slot) = slot
            && let Some(sample) = self.ball_roll[slot].clone()
        {
            if super::string_to_int(special, 0) == 0 {
                if let Some(id) = self.rolling[slot].take() {
                    sink().stop_voice(id);
                }
            } else if let Some(id) = self.rolling[slot] {
                sink().set_voice_gains(id, gains(x, volume));
            } else {
                self.rolling[slot] = sink().play_voice(sample, gains(x, volume), true);
            }
            return;
        }
        let Some(sample) = self.samples.get(&key) else { return };
        if volume <= 0 {
            return;
        }
        sink().play_voice(sample.clone(), gains(x, volume), false);
    }
}

/// A sound's loudness on the left and right: its volume (100 as made, up to 200), panned by its
/// x (-10 left to 10 right) at constant power, the listener at the centre (VPX's `Queue`).
fn gains(x: i32, volume: i32) -> [f32; 2] {
    let pan = (x as f32 / 10.0).clamp(-1.0, 1.0);
    let angle = (pan + 1.0) * 0.785_398_16;
    let v = (volume as f32 / 100.0).clamp(0.0, 2.0);
    [angle.cos() * v, angle.sin() * v]
}

/// A decoded sound made stereo (a mono one on both sides, a longer layout's first two channels)
/// at the mixer's rate, by linear interpolation (VPX hands the stream its own rate).
fn stereo_at_output_rate(pcm: &crate::decode::Pcm) -> Vec<f32> {
    let channels = pcm.channels.max(1);
    let frames = pcm.samples.len() / channels;
    if frames == 0 || pcm.rate == 0 {
        return Vec::new();
    }
    let frame = |f: usize| -> [f32; 2] {
        let s = &pcm.samples[f * channels..];
        if channels == 1 { [s[0], s[0]] } else { [s[0], s[1]] }
    };
    let ratio = f64::from(pcm.rate) / f64::from(OUTPUT_RATE);
    let out_frames = (frames as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_frames * 2);
    for i in 0..out_frames {
        let at = i as f64 * ratio;
        let k = (at as usize).min(frames - 1);
        let t = (at - k as f64) as f32;
        let (a, b) = (frame(k), frame((k + 1).min(frames - 1)));
        out.push(a[0] + (b[0] - a[0]) * t);
        out.push(a[1] + (b[1] - a[1]) * t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_pan_at_constant_power() {
        let [l, r] = gains(0, 100);
        assert!((l - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6 && (r - l).abs() < 1e-6);
        let [l, r] = gains(-10, 100);
        assert!((l - 1.0).abs() < 1e-6 && r.abs() < 1e-6);
        let [l, r] = gains(25, 300);
        assert!(l.abs() < 1e-6 && (r - 2.0).abs() < 1e-6);
    }

    #[test]
    fn a_mono_sound_is_made_stereo_at_the_mixers_rate() {
        let pcm = crate::decode::Pcm { rate: 24_000, channels: 1, samples: vec![0.0, 1.0, 0.0, -1.0] };
        let out = stereo_at_output_rate(&pcm);
        assert_eq!(out.len(), 16);
        assert_eq!(&out[..6], &[0.0, 0.0, 0.5, 0.5, 1.0, 1.0]);
    }
}
