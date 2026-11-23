//! A copy of a table with some of its pictures swapped for the player's own, as VPX plays it: VPX
//! reads no folder of pictures beside a table, and a picture a script loads at run time is drawn
//! plain grey (builds 5436 and 6681), so the pictures go into a copy of the table file itself. The
//! table is read, each picture named as one of the table's own (`<table> pictures\backdrop2.png`)
//! put in its place — its file as it is, in whatever format, its size from its header — and the
//! copy written beside the table (`vpin`, its checksum as VPX checks it), the table's own files
//! named for it (its backglass, settings, views, script, physics) copied under the copy's name so
//! it plays as the table does. The table itself is never touched.

use std::path::{Path, PathBuf};

/// What a copy took.
#[derive(Debug, Default)]
pub struct Made {
    /// The copy's table file.
    pub table: PathBuf,
    /// The table's pictures swapped, by their names in the table.
    pub swapped: Vec<String>,
    /// Files of the folder naming no picture of the table's: left out.
    pub unknown: Vec<String>,
    /// The table's own files copied beside the copy.
    pub companions: Vec<PathBuf>,
}

/// The name the copy of `table` with the player's own pictures takes: `<table> (alt pictures)`.
pub fn copy_stem(table: &Path) -> String {
    format!("{} (alt pictures)", table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
}

/// The kinds of a table's own files beside it, named for it, that a copy takes too.
const COMPANIONS: [&str; 5] = ["directb2s", "ini", "pov", "vbs", "vpp"];

/// A copy of `table` with each of `pictures` in place of the table's picture of the same name (its
/// file's name without its ending, any case), written beside the table as `<stem>.vpx` with the
/// table's own files copied under that name. A copy there already is written over (the caller
/// sends the old one away first, where it keeps one).
pub fn make_copy(table: &Path, pictures: &[PathBuf], stem: &str) -> Result<Made, String> {
    let dir = table.parent().ok_or("The table's folder isn't known.")?;
    let mut vpx = vpin::vpx::read(table).map_err(|e| format!("{}: {e}", table.display()))?;
    let mut made = Made { table: dir.join(format!("{stem}.vpx")), ..Made::default() };
    for picture in pictures {
        let name = picture.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let Some(img) = vpx.images.iter_mut().find(|i| i.name.eq_ignore_ascii_case(&name)) else {
            made.unknown.push(picture.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            continue;
        };
        let bytes = std::fs::read(picture).map_err(|e| format!("{}: {e}", picture.display()))?;
        // its size from its header, where it's a kind the header tells; VPX corrects it from the
        // decoded picture anyway
        if let Ok((w, h)) = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format().map_err(|_| ()).and_then(|r| r.into_dimensions().map_err(|_| ())) {
            img.width = w;
            img.height = h;
        }
        let path = picture.to_string_lossy().into_owned();
        img.path.clone_from(&path);
        img.jpeg = Some(vpin::vpx::pinbinary::PinBinary { name: img.name.clone(), internal_name: None, path, data: bytes });
        // the file as it is: no old bitmap, no link to the table's screenshot, and what VPX works
        // out from the picture itself (its hash, whether it's opaque or signed) left for VPX
        img.bits = None;
        img.link = None;
        img.md5_hash = None;
        img.is_opaque = None;
        img.is_signed = None;
        made.swapped.push(img.name.clone());
    }
    if made.swapped.is_empty() {
        return Err("None of the pictures is named as one of the table's own: nothing to swap.".into());
    }
    vpin::vpx::write(&made.table, &vpx).map_err(|e| format!("{}: {e}", made.table.display()))?;
    // the table's own files beside it, under the copy's name
    let own = table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    for ext in COMPANIONS {
        let from = dir.join(format!("{own}.{ext}"));
        if from.is_file() {
            let to = dir.join(format!("{stem}.{ext}"));
            std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
            made.companions.push(to);
        }
    }
    Ok(made)
}

/// The table's pictures by name, for checking a folder against them: each picture's name.
pub fn picture_names(table: &Path) -> Result<Vec<String>, String> {
    let vpx = vpin::vpx::read(table).map_err(|e| format!("{}: {e}", table.display()))?;
    Ok(vpx.images.iter().map(|i| i.name.clone()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_copys_name() {
        assert_eq!(copy_stem(Path::new("C:/Tables/Leprechaun King (Orbital 2020).vpx")), "Leprechaun King (Orbital 2020) (alt pictures)");
    }
}
