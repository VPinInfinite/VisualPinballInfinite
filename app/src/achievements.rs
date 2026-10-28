//! Achievements, the app's side: the packs kept in the data folder (`Achievements\packs\<pack
//! ID>\<version>\`, each a pack's `achievements.json` and its pictures), added from a file the user
//! chooses (a pack's zip or its `achievements.json`, checked first) and removed to the Recycle Bin;
//! which packs fit a table (its VPS game, and the tables and ROM sets a pack names); whether a table
//! plays hardcore or softcore (softcore — `practice` in the records — where its physics, script,
//! options or difficulty aren't its author's, or PinMAME's cheat mode is on; Play Hardcore leaves
//! them out for one game); what's earned (`Achievements\earned.json`, which the player writes); and
//! the file handed to the player at PLAY (`VPI_ACHIEVEMENTS`). A pack's pictures are served by the
//! pictures protocol (`h/<pack ID>/<achievement ID or "pack">`).

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};
use vpi_achievements::earned::Earned;
use vpi_achievements::pack::{Pack, version_key};

use crate::library::{self, Shared, Table};

fn folder() -> Option<PathBuf> {
    Some(library::data_folder()?.join("Achievements"))
}

pub fn earned_file() -> Option<PathBuf> {
    Some(folder()?.join("earned.json"))
}

/// Every pack kept, its newest version of each: its folder and what it says.
pub fn installed() -> Vec<(PathBuf, Pack)> {
    let Some(root) = folder().map(|f| f.join("packs")) else { return Vec::new() };
    let mut out: Vec<(PathBuf, Pack)> = Vec::new();
    for pack_dir in std::fs::read_dir(&root).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let newest = std::fs::read_dir(&pack_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter_map(|dir| Pack::read(&dir).ok().map(|p| (dir, p)))
            .max_by_key(|(_, p)| version_key(&p.pack.version));
        out.extend(newest);
    }
    out.sort_by(|a, b| a.1.pack.name.to_lowercase().cmp(&b.1.pack.name.to_lowercase()));
    out
}

/// Whether a pack is made for a table: its VPS game (else its name, maker and year), the VPS
/// table files it names if it names any, and the ROM sets it names if it names any.
pub fn fits(st: &library::State, t: &Table, pack: &Pack) -> bool {
    let f = &pack.made_for;
    let game = match (&f.game.vps, &t.on_vps.game) {
        (Some(want), Some(have)) => want == have,
        _ => {
            let view = library::view(st, t);
            let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
            same(&f.game.name, &view.name) && f.game.year.is_none_or(|y| view.year == Some(y)) && f.game.maker.as_deref().is_none_or(|m| view.maker.as_deref().is_some_and(|v| same(m, v)))
        }
    };
    let tables = f.tables.iter().filter_map(|x| x.vps.as_deref()).collect::<Vec<_>>();
    let table = tables.is_empty() || t.on_vps.table_file.as_deref().is_some_and(|id| tables.contains(&id));
    let rom = f.roms.is_empty() || t.holds.rom_set.as_deref().is_some_and(|r| f.roms.iter().any(|x| x.eq_ignore_ascii_case(r)));
    game && table && rom
}

/// Why a table plays practice rather than hardcore, if it does: its physics or script not its
/// author's, or PinMAME's cheat mode on for its game.
pub fn practice_because(st: &library::State, t: &Table) -> Vec<String> {
    let mut why = Vec::new();
    if let Some(p) = &t.holds.physics
        && !p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("autovpp.vpp"))
    {
        why.push("its physics are changed (its own physics file)".to_string());
    }
    if let Some(set) = t.read.physics_set
        && physics_set_changed(set)
    {
        why.push(format!("it plays VPX's physics set {set}, which you've changed"));
    }
    if t.holds.script.is_some() || st.user.script_edits.contains_key(&library::key(&t.path)) {
        why.push("its script is edited".to_string());
    }
    if let Some(rom) = &t.holds.rom_set
        && cheat_on(rom)
    {
        why.push("PinMAME's cheat mode is on for its game".to_string());
    }
    // its own settings: its script's options, or its difficulty, changed from its author's
    if let Ok(bytes) = std::fs::read(t.path.with_extension("ini")) {
        let entries = vpi_settings::parse_ini(&String::from_utf8_lossy(&bytes));
        if entries.iter().any(|(s, _, v)| s.eq_ignore_ascii_case("TableOption") && !v.trim().is_empty()) {
            why.push("its options are changed in its table settings".to_string());
        }
        if entries.iter().any(|(s, k, v)| s.eq_ignore_ascii_case("TableOverride") && k.eq_ignore_ascii_case("Difficulty") && !v.trim().is_empty()) {
            why.push("its difficulty is changed".to_string());
        }
    }
    why
}

/// Whether VPX's physics set `set` (1 to 8, as a table names it) has values of the user's in the
/// settings (`[Player]` `TablePhysics…<n>`, `FlipperPhysics…<n>`, numbered from 0), rather than
/// VPX's own.
fn physics_set_changed(set: u32) -> bool {
    let Some(file) = vpi_settings::settings_path() else { return false };
    let Ok(bytes) = std::fs::read(&file) else { return false };
    let n = set.saturating_sub(1).to_string();
    vpi_settings::parse_ini(&String::from_utf8_lossy(&bytes)).iter().any(|(s, k, v)| {
        s.eq_ignore_ascii_case("Player") && (k.starts_with("TablePhysics") || k.starts_with("FlipperPhysics")) && k.ends_with(&n) && !k[..k.len() - n.len()].ends_with(|c: char| c.is_ascii_digit()) && !v.trim().is_empty()
    })
}

/// PinMAME's cheat mode for a game, as `Machine settings.ini` sets it: its own section, else the
/// default.
fn cheat_on(rom: &str) -> bool {
    let Some(file) = vpi_settings::settings_path().and_then(|p| p.parent().map(|d| d.join("Machine settings.ini"))) else { return false };
    let Ok(bytes) = std::fs::read(&file) else { return false };
    let entries = vpi_settings::parse_ini(&String::from_utf8_lossy(&bytes));
    let get = |section: &str| entries.iter().rev().find(|(s, k, _)| s.eq_ignore_ascii_case(section) && k.eq_ignore_ascii_case("cheat")).map(|(_, _, v)| v.trim() == "1");
    get(rom).or_else(|| get("default")).unwrap_or(false)
}

/// How PLAY was asked to play a table for achievements: as it stands, Play Hardcore (what makes it
/// softcore left out for this game), or Play Softcore (the player's choice: a guest, say).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    AsItStands,
    Hardcore,
    Softcore,
}

/// The file handed to the player at PLAY for a table with packs, in the data folder; None for one
/// without.
pub fn for_play(st: &library::State, t: &Table, asked: Asked) -> Option<PathBuf> {
    let packs: Vec<PathBuf> = installed().into_iter().filter(|(_, p)| fits(st, t, p)).map(|(d, _)| d).collect();
    if packs.is_empty() {
        return None;
    }
    let why = match asked {
        Asked::Hardcore => Vec::new(),
        Asked::Softcore => vec!["you chose to play it softcore".to_string()],
        Asked::AsItStands => practice_because(st, t),
    };
    let mode = if why.is_empty() { "hardcore" } else { "practice" };
    let file = folder()?.join("playing.json");
    let asked = serde_json::json!({
        "mode": mode,
        "why": (!why.is_empty()).then(|| why.join("; ")),
        "table": t.path.file_name().map(|n| n.to_string_lossy().into_owned()),
        "packs": packs,
        "earned": earned_file()?,
    });
    std::fs::create_dir_all(file.parent()?).ok()?;
    std::fs::write(&file, serde_json::to_vec_pretty(&asked).ok()?).ok()?;
    Some(file)
}

/// One achievement as the screens show it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementView {
    pub id: String,
    pub name: String,
    pub about: String,
    pub points: u32,
    pub secret: bool,
    /// Its picture, through the pictures protocol, where the pack has one.
    pub icon: Option<String>,
    /// When it was earned, and how: the hardcore record where there's one, else the practice one.
    pub earned: Option<(String, String)>,
    /// When it was earned in hardcore, and in softcore (`practice` in the records), each if it was.
    pub hardcore: Option<String>,
    pub softcore: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub about: Option<String>,
    pub icon: Option<String>,
    pub game: String,
    pub achievements: Vec<AchievementView>,
    /// Each version mastered, and when.
    pub mastered: Vec<(String, String)>,
    /// The tables of the library it's made for, by ID and name.
    pub tables: Vec<(String, String)>,
}

fn view_of(dir: &Path, pack: &Pack, earned: &Earned, tables: Vec<(String, String)>) -> PackView {
    let base = crate::pictures::base();
    let mine = earned.packs.get(&pack.pack.id);
    let icon = |which: &str, has: bool| has.then(|| format!("{base}h/{}/{which}", pack.pack.id));
    PackView {
        id: pack.pack.id.clone(),
        name: pack.pack.name.clone(),
        version: pack.pack.version.clone(),
        authors: pack.pack.authors.clone(),
        about: pack.pack.about.clone(),
        icon: icon("pack", pack.pack.icon.as_ref().is_some_and(|i| dir.join(i).is_file())),
        game: pack.made_for.game.name.clone(),
        achievements: pack
            .achievements
            .iter()
            .map(|a| {
                let records = mine.and_then(|m| m.achievements.get(&a.id));
                let earned = records.and_then(|rs| rs.iter().find(|r| r.mode == "hardcore").or_else(|| rs.first())).map(|r| (r.at.clone(), r.mode.clone()));
                let when = |mode: &str| records.and_then(|rs| rs.iter().find(|r| r.mode == mode)).map(|r| r.at.clone());
                AchievementView {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    about: a.about.clone(),
                    points: a.points,
                    secret: a.secret,
                    icon: icon(&a.id, pack.icon_of(dir, a).is_some()),
                    earned,
                    hardcore: when("hardcore"),
                    softcore: when("practice"),
                }
            })
            .collect(),
        mastered: mine.map(|m| m.mastered.iter().map(|x| (x.pack_version.clone(), x.at.clone())).collect()).unwrap_or_default(),
        tables,
    }
}

/// A table's packs, with what's earned, and how it plays: practice, and why, or hardcore.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableAchievements {
    pub packs: Vec<PackView>,
    pub practice: Vec<String>,
}

/// How PLAY plays a table: how many packs it has, and what makes it softcore (none: hardcore).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayMode {
    pub packs: usize,
    pub softcore: Vec<String>,
}

#[tauri::command]
pub fn play_mode(shared: State<'_, Shared>, id: String) -> Result<PlayMode, String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let packs = installed().into_iter().filter(|(_, p)| fits(&st, t, p)).count();
    Ok(PlayMode { packs, softcore: if packs > 0 { practice_because(&st, t) } else { Vec::new() } })
}

#[tauri::command]
pub fn achievements_for(shared: State<'_, Shared>, id: String) -> Result<TableAchievements, String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let earned = earned_file().map(|f| Earned::load(&f)).unwrap_or_default();
    let packs = installed().into_iter().filter(|(_, p)| fits(&st, t, p)).map(|(d, p)| view_of(&d, &p, &earned, Vec::new())).collect();
    Ok(TableAchievements { packs, practice: practice_because(&st, t) })
}

/// Every pack kept, with the library's tables each is made for.
#[tauri::command]
pub fn achievements_all(shared: State<'_, Shared>) -> Vec<PackView> {
    let st = shared.0.lock().unwrap();
    let earned = earned_file().map(|f| Earned::load(&f)).unwrap_or_default();
    installed()
        .into_iter()
        .map(|(d, p)| {
            let tables = st.tables.iter().filter(|t| fits(&st, t, &p)).map(|t| (t.id.clone(), library::view(&st, t).name)).collect();
            view_of(&d, &p, &earned, tables)
        })
        .collect()
}

/// A pack's picture: the pack's own (`pack`), or an achievement's (its ID), the pack's where it
/// has none of its own.
pub fn icon(pack_id: &str, which: &str) -> Option<PathBuf> {
    let (dir, pack) = installed().into_iter().find(|(_, p)| p.pack.id == pack_id)?;
    if which == "pack" {
        return pack.pack.icon.as_ref().map(|i| dir.join(i)).filter(|p| p.is_file() && p.starts_with(&dir));
    }
    let a = pack.achievements.iter().find(|a| a.id == which)?;
    pack.icon_of(&dir, a).filter(|p| p.starts_with(&dir))
}

/// A pack added from a file the user chooses in Windows' picker — a pack's zip, or its
/// `achievements.json` alone — checked first, kept as its ID and version say; its name.
#[tauri::command]
pub async fn achievements_add(app: AppHandle, window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    let name = tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>, String> {
        let kinds = [("An achievement pack", "*.zip;achievements.json;*.json")];
        let Some(picked) = crate::pick::file(owner, "The achievement pack to add", &kinds)? else { return Ok(None) };
        add_from(&picked).map(Some)
    })
    .await
    .map_err(|e| e.to_string())??;
    library::start(app);
    Ok(name)
}

/// A pack's zip, or its `achievements.json`, checked and kept.
pub fn add_from(picked: &Path) -> Result<String, String> {
    let root = folder().ok_or("no APPDATA folder")?.join("packs");
    let passing = root.join(format!(".adding-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&passing);
    std::fs::create_dir_all(&passing).map_err(|e| format!("{}: {e}", passing.display()))?;
    let unpacked = (|| -> Result<Pack, String> {
        if picked.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")) {
            let file = std::fs::File::open(picked).map_err(|e| format!("{}: {e}", picked.display()))?;
            let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
            // the pack's own files, at its top or in the one folder it holds
            let names: Vec<String> = zip.file_names().map(str::to_string).collect();
            let top = names.iter().find(|n| n.rsplit('/').next() == Some("achievements.json")).ok_or("It holds no achievements.json.")?;
            let prefix = top.strip_suffix("achievements.json").unwrap_or("").to_string();
            for i in 0..zip.len() {
                let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
                let Some(rel) = f.enclosed_name().and_then(|p| p.to_str().map(|s| s.replace('\\', "/"))) else { continue };
                let Some(rel) = rel.strip_prefix(&prefix).map(str::to_string) else { continue };
                if f.is_dir() || rel.is_empty() {
                    continue;
                }
                let to = passing.join(&rel);
                std::fs::create_dir_all(to.parent().unwrap_or(&passing)).map_err(|e| e.to_string())?;
                let mut out = std::fs::File::create(&to).map_err(|e| format!("{}: {e}", to.display()))?;
                std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
            }
        } else {
            std::fs::copy(picked, passing.join("achievements.json")).map_err(|e| format!("{}: {e}", picked.display()))?;
            // the pictures it names, from beside it
            let beside = picked.parent().unwrap_or(Path::new("."));
            let pack = Pack::read(&passing)?;
            for icon in pack.pack.icon.iter().chain(pack.achievements.iter().filter_map(|a| a.icon.as_ref())) {
                let (from, to) = (beside.join(icon), passing.join(icon));
                if !icon.contains("..") && from.is_file() {
                    std::fs::create_dir_all(to.parent().unwrap_or(&passing)).map_err(|e| e.to_string())?;
                    std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
                }
            }
        }
        let pack = Pack::read(&passing)?;
        let problems = pack.problems(Some(&passing));
        if !problems.is_empty() {
            return Err(format!("That pack isn't right: {}.", problems.join("; ")));
        }
        Ok(pack)
    })();
    let pack = match unpacked {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&passing);
            return Err(e);
        }
    };
    let to = root.join(&pack.pack.id).join(&pack.pack.version);
    if to.exists() {
        let _ = std::fs::remove_dir_all(&passing);
        return Err(format!("{} {} is kept already.", pack.pack.name, pack.pack.version));
    }
    std::fs::create_dir_all(to.parent().unwrap_or(&root)).map_err(|e| e.to_string())?;
    std::fs::rename(&passing, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(format!("{} {}", pack.pack.name, pack.pack.version))
}

/// What `add_from` takes for a dropped file or folder that's an achievement pack — its
/// `achievements.json`, a folder holding one, or a zip holding one — told by what it says it is;
/// None for anything else.
pub fn pack_in(path: &Path) -> Option<PathBuf> {
    let ours = |text: &str| Pack::parse(text).is_ok();
    if path.is_dir() {
        let json = path.join("achievements.json");
        return std::fs::read_to_string(&json).ok().filter(|t| ours(t)).map(|_| json);
    }
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name == "achievements.json" {
        return std::fs::read_to_string(path).ok().filter(|t| ours(t)).map(|_| path.to_path_buf());
    }
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
        let entry = zip.file_names().find(|n| n.rsplit('/').next() == Some("achievements.json"))?.to_string();
        let mut text = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(&entry).ok()?, &mut text).ok()?;
        return ours(&text).then(|| path.to_path_buf());
    }
    None
}

/// Files dropped onto the app: the achievement packs among them added at once, the rest handed
/// back for the drop window. What was added, and the rest.
#[tauri::command]
pub async fn achievements_drop(app: AppHandle, paths: Vec<String>) -> Result<(Vec<String>, Vec<String>), String> {
    let (added, rest) = tauri::async_runtime::spawn_blocking(move || {
        let mut added = Vec::new();
        let mut rest = Vec::new();
        for p in paths {
            match pack_in(Path::new(&p)) {
                Some(from) => added.push(add_from(&from).unwrap_or_else(|e| format!("not added: {e}"))),
                None => rest.push(p),
            }
        }
        (added, rest)
    })
    .await
    .map_err(|e| e.to_string())?;
    if !added.is_empty() {
        library::start(app);
    }
    Ok((added, rest))
}

/// A pack deleted, every version of it, for good: the screens warn first that it can't be
/// recovered. What's earned of it stays.
#[tauri::command]
pub fn achievements_remove(app: AppHandle, id: String) -> Result<(), String> {
    let dir = folder().ok_or("no APPDATA folder")?.join("packs").join(&id);
    if id.contains(['/', '\\', '.']) || !dir.is_dir() {
        return Err("That pack isn't kept.".into());
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("It couldn't be deleted: {e}"))?;
    library::start(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run alone, as it sets `APPDATA` for the process: `cargo test -p app --release achievements -- --ignored`.
    #[test]
    #[ignore]
    fn a_zipped_pack_added_once_and_a_wrong_one_refused() {
        let data = std::env::temp_dir().join(format!("vpi-ach-add-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&data).unwrap();
        let pack = r#"{"format":"vpi-achievements","formatVersion":1,"pack":{"id":"8d6c2f0e-4c9a-4b8e-9f51-3a7d2b1c9e40","name":"AFM","version":"1.0","authors":["x"],"icon":"icons/pack.png"},"for":{"game":{"name":"Attack from Mars"}},"achievements":[{"id":"a","name":"A","about":"a","points":5,"since":"1.0","icon":"icons/a.png","when":{"game":"started"}}]}"#;
        let zipped = data.join("pack.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&zipped).unwrap());
        for (name, bytes) in [("AFM pack/achievements.json", pack.as_bytes()), ("AFM pack/icons/pack.png", b"png".as_slice()), ("AFM pack/icons/a.png", b"png".as_slice())] {
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut w, bytes).unwrap();
        }
        w.finish().unwrap();
        let wrong = data.join("wrong.json");
        std::fs::write(&wrong, pack.replace("\"points\":5", "\"points\":500")).unwrap();
        // the data folder where the app keeps them, for this check alone
        unsafe { std::env::set_var("APPDATA", &data) };
        let added = add_from(&zipped);
        let again = add_from(&zipped);
        let refused = add_from(&wrong);
        let kept = installed();
        let _ = std::fs::remove_dir_all(&data);
        assert_eq!(added.unwrap(), "AFM 1.0");
        assert!(again.unwrap_err().contains("kept already"));
        assert!(refused.unwrap_err().contains("1 to 100"));
        assert_eq!(kept.len(), 1);
    }
}
