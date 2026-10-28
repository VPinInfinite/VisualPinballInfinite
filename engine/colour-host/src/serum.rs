// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! A game's Serum file, through libserum (PPUC, GPL-2.0), declared from its
//! headers `serum-decode.h` and `serum.h`: a frame of raw shades in, its
//! colours out — the frame at the game's own size, and for the format's
//! second version a 64-dot-high one too when the file has it — with the
//! palette's or the frame's colour rotations stepped in time, and the
//! trigger a frame carries passed on for PinUP packs.

use std::ffi::{CStr, c_char, c_void};
use std::path::Path;

use colour_link::{DOTS_RGB565, DOTS_RGB888, FORMAT_SERUM_V1, FORMAT_SERUM_V2, IdentifyFrame, MAX_DOTS, Shared};

use crate::win::Library;

/// What `Serum_Load` hands back and every call after fills in (`Serum_Frame_Struc`).
#[repr(C)]
struct SerumFrame {
    // the first version: a palette index a dot, the palette, its rotations
    frame: *mut u8,
    palette: *mut u8,
    rotations: *mut u8,
    // the second: RGB565 dots, 32 and 64 dots high
    frame32: *mut u16,
    width32: u32,
    rotations32: *mut u16,
    rotations_in_frame32: *mut u16,
    modified_elements32: *mut u8,
    frame64: *mut u16,
    width64: u32,
    rotations64: *mut u16,
    rotations_in_frame64: *mut u16,
    modified_elements64: *mut u8,
    // both
    version: u32,
    flags: u8,
    colours: u32,
    triggers: u32,
    trigger_id: u32,
    frame_id: u32,
    rotation_timer: u32,
}

/// `Serum_Load`'s requests: the 32- and the 64-dot-high frame.
const REQUEST_32P_FRAMES: u8 = 1;
const REQUEST_64P_FRAMES: u8 = 2;
/// `flags` after `Serum_Colorize`: which frames it filled.
const RETURNED_32P_FRAME_OK: u8 = 1;
const RETURNED_64P_FRAME_OK: u8 = 2;
/// Added to `Serum_Rotate`'s delay when it rotated a frame's colours.
const ROTATED_V1: u32 = 0x10000;
const ROTATED_V2_32: u32 = 0x10000;
const ROTATED_V2_64: u32 = 0x20000;
/// `Serum_Colorize`: no frame of the file matched, or the same one again.
const IDENTIFY_NO_FRAME: u32 = 0xffff_ffff;
const IDENTIFY_SAME_FRAME: u32 = 0xffff_fffe;
/// A frame with no trigger.
const NO_TRIGGER: u32 = 0xffff_ffff;
/// Delays from here up aren't rotations (the low word of a result).
const LONGEST_DELAY_MS: u32 = 2048;

type LogCallback = unsafe extern "system" fn(format: *const c_char, args: *mut c_void, user: *const c_void);

/// libserum's functions.
struct Api {
    load: unsafe extern "C" fn(path: *const c_char, rom: *const c_char, flags: u8) -> *mut SerumFrame,
    dispose: unsafe extern "C" fn(),
    colorize: unsafe extern "C" fn(frame: *mut u8) -> u32,
    rotate: unsafe extern "C" fn() -> u32,
    version: Option<unsafe extern "C" fn() -> *const c_char>,
    set_log: Option<unsafe extern "C" fn(callback: LogCallback, user: *const c_void)>,
    set_generate_cromc: Option<unsafe extern "C" fn(generate: bool)>,
}

/// What a new frame of raw shades did.
pub enum Coloured {
    /// The file doesn't know it, or it's the frame before again: what's shown
    /// stays, rotating as it did.
    Unchanged,
    /// A new coloured frame, published, with the delay to its first colour
    /// rotation if it has one.
    New(Option<u32>),
}

pub struct Serum {
    api: Api,
    frame: *mut SerumFrame,
    /// The frame handed to libserum: room for the largest display, so a
    /// file made for a larger one never reads past it.
    shades: Vec<u8>,
    /// The game's display size, the first version's frame size.
    size: (u32, u32),
    _library: Library,
}

unsafe extern "C" {
    /// The C runtime's formatter behind every `printf` (UCRT).
    fn __stdio_common_vsprintf(options: u64, buffer: *mut c_char, count: usize, format: *const c_char, locale: *mut c_void, args: *mut c_void) -> i32;
}

unsafe extern "system" fn on_log(format: *const c_char, args: *mut c_void, _user: *const c_void) {
    if format.is_null() {
        return;
    }
    let mut buffer = [0 as c_char; 1024];
    // SAFETY: libserum hands a printf format and its arguments; the buffer is
    // null-terminated whatever the result
    let text = unsafe {
        __stdio_common_vsprintf(2, buffer.as_mut_ptr(), buffer.len(), format, std::ptr::null_mut(), args);
        CStr::from_ptr(buffer.as_ptr()).to_string_lossy().into_owned()
    };
    eprintln!("vpi-colour: Serum: {}", text.trim_end());
}

impl Serum {
    /// Loads `rom`'s file from `folder` (holding `<rom>\<rom>.cROMc` or
    /// `.cRZ`). `cache`: libserum may write the faster `.cROMc` beside a
    /// `.cRZ`, as it does by default.
    // each transmute's target is the field it fills, typed as the header has it
    #[allow(clippy::missing_transmute_annotations)]
    pub fn load(library: &Path, folder: &Path, rom: &str, cache: bool) -> Result<Serum, String> {
        let lib = Library::load(library)?;
        macro_rules! func {
            ($name:literal) => {
                // SAFETY: the function the header declares under that name
                lib.symbol(concat!($name, "\0").as_bytes()).map(|p| unsafe { std::mem::transmute::<*const c_void, _>(p) })
            };
        }
        let missing = |name: &str| format!("{} has no {name}", library.display());
        let api = Api {
            load: func!("Serum_Load").ok_or_else(|| missing("Serum_Load"))?,
            dispose: func!("Serum_Dispose").ok_or_else(|| missing("Serum_Dispose"))?,
            colorize: func!("Serum_Colorize").ok_or_else(|| missing("Serum_Colorize"))?,
            rotate: func!("Serum_Rotate").ok_or_else(|| missing("Serum_Rotate"))?,
            version: func!("Serum_GetVersion"),
            set_log: func!("Serum_SetLogCallback"),
            set_generate_cromc: func!("Serum_SetGenerateCRomC"),
        };
        let path = crate::win::ansi_path(folder).ok_or_else(|| format!("{} can't be said in Windows' code page", folder.display()))?;
        let rom_c = std::ffi::CString::new(rom).map_err(|_| "a ROM name with a null in it".to_string())?;
        // SAFETY: libserum's calls, as its header describes them
        let frame = unsafe {
            if let Some(set_log) = api.set_log {
                set_log(on_log, std::ptr::null());
            }
            if let Some(set) = api.set_generate_cromc {
                set(cache);
            }
            (api.load)(path.as_ptr(), rom_c.as_ptr(), REQUEST_32P_FRAMES | REQUEST_64P_FRAMES)
        };
        if frame.is_null() {
            return Err(format!("libserum couldn't load {rom}'s file from {}", folder.display()));
        }
        Ok(Serum { api, frame, shades: vec![0; MAX_DOTS], size: (0, 0), _library: lib })
    }

    /// libserum's version, "major.minor.patch".
    pub fn library_version(&self) -> String {
        // SAFETY: a static string from libserum
        self.api.version.map_or_else(String::new, |v| unsafe { CStr::from_ptr(v()).to_string_lossy().into_owned() })
    }

    /// The file's format version, as the link names it.
    pub fn format(&self) -> u32 {
        // SAFETY: libserum's frame, filled in by Serum_Load
        if unsafe { (*self.frame).version } == 2 { FORMAT_SERUM_V2 } else { FORMAT_SERUM_V1 }
    }

    /// Colours a new frame of raw shades and publishes what changed.
    pub fn colour(&mut self, dots: &[u8], frame: &IdentifyFrame, shared: &Shared) -> Coloured {
        let n = dots.len().min(MAX_DOTS);
        self.shades[..n].copy_from_slice(&dots[..n]);
        self.shades[n..].fill(0);
        self.size = (frame.width, frame.height);
        // SAFETY: a buffer as large as the largest display libserum reads
        let first = unsafe { (self.api.colorize)(self.shades.as_mut_ptr()) };
        if first == IDENTIFY_NO_FRAME || first == IDENTIFY_SAME_FRAME {
            return Coloured::Unchanged;
        }
        // SAFETY: libserum's frame, filled in by Serum_Colorize
        let s = unsafe { &*self.frame };
        if s.version == 2 {
            if s.flags & RETURNED_32P_FRAME_OK != 0 {
                self.publish_32(shared);
            }
            if s.flags & RETURNED_64P_FRAME_OK != 0 {
                self.publish_64(shared);
            }
        } else {
            self.publish_v1(shared);
        }
        if s.trigger_id != NO_TRIGGER {
            shared.publish_trigger(s.trigger_id);
        }
        Coloured::New(delay(first))
    }

    /// Steps the colour rotations once and publishes what they changed; the
    /// delay to the next step, if there's one.
    pub fn rotate(&mut self, shared: &Shared) -> Option<u32> {
        // SAFETY: libserum's call on a loaded file
        let next = unsafe { (self.api.rotate)() };
        // SAFETY: as above
        if unsafe { (*self.frame).version } == 2 {
            if next & ROTATED_V2_32 != 0 {
                self.publish_32(shared);
            }
            if next & ROTATED_V2_64 != 0 {
                self.publish_64(shared);
            }
        } else if next & ROTATED_V1 != 0 {
            self.publish_v1(shared);
        }
        delay(next)
    }

    /// The first version's frame: each dot's palette entry, at the game's size.
    fn publish_v1(&self, shared: &Shared) {
        let (width, height) = self.size;
        let n = (width * height) as usize;
        // SAFETY: libserum's frame of `n` indices and its palette of 64 colours
        let (frame, palette) = unsafe {
            let s = &*self.frame;
            if s.frame.is_null() || s.palette.is_null() {
                return;
            }
            (std::slice::from_raw_parts(s.frame, n), std::slice::from_raw_parts(s.palette, 64 * 3))
        };
        let colour = |i: u8| {
            let p = &palette[(usize::from(i) % 64) * 3..][..3];
            (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2])
        };
        shared.publish_plane(0, width, height, DOTS_RGB888, frame.iter().map(|&i| colour(i)));
    }

    /// The second version's 32-dot-high frame.
    fn publish_32(&self, shared: &Shared) {
        // SAFETY: libserum's frame
        let (dots, width) = unsafe { ((*self.frame).frame32, (*self.frame).width32) };
        publish_565(shared, 0, dots, width, 32);
    }

    /// The second version's 64-dot-high frame.
    fn publish_64(&self, shared: &Shared) {
        // SAFETY: libserum's frame
        let (dots, width) = unsafe { ((*self.frame).frame64, (*self.frame).width64) };
        publish_565(shared, 1, dots, width, 64);
    }
}

/// An RGB565 frame of `width` × `height` into plane `index`, when there is one.
fn publish_565(shared: &Shared, index: usize, dots: *const u16, width: u32, height: u32) {
    if dots.is_null() || width == 0 {
        return;
    }
    // SAFETY: libserum's frame of `width` × `height` dots
    let dots = unsafe { std::slice::from_raw_parts(dots, (width * height) as usize) };
    shared.publish_plane(index, width, height, DOTS_RGB565, dots.iter().map(|&d| u32::from(d)));
}

/// The delay in a result's low word, when it's a rotation's.
fn delay(result: u32) -> Option<u32> {
    let ms = result & 0xffff;
    (ms != 0 && ms < LONGEST_DELAY_MS).then_some(ms)
}

impl Drop for Serum {
    fn drop(&mut self) {
        // SAFETY: the file loaded, released once
        unsafe { (self.api.dispose)() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_struct_is_laid_out_as_libserum_lays_it_out() {
        // serum.h's Serum_Frame_Struc on 64-bit Windows: five pointers before width32's padding
        assert_eq!(std::mem::offset_of!(SerumFrame, width32), 32);
        assert_eq!(std::mem::offset_of!(SerumFrame, frame64), 64);
        assert_eq!(std::mem::offset_of!(SerumFrame, version), 104);
        assert_eq!(std::mem::offset_of!(SerumFrame, flags), 108);
        assert_eq!(std::mem::offset_of!(SerumFrame, trigger_id), 120);
        assert_eq!(std::mem::size_of::<SerumFrame>(), 136);
    }

    #[test]
    fn only_short_delays_are_rotations() {
        assert_eq!(delay(0), None);
        assert_eq!(delay(ROTATED_V2_64 | 40), Some(40));
        assert_eq!(delay(LONGEST_DELAY_MS), None);
        assert_eq!(delay(IDENTIFY_NO_FRAME), None);
    }
}
