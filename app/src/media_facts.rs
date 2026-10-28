//! What the screens need to know of a media file beyond its kind, read from the file itself and
//! kept while the app runs (each file's facts until its length or time changes): its size as it's
//! shown — a picture's from its header, a video's from its MP4 track's header, turned as the track
//! says — so a front end's playfield picture or video kept on its side for a cabinet's screen is
//! shown upright, turned the way that matches an upright picture of the table; and whether a
//! picture looks like a wheel — about square and see-through at its top corners, as a wheel's logo
//! cut out is — so a rule card or a banner kept in a front end's wheel folder under the table's name
//! isn't taken for its wheel.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use image::GenericImageView;
use image::imageops::FilterType;

/// A file's facts by its path, with the length and time it had when they were read.
type Known<T> = LazyLock<Mutex<HashMap<PathBuf, (u64, Option<SystemTime>, T)>>>;

static SIZES: Known<Option<(u32, u32)>> = LazyLock::new(Default::default);
static WHEELS: Known<bool> = LazyLock::new(Default::default);
static SOUNDS: Known<bool> = LazyLock::new(Default::default);
static PRINTS: Known<Option<u64>> = LazyLock::new(Default::default);
/// The turn a picture on its side stands up by, by the picture and the upright one it was matched
/// against, with both files' lengths and times then.
type Stamp = (u64, Option<SystemTime>);
static TURNS: LazyLock<Mutex<HashMap<(PathBuf, PathBuf), (Stamp, Stamp, u16)>>> = LazyLock::new(Default::default);

fn stamp_of(path: &Path) -> Stamp {
    std::fs::metadata(path).map(|m| (m.len(), m.modified().ok())).unwrap_or((0, None))
}

/// `read`'s answer for `path`, read again only once the file has changed.
fn known<T: Copy>(map: &Known<T>, path: &Path, read: impl FnOnce(&Path) -> T) -> T {
    let stamp = std::fs::metadata(path).map(|m| (m.len(), m.modified().ok())).unwrap_or((0, None));
    if let Some(&(len, when, value)) = map.lock().unwrap_or_else(|e| e.into_inner()).get(path)
        && (len, when) == stamp
    {
        return value;
    }
    let value = read(path);
    map.lock().unwrap_or_else(|e| e.into_inner()).insert(path.to_path_buf(), (stamp.0, stamp.1, value));
    value
}

/// Its size as shown, width by height: a picture's, or a video's (MP4 and its kin).
pub fn size(path: &Path, video: bool) -> Option<(u32, u32)> {
    known(&SIZES, path, |p| if video { video_size(p) } else { picture_size(p) })
}

/// Whether a front end keeping it on its side shows it so: a playfield picture or video wider than
/// tall by 16:9 or more — its table lying across it, as PinUP Popper keeps those made for a
/// cabinet's playfield screen, laid on its side. A picture a little wider than tall (a backglass or
/// a score display filed with the playfield's) isn't.
pub fn on_its_side(path: &Path, video: bool) -> bool {
    size(path, video).is_some_and(|(w, h)| h > 0 && w as f32 / h as f32 >= 1.6)
}

/// Which way a playfield picture on its side stands up: turned a quarter right (90) or a quarter
/// left (270) — front ends' are kept both ways, the table's foot to the right or to the left —
/// whichever matches `upright`, a picture of the same table standing up, better (`match_turn`). A
/// quarter right with nothing to match, or when either won't read (most are kept that way).
pub fn upright_turn(side: &Path, upright: Option<&Path>) -> u16 {
    let Some(upright) = upright else { return 90 };
    let key = (side.to_path_buf(), upright.to_path_buf());
    let stamps = (stamp_of(side), stamp_of(upright));
    if let Some(&(a, b, turn)) = TURNS.lock().unwrap_or_else(|e| e.into_inner()).get(&key)
        && (a, b) == stamps
    {
        return turn;
    }
    let turn = match_turn(side, upright).unwrap_or(90);
    TURNS.lock().unwrap_or_else(|e| e.into_inner()).insert(key, (stamps.0, stamps.1, turn));
    turn
}

/// A picture read whatever its name says it is.
fn read_picture(path: &Path) -> Option<image::DynamicImage> {
    image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()
}

/// The turn, 90 or 270, that makes `side` look most like `upright`, matched at three small sizes in
/// colour (a cabinet's view and a flat picture of the same table differ in their detail, less in
/// where their colours lie: a backboard's banner at the top, the apron at the foot); None when
/// either won't read.
fn match_turn(side: &Path, upright: &Path) -> Option<u16> {
    // small, each colour less its average: where the table's light, dark and colours lie
    let small = |picture: &image::DynamicImage, w: u32, h: u32| -> Vec<f32> {
        let rgb = picture.resize_exact(w, h, FilterType::Triangle).to_rgb32f();
        let n = (w * h) as f32;
        (0..3)
            .flat_map(|c| {
                let mean = rgb.pixels().map(|p| p[c]).sum::<f32>() / n;
                rgb.pixels().map(move |p| p[c] - mean)
            })
            .collect()
    };
    // how alike two are: their correlation, -1 to 1
    let alike = |a: &[f32], b: &[f32]| {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm = (a.iter().map(|x| x * x).sum::<f32>() * b.iter().map(|y| y * y).sum::<f32>()).sqrt();
        if norm > 0.0 { dot / norm } else { 0.0 }
    };
    let (side, upright) = (read_picture(side)?, read_picture(upright)?);
    let (right, left) = (side.rotate90(), side.rotate270());
    let (mut r, mut l) = (0.0, 0.0);
    for (w, h) in [(3, 6), (6, 12), (12, 24)] {
        let u = small(&upright, w, h);
        r += alike(&small(&right, w, h), &u);
        l += alike(&small(&left, w, h), &u);
    }
    Some(if l > r { 270 } else { 90 })
}

/// Whether a picture looks like a wheel: about square, and see-through at both top corners, as a
/// round logo cut out is — a rule card, a banner or a collage isn't.
pub fn looks_like_wheel(path: &Path) -> bool {
    known(&WHEELS, path, |p| {
        let Some((w, h)) = picture_size(p) else { return false };
        if !(0.85..=1.18).contains(&(w as f32 / h.max(1) as f32)) {
            return false;
        }
        let Ok(picture) = image::ImageReader::open(p).and_then(|r| r.with_guessed_format()).map_err(|_| ()).and_then(|r| r.decode().map_err(|_| ())) else {
            return false;
        };
        let (w, h) = picture.dimensions();
        picture.color().has_alpha() && w > 0 && h > 0 && picture.get_pixel(0, 0)[3] < 255 && picture.get_pixel(w - 1, 0)[3] < 255
    })
}

/// A picture's size, from its header (whatever its name says it is).
fn picture_size(path: &Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_dimensions().ok()
}

/// A file's fingerprint, to know copies of one file: its length and its first and last 64 KB,
/// hashed (FNV-1a). Two files with the same are taken for the same file.
pub fn fingerprint(path: &Path) -> Option<u64> {
    known(&PRINTS, path, |p| {
        let mut file = std::fs::File::open(p).ok()?;
        let len = file.metadata().ok()?.len();
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        eat(&len.to_le_bytes());
        let mut chunk = vec![0u8; 64 * 1024];
        let n = file.read(&mut chunk).ok()?;
        eat(&chunk[..n]);
        if len > 128 * 1024 {
            file.seek(SeekFrom::End(-64 * 1024)).ok()?;
            let n = file.read(&mut chunk).ok()?;
            eat(&chunk[..n]);
        }
        Some(hash)
    })
}

/// Whether a video carries sound: a sound track (`soun`, its handler) in its movie box. Only the
/// MP4 family.
pub fn has_sound(path: &Path) -> bool {
    known(&SOUNDS, path, |p| {
        movie_box(p).is_some_and(|moov| {
            boxes(&moov).filter(|(kind, _)| *kind == b"trak").any(|(_, trak)| {
                boxes(trak)
                    .filter(|(kind, _)| *kind == b"mdia")
                    .flat_map(|(_, mdia)| boxes(mdia))
                    // after its version and flags, and a nought: the kind of track it handles
                    .any(|(kind, hdlr)| kind == b"hdlr" && hdlr.get(8..12) == Some(&b"soun"[..]))
            })
        })
    })
}

/// A video's size as shown: its first picture track's width and height in its track header
/// (`tkhd`), swapped when the track is turned a quarter (its matrix's). Only the MP4 family.
fn video_size(path: &Path) -> Option<(u32, u32)> {
    track_size(&movie_box(path)?)
}

/// A video's movie box (`moov`), the boxes before it skipped, unread. Only the MP4 family
/// (`.mp4`, `.m4v`, `.mov`, `.f4v`).
fn movie_box(path: &Path) -> Option<Vec<u8>> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    if !["mp4", "m4v", "mov", "f4v"].contains(&ext.as_str()) {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut at = 0u64;
    while at + 8 <= len {
        file.seek(SeekFrom::Start(at)).ok()?;
        let mut head = [0u8; 16];
        file.read_exact(&mut head[..8]).ok()?;
        let (mut size, mut header) = (u64::from(u32::from_be_bytes(head[0..4].try_into().ok()?)), 8u64);
        if size == 1 {
            file.read_exact(&mut head[8..16]).ok()?;
            (size, header) = (u64::from_be_bytes(head[8..16].try_into().ok()?), 16);
        } else if size == 0 {
            size = len - at;
        }
        if size < header {
            return None;
        }
        if &head[4..8] == b"moov" {
            let mut moov = vec![0; usize::try_from(size - header).ok()?.min(64 << 20)];
            file.read_exact(&mut moov).ok()?;
            return Some(moov);
        }
        at += size;
    }
    None
}

/// The boxes in `b`, each its type and what it holds.
fn boxes(b: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let size = usize::try_from(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?)).ok()?;
        let kind = b.get(at + 4..at + 8)?;
        let (start, end) = match size {
            0 => (at + 8, b.len()),
            1 => (at + 16, at.checked_add(usize::try_from(u64::from_be_bytes(b.get(at + 8..at + 16)?.try_into().ok()?)).ok()?)?),
            n => (at + 8, at.checked_add(n)?),
        };
        if end > b.len() || start > end || end <= at {
            return None;
        }
        at = end;
        Some((kind, &b[start..end]))
    })
}

/// The first track with pictures (a width and a height) in a movie box: its size as shown.
fn track_size(moov: &[u8]) -> Option<(u32, u32)> {
    boxes(moov).filter(|(kind, _)| *kind == b"trak").find_map(|(_, trak)| {
        let (_, tkhd) = boxes(trak).find(|(kind, _)| *kind == b"tkhd")?;
        // after its version and flags, its times, track and length (longer in version 1), and its
        // layer, group and volume: the matrix, then the width and height (16.16 fixed point)
        let matrix = 4 + (if *tkhd.first()? == 1 { 32 } else { 20 }) + 16;
        let word = |at: usize| Some(u32::from_be_bytes(tkhd.get(at..at + 4)?.try_into().ok()?));
        let (w, h) = (word(matrix + 36)? >> 16, word(matrix + 40)? >> 16);
        if w == 0 || h == 0 {
            return None;
        }
        // a quarter turn: the matrix's first and fifth (its a and d) nought
        Some(if word(matrix)? == 0 && word(matrix + 16)? == 0 { (h, w) } else { (w, h) })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box of `kind` holding `body`.
    fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        [&(8 + body.len() as u32).to_be_bytes()[..], kind, body].concat()
    }

    /// A version 0 track header of `w` × `h`, turned a quarter if `turned`.
    fn tkhd(w: u32, h: u32, turned: bool) -> Vec<u8> {
        let mut body = vec![0u8; 4 + 20 + 16];
        let one = 1u32 << 16;
        let matrix: [u32; 9] = if turned { [0, one, 0, one.wrapping_neg(), 0, 0, 0, 0, 1 << 30] } else { [one, 0, 0, 0, one, 0, 0, 0, 1 << 30] };
        for m in matrix {
            body.extend_from_slice(&m.to_be_bytes());
        }
        body.extend_from_slice(&(w << 16).to_be_bytes());
        body.extend_from_slice(&(h << 16).to_be_bytes());
        mp4_box(b"tkhd", &body)
    }

    #[test]
    fn a_videos_size_from_its_track_header() {
        let dir = std::env::temp_dir().join(format!("vpi-media-facts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // a sound track first (no size), then the pictures; the data before the movie box skipped
        let moov = mp4_box(b"moov", &[mp4_box(b"trak", &tkhd(0, 0, false)), mp4_box(b"trak", &tkhd(3840, 2160, false))].concat());
        let file = [mp4_box(b"ftyp", b"isom"), mp4_box(b"mdat", &[7u8; 100]), moov].concat();
        std::fs::write(dir.join("wide.mp4"), &file).unwrap();
        let turned = [mp4_box(b"ftyp", b"isom"), mp4_box(b"moov", &mp4_box(b"trak", &tkhd(1920, 1080, true)))].concat();
        std::fs::write(dir.join("turned.mp4"), &turned).unwrap();
        // and one with a sound track beside its pictures
        let hdlr = |kind: &[u8; 4]| mp4_box(b"mdia", &mp4_box(b"hdlr", &[&[0u8; 8][..], &kind[..], &[0u8; 12][..]].concat()));
        let sounding = [mp4_box(b"ftyp", b"isom"), mp4_box(b"moov", &[mp4_box(b"trak", &[tkhd(640, 360, false), hdlr(b"vide")].concat()), mp4_box(b"trak", &[tkhd(0, 0, false), hdlr(b"soun")].concat())].concat())].concat();
        std::fs::write(dir.join("sounding.mp4"), &sounding).unwrap();
        let (wide, upright) = (size(&dir.join("wide.mp4"), true), size(&dir.join("turned.mp4"), true));
        let side = on_its_side(&dir.join("wide.mp4"), true);
        let heard = [has_sound(&dir.join("sounding.mp4")), has_sound(&dir.join("wide.mp4"))];
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(wide, Some((3840, 2160)));
        assert!(side);
        assert_eq!(upright, Some((1080, 1920)));
        assert_eq!(heard, [true, false]);
    }

    #[test]
    fn a_picture_on_its_side_stands_up_the_way_that_matches() {
        let dir = std::env::temp_dir().join(format!("vpi-media-facts-turn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // upright: light at its top, dark at its foot, a bright spot low on the left
        let upright = image::RgbImage::from_fn(90, 160, |x, y| {
            let base = 255 - (y * 200 / 160) as u8;
            if x < 30 && (110..130).contains(&y) { image::Rgb([255, 255, 255]) } else { image::Rgb([base, base, base]) }
        });
        upright.save(dir.join("upright.png")).unwrap();
        // kept on its side both ways: its foot to the left (stands up turned a quarter left) and
        // to the right (a quarter right)
        image::imageops::rotate90(&upright).save(dir.join("foot-left.png")).unwrap();
        image::imageops::rotate270(&upright).save(dir.join("foot-right.png")).unwrap();
        let up = dir.join("upright.png");
        let found = [upright_turn(&dir.join("foot-left.png"), Some(&up)), upright_turn(&dir.join("foot-right.png"), Some(&up)), upright_turn(&dir.join("foot-left.png"), None)];
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(found, [270, 90, 90]);
    }

    #[test]
    fn a_wheel_is_square_and_see_through_at_its_corners() {
        let dir = std::env::temp_dir().join(format!("vpi-media-facts-wheel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // a round logo: see-through corners
        let wheel = image::RgbaImage::from_fn(64, 64, |x, y| {
            let (dx, dy) = (x as i32 - 32, y as i32 - 32);
            if dx * dx + dy * dy < 30 * 30 { image::Rgba([200, 40, 40, 255]) } else { image::Rgba([0, 0, 0, 0]) }
        });
        wheel.save(dir.join("wheel.png")).unwrap();
        // a collage, square but solid at its top; a rule card, wide
        image::RgbaImage::from_pixel(64, 62, image::Rgba([10, 10, 10, 255])).save(dir.join("collage.png")).unwrap();
        image::RgbaImage::from_fn(120, 64, |_, _| image::Rgba([0, 0, 0, 0])).save(dir.join("card.png")).unwrap();
        let found = ["wheel.png", "collage.png", "card.png"].map(|n| looks_like_wheel(&dir.join(n)));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(found, [true, false, false]);
    }
}
