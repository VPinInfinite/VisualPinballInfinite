//! First setup: where the library lives, kept in the settings
//! file for the app and the player alike (`vpi_settings::LibraryFolders`) — an existing VPX setup's
//! folders, found as VPX's own programs find them or chosen by the user, any of them changed,
//! shared with VPX, PinUP Popper and VPin Studio and nothing moved; or a new library of our own,
//! its folders made. Shown when the settings name no tables' folder.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use vpi_settings::Change;

/// A library's folders, as the screens show them.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folders {
    pub tables: String,
    pub scripts: String,
    pub pinmame: String,
    pub pinup: String,
}

/// What first setup offers: the VPX setup found on this PC, if any; a new library's folder; and
/// the folders the settings name now, if they do (setting up again).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub vpx: Option<Folders>,
    pub fresh: String,
    pub now: Option<Folders>,
}

/// A text value of the registry's classes (`HKEY_CLASSES_ROOT`), any view.
#[cfg(windows)]
fn class_text(key: &str) -> Option<String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(key: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut std::ffi::c_void, size: *mut u32) -> i32;
    }
    const HKEY_CLASSES_ROOT: isize = 0x8000_0000_u32 as i32 as isize;
    // RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ; the 64-bit view, the 32-bit one, or the default
    const TEXT: u32 = 0x2 | 0x4;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let k = wide(key);
    [0x0001_0000u32, 0x0002_0000, 0].into_iter().find_map(|view| {
        let mut buffer = vec![0u16; 1024];
        let mut bytes = (buffer.len() * 2) as u32;
        // SAFETY: a NUL-terminated key, the default value, a buffer of `bytes` bytes
        let r = unsafe { RegGetValueW(HKEY_CLASSES_ROOT, k.as_ptr(), std::ptr::null(), TEXT | view, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut bytes) };
        (r == 0).then(|| {
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            String::from_utf16_lossy(&buffer[..len])
        })
    })
}

#[cfg(not(windows))]
fn class_text(_key: &str) -> Option<String> {
    None
}

/// The program's file a registry command names: its first word, quoted or not.
fn program_of(command: &str) -> PathBuf {
    let c = command.trim();
    match c.strip_prefix('"') {
        Some(rest) => PathBuf::from(rest.split('"').next().unwrap_or_default()),
        None => PathBuf::from(c.split(" /").next().unwrap_or(c).split(" -").next().unwrap_or(c).trim()),
    }
}

/// The folder of a COM class's program, by its name, as registered.
fn registered_folder(progid: &str) -> Option<PathBuf> {
    let clsid = class_text(&format!(r"{progid}\CLSID"))?;
    ["InprocServer32", "LocalServer32"].into_iter().find_map(|server| {
        let file = program_of(&class_text(&format!(r"CLSID\{clsid}\{server}"))?);
        file.is_file().then(|| file.parent().map(Path::to_path_buf)).flatten()
    })
}

/// A folder's child of a name in any case, when there.
fn child(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)))
}

/// The VPX setup on this PC, found as its own programs find each other: VPinMAME's registered
/// controller, VPX's own last tables folder (`[RecentDir]` `LoadDir` in its settings), the usual
/// places; PinUP Popper's registered player, else `PinUPSystem` beside VPX's folder.
pub fn find_vpx() -> Option<Folders> {
    let pinmame = registered_folder("VPinMAME.Controller");
    let vpx_ini = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("VPinballX").join("VPinballX.ini"));
    let load_dir = vpx_ini
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| vpi_settings::get(&vpi_settings::parse_ini(&String::from_utf8_lossy(&b)), "RecentDir", "LoadDir").map(|v| PathBuf::from(vpi_settings::vpx_trim(v))))
        .filter(|d| d.is_dir());
    // VPX's own folder: the one holding its tables' folder
    let vpx = pinmame
        .as_deref()
        .and_then(Path::parent)
        .filter(|d| child(d, "Tables").is_some())
        .map(Path::to_path_buf)
        .or_else(|| load_dir.as_deref().and_then(Path::parent).filter(|d| child(d, "Tables").is_some()).map(Path::to_path_buf))
        .or_else(|| {
            [r"C:\vPinball\VisualPinball", r"C:\Visual Pinball", r"C:\VisualPinball", r"C:\Games\Visual Pinball", r"D:\Visual Pinball", r"D:\vPinball\VisualPinball"]
                .into_iter()
                .map(PathBuf::from)
                .find(|d| child(d, "Tables").is_some())
        })?;
    let tables = child(&vpx, "Tables")?;
    Some(folders_of(&vpx, &tables, pinmame.or_else(|| child(&vpx, "VPinMAME"))))
}

/// A VPX setup's folders, its own folder and its tables' given: its scripts beside them; PinUP's
/// packs from PinUP Popper's registered player, else `PinUPSystem` beside VPX's folder.
fn folders_of(vpx: &Path, tables: &Path, pinmame: Option<PathBuf>) -> Folders {
    let pinup = registered_folder("PinUpPlayer.PinDisplay")
        .or_else(|| vpx.parent().and_then(|p| child(p, "PinUPSystem")))
        .or_else(|| Some(PathBuf::from(r"C:\PinUPSystem")).filter(|d| d.is_dir()))
        .and_then(|d| child(&d, "PUPVideos"));
    let text = |p: Option<PathBuf>| p.map(|p| p.display().to_string()).unwrap_or_default();
    Folders { tables: tables.display().to_string(), scripts: text(child(vpx, "Scripts")), pinmame: text(pinmame), pinup: text(pinup) }
}

/// A VPX setup in a folder the user chose: VPX's own folder, the one holding its tables' folder, or the tables'
/// folder itself; the rest found beside it — its VPinMAME before the one registered.
pub fn look_in(folder: &Path) -> Folders {
    let (vpx, tables) = match child(folder, "Tables") {
        Some(tables) => (folder.to_path_buf(), tables),
        None => (folder.parent().unwrap_or(folder).to_path_buf(), folder.to_path_buf()),
    };
    let pinmame = child(&vpx, "VPinMAME").or_else(|| registered_folder("VPinMAME.Controller"));
    folders_of(&vpx, &tables, pinmame)
}

/// What first setup offers.
pub fn offer() -> Offer {
    let now = vpi_settings::library_folders_now().map(|f| {
        let text = |p: Option<PathBuf>| p.map(|p| p.display().to_string()).unwrap_or_default();
        Folders { tables: f.tables.display().to_string(), scripts: text(f.scripts), pinmame: text(f.pinmame), pinup: text(f.pinup) }
    });
    Offer { vpx: find_vpx(), fresh: r"C:\VPI".into(), now }
}

/// A new library's folders under `root`, laid out as VPX's: its tables, its scripts, PinMAME's
/// ROMs and the rest, PinUP's packs.
pub fn fresh(root: &str) -> Result<Folders, String> {
    let root = PathBuf::from(root.trim());
    if !root.is_absolute() || root.as_os_str().is_empty() {
        return Err("Give the new library's folder as a whole path, such as C:\\VPI.".into());
    }
    let folders = Folders {
        tables: root.join("Tables").display().to_string(),
        scripts: root.join("Scripts").display().to_string(),
        pinmame: root.join("VPinMAME").display().to_string(),
        pinup: root.join("PUPVideos").display().to_string(),
    };
    for dir in [&folders.tables, &folders.scripts, &folders.pinup] {
        std::fs::create_dir_all(dir).map_err(|e| format!("{dir} couldn't be made: {e}"))?;
    }
    for sub in ["roms", "nvram", "altsound", "altcolor"] {
        let dir = Path::new(&folders.pinmame).join(sub);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{} couldn't be made: {e}", dir.display()))?;
    }
    Ok(folders)
}

/// Whether a VPX setup keeps its tables in folders of their own (VPX 10.8.1's layout): every
/// table one folder down, none in the tables folder itself.
pub fn looks_per_game(tables: &Path) -> bool {
    let found = ::library::installed::table_files(tables).unwrap_or_default();
    let together = found.iter().filter(|p| p.parent() == Some(tables)).count();
    !found.is_empty() && together == 0
}

/// Whether tables are kept in a folder for each game, kept in the settings file
/// (`[VPI]` `LibraryLayout`), the file before kept; nothing written for a scratch layout
/// (`VPI_LAYOUT`).
pub fn keep_layout(per_game: bool) -> Result<(), String> {
    if std::env::var_os("VPI_LAYOUT").is_some() {
        return Ok(());
    }
    let path = vpi_settings::settings_path().ok_or("no APPDATA folder")?;
    let before = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let change = if per_game { Change::set("VPI", "LibraryLayout", "game") } else { Change::remove("VPI", "LibraryLayout") };
    crate::screens::write_settings(&path, &before, &vpi_settings::edit(&before, &[change]))
}

/// The library's folders kept in the settings file, with whether its tables are kept in a folder
/// for each game, only what changed, the file before kept; then the library read from them.
pub fn keep(app: &AppHandle, f: &Folders, per_game: bool) -> Result<(), String> {
    if f.tables.trim().is_empty() || !Path::new(f.tables.trim()).is_dir() {
        return Err("The tables' folder isn't there.".into());
    }
    let one_line = |v: &str| !v.contains(['\r', '\n']);
    if ![&f.tables, &f.scripts, &f.pinmame, &f.pinup].iter().all(|v| one_line(v)) {
        return Err("A folder can't be written that way.".into());
    }
    let path = vpi_settings::settings_path().ok_or("no APPDATA folder")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let before = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let set = |section: &str, name: &str, v: &str| if v.trim().is_empty() { Change::remove(section, name) } else { Change::set(section, name, v.trim()) };
    let changes = [
        set("VPI", "TablesFolder", &f.tables),
        set("VPI", "ScriptsFolder", &f.scripts),
        set("Plugin.PinMAME", "PinMAMEPath", &f.pinmame),
        set("VPI", "PinUPFolder", &f.pinup),
        if per_game { Change::set("VPI", "LibraryLayout", "game") } else { Change::remove("VPI", "LibraryLayout") },
    ];
    crate::screens::write_settings(&path, &before, &vpi_settings::edit(&before, &changes))?;
    crate::library::start(app.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_registered_programs_file_read_from_its_command() {
        assert_eq!(program_of(r#""C:\vPinball\PinUPSystem\PinUpPlayer.exe" /automation"#), PathBuf::from(r"C:\vPinball\PinUPSystem\PinUpPlayer.exe"));
        assert_eq!(program_of(r"C:\Visual Pinball\VPinMAME\VPinMAME64.dll"), PathBuf::from(r"C:\Visual Pinball\VPinMAME\VPinMAME64.dll"));
    }

    #[test]
    fn a_vpx_setup_found_in_the_folder_chosen_or_from_its_tables_folder() {
        let dir = std::env::temp_dir().join(format!("vpi-setup-look-{}", std::process::id()));
        for sub in ["Tables", "VPinMAME", "Scripts"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        for chosen in [dir.clone(), dir.join("Tables")] {
            let f = look_in(&chosen);
            assert_eq!(PathBuf::from(&f.tables), dir.join("Tables"));
            assert_eq!((PathBuf::from(&f.pinmame), PathBuf::from(&f.scripts)), (dir.join("VPinMAME"), dir.join("Scripts")));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_librarys_folders_made_and_kept_in_the_settings() {
        let dir = std::env::temp_dir().join(format!("vpi-setup-{}", std::process::id()));
        let f = fresh(&dir.display().to_string()).unwrap();
        assert!(Path::new(&f.tables).is_dir() && Path::new(&f.pinmame).join("roms").is_dir() && Path::new(&f.pinup).is_dir());
        let entries = vpi_settings::parse_ini(&vpi_settings::edit("", &[Change::set("VPI", "TablesFolder", &f.tables), Change::set("Plugin.PinMAME", "PinMAMEPath", &f.pinmame)]));
        let read = vpi_settings::library_folders(&entries).unwrap();
        assert_eq!((read.tables, read.pinmame), (PathBuf::from(&f.tables), Some(PathBuf::from(&f.pinmame))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
