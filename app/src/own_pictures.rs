//! Pictures we make for a table VPS has no pictures of (originals, test tables): a backglass picture
//! from the table's own backglass file, playfield pictures and preview videos from our player. Our
//! player draws the table with no window and no
//! sound (its test mode), its script running for twelve seconds so its lights, backglass and score
//! display are as they are once it's up — a ROM machine's start-up tests over — seen from straight
//! above, upright, and cut to the playfield (`--crop playfield`), as VPS's pictures show tables;
//! and keeps the picture, with the backglass's (the table's own backglass file, as the player draws
//! it, lit as the script lights it) and the score display's, which test mode saves beside it; then
//! twenty seconds of it as a game starts as a preview video (`--video`: H.264 through Windows' own
//! encoder, 25 pictures a second), cut the same, its sound beside the pictures (`--video-sound`:
//! recorded with no output, AAC) — the table's music, heard in cabinet mode and where the video
//! plays. Kept in `Pictures\<table>` in the player's data folder;
//! shown where VPS and PinUP Popper have none. Those wanted made, the rest kept (the make-media
//! window), what the table turned out to have none of remembered (`none`); and a wheel the screens
//! draw from its pictures (`save_wheel`). One table's
//! made at a time, the others waiting their turn (`turn`), each step said done as the player saves
//! it (hovering the background tasks shows what's queued and in progress). A ROM machine
//! that has never run here is started once first, ten seconds, before it's drawn: on its first
//! start, with no memory, a machine restores its factory settings and some then sit still until
//! they're started again — Flash (Williams 1979) lit nothing in its first preview video, then
//! changed up to 8% of its picture from one picture to the next once started before.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};

/// The pictures made, by kind: their files, as test mode names them beside the shot; and the wheel,
/// which the screens draw from the table's pictures (`save_wheel`).
pub const KINDS: [(&str, &str); 5] = [("playfield", "playfield.png"), ("backglass", "playfield-backglass.png"), ("dmd", "playfield-dmd.png"), ("video", "playfield.mp4"), ("wheel", "wheel.png")];

/// The wheel the screens drew for a table, kept with its other pictures.
pub fn save_wheel(id: &str, png: &[u8]) -> Result<(), String> {
    if !png.starts_with(b"\x89PNG") {
        return Err("That isn't a picture.".into());
    }
    let dir = folder(id).ok_or("no APPDATA folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("wheel.png");
    std::fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))
}

/// The table's folder of pictures we made.
pub fn folder(id: &str) -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Pictures").join(id))
}

/// The picture of `kind` made for the table, and when, in ms since 1970.
pub fn made(id: &str, kind: &str) -> Option<(PathBuf, i64)> {
    let name = KINDS.iter().find(|(k, _)| *k == kind)?.1;
    let path = folder(id)?.join(name);
    let when = std::fs::metadata(&path).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
    Some((path, when))
}

/// The tags a backglass file keeps its backglass's pictures under, the one to show first.
const BACKGLASS_TAGS: [&str; 3] = ["<BackglassImage", "<BackglassOffImage", "<BackglassOnImage"];

/// The size of the backglass picture in the table's backglass file beside it (`<table>.directb2s`):
/// its first picture's header, decoded from the start of the base64 text the file keeps it in, as
/// the play launchers read it.
fn backglass_size(table: &Path) -> Option<(u32, u32)> {
    let text = std::fs::read(table.with_extension("directb2s")).ok()?;
    BACKGLASS_TAGS.iter().find_map(|tag| picture_size(&backglass_bytes(&text, tag, Some(1 << 16))?))
}

/// A backglass file's picture under `tag`, decoded from the base64 text the file keeps it in (line
/// breaks written as entities skipped): its first `most` characters' worth, or all of it.
fn backglass_bytes(text: &[u8], tag: &str, most: Option<usize>) -> Option<Vec<u8>> {
    let at = find(text, tag.as_bytes())?;
    let value = at + find(&text[at..], b"Value=\"")? + 7;
    let mut chars = Vec::new();
    let mut bytes = text[value..].iter();
    while let Some(&c) = bytes.next() {
        match c {
            b'"' => break,
            // `&#xD;` and the like: a line break, not the picture's
            b'&' => {
                for &d in bytes.by_ref() {
                    if d == b';' {
                        break;
                    }
                }
            }
            c if c.is_ascii_alphanumeric() || c == b'+' || c == b'/' => chars.push(c),
            _ => {}
        }
        if most.is_some_and(|m| chars.len() >= m) {
            break;
        }
    }
    Some(base64(&chars))
}

/// A small picture of a backglass file's backglass, for choosing among a table's: written to `out`
/// as a JPEG at most 640 pixels across.
pub fn backglass_thumbnail(file: &Path, out: &Path) -> Result<(), String> {
    let text = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let picture = BACKGLASS_TAGS
        .iter()
        .find_map(|tag| image::load_from_memory(&backglass_bytes(&text, tag, None)?).ok())
        .ok_or_else(|| format!("{}: no backglass picture in it", file.display()))?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    picture.thumbnail(640, 480).to_rgb8().save_with_format(out, image::ImageFormat::Jpeg).map_err(|e| format!("{}: {e}", out.display()))
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Base64 text's bytes, as far as whole groups of four go.
fn base64(chars: &[u8]) -> Vec<u8> {
    let value = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            _ => 63,
        }
    };
    chars
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|g| {
            let n = g.iter().fold(0u32, |n, &c| (n << 6) | value(c));
            [(n >> 16) as u8, (n >> 8) as u8, n as u8]
        })
        .collect()
}

/// A PNG's, JPEG's, GIF's or BMP's size, from its header.
fn picture_size(b: &[u8]) -> Option<(u32, u32)> {
    let be16 = |i: usize| Some(u32::from(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?])));
    if b.starts_with(b"\x89PNG") {
        let be32 = |i: usize| Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?));
        return Some((be32(16)?, be32(20)?));
    }
    if b.starts_with(b"GIF8") {
        return Some((u32::from(u16::from_le_bytes([*b.get(6)?, *b.get(7)?])), u32::from(u16::from_le_bytes([*b.get(8)?, *b.get(9)?]))));
    }
    if b.starts_with(b"BM") {
        let le32 = |i: usize| Some(i32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?));
        return Some((le32(18)?.unsigned_abs(), le32(22)?.unsigned_abs()));
    }
    if b.starts_with(&[0xFF, 0xD8]) {
        // the markers up to the frame's: its height, then its width
        let mut i = 2;
        while i + 4 <= b.len() {
            if b[i] != 0xFF {
                return None;
            }
            let marker = b[i + 1];
            let length = be16(i + 2)? as usize;
            if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
                return Some((be16(i + 7)?, be16(i + 5)?));
            }
            i += 2 + length;
        }
    }
    None
}

/// What to make (the make-media window's ticks): each kind of `KINDS` wanted.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wanted {
    pub kinds: Vec<String>,
}

/// The kinds our player drew none of when they were last wanted, the table having none — no
/// backglass file beside it, no score display — kept beside its pictures (`none.txt`, a kind a
/// line) so they aren't offered to make again.
pub fn none(id: &str) -> Vec<String> {
    let Some(text) = folder(id).and_then(|f| std::fs::read_to_string(f.join("none.txt")).ok()) else {
        return Vec::new();
    };
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

/// One table's media made at a time: each is our player drawing at 1080 × 1920 and filming, which
/// several at once would make heavy work of on the user's PC.
static TURN: Mutex<()> = Mutex::new(());

/// The turn to make a table's media, waited for.
pub fn turn() -> MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

/// What making the media goes through, in order, as the background tasks show it: its machine
/// started once first when it has never run here (`warm_up`), then each kind wanted that our player
/// draws, by its kind in `KINDS`.
pub fn steps(wanted: &Wanted, warm_up: bool) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    if warm_up {
        out.push(("warm-up", "Its machine started once, its first time here"));
    }
    for (kind, label) in [("playfield", "Its playfield picture"), ("backglass", "Its backglass"), ("dmd", "Its score display"), ("video", "Its preview video, with its sound")] {
        if wanted.kinds.iter().any(|k| k == kind) {
            out.push((kind, label));
        }
    }
    out
}

/// The table's pictures and preview video wanted made afresh by our player, about half a minute
/// with the video, no window; those not wanted kept as they were. `warm_up`: its machine started
/// once first. `done` is told each of `steps`' kinds as it's done.
pub fn make(table: &Path, id: &str, wanted: &Wanted, warm_up: bool, done: &dyn Fn(&str)) -> Result<(), String> {
    let final_dir = folder(id).ok_or("no APPDATA folder")?;
    // made beside the table's folder, then those wanted moved in
    let dir = final_dir.with_file_name(format!("{id}.making"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let result = (|| {
        if warm_up {
            start_once(table, &dir)?;
            done("warm-up");
        }
        draw(table, &dir, wanted, done)
    })();
    if result.is_ok() {
        std::fs::create_dir_all(&final_dir).map_err(|e| format!("{}: {e}", final_dir.display()))?;
        let mut nothing = none(id);
        for (kind, name) in KINDS {
            // the wheel isn't our player's to draw
            if kind == "wheel" || !wanted.kinds.iter().any(|k| k == kind) {
                continue;
            }
            let made = dir.join(name);
            let kept = final_dir.join(name);
            // one wanted that the table hasn't (no backglass file, no score display): the old one goes
            let _ = std::fs::remove_file(&kept);
            nothing.retain(|k| k != kind);
            if made.is_file() {
                std::fs::rename(&made, &kept).map_err(|e| format!("{}: {e}", kept.display()))?;
            } else if matches!(kind, "backglass" | "dmd") {
                nothing.push(kind.to_string());
            }
        }
        let list = final_dir.join("none.txt");
        if nothing.is_empty() {
            let _ = std::fs::remove_file(&list);
        } else {
            std::fs::write(&list, nothing.join("\r\n")).map_err(|e| format!("{}: {e}", list.display()))?;
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Our player started out of the user's way: no console window, below the user's own programs.
fn quiet(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    command
}

/// The table played once for ten seconds, small and unseen, so its machine's memory is made as its
/// first start makes it (its factory settings restored) and written as play ends; the next start
/// begins from it, as every later play does.
fn start_once(table: &Path, dir: &Path) -> Result<(), String> {
    let shot = dir.join("first-start.png");
    let mut command = Command::new(crate::screens::player()?);
    command.arg(table).arg("--shot").arg(&shot).args(["--at", "10000", "--size", "270x480", "--no-readout"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let status = quiet(&mut command).status().map_err(|e| format!("Our player couldn't be started: {e}"))?;
    let _ = std::fs::remove_file(&shot);
    if !status.success() {
        return Err("Our player couldn't start its machine.".into());
    }
    Ok(())
}

/// Our player's run: the playfield's picture, always (the others are drawn with it), and what else
/// is wanted, into `dir`; each wanted kind told to `done` as the player says it's saved.
fn draw(table: &Path, dir: &Path, wanted: &Wanted, done: &dyn Fn(&str)) -> Result<(), String> {
    let wants = |kind: &str| wanted.kinds.iter().any(|k| k == kind);
    // the table seen from straight above, upright, flippers at the foot: VPX's legacy camera looking
    // straight down (no inclination) through its narrowest view (the least perspective), nothing
    // stretched, laid back or moved, fitted to the table as VPX fits it — every value set, so no
    // table's own view (some set for a screen laid on its side, some for a VR room) comes into it;
    // then cut to the playfield, as VPS's pictures show tables. Colour files on, as when playing.
    let settings = dir.join("settings.ini");
    let view = ["Mode = 0", "LookAt = 0", "FOV = 25", "Layback = 0", "ScaleX = 1", "ScaleY = 1", "ScaleZ = 1", "PlayerX = 0", "PlayerY = 0", "PlayerZ = 0", "Rotation = 0"]
        .map(|v| format!("ViewCab{v}\r\n"))
        .concat();
    std::fs::write(&settings, format!("[Player]\r\nBGSet = 1\r\n\r\n[TableOverride]\r\n{view}\r\n[Plugin.Serum]\r\nEnable = 1\r\n\r\n[Plugin.VNI]\r\nEnable = 1\r\n"))
        .map_err(|e| format!("{}: {e}", settings.display()))?;
    let mut command = Command::new(crate::screens::player()?);
    command
        .env("VPI_SETTINGS", &settings)
        .arg(table)
        .arg("--shot")
        .arg(dir.join("playfield.png"))
        .args(["--at", "12000", "--view", "cabinet", "--size", "1080x1920", "--no-readout", "--crop", "playfield"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // and twenty seconds of it as a game starts — two coins and Start just after the picture, Start
    // again in case a machine was still starting up — every picture drawn sharp (motion blur would
    // smear the frames between the video's pictures, which aren't drawn), with its sound: its music
    if wants("video") {
        let keys = [(12300, '5'), (12700, '5'), (13500, '1'), (15500, '1')].map(|(at, key)| format!("{at}:{key}:d,{}:{key}:u", at + 100)).join(",");
        command.arg("--video").arg(dir.join("playfield.mp4")).args(["--seconds", "20", "--motion-blur", "off", "--video-sound", "--keys"]).arg(keys);
    }
    // the score display's window, and the backglass's in the shape of the table's own backglass
    // picture (a backglass is stretched to its window, as B2S does), hidden as test mode keeps them:
    // their pictures saved beside the playfield's
    if wants("dmd") {
        command.args(["--dmd-window", "0,0,1024,256"]);
    }
    if wants("backglass")
        && let Some((w, h)) = backglass_size(table)
    {
        let scale = (1600.0 / f64::from(w.max(h))).min(1.0);
        let (w, h) = ((f64::from(w) * scale).round().max(1.0), (f64::from(h) * scale).round().max(1.0));
        command.arg("--backglass-window").arg(format!("0,0,{w},{h}"));
    }
    let mut child = match quiet(&mut command).spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_file(&settings);
            return Err(format!("Our player couldn't be started: {e}"));
        }
    };
    // what it says of trouble read beside what it says as it goes, so neither fills and stops it
    let mut errors = child.stderr.take();
    let trouble = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(e) = errors.as_mut() {
            let _ = e.read_to_string(&mut text);
        }
        text
    });
    // each file saved, by its name (`saved <path>…`)
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(rest) = line.strip_prefix("saved ") {
                // `playfield.png` is a part of none of the other names
                for (kind, name) in [("dmd", "playfield-dmd.png"), ("backglass", "playfield-backglass.png"), ("video", "playfield.mp4"), ("playfield", "playfield.png")] {
                    if rest.contains(name) {
                        if wants(kind) {
                            done(kind);
                        }
                        break;
                    }
                }
            }
        }
    }
    let status = child.wait();
    let stderr = trouble.join().unwrap_or_default();
    let _ = std::fs::remove_file(&settings);
    status.map_err(|e| format!("Our player couldn't be waited for: {e}"))?;
    if !dir.join("playfield.png").is_file() {
        // what the player last said, if anything
        let said = stderr.lines().rev().map(str::trim).find(|l| !l.is_empty()).map(str::to_string);
        return Err(match said {
            Some(s) => format!("Our player couldn't draw it: {s}"),
            None => "Our player couldn't draw it.".into(),
        });
    }
    // a kind wanted that the table hasn't (no backglass file, no score display) is done as well
    for kind in ["backglass", "dmd"] {
        if wants(kind) && KINDS.iter().find(|(k, _)| *k == kind).is_some_and(|(_, name)| !dir.join(name).is_file()) {
            done(kind);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(bytes: &[u8]) -> String {
        const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        bytes
            .chunks(3)
            .flat_map(|c| {
                let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
                (0..4).map(move |k| if k <= c.len() { ABC[(n >> (18 - 6 * k)) as usize & 63] as char } else { '=' })
            })
            .collect()
    }

    #[test]
    fn a_backglass_pictures_size_from_its_file() {
        // a PNG's first bytes: its signature and header, 1280 × 1024
        let png = [&b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR"[..], &1280u32.to_be_bytes(), &1024u32.to_be_bytes(), &[8, 6, 0, 0, 0, 1, 2, 3]].concat();
        let text = encode(&png);
        // a line break written as entities partway through, as some backglass files keep them
        let text = format!("{}&#xD;&#xA;{}", &text[..8], &text[8..]);
        let dir = std::env::temp_dir().join(format!("vpi-own-pictures-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("t.directb2s"), format!("<DirectB2SData><Images><BackglassImage Value=\"{text}\" /></Images></DirectB2SData>")).unwrap();
        let size = backglass_size(&dir.join("t.vpx"));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(size, Some((1280, 1024)));
        // a JPEG's frame header: 600 high, 800 wide
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x02, 0x58, 0x03, 0x20];
        assert_eq!(picture_size(&jpeg), Some((800, 600)));
    }
}
