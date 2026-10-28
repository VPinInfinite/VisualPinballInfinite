//! The screens' pictures and videos: VPS's, fetched once and kept on the PC (`VPS\pictures`
//! in the player's data folder), and PinUP Popper's media for a table, served to the window by a
//! protocol of the app's own (`http://pic.localhost/` on Windows). Only what the library names is
//! served — VPS's pictures by the ID of what they show, Popper's by table and number — never a
//! path the screens give; and only pictures on VPS's own site are fetched.

use std::io::{Read as _, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};

use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::library::{Shared, State, data_folder};

/// Where the screens find the protocol.
pub fn base() -> &'static str {
    if cfg!(windows) { "http://pic.localhost/" } else { "pic://localhost/" }
}

/// Where VPS's pictures come from: nothing else is fetched.
const VPS_SITE: &str = "https://virtualpinballspreadsheet.github.io/";

/// At most this many pictures fetched at once.
const FETCHES: usize = 6;

/// The most of a video handed over at once, when the window asks for the rest of it.
const CHUNK: u64 = 4 << 20;

enum Source {
    Vps(String),
    File(PathBuf),
    /// One of a table's backglass files, by the table's ID: a small picture of it, made once.
    Backglass(String, PathBuf),
}

fn resolve(st: &State, path: &str) -> Option<Source> {
    let mut parts = path.split('/');
    match (parts.next()?, parts.next()?, parts.next()) {
        ("v" | "g", id, None) => st.vps_pictures.get(id).map(|url| Source::Vps(url.clone())),
        ("t", table, Some(n)) => {
            let t = st.tables.iter().find(|t| t.id == table)?;
            let m = t.holds.media.get(n.parse::<usize>().ok()?)?;
            Some(Source::File(m.path.clone()))
        }
        // the pictures our player made of a table in the library
        ("o", table, Some(kind)) => {
            st.tables.iter().find(|t| t.id == table)?;
            Some(Source::File(crate::own_pictures::made(table, kind)?.0))
        }
        // its music, as `tunes::of` found it, else the sound of the video our player made of it
        ("a", table, None) => {
            let t = st.tables.iter().find(|t| t.id == table)?;
            let ours = || crate::own_pictures::made(table, "video").map(|(path, _)| path).filter(|p| crate::media_facts::has_sound(p));
            st.tunes.get(&crate::library::key(&t.path)).cloned().or_else(ours).map(Source::File)
        }
        // the music its script plays, by its place in the list
        ("m", table, Some(n)) => {
            let t = st.tables.iter().find(|t| t.id == table)?;
            Some(Source::File(t.holds.music.get(n.parse::<usize>().ok()?)?.clone()))
        }
        // its screenshots, by name
        ("s", table, Some(name)) => {
            let t = st.tables.iter().find(|t| t.id == table)?;
            Some(Source::File(crate::screenshots::file(&t.stem, name)?))
        }
        // an achievement pack's pictures: the pack's own, or an achievement's
        ("h", pack, Some(which)) => crate::achievements::icon(pack, which).map(Source::File),
        // a picture chosen in the achievements editor, not saved in a pack yet
        ("e", n, None) => crate::achievements_editor::picked(n).map(Source::File),
        // its backglasses, small, for choosing which plays: the one playing (0), then those kept
        ("b", table, Some(n)) => {
            let t = st.tables.iter().find(|t| t.id == table)?;
            let file = match n.parse::<usize>().ok()? {
                0 => t.holds.backglass.clone()?,
                n => t.holds.backglasses.get(n - 1)?.clone(),
            };
            Some(Source::Backglass(t.id.clone(), file))
        }
        _ => None,
    }
}

pub fn handle<R: Runtime>(ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let path = request.uri().path().trim_start_matches('/').to_string();
    let range = request.headers().get(header::RANGE).and_then(|v| v.to_str().ok()).map(str::to_string);
    let source = {
        let shared = ctx.app_handle().state::<Shared>();
        let st = shared.0.lock().unwrap();
        resolve(&st, &path)
    };
    std::thread::spawn(move || {
        let response = match source {
            Some(Source::Vps(url)) => vps_picture(&url),
            Some(Source::File(path)) => file(&path, range.as_deref(), false),
            Some(Source::Backglass(id, path)) => match backglass_picture(&id, &path) {
                Some(small) => file(&small, None, false),
                None => status(StatusCode::NOT_FOUND),
            },
            None => status(StatusCode::NOT_FOUND),
        };
        responder.respond(response);
    });
}

/// A small picture of a table's backglass file, made once for the file as it is (its size and when it
/// last changed in the name) and kept with the pictures we make of the table.
fn backglass_picture(id: &str, path: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(path).ok()?;
    let when = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let small = crate::own_pictures::folder(id)?.join(format!("backglass {} {when}.jpg", meta.len()));
    if !small.is_file() {
        // under a passing name of its own first, so a picture asked for twice at once isn't read half-written
        let passing = small.with_extension(format!("{}.part", PASSING.fetch_add(1, Ordering::Relaxed)));
        crate::own_pictures::backglass_thumbnail(path, &passing).ok()?;
        if std::fs::rename(&passing, &small).is_err() {
            let _ = std::fs::remove_file(&passing);
        }
    }
    small.is_file().then_some(small)
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder().status(code).body(Vec::new()).unwrap()
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("apng") => "image/apng",
        Some("gif") => "image/gif",
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("m4a" | "aac") => "audio/mp4",
        Some("flac") => "audio/flac",
        _ => "application/octet-stream",
    }
}

/// A file, or the part of it a video's player asks for (`Range: bytes=start-end`).
fn file(path: &Path, range: Option<&str>, lasting: bool) -> Response<Vec<u8>> {
    let Ok(mut f) = std::fs::File::open(path) else { return status(StatusCode::NOT_FOUND) };
    let len = f.metadata().map_or(0, |m| m.len());
    let builder = Response::builder()
        .header(header::CONTENT_TYPE, content_type(path))
        .header(header::ACCEPT_RANGES, "bytes")
        // the screens may read a picture back from a canvas they drew it in (a wheel we make): only
        // the app's own window reaches this protocol
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, if lasting { "max-age=31536000, immutable" } else { "no-cache" });
    let asked = range.and_then(|r| r.strip_prefix("bytes=")).and_then(|r| {
        let (from, to) = r.split_once('-')?;
        let from: u64 = from.trim().parse().ok()?;
        let to = to.trim().parse::<u64>().ok().unwrap_or(from + CHUNK - 1);
        Some((from, to.min(len.saturating_sub(1))))
    });
    match asked {
        Some((from, to)) if from < len && from <= to => {
            let mut body = vec![0; (to - from + 1) as usize];
            if f.seek(SeekFrom::Start(from)).is_err() || f.read_exact(&mut body).is_err() {
                return status(StatusCode::INTERNAL_SERVER_ERROR);
            }
            builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_RANGE, format!("bytes {from}-{to}/{len}"))
                .body(body)
                .unwrap()
        }
        Some(_) => Response::builder().status(StatusCode::RANGE_NOT_SATISFIABLE).header(header::CONTENT_RANGE, format!("bytes */{len}")).body(Vec::new()).unwrap(),
        None => {
            let mut body = Vec::with_capacity(len as usize);
            if f.read_to_end(&mut body).is_err() {
                return status(StatusCode::INTERNAL_SERVER_ERROR);
            }
            builder.status(StatusCode::OK).body(body).unwrap()
        }
    }
}

static BUSY: Mutex<usize> = Mutex::new(0);
static FREE: Condvar = Condvar::new();
static PASSING: AtomicU64 = AtomicU64::new(0);

/// Where one of VPS's pictures is kept on the PC (its name its own, made safe), there yet or not.
fn vps_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix(VPS_SITE)?;
    let name = rest.rsplit('/').next().unwrap_or("");
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) || name.starts_with('.') {
        return None;
    }
    Some(data_folder()?.join("VPS").join("pictures").join(name))
}

/// One of VPS's pictures when it's on the PC already, never fetched for it.
pub fn vps_on_pc(url: &str) -> Option<PathBuf> {
    vps_path(url).filter(|p| p.is_file())
}

/// A VPS picture, from the PC when it was fetched before (its name carries the time it was made,
/// so a changed picture has a new name), else fetched first.
fn vps_picture(url: &str) -> Response<Vec<u8>> {
    match kept_vps(url) {
        Some(kept) => file(&kept, None, true),
        None => status(StatusCode::NOT_FOUND),
    }
}

/// One of VPS's pictures kept on the PC, fetched first if it isn't yet (for the screens, and for
/// PinUP Popper's folders, `popper_media`): its file, or None when it isn't VPS's or couldn't be
/// fetched.
pub fn kept_vps(url: &str) -> Option<PathBuf> {
    let kept = vps_path(url)?;
    let (dir, name) = (kept.parent()?.to_path_buf(), kept.file_name()?.to_string_lossy().into_owned());
    if !kept.exists() {
        let mut busy = BUSY.lock().unwrap();
        while *busy >= FETCHES {
            busy = FREE.wait(busy).unwrap();
        }
        *busy += 1;
        drop(busy);
        // another request may have fetched it meanwhile
        if !kept.exists() {
            let _ = std::fs::create_dir_all(&dir);
            let part = dir.join(format!("{name}.{}.part", PASSING.fetch_add(1, Ordering::Relaxed)));
            let fetched = library::vps::curl()
                .args(["--fail", "--silent", "--location", "--max-time", "60", "--output"])
                .arg(&part)
                .arg(url)
                .status()
                .is_ok_and(|s| s.success());
            if fetched {
                let _ = std::fs::rename(&part, &kept);
            } else {
                let _ = std::fs::remove_file(&part);
            }
        }
        *BUSY.lock().unwrap() -= 1;
        FREE.notify_one();
    }
    kept.is_file().then_some(kept)
}
