//! Lists the part groups of VPX 10.8.1 tables — the views each group's parts show in (VPX's
//! player mode visibility mask) and the space they're placed in — with how many parts each holds
//! directly, for finding the tables whose parts show only in some views or sit outside the
//! playfield: `cargo run --release -p player --example part_groups -- <table.vpx or folder>...`.
//! (vpin 0.38.4 doesn't read a group's own parent group yet.)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use vpin::vpx::gameitem::GameItemEnum;

fn tables(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
            tables(&entry.path(), out);
        }
    } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vpx")) {
        out.push(path.to_path_buf());
    }
}

/// The views a mask names: desktop, full single screen, cabinet, mixed and virtual reality.
fn views(mask: u32) -> String {
    if mask & 0x1F == 0x1F {
        return "all".into();
    }
    let names = [(1, "desktop"), (2, "fss"), (4, "cabinet"), (8, "mixed reality"), (16, "vr")];
    let shown: Vec<&str> = names.iter().filter(|(bit, _)| mask & bit != 0).map(|(_, n)| *n).collect();
    if shown.is_empty() { "none".into() } else { shown.join(", ") }
}

fn main() {
    let mut paths = Vec::new();
    for a in std::env::args().skip(1) {
        tables(Path::new(&a), &mut paths);
    }
    paths.sort();
    for path in paths {
        let vpx = match vpin::vpx::read(&path) {
            Ok(v) => v,
            Err(e) => {
                println!("{}: could not read: {e}", path.display());
                continue;
            }
        };
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for item in &vpx.gameitems {
            if let Some(group) = item.part_group_name() {
                *counts.entry(group).or_default() += 1;
            }
        }
        let groups: Vec<_> = vpx
            .gameitems
            .iter()
            .filter_map(|i| match i {
                GameItemEnum::PartGroup(g) => Some(g),
                _ => None,
            })
            .collect();
        let special: Vec<_> = groups
            .iter()
            .filter(|g| {
                g.player_mode_visibility_mask.is_some_and(|m| m & 0x1F != 0x1F)
                    || !matches!(g.space_reference, vpin::vpx::gameitem::partgroup::SpaceReference::Playfield | vpin::vpx::gameitem::partgroup::SpaceReference::Inherit)
            })
            .collect();
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        println!("{name}: {} groups, {} limited to some views or outside the playfield", groups.len(), special.len());
        for g in special {
            println!(
                "    {}\tviews {}\tspace {:?}\t{} parts",
                g.name,
                g.player_mode_visibility_mask.map_or("all (unset)".into(), views),
                g.space_reference,
                counts.get(g.name.as_str()).copied().unwrap_or(0)
            );
        }
    }
}
