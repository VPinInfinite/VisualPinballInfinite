//! The bridge to FFmpeg (`native/pup_av.cpp`): a PinUP pack's file opened, its pictures
//! as RGBA and its sound as stereo floats at the mixer's rate, one piece at a time in the
//! file's order; and a video written, a table's preview, its pictures handed over one at a time
//! (`VideoWriter`: H.264 through Windows' own encoder, in an MP4), with the table's sound beside
//! them if wanted (AAC, FFmpeg's own encoder). FFmpeg's libraries load the first time a pack plays — beside the player, else,
//! in a development build, in our own build or VPX 5436's third-party package under `vpx-build`, as
//! PinMAME's library is found — so the player starts and plays tables without them. Ours.

use std::ffi::{CString, c_char, c_void};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[repr(C)]
#[derive(Default)]
struct RawInfo {
    has_video: i32,
    has_audio: i32,
    width: i32,
    height: i32,
    fps: f64,
    duration_ms: i64,
}

#[repr(C)]
struct RawItem {
    kind: i32,
    pts_ms: i64,
    width: i32,
    height: i32,
    rgba: *const u8,
    frames: i32,
    pcm: *const f32,
}

unsafe extern "C" {
    fn vpi_av_load(dir: *const u16) -> i32;
    fn vpi_av_open(path: *const c_char, want_video: i32, want_audio: i32, out_rate: i32, info: *mut RawInfo) -> *mut c_void;
    fn vpi_av_next(media: *mut c_void, item: *mut RawItem) -> i32;
    fn vpi_av_rewind(media: *mut c_void) -> i32;
    fn vpi_av_close(media: *mut c_void);
    fn vpi_av_writer_open(path: *const c_char, width: i32, height: i32, fps: i32, sound_rate: i32, error: *mut c_char, error_len: i32) -> *mut c_void;
    fn vpi_av_writer_frame(writer: *mut c_void, rgba: *const u8) -> i32;
    fn vpi_av_writer_sound(writer: *mut c_void, samples: *const f32, frames: i32) -> i32;
    fn vpi_av_writer_close(writer: *mut c_void) -> i32;
}

/// The folders FFmpeg's libraries are looked for in, in order.
fn folders() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        out.push(dir.to_path_buf());
        // in the repository (development builds): our own build (tools\thirdparty\setup.ps1), then
        // VPX 5436's bundle's; each tried until one loads
        if crate::DEVELOPMENT {
            for under in [r"vpx-build\thirdparty\out\windows-x64\bin", r"vpx-build\downloads\5436-third-party-windows-x64-Release\runtime-libs\windows-x64"] {
                out.extend(dir.ancestors().take(4).map(|up| up.join(under)).find(|d| d.is_dir()));
            }
        }
    }
    out
}

/// Whether FFmpeg's libraries are here, loading them the first time it's asked.
pub fn available() -> bool {
    static LOADED: OnceLock<bool> = OnceLock::new();
    *LOADED.get_or_init(|| {
        folders().iter().any(|dir| {
            let wide: Vec<u16> = dir.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect();
            // SAFETY: a NUL-terminated path; the bridge only loads libraries and looks up functions
            unsafe { vpi_av_load(wide.as_ptr()) == 1 }
        })
    })
}

/// What a file holds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Info {
    /// Its pictures' size, when it has pictures (a video, an animated GIF, a picture).
    pub video: Option<(u32, u32)>,
    pub audio: bool,
    /// Pictures a second, where the file says.
    pub fps: f64,
    pub duration_ms: Option<u64>,
}

/// One piece of a file: a picture, or some sound. Its data is the bridge's until the next one.
pub enum Item<'a> {
    Picture { pts_ms: i64, width: u32, height: u32, rgba: &'a [u8] },
    /// Stereo frames, interleaved, at the rate the file was opened for.
    Sound { pts_ms: i64, pcm: &'a [f32] },
}

/// A file being decoded.
pub struct Media {
    raw: *mut c_void,
    pub info: Info,
}

// SAFETY: the bridge's state belongs to this one value, used from one thread at a time
unsafe impl Send for Media {}

impl Media {
    /// Opens a file for its pictures, its sound, or both, its sound made stereo at `rate`; None
    /// when FFmpeg isn't here, or the file can't be read or has neither.
    pub fn open(path: &Path, video: bool, audio: bool, rate: u32) -> Option<Media> {
        if !available() {
            return None;
        }
        let path = CString::new(path.to_str()?).ok()?;
        let mut raw_info = RawInfo::default();
        // SAFETY: a NUL-terminated UTF-8 path and an info record to fill
        let raw = unsafe { vpi_av_open(path.as_ptr(), i32::from(video), i32::from(audio), rate as i32, &mut raw_info) };
        if raw.is_null() {
            return None;
        }
        let info = Info {
            video: (raw_info.has_video != 0).then_some((raw_info.width.max(0) as u32, raw_info.height.max(0) as u32)),
            audio: raw_info.has_audio != 0,
            fps: raw_info.fps,
            duration_ms: u64::try_from(raw_info.duration_ms).ok(),
        };
        Some(Media { raw, info })
    }

    /// The next picture or piece of sound; None at the file's end.
    pub fn next_item(&mut self) -> Option<Item<'_>> {
        let mut item = RawItem { kind: 0, pts_ms: 0, width: 0, height: 0, rgba: std::ptr::null(), frames: 0, pcm: std::ptr::null() };
        // SAFETY: our open file and a record to fill
        if unsafe { vpi_av_next(self.raw, &mut item) } != 1 {
            return None;
        }
        match item.kind {
            1 if !item.rgba.is_null() => {
                let (w, h) = (item.width.max(0) as u32, item.height.max(0) as u32);
                // SAFETY: the bridge's picture, w × h × 4 bytes, valid until the next call
                let rgba = unsafe { std::slice::from_raw_parts(item.rgba, w as usize * h as usize * 4) };
                Some(Item::Picture { pts_ms: item.pts_ms, width: w, height: h, rgba })
            }
            2 if !item.pcm.is_null() => {
                // SAFETY: the bridge's sound, frames × 2 floats, valid until the next call
                let pcm = unsafe { std::slice::from_raw_parts(item.pcm, item.frames.max(0) as usize * 2) };
                Some(Item::Sound { pts_ms: item.pts_ms, pcm })
            }
            _ => None,
        }
    }

    /// Back to the start, for a file that loops.
    pub fn rewind(&mut self) -> bool {
        // SAFETY: our open file
        unsafe { vpi_av_rewind(self.raw) == 1 }
    }
}

impl Drop for Media {
    fn drop(&mut self) {
        // SAFETY: our open file, closed once
        unsafe { vpi_av_close(self.raw) };
    }
}

/// A video being written: RGBA pictures in, one at a time, an MP4 out — H.264 through Windows'
/// own encoder (Media Foundation's), so no encoder of ours ships — and, if wanted, its sound
/// beside them: stereo floats, as AAC (FFmpeg's own encoder). The encoders work on a thread of
/// their own, as Media Foundation needs one free of the window's COM apartment (`COM must not be in
/// STA mode`), a few pieces queued for them.
pub struct VideoWriter {
    pieces: Option<std::sync::mpsc::SyncSender<Piece>>,
    worker: Option<std::thread::JoinHandle<Result<(), String>>>,
    size: (u32, u32),
    /// Whether it has a sound track.
    sound: bool,
    /// Pictures handed over so far.
    pub frames: u32,
    /// The sound's stereo frames handed over so far.
    pub sound_frames: u64,
}

/// What the encoders' thread is handed: a picture, or a piece of the sound.
enum Piece {
    Picture(Vec<u8>),
    Sound(Vec<f32>),
}

/// The bridge's writer, used on the encoder's thread alone.
struct RawWriter(*mut c_void);

// SAFETY: made, used and closed on the encoder's one thread; only moved there once
unsafe impl Send for RawWriter {}

impl VideoWriter {
    /// A video of `width` × `height` pictures, `fps` a second, written to `path`; with a sound
    /// track of stereo floats at `sound_rate` a second, when given.
    pub fn create(path: &Path, width: u32, height: u32, fps: u32, sound_rate: Option<u32>) -> Result<VideoWriter, String> {
        if !available() {
            return Err("FFmpeg's libraries aren't beside the player".into());
        }
        let c = CString::new(path.to_str().ok_or("the video's path isn't text")?).map_err(|e| e.to_string())?;
        let shown = path.display().to_string();
        let (pieces, queue) = std::sync::mpsc::sync_channel::<Piece>(8);
        let (opened, open) = std::sync::mpsc::channel::<Result<(), String>>();
        let worker = std::thread::Builder::new()
            .name("video encoder".into())
            .spawn(move || -> Result<(), String> {
                let mut error = [0 as c_char; 256];
                // SAFETY: a NUL-terminated UTF-8 path and a buffer for the reason it failed
                let raw = RawWriter(unsafe { vpi_av_writer_open(c.as_ptr(), width as i32, height as i32, fps as i32, sound_rate.unwrap_or(0) as i32, error.as_mut_ptr(), error.len() as i32) });
                if raw.0.is_null() {
                    // SAFETY: the bridge wrote a NUL-terminated reason, or left it empty
                    let why = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned();
                    let _ = opened.send(Err(format!("{shown}: {why}")));
                    return Ok(());
                }
                let _ = opened.send(Ok(()));
                let mut written = true;
                for piece in queue {
                    written &= match piece {
                        // SAFETY: our writer, and a picture of its size
                        Piece::Picture(rgba) => unsafe { vpi_av_writer_frame(raw.0, rgba.as_ptr()) == 1 },
                        // SAFETY: our writer, made with a sound track, and whole stereo frames
                        Piece::Sound(samples) => unsafe { vpi_av_writer_sound(raw.0, samples.as_ptr(), (samples.len() / 2) as i32) == 1 },
                    };
                }
                // SAFETY: our writer, closed once
                let closed = unsafe { vpi_av_writer_close(raw.0) } == 1;
                match (written, closed) {
                    (true, true) => Ok(()),
                    (false, _) => Err(format!("{shown}: a picture or its sound couldn't be written to the video")),
                    (_, false) => Err(format!("{shown}: the video's end couldn't be written")),
                }
            })
            .map_err(|e| e.to_string())?;
        open.recv().map_err(|e| e.to_string())??;
        Ok(VideoWriter { pieces: Some(pieces), worker: Some(worker), size: (width, height), sound: sound_rate.is_some(), frames: 0, sound_frames: 0 })
    }

    /// One picture, the video's size, RGBA, rows top first.
    pub fn frame(&mut self, rgba: &[u8]) -> Result<(), String> {
        if rgba.len() != self.size.0 as usize * self.size.1 as usize * 4 {
            return Err("a picture of another size than the video's".into());
        }
        let pieces = self.pieces.as_ref().ok_or("the video is finished")?;
        pieces.send(Piece::Picture(rgba.to_vec())).map_err(|_| "the video's encoder stopped".to_string())?;
        self.frames += 1;
        Ok(())
    }

    /// A piece of its sound: stereo floats, left first, at the rate it was made with. Nothing for a
    /// video made without sound, or for an empty piece.
    pub fn sound(&mut self, samples: &[f32]) -> Result<(), String> {
        if !self.sound || samples.len() < 2 {
            return Ok(());
        }
        let whole = &samples[..samples.len() / 2 * 2];
        let pieces = self.pieces.as_ref().ok_or("the video is finished")?;
        pieces.send(Piece::Sound(whole.to_vec())).map_err(|_| "the video's encoder stopped".to_string())?;
        self.sound_frames += (whole.len() / 2) as u64;
        Ok(())
    }

    /// The video ended, its file finished once the encoder has written every picture.
    pub fn finish(mut self) -> Result<(), String> {
        self.pieces = None;
        match self.worker.take() {
            Some(worker) => worker.join().map_err(|_| "the video's encoder failed".to_string())?,
            None => Ok(()),
        }
    }
}

impl Drop for VideoWriter {
    fn drop(&mut self) {
        // the encoder finishes what it has, closing the file
        self.pieces = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
