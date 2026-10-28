//! How long a pack's media file plays, read from the file's own headers
//! without decoding it: an MP4's (or MOV's) movie header, a sound's frame
//! count (Symphonia), a GIF's frame delays. Until the screens play video
//! (FFmpeg), this keeps what the scripts see about a file playing
//! and ending close to the real thing. Ours.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The file's length in milliseconds, where its headers give it.
pub fn length(path: &Path) -> Option<u64> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match extension.as_str() {
        "mp4" | "m4v" | "mov" | "f4v" => mp4_length(path),
        "gif" => gif_length(path),
        "mp3" | "wav" | "ogg" | "flac" => sound_length(path),
        _ => None,
    }
}

/// An ISO media file's length: the movie header (`moov/mvhd`), its duration over its time scale.
fn mp4_length(path: &Path) -> Option<u64> {
    let mut f = File::open(path).ok()?;
    let end = f.metadata().ok()?.len();
    let moov = find_box(&mut f, 0, end, b"moov")?;
    let (start, _) = find_box(&mut f, moov.0, moov.1, b"mvhd")?;
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut version = [0u8; 4];
    f.read_exact(&mut version).ok()?;
    let (scale, duration) = if version[0] == 1 {
        let mut b = [0u8; 28];
        f.read_exact(&mut b).ok()?;
        (u32::from_be_bytes(b[16..20].try_into().ok()?), u64::from_be_bytes(b[20..28].try_into().ok()?))
    } else {
        let mut b = [0u8; 16];
        f.read_exact(&mut b).ok()?;
        (u32::from_be_bytes(b[8..12].try_into().ok()?), u64::from(u32::from_be_bytes(b[12..16].try_into().ok()?)))
    };
    (scale > 0).then(|| duration * 1000 / u64::from(scale))
}

/// A box of `kind` among those from `start` to `end`: its contents' start and end.
fn find_box(f: &mut File, start: u64, end: u64, kind: &[u8; 4]) -> Option<(u64, u64)> {
    let mut at = start;
    while at + 8 <= end {
        f.seek(SeekFrom::Start(at)).ok()?;
        let mut head = [0u8; 8];
        f.read_exact(&mut head).ok()?;
        let mut size = u64::from(u32::from_be_bytes(head[0..4].try_into().ok()?));
        let mut header = 8;
        if size == 1 {
            let mut large = [0u8; 8];
            f.read_exact(&mut large).ok()?;
            size = u64::from_be_bytes(large);
            header = 16;
        } else if size == 0 {
            size = end - at;
        }
        if size < header {
            return None;
        }
        if &head[4..8] == kind {
            return Some((at + header, (at + size).min(end)));
        }
        at += size;
    }
    None
}

/// A GIF's frame delays added up (in hundredths of a second; a frame of none counts 10, as FFmpeg plays it).
fn gif_length(path: &Path) -> Option<u64> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 13 || &data[0..3] != b"GIF" {
        return None;
    }
    let mut at = 13;
    let packed = data[10];
    if packed & 0x80 != 0 {
        at += 3 << ((packed & 7) + 1);
    }
    let mut total = 0u64;
    let skip_blocks = |mut at: usize| -> Option<usize> {
        loop {
            let n = *data.get(at)? as usize;
            at += 1;
            if n == 0 {
                return Some(at);
            }
            at += n;
        }
    };
    while let Some(&b) = data.get(at) {
        match b {
            0x21 => {
                let label = *data.get(at + 1)?;
                if label == 0xF9 && data.get(at + 2) == Some(&4) {
                    let delay = u16::from_le_bytes([*data.get(at + 4)?, *data.get(at + 5)?]);
                    total += u64::from(if delay == 0 { 10 } else { delay });
                }
                at = skip_blocks(at + 2)?;
            }
            0x2C => {
                let flags = *data.get(at + 9)?;
                at += 10;
                if flags & 0x80 != 0 {
                    at += 3 << ((flags & 7) + 1);
                }
                at = skip_blocks(at + 1)?;
            }
            0x3B => break,
            _ => return None,
        }
    }
    (total > 0).then_some(total * 10)
}

/// A sound's length from its frame count and rate, where its headers give them.
fn sound_length(path: &Path) -> Option<u64> {
    use symphonia::core::codecs::CodecParameters;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
    use symphonia::core::meta::MetadataOptions;
    let file = File::open(path).ok()?;
    let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(e) = path.extension() {
        hint.with_extension(&e.to_string_lossy());
    }
    let format = symphonia::default::get_probe().probe(&hint, stream, FormatOptions::default(), MetadataOptions::default()).ok()?;
    let track = format.default_track(TrackType::Audio)?;
    let frames = track.num_frames?;
    let Some(CodecParameters::Audio(params)) = &track.codec_params else { return None };
    let rate = u64::from(params.sample_rate?);
    (rate > 0).then(|| frames * 1000 / rate)
}
