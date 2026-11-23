// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! PIN2DMD's older colour files — a palette file (`.pal`) and, often, an
//! animation file (`.vni`) — through libvni (PPUC, GPL-2.0), declared from
//! its header `vni.h`: a frame of raw shades in, its colours out, at the
//! size libvni gives. Its newer, encrypted files (`.pac`) libvni can't read.

use std::ffi::c_void;
use std::path::Path;

use colour_link::{DOTS_RGB888, FORMAT_VNI, IdentifyFrame, Shared};

use crate::win::Library;

/// `Vni_Frame_Struc`: the coloured frame, a palette index a dot.
#[repr(C)]
struct VniFrame {
    width: u32,
    height: u32,
    bits: u8,
    has_frame: u8,
    frame: *const u8,
    /// RGB triples, `1 << bits` of them.
    palette: *const u8,
}

/// libvni's functions.
struct Api {
    load: unsafe extern "C" fn(pal: *const i8, vni: *const i8, pac: *const i8, key: *const i8) -> *mut c_void,
    dispose: unsafe extern "C" fn(context: *mut c_void),
    get_frame: unsafe extern "C" fn(context: *const c_void) -> *const VniFrame,
    colorize: unsafe extern "C" fn(context: *mut c_void, frame: *const u8, width: u32, height: u32, bits: u8) -> u32,
}

pub struct Vni {
    api: Api,
    context: *mut c_void,
    _library: Library,
}

impl Vni {
    /// Loads the palette file `pal` and, if given, the animation file `vni`.
    // each transmute's target is the field it fills, typed as the header has it
    #[allow(clippy::missing_transmute_annotations)]
    pub fn load(library: &Path, pal: &Path, vni: Option<&Path>) -> Result<Vni, String> {
        let lib = Library::load(library)?;
        macro_rules! func {
            ($name:literal) => {
                // SAFETY: the function the header declares under that name
                lib.symbol(concat!($name, "\0").as_bytes())
                    .map(|p| unsafe { std::mem::transmute::<*const c_void, _>(p) })
                    .ok_or_else(|| format!("{} has no {}", library.display(), $name))?
            };
        }
        let api = Api { load: func!("Vni_LoadFromPaths"), dispose: func!("Vni_Dispose"), get_frame: func!("Vni_GetFrame"), colorize: func!("Vni_Colorize") };
        let ansi = |p: &Path| crate::win::ansi_path(p).ok_or_else(|| format!("{} can't be said in Windows' code page", p.display()));
        let pal_c = ansi(pal)?;
        let vni_c = vni.map(ansi).transpose()?;
        // SAFETY: libvni's call, as its header describes it; no encrypted file, no key
        let context = unsafe { (api.load)(pal_c.as_ptr(), vni_c.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()), std::ptr::null(), std::ptr::null()) };
        if context.is_null() {
            return Err(format!("libvni couldn't load {}", pal.display()));
        }
        Ok(Vni { api, context, _library: lib })
    }

    pub fn format(&self) -> u32 {
        FORMAT_VNI
    }

    /// Colours a new frame of raw shades and publishes it, when libvni gives
    /// one.
    pub fn colour(&mut self, dots: &[u8], frame: &IdentifyFrame, shared: &Shared) {
        if dots.len() < (frame.width * frame.height) as usize {
            return;
        }
        let bits = if frame.bits == 4 { 4 } else { 2 };
        // SAFETY: a frame of `width` × `height` shades
        if unsafe { (self.api.colorize)(self.context, dots.as_ptr(), frame.width, frame.height, bits) } == 0 {
            return;
        }
        // SAFETY: libvni's current frame, valid until the next call
        let Some(f) = (unsafe { (self.api.get_frame)(self.context).as_ref() }) else { return };
        if f.has_frame == 0 || f.frame.is_null() || f.palette.is_null() || f.bits > 8 {
            return;
        }
        let colours = 1usize << f.bits;
        // SAFETY: as its header describes them, `width` × `height` indices and `colours` triples
        let (indices, palette) = unsafe { (std::slice::from_raw_parts(f.frame, (f.width * f.height) as usize), std::slice::from_raw_parts(f.palette, colours * 3)) };
        let colour = |i: u8| {
            let p = &palette[(usize::from(i) % colours) * 3..][..3];
            (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2])
        };
        shared.publish_plane(0, f.width, f.height, DOTS_RGB888, indices.iter().map(|&i| colour(i)));
    }
}

impl Drop for Vni {
    fn drop(&mut self) {
        // SAFETY: the context loaded, released once
        unsafe { (self.api.dispose)(self.context) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_struct_is_laid_out_as_libvni_lays_it_out() {
        assert_eq!(std::mem::offset_of!(VniFrame, has_frame), 9);
        assert_eq!(std::mem::offset_of!(VniFrame, frame), 16);
        assert_eq!(std::mem::size_of::<VniFrame>(), 32);
    }
}
