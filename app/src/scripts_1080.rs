//! A game in VPX 10.8.0 given the shared scripts VPX 10.8.0 was released with, lent beside its table
//! for the game alone (`library::scripts_1080`): the ones a VPX 10.8.1 build brought into the
//! folder both share stop VPX 10.8.0 on every ROM table. Taken back as the game ends, or as the app
//! starts where it stopped during one.

use std::path::{Path, PathBuf};

use ::library::scripts_1080;

fn kept() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("VPX"))
}

fn record() -> Option<PathBuf> {
    Some(kept()?.join("lent scripts.json"))
}

/// Before a game in VPX 10.8.0 (a VPX taking no add-ons): the shared scripts its table loads,
/// VPX 10.8.0's own, lent beside it where they differ from the ones in VPX's folder.
pub fn lend_for(vpx: &::library::vpx::Vpx, table: &Path) {
    if vpx.has_addons() {
        return;
    }
    let (Some(kept), Some(record), Some(folder), Some(vpx_folder)) = (kept(), record(), table.parent(), vpx.program.parent()) else { return };
    let Some(own) = scripts_1080::kept_for_1080(&kept) else {
        eprintln!("VPX 10.8.0's own shared scripts aren't kept on this PC: {} plays with those in VPX's folder", table.display());
        return;
    };
    // the script that plays: one beside the table, else the table's own
    let script = std::fs::read(table.with_extension("vbs")).ok().or_else(|| ::library::table::own_script(table).ok());
    let Some(script) = script else { return };
    let needed = scripts_1080::loaded_by(&String::from_utf8_lossy(&script), &own);
    match scripts_1080::lend(&record, folder, &vpx_folder.join("scripts"), &needed) {
        Ok(lent) if !lent.is_empty() => {
            let names: Vec<String> = lent.iter().filter_map(|l| l.to.file_name()).map(|n| n.to_string_lossy().into_owned()).collect();
            eprintln!("VPX 10.8.0's own {} lent beside {} for the game", names.join(", "), table.display());
        }
        Ok(_) => {}
        Err(e) => eprintln!("VPX 10.8.0's own shared scripts couldn't be lent for {}: {e}", table.display()),
    }
}

/// What was lent for a game taken back: after it, or as the app starts.
pub fn take_back() {
    if let Some(record) = record()
        && let Err(e) = scripts_1080::take_back(&record)
    {
        eprintln!("VPX 10.8.0's lent scripts: {e}");
    }
}
