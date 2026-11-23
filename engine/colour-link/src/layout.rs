// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! The shared block: every field an atomic, so both programs may touch it
//! at once without undefined behaviour. Frames go across under sequence
//! locks, odd while being written, so a reader that caught one half
//! written reads it again.

use std::sync::atomic::{AtomicU32, Ordering, fence};

/// Marks a block as a colour link.
pub const MAGIC: u32 = u32::from_le_bytes(*b"VPIC");
/// Changes whenever the layout below does; the host refuses another one.
pub const LAYOUT_VERSION: u32 = 1;

/// The largest score display carried: 256×64, the biggest PinMAME draws and
/// the biggest a colour file's extra resolution makes.
pub const MAX_WIDTH: usize = 256;
pub const MAX_HEIGHT: usize = 64;
pub const MAX_DOTS: usize = MAX_WIDTH * MAX_HEIGHT;
/// Triggers kept for the engine to read: far more than a frame brings.
pub const TRIGGER_SLOTS: usize = 64;

/// The host's state.
pub const STATE_STARTING: u32 = 0;
/// The colour file is loaded; frames are coloured as they come.
pub const STATE_READY: u32 = 1;
/// No colour file could be loaded: the game shows its own display.
pub const STATE_FAILED: u32 = 2;

/// Which kind of file colours the game.
pub const FORMAT_NONE: u32 = 0;
pub const FORMAT_SERUM_V1: u32 = 1;
pub const FORMAT_SERUM_V2: u32 = 2;
pub const FORMAT_VNI: u32 = 3;

/// How a coloured frame's dots are kept: `0x00RRGGBB`, sRGB values.
pub const DOTS_RGB888: u32 = 0;
/// Or RGB565 in the low 16 bits (Serum's second format), as the file made it.
pub const DOTS_RGB565: u32 = 1;

/// The game's newest frame to colour: its raw shades, a byte a dot.
#[repr(C)]
struct Input {
    seq: AtomicU32,
    number: AtomicU32,
    width: AtomicU32,
    height: AtomicU32,
    bits: AtomicU32,
    /// Four dots a word, the first in the lowest byte.
    dots: [AtomicU32; MAX_DOTS / 4],
}

/// A coloured frame: plane 0 the game's own size (a second-format file's
/// 32-dot-high frame), plane 1 a second-format file's 64-dot-high frame.
#[repr(C)]
struct Plane {
    seq: AtomicU32,
    width: AtomicU32,
    height: AtomicU32,
    kind: AtomicU32,
    dots: [AtomicU32; MAX_DOTS],
}

/// The whole shared block.
#[repr(C)]
pub struct Shared {
    magic: AtomicU32,
    version: AtomicU32,
    state: AtomicU32,
    format: AtomicU32,
    stop: AtomicU32,
    input: Input,
    planes: [Plane; 2],
    trigger_count: AtomicU32,
    triggers: [AtomicU32; TRIGGER_SLOTS],
}

/// The frame to colour, as the host reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentifyFrame {
    /// Counts the frames sent: a new number, a new frame.
    pub seq: u32,
    /// The game's own frame number.
    pub number: u32,
    pub width: u32,
    pub height: u32,
    /// Bits a dot: 2 or 4.
    pub bits: u32,
}

/// A coloured frame, as the engine reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaneFrame {
    /// Counts the frames published in this plane.
    pub seq: u32,
    pub width: u32,
    pub height: u32,
    /// [`DOTS_RGB888`] or [`DOTS_RGB565`].
    pub kind: u32,
}

impl Shared {
    pub(crate) fn init(&self) {
        self.version.store(LAYOUT_VERSION, Ordering::Relaxed);
        self.magic.store(MAGIC, Ordering::Release);
    }

    /// Whether the block is a link of this layout.
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.magic.load(Ordering::Acquire) != MAGIC {
            return Err("not a colour link".into());
        }
        let v = self.version.load(Ordering::Relaxed);
        if v != LAYOUT_VERSION {
            return Err(format!("link layout {v}, this program speaks {LAYOUT_VERSION}"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- state

    /// The host's state and the kind of file it loaded.
    pub fn state(&self) -> (u32, u32) {
        let format = self.format.load(Ordering::Relaxed);
        (self.state.load(Ordering::Acquire), format)
    }

    pub fn set_state(&self, state: u32, format: u32) {
        self.format.store(format, Ordering::Relaxed);
        self.state.store(state, Ordering::Release);
    }

    /// The engine asks the host to end.
    pub fn request_stop(&self) {
        self.stop.store(1, Ordering::Release);
    }

    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Acquire) != 0
    }

    // ---------------------------------------------------------------- input

    /// The engine hands over the game's newest frame to colour; false when it
    /// doesn't fit.
    pub fn send_identify(&self, number: u32, width: u32, height: u32, bits: u32, dots: &[u8]) -> bool {
        let n = (width * height) as usize;
        if width as usize > MAX_WIDTH || height as usize > MAX_HEIGHT || dots.len() < n {
            return false;
        }
        let input = &self.input;
        let seq = input.seq.load(Ordering::Relaxed);
        input.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        for (word, four) in input.dots.iter().zip(dots[..n].chunks(4)) {
            let mut w = [0u8; 4];
            w[..four.len()].copy_from_slice(four);
            word.store(u32::from_le_bytes(w), Ordering::Relaxed);
        }
        input.number.store(number, Ordering::Relaxed);
        input.width.store(width, Ordering::Relaxed);
        input.height.store(height, Ordering::Relaxed);
        input.bits.store(bits, Ordering::Relaxed);
        input.seq.store(seq.wrapping_add(2), Ordering::Release);
        true
    }

    /// How many frames the engine has sent: a cheap check for a new one.
    pub fn identify_seq(&self) -> u32 {
        self.input.seq.load(Ordering::Acquire) / 2
    }

    /// The host copies the newest whole frame into `out`, a byte a dot; None
    /// before the first.
    pub fn read_identify(&self, out: &mut Vec<u8>) -> Option<IdentifyFrame> {
        let input = &self.input;
        loop {
            let before = input.seq.load(Ordering::Acquire);
            if before == 0 {
                return None;
            }
            if before & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let width = input.width.load(Ordering::Relaxed).min(MAX_WIDTH as u32);
            let height = input.height.load(Ordering::Relaxed).min(MAX_HEIGHT as u32);
            let frame = IdentifyFrame { seq: before / 2, number: input.number.load(Ordering::Relaxed), width, height, bits: input.bits.load(Ordering::Relaxed) };
            let n = (width * height) as usize;
            out.clear();
            for word in input.dots.iter().take(n.div_ceil(4)) {
                out.extend_from_slice(&word.load(Ordering::Relaxed).to_le_bytes());
            }
            out.truncate(n);
            fence(Ordering::Acquire);
            if input.seq.load(Ordering::Relaxed) == before {
                return Some(frame);
            }
        }
    }

    // --------------------------------------------------------------- output

    /// The host publishes a coloured frame in plane `index` (0 or 1); false
    /// when it doesn't fit.
    pub fn publish_plane(&self, index: usize, width: u32, height: u32, kind: u32, dots: impl Iterator<Item = u32>) -> bool {
        let Some(plane) = self.planes.get(index) else { return false };
        if width as usize > MAX_WIDTH || height as usize > MAX_HEIGHT {
            return false;
        }
        let seq = plane.seq.load(Ordering::Relaxed);
        plane.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        for (slot, v) in plane.dots.iter().zip(dots.take((width * height) as usize)) {
            slot.store(v, Ordering::Relaxed);
        }
        plane.width.store(width, Ordering::Relaxed);
        plane.height.store(height, Ordering::Relaxed);
        plane.kind.store(kind, Ordering::Relaxed);
        plane.seq.store(seq.wrapping_add(2), Ordering::Release);
        true
    }

    /// How many frames plane `index` has had: a cheap check for a new one.
    pub fn plane_seq(&self, index: usize) -> u32 {
        self.planes.get(index).map_or(0, |p| p.seq.load(Ordering::Acquire) / 2)
    }

    /// The engine copies plane `index`'s newest whole frame into `out`, a
    /// word a dot; None before its first.
    pub fn read_plane(&self, index: usize, out: &mut Vec<u32>) -> Option<PlaneFrame> {
        let plane = self.planes.get(index)?;
        loop {
            let before = plane.seq.load(Ordering::Acquire);
            if before == 0 {
                return None;
            }
            if before & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let width = plane.width.load(Ordering::Relaxed).min(MAX_WIDTH as u32);
            let height = plane.height.load(Ordering::Relaxed).min(MAX_HEIGHT as u32);
            let frame = PlaneFrame { seq: before / 2, width, height, kind: plane.kind.load(Ordering::Relaxed) };
            out.clear();
            out.extend(plane.dots.iter().take((width * height) as usize).map(|d| d.load(Ordering::Relaxed)));
            fence(Ordering::Acquire);
            if plane.seq.load(Ordering::Relaxed) == before {
                return Some(frame);
            }
        }
    }

    // ------------------------------------------------------------- triggers

    /// The host passes on a trigger its file gives a frame (for PinUP packs).
    pub fn publish_trigger(&self, id: u32) {
        let n = self.trigger_count.load(Ordering::Relaxed);
        self.triggers[n as usize % TRIGGER_SLOTS].store(id, Ordering::Relaxed);
        self.trigger_count.store(n.wrapping_add(1), Ordering::Release);
    }

    /// The triggers published since `cursor`, which moves on; a reader a
    /// whole ring behind gets the newest ring's worth.
    pub fn triggers_since(&self, cursor: &mut u32, out: &mut Vec<u32>) {
        let n = self.trigger_count.load(Ordering::Acquire);
        let behind = n.wrapping_sub(*cursor).min(TRIGGER_SLOTS as u32);
        for k in 0..behind {
            let i = n.wrapping_sub(behind).wrapping_add(k);
            out.push(self.triggers[i as usize % TRIGGER_SLOTS].load(Ordering::Relaxed));
        }
        *cursor = n;
    }
}
