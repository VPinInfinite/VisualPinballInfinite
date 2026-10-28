// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors
// Follows PinMAME's VPinMAME driver for display devices (src/win32com/ControllerDmdDevice.cpp: which
// functions it asks for, how it starts one and hands it frames) and the device's header
// (ext/dmddevice/dmddevice.h), both BSD-3-Clause, from the PinMAME commit VPX 5436 builds.

//! `vpi-dmddevice`: drives a cabinet's real DMD panel beside the player
//! through a VPinMAME display device — DMD Extensions' `DmdDevice64.dll`,
//! or a panel maker's own (PIN2DMD's, PinDMD's) — as VPinMAME drives one in
//! VPX 10.8.0's cabinets: the device opened, handed the game's name, its
//! hardware generation and VPinMAME's display options, then each frame as
//! VPinMAME hands it: a machine's DMD frame with its brightness and its raw
//! shades (`Render_FloatLum_And_Raw` when the device has it, else the shades
//! alone, 4 or 16 of them), a segment game's segments with PinMAME's layout of
//! them, and other displays' frames (FlexDMD's, UltraDMD's) in colour
//! (`Render_RGB24`). The device does the rest as the user set it up in its own
//! settings — which panels, colour files, a picture on screen — so a cabinet
//! set up for VPX keeps its `DmdDevice.ini` as it is. A program of its own:
//! a fault in the device can't end a game, and its licence stays its own.
//!
//! ```text
//! vpi-dmddevice --link <name> --library <DmdDevice64.dll> --game <rom> [--hardware <gen>]
//!               [--options <19 numbers>] [--config <DmdDevice.ini>] [--parent <pid>]
//! ```
//!
//! `--options` are VPinMAME's display options in `tPMoptions`' order (`dmd_red`, `dmd_green`,
//! `dmd_blue`, `dmd_perc66`, `dmd_perc33`, `dmd_perc0`, `dmd_only`, `dmd_compact`,
//! `dmd_antialias`, `dmd_colorize`, then the colours at 66%, 33% and 0%), VPinMAME's own defaults
//! when not given. `--config` names the device's settings file through `DMDDEVICE_CONFIG`, as DMD
//! Extensions' installer sets it (for checks with a settings file of their own).

use std::ffi::{CString, c_char, c_int, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::ExitCode;

use dmd_link::{FORMAT_ALPHANUMERIC, FORMAT_LUM, FORMAT_LUM_RAW, FORMAT_RGB565, FORMAT_RGB888, FORMAT_SHADES4, FORMAT_SHADES16, Frame, Link, SEG_WORDS, STATE_FAILED, STATE_READY};

const USAGE: &str = "vpi-dmddevice --link <name> --library <DmdDevice64.dll> --game <rom> [--hardware <gen>] [--options <19 numbers>] [--config <DmdDevice.ini>] [--parent <pid>]";

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn WaitForMultipleObjects(count: u32, handles: *const *mut c_void, wait_all: i32, milliseconds: u32) -> u32;
}

/// The device's own folder searched first for what it loads (its .NET parts, its USB libraries).
const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0008;
const SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_OBJECT_0: u32 = 0;

/// PinMAME's hardware generation for Gottlieb System 3 (`GEN_GTS3`), whose devices are handed
/// its brightness as shades, not its raw frames (VPinMAME's legacy path).
const GEN_GTS3: u64 = 0x200_0000_0000;

/// The device header's `tPMoptions`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct PmOptions {
    dmd_red: c_int,
    dmd_green: c_int,
    dmd_blue: c_int,
    dmd_perc66: c_int,
    dmd_perc33: c_int,
    dmd_perc0: c_int,
    dmd_only: c_int,
    dmd_compact: c_int,
    dmd_antialias: c_int,
    dmd_colorize: c_int,
    dmd_red66: c_int,
    dmd_green66: c_int,
    dmd_blue66: c_int,
    dmd_red33: c_int,
    dmd_green33: c_int,
    dmd_blue33: c_int,
    dmd_red0: c_int,
    dmd_green0: c_int,
    dmd_blue0: c_int,
}

impl PmOptions {
    /// VPinMAME's defaults (src/windows/config.c, src/win32com/VPinMAMEConfig.cpp).
    const VPINMAME: PmOptions = PmOptions {
        dmd_red: 255,
        dmd_green: 88,
        dmd_blue: 32,
        dmd_perc66: 67,
        dmd_perc33: 33,
        dmd_perc0: 20,
        dmd_only: 0,
        dmd_compact: 0,
        dmd_antialias: 50,
        dmd_colorize: 0,
        dmd_red66: 225,
        dmd_green66: 15,
        dmd_blue66: 193,
        dmd_red33: 6,
        dmd_green33: 0,
        dmd_blue33: 214,
        dmd_red0: 0,
        dmd_green0: 0,
        dmd_blue0: 0,
    };

    fn from_list(text: &str) -> Option<PmOptions> {
        let v: Vec<c_int> = text.split(',').map(|n| n.trim().parse().ok()).collect::<Option<_>>()?;
        let &[r, g, b, p66, p33, p0, only, compact, aa, colorize, r66, g66, b66, r33, g33, b33, r0, g0, b0] = v.as_slice() else { return None };
        Some(PmOptions {
            dmd_red: r,
            dmd_green: g,
            dmd_blue: b,
            dmd_perc66: p66,
            dmd_perc33: p33,
            dmd_perc0: p0,
            dmd_only: only,
            dmd_compact: compact,
            dmd_antialias: aa,
            dmd_colorize: colorize,
            dmd_red66: r66,
            dmd_green66: g66,
            dmd_blue66: b66,
            dmd_red33: r33,
            dmd_green33: g33,
            dmd_blue33: b33,
            dmd_red0: r0,
            dmd_green0: g0,
            dmd_blue0: b0,
        })
    }
}

/// The device header's `rgb24`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Rgb24 {
    red: u8,
    green: u8,
    blue: u8,
}

type OpenFn = unsafe extern "C" fn() -> c_int;
type CloseFn = unsafe extern "C" fn() -> bool;
type GameSettingsFn = unsafe extern "C" fn(*const c_char, u64, *const PmOptions);
type Palette4Fn = unsafe extern "C" fn(Rgb24, Rgb24, Rgb24, Rgb24);
type ShadesFn = unsafe extern "C" fn(u16, u16, *mut u8);
type FloatLumRawFn = unsafe extern "C" fn(u16, u16, *mut f32, *mut u8, u8);
type Rgb24Fn = unsafe extern "C" fn(u16, u16, *mut Rgb24);
type AlphanumericFn = unsafe extern "C" fn(c_int, *const u16, *const u16);

/// A display device's functions, as VPinMAME asks for them: the six it needs, the rest when there.
struct Device {
    close: CloseFn,
    render_4: ShadesFn,
    render_16: ShadesFn,
    alphanumeric: AlphanumericFn,
    float_lum_raw: Option<FloatLumRawFn>,
    rgb24: Option<Rgb24Fn>,
}

impl Device {
    /// The device loaded and opened for the game, as VPinMAME's `dmddeviceInit` does it.
    fn open(library: &PathBuf, game: &str, hardware: u64, options: &PmOptions) -> Result<Device, String> {
        let wide: Vec<u16> = library.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: a null-terminated path; the device stays loaded for as long as this program runs
        let module = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
        if module.is_null() {
            return Err(format!("could not load {}: {}", library.display(), std::io::Error::last_os_error()));
        }
        let find = |name: &str| {
            let name = CString::new(name).expect("a function's name");
            // SAFETY: the loaded module, a null-terminated name
            let f = unsafe { GetProcAddress(module, name.as_ptr()) };
            (!f.is_null()).then_some(f)
        };
        let (Some(open), Some(close), Some(settings), Some(render_4), Some(render_16), Some(alphanumeric)) =
            (find("Open"), find("Close"), find("PM_GameSettings"), find("Render_4_Shades"), find("Render_16_Shades"), find("Render_PM_Alphanumeric_Frame"))
        else {
            return Err(format!("{} isn't a display device VPinMAME can drive (functions missing)", library.display()));
        };
        // SAFETY: each is the device's exported function of that name, with the header's signature
        unsafe {
            let device = Device {
                close: std::mem::transmute::<*mut c_void, CloseFn>(close),
                render_4: std::mem::transmute::<*mut c_void, ShadesFn>(render_4),
                render_16: std::mem::transmute::<*mut c_void, ShadesFn>(render_16),
                alphanumeric: std::mem::transmute::<*mut c_void, AlphanumericFn>(alphanumeric),
                float_lum_raw: find("Render_FloatLum_And_Raw").map(|f| std::mem::transmute::<*mut c_void, FloatLumRawFn>(f)),
                rgb24: find("Render_RGB24").map(|f| std::mem::transmute::<*mut c_void, Rgb24Fn>(f)),
            };
            std::mem::transmute::<*mut c_void, OpenFn>(open)();
            // VPinMAME's own colours for the shades, when its options colourise them
            if options.dmd_colorize != 0
                && let Some(palette) = find("Set_4_Colors_Palette")
            {
                let c = |r: c_int, g: c_int, b: c_int| Rgb24 { red: r as u8, green: g as u8, blue: b as u8 };
                let o = options;
                std::mem::transmute::<*mut c_void, Palette4Fn>(palette)(
                    c(o.dmd_red0, o.dmd_green0, o.dmd_blue0),
                    c(o.dmd_red33, o.dmd_green33, o.dmd_blue33),
                    c(o.dmd_red66, o.dmd_green66, o.dmd_blue66),
                    c(o.dmd_red, o.dmd_green, o.dmd_blue),
                );
            }
            let game = CString::new(game).map_err(|_| "the game's name holds a null character".to_string())?;
            std::mem::transmute::<*mut c_void, GameSettingsFn>(settings)(game.as_ptr(), hardware, options);
            Ok(device)
        }
    }
}

struct Options {
    link: String,
    library: PathBuf,
    game: String,
    hardware: u64,
    pm: PmOptions,
    config: Option<PathBuf>,
    parent: Option<u32>,
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let (mut link, mut library, mut game, mut hardware, mut pm, mut config, mut parent) = (None, None, None, 0u64, PmOptions::VPINMAME, None, None);
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{a} needs a value\n{USAGE}"));
        match a.as_str() {
            "--link" => link = Some(value()?),
            "--library" => library = Some(PathBuf::from(value()?)),
            "--game" => game = Some(value()?),
            "--hardware" => hardware = value()?.parse().map_err(|_| "--hardware takes PinMAME's generation, a number".to_string())?,
            "--options" => pm = PmOptions::from_list(&value()?).ok_or("--options takes 19 numbers, separated by commas")?,
            "--config" => config = Some(PathBuf::from(value()?)),
            "--parent" => parent = Some(value()?.parse().map_err(|_| "--parent takes a process id".to_string())?),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    Ok(Options { link: link.ok_or(USAGE)?, library: library.ok_or(USAGE)?, game: game.ok_or(USAGE)?, hardware, pm, config, parent })
}

/// The newest frame handed to the device as VPinMAME hands it; the size of the last DMD frame
/// kept, to blank the panel at the end as VPinMAME does.
fn render(device: &Device, frame: &Frame, hardware: u64, last_dmd: &mut Option<(u16, u16)>) {
    let (w, h) = (frame.width as u16, frame.height as u16);
    let lum = |frame: &Frame| -> Vec<f32> { frame.dots.iter().map(|&d| f32::from_bits(d)).collect() };
    // SAFETY: the device's functions with frames as long as the sizes say
    unsafe {
        match frame.format {
            FORMAT_LUM_RAW => {
                *last_dmd = Some((w, h));
                let sixteen = frame.raw_bits >= 4;
                let mut raw = frame.raw.clone();
                if let Some(f) = device.float_lum_raw {
                    // the newer way: brightness to draw, raw shades to identify frames by
                    let mut lum = lum(frame);
                    f(w, h, lum.as_mut_ptr(), raw.as_mut_ptr(), if sixteen { 4 } else { 2 });
                    return;
                }
                // the older way, one stream for both: the raw shades, but Gottlieb System 3's
                // brightness as shades (VPinMAME's legacy path)
                if hardware & GEN_GTS3 != 0 {
                    let scale = if sixteen { 15.0 } else { 3.0 };
                    raw = lum(frame).iter().map(|l| (l * scale) as u8).collect();
                }
                if sixteen { (device.render_16)(w, h, raw.as_mut_ptr()) } else { (device.render_4)(w, h, raw.as_mut_ptr()) }
            }
            FORMAT_ALPHANUMERIC => {
                let (seg_data, seg_data2) = frame.segs.split_at(SEG_WORDS.min(frame.segs.len()));
                (device.alphanumeric)(frame.seg_layout as c_int, seg_data.as_ptr(), seg_data2.as_ptr());
            }
            FORMAT_RGB888 | FORMAT_RGB565 => {
                *last_dmd = Some((w, h));
                let colour = |d: u32| -> Rgb24 {
                    if frame.format == FORMAT_RGB888 {
                        Rgb24 { red: (d >> 16) as u8, green: (d >> 8) as u8, blue: d as u8 }
                    } else {
                        // 5, 6 and 5 bits widened as their bits repeated
                        let (r, g, b) = ((d >> 11) & 0x1f, (d >> 5) & 0x3f, d & 0x1f);
                        Rgb24 { red: ((r << 3) | (r >> 2)) as u8, green: ((g << 2) | (g >> 4)) as u8, blue: ((b << 3) | (b >> 2)) as u8 }
                    }
                };
                let mut rgb: Vec<Rgb24> = frame.dots.iter().map(|&d| colour(d)).collect();
                match device.rgb24 {
                    Some(f) => f(w, h, rgb.as_mut_ptr()),
                    None => {
                        // a device without colour: 16 shades of the colours' brightness
                        let mut shades: Vec<u8> =
                            rgb.iter().map(|c| ((0.299 * f32::from(c.red) + 0.587 * f32::from(c.green) + 0.114 * f32::from(c.blue)) / 255.0 * 15.0).round() as u8).collect();
                        (device.render_16)(w, h, shades.as_mut_ptr());
                    }
                }
            }
            FORMAT_SHADES4 | FORMAT_SHADES16 => {
                // a FlexDMD's grey, as FlexDMD hands it: 4 or 16 shades
                *last_dmd = Some((w, h));
                let mut shades: Vec<u8> = frame.dots.iter().map(|&d| d as u8).collect();
                if frame.format == FORMAT_SHADES4 { (device.render_4)(w, h, shades.as_mut_ptr()) } else { (device.render_16)(w, h, shades.as_mut_ptr()) }
            }
            FORMAT_LUM => {
                // a brightness without raw shades: 16 shades of it
                *last_dmd = Some((w, h));
                let mut shades: Vec<u8> = lum(frame).iter().map(|l| (l.clamp(0.0, 1.0) * 15.0).round() as u8).collect();
                (device.render_16)(w, h, shades.as_mut_ptr());
            }
            _ => {}
        }
    }
}

fn main() -> ExitCode {
    let o = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("vpi-dmddevice: {e}");
            return ExitCode::FAILURE;
        }
    };
    let link = match Link::open(&o.link) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vpi-dmddevice: could not open the link {}: {e}", o.link);
            return ExitCode::FAILURE;
        }
    };
    let shared = link.shared();
    if let Some(config) = &o.config {
        // SAFETY: set before the device loads, while this program has a single thread
        unsafe { std::env::set_var("DMDDEVICE_CONFIG", config) };
    }
    let device = match Device::open(&o.library, &o.game, o.hardware, &o.pm) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("vpi-dmddevice: {e}; no panel is driven");
            shared.set_state(STATE_FAILED);
            return ExitCode::FAILURE;
        }
    };
    let settings = match (&o.config, std::env::var_os("DMDDEVICE_CONFIG")) {
        (Some(c), _) => c.display().to_string(),
        (None, Some(c)) => PathBuf::from(c).display().to_string(),
        (None, None) => "its folder's DmdDevice.ini".to_string(),
    };
    let way = if device.float_lum_raw.is_some() { "brightness with raw shades" } else { "shades, the older way" };
    eprintln!("vpi-dmddevice: {} drives the panel for {}, as its settings say ({settings}); DMD frames as {way}", o.library.display(), o.game);
    shared.set_state(STATE_READY);

    // SAFETY: the player's process, waited on so this program ends with it
    let parent = o.parent.map(|pid| unsafe { OpenProcess(SYNCHRONIZE, 0, pid) }).filter(|h| !h.is_null());
    let mut handles = vec![link.to_host_event()];
    handles.extend(parent);
    let (mut last, mut last_dmd, mut shown) = (None, None, 0u64);
    // `VPI_DMDDEVICE_TRACE=<file>`: each frame handed over, written down for checks
    let mut trace = std::env::var_os("VPI_DMDDEVICE_TRACE").and_then(|p| std::fs::File::create(p).ok());
    let start = std::time::Instant::now();
    loop {
        // SAFETY: waits for a frame, the player's end, or 1/60 s
        let woke = unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, 16) };
        if parent.is_some() && woke == WAIT_OBJECT_0 + 1 {
            break;
        }
        if shared.stop_requested() {
            break;
        }
        if let Some(frame) = shared.read(last) {
            last = Some(frame.id);
            render(&device, &frame, o.hardware, &mut last_dmd);
            shown += 1;
            if let Some(t) = trace.as_mut() {
                use std::io::Write;
                let _ = writeln!(t, "{} ms: {}", start.elapsed().as_millis(), describe(&frame));
            }
        }
    }
    // the panel blanked, then the device closed, as VPinMAME's `dmddeviceDeInit` does
    // SAFETY: the device is open; the blank frame is as long as its size says
    unsafe {
        if let Some((w, h)) = last_dmd {
            let mut blank = vec![0u8; usize::from(w) * usize::from(h)];
            (device.render_4)(w, h, blank.as_mut_ptr());
        }
        (device.close)();
    }
    eprintln!("vpi-dmddevice: {shown} frames handed to the device");
    ExitCode::SUCCESS
}

/// A frame in a line: a segment frame's layout and words (to the last lit one), else its size,
/// format and lit dots.
fn describe(frame: &Frame) -> String {
    if frame.format == FORMAT_ALPHANUMERIC {
        let words = &frame.segs[..SEG_WORDS.min(frame.segs.len())];
        let used = words.iter().rposition(|&w| w != 0).map_or(0, |i| i + 1);
        let hex: Vec<String> = words[..used].iter().map(|w| format!("{w:04x}")).collect();
        format!("segments, layout {}: {}", frame.seg_layout, hex.join(" "))
    } else {
        let lit = frame.dots.iter().filter(|&&d| d != 0).count();
        format!("{}x{} format {}, {lit} dots lit", frame.width, frame.height, frame.format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vpinmames_options_read_in_their_order() {
        let o = PmOptions::from_list("1,2,3,4,5,6,0,1,50,1,7,8,9,10,11,12,13,14,15").expect("19 numbers");
        assert_eq!((o.dmd_red, o.dmd_perc0, o.dmd_compact, o.dmd_colorize, o.dmd_blue0), (1, 6, 1, 1, 15));
        assert!(PmOptions::from_list("1,2,3").is_none());
        assert_eq!(std::mem::size_of::<PmOptions>(), 19 * 4);
        assert_eq!(std::mem::size_of::<Rgb24>(), 3);
    }
}
