//! Our own part shapes: each part's 3D shape built from
//! VPX build 5436's own code, beside `vpin`'s glTF export, until every part
//! type is ported and proven. [`replace`] hands the
//! export's model the ported part types' geometry in place of its own — the
//! same nodes, materials and places — so the scene is built as before.
//! Ported so far: walls, rubbers, ramps, flippers, bumpers, gates,
//! spinners, kickers, triggers, targets, lights' bulbs, sockets and own
//! shapes, picture decals and primitives; and plungers, which the scene
//! takes as VPX's frames instead (`Scene::use_own_plungers`).

pub mod bulb;
mod bulb_mesh;
pub mod bumper;
mod bumper_mesh;
pub mod decal;
pub mod flipper;
mod flipper_mesh;
pub mod gate;
mod gate_mesh;
pub mod hit_target;
mod hit_target_mesh;
pub mod kicker;
pub mod kicker_mesh;
pub mod light;
pub mod plunger;
pub mod primitive;
pub mod ramp;
pub mod rubber;
pub mod spinner;
mod spinner_mesh;
pub mod trigger;
mod trigger_mesh;
pub mod wall;

use std::collections::HashMap;

use physics_compat::math::{Vertex3Ds, cross_product};
use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;

use crate::gltf::Document;

/// A piece of a part's shape, in VPX's table space (z up), its triangles
/// three indices each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// The ported parts' pieces, by the export's name for them (a wall's
/// `<name>Top` and `<name>Side`, a flipper's `<name>Base` and `<name>Rubber`,
/// a bumper's `<name>Base`, `<name>Socket`, `<name>Ring` and `<name>Cap`,
/// a gate's `<name>Bracket` and `<name>Wire`, a spinner's `<name>Bracket`
/// and `<name>Plate`, a kicker's `<name>Plate` and `<name>Kicker`, a
/// light's `<name>_bulb`, `<name>_socket` and `<name>_insert` (its own
/// shape, which its glow is drawn on), a trigger's, a target's, a
/// decal's and a primitive's own name). Parts can share a name (decals often do): the pieces
/// of a name are kept in the table's order, as the export's are.
pub fn shapes(vpx: &VPX) -> HashMap<String, Vec<Mesh>> {
    let mut out: HashMap<String, Vec<Mesh>> = HashMap::new();
    let mut add = |name: String, mesh: Mesh| out.entry(name).or_default().push(mesh);
    let materials = crate::look::Materials::new(vpx);
    let surfaces = table_physics::Surfaces::of_table(vpx);
    let groups = primitive::in_render_groups(vpx);
    for (i, item) in vpx.gameitems.iter().enumerate() {
        match item {
            GameItemEnum::Wall(w) => {
                let s = wall::shape(vpx, w);
                add(format!("{}Top", w.name), s.top);
                add(format!("{}Side", w.name), s.side);
            }
            GameItemEnum::Rubber(r) => {
                add(r.name.clone(), rubber::shape(r));
            }
            GameItemEnum::Ramp(r) => {
                add(r.name.clone(), ramp::shape(vpx, &materials, r));
            }
            GameItemEnum::Flipper(f) => {
                let s = flipper::shape(vpx, &surfaces, f);
                add(format!("{}Base", f.name), s.base);
                if let Some(rubber) = s.rubber {
                    add(format!("{}Rubber", f.name), rubber);
                }
            }
            GameItemEnum::Bumper(b) => {
                let s = bumper::shape(&surfaces, b);
                add(format!("{}Base", b.name), s.base);
                add(format!("{}Socket", b.name), s.socket);
                add(format!("{}Ring", b.name), s.ring);
                add(format!("{}Cap", b.name), s.cap);
            }
            GameItemEnum::Gate(g) => {
                let s = gate::shape(&surfaces, g);
                add(format!("{}Bracket", g.name), s.bracket);
                add(format!("{}Wire", g.name), s.wire);
            }
            GameItemEnum::Spinner(sp) => {
                let s = spinner::shape(&surfaces, sp);
                add(format!("{}Bracket", sp.name), s.bracket);
                add(format!("{}Plate", sp.name), s.plate);
            }
            GameItemEnum::Kicker(k) => {
                if let Some(s) = kicker::shape(&surfaces, k) {
                    add(format!("{}Plate", k.name), s.plate);
                    add(format!("{}Kicker", k.name), s.kicker);
                }
            }
            GameItemEnum::Trigger(t) => {
                if let Some(s) = trigger::shape(&surfaces, t) {
                    add(t.name.clone(), s);
                }
            }
            GameItemEnum::HitTarget(t) => {
                add(t.name.clone(), hit_target::shape(t));
            }
            GameItemEnum::Primitive(p) => {
                if let Some(s) = primitive::shape(p, groups.contains(&i)) {
                    add(p.name.clone(), s);
                }
            }
            GameItemEnum::Light(l) => {
                // VPX's bulb rules for older tables: classic lights and hidden lights had none
                if l.show_bulb_mesh && (vpx.version.u32() >= 1080 || (l.is_bulb_light && l.visible.unwrap_or(true))) {
                    let s = bulb::shape(&surfaces, l);
                    add(format!("{}_bulb", l.name), s.bulb);
                    add(format!("{}_socket", l.name), s.socket);
                }
                if let Some(s) = light::shape(vpx, &surfaces, l) {
                    add(format!("{}_insert", l.name), s);
                }
            }
            // only the decals the export draws too (VPX also draws a picture decal without
            // its picture, in its material's colour), so that pieces of a name pair up
            GameItemEnum::Decal(d) if !d.image.is_empty() => {
                if let Some(s) = decal::shape(vpx, &surfaces, d) {
                    add(d.name.clone(), s);
                }
            }
            _ => {}
        }
    }
    out
}

/// Our pieces for the model's nodes: the k-th node with a mesh and a given
/// name takes the k-th of our pieces of that name. By node index.
pub fn matched<'a>(shapes: &'a HashMap<String, Vec<Mesh>>, nodes: &[crate::gltf::Node]) -> Vec<(usize, &'a Mesh)> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut out = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let (Some(_), Some(list)) = (node.mesh, shapes.get(&node.name)) else { continue };
        let k = seen.entry(node.name.as_str()).or_insert(0);
        if let Some(mesh) = list.get(*k) {
            out.push((i, mesh));
        }
        *k += 1;
    }
    out
}

/// The table's scene as the player builds it: from our own model of the
/// table, our shapes and the table file alone, when `own` (the player's
/// default; `crate::model`); else from the export's model
/// and shapes, the backup (`--shapes export`).
pub fn scene(vpx: &mut VPX, own: bool) -> Result<crate::scene::Scene, String> {
    let (doc, pictures) = if own { crate::model::load(vpx)? } else { crate::table_meshes::load(vpx)? };
    let mut scene = crate::scene::Scene::build(vpx, &doc, pictures);
    if own {
        scene.use_own_plungers(vpx);
    }
    Ok(scene)
}

/// The scene as it was built before our own model: the
/// export's model with our shapes in place of its pieces. For the checks
/// comparing the two.
pub fn scene_from_export(vpx: &mut VPX) -> Result<crate::scene::Scene, String> {
    let (mut doc, pictures) = crate::table_meshes::load(vpx)?;
    replace(vpx, &mut doc);
    let mut scene = crate::scene::Scene::build(vpx, &doc, pictures);
    scene.use_own_plungers(vpx);
    Ok(scene)
}

/// Gives the export's model our shapes where we have them: a node of that
/// name, with one piece, no turn of its own and the same scale every way (a
/// gate's wire is scaled by its length), keeps its material and place and
/// takes our geometry, in the export's axes (y up: y and z swapped). Returns
/// how many nodes took ours.
pub fn replace(vpx: &VPX, doc: &mut Document) -> usize {
    let shapes = shapes(vpx);
    let Document { nodes, meshes, .. } = doc;
    let mut replaced = 0;
    for (i, shape) in matched(&shapes, nodes) {
        let node = &nodes[i];
        let Some(m) = node.mesh else { continue };
        let [scale, sy, sz] = node.scale;
        if node.rotation != [0.0, 0.0, 0.0, 1.0] || scale != sy || scale != sz || scale == 0.0 {
            continue;
        }
        let Some(mesh) = meshes.get_mut(m) else { continue };
        let [piece] = mesh.primitives.as_mut_slice() else { continue };
        let [tx, ty, tz] = node.translation;
        piece.positions = shape.positions.iter().map(|&[x, y, z]| [(x - tx) / scale, (z - ty) / scale, (y - tz) / scale]).collect();
        piece.normals = shape.normals.iter().map(|&[x, y, z]| [x, z, y]).collect();
        piece.uvs = shape.uvs.clone();
        piece.indices = shape.indices.clone();
        replaced += 1;
    }
    replaced
}

/// VPX's `ComputeNormals`: each point's normal the sum of its triangles'
/// (each made unit length), made unit length itself; none where they cancel.
pub(crate) fn compute_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for t in indices.as_chunks::<3>().0 {
        let [a, b, c] = t.map(|i| positions[i as usize]);
        let e0 = Vertex3Ds::new(b[0] - a[0], b[1] - a[1], b[2] - a[2]);
        let e1 = Vertex3Ds::new(c[0] - a[0], c[1] - a[1], c[2] - a[2]);
        let mut normal = cross_product(&e0, &e1);
        normal.normalize_safe();
        for &i in t {
            let n = &mut normals[i as usize];
            n[0] += normal.x;
            n[1] += normal.y;
            n[2] += normal.z;
        }
    }
    for n in &mut normals {
        let l = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
        *n = if l <= f32::MIN_POSITIVE {
            [0.0; 3]
        } else {
            let inv = 1.0 / l.sqrt();
            [n[0] * inv, n[1] * inv, n[2] * inv]
        };
    }
    normals
}
