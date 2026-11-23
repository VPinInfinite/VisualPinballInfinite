//! A screen's file playing (ours): decoded through the bridge to FFmpeg (`av`) on a
//! thread of its own, a little ahead of where it's heard and seen. Its sound goes into the
//! mixer with every other screen's, as one stream on the backglass speakers (VPX streams a
//! pack's sound there); its pictures are kept for the screens' drawing, each shown from its
//! moment in the file, the file's moment following the game's clock (as VPX hands its media
//! players the game's time). A file that loops goes back to its start at its end.
//!
//! A file is decoded only where it's heard or seen: its sound where the player has sound
//! output (not in test mode or the headless runner), its pictures where its screen has a
//! window. Otherwise the pack's state runs on the lengths in the files' headers alone
//! (`screen::Player`).

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::Duration;

use super::av::{Item, Media};
use crate::audio::{OUTPUT_RATE, StreamSource};

/// Sound decoded ahead of the mixer: half a second (stereo samples).
const SOUND_AHEAD: usize = OUTPUT_RATE as usize;
/// Sound held at most while pictures are wanted further on in the file: two seconds.
const SOUND_MOST: usize = 4 * OUTPUT_RATE as usize;
/// Pictures decoded ahead of the one showing.
const PICTURES_AHEAD: usize = 3;

/// One picture of a file, as decoded: RGBA, rows top first.
pub struct Picture {
    /// Unique to this picture, for knowing when a screen's picture changed.
    pub id: u64,
    /// Its moment in the file's playing, loops included (ms).
    pub at_ms: i64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

static NEXT_PICTURE: AtomicU64 = AtomicU64::new(1);

/// Whether pictures are waited for (test mode): each frame then shows exactly the picture of its
/// moment, so a test picture repeats.
static EXACT: AtomicBool = AtomicBool::new(false);

/// Test mode: every frame waits for its pictures to be decoded.
pub fn set_exact(exact: bool) {
    EXACT.store(exact, Ordering::Release);
}

/// Whether frames wait for their pictures (test mode).
pub fn exact() -> bool {
    EXACT.load(Ordering::Acquire)
}

/// What a playing file's thread and its listeners share.
struct Shared {
    queue: Mutex<Queue>,
    /// Signalled when the mixer takes sound, the file's moment moves on, or it's stopped.
    room: Condvar,
    /// Signalled when a picture is decoded, or the file ends.
    decoded: Condvar,
    stop: AtomicBool,
    looping: AtomicBool,
    paused: AtomicBool,
    /// Its loudness, 0 to 1 (an f32's bits).
    gain: AtomicU32,
    /// Where in the file the screen is (ms, loops included), from the game's clock.
    position: AtomicI64,
    /// Heard: its sound is decoded.
    audio: bool,
    /// Seen: its pictures are decoded.
    video: bool,
    /// Opened, it turned out to have no pictures, or it couldn't be opened.
    no_pictures: AtomicBool,
}

#[derive(Default)]
struct Queue {
    /// Stereo frames at the mixer's rate, interleaved.
    sound: VecDeque<f32>,
    /// Pictures from the one showing on, oldest first.
    pictures: VecDeque<Arc<Picture>>,
    /// The file is decoded to its end (and doesn't loop), or couldn't be read.
    finished: bool,
}

impl Queue {
    /// Drops the pictures whose moment has passed, keeping the one showing at `position`.
    fn prune(&mut self, position: i64) {
        while self.pictures.len() > 1 && self.pictures[1].at_ms <= position {
            self.pictures.pop_front();
        }
    }

    fn pictures_ahead(&self, position: i64) -> usize {
        self.pictures.iter().filter(|p| p.at_ms > position).count()
    }
}

/// A sound of a pack's sound bank playing (`sound::SoundBank`): stereo at the mixer's rate,
/// its loudness on each side, round and round where it loops.
struct Voice {
    id: u64,
    samples: Arc<Vec<f32>>,
    position: usize,
    gains: [f32; 2],
    looping: bool,
}

static NEXT_VOICE: AtomicU64 = AtomicU64::new(1);

/// Every file playing, and the sound bank's sounds: the mixer reads their sound through this.
#[derive(Default)]
pub struct Sink {
    playing: Mutex<Vec<Weak<Shared>>>,
    voices: Mutex<Vec<Voice>>,
    /// The mixer has it: files' sound may decode.
    connected: AtomicBool,
}

/// The one sink, which the player hands its mixer when a pack starts.
pub fn sink() -> &'static Arc<Sink> {
    static SINK: OnceLock<Arc<Sink>> = OnceLock::new();
    SINK.get_or_init(Arc::default)
}

impl Sink {
    /// The stream the mixer plays the packs' sound from. Once connected, files' sound decodes.
    pub fn source(self: &Arc<Sink>) -> Box<dyn StreamSource> {
        self.connected.store(true, Ordering::Release);
        Box::new(SinkSource(self.clone()))
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    /// Plays a sound of a pack's sound bank, at `gains` (left, right); its number, or None where
    /// no sound is heard (test mode, the headless runner).
    pub fn play_voice(&self, samples: Arc<Vec<f32>>, gains: [f32; 2], looping: bool) -> Option<u64> {
        if !self.is_connected() || samples.len() < 2 {
            return None;
        }
        let id = NEXT_VOICE.fetch_add(1, Ordering::Relaxed);
        self.voices.lock().unwrap_or_else(|e| e.into_inner()).push(Voice { id, samples, position: 0, gains, looping });
        Some(id)
    }

    /// A playing sound's new loudness.
    pub fn set_voice_gains(&self, id: u64, gains: [f32; 2]) {
        if let Some(v) = self.voices.lock().unwrap_or_else(|e| e.into_inner()).iter_mut().find(|v| v.id == id) {
            v.gains = gains;
        }
    }

    pub fn stop_voice(&self, id: u64) {
        self.voices.lock().unwrap_or_else(|e| e.into_inner()).retain(|v| v.id != id);
    }
}

struct SinkSource(Arc<Sink>);

impl StreamSource for SinkSource {
    fn mix_into(&mut self, out: &mut [f32], gain: f32) {
        let mut playing = self.0.playing.lock().unwrap_or_else(|e| e.into_inner());
        playing.retain(|w| {
            let Some(s) = w.upgrade() else { return false };
            if s.paused.load(Ordering::Acquire) || s.stop.load(Ordering::Acquire) {
                return true;
            }
            let g = f32::from_bits(s.gain.load(Ordering::Acquire)) * gain;
            let mut q = s.queue.lock().unwrap_or_else(|e| e.into_inner());
            let n = out.len().min(q.sound.len());
            for (o, x) in out.iter_mut().zip(q.sound.drain(..n)) {
                *o += x * g;
            }
            drop(q);
            s.room.notify_one();
            true
        });
        drop(playing);
        let mut voices = self.0.voices.lock().unwrap_or_else(|e| e.into_inner());
        voices.retain_mut(|v| {
            for frame in out.chunks_exact_mut(2) {
                if v.position + 1 >= v.samples.len() {
                    if !v.looping {
                        return false;
                    }
                    v.position = 0;
                }
                frame[0] += v.samples[v.position] * v.gains[0] * gain;
                frame[1] += v.samples[v.position + 1] * v.gains[1] * gain;
                v.position += 2;
            }
            true
        });
    }
}

/// One file playing on a screen.
pub struct Playback {
    shared: Arc<Shared>,
}

impl Playback {
    /// Starts playing `path` at `volume` (0 to 100), its pictures decoded when `seen`; None
    /// where it would be neither heard nor seen, or FFmpeg's libraries are missing.
    pub fn start(path: &Path, volume: f32, looping: bool, seen: bool) -> Option<Playback> {
        let sink = sink();
        let heard = sink.is_connected();
        if !(heard || seen) || !super::av::available() {
            return None;
        }
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            room: Condvar::new(),
            decoded: Condvar::new(),
            stop: AtomicBool::new(false),
            looping: AtomicBool::new(looping),
            paused: AtomicBool::new(false),
            gain: AtomicU32::new(gain_of(volume).to_bits()),
            position: AtomicI64::new(0),
            audio: heard,
            video: seen,
            no_pictures: AtomicBool::new(false),
        });
        if heard {
            sink.playing.lock().unwrap_or_else(|e| e.into_inner()).push(Arc::downgrade(&shared));
        }
        let s = shared.clone();
        let path = path.to_path_buf();
        let spawned = std::thread::Builder::new().name("pup media".into()).spawn(move || decode(&path, &s));
        if spawned.is_err() {
            return None;
        }
        Some(Playback { shared })
    }

    pub fn set_volume(&self, volume: f32) {
        self.shared.gain.store(gain_of(volume).to_bits(), Ordering::Release);
    }

    /// Whether its pictures are decoded.
    pub fn shows_pictures(&self) -> bool {
        self.shared.video
    }

    /// Whether it turned out to have no pictures to show: a file of sound only, or one that
    /// couldn't be opened.
    pub fn has_no_pictures(&self) -> bool {
        self.shared.no_pictures.load(Ordering::Acquire)
    }

    pub fn set_looping(&self, looping: bool) {
        self.shared.looping.store(looping, Ordering::Release);
    }

    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Release);
    }

    /// The screen's moment in the file (ms, loops included), as the game's clock has it.
    pub fn set_position(&self, ms: i64) {
        let s = &self.shared;
        if s.position.swap(ms, Ordering::AcqRel) != ms {
            s.room.notify_one();
        }
    }

    /// The picture showing now: the last whose moment has come (in test mode, waiting until it's
    /// decoded); None before the first, or where pictures aren't decoded.
    pub fn picture(&self) -> Option<Arc<Picture>> {
        let s = &self.shared;
        if !s.video {
            return None;
        }
        let position = s.position.load(Ordering::Acquire);
        let mut q = s.queue.lock().unwrap_or_else(|e| e.into_inner());
        if EXACT.load(Ordering::Acquire) {
            // until a picture past this moment is decoded, or the file has ended
            let mut waited = Duration::ZERO;
            while !q.finished && !s.stop.load(Ordering::Acquire) && q.pictures.back().is_none_or(|p| p.at_ms <= position) && waited < Duration::from_secs(5) {
                s.room.notify_one();
                let (g, _) = s.decoded.wait_timeout(q, Duration::from_millis(20)).unwrap_or_else(|e| e.into_inner());
                q = g;
                waited += Duration::from_millis(20);
            }
        }
        q.prune(position);
        let shown = q.pictures.front().filter(|p| p.at_ms <= position).cloned();
        drop(q);
        s.room.notify_one();
        shown
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.room.notify_all();
        self.shared.decoded.notify_all();
    }
}

/// A volume in percent as a gain.
fn gain_of(volume: f32) -> f32 {
    (volume / 100.0).max(0.0)
}

/// The file's thread: decodes while what's heard or seen needs more ahead, round again at its
/// end while it loops, until it's stopped.
fn decode(path: &Path, s: &Shared) {
    let Some(mut media) = Media::open(path, s.video, s.audio, OUTPUT_RATE as u32) else {
        s.no_pictures.store(true, Ordering::Release);
        s.queue.lock().unwrap_or_else(|e| e.into_inner()).finished = true;
        s.decoded.notify_all();
        return;
    };
    if media.info.video.is_none() {
        s.no_pictures.store(true, Ordering::Release);
    }
    let frame_ms = if media.info.fps > 0.0 { (1000.0 / media.info.fps) as i64 } else { 40 };
    // added to each picture's moment, a file's length more each time round
    let mut offset = 0i64;
    let mut last = 0i64;
    loop {
        if s.stop.load(Ordering::Acquire) {
            return;
        }
        // wait while enough is decoded ahead of what's heard and seen
        {
            let mut q = s.queue.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if s.stop.load(Ordering::Acquire) {
                    return;
                }
                let position = s.position.load(Ordering::Acquire);
                q.prune(position);
                let sound_wanted = s.audio && q.sound.len() < SOUND_AHEAD;
                let pictures_wanted = s.video && q.pictures_ahead(position) < PICTURES_AHEAD && (!s.audio || q.sound.len() < SOUND_MOST);
                if sound_wanted || pictures_wanted {
                    break;
                }
                q = s.room.wait_timeout(q, Duration::from_millis(50)).map(|(g, _)| g).unwrap_or_else(|e| e.into_inner().0);
            }
        }
        match media.next_item() {
            Some(Item::Sound { pcm, .. }) => s.queue.lock().unwrap_or_else(|e| e.into_inner()).sound.extend(pcm.iter().copied()),
            Some(Item::Picture { pts_ms, width, height, rgba }) => {
                last = last.max(pts_ms);
                let picture = Picture { id: NEXT_PICTURE.fetch_add(1, Ordering::Relaxed), at_ms: offset + pts_ms, width, height, rgba: rgba.to_vec() };
                s.queue.lock().unwrap_or_else(|e| e.into_inner()).pictures.push_back(Arc::new(picture));
                s.decoded.notify_all();
            }
            None => {
                if s.looping.load(Ordering::Acquire) && media.rewind() {
                    // the next time round starts where this one's length ends
                    let length = media.info.duration_ms.map_or(last + frame_ms, |d| (d as i64).max(last + frame_ms));
                    offset += length;
                    last = 0;
                    continue;
                }
                s.queue.lock().unwrap_or_else(|e| e.into_inner()).finished = true;
                s.decoded.notify_all();
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pack's sound, decoded and pulled as the mixer pulls it (where this machine has the
    /// test packs and FFmpeg's libraries).
    #[test]
    fn a_packs_sound_reaches_the_mixer_and_loops() {
        let file = Path::new(r"C:\vPinball\PinUPSystem\PUPVideos\leprechaun\ballsaved-L1\L1- ball saved.wav");
        if !file.exists() || !super::super::av::available() {
            return;
        }
        let mut source = sink().source();
        let playback = Playback::start(file, 100.0, true, false).expect("plays");
        // 3 s of sound, a 10 ms block at a time, waiting for the decoder as the mixer would
        let mut heard = 0usize;
        let mut block = vec![0.0f32; 960];
        for _ in 0..300 {
            block.fill(0.0);
            source.mix_into(&mut block, 1.0);
            heard += block.iter().filter(|x| x.abs() > 1e-4).count();
            std::thread::sleep(Duration::from_millis(10));
        }
        // the 2.1 s file looped past its end: sound throughout most of the 3 s
        assert!(heard > 960 * 150, "only {heard} samples heard");
        playback.set_volume(0.0);
        std::thread::sleep(Duration::from_millis(50));
        block.fill(0.0);
        source.mix_into(&mut block, 1.0);
        assert!(block.iter().all(|x| x.abs() < 1e-6));
    }

    /// An animated GIF's pictures, each from its moment on, round again when it loops; in test
    /// mode, the same picture for the same moment every time.
    #[test]
    fn pictures_follow_the_files_moment_and_loop() {
        let file = Path::new(r"C:\vPinball\PinUPSystem\PUPVideos\DarkestDungeon\Boss\Brigand-vvulf.gif");
        if !file.exists() || !super::super::av::available() {
            return;
        }
        set_exact(true);
        let playback = Playback::start(file, 100.0, true, true).expect("plays");
        let at = |ms: i64| {
            playback.set_position(ms);
            playback.picture().map(|p| (p.at_ms, p.width, p.height))
        };
        // 37 pictures over 1.11 s
        assert_eq!(at(0).map(|p| p.0), Some(0));
        let mid = at(500).expect("a picture at 0.5 s");
        assert!(mid.0 <= 500 && mid.0 > 400, "{mid:?}");
        assert_eq!((mid.1, mid.2), (311, 376));
        // the second time round, the same picture a length later
        let again = at(500 + 1110).expect("a picture the second time round");
        assert_eq!(again.0, mid.0 + 1110);
        set_exact(false);
    }
}
