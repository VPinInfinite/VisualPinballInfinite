//! The colliders built from real tables, compared with the ones VPX builds:
//! the reference tester's table dumps (`vpx-build/table-dumps/`, made by the
//! tester built with exact float rules). Each part's colliders must match bit for bit, and so must the
//! quadtree's order of every collider of the table.
//!
//! Two variants reach code none of the test tables uses: every primitive mesh
//! reordered as VPX reorders the meshes of tables saved before 10.1.1, and
//! every primitive mesh (up to 20,000 corners) given a collision reduction
//! factor, so VPX simplifies it. The tester makes the same changes to its
//! tables before dumping (`VPI_TABLES_FORSYTH`, `VPI_TABLES_REDUCTION`).
//!
//! Two exceptions:
//! - A mesh edge of zero length holds "not a number" values in VPX and here
//!   alike, but their sign bit depends on the instructions the compiler
//!   chose. Nothing reads it (such an edge can never be hit), so NaNs compare
//!   equal whatever their bits.
//! - Where the quadtree runs out of nodes, VPX's own order varies from run to
//!   run (it builds large subtrees on worker threads that share the nodes;
//!   five runs on one table gave five orders). There the colliders must still
//!   match, but their order can't be compared.
//!
//! Needs the tables (the test set's 21) and the dumps, which stay
//! on the development machine; a check passes without checking anything where
//! they are missing.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use physics_compat::dump::collider_line;
use physics_compat::engine::{PhysicsEngine, PlayfieldMaterial};
use physics_compat::env::PartId;
use physics_compat::fpmode::VpxFloatMode;
use physics_compat::quadtree::{Owner, OwnerKind};
use physics_compat::setup::forsyth::reorder_forsyth;
use table_physics::settings::PhysicsSettings;
use table_physics::{TABLE, build};
use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;

/// Where the test set's tables are: the reference
/// install's Tables folder, VPX's sample tables, and FlexDMD's demo.
const TABLE_DIRS: [&str; 3] =
    [r"C:\vPinball\VisualPinball\Tables", r"C:\vPinball\VisualPinball\assets", r"C:\vPinball\VisualPinball\VPinMAME"];

/// The collision reduction factor of the simplified-mesh variant, and the
/// largest mesh it applies to (the simplification takes time quadratic in a
/// mesh's corners).
const REDUCTION: f32 = 0.6;
const REDUCTION_MAX_VERTICES: u32 = 20000;

fn dumps_dir(variant: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vpx-build/table-dumps").join(variant)
}

/// A tester dump: part names by index, and collider lines in quadtree order.
struct Dump {
    part_names: Vec<String>,
    lines: Vec<String>,
}

fn read_dump(path: &Path) -> Dump {
    let text = std::fs::read_to_string(path).expect("dump");
    let mut part_names = Vec::new();
    let mut lines = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("part ") {
            // "part <index> <type> <name>"
            let mut f = rest.splitn(3, ' ');
            let index: usize = f.next().unwrap().parse().unwrap();
            assert_eq!(index, part_names.len());
            f.next();
            part_names.push(f.next().unwrap_or("").to_string());
        } else if line.starts_with("c ") {
            lines.push(line.to_string());
        }
    }
    Dump { part_names, lines }
}

/// A collider line with every NaN written the same way.
fn same_nans(line: &str) -> String {
    line.split(' ')
        .map(|t| match u32::from_str_radix(t, 16) {
            Ok(bits) if t.len() == 8 && f32::from_bits(bits).is_nan() => "nan",
            _ => t,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The part a collider line belongs to (its third field).
fn line_part(line: &str) -> i32 {
    line.split(' ').nth(2).unwrap().parse().unwrap()
}

/// Every table that has a dump in `variant`, changed by `modify` as the
/// tester changed it, against that dump. Returns the mismatches.
fn compare_tables(variant: &str, modify: impl Fn(&mut VPX)) -> Vec<String> {
    let dumps = dumps_dir(variant);
    if !Path::new(TABLE_DIRS[0]).is_dir() || !dumps.is_dir() {
        eprintln!("{variant}: tables or dumps missing, nothing checked");
        return Vec::new();
    }
    let _mode = VpxFloatMode::enter();
    let settings = PhysicsSettings::default();

    let mut failures = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dumps).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for dump_path in entries {
        let stem = dump_path.file_stem().unwrap().to_string_lossy().to_string();
        let Some(vpx_path) =
            TABLE_DIRS.iter().map(|d| Path::new(d).join(format!("{stem}.vpx"))).find(|p| p.is_file())
        else {
            continue;
        };
        let dump = read_dump(&dump_path);
        let mut vpx = vpin::vpx::read(&vpx_path).expect("table");
        modify(&mut vpx);
        let tp = build(&vpx, &settings);

        // our part ids -> the dump's part indices, by name
        let dump_index: HashMap<&str, i32> =
            dump.part_names.iter().enumerate().map(|(i, n)| (n.as_str(), i as i32)).collect();
        let names: Vec<&str> = vpx.gameitems.iter().map(|g| g.name()).collect();
        let index = |p: PartId| -> i32 {
            if p == TABLE {
                -1
            } else {
                *dump_index.get(names[p.0 as usize]).unwrap_or(&-3)
            }
        };

        let not_built = tp.not_built.len();
        // primitives whose collision mesh VPX simplifies (progressive mesh)
        let simplified = vpx
            .gameitems
            .iter()
            .filter(|g| match g {
                GameItemEnum::Primitive(p) => {
                    let n = p.num_vertices.unwrap_or(0) as f64;
                    let f = p.collision_reduction_factor.unwrap_or(0.0);
                    let e = (1.0 - f).clamp(0.0, 1.0) * 0.25 + 0.75;
                    p.use_3d_mesh && !p.is_toy && ((n.powf(e as f64) as u32).max(420) as f64) < n
                }
                _ => false,
            })
            .count();
        let playfield = PlayfieldMaterial { friction: 0.0, elasticity: 0.0, elasticity_falloff: 0.0, scatter_deg: 0.0 };
        let engine =
            PhysicsEngine::new(&tp.table, &playfield, Owner { id: TABLE.0, kind: OwnerKind::Other }, tp.colliders, vec![]);
        let ours: Vec<String> = engine.colliders()[2..].iter().map(|c| same_nans(&collider_line(c, index))).collect();
        let dump_lines: Vec<String> = dump.lines.iter().map(|l| same_nans(l)).collect();

        // part by part, ignoring order
        let mut by_part_ours: BTreeMap<i32, Vec<&str>> = BTreeMap::new();
        for l in &ours {
            by_part_ours.entry(line_part(l)).or_default().push(l);
        }
        let mut by_part_vpx: BTreeMap<i32, Vec<&str>> = BTreeMap::new();
        for l in &dump_lines {
            by_part_vpx.entry(line_part(l)).or_default().push(l);
        }
        let built: Vec<i32> = by_part_ours.keys().copied().collect();
        let (mut same, mut differ) = (0, Vec::new());
        for p in &built {
            let mut a = by_part_ours[p].clone();
            let mut b = by_part_vpx.get(p).cloned().unwrap_or_default();
            a.sort();
            b.sort();
            if a == b {
                same += 1;
            } else {
                differ.push(*p);
            }
        }
        // parts VPX built colliders for that we built none for (other than the ones not built yet)
        let not_built_names: Vec<&str> = tp.not_built.iter().map(|(i, _)| names[*i]).collect();
        let missing: Vec<i32> = by_part_vpx
            .keys()
            .copied()
            .filter(|p| !by_part_ours.contains_key(p))
            .filter(|p| *p < 0 || !not_built_names.contains(&dump.part_names[*p as usize].as_str()))
            .collect();

        eprintln!(
            "{variant} {stem} ({}): {same} of {} parts match{}{}{}{}",
            vpx.version,
            built.len(),
            if simplified == 0 { String::new() } else { format!(" ({simplified} simplified meshes)") },
            if differ.is_empty() { String::new() } else { format!(", {} differ", differ.len()) },
            if missing.is_empty() { String::new() } else { format!(", {} missing", missing.len()) },
            if not_built == 0 { String::new() } else { format!(", {not_built} parts not built yet") },
        );
        for p in differ.iter().take(3) {
            let name = if *p < 0 { "(table)" } else { dump.part_names[*p as usize].as_str() };
            let mut a = by_part_ours[p].clone();
            let mut b = by_part_vpx.get(p).cloned().unwrap_or_default();
            a.sort();
            b.sort();
            eprintln!("   part {p} {name}: ours {} colliders, VPX {}", a.len(), b.len());
            if let Some((x, y)) = a.iter().zip(&b).find(|(x, y)| x != y) {
                eprintln!("      ours {x}\n      VPX  {y}");
            }
            failures.push(format!("{variant} {stem}: part {name}"));
        }
        for p in missing.iter().take(3) {
            let name = if *p < 0 { "(table)" } else { dump.part_names[*p as usize].as_str() };
            eprintln!("   part {p} {name}: VPX has {} colliders, we built none", by_part_vpx[p].len());
            failures.push(format!("{variant} {stem}: missing {name}"));
        }

        // the whole table, in quadtree order, once every part is built (unless VPX's own order varies)
        if engine.quadtree_pool_ran_out() {
            eprintln!("   the quadtree ran out of nodes: VPX's own order varies here, so it isn't compared");
        } else if not_built == 0 && differ.is_empty() && missing.is_empty() {
            let order: Vec<&String> = engine.tree_order().map(|i| &ours[i as usize - 2]).collect();
            match order.iter().zip(&dump_lines).position(|(a, b)| *a != b) {
                None if order.len() == dump_lines.len() => eprintln!("   quadtree order matches"),
                at => {
                    eprintln!("   quadtree order differs at {at:?} ({} vs {})", order.len(), dump_lines.len());
                    if let Some(k) = at {
                        for j in k..(k + 3).min(order.len()) {
                            eprintln!("      ours {}\n      VPX  {}", order[j], dump_lines[j]);
                        }
                    }
                    failures.push(format!("{variant} {stem}: quadtree order"));
                }
            }
        }
    }
    failures
}

#[test]
fn table_colliders_match_exact_vpx() {
    let failures = compare_tables("precise", |_| {});
    assert!(failures.is_empty(), "{} mismatches: {failures:?}", failures.len());
}

/// Every primitive mesh's triangles reordered, as VPX does to tables saved
/// before 10.1.1 as it loads them. The mesh is stored back uncompressed.
fn reorder_meshes(vpx: &mut VPX) {
    for g in vpx.gameitems.iter_mut() {
        let GameItemEnum::Primitive(p) = g else { continue };
        if !p.use_3d_mesh {
            continue;
        }
        let Ok(Some(m)) = p.read_mesh() else { continue };
        let n = m.vertices.len();
        let mut indices: Vec<u32> = m.indices.iter().flat_map(|t| [t.i0 as u32, t.i1 as u32, t.i2 as u32]).collect();
        if indices.iter().any(|&i| i as usize >= n) {
            continue;
        }
        if let Some(r) = reorder_forsyth(&indices, n) {
            indices = r;
        }
        p.vertices_data = Some(m.vertices.iter().flat_map(|w| w.vpx_encoded_vertex).collect());
        p.indices_data = Some(if n > 65535 {
            indices.iter().flat_map(|i| i.to_le_bytes()).collect()
        } else {
            indices.iter().flat_map(|&i| (i as u16).to_le_bytes()).collect()
        });
        p.compressed_vertices_data = None;
        p.compressed_indices_data = None;
        p.compressed_vertices_len = None;
        p.compressed_indices_len = None;
    }
}

#[test]
fn reordered_meshes_match_exact_vpx() {
    let failures = compare_tables("precise-forsyth", reorder_meshes);
    assert!(failures.is_empty(), "{} mismatches: {failures:?}", failures.len());
}

/// Every primitive mesh of at most `REDUCTION_MAX_VERTICES` corners given the
/// collision reduction factor `REDUCTION`.
fn reduce_meshes(vpx: &mut VPX) {
    for g in vpx.gameitems.iter_mut() {
        if let GameItemEnum::Primitive(p) = g
            && p.use_3d_mesh
            && p.num_vertices.unwrap_or(0) <= REDUCTION_MAX_VERTICES
        {
            p.collision_reduction_factor = Some(REDUCTION);
        }
    }
}

#[test]
fn simplified_meshes_match_exact_vpx() {
    let failures = compare_tables("precise-reduced", reduce_meshes);
    assert!(failures.is_empty(), "{} mismatches: {failures:?}", failures.len());
}
