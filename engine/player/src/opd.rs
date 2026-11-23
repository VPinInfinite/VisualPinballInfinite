// Ported from Visual Pinball X build 5436 (10.8.1 pre-release, commit af26b2d): src/input/OpenPinDevHandler.cpp
// (license:GPLv3+): the report's layout and how it's read, the devices found by their usages and their struct's
// string, the buttons and axes handed on and their default mappings. Copyright (C) 2000-2026 Visual Pinball
// development team and contributors. This port is also GPL-3.0-or-later. The report descriptor is read by a small
// parser of ours, written from the USB HID specification (HID 1.11, section 6.2.2), where VPX uses the
// hid-report-parser library.

//! Open Pinball Device: MJR's open standard for a cabinet board's
//! USB report — its buttons, plunger and nudge in one message — which Pinscape Pico sends and VPX
//! 5436 reads (`OpenPinDevHandler`). A board is found at the start among the HID interfaces of
//! usage page 5 (Game Controls), usage 2 (Pinball Device), whose report descriptor has an input
//! field of usage 5:0 labelled by a string starting `OpenPinballDeviceStruct/`. Its reports are
//! read on a thread of its own as they arrive, where VPX reads them each frame: the 27 pinball
//! buttons (elements 0 to 26) and the 32 generic ones (0x0101 to 0x0120) go to the mappings as
//! buttons, the six axes (0x0200 to 0x0205: plunger position and speed, nudge acceleration and
//! speed) to the sensors and the mappings, each over 32768 as VPX scales them, on the board's
//! own clock as VPX times them. The device is `OpenPinDev` in the settings, as VPX names it, with
//! VPX's default mapping unless the settings set it up: every pinball button to its action, the
//! plunger's position and speed to a plunger, the nudge speeds to a gamepad nudge.

use std::ffi::{CStr, c_int};
use std::sync::Arc;

use sdl3_sys::everything::*;

use crate::input::InputSink;
use crate::input_map::Input;

/// The pinball buttons VPX names (bits 0 to 26 of the report's pinball buttons).
pub const PINBALL_BUTTONS: u16 = 27;
/// The generic buttons' elements: 0x0101 to 0x0120.
pub const GENERIC: u16 = 0x0100;
/// The axes' elements: plunger position, plunger speed, nudge acceleration X and Y, nudge speed X and Y.
pub const PLUNGER_POSITION: u16 = 0x0200;
pub const PLUNGER_SPEED: u16 = 0x0201;
pub const NUDGE_ACCELERATION_X: u16 = 0x0202;
pub const NUDGE_ACCELERATION_Y: u16 = 0x0203;
pub const NUDGE_SPEED_X: u16 = 0x0204;
pub const NUDGE_SPEED_Y: u16 = 0x0205;

/// VPX's `OpenPinballDeviceReport`: the standard's report, version 1.0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Microseconds since an arbitrary zero point, on the board's clock.
    pub timestamp: u64,
    pub generic_buttons: u32,
    pub pinball_buttons: u32,
    pub ax_nudge: i16,
    pub ay_nudge: i16,
    pub vx_nudge: i16,
    pub vy_nudge: i16,
    pub plunger_pos: i16,
    pub plunger_speed: i16,
}

impl Report {
    /// VPX's `LoadFromUSB`: the fields in order, little-endian, as far as the bytes go; a field the
    /// bytes don't complete, and every one after it, stays 0.
    pub fn load(bytes: &[u8]) -> Report {
        let mut r = Report::default();
        let mut p = bytes;
        fn take<const N: usize>(p: &mut &[u8]) -> Option<[u8; N]> {
            let (head, rest) = p.split_first_chunk::<N>()?;
            *p = rest;
            Some(*head)
        }
        let _ = (|| -> Option<()> {
            r.timestamp = u64::from_le_bytes(take(&mut p)?);
            r.generic_buttons = u32::from_le_bytes(take(&mut p)?);
            r.pinball_buttons = u32::from_le_bytes(take(&mut p)?);
            r.ax_nudge = i16::from_le_bytes(take(&mut p)?);
            r.ay_nudge = i16::from_le_bytes(take(&mut p)?);
            r.vx_nudge = i16::from_le_bytes(take(&mut p)?);
            r.vy_nudge = i16::from_le_bytes(take(&mut p)?);
            r.plunger_pos = i16::from_le_bytes(take(&mut p)?);
            r.plunger_speed = i16::from_le_bytes(take(&mut p)?);
            Some(())
        })();
        r
    }

    fn axes(&self) -> [(u16, i16); 6] {
        [
            (PLUNGER_POSITION, self.plunger_pos),
            (PLUNGER_SPEED, self.plunger_speed),
            (NUDGE_ACCELERATION_X, self.ax_nudge),
            (NUDGE_ACCELERATION_Y, self.ay_nudge),
            (NUDGE_SPEED_X, self.vx_nudge),
            (NUDGE_SPEED_Y, self.vy_nudge),
        ]
    }
}

/// What changed from one report to the next (VPX's new-report handler): each button pressed or
/// let go, by element, then each axis that moved, by element, from -1 to 1.
pub fn changes(prev: &Report, now: &Report) -> (Vec<(u16, bool)>, Vec<(u16, f32)>) {
    let mut buttons = Vec::new();
    for n in 0..PINBALL_BUTTONS {
        let bit = 1u32 << n;
        if (prev.pinball_buttons ^ now.pinball_buttons) & bit != 0 {
            buttons.push((n, now.pinball_buttons & bit != 0));
        }
    }
    for n in 1..=32u16 {
        let bit = 1u32 << (n - 1);
        if (prev.generic_buttons ^ now.generic_buttons) & bit != 0 {
            buttons.push((GENERIC | n, now.generic_buttons & bit != 0));
        }
    }
    let axes = now.axes().into_iter().zip(prev.axes()).filter(|((_, v), (_, p))| v != p).map(|((e, v), _)| (e, f32::from(v) / 32768.0)).collect();
    (buttons, axes)
}

/// A field a report descriptor may hold the standard's struct in: its report's ID, the string
/// that should name it, and its input report's size in bytes as read (its fields' bits rounded up
/// to bytes, and the ID byte when there is an ID).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub report_id: u8,
    pub string_index: u32,
    pub report_size: usize,
}

/// The fields of a report descriptor that may be the standard's struct, as VPX tests them before
/// reading the string: in an application collection of usage page 5, usage 2, an input field whose
/// one usage is 5:0 and that carries one string index. The descriptor's items are read as the HID
/// specification lays them out (6.2.2): global items kept, and pushed or popped; local ones until
/// the next main item.
pub fn candidates(desc: &[u8]) -> Vec<Field> {
    #[derive(Clone, Copy, Default)]
    struct Global {
        usage_page: u32,
        report_size: u32,
        report_count: u32,
        report_id: u8,
    }
    #[derive(Default)]
    struct Local {
        usages: Vec<u32>,
        usage_min: Option<u32>,
        usage_max: Option<u32>,
        string_index: Option<u32>,
        string_range: bool,
    }
    let mut g = Global::default();
    let mut stack: Vec<Global> = Vec::new();
    let mut local = Local::default();
    // the collections open: whether each is an application collection of a pinball device
    let mut collections: Vec<Option<bool>> = Vec::new();
    let mut input_bits: Vec<(u8, u32)> = Vec::new();
    let mut found: Vec<(u8, u32)> = Vec::new();
    let mut i = 0;
    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xFE {
            // a long item: its size, its tag, its data, all skipped
            let size = desc.get(i + 1).copied().unwrap_or(0) as usize;
            i += 3 + size;
            continue;
        }
        let size = [0, 1, 2, 4][(prefix & 3) as usize];
        let (kind, tag) = ((prefix >> 2) & 3, prefix >> 4);
        let Some(bytes) = desc.get(i + 1..i + 1 + size) else { break };
        let data = bytes.iter().rev().fold(0u32, |v, &b| (v << 8) | u32::from(b));
        i += 1 + size;
        // a usage with the page in it, or the page in force
        let usage = |u: u32, g: &Global| if size == 4 { u } else { (g.usage_page << 16) | u };
        match (kind, tag) {
            // main items
            (0, 8) => {
                let bits = g.report_size * g.report_count;
                match input_bits.iter_mut().find(|r| r.0 == g.report_id) {
                    Some(r) => r.1 += bits,
                    None => input_bits.push((g.report_id, bits)),
                }
                let in_pinball_device = collections.iter().rev().find_map(|c| *c).unwrap_or(false);
                let one_usage = match (local.usages.as_slice(), local.usage_min, local.usage_max) {
                    ([u], None, None) => Some(*u),
                    ([], Some(a), Some(b)) if a == b => Some(a),
                    _ => None,
                };
                if in_pinball_device
                    && one_usage == Some(0x0005_0000)
                    && !local.string_range
                    && let Some(s) = local.string_index
                {
                    found.push((g.report_id, s));
                }
                local = Local::default();
            }
            (0, 10) => {
                // a collection: an application collection (type 1) says whether it's a pinball device
                let u = local.usages.first().copied().or(local.usage_min).unwrap_or(g.usage_page << 16);
                collections.push(if data == 1 { Some(u == 0x0005_0002) } else { None });
                local = Local::default();
            }
            (0, 12) => {
                collections.pop();
                local = Local::default();
            }
            (0, _) => local = Local::default(),
            // global items
            (1, 0) => g.usage_page = data,
            (1, 7) => g.report_size = data,
            (1, 8) => g.report_id = data as u8,
            (1, 9) => g.report_count = data,
            (1, 10) => stack.push(g),
            (1, 11) => g = stack.pop().unwrap_or(g),
            // local items
            (2, 0) => local.usages.push(usage(data, &g)),
            (2, 1) => local.usage_min = Some(usage(data, &g)),
            (2, 2) => local.usage_max = Some(usage(data, &g)),
            (2, 7) => local.string_index = Some(data),
            (2, 8) | (2, 9) => local.string_range = true,
            _ => {}
        }
    }
    found
        .into_iter()
        .map(|(report_id, string_index)| {
            let bits = input_bits.iter().find(|r| r.0 == report_id).map_or(0, |r| r.1);
            Field { report_id, string_index, report_size: bits.div_ceil(8) as usize + usize::from(report_id != 0) }
        })
        .collect()
}

/// The struct's version from its string's suffix (VPX's `deviceStructVersion`): the major number
/// in the high 16 bits, the minor in the low.
pub fn struct_version(suffix: &str) -> u32 {
    let number = |s: &str| s.bytes().take_while(u8::is_ascii_digit).fold(0u32, |v, d| v.wrapping_mul(10).wrapping_add(u32::from(d - b'0')));
    let minor = suffix.split_once('.').map_or(0, |(_, m)| number(m));
    (number(suffix) << 16) | minor
}

/// A board found: its handle, the field its reports carry, and its device in the input map.
struct Board {
    handle: *mut SDL_hid_device,
    field: Field,
}

// SAFETY: the handle is used by the one thread it's handed to, and closed there
unsafe impl Send for Board {}

/// Looks for Open Pinball Devices among the HID interfaces and reads each on a thread of its own,
/// handing what it reports to `sink` (VPX's `OpenPinDevHandler`, which looks once, at the start, as
/// here). Says what it found, once.
pub fn start(sink: Arc<InputSink>) {
    let boards = find();
    if boards.is_empty() {
        return;
    }
    let device = sink.open_pin_device_added();
    for (n, board) in boards.into_iter().enumerate() {
        let sink = sink.clone();
        let spawned = std::thread::Builder::new().name(format!("open pinball device {}", n + 1)).spawn(move || read(board, device, sink));
        if let Err(e) = spawned {
            println!("open pinball device: can't start its reader: {e}");
        }
    }
}

/// The boards connected: every interface of a pinball device's usages whose descriptor holds the
/// standard's struct (VPX stops at the first field of an interface whose string matches).
fn find() -> Vec<Board> {
    let mut boards = Vec::new();
    // SAFETY: SDL's HID calls, the enumeration freed once read, each handle closed unless kept
    unsafe {
        if SDL_hid_init() != 0 {
            return boards;
        }
        let list = SDL_hid_enumerate(0, 0);
        let mut cur = list;
        while !cur.is_null() {
            let info = &*cur;
            cur = info.next;
            if info.usage_page != 0x05 || info.usage != 0x02 || info.path.is_null() {
                continue;
            }
            let handle = SDL_hid_open_path(info.path);
            if handle.is_null() {
                continue;
            }
            let mut desc = vec![0u8; 4096];
            let n = SDL_hid_get_report_descriptor(handle, desc.as_mut_ptr(), desc.len());
            let mut kept = false;
            if n > 0 {
                for field in candidates(&desc[..n as usize]) {
                    let Some(label) = indexed_string(handle, field.string_index) else { continue };
                    let Some(version) = label.strip_prefix("OpenPinballDeviceStruct/") else { continue };
                    let product = if info.product_string.is_null() { String::new() } else { wide(info.product_string) };
                    let v = struct_version(version);
                    println!("open pinball device: {product} (struct {}.{}, report {} bytes): VPX's default mapping unless the settings set OpenPinDev up", v >> 16, v & 0xFFFF, field.report_size);
                    SDL_hid_set_nonblocking(handle, 0);
                    boards.push(Board { handle, field });
                    kept = true;
                    break;
                }
            }
            if !kept {
                SDL_hid_close(handle);
            }
        }
        SDL_hid_free_enumeration(list);
    }
    boards
}

/// A string descriptor by its index (VPX reads up to 128 characters).
///
/// # Safety
/// `handle` must be an open device.
unsafe fn indexed_string(handle: *mut SDL_hid_device, index: u32) -> Option<String> {
    let mut buf: [sdl3_sys::ffi::c_wchar_t; 128] = [0; 128];
    // SAFETY: the buffer's own length less one, so it stays terminated
    let ok = unsafe { SDL_hid_get_indexed_string(handle, index as c_int, buf.as_mut_ptr(), buf.len() - 1) } == 0;
    ok.then(|| wide(buf.as_ptr()))
}

/// A wide string SDL hands over (16 bits a character on Windows, 32 elsewhere), read.
fn wide(p: *const sdl3_sys::ffi::c_wchar_t) -> String {
    let mut units: Vec<u32> = Vec::new();
    // SAFETY: a terminated string SDL gives, read up to its end
    unsafe {
        let mut q = p;
        while !q.is_null() && *q != 0 {
            units.push(*q as u32);
            q = q.add(1);
        }
    }
    if cfg!(windows) {
        String::from_utf16_lossy(&units.iter().map(|&u| u as u16).collect::<Vec<_>>())
    } else {
        units.iter().filter_map(|&u| char::from_u32(u)).collect()
    }
}

/// Reads a board's reports as they come, handing their changes on: buttons timed as they arrive
/// here, axes by the board's own clock, as VPX times them.
fn read(board: Board, device: u16, sink: Arc<InputSink>) {
    let field = board.field;
    let mut buf = vec![0u8; field.report_size.max(1)];
    let mut prev = Report::default();
    let mut errors = 0u32;
    loop {
        if field.report_id != 0 {
            buf[0] = field.report_id;
        }
        // SAFETY: the board's handle, open for this thread alone; the buffer's own length
        let n = unsafe { SDL_hid_read_timeout(board.handle, buf.as_mut_ptr(), buf.len(), 100) };
        if n < 0 {
            // unplugged, or a fault that may pass: VPX keeps trying, as here, but gently
            errors += 1;
            if errors == 1 {
                println!("open pinball device: reading stopped ({}); trying again", sdl_error());
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
            continue;
        }
        errors = 0;
        if n == 0 {
            continue;
        }
        // VPX's reports carry the report's ID first; a board without IDs, as the standard
        // allows, has its struct from the first byte
        let start = usize::from(field.report_id != 0);
        let report = Report::load(buf.get(start..n as usize).unwrap_or(&[]));
        let (buttons, axes) = changes(&prev, &report);
        // SAFETY: SDL's clock
        let now = unsafe { SDL_GetTicksNS() };
        for (element, down) in buttons {
            sink.device_element(device, element, Input::Button(down), now);
        }
        let device_time = report.timestamp.wrapping_mul(1000);
        for (element, value) in axes {
            crate::sensors::set_axis(device, element, device_time, value);
            sink.device_element(device, element, Input::Axis(value), now);
        }
        prev = report;
    }
}

fn sdl_error() -> String {
    // SAFETY: SDL's own error text, valid until the next SDL call on this thread
    unsafe {
        let p = SDL_GetError();
        if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The standard's report descriptor with report ID 3 and string index 4.
    const DESCRIPTOR: &[u8] = &[
        0x05, 0x05, // usage page: game controls
        0x09, 0x02, // usage: pinball device
        0xA1, 0x01, // collection: application
        0x85, 0x03, // report ID 3
        0x79, 0x04, // string index 4
        0x09, 0x00, // usage 0
        0x15, 0x00, // logical minimum 0
        0x26, 0xFF, 0x00, // logical maximum 255
        0x75, 0x08, // report size 8
        0x95, 0x1E, // report count 30
        0x81, 0x00, // input: data, array
        0xC0, // end collection
    ];

    #[test]
    fn the_standards_descriptor_is_found_with_its_size() {
        assert_eq!(candidates(DESCRIPTOR), [Field { report_id: 3, string_index: 4, report_size: 31 }]);
        // without a report ID, as the standard allows for a board with this one report
        let no_id: Vec<u8> = DESCRIPTOR.iter().copied().enumerate().filter(|&(i, _)| i != 6 && i != 7).map(|(_, b)| b).collect();
        assert_eq!(candidates(&no_id), [Field { report_id: 0, string_index: 4, report_size: 30 }]);
        // a gamepad's application collection, or a field with no string: nothing
        let mut gamepad = DESCRIPTOR.to_vec();
        gamepad[3] = 0x05;
        assert!(candidates(&gamepad).is_empty());
        let no_string: Vec<u8> = DESCRIPTOR.iter().copied().enumerate().filter(|&(i, _)| i != 8 && i != 9).map(|(_, b)| b).collect();
        assert!(candidates(&no_string).is_empty());
    }

    #[test]
    fn a_report_reads_as_far_as_its_bytes_go() {
        let mut bytes = Vec::new();
        bytes.extend(1_234_567u64.to_le_bytes());
        bytes.extend(0b101u32.to_le_bytes());
        bytes.extend((1u32 << 9 | 1 << 26).to_le_bytes());
        for v in [100i16, -200, 300, -400, 16384, -32768] {
            bytes.extend(v.to_le_bytes());
        }
        let r = Report::load(&bytes);
        assert_eq!((r.timestamp, r.generic_buttons, r.pinball_buttons), (1_234_567, 0b101, 1 << 9 | 1 << 26));
        assert_eq!((r.ax_nudge, r.ay_nudge, r.vx_nudge, r.vy_nudge, r.plunger_pos, r.plunger_speed), (100, -200, 300, -400, 16384, -32768));
        // cut off inside the plunger's position: it and the speed stay 0
        let short = Report::load(&bytes[..25]);
        assert_eq!((short.vy_nudge, short.plunger_pos, short.plunger_speed), (-400, 0, 0));
    }

    #[test]
    fn changes_are_buttons_then_axes_scaled_as_vpx_scales_them() {
        let prev = Report::default();
        let now = Report { pinball_buttons: 1 << 9, generic_buttons: 1, plunger_pos: 16384, ..Report::default() };
        let (buttons, axes) = changes(&prev, &now);
        assert_eq!(buttons, [(9, true), (GENERIC | 1, true)]);
        assert_eq!(axes, [(PLUNGER_POSITION, 0.5)]);
        // the left flipper let go, nothing else moved
        let later = Report { pinball_buttons: 0, ..now };
        assert_eq!(changes(&now, &later), (vec![(9, false)], vec![]));
    }

    #[test]
    fn the_struct_version_reads_major_and_minor() {
        assert_eq!((struct_version("1.0"), struct_version("2.5"), struct_version("1")), (0x0001_0000, 0x0002_0005, 0x0001_0000));
    }
}
