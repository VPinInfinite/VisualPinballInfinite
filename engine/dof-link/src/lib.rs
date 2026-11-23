// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! The link between the player and `vpi-dof`, the program that drives a
//! cabinet's toys through libdof beside it: one block of shared
//! memory and a named event.
//!
//! The engine creates the link and starts `vpi-dof` with its name; the host
//! opens it. Engine to host: the game's events in a ring — each a letter
//! (VPX's event stream's: `L` lamps, `S` solenoids, `G` lighting strings,
//! `W` switches, `N` mechanisms, `D` displays, `E` the script's data, `B`
//! and `C` scores), a number and a value — the event set after each batch,
//! and a stop. Host to engine: its state.

mod win;

use std::io;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

/// Marks a block as a DOF link.
pub const MAGIC: u32 = u32::from_le_bytes(*b"VPID");
/// Changes whenever the layout below does; the host refuses another one.
pub const LAYOUT_VERSION: u32 = 1;
/// Events the ring holds: far more than one of the event stream's polls brings.
pub const RING: usize = 4096;

/// The host's state: starting (libdof reading its configuration).
pub const STATE_STARTING: u32 = 0;
/// libdof is running and takes the events.
pub const STATE_READY: u32 = 1;
/// libdof couldn't be loaded: the toys stay still.
pub const STATE_FAILED: u32 = 2;

#[repr(C)]
struct Slot {
    kind: AtomicU32,
    number: AtomicI32,
    value: AtomicI32,
}

/// The whole shared block.
#[repr(C)]
pub struct Shared {
    magic: AtomicU32,
    version: AtomicU32,
    state: AtomicU32,
    stop: AtomicU32,
    /// Events written so far by the engine, and read so far by the host.
    written: AtomicU32,
    read: AtomicU32,
    /// Events the engine dropped because the ring was full.
    dropped: AtomicU32,
    ring: [Slot; RING],
}

/// One event: its letter, number and value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: char,
    pub number: i32,
    pub value: i32,
}

impl Shared {
    fn init(&self) {
        self.version.store(LAYOUT_VERSION, Ordering::Relaxed);
        self.magic.store(MAGIC, Ordering::Release);
    }

    fn check(&self) -> Result<(), String> {
        if self.magic.load(Ordering::Acquire) != MAGIC {
            return Err("not a DOF link".into());
        }
        let v = self.version.load(Ordering::Relaxed);
        if v != LAYOUT_VERSION {
            return Err(format!("link layout {v}, this program speaks {LAYOUT_VERSION}"));
        }
        Ok(())
    }

    /// The engine adds an event; false when the ring is full and it was dropped.
    pub fn push(&self, e: Event) -> bool {
        let written = self.written.load(Ordering::Relaxed);
        if written.wrapping_sub(self.read.load(Ordering::Acquire)) as usize >= RING {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let slot = &self.ring[written as usize % RING];
        slot.kind.store(u32::from(e.kind), Ordering::Relaxed);
        slot.number.store(e.number, Ordering::Relaxed);
        slot.value.store(e.value, Ordering::Relaxed);
        self.written.store(written.wrapping_add(1), Ordering::Release);
        true
    }

    /// The host takes every event waiting, in order.
    pub fn drain(&self, mut each: impl FnMut(Event)) {
        let mut read = self.read.load(Ordering::Relaxed);
        let written = self.written.load(Ordering::Acquire);
        while read != written {
            let slot = &self.ring[read as usize % RING];
            let kind = char::from_u32(slot.kind.load(Ordering::Relaxed)).unwrap_or('?');
            each(Event { kind, number: slot.number.load(Ordering::Relaxed), value: slot.value.load(Ordering::Relaxed) });
            read = read.wrapping_add(1);
            self.read.store(read, Ordering::Release);
        }
    }

    /// Events dropped so far, the ring having been full.
    pub fn dropped(&self) -> u32 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn set_state(&self, state: u32) {
        self.state.store(state, Ordering::Release);
    }

    pub fn state(&self) -> u32 {
        self.state.load(Ordering::Acquire)
    }

    /// The engine asks the host to end (libdof finishing its toys first).
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

    /// The engine tells the host there are new events, or a stop.
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
    fn events_cross_the_link_in_order() {
        let name = format!("Local\\VPI-dof-test-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        let sent = [Event { kind: 'L', number: 12, value: 1 }, Event { kind: 'S', number: 5, value: 0 }, Event { kind: 'E', number: 101, value: 1 }];
        for e in sent {
            assert!(engine.shared().push(e));
        }
        let mut got = Vec::new();
        host.shared().drain(|e| got.push(e));
        assert_eq!(got, sent);
        // a full ring drops what doesn't fit, and says how many
        for n in 0..RING as i32 + 3 {
            engine.shared().push(Event { kind: 'W', number: n, value: 1 });
        }
        assert_eq!(engine.shared().dropped(), 3);
        let mut count = 0;
        host.shared().drain(|_| count += 1);
        assert_eq!(count, RING);
        engine.shared().request_stop();
        assert!(host.shared().stop_requested());
    }
}
