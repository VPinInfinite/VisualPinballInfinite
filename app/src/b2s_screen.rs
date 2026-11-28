//! B2S Server's screen file kept as Settings › Screens has the screens. Where the classic B2S
//! Server shows the backglasses — VPX 10.8.0, or VPX 10.8.1 with its own backglass add-on off — it
//! puts its windows where its own screen file says (`library::b2s_screen`), and VPX's backglass
//! window settings place nothing of it. So that file is made to say where the page has the
//! backglass, when the page saves and before a table plays, only where it says something else
//! (its second window, for a backglass file's score panel, left as its own settings have it), the
//! file before
//! kept beside it (`ScreenRes.previous.txt`): B2S Server's own folder's, which it reads last, and
//! the tables folder's, which it reads first for every table there. A table's own (`<table>.res`,
//! a backglass's) is left as it is.

use std::path::{Path, PathBuf};

use ::library::b2s_screen::{FILE_NAME, Place, Rect, with_place};

use crate::screens::{Screen, ScreenSettings, WindowPlace};

/// Whether B2S Server shows the backglasses VPX plays: it's installed, and that VPX takes no
/// add-ons (10.8.0) or has its own backglass add-on off (`[Plugin.B2S]` `Enable`).
fn shows_backglasses(takes_addons: bool, entries: &[(String, String, String)]) -> bool {
    crate::addons::registered("B2S.Server") && (!takes_addons || vpi_settings::get(entries, "Plugin.B2S", "Enable").and_then(vpi_settings::vpx_bool) != Some(true))
}

/// A text value of the registry (`root`: `HKEY_CLASSES_ROOT` or `HKEY_CURRENT_USER`), the 64-bit
/// view first.
#[cfg(windows)]
pub(crate) fn registry_text(root: isize, key: &str, value: &str) -> Option<String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(key: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut std::ffi::c_void, size: *mut u32) -> i32;
    }
    // RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ; the 64-bit view, the 32-bit one, or the default
    const TEXT: u32 = 0x2 | 0x4;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (k, v) = (wide(key), wide(value));
    [0x0001_0000u32, 0x0002_0000, 0].into_iter().find_map(|view| {
        let mut buffer = vec![0u16; 2048];
        let mut bytes = (buffer.len() * 2) as u32;
        // SAFETY: NUL-terminated names, a buffer of `bytes` bytes
        let r = unsafe { RegGetValueW(root, k.as_ptr(), if value.is_empty() { std::ptr::null() } else { v.as_ptr() }, TEXT | view, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut bytes) };
        (r == 0).then(|| {
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            String::from_utf16_lossy(&buffer[..len])
        })
    })
}

#[cfg(windows)]
pub(crate) const HKEY_CLASSES_ROOT: isize = 0x8000_0000_u32 as i32 as isize;
#[cfg(windows)]
pub(crate) const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;

/// The folder B2S Server is installed in: its program's, as Windows has it registered for
/// `B2S.Server` (a .NET program's `CodeBase`, a `file:` address).
#[cfg(windows)]
fn b2s_folder() -> Option<PathBuf> {
    let clsid = registry_text(HKEY_CLASSES_ROOT, r"B2S.Server\CLSID", "")?;
    let code_base = registry_text(HKEY_CLASSES_ROOT, &format!(r"CLSID\{}\InprocServer32", clsid.trim()), "CodeBase")?;
    let program = path_of_address(&code_base)?;
    program.parent().filter(|p| p.is_dir()).map(Path::to_path_buf)
}

#[cfg(not(windows))]
fn b2s_folder() -> Option<PathBuf> {
    None
}

/// A `file:` address's path (`file:///C:/vPinball/Tables/B2SBackglassServer.DLL`), any `%xx` in it
/// read back.
fn path_of_address(address: &str) -> Option<PathBuf> {
    let a = address.trim();
    let rest = a.strip_prefix("file:///").or_else(|| a.strip_prefix("file://")).or_else(|| a.strip_prefix("file:"))?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(b) = rest.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'/' { b'\\' } else { bytes[i] });
        i += 1;
    }
    Some(PathBuf::from(String::from_utf8_lossy(&out).into_owned()))
}

/// The screen file's name: B2S Server's own setting's, where it renames it
/// (`HKEY_CURRENT_USER\Software\B2S` `B2SScreenResFileNameOverride`).
fn file_name() -> String {
    #[cfg(windows)]
    if let Some(name) = registry_text(HKEY_CURRENT_USER, r"Software\B2S", "B2SScreenResFileNameOverride").map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
        return name;
    }
    FILE_NAME.into()
}

/// Windows' screens: each by its top-left corner in pixels, as the player lists the screens, its
/// number (`\\.\DISPLAY<n>`) and its size in pixels.
#[cfg(windows)]
pub(crate) fn screens() -> Vec<((i32, i32), u32, (u32, u32))> {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct MonitorInfo {
        size: u32,
        monitor: WinRect,
        work: WinRect,
        flags: u32,
        device: [u16; 32],
    }
    type Each = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut WinRect, isize) -> i32;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumDisplayMonitors(dc: *mut c_void, clip: *const WinRect, each: Each, data: isize) -> i32;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn SetThreadDpiAwarenessContext(context: isize) -> isize;
    }
    unsafe extern "system" fn each(monitor: *mut c_void, _: *mut c_void, _: *mut WinRect, data: isize) -> i32 {
        // SAFETY: `data` is the list handed to EnumDisplayMonitors below, alive for its call
        let out = unsafe { &mut *(data as *mut Vec<((i32, i32), u32, (u32, u32))>) };
        let mut info = MonitorInfo { size: size_of::<MonitorInfo>() as u32, monitor: WinRect::default(), work: WinRect::default(), flags: 0, device: [0; 32] };
        // SAFETY: a monitor Windows handed over, and a MONITORINFOEXW with its size set
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            let len = info.device.iter().position(|&c| c == 0).unwrap_or(info.device.len());
            let device = String::from_utf16_lossy(&info.device[..len]);
            if let Some(n) = device.strip_prefix(r"\\.\DISPLAY").and_then(|n| n.parse().ok()) {
                let m = &info.monitor;
                out.push(((m.left, m.top), n, ((m.right - m.left) as u32, (m.bottom - m.top) as u32)));
            }
        }
        1
    }
    let mut out: Vec<((i32, i32), u32, (u32, u32))> = Vec::new();
    // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: the screens' places in pixels, whatever their scale
    // SAFETY: the thread's own awareness, put back after
    let before = unsafe { SetThreadDpiAwarenessContext(-4) };
    // SAFETY: the whole desktop, a callback above, the list it fills
    unsafe { EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), each, (&raw mut out) as isize) };
    if before != 0 {
        // SAFETY: the awareness the thread had
        unsafe { SetThreadDpiAwarenessContext(before) };
    }
    out
}

/// Windows' scale for the screen at `(x, y)` on the desktop, in pixels, in percent: none where no
/// screen is there.
#[cfg(windows)]
pub(crate) fn scale_at(x: i32, y: i32) -> Option<f32> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MonitorFromPoint(point: Point, flags: u32) -> *mut c_void;
        fn SetThreadDpiAwarenessContext(context: isize) -> isize;
    }
    #[link(name = "shcore")]
    unsafe extern "system" {
        fn GetDpiForMonitor(monitor: *mut c_void, kind: u32, x: *mut u32, y: *mut u32) -> i32;
    }
    // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, as `screens` places them
    // SAFETY: the thread's own awareness, put back after
    let before = unsafe { SetThreadDpiAwarenessContext(-4) };
    // MONITOR_DEFAULTTONULL: none where no screen is
    // SAFETY: a point on the desktop
    let monitor = unsafe { MonitorFromPoint(Point { x, y }, 0) };
    let (mut dx, mut dy) = (0u32, 0u32);
    // MDT_EFFECTIVE_DPI, the scale Windows' display settings set
    // SAFETY: a monitor Windows handed over, two numbers to fill
    let found = !monitor.is_null() && unsafe { GetDpiForMonitor(monitor, 0, &mut dx, &mut dy) } == 0;
    if before != 0 {
        // SAFETY: the awareness the thread had
        unsafe { SetThreadDpiAwarenessContext(before) };
    }
    found.then(|| dx as f32 / 96.0 * 100.0)
}

#[cfg(not(windows))]
pub(crate) fn scale_at(_x: i32, _y: i32) -> Option<f32> {
    None
}

#[cfg(not(windows))]
pub(crate) fn screens() -> Vec<((i32, i32), u32, (u32, u32))> {
    Vec::new()
}

/// Windows' number for each screen (`\\.\DISPLAY<n>`), by its top-left corner in pixels.
fn display_numbers() -> Vec<((i32, i32), u32)> {
    screens().into_iter().map(|(at, n, _)| (at, n)).collect()
}

/// Where the page's settings put B2S Server's backglass: on its own window's place, or over its place
/// inside the playfield's window. None where the backglass shows nowhere or its screen can't be told.
/// B2S Server's second window, its DMD window (a backglass file's score panel), stays where B2S
/// Server's own settings put it: placed on the score window's place, it showed that picture over
/// the playfield on a desktop, and on a screen at another scale than the backglass's, B2S Server
/// enlarged it by that scale as it reached that screen, past its edge.
fn place_of(settings: &ScreenSettings, screens: &[Screen], numbers: &[((i32, i32), u32)]) -> Option<Place> {
    let find = |id: &str| settings.windows.iter().find(|w| w.id == id);
    let corner = |name: &str| screens.iter().find(|s| s.name == name).map(|s| (s.x, s.y));
    let playfield = find("playfield")?;
    let pf_corner = corner(&playfield.screen)?;
    // a window's screen, by its top-left corner, and its place in that screen
    let at = |w: &WindowPlace| match w.output.as_str() {
        "window" => corner(&w.screen).map(|c| (c, w.x, w.y)),
        "inside" => Some((pf_corner, playfield.x + w.x, playfield.y + w.y)),
        _ => None,
    };
    let backglass = find("backglass")?;
    let ((sx, sy), x, y) = at(backglass)?;
    let number = numbers.iter().find(|(c, _)| *c == (sx, sy))?.1;
    Some(Place { playfield: (playfield.width, playfield.height), screen: number, backglass: Rect { x, y, width: backglass.width, height: backglass.height }, dmd: None })
}

/// The screen files B2S Server reads for every table: its own folder's, and the tables folder's
/// where one's there.
fn files(tables: Option<&Path>) -> Vec<PathBuf> {
    let name = file_name();
    let mut out: Vec<PathBuf> = b2s_folder().map(|f| f.join(&name)).into_iter().collect();
    if let Some(t) = tables.map(|t| t.join(&name)).filter(|p| p.is_file())
        && !out.iter().any(|o| same_file(o, &t))
    {
        out.push(t);
    }
    out
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a.as_os_str().eq_ignore_ascii_case(b.as_os_str()),
    }
}

/// The file before a change, kept beside it: `ScreenRes.previous.txt`.
fn previous_of(file: &Path) -> PathBuf {
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    match file.extension() {
        Some(ext) => file.with_file_name(format!("{stem}.previous.{}", ext.to_string_lossy())),
        None => file.with_file_name(format!("{stem}.previous")),
    }
}

/// B2S Server's screen files made to say where the page's settings put its windows, where they say
/// something else (`numbers`: Windows' number for each screen). How many were written.
fn follow_with(settings: &ScreenSettings, screens: &[Screen], numbers: &[((i32, i32), u32)], files: &[PathBuf]) -> Result<usize, String> {
    let Some(place) = place_of(settings, screens, numbers) else { return Ok(0) };
    let mut written = 0;
    for file in files {
        let before = std::fs::read(file).ok();
        let after = with_place(before.as_deref(), &place);
        if before.as_deref() == Some(after.as_slice()) {
            continue;
        }
        if let Some(b) = &before {
            let _ = std::fs::write(previous_of(file), b);
        }
        std::fs::write(file, &after).map_err(|e| format!("B2S Server's screen file {} couldn't be written: {e}", file.display()))?;
        written += 1;
    }
    Ok(written)
}

/// After Settings › Screens saves: B2S Server's screen files made to say the same, where B2S
/// Server shows the backglasses of the VPX whose settings the page keeps.
pub fn after_save(tables: Option<&Path>) -> Result<usize, String> {
    let entries = vpi_settings::read_all();
    if !shows_backglasses(vpi_settings::followed_vpx().0 >= [10, 8, 1, 0], &entries) {
        return Ok(0);
    }
    let view = crate::screens::load()?;
    follow_with(&view.settings, &view.screens, &display_numbers(), &files(tables))
}

/// Before a table plays in `vpx`: B2S Server's screen files made to say where the screens put its
/// windows, where B2S Server shows that VPX's backglasses. Never in the way of the game.
pub fn before_play(vpx: &::library::vpx::Vpx, tables: Option<&Path>) {
    let entries = vpi_settings::read_all();
    if !shows_backglasses(vpx.has_addons(), &entries) {
        return;
    }
    match crate::screens::load().and_then(|view| follow_with(&view.settings, &view.screens, &display_numbers(), &files(tables))) {
        Ok(0) => {}
        Ok(n) => eprintln!("B2S Server's screen file set as the screens have it ({n})"),
        Err(e) => eprintln!("{e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(number: u32, name: &str, x: i32, width: i32, height: i32) -> Screen {
        Screen { number, name: name.into(), x, y: 0, width, height, density: 1.0, scale: 100.0, refresh: 60.0, size_cm: None, hdr: false, primary: number == 1 }
    }

    fn window(id: &str, output: &str, screen: &str, x: i32, y: i32, width: i32, height: i32) -> WindowPlace {
        WindowPlace { id: id.into(), output: output.into(), screen: screen.into(), fullscreen: false, x, y, width, height }
    }

    fn settings(windows: Vec<WindowPlace>) -> ScreenSettings {
        ScreenSettings {
            windows,
            view: "desktop".into(),
            backglass_dmd: "off".into(),
            score_panel_dmd: "off".into(),
            show_grill: false,
            popup_on: "backglass".into(),
            popup_size: "medium".into(),
        }
    }

    // a laptop's own screen right of the main one, which Windows numbers first
    fn laptop() -> (Vec<Screen>, Vec<((i32, i32), u32)>) {
        (vec![screen(1, "Main screen", 0, 2560, 1440), screen(2, "Laptop screen", 2560, 2560, 1600)], vec![((2560, 0), 1), ((0, 0), 2)])
    }

    #[test]
    fn the_backglass_goes_on_its_screen_by_windows_number() {
        let (screens, numbers) = laptop();
        let s = settings(vec![window("playfield", "window", "Main screen", 0, 0, 2560, 1440), window("backglass", "window", "Laptop screen", 0, 0, 2560, 1600), window("scoreView", "off", "Main screen", 0, 0, 0, 0)]);
        let place = place_of(&s, &screens, &numbers).unwrap();
        assert_eq!(place, Place { playfield: (2560, 1440), screen: 1, backglass: Rect { x: 0, y: 0, width: 2560, height: 1600 }, dmd: None });
    }

    #[test]
    fn b2s_servers_second_window_is_left_where_its_own_settings_put_it() {
        let (screens, numbers) = laptop();
        let s = settings(vec![
            window("playfield", "window", "Main screen", 0, 0, 2560, 1440),
            window("backglass", "window", "Laptop screen", 0, 0, 2560, 1200),
            window("scoreView", "window", "Main screen", 1500, 0, 1024, 576),
            window("topper", "off", "Main screen", 0, 0, 0, 0),
        ]);
        let place = place_of(&s, &screens, &numbers).unwrap();
        assert_eq!((place.screen, place.dmd), (1, None));
        // the file's own lines for it kept as they are
        let before = b"1920\r\n1080\r\n1024\r\n768\r\n2\r\n0\r\n0\r\n512\r\n128\r\n100\r\n-200\r\n0\r\n";
        let after = with_place(Some(before), &place);
        assert_eq!(::library::b2s_screen::values(&after)[7..], ["512", "128", "100", "-200", "0"]);
    }

    #[test]
    fn a_backglass_inside_the_playfield_goes_over_its_place_there() {
        let (screens, numbers) = laptop();
        let s = settings(vec![window("playfield", "window", "Main screen", 100, 40, 2000, 1300), window("backglass", "inside", "Main screen", 1500, 0, 500, 400)]);
        let place = place_of(&s, &screens, &numbers).unwrap();
        assert_eq!((place.screen, place.backglass), (2, Rect { x: 1600, y: 40, width: 500, height: 400 }));
    }

    #[test]
    fn nothing_is_placed_for_a_backglass_shown_nowhere_or_on_a_screen_not_told() {
        let (screens, numbers) = laptop();
        let off = settings(vec![window("playfield", "window", "Main screen", 0, 0, 2560, 1440), window("backglass", "off", "Laptop screen", 0, 0, 2560, 1600)]);
        assert_eq!(place_of(&off, &screens, &numbers), None);
        let on = settings(vec![window("playfield", "window", "Main screen", 0, 0, 2560, 1440), window("backglass", "window", "Laptop screen", 0, 0, 2560, 1600)]);
        assert_eq!(place_of(&on, &screens, &[]), None);
    }

    #[test]
    fn a_registered_address_is_read_as_a_path() {
        assert_eq!(path_of_address("file:///C:/vPinball/VisualPinball/Tables/B2SBackglassServer.DLL"), Some(PathBuf::from(r"C:\vPinball\VisualPinball\Tables\B2SBackglassServer.DLL")));
        assert_eq!(path_of_address("file:///D:/My%20Tables/B2SBackglassServer.dll"), Some(PathBuf::from(r"D:\My Tables\B2SBackglassServer.dll")));
        assert_eq!(path_of_address(r"C:\no\address.dll"), None);
    }

    #[test]
    fn the_file_before_is_kept_beside_it() {
        assert_eq!(previous_of(Path::new(r"C:\T\ScreenRes.txt")), PathBuf::from(r"C:\T\ScreenRes.previous.txt"));
        assert_eq!(previous_of(Path::new(r"C:\T\Screens")), PathBuf::from(r"C:\T\Screens.previous"));
    }

    /// What this PC's B2S Server would be given, written nowhere: its folder, Windows' screen
    /// numbers, and each screen file as the settings' screens would make it. `VPI_B2S_TABLES=<the
    /// tables folder> cargo test -p app b2s_screen_on_this_pc -- --ignored --nocapture`, with the
    /// player built for release.
    #[test]
    #[ignore]
    fn b2s_screen_on_this_pc() {
        let player = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(r"..\target\release\vpi-player.exe");
        let out = std::process::Command::new(&player).args(["--displays", "--json"]).output().expect("the player built for release");
        let screens: Vec<Screen> = serde_json::from_slice(&out.stdout).unwrap();
        let numbers = display_numbers();
        println!("B2S Server's folder: {:?}, its file: {}", b2s_folder(), file_name());
        for s in &screens {
            println!("screen {} at {},{}: Windows' {:?}", s.name, s.x, s.y, numbers.iter().find(|(c, _)| *c == (s.x, s.y)).map(|(_, n)| n));
        }
        let entries = vpi_settings::read_all();
        println!("B2S Server shows the backglasses (VPX 10.8.1): {}", shows_backglasses(true, &entries));
        let settings = crate::screens::settings_of(&entries, &screens);
        let place = place_of(&settings, &screens, &numbers);
        println!("{place:?}");
        let tables = std::env::var_os("VPI_B2S_TABLES").map(PathBuf::from);
        for file in files(tables.as_deref()) {
            let before = std::fs::read(&file).ok();
            let after = place.as_ref().map(|p| with_place(before.as_deref(), p));
            println!("{}: {}", file.display(), if after.is_some() && after.as_deref() == before.as_deref() { "as it is" } else { "would change" });
            println!("  before {:?}", before.as_deref().map(::library::b2s_screen::values));
            println!("  after  {:?}", after.as_deref().map(::library::b2s_screen::values));
        }
    }

    #[test]
    fn the_files_are_written_only_where_they_say_something_else() {
        let dir = std::env::temp_dir().join(format!("vpi-b2s-screen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ScreenRes.txt");
        std::fs::write(&file, b"1920\r\n1080\r\n1024\r\n768\r\n2\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n").unwrap();
        let (screens, numbers) = laptop();
        let s = settings(vec![window("playfield", "window", "Main screen", 0, 0, 2560, 1440), window("backglass", "window", "Laptop screen", 0, 0, 2560, 1600)]);
        let files = [file.clone()];
        assert_eq!(follow_with(&s, &screens, &numbers, &files), Ok(1));
        assert_eq!(std::fs::read(&file).unwrap(), b"2560\r\n1440\r\n2560\r\n1600\r\n1\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n");
        assert!(std::fs::read(dir.join("ScreenRes.previous.txt")).unwrap().starts_with(b"1920\r\n"));
        assert_eq!(follow_with(&s, &screens, &numbers, &files), Ok(0));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
