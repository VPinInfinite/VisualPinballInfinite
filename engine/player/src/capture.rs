//! A table's media made from its own program's windows as they show, for a table the player
//! doesn't draw (a Future Pinball table): `vpi-player --capture <parts> [--video <name>]
//! [--seconds <s>] [--programs <a.exe;b.exe>] --out <folder>`. Each part of the desktop
//! (`name:x,y,w,h`, desktop pixels, on one screen) saved as a picture, `<folder>\<name>.png`, at
//! once; then the part `--video` names recorded for `--seconds` (20 unless said), 25 pictures a
//! second, as an MP4 no wider or higher than 1920 pixels, `<folder>\<name>.mp4` — through Windows'
//! Desktop Duplication (`native/capture.cpp`), the video written as the player's previews are
//! (`pup::av::VideoWriter`). With `--programs`, a part is captured only where those programs' own
//! windows show it, the rest left out (`left out: <name>`), the video given up should another
//! window come over it. The mouse's pointer isn't in its pictures. And `vpi-player --wait-playing
//! <x,y,w,h> [--process <pid>] [--seconds <s>]`: waits until a game shown there plays, not its
//! loading screen (`wait_playing`). Ours.

use std::ffi::{c_char, c_void};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn vpi_capture_open(left: i32, top: i32, width: i32, height: i32, error: *mut c_char, error_len: i32) -> *mut c_void;
    fn vpi_capture_frame(capture: *mut c_void, rgba: *mut u8, timeout_ms: i32) -> i32;
    fn vpi_capture_close(capture: *mut c_void);
}

/// A part of the desktop being captured.
pub struct Capture {
    raw: *mut c_void,
    pub width: u32,
    pub height: u32,
}

impl Capture {
    /// The part at `x`, `y`, `width` × `height`, in desktop pixels, on one screen.
    pub fn open(x: i32, y: i32, width: u32, height: u32) -> Result<Capture, String> {
        let mut error = [0 as c_char; 256];
        // SAFETY: a buffer for the reason it failed, and its length
        let raw = unsafe { vpi_capture_open(x, y, width as i32, height as i32, error.as_mut_ptr(), error.len() as i32) };
        if raw.is_null() {
            // SAFETY: the bridge wrote a NUL-ended reason, or left it empty
            let why = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned();
            return Err(if why.is_empty() { "the screen couldn't be captured".into() } else { why });
        }
        Ok(Capture { raw, width, height })
    }

    /// Its next picture into `rgba` (its size, RGBA, rows top first), waiting up to `timeout`:
    /// whether a new one came.
    pub fn next(&mut self, rgba: &mut [u8], timeout: Duration) -> Result<bool, String> {
        if rgba.len() != self.width as usize * self.height as usize * 4 {
            return Err("a picture of another size than the part's".into());
        }
        let wait = timeout.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: our capture, and a buffer of its size
        match unsafe { vpi_capture_frame(self.raw, rgba.as_mut_ptr(), wait) } {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err("the screen can't be captured any more".into()),
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // SAFETY: our capture, closed once
        unsafe { vpi_capture_close(self.raw) };
    }
}

/// One part to capture: its name (its files'), and where it is on the desktop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The parts given as `name:x,y,w,h`, `;` between them.
pub fn parts(spec: &str) -> Result<Vec<Part>, String> {
    spec.split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| {
            let bad = || format!("a part isn't name:x,y,w,h: {p}");
            let (name, at) = p.split_once(':').ok_or_else(bad)?;
            let n: Vec<i64> = at.split(',').map(|v| v.trim().parse::<i64>()).collect::<Result<_, _>>().map_err(|_| bad())?;
            let [x, y, w, h] = n[..] else { return Err(bad()) };
            let name = name.trim();
            if name.is_empty() || name.contains(['\\', '/', ':', '.']) || w <= 0 || h <= 0 {
                return Err(bad());
            }
            Ok(Part { name: name.to_string(), x: x as i32, y: y as i32, width: w as u32, height: h as u32 })
        })
        .collect()
}

/// A video's size for a part `width` × `height`: shrunk by the smallest whole factor that brings
/// both sides to `longest` or under, each side even (as Windows' H.264 encoder takes it); and the
/// factor.
pub fn video_size(width: u32, height: u32, longest: u32) -> (u32, u32, u32) {
    let factor = width.max(height).div_ceil(longest.max(1)).max(1);
    ((width / factor) & !1, (height / factor) & !1, factor)
}

/// A picture `width` wide shrunk by a whole `factor` to `out_w` × `out_h`, each pixel the average
/// of the square it stands for.
pub fn shrink(rgba: &[u8], width: u32, factor: u32, out_w: u32, out_h: u32) -> Vec<u8> {
    if factor == 1 {
        // only cut to the even size
        let mut out = Vec::with_capacity(out_w as usize * out_h as usize * 4);
        for y in 0..out_h as usize {
            let row = y * width as usize * 4;
            out.extend_from_slice(&rgba[row..row + out_w as usize * 4]);
        }
        return out;
    }
    let (f, w) = (factor as usize, width as usize);
    let n = (f * f) as u32;
    let mut out = vec![0u8; out_w as usize * out_h as usize * 4];
    for oy in 0..out_h as usize {
        for ox in 0..out_w as usize {
            let mut sum = [0u32; 3];
            for dy in 0..f {
                let row = (oy * f + dy) * w * 4;
                for dx in 0..f {
                    let at = row + (ox * f + dx) * 4;
                    sum[0] += u32::from(rgba[at]);
                    sum[1] += u32::from(rgba[at + 1]);
                    sum[2] += u32::from(rgba[at + 2]);
                }
            }
            let o = (oy * out_w as usize + ox) * 4;
            out[o] = (sum[0] / n) as u8;
            out[o + 1] = (sum[1] / n) as u8;
            out[o + 2] = (sum[2] / n) as u8;
            out[o + 3] = 255;
        }
    }
    out
}

/// What `--capture` was asked for.
struct Asked {
    parts: Vec<Part>,
    video: Option<String>,
    seconds: f64,
    out: PathBuf,
    /// The programs whose windows a part may show, by their files' names, lower case; none, any.
    programs: Vec<String>,
}

fn asked(args: &[String]) -> Result<Asked, String> {
    const USAGE: &str = "vpi-player --capture <name:x,y,w,h;...> [--video <name>] [--seconds <s>] [--programs <a.exe;b.exe>] --out <folder>";
    let mut it = args.iter();
    let (mut parts_spec, mut video, mut seconds, mut out, mut programs) = (None, None, 20.0, None, Vec::new());
    while let Some(a) = it.next() {
        match a.as_str() {
            "--capture" => parts_spec = it.next().cloned(),
            "--video" => video = it.next().cloned(),
            "--seconds" => seconds = it.next().and_then(|s| s.parse::<f64>().ok()).filter(|s| *s > 0.0 && *s <= 120.0).ok_or(USAGE)?,
            "--out" => out = it.next().map(PathBuf::from),
            "--programs" => programs = it.next().ok_or(USAGE)?.split(';').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect(),
            _ => return Err(USAGE.into()),
        }
    }
    let parts = parts(&parts_spec.ok_or(USAGE)?)?;
    if parts.is_empty() {
        return Err(USAGE.into());
    }
    if let Some(v) = &video
        && !parts.iter().any(|p| &p.name == v)
    {
        return Err(format!("--video names no part: {v}"));
    }
    Ok(Asked { parts, video, seconds, out: out.ok_or(USAGE)?, programs })
}

/// `vpi-player --capture …`: each part's picture, then the video asked for. What's written is said
/// on its own line (`picture: <file>`, `video: <file>`), and each part left out (`left out: <name>:
/// <why>`).
pub fn run(args: &[String]) -> Result<(), String> {
    let a = asked(args)?;
    std::fs::create_dir_all(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
    for p in &a.parts {
        if let Err(why) = shown_by(p, &a.programs) {
            if a.video.as_deref() == Some(p.name.as_str()) {
                return Err(format!("{}: {why}", p.name));
            }
            println!("left out: {}: {why}", p.name);
            continue;
        }
        let mut cap = Capture::open(p.x, p.y, p.width, p.height).map_err(|e| format!("{}: {e}", p.name))?;
        let mut rgba = vec![0u8; p.width as usize * p.height as usize * 4];
        // the screen's picture as it is: the first a new duplication hands over
        let deadline = Instant::now() + Duration::from_secs(12);
        let mut got = false;
        while !got && Instant::now() < deadline {
            got = cap.next(&mut rgba, Duration::from_millis(500))?;
        }
        if !got {
            return Err(format!("{}: nothing showed there to capture", p.name));
        }
        let file = a.out.join(format!("{}.png", p.name));
        image::save_buffer(&file, &rgba, p.width, p.height, image::ExtendedColorType::Rgba8).map_err(|e| format!("{}: {e}", file.display()))?;
        println!("picture: {}", file.display());
    }
    if let Some(name) = &a.video {
        let p = a.parts.iter().find(|p| &p.name == name).ok_or("no such part")?;
        let file = a.out.join(format!("{name}.mp4"));
        if let Err(e) = record(p, a.seconds, &file, &a.programs) {
            let _ = std::fs::remove_file(&file);
            return Err(e);
        }
        println!("video: {}", file.display());
    }
    Ok(())
}

/// A part recorded for `seconds`, 25 pictures a second — each the newest the screen showed by its
/// moment — into an MP4 no side past 1920 pixels; given up should a window of none of `programs`
/// come over it (looked at each second).
fn record(p: &Part, seconds: f64, file: &Path, programs: &[String]) -> Result<(), String> {
    const FPS: u32 = 25;
    let (out_w, out_h, factor) = video_size(p.width, p.height, 1920);
    let mut cap = Capture::open(p.x, p.y, p.width, p.height)?;
    let mut writer = crate::pup::av::VideoWriter::create(file, out_w, out_h, FPS, None)?;
    let mut rgba = vec![0u8; p.width as usize * p.height as usize * 4];
    // its first picture before the clock starts
    let first = Instant::now() + Duration::from_secs(12);
    while !cap.next(&mut rgba, Duration::from_millis(500))? {
        if Instant::now() > first {
            return Err(format!("{}: nothing showed there to record", p.name));
        }
    }
    let total = (seconds * f64::from(FPS)).round() as u32;
    let step = Duration::from_secs_f64(1.0 / f64::from(FPS));
    let start = Instant::now();
    for n in 0..total {
        let due = start + step * n;
        // the newest picture by its moment
        loop {
            let now = Instant::now();
            if now >= due {
                break;
            }
            cap.next(&mut rgba, due - now)?;
        }
        if n % FPS == 0 {
            shown_by(p, programs).map_err(|why| format!("{}: {why}", p.name))?;
        }
        writer.frame(&shrink(&rgba, p.width, factor, out_w, out_h))?;
    }
    writer.finish()
}

/// Whether a part shows only what `programs` show: the window at its middle and near each of its
/// corners (an eighth of its size in) one of theirs — its top-level window's program, by its file's
/// name — or why not. Any window where `programs` names none.
pub fn shown_by(p: &Part, programs: &[String]) -> Result<(), String> {
    if programs.is_empty() {
        return Ok(());
    }
    let (w, h) = (p.width as i32, p.height as i32);
    let points = [(w / 2, h / 2), (w / 8, h / 8), (w - w / 8, h / 8), (w / 8, h - h / 8), (w - w / 8, h - h / 8)];
    for (dx, dy) in points {
        match program_at(p.x + dx, p.y + dy) {
            Some(name) if programs.iter().any(|wanted| *wanted == name) => {}
            Some(name) => return Err(format!("another program's window shows there ({name})")),
            None => return Err("no program's window shows there".into()),
        }
    }
    Ok(())
}

/// The program whose window shows at a place on the desktop (desktop pixels): its file's name, lower
/// case.
#[cfg(windows)]
fn program_at(x: i32, y: i32) -> Option<String> {
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetThreadDpiAwarenessContext(context: isize) -> isize;
        fn WindowFromPoint(point: Point) -> *mut c_void;
        fn GetAncestor(hwnd: *mut c_void, flags: u32) -> *mut c_void;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(process: *mut c_void, flags: u32, name: *mut u16, len: *mut u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: the desktop in its own pixels, whatever Windows
    // scales a screen by
    const PER_MONITOR_V2: isize = -4;
    const GA_ROOT: u32 = 2;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    // SAFETY: this thread's awareness set; a place on the desktop asked of Windows
    let hwnd = unsafe {
        SetThreadDpiAwarenessContext(PER_MONITOR_V2);
        WindowFromPoint(Point { x, y })
    };
    if hwnd.is_null() {
        return None;
    }
    // SAFETY: a window Windows found
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let mut pid = 0u32;
    // SAFETY: a window Windows found, a number to fill
    unsafe { GetWindowThreadProcessId(if root.is_null() { hwnd } else { root }, &mut pid) };
    if pid == 0 {
        return None;
    }
    // SAFETY: a process asked for its name alone
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut name = [0u16; 1024];
    let mut len = name.len() as u32;
    // SAFETY: the process opened above, a buffer of `len` characters
    let named = unsafe { QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut len) } != 0;
    // SAFETY: the handle opened above
    unsafe { CloseHandle(process) };
    if !named {
        return None;
    }
    let path = String::from_utf16_lossy(&name[..len as usize]);
    Path::new(&path).file_name().map(|n| n.to_string_lossy().to_lowercase())
}

#[cfg(not(windows))]
fn program_at(_x: i32, _y: i32) -> Option<String> {
    None
}

/// How a game's screen is told to be playing, not loading its table: its picture changing in at
/// least this many of `ROWS` bands across it, within each of `MOVING_SPANS` windows of time in a
/// row (`SPAN`) — a table's attract mode lights its playfield all over, a loading screen moves its
/// bar and its line of text alone, and fades in or out over a moment.
const MOVING_ROWS: u32 = 5;
const MOVING_SPANS: u32 = 3;

/// The bands and columns a screen's picture is cut into, to be compared.
const ROWS: usize = 18;
const COLUMNS: usize = 32;

/// The time a picture's changes are gathered over.
const SPAN: Duration = Duration::from_millis(1500);

/// A picture's average brightness in each of its `ROWS` × `COLUMNS` cells, from a pixel in every
/// eight across and down: rows first.
pub fn cells(rgba: &[u8], width: u32, height: u32) -> Vec<u16> {
    let (w, h) = (width as usize, height as usize);
    let mut sum = vec![0u32; ROWS * COLUMNS];
    let mut count = vec![0u32; ROWS * COLUMNS];
    for y in (0..h).step_by(8) {
        let row = (y * ROWS / h.max(1)).min(ROWS - 1);
        for x in (0..w).step_by(8) {
            let at = (y * w + x) * 4;
            let Some(px) = rgba.get(at..at + 3) else { continue };
            // the eye's brightness, roughly
            let lum = (u32::from(px[0]) * 3 + u32::from(px[1]) * 6 + u32::from(px[2])) / 10;
            let cell = row * COLUMNS + (x * COLUMNS / w.max(1)).min(COLUMNS - 1);
            sum[cell] += lum;
            count[cell] += 1;
        }
    }
    sum.iter().zip(&count).map(|(s, n)| if *n == 0 { 0 } else { (s / n) as u16 }).collect()
}

/// The bands (bits, the top band first) in which a cell's brightness moved by more than a few steps
/// between two pictures' cells.
pub fn moved_rows(before: &[u16], after: &[u16]) -> u32 {
    let mut rows = 0u32;
    for (i, (a, b)) in before.iter().zip(after).enumerate() {
        if a.abs_diff(*b) > 6 {
            rows |= 1 << (i / COLUMNS);
        }
    }
    rows
}

/// `vpi-player --wait-playing <x,y,w,h> [--process <pid>] [--seconds <s>]`: waits until the game
/// shown at that part of the desktop plays — its picture moving across much of it (`MOVING_ROWS`),
/// as a table's attract mode does, not in a band or two as a loading screen's bar does — or, a
/// picture moving that little all along (an attract mode with nothing lit changing), until the
/// game's program (`--process`) has answered Windows for half a minute, three quarters of a minute
/// in. Up to `--seconds` (180 unless said). Says why it took it as playing on its own line
/// (`playing: moving` or `playing: answering`).
pub fn wait_playing(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "vpi-player --wait-playing <x,y,w,h> [--process <pid>] [--seconds <s>]";
    let mut it = args.iter();
    let (mut at, mut process, mut seconds) = (None, None, 180u64);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--wait-playing" => at = it.next().cloned(),
            "--process" => process = Some(it.next().and_then(|p| p.parse::<u32>().ok()).ok_or(USAGE)?),
            "--seconds" => seconds = it.next().and_then(|s| s.parse::<u64>().ok()).filter(|s| *s > 0 && *s <= 600).ok_or(USAGE)?,
            _ => return Err(USAGE.into()),
        }
    }
    let part = parts(&format!("game:{}", at.ok_or(USAGE)?))?.remove(0);
    let mut cap = Capture::open(part.x, part.y, part.width, part.height)?;
    let mut rgba = vec![0u8; part.width as usize * part.height as usize * 4];
    let started = Instant::now();
    let deadline = started + Duration::from_secs(seconds);
    let mut before: Option<Vec<u16>> = None;
    let (mut rows, mut span_from, mut spans_moving) = (0u32, Instant::now(), 0u32);
    let mut answering_since: Option<Instant> = None;
    loop {
        std::thread::sleep(Duration::from_millis(250));
        // the newest picture (the one before where nothing has changed)
        cap.next(&mut rgba, Duration::from_millis(50))?;
        let now_cells = cells(&rgba, part.width, part.height);
        if let Some(b) = &before {
            rows |= moved_rows(b, &now_cells);
        }
        before = Some(now_cells);
        if span_from.elapsed() >= SPAN {
            spans_moving = if rows.count_ones() >= MOVING_ROWS { spans_moving + 1 } else { 0 };
            rows = 0;
            span_from = Instant::now();
            if spans_moving >= MOVING_SPANS {
                println!("playing: moving");
                return Ok(());
            }
        }
        if let Some(pid) = process {
            if program_answers(pid) {
                answering_since.get_or_insert_with(Instant::now);
            } else {
                answering_since = None;
            }
            if started.elapsed() >= Duration::from_secs(45) && answering_since.is_some_and(|s| s.elapsed() >= Duration::from_secs(30)) {
                println!("playing: answering");
                return Ok(());
            }
        }
        if Instant::now() > deadline {
            return Err("the table never started playing".into());
        }
    }
}

/// Whether a program's largest window shown answers Windows (a message asking nothing handled
/// within a quarter of a second): a program loading on its windows' thread doesn't.
#[cfg(windows)]
fn program_answers(pid: u32) -> bool {
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(each: unsafe extern "system" fn(*mut c_void, isize) -> i32, data: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: *mut c_void) -> i32;
        fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
        fn SendMessageTimeoutW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize, flags: u32, timeout: u32, result: *mut usize) -> isize;
    }
    struct Search {
        pid: u32,
        largest: (*mut c_void, i64),
    }
    unsafe extern "system" fn each(hwnd: *mut c_void, data: isize) -> i32 {
        // SAFETY: `data` is the search below, alive through EnumWindows
        let search = unsafe { &mut *(data as *mut Search) };
        let mut pid = 0u32;
        // SAFETY: a window Windows lists, a number to fill
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        // SAFETY: the same window
        if pid != search.pid || unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let mut r = Rect::default();
        // SAFETY: the same window, a rectangle to fill
        if unsafe { GetWindowRect(hwnd, &mut r) } != 0 {
            let area = i64::from(r.right - r.left) * i64::from(r.bottom - r.top);
            if area > search.largest.1 {
                search.largest = (hwnd, area);
            }
        }
        1
    }
    let mut search = Search { pid, largest: (std::ptr::null_mut(), 0) };
    // SAFETY: the callback above, handed the search for the call's length
    unsafe { EnumWindows(each, &mut search as *mut Search as isize) };
    if search.largest.0.is_null() {
        return false;
    }
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    let mut result = 0usize;
    // SAFETY: a window Windows found; a message that changes nothing; a number to fill
    unsafe { SendMessageTimeoutW(search.largest.0, 0, 0, 0, SMTO_ABORTIFHUNG, 250, &mut result) != 0 }
}

#[cfg(not(windows))]
fn program_answers(_pid: u32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_read_and_their_videos_sized() {
        let p = parts("playfield:0,0,3840,2160; playfield-backglass:-3440,0,1920,1440").unwrap();
        assert_eq!(p[1], Part { name: "playfield-backglass".into(), x: -3440, y: 0, width: 1920, height: 1440 });
        assert!(parts("playfield:0,0,3840").is_err());
        assert!(parts("../x:0,0,10,10").is_err());
        // a 4K screen halved, an even size kept, an odd one cut by a pixel
        assert_eq!(video_size(3840, 2160, 1920), (1920, 1080, 2));
        assert_eq!(video_size(1080, 1920, 1920), (1080, 1920, 1));
        assert_eq!(video_size(1081, 1919, 1920), (1080, 1918, 1));
        assert_eq!(video_size(3440, 1440, 1920), (1720, 720, 2));
    }

    /// A loading screen's bar and line of text move in a band or two; a table's lights all over it.
    #[test]
    fn a_table_playing_told_from_its_loading_screen() {
        let (w, h) = (320u32, 180u32);
        let blank = vec![0u8; (w * h * 4) as usize];
        let paint = |rgba: &mut Vec<u8>, x0: u32, y0: u32, x1: u32, y1: u32| {
            for y in y0..y1 {
                for x in x0..x1 {
                    let at = ((y * w + x) * 4) as usize;
                    rgba[at..at + 3].copy_from_slice(&[255, 255, 255]);
                }
            }
        };
        let before = cells(&blank, w, h);
        // a loading bar across the screen, a tenth of the way up from its bottom, and its text
        let mut loading = blank.clone();
        paint(&mut loading, 20, 150, 300, 158);
        paint(&mut loading, 100, 165, 220, 172);
        let rows = moved_rows(&before, &cells(&loading, w, h));
        assert!(rows.count_ones() < MOVING_ROWS, "{rows:b}");
        // lights lit from the top of the playfield to its bottom
        let mut lit = blank.clone();
        for i in 0..9 {
            paint(&mut lit, 40 + i * 25, 5 + i * 19, 56 + i * 25, 21 + i * 19);
        }
        let rows = moved_rows(&before, &cells(&lit, w, h));
        assert!(rows.count_ones() >= MOVING_ROWS, "{rows:b}");
        // nothing changed, nothing moved
        assert_eq!(moved_rows(&before, &before), 0);
    }

    #[test]
    fn a_picture_shrunk_by_the_average_of_each_square() {
        // 4 × 2, halved: each 2 × 2 square's average
        let mut rgba = Vec::new();
        for v in [0u8, 100, 200, 40, 20, 60, 0, 120] {
            rgba.extend_from_slice(&[v, v, v, 255]);
        }
        let out = shrink(&rgba, 4, 2, 2, 1);
        assert_eq!(out, vec![45, 45, 45, 255, 90, 90, 90, 255]);
        // whole, a pixel cut off each row to an even width
        let out = shrink(&rgba[..12], 3, 1, 2, 1);
        assert_eq!(out, rgba[..8].to_vec());
    }
}
