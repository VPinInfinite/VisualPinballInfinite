//! The Sound page's side: the sound outputs as our player sees them (`vpi-player
//! --sound-outputs --json`), and the settings that say what plays where, under VPX 5436's names —
//! `[Player]` `SoundDevice` (the playfield's sounds) and `SoundDeviceBG` (the backglass's: music,
//! the machine's own sound, PinUP packs), `Sound3D` (the playfield's speaker layout), `SoundVolume`
//! and `MusicVolume` — with a PinUP pack's main volume (`[Plugin.PUP]` `MainVol`) and AltSound packs
//! on or off (`[Plugin.AltSound]` `Enable`); and, in VPX from build 6640, a copy of the playfield's
//! sounds for a bass shaker on an output of its own (`SoundDeviceHaptic`, off while unset; its
//! layout `Sound3DHaptic`, its volume `HapticVolume`). Saved as the Screens page saves: only what changed,
//! each file before kept beside it as `<name>.previous.ini` (VPX's as `VPinballX.previous.ini`).

use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use vpi_settings::Change;

/// One of the PC's sound outputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub name: String,
    /// Its channels, as Windows says: 2 for stereo, 6 for 5.1, 8 for 7.1.
    pub channels: Option<i32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Outputs {
    outputs: Vec<Output>,
    default_channels: Option<i32>,
    default_name: Option<String>,
}

/// What the page shows and saves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundSettings {
    /// The playfield's sounds' output, by name; None: Windows' default.
    pub playfield_output: Option<String>,
    /// The backglass's sounds' output.
    pub backglass_output: Option<String>,
    /// The playfield's speaker layout, VPX's 0 to 5.
    pub layout: i32,
    /// 0 to 100.
    pub playfield_volume: i32,
    pub backglass_volume: i32,
    /// A PinUP pack's main volume, 0 to 100.
    pub pup_volume: i32,
    /// AltSound packs play when a game has one.
    pub altsound: bool,
    /// A sound plays as each achievement is announced (`[VPI]` `AchievementSound`).
    pub achievement_sound: bool,
    /// Its level, 0 to 200 (`[VPI]` `AchievementGain`, 0 to 2: 100 as it comes).
    pub achievement_volume: i32,
    /// The bass shaker's output, a copy of the playfield's sounds; None: none, the copy off.
    pub haptic_output: Option<String>,
    /// Its speaker layout, as the playfield's.
    pub haptic_layout: i32,
    /// 0 to 100.
    pub haptic_volume: i32,
}

/// The first VPX build that plays a copy of the playfield's sounds to an output of its own, for a
/// bass shaker (its efb29490b).
pub const HAPTIC_FROM: u16 = 6640;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundView {
    pub outputs: Vec<Output>,
    pub default_channels: Option<i32>,
    /// The output Windows plays through by default now: where VPX plays a sound naming none.
    pub default_name: Option<String>,
    pub settings: SoundSettings,
    /// The settings file, and whether it's there yet.
    pub file: String,
    pub exists: bool,
    /// The VPX tables play with has the bass shaker's output (`HAPTIC_FROM`).
    pub haptic: bool,
}

/// The sound outputs, as the player sees them.
fn outputs() -> Result<Outputs, String> {
    let mut command = Command::new(crate::screens::player()?);
    command.args(["--sound-outputs", "--json"]).stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the player is a console program; no console shows
        command.creation_flags(0x0800_0000);
    }
    let out = command.output().map_err(|e| format!("The player couldn't list the sound outputs: {e}"))?;
    if !out.status.success() {
        return Err("The player couldn't list the sound outputs.".into());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("The player's list of sound outputs couldn't be read: {e}"))
}

/// The settings as the file has them, each unset one as VPX's own default.
fn settings_of(entries: &[(String, String, String)]) -> SoundSettings {
    let get = |section: &str, name: &str| vpi_settings::get(entries, section, name).map(|v| vpi_settings::vpx_trim(v).to_string());
    let name = |key: &str| get("Player", key).filter(|v| !v.is_empty());
    let int = |section: &str, key: &str, min: i32, max: i32, default: i32| get(section, key).and_then(|v| vpi_settings::vpx_int(&v)).filter(|v| (min..=max).contains(v)).unwrap_or(default);
    SoundSettings {
        playfield_output: name("SoundDevice"),
        backglass_output: name("SoundDeviceBG"),
        layout: int("Player", "Sound3D", 0, 5, 0),
        playfield_volume: int("Player", "SoundVolume", 0, 100, 100),
        backglass_volume: int("Player", "MusicVolume", 0, 100, 100),
        pup_volume: get("Plugin.PUP", "MainVol").and_then(|v| vpi_settings::vpx_float(&v)).filter(|v| (0.0..=1.0).contains(v)).map_or(100, |v| (v * 100.0).round() as i32),
        altsound: get("Plugin.AltSound", "Enable").as_deref().and_then(vpi_settings::vpx_bool).unwrap_or(true),
        achievement_sound: get("VPI", "AchievementSound").as_deref().and_then(vpi_settings::vpx_bool).unwrap_or(true),
        achievement_volume: get("VPI", "AchievementGain").and_then(|v| vpi_settings::vpx_float(&v)).filter(|v| (0.0..=2.0).contains(v)).map_or(100, |v| (v * 100.0).round() as i32),
        haptic_output: name("SoundDeviceHaptic"),
        haptic_layout: int("Player", "Sound3DHaptic", 0, 6, 0),
        haptic_volume: int("Player", "HapticVolume", 0, 100, 100),
    }
}

/// Whether the VPX tables play with has the bass shaker's output.
fn haptic_played() -> bool {
    let Some(folders) = vpi_settings::library_folders_now() else { return false };
    let layout = ::library::plan::Layout::from_folders(&folders);
    ::library::vpx::to_play(&layout).is_some_and(|v| v.version >= [10, 8, 1, HAPTIC_FROM])
}

pub fn load() -> Result<SoundView, String> {
    let found = outputs()?;
    let path = vpi_settings::vpx_settings_path().ok_or("no APPDATA folder")?;
    Ok(SoundView {
        outputs: found.outputs,
        default_channels: found.default_channels,
        default_name: found.default_name,
        settings: settings_of(&vpi_settings::read_all()),
        file: path.display().to_string(),
        exists: path.exists(),
        haptic: haptic_played(),
    })
}

/// What saving `after` changes, the file's settings before being `before`: only what differs, an
/// output back to Windows' default removed, every other setting written as VPX writes it.
fn changes(before: &SoundSettings, after: &SoundSettings) -> Vec<Change> {
    let mut out = Vec::new();
    let device = |key: &str, was: &Option<String>, now: &Option<String>, out: &mut Vec<Change>| {
        if was != now {
            out.push(match now {
                Some(name) => Change::set("Player", key, name),
                None => Change::remove("Player", key),
            });
        }
    };
    device("SoundDevice", &before.playfield_output, &after.playfield_output, &mut out);
    device("SoundDeviceBG", &before.backglass_output, &after.backglass_output, &mut out);
    device("SoundDeviceHaptic", &before.haptic_output, &after.haptic_output, &mut out);
    if before.haptic_layout != after.haptic_layout {
        out.push(Change::set("Player", "Sound3DHaptic", after.haptic_layout.clamp(0, 6)));
    }
    if before.haptic_volume != after.haptic_volume {
        out.push(Change::set("Player", "HapticVolume", after.haptic_volume.clamp(0, 100)));
    }
    if before.layout != after.layout {
        out.push(Change::set("Player", "Sound3D", after.layout.clamp(0, 5)));
    }
    if before.playfield_volume != after.playfield_volume {
        out.push(Change::set("Player", "SoundVolume", after.playfield_volume.clamp(0, 100)));
    }
    if before.backglass_volume != after.backglass_volume {
        out.push(Change::set("Player", "MusicVolume", after.backglass_volume.clamp(0, 100)));
    }
    if before.pup_volume != after.pup_volume {
        out.push(Change::set("Plugin.PUP", "MainVol", format!("{:.2}", f64::from(after.pup_volume.clamp(0, 100)) / 100.0)));
    }
    if before.altsound != after.altsound {
        out.push(Change::set("Plugin.AltSound", "Enable", i32::from(after.altsound)));
    }
    if before.achievement_sound != after.achievement_sound {
        out.push(Change::set("VPI", "AchievementSound", i32::from(after.achievement_sound)));
    }
    if before.achievement_volume != after.achievement_volume {
        // as the pause menu writes the level
        out.push(Change::set("VPI", "AchievementGain", format!("{:.6}", f64::from(after.achievement_volume.clamp(0, 200)) / 100.0)));
    }
    out
}

/// The settings saved into the player's settings file in place, only what changed.
pub fn save(settings: &SoundSettings) -> Result<(), String> {
    let was = settings_of(&vpi_settings::read_all());
    vpi_settings::save(&changes(&was, settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_what_changed_is_written() {
        let file = "[Player]\nSoundVolume = 80\n; my own note\nSound3D = 5\n\n[Plugin.VNI]\nPacKey = secret\n";
        let before = settings_of(&vpi_settings::parse_ini(file));
        assert_eq!((before.playfield_volume, before.backglass_volume, before.layout, before.pup_volume, before.altsound), (80, 100, 5, 100, true));
        assert!(changes(&before, &before).is_empty());
        let mut after = before.clone();
        after.backglass_volume = 60;
        after.playfield_output = Some("Speakers (Realtek(R) Audio)".into());
        after.pup_volume = 50;
        after.altsound = false;
        let out = vpi_settings::edit(file, &changes(&before, &after));
        let e = vpi_settings::parse_ini(&out);
        assert_eq!(settings_of(&e), after);
        // the rest of the file as it was
        assert!(out.contains("; my own note") && out.contains("PacKey = secret") && out.contains("SoundVolume = 80"));
        // an output back to Windows' default: the setting gone
        let mut back = after.clone();
        back.playfield_output = None;
        let out2 = vpi_settings::edit(&out, &changes(&after, &back));
        assert!(!out2.contains("SoundDevice ="));
    }
}
