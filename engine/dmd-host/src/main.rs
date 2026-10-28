// SPDX-License-Identifier: GPL-3.0-or-later
// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its DMDUtil plugin,
// plugins/dmdutil/DMDUtilPlugin.cpp — its settings, the frames handed to libdmdutil and their tint.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors; this program, the Visual Pinball
// Infinite contributors, 2026.

//! `vpi-dmd`: drives a cabinet's real DMD panel beside the player
//! through libdmdutil, the VPX team's library for ZeDMD, Pixelcade and
//! PIN2DMD panels — our own build of it, without its Serum and VNI parts
//! (`tools\dmdutil\setup.ps1`), else, while developing, VPX
//! 5436's (`dmdutil64.dll` and the libraries beside it in its bundle), which
//! carries both and is never shipped. The player creates the link,
//! starts this program with the panel settings (VPX's `[Plugin.DMDUtil]`,
//! under VPX's names) and hands over the display's newest frame as VPX
//! 5436's plugin picks it; this program hands it to libdmdutil as the plugin
//! does — a brightness tinted (`LumTint`, amber unless set), colours as they
//! come — at most once every sixtieth of a second, as the plugin's own
//! sixty-a-second look, but at once when a new frame comes after a pause.
//! With no panel enabled in the settings, libdmdutil looks for none.
//!
//! For checks without a panel, `--check <folder>` gives libdmdutil a virtual
//! panel of its own, which gets every frame as a colour panel would; the
//! player asks for pictures of it, saved there as `<name> at <n> ms, panel.png`.
//!
//! ```text
//! vpi-dmd --link <name> [--set Name=value ...] [--library <folder>] [--parent <pid>] [--check <folder> --name <table>]
//! ```

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use dmd_link::{FORMAT_LUM, FORMAT_RGB565, FORMAT_RGB888, Frame, Link, STATE_FAILED, STATE_READY};

const USAGE: &str = "vpi-dmd --link <name> [--set Name=value ...] [--library <folder>] [--parent <pid>] [--check <folder> --name <table>]";

/// The bridge's settings, `VpiDmdSettings` in `native/dmdutil.cpp`.
#[repr(C)]
struct BridgeSettings {
    zedmd: c_int,
    zedmd_device: *const c_char,
    zedmd_debug: c_int,
    zedmd_brightness: c_int,
    zedmd_wifi: c_int,
    zedmd_wifi_addr: *const c_char,
    zedmd_spi: c_int,
    zedmd_spi_speed: c_int,
    zedmd_spi_frame_pause: c_int,
    zedmd_spi_width: c_int,
    zedmd_spi_height: c_int,
    pixelcade: c_int,
    pixelcade_device: *const c_char,
    pin2dmd: c_int,
    dmd_server: c_int,
    dmd_server_addr: *const c_char,
    dmd_server_port: c_int,
}

unsafe extern "C" {
    fn vpi_dmd_built_in() -> c_int;
    fn vpi_dmd_load(folder: *const u16) -> c_int;
    fn vpi_dmd_configure(settings: *const BridgeSettings, log: extern "C" fn(c_int, *const c_char)) -> c_int;
    fn vpi_dmd_start(find: c_int, dump_txt: c_int, dump_raw: c_int) -> c_int;
    fn vpi_dmd_finding() -> c_int;
    fn vpi_dmd_has_display() -> c_int;
    fn vpi_dmd_rgb24(data: *const u8, width: u16, height: u16) -> c_int;
    fn vpi_dmd_rgb16(data: *const u16, width: u16, height: u16) -> c_int;
    fn vpi_dmd_virtual(width: u16, height: u16) -> c_int;
    fn vpi_dmd_virtual_read(out: *mut u8, capacity: c_int, width: *mut u16, height: *mut u16) -> c_int;
    fn vpi_dmd_finish();
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn WaitForMultipleObjects(count: u32, handles: *const *mut c_void, wait_all: i32, milliseconds: u32) -> u32;
}

const SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_OBJECT_0: u32 = 0;
/// VPX's plugin looks for a new frame sixty times a second.
const FRAME_INTERVAL: Duration = Duration::from_micros(16_666);

struct Options {
    link: String,
    settings: Vec<(String, String)>,
    library: Option<PathBuf>,
    parent: Option<u32>,
    check: Option<PathBuf>,
    name: String,
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let mut o = Options { link: String::new(), settings: Vec::new(), library: None, parent: None, check: None, name: "panel".into() };
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{a} needs a value\n{USAGE}"));
        match a.as_str() {
            "--link" => o.link = value()?,
            "--set" => {
                let v = value()?;
                let (name, value) = v.split_once('=').ok_or_else(|| format!("--set takes Name=value, not {v}"))?;
                o.settings.push((name.trim().to_string(), value.trim().to_string()));
            }
            "--library" => o.library = Some(PathBuf::from(value()?)),
            "--parent" => o.parent = Some(value()?.parse().map_err(|_| "--parent takes a process id".to_string())?),
            "--check" => o.check = Some(PathBuf::from(value()?)),
            "--name" => o.name = value()?,
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    if o.link.is_empty() {
        return Err(USAGE.into());
    }
    Ok(o)
}

/// The settings as VPX 5436's plugin reads them: a setting by its name in any case, the last
/// given counting.
struct Settings<'a>(&'a [(String, String)]);

impl Settings<'_> {
    fn get(&self, name: &str) -> Option<&str> {
        self.0.iter().rev().find(|(n, v)| n.eq_ignore_ascii_case(name) && !v.is_empty()).map(|(_, v)| v.as_str())
    }

    /// A yes or no as VPX reads one (`LayeredINIPropertyStore::LoadFromINI`): a whole number, yes
    /// unless 0; else true, on or enable, or false, off or disable, in any case; else `default`.
    fn flag(&self, name: &str, default: bool) -> bool {
        let Some(v) = self.get(name) else { return default };
        if let Some(n) = whole_number(v) {
            return n != 0;
        }
        let t = v.trim();
        if ["true", "on", "enable"].iter().any(|w| t.eq_ignore_ascii_case(w)) {
            true
        } else if ["false", "off", "disable"].iter().any(|w| t.eq_ignore_ascii_case(w)) {
            false
        } else {
            default
        }
    }

    /// A whole number within its range, else `default`, as VPX's integer settings read.
    fn int(&self, name: &str, min: i32, max: i32, default: i32) -> i32 {
        self.get(name).and_then(whole_number).filter(|v| (min..=max).contains(v)).unwrap_or(default)
    }

    fn text(&self, name: &str, default: &str) -> CString {
        CString::new(self.get(name).unwrap_or(default)).unwrap_or_default()
    }
}

/// A whole number as VPX reads one (`try_parse_int`: `from_chars` on the trimmed text): its
/// leading digits, a minus sign before them allowed, whatever follows them ignored.
fn whole_number(s: &str) -> Option<i32> {
    let t = s.trim_matches([' ', '\t', '\r', '\n']);
    let b = t.as_bytes();
    let start = usize::from(b.first() == Some(&b'-'));
    let end = start + b[start..].iter().take_while(|c| c.is_ascii_digit()).count();
    if end == start { None } else { t[..end].parse().ok() }
}

/// Whether this is a development build, which also looks in the repository's `vpx-build` folders
/// for the libraries it loads. A release build, compiled with `VPI_RELEASE_BUILD` set
/// (`tools\release`), looks beside the program alone.
const DEVELOPMENT: bool = option_env!("VPI_RELEASE_BUILD").is_none();

/// libdmdutil's folder: beside this program (how it ships), else, in the repository, our own
/// build of it or VPX 5436's library bundle (development builds).
fn find_library() -> Option<PathBuf> {
    const NAME: &str = "dmdutil64.dll";
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    if dir.join(NAME).exists() {
        return Some(dir.to_path_buf());
    }
    if !DEVELOPMENT {
        return None;
    }
    dir.ancestors()
        .take(4)
        .flat_map(|up| [up.join(r"vpx-build\libdmdutil\build\Release"), up.join(r"vpx-build\downloads\5436-third-party-windows-x64-Release\runtime-libs\windows-x64")])
        .find(|p| p.join(NAME).exists())
}

/// libdmdutil's log, a line each, as VPX's plugin logs it (its debug lines left out).
extern "C" fn log(level: c_int, text: *const c_char) {
    if text.is_null() || level == 0 {
        return;
    }
    // SAFETY: the bridge hands over a null-terminated line it formatted
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    eprintln!("vpi-dmd: {}{}", if level == 2 { "error: " } else { "" }, text.trim_end());
}

/// What was handed to the panel, for the end's account.
#[derive(Default)]
struct Counts {
    frames: u64,
    lum: u64,
    rgb24: u64,
    rgb16: u64,
    refused: u64,
    sizes: Vec<(u32, u32)>,
}

fn main() -> ExitCode {
    let options = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("vpi-dmd: {e}");
            return ExitCode::FAILURE;
        }
    };
    let link = match Link::open(&options.link) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vpi-dmd: could not open the link {}: {e}", options.link);
            return ExitCode::FAILURE;
        }
    };
    let shared = link.shared();
    let settings = Settings(&options.settings);
    let tint = [settings.int("LumTintR", 0, 255, 255), settings.int("LumTintG", 0, 255, 140), settings.int("LumTintB", 0, 255, 0)].map(|c| c as f32);
    if let Err(e) = start(&options, &settings) {
        eprintln!("vpi-dmd: {e}; no panel is driven");
        shared.set_state(STATE_FAILED);
        return ExitCode::FAILURE;
    }
    shared.set_state(STATE_READY);

    // SAFETY: the player's process, waited on so this program ends with it
    let parent = options.parent.map(|pid| unsafe { OpenProcess(SYNCHRONIZE, 0, pid) }).filter(|h| !h.is_null());
    let mut handles = vec![link.to_host_event()];
    handles.extend(parent);
    let mut counts = Counts::default();
    let (mut last, mut sent_at, mut panel_size): (Option<u32>, Option<Instant>, Option<(u32, u32)>) = (None, None, None);
    let mut rgb = Vec::new();
    let mut said_found = false;
    loop {
        // the next frame no sooner than a sixtieth of a second after the last
        let wait = sent_at.map_or(1000, |t| FRAME_INTERVAL.saturating_sub(t.elapsed()).as_millis().max(1) as u32);
        let pending = sent_at.is_some_and(|t| t.elapsed() < FRAME_INTERVAL);
        // SAFETY: waits for a new frame, a picture asked for, a stop or the player's end
        let woke = unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, if pending { wait } else { 1000 }) };
        if parent.is_some() && woke == WAIT_OBJECT_0 + 1 {
            break;
        }
        if shared.stop_requested() {
            break;
        }
        if !said_found {
            // SAFETY: libdmdutil started
            if unsafe { vpi_dmd_finding() } == 0 {
                said_found = true;
                // SAFETY: as above
                let found = unsafe { vpi_dmd_has_display() } != 0;
                if found || options.check.is_none() {
                    eprintln!("vpi-dmd: {}", if found { "a panel found" } else { "no panel found" });
                }
            }
        }
        if sent_at.is_none_or(|t| t.elapsed() >= FRAME_INTERVAL)
            && let Some(frame) = shared.read(last)
        {
            last = Some(frame.id);
            if options.check.is_some() && panel_size != Some((frame.width, frame.height)) {
                panel_size = Some((frame.width, frame.height));
                // SAFETY: libdmdutil started; a size libdmdutil takes
                unsafe { vpi_dmd_virtual(frame.width as u16, frame.height as u16) };
            }
            if hand_over(&frame, tint, &mut rgb, &mut counts) {
                sent_at = Some(Instant::now());
            }
        }
        if let Some(number) = shared.picture_wanted() {
            if let Some(folder) = &options.check {
                // libdmdutil's own thread passes the frame on to its panels: a moment for it
                std::thread::sleep(Duration::from_millis(30));
                save_picture(folder, &options.name, number);
            }
            shared.picture_done(number);
        }
    }
    // SAFETY: libdmdutil ends with the game
    unsafe { vpi_dmd_finish() };
    let sizes: Vec<String> = counts.sizes.iter().map(|(w, h)| format!("{w}x{h}")).collect();
    eprintln!(
        "vpi-dmd: {} frames handed to libdmdutil ({} brightness, tinted; {} in 24-bit colour; {} in 16-bit colour), {}{}",
        counts.frames,
        counts.lum,
        counts.rgb24,
        counts.rgb16,
        if sizes.is_empty() { "none".to_string() } else { sizes.join(", ") },
        if counts.refused > 0 { format!("; {} refused", counts.refused) } else { String::new() }
    );
    ExitCode::SUCCESS
}

/// libdmdutil loaded, set up and started, as VPX 5436's plugin does when it loads and when the
/// first display comes.
fn start(o: &Options, s: &Settings) -> Result<(), String> {
    // SAFETY: a plain query of the bridge
    if unsafe { vpi_dmd_built_in() } == 0 {
        return Err("this vpi-dmd was built without libdmdutil".into());
    }
    let folder = o.library.clone().or_else(find_library).ok_or("dmdutil64.dll wasn't found")?;
    let wide: Vec<u16> = folder.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: a null-terminated path
    if unsafe { vpi_dmd_load(wide.as_ptr()) } == 0 {
        return Err(format!("could not load libdmdutil from {}", folder.display()));
    }
    // libdmdutil writes its frame dumps into the current folder: a folder of our own, or the check's
    let own = match &o.check {
        Some(c) => Some(c.clone()),
        None => std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Visual Pinball Infinite").join("DMD")),
    };
    if let Some(own) = own
        && std::fs::create_dir_all(&own).is_ok()
    {
        let _ = std::env::set_current_dir(&own);
    }
    let (zedmd_device, wifi_addr, pixelcade_device, server_addr) =
        (s.text("ZeDMDDevice", ""), s.text("ZeDMDWiFiAddr", "zedmd-wifi.local"), s.text("PixelcadeDevice", ""), s.text("DMDServerAddr", "localhost"));
    let b = BridgeSettings {
        zedmd: s.flag("ZeDMD", false).into(),
        zedmd_device: zedmd_device.as_ptr(),
        zedmd_debug: s.flag("ZeDMDDebug", false).into(),
        zedmd_brightness: s.int("ZeDMDBrightness", -1, 1000, -1),
        zedmd_wifi: s.flag("ZeDMDWiFiEnabled", false).into(),
        zedmd_wifi_addr: wifi_addr.as_ptr(),
        zedmd_spi: s.flag("ZeDMDSPIEnabled", false).into(),
        zedmd_spi_speed: s.int("ZeDMDSPISpeed", 0, 100_000_000, 72_000_000),
        zedmd_spi_frame_pause: s.int("ZeDMDSPIFramePause", 0, 1000, 2),
        zedmd_spi_width: s.int("ZeDMDSPIWidth", 0, 1000, 128),
        zedmd_spi_height: s.int("ZeDMDSPIHeight", 0, 1000, 32),
        pixelcade: s.flag("Pixelcade", false).into(),
        pixelcade_device: pixelcade_device.as_ptr(),
        pin2dmd: s.flag("PIN2DMD", false).into(),
        dmd_server: s.flag("DMDServer", false).into(),
        dmd_server_addr: server_addr.as_ptr(),
        dmd_server_port: s.int("DMDServerPort", 0, 65535, 6789),
    };
    // SAFETY: the strings live across the call; the bridge copies them
    if unsafe { vpi_dmd_configure(&b, log) } == 0 {
        return Err("libdmdutil refused its settings".into());
    }
    let panels: Vec<&str> = [("ZeDMD", b.zedmd), ("Pixelcade", b.pixelcade), ("PIN2DMD", b.pin2dmd), ("DMDServer", b.dmd_server)]
        .into_iter()
        .filter(|(_, on)| *on != 0)
        .map(|(n, _)| n)
        .collect();
    eprintln!(
        "vpi-dmd: libdmdutil from {}; {}{}",
        folder.display(),
        if panels.is_empty() { "no panel enabled in the settings".to_string() } else { format!("looking for {}", panels.join(", ")) },
        if o.check.is_some() { "; libdmdutil's virtual panel for the check" } else { "" }
    );
    // SAFETY: libdmdutil loaded and set up
    if unsafe { vpi_dmd_start(s.flag("FindDisplays", true).into(), s.flag("DumpDMDTxt", false).into(), s.flag("DumpDMDRaw", false).into()) } == 0 {
        return Err("libdmdutil failed to start".into());
    }
    Ok(())
}

/// A frame handed to libdmdutil as VPX's plugin hands it: a brightness tinted into 24-bit colour
/// (`(uint8_t)(lum * tint)`), 24-bit colour as it is, 16-bit colour as it is.
fn hand_over(frame: &Frame, tint: [f32; 3], rgb: &mut Vec<u8>, counts: &mut Counts) -> bool {
    let (w, h) = (frame.width as u16, frame.height as u16);
    // SAFETY: each buffer holds the frame's dots, handed over for the call only
    let ok = match frame.format {
        FORMAT_LUM => {
            rgb.clear();
            rgb.extend(frame.dots.iter().flat_map(|&d| {
                let lum = f32::from_bits(d);
                tint.map(|t| (lum * t) as u8)
            }));
            counts.lum += 1;
            unsafe { vpi_dmd_rgb24(rgb.as_ptr(), w, h) }
        }
        FORMAT_RGB888 => {
            rgb.clear();
            rgb.extend(frame.dots.iter().flat_map(|&d| [(d >> 16) as u8, (d >> 8) as u8, d as u8]));
            counts.rgb24 += 1;
            unsafe { vpi_dmd_rgb24(rgb.as_ptr(), w, h) }
        }
        FORMAT_RGB565 => {
            let words: Vec<u16> = frame.dots.iter().map(|&d| d as u16).collect();
            counts.rgb16 += 1;
            unsafe { vpi_dmd_rgb16(words.as_ptr(), w, h) }
        }
        _ => 0,
    } != 0;
    if ok {
        counts.frames += 1;
        if !counts.sizes.contains(&(frame.width, frame.height)) {
            counts.sizes.push((frame.width, frame.height));
        }
    } else {
        counts.refused += 1;
    }
    ok
}

/// What libdmdutil's virtual panel shows now, saved as `<name> at <number> ms, panel.png`.
fn save_picture(folder: &Path, name: &str, number: u32) {
    let mut data = vec![0u8; 256 * 64 * 3];
    let (mut w, mut h) = (0u16, 0u16);
    // SAFETY: a buffer of the largest panel's size
    if unsafe { vpi_dmd_virtual_read(data.as_mut_ptr(), data.len() as c_int, &mut w, &mut h) } == 0 || w == 0 || h == 0 {
        eprintln!("vpi-dmd: no picture of the virtual panel at {number} ms: it has shown nothing yet");
        return;
    }
    data.truncate(usize::from(w) * usize::from(h) * 3);
    let path = folder.join(format!("{name} at {number} ms, panel.png"));
    match image::RgbImage::from_raw(u32::from(w), u32::from(h), data).map(|img| img.save(&path)) {
        Some(Ok(())) => {}
        Some(Err(e)) => eprintln!("vpi-dmd: {}: {e}", path.display()),
        None => eprintln!("vpi-dmd: the virtual panel's picture didn't hold together"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_read_as_vpx_reads_them() {
        let given = [
            ("zedmd".to_string(), "1".to_string()),
            ("LumTintG".into(), "300".into()),
            ("LumTintB".into(), " 20 ".into()),
            ("FindDisplays".into(), "Off".into()),
            ("PIN2DMD".into(), "yes".into()),
        ];
        let s = Settings(&given);
        assert!(s.flag("ZeDMD", false));
        assert!(!s.flag("FindDisplays", true));
        // not a word VPX takes: the default
        assert!(!s.flag("PIN2DMD", false));
        // out of range: the default
        assert_eq!(s.int("LumTintG", 0, 255, 140), 140);
        assert_eq!(s.int("LumTintB", 0, 255, 0), 20);
    }

    #[test]
    fn a_brightness_is_tinted_as_vpx_tints_it() {
        // (uint8_t)(lum * tint): truncated
        let frame = Frame { id: 1, width: 2, height: 1, format: FORMAT_LUM, dots: vec![1.0f32.to_bits(), 0.5f32.to_bits()], raw: Vec::new(), raw_bits: 0, seg_layout: 0, segs: Vec::new() };
        let mut rgb = Vec::new();
        let mut counts = Counts::default();
        let tint = [255.0, 140.0, 0.0];
        // the bridge refuses without libdmdutil started, but the colours are made first
        let _ = hand_over(&frame, tint, &mut rgb, &mut counts);
        assert_eq!(rgb, [255, 140, 0, 127, 70, 0]);
    }
}
