// license:BSD-3-Clause

//! PinMAME's alternate sound, AltSound 2: a pack of recordings that
//! replaces a machine's own sound — music, speech, effects — in any of its
//! three layouts (PinSound's folders, `altsound.csv`, G-Sound's
//! `g-sound.csv`). It runs through libaltsound (vpinball/libaltsound,
//! BSD-3-Clause), the library VPX build 5436's AltSound plugin uses, as that
//! plugin runs it: started once the game runs, its pack found by the player
//! (`<base>\altsound\<game>`), handed every command the game sends its sound
//! boards, and its mixed sound played in place of PinMAME's — VPX mutes
//! PinMAME's stream while AltSound's plays.

use std::ffi::{CString, c_char, c_void};
use std::path::Path;

/// libaltsound's audio callback: `frames` frames of `channels` floats.
pub type AudioCallback = unsafe extern "C" fn(samples: *const f32, frames: usize, rate: u32, channels: u32, user: *mut c_void);

unsafe extern "C" {
    fn vpi_altsound_load(path: *const u16) -> i32;
    fn vpi_altsound_set_logger(path: *const c_char, level: i32);
    fn vpi_altsound_init(pinmame_path: *const c_char, game: *const c_char, rate: u32, channels: u32, buffer_frames: u32) -> i32;
    fn vpi_altsound_set_hardware_gen(hardware_gen: u64);
    fn vpi_altsound_set_audio_callback(callback: Option<AudioCallback>, user: *mut c_void);
    fn vpi_altsound_process_command(board: u32, cmd: u32, attenuation: i32) -> i32;
    fn vpi_altsound_shutdown();
}

/// `ALTSOUND_LOG_LEVEL_INFO`, the level VPX's plugin logs at.
const LOG_LEVEL_INFO: i32 = 1;

/// The frames libaltsound mixes at a time (VPX's plugin: `BUFFER_SIZE_FRAMES`).
const BUFFER_FRAMES: u32 = 128;

/// AltSound running for the game; shut down when dropped.
pub struct AltSound;

impl AltSound {
    /// Loads `library` and starts the pack in `base\altsound\<game>` for a machine of PinMAME's
    /// hardware generation `hardware_gen` (libaltsound's own numbers are PinMAME's), stereo at `rate`, its
    /// sound handed to `on_audio`, its log in the folder `log_dir` (VPX's plugin: its own
    /// preferences folder). None when the library or the pack can't start (libaltsound logs why).
    pub fn start(library: &Path, base: &Path, game: &str, hardware_gen: u64, rate: u32, log_dir: &Path, on_audio: AudioCallback) -> Result<AltSound, String> {
        let wide: Vec<u16> = library.as_os_str().to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a null-terminated path
        if unsafe { vpi_altsound_load(wide.as_ptr()) } == 0 {
            return Err(format!("could not load {}", library.display()));
        }
        let text = |p: &Path| CString::new(p.to_string_lossy().into_owned()).map_err(|_| format!("{} holds a null", p.display()));
        let (log, base_c) = (text(log_dir)?, text(base)?);
        let game_c = CString::new(game).map_err(|_| "the game's name holds a null".to_string())?;
        // SAFETY: null-terminated strings; the library is loaded
        unsafe {
            vpi_altsound_set_logger(log.as_ptr(), LOG_LEVEL_INFO);
            if vpi_altsound_init(base_c.as_ptr(), game_c.as_ptr(), rate, 2, BUFFER_FRAMES) == 0 {
                return Err(format!("AltSound didn't start for {game} (its log: {})", log_dir.join("altsound.log").display()));
            }
            vpi_altsound_set_audio_callback(Some(on_audio), std::ptr::null_mut());
            vpi_altsound_set_hardware_gen(hardware_gen);
        }
        Ok(AltSound)
    }

    /// A command the game sent sound board `board` (VPX's plugin hands it on with its board, so
    /// libaltsound can tell System 11's two boards apart, and no attenuation).
    pub fn command(&self, board: u32, cmd: u32) {
        // SAFETY: started; libaltsound takes commands from any thread, as VPX's plugin hands them
        unsafe { vpi_altsound_process_command(board, cmd, 0) };
    }
}

impl Drop for AltSound {
    fn drop(&mut self) {
        // SAFETY: started; its own thread stops first, then its streams go
        unsafe {
            vpi_altsound_set_audio_callback(None, std::ptr::null_mut());
            vpi_altsound_shutdown();
        }
    }
}
