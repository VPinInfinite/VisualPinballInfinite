//! A game in VPX 10.8.0 played with what the app's settings pages set in VPX 10.8.1's settings,
//! where 10.8.0 has the same setting: the sound outputs, the volumes, the ball trail and the
//! supersampling. A 10.8.0 game is handed a copy of 10.8.0's own settings, which the pages never
//! change, so each value 10.8.1's settings give is put in that copy over 10.8.0's own.
//!
//! The outputs are the one setting told differently: 10.8.1 names each (`[Player]` `SoundDevice`,
//! the table's sounds, and `SoundDeviceBG`, the backglass's: the music and the machine's sound),
//! 10.8.0 numbers it by its place in Windows' DirectSound list, the first, 0, being Windows'
//! default, as its `PinDirectSound::InitDirectSound` and `AudioMusicPlayer::InitPinDirectSound`
//! read it — so the same name is found in that list as 10.8.0 lists it. Where 10.8.1's names none,
//! or one that isn't there, 10.8.0's own stands. The controller is handed over by its own rules
//! (`controller_1080`). Ours.

use vpi_settings::Change;

/// The `[Player]` settings 10.8.0 reads under the same names and with the same meaning as 10.8.1:
/// the volumes (0 to 100), the ball trail (on or off) and its strength, and the supersampling (the
/// picture drawn at that much of the window's size before it's scaled to it).
const SAME: [&str; 5] = ["SoundVolume", "MusicVolume", "BallTrail", "BallTrailStrength", "AAFactor"];

/// What VPX 10.8.0's settings (`own`) are changed by for a game so that it plays as VPX 10.8.1's
/// (`newer`) say: `outputs`, DirectSound's outputs by name, in the order it lists them;
/// `default_output`, the one of them that's Windows' default now.
///
/// The table's sounds left on Windows' default where the backglass's go to another output are
/// given Windows' default output by its place: the app gives VPX 10.8.0's program the backglass's
/// output as its own in Windows' sound settings, for the machine's sound, which plays on Windows'
/// default (the app's `sound::route_older`), and 10.8.0's first place, Windows' default, would
/// follow it there.
pub fn changes(newer: &[(String, String, String)], own: &[(String, String, String)], outputs: &[String], default_output: Option<&str>) -> Vec<Change> {
    let differs = |name: &str, value: &str| vpi_settings::get(own, "Player", name).is_none_or(|v| v.trim() != value);
    let place = |wanted: &str| outputs.iter().skip(1).position(|o| o.trim() == wanted.trim()).map(|i| i + 1);
    let named = |name: &str| vpi_settings::get(newer, "Player", name).map(str::trim).filter(|v| !v.is_empty());
    let backglass_elsewhere = named("SoundDeviceBG").and_then(place).is_some();
    let mut out = Vec::new();
    for name in ["SoundDevice", "SoundDeviceBG"] {
        let wanted = named(name).or_else(|| (name == "SoundDevice" && backglass_elsewhere).then_some(default_output).flatten());
        let Some(wanted) = wanted else { continue };
        // the first is Windows' default output, which 10.8.0 plays on where nothing's set
        if let Some(i) = place(wanted).map(|i| i.to_string())
            && differs(name, &i)
        {
            out.push(Change::set("Player", name, i));
        }
    }
    for name in SAME {
        if let Some(v) = vpi_settings::get(newer, "Player", name).map(str::trim).filter(|v| !v.is_empty())
            && differs(name, v)
        {
            out.push(Change::set("Player", name, v));
        }
    }
    out
}

/// The backglass's volume a game in VPX 10.8.0 plays at (`[Player]` `MusicVolume`, 0 to 100), as
/// `changes` gives it: 10.8.1's (`newer`), else 10.8.0's own, else VPX's 100.
pub fn backglass_volume(newer: &[(String, String, String)], own: &[(String, String, String)]) -> i32 {
    let of = |settings: &[(String, String, String)]| vpi_settings::get(settings, "Player", "MusicVolume").and_then(vpi_settings::vpx_int);
    of(newer).or_else(|| of(own)).unwrap_or(100).clamp(0, 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backglass_volume_as_the_game_is_given_it() {
        assert_eq!(backglass_volume(&ini("[Player]\nMusicVolume = 45\n"), &ini("[Player]\nMusicVolume = 80\n")), 45);
        assert_eq!(backglass_volume(&ini("[Player]\nMusicVolume = \n"), &ini("[Player]\nMusicVolume = 80\n")), 80);
        assert_eq!(backglass_volume(&ini(""), &ini("")), 100);
    }

    fn ini(text: &str) -> Vec<(String, String, String)> {
        vpi_settings::parse_ini(text)
    }

    fn said(changes: &[Change]) -> Vec<(String, String)> {
        changes.iter().map(|c| (c.name.clone(), c.value.clone().unwrap_or_default())).collect()
    }

    fn outputs() -> Vec<String> {
        ["Primary Sound Driver", "Speakers (Realtek(R) Audio)", "Game (2- USB Mixer)", "Music (2- USB Mixer)"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_outputs_by_their_place_and_the_rest_as_10_8_1_has_them() {
        let newer = ini("[Player]\nSoundDevice = Game (2- USB Mixer)\nSoundDeviceBG = Music (2- USB Mixer)\nMusicVolume = 45\nSoundVolume = 60\nBallTrail = 0\nBallTrailStrength = 0.5\nAAFactor = 1.0\n");
        // as a 10.8.0 set up before the app has it: the trail on, drawn at twice the size
        let own = ini("[Player]\nSound3D = \nSoundDevice = \nSoundDeviceBG = \nMusicVolume = \nSoundVolume = \nBallTrail = 1\nBallTrailStrength = 0.500000\nAAFactor = 2.000000\n");
        let changes = changes(&newer, &own, &outputs(), None);
        assert_eq!(
            said(&changes),
            [
                ("SoundDevice".into(), "2".into()),
                ("SoundDeviceBG".into(), "3".into()),
                ("SoundVolume".into(), "60".into()),
                ("MusicVolume".into(), "45".into()),
                ("BallTrail".into(), "0".into()),
                ("BallTrailStrength".into(), "0.5".into()),
                ("AAFactor".into(), "1.0".into()),
            ]
        );
        assert!(changes.iter().all(|c| c.section == "Player"));
    }

    #[test]
    fn what_10_8_1_leaves_unset_stays_as_10_8_0_has_it() {
        let newer = ini("[Player]\nSoundVolume = 60\n");
        let own = ini("[Player]\nSoundDevice = 1\nMusicVolume = 80\nBallTrail = 1\n");
        assert_eq!(said(&changes(&newer, &own, &outputs(), None)), [("SoundVolume".into(), "60".into())]);
        // and nothing where they agree
        let same = ini("[Player]\nSoundDevice = 2\nSoundVolume = 60\n");
        assert!(changes(&ini("[Player]\nSoundDevice = Game (2- USB Mixer)\nSoundVolume = 60\n"), &same, &outputs(), None).is_empty());
    }

    #[test]
    fn an_output_not_there_or_windows_default_leaves_10_8_0_on_its_own() {
        let newer = ini("[Player]\nSoundDevice = Headphones (USB)\nSoundDeviceBG = Primary Sound Driver\n");
        assert!(changes(&newer, &ini("[Player]\nSoundDevice = 3\n"), &outputs(), None).is_empty());
        // and with no list at all
        let newer = ini("[Player]\nSoundDevice = Game (2- USB Mixer)\n");
        assert!(changes(&newer, &[], &[], None).is_empty());
    }

    #[test]
    fn the_tables_sounds_on_windows_default_named_where_the_backglass_goes_elsewhere() {
        // the table's sounds on Windows' default, the backglass's on the mixer's Music: Windows'
        // default named by its place, as VPX's program is given the backglass's output
        let newer = ini("[Player]\nSoundDeviceBG = Music (2- USB Mixer)\n");
        let given = changes(&newer, &[], &outputs(), Some("Speakers (Realtek(R) Audio)"));
        assert_eq!(said(&given), [("SoundDevice".into(), "1".into()), ("SoundDeviceBG".into(), "3".into())]);
        // both on Windows' default: nothing named, nothing routed
        assert!(changes(&ini("[Player]\nSoundVolume = \n"), &[], &outputs(), Some("Speakers (Realtek(R) Audio)")).is_empty());
        // Windows' default not told: 10.8.0's own left
        assert_eq!(said(&changes(&newer, &[], &outputs(), None)), [("SoundDeviceBG".into(), "3".into())]);
    }
}
