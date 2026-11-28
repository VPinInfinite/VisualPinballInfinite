//! Settings › Library's VPX: the VPX the library plays its tables in (`library::vpx::find`), its
//! program and version; VPX's folder kept where it isn't the one above the tables (`[VPI]`
//! `VPXFolder`); and VPX 10.8.1 got: the build the app is made for (`library::vpx::made_for`, one of
//! VPX's automatic builds), else, where GitHub no longer keeps it, VPX's newest automatic build of
//! 10.8.1 (`library::vpx::newest_automatic`); downloaded in the background, checked against the
//! checksum GitHub keeps of it, and put in VPX's folder — the found VPX's, else the one above the
//! tables — each file it replaces kept in the data folder (`VPX\Before <tag>`), with what's needed
//! to put the VPX before back (`VPX\put back.json`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::library::{self, Shared};

/// The line of VPX the app plays tables in, and gets.
const LINE: &str = ::library::vpx::MADE_FOR_LINE;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub program: String,
    pub version: String,
    /// Whether the app plays tables in it: 10.8.0 or newer.
    pub plays: bool,
    /// Whether it carries VPX's own add-ons and runs quietly: 10.8.1 or newer.
    pub addons: bool,
    /// Its line, as `[VPI]` `PlayWith` names it (`10.8.0`).
    pub line: String,
}

impl Found {
    fn of(v: &::library::vpx::Vpx) -> Found {
        Found { program: v.program.display().to_string(), version: version_text(v.version), plays: v.plays(), addons: v.has_addons(), line: v.line() }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The VPX that plays the tables.
    pub found: Option<Found>,
    /// Each line of VPX found for the library, its newest program, the newest line first.
    pub installed: Vec<Found>,
    /// The line the settings choose to play with, where they choose one (`[VPI]` `PlayWith`).
    pub play_with: Option<String>,
    /// The folder Get VPX puts it in.
    pub get_into: Option<String>,
    /// The VPX that putting back brings back, where one was replaced: its version, or "none".
    pub put_back: Option<String>,
    /// The VPX build the app is made for, as Get VPX brings it: `10.9.0 (build 7001)`.
    pub made_for: String,
    /// That build's line: `10.9.0`.
    pub made_for_line: String,
    /// Whether the library's newest VPX with add-ons is older than that build.
    pub older: bool,
}

/// What putting the VPX before back needs.
#[derive(Serialize, Deserialize)]
struct PutBack {
    folder: PathBuf,
    aside: PathBuf,
    /// The VPX it replaced, by its version, where there was one.
    before: Option<String>,
}

fn vpx_folder() -> Option<PathBuf> {
    Some(library::data_folder()?.join("VPX"))
}

fn put_back_file() -> Option<PathBuf> {
    Some(vpx_folder()?.join("put back.json"))
}

/// Each VPX put in, oldest first, with what putting it back needs: the file once held one alone.
fn read_put_backs() -> Vec<PutBack> {
    let Some(bytes) = put_back_file().and_then(|f| std::fs::read(f).ok()) else { return Vec::new() };
    serde_json::from_slice::<Vec<PutBack>>(&bytes).or_else(|_| serde_json::from_slice::<PutBack>(&bytes).map(|p| vec![p])).unwrap_or_default()
}

fn write_put_backs(records: &[PutBack]) -> Result<(), String> {
    let file = put_back_file().ok_or("no APPDATA folder")?;
    if records.is_empty() {
        return match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    std::fs::write(&file, serde_json::to_vec_pretty(records).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// A VPX build put into `folder` from its zip: the files it replaces, and the add-ons of the VPX
/// before that it drops, kept aside to put back (`Before <tag>`), remembered after any put in
/// before it; then its add-ons, and the folders they look in, where the settings don't say yet.
pub(crate) fn put_in(zip: &Path, tag: &str, folder: &Path, before: Option<String>, layout: &::library::plan::Layout) -> Result<(), String> {
    let aside = vpx_folder().ok_or("no APPDATA folder")?.join(format!("Before {tag}"));
    if aside.exists() {
        return Err(format!("VPX {tag} was put in before, and what it replaced is still kept: {}", aside.display()));
    }
    let mut records = read_put_backs();
    let brought_before = records.last().filter(|r| r.folder == folder).map(|r| ::library::vpx::came_in(&r.aside)).unwrap_or_default();
    ::library::vpx::install(zip, folder, &aside, &brought_before)?;
    records.push(PutBack { folder: folder.to_path_buf(), aside, before });
    write_put_backs(&records)?;
    // VPX's settings kept for the VPX that plays now, which may be this one
    ::library::vpx::follow(layout);
    // our add-on as this app carries it, at once, where this VPX loads it: one left from before
    // might not know this VPX's add-on interface, and VPX loads it in every game
    if let Some(vpx) = ::library::vpx::newest_in(&[folder.to_path_buf()]).filter(::library::vpx::Vpx::has_addons) {
        ::library::vpx_addon::put_in_for(&vpx, ::library::vpx::prefs_override().as_deref())?;
    }
    crate::addons::decide_first()?;
    crate::addons::set_folders(layout)
}

fn version_text(v: [u16; 4]) -> String {
    format!("{}.{}.{} (build {})", v[0], v[1], v[2], v[3])
}

/// Said where GitHub no longer keeps the build the app is made for and VPX's newest automatic
/// builds are of a newer VPX than this version of the app is made for: a newer version of the app
/// brings the VPX it's made for.
fn made_for_gone() -> String {
    format!(
        "VPX {LINE} build {}, the build this version of Visual Pinball Infinite is made for, is no longer kept on VPX's GitHub, and VPX's newer builds are a newer VPX this version isn't made for. A new version of Visual Pinball Infinite gets the VPX it's made for: Settings › General › Look for a new version.",
        ::library::vpx::MADE_FOR
    )
}

/// Where Get VPX 10.8.1 puts it: the found VPX's folder, else the one above the tables.
fn get_into(layout: &::library::plan::Layout) -> Option<PathBuf> {
    ::library::vpx::find(layout).and_then(|v| v.program.parent().map(Path::to_path_buf)).or_else(|| layout.vpx_folder.clone()).or_else(|| layout.tables.parent().map(Path::to_path_buf))
}

#[tauri::command]
pub fn vpx_status(shared: State<'_, Shared>) -> Status {
    let layout = shared.0.lock().unwrap().layout.clone();
    // a VPX put in VPX's folder meanwhile followed
    if let Some(l) = &layout {
        ::library::vpx::follow(l);
    }
    let found = layout.as_ref().and_then(::library::vpx::to_play);
    let mut installed: Vec<Found> = Vec::new();
    for v in layout.as_ref().map(::library::vpx::all_for).unwrap_or_default().iter().filter(|v| v.plays()) {
        if !installed.iter().any(|f| f.line == v.line()) {
            installed.push(Found::of(v));
        }
    }
    let put_back = read_put_backs().pop().map(|p| p.before.unwrap_or_else(|| "none".into()));
    let newest_1081 = layout.as_ref().map(::library::vpx::all_for).unwrap_or_default().iter().filter(|v| v.has_addons()).map(|v| v.version).max();
    Status {
        found: found.as_ref().map(Found::of),
        installed,
        play_with: layout.as_ref().and_then(|l| l.play_with.clone()),
        get_into: layout.as_ref().and_then(get_into).map(|p| p.display().to_string()),
        put_back,
        made_for: version_text(::library::vpx::MADE_FOR_VERSION),
        made_for_line: LINE.into(),
        older: newest_1081.is_some_and(::library::vpx::older_than_made_for),
    }
}

/// VPX's folder kept, a folder holding one of VPX's programs; then the library read again.
#[tauri::command]
pub fn vpx_set_folder(app: AppHandle, folder: String) -> Result<(), String> {
    let found = ::library::vpx::newest_in(&[PathBuf::from(&folder)]).ok_or("No VPX program is in that folder (VPinballX_BGFX64.exe, VPinballX_GL64.exe or VPinballX64.exe).")?;
    if !found.plays() {
        return Err(format!("The VPX in that folder is {}: tables play in VPX 10.8.0 or newer.", version_text(found.version)));
    }
    vpi_settings::save(&[vpi_settings::Change::set("VPI", "VPXFolder", folder.trim())])?;
    library::start(app);
    Ok(())
}

/// The line of VPX the tables play in kept (`[VPI]` `PlayWith`), one found for the library; the
/// newest's chosen by taking the setting out, so a newer one got later plays. Then the library read
/// again, its layout with it.
#[tauri::command]
pub fn vpx_play_with(app: AppHandle, shared: State<'_, Shared>, line: String) -> Result<(), String> {
    let layout = shared.0.lock().unwrap().layout.clone().ok_or("The library isn't read yet.")?;
    let all = ::library::vpx::all_for(&layout);
    let newest = all.first().ok_or("VPX wasn't found for this library.")?;
    let chosen = all.iter().find(|v| v.line() == line.trim() && v.plays()).ok_or_else(|| format!("VPX {line} wasn't found for this library."))?;
    let change = if chosen.line() == newest.line() { vpi_settings::Change::remove("VPI", "PlayWith") } else { vpi_settings::Change::set("VPI", "PlayWith", chosen.line()) };
    vpi_settings::save(&[change])?;
    library::start(app.clone());
    let _ = app.emit("vpx-changed", ());
    Ok(())
}

/// VPX 10.8.1 got in the background, told in the tasks bar: the build the app is made for
/// downloaded (else, where GitHub no longer keeps it, VPX's newest automatic build of 10.8.1),
/// checked, and put in VPX's folder (`get_into`), the files it replaces kept to put back; then the
/// library read again.
#[tauri::command]
pub fn vpx_get(app: AppHandle) -> Result<(), String> {
    let layout = app.state::<Shared>().0.lock().unwrap().layout.clone().ok_or("The library isn't read yet.")?;
    let folder = get_into(&layout).ok_or("There's no folder to put VPX in.")?;
    let before = ::library::vpx::newest_in(std::slice::from_ref(&folder)).map(|v| version_text(v.version));
    let data = vpx_folder().ok_or("no APPDATA folder")?;
    let task = crate::tasks::begin(&app, format!("Getting VPX {LINE}"), 3);
    std::thread::Builder::new()
        .name("vpx-get".into())
        .spawn(move || {
            let got = (|| -> Result<(), String> {
                let made_for = ::library::vpx::made_for();
                task.step(0, Some(format!("Getting VPX {} (build {})", LINE, ::library::vpx::MADE_FOR)));
                let mb = |b: u64| b / 1_000_000;
                let fetch = |release: &::library::vpx::Release| {
                    ::library::vpx::download(release, &data.join("Downloads"), &|got| task.step(1, Some(format!("Downloading {} · {} of {} MB", release.tag, mb(got), mb(release.size)))))
                };
                let (release, zip) = match fetch(&made_for) {
                    Ok(zip) => (made_for, zip),
                    // GitHub keeps an automatic build 90 days: where it no longer keeps this one,
                    // VPX's newest automatic build of the line; merely out of reach, it's said so
                    Err(::library::vpx::NotKept::Unreachable(why)) => {
                        if ::library::vpx::made_for_kept() != Some(false) {
                            return Err(why);
                        }
                        task.step(0, Some(format!("Build {} is no longer kept on GitHub: getting VPX's newest automatic build of {LINE} instead", ::library::vpx::MADE_FOR)));
                        let release = ::library::vpx::newest_automatic(LINE)?.ok_or_else(made_for_gone)?;
                        let zip = fetch(&release).map_err(|e| e.to_string())?;
                        (release, zip)
                    }
                    Err(e) => return Err(e.to_string()),
                };
                task.step(2, Some(format!("Putting VPX {} in {}", release.tag, folder.display())));
                put_in(&zip, &release.tag, &folder, before, &layout)?;
                let _ = std::fs::remove_file(&zip);
                Ok(())
            })();
            task.step(3, None);
            match got {
                Ok(()) => {
                    let _ = app.emit("vpx-changed", ());
                    library::start(app.clone());
                }
                Err(e) => {
                    let _ = app.emit("vpx-changed", e);
                }
            }
            drop(task);
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// What the settings the app kept for its own player bring to VPX's: each setting in the app's own
/// file under VPX's names (its screens, sound, controls, picture, add-ons), where VPX's file says
/// otherwise — none once they've been brought (`[VPI]` `SettingsBroughtToVPX`).
fn settings_to_bring() -> Vec<vpi_settings::Change> {
    let read = |p: Option<PathBuf>| p.and_then(|p| std::fs::read(p).ok()).map(|b| vpi_settings::parse_ini(&String::from_utf8_lossy(&b))).unwrap_or_default();
    let ours = read(vpi_settings::settings_path());
    if vpi_settings::get(&ours, "VPI", "SettingsBroughtToVPX").and_then(vpi_settings::vpx_bool) == Some(true) {
        return Vec::new();
    }
    let vpx = vpi_settings::parse_ini(&vpi_settings::vpx_settings_text());
    ours.iter()
        .filter(|(s, _, _)| !vpi_settings::own_section(s))
        .filter(|(s, k, v)| vpi_settings::get(&vpx, s, k).map(vpi_settings::vpx_trim) != Some(vpi_settings::vpx_trim(v)))
        .map(|(s, k, v)| vpi_settings::Change::set(s, k, vpi_settings::vpx_trim(v)))
        .collect()
}

/// How many settings the app kept for its own player aren't in VPX's yet.
#[tauri::command]
pub fn vpx_settings_to_bring() -> usize {
    settings_to_bring().len()
}

/// The settings the app kept for its own player brought into VPX's, VPX's file kept before
/// (`VPinballX.previous.ini`), and noted as brought; the app's own file otherwise as it was.
#[tauri::command]
pub fn vpx_bring_settings() -> Result<usize, String> {
    let changes = settings_to_bring();
    vpi_settings::save(&changes)?;
    vpi_settings::save(&[vpi_settings::Change::set("VPI", "SettingsBroughtToVPX", 1)])?;
    Ok(changes.len())
}

/// The VPX before put back where Get VPX 10.8.1 replaced it: the files it brought to the Recycle
/// Bin, those it replaced moved back; then the library read again.
#[tauri::command]
pub fn vpx_put_back(app: AppHandle) -> Result<(), String> {
    // the latest put in first, the one before it then next to put back
    let mut records = read_put_backs();
    let record = records.pop().ok_or("There's no VPX to put back.")?;
    ::library::vpx::put_back(&record.folder, &record.aside, &|f| ::library::install::to_recycle_bin(f))?;
    write_put_backs(&records)?;
    library::start(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A VPX build put into the library's VPX folder from a zip of it, as Get VPX 10.8.1 puts one
    /// in, for one of VPX's automatic builds downloaded by hand: `VPI_VPX_ZIP=<zip>
    /// VPI_VPX_TAG=<its tag, as v10.8.1-6681-934be537e> cargo test --release -p app
    /// put_in_a_vpx_build_from_its_zip -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn put_in_a_vpx_build_from_its_zip() {
        let zip = PathBuf::from(std::env::var_os("VPI_VPX_ZIP").expect("VPI_VPX_ZIP"));
        let tag = std::env::var("VPI_VPX_TAG").expect("VPI_VPX_TAG");
        let layout = ::library::plan::Layout::from_folders(&vpi_settings::library_folders_now().expect("a library"));
        let folder = get_into(&layout).expect("VPX's folder");
        let before = ::library::vpx::newest_in(std::slice::from_ref(&folder)).map(|v| version_text(v.version));
        put_in(&zip, &tag, &folder, before.clone(), &layout).unwrap();
        let now = ::library::vpx::newest_in(std::slice::from_ref(&folder)).map(|v| version_text(v.version));
        println!("{}: {before:?} -> {now:?}", folder.display());
        // the screen settings named as this VPX reads them, the player beside the app listing the screens
        crate::screens::note_vpx(&layout);
        let player = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("target").join("release").join("vpi-player.exe");
        println!("screen settings named again: {}", crate::screens::fit_screen_names_with(Some(&player), Some(&layout.tables)).unwrap());
    }
}
