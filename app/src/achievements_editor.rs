//! The achievements editor's side, the table editor's second stage: a table's achievement packs
//! opened whole for changing — each pack's newest version as its file says, every field kept, its
//! pictures' addresses — or a new pack started for it from what VPS says of its game and table
//! file; a picture chosen in Windows' picker for a pack or an achievement, served to the screens
//! while it's chosen (`pic.localhost/e/<n>`); a pack saved as its next version beside the versions
//! kept (`Achievements\packs\<ID>\<version>`), checked first as VPIDB checks an upload, the pictures
//! it names carried over from the version it's made from and those chosen copied in; and a pack
//! tried by playing: the table started as PLAY starts it, that pack alone followed, in practice,
//! what's earned kept in a passing file (`Achievements\trying`), never the player's own record.
//! A pack being made is saved as a draft as often as its maker likes — no version, nothing played
//! with it — in `Achievements\drafts\<ID>.json`, its chosen pictures and sound copied beside it
//! (`drafts\<ID>\`); finalized, it's packed as its next version and the draft let go.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};
use vpi_achievements::earned::Earned;
use vpi_achievements::pack::{Pack, version_key};

use crate::library::{self, Shared};

fn achievements_folder() -> Option<PathBuf> {
    Some(library::data_folder()?.join("Achievements"))
}

/// A table's packs as the editor opens them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    /// The packs made for the table, the newest version kept of each.
    pub packs: Vec<Kept>,
    /// A new pack's start: the table's game as VPS names it, and its table file where VPS knows
    /// it; its ID left for the screens to make.
    pub fresh: Value,
    /// Why the table plays practice, where it does.
    pub practice: Vec<String>,
    /// The drafts of packs made for the table, by the pack's ID: the editor's own state as it was
    /// saved, its pictures' and sound's addresses filled in.
    pub drafts: Vec<Value>,
    /// What the table can tell achievements: the editor offers only that.
    pub can: Can,
}

/// What a table can tell achievements as it plays: the game's facts — a ROM machine's from its
/// memory map, a table without one from what its script hands its backglass — the machine's lamps,
/// switches and coils (ROM tables), and events of its own (a script written for achievements);
/// its coils as its script names them. Balls in play and its parts' hits always.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Can {
    /// Its ROM set, where it drives PinMAME.
    pub rom: Option<String>,
    /// A memory map knows its machine.
    pub map: bool,
    /// A game starting and ending.
    pub game: bool,
    pub ball: bool,
    pub score: bool,
    pub credits: bool,
    pub extra_ball: bool,
    pub tilt: bool,
    /// Its machine's lamps, switches and coils.
    pub machine: bool,
    /// Its script tells events of its own (`Achievement.Event`).
    pub events: bool,
    pub coils: Vec<Coil>,
}

/// A coil as the table's script names it.
#[derive(Serialize)]
pub struct Coil {
    pub n: u32,
    /// What its `SolCallback` line hands it to.
    pub name: String,
    /// The comment after the line.
    pub note: Option<String>,
}

fn can_of(t: &crate::library::Table) -> Can {
    let script = ::library::table::script_that_plays(&t.path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let code = ::library::table::strip_comments(&script).to_ascii_lowercase();
    let events = code.contains("achievement.event");
    match &t.holds.rom_set {
        Some(rom) => {
            let facts = ::library::scores::game_facts(rom);
            let has = |k: &str| facts.iter().any(|f| f == k);
            Can {
                rom: Some(rom.clone()),
                map: !facts.is_empty(),
                game: has("game_over") || has("current_ball"),
                ball: has("current_ball"),
                score: has("scores"),
                credits: has("credits"),
                extra_ball: has("extra_balls"),
                tilt: has("tilted"),
                machine: true,
                events,
                coils: ::library::table::coil_names(&script).into_iter().map(|(n, name, note)| Coil { n, name, note }).collect(),
            }
        }
        // B2S's calls, as the player reads them (`game_state::from_b2s`)
        None => Can {
            game: code.contains("b2ssetgameover"),
            ball: code.contains("b2ssetballinplay"),
            score: code.contains("b2ssetscoreplayer"),
            credits: code.contains("b2ssetcredits"),
            tilt: code.contains("b2ssettilt"),
            events,
            ..Can::default()
        },
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    /// Its folder: the version it's opened at.
    pub dir: String,
    /// Its file whole, as it says.
    pub pack: Value,
    /// Its pictures through the pictures protocol: the pack's own (`pack`) and each achievement's
    /// own, by its ID.
    pub pictures: BTreeMap<String, String>,
    /// The version a save of it makes.
    pub next: String,
}

#[tauri::command]
pub fn ach_edit_open(shared: State<'_, Shared>, id: String) -> Result<Opened, String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let root = achievements_folder().ok_or("no APPDATA folder")?.join("packs");
    let base = crate::pictures::base();
    let packs = crate::achievements::installed()
        .into_iter()
        .filter(|(_, p)| crate::achievements::fits(&st, t, p))
        .filter_map(|(dir, pack)| {
            let whole: Value = serde_json::from_str(std::fs::read_to_string(dir.join("achievements.json")).ok()?.trim_start_matches('\u{feff}')).ok()?;
            let at = |which: &str| format!("{base}h/{}/{which}?v={}", pack.pack.id, pack.pack.version);
            let mut pictures = BTreeMap::new();
            if pack.pack.icon.as_ref().is_some_and(|i| dir.join(i).is_file()) {
                pictures.insert("pack".to_string(), at("pack"));
            }
            for a in &pack.achievements {
                if a.icon.as_ref().is_some_and(|i| dir.join(i).is_file()) {
                    pictures.insert(a.id.clone(), at(&a.id));
                }
            }
            Some(Kept { dir: dir.to_string_lossy().into_owned(), pack: whole, pictures, next: next_version(&root, &pack.pack.id) })
        })
        .collect();
    let view = library::view(&st, t);
    let mut game = serde_json::Map::new();
    if let Some(vps) = &t.on_vps.game {
        game.insert("vps".into(), vps.clone().into());
    }
    game.insert("name".into(), view.name.clone().into());
    if let Some(maker) = &view.maker {
        game.insert("maker".into(), maker.clone().into());
    }
    if let Some(year) = view.year {
        game.insert("year".into(), year.into());
    }
    // made on this table file: the parts it names are named as this one names them
    let tables: Vec<Value> = t
        .on_vps
        .table_file
        .iter()
        .map(|file| {
            let mut one = serde_json::Map::new();
            one.insert("vps".into(), file.clone().into());
            if let Some(v) = &view.version {
                one.insert("version".into(), v.clone().into());
            }
            one.insert("authors".into(), view.authors.clone().into());
            Value::Object(one)
        })
        .collect();
    let fresh = serde_json::json!({
        "format": vpi_achievements::pack::FORMAT,
        "formatVersion": vpi_achievements::pack::FORMAT_VERSION,
        "pack": { "id": "", "name": view.name, "version": "1.0", "authors": [] },
        "for": { "game": game, "tables": tables },
        "achievements": [],
    });
    let practice = crate::achievements::practice_because(&st, t);
    // the drafts made for this table: those whose pack fits it
    let mut drafts = Vec::new();
    if let Some(dir) = drafts_folder() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = e.path();
            if !path.extension().is_some_and(|x| x.eq_ignore_ascii_case("json")) {
                continue;
            }
            let Some(mut draft) = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else { continue };
            let mut as_pack = draft["pack"].clone();
            as_pack["achievements"] = Value::Array(Vec::new());
            let fits = serde_json::from_value::<Pack>(as_pack).is_ok_and(|p| crate::achievements::fits(&st, t, &p));
            if !fits {
                continue;
            }
            // its pictures and sound served while the editor's open
            if let Some(files) = draft.get_mut("files").and_then(Value::as_object_mut) {
                for (_, f) in files.iter_mut() {
                    if let Some(path) = f.get("path").and_then(Value::as_str).map(PathBuf::from).filter(|p| p.is_file()) {
                        let mut picked = PICKED.lock().unwrap();
                        picked.push(path);
                        f["url"] = format!("{}e/{}", crate::pictures::base(), picked.len() - 1).into();
                    }
                }
            }
            drafts.push(draft);
        }
    }
    Ok(Opened { packs, fresh, practice, drafts, can: can_of(t) })
}

fn drafts_folder() -> Option<PathBuf> {
    Some(achievements_folder()?.join("drafts"))
}

/// The ID a draft is kept under: the pack's, safe as a file name.
fn draft_id(draft: &Value) -> Result<String, String> {
    let id = draft.pointer("/pack/pack/id").and_then(Value::as_str).unwrap_or_default().to_string();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("The pack has no ID.".into());
    }
    Ok(id)
}

/// A pack being made saved as a draft: the editor's state whole, no version made, nothing played
/// with it. The files it names (`files`: what each is for, its file) copied beside it the first
/// time, so the draft keeps them wherever the originals go. When it was saved.
#[tauri::command]
pub async fn ach_edit_draft_save(draft: Value) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let mut draft = draft;
        let id = draft_id(&draft)?;
        let dir = drafts_folder().ok_or("no APPDATA folder")?;
        let own = dir.join(&id);
        std::fs::create_dir_all(&own).map_err(|e| format!("{}: {e}", own.display()))?;
        if let Some(files) = draft.get_mut("files").and_then(Value::as_object_mut) {
            for (what, f) in files.iter_mut() {
                let Some(src) = f.get("path").and_then(Value::as_str).map(PathBuf::from) else { continue };
                if src.starts_with(&own) || !src.is_file() {
                    continue;
                }
                let kind = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                let safe: String = what.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
                let to = own.join(format!("{safe}-{}.{kind}", vpi_achievements::earned::now().replace([':', '-'], "")));
                std::fs::copy(&src, &to).map_err(|e| format!("{}: {e}", src.display()))?;
                f["path"] = to.to_string_lossy().into_owned().into();
            }
            for (_, f) in files.iter_mut() {
                if let Some(o) = f.as_object_mut() {
                    o.remove("url");
                }
            }
        }
        let at = vpi_achievements::earned::now();
        draft["savedAt"] = at.clone().into();
        let file = dir.join(format!("{id}.json"));
        let part = dir.join(format!("{id}.json.part"));
        std::fs::write(&part, serde_json::to_vec_pretty(&draft).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", part.display()))?;
        std::fs::rename(&part, &file).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(at)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A pack's draft let go: packed now, or thrown away on the maker's word (its files with it, to the
/// Recycle Bin).
#[tauri::command]
pub fn ach_edit_draft_drop(id: String) -> Result<(), String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("The pack has no ID.".into());
    }
    let dir = drafts_folder().ok_or("no APPDATA folder")?;
    for p in [dir.join(format!("{id}.json")), dir.join(&id)] {
        if p.exists() {
            ::library::install::to_recycle_bin(&p)?;
        }
    }
    Ok(())
}

/// A sound for a pack, chosen in Windows' picker: played as its achievements are announced.
#[tauri::command]
pub async fn ach_edit_sound(window: tauri::WebviewWindow) -> Result<Option<Picked>, String> {
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<Picked>, String> {
        let kinds = [("A sound", "*.wav;*.ogg;*.mp3;*.flac")];
        let Some(file) = crate::pick::file(owner, "Its sound", &kinds)? else { return Ok(None) };
        // as VPIDB takes a pack's sounds
        if let Some(p) = vpi_achievements::pack::sound_file_problem(&file) {
            return Err(format!("{p}."));
        }
        let mut picked = PICKED.lock().unwrap();
        picked.push(file.clone());
        Ok(Some(Picked { path: file.to_string_lossy().into_owned(), url: format!("{}e/{}", crate::pictures::base(), picked.len() - 1) }))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The version a pack's next save makes: the minor after the newest kept, else 1.0.
fn next_version(root: &Path, id: &str) -> String {
    let newest = std::fs::read_dir(root.join(id)).into_iter().flatten().flatten().filter_map(|e| version_key(&e.file_name().to_string_lossy())).max();
    match newest {
        Some((major, minor)) => format!("{major}.{}", minor + 1),
        None => "1.0".to_string(),
    }
}

/// The pictures chosen while the editor's open, by their place in the list: the pictures
/// protocol serves these alone.
static PICKED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// A picture chosen: its file, and where the screens show it from.
#[derive(Serialize)]
pub struct Picked {
    pub path: String,
    pub url: String,
}

/// A chosen picture's file, by its place (the pictures protocol's `e/<n>`).
pub fn picked(n: &str) -> Option<PathBuf> {
    PICKED.lock().unwrap().get(n.parse::<usize>().ok()?).cloned()
}

const PICTURE_KINDS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
const SOUND_KINDS: [&str; 4] = ["wav", "ogg", "mp3", "flac"];

/// A picture for a pack or an achievement, chosen in Windows' picker.
#[tauri::command]
pub async fn ach_edit_picture(window: tauri::WebviewWindow) -> Result<Option<Picked>, String> {
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<Picked>, String> {
        let kinds = [("A picture", "*.png;*.jpg;*.jpeg;*.webp")];
        let Some(file) = crate::pick::file(owner, "Its picture", &kinds)? else { return Ok(None) };
        let size = std::fs::metadata(&file).map_err(|e| format!("{}: {e}", file.display()))?.len();
        if size > 4 << 20 {
            return Err("That picture is over 4 MB: a square one, 256 × 256, is plenty.".into());
        }
        let mut picked = PICKED.lock().unwrap();
        picked.push(file.clone());
        Ok(Some(Picked { path: file.to_string_lossy().into_owned(), url: format!("{}e/{}", crate::pictures::base(), picked.len() - 1) }))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A pack as the editor sends it to be saved or tried.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edited {
    /// The folder of the version it's made from; none for a new pack.
    pub from: Option<String>,
    /// Its file whole.
    pub pack: Value,
    /// Pictures chosen for it, by what they're for — `pack`, or an achievement's ID — each a file.
    #[serde(default)]
    pub pictures: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct Saved {
    pub name: String,
    pub version: String,
}

/// The pack saved as its next version, the library read again after.
#[tauri::command]
pub async fn ach_edit_save(app: AppHandle, edited: Edited) -> Result<Saved, String> {
    let root = achievements_folder().ok_or("no APPDATA folder")?.join("packs");
    let saved = tauri::async_runtime::spawn_blocking(move || save(&root, &edited)).await.map_err(|e| e.to_string())??;
    library::start(app);
    Ok(saved)
}

/// The pack written as its next version in `root` (`<ID>\<version>`), checked first: its version
/// and date set — the achievements whose rules date from the version the editor proposed dating
/// from this one — its pictures in it.
pub fn save(root: &Path, edited: &Edited) -> Result<Saved, String> {
    let mut value = edited.pack.clone();
    let id = text(&value, "/pack/id");
    if id.is_empty() || id.contains(['/', '\\', '.']) {
        return Err("The pack has no ID.".into());
    }
    let version = next_version(root, &id);
    let proposed = text(&value, "/pack/version");
    if let Some(list) = value.pointer_mut("/achievements").and_then(Value::as_array_mut) {
        for a in list.iter_mut().filter(|a| a.get("since").and_then(Value::as_str) == Some(proposed.as_str())) {
            a["since"] = version.clone().into();
        }
    }
    value["pack"]["version"] = version.clone().into();
    value["pack"]["date"] = vpi_achievements::earned::now()[..10].to_string().into();
    std::fs::create_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
    let passing = root.join(format!(".saving-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&passing);
    let made = write_checked(&passing, edited.from.as_deref().map(Path::new), &mut value, &edited.pictures);
    let pack = match made {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&passing);
            return Err(e);
        }
    };
    let to = root.join(&id).join(&version);
    if to.exists() {
        let _ = std::fs::remove_dir_all(&passing);
        return Err(format!("{} {version} is kept already.", pack.pack.name));
    }
    std::fs::create_dir_all(root.join(&id)).map_err(|e| e.to_string())?;
    std::fs::rename(&passing, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(Saved { name: pack.pack.name, version })
}

fn text(v: &Value, at: &str) -> String {
    v.pointer(at).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// The pack's file and pictures written into `dir`, made anew, then read back and checked as VPIDB
/// checks an upload.
fn write_checked(dir: &Path, from: Option<&Path>, value: &mut Value, pictures: &BTreeMap<String, String>) -> Result<Pack, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // the pictures chosen, as icons/<what>.<kind>, named in the pack in their new places; the pack's
    // sound (`sound`) as sounds/pack.<kind>
    for (what, source) in pictures {
        let src = Path::new(source);
        if what == "sound" {
            let kind = src.extension().map(|e| e.to_string_lossy().to_lowercase()).filter(|e| SOUND_KINDS.contains(&e.as_str())).ok_or_else(|| format!("{}: a sound is a WAV, Ogg, MP3 or FLAC file", src.display()))?;
            let rel = format!("sounds/pack.{kind}");
            if let Some(about) = value.pointer_mut("/pack").and_then(Value::as_object_mut) {
                about.insert("sound".into(), rel.clone().into());
            }
            let to = dir.join(&rel);
            std::fs::create_dir_all(to.parent().unwrap_or(dir)).map_err(|e| e.to_string())?;
            std::fs::copy(src, &to).map_err(|e| format!("{}: {e}", src.display()))?;
            continue;
        }
        let kind = src.extension().map(|e| e.to_string_lossy().to_lowercase()).filter(|e| PICTURE_KINDS.contains(&e.as_str())).ok_or_else(|| format!("{}: a picture is a PNG, JPEG or WebP file", src.display()))?;
        if what.is_empty() || what.contains(['/', '\\', '.']) {
            return Err(format!("\"{what}\" can't name a picture"));
        }
        let rel = format!("icons/{what}.{kind}");
        let named = if what == "pack" {
            value.pointer_mut("/pack")
        } else {
            value.pointer_mut("/achievements").and_then(Value::as_array_mut).and_then(|l| l.iter_mut().find(|a| a.get("id").and_then(Value::as_str) == Some(what)))
        };
        let Some(named) = named.and_then(Value::as_object_mut) else { return Err(format!("a picture for \"{what}\", which the pack doesn't hold")) };
        named.insert("icon".into(), rel.clone().into());
        let to = dir.join(&rel);
        std::fs::create_dir_all(to.parent().unwrap_or(dir)).map_err(|e| e.to_string())?;
        std::fs::copy(src, &to).map_err(|e| format!("{}: {e}", src.display()))?;
    }
    // the pictures and sound it names that weren't chosen anew, from the version it's made from
    if let Some(from) = from {
        let mut named: Vec<String> = value.pointer("/pack/icon").and_then(Value::as_str).map(str::to_string).into_iter().collect();
        named.extend(value.pointer("/pack/sound").and_then(Value::as_str).map(str::to_string));
        if let Some(list) = value.pointer("/achievements").and_then(Value::as_array) {
            named.extend(list.iter().filter_map(|a| a.get("icon").and_then(Value::as_str).map(str::to_string)));
        }
        for rel in named.iter().filter(|r| !r.contains("..") && !Path::new(r.as_str()).is_absolute()) {
            let (src, to) = (from.join(rel), dir.join(rel));
            if !to.exists() && src.is_file() {
                std::fs::create_dir_all(to.parent().unwrap_or(dir)).map_err(|e| e.to_string())?;
                std::fs::copy(&src, &to).map_err(|e| format!("{}: {e}", src.display()))?;
            }
        }
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("achievements.json"), bytes).map_err(|e| format!("{}: {e}", dir.display()))?;
    let pack = Pack::read(dir)?;
    let problems = pack.problems(Some(dir));
    if !problems.is_empty() {
        return Err(format!("It isn't right yet: {}.", problems.join("; ")));
    }
    Ok(pack)
}

/// The pack tried by playing the table: written into `Achievements\trying`, made anew, and handed
/// to the player alone, in practice, what's earned kept beside it.
#[tauri::command]
pub async fn ach_edit_try(app: AppHandle, shared: State<'_, Shared>, id: String, edited: Edited) -> Result<(), String> {
    let table = {
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        t.path.file_name().map(|n| n.to_string_lossy().into_owned())
    };
    let trying = achievements_folder().ok_or("no APPDATA folder")?.join("trying");
    let asked = tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf, String> {
        let _ = std::fs::remove_dir_all(&trying);
        let mut value = edited.pack.clone();
        write_checked(&trying.join("pack"), edited.from.as_deref().map(Path::new), &mut value, &edited.pictures)?;
        let asked = trying.join("asked.json");
        let said = serde_json::json!({
            "mode": "practice",
            "trying": true,
            "table": table,
            "packs": [trying.join("pack")],
            "earned": trying.join("earned.json"),
        });
        std::fs::write(&asked, serde_json::to_vec_pretty(&said).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", asked.display()))?;
        Ok(asked)
    })
    .await
    .map_err(|e| e.to_string())??;
    crate::play::try_achievements(&app, &id, asked)
}

/// A pack's newest version kept, its folder shown in Explorer.
#[tauri::command]
pub fn ach_edit_show(id: String) -> Result<(), String> {
    let (dir, _) = crate::achievements::installed().into_iter().find(|(_, p)| p.pack.id == id).ok_or("That pack isn't kept.")?;
    crate::shell::show(&dir.join("achievements.json"))
}

/// The achievements earned in the last try, by their IDs.
#[tauri::command]
pub fn ach_edit_tried() -> Vec<String> {
    let Some(file) = achievements_folder().map(|f| f.join("trying").join("earned.json")) else { return Vec::new() };
    Earned::load(&file).packs.into_values().flat_map(|p| p.achievements.into_keys()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_saved_as_its_next_version_with_its_pictures() {
        let root = std::env::temp_dir().join(format!("vpi-ach-edit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let id = "8d6c2f0e-4c9a-4b8e-9f51-3a7d2b1c9e40";
        // version 1.0 kept, with its pictures
        let v10 = root.join(id).join("1.0");
        std::fs::create_dir_all(v10.join("icons")).unwrap();
        std::fs::write(v10.join("icons/pack.png"), b"pack").unwrap();
        std::fs::write(v10.join("icons/a.png"), b"a").unwrap();
        let mine = root.join("mine.png");
        std::fs::write(&mine, b"new").unwrap();
        let pack = serde_json::json!({
            "format": "vpi-achievements", "formatVersion": 1, "future": "kept",
            "pack": { "id": id, "name": "AFM", "version": "1.1", "authors": ["x"], "icon": "icons/pack.png" },
            "for": { "game": { "name": "Attack from Mars" } },
            "achievements": [
                { "id": "a", "name": "A", "about": "a", "points": 5, "since": "1.0", "icon": "icons/a.png", "when": { "game": "started" } },
                { "id": "pops", "name": "Pops", "about": "Hit the pops 25 times in one ball", "points": 10, "since": "1.1", "when": { "count": { "part": "Bumper1|Bumper2", "does": "hit" }, "times": 25, "within": "ball" } }
            ]
        });
        let edited = Edited { from: Some(v10.to_string_lossy().into_owned()), pack: pack.clone(), pictures: BTreeMap::from([("pops".to_string(), mine.to_string_lossy().into_owned())]) };
        let saved = save(&root, &edited);
        let v11 = root.join(id).join("1.1");
        let written: Option<Value> = std::fs::read(v11.join("achievements.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let carried = std::fs::read(v11.join("icons/a.png")).ok();
        let copied = std::fs::read(v11.join("icons/pops.png")).ok();
        // a second save makes 1.2; a wrong one is refused and leaves nothing behind
        let again = save(&root, &edited).map(|s| s.version);
        let mut wrong = pack.clone();
        wrong["achievements"][0]["points"] = 0.into();
        let refused = save(&root, &Edited { from: None, pack: wrong, pictures: BTreeMap::new() });
        let left: Vec<String> = std::fs::read_dir(&root).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(saved.unwrap().version, "1.1");
        let written = written.unwrap();
        assert_eq!(written["future"], "kept");
        assert_eq!(written["achievements"][1]["icon"], "icons/pops.png");
        assert_eq!(written["achievements"][1]["since"], "1.1");
        assert_eq!(written["achievements"][0]["since"], "1.0");
        assert_eq!((carried.as_deref(), copied.as_deref()), (Some(b"a".as_slice()), Some(b"new".as_slice())));
        assert_eq!(again.unwrap(), "1.2");
        assert!(refused.unwrap_err().contains("1 to 100"));
        left.iter().for_each(|n| assert!(!n.starts_with(".saving"), "{n}"));
    }
}
