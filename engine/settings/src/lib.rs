//! The settings file's own rules, shared by the player and the app: `%APPDATA%\Visual Pinball Infinite\Settings.ini`, VPX build
//! 5436's settings under its own section and key names, read as VPX's
//! `LayeredINIPropertyStore` reads its ini; its window settings
//! (`Settings_properties.inl`: `PlayfieldDisplay`, `BackglassOutput`,
//! `ScoreViewWndX`, …) and where they put a window, as its `Window::Window`
//! places one (`renderer/Window.cpp`); and changes written back in place, as
//! a person would make them (`edit`), for the app's settings pages.

mod edit;

use std::path::{Path, PathBuf};

pub use edit::{Change, edit};

/// The settings file: the one `VPI_SETTINGS` names, else
/// `%APPDATA%\Visual Pinball Infinite\Settings.ini`.
pub fn settings_path() -> Option<PathBuf> {
    std::env::var_os("VPI_SETTINGS")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite").join("Settings.ini")))
}

/// The version whose preferences folder VPX's settings are kept in where no VPX is followed
/// (`follow_vpx`): 10.8, the first minor version with a folder of its own. VPX keeps one for each
/// minor version (`FileLocator::SetupPrefPath`).
pub const VPX_PREFS_VERSION: [u16; 4] = [10, 8, 1, 0];

/// VPX's settings file's name, in its preferences folder or beside its program.
const VPX_INI: &str = "VPinballX.ini";

/// The VPX whose settings are kept (`follow_vpx`): its version and its program's folder.
static FOLLOWED: std::sync::RwLock<Option<([u16; 4], Option<PathBuf>)>> = std::sync::RwLock::new(None);

/// The VPX whose settings the app keeps from now on, its version and its program's folder: the one
/// that plays the tables, 10.8.1 or newer. VPX keeps a preferences folder for each minor version,
/// so a newer one's settings are its own folder's; beside its program is where VPX looks for an
/// older layout's settings file. `None`: 10.8's (`VPX_PREFS_VERSION`).
pub fn follow_vpx(vpx: Option<([u16; 4], Option<PathBuf>)>) {
    *FOLLOWED.write().unwrap_or_else(std::sync::PoisonError::into_inner) = vpx;
}

/// The VPX followed: its version and its program's folder.
pub fn followed_vpx() -> ([u16; 4], Option<PathBuf>) {
    FOLLOWED.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone().unwrap_or((VPX_PREFS_VERSION, None))
}

/// The preferences folder `VPI_VPX_PREFS` names in place of VPX's own (a test library's, so VPX's
/// own settings are never read or written).
fn prefs_override() -> Option<PathBuf> {
    std::env::var_os("VPI_VPX_PREFS").filter(|p| !p.is_empty()).map(PathBuf::from)
}

/// The folder VPX keeps a preferences folder for each minor version in: `%APPDATA%\VPinballX`.
fn vpx_prefs_base() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("VPinballX"))
}

/// A minor version's preferences folder's name: `10.9`.
fn minor_folder(version: [u16; 4]) -> String {
    format!("{}.{}", version[0], version[1])
}

/// VPX's preferences folder for a VPX of `version`: `%APPDATA%\VPinballX\<major>.<minor>`, or the
/// one `VPI_VPX_PREFS` names.
pub fn vpx_prefs_folder_for(version: [u16; 4]) -> Option<PathBuf> {
    match prefs_override() {
        Some(p) => Some(p),
        None => Some(vpx_prefs_base()?.join(minor_folder(version))),
    }
}

/// VPX's preferences folder, the followed VPX's (`follow_vpx`).
pub fn vpx_prefs_folder() -> Option<PathBuf> {
    vpx_prefs_folder_for(followed_vpx().0)
}

/// The settings file a VPX of `version` reads, its settings and its add-ons' (`VPApp`): `VPinballX.ini`
/// in its preferences folder where there's one, else one beside its program (`program_folder`),
/// else the one in its preferences folder, which it makes. Before VPX has made that folder, the
/// file it reads once its first start has (`first_start`).
pub fn vpx_settings_path_for(version: [u16; 4], program_folder: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = prefs_override() {
        return Some(p.join(VPX_INI));
    }
    Some(settings_file_in(&vpx_prefs_base()?, version, program_folder))
}

/// VPX's settings file, the followed VPX's.
pub fn vpx_settings_path() -> Option<PathBuf> {
    let (version, folder) = followed_vpx();
    vpx_settings_path_for(version, folder.as_deref())
}

/// `vpx_settings_path_for` with `base` as `%APPDATA%\VPinballX`.
fn settings_file_in(base: &Path, version: [u16; 4], program_folder: Option<&Path>) -> PathBuf {
    let prefs = base.join(minor_folder(version));
    let beside = program_folder.map(|d| d.join(VPX_INI)).filter(|f| f.is_file());
    if prefs.is_dir() {
        if prefs.join(VPX_INI).is_file() {
            return prefs.join(VPX_INI);
        }
        return beside.unwrap_or_else(|| prefs.join(VPX_INI));
    }
    match first_start(base, version, program_folder) {
        FirstStart::InPlace(file) => file,
        _ => prefs.join(VPX_INI),
    }
}

/// Where a VPX's first start, its preferences folder not there yet, takes its settings from
/// (`FileLocator::SetupPrefPath`).
#[derive(Debug, PartialEq, Eq)]
enum FirstStart {
    /// Its settings beside its program, naming its version (VPX's older layout): read there, its
    /// folder left empty.
    InPlace(PathBuf),
    /// The settings file it brings into its folder: the one in the folder it copies in whole (the
    /// newest older version's, else the base folder, where VPX kept them before 10.8.1), else the
    /// one beside its program.
    Brought(PathBuf),
    /// Neither: those Windows' registry holds for VPX before 10.8, written into its folder.
    Registry,
}

/// The beside-its-program settings file of a VPX in `program_folder`, where there's one.
fn beside_program(program_folder: Option<&Path>) -> Option<PathBuf> {
    program_folder.map(|d| d.join(VPX_INI)).filter(|f| f.is_file())
}

/// The folder a VPX's first start copies into its own: the newest older version's, else the base
/// folder, where it's there.
fn copied_from(base: &Path, version: [u16; 4]) -> Option<PathBuf> {
    older_prefs(base, version).or_else(|| base.is_dir().then(|| base.to_path_buf()))
}

fn first_start(base: &Path, version: [u16; 4], program_folder: Option<&Path>) -> FirstStart {
    let beside = beside_program(program_folder);
    if let Some(file) = beside.as_ref().filter(|f| names_version(f, version)) {
        return FirstStart::InPlace(file.clone());
    }
    match copied_from(base, version).map(|f| f.join(VPX_INI)).filter(|f| f.is_file()).or(beside) {
        Some(file) => FirstStart::Brought(file),
        None => FirstStart::Registry,
    }
}

/// The newest older version's preferences folder kept in `base`, as VPX looks for one: down from its
/// own minor version to 10.0, then the minor versions of each older major one, 9 down to 0.
fn older_prefs(base: &Path, version: [u16; 4]) -> Option<PathBuf> {
    let own = base.join(minor_folder(version));
    (10..=version[0])
        .rev()
        .flat_map(|major| (0..=if major == version[0] { version[1] } else { 9 }).rev().map(move |minor| [major, minor, 0, 0]))
        .map(|v| base.join(minor_folder(v)))
        .find(|p| *p != own && p.is_dir())
}

/// Whether a settings file names VPX's `major.minor` as the one it was kept by, as VPX reads
/// `[Version]` `VPinball`: its first two words, numbers. (VPX writes one number there, so a file it
/// kept never does.)
fn names_version(file: &Path, version: [u16; 4]) -> bool {
    let entries = parse_ini(&read_text(file));
    let mut words = get(&entries, "Version", "VPinball").unwrap_or_default().split_whitespace().map(|w| w.parse::<i64>().ok());
    words.next().flatten() == Some(i64::from(version[0])) && words.next().flatten() == Some(i64::from(version[1]))
}

/// VPX's preferences folder for a VPX of `version` made as VPX's first start makes it, where it
/// isn't there yet (`FileLocator::SetupPrefPath`), so the app writing in it first leaves behind
/// nothing VPX would have brought: nothing where its settings beside its program name its version
/// (read there); else the newest older version's folder copied in whole (else the base folder,
/// where VPX kept its settings before 10.8.1), then, where that brings no settings file, the one
/// beside its program, else those Windows' registry holds for VPX before 10.8. The folder, or
/// `VPI_VPX_PREFS`'s.
pub fn vpx_prefs_ready_for(version: [u16; 4], program_folder: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = prefs_override() {
        std::fs::create_dir_all(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        return Ok(p);
    }
    make_prefs(&vpx_prefs_base().ok_or("no APPDATA folder")?, version, program_folder)
}

/// VPX's preferences folder made ready, the followed VPX's.
pub fn vpx_prefs_ready() -> Result<PathBuf, String> {
    let (version, folder) = followed_vpx();
    vpx_prefs_ready_for(version, folder.as_deref())
}

/// `vpx_prefs_ready_for` with `base` as `%APPDATA%\VPinballX`.
fn make_prefs(base: &Path, version: [u16; 4], program_folder: Option<&Path>) -> Result<PathBuf, String> {
    let prefs = base.join(minor_folder(version));
    if prefs.is_dir() {
        return Ok(prefs);
    }
    std::fs::create_dir_all(&prefs).map_err(|e| format!("VPX's settings folder couldn't be made: {e}"))?;
    let beside = beside_program(program_folder);
    if beside.as_ref().is_some_and(|f| names_version(f, version)) {
        return Ok(prefs);
    }
    // looked for once its own folder is made, as VPX looks: the base folder is there by then
    if let Some(folder) = copied_from(base, version) {
        copy_tree(&folder, &prefs, &prefs);
    }
    let ini = prefs.join(VPX_INI);
    if !ini.is_file()
        && let Some(beside) = &beside
    {
        std::fs::copy(beside, &ini).map_err(|e| format!("VPX's settings couldn't be brought in: {e}"))?;
    }
    if !ini.is_file() {
        std::fs::write(&ini, registry_settings()).map_err(|e| format!("VPX's settings couldn't be written: {e}"))?;
    }
    Ok(prefs)
}

/// Every folder and file in `from` copied into `to` as VPX copies an older version's preferences
/// (its `recursive_directory_iterator`): folders made, files replaced, `skip` (the new folder, when
/// it's inside) passed over; one that can't be copied said and passed over, as VPX does. A link to
/// a folder made a folder, not followed.
fn copy_tree(from: &Path, to: &Path, skip: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == skip {
            continue;
        }
        let dest = to.join(entry.file_name());
        let link = entry.file_type().is_ok_and(|t| t.is_symlink());
        let made = if path.is_dir() {
            let made = std::fs::create_dir_all(&dest);
            if made.is_ok() && !link {
                copy_tree(&path, &dest, skip);
            }
            made
        } else {
            std::fs::copy(&path, &dest).map(|_| ())
        };
        if let Err(e) = made {
            eprintln!("{} couldn't be brought into VPX's settings folder: {e}", path.display());
        }
    }
}

/// The settings Windows' registry holds for VPX before 10.8, as VPX's first start writes them into
/// its settings file: each value of its keys, text or number, under a section named for its key.
#[cfg(windows)]
fn registry_settings() -> String {
    const KEYS: [&str; 30] = [
        "Controller", "Editor", "Player", "PlayerVR", "RecentDir", "Version", "CVEdit", "TableOverride", "TableOption", "DefaultProps\\Bumper", "DefaultProps\\Decal",
        "DefaultProps\\EMReel", "DefaultProps\\Flasher", "DefaultProps\\Flipper", "DefaultProps\\Gate", "DefaultProps\\HitTarget", "DefaultProps\\Kicker", "DefaultProps\\Light",
        "DefaultProps\\LightSequence", "DefaultProps\\Plunger", "DefaultProps\\Primitive", "DefaultProps\\Ramp", "DefaultProps\\Rubber", "DefaultProps\\Spinner", "DefaultProps\\Wall",
        "DefaultProps\\Target", "DefaultProps\\TextBox", "DefaultProps\\Timer", "DefaultProps\\Trigger", "Defaults\\Camera",
    ];
    let sections: Vec<(&str, Vec<(String, String)>)> = KEYS
        .iter()
        .map(|key| {
            let path = if *key == "Controller" { "Software\\Visual Pinball\\Controller".to_string() } else { format!("Software\\Visual Pinball\\VP10\\{key}") };
            (*key, registry::values(&path))
        })
        .collect();
    registry_ini(&sections)
}

#[cfg(not(windows))]
fn registry_settings() -> String {
    String::new()
}

/// Sections and their values written as VPX writes an ini (mINI's): `[Section]`, then `Name = value`
/// lines; a section with no values left out.
fn registry_ini(sections: &[(&str, Vec<(String, String)>)]) -> String {
    sections
        .iter()
        .filter(|(_, values)| !values.is_empty())
        .map(|(section, values)| {
            let lines: Vec<String> = values.iter().map(|(name, value)| format!("{name} = {value}")).collect();
            format!("[{section}]\r\n{}", lines.join("\r\n"))
        })
        .collect::<Vec<_>>()
        .join("\r\n\r\n")
}

#[cfg(windows)]
mod registry {
    use std::ffi::c_void;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(key: isize, sub_key: *const u16, options: u32, access: u32, result: *mut isize) -> i32;
        fn RegEnumValueW(key: isize, index: u32, name: *mut u16, name_len: *mut u32, reserved: *mut u32, kind: *mut u32, data: *mut c_void, data_len: *mut u32) -> i32;
        fn RegCloseKey(key: isize) -> i32;
    }
    const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
    const KEY_READ: u32 = 0x2_0019;
    const ERROR_NO_MORE_ITEMS: i32 = 259;
    const REG_SZ: u32 = 1;
    const REG_DWORD: u32 = 4;

    /// A key's values under the current user, as VPX reads them: text, and numbers written out;
    /// those of other kinds, and VPX's docking's own two, passed over.
    pub fn values(path: &str) -> Vec<(String, String)> {
        let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let mut key = 0isize;
        // SAFETY: a NUL-ended key name; `key` receives the opened key
        if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for index in 0.. {
            let (mut name, mut data) = ([0u16; 260], [0u8; 4096]);
            let (mut name_len, mut data_len, mut kind) = (name.len() as u32, data.len() as u32, 0u32);
            // SAFETY: each buffer with its length in its own units, as the call takes them
            let r = unsafe { RegEnumValueW(key, index, name.as_mut_ptr(), &mut name_len, std::ptr::null_mut(), &mut kind, data.as_mut_ptr().cast(), &mut data_len) };
            if r == ERROR_NO_MORE_ITEMS {
                break;
            }
            if r != 0 || name_len == 0 {
                continue;
            }
            let name = String::from_utf16_lossy(&name[..name_len as usize]);
            let value = match kind {
                REG_SZ => {
                    let (pairs, _) = data[..data_len as usize].as_chunks::<2>();
                    let units: Vec<u16> = pairs.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&u| u != 0).collect();
                    String::from_utf16_lossy(&units)
                }
                REG_DWORD if data_len >= 4 => u32::from_le_bytes([data[0], data[1], data[2], data[3]]).to_string(),
                _ => continue,
            };
            if value == "Dock Windows" || value == "Dock Settings" {
                continue;
            }
            out.push((name, value));
        }
        // SAFETY: the key opened above
        unsafe { RegCloseKey(key) };
        out
    }
}

/// VPX 10.8.0's settings file, kept apart from 10.8.1's: `VPinballX.ini` beside its program where
/// one is there, else in `%APPDATA%\VPinballX`, the folder above 10.8.1's (its `main.cpp`). The test
/// library's preferences folder (`VPI_VPX_PREFS`) holds its own.
pub fn vpx_1080_settings_path(program_folder: Option<&std::path::Path>) -> Option<PathBuf> {
    if let Some(prefs) = std::env::var_os("VPI_VPX_PREFS").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(prefs).join("10.8.0").join("VPinballX.ini"));
    }
    if let Some(beside) = program_folder.map(|d| d.join("VPinballX.ini")).filter(|p| p.is_file()) {
        return Some(beside);
    }
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("VPinballX").join("VPinballX.ini"))
}

/// Whether a section is the app's own, kept in the app's settings file: `[VPI]`, and the tables
/// let outside their files (`[ScriptsReachOutside]`). Every other is VPX's, kept in VPX's file.
pub fn own_section(section: &str) -> bool {
    ["VPI", "ScriptsReachOutside"].iter().any(|s| s.eq_ignore_ascii_case(section))
}

fn read_text(path: &std::path::Path) -> String {
    std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// The settings as VPX and the app keep them: VPX's from its file, the app's own sections from the
/// app's.
pub fn read_all() -> Vec<(String, String, String)> {
    let vpx = parse_ini(&vpx_settings_text());
    let own = settings_path().map(|p| parse_ini(&read_text(&p))).unwrap_or_default();
    vpx.into_iter().filter(|(s, _, _)| !own_section(s)).chain(own.into_iter().filter(|(s, _, _)| own_section(s))).collect()
}

/// The settings a VPX of `version` reads now, from its file (`vpx_settings_path_for`); before it
/// has made its preferences folder, those its first start brings in (`first_start`).
pub fn vpx_settings_text_for(version: [u16; 4], program_folder: Option<&Path>) -> String {
    if let Some(p) = prefs_override() {
        return read_text(&p.join(VPX_INI));
    }
    vpx_prefs_base().map(|base| settings_text_in(&base, version, program_folder)).unwrap_or_default()
}

/// VPX's settings as VPX reads them now, the followed VPX's.
pub fn vpx_settings_text() -> String {
    let (version, folder) = followed_vpx();
    vpx_settings_text_for(version, folder.as_deref())
}

/// `vpx_settings_text_for` with `base` as `%APPDATA%\VPinballX`.
fn settings_text_in(base: &Path, version: [u16; 4], program_folder: Option<&Path>) -> String {
    if base.join(minor_folder(version)).is_dir() {
        return read_text(&settings_file_in(base, version, program_folder));
    }
    match first_start(base, version, program_folder) {
        FirstStart::InPlace(file) | FirstStart::Brought(file) => read_text(&file),
        FirstStart::Registry => registry_settings(),
    }
}

/// The file the followed VPX's settings are read from now, for a program that reads them itself:
/// its own, or before VPX has made its preferences folder, the one its first start brings in;
/// `None` where they'd come from Windows' registry or there's none.
pub fn vpx_settings_source() -> Option<PathBuf> {
    if let Some(p) = prefs_override() {
        return Some(p.join(VPX_INI)).filter(|f| f.is_file());
    }
    let (version, folder) = followed_vpx();
    let base = vpx_prefs_base()?;
    if base.join(minor_folder(version)).is_dir() {
        return Some(settings_file_in(&base, version, folder.as_deref())).filter(|f| f.is_file());
    }
    match first_start(&base, version, folder.as_deref()) {
        FirstStart::InPlace(file) | FirstStart::Brought(file) => Some(file),
        FirstStart::Registry => None,
    }
}

/// `path` written as `after`, if that differs from what it was (`before`): the file before kept
/// beside it as `<name>.previous.ini`, the new one written beside it and moved into place, so a
/// failed write leaves the old one.
pub fn write_file(path: &std::path::Path, before: &str, after: &str) -> Result<(), String> {
    if after == before {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("The settings folder couldn't be made: {e}"))?;
    }
    if !before.is_empty() && path.exists() {
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        std::fs::write(path.with_file_name(format!("{stem}.previous.ini")), before).map_err(|e| format!("The settings couldn't be kept before changing them: {e}"))?;
    }
    let new = path.with_extension("ini.new");
    std::fs::write(&new, after).map_err(|e| format!("The settings couldn't be written: {e}"))?;
    std::fs::rename(&new, path).map_err(|e| format!("The settings couldn't be written: {e}"))
}

/// Settings changed where each belongs, as `edit` makes changes: VPX's sections in VPX's file (the
/// followed VPX's, its preferences folder made first as VPX's first start makes it), the app's own
/// in the app's (`own_section`), each file written only where it changes.
pub fn save(changes: &[Change]) -> Result<(), String> {
    let (own, vpx): (Vec<Change>, Vec<Change>) = changes.iter().cloned().partition(|c| own_section(&c.section));
    if !vpx.is_empty() {
        vpx_prefs_ready()?;
        let path = vpx_settings_path().ok_or("no APPDATA folder")?;
        let before = read_text(&path);
        write_file(&path, &before, &edit(&before, &vpx))?;
    }
    if !own.is_empty() {
        let path = settings_path().ok_or("no APPDATA folder")?;
        let before = read_text(&path);
        write_file(&path, &before, &edit(&before, &own))?;
    }
    Ok(())
}

/// The library's folders, as first setup keeps them in the settings file: the tables', VPX's scripts' and PinUP's packs' (`[VPI]` `TablesFolder`, `ScriptsFolder`,
/// `PinUPFolder`), and PinMAME's (VPX 5436's own `[Plugin.PinMAME]` `PinMAMEPath`). Only the
/// tables' must be there for a library; the others have their place in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFolders {
    pub tables: PathBuf,
    pub scripts: Option<PathBuf>,
    pub pinmame: Option<PathBuf>,
    pub pinup: Option<PathBuf>,
    /// PinballX's and PinballY's own folders, where they aren't in their usual places.
    pub pinballx: Option<PathBuf>,
    pub pinbally: Option<PathBuf>,
    /// Tables kept in a folder for each game (`[VPI]` `LibraryLayout = game`), VPX 10.8.1's layout,
    /// where a table being added goes; else together in the tables folder.
    pub per_game: bool,
    /// VPX's own folder, where it isn't the one above the tables (`[VPI]` `VPXFolder`).
    pub vpx: Option<PathBuf>,
    /// Future Pinball's folder, where it's named (`[VPI]` `FuturePinballFolder`): else it's looked
    /// for beside VPX's.
    pub future_pinball: Option<PathBuf>,
    /// The line of VPX the tables play in where it isn't the newest found (`[VPI]` `PlayWith`,
    /// `10.8.0`): VPX's release before 10.8.1, kept beside it.
    pub play_with: Option<String>,
}

/// The library's folders the settings name, if they name its tables' folder.
pub fn library_folders(entries: &[(String, String, String)]) -> Option<LibraryFolders> {
    let path = |section: &str, name: &str| get(entries, section, name).map(vpx_trim).filter(|v| !v.is_empty()).map(PathBuf::from);
    Some(LibraryFolders {
        tables: path("VPI", "TablesFolder")?,
        scripts: path("VPI", "ScriptsFolder"),
        pinmame: path("Plugin.PinMAME", "PinMAMEPath"),
        pinup: path("VPI", "PinUPFolder"),
        pinballx: path("VPI", "PinballXFolder"),
        pinbally: path("VPI", "PinballYFolder"),
        per_game: get(entries, "VPI", "LibraryLayout").map(vpx_trim).is_some_and(|v| v.eq_ignore_ascii_case("game")),
        vpx: path("VPI", "VPXFolder"),
        future_pinball: path("VPI", "FuturePinballFolder"),
        play_with: get(entries, "VPI", "PlayWith").map(vpx_trim).filter(|v| !v.is_empty()).map(str::to_string),
    })
}

/// The library's folders as the settings name them, VPX's and the app's (`read_all`).
pub fn library_folders_now() -> Option<LibraryFolders> {
    library_folders(&read_all())
}

/// A screen preset's file (Save as preset on the Screens page, chosen for a
/// table in its settings): `Screen presets\<name>.ini` beside the settings file; None for a name
/// that can't be a file's.
pub fn preset_path(name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 || name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|', '\r', '\n']) || name.starts_with('.') {
        return None;
    }
    Some(settings_path()?.parent()?.join("Screen presets").join(format!("{name}.ini")))
}

/// The screen preset a table's settings or the main ones name (`[VPI]` `ScreenPreset`), the
/// table's own first, and its file's text: the player reads it between the main settings and the
/// table's own.
pub fn preset_named(table: &[(String, String, String)], main: &[(String, String, String)]) -> Option<(String, String)> {
    let name = get(table, "VPI", "ScreenPreset").or_else(|| get(main, "VPI", "ScreenPreset")).map(|n| vpx_trim(n).to_string()).filter(|n| !n.is_empty())?;
    let text = std::fs::read(preset_path(&name)?).ok()?;
    Some((name, String::from_utf8_lossy(&text).into_owned()))
}

/// An ini file's settings: (section, name, value), in file order. Lines
/// starting with `;` are comments.
pub fn parse_ini(text: &str) -> Vec<(String, String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = s.trim().to_string();
        } else if let Some((k, v)) = line.split_once('=')
            && !line.starts_with(';')
        {
            out.push((section.clone(), k.trim().to_string(), v.trim().to_string()));
        }
    }
    out
}

/// A setting, its section and name in any case, when set: the last one read
/// counts (a table's own file read over the main one), and an empty value
/// counts as not set, as VPX's ini reads it.
pub fn get<'a>(entries: &'a [(String, String, String)], section: &str, name: &str) -> Option<&'a str> {
    entries.iter().rev().find(|(s, n, v)| s.eq_ignore_ascii_case(section) && n.eq_ignore_ascii_case(name) && !v.is_empty()).map(|e| e.2.as_str())
}

/// A whole-number setting as VPX reads it (`IntPropertyDef::GetValid`, and an enumeration's):
/// one outside its range counts as not set, so its default holds.
pub fn int(entries: &[(String, String, String)], section: &str, name: &str, min: i32, max: i32) -> Option<i32> {
    get(entries, section, name).and_then(|v| v.parse::<f64>().ok()).filter(|v| *v >= f64::from(min) && *v <= f64::from(max)).map(|v| v as i32)
}

/// The player's windows, as VPX numbers them (`VPXWindowId`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowId {
    Playfield,
    Backglass,
    ScoreView,
    Topper,
}

impl WindowId {
    pub const ALL: [WindowId; 4] = [WindowId::Playfield, WindowId::Backglass, WindowId::ScoreView, WindowId::Topper];

    /// VPX's section and key prefix for the window's settings.
    pub fn names(self) -> (&'static str, &'static str) {
        match self {
            WindowId::Playfield => ("Player", "Playfield"),
            WindowId::Backglass => ("Backglass", "Backglass"),
            WindowId::ScoreView => ("ScoreView", "ScoreView"),
            WindowId::Topper => ("Topper", "Topper"),
        }
    }
}

/// How a window other than the playfield shows (VPX's
/// `RenderOutput::OutputMode`, its settings' order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// No window (VPX's default for all three).
    Disabled,
    /// A window of its own.
    Floating,
    /// Drawn inside the playfield's window.
    Embedded,
}

/// A window's settings (VPX's for one `VPXWindowId`), with VPX's defaults
/// and ranges: a value outside its range counts as not set.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowSettings {
    pub output: Output,
    /// The screen, by the name SDL gives it; none or one that isn't there:
    /// the main screen.
    pub display: String,
    /// Borderless fullscreen (VPX's display mode 1; its BGFX build, like
    /// ours, has no other fullscreen); else a window.
    pub fullscreen: bool,
    /// Where a window sits inside its screen, in pixels; None: centred
    /// (VPX's default for the position).
    pub x: Option<i32>,
    pub y: Option<i32>,
    /// A window's size in pixels (VPX: 16384 unless set, so as big as the
    /// screen takes, its shape kept).
    pub width: i32,
    pub height: i32,
}

/// A window's settings, VPX's defaults for what isn't set.
pub fn window(entries: &[(String, String, String)], id: WindowId) -> WindowSettings {
    let (section, prefix) = id.names();
    let key = |k: &str| format!("{prefix}{k}");
    let output = match id {
        WindowId::Playfield => Output::Floating,
        _ => match int(entries, section, &key("Output"), 0, 2) {
            Some(1) => Output::Floating,
            Some(2) => Output::Embedded,
            _ => Output::Disabled,
        },
    };
    WindowSettings {
        output,
        display: get(entries, section, &key("Display")).unwrap_or_default().to_string(),
        // VPX's BGFX build offers windowed and borderless fullscreen alone (2, its other
        // builds' exclusive fullscreen, isn't among them, so it reads as windowed)
        fullscreen: int(entries, section, &key("FullScreen"), 0, 1) == Some(1),
        x: int(entries, section, &key("WndX"), 0, 16384),
        y: int(entries, section, &key("WndY"), 0, 16384),
        width: int(entries, section, &key("Width"), 0, 16384).unwrap_or(16384),
        height: int(entries, section, &key("Height"), 0, 16384).unwrap_or(16384),
    }
}

/// Whether any of a window's settings is in the file.
pub fn has_window(entries: &[(String, String, String)], id: WindowId) -> bool {
    let (section, prefix) = id.names();
    entries.iter().any(|(s, n, v)| s.eq_ignore_ascii_case(section) && n.len() > prefix.len() && n.get(..prefix.len()).is_some_and(|p| p.eq_ignore_ascii_case(prefix)) && !v.is_empty())
}

/// A screen, as SDL lists it (VPX's `Window::DisplayConfig`).
#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    pub name: String,
    /// Its place on the desktop, in SDL's units.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Pixels per unit (VPX's `pixelDensity`).
    pub density: f32,
    pub primary: bool,
    /// Its picture's physical size in centimetres, the long side first, as the screen reports
    /// it of itself; None where it doesn't.
    pub size_cm: Option<(f32, f32)>,
}

/// Where a window goes: its place on the desktop in SDL's units, and
/// whether it's made borderless fullscreen there.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub fullscreen: bool,
    /// The screen it's on, by name.
    pub display: String,
}

/// Screens of one name told apart, as two of the same model are: the first
/// keeps its name — VPX finds a screen by its name and takes the first, so a
/// settings file naming it finds that one still — and each after it is named
/// with its place among them, `<name> (2)` and on, which only ours knows.
pub fn tell_apart(names: &[String]) -> Vec<String> {
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    names
        .iter()
        .map(|n| {
            let k = seen.entry(n.as_str()).or_insert(0);
            *k += 1;
            if *k == 1 { n.clone() } else { format!("{n} ({k})") }
        })
        .collect()
}

/// The first VPX build to name a screen with its place (`Window::GetDisplays`'s
/// `displayId`), so two of one model are told apart; earlier builds name it alone.
pub const DISPLAY_IDS_FROM: u16 = 5515;

/// The four screen settings VPX reads, by section and name.
pub const DISPLAY_SETTINGS: [(&str, &str); 4] = [("Player", "PlayfieldDisplay"), ("Backglass", "BackglassDisplay"), ("ScoreView", "ScoreViewDisplay"), ("Topper", "TopperDisplay")];

/// A screen as VPX names it from build 5515 on: its name and its place on the
/// desktop, `Main Screen [0, 0]`.
pub fn display_id(name: &str, x: i32, y: i32) -> String {
    format!("{name} [{x}, {y}]")
}

/// A screen setting's name and place, where it names a screen with its place.
pub fn split_display_id(value: &str) -> Option<(&str, i32, i32)> {
    let (name, place) = value.rsplit_once(" [")?;
    let (x, y) = place.strip_suffix(']')?.split_once(',')?;
    Some((name, x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// The screen a window goes on (VPX's `Window::GetDisplayConfig`): the one
/// named — by its name and place, as later builds name it, or by its name
/// alone, as earlier ones did — else the main screen (else the first).
pub fn pick_display<'a>(name: &str, displays: &'a [Display]) -> Option<&'a Display> {
    let primary = || displays.iter().find(|d| d.primary).or(displays.first());
    if let Some((n, x, y)) = split_display_id(name) {
        return displays.iter().find(|d| d.name == n && d.x == x && d.y == y).or_else(primary);
    }
    displays.iter().find(|d| d.name == name).or_else(primary)
}

/// Screen settings naming a screen alone, as VPX wrote them before build 5515,
/// named again with its place for a later build, each the first screen of
/// that name, as the earlier builds took it. Those already naming a place, or
/// a screen not connected, are left as they are.
pub fn display_ids_for(entries: &[(String, String, String)], displays: &[Display]) -> Vec<Change> {
    DISPLAY_SETTINGS
        .iter()
        .filter_map(|(section, key)| {
            let value = get(entries, section, key)?.trim();
            if value.is_empty() || split_display_id(value).is_some() {
                return None;
            }
            let d = displays.iter().find(|d| d.name == value)?;
            Some(Change::set(*section, *key, display_id(&d.name, d.x, d.y)))
        })
        .collect()
}

/// The windows' place and output settings, by window, as VPX names them (`Settings_properties.inl`):
/// the playfield's `Playfield…` in `[Player]`, the others' `<Window>…` in their own sections.
pub const WINDOW_SECTIONS: [(&str, &str); 4] = [("Player", "Playfield"), ("Backglass", "Backglass"), ("ScoreView", "ScoreView"), ("Topper", "Topper")];

/// What each window's settings are named after its prefix.
const WINDOW_KEYS: [&str; 12] = ["Output", "Display", "FullScreen", "WndX", "WndY", "Width", "Height", "Rotation", "FSWidth", "FSHeight", "RefreshRate", "ColorDepth"];

/// The window settings `entries` hold, window by window: its section, its prefix and its own
/// (name, value)s as written; a window with none left out.
pub fn window_settings(entries: &[(String, String, String)]) -> Vec<(&'static str, &'static str, Vec<(String, String)>)> {
    WINDOW_SECTIONS
        .iter()
        .filter_map(|&(section, prefix)| {
            let own: Vec<(String, String)> = entries
                .iter()
                .filter(|(s, k, _)| {
                    s.eq_ignore_ascii_case(section)
                        && k.get(..prefix.len()).is_some_and(|p| p.eq_ignore_ascii_case(prefix))
                        && k.get(prefix.len()..).is_some_and(|rest| WINDOW_KEYS.iter().any(|w| rest.eq_ignore_ascii_case(w)))
                })
                .map(|(_, k, v)| (k.clone(), v.clone()))
                .collect();
            (!own.is_empty()).then_some((section, prefix, own))
        })
        .collect()
}

/// A settings file's text with its window settings taken out, everything else in it as it was.
pub fn without_window_settings(text: &str) -> String {
    let changes: Vec<Change> = window_settings(&parse_ini(text)).into_iter().flat_map(|(section, _, own)| own.into_iter().map(move |(k, _)| Change::remove(section, &k))).collect();
    if changes.is_empty() { text.to_string() } else { edit(text, &changes) }
}

/// A window as `entries` set it, in words, VPX's defaults where they say nothing: `Off`, `Its own
/// window on Wide Screen`, `Inside the playfield, at its top left`; the playfield's own, `Full screen on
/// Main Screen`. `playfield`: the playfield window's size, for where a window inside it sits.
pub fn window_in_words(entries: &[(String, String, String)], section: &str, prefix: &str, playfield: (i32, i32)) -> String {
    let value = |k: &str| get(entries, section, &format!("{prefix}{k}")).map(str::trim).filter(|v| !v.is_empty());
    let screen = || value("Display").map(|d| split_display_id(d).map_or(d, |(name, _, _)| name).to_string()).unwrap_or_else(|| "the main screen".to_string());
    let full = value("FullScreen").and_then(vpx_bool).unwrap_or(false);
    if section.eq_ignore_ascii_case("Player") {
        return if full { format!("Full screen on {}", screen()) } else { format!("A window on {}", screen()) };
    }
    match value("Output").and_then(vpx_int).unwrap_or(0) {
        1 if full => format!("Its own window on {}, full screen", screen()),
        1 => format!("Its own window on {}", screen()),
        2 => {
            let number = |k: &str, default: i32| value(k).and_then(vpx_float).map_or(default, |v| v as i32);
            let (pw, ph) = (playfield.0.max(1), playfield.1.max(1));
            let (x, y) = (number("WndX", 0), number("WndY", 0));
            let (w, h) = (number("Width", 16384).clamp(1, pw), number("Height", 16384).clamp(1, ph));
            if w >= pw * 9 / 10 && h >= ph * 9 / 10 {
                return "Inside the playfield, over all of it".to_string();
            }
            let (cx, cy) = (x + w / 2, y + h / 2);
            let across = if cx < pw / 3 { "left" } else if cx > pw * 2 / 3 { "right" } else { "" };
            let down = if cy < ph / 3 { "top" } else if cy > ph * 2 / 3 { "bottom" } else { "" };
            match (down, across) {
                ("", "") => "Inside the playfield, in its middle".to_string(),
                (d, "") => format!("Inside the playfield, at its {d}"),
                ("", a) => format!("Inside the playfield, at its {a}"),
                (d, a) => format!("Inside the playfield, at its {d} {a}"),
            }
        }
        _ => "Off".to_string(),
    }
}

/// Where a window goes (VPX's `Window::Window`): borderless fullscreen
/// covers its screen; a window takes its size in pixels, shrunk to fit the
/// screen with its shape kept, centred, then moved to its position when
/// that keeps it on the screen. None when there's no screen.
pub fn place(ws: &WindowSettings, displays: &[Display]) -> Option<Placement> {
    let d = pick_display(&ws.display, displays)?;
    let density = if d.density > 0.0 { d.density } else { 1.0 };
    let (mut x, mut y) = (d.x, d.y);
    let (mut w, mut h) = if ws.fullscreen {
        (d.width, d.height)
    } else {
        ((ws.width as f32 / density).round() as i32, (ws.height as f32 / density).round() as i32)
    };
    // constrain the window to the screen
    if w > d.width {
        h = (h * d.width) / w.max(1);
        w = d.width;
    }
    if h > d.height {
        w = (w * d.height) / h.max(1);
        h = d.height;
    }
    x += (d.width - w) / 2;
    y += (d.height - h) / 2;
    // its saved position, inside the screen, when it isn't as big as the screen
    if !ws.fullscreen && (w != d.width || h != d.height) {
        if let Some(sx) = ws.x {
            let xn = (sx as f32 / density).round() as i32;
            if 0 <= xn && xn + w <= d.width {
                x = d.x + xn;
            }
        }
        if let Some(sy) = ws.y {
            let yn = (sy as f32 / density).round() as i32;
            if 0 <= yn && yn + h <= d.height {
                y = d.y + yn;
            }
        }
    }
    Some(Placement { x, y, width: w.max(1), height: h.max(1), fullscreen: ws.fullscreen, display: d.name.clone() })
}

/// VPX's `trim_string`: spaces, tabs and line ends off both ends.
pub fn vpx_trim(s: &str) -> &str {
    s.trim_matches([' ', '\t', '\r', '\n'])
}

/// A whole number as VPX reads one (`try_parse_int`: `from_chars` on the trimmed text): its
/// leading digits, a minus sign before them allowed, whatever follows them ignored.
pub fn vpx_int(s: &str) -> Option<i32> {
    let t = vpx_trim(s);
    let b = t.as_bytes();
    let start = usize::from(b.first() == Some(&b'-'));
    let end = start + b[start..].iter().take_while(|c| c.is_ascii_digit()).count();
    if end == start { None } else { t[..end].parse().ok() }
}

/// A number as VPX reads one (`try_parse_float`: `from_chars` on the trimmed text): its leading
/// decimal number, an exponent included when it has digits, whatever follows ignored.
pub fn vpx_float(s: &str) -> Option<f32> {
    let t = vpx_trim(s);
    let b = t.as_bytes();
    let digits = |from: usize| b[from.min(b.len())..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut i = usize::from(b.first() == Some(&b'-'));
    let whole = digits(i);
    i += whole;
    let mut fraction = 0;
    if b.get(i) == Some(&b'.') {
        fraction = digits(i + 1);
        i += 1 + fraction;
    }
    if whole + fraction == 0 {
        return None;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(b.get(i + 1), Some(b'+' | b'-')));
        let exponent = digits(i + 1 + sign);
        if exponent > 0 {
            i += 1 + sign + exponent;
        }
    }
    t[..i].parse().ok()
}

/// A yes or no as VPX reads one (`LayeredINIPropertyStore::LoadFromINI`): a whole number, yes
/// unless 0; else true, on or enable, or false, off or disable, in any case.
pub fn vpx_bool(s: &str) -> Option<bool> {
    if let Some(v) = vpx_int(s) {
        return Some(v != 0);
    }
    let t = s.trim();
    if ["true", "on", "enable"].iter().any(|w| t.eq_ignore_ascii_case(w)) {
        Some(true)
    } else if ["false", "off", "disable"].iter().any(|w| t.eq_ignore_ascii_case(w)) {
        Some(false)
    } else {
        None
    }
}

/// A camera file's views as settings (VPX's `ImportBackdropPOV` into the table's settings): each
/// field read as `from_chars` reads it, from its first character, as `[TableOverride]`'s view
/// settings; the file's other settings (its smoothing, ambient occlusion, frame limit) aren't
/// the player's settings yet, so they're left. Nothing from text that isn't a POV file.
pub fn pov_entries(text: &str) -> Vec<(String, String, String)> {
    const VIEWS: [(&str, &str); 3] = [("desktop", "ViewDT"), ("fullscreen", "ViewCab"), ("fullsinglescreen", "ViewFSS")];
    // VPX's order: a later field of the same setting wins ("lookat" over "inclination")
    const FIELDS: [(&str, &str); 16] = [
        ("inclination", "LookAt"),
        ("fov", "FOV"),
        ("layback", "Layback"),
        ("lookat", "LookAt"),
        ("rotation", "Rotation"),
        ("xscale", "ScaleX"),
        ("yscale", "ScaleY"),
        ("zscale", "ScaleZ"),
        ("xoffset", "PlayerX"),
        ("yoffset", "PlayerY"),
        ("zoffset", "PlayerZ"),
        ("ViewHOfs", "HOfs"),
        ("ViewVOfs", "VOfs"),
        ("WindowTopZOfs", "WindowTop"),
        ("WindowBottomZOfs", "WindowBot"),
        ("LayoutMode", "Mode"),
    ];
    let Ok(doc) = roxmltree::Document::parse(text) else { return Vec::new() };
    let root = doc.root_element();
    if root.tag_name().name() != "POV" {
        return Vec::new();
    }
    fn child<'a, 'i>(node: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
        node.children().find(|c| c.is_element() && c.tag_name().name() == name)
    }
    let mut out = Vec::new();
    for (view, prefix) in VIEWS {
        let Some(section) = child(root, view) else { continue };
        for (field, setting) in FIELDS {
            let Some(t) = child(section, field).and_then(|n| n.text()) else { continue };
            // `from_chars` reads from the first character: none before the number
            if t.starts_with(char::is_whitespace) {
                continue;
            }
            let value = if setting == "Mode" { vpx_int(t).map(|v| v.to_string()) } else { vpx_float(t).map(|v| v.to_string()) };
            if let Some(value) = value {
                out.push(("TableOverride".to_string(), format!("{prefix}{setting}"), value));
            }
        }
    }
    out
}

/// What a table's settings file starts with when one is first made beside it: the views of its
/// POV file, `<table>.pov`, which VPX imports into a table's settings while it has no file of its
/// own (`PinTable::LoadGameFromFilename`, `ImportBackdropPOV`) and saves with them. Once the file
/// is there the POV is read no more, so its views go into it. Nothing where the file is there
/// already, or there's no POV.
pub fn first_table_settings(table: &std::path::Path) -> Vec<Change> {
    let pov = table.with_extension("pov");
    if table.with_extension("ini").exists() || !pov.is_file() {
        return Vec::new();
    }
    let Ok(bytes) = std::fs::read(&pov) else { return Vec::new() };
    pov_entries(&String::from_utf8_lossy(&bytes)).into_iter().map(|(section, name, value)| Change::set(&section, &name, value)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tables_own_windows_are_found_said_and_taken_out() {
        // as VPX's menu keeps one table's windows, beside its other settings
        let text = "[ScoreView]\r\nScoreViewOutput = 0\r\n\r\n[Topper]\r\nTopperOutput = 2\r\nTopperWndX = 673\r\nTopperWndY = 1400\r\nTopperWidth = 1018\r\nTopperHeight = 573\r\n\r\n[Backglass]\r\nBackglassOutput = 1\r\nBackglassDisplay = Wide Screen [-3440, 0]\r\nPriority.PUP = 1\r\n\r\n[Player]\r\nSoundVolume = 65\r\n";
        let entries = parse_ini(text);
        let found = window_settings(&entries);
        assert_eq!(found.iter().map(|(s, _, own)| (*s, own.len())).collect::<Vec<_>>(), vec![("Backglass", 2), ("ScoreView", 1), ("Topper", 5)]);
        let words = |s: &str, p: &str| window_in_words(&entries, s, p, (3840, 2160));
        assert_eq!(words("Backglass", "Backglass"), "Its own window on Wide Screen");
        assert_eq!(words("ScoreView", "ScoreView"), "Off");
        assert_eq!(words("Topper", "Topper"), "Inside the playfield, at its bottom left");
        assert_eq!(words("Player", "Playfield"), "A window on the main screen");
        // taken out: the windows' own lines alone, the renderer's priority and the sound kept
        let left = without_window_settings(text);
        assert!(window_settings(&parse_ini(&left)).is_empty(), "{left}");
        assert!(left.contains("Priority.PUP = 1") && left.contains("SoundVolume = 65"), "{left}");
        assert_eq!(without_window_settings("[Player]\nSoundVolume = 65\n"), "[Player]\nSoundVolume = 65\n");
    }

    #[test]
    fn settings_saved_where_each_belongs_and_read_from_there() {
        let dir = std::env::temp_dir().join(format!("vpi-settings-split-{}", std::process::id()));
        let (ours, prefs) = (dir.join("Settings.ini"), dir.join("VPX"));
        std::fs::create_dir_all(&prefs).unwrap();
        std::fs::write(&ours, "[VPI]\nTablesFolder = C:\\Tables\n[Player]\nSoundVolume = 50\n").unwrap();
        std::fs::write(prefs.join("VPinballX.ini"), "; VPX's own\n[Player]\nSoundVolume = 100\nMusicVolume = 70\n").unwrap();
        // SAFETY: no other test here reads these
        unsafe {
            std::env::set_var("VPI_SETTINGS", &ours);
            std::env::set_var("VPI_VPX_PREFS", &prefs);
        }
        let saved = save(&[Change::set("Player", "SoundVolume", 80), Change::set("VPI", "VPXFolder", "C:\\VPX")]);
        let all = read_all();
        let (vpx_text, ours_text) = (std::fs::read_to_string(prefs.join("VPinballX.ini")).unwrap(), std::fs::read_to_string(&ours).unwrap());
        let kept = prefs.join("VPinballX.previous.ini").is_file();
        unsafe {
            std::env::remove_var("VPI_SETTINGS");
            std::env::remove_var("VPI_VPX_PREFS");
        }
        let _ = std::fs::remove_dir_all(&dir);
        saved.unwrap();
        // VPX's in VPX's file, its comments kept and the one before beside it; the app's own in the app's
        assert!(vpx_text.contains("; VPX's own") && vpx_text.contains("SoundVolume = 80") && kept, "{vpx_text}");
        assert!(ours_text.contains("VPXFolder = C:\\VPX") && ours_text.contains("SoundVolume = 50"), "{ours_text}");
        // read: VPX's sections from VPX's file, the app's own from the app's
        assert_eq!(get(&all, "Player", "SoundVolume"), Some("80"));
        assert_eq!(get(&all, "Player", "MusicVolume"), Some("70"));
        assert_eq!(get(&all, "VPI", "TablesFolder"), Some("C:\\Tables"));
    }

    /// A scratch `%APPDATA%\VPinballX` of its own for each test, and a VPX program's folder beside it.
    fn scratch(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("vpi-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (base, program) = (dir.join("VPinballX"), dir.join("VPX"));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&program).unwrap();
        (base, program)
    }

    fn put(file: &Path, text: &str) {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }

    const V109: [u16; 4] = [10, 9, 0, 6948];

    #[test]
    fn a_newer_vpx_brings_the_older_versions_folder_in_as_vpx_does() {
        let (base, program) = scratch("newer");
        let older = base.join("10.8");
        put(&older.join(VPX_INI), "[Player]\r\nSoundVolume = 70\r\n[Plugin.VPIAchievements]\r\nEnable = 1\r\n");
        put(&older.join("plugins").join("vpi-achievements").join("plugin.cfg"), "[configuration]\r\n");
        put(&older.join("Cache").join("A table").join("used.txt"), "kept");
        // 10.8.0's, in the folder above, which the newer folder passes over
        put(&base.join(VPX_INI), "[Player]\r\nSoundVolume = 10\r\n");
        // before VPX's first start: what it'll read, nothing made
        let own = base.join("10.9");
        assert_eq!(first_start(&base, V109, Some(&program)), FirstStart::Brought(older.join(VPX_INI)));
        assert_eq!(settings_file_in(&base, V109, Some(&program)), own.join(VPX_INI));
        assert!(settings_text_in(&base, V109, Some(&program)).contains("SoundVolume = 70"));
        assert!(!own.exists());
        // made as VPX makes it: the older folder copied in whole, itself as it was
        assert_eq!(make_prefs(&base, V109, Some(&program)).unwrap(), own);
        assert_eq!(std::fs::read(own.join(VPX_INI)).unwrap(), std::fs::read(older.join(VPX_INI)).unwrap());
        assert!(own.join("plugins").join("vpi-achievements").join("plugin.cfg").is_file());
        assert_eq!(std::fs::read_to_string(own.join("Cache").join("A table").join("used.txt")).unwrap(), "kept");
        assert!(older.join(VPX_INI).is_file());
        // there, it's its own: changed, made again, the change stays
        put(&own.join(VPX_INI), "[Player]\r\nSoundVolume = 55\r\n");
        make_prefs(&base, V109, Some(&program)).unwrap();
        assert!(settings_text_in(&base, V109, Some(&program)).contains("SoundVolume = 55"));
        assert!(std::fs::read_to_string(older.join(VPX_INI)).unwrap().contains("SoundVolume = 70"));
        let _ = std::fs::remove_dir_all(base.parent().unwrap());
    }

    #[test]
    fn the_first_10_8_1_brings_the_folder_vpx_kept_its_settings_in_before() {
        let (base, program) = scratch("first");
        put(&base.join(VPX_INI), "[Player]\r\nSoundVolume = 10\r\n");
        put(&base.join("BAMViewSettings.xml"), "<x/>");
        put(&base.join("Cache").join("T").join("c.bin"), "c");
        let own = base.join("10.8");
        assert_eq!(first_start(&base, VPX_PREFS_VERSION, Some(&program)), FirstStart::Brought(base.join(VPX_INI)));
        make_prefs(&base, VPX_PREFS_VERSION, Some(&program)).unwrap();
        assert!(std::fs::read_to_string(own.join(VPX_INI)).unwrap().contains("SoundVolume = 10"));
        assert!(own.join("BAMViewSettings.xml").is_file() && own.join("Cache").join("T").join("c.bin").is_file());
        // its own folder, inside the one copied, never copied into itself
        assert!(!own.join("10.8").exists());
        let _ = std::fs::remove_dir_all(base.parent().unwrap());
    }

    #[test]
    fn settings_beside_the_program_brought_in_where_no_folder_holds_any() {
        let (base, program) = scratch("beside");
        // as VPX writes its version there: one number, never read as naming 10.9
        put(&program.join(VPX_INI), "[Version]\r\nVPinball = 10816758\r\n[Player]\r\nSoundVolume = 33\r\n");
        assert_eq!(first_start(&base, V109, Some(&program)), FirstStart::Brought(program.join(VPX_INI)));
        make_prefs(&base, V109, Some(&program)).unwrap();
        assert_eq!(std::fs::read(base.join("10.9").join(VPX_INI)).unwrap(), std::fs::read(program.join(VPX_INI)).unwrap());
        assert_eq!(settings_file_in(&base, V109, Some(&program)), base.join("10.9").join(VPX_INI));
        let _ = std::fs::remove_dir_all(base.parent().unwrap());
    }

    #[test]
    fn settings_beside_the_program_naming_its_version_read_there() {
        let (base, program) = scratch("in-place");
        put(&base.join("10.8").join(VPX_INI), "[Player]\r\nSoundVolume = 70\r\n");
        put(&program.join(VPX_INI), "[Version]\r\nVPinball = 10 9 0 6948\r\n[Player]\r\nSoundVolume = 44\r\n");
        assert_eq!(first_start(&base, V109, Some(&program)), FirstStart::InPlace(program.join(VPX_INI)));
        assert_eq!(settings_file_in(&base, V109, Some(&program)), program.join(VPX_INI));
        make_prefs(&base, V109, Some(&program)).unwrap();
        // its folder made, empty, as VPX leaves it: the file beside the program still the one read
        assert!(base.join("10.9").is_dir() && std::fs::read_dir(base.join("10.9")).unwrap().next().is_none());
        assert_eq!(settings_file_in(&base, V109, Some(&program)), program.join(VPX_INI));
        assert!(settings_text_in(&base, V109, Some(&program)).contains("SoundVolume = 44"));
        // naming another version, it's brought in as any other
        assert!(!names_version(&program.join(VPX_INI), VPX_PREFS_VERSION));
        let _ = std::fs::remove_dir_all(base.parent().unwrap());
    }

    #[test]
    fn the_newest_older_version_looked_for_as_vpx_looks() {
        let (base, _) = scratch("older");
        for v in ["10.5", "10.8", "10.9"] {
            std::fs::create_dir_all(base.join(v)).unwrap();
        }
        assert_eq!(older_prefs(&base, V109), Some(base.join("10.8")));
        assert_eq!(older_prefs(&base, [11, 0, 0, 0]), Some(base.join("10.9")));
        assert_eq!(older_prefs(&base, [10, 5, 0, 0]), None);
        assert_eq!(copied_from(&base, [10, 5, 0, 0]), Some(base.clone()));
        let _ = std::fs::remove_dir_all(base.parent().unwrap());
    }

    /// A preferences folder made as a real VPX's first start would make it, to set beside the one
    /// VPX makes: in a copy of `%APPDATA%\VPinballX` (`VPI_PREFS_BASE`), for VPX's version
    /// (`VPI_PREFS_VERSION`, `10.9.0.6955`) and its program's folder (`VPI_VPX_PROGRAM_FOLDER`).
    /// `cargo test -p vpi-settings --lib a_folder_made_as_a_real_vpxs_first_start -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_folder_made_as_a_real_vpxs_first_start() {
        let base = PathBuf::from(std::env::var_os("VPI_PREFS_BASE").expect("VPI_PREFS_BASE names a copy of %APPDATA%\\VPinballX"));
        let numbers: Vec<u16> = std::env::var("VPI_PREFS_VERSION").expect("VPI_PREFS_VERSION").split('.').map(|n| n.parse().unwrap()).collect();
        let version = [numbers[0], numbers[1], numbers.get(2).copied().unwrap_or(0), numbers.get(3).copied().unwrap_or(0)];
        let program = std::env::var_os("VPI_VPX_PROGRAM_FOLDER").map(PathBuf::from);
        println!("its first start: {:?}", first_start(&base, version, program.as_deref()));
        println!("made: {}", make_prefs(&base, version, program.as_deref()).unwrap().display());
    }

    #[test]
    fn the_registrys_settings_written_as_vpx_writes_them() {
        let sections = [("Player", vec![("SoundVolume".to_string(), "70".to_string()), ("BGSet".to_string(), "1".to_string())]), ("Editor", vec![]), ("DefaultProps\\Light", vec![("Color".to_string(), "255".to_string())])];
        assert_eq!(registry_ini(&sections), "[Player]\r\nSoundVolume = 70\r\nBGSet = 1\r\n\r\n[DefaultProps\\Light]\r\nColor = 255");
        assert_eq!(registry_ini(&[]), "");
    }

    #[test]
    fn a_tables_first_settings_take_in_its_pov() {
        let dir = std::env::temp_dir().join(format!("vpi-first-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("A Table.vpx");
        assert!(first_table_settings(&table).is_empty());
        std::fs::write(table.with_extension("pov"), "<POV><desktop><fov>45.5</fov><lookat>52</lookat></desktop></POV>").unwrap();
        let made = parse_ini(&edit("", &first_table_settings(&table)));
        assert_eq!((get(&made, "TableOverride", "ViewDTFOV"), get(&made, "TableOverride", "ViewDTLookAt")), (Some("45.5"), Some("52")));
        // a file of its own there: the POV is read no more, and nothing's taken in
        std::fs::write(table.with_extension("ini"), "").unwrap();
        assert!(first_table_settings(&table).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn screens() -> Vec<Display> {
        // two screens: a 4K screen at 125% (3072x1728 units) as the main one, an ultrawide to its left
        vec![
            Display { name: "ULTRAWIDE".into(), x: -3440, y: 0, width: 3440, height: 1440, density: 1.0, primary: false, size_cm: None },
            Display { name: "4K MAIN".into(), x: 0, y: 0, width: 3072, height: 1728, density: 1.25, primary: true, size_cm: None },
        ]
    }

    #[test]
    fn vpx_names_and_defaults() {
        let e = parse_ini(
            "[Player]\nPlayfieldDisplay = 4K MAIN\nPlayfieldFullScreen = 1\nBGSet = 1\n\n[Backglass]\nBackglassOutput = 1\nBackglassDisplay = ULTRAWIDE\nBackglassWndX = 100\nBackglassWidth = 1280\nBackglassHeight = 1024\n\n[topper]\ntopperoutput = 2\nTopperWidth =\n",
        );
        let p = window(&e, WindowId::Playfield);
        assert_eq!((p.output, p.display.as_str(), p.fullscreen), (Output::Floating, "4K MAIN", true));
        let b = window(&e, WindowId::Backglass);
        assert_eq!((b.output, b.x, b.y, b.width, b.height), (Output::Floating, Some(100), None, 1280, 1024));
        // names in any case; an empty value is VPX's default
        let t = window(&e, WindowId::Topper);
        assert_eq!((t.output, t.width), (Output::Embedded, 16384));
        assert_eq!(window(&e, WindowId::ScoreView).output, Output::Disabled);
        assert!(has_window(&e, WindowId::Backglass) && has_window(&e, WindowId::Topper) && !has_window(&e, WindowId::ScoreView));
    }

    #[test]
    fn windows_go_where_vpx_puts_them() {
        let d = screens();
        let ws = |display: &str, fullscreen, x, y, width, height| WindowSettings { output: Output::Floating, display: display.into(), fullscreen, x, y, width, height };
        // borderless fullscreen covers its screen
        let p = place(&ws("ULTRAWIDE", true, Some(5), Some(5), 640, 480), &d).unwrap();
        assert_eq!((p.x, p.y, p.width, p.height, p.fullscreen), (-3440, 0, 3440, 1440, true));
        // a window's pixels become units on a 125% screen, centred unless placed
        let p = place(&ws("4K MAIN", false, None, None, 1600, 1000), &d).unwrap();
        assert_eq!((p.x, p.y, p.width, p.height), ((3072 - 1280) / 2, (1728 - 800) / 2, 1280, 800));
        let p = place(&ws("4K MAIN", false, Some(250), Some(0), 1600, 1000), &d).unwrap();
        assert_eq!((p.x, p.y), (200, 0));
        // a position that would put it off the screen is left centred
        let p = place(&ws("ULTRAWIDE", false, Some(3000), Some(10), 1280, 1024), &d).unwrap();
        assert_eq!((p.x, p.y, p.width, p.height), (-3440 + (3440 - 1280) / 2, 10, 1280, 1024));
        // VPX's default size, 16384 square, shrinks to the screen's height, its shape kept
        let p = place(&ws("", false, None, None, 16384, 16384), &d).unwrap();
        assert_eq!((p.display.as_str(), p.width, p.height, p.x, p.y), ("4K MAIN", 1728, 1728, (3072 - 1728) / 2, 0));
        // a screen that isn't there: the main one
        assert_eq!(place(&ws("GONE", true, None, None, 1, 1), &d).unwrap().display, "4K MAIN");
        assert!(place(&ws("", true, None, None, 1, 1), &[]).is_none());
    }

    #[test]
    fn out_of_range_values_count_as_not_set() {
        // VPX: a value outside its range is its default, not the nearest end of the range
        let e = parse_ini("[Backglass]\nBackglassOutput = 5\nBackglassWidth = 20000\nBackglassWndX = -4\nBackglassFullScreen = 2\n");
        let b = window(&e, WindowId::Backglass);
        assert_eq!((b.output, b.width, b.x, b.fullscreen), (Output::Disabled, 16384, None, false));
    }

    #[test]
    fn two_screens_of_one_name_told_apart() {
        let names: Vec<String> = ["E24-28", "E24-28", "LG HDR QHD", "E24-28"].map(String::from).to_vec();
        assert_eq!(tell_apart(&names), ["E24-28", "E24-28 (2)", "LG HDR QHD", "E24-28 (3)"]);
        let mut d = screens();
        d[0].name = "E24-28".into();
        d[1].name = "E24-28 (2)".into();
        assert_eq!(pick_display("E24-28 (2)", &d).unwrap().x, d[1].x);
        assert_eq!(pick_display("E24-28", &d).unwrap().x, d[0].x);
    }

    #[test]
    fn screens_named_with_their_place_as_later_builds_name_them() {
        let d = screens();
        assert_eq!(split_display_id("Main Screen [0, 0]"), Some(("Main Screen", 0, 0)));
        assert_eq!(split_display_id("Wide Screen [-3440, 0]"), Some(("Wide Screen", -3440, 0)));
        assert_eq!(split_display_id("Wide Screen"), None);
        // by name and place; a place no screen has falls back to the main one, as VPX does
        assert_eq!(pick_display("ULTRAWIDE [-3440, 0]", &d).unwrap().name, "ULTRAWIDE");
        assert_eq!(pick_display("ULTRAWIDE [100, 0]", &d).unwrap().name, "4K MAIN");
        // the settings naming a screen alone named again with its place, once
        let entries = parse_ini("[Player]\nPlayfieldDisplay = 4K MAIN\n[Backglass]\nBackglassDisplay = ULTRAWIDE\n[ScoreView]\nScoreViewDisplay = Gone\n[Topper]\nTopperDisplay = 4K MAIN [0, 0]\n");
        let changes = display_ids_for(&entries, &d);
        let text = edit("", &changes);
        assert!(text.contains("PlayfieldDisplay = 4K MAIN [0, 0]") && text.contains("BackglassDisplay = ULTRAWIDE [-3440, 0]"), "{text}");
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn the_first_screen_without_a_main_one() {
        let mut d = screens();
        d.iter_mut().for_each(|s| s.primary = false);
        assert_eq!(pick_display("", &d).unwrap().name, "ULTRAWIDE");
    }

    #[test]
    fn numbers_read_as_vpx_reads_them() {
        assert_eq!(vpx_int(" 0.000000"), Some(0));
        assert_eq!(vpx_int("-12abc"), Some(-12));
        assert_eq!(vpx_int("+3"), None);
        assert_eq!(vpx_int("x"), None);
        assert_eq!(vpx_float("0.490196"), Some(0.490196));
        assert_eq!(vpx_float("2.5e2x"), Some(250.0));
        assert_eq!(vpx_float("1e"), Some(1.0));
        assert_eq!(vpx_float(".5"), Some(0.5));
        assert_eq!(vpx_float("-"), None);
        assert_eq!(vpx_bool("0.000000"), Some(false));
        assert_eq!(vpx_bool("Enable"), Some(true));
        assert_eq!(vpx_bool("maybe"), None);
    }
}
