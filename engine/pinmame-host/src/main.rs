// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! `vpi-pinmame`: runs one game's ROM in PinMAME beside the player.
//!
//! The player creates a link (`pinmame-link`) and starts this program with
//! its name. It loads libpinmame, runs the game, applies the switch changes
//! and other commands the player sends, passes on the time fence, and
//! publishes the lamps, solenoids, lighting strings and segment display
//! digits that changed: every millisecond, whenever the player moves the
//! fence, and when PinMAME reports a solenoid change through its callback
//! (it works out modulated coils only when asked, so those wait for the next
//! look). It wakes the player when solenoids changed. It ends when
//! told to stop, when the game stops, or when the player is gone.
//!
//! ```text
//! vpi-pinmame --link <name> --game <rom> --roms <folder> --data <folder>
//!             [--library <pinmame64.dll>] [--samples <folder>] [--parent <pid>]
//!             [--handle-mechanics <n>] [--no-sound] [--cheat] [--use-samples] [--volume <dB>]
//!             [--altsound <folder>] [--altsound-library <altsound64.dll>]
//! vpi-pinmame --rom-files <set> [--library <pinmame64.dll>]
//! vpi-pinmame --all-rom-files [--library <pinmame64.dll>]
//! vpi-pinmame --sets [--library <pinmame64.dll>]
//! ```
//!
//! PinMAME reads ROMs from `--roms` and writes its NVRAM, settings and high
//! scores only under `--data`. `--handle-mechanics` is `PinmameSetHandleMechanics`
//! (VPX's controller sets 0xFF, PinMAME simulating every mechanism it
//! knows; a table's script may set 0 to simulate them itself), and
//! `--no-sound` runs the game without its sound boards' output. `--cheat` is PinMAME's cheat
//! mode (`PinmameSetCheat`; VPinMAME's `cheat`, which skips some games' start-up tests),
//! `--use-samples` plays the game's samples, older machines' recorded sounds (VPinMAME's `samples`;
//! our build of PinMAME's `PinmameSetUseSamples`, off otherwise as in libpinmame), and
//! `--volume` makes the machine's own sound louder or softer by that many decibels, -32 to 32
//! (VPinMAME's `volume`), AltSound's mix keeping its own levels.
//! `--altsound` plays the game's AltSound pack, in `<folder>\altsound\<rom>`,
//! in place of its own sound.

mod altsound;
mod crash;
mod libpinmame;

use std::ffi::{CString, c_char, c_void};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use libpinmame::{Config, Library, NumberState};
use pinmame_link::{Command, DisplayLayout, HostState, Link, MAX_DISPLAYS, MECH_NUMBERS, Output, OutputKind};

const USAGE: &str = "vpi-pinmame --link <name> --game <rom> --roms <folder> --data <folder> [--library <pinmame64.dll>] [--samples <folder>] [--parent <pid>] [--handle-mechanics <n>] [--no-sound] [--cheat] [--use-samples] [--volume <dB>] [--altsound <folder>] [--altsound-library <altsound64.dll>]";

/// How long a game may take to start before the host gives up.
const START_TIMEOUT: Duration = Duration::from_secs(20);

struct Options {
    link: String,
    game: String,
    roms: PathBuf,
    data: PathBuf,
    library: Option<PathBuf>,
    samples: Option<PathBuf>,
    parent: Option<u32>,
    handle_mechanics: i32,
    sound: bool,
    /// PinMAME's cheat mode (`--cheat`; VPinMAME's `cheat`, which skips some games' start-up tests).
    cheat: bool,
    /// PinMAME's samples, older machines' recorded sounds (`--use-samples`; VPinMAME's `samples`).
    use_samples: bool,
    /// The machine's own sound louder or softer, in decibels, -32 to 32 (`--volume`; VPinMAME's
    /// `volume`, its attenuation); AltSound's mix keeps its own levels.
    volume_db: f32,
    /// The folder holding `altsound\<rom>`, when the game's AltSound pack plays.
    altsound: Option<PathBuf>,
    altsound_library: Option<PathBuf>,
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let (mut link, mut game, mut roms, mut data, mut library, mut samples, mut parent) = (None, None, None, None, None, None, None);
    let (mut handle_mechanics, mut sound, mut cheat, mut volume_db) = (0xFF, true, false, 0.0f32);
    let mut use_samples = false;
    let (mut altsound, mut altsound_library) = (None, None);
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{a} needs a value\n{USAGE}"));
        match a.as_str() {
            "--link" => link = Some(value()?),
            "--game" => game = Some(value()?),
            "--roms" => roms = Some(PathBuf::from(value()?)),
            "--data" => data = Some(PathBuf::from(value()?)),
            "--library" => library = Some(PathBuf::from(value()?)),
            "--samples" => samples = Some(PathBuf::from(value()?)),
            "--parent" => parent = Some(value()?.parse().map_err(|_| "--parent takes a process id".to_string())?),
            "--handle-mechanics" => handle_mechanics = value()?.parse().map_err(|_| "--handle-mechanics takes a number".to_string())?,
            "--no-sound" => sound = false,
            "--cheat" => cheat = true,
            "--use-samples" => use_samples = true,
            "--volume" => volume_db = value()?.parse::<f32>().map_err(|_| "--volume takes decibels".to_string())?.clamp(-32.0, 32.0),
            "--altsound" => altsound = Some(PathBuf::from(value()?)),
            "--altsound-library" => altsound_library = Some(PathBuf::from(value()?)),
            _ => return Err(format!("unknown argument {a}\n{USAGE}")),
        }
    }
    let missing = |what: &str| format!("{what} is required\n{USAGE}");
    Ok(Options {
        link: link.ok_or_else(|| missing("--link"))?,
        game: game.ok_or_else(|| missing("--game"))?,
        roms: roms.ok_or_else(|| missing("--roms"))?,
        data: data.ok_or_else(|| missing("--data"))?,
        library,
        samples,
        parent,
        handle_mechanics,
        sound,
        cheat,
        use_samples,
        volume_db,
        altsound,
        altsound_library,
    })
}

/// Whether this is a development build, which also looks in the repository's
/// `vpx-build` folders for the libraries it loads. A release build, compiled
/// with `VPI_RELEASE_BUILD` set (`tools\release`), looks beside the program
/// alone.
const DEVELOPMENT: bool = option_env!("VPI_RELEASE_BUILD").is_none();

/// Where libpinmame is: beside this program (how it ships), else, in the
/// repository, our build or the copy from VPX's library bundle (development
/// builds).
fn find_library() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let beside = dir.join("pinmame64.dll");
    if beside.exists() {
        return Some(beside);
    }
    if !DEVELOPMENT {
        return None;
    }
    // our build of VPX 5436's PinMAME, which also gives the DMDs' identify frames
    let ours = dir.ancestors().take(4).map(|up| up.join(r"vpx-build\pinmame-5436\build\Release\pinmame64.dll")).find(|p| p.exists());
    ours.or_else(|| dir.ancestors().take(4).map(|up| up.join(r"vpx-build\downloads\5436-third-party-windows-x64-Release\runtime-libs\windows-x64\pinmame64.dll")).find(|p| p.exists()))
}

/// The link, for PinMAME's callbacks. Never dropped: PinMAME's thread may
/// still call back while the program ends.
static LINK: OnceLock<Link> = OnceLock::new();

/// Set once a frame too big for the link was dropped, so it's said once.
static FRAME_TOO_BIG: AtomicBool = AtomicBool::new(false);

/// Our PinMAME build's identify-frame functions, when the library has them.
type IdentifyFrameFn = unsafe extern "C" fn(i32, *mut u8, i32, *mut u32) -> i32;
type IdentifyBitsFn = unsafe extern "C" fn(i32) -> i32;
static IDENTIFY: OnceLock<(IdentifyFrameFn, IdentifyBitsFn)> = OnceLock::new();

/// The identify frame number last published, by display, so each goes over once.
static IDENTIFY_LAST: [AtomicU32; pinmame_link::MAX_DISPLAYS] = [const { AtomicU32::new(u32::MAX) }; pinmame_link::MAX_DISPLAYS];

/// Hands DMD `index`'s identify frame over the link when it's new: read while PinMAME is in its
/// display callback, as the function asks.
fn publish_identify(link: &Link, index: usize) {
    let (Some(&(frame_of, bits_of)), Some(last)) = (IDENTIFY.get(), IDENTIFY_LAST.get(index)) else { return };
    thread_local! {
        static BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0; pinmame_link::DISPLAY_BYTES]);
    }
    BUFFER.with(|b| {
        let mut b = b.borrow_mut();
        let mut number = 0u32;
        // SAFETY: a buffer as long as it says; PinMAME is in its display callback
        let n = unsafe { frame_of(index as i32, b.as_mut_ptr(), b.len() as i32, &mut number) };
        if n <= 0 || last.swap(number, Ordering::Relaxed) == number {
            return;
        }
        let bits = unsafe { bits_of(index as i32) }.max(0) as u32;
        link.shared().publish_identify(index, bits, &b[..n as usize]);
    });
}

// The header doesn't say which callbacks may be left out, so the host gives
// every one; these stand in for what it doesn't pass on yet (sound,
// mechanics, sound-board commands). The game's state is asked with
// `PinmameIsRunning` instead.
unsafe extern "system" fn on_state_updated(_state: i32, _user: *mut c_void) {}

unsafe extern "system" fn on_display_available(index: i32, count: i32, layout: *mut libpinmame::DisplayLayout, _user: *mut c_void) {
    let (Some(link), false) = (LINK.get(), layout.is_null()) else {
        return;
    };
    // SAFETY: PinMAME hands the display's description
    let l = unsafe { *layout };
    let layout = DisplayLayout { kind: l.kind as u32, top: l.top, left: l.left, length: l.length, width: l.width, height: l.height, depth: l.depth };
    if !link.shared().set_display(index.max(0) as usize, count.max(0) as usize, layout) {
        eprintln!("vpi-pinmame: display {index} of {count} is beyond the {MAX_DISPLAYS} the link carries");
    }
}

unsafe extern "system" fn on_display_updated(index: i32, data: *mut c_void, layout: *mut libpinmame::DisplayLayout, _user: *mut c_void) {
    let (Some(link), false) = (LINK.get(), layout.is_null()) else {
        return;
    };
    // SAFETY: PinMAME hands the display's description, and its new frame unless it didn't change
    let l = unsafe { *layout };
    let shown = DisplayLayout { kind: l.kind as u32, ..DisplayLayout::default() };
    // a DMD's identify frame can be new while what it shows isn't
    if shown.is_dmd() {
        publish_identify(link, index.max(0) as usize);
    }
    if data.is_null() {
        return;
    }
    let size = if shown.is_dmd() {
        l.width * l.height
    } else if shown.is_video() {
        l.width * l.height * if l.depth == 16 { 2 } else { 3 }
    } else {
        l.length * 2
    };
    if size <= 0 {
        return;
    }
    let frame = unsafe { std::slice::from_raw_parts(data as *const u8, size as usize) };
    if !link.shared().publish_frame(index.max(0) as usize, frame) && !FRAME_TOO_BIG.swap(true, Ordering::Relaxed) {
        eprintln!("vpi-pinmame: display {index}'s frames are too big for the link ({size} bytes)");
    }
}

/// AltSound, while it plays the game's pack: PinMAME's sound boards' commands go to it,
/// and its sound into the link in place of PinMAME's.
static ALTSOUND: std::sync::Mutex<Option<altsound::AltSound>> = std::sync::Mutex::new(None);
static ALTSOUND_PLAYING: AtomicBool = AtomicBool::new(false);

/// Starts the game's AltSound pack in `base`: its sound goes into the link in place of
/// PinMAME's from now on. Said and left out when it can't start.
fn start_altsound(options: &Options, base: &Path, hardware_gen: u64, shared: &pinmame_link::Shared) {
    let Some(library) = options.altsound_library.clone().or_else(find_altsound_library) else {
        eprintln!("vpi-pinmame: AltSound's library (altsound64.dll) wasn't found; the game plays its own sound");
        return;
    };
    // the link carries one rate: PinMAME's, which AltSound mixes at too
    let rate = match shared.audio_rate() {
        0 => {
            shared.set_audio_rate(48_000);
            48_000
        }
        r => r,
    };
    let log_dir = options.data.parent().unwrap_or(&options.data);
    match altsound::AltSound::start(&library, base, &options.game, hardware_gen, rate, log_dir, on_altsound_audio) {
        Ok(a) => {
            *ALTSOUND.lock().unwrap_or_else(|e| e.into_inner()) = Some(a);
            ALTSOUND_PLAYING.store(true, Ordering::Release);
            eprintln!("vpi-pinmame: AltSound plays {}'s pack from {}", options.game, base.join("altsound").join(&options.game).display());
        }
        Err(e) => eprintln!("vpi-pinmame: {e}; the game plays its own sound"),
    }
}

/// Stops AltSound, if it played.
fn stop_altsound() {
    ALTSOUND_PLAYING.store(false, Ordering::Release);
    drop(ALTSOUND.lock().unwrap_or_else(|e| e.into_inner()).take());
}

/// AltSound's mixed sound: into the link's ring, as PinMAME's would go.
unsafe extern "C" fn on_altsound_audio(samples: *const f32, frames: usize, _rate: u32, channels: u32, _user: *mut c_void) {
    let (Some(link), false, 1..=2) = (LINK.get(), samples.is_null(), channels) else { return };
    let channels = channels as usize;
    // SAFETY: libaltsound hands `frames` frames of `channels` floats
    let values = unsafe { std::slice::from_raw_parts(samples, frames * channels) };
    let to_i16 = |x: f32| (x.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
    link.shared().publish_audio(values.chunks_exact(channels).map(|f| [to_i16(f[0]), to_i16(f[channels - 1])]));
}

/// The AltSound library: beside this program (how it ships), else, in the repository, our build
/// of VPX 5436's (`tools\altsound`, which reads every folder of a PinSound pack), else VPX
/// 5436's from its library bundle (development builds).
fn find_altsound_library() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let beside = dir.join("altsound64.dll");
    if beside.exists() {
        return Some(beside);
    }
    if !DEVELOPMENT {
        return None;
    }
    let ours = dir.ancestors().take(4).map(|up| up.join(r"vpx-build\libaltsound\build\Release\altsound64.dll")).find(|p| p.exists());
    ours.or_else(|| dir.ancestors().take(4).map(|up| up.join(r"vpx-build\downloads\5436-third-party-windows-x64-Release\runtime-libs\windows-x64\altsound64.dll")).find(|p| p.exists()))
}

/// How PinMAME's sound comes, once it said: its channels (1 or 2; 0 before
/// it said, or a kind the link doesn't carry) and whether its samples are
/// floating point rather than 16-bit.
static AUDIO_CHANNELS: AtomicI32 = AtomicI32::new(0);
/// The machine's own sound's gain (`--volume`), as an `f32`'s bits; 1 unless asked.
static GAIN: AtomicU32 = AtomicU32::new(0x3F80_0000);
static AUDIO_FLOAT: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn on_audio_available(info: *mut libpinmame::AudioInfo, _user: *mut c_void) -> i32 {
    let (Some(link), false) = (LINK.get(), info.is_null()) else { return 0 };
    // SAFETY: PinMAME hands its sound's description
    let info = unsafe { &*info };
    let carried = info.sample_rate > 0.0 && matches!(info.channels, 1 | 2) && matches!(info.format, libpinmame::AUDIO_FORMAT_INT16 | libpinmame::AUDIO_FORMAT_FLOAT);
    AUDIO_FLOAT.store(info.format == libpinmame::AUDIO_FORMAT_FLOAT, Ordering::Relaxed);
    AUDIO_CHANNELS.store(if carried { info.channels } else { 0 }, Ordering::Release);
    link.shared().set_audio_rate(if carried { info.sample_rate.round() as u32 } else { 0 });
    info.samples_per_frame
}

/// PinMAME's sound boards' newest output: into the link's ring, as stereo
/// 16-bit frames (a mono board plays on both sides).
unsafe extern "system" fn on_audio_updated(buffer: *mut c_void, samples: i32, _user: *mut c_void) -> i32 {
    // AltSound's sound goes in its place (VPX mutes PinMAME's stream for it)
    if ALTSOUND_PLAYING.load(Ordering::Acquire) {
        return samples;
    }
    let channels = AUDIO_CHANNELS.load(Ordering::Acquire) as usize;
    let (Some(link), false, 1..=2) = (LINK.get(), buffer.is_null(), channels) else { return samples };
    let n = samples.max(0) as usize * channels;
    let to_i16 = |x: f32| (x.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
    let gain = f32::from_bits(GAIN.load(Ordering::Relaxed));
    let mut values: Vec<i16> = if AUDIO_FLOAT.load(Ordering::Relaxed) {
        // SAFETY: PinMAME hands `samples` frames of its format
        unsafe { std::slice::from_raw_parts(buffer as *const f32, n) }.iter().map(|&x| to_i16(x * gain)).collect()
    } else {
        unsafe { std::slice::from_raw_parts(buffer as *const i16, n) }.to_vec()
    };
    if gain != 1.0 && !AUDIO_FLOAT.load(Ordering::Relaxed) {
        for v in &mut values {
            *v = (f32::from(*v) * gain).clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
        }
    }
    link.shared().publish_audio(values.chunks_exact(channels).map(|f| [f[0], f[channels - 1]]));
    samples
}

unsafe extern "system" fn on_mech_available(_mech: i32, _info: *mut c_void, _user: *mut c_void) {}

unsafe extern "system" fn on_mech_updated(_mech: i32, _info: *mut c_void, _user: *mut c_void) {}

/// An event PinMAME's thread sets when it reports a solenoid change through
/// its callback, so the loop below hands the change on at once rather than
/// at its next look (modulated coils aren't reported this way).
static SOLENOID_CHANGED: OnceLock<usize> = OnceLock::new();

unsafe extern "system" fn on_solenoid_updated(_state: *mut NumberState, _user: *mut c_void) {
    if let Some(&event) = SOLENOID_CHANGED.get() {
        // SAFETY: an event handle that lives as long as the program
        unsafe { SetEvent(event as *mut c_void) };
    }
}

unsafe extern "system" fn on_console_data_updated(_data: *mut c_void, _size: i32, _user: *mut c_void) {}

unsafe extern "system" fn is_key_pressed(_keycode: u32, _user: *mut c_void) -> i32 {
    0
}

/// A command the game sent a sound board: to AltSound, when it plays (VPX's plugin hears every
/// board's and hands each on as it is, with its board's number).
unsafe extern "system" fn on_sound_command(board: i32, command: i32, _user: *mut c_void) {
    if ALTSOUND_PLAYING.load(Ordering::Acquire)
        && let Some(a) = ALTSOUND.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
    {
        a.command(board.max(0) as u32, command as u32);
    }
}

unsafe extern "C" {
    /// The C runtime's formatter behind every `printf` (UCRT).
    fn __stdio_common_vsprintf(options: u64, buffer: *mut c_char, count: usize, format: *const c_char, locale: *mut c_void, args: *mut c_void) -> i32;
}

unsafe extern "system" fn on_log_message(level: i32, format: *const c_char, args: *mut c_void, _user: *mut c_void) {
    if level < libpinmame::LOG_LEVEL_INFO || format.is_null() {
        return;
    }
    let mut buffer = [0 as c_char; 1024];
    // SAFETY: PinMAME hands a printf format and its arguments; the buffer
    // is null-terminated whatever the result
    let text = unsafe {
        __stdio_common_vsprintf(2, buffer.as_mut_ptr(), buffer.len(), format, std::ptr::null_mut(), args);
        std::ffi::CStr::from_ptr(buffer.as_ptr()).to_string_lossy().into_owned()
    };
    let tag = if level >= libpinmame::LOG_LEVEL_ERROR { "error" } else { "info" };
    eprintln!("vpi-pinmame: PinMAME {tag}: {}", text.trim_end());
}

/// One line of the host's timing (`VPI_LATENCY_LOG`): `kind time number value`.
fn time_it(timing: &mut Option<std::io::BufWriter<std::fs::File>>, kind: char, number: i32, value: i64) {
    if let Some(t) = timing {
        let _ = std::io::Write::write_fmt(t, format_args!("{kind} {} {number} {value}\n", pinmame_link::clock_us()));
    }
}

// Windows waiting: a high-resolution timer for the millisecond beat, an
// event for solenoid changes, and the player's process, so the host ends if
// the player does.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(attributes: *const c_void, manual_reset: i32, initial_state: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
    fn CreateWaitableTimerExW(attributes: *const c_void, name: *const u16, flags: u32, access: u32) -> *mut c_void;
    fn SetWaitableTimer(timer: *mut c_void, due: *const i64, period: i32, routine: *const c_void, argument: *const c_void, resume: i32) -> i32;
    fn WaitForMultipleObjects(count: u32, handles: *const *mut c_void, wait_all: i32, milliseconds: u32) -> u32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
}
const CREATE_WAITABLE_TIMER_HIGH_RESOLUTION: u32 = 0x2;
const TIMER_ALL_ACCESS: u32 = 0x001F_0003;
const SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_OBJECT_0: u32 = 0;
const INFINITE: u32 = 0xFFFF_FFFF;

/// `vpi-pinmame --rom-files <set> [--library <pinmame64.dll>]`: the set's ROM files as PinMAME
/// expects them — each file's name, size and CRC, and whether it's optional, never dumped, a bad
/// dump, or its parent set's — and the set it's a clone of, as JSON on one line, for checking a ROM
/// set before it's installed (our build's `PinmameGetGameRoms`). No link, no game run.
fn rom_files(args: &[String]) -> ExitCode {
    let (mut set, mut library) = (None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--rom-files" => set = it.next().cloned(),
            "--library" => library = it.next().map(PathBuf::from),
            _ => {
                eprintln!("vpi-pinmame: unknown argument {a}");
                return ExitCode::FAILURE;
            }
        }
    }
    let Some(set) = set.filter(|s| !s.is_empty() && !s.contains('\0')) else {
        eprintln!("vpi-pinmame: --rom-files needs a set's name");
        return ExitCode::FAILURE;
    };
    let lib = match library.or_else(find_library).ok_or_else(|| "pinmame64.dll not found (pass --library)".to_string()).and_then(|p| Library::load(&p)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vpi-pinmame: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(get_roms) = lib.GetGameRoms else {
        eprintln!("vpi-pinmame: this PinMAME can't list a set's files (ours can, tools/pinmame)");
        return ExitCode::FAILURE;
    };
    let config = bare_config();
    let name: Vec<c_char> = set.bytes().map(|b| b as c_char).chain(std::iter::once(0)).collect();
    unsafe extern "system" fn on_game(game: *mut libpinmame::Game, user: *mut c_void) {
        // SAFETY: PinMAME's game for the length of the call; `user` the facts below
        let (game, out) = unsafe { (&*game, &mut *(user as *mut Vec<Option<String>>)) };
        let text = |p: *const c_char| if p.is_null() { None } else { Some(unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned()) };
        *out = vec![text(game.name), text(game.clone_of), text(game.description)];
    }
    unsafe extern "system" fn on_rom(rom: *mut libpinmame::Rom, user: *mut c_void) {
        // SAFETY: PinMAME's ROM entry for the length of the call; `user` the list below
        let (rom, out) = unsafe { (&*rom, &mut *(user as *mut Vec<(String, u32, u32, u32)>)) };
        let name = if rom.name.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(rom.name) }.to_string_lossy().into_owned() };
        out.push((name, rom.length, rom.crc, rom.flags));
    }
    let mut facts: Vec<Option<String>> = Vec::new();
    let mut files: Vec<(String, u32, u32, u32)> = Vec::new();
    // SAFETY: the configuration lives on; each call hands its results to the callbacks above
    let (game_status, roms_status) = unsafe {
        (lib.SetConfig)(config);
        let g = (lib.GetGame)(name.as_ptr(), Some(on_game), &mut facts as *mut _ as *mut c_void);
        let r = get_roms(name.as_ptr(), Some(on_rom), &mut files as *mut _ as *mut c_void);
        (g, r)
    };
    if game_status == libpinmame::STATUS_GAME_NOT_FOUND || roms_status == libpinmame::STATUS_GAME_NOT_FOUND {
        println!("{{\"set\":{},\"known\":false}}", json_string(&set));
        return ExitCode::SUCCESS;
    }
    if game_status != 0 || roms_status != 0 {
        eprintln!("vpi-pinmame: PinMAME couldn't list {set} ({game_status}, {roms_status})");
        return ExitCode::FAILURE;
    }
    let fact = |i: usize| facts.get(i).cloned().flatten();
    let list: Vec<String> = files
        .iter()
        .map(|(n, size, crc, flags)| {
            format!(
                "{{\"name\":{},\"size\":{size},\"crc\":\"{crc:08x}\",\"optional\":{},\"noDump\":{},\"badDump\":{},\"inParent\":{}}}",
                json_string(n),
                flags & libpinmame::ROM_OPTIONAL != 0,
                flags & libpinmame::ROM_NO_DUMP != 0,
                flags & libpinmame::ROM_BAD_DUMP != 0,
                flags & libpinmame::ROM_IN_PARENT != 0
            )
        })
        .collect();
    let opt = |v: Option<String>| v.map_or("null".to_string(), |s| json_string(&s));
    println!(
        "{{\"set\":{},\"known\":true,\"cloneOf\":{},\"description\":{},\"files\":[{}]}}",
        json_string(&fact(0).unwrap_or(set)),
        opt(fact(1)),
        opt(fact(2)),
        list.join(",")
    );
    ExitCode::SUCCESS
}

/// A configuration with nothing to call back and no folders, for asking PinMAME about its games:
/// no aliases looked up, nothing run. It lives on, as PinMAME keeps it.
fn bare_config() -> &'static Config {
    Box::leak(Box::new(Config {
        audio_format: libpinmame::AUDIO_FORMAT_INT16,
        sample_rate: 0,
        vpm_path: [0; libpinmame::MAX_PATH],
        on_state_updated: None,
        on_display_available: None,
        on_display_updated: None,
        on_audio_available: None,
        on_audio_updated: None,
        on_mech_available: None,
        on_mech_updated: None,
        on_solenoid_updated: None,
        on_console_data_updated: None,
        is_key_pressed: None,
        on_log_message: None,
        on_sound_command: None,
    }))
}

/// `vpi-pinmame --sets [--library <pinmame64.dll>]`: every game PinMAME knows — its set, the set
/// it's a clone of, and its name — as JSON on one line, for offering a table the other sets of its
/// game (Change the ROM). No link, no game run.
fn sets(args: &[String]) -> ExitCode {
    let mut library = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--sets" => {}
            "--library" => library = it.next().map(PathBuf::from),
            _ => {
                eprintln!("vpi-pinmame: unknown argument {a}");
                return ExitCode::FAILURE;
            }
        }
    }
    let lib = match library.or_else(find_library).ok_or_else(|| "pinmame64.dll not found (pass --library)".to_string()).and_then(|p| Library::load(&p)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vpi-pinmame: {e}");
            return ExitCode::FAILURE;
        }
    };
    unsafe extern "system" fn on_game(game: *mut libpinmame::Game, user: *mut c_void) {
        // SAFETY: PinMAME's game for the length of the call; `user` the list below
        let (game, out) = unsafe { (&*game, &mut *(user as *mut Vec<[Option<String>; 3]>)) };
        let text = |p: *const c_char| if p.is_null() { None } else { Some(unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned()) };
        out.push([text(game.name), text(game.clone_of), text(game.description)]);
    }
    let mut games: Vec<[Option<String>; 3]> = Vec::new();
    let config = bare_config();
    // SAFETY: the configuration lives on; each game handed to the callback above for the length of
    // the call
    let status = unsafe {
        (lib.SetConfig)(config);
        (lib.GetGames)(Some(on_game), &mut games as *mut _ as *mut c_void)
    };
    if status != 0 {
        eprintln!("vpi-pinmame: PinMAME couldn't list its games ({status})");
        return ExitCode::FAILURE;
    }
    let opt = |v: &Option<String>| v.as_deref().filter(|s| !s.is_empty()).map_or("null".to_string(), json_string);
    let list: Vec<String> = games
        .iter()
        .filter(|[name, _, _]| name.as_deref().is_some_and(|n| !n.is_empty()))
        .map(|[name, clone_of, description]| format!("{{\"set\":{},\"cloneOf\":{},\"description\":{}}}", opt(name), opt(clone_of), opt(description)))
        .collect();
    println!("[{}]", list.join(","));
    ExitCode::SUCCESS
}

/// `vpi-pinmame --all-rom-files [--library <pinmame64.dll>]`: every set PinMAME knows with its ROM
/// files, as `--rom-files` gives one, as a JSON array on one line — for telling which set a zip
/// holds by its files, whatever it's named. No link, no game run.
fn all_rom_files(args: &[String]) -> ExitCode {
    let mut library = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--all-rom-files" => {}
            "--library" => library = it.next().map(PathBuf::from),
            _ => {
                eprintln!("vpi-pinmame: unknown argument {a}");
                return ExitCode::FAILURE;
            }
        }
    }
    let lib = match library.or_else(find_library).ok_or_else(|| "pinmame64.dll not found (pass --library)".to_string()).and_then(|p| Library::load(&p)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vpi-pinmame: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(get_roms) = lib.GetGameRoms else {
        eprintln!("vpi-pinmame: this PinMAME can't list a set's files (ours can, tools/pinmame)");
        return ExitCode::FAILURE;
    };
    unsafe extern "system" fn on_game(game: *mut libpinmame::Game, user: *mut c_void) {
        // SAFETY: PinMAME's game for the length of the call; `user` the list below
        let (game, out) = unsafe { (&*game, &mut *(user as *mut Vec<[Option<String>; 3]>)) };
        let text = |p: *const c_char| if p.is_null() { None } else { Some(unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned()) };
        out.push([text(game.name), text(game.clone_of), text(game.description)]);
    }
    unsafe extern "system" fn on_rom(rom: *mut libpinmame::Rom, user: *mut c_void) {
        // SAFETY: PinMAME's ROM entry for the length of the call; `user` the list below
        let (rom, out) = unsafe { (&*rom, &mut *(user as *mut Vec<(String, u32, u32, u32)>)) };
        let name = if rom.name.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(rom.name) }.to_string_lossy().into_owned() };
        out.push((name, rom.length, rom.crc, rom.flags));
    }
    let mut games: Vec<[Option<String>; 3]> = Vec::new();
    let config = bare_config();
    // SAFETY: the configuration lives on; each game handed to the callback above for the length of
    // the call
    let status = unsafe {
        (lib.SetConfig)(config);
        (lib.GetGames)(Some(on_game), &mut games as *mut _ as *mut c_void)
    };
    if status != 0 {
        eprintln!("vpi-pinmame: PinMAME couldn't list its games ({status})");
        return ExitCode::FAILURE;
    }
    let opt = |v: &Option<String>| v.as_deref().filter(|s| !s.is_empty()).map_or("null".to_string(), json_string);
    let mut out: Vec<String> = Vec::with_capacity(games.len());
    for [name, clone_of, description] in games.iter().filter(|[n, _, _]| n.as_deref().is_some_and(|n| !n.is_empty())) {
        let set = name.clone().unwrap_or_default();
        let c_name: Vec<c_char> = set.bytes().map(|b| b as c_char).chain(std::iter::once(0)).collect();
        let mut files: Vec<(String, u32, u32, u32)> = Vec::new();
        // SAFETY: the set's files handed to the callback above for the length of the call
        let status = unsafe { get_roms(c_name.as_ptr(), Some(on_rom), &mut files as *mut _ as *mut c_void) };
        if status != 0 {
            continue;
        }
        let list: Vec<String> = files
            .iter()
            .map(|(n, size, crc, flags)| {
                format!(
                    "{{\"name\":{},\"size\":{size},\"crc\":\"{crc:08x}\",\"optional\":{},\"noDump\":{},\"badDump\":{},\"inParent\":{}}}",
                    json_string(n),
                    flags & libpinmame::ROM_OPTIONAL != 0,
                    flags & libpinmame::ROM_NO_DUMP != 0,
                    flags & libpinmame::ROM_BAD_DUMP != 0,
                    flags & libpinmame::ROM_IN_PARENT != 0
                )
            })
            .collect();
        out.push(format!("{{\"set\":{},\"known\":true,\"cloneOf\":{},\"description\":{},\"files\":[{}]}}", json_string(&set), opt(clone_of), opt(description), list.join(",")));
    }
    println!("[{}]", out.join(","));
    ExitCode::SUCCESS
}

/// Text as a JSON string.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() -> ExitCode {
    // a crash names where it happened and the switches sent just before (a crash otherwise leaves
    // no trace)
    crash::install();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--rom-files") {
        return rom_files(&args);
    }
    if args.first().is_some_and(|a| a == "--sets") {
        return sets(&args);
    }
    if args.first().is_some_and(|a| a == "--all-rom-files") {
        return all_rom_files(&args);
    }
    let options = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("vpi-pinmame: {e}");
            return ExitCode::FAILURE;
        }
    };
    let link = match Link::open(&options.link) {
        Ok(l) => LINK.get_or_init(|| l),
        Err(e) => {
            eprintln!("vpi-pinmame: could not open the link {}: {e}", options.link);
            return ExitCode::FAILURE;
        }
    };
    let fail = |message: String, status: i32| {
        eprintln!("vpi-pinmame: {message}");
        link.shared().set_run_status(status);
        link.shared().set_host_state(HostState::Failed);
        link.wake_engine();
        ExitCode::FAILURE
    };
    let Some(library_path) = options.library.clone().or_else(find_library) else {
        return fail("pinmame64.dll not found (pass --library)".into(), -1);
    };
    let library = match Library::load(&library_path) {
        Ok(l) => l,
        Err(e) => return fail(e, -1),
    };
    if let (Some(frame), Some(bits)) = (library.GetDisplayIdentifyFrame, library.GetDisplayIdentifyBits) {
        let _ = IDENTIFY.set((frame, bits));
    }
    match run(&options, link, &library) {
        Ok(()) => ExitCode::SUCCESS,
        Err((message, status)) => fail(message, status),
    }
}

fn run(options: &Options, link: &Link, lib: &Library) -> Result<(), (String, i32)> {
    let shared = link.shared();
    for dir in ["nvram", "cfg", "hi"] {
        let _ = std::fs::create_dir_all(options.data.join(dir));
    }
    // the configuration lives as long as the program
    let mut config = Box::new(Config {
        audio_format: libpinmame::AUDIO_FORMAT_INT16,
        // no sound: PinMAME reads a sample rate of 0 as "leave it out", as VPX's controller does
        sample_rate: if options.sound { 48_000 } else { 0 },
        vpm_path: [0; libpinmame::MAX_PATH],
        on_state_updated: Some(on_state_updated),
        on_display_available: Some(on_display_available),
        on_display_updated: Some(on_display_updated),
        on_audio_available: Some(on_audio_available),
        on_audio_updated: Some(on_audio_updated),
        on_mech_available: Some(on_mech_available),
        on_mech_updated: Some(on_mech_updated),
        on_solenoid_updated: Some(on_solenoid_updated),
        on_console_data_updated: Some(on_console_data_updated),
        is_key_pressed: Some(is_key_pressed),
        on_log_message: Some(on_log_message),
        on_sound_command: Some(on_sound_command),
    });
    let base = libpinmame::folder(&options.data);
    if base.len() > libpinmame::MAX_PATH {
        return Err((format!("the data folder's path is longer than PinMAME takes: {}", options.data.display()), -1));
    }
    config.vpm_path[..base.len()].copy_from_slice(&base);
    let config: &'static Config = Box::leak(config);
    let set_path = |kind: i32, path: &Path| {
        let p = libpinmame::folder(path);
        // SAFETY: a null-terminated path PinMAME copies
        unsafe { (lib.SetPath)(kind, p.as_ptr()) };
    };
    // SAFETY: libpinmame's setup calls, before the game runs
    unsafe {
        (lib.SetConfig)(config);
        (lib.SetHandleKeyboard)(0);
        (lib.SetHandleMechanics)(options.handle_mechanics);
        (lib.SetCheat)(i32::from(options.cheat));
        if options.use_samples {
            match lib.SetUseSamples {
                Some(set) => set(1),
                None => eprintln!("vpi-pinmame: this PinMAME has no samples switch (ours does, tools/pinmame); its samples stay off"),
            }
        }
    }
    GAIN.store((10.0f32).powf(options.volume_db / 20.0).to_bits(), Ordering::Relaxed);
    // the game's settings that differ from PinMAME's own, said once
    let mut asked = Vec::new();
    if options.cheat {
        asked.push("cheat mode".to_string());
    }
    if options.use_samples && lib.SetUseSamples.is_some() {
        asked.push("samples".to_string());
    }
    if options.volume_db != 0.0 {
        asked.push(format!("its sound {:+} dB", options.volume_db));
    }
    if !asked.is_empty() {
        eprintln!("vpi-pinmame: {}: {}", options.game, asked.join(", "));
    }
    set_path(libpinmame::FILE_TYPE_ROMS, &options.roms);
    set_path(libpinmame::FILE_TYPE_NVRAM, &options.data.join("nvram"));
    set_path(libpinmame::FILE_TYPE_CONFIG, &options.data.join("cfg"));
    set_path(libpinmame::FILE_TYPE_HIGHSCORE, &options.data.join("hi"));
    if let Some(samples) = &options.samples {
        set_path(libpinmame::FILE_TYPE_SAMPLES, samples);
    }

    // SAFETY: an auto-reset event, initially unset, kept for the program's life
    let solenoid_event = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
    if !solenoid_event.is_null() {
        let _ = SOLENOID_CHANGED.set(solenoid_event as usize);
    }

    let game = CString::new(options.game.as_str()).map_err(|_| ("the game's name has a null in it".to_string(), -1))?;
    // SAFETY: a null-terminated name; PinMAME starts its own thread
    let status = unsafe { (lib.Run)(game.as_ptr()) };
    if status != libpinmame::STATUS_OK {
        return Err((format!("PinMAME could not run {} (status {status})", options.game), status));
    }
    let started = Instant::now();
    // starting, until the game runs, or stops again (a ROM missing)
    loop {
        // SAFETY: libpinmame's state query
        match unsafe { (lib.IsRunning)() } {
            libpinmame::STATE_RUNNING => break,
            libpinmame::STATE_STOPPED => {
                return Err((format!("PinMAME stopped {} while starting it (see its messages above)", options.game), -1));
            }
            _ => {}
        }
        if started.elapsed() > START_TIMEOUT {
            unsafe { (lib.Stop)() };
            return Err((format!("{} didn't start within {} s", options.game, START_TIMEOUT.as_secs()), -1));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // SAFETY: libpinmame's queries, now the game runs
    let (hardware, lamps, solenoids, gis) = unsafe { ((lib.GetHardwareGen)(), (lib.GetMaxLamps)(), (lib.GetMaxSolenoids)(), (lib.GetMaxGIs)()) };
    shared.set_game_info(hardware, lamps.max(0) as u32, solenoids.max(0) as u32, gis.max(0) as u32);
    // the DIP switches as PinMAME has them (its settings, or the game's defaults), for the
    // script's `Controller.Dip` (VPX's plugin reads them from the machine)
    for bank in 0..pinmame_link::DIP_BANKS {
        shared.set_dip_bank(bank, unsafe { (lib.GetDIP)(bank as i32) });
    }
    shared.set_host_state(HostState::Running);
    link.wake_engine();
    eprintln!("vpi-pinmame: {} running ({lamps} lamps, {solenoids} solenoids, {gis} lighting strings)", options.game);
    // the game's AltSound pack, in place of its own sound
    if let (Some(base), true) = (&options.altsound, options.sound) {
        start_altsound(options, base, hardware, shared);
    }

    // SAFETY: Windows objects this loop waits on; a null parent means none
    let timer = unsafe {
        let t = CreateWaitableTimerExW(std::ptr::null(), std::ptr::null(), CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS);
        if t.is_null() { CreateWaitableTimerExW(std::ptr::null(), std::ptr::null(), 0, TIMER_ALL_ACCESS) } else { t }
    };
    let parent = options.parent.map(|pid| unsafe { OpenProcess(SYNCHRONIZE, 0, pid) }).filter(|h| !h.is_null());
    let mut handles = vec![link.to_host_event(), timer];
    if !solenoid_event.is_null() {
        handles.push(solenoid_event);
    }
    let parent_woke = parent.map(|p| {
        handles.push(p);
        WAIT_OBJECT_0 + handles.len() as u32 - 1
    });
    let mut changed = vec![NumberState::default(); (lamps.max(solenoids).max(gis).max(0) as usize) + 64];
    // sized as VPX's controller sizes it: PinMAME reports fewer digits than that
    // SAFETY: libpinmame's query on a running game
    let mut digits = vec![libpinmame::LedState::default(); unsafe { (lib.GetMaxLEDs)() }.max(pinmame_link::MAX_LEDS as i32) as usize];
    // SAFETY: libpinmame's query on a running game
    let mut sound_commands = vec![0i32; unsafe { (lib.GetMaxSoundCommands)() }.max(1) as usize];
    let mut fence = None;
    let mut stop = false;
    // the game's NVRAM as libpinmame hands it out, made the first time the engine asks
    let mut nvram_buffer: Vec<libpinmame::NvramState> = Vec::new();
    let (mut wakes, mut solenoid_wakes, running_since) = (0u64, 0u64, Instant::now());
    // the host's side of the switch-to-solenoid measurement, beside the player's log (the
    // player passes its environment on): each switch handed to PinMAME (W), each time
    // fence (F, in microseconds) and each solenoid change published (P), on the player's
    // clock, kept in memory until the game ends
    let mut timing = std::env::var_os("VPI_LATENCY_LOG").and_then(|log| {
        let mut path = log;
        path.push(".host");
        std::fs::File::create(path).ok().map(|f| std::io::BufWriter::with_capacity(1 << 22, f))
    });
    while !stop {
        // SAFETY: re-arms the 1 ms beat, then waits for it, the player's
        // command, a solenoid change, or the player's end
        let woke = unsafe {
            let due: i64 = -10_000; // 1 ms, in 100 ns steps, relative
            SetWaitableTimer(timer, &due, 0, std::ptr::null(), std::ptr::null(), 0);
            WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, INFINITE)
        };
        if Some(woke) == parent_woke {
            eprintln!("vpi-pinmame: the player is gone; stopping");
            break;
        }
        wakes += 1;
        solenoid_wakes += u64::from(!solenoid_event.is_null() && woke == WAIT_OBJECT_0 + 2);
        // the game's NVRAM when the engine asks for it (VPX's PinMAME plugin's `GetNVRAM`: each
        // byte's current value, none while the game can't give it)
        if let Some(request) = shared.nvram_requested() {
            if nvram_buffer.is_empty() {
                // SAFETY: a plain query of libpinmame's limit
                nvram_buffer = vec![libpinmame::NvramState::default(); unsafe { (lib.GetMaxNVRAM)() }.max(0) as usize];
            }
            // SAFETY: the buffer holds as many states as libpinmame hands out at most
            let count = unsafe { (lib.GetNVRAM)(nvram_buffer.as_mut_ptr()) };
            let bytes: Option<Vec<u8>> = (count > 0).then(|| nvram_buffer[..(count as usize).min(nvram_buffer.len())].iter().map(|s| s.now).collect());
            shared.answer_nvram(request, bytes.as_deref());
        }
        while let Some(command) = shared.take_command() {
            // SAFETY: libpinmame's calls on a running game
            unsafe {
                match command {
                    Command::Switch { number, closed } => {
                        crash::note_switch(number, closed);
                        (lib.SetSwitch)(number, closed as i32);
                        time_it(&mut timing, 'W', number, i64::from(closed));
                    }
                    Command::Dip { bank, value } => {
                        (lib.SetDIP)(bank, value);
                        if (0..pinmame_link::DIP_BANKS as i32).contains(&bank) {
                            shared.set_dip_bank(bank as usize, (lib.GetDIP)(bank));
                        }
                    }
                    Command::Pause(on) => {
                        (lib.Pause)(on as i32);
                    }
                    Command::SolenoidMask { which, mask } => (lib.SetSolenoidMask)(which, mask),
                    Command::ModOutputType { output, number, kind } => (lib.SetModOutputType)(output, number, kind),
                    Command::Reset => {
                        (lib.Reset)();
                    }
                    Command::Stop => stop = true,
                    Command::SetMech { number } => {
                        if let Some(c) = shared.mech_config(number) {
                            let mut config = libpinmame::MechConfig {
                                kind: c.kind,
                                sol1: c.sol1,
                                sol2: c.sol2,
                                length: c.length,
                                steps: c.steps,
                                initial_pos: c.initial_pos,
                                acc: c.acc,
                                ret: c.ret,
                                ..Default::default()
                            };
                            for (to, from) in config.sw.iter_mut().zip(c.switches) {
                                *to = libpinmame::MechSwitchConfig { sw_no: from.number, start_pos: from.start, end_pos: from.end, pulse: from.pulse };
                            }
                            let status = (lib.SetMech)(number, &config);
                            if status != libpinmame::STATUS_OK {
                                eprintln!("vpi-pinmame: PinMAME refused mechanism {number} (status {status})");
                            }
                        }
                    }
                }
            }
        }
        if let Some(f) = shared.time_fence()
            && fence != Some(f)
        {
            // SAFETY: libpinmame's call on a running game
            unsafe { (lib.SetTimeFence)(f) };
            fence = Some(f);
            time_it(&mut timing, 'F', 0, (f * 1e6) as i64);
        }
        // the engine is woken for solenoids only: what a script answers at once (lamps and
        // lighting strings show at its next frame, and it reads them then)
        let mut solenoids_changed = false;
        for (kind, get) in [(OutputKind::Lamp, lib.GetChangedLamps), (OutputKind::Solenoid, lib.GetChangedSolenoids), (OutputKind::Gi, lib.GetChangedGIs)] {
            // SAFETY: the buffer holds more entries than the game has outputs of any kind
            let n = unsafe { get(changed.as_mut_ptr()) }.clamp(0, changed.len() as i32) as usize;
            for c in &changed[..n] {
                shared.publish(Output { kind, number: c.number, value: c.state });
                if kind == OutputKind::Solenoid {
                    time_it(&mut timing, 'P', c.number, i64::from(c.state));
                }
            }
            solenoids_changed |= kind == OutputKind::Solenoid && n > 0;
        }
        if solenoids_changed {
            link.wake_engine();
        }
        // the segment displays' digits whose lit segments changed: PinMAME keeps a single
        // record of what it last reported, and only this host asks, so every digit is asked
        // for and the player applies `ChangedLEDs`' masks to what it tells the script
        // SAFETY: the buffer holds more entries than PinMAME has digits
        let n = unsafe { (lib.GetChangedLEDs)(u64::MAX, u64::MAX, digits.as_mut_ptr()) }.clamp(0, digits.len() as i32) as usize;
        for d in &digits[..n] {
            shared.publish(Output { kind: OutputKind::Led, number: d.number, value: d.state });
        }
        // the commands the game sent its sound board, which scripts read (`NewSoundCommands`)
        // SAFETY: the buffer holds as many as libpinmame gives at once
        let n = unsafe { (lib.GetNewSoundCommands)(sound_commands.as_mut_ptr()) }.clamp(0, sound_commands.len() as i32) as usize;
        for &command in &sound_commands[..n] {
            shared.publish(Output { kind: OutputKind::SoundCommand, number: command, value: 0 });
        }
        // every mechanism's position, and its speed under its negative number
        for number in -MECH_NUMBERS..=MECH_NUMBERS {
            if number != 0 {
                // SAFETY: libpinmame's query on a running game
                shared.set_mech(number, unsafe { (lib.GetMech)(number) });
            }
        }
        // SAFETY: libpinmame's state query
        if unsafe { (lib.IsRunning)() } != libpinmame::STATE_RUNNING {
            eprintln!("vpi-pinmame: PinMAME stopped the game");
            break;
        }
    }
    if let Some(t) = &mut timing {
        let _ = std::io::Write::flush(t);
    }
    let seconds = running_since.elapsed().as_secs_f64().max(1e-3);
    eprintln!(
        "vpi-pinmame: {} stopped after {seconds:.0} s, woken {:.0} times a second ({:.0} for solenoid changes)",
        options.game,
        wakes as f64 / seconds,
        solenoid_wakes as f64 / seconds
    );
    stop_altsound();
    // SAFETY: libpinmame stops its thread and saves the NVRAM
    unsafe { (lib.Stop)() };
    let stopping = Instant::now();
    while unsafe { (lib.IsRunning)() } != libpinmame::STATE_STOPPED && stopping.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    shared.set_host_state(HostState::Stopped);
    link.wake_engine();
    Ok(())
}
