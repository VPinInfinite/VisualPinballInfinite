//! VPinMAME's settings for each game, kept in a file of ours: `Machine settings.ini`
//! beside the settings file, a `[default]` section for every game and one named for a game (the
//! name its table's script gives, as VPinMAME keys them) for that game, each line one of
//! VPinMAME's own settings under its own name. VPinMAME keeps the same in the registry
//! (`Software\Freeware\Visual PinMame\default` and `\<game>`), so a setup can copy them over
//! unchanged. A game's value is its own section's, else the default section's,
//! else VPinMAME's default (`script::controller::setting_default`). A table's script may still
//! set one for the game it plays, over the file's, as VPinMAME lets it.
//!
//! What the player does with them: `sound` (0: the machine plays no sound), `volume` (its sound
//! louder or softer, in decibels, -32 to 32), `cheat` (PinMAME's cheat mode, which skips some
//! games' start-up tests), `samples` (1: older machines' recorded sounds played, through our build
//! of PinMAME; off otherwise), `sound_mode` (0: the game's own sound, never its AltSound pack; 1 to
//! 3: its pack, if it has one), `showpindmd` (0: no real DMD for the game; 1: the one the
//! settings name), `showwindmd` (0: no floating score window for the game), and the DMD's
//! colours and shades handed to a real DMD (`dmd_red` and the rest, `display_options`). The rest
//! of VPinMAME's (its own window's looks, its timing) wait for what they belong to, or have no
//! place in ours. Ours.

use std::path::{Path, PathBuf};

/// The file's sections, lower case: each a game's (or `default`), with its settings, lower case.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MachineSettings {
    sections: Vec<(String, Vec<(String, i32)>)>,
}

impl MachineSettings {
    /// The file beside the settings file in use (`settings`), else in the player's data folder;
    /// none read when it isn't there.
    pub fn load(settings: Option<&Path>) -> MachineSettings {
        Self::file_for(settings).and_then(|f| std::fs::read_to_string(f).ok()).map_or_else(MachineSettings::default, |t| Self::from_text(&t))
    }

    /// Where the file is: beside the settings file, else in `%APPDATA%\Visual Pinball Infinite`.
    pub fn file_for(settings: Option<&Path>) -> Option<PathBuf> {
        let dir = match settings.and_then(Path::parent) {
            Some(d) => d.to_path_buf(),
            None => PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite"),
        };
        Some(dir.join("Machine settings.ini"))
    }

    pub fn from_text(text: &str) -> MachineSettings {
        let mut sections: Vec<(String, Vec<(String, i32)>)> = Vec::new();
        for (section, key, value) in crate::settings::parse_ini(text) {
            let Some(v) = crate::settings::vpx_int(&value) else { continue };
            let section = section.to_lowercase();
            let at = match sections.iter().position(|(s, _)| *s == section) {
                Some(i) => i,
                None => {
                    sections.push((section, Vec::new()));
                    sections.len() - 1
                }
            };
            let entries = &mut sections[at].1;
            let key = key.to_lowercase();
            match entries.iter_mut().find(|(k, _)| *k == key) {
                Some(e) => e.1 = v,
                None => entries.push((key, v)),
            }
        }
        MachineSettings { sections }
    }

    /// Game `game`'s value for `key` in the file: its section's, else the default section's.
    pub fn get(&self, game: &str, key: &str) -> Option<i32> {
        let (game, key) = (game.to_lowercase(), key.to_lowercase());
        let in_section = |name: &str| self.sections.iter().find(|(s, _)| s == name).and_then(|(_, e)| e.iter().find(|(k, _)| *k == key)).map(|&(_, v)| v);
        in_section(&game).or_else(|| in_section("default"))
    }
}

/// VPX 5436's PinMAME settings (`[Plugin.PinMAME]`): the machines' sound on unless `Sound = 0`,
/// PinMAME's cheat mode (`Cheat`), and a PinMAME folder of the user's own (`PinMAMEPath`, the
/// one holding `roms`), tried after a `pinmame` folder beside the table, as VPX 5436 tries them.
#[derive(Clone, Debug, PartialEq)]
pub struct PinmameSettings {
    pub sound: bool,
    pub cheat: bool,
    pub path: Option<PathBuf>,
}

impl Default for PinmameSettings {
    fn default() -> Self {
        PinmameSettings { sound: true, cheat: false, path: None }
    }
}

impl PinmameSettings {
    pub fn from_settings(s: &crate::settings::Settings) -> PinmameSettings {
        let flag = |key: &str| s.get("Plugin.PinMAME", key).and_then(crate::settings::vpx_bool);
        PinmameSettings {
            sound: flag("Sound").unwrap_or(true),
            cheat: flag("Cheat").unwrap_or(false),
            path: s.get("Plugin.PinMAME", "PinMAMEPath").map(str::trim).filter(|p| !p.is_empty()).map(PathBuf::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_game_then_the_default_then_nothing() {
        let m = MachineSettings::from_text("[default]\ndmd_red = 200\nsound = 1\n[AFM_113B]\nsound = 0\nVolume = -6\n");
        assert_eq!(m.get("afm_113b", "sound"), Some(0));
        assert_eq!(m.get("afm_113b", "volume"), Some(-6));
        assert_eq!(m.get("afm_113b", "dmd_red"), Some(200));
        assert_eq!(m.get("tz_94h", "sound"), Some(1));
        assert_eq!(m.get("tz_94h", "cheat"), None);
        assert_eq!(MachineSettings::default().get("x", "sound"), None);
    }

    #[test]
    fn the_plugins_settings() {
        let s = crate::settings::Settings::from_text("[Plugin.PinMAME]\nSound = 0\nCheat = 1\nPinMAMEPath = C:\\PinMAME\n");
        let p = PinmameSettings::from_settings(&s);
        assert_eq!((p.sound, p.cheat), (false, true));
        assert_eq!(p.path, Some(PathBuf::from("C:\\PinMAME")));
        assert_eq!(PinmameSettings::from_settings(&crate::settings::Settings::default()), PinmameSettings::default());
    }
}
