// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// PinTable::PlaySound and StopSound (src/parts/pintable.cpp), AudioPlayer::PlaySound and StopSound
// (src/audio/AudioPlayer.cpp), and SoundPlayer::Play, ApplyVolume and the playfield and backglass
// mixing of its mix node (src/audio/SoundPlayer.cpp), for stereo output at VPX's default volumes; and
// the music of AudioPlayer::PlayMusic, PauseMusic and SetMusicVolume, streamed from a file as
// SoundPlayer streams it (played once, on the backglass speakers); and its WMP plugin's audio player
// (plugins/wmp/WMPAudioPlayer.cpp), the stand-in for Windows Media Player's object.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! Sound: the table's own sounds, played the way VPX plays them, through
//! SDL3's audio on its own thread with a small mixer of ours.
//!
//! The game thread turns each `PlaySound` into a command — which sound, how
//! loud on each side, how fast, how many times — and the audio thread mixes
//! whatever is playing. VPX's rules: a playfield sound is mixed to mono and
//! panned between the two speakers, a backglass sound stays stereo; the
//! loudness is the square root of the volume; the pan is softened (VPX
//! undoes the tenth power tables apply to it); the pitch shifts the
//! playback frequency in hertz; a sound loops forever, plays once, or plays
//! a given number of times; "use same" changes a copy already playing
//! instead of starting another, and "restart" stops every copy first.

use std::collections::{HashMap, VecDeque};
use std::ffi::{c_int, c_void};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use sdl3_sys::everything::*;
use vpin::vpx::VPX;

/// One of the table's sounds, decoded.
pub struct Sound {
    pub name: String,
    pub rate: u32,
    /// 1 for a playfield sound (mixed to mono), 2 for a backglass sound.
    pub channels: u16,
    /// Interleaved samples, -1 to 1.
    pub samples: Vec<f32>,
    /// The sound's own volume and pan settings, -1 to 1 (VPX's
    /// `dequantizeSignedPercent`).
    pub volume: f32,
    pub balance: f32,
    /// Its own place from the front of the playfield to the back, -1 to 1 (VPX's `m_frontRearFade`),
    /// added to the script's as it plays.
    pub fade: f32,
    /// Plays on the backglass speakers (stereo) rather than the playfield's.
    pub backglass: bool,
    /// The stored sound's channel count: mixing to mono averages the
    /// channels, and VPX multiplies the loudness back by their number.
    pub mono_compensation: f32,
}

impl Sound {
    fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }
}

/// VPX's `dequantizeSignedPercent`: -100..100 percent to -1..1. vpin keeps
/// the stored 32-bit value as is, so negative settings arrive wrapped.
fn signed_percent(raw: u32) -> f32 {
    (raw as i32 as f32 / 100.0).clamp(-1.0, 1.0)
}

/// The extension of a sound's stored path; a path without one is WAV, as
/// VPX treats it.
fn extension(path: &str) -> Option<String> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.rsplit_once('.').map(|(_, ext)| ext.to_lowercase())
}

/// WAV data as the table stores it (PCM 8, 16, 24 or 32 bit, or 32-bit
/// float, after its format header): rate, channels, interleaved samples.
fn wav_samples(s: &vpin::vpx::sound::SoundData) -> Option<(u32, u16, Vec<f32>)> {
    let w = &s.wave_form;
    let channels = w.channels.max(1);
    let bits = w.bits_per_sample;
    let float = w.format_tag == 3;
    if !(w.format_tag == 1 || float || w.format_tag == 0xFFFE) || w.samples_per_sec == 0 {
        return None;
    }
    let interleaved: Vec<f32> = match (bits, float) {
        (8, false) => s.data.iter().map(|&b| (f32::from(b) - 128.0) / 128.0).collect(),
        (16, false) => s.data.as_chunks::<2>().0.iter().map(|c| f32::from(i16::from_le_bytes(*c)) / 32768.0).collect(),
        (24, false) => s.data.as_chunks::<3>().0.iter().map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.0).collect(),
        (32, false) => s.data.as_chunks::<4>().0.iter().map(|c| i32::from_le_bytes(*c) as f32 / 2_147_483_648.0).collect(),
        (32, true) => s.data.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect(),
        _ => return None,
    };
    Some((w.samples_per_sec, channels, interleaved))
}

/// Decodes a sound stored in the table: WAV data with its format header, or
/// a compressed file kept whole (MP3, Ogg Vorbis, FLAC), as VPX decodes it
/// with miniaudio.
fn decode(s: &vpin::vpx::sound::SoundData) -> Option<Sound> {
    let ext = extension(&s.path);
    let (rate, channels, interleaved) = match ext.as_deref() {
        None | Some("wav") => wav_samples(s)?,
        Some(ext) => {
            let pcm = crate::decode::decode_all(s.data.clone(), Some(ext)).ok()?;
            (pcm.rate, u16::try_from(pcm.channels).ok()?, pcm.samples)
        }
    };
    let channels = channels.max(1);
    // VPX decodes a playfield sound to mono (averaging the channels) and a
    // backglass sound to stereo
    let backglass = matches!(s.output_target, vpin::vpx::sound::OutputTarget::Backglass);
    let ch = usize::from(channels);
    let frames = interleaved.chunks_exact(ch);
    let (samples, out_channels): (Vec<f32>, u16) = if backglass {
        (frames.flat_map(|f| [f[0], f[if ch > 1 { 1 } else { 0 }]]).collect(), 2)
    } else {
        (frames.map(|f| f.iter().sum::<f32>() / ch as f32).collect(), 1)
    };
    Some(Sound {
        name: s.name.clone(),
        rate,
        channels: out_channels,
        samples,
        volume: signed_percent(s.volume),
        balance: signed_percent(s.balance),
        fade: signed_percent(s.fade),
        backglass,
        mono_compensation: if backglass { 1.0 } else { f32::from(channels) },
    })
}

/// How the playfield's sound is laid out among its output's speakers (VPX's `[Player]` `Sound3D`,
/// `SoundConfigTypes`): two front speakers; all at the back; the playfield's front at the front or
/// at the back; and two layouts that keep the front pair free for the backglass, the second for
/// the exciters of SSF (surround sound feedback) cabinets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Stereo,
    AllRear,
    FrontIsRear,
    FrontIsFront,
    SixChannels,
    Ssf,
}

impl Layout {
    /// VPX's setting's value, 0 to 5.
    pub fn from_setting(value: i32) -> Layout {
        match value {
            1 => Layout::AllRear,
            2 => Layout::FrontIsRear,
            3 => Layout::FrontIsFront,
            4 => Layout::SixChannels,
            5 => Layout::Ssf,
            _ => Layout::Stereo,
        }
    }
}

/// The most channels an output is mixed for (VPX's layouts stop at 7.1).
const MAX_CHANNELS: usize = 8;

/// What the game thread asks of the audio thread.
pub enum Command {
    /// `volume`: the script's volume, added to the sound's own, and `pan` and `fade` the script's
    /// (side to side, front to back), all before the player's volumes, which the mixer applies
    /// (`voice_gains`); `loops`: repeats after the first play, or `None` to loop forever.
    Play { sound: usize, volume: f32, pan: f32, fade: f32, rate: f32, loops: Option<u32>, use_same: bool, restart: bool },
    Stop { sound: usize },
    /// New music, replacing any playing, at a volume before the player's backglass volume.
    Music { stream: Arc<MusicStream>, volume: f32 },
    /// VPX's `EndMusic`: the music pauses.
    PauseMusic,
    MusicVolume(f32),
    /// A stream of sound made elsewhere starts, or stops (`None`).
    Stream(Option<Box<dyn StreamSource>>),
    /// The PinUP packs' sound, every screen's mixed (`pup::playback`), beside PinMAME's stream.
    PupStream(Box<dyn StreamSource>),
    /// A file a script's Windows Media Player object loaded, mixed from now on while it plays.
    Media(Arc<MediaStream>),
    /// An achievement's sound as it's announced, mixed while it plays at the achievements' level.
    Chime(Arc<MediaStream>),
    /// The player's playfield and backglass volumes, from now on, for everything playing too
    /// (VPX's `AudioPlayer::SetMainVolume`): sounds, music and every stream.
    Volumes { playfield: f32, backglass: f32 },
    /// Each source's level on the backglass's speakers, from now on (`Levels`).
    Levels(Levels),
    /// The game paused (the pause menu): every sound of the table's and its music held where they
    /// are, or going on again from there.
    Hold(bool),
}

/// The level of each kind of sound the backglass's speakers play besides the table's own sounds,
/// over the backglass volume, 0 to 2 (1: as it comes): VPX 5436's audio sources' gains (`[Player]`
/// `AudioSource.PinMAME.Gain`, `…AltSound.Gain`, `…PUP.Gain`, `…WMP.Gain`;
/// `Player::SetAudioLaneMixerVolume`), and the table's music's, ours (`[VPI]` `MusicGain`), which
/// VPX plays at the backglass volume alone; and the achievements' sound as each is announced, ours
/// (`[VPI]` `AchievementGain`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub pinmame: f32,
    /// The machine's AltSound pack, when it plays in PinMAME's place (VPX's AltSound plugin's source).
    pub altsound: f32,
    pub pup: f32,
    pub wmp: f32,
    pub music: f32,
    pub achievements: f32,
}

impl Default for Levels {
    fn default() -> Levels {
        Levels { pinmame: 1.0, altsound: 1.0, pup: 1.0, wmp: 1.0, music: 1.0, achievements: 1.0 }
    }
}

impl Levels {
    /// The levels the settings give, each 0 to 2, 1 when not set (VPX's `FloatPropertyDef`).
    pub fn from_settings(s: &crate::settings::Settings) -> Levels {
        let gain = |section: &str, name: &str| s.float_or(section, name, 1.0, 0.0, 2.0, 0.0);
        Levels {
            pinmame: gain("Player", "AudioSource.PinMAME.Gain"),
            altsound: gain("Player", "AudioSource.AltSound.Gain"),
            pup: gain("Player", "AudioSource.PUP.Gain"),
            wmp: gain("Player", "AudioSource.WMP.Gain"),
            music: gain("VPI", "MusicGain"),
            achievements: gain("VPI", "AchievementGain"),
        }
    }
}

/// Sound made outside the table, played as it comes: PinMAME's sound
/// boards, a PinUP pack's sound. VPX plays them as streams on the backglass speakers.
pub trait StreamSource: Send {
    /// Adds the next `out.len() / 2` stereo frames into `out`, at `gain`;
    /// silence where there's nothing yet.
    fn mix_into(&mut self, out: &mut [f32], gain: f32);
}

/// Two seconds of stereo music decoded ahead of the mixer.
const MUSIC_AHEAD: usize = 2 * 2 * OUTPUT_RATE as usize;

/// Music played from a file (VPX's `PlayMusic`): decoded on a thread of its
/// own a little ahead of the mixer, resampled to the output rate, and
/// played once on the backglass speakers, as VPX streams it.
pub struct MusicStream {
    /// Stereo frames at the output rate, interleaved, waiting to play.
    queue: Mutex<VecDeque<f32>>,
    /// The file has been decoded to its end (or couldn't be).
    finished: AtomicBool,
    /// No one will play it any more: the decoding stops.
    abandoned: AtomicBool,
}

impl MusicStream {
    /// Starts decoding the file on a thread of its own.
    pub fn start(path: PathBuf) -> Arc<MusicStream> {
        let stream = Arc::new(MusicStream { queue: Mutex::new(VecDeque::new()), finished: AtomicBool::new(false), abandoned: AtomicBool::new(false) });
        let s = stream.clone();
        let spawned = std::thread::Builder::new().name("music".into()).spawn(move || {
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
            let mut resampler = Resampler::default();
            let mut out = Vec::new();
            let result = std::fs::File::open(&path).map_err(|e| e.to_string()).and_then(|file| {
                crate::decode::stream(Box::new(file), ext.as_deref(), |rate, channels, samples| {
                    out.clear();
                    resampler.push(rate, channels, samples, &mut out);
                    // wait while enough is decoded ahead, unless the music is dropped
                    while !s.abandoned.load(Ordering::Acquire) && s.queue.lock().unwrap_or_else(|e| e.into_inner()).len() >= MUSIC_AHEAD {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    if s.abandoned.load(Ordering::Acquire) {
                        return false;
                    }
                    s.queue.lock().unwrap_or_else(|e| e.into_inner()).extend(out.iter().copied());
                    true
                })
            });
            if let Err(e) = result {
                eprintln!("[script] PlayMusic: {}: {e}", path.display());
            }
            s.finished.store(true, Ordering::Release);
        });
        if spawned.is_err() {
            stream.finished.store(true, Ordering::Release);
        }
        stream
    }
}

/// A file a script plays through its Windows Media Player object (VPX 5436's WMP plugin,
/// `WMPAudioPlayer`): decoded on a thread of its own a little ahead, resampled to the output rate,
/// and mixed on the backglass speakers while it plays; it can pause, stop (back to its start) and
/// go to a moment. VPX decodes it with miniaudio (WAV, FLAC, MP3, Ogg Vorbis), the formats our
/// decoder reads too.
pub struct MediaStream {
    path: PathBuf,
    /// Stereo frames at the output rate, interleaved, waiting to play.
    queue: Mutex<VecDeque<f32>>,
    /// Bumped when the file is to be decoded again from `start_frame` (a stop or a jump).
    generation: AtomicU64,
    start_frame: AtomicU64,
    /// The decoder reached the file's end for the present generation.
    finished: AtomicBool,
    /// Frames played from the file's start, at the output rate: where it is.
    played: AtomicU64,
    playing: AtomicBool,
    paused: AtomicBool,
    /// 0 to 1, as f32 bits.
    volume: AtomicU32,
    /// The script let go of it: the decoding stops and the mixer drops it.
    abandoned: AtomicBool,
}

impl MediaStream {
    /// Starts decoding `path` on a thread of its own; it plays once asked.
    pub fn load(path: PathBuf, volume: f32) -> Arc<MediaStream> {
        let m = Arc::new(MediaStream {
            path,
            queue: Mutex::new(VecDeque::new()),
            generation: AtomicU64::new(0),
            start_frame: AtomicU64::new(0),
            finished: AtomicBool::new(false),
            played: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            volume: AtomicU32::new(volume.to_bits()),
            abandoned: AtomicBool::new(false),
        });
        let s = m.clone();
        let spawned = std::thread::Builder::new().name("media".into()).spawn(move || s.decode());
        if spawned.is_err() {
            m.finished.store(true, Ordering::Release);
        }
        m
    }

    /// The decoder's thread: the file from `start_frame`, again whenever the generation moves on.
    fn decode(&self) {
        let ext = self.path.extension().map(|e| e.to_string_lossy().to_lowercase());
        while !self.abandoned.load(Ordering::Acquire) {
            let generation = self.generation.load(Ordering::Acquire);
            let mut skip = self.start_frame.load(Ordering::Acquire) as usize * 2;
            let mut resampler = Resampler::default();
            let mut out = Vec::new();
            let moved_on = || self.abandoned.load(Ordering::Acquire) || self.generation.load(Ordering::Acquire) != generation;
            let result = std::fs::File::open(&self.path).map_err(|e| e.to_string()).and_then(|file| {
                crate::decode::stream(Box::new(file), ext.as_deref(), |rate, channels, samples| {
                    out.clear();
                    resampler.push(rate, channels, samples, &mut out);
                    let dropped = skip.min(out.len());
                    skip -= dropped;
                    while !moved_on() && self.queue.lock().unwrap_or_else(|e| e.into_inner()).len() >= MUSIC_AHEAD {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    if moved_on() {
                        return false;
                    }
                    self.queue.lock().unwrap_or_else(|e| e.into_inner()).extend(out[dropped..].iter().copied());
                    true
                })
            });
            if let Err(e) = result {
                crate::script::log::once(&format!("WMPlayer: {}: {e}", self.path.display()));
            }
            if self.generation.load(Ordering::Acquire) == generation {
                self.finished.store(true, Ordering::Release);
                // wait for a stop, a jump or the end of the object
                while !moved_on() {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        }
    }

    /// Decodes again from output frame `frame` (`SetPosition`; a stop goes back to 0).
    fn restart_at(&self, frame: u64) {
        self.start_frame.store(frame, Ordering::Release);
        self.played.store(frame, Ordering::Release);
        self.finished.store(false, Ordering::Release);
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// VPX's `Play`: from where it is, from the start again after it played to its end.
    pub fn play(&self) {
        if self.playing.load(Ordering::Acquire) && !self.paused.load(Ordering::Acquire) {
            return;
        }
        if !self.playing.load(Ordering::Acquire) && self.at_end() {
            self.restart_at(0);
        }
        self.paused.store(false, Ordering::Release);
        self.playing.store(true, Ordering::Release);
    }

    /// VPX's `Pause`.
    pub fn pause(&self) {
        if self.playing.load(Ordering::Acquire) {
            self.paused.store(true, Ordering::Release);
        }
    }

    /// VPX's `Stop`: not playing, back to its start.
    pub fn stop(&self) {
        if !self.playing.load(Ordering::Acquire) {
            return;
        }
        self.playing.store(false, Ordering::Release);
        self.paused.store(false, Ordering::Release);
        self.restart_at(0);
    }

    /// Where it is, in seconds.
    pub fn position(&self) -> f64 {
        self.played.load(Ordering::Acquire) as f64 / f64::from(OUTPUT_RATE)
    }

    /// Goes to `seconds` from the start.
    pub fn set_position(&self, seconds: f64) {
        self.restart_at((seconds.max(0.0) * f64::from(OUTPUT_RATE)) as u64);
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume.store(volume.to_bits(), Ordering::Release);
    }

    /// The script let go of it.
    pub fn close(&self) {
        self.playing.store(false, Ordering::Release);
        self.abandoned.store(true, Ordering::Release);
    }

    /// Decoded to its end and all played.
    fn at_end(&self) -> bool {
        self.finished.load(Ordering::Acquire) && self.queue.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
    }

    /// Mixes its next frames while it plays; a file played to its end stops (VPX's
    /// `OnEngineProcess`, the sound's end).
    fn mix_into(&self, out: &mut [f32], gain: f32) {
        if !self.playing.load(Ordering::Acquire) || self.paused.load(Ordering::Acquire) {
            return;
        }
        let g = f32::from_bits(self.volume.load(Ordering::Acquire)) * gain;
        let n = {
            let mut q = self.queue.lock().unwrap_or_else(|e| e.into_inner());
            let n = out.len().min(q.len());
            for (o, x) in out.iter_mut().zip(q.drain(..n)) {
                *o += x * g;
            }
            n
        };
        self.played.fetch_add((n / 2) as u64, Ordering::AcqRel);
        if self.at_end() {
            self.playing.store(false, Ordering::Release);
        }
    }
}

/// Linear resampling of stereo frames to the output rate, carried on from
/// one decoded packet to the next. A mono file plays on both sides, as VPX
/// expands it; a file with more channels plays its first two.
#[derive(Default)]
struct Resampler {
    /// Where the next output frame falls, in input frames from `last`.
    pos: f64,
    /// The previous packet's last frame.
    last: Option<[f32; 2]>,
}

impl Resampler {
    fn push(&mut self, rate: u32, channels: usize, samples: &[f32], out: &mut Vec<f32>) {
        let n = samples.len() / channels.max(1);
        if n == 0 || rate == 0 {
            return;
        }
        let frame = |i: usize| [samples[i * channels], samples[i * channels + usize::from(channels > 1)]];
        // the frames to work through: the previous packet's last, then this one's
        let carried = usize::from(self.last.is_some());
        let last = self.last;
        let get = |k: usize| if k < carried { last.unwrap_or_default() } else { frame(k - carried) };
        let total = n + carried;
        let step = f64::from(rate) / f64::from(OUTPUT_RATE);
        while self.pos + 1.0 < total as f64 {
            let k = self.pos as usize;
            let t = (self.pos - k as f64) as f32;
            let (a, b) = (get(k), get(k + 1));
            out.extend_from_slice(&[a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            self.pos += step;
        }
        self.pos -= (total - 1) as f64;
        self.last = Some(get(total - 1));
    }
}

/// The music playing on the audio thread.
struct MusicVoice {
    stream: Arc<MusicStream>,
    /// Its volume before the backglass volume, and its loudness after it.
    volume: f32,
    gain: f32,
    paused: bool,
}

impl Drop for MusicVoice {
    fn drop(&mut self) {
        self.stream.abandoned.store(true, Ordering::Release);
    }
}

/// The game thread's side: the table's sounds by name, and the lines to the
/// two outputs' mixers, as VPX 5436 plays through two (`AudioPlayer`): the
/// playfield's sounds to one, the backglass's sounds, the music and every
/// stream to the other.
pub struct Sounds {
    pub sounds: Vec<Arc<Sound>>,
    by_name: HashMap<String, usize>,
    playfield: Option<Sender<Command>>,
    backglass: Option<Sender<Command>>,
    /// The player's playfield volume, 0 to 1 (VPX's default: all of it).
    pub playfield_volume: f32,
    /// The player's backglass volume, 0 to 1.
    pub backglass_volume: f32,
    /// Each source's level on the backglass's speakers.
    pub levels: Levels,
    /// What the table has played so far besides its sounds, for the pause menu's levels: a
    /// machine (its AltSound pack, `true`, or PinMAME's own sound), a PinUP pack's sound (or a
    /// FlexDMD's videos'), Windows Media Player's files, music.
    pub machine: Option<bool>,
    pub pup_played: bool,
    pub media_played: bool,
    pub music_played: bool,
}

impl Sounds {
    /// Decodes every sound in the table. A sound that won't decode is named
    /// once and left out.
    pub fn load(vpx: &VPX) -> Sounds {
        let mut sounds = Vec::new();
        let mut by_name = HashMap::new();
        let mut skipped = Vec::new();
        for s in &vpx.sounds {
            match decode(s) {
                Some(d) => {
                    by_name.entry(s.name.to_lowercase()).or_insert(sounds.len());
                    sounds.push(Arc::new(d));
                }
                None => skipped.push(s.name.clone()),
            }
        }
        if !skipped.is_empty() {
            crate::script::log::once(&format!("sounds that won't decode, left out: {}", skipped.join(", ")));
        }
        Sounds {
            sounds,
            by_name,
            playfield: None,
            backglass: None,
            playfield_volume: 1.0,
            backglass_volume: 1.0,
            levels: Levels::default(),
            machine: None,
            pup_played: false,
            media_played: false,
            music_played: false,
        }
    }

    /// No sounds, nothing played yet, every level at its own (for tests elsewhere).
    #[cfg(test)]
    pub fn none_played_for_tests() -> Sounds {
        Sounds::none_played()
    }

    /// No sounds, nothing played yet, every level at its own (for tests).
    #[cfg(test)]
    fn none_played() -> Sounds {
        Sounds {
            sounds: Vec::new(),
            by_name: HashMap::new(),
            playfield: None,
            backglass: None,
            playfield_volume: 1.0,
            backglass_volume: 1.0,
            levels: Levels::default(),
            machine: None,
            pup_played: false,
            media_played: false,
            music_played: false,
        }
    }

    /// The lines to the playfield's and the backglass's mixers (the same one, when only one
    /// output opened).
    pub fn connect(&mut self, playfield: Sender<Command>, backglass: Sender<Command>) {
        (self.playfield, self.backglass) = (Some(playfield), Some(backglass));
        self.send(Command::Volumes { playfield: self.playfield_volume, backglass: self.backglass_volume });
        self.send(Command::Levels(self.mixer_levels()));
    }

    /// The levels as the mixer applies them: the machine's stream at its AltSound pack's level
    /// when that plays in PinMAME's place.
    fn mixer_levels(&self) -> Levels {
        let mut l = self.levels;
        if self.machine == Some(true) {
            l.pinmame = l.altsound;
        }
        l
    }

    /// Each source's level on the backglass's speakers from now on, for what plays already too.
    pub fn set_levels(&mut self, levels: Levels) {
        self.levels = levels;
        self.send(Command::Levels(self.mixer_levels()));
    }

    /// The game paused or going on (the pause menu): the table's sounds and its music held where
    /// they are, or going on from there. VPX 5436 pauses the music alone (`Player::PauseMusic`);
    /// a table's looping sounds — a ball's roll among them — would play on through the pause.
    pub fn hold(&self, held: bool) {
        // both mixers (the same one twice, when only one output opened)
        for tx in [&self.playfield, &self.backglass].into_iter().flatten() {
            let _ = tx.send(Command::Hold(held));
        }
    }

    /// The player's playfield and backglass volumes, 0 to 1, from now on, for what plays already too.
    pub fn set_volumes(&mut self, playfield: f32, backglass: f32) {
        (self.playfield_volume, self.backglass_volume) = (playfield.clamp(0.0, 1.0), backglass.clamp(0.0, 1.0));
        self.send(Command::Volumes { playfield: self.playfield_volume, backglass: self.backglass_volume });
    }

    /// A command to the mixer it's for: a sound's to its speakers' output, the volumes to both,
    /// everything else (music, streams) to the backglass's.
    fn send(&self, c: Command) {
        let to_playfield = match &c {
            Command::Play { sound, .. } | Command::Stop { sound } => !self.sounds[*sound].backglass,
            Command::Volumes { playfield, backglass } => {
                if let Some(tx) = &self.playfield {
                    let _ = tx.send(Command::Volumes { playfield: *playfield, backglass: *backglass });
                }
                false
            }
            _ => false,
        };
        if let Some(tx) = if to_playfield { &self.playfield } else { &self.backglass } {
            let _ = tx.send(c);
        }
    }

    /// VPX's `PlaySound`. `rand_01` draws from VPX's second random generator
    /// for a random pitch.
    #[allow(clippy::too_many_arguments)] // PlaySound's arguments
    pub fn play(&self, name: &str, loop_count: i32, volume: f32, pan: f32, fade: f32, random_pitch: f32, pitch: i32, use_same: bool, restart: bool, mut rand_01: impl FnMut() -> f32) {
        let Some(&index) = self.by_name.get(&name.to_lowercase()) else {
            if !name.is_empty() {
                crate::script::log::once(&format!("PlaySound: no sound named \"{name}\" in the table"));
            }
            return;
        };
        let sound = &self.sounds[index];
        // the frequency: the sound's own, plus pitch hertz, then a random spread
        let sample_freq = sound.rate as f32;
        let mut freq = sample_freq + pitch as f32;
        if random_pitch > 0.0 {
            let (h, l) = (rand_01(), rand_01());
            freq *= 1.0 + (random_pitch * h * h) - (random_pitch * l * l * 0.5);
        }
        let rate = (freq / sample_freq).max(0.0);
        // a negative count loops forever; 0 and 1 play once; more play that many times
        let loops = if loop_count < 0 { None } else { Some((loop_count.max(1) - 1) as u32) };
        self.send(Command::Play { sound: index, volume, pan, fade, rate, loops, use_same, restart });
    }

    /// VPX's `StopSound`: stops every copy of the sound.
    pub fn stop(&self, name: &str) {
        if let Some(&index) = self.by_name.get(&name.to_lowercase()) {
            self.send(Command::Stop { sound: index });
        }
    }

    /// VPX's `PlayMusic`: the file plays from its start, once, replacing
    /// any music playing; `volume` is the table's music volume times the
    /// script's.
    pub fn play_music(&mut self, path: PathBuf, volume: f32) {
        if self.backglass.is_none() {
            return; // no sound output (test mode, the headless runner)
        }
        self.music_played = true;
        self.send(Command::Music { stream: MusicStream::start(path), volume });
    }

    /// VPX's `EndMusic`: the music pauses.
    pub fn pause_music(&self) {
        self.send(Command::PauseMusic);
    }

    /// VPX's `MusicVolume`: the music's volume from now on.
    pub fn set_music_volume(&self, volume: f32) {
        self.send(Command::MusicVolume(volume));
    }

    /// A stream of sound made elsewhere plays, replacing any other, at the
    /// player's backglass volume (VPX sets a stream's gain to it as is).
    pub fn play_stream(&self, source: Box<dyn StreamSource>) {
        self.send(Command::Stream(Some(source)));
    }

    /// The machine's sound plays from now on as a stream (`play_stream`), PinMAME's own or, with
    /// `altsound`, its AltSound pack's in its place: at that source's level.
    pub fn play_machine(&mut self, source: Box<dyn StreamSource>, altsound: bool) {
        self.machine = Some(altsound);
        self.send(Command::Levels(self.mixer_levels()));
        self.play_stream(source);
    }

    pub fn stop_stream(&self) {
        self.send(Command::Stream(None));
    }

    /// The PinUP packs' sound plays from now on, at the player's backglass volume, as VPX
    /// streams a pack's sound; nothing where there's no sound output.
    pub fn play_pup(&mut self, sink: &Arc<crate::pup::playback::Sink>) {
        self.pup_played = true;
        if self.backglass.is_some() && !sink.is_connected() {
            self.send(Command::PupStream(sink.source()));
        }
    }

    /// A Windows Media Player object's file is mixed from now on, while it plays, at the player's
    /// backglass volume (VPX streams it as a backglass sound).
    pub fn play_media(&mut self, stream: &Arc<MediaStream>) {
        self.media_played = true;
        self.send(Command::Media(stream.clone()));
    }

    /// An achievement's sound mixed while it plays, at the backglass volume and the achievements'
    /// level.
    pub fn play_chime(&mut self, stream: &Arc<MediaStream>) {
        self.send(Command::Chime(stream.clone()));
    }
}

/// A sound playing.
struct Voice {
    sound: usize,
    /// Position in the sound's frames.
    at: f64,
    /// Frames of the sound per output frame.
    step: f64,
    /// Its loudness on each of the output's channels.
    gains: [f32; MAX_CHANNELS],
    /// Repeats still to play after this one, or `None` to loop forever.
    loops: Option<u32>,
    /// The script's volume, pan and fade, which the player's volumes are applied to again when
    /// they change.
    volume: f32,
    pan: f32,
    fade: f32,
}

/// The audio thread's side.
struct Mixer {
    sounds: Vec<Arc<Sound>>,
    commands: Receiver<Command>,
    voices: Vec<Voice>,
    music: Option<MusicVoice>,
    stream: Option<Box<dyn StreamSource>>,
    pup: Option<Box<dyn StreamSource>>,
    /// Scripts' Windows Media Player files.
    media: Vec<Arc<MediaStream>>,
    chimes: Vec<Arc<MediaStream>>,
    /// The player's playfield and backglass volumes now, 0 to 1.
    playfield_volume: f32,
    backglass_volume: f32,
    /// Each source's level.
    levels: Levels,
    /// The game is paused: the table's sounds and its music hold where they are.
    held: bool,
    /// The output's channels, and how the playfield's sound is laid out among them.
    channels: usize,
    layout: Layout,
    rate: f64,
    buffer: Vec<f32>,
    /// The music and the streams, stereo, before they go to an output of more channels.
    stereo: Vec<f32>,
}

/// A sound's loudness on each of the output's `channels` (VPX's `PlaySound`, `SoundPlayer::Play`
/// and its mixing node, `vpx_node_process_pcm_frames`): the square root of its own volume added
/// to the script's, times the player's volume for its speakers; its pan and its front-to-back
/// place, each its own and the script's, softened (tables apply a tenth power, VPX undoes it).
/// A backglass sound plays on the front pair, its far side lowered; a playfield sound is spread
/// between the speakers the layout gives it — SDL's channel orders: stereo FL FR, quad FL FR BL
/// BR, 5.1 FL FR FC LFE BL BR, 7.1 FL FR FC LFE BL BR SL SR.
fn voice_gains(sound: &Sound, volume: f32, pan: f32, fade: f32, playfield: f32, backglass: f32, channels: usize, layout: Layout) -> [f32; MAX_CHANNELS] {
    let main = if sound.backglass { backglass } else { playfield };
    let total = ((sound.volume + volume) * main).clamp(0.0, 1.0);
    let gain = sound.mono_compensation * total.sqrt();
    let soften = |x: f32| {
        let x = x.clamp(-1.0, 1.0);
        if x < 0.0 { -(-x).powf(0.1) } else { x.powf(0.1) }
    };
    let pan = soften(sound.balance + pan);
    let mut g = [0.0f32; MAX_CHANNELS];
    let n = channels.clamp(1, MAX_CHANNELS);
    if sound.backglass {
        // stereo: the far side is lowered, nothing is blended; on one channel, both halves
        if n == 1 {
            g[0] = gain * 0.5;
        } else {
            g[0] = gain * if pan < 0.0 { 1.0 } else { 1.0 - pan };
            g[1] = gain * if pan > 0.0 { 1.0 } else { 1.0 + pan };
        }
        return g;
    }
    let fade = match layout {
        Layout::Stereo => 0.0,
        Layout::AllRear => 1.0,
        Layout::FrontIsFront | Layout::SixChannels | Layout::Ssf => soften(sound.fade + fade),
        Layout::FrontIsRear => -soften(sound.fade + fade),
    };
    // the layouts that keep the front pair free for the backglass
    let no_front = matches!(layout, Layout::SixChannels | Layout::Ssf);
    let (left, right) = (gain * 0.5 * (1.0 - pan), gain * 0.5 * (1.0 + pan));
    if n == 1 {
        g[0] = gain;
    } else if n < 4 || (no_front && n < 8) {
        // two speakers: the front pair, or the last pair
        let l = if n < 4 { 0 } else { n - 2 };
        (g[l], g[l + 1]) = (left, right);
    } else if n < 8 || (no_front && n == 8) || fade == 0.0 {
        // four speakers, front and back, faded between by the sound's place. (VPX's six-speaker
        // way below sends a sound in the middle of the table nowhere, and its stereo layout puts
        // every sound there on a 7.1 output: such a sound plays here as on four speakers.)
        let front = 0.5 * (1.0 + fade);
        let rear = 1.0 - front;
        let (f, r) = if n == 8 && no_front { (4, 6) } else if n == 8 { (0, 4) } else { (0, n - 2) };
        (g[f], g[f + 1], g[r], g[r + 1]) = (front * left, front * right, rear * left, rear * right);
    } else {
        // six speakers: front, middle and back pairs
        let (front, center, rear) = (fade.max(0.0), fade.abs(), (-fade).max(0.0));
        (g[0], g[1], g[4], g[5], g[6], g[7]) = (front * left, front * right, center * left, center * right, rear * left, rear * right);
    }
    g
}

/// The loudness of music at `volume`: the square root, as for sounds, after the player's
/// backglass volume (VPX's `SoundPlayer::ApplyVolume`).
fn music_gain(volume: f32, backglass: f32) -> f32 {
    (volume * backglass).clamp(0.0, 1.0).sqrt()
}

impl Mixer {
    fn new(sounds: Vec<Arc<Sound>>, commands: Receiver<Command>, playfield_volume: f32, backglass_volume: f32) -> Mixer {
        Mixer {
            sounds,
            commands,
            voices: Vec::new(),
            music: None,
            stream: None,
            pup: None,
            media: Vec::new(),
            chimes: Vec::new(),
            playfield_volume,
            backglass_volume,
            levels: Levels::default(),
            held: false,
            channels: 2,
            layout: Layout::Stereo,
            rate: f64::from(OUTPUT_RATE),
            buffer: Vec::new(),
            stereo: Vec::new(),
        }
    }

    fn gains_of(&self, v: &Voice) -> [f32; MAX_CHANNELS] {
        voice_gains(&self.sounds[v.sound], v.volume, v.pan, v.fade, self.playfield_volume, self.backglass_volume, self.channels, self.layout)
    }

    fn take_commands(&mut self) {
        while let Ok(c) = self.commands.try_recv() {
            match c {
                Command::Play { sound, volume, pan, fade, rate, loops, use_same, restart } => {
                    let step = f64::from(rate) * f64::from(self.sounds[sound].rate) / self.rate;
                    let gains = voice_gains(&self.sounds[sound], volume, pan, fade, self.playfield_volume, self.backglass_volume, self.channels, self.layout);
                    // restart stops every copy; "use same" changes the copy
                    // playing, if any; otherwise a new copy starts
                    if restart {
                        self.voices.retain(|v| v.sound != sound);
                    }
                    let existing = if use_same { self.voices.iter_mut().find(|v| v.sound == sound) } else { None };
                    match existing {
                        Some(v) => (v.gains, v.step, v.loops, v.volume, v.pan, v.fade) = (gains, step, loops, volume, pan, fade),
                        None => self.voices.push(Voice { sound, at: 0.0, step, gains, loops, volume, pan, fade }),
                    }
                }
                Command::Stop { sound } => self.voices.retain(|v| v.sound != sound),
                Command::Music { stream, volume } => self.music = Some(MusicVoice { stream, volume, gain: music_gain(volume, self.backglass_volume) * self.levels.music, paused: false }),
                Command::PauseMusic => {
                    if let Some(m) = &mut self.music {
                        m.paused = true;
                    }
                }
                Command::MusicVolume(volume) => {
                    if let Some(m) = &mut self.music {
                        (m.volume, m.gain) = (volume, music_gain(volume, self.backglass_volume) * self.levels.music);
                    }
                }
                Command::Stream(source) => self.stream = source,
                Command::PupStream(source) => self.pup = Some(source),
                Command::Media(stream) => self.media.push(stream),
                Command::Chime(stream) => self.chimes.push(stream),
                Command::Volumes { playfield, backglass } => {
                    (self.playfield_volume, self.backglass_volume) = (playfield, backglass);
                    let gains: Vec<[f32; MAX_CHANNELS]> = self.voices.iter().map(|v| self.gains_of(v)).collect();
                    for (v, g) in self.voices.iter_mut().zip(gains) {
                        v.gains = g;
                    }
                    if let Some(m) = &mut self.music {
                        m.gain = music_gain(m.volume, backglass) * self.levels.music;
                    }
                }
                Command::Levels(levels) => {
                    self.levels = levels;
                    if let Some(m) = &mut self.music {
                        m.gain = music_gain(m.volume, self.backglass_volume) * levels.music;
                    }
                }
                Command::Hold(held) => self.held = held,
            }
        }
    }

    /// Adds the music's next frames to the buffer; music that has played
    /// to its end is done.
    fn mix_music(&mut self, frames: usize) {
        let Some(m) = &self.music else { return };
        if m.paused || self.held {
            return;
        }
        let ended = {
            let mut queue = m.stream.queue.lock().unwrap_or_else(|e| e.into_inner());
            let n = (frames * 2).min(queue.len());
            for (out, x) in self.stereo.iter_mut().zip(queue.drain(..n)) {
                *out += x * m.gain;
            }
            queue.is_empty() && m.stream.finished.load(Ordering::Acquire)
        };
        if ended {
            self.music = None;
        }
    }

    /// Mixes `frames` frames of the output's channels into the buffer.
    fn mix(&mut self, frames: usize) {
        self.take_commands();
        let ch = self.channels.clamp(1, MAX_CHANNELS);
        self.buffer.clear();
        self.buffer.resize(frames * ch, 0.0);
        self.stereo.clear();
        self.stereo.resize(frames * 2, 0.0);
        let sounds = &self.sounds;
        // paused: every sound holds where it is
        let held = self.held;
        self.voices.retain_mut(|v| {
            if held {
                return true;
            }
            let s = &sounds[v.sound];
            let n = s.frames();
            if n == 0 {
                return false;
            }
            let sch = usize::from(s.channels);
            for f in 0..frames {
                if v.at >= n as f64 {
                    match &mut v.loops {
                        Some(0) => return false,
                        Some(left) => *left -= 1,
                        None => {}
                    }
                    v.at -= n as f64;
                }
                // linear interpolation between neighbouring frames
                let i = v.at as usize;
                let t = (v.at - i as f64) as f32;
                let j = if i + 1 < n { i + 1 } else if v.loops != Some(0) { 0 } else { i };
                let sample = |frame: usize, c: usize| s.samples[frame * sch + c.min(sch - 1)];
                let l = sample(i, 0) + (sample(j, 0) - sample(i, 0)) * t;
                let out = &mut self.buffer[f * ch..f * ch + ch];
                if s.backglass {
                    // stereo: each side to its channel, both halves together on one
                    let r = sample(i, 1) + (sample(j, 1) - sample(i, 1)) * t;
                    if ch == 1 {
                        out[0] += (l + r) * v.gains[0];
                    } else {
                        out[0] += l * v.gains[0];
                        out[1] += r * v.gains[1];
                    }
                } else {
                    for (o, g) in out.iter_mut().zip(&v.gains) {
                        *o += l * g;
                    }
                }
                v.at += v.step;
            }
            true
        });
        self.mix_music(frames);
        // the streams at the player's backglass volume, as VPX sets their gain, each at its source's
        // level (VPX's audio lanes' mixer volumes)
        let (backglass, l) = (self.backglass_volume, self.levels);
        if let Some(source) = &mut self.stream {
            source.mix_into(&mut self.stereo, backglass * l.pinmame);
        }
        if let Some(source) = &mut self.pup {
            source.mix_into(&mut self.stereo, backglass * l.pup);
        }
        self.media.retain(|m| !m.abandoned.load(Ordering::Acquire));
        for m in &self.media {
            m.mix_into(&mut self.stereo, backglass * l.wmp);
        }
        self.chimes.retain(|m| !m.abandoned.load(Ordering::Acquire));
        for m in &self.chimes {
            m.mix_into(&mut self.stereo, backglass * l.achievements);
        }
        // the music and the streams on the front pair (both halves together on one channel)
        for (f, lr) in self.stereo.chunks_exact(2).enumerate() {
            if ch == 1 {
                self.buffer[f] += 0.5 * (lr[0] + lr[1]);
            } else {
                self.buffer[f * ch] += lr[0];
                self.buffer[f * ch + 1] += lr[1];
            }
        }
        for x in &mut self.buffer {
            *x = x.clamp(-1.0, 1.0);
        }
    }
}

/// The rate the mixer plays at, in frames a second.
pub const OUTPUT_RATE: i32 = 48_000;

/// SDL's audio thread asks for more sound.
unsafe extern "C" fn feed(userdata: *mut c_void, stream: *mut SDL_AudioStream, additional: c_int, _total: c_int) {
    if additional <= 0 {
        return;
    }
    // SAFETY: the mixer lives as long as the audio stream (see `Audio::drop`)
    unsafe {
        let mixer = &mut *(userdata as *mut Mixer);
        let frames = additional as usize / (mixer.channels.clamp(1, MAX_CHANNELS) * size_of::<f32>());
        mixer.mix(frames);
        SDL_PutAudioStreamData(stream, mixer.buffer.as_ptr().cast(), (mixer.buffer.len() * size_of::<f32>()) as c_int);
    }
}

/// A table's sound recorded as it plays, with no output (the player's test mode, for a table's
/// media): VPX's two mixers, the playfield's and the backglass's, each in stereo, mixed in step
/// with the clock on a thread of their own and added together; what they mix kept from `record`'s
/// call for as long as it asks.
pub struct Recorder {
    shared: Arc<Recording>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[derive(Default)]
struct Recording {
    /// Stereo frames still to keep.
    wanted: AtomicU64,
    kept: Mutex<Vec<f32>>,
    quit: AtomicBool,
}

/// The two mixers, moved to the recorder's thread and used there alone, as SDL uses an output's
/// mixer on its own thread.
struct Mixers(Mixer, Mixer);

// SAFETY: moved to the recorder's thread once, used there alone
unsafe impl Send for Mixers {}

impl Recorder {
    /// The table's sounds connected to two mixers of the recorder's (the playfield's, the
    /// backglass's), mixed from now on.
    pub fn start(sounds: &mut Sounds) -> Recorder {
        let (playfield_tx, playfield_rx) = channel();
        let (backglass_tx, backglass_rx) = channel();
        let mixers = Mixers(
            Mixer::new(sounds.sounds.clone(), playfield_rx, sounds.playfield_volume, sounds.backglass_volume),
            Mixer::new(sounds.sounds.clone(), backglass_rx, sounds.playfield_volume, sounds.backglass_volume),
        );
        sounds.connect(playfield_tx, backglass_tx);
        let shared = Arc::new(Recording::default());
        let on = shared.clone();
        let thread = std::thread::Builder::new()
            .name("sound recorder".into())
            .spawn(move || {
                let Mixers(mut playfield, mut backglass) = mixers;
                let start = std::time::Instant::now();
                let mut mixed = 0u64;
                while !on.quit.load(Ordering::Acquire) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    let due = (start.elapsed().as_secs_f64() * f64::from(OUTPUT_RATE)) as u64;
                    while mixed < due {
                        let n = (due - mixed).min(1024) as usize;
                        playfield.mix(n);
                        backglass.mix(n);
                        let wanted = on.wanted.load(Ordering::Acquire);
                        if wanted > 0 {
                            let take = (n as u64).min(wanted) as usize;
                            let both = playfield.buffer[..take * 2].iter().zip(&backglass.buffer[..take * 2]);
                            // not clipped here: `even_out` brings the peaks under full scale
                            on.kept.lock().unwrap_or_else(|e| e.into_inner()).extend(both.map(|(p, b)| p + b));
                            on.wanted.fetch_sub(take as u64, Ordering::AcqRel);
                        }
                        mixed += n as u64;
                    }
                }
            })
            .ok();
        Recorder { shared, thread }
    }

    /// Keep what's mixed from now on, `seconds` of it.
    pub fn record(&self, seconds: u32) {
        self.shared.wanted.store(u64::from(seconds) * OUTPUT_RATE as u64, Ordering::Release);
    }

    /// What was kept: stereo floats, left first, at the mixers' rate (`OUTPUT_RATE`), as mixed
    /// (peaks past full scale too); the mixing stopped.
    pub fn finish(mut self) -> Vec<f32> {
        self.stop();
        std::mem::take(&mut *self.shared.kept.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn stop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The loudness a recording is set to, in LUFS: every table's alike, whatever its own mix.
pub const RECORDING_LOUDNESS: f64 = -16.0;

/// A recording (stereo floats, left first, at `OUTPUT_RATE`) set to `RECORDING_LOUDNESS`, its
/// peaks kept under -1 dBFS (the gain smaller where they'd pass it), then clipped at full scale;
/// one all but silent, or shorter than a block, only clipped. Its loudness as recorded and the
/// gain given it, in dB, when it had one.
pub fn even_out(samples: &mut [f32]) -> Option<(f64, f64)> {
    let measured = loudness(samples).map(|loudness| {
        let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
        let gain = 10f64.powf((RECORDING_LOUDNESS - loudness) / 20.0).min(0.891 / f64::from(peak));
        for s in samples.iter_mut() {
            *s *= gain as f32;
        }
        (loudness, 20.0 * gain.log10())
    });
    for s in samples.iter_mut() {
        *s = s.clamp(-1.0, 1.0);
    }
    measured
}

/// A stereo recording's loudness at 48 kHz, in LUFS, as ITU-R BS.1770-4 measures it: each
/// channel K-weighted (its high shelf, then its high pass, by the standard's coefficients for
/// 48 kHz), the mean square of 400 ms blocks a block every 100 ms, the blocks under -70 LUFS left
/// out and then those 10 LU under the rest. None for a recording all but silent.
pub fn loudness(samples: &[f32]) -> Option<f64> {
    const SHELF: ([f64; 3], [f64; 2]) = ([1.535_124_859_586_97, -2.691_696_189_406_38, 1.198_392_810_852_85], [-1.690_659_293_182_41, 0.732_480_774_215_85]);
    const HIGH_PASS: ([f64; 3], [f64; 2]) = ([1.0, -2.0, 1.0], [-1.990_047_454_833_98, 0.990_072_250_366_21]);
    const BLOCK: usize = 19_200;
    const STEP: usize = 4_800;
    let frames = samples.len() / 2;
    // the weighted squares of both channels added, summed from the start: a block's in two looks
    let mut sums = Vec::with_capacity(frames + 1);
    sums.push(0f64);
    // each channel's two filters: their last two inputs and outputs
    let mut state = [[[0f64; 4]; 2]; 2];
    let mut total = 0f64;
    for i in 0..frames {
        for (ch, filters) in state.iter_mut().enumerate() {
            let mut x = f64::from(samples[i * 2 + ch]);
            for (f, (b, a)) in filters.iter_mut().zip([SHELF, HIGH_PASS]) {
                let [x1, x2, y1, y2] = *f;
                let y = b[0] * x + b[1] * x1 + b[2] * x2 - a[0] * y1 - a[1] * y2;
                *f = [x, x1, y, y1];
                x = y;
            }
            total += x * x;
        }
        sums.push(total);
    }
    let lufs = |z: f64| -0.691 + 10.0 * z.log10();
    let blocks: Vec<f64> = (0..).map(|k| k * STEP).take_while(|s| s + BLOCK <= frames).map(|s| (sums[s + BLOCK] - sums[s]) / BLOCK as f64).filter(|&z| z > 0.0 && lufs(z) > -70.0).collect();
    if blocks.is_empty() {
        return None;
    }
    let relative = lufs(blocks.iter().sum::<f64>() / blocks.len() as f64) - 10.0;
    let kept: Vec<f64> = blocks.into_iter().filter(|&z| lufs(z) > relative).collect();
    Some(lufs(kept.iter().sum::<f64>() / kept.len() as f64))
}

/// The open audio device, playing the mixer.
pub struct Audio {
    stream: *mut SDL_AudioStream,
    mixer: *mut Mixer,
}

/// The sound outputs SDL knows, by their names (VPX names them so in its settings: the only
/// stable thing about them).
pub fn output_names() -> Vec<String> {
    let mut names = Vec::new();
    // SAFETY: SDL's audio is started; the list is freed once read
    unsafe {
        let mut count = 0;
        let ids = SDL_GetAudioPlaybackDevices(&mut count);
        if ids.is_null() {
            return names;
        }
        for i in 0..count.max(0) as usize {
            let name = SDL_GetAudioDeviceName(*ids.add(i));
            if !name.is_null() {
                names.push(std::ffi::CStr::from_ptr(name).to_string_lossy().into_owned());
            }
        }
        SDL_free(ids.cast());
    }
    names
}

/// How many channels the output of that name (else the default) mixes for, as Windows reports
/// it: 2 for stereo speakers or headphones, 6 for 5.1, 8 for 7.1. Needs SDL's audio started.
pub fn output_channels(name: Option<&str>) -> Option<i32> {
    device_channels(output_named(name))
}

fn device_channels(device: SDL_AudioDeviceID) -> Option<i32> {
    // SAFETY: SDL's audio is started; the spec is written by SDL
    unsafe {
        let mut spec: SDL_AudioSpec = std::mem::zeroed();
        SDL_GetAudioDeviceFormat(device, &mut spec, std::ptr::null_mut()).then_some(spec.channels)
    }
}

/// The output of that name (VPX's `AudioPlayer` finds its devices by name), else the default.
fn output_named(name: Option<&str>) -> SDL_AudioDeviceID {
    let Some(name) = name.filter(|n| !n.is_empty()) else { return SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK };
    // SAFETY: SDL's audio is started; the list is freed once read
    unsafe {
        let mut count = 0;
        let ids = SDL_GetAudioPlaybackDevices(&mut count);
        if ids.is_null() {
            return SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK;
        }
        let found = (0..count.max(0) as usize).map(|i| *ids.add(i)).find(|&id| {
            let n = SDL_GetAudioDeviceName(id);
            !n.is_null() && std::ffi::CStr::from_ptr(n).to_string_lossy() == name
        });
        SDL_free(ids.cast());
        found.unwrap_or(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK)
    }
}

impl Audio {
    /// Opens a sound output — the one named (VPX's `[Player]` `SoundDevice` for the playfield,
    /// `SoundDeviceBG` for the backglass), else the default, as VPX falls back to it — with a
    /// small buffer (256 frames, about 5 ms) for prompt sound, and starts it. The playfield's
    /// (`layout` given) is mixed for the device's own channels, its sounds laid out among them,
    /// as VPX opens its playfield output; the backglass's in stereo. Needs SDL's audio started.
    pub fn open(sounds: &Sounds, output: Option<&str>, layout: Option<Layout>) -> Result<(Audio, Sender<Command>), String> {
        let (tx, rx) = channel();
        let device = output_named(output);
        if output.is_some_and(|n| !n.is_empty()) && device == SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK {
            eprintln!("vpi-player: the sound output \"{}\" wasn't found; the default plays (vpi-player --sound-outputs lists them)", output.unwrap_or_default());
        }
        let mut m = Mixer::new(sounds.sounds.clone(), rx, sounds.playfield_volume, sounds.backglass_volume);
        if let Some(layout) = layout {
            let native = device_channels(device).unwrap_or(2);
            (m.channels, m.layout) = ((native.clamp(1, MAX_CHANNELS as i32)) as usize, layout);
        }
        let channels = m.channels as i32;
        let mixer = Box::into_raw(Box::new(m));
        // SAFETY: SDL calls `feed` with the mixer until the stream is destroyed
        unsafe {
            SDL_SetHint(SDL_HINT_AUDIO_DEVICE_SAMPLE_FRAMES, c"256".as_ptr());
            let spec = SDL_AudioSpec { format: SDL_AUDIO_F32, channels, freq: OUTPUT_RATE };
            let stream = SDL_OpenAudioDeviceStream(device, &spec, Some(feed), mixer.cast());
            if stream.is_null() {
                drop(Box::from_raw(mixer));
                return Err(format!("open the sound output: {}", crate::render::sdl_error()));
            }
            SDL_ResumeAudioStreamDevice(stream);
            Ok((Audio { stream, mixer }, tx))
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        // SAFETY: destroying the stream stops the callback before the mixer goes
        unsafe {
            SDL_DestroyAudioStream(self.stream);
            drop(Box::from_raw(self.mixer));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WAV file of `seconds` of a 16-bit mono tone at 48 kHz.
    fn tone_wav(seconds: f32) -> Vec<u8> {
        let n = (48_000.0 * seconds) as u32;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + n * 2).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&48_000u32.to_le_bytes());
        b.extend_from_slice(&96_000u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(n * 2).to_le_bytes());
        for i in 0..n {
            b.extend_from_slice(&((((i as f32) * 0.05).sin() * 8000.0) as i16).to_le_bytes());
        }
        b
    }

    /// A media file plays only when asked, plays to its end and stops, and starts over when
    /// played again (VPX's WMP audio player).
    #[test]
    fn a_media_file_plays_to_its_end_and_starts_over() {
        let path = std::env::temp_dir().join(format!("vpi-media-test-{}.wav", std::process::id()));
        std::fs::write(&path, tone_wav(0.25)).unwrap();
        let m = MediaStream::load(path.clone(), 0.5);
        let mut out = vec![0.0f32; 2 * 480];
        // nothing before Play
        m.mix_into(&mut out, 1.0);
        assert!(out.iter().all(|&x| x == 0.0));
        m.play();
        let mut rounds = 0;
        while m.is_playing() && rounds < 2000 {
            out.iter_mut().for_each(|x| *x = 0.0);
            m.mix_into(&mut out, 1.0);
            if out.iter().all(|&x| x == 0.0) {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            rounds += 1;
        }
        assert!(!m.is_playing(), "it stops at its end");
        assert!((m.position() - 0.25).abs() < 0.02, "played to {} s", m.position());
        m.play();
        assert!(m.is_playing());
        assert!(m.position() < 0.01, "starts over");
        m.stop();
        assert!(!m.is_playing() && m.position() == 0.0);
        m.close();
        let _ = std::fs::remove_file(&path);
    }

    const EXAMPLE: &str = r"C:\vPinball\VisualPinball\assets\exampleTable.vpx";

    /// The example table's sounds decode, and a flipper sound mixed off-line
    /// comes out at VPX's loudness: the square root of its volume, half on
    /// each side when centered.
    #[test]
    fn example_table_sounds_mix() {
        let Ok(vpx) = vpin::vpx::read(std::path::Path::new(EXAMPLE)) else {
            eprintln!("skipped: {EXAMPLE} not found");
            return;
        };
        let mut sounds = Sounds::load(&vpx);
        assert!(!sounds.sounds.is_empty(), "no sounds decoded");
        println!("{} of {} sounds decoded", sounds.sounds.len(), vpx.sounds.len());
        let (tx, rx) = channel();
        sounds.connect(tx.clone(), tx);
        let index = sounds.by_name["fx_flipperup"];
        let own = sounds.sounds[index].volume;
        sounds.play("fx_flipperup", 0, 0.67, 0.0, 0.0, 0.0, 0, false, true, || 0.0);
        let mut mixer = Mixer::new(sounds.sounds.clone(), rx, 1.0, 1.0);
        mixer.mix(4800);
        assert_eq!(mixer.voices.len(), 1, "the sound should still be playing after 0.1 s");
        let gain = mixer.voices[0].gains[0];
        let s = &sounds.sounds[index];
        let expected = s.mono_compensation * (0.67 + own).clamp(0.0, 1.0).sqrt() * 0.5;
        assert!((gain - expected).abs() < 1e-3, "gain {gain}, expected {expected}");
        assert_eq!(mixer.voices[0].gains[0], mixer.voices[0].gains[1]);
        let peak = mixer.buffer.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.01, "silent mix: peak {peak}");
        println!("gain {gain:.3}, peak {peak:.3}");
    }

    /// Music at half the output rate comes out twice as long, its frames
    /// interpolated, with no seam where one packet ends and the next begins;
    /// a mono file plays on both sides.
    #[test]
    fn music_resamples_across_packets() {
        let mut r = Resampler::default();
        let mut out = Vec::new();
        let ramp: Vec<f32> = (0..200).map(|i| i as f32).collect();
        r.push(OUTPUT_RATE as u32 / 2, 1, &ramp[..100], &mut out);
        r.push(OUTPUT_RATE as u32 / 2, 1, &ramp[100..], &mut out);
        let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        assert!(out.chunks(2).all(|f| f[0] == f[1]), "mono should play on both sides");
        assert_eq!(left.len(), 398, "two frames out per frame in, less the last's neighbour");
        for (k, v) in left.iter().enumerate() {
            assert!((v - k as f32 * 0.5).abs() < 1e-4, "frame {k}: {v}");
        }
    }

    /// Decoded music plays at its volume's loudness, and is done once played.
    #[test]
    fn music_plays_once() {
        let stream = Arc::new(MusicStream { queue: Mutex::new(vec![0.5; 2 * 100].into()), finished: AtomicBool::new(true), abandoned: AtomicBool::new(false) });
        let (tx, rx) = channel();
        let sounds = Sounds { sounds: Vec::new(), by_name: HashMap::new(), playfield: Some(tx.clone()), backglass: Some(tx), playfield_volume: 1.0, backglass_volume: 1.0, ..Sounds::none_played() };
        sounds.send(Command::Music { stream: stream.clone(), volume: 0.25 });
        let mut mixer = Mixer::new(Vec::new(), rx, 1.0, 1.0);
        mixer.mix(60);
        assert!((mixer.buffer[0] - 0.25).abs() < 1e-6, "the square root of 0.25 times 0.5: {}", mixer.buffer[0]);
        mixer.mix(60);
        assert_eq!(mixer.buffer[2 * 40], 0.0, "only 100 frames to play");
        assert!(mixer.music.is_none(), "played to its end");
        assert!(stream.abandoned.load(Ordering::Acquire), "its decoding is told to stop");
    }

    /// A real MP3 decodes, and streams to the output rate.
    #[test]
    fn mp3_music_streams() {
        const MP3: &str = r"C:\vPinball\PinUPSystem\PUPVideos\DarkestDungeon\audioclear\clear.mp3";
        if !std::path::Path::new(MP3).is_file() {
            eprintln!("skipped: {MP3} not found");
            return;
        }
        let pcm = crate::decode::decode_all(std::fs::read(MP3).expect("read"), Some("mp3")).expect("decodes");
        let seconds = pcm.samples.len() as f64 / pcm.channels as f64 / f64::from(pcm.rate);
        assert!(seconds > 0.1, "{seconds} s");
        let stream = MusicStream::start(MP3.into());
        let started = std::time::Instant::now();
        while !stream.finished.load(Ordering::Acquire) {
            assert!(started.elapsed().as_secs() < 10, "decoding took too long");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let queued = stream.queue.lock().unwrap().len() as f64 / 2.0 / f64::from(OUTPUT_RATE);
        assert!((queued - seconds).abs() < 0.01, "streamed {queued} s of {seconds} s");
        println!("{:.2} s at {} Hz, {} channels", seconds, pcm.rate, pcm.channels);
    }

    /// A mono sound at 48 kHz, `hundredths` of a second long.
    fn test_sound(hundredths: usize) -> Arc<Sound> {
        Arc::new(Sound {
            name: "t".into(),
            rate: 48_000,
            channels: 1,
            samples: vec![0.5; 480 * hundredths],
            volume: 0.0,
            balance: 0.0,
            fade: 0.0,
            backglass: false,
            mono_compensation: 1.0,
        })
    }

    /// VPX's playfield pan: a pan of 0.5 (as tables compute it, a tenth
    /// power of the position) is softened to 0.5^0.1 before splitting.
    #[test]
    fn pan_splits_between_the_speakers() {
        let sound = test_sound(1);
        let g = voice_gains(&sound, 1.0, 0.5, 0.0, 1.0, 1.0, 2, Layout::Stereo);
        let (left, right) = (g[0], g[1]);
        let pan = 0.5f32.powf(0.1);
        assert!((left - 0.5 * (1.0 - pan)).abs() < 1e-6, "left {left}");
        assert!((right - 0.5 * (1.0 + pan)).abs() < 1e-6, "right {right}");
        // the play command carries the script's volume and pan, the rate and the repeats
        let mut sounds = Sounds { sounds: vec![sound], by_name: HashMap::from([("t".to_string(), 0)]), playfield: None, backglass: None, playfield_volume: 1.0, backglass_volume: 1.0, ..Sounds::none_played() };
        let (tx, rx) = channel();
        sounds.connect(tx.clone(), tx);
        sounds.play("t", 3, 1.0, 0.5, 0.0, 0.0, 0, false, true, || 0.0);
        let play = rx.try_iter().find_map(|c| match c {
            Command::Play { volume, pan, rate, loops, .. } => Some((volume, pan, rate, loops)),
            _ => None,
        });
        let (volume, pan, rate, loops) = play.expect("a play command");
        assert_eq!((volume, pan, loops), (1.0, 0.5, Some(2)));
        assert!((rate - 1.0).abs() < 1e-6);
    }

    /// A playfield sound among an output's speakers, by VPX's layouts and channel counts: stereo on
    /// the front pair; quad faded between front and back by its place; SSF on the last pair of
    /// a quad output, keeping the front free; one channel all of it; and VPX's six-speaker way's
    /// silence for a sound in the middle played as on four speakers.
    #[test]
    fn playfield_sounds_go_to_the_speakers_their_layout_gives() {
        let s = test_sound(1);
        let g = |fade: f32, channels: usize, layout: Layout| voice_gains(&s, 1.0, 0.0, fade, 1.0, 1.0, channels, layout);
        // stereo: half each side
        assert_eq!(&g(0.0, 2, Layout::Stereo)[..2], [0.5, 0.5]);
        // quad, the sound at the front (1, softened stays 1): front pair only
        assert_eq!(&g(1.0, 4, Layout::FrontIsFront)[..4], [0.5, 0.5, 0.0, 0.0]);
        // the same with the playfield's front at the back
        assert_eq!(&g(1.0, 4, Layout::FrontIsRear)[..4], [0.0, 0.0, 0.5, 0.5]);
        // SSF on quad: the back pair, the front kept free
        assert_eq!(&g(0.3, 4, Layout::Ssf)[..4], [0.0, 0.0, 0.5, 0.5]);
        // 7.1, stereo layout: VPX's six-speaker way would send nothing; front and back halved
        assert_eq!(g(0.0, 8, Layout::Stereo), [0.25, 0.25, 0.0, 0.0, 0.25, 0.25, 0.0, 0.0]);
        // 7.1, a sound at the back: the back pair (SL SR) and the middle pair
        assert_eq!(g(-1.0, 8, Layout::FrontIsFront), [0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 0.5]);
        // one channel: all of it
        assert_eq!(g(0.0, 1, Layout::Stereo)[0], 1.0);
    }

    /// The playfield's sounds go to its output's mixer, the backglass's sounds and the music to
    /// the backglass's, the volumes to both (VPX 5436's two outputs).
    #[test]
    fn each_sound_goes_to_its_outputs_mixer() {
        let backglass_sound = Arc::new(Sound {
            name: "bg".into(),
            rate: 48_000,
            channels: 1,
            samples: vec![0.5; 480],
            volume: 0.0,
            balance: 0.0,
            fade: 0.0,
            backglass: true,
            mono_compensation: 1.0,
        });
        let mut sounds = Sounds {
            sounds: vec![test_sound(1), backglass_sound],
            by_name: HashMap::from([("pf".to_string(), 0), ("bg".to_string(), 1)]),
            playfield: None,
            backglass: None,
            playfield_volume: 1.0,
            backglass_volume: 1.0,
            ..Sounds::none_played()
        };
        let ((ptx, prx), (btx, brx)) = (channel(), channel());
        sounds.connect(ptx, btx);
        sounds.play("pf", 0, 1.0, 0.0, 0.0, 0.0, 0, false, true, || 0.0);
        sounds.play("bg", 0, 1.0, 0.0, 0.0, 0.0, 0, false, true, || 0.0);
        sounds.set_music_volume(0.5);
        let names = |rx: &Receiver<Command>| {
            rx.try_iter()
                .map(|c| match c {
                    Command::Play { sound, .. } => format!("play {sound}"),
                    Command::Volumes { .. } => "volumes".into(),
                    Command::MusicVolume(_) => "music".into(),
                    Command::Levels(_) => "levels".into(),
                    _ => "other".into(),
                })
                .collect::<Vec<_>>()
        };
        // the sources' levels go to the backglass's mixer, which plays them
        assert_eq!(names(&prx), ["volumes", "play 0"]);
        assert_eq!(names(&brx), ["volumes", "levels", "play 1", "music"]);
    }

    /// A change of the player's volumes reaches what's playing already, as VPX's
    /// `SetMainVolume` does: a sound at a quarter of the volume plays at half the loudness (the
    /// square root), the backglass's music too.
    #[test]
    fn volumes_apply_to_what_plays() {
        let mut sounds = Sounds { sounds: vec![test_sound(100)], by_name: HashMap::from([("t".to_string(), 0)]), playfield: None, backglass: None, playfield_volume: 1.0, backglass_volume: 1.0, ..Sounds::none_played() };
        let (tx, rx) = channel();
        sounds.connect(tx.clone(), tx);
        sounds.play("t", -1, 1.0, 0.0, 0.0, 0.0, 0, false, true, || 0.0);
        let music = Arc::new(MusicStream { queue: Mutex::new(vec![0.5; 2 * 1000].into()), finished: AtomicBool::new(true), abandoned: AtomicBool::new(false) });
        sounds.send(Command::Music { stream: music, volume: 1.0 });
        let mut mixer = Mixer::new(sounds.sounds.clone(), rx, 1.0, 1.0);
        mixer.mix(10);
        let (sound, tune) = (mixer.voices[0].gains[0], mixer.music.as_ref().expect("music").gain);
        assert_eq!((sound, tune), (0.5, 1.0));
        sounds.set_volumes(0.25, 0.25);
        mixer.mix(10);
        assert_eq!((mixer.voices[0].gains[0], mixer.music.as_ref().expect("music").gain), (0.25, 0.5));
        // a sound started later plays at the new volume too
        sounds.play("t", -1, 1.0, 0.0, 0.0, 0.0, 0, false, false, || 0.0);
        mixer.mix(10);
        assert_eq!(mixer.voices.iter().map(|v| v.gains[0]).collect::<Vec<_>>(), [0.25, 0.25]);
    }

    /// `seconds` of a 997 Hz tone at `amplitude` in both channels, at 48 kHz.
    fn stereo_tone(seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(48_000.0 * seconds) as usize).flat_map(|i| [((i * 997 % 48_000) as f32 / 48_000.0 * std::f32::consts::TAU).sin() * amplitude; 2]).collect()
    }

    /// BS.1770's own check: a 997 Hz tone at full scale in both channels measures 0 LUFS, and so
    /// 20 dB down measures -20; silence measures nothing.
    #[test]
    fn a_recordings_loudness_as_bs1770_measures_it() {
        let measured = loudness(&stereo_tone(3.0, 0.1)).expect("a loudness");
        assert!((measured + 20.0).abs() < 0.05, "{measured}");
        assert_eq!(loudness(&vec![0.0; 2 * 48_000]), None);
    }

    /// A quiet recording and a loud one come out equally loud; one whose peaks would pass -1 dBFS
    /// stops there.
    #[test]
    fn recordings_come_out_equally_loud() {
        for amplitude in [0.01, 0.5] {
            let mut tone = stereo_tone(3.0, amplitude);
            even_out(&mut tone);
            let measured = loudness(&tone).expect("a loudness");
            assert!((measured - RECORDING_LOUDNESS).abs() < 0.05, "{amplitude}: {measured}");
        }
        // a click 40 dB over a quiet tone: its peak, not the loudness, sets the gain
        let mut clicked = stereo_tone(3.0, 0.005);
        clicked[48_000] = 1.5;
        even_out(&mut clicked);
        let peak = clicked.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((peak - 0.891).abs() < 0.001, "{peak}");
    }
}
