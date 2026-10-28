// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! The link between the player and `vpi-colour`, the program that colours a
//! ROM game's score display beside it with the game's Serum or PIN2DMD file
//!: one block of shared memory and a named event.
//!
//! The engine creates the link and starts `vpi-colour` with its name; the
//! host opens it. Engine to host: the game's newest frame as its raw
//! shades (PinMAME's identify frame), the event set each time, and a stop.
//! Host to engine: its state, the newest coloured frame in up to two planes
//! (the game's size, and a 64-dot-high one some files add), and the
//! triggers the file gives frames, for PinUP packs.
//!
//! BSD-3-Clause, not GPL-3.0: `vpi-colour` runs libserum and libvni, under
//! version 2 of the GPL alone, and uses this crate.

mod layout;
mod win;

use std::io;

pub use layout::*;

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

    /// The engine tells the host there's a new frame, or a stop.
    pub fn wake_host(&self) {
        // SAFETY: an event handle we own
        unsafe { win::SetEvent(self.to_host) };
    }

    /// The event the host waits on, for its own wait among several.
    pub fn to_host_event(&self) -> *mut std::ffi::c_void {
        self.to_host
    }

    /// The host waits up to `milliseconds` for the engine's next wake-up;
    /// true if it came.
    pub fn wait_for_engine(&self, milliseconds: u32) -> bool {
        // SAFETY: an event handle we own
        unsafe { win::WaitForSingleObject(self.to_host, milliseconds) == win::WAIT_OBJECT_0 }
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
    fn frames_cross_the_link_both_ways() {
        let name = format!("Local\\VPI-colour-test-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        let mut dots = Vec::new();
        assert_eq!(host.shared().read_identify(&mut dots), None);
        // a frame whose size isn't a whole number of words
        let shades: Vec<u8> = (0..127 * 32).map(|i| (i % 4) as u8).collect();
        assert!(engine.shared().send_identify(77, 127, 32, 2, &shades));
        engine.wake_host();
        assert!(host.wait_for_engine(0));
        assert_eq!(host.shared().read_identify(&mut dots), Some(IdentifyFrame { seq: 1, number: 77, width: 127, height: 32, bits: 2 }));
        assert_eq!(dots, shades);
        assert!(!engine.shared().send_identify(1, 257, 32, 2, &shades));
        // a coloured frame back, in the second plane
        let mut rgb = Vec::new();
        assert_eq!(engine.shared().read_plane(1, &mut rgb), None);
        assert!(host.shared().publish_plane(1, 256, 64, DOTS_RGB565, (0..256 * 64).map(|i| i as u32)));
        assert_eq!(engine.shared().plane_seq(1), 1);
        assert_eq!(engine.shared().read_plane(1, &mut rgb), Some(PlaneFrame { seq: 1, width: 256, height: 64, kind: DOTS_RGB565 }));
        assert_eq!((rgb.len(), rgb[300]), (256 * 64, 300));
        assert_eq!(engine.shared().plane_seq(0), 0);
        // the host's state and the engine's stop
        host.shared().set_state(STATE_READY, FORMAT_SERUM_V2);
        assert_eq!(engine.shared().state(), (STATE_READY, FORMAT_SERUM_V2));
        assert!(!host.shared().stop_requested());
        engine.shared().request_stop();
        assert!(host.shared().stop_requested());
        // a second link of the same name is refused
        assert!(Link::create(&name).is_err());
    }

    #[test]
    fn triggers_are_read_once_and_a_reader_far_behind_gets_the_newest() {
        let name = format!("Local\\VPI-colour-triggers-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        let (mut cursor, mut out) = (0, Vec::new());
        engine.shared().triggers_since(&mut cursor, &mut out);
        assert!(out.is_empty());
        host.shared().publish_trigger(12);
        host.shared().publish_trigger(40);
        engine.shared().triggers_since(&mut cursor, &mut out);
        assert_eq!(out, [12, 40]);
        out.clear();
        engine.shared().triggers_since(&mut cursor, &mut out);
        assert!(out.is_empty());
        for id in 0..(TRIGGER_SLOTS as u32 + 5) {
            host.shared().publish_trigger(id);
        }
        engine.shared().triggers_since(&mut cursor, &mut out);
        assert_eq!(out.len(), TRIGGER_SLOTS);
        assert_eq!((out[0], *out.last().unwrap()), (5, TRIGGER_SLOTS as u32 + 4));
    }
}
