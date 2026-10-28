// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! The block of shared memory both programs see. Every field is an atomic,
//! so either side can read and write it from any thread; Windows zeroes a
//! new block, and zero is a valid starting value for every field.

use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, AtomicU8, Ordering};

/// The block's first four bytes: "VPIM".
pub const MAGIC: u32 = u32::from_le_bytes(*b"VPIM");
/// Changes whenever the layout below does; the host refuses another one.
pub const LAYOUT_VERSION: u32 = 8;

/// Room for the outputs PinMAME numbers. Numbers beyond these still travel
/// in the ring of changes; only their current values aren't kept.
pub const MAX_LAMPS: usize = 1024;
pub const MAX_SOLENOIDS: usize = 1024;
pub const MAX_GIS: usize = 64;
/// The digits of segment displays PinMAME numbers for `ChangedLEDs`
/// (`CORE_SEGCOUNT`): every one is kept.
pub const MAX_LEDS: usize = 128;
/// PinMAME's DIP switch banks (`VP_MAXDIPBANKS`), eight switches a bank.
pub const DIP_BANKS: usize = 10;
/// The most NVRAM bytes PinMAME hands out (`CORE_MAXNVRAM`).
pub const MAX_NVRAM: usize = 131_118;
/// Commands waiting for the host (a switch change is one).
pub const COMMAND_SLOTS: usize = 1024;
/// Changes kept for the engine to read: several seconds of the busiest
/// lamp shows, read every few milliseconds.
pub const OUTPUT_SLOTS: usize = 8192;
/// Displays a game can have: a DMD, or rows of segment displays, plus the
/// DMD PinMAME draws from segments for games without one.
pub const MAX_DISPLAYS: usize = 16;
/// Room for one display's frame: a 256×64 DMD at a byte a dot (the largest
/// DMD is 192×64), or thousands of segment characters at two bytes each.
/// The few 1980s machines with a video screen don't fit, and don't travel.
pub const DISPLAY_BYTES: usize = 256 * 64;
/// Mechanisms a table's script can describe (`PinmameSetMech` takes 1 to 5).
pub const SCRIPT_MECHS: usize = 5;
/// Mechanisms PinMAME reports: `PinmameGetMech(n)` gives mechanism n's
/// position, and its speed under -n, for n up to 10.
pub const MECH_NUMBERS: i32 = 10;
/// Switches one mechanism drives (`PINMAME_MAX_MECHSW`).
pub const MECH_SWITCHES: usize = 20;
/// Sound frames the ring holds: about two thirds of a second at 48 kHz,
/// far more than the engine lets build up.
pub const AUDIO_FRAMES: usize = 32_768;

/// A mechanism a table's script describes: PinMAME then moves it as its
/// solenoids drive it, and works its switches (`PinmameMechConfig`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MechConfig {
    pub kind: i32,
    pub sol1: i32,
    pub sol2: i32,
    pub length: i32,
    pub steps: i32,
    pub initial_pos: i32,
    pub acc: i32,
    pub ret: i32,
    pub switches: [MechSwitch; MECH_SWITCHES],
}

/// A switch a mechanism closes between two positions, or pulses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MechSwitch {
    pub number: i32,
    pub start: i32,
    pub end: i32,
    pub pulse: i32,
}

/// `MechConfig` as whole numbers, in `PinmameMechConfig`'s order.
const MECH_INTS: usize = 8 + MECH_SWITCHES * 4;

impl MechConfig {
    fn to_ints(self) -> [i32; MECH_INTS] {
        let mut out = [0; MECH_INTS];
        out[..8].copy_from_slice(&[self.kind, self.sol1, self.sol2, self.length, self.steps, self.initial_pos, self.acc, self.ret]);
        for (i, s) in self.switches.iter().enumerate() {
            out[8 + i * 4..12 + i * 4].copy_from_slice(&[s.number, s.start, s.end, s.pulse]);
        }
        out
    }

    fn from_ints(v: &[i32; MECH_INTS]) -> MechConfig {
        let mut c = MechConfig { kind: v[0], sol1: v[1], sol2: v[2], length: v[3], steps: v[4], initial_pos: v[5], acc: v[6], ret: v[7], ..Default::default() };
        for (i, s) in c.switches.iter_mut().enumerate() {
            *s = MechSwitch { number: v[8 + i * 4], start: v[9 + i * 4], end: v[10 + i * 4], pulse: v[11 + i * 4] };
        }
        c
    }
}

/// What the host is doing, in [`Shared::host_state`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum HostState {
    /// Started, PinMAME not running yet.
    Starting = 0,
    /// PinMAME is running the game.
    Running = 1,
    /// The game stopped, as asked or because PinMAME ended it.
    Stopped = 2,
    /// PinMAME couldn't run the game: see [`Shared::run_status`].
    Failed = 3,
}

impl HostState {
    fn from_u32(v: u32) -> HostState {
        match v {
            1 => HostState::Running,
            2 => HostState::Stopped,
            3 => HostState::Failed,
            _ => HostState::Starting,
        }
    }
}

/// What the engine asks of the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// A switch closes or opens (PinMAME's switch number).
    Switch { number: i32, closed: bool },
    /// A bank of DIP switches.
    Dip { bank: i32, value: i32 },
    /// Pause or resume the emulation.
    Pause(bool),
    /// `PinmameSetSolenoidMask`: `which` 0 masks solenoids 1–32, 1 masks
    /// 33–64, and 2 sets how outputs report (0 on or off, 1 modulated
    /// solenoids, 2 physical levels for everything).
    SolenoidMask { which: i32, mask: u32 },
    /// How one output reports its level: `output` and `kind` are PinMAME's
    /// output and output-type numbers.
    ModOutputType { output: i32, number: i32, kind: i32 },
    /// Reset the emulated machine.
    Reset,
    /// Stop the game and end the host.
    Stop,
    /// Hand PinMAME mechanism `number` (1 to [`SCRIPT_MECHS`]), as the
    /// engine described it with [`Shared::set_mech_config`].
    SetMech { number: i32 },
}

impl Command {
    fn encode(self) -> (u32, i32, i32, i32) {
        match self {
            Command::Switch { number, closed } => (1, number, closed as i32, 0),
            Command::Dip { bank, value } => (2, bank, value, 0),
            Command::Pause(on) => (3, on as i32, 0, 0),
            Command::SolenoidMask { which, mask } => (4, which, mask as i32, 0),
            Command::ModOutputType { output, number, kind } => (5, output, number, kind),
            Command::Reset => (6, 0, 0, 0),
            Command::Stop => (7, 0, 0, 0),
            Command::SetMech { number } => (8, number, 0, 0),
        }
    }

    fn decode(kind: u32, a: i32, b: i32, c: i32) -> Option<Command> {
        Some(match kind {
            1 => Command::Switch { number: a, closed: b != 0 },
            2 => Command::Dip { bank: a, value: b },
            3 => Command::Pause(a != 0),
            4 => Command::SolenoidMask { which: a, mask: b as u32 },
            5 => Command::ModOutputType { output: a, number: b, kind: c },
            6 => Command::Reset,
            7 => Command::Stop,
            8 => Command::SetMech { number: a },
            _ => return None,
        })
    }
}

/// The kinds of output PinMAME reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum OutputKind {
    Lamp = 1,
    Solenoid = 2,
    /// A general illumination (lighting) string.
    Gi = 3,
    /// A digit of a segment display: its lit segments, a bit each, as
    /// PinMAME draws them (`ChangedLEDs`).
    Led = 4,
    /// A command the game sent its sound board (`NewSoundCommands`): the
    /// command in `number`; nothing is kept as a current value.
    SoundCommand = 5,
}

impl OutputKind {
    fn from_u32(v: u32) -> Option<OutputKind> {
        match v {
            1 => Some(OutputKind::Lamp),
            2 => Some(OutputKind::Solenoid),
            3 => Some(OutputKind::Gi),
            4 => Some(OutputKind::Led),
            5 => Some(OutputKind::SoundCommand),
            _ => None,
        }
    }
}

/// One output's change: its new value (on or off, or a level, as PinMAME
/// reports it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub kind: OutputKind,
    pub number: i32,
    pub value: i32,
}

/// One display, as PinMAME describes it (`PinmameDisplayLayout`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DisplayLayout {
    /// PinMAME's display type: [`DISPLAY_DMD`], [`DISPLAY_VIDEO`], or a
    /// kind of segment display, with flags above [`DISPLAY_TYPE_MASK`].
    pub kind: u32,
    pub top: i32,
    pub left: i32,
    /// Characters, for a segment display.
    pub length: i32,
    /// Dots, for a DMD or a video screen.
    pub width: i32,
    pub height: i32,
    /// Bits per dot PinMAME works with (a DMD's shades), or the video
    /// screen's bits per pixel.
    pub depth: i32,
}

/// `PINMAME_DISPLAY_TYPE_SEGMASK`: the type without its flags.
pub const DISPLAY_TYPE_MASK: u32 = 0x3f;
/// `PINMAME_DISPLAY_TYPE_DMD`: a frame is a byte a dot, its brightness
/// from 0 to 255.
pub const DISPLAY_DMD: u32 = 14;
/// `PINMAME_DISPLAY_TYPE_VIDEO`.
pub const DISPLAY_VIDEO: u32 = 15;

impl DisplayLayout {
    pub fn is_dmd(&self) -> bool {
        self.kind & DISPLAY_TYPE_MASK == DISPLAY_DMD
    }

    pub fn is_video(&self) -> bool {
        self.kind & DISPLAY_TYPE_MASK == DISPLAY_VIDEO
    }

    /// A segment display: a frame is two bytes a character, one bit a
    /// segment.
    pub fn is_segments(&self) -> bool {
        !self.is_dmd() && !self.is_video()
    }
}

#[repr(C)]
struct DisplaySlot {
    /// Set once the layout below is filled in.
    ready: AtomicU32,
    /// Twice the frames written so far; odd while the host writes one.
    seq: AtomicU32,
    kind: AtomicU32,
    top: AtomicI32,
    left: AtomicI32,
    length: AtomicI32,
    width: AtomicI32,
    height: AtomicI32,
    depth: AtomicI32,
    /// The last frame's size in bytes.
    size: AtomicU32,
    _pad: [AtomicU32; 2],
    data: [AtomicU32; DISPLAY_BYTES / 4],
}

/// A DMD's identify frame: PinMAME's raw shades, a byte a dot, as PinUP's frame triggers and
/// colour files match them (our PinMAME build's).
#[repr(C)]
struct IdentifySlot {
    /// Twice the frames written so far; odd while the host writes one.
    seq: AtomicU32,
    /// Bits a dot: 2 or 4.
    bits: AtomicU32,
    /// The frame's size in bytes.
    size: AtomicU32,
    _pad: AtomicU32,
    data: [AtomicU32; DISPLAY_BYTES / 4],
}

#[repr(C)]
struct CommandSlot {
    kind: AtomicU32,
    a: AtomicI32,
    b: AtomicI32,
    c: AtomicI32,
}

#[repr(C)]
struct OutputSlot {
    kind: AtomicU32,
    number: AtomicI32,
    value: AtomicI32,
    _pad: AtomicU32,
}

/// The whole block.
#[repr(C)]
pub struct Shared {
    magic: AtomicU32,
    version: AtomicU32,
    host_state: AtomicU32,
    /// `PinmameRun`'s answer when it couldn't run the game (0 when fine).
    run_status: AtomicI32,
    /// PinMAME's hardware generation of the running game.
    hardware_gen: AtomicU64,
    /// How many lamps, solenoids and lighting strings the game has.
    max_lamps: AtomicU32,
    max_solenoids: AtomicU32,
    max_gis: AtomicU32,
    _pad: AtomicU32,
    /// The running game's DIP switches, a bank each, as PinMAME has them.
    dips: [AtomicI32; DIP_BANKS],
    /// The running game's NVRAM, read by the host when the engine asks (`Controller.NVRAM`): the
    /// engine bumps the request, the host fills the bytes and their count (-1: none) and answers it.
    nvram_request: AtomicU32,
    nvram_answer: AtomicU32,
    nvram_len: AtomicI32,
    _nvram_pad: AtomicU32,
    nvram: [AtomicU8; MAX_NVRAM],
    /// The time fence, in seconds as f64 bits; 0 for none.
    time_fence: AtomicU64,
    // engine to host: a single-producer, single-consumer ring
    command_write: AtomicU64,
    command_read: AtomicU64,
    commands: [CommandSlot; COMMAND_SLOTS],
    // host to engine: current values, and every change in order
    lamps: [AtomicI32; MAX_LAMPS],
    solenoids: [AtomicI32; MAX_SOLENOIDS],
    gis: [AtomicI32; MAX_GIS],
    leds: [AtomicI32; MAX_LEDS],
    output_write: AtomicU64,
    outputs: [OutputSlot; OUTPUT_SLOTS],
    // host to engine: the displays and their latest frames
    display_count: AtomicU32,
    _pad2: AtomicU32,
    displays: [DisplaySlot; MAX_DISPLAYS],
    // host to engine: each DMD's latest identify frame
    identify: [IdentifySlot; MAX_DISPLAYS],
    // mechanisms: the scripts' descriptions, and what PinMAME reports
    mech_configs: [[AtomicI32; MECH_INTS]; SCRIPT_MECHS],
    mech_values: [AtomicI32; 2 * MECH_NUMBERS as usize + 1],
    // host to engine: the sound boards' output, a stereo frame (two 16-bit
    // samples) a slot, and how many frames were written in all
    audio_rate: AtomicU32,
    _pad3: AtomicU32,
    audio_write: AtomicU64,
    audio: [AtomicU32; AUDIO_FRAMES],
}

impl Shared {
    /// The engine marks a new block as a link of this layout.
    pub(crate) fn init(&self) {
        self.version.store(LAYOUT_VERSION, Ordering::Relaxed);
        self.magic.store(MAGIC, Ordering::Release);
    }

    /// Whether the block is a link of this layout.
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.magic.load(Ordering::Acquire) != MAGIC {
            return Err("not a PinMAME link".into());
        }
        let v = self.version.load(Ordering::Relaxed);
        if v != LAYOUT_VERSION {
            return Err(format!("link layout {v}, this program speaks {LAYOUT_VERSION}"));
        }
        Ok(())
    }

    // ---------------------------------------------------------------- state

    pub fn host_state(&self) -> HostState {
        HostState::from_u32(self.host_state.load(Ordering::Acquire))
    }

    pub fn set_host_state(&self, state: HostState) {
        self.host_state.store(state as u32, Ordering::Release);
    }

    pub fn run_status(&self) -> i32 {
        self.run_status.load(Ordering::Acquire)
    }

    pub fn set_run_status(&self, status: i32) {
        self.run_status.store(status, Ordering::Release);
    }

    pub fn hardware_gen(&self) -> u64 {
        self.hardware_gen.load(Ordering::Acquire)
    }

    /// The running game's hardware and output counts, set by the host once
    /// PinMAME runs it.
    pub fn set_game_info(&self, hardware_gen: u64, lamps: u32, solenoids: u32, gis: u32) {
        self.max_lamps.store(lamps, Ordering::Relaxed);
        self.max_solenoids.store(solenoids, Ordering::Relaxed);
        self.max_gis.store(gis, Ordering::Relaxed);
        self.hardware_gen.store(hardware_gen, Ordering::Release);
    }

    /// The host publishes a bank of the running game's DIP switches.
    pub fn set_dip_bank(&self, bank: usize, value: i32) {
        if let Some(d) = self.dips.get(bank) {
            d.store(value, Ordering::Release);
        }
    }

    /// A bank of the running game's DIP switches, as last published (0 outside PinMAME's banks).
    pub fn dip_bank(&self, bank: i32) -> i32 {
        usize::try_from(bank).ok().and_then(|b| self.dips.get(b)).map_or(0, |d| d.load(Ordering::Acquire))
    }

    /// The engine asks the host for the game's NVRAM; the request's number, to wait for.
    pub fn request_nvram(&self) -> u32 {
        self.nvram_request.fetch_add(1, Ordering::AcqRel).wrapping_add(1)
    }

    /// The host's side: an NVRAM request waiting, if one is.
    pub fn nvram_requested(&self) -> Option<u32> {
        let request = self.nvram_request.load(Ordering::Acquire);
        (request != self.nvram_answer.load(Ordering::Acquire)).then_some(request)
    }

    /// The host answers request `request` with the game's NVRAM (None: the game has none to give).
    pub fn answer_nvram(&self, request: u32, bytes: Option<&[u8]>) {
        match bytes {
            Some(b) => {
                let n = b.len().min(MAX_NVRAM);
                for (slot, &v) in self.nvram.iter().zip(&b[..n]) {
                    slot.store(v, Ordering::Relaxed);
                }
                self.nvram_len.store(n as i32, Ordering::Release);
            }
            None => self.nvram_len.store(-1, Ordering::Release),
        }
        self.nvram_answer.store(request, Ordering::Release);
    }

    /// The engine's side: the NVRAM once request `request` is answered (Some(None): none to give),
    /// None while it isn't yet.
    pub fn nvram_answer(&self, request: u32) -> Option<Option<Vec<u8>>> {
        if self.nvram_answer.load(Ordering::Acquire) != request {
            return None;
        }
        let n = self.nvram_len.load(Ordering::Acquire);
        Some((n >= 0).then(|| self.nvram[..n as usize].iter().map(|v| v.load(Ordering::Relaxed)).collect()))
    }

    /// The same, answered by request `request` or one made after it: two asking at once (a
    /// table's script, and the player watching the high scores) each take the newer answer, which
    /// holds the same memory or newer.
    pub fn nvram_answer_since(&self, request: u32) -> Option<Option<Vec<u8>>> {
        let answered = self.nvram_answer.load(Ordering::Acquire);
        if answered.wrapping_sub(request) >= 0x8000_0000 {
            return None;
        }
        let n = self.nvram_len.load(Ordering::Acquire);
        Some((n >= 0).then(|| self.nvram[..(n as usize).min(self.nvram.len())].iter().map(|v| v.load(Ordering::Relaxed)).collect()))
    }

    /// The running game's lamp, solenoid and lighting string counts.
    pub fn output_counts(&self) -> (u32, u32, u32) {
        (self.max_lamps.load(Ordering::Acquire), self.max_solenoids.load(Ordering::Acquire), self.max_gis.load(Ordering::Acquire))
    }

    // ----------------------------------------------------------- time fence

    /// The engine lets PinMAME's emulation run up to `seconds` of its own
    /// time, and no further.
    pub fn set_time_fence(&self, seconds: f64) {
        self.time_fence.store(seconds.to_bits(), Ordering::Release);
    }

    /// The fence the engine set, if any.
    pub fn time_fence(&self) -> Option<f64> {
        let bits = self.time_fence.load(Ordering::Acquire);
        (bits != 0).then(|| f64::from_bits(bits))
    }

    // ------------------------------------------------------------- commands

    /// The engine sends a command; false when the host hasn't taken the
    /// last [`COMMAND_SLOTS`] yet (the caller waits and sends again).
    pub fn send(&self, command: Command) -> bool {
        let w = self.command_write.load(Ordering::Relaxed);
        let r = self.command_read.load(Ordering::Acquire);
        if w.wrapping_sub(r) >= COMMAND_SLOTS as u64 {
            return false;
        }
        let slot = &self.commands[(w % COMMAND_SLOTS as u64) as usize];
        let (kind, a, b, c) = command.encode();
        slot.kind.store(kind, Ordering::Relaxed);
        slot.a.store(a, Ordering::Relaxed);
        slot.b.store(b, Ordering::Relaxed);
        slot.c.store(c, Ordering::Relaxed);
        self.command_write.store(w.wrapping_add(1), Ordering::Release);
        true
    }

    /// The host takes the next command, if any.
    pub fn take_command(&self) -> Option<Command> {
        loop {
            let r = self.command_read.load(Ordering::Relaxed);
            let w = self.command_write.load(Ordering::Acquire);
            if r == w {
                return None;
            }
            let slot = &self.commands[(r % COMMAND_SLOTS as u64) as usize];
            let command = Command::decode(slot.kind.load(Ordering::Relaxed), slot.a.load(Ordering::Relaxed), slot.b.load(Ordering::Relaxed), slot.c.load(Ordering::Relaxed));
            self.command_read.store(r.wrapping_add(1), Ordering::Release);
            // an unknown command (a newer engine) is skipped
            if command.is_some() {
                return command;
            }
        }
    }

    // -------------------------------------------------------------- outputs

    /// The host publishes a change: the current value, then the ring entry.
    pub fn publish(&self, output: Output) {
        let current = match output.kind {
            OutputKind::Lamp => self.lamps.get(output.number as usize),
            OutputKind::Solenoid => self.solenoids.get(output.number as usize),
            OutputKind::Gi => self.gis.get(output.number as usize),
            OutputKind::Led => self.leds.get(output.number as usize),
            OutputKind::SoundCommand => None,
        };
        if let Some(c) = current.filter(|_| output.number >= 0) {
            c.store(output.value, Ordering::Relaxed);
        }
        let w = self.output_write.load(Ordering::Relaxed);
        let slot = &self.outputs[(w % OUTPUT_SLOTS as u64) as usize];
        slot.kind.store(output.kind as u32, Ordering::Relaxed);
        slot.number.store(output.number, Ordering::Relaxed);
        slot.value.store(output.value, Ordering::Relaxed);
        self.output_write.store(w.wrapping_add(1), Ordering::Release);
    }

    /// An output's current value (0 for numbers beyond what's kept).
    pub fn current(&self, kind: OutputKind, number: i32) -> i32 {
        let slot = match kind {
            OutputKind::Lamp => self.lamps.get(number as usize),
            OutputKind::Solenoid => self.solenoids.get(number as usize),
            OutputKind::Gi => self.gis.get(number as usize),
            OutputKind::Led => self.leds.get(number as usize),
            OutputKind::SoundCommand => None,
        };
        slot.filter(|_| number >= 0).map_or(0, |c| c.load(Ordering::Relaxed))
    }

    /// How many changes the host has published so far.
    pub fn outputs_written(&self) -> u64 {
        self.output_write.load(Ordering::Acquire)
    }

    // ------------------------------------------------------------- displays

    /// The host describes display `index` of `count`, before its first
    /// frame; false for an index beyond [`MAX_DISPLAYS`].
    pub fn set_display(&self, index: usize, count: usize, layout: DisplayLayout) -> bool {
        let Some(slot) = self.displays.get(index) else {
            return false;
        };
        slot.kind.store(layout.kind, Ordering::Relaxed);
        slot.top.store(layout.top, Ordering::Relaxed);
        slot.left.store(layout.left, Ordering::Relaxed);
        slot.length.store(layout.length, Ordering::Relaxed);
        slot.width.store(layout.width, Ordering::Relaxed);
        slot.height.store(layout.height, Ordering::Relaxed);
        slot.depth.store(layout.depth, Ordering::Relaxed);
        slot.ready.store(1, Ordering::Release);
        self.display_count.store(count.min(MAX_DISPLAYS) as u32, Ordering::Release);
        true
    }

    /// How many displays the game has (some may not be described yet).
    pub fn display_count(&self) -> usize {
        self.display_count.load(Ordering::Acquire) as usize
    }

    /// Display `index`, once the host has described it.
    pub fn display(&self, index: usize) -> Option<DisplayLayout> {
        let slot = self.displays.get(index)?;
        if slot.ready.load(Ordering::Acquire) == 0 {
            return None;
        }
        Some(DisplayLayout {
            kind: slot.kind.load(Ordering::Relaxed),
            top: slot.top.load(Ordering::Relaxed),
            left: slot.left.load(Ordering::Relaxed),
            length: slot.length.load(Ordering::Relaxed),
            width: slot.width.load(Ordering::Relaxed),
            height: slot.height.load(Ordering::Relaxed),
            depth: slot.depth.load(Ordering::Relaxed),
        })
    }

    /// The host writes display `index`'s new frame; false when it doesn't
    /// fit (a video screen) or the display is unknown.
    pub fn publish_frame(&self, index: usize, frame: &[u8]) -> bool {
        let Some(slot) = self.displays.get(index) else {
            return false;
        };
        if frame.len() > DISPLAY_BYTES {
            return false;
        }
        // a sequence lock: odd while writing, so a reader can tell it
        // caught a frame half written, and read again
        let seq = slot.seq.load(Ordering::Relaxed);
        slot.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        std::sync::atomic::fence(Ordering::Release);
        for (word, bytes) in slot.data.iter().zip(frame.chunks(4)) {
            let mut w = [0u8; 4];
            w[..bytes.len()].copy_from_slice(bytes);
            word.store(u32::from_le_bytes(w), Ordering::Relaxed);
        }
        slot.size.store(frame.len() as u32, Ordering::Relaxed);
        slot.seq.store(seq.wrapping_add(2), Ordering::Release);
        true
    }

    /// The host writes DMD `index`'s new identify frame, `bits` a dot; false when it doesn't
    /// fit or the display is unknown.
    pub fn publish_identify(&self, index: usize, bits: u32, frame: &[u8]) -> bool {
        let Some(slot) = self.identify.get(index) else {
            return false;
        };
        if frame.len() > DISPLAY_BYTES {
            return false;
        }
        let seq = slot.seq.load(Ordering::Relaxed);
        slot.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        std::sync::atomic::fence(Ordering::Release);
        for (word, bytes) in slot.data.iter().zip(frame.chunks(4)) {
            let mut w = [0u8; 4];
            w[..bytes.len()].copy_from_slice(bytes);
            word.store(u32::from_le_bytes(w), Ordering::Relaxed);
        }
        slot.bits.store(bits, Ordering::Relaxed);
        slot.size.store(frame.len() as u32, Ordering::Relaxed);
        slot.seq.store(seq.wrapping_add(2), Ordering::Release);
        true
    }

    /// How many identify frames DMD `index` has had: a cheap check for a new one.
    pub fn identify_published(&self, index: usize) -> u32 {
        self.identify.get(index).map_or(0, |slot| slot.seq.load(Ordering::Acquire) / 2)
    }

    /// Copies DMD `index`'s latest whole identify frame into `out`; its frame number and bits a
    /// dot, or None before the first (or with a PinMAME build that doesn't give them).
    pub fn read_identify(&self, index: usize, out: &mut Vec<u8>) -> Option<(u32, u32)> {
        let slot = self.identify.get(index)?;
        loop {
            let before = slot.seq.load(Ordering::Acquire);
            if before == 0 {
                return None;
            }
            if before & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let size = (slot.size.load(Ordering::Relaxed) as usize).min(DISPLAY_BYTES);
            let bits = slot.bits.load(Ordering::Relaxed);
            out.clear();
            for word in slot.data.iter().take(size.div_ceil(4)) {
                out.extend_from_slice(&word.load(Ordering::Relaxed).to_le_bytes());
            }
            out.truncate(size);
            std::sync::atomic::fence(Ordering::Acquire);
            if slot.seq.load(Ordering::Relaxed) == before {
                return Some((before / 2, bits));
            }
        }
    }

    // ---------------------------------------------------------------- sound

    /// The host says at what rate its sound comes, in frames a second (0
    /// for none).
    pub fn set_audio_rate(&self, rate: u32) {
        self.audio_rate.store(rate, Ordering::Release);
    }

    pub fn audio_rate(&self) -> u32 {
        self.audio_rate.load(Ordering::Acquire)
    }

    /// The host adds stereo frames to the ring, oldest first.
    pub fn publish_audio(&self, frames: impl Iterator<Item = [i16; 2]>) {
        let mut at = self.audio_write.load(Ordering::Relaxed);
        for [left, right] in frames {
            self.audio[(at % AUDIO_FRAMES as u64) as usize].store(u32::from(left as u16) | (u32::from(right as u16) << 16), Ordering::Relaxed);
            at += 1;
        }
        self.audio_write.store(at, Ordering::Release);
    }

    /// How many frames the host has added in all.
    pub fn audio_written(&self) -> u64 {
        self.audio_write.load(Ordering::Acquire)
    }

    /// Frame number `index`, counted from the first ever added; the engine
    /// reads only frames less than [`AUDIO_FRAMES`] behind the newest.
    pub fn audio_frame(&self, index: u64) -> [i16; 2] {
        let v = self.audio[(index % AUDIO_FRAMES as u64) as usize].load(Ordering::Relaxed);
        [v as u16 as i16, (v >> 16) as u16 as i16]
    }

    // ----------------------------------------------------------- mechanisms

    /// The engine describes mechanism `number` (1 to [`SCRIPT_MECHS`]), then
    /// sends [`Command::SetMech`]; false for another number.
    pub fn set_mech_config(&self, number: i32, config: &MechConfig) -> bool {
        let Some(slot) = usize::try_from(number - 1).ok().and_then(|i| self.mech_configs.get(i)) else { return false };
        for (cell, v) in slot.iter().zip(config.to_ints()) {
            cell.store(v, Ordering::Relaxed);
        }
        true
    }

    /// The host reads mechanism `number`'s description, once the command
    /// that names it arrived.
    pub fn mech_config(&self, number: i32) -> Option<MechConfig> {
        let slot = usize::try_from(number - 1).ok().and_then(|i| self.mech_configs.get(i))?;
        let mut v = [0; MECH_INTS];
        for (out, cell) in v.iter_mut().zip(slot) {
            *out = cell.load(Ordering::Relaxed);
        }
        Some(MechConfig::from_ints(&v))
    }

    /// The host publishes what PinMAME reports for `number` (-10 to 10): a
    /// mechanism's position, or under its negative number, its speed.
    pub fn set_mech(&self, number: i32, value: i32) {
        if let Some(cell) = self.mech_values.get((number + MECH_NUMBERS) as usize) {
            cell.store(value, Ordering::Relaxed);
        }
    }

    /// `PinmameGetMech(number)`, as the host last read it.
    pub fn mech(&self, number: i32) -> i32 {
        self.mech_values.get((number + MECH_NUMBERS) as usize).map_or(0, |cell| cell.load(Ordering::Relaxed))
    }

    /// How many frames display `index` has had: a cheap check for a new one.
    pub fn frames_published(&self, index: usize) -> u32 {
        self.displays.get(index).map_or(0, |slot| slot.seq.load(Ordering::Acquire) / 2)
    }

    /// Copies display `index`'s latest whole frame into `out`; its frame
    /// number, or None before the first frame.
    pub fn read_frame(&self, index: usize, out: &mut Vec<u8>) -> Option<u32> {
        let slot = self.displays.get(index)?;
        loop {
            let before = slot.seq.load(Ordering::Acquire);
            if before == 0 {
                return None;
            }
            if before & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let size = (slot.size.load(Ordering::Relaxed) as usize).min(DISPLAY_BYTES);
            out.clear();
            for word in slot.data.iter().take(size.div_ceil(4)) {
                out.extend_from_slice(&word.load(Ordering::Relaxed).to_le_bytes());
            }
            out.truncate(size);
            std::sync::atomic::fence(Ordering::Acquire);
            if slot.seq.load(Ordering::Relaxed) == before {
                return Some(before / 2);
            }
        }
    }
}

/// The engine's place in the ring of changes.
#[derive(Debug, Default)]
pub struct OutputReader {
    next: u64,
}

/// What [`OutputReader::read`] found.
#[derive(Debug, PartialEq, Eq)]
pub enum Read {
    /// Every change since the last read, in order.
    Complete,
    /// The engine fell more than [`OUTPUT_SLOTS`] changes behind, so some
    /// were overwritten: it should take the current values instead.
    Lost,
}

impl OutputReader {
    /// A reader that starts with the changes published from now on.
    pub fn from_now(shared: &Shared) -> OutputReader {
        OutputReader { next: shared.outputs_written() }
    }

    /// Appends every change since the last read to `out`.
    pub fn read(&mut self, shared: &Shared, out: &mut Vec<Output>) -> Read {
        let w = shared.output_write.load(Ordering::Acquire);
        if w.wrapping_sub(self.next) > OUTPUT_SLOTS as u64 {
            self.next = w;
            return Read::Lost;
        }
        let start = out.len();
        for i in self.next..w {
            let slot = &shared.outputs[(i % OUTPUT_SLOTS as u64) as usize];
            if let Some(kind) = OutputKind::from_u32(slot.kind.load(Ordering::Relaxed)) {
                out.push(Output { kind, number: slot.number.load(Ordering::Relaxed), value: slot.value.load(Ordering::Relaxed) });
            }
        }
        // the host may have lapped the oldest of those while they were read
        let after = shared.output_write.load(Ordering::Acquire);
        if after.wrapping_sub(self.next) > OUTPUT_SLOTS as u64 {
            out.truncate(start);
            self.next = after;
            return Read::Lost;
        }
        self.next = w;
        Read::Complete
    }
}
