// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! libpinmame's interface, declared from its header (`libpinmame.h`,
//! BSD-3-Clause), and loaded from the PinMAME project's own build at run
//! time, so the library stays a separate, replaceable file.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_void};
use std::path::Path;

/// `PINMAME_MAX_PATH`.
pub const MAX_PATH: usize = 512;

/// `PINMAME_STATUS_OK`.
pub const STATUS_OK: i32 = 0;
pub const STATUS_GAME_NOT_FOUND: i32 = 2;

/// `PINMAME_FILE_TYPE`.
pub const FILE_TYPE_ROMS: i32 = 0;
pub const FILE_TYPE_NVRAM: i32 = 1;
pub const FILE_TYPE_SAMPLES: i32 = 2;
pub const FILE_TYPE_CONFIG: i32 = 3;
pub const FILE_TYPE_HIGHSCORE: i32 = 4;

/// What `PinmameIsRunning` and the state callback report. The header doesn't
/// name them; these are the values PinMAME's own `libpinmame.cpp` (also
/// BSD-3-Clause) sets. Only while running may the game be asked anything:
/// the 3.7.0 build answers `PinmameGetHardwareGen` while starting by reading
/// game data that isn't there yet, and crashes.
pub const STATE_STOPPED: i32 = 0;
pub const STATE_RUNNING: i32 = 1;

/// `PINMAME_AUDIO_FORMAT_INT16`.
pub const AUDIO_FORMAT_INT16: i32 = 0;
/// `PINMAME_AUDIO_FORMAT_FLOAT`.
pub const AUDIO_FORMAT_FLOAT: i32 = 1;

/// `PINMAME_LOG_LEVEL`.
pub const LOG_LEVEL_INFO: i32 = 1;
pub const LOG_LEVEL_ERROR: i32 = 2;

/// `PinmameSolenoidState`, `PinmameLampState` and `PinmameGIState` share
/// this shape: a number and its state.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NumberState {
    pub number: i32,
    pub state: i32,
}

/// `PinmameNVRAMState`: a byte of the game's NVRAM, its number, the value it
/// had and the value it has.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvramState {
    pub number: i32,
    pub old: u8,
    pub now: u8,
}

/// `PinmameLEDState`: a segment display's digit, the segments that changed,
/// and those lit now.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LedState {
    pub number: i32,
    pub changed: i32,
    pub state: i32,
}

/// `PinmameDisplayLayout`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DisplayLayout {
    pub kind: i32,
    pub top: i32,
    pub left: i32,
    pub length: i32,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
}

/// `PinmameMechSwitchConfig`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MechSwitchConfig {
    pub sw_no: i32,
    pub start_pos: i32,
    pub end_pos: i32,
    pub pulse: i32,
}

/// `PinmameMechConfig`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MechConfig {
    pub kind: i32,
    pub sol1: i32,
    pub sol2: i32,
    pub length: i32,
    pub steps: i32,
    pub initial_pos: i32,
    pub acc: i32,
    pub ret: i32,
    pub sw: [MechSwitchConfig; 20],
}

/// `PinmameAudioInfo`.
#[repr(C)]
pub struct AudioInfo {
    pub format: i32,
    pub channels: i32,
    pub sample_rate: f64,
    pub frames_per_second: f64,
    pub samples_per_frame: i32,
    pub buffer_size: i32,
}

pub type StateCallback = unsafe extern "system" fn(state: i32, user: *mut c_void);
pub type DisplayAvailableCallback = unsafe extern "system" fn(index: i32, count: i32, layout: *mut DisplayLayout, user: *mut c_void);
pub type DisplayUpdatedCallback = unsafe extern "system" fn(index: i32, data: *mut c_void, layout: *mut DisplayLayout, user: *mut c_void);
pub type AudioAvailableCallback = unsafe extern "system" fn(info: *mut AudioInfo, user: *mut c_void) -> i32;
pub type AudioUpdatedCallback = unsafe extern "system" fn(buffer: *mut c_void, samples: i32, user: *mut c_void) -> i32;
pub type MechCallback = unsafe extern "system" fn(mech: i32, info: *mut c_void, user: *mut c_void);
pub type SolenoidCallback = unsafe extern "system" fn(state: *mut NumberState, user: *mut c_void);
pub type ConsoleCallback = unsafe extern "system" fn(data: *mut c_void, size: i32, user: *mut c_void);
pub type KeyPressedFunction = unsafe extern "system" fn(keycode: u32, user: *mut c_void) -> i32;
pub type LogCallback = unsafe extern "system" fn(level: i32, format: *const c_char, args: *mut c_void, user: *mut c_void);
pub type SoundCommandCallback = unsafe extern "system" fn(board: i32, command: i32, user: *mut c_void);

/// `PinmameConfig`. The header doesn't say which callbacks may be left
/// out, so the host gives every one (stand-ins where it has no use yet).
#[repr(C)]
pub struct Config {
    pub audio_format: i32,
    pub sample_rate: i32,
    pub vpm_path: [c_char; MAX_PATH],
    pub on_state_updated: Option<StateCallback>,
    pub on_display_available: Option<DisplayAvailableCallback>,
    pub on_display_updated: Option<DisplayUpdatedCallback>,
    pub on_audio_available: Option<AudioAvailableCallback>,
    pub on_audio_updated: Option<AudioUpdatedCallback>,
    pub on_mech_available: Option<MechCallback>,
    pub on_mech_updated: Option<MechCallback>,
    pub on_solenoid_updated: Option<SolenoidCallback>,
    pub on_console_data_updated: Option<ConsoleCallback>,
    pub is_key_pressed: Option<KeyPressedFunction>,
    pub on_log_message: Option<LogCallback>,
    pub on_sound_command: Option<SoundCommandCallback>,
}

/// A game's facts (`PinmameGame`).
#[repr(C)]
pub struct Game {
    pub name: *const c_char,
    pub clone_of: *const c_char,
    pub description: *const c_char,
    pub year: *const c_char,
    pub manufacturer: *const c_char,
    pub flags: u32,
    pub found: i32,
}

/// One of a game's ROM files, as PinMAME's audit expects it (our build's `PinmameRom`).
#[repr(C)]
pub struct Rom {
    pub name: *const c_char,
    pub length: u32,
    pub crc: u32,
    pub flags: u32,
}

pub const ROM_OPTIONAL: u32 = 1;
pub const ROM_NO_DUMP: u32 = 2;
pub const ROM_BAD_DUMP: u32 = 4;
pub const ROM_IN_PARENT: u32 = 8;

pub type GameCallback = unsafe extern "system" fn(game: *mut Game, user: *mut c_void);
pub type RomCallback = unsafe extern "system" fn(rom: *mut Rom, user: *mut c_void);

/// The library's functions this host uses.
pub struct Library {
    /// A game's facts: its name, the set it's a clone of, its title, year and maker.
    pub GetGame: unsafe extern "C" fn(*const c_char, Option<GameCallback>, *mut c_void) -> i32,
    /// Every game PinMAME knows, each handed to the callback.
    pub GetGames: unsafe extern "C" fn(Option<GameCallback>, *mut c_void) -> i32,
    /// Our build's (`tools/pinmame`, patch 3): a game's ROM files as PinMAME's own
    /// audit expects them. None in PinMAME's own builds.
    pub GetGameRoms: Option<unsafe extern "C" fn(*const c_char, Option<RomCallback>, *mut c_void) -> i32>,
    pub SetConfig: unsafe extern "C" fn(*const Config),
    pub SetPath: unsafe extern "C" fn(i32, *const c_char),
    pub SetHandleKeyboard: unsafe extern "C" fn(i32),
    pub SetHandleMechanics: unsafe extern "C" fn(i32),
    /// PinMAME's cheat mode (`PinmameSetCheat`).
    pub SetCheat: unsafe extern "C" fn(i32),
    /// Our build's (`tools/pinmame`): PinMAME's samples, older machines' recorded sounds, on
    /// or off (`options.use_samples`, which `PinmameSetConfig` clears). None in other builds.
    pub SetUseSamples: Option<unsafe extern "C" fn(i32)>,
    pub Run: unsafe extern "C" fn(*const c_char) -> i32,
    pub IsRunning: unsafe extern "C" fn() -> i32,
    pub Pause: unsafe extern "C" fn(i32) -> i32,
    pub Reset: unsafe extern "C" fn() -> i32,
    pub Stop: unsafe extern "C" fn(),
    pub GetHardwareGen: unsafe extern "C" fn() -> u64,
    pub SetSwitch: unsafe extern "C" fn(i32, i32),
    pub SetDIP: unsafe extern "C" fn(i32, i32),
    pub GetDIP: unsafe extern "C" fn(i32) -> i32,
    pub GetMaxNVRAM: unsafe extern "C" fn() -> i32,
    pub GetNVRAM: unsafe extern "C" fn(*mut NvramState) -> i32,
    pub SetSolenoidMask: unsafe extern "C" fn(i32, u32),
    pub SetModOutputType: unsafe extern "C" fn(i32, i32, i32),
    pub SetTimeFence: unsafe extern "C" fn(f64),
    pub GetMaxLamps: unsafe extern "C" fn() -> i32,
    pub GetMaxSolenoids: unsafe extern "C" fn() -> i32,
    pub GetMaxGIs: unsafe extern "C" fn() -> i32,
    pub GetChangedLamps: unsafe extern "C" fn(*mut NumberState) -> i32,
    pub GetChangedSolenoids: unsafe extern "C" fn(*mut NumberState) -> i32,
    pub GetChangedGIs: unsafe extern "C" fn(*mut NumberState) -> i32,
    pub GetMaxLEDs: unsafe extern "C" fn() -> i32,
    pub GetChangedLEDs: unsafe extern "C" fn(u64, u64, *mut LedState) -> i32,
    pub SetMech: unsafe extern "C" fn(i32, *const MechConfig) -> i32,
    pub GetMech: unsafe extern "C" fn(i32) -> i32,
    pub GetMaxSoundCommands: unsafe extern "C" fn() -> i32,
    /// The commands the game sent its sound board since the last call, each
    /// a `PinmameSoundCommand` (one `int`, the command).
    pub GetNewSoundCommands: unsafe extern "C" fn(*mut i32) -> i32,
    /// Our build's (`tools/pinmame`): a DMD's latest identify frame, PinMAME's raw shades
    /// a byte a dot, copied while PinMAME is in the display callback; the dots copied, its frame
    /// number through the last argument. None in PinMAME's own builds.
    pub GetDisplayIdentifyFrame: Option<unsafe extern "C" fn(i32, *mut u8, i32, *mut u32) -> i32>,
    /// Our build's: the identify frame's bits a dot, 2 or 4 (0 for none).
    pub GetDisplayIdentifyBits: Option<unsafe extern "C" fn(i32) -> i32>,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
}

/// Look for the library's own dependencies beside it.
const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;

impl Library {
    /// Loads the library at `path` and finds every function this host uses.
    // each transmute's target is the field it fills, typed as the header has it
    #[allow(clippy::missing_transmute_annotations)]
    pub fn load(path: &Path) -> Result<Library, String> {
        let wide: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a null-terminated path; the module stays loaded for the
        // life of the program
        let module = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
        if module.is_null() {
            return Err(format!("could not load {}: {}", path.display(), std::io::Error::last_os_error()));
        }
        macro_rules! function {
            ($name:literal) => {{
                // SAFETY: the export's address, cast to the type the header gives it
                let f = unsafe { GetProcAddress(module, concat!($name, "\0").as_ptr() as *const c_char) };
                if f.is_null() {
                    return Err(format!("{} has no {}", path.display(), $name));
                }
                unsafe { std::mem::transmute::<*mut c_void, _>(f) }
            }};
        }
        // a function only some builds have
        macro_rules! optional {
            ($name:literal) => {{
                // SAFETY: as above, when the export is there
                let f = unsafe { GetProcAddress(module, concat!($name, "\0").as_ptr() as *const c_char) };
                if f.is_null() { None } else { Some(unsafe { std::mem::transmute::<*mut c_void, _>(f) }) }
            }};
        }
        Ok(Library {
            GetGame: function!("PinmameGetGame"),
            GetGames: function!("PinmameGetGames"),
            GetGameRoms: optional!("PinmameGetGameRoms"),
            SetConfig: function!("PinmameSetConfig"),
            SetPath: function!("PinmameSetPath"),
            SetHandleKeyboard: function!("PinmameSetHandleKeyboard"),
            SetHandleMechanics: function!("PinmameSetHandleMechanics"),
            SetCheat: function!("PinmameSetCheat"),
            SetUseSamples: optional!("PinmameSetUseSamples"),
            Run: function!("PinmameRun"),
            IsRunning: function!("PinmameIsRunning"),
            Pause: function!("PinmamePause"),
            Reset: function!("PinmameReset"),
            Stop: function!("PinmameStop"),
            GetHardwareGen: function!("PinmameGetHardwareGen"),
            SetSwitch: function!("PinmameSetSwitch"),
            SetDIP: function!("PinmameSetDIP"),
            GetDIP: function!("PinmameGetDIP"),
            GetMaxNVRAM: function!("PinmameGetMaxNVRAM"),
            GetNVRAM: function!("PinmameGetNVRAM"),
            SetSolenoidMask: function!("PinmameSetSolenoidMask"),
            SetModOutputType: function!("PinmameSetModOutputType"),
            SetTimeFence: function!("PinmameSetTimeFence"),
            GetMaxLamps: function!("PinmameGetMaxLamps"),
            GetMaxSolenoids: function!("PinmameGetMaxSolenoids"),
            GetMaxGIs: function!("PinmameGetMaxGIs"),
            GetChangedLamps: function!("PinmameGetChangedLamps"),
            GetChangedSolenoids: function!("PinmameGetChangedSolenoids"),
            GetChangedGIs: function!("PinmameGetChangedGIs"),
            GetMaxLEDs: function!("PinmameGetMaxLEDs"),
            GetChangedLEDs: function!("PinmameGetChangedLEDs"),
            SetMech: function!("PinmameSetMech"),
            GetMech: function!("PinmameGetMech"),
            GetMaxSoundCommands: function!("PinmameGetMaxSoundCommands"),
            GetNewSoundCommands: function!("PinmameGetNewSoundCommands"),
            GetDisplayIdentifyFrame: optional!("PinmameGetDisplayIdentifyFrame"),
            GetDisplayIdentifyBits: optional!("PinmameGetDisplayIdentifyBits"),
        })
    }
}

/// A path as libpinmame takes it: a null-terminated string with a
/// separator at the end.
pub fn folder(path: &Path) -> Vec<c_char> {
    let text = path.to_string_lossy();
    // without the `\\?\` that canonical Windows paths carry
    let mut s = text.strip_prefix(r"\\?\").unwrap_or(&text).to_string();
    if !s.ends_with('\\') && !s.ends_with('/') {
        s.push('\\');
    }
    s.bytes().map(|b| b as c_char).chain(std::iter::once(0)).collect()
}
