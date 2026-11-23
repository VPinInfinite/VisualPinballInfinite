//! Screenshots while VPX plays: a key VPX leaves free (`VPI_SCREENSHOT_KEY`, a Windows virtual-key
//! code; F9 where it's unset) saves each of VPX's windows as it shows — the playfield, the
//! backglass, the score display and the topper, told by the titles VPX gives them, and the classic
//! B2S Server's backglass — into the table's folder of screenshots (`VPI_SCREENSHOTS`), each named
//! for the moment and its window (`YYYY-MM-DD HH-MM-SS playfield.png`), as the app's table page
//! reads them; and, where the app asks (`VPI_SCREENSHOT_AT_HIGH_SCORE=1`), when a game makes the
//! machine's high scores. The pictures are taken on a thread of their own, so VPX's frames never
//! wait. Ours.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Screenshots being written, on their threads: waited for before VPX lets the add-on go.
static RUNNING: AtomicUsize = AtomicUsize::new(0);

/// Every screenshot being written finished, or `most` passed.
pub fn wait_for_all(most: std::time::Duration) {
    let until = std::time::Instant::now() + most;
    while RUNNING.load(Ordering::Acquire) > 0 && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// F9: no key of VPX 10.8.1's own by default.
const F9: i32 = 0x78;

/// VPX's windows by their titles, and the names a screenshot gives them; the classic B2S Server's
/// backglass windows (another program's) as the backglass.
const WINDOWS: [(&str, &str); 6] = [
    ("Visual Pinball Player", "playfield"),
    ("Visual Pinball Backglass", "backglass"),
    ("Visual Pinball Score View", "score display"),
    ("Visual Pinball Topper", "topper"),
    ("B2S Backglass Server", "backglass"),
    ("B2S Backglass", "backglass"),
];

pub struct Shots {
    folder: PathBuf,
    key: i32,
    down: bool,
    /// For checks, where `VPI_SCREENSHOT_AT_MS` asks: one taken once the game's clock reaches it.
    at_ms: Option<u64>,
    /// One taken when a game makes the machine's high scores.
    pub at_high_score: bool,
    /// For a script keeping its own high scores, the lowest on its list
    /// (`VPI_SCREENSHOT_HIGH_SCORE_FROM`): a game its backglass tells over past it made one.
    pub high_score_from: Option<u64>,
}

impl Shots {
    /// Screenshots as the app asks for them, where it does.
    pub fn from_env() -> Option<Shots> {
        let folder = PathBuf::from(std::env::var_os("VPI_SCREENSHOTS").filter(|f| !f.is_empty())?);
        let key = std::env::var("VPI_SCREENSHOT_KEY").ok().and_then(|k| k.trim().parse::<i32>().ok()).filter(|k| (1..=0xFE).contains(k)).unwrap_or(F9);
        let at_ms = std::env::var("VPI_SCREENSHOT_AT_MS").ok().and_then(|v| v.trim().parse().ok());
        let at_high_score = std::env::var_os("VPI_SCREENSHOT_AT_HIGH_SCORE").is_some_and(|v| v == "1");
        let high_score_from = std::env::var("VPI_SCREENSHOT_HIGH_SCORE_FROM").ok().and_then(|v| v.trim().parse().ok()).filter(|_| at_high_score);
        Some(Shots { folder, key, down: false, at_ms, at_high_score, high_score_from })
    }

    /// The screenshots taken now, on a thread of their own.
    pub fn take_now(&self) {
        let folder = self.folder.clone();
        RUNNING.fetch_add(1, Ordering::AcqRel);
        let spawned = std::thread::Builder::new().name("vpi-screenshots".into()).spawn(move || {
            take(&folder);
            RUNNING.fetch_sub(1, Ordering::AcqRel);
        });
        if spawned.is_err() {
            RUNNING.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Each frame, at `now` on the game's clock: the key pressed (not held) takes the screenshots.
    /// Whether it did.
    pub fn poll(&mut self, now: u64) -> bool {
        let down = key_down(self.key);
        let mut pressed = down && !self.down;
        self.down = down;
        if self.at_ms.is_some_and(|at| now >= at) {
            self.at_ms = None;
            pressed = true;
        }
        if pressed {
            self.take_now();
        }
        pressed
    }
}

/// The moment, as a screenshot's name starts: `YYYY-MM-DD HH-MM-SS`.
fn stamp() -> String {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct SystemTime {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            ms: u16,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetLocalTime(t: *mut SystemTime);
        }
        let mut t = SystemTime::default();
        // SAFETY: Windows fills the struct it's handed
        unsafe { GetLocalTime(&mut t) };
        format!("{:04}-{:02}-{:02} {:02}-{:02}-{:02}", t.year, t.month, t.day, t.hour, t.minute, t.second)
    }
    #[cfg(not(windows))]
    String::from("0000-00-00 00-00-00")
}

/// Each of VPX's windows that shows saved as a picture in `folder`. How many were saved.
pub fn take(folder: &std::path::Path) -> usize {
    let stamp = stamp();
    let _ = std::fs::create_dir_all(folder);
    let mut saved = 0;
    let mut named: Vec<String> = Vec::new();
    for (window, name) in windows() {
        // one picture a window name: VPX's own backglass before the classic one
        if named.contains(&name) {
            continue;
        }
        let Some((width, height, rgba)) = capture(window) else { continue };
        let file = folder.join(format!("{stamp} {name}.png"));
        if write_png(&file, width, height, &rgba).is_ok() {
            named.push(name);
            saved += 1;
        }
    }
    saved
}

/// A picture written as a PNG.
fn write_png(file: &std::path::Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let out = std::fs::File::create(file).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(out), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    pub type Hwnd = *mut c_void;

    #[repr(C)]
    #[derive(Default)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    pub struct BitmapInfoHeader {
        pub size: u32,
        pub width: i32,
        pub height: i32,
        pub planes: u16,
        pub bit_count: u16,
        pub compression: u32,
        pub size_image: u32,
        pub x_ppm: i32,
        pub y_ppm: i32,
        pub clr_used: u32,
        pub clr_important: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn EnumWindows(each: unsafe extern "system" fn(Hwnd, isize) -> i32, data: isize) -> i32;
        pub fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        pub fn IsWindowVisible(hwnd: Hwnd) -> i32;
        pub fn IsIconic(hwnd: Hwnd) -> i32;
        pub fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, max: i32) -> i32;
        pub fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        pub fn GetDC(hwnd: Hwnd) -> *mut c_void;
        pub fn ReleaseDC(hwnd: Hwnd, dc: *mut c_void) -> i32;
        pub fn PrintWindow(hwnd: Hwnd, dc: *mut c_void, flags: u32) -> i32;
        pub fn GetAsyncKeyState(key: i32) -> i16;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        pub fn CreateCompatibleDC(dc: *mut c_void) -> *mut c_void;
        pub fn CreateDIBSection(dc: *mut c_void, info: *const BitmapInfoHeader, usage: u32, bits: *mut *mut c_void, section: *mut c_void, offset: u32) -> *mut c_void;
        pub fn SelectObject(dc: *mut c_void, object: *mut c_void) -> *mut c_void;
        pub fn DeleteObject(object: *mut c_void) -> i32;
        pub fn DeleteDC(dc: *mut c_void) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetCurrentProcessId() -> u32;
    }
}

/// Whether the key is down now.
#[cfg(windows)]
fn key_down(key: i32) -> bool {
    // SAFETY: a plain read of the key's state
    (unsafe { win::GetAsyncKeyState(key) } as u16 & 0x8000) != 0
}

#[cfg(not(windows))]
fn key_down(_key: i32) -> bool {
    false
}

/// The windows to take: VPX's own (this program's, by their titles) and the classic B2S Server's
/// backglass (by its title, whatever program shows it), each shown and not minimised, with the
/// name its picture takes.
#[cfg(windows)]
fn windows() -> Vec<(usize, String)> {
    unsafe extern "system" fn each(hwnd: win::Hwnd, data: isize) -> i32 {
        // SAFETY: `data` is the list below, alive through EnumWindows
        let found = unsafe { &mut *(data as *mut Vec<(usize, String)>) };
        // SAFETY: a window Windows handed over; a buffer of the size given
        unsafe {
            if win::IsWindowVisible(hwnd) == 0 || win::IsIconic(hwnd) != 0 {
                return 1;
            }
            let mut text = [0u16; 256];
            let n = win::GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
            let title = String::from_utf16_lossy(&text[..n]);
            let mut pid = 0;
            win::GetWindowThreadProcessId(hwnd, &mut pid);
            let ours = pid == win::GetCurrentProcessId();
            if let Some((_, name)) = WINDOWS.iter().enumerate().find(|(i, (t, _))| *t == title && (ours || *i >= 4)).map(|(_, w)| *w) {
                found.push((hwnd as usize, name.to_string()));
            }
        }
        1
    }
    let mut found: Vec<(usize, String)> = Vec::new();
    // SAFETY: the callback reads only the list, which outlives the call
    unsafe { win::EnumWindows(each, &mut found as *mut Vec<(usize, String)> as isize) };
    // VPX's own first, in the order of WINDOWS
    found.sort_by_key(|(_, name)| WINDOWS.iter().position(|(_, n)| n == name).unwrap_or(WINDOWS.len()));
    found
}

#[cfg(not(windows))]
fn windows() -> Vec<(usize, String)> {
    Vec::new()
}

/// A window's inside as it shows, as rows of RGBA from the top: Windows' own picture of it, asked
/// for whole (`PW_RENDERFULLCONTENT`, which takes what DirectX draws).
#[cfg(windows)]
fn capture(window: usize) -> Option<(u32, u32, Vec<u8>)> {
    let hwnd = window as win::Hwnd;
    let mut rect = win::Rect::default();
    // SAFETY: the window's handle, a rect to fill
    if unsafe { win::GetClientRect(hwnd, &mut rect) } == 0 {
        return None;
    }
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    if width <= 0 || height <= 0 || width > 16384 || height > 16384 {
        return None;
    }
    let info = win::BitmapInfoHeader { size: std::mem::size_of::<win::BitmapInfoHeader>() as u32, width, height: -height, planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
    // SAFETY: each object made here is let go below, once; `bits` points into the DIB section while
    // it lives, `width` × `height` × 4 bytes, top row first
    unsafe {
        let screen = win::GetDC(hwnd);
        let dc = win::CreateCompatibleDC(screen);
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = win::CreateDIBSection(dc, &info, 0, &mut bits, std::ptr::null_mut(), 0);
        let mut out = None;
        if !bitmap.is_null() && !bits.is_null() {
            let before = win::SelectObject(dc, bitmap);
            // PW_CLIENTONLY | PW_RENDERFULLCONTENT
            if win::PrintWindow(hwnd, dc, 1 | 2) != 0 {
                let bgra = std::slice::from_raw_parts(bits as *const u8, (width * height * 4) as usize);
                let mut rgba = Vec::with_capacity(bgra.len());
                for [b, g, r, _] in bgra.as_chunks::<4>().0 {
                    rgba.extend_from_slice(&[*r, *g, *b, 255]);
                }
                out = Some((width as u32, height as u32, rgba));
            }
            win::SelectObject(dc, before);
            win::DeleteObject(bitmap);
        }
        win::DeleteDC(dc);
        win::ReleaseDC(hwnd, screen);
        out
    }
}

#[cfg(not(windows))]
fn capture(_window: usize) -> Option<(u32, u32, Vec<u8>)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_named_as_the_app_reads_it() {
        let s = stamp();
        assert_eq!(s.len(), "YYYY-MM-DD HH-MM-SS".len());
        assert_eq!(s.matches(' ').count(), 1);
    }

    #[test]
    fn a_picture_written_as_a_png() {
        let file = std::env::temp_dir().join(format!("vpi-shot-test-{}.png", std::process::id()));
        write_png(&file, 2, 1, &[255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
        let bytes = std::fs::read(&file).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        let _ = std::fs::remove_file(&file);
    }
}
