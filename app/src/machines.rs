//! The classic VPinMAME's settings for each machine, where it keeps them: in the user's part of
//! Windows' registry, `HKEY_CURRENT_USER\Software\Freeware\Visual PinMame\<ROM set>`, each a
//! number (`cheat`, `sound_mode`, `samples`, `dmd_red`, …), those for every game under `default`
//! (VPinMAME's `Settings` and `GameSettings` read them so). The app's pages read and change them as
//! a settings file's lines — `[<ROM set>]` or `[default]`, a name and its number — so they're
//! shown and saved as every other setting. VPX's own PinMAME add-on keeps none for each machine:
//! its few are VPX's settings (`[Plugin.PinMAME]`). A game in VPX 10.8.0, which plays with the
//! classic VPinMAME, has its machine's volume lowered by the backglass's for the game, as VPX's
//! newer builds mix it, and put back after (`lower_volume_for`, `put_volume_back`). Ours.

use vpi_settings::Change;

/// VPinMAME's settings the app's pages show, by their names.
pub const NAMES: [&str; 10] = ["sound", "volume", "sound_mode", "samples", "cheat", "showpindmd", "showwindmd", "dmd_red", "dmd_green", "dmd_blue"];

/// Where VPinMAME keeps a machine's settings, as the app shows it.
pub fn place_of(rom: &str) -> String {
    format!(r"HKEY_CURRENT_USER\{}", key_of(rom))
}

fn key_of(rom: &str) -> String {
    format!(r"Software\Freeware\Visual PinMame\{rom}")
}

/// One of a machine's settings as the classic VPinMAME reads it: its own, else every game's
/// (`default`); None where neither is set.
pub fn value(rom: &str, name: &str) -> Option<u32> {
    read(&key_of(rom), name).or_else(|| read(&key_of("default"), name))
}

/// One of a machine's own settings, not every game's: None where its own isn't set.
pub fn own(rom: &str, name: &str) -> Option<u32> {
    read(&key_of(rom), name)
}

/// The settings of the machines named (`default` for every game's), as a settings file's lines:
/// the machine's name, the setting's, its number.
pub fn entries(roms: &[&str]) -> Vec<(String, String, String)> {
    roms.iter().flat_map(|rom| NAMES.iter().filter_map(move |name| read(&key_of(rom), name).map(|v| (rom.to_string(), name.to_string(), (v as i32).to_string())))).collect()
}

/// A machine's volume in a game in VPX 10.8.0 as VPX's newer builds play it: they mix the machine's
/// sound from their PinMAME add-on at the backglass's volume (`[Player]` `MusicVolume`, a
/// hundredth of it the sound's gain), where the classic VPinMAME 10.8.0 plays with takes its own
/// (`volume`, in decibels, -32 its silence: VPinMAME's `osd_set_mastervolume`), which 10.8.0's
/// settings don't reach. The machine's own volume (`own`) lowered by the backglass's (`backglass`,
/// 0 to 100).
pub fn volume_in_1080(own: i32, backglass: i32) -> i32 {
    if backglass <= 0 {
        return -32;
    }
    let lowered = 20.0 * (f64::from(backglass.min(100)) / 100.0).log10();
    ((f64::from(own) + lowered).round() as i32).clamp(-32, 32)
}

/// A machine's volume before a game in VPX 10.8.0 lowered it, kept until it's put back.
#[derive(serde::Serialize, serde::Deserialize)]
struct Lowered {
    rom: String,
    /// The machine's own volume before, none where it had none of its own.
    before: Option<i32>,
    /// What the game was given.
    set: i32,
}

fn lowered_record() -> Option<std::path::PathBuf> {
    Some(crate::library::data_folder()?.join("VPX").join("machine volume.json"))
}

/// Before a game in VPX 10.8.0 of a ROM machine (`rom`, the set VPinMAME runs, which it keeps its
/// settings under): its volume lowered by the backglass's (`backglass`, 0 to 100) for the game, as
/// `volume_in_1080` says, the volume before kept until `put_volume_back`. Nothing changed at 100.
pub fn lower_volume_for(rom: &str, backglass: i32) {
    put_volume_back();
    if backglass >= 100 || rom.is_empty() || rom.contains(['\\', '/']) {
        return;
    }
    let Some(record) = lowered_record() else { return };
    let before = own(rom, "volume").map(|v| v as i32);
    let from = before.or_else(|| own("default", "volume").map(|v| v as i32)).unwrap_or(0);
    let set = volume_in_1080(from, backglass);
    if set == from {
        return;
    }
    // the record first, so the volume is put back even where the app stops during the game
    let kept = Lowered { rom: rom.to_string(), before, set };
    let written = record.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&record, serde_json::to_vec(&kept).unwrap_or_default()));
    if let Err(e) = written {
        eprintln!("{rom}'s volume in VPX 10.8.0 left as it is: the volume before couldn't be kept ({e})");
        return;
    }
    match write(&key_of(rom), "volume", set) {
        Ok(()) => eprintln!("{rom}'s volume in VPX 10.8.0: {set} dB for the game, the backglass's {backglass}%"),
        Err(e) => {
            eprintln!("{rom}'s volume in VPX 10.8.0: {e}");
            let _ = std::fs::remove_file(&record);
        }
    }
}

/// A machine's volume put back as it was before a game in VPX 10.8.0 lowered it: after the game, or
/// as the app starts where it stopped during one. Left as it is where it's been changed since.
pub fn put_volume_back() {
    let Some(record) = lowered_record() else { return };
    let Ok(bytes) = std::fs::read(&record) else { return };
    if let Ok(kept) = serde_json::from_slice::<Lowered>(&bytes)
        && !kept.rom.is_empty()
        && !kept.rom.contains(['\\', '/'])
        && own(&kept.rom, "volume").map(|v| v as i32) == Some(kept.set)
    {
        let back = match kept.before {
            Some(v) => write(&key_of(&kept.rom), "volume", v),
            None => remove(&key_of(&kept.rom), "volume"),
        };
        if let Err(e) = back {
            // the record kept, to try again as the app next starts
            eprintln!("{}'s volume wasn't put back: {e}", kept.rom);
            return;
        }
    }
    let _ = std::fs::remove_file(&record);
}

/// Changes made where VPinMAME keeps them, each by its machine's name (`default`, every game's): a
/// number set, or taken out.
pub fn apply(changes: &[Change]) -> Result<(), String> {
    for c in changes {
        let name = c.name.to_ascii_lowercase();
        if !NAMES.contains(&name.as_str()) || c.section.is_empty() || c.section.contains(['\\', '/']) {
            return Err(format!("{} isn't one of VPinMAME's settings the app changes.", c.name));
        }
        match &c.value {
            Some(v) => write(&key_of(&c.section), &name, vpi_settings::vpx_int(v).ok_or_else(|| format!("{} isn't a number.", c.name))?)?,
            None => remove(&key_of(&c.section), &name)?,
        }
    }
    Ok(())
}

#[cfg(windows)]
mod registry {
    use std::ffi::c_void;
    #[link(name = "advapi32")]
    unsafe extern "system" {
        pub fn RegGetValueW(hkey: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
        pub fn RegSetKeyValueW(hkey: isize, sub_key: *const u16, value: *const u16, kind: u32, data: *const c_void, len: u32) -> i32;
        pub fn RegDeleteKeyValueW(hkey: isize, sub_key: *const u16, value: *const u16) -> i32;
    }
    pub const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
    pub const RRF_RT_REG_DWORD: u32 = 0x10;
    pub const REG_DWORD: u32 = 4;
    pub const ERROR_FILE_NOT_FOUND: i32 = 2;
    pub fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }
}

#[cfg(windows)]
fn read(key: &str, name: &str) -> Option<u32> {
    use registry::*;
    let (key, name) = (wide(key), wide(name));
    let (mut data, mut len) = (0u32, 4u32);
    // SAFETY: NUL-ended names; `data` holds the four bytes a number takes
    let found = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut data as *mut u32).cast(), &mut len) };
    (found == 0).then_some(data)
}

#[cfg(windows)]
fn write(key: &str, name: &str, value: i32) -> Result<(), String> {
    use registry::*;
    let (k, n) = (wide(key), wide(name));
    let data = value as u32;
    // SAFETY: NUL-ended names; four bytes of `data`; the key's made where it isn't there
    let done = unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), n.as_ptr(), REG_DWORD, (&data as *const u32).cast(), 4) };
    if done == 0 { Ok(()) } else { Err(format!("VPinMAME's {name} couldn't be set ({})", std::io::Error::from_raw_os_error(done))) }
}

#[cfg(windows)]
fn remove(key: &str, name: &str) -> Result<(), String> {
    use registry::*;
    let (k, n) = (wide(key), wide(name));
    // SAFETY: NUL-ended names
    let done = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), n.as_ptr()) };
    if done == 0 || done == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(format!("VPinMAME's {name} couldn't be taken out ({})", std::io::Error::from_raw_os_error(done))) }
}

#[cfg(not(windows))]
fn read(_key: &str, _name: &str) -> Option<u32> {
    None
}

#[cfg(not(windows))]
fn write(_key: &str, _name: &str, _value: i32) -> Result<(), String> {
    Err("VPinMAME's settings are Windows' own".into())
}

#[cfg(not(windows))]
fn remove(_key: &str, _name: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_never_set_reads_as_none_and_only_vpinmames_are_changed() {
        // every game's (`default`) answers for a game of its own none, as VPinMAME reads it
        assert_eq!(value("no_such_game_vpi_test", "no_such_setting_vpi_test"), None);
        assert!(entries(&["no_such_game_vpi_test"]).is_empty());
        // never a name the pages don't show, nor a machine's name reaching another key
        assert!(apply(&[Change::set("default", "Run", 1)]).is_err());
        assert!(apply(&[Change::set(r"..\..\x", "cheat", 1)]).is_err());
    }

    #[test]
    fn a_machine_in_10_8_0_as_loud_as_in_vpxs_newer_builds() {
        // a hundredth of the backglass's volume its gain there: 45% is 6.9 dB down
        assert_eq!(volume_in_1080(0, 45), -7);
        assert_eq!(volume_in_1080(0, 100), 0);
        assert_eq!(volume_in_1080(-3, 50), -9);
        assert_eq!(volume_in_1080(5, 100), 5);
        // VPinMAME's silence at the least
        assert_eq!(volume_in_1080(0, 0), -32);
        assert_eq!(volume_in_1080(-20, 1), -32);
    }
}
