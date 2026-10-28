//! A table's backglasses: the one that plays named as the table (`<table>.directb2s`), where VPX,
//! B2S and our player look for it, and the others kept beside it numbered as VPin Studio numbers a
//! backglass's versions (`<table> (1).directb2s` and on), so either program sees the same ones. One
//! is chosen to play, or none, by swapping their names, as VPin Studio's backglass manager does.
//! Ours.

use std::path::{Path, PathBuf};

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Whether `version` is one of the table's (`table`, its file) kept versions: beside it, named
/// `<table> (n).directb2s`.
fn is_version(table: &Path, version: &Path) -> bool {
    let stem = table.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let named = version.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    version.parent() == table.parent() && named.strip_suffix(".directb2s").and_then(|s| crate::installed::number_after(&stem, s)).is_some()
}

/// The backglass `chosen`, one of the table's kept versions, made the one that plays: the one that
/// played takes its number. Through a passing name, so nothing is lost if a step fails.
pub fn play(table: &Path, chosen: &Path) -> Result<(), String> {
    let playing = table.with_extension("directb2s");
    if !is_version(table, chosen) {
        return Err(format!("{} isn't one of this table's backglasses.", name(chosen)));
    }
    if !chosen.is_file() {
        return Err(format!("{} isn't there any more.", name(chosen)));
    }
    if !playing.is_file() {
        return std::fs::rename(chosen, &playing).map_err(|e| format!("{} couldn't be renamed: {e}", name(chosen)));
    }
    let passing = chosen.with_file_name(format!("{}.vpi-swap", name(chosen)));
    std::fs::rename(chosen, &passing).map_err(|e| format!("{} couldn't be renamed: {e}", name(chosen)))?;
    if let Err(e) = std::fs::rename(&playing, chosen) {
        let _ = std::fs::rename(&passing, chosen);
        return Err(format!("{} couldn't be renamed: {e}", name(&playing)));
    }
    if let Err(e) = std::fs::rename(&passing, &playing) {
        let _ = std::fs::rename(chosen, &playing);
        let _ = std::fs::rename(&passing, chosen);
        return Err(format!("{} couldn't be renamed: {e}", name(chosen)));
    }
    Ok(())
}

/// No backglass file played for the table: the one that played kept beside it under the first
/// number free. Its place, if there was one.
pub fn play_none(table: &Path) -> Result<Option<PathBuf>, String> {
    let playing = table.with_extension("directb2s");
    if !playing.is_file() {
        return Ok(None);
    }
    let stem = table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let free = (1..).map(|n| table.with_file_name(format!("{stem} ({n}).directb2s"))).find(|p| !p.exists()).expect("a free number");
    std::fs::rename(&playing, &free).map_err(|e| format!("{} couldn't be renamed: {e}", name(&playing)))?;
    Ok(Some(free))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_backglass_chosen_and_none() {
        let d = std::env::temp_dir().join(format!("vpi-backglass-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let table = d.join("Blizzard.vpx");
        std::fs::write(&table, b"t").unwrap();
        std::fs::write(d.join("Blizzard.directb2s"), b"first").unwrap();
        std::fs::write(d.join("Blizzard (1).directb2s"), b"alternate").unwrap();
        let read = |n: &str| std::fs::read_to_string(d.join(n)).unwrap();
        // the kept one plays; the one that played takes its number
        play(&table, &d.join("Blizzard (1).directb2s")).unwrap();
        assert_eq!((read("Blizzard.directb2s"), read("Blizzard (1).directb2s")), ("alternate".into(), "first".into()));
        // none: the one playing kept under the first number free
        assert_eq!(play_none(&table).unwrap(), Some(d.join("Blizzard (2).directb2s")));
        assert!(!d.join("Blizzard.directb2s").exists());
        // and one chosen again with none playing
        play(&table, &d.join("Blizzard (1).directb2s")).unwrap();
        assert_eq!(read("Blizzard.directb2s"), "first");
        // another table's, or a file not numbered for it, is refused
        std::fs::write(d.join("Other (1).directb2s"), b"x").unwrap();
        assert!(play(&table, &d.join("Other (1).directb2s")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
