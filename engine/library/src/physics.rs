//! A table's physics for the app's physics window: the table author's own, as VPX's editor exports
//! them; the physics settings file that plays in their place, as VPX loads one before play —
//! `<table>.vpp` beside it, else the folder's `autovpp.vpp`; and presets, physics settings files
//! kept in the player's data folder to put on any table. A file is written as VPX's editor writes
//! one (`table_physics::vpp::write`), so VPX plays it too; the table file never changes. Ours.

use std::path::{Path, PathBuf};

use serde::Serialize;
use table_physics::vpp;

/// One physics settings file's values: None for one it doesn't hold.
pub type Values = [Option<f32>; 18];

/// A physics settings file beside the table or among the presets.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub path: PathBuf,
    /// The name it gives itself (VPX's editor writes the table's name).
    pub name: Option<String>,
    pub values: Values,
}

/// A table's physics, for its window.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// The element names, in the files' order: the table's eight, then the flipper's ten.
    pub elements: [&'static str; 18],
    /// The table author's own, in the files' units; None for a table with no flipper.
    pub own: Option<[f32; 18]>,
    /// The physics set of VPX's settings the table plays instead of its own (VPX's table
    /// property "Physics set": 1 to 8), if it chooses one: it plays over any file.
    pub set: Option<u32>,
    /// Its own file beside it, `<table>.vpp`.
    pub file: Option<File>,
    /// The folder's `autovpp.vpp`, which plays for every table there without a file of its own.
    pub shared: Option<File>,
}

fn read_file(path: &Path) -> Option<File> {
    let text = std::fs::read_to_string(path).ok()?;
    let (name, values) = vpp::read(&text).ok()?;
    Some(File { path: path.to_path_buf(), name, values })
}

/// What the table at `table` plays with, read from its file's game data and parts alone (its
/// pictures and sounds left unread), and the files beside it.
pub fn view(table: &Path) -> Result<View, String> {
    let fail = |e: std::io::Error| format!("{}: {e}", table.display());
    let mut vpx = vpin::vpx::open(table).map_err(fail)?;
    let game = vpx.read_gamedata().map_err(fail)?;
    let items = vpx.read_gameitems().map_err(fail)?;
    let shared = table.parent().map(|d| d.join("autovpp.vpp"));
    Ok(View {
        elements: vpp::ELEMENTS,
        own: vpp::own(&game, &items),
        set: (game.override_physics > 0).then_some(game.override_physics),
        file: read_file(&table.with_extension("vpp")),
        shared: shared.as_deref().and_then(read_file),
    })
}

/// The values a file leaves out taken from `own`, as VPX would play them: a value missing from
/// a file is 0 for a flipper and VPX's 6° for a slope, the rest the table's — only where `own`
/// has them.
pub fn filled(values: &Values, own: &[f32; 18]) -> [f32; 18] {
    std::array::from_fn(|i| values[i].unwrap_or(own[i]))
}

/// The physics settings file for `table` written: `<table>.vpp` beside it, named `name`, the one
/// there before sent away by `recycle` (the Recycle Bin); it plays from the next game on.
pub fn save(table: &Path, name: &str, values: &[f32; 18], recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<PathBuf, String> {
    let to = table.with_extension("vpp");
    write_over(&to, &vpp::write(name, values), recycle)?;
    Ok(to)
}

/// The table's own physics back: its file beside it sent away by `recycle`.
pub fn back_to_its_own(table: &Path, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<(), String> {
    let file = table.with_extension("vpp");
    if file.is_file() { recycle(&file) } else { Ok(()) }
}

/// A file written whole under a passing name, then put in place of the one there (that one sent
/// away by `recycle`), so a failure halfway leaves the one there as it was.
fn write_over(to: &Path, text: &str, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<(), String> {
    let passing = to.with_extension("vpp.vpi-part");
    std::fs::write(&passing, text).map_err(|e| format!("{}: {e}", passing.display()))?;
    if to.is_file()
        && let Err(e) = recycle(to)
    {
        let _ = std::fs::remove_file(&passing);
        return Err(e);
    }
    std::fs::rename(&passing, to).map_err(|e| format!("{}: {e}", to.display()))
}

/// The presets kept in `dir`, by name.
pub fn presets(dir: &Path) -> Vec<File> {
    let mut out: Vec<File> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpp")))
        .filter_map(|p| read_file(&p))
        .collect();
    out.sort_by_key(|f| f.path.file_stem().map(|s| s.to_string_lossy().to_lowercase()));
    out
}

/// A name made safe for a file's: what Windows doesn't allow in one taken out.
fn file_safe(name: &str) -> String {
    let safe: String = name.chars().filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()).collect();
    safe.trim().trim_end_matches('.').to_string()
}

/// These values kept as a preset named `name` in `dir`; one of that name there replaced, the old
/// one sent away by `recycle`.
pub fn save_preset(dir: &Path, name: &str, values: &[f32; 18], recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<PathBuf, String> {
    let stem = file_safe(name);
    if stem.is_empty() {
        return Err("Give the preset a name.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let to = dir.join(format!("{stem}.vpp"));
    write_over(&to, &vpp::write(name.trim(), values), recycle)?;
    Ok(to)
}

/// A physics settings file from anywhere (one shared, or exported from VPX's editor) kept among the
/// presets, under its own name, once it reads as one.
pub fn import_preset(dir: &Path, from: &Path, recycle: &dyn Fn(&Path) -> Result<(), String>) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(from).map_err(|e| format!("{}: {e}", from.display()))?;
    vpp::read(&text).map_err(|e| format!("{}: {e}", from.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let to = dir.join(from.file_name().ok_or("That file has no name.")?);
    write_over(&to, &text, recycle)?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tables_physics_saved_beside_it_put_back_and_kept_as_presets() {
        let d = std::env::temp_dir().join(format!("vpi-physics-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let table = d.join("Taxi.vpx");
        let recycled = std::cell::RefCell::new(Vec::new());
        let recycle = |p: &Path| {
            recycled.borrow_mut().push(p.to_path_buf());
            std::fs::remove_file(p).map_err(|e| e.to_string())
        };
        let mut values = [1.0f32; 18];
        values[0] = 0.97;
        // saved beside the table, as VPX's editor writes one; saved again, the one before recycled
        let file = save(&table, "Taxi", &values, &recycle).unwrap();
        assert_eq!(file, d.join("Taxi.vpp"));
        values[9] = 3600.0;
        save(&table, "Taxi", &values, &recycle).unwrap();
        assert_eq!(recycled.borrow().len(), 1);
        let back = read_file(&file).unwrap();
        assert_eq!((back.name.as_deref(), back.values[0], back.values[9]), (Some("Taxi"), Some(0.97), Some(3600.0)));
        // filled from the table's own where a file leaves a value out
        let mut part = back.values;
        part[17] = None;
        assert_eq!(filled(&part, &[5.0; 18])[17], 5.0);
        // a preset kept under a name made safe for a file's, and listed
        let presets_dir = d.join("presets");
        save_preset(&presets_dir, "Fast: flippers?", &values, &recycle).unwrap();
        assert!(presets_dir.join("Fast flippers.vpp").is_file());
        let kept = presets(&presets_dir);
        assert_eq!((kept.len(), kept[0].name.as_deref()), (1, Some("Fast: flippers?")));
        // imported from anywhere, once it reads as one; one that doesn't, refused
        assert!(import_preset(&presets_dir, &file, &recycle).is_ok());
        std::fs::write(d.join("not one.vpp"), "<nope/>").unwrap();
        assert!(import_preset(&presets_dir, &d.join("not one.vpp"), &recycle).is_err());
        // back to the table's own
        back_to_its_own(&table, &recycle).unwrap();
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
