// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! The link between the player and the programs that drive a cabinet's real
//! DMD panel beside it: `vpi-dmd`, through libdmdutil, and
//! `vpi-dmddevice`, through a VPinMAME display device (DMD Extensions, or a
//! panel maker's own): one block of shared memory and a named event.
//!
//! The engine creates the link and starts the host with its name; the host
//! opens it. Engine to host: the display's newest frame — its number, size
//! and format, VPX's display formats (a brightness a dot as a 32-bit float,
//! or sRGB colours in 24 or 16 bits), and for a display device what
//! VPinMAME hands one: a machine's DMD frame with its raw shades beside its
//! brightness, or a segment game's segments with PinMAME's layout — written
//! under a sequence count so the host never reads half of one — requests for
//! a picture of what the panel shows (for checks without a panel), and a
//! stop. Host to engine: its state.

mod win;

use std::io;
use std::sync::atomic::{AtomicU32, Ordering};

/// Marks a block as a DMD panel link.
pub const MAGIC: u32 = u32::from_le_bytes(*b"VPIM");
/// Changes whenever the layout below does; the host refuses another one.
pub const LAYOUT_VERSION: u32 = 2;
/// The most dots a frame holds: libdmdutil's own limit, 256 × 64.
pub const MAX_DOTS: usize = 256 * 64;

/// A brightness a dot from 0 to 1, a 32-bit float's bits (VPX's `LUM32F`).
pub const FORMAT_LUM: u32 = 1;
/// sRGB colours, 8 bits each, as `0x00RRGGBB` (VPX's `SRGB888`).
pub const FORMAT_RGB888: u32 = 2;
/// sRGB colours, 5, 6 and 5 bits, in the low 16 bits (VPX's `SRGB565`).
pub const FORMAT_RGB565: u32 = 3;
/// A machine's DMD frame as VPinMAME hands a display device: its brightness a dot, as
/// `FORMAT_LUM`, and beside it PinMAME's raw shades (its identify frame), `raw_bits` bits a dot.
pub const FORMAT_LUM_RAW: u32 = 4;
/// A segment game's displays as VPinMAME hands a display device: PinMAME's layout of them
/// (`core_segOverallLayout_t`) and its segment words (`seg_data`, then `seg_data2`).
pub const FORMAT_ALPHANUMERIC: u32 = 5;
/// Shades a dot, 0 to 3, as a display device's `Render_4_Shades` takes them (FlexDMD's 4-shade
/// grey).
pub const FORMAT_SHADES4: u32 = 6;
/// Shades a dot, 0 to 15, as `Render_16_Shades` takes them (FlexDMD's 16-shade grey).
pub const FORMAT_SHADES16: u32 = 7;
/// The segment words a frame carries, each of `seg_data` and `seg_data2` (PinMAME's
/// `CORE_SEGCOUNT`).
pub const SEG_WORDS: usize = 128;

/// The host's state: starting (libdmdutil loading and looking for panels).
pub const STATE_STARTING: u32 = 0;
/// libdmdutil is running and takes the frames.
pub const STATE_READY: u32 = 1;
/// libdmdutil couldn't be loaded: no panel is driven.
pub const STATE_FAILED: u32 = 2;

/// The whole shared block.
#[repr(C)]
pub struct Shared {
    magic: AtomicU32,
    version: AtomicU32,
    state: AtomicU32,
    stop: AtomicU32,
    /// Odd while the engine writes a frame, even once it's whole.
    seq: AtomicU32,
    frame_id: AtomicU32,
    width: AtomicU32,
    height: AtomicU32,
    format: AtomicU32,
    /// The newest picture asked for (a number the engine chooses, never 0), and the newest the
    /// host has saved.
    picture_asked: AtomicU32,
    picture_saved: AtomicU32,
    dots: [AtomicU32; MAX_DOTS],
    /// `FORMAT_LUM_RAW`'s raw shades, a byte a dot, four to a word, and their bits a dot.
    raw: [AtomicU32; MAX_DOTS / 4],
    raw_bits: AtomicU32,
    /// `FORMAT_ALPHANUMERIC`'s layout and segment words.
    seg_layout: AtomicU32,
    segs: [AtomicU32; 2 * SEG_WORDS],
}

/// A frame as the host reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub dots: Vec<u32>,
    /// `FORMAT_LUM_RAW`'s raw shades and their bits a dot; empty otherwise.
    pub raw: Vec<u8>,
    pub raw_bits: u32,
    /// `FORMAT_ALPHANUMERIC`'s layout and segment words (`seg_data`, then `seg_data2`).
    pub seg_layout: u32,
    pub segs: Vec<u16>,
}

impl Shared {
    fn init(&self) {
        self.version.store(LAYOUT_VERSION, Ordering::Relaxed);
        self.magic.store(MAGIC, Ordering::Release);
    }

    fn check(&self) -> Result<(), String> {
        if self.magic.load(Ordering::Acquire) != MAGIC {
            return Err("not a DMD panel link".into());
        }
        let v = self.version.load(Ordering::Relaxed);
        if v != LAYOUT_VERSION {
            return Err(format!("link layout {v}, this program speaks {LAYOUT_VERSION}"));
        }
        Ok(())
    }

    /// The engine hands over a frame, numbered `id`; false when it's too large for a panel.
    pub fn send(&self, id: u32, width: u32, height: u32, format: u32, dots: &[u32]) -> bool {
        self.write(id, width, height, format, dots, None, None)
    }

    /// The engine hands over a machine's DMD frame for a display device (`FORMAT_LUM_RAW`): its
    /// brightness a dot as a float's bits, and PinMAME's raw shades, `bits` bits a dot.
    pub fn send_lum_raw(&self, id: u32, width: u32, height: u32, lum: &[u32], raw: &[u8], bits: u32) -> bool {
        let n = (width * height) as usize;
        raw.len() >= n && self.write(id, width, height, FORMAT_LUM_RAW, lum, Some((&raw[..n], bits)), None)
    }

    /// The engine hands over a segment game's displays for a display device
    /// (`FORMAT_ALPHANUMERIC`): PinMAME's layout of them and its segment words.
    pub fn send_alphanumeric(&self, id: u32, layout: u32, seg_data: &[u16], seg_data2: &[u16]) -> bool {
        self.write(id, 0, 0, FORMAT_ALPHANUMERIC, &[], None, Some((layout, seg_data, seg_data2)))
    }

    fn write(&self, id: u32, width: u32, height: u32, format: u32, dots: &[u32], raw: Option<(&[u8], u32)>, segs: Option<(u32, &[u16], &[u16])>) -> bool {
        let n = (width * height) as usize;
        if n > MAX_DOTS || dots.len() < n {
            return false;
        }
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        std::sync::atomic::fence(Ordering::Release);
        for (slot, &d) in self.dots.iter().zip(&dots[..n]) {
            slot.store(d, Ordering::Relaxed);
        }
        if let Some((raw, bits)) = raw {
            for (slot, four) in self.raw.iter().zip(raw.chunks(4)) {
                let mut word = [0u8; 4];
                word[..four.len()].copy_from_slice(four);
                slot.store(u32::from_le_bytes(word), Ordering::Relaxed);
            }
            self.raw_bits.store(bits, Ordering::Relaxed);
        }
        if let Some((layout, seg_data, seg_data2)) = segs {
            self.seg_layout.store(layout, Ordering::Relaxed);
            for (k, slot) in self.segs.iter().enumerate() {
                let word = if k < SEG_WORDS { seg_data.get(k) } else { seg_data2.get(k - SEG_WORDS) };
                slot.store(u32::from(word.copied().unwrap_or(0)), Ordering::Relaxed);
            }
        }
        self.width.store(width, Ordering::Relaxed);
        self.height.store(height, Ordering::Relaxed);
        self.format.store(format, Ordering::Relaxed);
        self.frame_id.store(id, Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(2), Ordering::Release);
        true
    }

    /// The host reads the newest frame, when it isn't `last` (its number); None while the
    /// engine is writing one or nothing new came.
    pub fn read(&self, last: Option<u32>) -> Option<Frame> {
        let before = self.seq.load(Ordering::Acquire);
        if before & 1 == 1 || before == 0 {
            return None;
        }
        let id = self.frame_id.load(Ordering::Relaxed);
        if last == Some(id) {
            return None;
        }
        let (width, height, format) = (self.width.load(Ordering::Relaxed), self.height.load(Ordering::Relaxed), self.format.load(Ordering::Relaxed));
        let n = ((width * height) as usize).min(MAX_DOTS);
        let dots: Vec<u32> = self.dots[..n].iter().map(|d| d.load(Ordering::Relaxed)).collect();
        let (mut raw, mut raw_bits, mut seg_layout, mut segs) = (Vec::new(), 0, 0, Vec::new());
        if format == FORMAT_LUM_RAW {
            raw = self.raw[..n.div_ceil(4)].iter().flat_map(|w| w.load(Ordering::Relaxed).to_le_bytes()).take(n).collect();
            raw_bits = self.raw_bits.load(Ordering::Relaxed);
        }
        if format == FORMAT_ALPHANUMERIC {
            seg_layout = self.seg_layout.load(Ordering::Relaxed);
            segs = self.segs.iter().map(|w| w.load(Ordering::Relaxed) as u16).collect();
        }
        std::sync::atomic::fence(Ordering::Acquire);
        (self.seq.load(Ordering::Relaxed) == before).then_some(Frame { id, width, height, format, dots, raw, raw_bits, seg_layout, segs })
    }

    /// The engine asks for a picture of what the panel shows now, numbered `number` (not 0).
    pub fn ask_picture(&self, number: u32) {
        self.picture_asked.store(number, Ordering::Release);
    }

    /// The picture asked for, when the host hasn't saved it yet.
    pub fn picture_wanted(&self) -> Option<u32> {
        let asked = self.picture_asked.load(Ordering::Acquire);
        (asked != 0 && asked != self.picture_saved.load(Ordering::Acquire)).then_some(asked)
    }

    /// The host has saved the picture numbered `number`.
    pub fn picture_done(&self, number: u32) {
        self.picture_saved.store(number, Ordering::Release);
    }

    /// The newest picture the host saved.
    pub fn pictures_saved(&self) -> u32 {
        self.picture_saved.load(Ordering::Acquire)
    }

    pub fn set_state(&self, state: u32) {
        self.state.store(state, Ordering::Release);
    }

    pub fn state(&self) -> u32 {
        self.state.load(Ordering::Acquire)
    }

    /// The engine asks the host to end (libdmdutil blanking the panel first).
    pub fn request_stop(&self) {
        self.stop.store(1, Ordering::Release);
    }

    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Acquire) != 0
    }
}

/// One end of a link.
pub struct Link {
    name: String,
    mapping: win::Handle,
    view: *mut Shared,
    to_host: win::Handle,
}

// SAFETY: the block is shared memory made of atomics, and the handles are
// process-wide; nothing here is tied to the thread that made it
unsafe impl Send for Link {}
unsafe impl Sync for Link {}

impl Link {
    /// The engine's end: a new link of that name, for one game.
    pub fn create(name: &str) -> io::Result<Link> {
        let size = size_of::<Shared>();
        // SAFETY: a page-file-backed mapping of `size` bytes, which Windows zeroes
        let mapping =
            unsafe { win::CreateFileMappingW(win::INVALID_HANDLE_VALUE, std::ptr::null(), win::PAGE_READWRITE, (size as u64 >> 32) as u32, size as u32, win::wide(name).as_ptr()) };
        if mapping.is_null() {
            return Err(io::Error::last_os_error());
        }
        if io::Error::last_os_error().raw_os_error() == Some(win::ERROR_ALREADY_EXISTS) {
            // SAFETY: ours to close
            unsafe { win::CloseHandle(mapping) };
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("a link named {name} already exists")));
        }
        let link = Link::finish(name, mapping, true)?;
        link.shared().init();
        Ok(link)
    }

    /// The host's end: the link the engine made.
    pub fn open(name: &str) -> io::Result<Link> {
        // SAFETY: plain Windows calls with a null-terminated name
        let mapping = unsafe { win::OpenFileMappingW(win::FILE_MAP_ALL_ACCESS, 0, win::wide(name).as_ptr()) };
        if mapping.is_null() {
            return Err(io::Error::last_os_error());
        }
        let link = Link::finish(name, mapping, false)?;
        link.shared().check().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(link)
    }

    fn finish(name: &str, mapping: win::Handle, create: bool) -> io::Result<Link> {
        // SAFETY: maps the whole block; the mapping is valid
        let view = unsafe { win::MapViewOfFile(mapping, win::FILE_MAP_ALL_ACCESS, 0, 0, size_of::<Shared>()) } as *mut Shared;
        let mut link = Link { name: name.to_string(), mapping, view, to_host: std::ptr::null_mut() };
        if view.is_null() {
            return Err(io::Error::last_os_error());
        }
        let n = win::wide(&format!("{name}.to-host"));
        // SAFETY: an auto-reset event, initially unset
        let h = unsafe { if create { win::CreateEventW(std::ptr::null(), 0, 0, n.as_ptr()) } else { win::OpenEventW(win::EVENT_MODIFY_STATE | win::SYNCHRONIZE, 0, n.as_ptr()) } };
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        link.to_host = h;
        Ok(link)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn shared(&self) -> &Shared {
        // SAFETY: mapped for as long as the link lives; all of it is atomics
        unsafe { &*self.view }
    }

    /// The engine tells the host there's a new frame, a picture wanted, or a stop.
    pub fn wake_host(&self) {
        // SAFETY: an event handle we own
        unsafe { win::SetEvent(self.to_host) };
    }

    /// The event the host waits on, for its own wait among several.
    pub fn to_host_event(&self) -> *mut std::ffi::c_void {
        self.to_host
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        // SAFETY: each was made by this link and is released once
        unsafe {
            if !self.view.is_null() {
                win::UnmapViewOfFile(self.view as *const _);
            }
            for h in [self.to_host, self.mapping] {
                if !h.is_null() {
                    win::CloseHandle(h);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_cross_the_link_whole() {
        let name = format!("Local\\VPI-dmd-test-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        assert_eq!(host.shared().read(None), None, "nothing sent yet");
        let dots: Vec<u32> = (0..128 * 32).collect();
        assert!(engine.shared().send(7, 128, 32, FORMAT_RGB888, &dots));
        let f = host.shared().read(None).expect("a frame");
        assert_eq!((f.id, f.width, f.height, f.format), (7, 128, 32, FORMAT_RGB888));
        assert_eq!(f.dots, dots);
        // the same frame isn't read twice
        assert_eq!(host.shared().read(Some(7)), None);
        // larger than a panel takes: refused
        assert!(!engine.shared().send(8, 512, 128, FORMAT_LUM, &vec![0; 512 * 128]));
        // pictures asked for and saved
        assert_eq!(host.shared().picture_wanted(), None);
        engine.shared().ask_picture(3);
        assert_eq!(host.shared().picture_wanted(), Some(3));
        host.shared().picture_done(3);
        assert_eq!(host.shared().picture_wanted(), None);
        assert_eq!(engine.shared().pictures_saved(), 3);
        engine.shared().request_stop();
        assert!(host.shared().stop_requested());
    }

    #[test]
    fn a_display_devices_frames_cross_with_their_raw_shades_and_segments() {
        let name = format!("Local\\VPI-dmd-test-dev-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        let lum: Vec<u32> = (0..128 * 32).map(|k| (k as f32 / 4096.0).to_bits()).collect();
        let raw: Vec<u8> = (0..128 * 32).map(|k| (k % 4) as u8).collect();
        assert!(engine.shared().send_lum_raw(1, 128, 32, &lum, &raw, 2));
        let f = host.shared().read(None).expect("a frame");
        assert_eq!((f.format, f.raw_bits, f.raw.len()), (FORMAT_LUM_RAW, 2, 128 * 32));
        assert_eq!((f.dots, f.raw), (lum, raw));
        let segs: Vec<u16> = (0..40).map(|k| k * 3).collect();
        assert!(engine.shared().send_alphanumeric(2, 9, &segs, &[7, 8]));
        let f = host.shared().read(Some(1)).expect("a frame");
        assert_eq!((f.format, f.seg_layout, f.segs.len()), (FORMAT_ALPHANUMERIC, 9, 2 * SEG_WORDS));
        assert_eq!((&f.segs[..40], f.segs[40], f.segs[SEG_WORDS], f.segs[SEG_WORDS + 1]), (&segs[..], 0, 7, 8));
    }
}
