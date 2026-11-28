//! A table's backglasses: those that play named as the table — `<table>.directb2s`, where VPX's own
//! backglass add-on, B2S Server and our player look, and `<table>.B2SPro`, B2S Pro's own, which B2S
//! Pro (its server or its VPX add-on) takes before the other and nothing else reads — and the others
//! kept beside it numbered as VPin Studio numbers a backglass's versions, both kinds counted
//! together (`<table> (1).directb2s`, `<table> (2).B2SPro` and on), so either program sees the same
//! ones. One is chosen to play, or none, by swapping their names, as VPin Studio's backglass manager
//! does: a `.B2SPro` chosen plays in B2S Pro, the `.directb2s` left playing where B2S Pro isn't; a
//! `.directb2s` chosen plays everywhere, a `.B2SPro` named for the table numbered out of B2S Pro's
//! way. A `.B2SPro`'s score display places, which B2S Pro's add-on keeps beside it
//! (`<name>.B2SPro.displays.xml`), go with it. Ours.

use std::path::{Path, PathBuf};

/// The backglass files' kinds, by their names' ends in lower case: B2S's and B2S Pro's.
pub const KINDS: [&str; 2] = ["directb2s", "b2spro"];

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A backglass file's kind (`directb2s`, `b2spro`), by its name's end in any case.
pub fn kind_of(p: &Path) -> Option<&'static str> {
    let ext = p.extension()?.to_string_lossy().to_ascii_lowercase();
    KINDS.iter().copied().find(|k| *k == ext)
}

/// A kind's end as its program spells it.
fn spelled(kind: &str) -> &'static str {
    if kind == "b2spro" { "B2SPro" } else { "directb2s" }
}

/// The file of this kind named as the table: as it's spelled beside it where it's there, else as its
/// program spells it.
pub fn playing(table: &Path, kind: &str) -> PathBuf {
    let stem = table.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let there = table.parent().and_then(|dir| {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| kind_of(p) == Some(kind) && p.file_stem().is_some_and(|s| s.to_string_lossy().to_lowercase() == stem) && p.is_file())
    });
    there.unwrap_or_else(|| table.with_extension(spelled(kind)))
}

/// What goes with a backglass file: B2S Pro's add-on's score display places beside a `.B2SPro`.
fn companion(p: &Path) -> PathBuf {
    p.with_file_name(format!("{}.displays.xml", name(p)))
}

/// A backglass renamed, what goes with it too.
fn rename_with(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)?;
    let c = companion(from);
    if c.is_file() {
        let _ = std::fs::rename(&c, companion(to));
    }
    Ok(())
}

/// A kept version's place: `<table> (n).<kind>`.
fn version_path(table: &Path, n: u32, kind: &str) -> PathBuf {
    let stem = table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    table.with_file_name(format!("{stem} ({n}).{}", spelled(kind)))
}

/// The first number no kept version of either kind holds.
fn free_number(table: &Path) -> u32 {
    (1..).find(|&n| KINDS.iter().all(|k| !version_path(table, n, k).exists())).expect("a free number")
}

/// Whether `version` is one of the table's (`table`, its file) kept versions: beside it, named
/// `<table> (n).directb2s` or `<table> (n).B2SPro`.
fn is_version(table: &Path, version: &Path) -> bool {
    let stem = table.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let Some(kind) = kind_of(version) else { return false };
    let named = version.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    version.parent() == table.parent() && named.strip_suffix(&format!(".{kind}")).and_then(|s| crate::installed::number_after(&stem, s)).is_some()
}

/// The backglass `chosen`, one of the table's kept versions, made the one of its kind that plays:
/// the one that played takes its number. Through a passing name, so nothing is lost if a step
/// fails. A `.directb2s` chosen: a `.B2SPro` named for the table numbered out of the way, as B2S
/// Pro would show it instead.
pub fn play(table: &Path, chosen: &Path) -> Result<(), String> {
    if !is_version(table, chosen) {
        return Err(format!("{} isn't one of this table's backglasses.", name(chosen)));
    }
    if !chosen.is_file() {
        return Err(format!("{} isn't there any more.", name(chosen)));
    }
    let kind = kind_of(chosen).unwrap_or("directb2s");
    let playing = playing(table, kind);
    if !playing.is_file() {
        rename_with(chosen, &playing).map_err(|e| format!("{} couldn't be renamed: {e}", name(chosen)))?;
    } else {
        let passing = chosen.with_file_name(format!("{}.vpi-swap", name(chosen)));
        rename_with(chosen, &passing).map_err(|e| format!("{} couldn't be renamed: {e}", name(chosen)))?;
        if let Err(e) = rename_with(&playing, chosen) {
            let _ = rename_with(&passing, chosen);
            return Err(format!("{} couldn't be renamed: {e}", name(&playing)));
        }
        if let Err(e) = rename_with(&passing, &playing) {
            let _ = rename_with(chosen, &playing);
            let _ = rename_with(&passing, chosen);
            return Err(format!("{} couldn't be renamed: {e}", name(chosen)));
        }
    }
    if kind == "directb2s" {
        aside(table, "b2spro")?;
    }
    Ok(())
}

/// The file of this kind named as the table kept beside it under the first number free, both kinds
/// counted. Its place, if there was one.
fn aside(table: &Path, kind: &str) -> Result<Option<PathBuf>, String> {
    let playing = playing(table, kind);
    if !playing.is_file() {
        return Ok(None);
    }
    let free = version_path(table, free_number(table), kind);
    rename_with(&playing, &free).map_err(|e| format!("{} couldn't be renamed: {e}", name(&playing)))?;
    Ok(Some(free))
}

/// No backglass file played for the table: those that played, of either kind, kept beside it under
/// the first numbers free. Their places.
pub fn play_none(table: &Path) -> Result<Vec<PathBuf>, String> {
    let mut kept = Vec::new();
    for kind in KINDS {
        kept.extend(aside(table, kind)?);
    }
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vpi-backglass-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_kept_backglass_chosen_and_none() {
        let d = scratch("one");
        let table = d.join("Blizzard.vpx");
        std::fs::write(&table, b"t").unwrap();
        std::fs::write(d.join("Blizzard.directb2s"), b"first").unwrap();
        std::fs::write(d.join("Blizzard (1).directb2s"), b"alternate").unwrap();
        let read = |n: &str| std::fs::read_to_string(d.join(n)).unwrap();
        // the kept one plays; the one that played takes its number
        play(&table, &d.join("Blizzard (1).directb2s")).unwrap();
        assert_eq!((read("Blizzard.directb2s"), read("Blizzard (1).directb2s")), ("alternate".into(), "first".into()));
        // none: the one playing kept under the first number free
        assert_eq!(play_none(&table).unwrap(), [d.join("Blizzard (2).directb2s")]);
        assert!(!d.join("Blizzard.directb2s").exists());
        // and one chosen again with none playing
        play(&table, &d.join("Blizzard (1).directb2s")).unwrap();
        assert_eq!(read("Blizzard.directb2s"), "first");
        // another table's, or a file not numbered for it, is refused
        std::fs::write(d.join("Other (1).directb2s"), b"x").unwrap();
        assert!(play(&table, &d.join("Other (1).directb2s")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn b2s_pros_own_beside_b2ss_numbered_together() {
        let d = scratch("pro");
        let table = d.join("Twister.vpx");
        std::fs::write(&table, b"t").unwrap();
        std::fs::write(d.join("Twister.directb2s"), b"b2s").unwrap();
        std::fs::write(d.join("Twister (1).B2SPro"), b"pro").unwrap();
        std::fs::write(d.join("Twister (1).B2SPro.displays.xml"), b"places").unwrap();
        let read = |n: &str| std::fs::read_to_string(d.join(n)).unwrap();
        // a .B2SPro chosen plays in B2S Pro, its places with it; the .directb2s stays for the rest
        play(&table, &d.join("Twister (1).B2SPro")).unwrap();
        assert_eq!(read("Twister.B2SPro"), "pro");
        assert_eq!(read("Twister.B2SPro.displays.xml"), "places");
        assert_eq!(read("Twister.directb2s"), "b2s");
        assert_eq!(playing(&table, "b2spro"), d.join("Twister.B2SPro"));
        // a .directb2s chosen plays everywhere: the .B2SPro numbered out of B2S Pro's way, under a
        // number neither kind holds
        std::fs::write(d.join("Twister (1).directb2s"), b"other b2s").unwrap();
        play(&table, &d.join("Twister (1).directb2s")).unwrap();
        assert_eq!((read("Twister.directb2s"), read("Twister (1).directb2s")), ("other b2s".into(), "b2s".into()));
        assert!(!d.join("Twister.B2SPro").exists());
        assert_eq!(read("Twister (2).B2SPro"), "pro");
        assert_eq!(read("Twister (2).B2SPro.displays.xml"), "places");
        // none: both kinds kept aside
        play(&table, &d.join("Twister (2).B2SPro")).unwrap();
        assert_eq!(play_none(&table).unwrap(), [d.join("Twister (2).directb2s"), d.join("Twister (3).B2SPro")]);
        assert_eq!(kind_of(&d.join("x.B2SPRO")), Some("b2spro"));
        assert_eq!(kind_of(&d.join("x.b2s")), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
