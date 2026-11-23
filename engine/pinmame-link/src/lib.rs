// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! The link between the player and `vpi-pinmame`, the program that runs
//! PinMAME beside it: one block of shared memory and two named
//! events, one each way, so neither side has to poll the other.
//!
//! The engine creates the link and starts `vpi-pinmame` with its name; the
//! host opens it. Engine to host: commands (switch changes, DIP switches,
//! pause, stop) in a ring, and the time fence. Host to engine: its state,
//! the current lamps, solenoids and lighting strings, a ring of their
//! changes in order, so the controller can hand a script every change
//! since it last asked, and each display's latest frame.
//!
//! BSD-3-Clause, not GPL: `vpi-pinmame` links PinMAME and uses this crate.

mod layout;
mod win;

use std::io;
use std::sync::OnceLock;

pub use layout::*;

/// Microseconds on the machine's high-resolution clock, which every program
/// on it shares, so the player and the host can time the same events
/// alike (`VPI_LATENCY_LOG`).
pub fn clock_us() -> u64 {
    static FREQUENCY: OnceLock<i64> = OnceLock::new();
    let frequency = *FREQUENCY.get_or_init(|| {
        let mut f = 0;
        // SAFETY: Windows writes the counter's frequency
        unsafe { win::QueryPerformanceFrequency(&mut f) };
        f.max(1)
    });
    let mut count = 0;
    // SAFETY: Windows writes the counter
    unsafe { win::QueryPerformanceCounter(&mut count) };
    (i128::from(count) * 1_000_000 / i128::from(frequency)) as u64
}

/// One end of a link.
pub struct Link {
    name: String,
    mapping: win::Handle,
    view: *mut Shared,
    to_host: win::Handle,
    to_engine: win::Handle,
}

// SAFETY: the block is shared memory made of atomics, and the handles are
// process-wide; nothing here is tied to the thread that made it
unsafe impl Send for Link {}
unsafe impl Sync for Link {}

impl Link {
    /// The engine's end: a new link of that name, for one game. A name in
    /// the `Local\` namespace keeps it to the current Windows session.
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
        let event = |suffix: &str| {
            let n = win::wide(&format!("{name}.{suffix}"));
            // SAFETY: auto-reset events, initially unset
            let h =
                unsafe { if create { win::CreateEventW(std::ptr::null(), 0, 0, n.as_ptr()) } else { win::OpenEventW(win::EVENT_MODIFY_STATE | win::SYNCHRONIZE, 0, n.as_ptr()) } };
            if h.is_null() { Err(io::Error::last_os_error()) } else { Ok(h) }
        };
        let mut link = Link { name: name.to_string(), mapping, view, to_host: std::ptr::null_mut(), to_engine: std::ptr::null_mut() };
        if view.is_null() {
            return Err(io::Error::last_os_error());
        }
        link.to_host = event("to-host")?;
        link.to_engine = event("to-engine")?;
        Ok(link)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn shared(&self) -> &Shared {
        // SAFETY: mapped for as long as the link lives; all of it is atomics
        unsafe { &*self.view }
    }

    /// The engine tells the host there's something for it.
    pub fn wake_host(&self) {
        // SAFETY: an event handle we own
        unsafe { win::SetEvent(self.to_host) };
    }

    /// The host tells the engine solenoids changed, or its state did.
    pub fn wake_engine(&self) {
        // SAFETY: an event handle we own
        unsafe { win::SetEvent(self.to_engine) };
    }

    /// The event the host waits on, for its own wait among several.
    pub fn to_host_event(&self) -> *mut std::ffi::c_void {
        self.to_host
    }

    /// The event the engine waits on, likewise: set when solenoids changed.
    pub fn to_engine_event(&self) -> *mut std::ffi::c_void {
        self.to_engine
    }

    /// The engine waits up to `milliseconds` for the host's next wake-up;
    /// true if it came.
    pub fn wait_for_host(&self, milliseconds: u32) -> bool {
        // SAFETY: an event handle we own
        unsafe { win::WaitForSingleObject(self.to_engine, milliseconds) == win::WAIT_OBJECT_0 }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        // SAFETY: each was made by this link and is released once
        unsafe {
            if !self.view.is_null() {
                win::UnmapViewOfFile(self.view as *const _);
            }
            for h in [self.to_host, self.to_engine, self.mapping] {
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
    fn commands_and_outputs_cross_the_link() {
        let name = format!("Local\\VPI-PinMAME-test-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        // engine to host, in order
        assert!(engine.shared().send(Command::Switch { number: 13, closed: true }));
        assert!(engine.shared().send(Command::Dip { bank: 0, value: 5 }));
        assert_eq!(host.shared().take_command(), Some(Command::Switch { number: 13, closed: true }));
        assert_eq!(host.shared().take_command(), Some(Command::Dip { bank: 0, value: 5 }));
        assert_eq!(host.shared().take_command(), None);
        engine.shared().set_time_fence(1.25);
        assert_eq!(host.shared().time_fence(), Some(1.25));
        // host to engine: current values and the changes in order
        let mut reader = OutputReader::from_now(engine.shared());
        host.shared().publish(Output { kind: OutputKind::Lamp, number: 11, value: 1 });
        host.shared().publish(Output { kind: OutputKind::Solenoid, number: 9, value: 1 });
        let mut out = Vec::new();
        assert_eq!(reader.read(engine.shared(), &mut out), Read::Complete);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1], Output { kind: OutputKind::Solenoid, number: 9, value: 1 });
        assert_eq!(engine.shared().current(OutputKind::Lamp, 11), 1);
        // a digit's lit segments, every digit PinMAME numbers kept
        host.shared().publish(Output { kind: OutputKind::Led, number: MAX_LEDS as i32 - 1, value: 0x3F });
        out.clear();
        assert_eq!(reader.read(engine.shared(), &mut out), Read::Complete);
        assert_eq!(out, [Output { kind: OutputKind::Led, number: MAX_LEDS as i32 - 1, value: 0x3F }]);
        assert_eq!(engine.shared().current(OutputKind::Led, MAX_LEDS as i32 - 1), 0x3F);
        // a reader that falls a full ring behind is told to resync
        for i in 0..(OUTPUT_SLOTS + 1) {
            host.shared().publish(Output { kind: OutputKind::Lamp, number: (i % 64) as i32, value: 1 });
        }
        out.clear();
        assert_eq!(reader.read(engine.shared(), &mut out), Read::Lost);
        assert!(out.is_empty());
        // and a second link of the same name is refused
        assert!(Link::create(&name).is_err());
    }

    #[test]
    fn identify_frames_cross_the_link() {
        let name = format!("vpi-link-test-identify-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        let mut frame = Vec::new();
        assert_eq!(engine.shared().read_identify(0, &mut frame), None);
        let dots: Vec<u8> = (0..128 * 32).map(|i| (i % 4) as u8).collect();
        assert!(host.shared().publish_identify(0, 2, &dots));
        assert_eq!(engine.shared().read_identify(0, &mut frame), Some((1, 2)));
        assert_eq!(frame, dots);
        assert_eq!(engine.shared().identify_published(0), 1);
        assert!(!host.shared().publish_identify(MAX_DISPLAYS, 2, &dots));
    }

    #[test]
    fn display_frames_cross_the_link() {
        let name = format!("Local\\VPI-PinMAME-display-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        let mut frame = Vec::new();
        assert_eq!(engine.shared().display(0), None);
        let dmd = DisplayLayout { kind: DISPLAY_DMD, width: 128, height: 32, depth: 2, ..DisplayLayout::default() };
        assert!(host.shared().set_display(0, 1, dmd));
        assert_eq!(engine.shared().display_count(), 1);
        assert_eq!(engine.shared().display(0), Some(dmd));
        assert!(engine.shared().display(0).unwrap().is_dmd());
        assert_eq!(engine.shared().read_frame(0, &mut frame), None);
        // a frame whose size isn't a whole number of words
        let dots: Vec<u8> = (0..4095u32).map(|i| (i % 251) as u8).collect();
        assert!(host.shared().publish_frame(0, &dots));
        assert!(host.shared().publish_frame(0, &dots));
        assert_eq!(engine.shared().read_frame(0, &mut frame), Some(2));
        assert_eq!(frame, dots);
        assert_eq!(engine.shared().frames_published(0), 2);
        // too big, or beyond the displays carried
        assert!(!host.shared().publish_frame(0, &vec![0; DISPLAY_BYTES + 1]));
        assert!(!host.shared().set_display(MAX_DISPLAYS, MAX_DISPLAYS + 1, dmd));
    }

    #[test]
    fn mechanisms_cross_the_link() {
        let name = format!("Local\\VPI-PinMAME-mech-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        let mut config = MechConfig { kind: 0x105, sol1: 22, sol2: 39, length: 148, steps: 148, ..MechConfig::default() };
        config.switches[0] = MechSwitch { number: 44, start: 0, end: 2, pulse: 0 };
        config.switches[19] = MechSwitch { number: 45, start: 140, end: 148, pulse: 3 };
        assert!(engine.shared().set_mech_config(2, &config));
        assert!(!engine.shared().set_mech_config(SCRIPT_MECHS as i32 + 1, &config));
        assert!(engine.shared().send(Command::SetMech { number: 2 }));
        assert_eq!(host.shared().take_command(), Some(Command::SetMech { number: 2 }));
        assert_eq!(host.shared().mech_config(2), Some(config));
        assert_eq!(host.shared().mech_config(0), None);
        host.shared().set_mech(2, 74);
        host.shared().set_mech(-2, -3);
        assert_eq!((engine.shared().mech(2), engine.shared().mech(-2), engine.shared().mech(11)), (74, -3, 0));
    }

    #[test]
    fn sound_crosses_the_link() {
        let name = format!("Local\\VPI-PinMAME-audio-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        let host = Link::open(&name).unwrap();
        assert_eq!(engine.shared().audio_rate(), 0);
        host.shared().set_audio_rate(48_000);
        host.shared().publish_audio([[1, -1], [i16::MAX, i16::MIN], [-300, 300]].into_iter());
        assert_eq!((engine.shared().audio_rate(), engine.shared().audio_written()), (48_000, 3));
        assert_eq!(engine.shared().audio_frame(1), [i16::MAX, i16::MIN]);
        assert_eq!(engine.shared().audio_frame(2), [-300, 300]);
        // the ring wraps: frame AUDIO_FRAMES + 2 takes frame 2's place
        host.shared().publish_audio(std::iter::repeat_n([7, 8], AUDIO_FRAMES));
        assert_eq!(engine.shared().audio_written(), AUDIO_FRAMES as u64 + 3);
        assert_eq!(engine.shared().audio_frame(AUDIO_FRAMES as u64 + 2), [7, 8]);
    }

    #[test]
    fn a_full_command_ring_refuses_more() {
        let name = format!("Local\\VPI-PinMAME-full-{}", std::process::id());
        let engine = Link::create(&name).unwrap();
        for _ in 0..COMMAND_SLOTS {
            assert!(engine.shared().send(Command::Pause(true)));
        }
        assert!(!engine.shared().send(Command::Pause(false)));
        assert_eq!(engine.shared().take_command(), Some(Command::Pause(true)));
        assert!(engine.shared().send(Command::Pause(false)));
    }
}
