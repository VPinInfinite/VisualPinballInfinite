//! VPX's own add-ons and the classic programs they stand in for. VPX 10.8.1 carries its own
//! (`plugins`), each switched on in its settings (`[Plugin.<id>]` `Enable`) — off on Windows until
//! set, when tables use the classic program installed for the part, found by the object it gives
//! scripts (`VPinMAME.Controller`, `B2S.Server`, `PinUpPlayer.PinDisplay`, `FlexDMD.FlexDMD`,
//! `WMPlayer.OCX`) in Windows' registry, as VPX's add-ons stand in for those objects. The first time
//! the app plays tables in VPX, each part not yet set gets VPX's own where no classic program is
//! there for it and keeps the classic one where it is (`decide_first`); the user switches them in
//! Settings › Library. The classic DOF runs inside the classic B2S Server, so VPX's own backglass
//! add-on goes with VPX's own DOF.
//! AltSound, colour DMDs and the score views go with VPX's own PinMAME, their source. Ours.

use serde::Serialize;

/// A part of what plays a table: VPX's add-on for it, by its ID, and the classic program it stands
/// in for, by the object that program gives scripts and its name.
struct Part {
    id: &'static str,
    name: &'static str,
    classic: Option<(&'static str, &'static str)>,
}

const PARTS: [Part; 6] = [
    Part { id: "PinMAME", name: "Machines’ ROMs", classic: Some(("VPinMAME.Controller", "VPinMAME")) },
    Part { id: "B2S", name: "Backglasses", classic: Some(("B2S.Server", "B2S Server")) },
    Part { id: "PUP", name: "PinUP packs", classic: Some(("PinUpPlayer.PinDisplay", "PinUP Player")) },
    Part { id: "FlexDMD", name: "FlexDMD and UltraDMD", classic: Some(("FlexDMD.FlexDMD", "FlexDMD")) },
    Part { id: "WMP", name: "Media Player’s music", classic: Some(("WMPlayer.OCX", "Windows Media Player")) },
    Part { id: "DOF", name: "Toys (DOF)", classic: Some(("B2S.Server", "DOF through B2S Server")) },
];

/// VPX's add-ons that go with its own PinMAME: what PinMAME's sound and display feed.
// the score views' add-on is `Nixie` in VPX's later builds, `ScoreView` before
const WITH_PINMAME: [&str; 5] = ["AltSound", "Serum", "VNI", "ScoreView", "Nixie"];

/// Whether Windows has an object registered for scripts by this name (its `CLSID` under
/// `HKEY_CLASSES_ROOT`, the view VPX's 64-bit program reads).
#[cfg(windows)]
pub fn registered(prog_id: &str) -> bool {
    use std::ffi::c_void;
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(hkey: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
    }
    const HKEY_CLASSES_ROOT: isize = 0x8000_0000_u32 as i32 as isize;
    const RRF_RT_ANY: u32 = 0xFFFF;
    let key: Vec<u16> = format!(r"{prog_id}\CLSID").encode_utf16().chain([0]).collect();
    // SAFETY: a NUL-ended key; only whether its value is there is asked
    unsafe { RegGetValueW(HKEY_CLASSES_ROOT, key.as_ptr(), std::ptr::null(), RRF_RT_ANY, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) == 0 }
}

#[cfg(not(windows))]
pub fn registered(_prog_id: &str) -> bool {
    false
}

fn enabled(entries: &[(String, String, String)], id: &str) -> Option<bool> {
    vpi_settings::get(entries, &format!("Plugin.{id}"), "Enable").and_then(vpi_settings::vpx_bool)
}

/// One part as Settings › Library shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartView {
    pub id: &'static str,
    pub name: &'static str,
    /// VPX's own on for it.
    pub own: bool,
    /// The classic program for it, where there's one, and whether it's installed.
    pub classic: Option<&'static str>,
    pub classic_installed: bool,
}

#[tauri::command]
pub fn addons() -> Vec<PartView> {
    let entries = vpi_settings::read_all();
    PARTS
        .iter()
        .map(|p| PartView { id: p.id, name: p.name, own: enabled(&entries, p.id).unwrap_or(false), classic: p.classic.map(|c| c.1), classic_installed: p.classic.is_some_and(|c| registered(c.0)) })
        .collect()
}

/// A part switched to VPX's own add-on (`own`) or to its classic program: VPX's own PinMAME takes
/// the add-ons that go with it along; and as the classic DOF runs only inside the classic B2S
/// Server, VPX's own backglass add-on takes VPX's own DOF with it, and the classic DOF the classic
/// B2S Server.
#[tauri::command]
pub fn addon_set(id: String, own: bool) -> Result<(), String> {
    let part = PARTS.iter().find(|p| p.id == id).ok_or("That isn't one of VPX's add-ons.")?;
    let set = |id: &str, on: bool| vpi_settings::Change::set(&format!("Plugin.{id}"), "Enable", i32::from(on));
    let mut changes = vec![set(part.id, own)];
    match (part.id, own) {
        ("PinMAME", _) => changes.extend(WITH_PINMAME.iter().map(|w| set(w, own))),
        ("B2S", true) => changes.push(set("DOF", true)),
        ("DOF", false) => changes.push(set("B2S", false)),
        _ => {}
    }
    vpi_settings::save(&changes)
}

/// The first time the app plays tables in VPX: each part whose add-on isn't set yet gets VPX's own
/// where no classic program is installed for it, and keeps the classic one where one is; the
/// add-ons that go with VPX's own PinMAME with it. Only what isn't set is written.
pub fn decide_first() -> Result<(), String> {
    let entries = vpi_settings::read_all();
    let mut changes: Vec<vpi_settings::Change> = Vec::new();
    for p in &PARTS {
        if enabled(&entries, p.id).is_some() {
            continue;
        }
        let mut own = !p.classic.is_some_and(|c| registered(c.0));
        // the classic DOF only runs inside the classic B2S Server: with VPX's own backglass
        // add-on (decided just before, or already), VPX's own DOF
        if p.id == "DOF" {
            own |= changes.iter().any(|c| c.section == "Plugin.B2S" && c.value.as_deref() == Some("1")) || enabled(&entries, "B2S") == Some(true);
        }
        changes.push(vpi_settings::Change::set(&format!("Plugin.{}", p.id), "Enable", i32::from(own)));
        if p.id == "PinMAME" {
            changes.extend(WITH_PINMAME.iter().filter(|w| enabled(&entries, w).is_none()).map(|w| vpi_settings::Change::set(&format!("Plugin.{w}"), "Enable", i32::from(own))));
        }
    }
    if changes.is_empty() { Ok(()) } else { vpi_settings::save(&changes) }
}

/// The folders VPX's own add-ons look in when nothing is beside a table, set where VPX's settings
/// leave them empty: VPX's PinMAME add-on finds a machine's ROMs, memory and maps beside the table
/// (`pinmame\roms`), else only in the folder its setting names — with none, a table kept beside
/// others can't start its machine at all; its Serum, VNI and AltSound add-ons look beside the table,
/// then only where their own setting says — not in PinMAME's folder, where colourings (`altcolor`)
/// and AltSound packs (`altsound`) are kept for the classic VPinMAME — and its PinUP add-on in the
/// folder holding `PUPVideos` its setting names. What isn't there isn't written.
pub fn set_folders(layout: &::library::plan::Layout) -> Result<(), String> {
    let entries = vpi_settings::read_all();
    let empty = |section: &str, name: &str| vpi_settings::get(&entries, section, name).is_none_or(|v| v.trim().is_empty());
    let mut changes = Vec::new();
    if empty("Plugin.PinMAME", "PinMAMEPath") {
        // the app's own settings named it before VPX's kept the app's library (first setup wrote it
        // there), else the library's PinMAME folder
        let before = vpi_settings::settings_path()
            .and_then(|p| std::fs::read(p).ok())
            .map(|b| vpi_settings::parse_ini(&String::from_utf8_lossy(&b)))
            .and_then(|own| vpi_settings::get(&own, "Plugin.PinMAME", "PinMAMEPath").map(|v| std::path::PathBuf::from(vpi_settings::vpx_trim(v))));
        if let Some(dir) = before.into_iter().chain([layout.vpinmame.clone()]).find(|d| d.join("roms").is_dir()) {
            changes.push(vpi_settings::Change::set("Plugin.PinMAME", "PinMAMEPath", dir.display().to_string()));
        }
    }
    for (section, name, folder) in [("Plugin.Serum", "SerumPath", "altcolor"), ("Plugin.VNI", "VniPath", "altcolor"), ("Plugin.AltSound", "Folder", "altsound")] {
        let dir = layout.vpinmame.join(folder);
        if dir.is_dir() && empty(section, name) {
            changes.push(vpi_settings::Change::set(section, name, dir.display().to_string()));
        }
    }
    if layout.pup_videos.is_dir()
        && let Some(pinup) = layout.pup_videos.parent()
        && empty("Plugin.PUP", "PUPFolder")
    {
        changes.push(vpi_settings::Change::set("Plugin.PUP", "PUPFolder", pinup.display().to_string()));
    }
    if changes.is_empty() { Ok(()) } else { vpi_settings::save(&changes) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_object_never_registered_isnt_found() {
        assert!(!super::registered("VisualPinballInfinite.NoSuchObject"));
    }
}
