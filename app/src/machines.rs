//! The classic VPinMAME's settings for each machine, where it keeps them: in the user's part of
//! Windows' registry, `HKEY_CURRENT_USER\Software\Freeware\Visual PinMame\<ROM set>`, each a
//! number (`cheat`, `sound_mode`, `samples`, `dmd_red`, …), those for every game under `default`
//! (VPinMAME's `Settings` and `GameSettings` read them so). The app's pages read and change them as
//! a settings file's lines — `[<ROM set>]` or `[default]`, a name and its number — so they're
//! shown and saved as every other setting. VPX's own PinMAME add-on keeps none for each machine:
//! its few are VPX's settings (`[Plugin.PinMAME]`). Ours.

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
}
