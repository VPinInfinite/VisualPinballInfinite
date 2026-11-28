//! VR's own programs on this PC — their OpenXR runtimes: Meta Horizon Link's, SteamVR's, Virtual
//! Desktop's and any other Windows lists — the one Windows has active, and the one VPX is to play
//! with, chosen in Settings › VR (`[VPI]` `VrProgram`, its manifest's path; none, Windows' own).
//! Windows keeps one VR program for every VR app, its administrator's to change, so the chosen one
//! is handed to VPX for its game alone (`XR_RUNTIME_JSON`, which the OpenXR loader takes over
//! Windows' choice for one program), Windows' left as it is. The headset is checked through it as
//! VPX would find it (`vpi-player --vr-check`, VPX's own OpenXR loader), and what VPX's log says
//! of its last game in VR is read back, and of each program's last. The headset's resolution is
//! kept for each kind of VR program (`[VPI]` `VrResFactor.<kind>`, VPX's `[PlayerVR]` `ResFactor`
//! while it plays with one): each recommends its own size, which VPX multiplies, so a factor
//! chosen for one is never carried over to another. Ours.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// The setting naming the VR program VPX plays with (its manifest), Windows' own where empty.
pub const SETTING: &str = "VrProgram";

/// The kinds of VR program `kind_of` tells apart: the headset's resolution is kept for each.
pub const KINDS: [&str; 6] = ["meta", "steamvr", "virtual-desktop", "pimax", "varjo", "other"];

/// The setting keeping VPX's `[PlayerVR]` `ResFactor` for a kind of VR program (`KINDS`); none, the
/// program's own resolution (VPX's default).
pub fn res_setting(kind: &str) -> &'static str {
    match kind {
        "meta" => "VrResFactor.meta",
        "steamvr" => "VrResFactor.steamvr",
        "virtual-desktop" => "VrResFactor.virtual-desktop",
        "pimax" => "VrResFactor.pimax",
        "varjo" => "VrResFactor.varjo",
        _ => "VrResFactor.other",
    }
}

/// A resolution factor as VPX takes one (`VRDevice::SetupHMD`): past 0.1 and up to 10; anything
/// else, VPX's default among it, draws at the size the VR program recommends.
fn factor(value: &str) -> Option<f32> {
    value.trim().parse::<f32>().ok().filter(|f| *f > 0.1 && *f <= 10.0)
}

/// A VR program found on this PC.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    /// What it calls itself, in words a player knows.
    pub name: String,
    /// Its OpenXR manifest.
    pub manifest: String,
    /// Which program it is: meta, steamvr, virtual-desktop, pimax, varjo, other.
    pub kind: &'static str,
    /// Whether Windows has it active.
    pub active: bool,
}

/// What VPX's log said of its last game in VR.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LastGame {
    pub runtime: Option<String>,
    pub recommended: Option<String>,
    pub maximum: Option<String>,
    pub selected: Option<String>,
}

/// Settings › VR's view of VR on this PC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VrState {
    pub programs: Vec<Program>,
    /// The manifest chosen for VPX, where one is.
    pub chosen: Option<String>,
    pub last: Option<LastGame>,
    /// Each kind of VR program's last game drawn in the headset.
    pub last_for: std::collections::BTreeMap<&'static str, LastGame>,
}

/// A program's kind and name from its manifest's place and its own name.
fn kind_of(manifest: &Path, own: Option<&str>) -> (&'static str, String) {
    let at = manifest.to_string_lossy().to_lowercase();
    let own_lower = own.unwrap_or_default().to_lowercase();
    let is = |w: &str| at.contains(w) || own_lower.contains(w);
    if is("oculus") || is("meta horizon") {
        ("meta", "Meta Horizon Link".into())
    } else if is("steamxr") || is("steamvr") {
        ("steamvr", "SteamVR".into())
    } else if is("virtualdesktop") {
        ("virtual-desktop", "Virtual Desktop".into())
    } else if is("pimax") {
        ("pimax", "Pimax Play".into())
    } else if is("varjo") {
        ("varjo", "Varjo Base".into())
    } else {
        ("other", own.map_or_else(|| manifest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), str::to_string))
    }
}

/// A manifest's own name for its runtime (`runtime.name`), where it has one.
fn own_name(manifest: &Path) -> Option<String> {
    let text = std::fs::read(manifest).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&text).ok()?;
    json.get("runtime")?.get("name")?.as_str().map(str::to_string)
}

/// Every 64-bit VR program found: those Windows lists, the active one, SteamVR's where OpenVR's
/// paths name it (it lists itself only once made active), Meta's in its usual folders.
pub fn programs() -> Vec<Program> {
    let active = registry::active_runtime();
    let mut manifests: Vec<PathBuf> = registry::available_runtimes();
    manifests.extend(active.clone());
    if let Some(steam) = steamvr_manifest() {
        manifests.push(steam);
    }
    for base in [std::env::var_os("ProgramFiles"), Some("C:\\Program Files".into())].into_iter().flatten() {
        let base = PathBuf::from(base);
        manifests.push(base.join(r"Meta Horizon\Support\oculus-runtime\oculus_openxr_64.json"));
        manifests.push(base.join(r"Oculus\Support\oculus-runtime\oculus_openxr_64.json"));
        manifests.push(base.join(r"Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json"));
    }
    let same = |a: &Path, b: &Path| a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy());
    let mut out: Vec<Program> = Vec::new();
    for m in manifests {
        // 64-bit alone, as VPX is
        let lower = m.to_string_lossy().to_lowercase();
        if lower.contains("_32.json") || lower.contains("win32") || !m.is_file() || out.iter().any(|p| same(Path::new(&p.manifest), &m)) {
            continue;
        }
        let (kind, name) = kind_of(&m, own_name(&m).as_deref());
        let is_active = active.as_deref().is_some_and(|a| same(a, &m));
        out.push(Program { name, manifest: m.display().to_string(), kind, active: is_active });
    }
    out.sort_by_key(|p| (!p.active, p.name.to_lowercase()));
    out
}

/// SteamVR's manifest, from OpenVR's own record of where SteamVR is (`openvrpaths.vrpath`).
fn steamvr_manifest() -> Option<PathBuf> {
    let file = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(r"openvr\openvrpaths.vrpath");
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
    let runtime = json.get("runtime")?.as_array()?.first()?.as_str()?;
    let manifest = PathBuf::from(runtime).join("steamxr_win64.json");
    manifest.is_file().then_some(manifest)
}

/// The VR program VPX plays with as chosen, where it's still there; none for Windows' own.
pub fn chosen() -> Option<PathBuf> {
    vpi_settings::get(&vpi_settings::read_all(), "VPI", SETTING).map(str::trim).filter(|v| !v.is_empty()).map(PathBuf::from).filter(|p| p.is_file())
}

/// The kind of VR program VPX plays with: the one chosen, else Windows' own; none where there's
/// neither.
pub fn kind_in_use() -> Option<&'static str> {
    let manifest = chosen().or_else(registry::active_runtime)?;
    Some(kind_of(&manifest, own_name(&manifest).as_deref()).0)
}

/// Whether the VR program VPX plays with — the one chosen, else Windows' own — takes a 32-bit
/// OpenGL program's pictures through its own OpenXR, as BAM's OpenComposite loader sends them:
/// Meta's Link and Virtual Desktop do (each lists `XR_KHR_opengl_enable` and has a 32-bit runtime),
/// so Future Pinball's VR reaches them without SteamVR, which may take the headset through another
/// program's driver; SteamVR's own and the others through SteamVR.
pub fn reaches_through_openxr() -> bool {
    matches!(kind_in_use(), Some("meta" | "virtual-desktop"))
}

/// The chosen VR program's manifest for a 32-bit program, as Future Pinball is (its OpenXR, through
/// OpenComposite, loads a runtime of its own size): the one of the same kind Windows lists for 32-bit
/// programs. None where none is chosen — Windows' own for 32-bit programs then — or the chosen
/// program has none for them.
pub fn chosen_32() -> Option<PathBuf> {
    let chosen = chosen()?;
    let kind = kind_of(&chosen, own_name(&chosen).as_deref()).0;
    registry::available_runtimes_32().into_iter().filter(|m| m.is_file()).find(|m| kind_of(m, own_name(m).as_deref()).0 == kind)
}

/// VPX's VR switch as its settings (`settings`) have it, `[PlayerVR]` `AskToTurnOn`: 0 always, 1
/// where a headset answers as a game starts, 2 off (VPX's default on Windows).
pub fn switch_of(settings: &[(String, String, String)]) -> i32 {
    vpi_settings::get(settings, "PlayerVR", "AskToTurnOn").and_then(vpi_settings::vpx_int).unwrap_or(2)
}

/// The change giving a game the headset's resolution kept for the VR program VPX plays with, where
/// VPX's own (`settings`, as `vpi_settings::read_all` has them) differs: VPX's `[PlayerVR]`
/// `ResFactor` set to it, or taken out for the program's own. None where no program has one kept
/// yet: VPX's own then stands for the one in use.
pub fn res_factor_change(settings: &[(String, String, String)]) -> Option<vpi_settings::Change> {
    res_factor_change_for(settings, kind_in_use()?)
}

/// `res_factor_change` for a VR program of `kind`.
fn res_factor_change_for(settings: &[(String, String, String)], kind: &str) -> Option<vpi_settings::Change> {
    let kept = |k: &str| vpi_settings::get(settings, "VPI", res_setting(k)).filter(|v| factor(v).is_some()).map(str::trim);
    if !KINDS.iter().any(|k| kept(k).is_some()) {
        return None;
    }
    let want = kept(kind);
    let have = vpi_settings::get(settings, "PlayerVR", "ResFactor").and_then(factor);
    if want.and_then(factor) == have {
        return None;
    }
    Some(match want {
        Some(v) => vpi_settings::Change::set("PlayerVR", "ResFactor", v),
        None => vpi_settings::Change::remove("PlayerVR", "ResFactor"),
    })
}

/// Settings › VR's view: the programs, the one chosen, VPX's last game in VR.
#[tauri::command]
pub fn vr_state(shared: tauri::State<'_, crate::library::Shared>) -> VrState {
    let layout = shared.0.lock().unwrap().layout.clone();
    let log = layout.as_ref().and_then(::library::vpx::to_play).and_then(|v| ::library::vpx::log_file(&v, ::library::vpx::prefs_override().as_deref())).and_then(|f| std::fs::read(f).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    VrState { programs: programs(), chosen: chosen().map(|p| p.display().to_string()), last: last_game(&log), last_for: last_games(&log) }
}

/// The VR program VPX plays with chosen (its manifest), or Windows' own (none).
#[tauri::command]
pub fn vr_set_program(manifest: Option<String>) -> Result<(), String> {
    let change = match manifest.filter(|m| !m.trim().is_empty()) {
        Some(m) => vpi_settings::Change::set("VPI", SETTING, m),
        None => vpi_settings::Change::remove("VPI", SETTING),
    };
    vpi_settings::save(&[change])
}

/// What the headset check found.
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Check {
    pub runtime: Option<String>,
    pub version: Option<String>,
    pub headset: Option<String>,
    pub ready: bool,
    pub problem: Option<String>,
}

/// The headset checked through the VR program VPX plays with, as VPX finds it, with VPX's own
/// OpenXR loader beside its program (`vpx_folder`): the player asked, at most a quarter of a minute.
pub fn check_in(vpx_folder: &Path) -> Check {
    let loader = vpx_folder.join("openxr_loader64.dll");
    if !loader.is_file() {
        return Check { problem: Some("VPX's OpenXR library isn't beside it: VR plays in VPX 10.8.1 or newer's BGFX program.".into()), ..Default::default() };
    }
    let player = match crate::screens::player() {
        Ok(p) => p,
        Err(e) => return Check { problem: Some(e), ..Default::default() },
    };
    let mut command = std::process::Command::new(player);
    command.arg("--vr-check").arg("--loader").arg(&loader).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    if let Some(m) = chosen() {
        command.env("XR_RUNTIME_JSON", m);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // a console program: no console window shows
        command.creation_flags(0x0800_0000);
    }
    let Ok(mut child) = command.spawn() else {
        return Check { problem: Some("the player didn't start".into()), ..Default::default() };
    };
    let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < until => std::thread::sleep(std::time::Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                return Check { problem: Some("the VR program didn't answer in time".into()), ..Default::default() };
            }
        }
    }
    let mut out = String::new();
    if let Some(mut s) = child.stdout.take() {
        use std::io::Read as _;
        let _ = s.read_to_string(&mut out);
    }
    serde_json::from_str(out.trim()).unwrap_or_else(|_| Check { problem: Some("the check said nothing".into()), ..Default::default() })
}

/// Settings › VR's Check: the headset, through the VR program VPX plays with.
#[tauri::command]
pub async fn vr_check(app: tauri::AppHandle) -> Result<Check, String> {
    use tauri::Manager;
    let layout = app.state::<crate::library::Shared>().0.lock().unwrap().layout.clone();
    let folder = layout.as_ref().and_then(::library::vpx::to_play).and_then(|v| v.program.parent().map(Path::to_path_buf)).ok_or("No VPX was found for this library: Settings › Library gets one.")?;
    tauri::async_runtime::spawn_blocking(move || check_in(&folder)).await.map_err(|e| e.to_string())
}

/// What VPX's log (`text`) says of its last game in VR: the VR program, and the headset's
/// resolutions — recommended, at most, and the one VPX drew at.
pub fn last_game(text: &str) -> Option<LastGame> {
    let at = text.rfind(RUNTIME_LINE)?;
    Some(game_in(&text[at..]))
}

/// The line VPX's log starts each game in VR with, naming the VR program.
const RUNTIME_LINE: &str = "OpenXR Runtime: ";

/// What VPX's log says of one game in VR, from its VR program's line on (`part`).
fn game_in(part: &str) -> LastGame {
    let after = |what: &str| part.find(what).map(|i| part[i + what.len()..].lines().next().unwrap_or_default().trim().to_string()).filter(|v| !v.is_empty());
    LastGame { runtime: after(RUNTIME_LINE), recommended: after("Headset recommended resolution: "), maximum: after("Headset maximum resolution: "), selected: after("Selected resolution: ") }
}

/// What VPX's log (`text`) says of each kind of VR program's last game drawn in the headset, by
/// the VR program's own name there: a game the headset wasn't there for, which draws nothing,
/// passed over.
pub fn last_games(text: &str) -> std::collections::BTreeMap<&'static str, LastGame> {
    let mut out = std::collections::BTreeMap::new();
    let starts: Vec<usize> = text.match_indices(RUNTIME_LINE).map(|(i, _)| i).collect();
    for (n, &at) in starts.iter().enumerate() {
        let game = game_in(&text[at..starts.get(n + 1).copied().unwrap_or(text.len())]);
        if let (Some(runtime), Some(_)) = (&game.runtime, &game.selected) {
            out.insert(kind_of(Path::new(""), Some(runtime)).0, game);
        }
    }
    out
}

/// The OpenXR loader's own record of the VR programs, in the PC's part of Windows' registry.
#[cfg(windows)]
mod registry {
    use std::ffi::c_void;
    use std::path::PathBuf;

    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
    const KEY_READ: u32 = 0x20019;
    const RRF_RT_REG_SZ: u32 = 0x2;
    const KEY: &str = r"SOFTWARE\Khronos\OpenXR\1";

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(hkey: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
        fn RegOpenKeyExW(hkey: isize, sub_key: *const u16, options: u32, access: u32, out: *mut isize) -> i32;
        fn RegEnumValueW(hkey: isize, index: u32, name: *mut u16, name_len: *mut u32, reserved: *mut u32, kind: *mut u32, data: *mut u8, data_len: *mut u32) -> i32;
        fn RegCloseKey(hkey: isize) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The VR program Windows has active (`ActiveRuntime`), environment strings expanded.
    pub fn active_runtime() -> Option<PathBuf> {
        let (key, name) = (wide(KEY), wide("ActiveRuntime"));
        let mut text = vec![0u16; 1024];
        let mut bytes = (text.len() * 2) as u32;
        // SAFETY: NUL-ended names, a buffer of `bytes` bytes; an expandable text comes back expanded
        if unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), text.as_mut_ptr().cast(), &mut bytes) } != 0 {
            return None;
        }
        let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
        Some(PathBuf::from(String::from_utf16_lossy(&text[..len]))).filter(|p| !p.as_os_str().is_empty())
    }

    /// The VR programs Windows lists (`AvailableRuntimes`: each value's name a manifest).
    pub fn available_runtimes() -> Vec<PathBuf> {
        runtimes_in(&format!(r"{KEY}\AvailableRuntimes"))
    }

    /// The VR programs Windows lists for 32-bit programs, which load a runtime of their own size: in
    /// the registry's 32-bit view (`WOW6432Node`).
    pub fn available_runtimes_32() -> Vec<PathBuf> {
        runtimes_in(r"SOFTWARE\WOW6432Node\Khronos\OpenXR\1\AvailableRuntimes")
    }

    /// The manifests a key of the OpenXR loader's lists, each value's name one.
    fn runtimes_in(key: &str) -> Vec<PathBuf> {
        let key = wide(key);
        let mut open = 0isize;
        // SAFETY: a key of the OpenXR loader's, read only, a handle to fill
        if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, key.as_ptr(), 0, KEY_READ, &mut open) } != 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for i in 0.. {
            let mut name = [0u16; 1024];
            let mut len = name.len() as u32;
            // SAFETY: the open key, a buffer for the value's name; its data not asked for
            if unsafe { RegEnumValueW(open, i, name.as_mut_ptr(), &mut len, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) } != 0 {
                break;
            }
            out.push(PathBuf::from(String::from_utf16_lossy(&name[..len as usize])));
        }
        // SAFETY: the key opened above
        unsafe { RegCloseKey(open) };
        out
    }
}

#[cfg(not(windows))]
mod registry {
    use std::path::PathBuf;
    pub fn active_runtime() -> Option<PathBuf> {
        None
    }
    pub fn available_runtimes() -> Vec<PathBuf> {
        Vec::new()
    }
    pub fn available_runtimes_32() -> Vec<PathBuf> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vr_programs_told_apart_by_their_manifests() {
        assert_eq!(kind_of(Path::new(r"C:\Program Files\Meta Horizon\Support\oculus-runtime\oculus_openxr_64.json"), Some("Oculus OpenXR")).0, "meta");
        assert_eq!(kind_of(Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\SteamVR\steamxr_win64.json"), Some("SteamVR")).1, "SteamVR");
        assert_eq!(kind_of(Path::new(r"C:\Program Files\Virtual Desktop Streamer\OpenXR\virtualdesktop-openxr.json"), Some("VirtualDesktopXR (Bundled)")).0, "virtual-desktop");
        assert_eq!(kind_of(Path::new(r"C:\X\some.json"), Some("Somebody's XR")), ("other", "Somebody's XR".to_string()));
    }

    #[test]
    fn vpxs_last_vr_game_read_from_its_log() {
        let log = "INFO [Main] Starting VPX\n\
            INFO [Main] [VRDevice::VRDevice@326] OpenXR Runtime: Oculus - 1.110.0\n\
            INFO [Main] [VRDevice::SetupHMD@607] Headset recommended resolution: 1680x1760\n\
            INFO [Main] [VRDevice::SetupHMD@608] Headset maximum resolution: 4128x4352\n\
            INFO [Main] [VRDevice::SetupHMD@609] Selected resolution: 2016x2112\n";
        let last = last_game(log).unwrap();
        assert_eq!(last.runtime.as_deref(), Some("Oculus - 1.110.0"));
        assert_eq!(last.maximum.as_deref(), Some("4128x4352"));
        assert_eq!(last.selected.as_deref(), Some("2016x2112"));
        assert_eq!(last_game("INFO no VR here\n"), None);
    }

    #[test]
    fn each_vr_programs_last_game_read_from_vpxs_log() {
        let log = "INFO [Main] [VRDevice::VRDevice@326] OpenXR Runtime: Oculus - 1.208.0\n\
            INFO [Main] [VRDevice::SetupHMD@607] Headset recommended resolution: 1824x1968\n\
            INFO [Main] [VRDevice::SetupHMD@608] Headset maximum resolution: 3648x3936\n\
            INFO [Main] [VRDevice::SetupHMD@609] Selected resolution: 1824x1968\n\
            INFO [Main] [VRDevice::VRDevice@326] OpenXR Runtime: VirtualDesktopXR - 1.1.0\n\
            INFO [Main] [VRDevice::SetupHMD@607] Headset recommended resolution: 3072x3264\n\
            INFO [Main] [VRDevice::SetupHMD@608] Headset maximum resolution: 16384x16384\n\
            INFO [Main] [VRDevice::SetupHMD@609] Selected resolution: 6144x6528\n\
            INFO [Main] [VRDevice::VRDevice@326] OpenXR Runtime: VirtualDesktopXR - 1.1.0\n\
            ERROR [Main] [VRDevice::SetupHMD@490] ERROR: OPENXR: -35 (XR_ERROR_FORM_FACTOR_UNAVAILABLE) Failed to get SystemID.\n\
            INFO [Main] [VRDevice::VRDevice@326] OpenXR Runtime: Oculus - 1.208.0\n\
            INFO [Main] [VRDevice::SetupHMD@607] Headset recommended resolution: 2720x2976\n\
            INFO [Main] [VRDevice::SetupHMD@608] Headset maximum resolution: 5440x5952\n\
            INFO [Main] [VRDevice::SetupHMD@609] Selected resolution: 2720x2976\n";
        let each = last_games(log);
        assert_eq!(each.len(), 2);
        assert_eq!(each["meta"].recommended.as_deref(), Some("2720x2976"));
        assert_eq!(each["meta"].maximum.as_deref(), Some("5440x5952"));
        // the try with no headset there, which drew nothing, passed over
        assert_eq!(each["virtual-desktop"].selected.as_deref(), Some("6144x6528"));
        assert_eq!(last_game(log).unwrap().selected.as_deref(), Some("2720x2976"));
    }

    #[test]
    fn the_headsets_resolution_kept_for_each_vr_program() {
        let s = vpi_settings::parse_ini;
        let said = |c: Option<vpi_settings::Change>| c.map(|c| (c.name, c.value));
        // none kept yet: VPX's own stands for the program in use
        assert_eq!(said(res_factor_change_for(&s("[PlayerVR]\nResFactor = 2\n"), "virtual-desktop")), None);
        // Link's kept at 2, Virtual Desktop's its own: VPX's 2 taken out for a game through Virtual
        // Desktop, kept for one through Link
        let kept = "[VPI]\nVrResFactor.meta = 2\n[PlayerVR]\nResFactor = 2\n";
        assert_eq!(said(res_factor_change_for(&s(kept), "virtual-desktop")), Some(("ResFactor".into(), None)));
        assert_eq!(said(res_factor_change_for(&s(kept), "meta")), None);
        // Link's set again after a game through another; the same factor however VPX writes it
        assert_eq!(said(res_factor_change_for(&s("[VPI]\nVrResFactor.meta = 1.4\n"), "meta")), Some(("ResFactor".into(), Some("1.4".into()))));
        assert_eq!(said(res_factor_change_for(&s("[VPI]\nVrResFactor.meta = 1.4\n[PlayerVR]\nResFactor = 1.400000\n"), "meta")), None);
        // VPX's default, -1, is the program's own
        assert_eq!(said(res_factor_change_for(&s("[VPI]\nVrResFactor.meta = 1.2\n[PlayerVR]\nResFactor = -1\n"), "steamvr")), None);
    }

    /// The VR programs on this PC and the headset's check through the one VPX plays with:
    /// `cargo test -p app vr_on_this_pc -- --ignored --nocapture`. Nothing written; the VR program
    /// may start (SteamVR does when it's the one asked).
    #[test]
    #[ignore]
    fn vr_on_this_pc() {
        for p in programs() {
            println!("{p:?}");
        }
        println!("for 32-bit programs: {:?}; the chosen one's: {:?}", registry::available_runtimes_32(), chosen_32());
        let layout = vpi_settings::library_folders_now().map(|f| ::library::plan::Layout::from_folders(&f)).expect("the library's folders");
        let folder = ::library::vpx::to_play(&layout).and_then(|v| v.program.parent().map(Path::to_path_buf)).expect("VPX's folder");
        println!("{:?}", check_in(&folder));
    }
}
