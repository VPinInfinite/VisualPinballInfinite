// The table's model, built from the table file alone: the pieces' shapes are our own
// ports of Visual Pinball X build 5436's code (`shapes`, GPL-3.0-or-later); which pieces a part has,
// their materials and pictures, and how they're laid out follow vpin 0.38.4's glTF export
// (src/vpx/export/gltf_export.rs, src/gltf.rs and src/vpx/compat.rs), which the player drew from until
// then, so the scene draws as it did. Those rules are vpin's, under its MIT license:
//
// Copyright (c) 2023 Francis De Brabandere
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all copies or
// substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
// NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
// OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! The table's model from the table file alone: every part's pieces, with
//! our own shapes (`shapes`), and the materials and pictures `vpin`'s glTF
//! export gave them, laid out as its nodes were (the same names, the same
//! order, the same groups under the editor's layers), so the scene is built
//! from it as it was from the export (`Scene::build`), without the export.
//!
//! The materials are the export's own reading of VPX's, which only the first
//! look draws with; VPX's look takes each piece's material from the table
//! file (`Draw::shading`). Each piece's picture is VPX's.

use std::collections::{BTreeMap, HashMap};

use physics_compat::math::Vertex2D;
use table_physics::Surfaces;
use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::gameitem::kicker::KickerType;
use vpin::vpx::gameitem::primitive::Primitive;
use vpin::vpx::material::MaterialType;

use crate::gltf::{self, AlphaMode, Document, Node};
use crate::scene::in_parallel;
use crate::shapes::{self, Mesh};
use crate::table_meshes::{self, Answer, Pictures};

/// One mesh of a part, as the export laid it out (its `NamedMesh`).
struct Piece {
    name: String,
    mesh: Mesh,
    material: Option<String>,
    texture: Option<String>,
    /// A colour of its own (lights' pieces, kickers'), with its alpha.
    tint: Option<[f32; 4]>,
    /// Light passing through it (from "disable lighting from below").
    transmission: Option<f32>,
    /// Where its node sits, in table space: the scene reads a moving part's
    /// pivot (a gate's, a spinner's axle), a bumper's skirt and a light's
    /// glow plane from it.
    origin: [f32; 3],
    visible: bool,
    /// The part it belongs to, by name (None: the implicit playfield).
    group: Option<String>,
}

/// A part, as the export groups its pieces: under the part's editor layer.
struct Group {
    name: String,
    layer: Option<String>,
}

/// A VPX material as the export reads it (`GltfMaterial`).
struct VpxMaterial {
    base_color: [f32; 4],
    metallic: f32,
    roughness: f32,
    opacity_active: bool,
}

/// The playfield's material when the table names none.
const PLAYFIELD_MATERIAL_NAME: &str = "__playfield__";

/// The table's model and its pictures, decoded meanwhile: what
/// [`table_meshes::load`] gives from the export, from the table file alone.
pub fn load(vpx: &VPX) -> Result<(Document, Pictures), String> {
    let n = vpx.images.len();
    let names: Vec<String> = vpx.images.iter().map(|i| i.name.to_lowercase()).collect();
    let wanted = table_meshes::export_picture_names(vpx);
    // where several pictures share a name, a part's is the one VPX finds (look::images_by_name)
    let by_name = crate::look::images_by_name(vpx);
    let candidate: Vec<bool> = (0..n).map(|i| wanted.contains(&names[i]) && by_name.get(&vpx.images[i].name.to_ascii_lowercase()) == Some(&(i as u32))).collect();
    // on every thread, each picture's answers (has it see-through texels? would the export have packed
    // it?), which decode a picture only when they must (then kept); the pieces meanwhile. The rest are
    // decoded as they're uploaded (`Image::pending`)
    let jobs: Vec<usize> = (0..n).collect();
    let (mut answers, built) = std::thread::scope(|scope| {
        let built = scope.spawn(|| pieces(vpx));
        let answers: Vec<Answer> = in_parallel(&jobs, |&i| table_meshes::answer(&vpx.images[i], candidate[i]));
        (answers, built.join())
    });
    let (pieces, groups) = built.map_err(|_| "building the table's pieces failed".to_string())?;
    let (doc, sources) = layout(vpx, pieces, groups, &answers);
    let pictures = Pictures { sources, material: answers.iter_mut().map(|a| a.picture.take()).collect(), script: (0..n).map(|_| None).collect() };
    Ok((doc, pictures))
}

/// The surface heights VPX places parts on.
fn height(surfaces: &Surfaces, name: &str, x: f32, y: f32) -> f32 {
    surfaces.height(name, Vertex2D::new(x, y))
}

/// A name, unless empty.
fn named(name: &str) -> Option<String> {
    (!name.is_empty()).then(|| name.to_string())
}

/// The export's light through a part (`calculate_transmission_factor`):
/// up to 30% where "disable lighting from below" is under 1.
fn transmission(disable_lighting_below: Option<f32>) -> Option<f32> {
    disable_lighting_below.filter(|&v| v < 1.0).map(|v| (1.0 - v) * 0.3)
}

/// `vpin`'s `material_opacity`: a material's opacity switch and opacity, by
/// name (the last of it), from 10.8's materials, else the older ones.
fn material_opacity(vpx: &VPX, name: &str) -> Option<(bool, f32)> {
    if name.is_empty() {
        return None;
    }
    if let Some(m) = vpx.gamedata.materials.as_ref().and_then(|all| all.iter().rev().find(|m| m.name.eq_ignore_ascii_case(name))) {
        return Some((m.opacity_active, m.opacity));
    }
    vpx.gamedata.materials_old.iter().rev().find(|m| m.name.eq_ignore_ascii_case(name)).map(|m| (m.opacity_active_edge_alpha & 1 != 0, m.opacity))
}

/// VPX's load for tables saved before 10.8 (`vpin`'s
/// `primitive_disable_lighting_below`): the playfield, and a primitive whose
/// material is opaque, get no light from below.
fn primitive_below(vpx: &VPX, p: &Primitive) -> Option<f32> {
    if vpx.version.u32() < 1080 {
        if p.is_playfield() {
            return Some(1.0);
        }
        let (active, opacity) = material_opacity(vpx, &p.material).unwrap_or((false, 1.0));
        if !active || opacity == 1.0 {
            return Some(1.0);
        }
    }
    p.disable_lighting_below
}

/// The export's name for a part's editor layer (`get_layer_name`).
fn layer_of(item: &GameItemEnum) -> Option<String> {
    match item.editor_layer_name() {
        Some(name) if !name.is_empty() => Some(format!("Layer_{name}")),
        _ => item.editor_layer().map(|layer| format!("Layer_{}", layer + 1)),
    }
}

/// Every part's pieces, in the table's order, as the export collected them
/// (`collect_meshes`), each with our own shape; and the parts, as it grouped
/// them. A flasher's piece is empty: the scene builds flashers itself.
fn pieces(vpx: &VPX) -> (Vec<Piece>, Vec<Group>) {
    let surfaces = Surfaces::of_table(vpx);
    let materials = crate::look::Materials::new(vpx);
    let render_groups = shapes::primitive::in_render_groups(vpx);
    let g = &vpx.gamedata;
    let older = vpx.version.u32() < 1080;
    let playfield_material = named(&g.playfield_material).unwrap_or_else(|| PLAYFIELD_MATERIAL_NAME.to_string());
    let (mut pieces, mut groups) = (Vec::new(), Vec::new());
    let mut explicit_playfield = false;
    let piece = |name: String, mesh: Mesh, material: Option<String>, texture: Option<String>, visible: bool, group: &str| Piece {
        name,
        mesh,
        material,
        texture,
        tint: None,
        transmission: None,
        origin: [0.0; 3],
        visible,
        group: Some(group.to_string()),
    };
    // a piece is drawn when it has triangles
    let solid = |m: &Mesh| !m.indices.is_empty();
    for (i, item) in vpx.gameitems.iter().enumerate() {
        let group = Group { name: item.name().to_string(), layer: layer_of(item) };
        match item {
            GameItemEnum::Primitive(p) => {
                let Some(mesh) = shapes::primitive::shape(p, render_groups.contains(&i)) else { continue };
                // the playfield takes the table's playfield material and picture
                let (material, texture) = if p.is_playfield() {
                    explicit_playfield = true;
                    (Some(playfield_material.clone()), named(&g.image))
                } else {
                    (named(&p.material), named(&p.image))
                };
                let visible = (older && p.is_playfield()) || p.is_visible;
                groups.push(group);
                pieces.push(Piece {
                    origin: [p.position.x, p.position.y, p.position.z],
                    transmission: transmission(primitive_below(vpx, p)),
                    ..piece(p.name.clone(), mesh, material, texture, visible, &p.name)
                });
            }
            GameItemEnum::Wall(w) => {
                let s = shapes::wall::shape(vpx, w);
                if !solid(&s.top) && !solid(&s.side) {
                    continue;
                }
                groups.push(group);
                let through = transmission(w.disable_lighting_below);
                if solid(&s.top) {
                    pieces.push(Piece { transmission: through, ..piece(format!("{}Top", w.name), s.top, named(&w.top_material), named(&w.image), w.is_top_bottom_visible, &w.name) });
                }
                if solid(&s.side) {
                    pieces.push(Piece {
                        transmission: through,
                        ..piece(format!("{}Side", w.name), s.side, named(&w.side_material), named(&w.side_image), w.is_side_visible, &w.name)
                    });
                }
            }
            GameItemEnum::Ramp(r) => {
                let mesh = shapes::ramp::shape(vpx, &materials, r);
                if (r.width_bottom == 0.0 && r.width_top == 0.0) || !solid(&mesh) {
                    continue;
                }
                groups.push(group);
                pieces.push(piece(r.name.clone(), mesh, named(&r.material), named(&r.image), r.is_visible, &r.name));
            }
            GameItemEnum::Rubber(r) => {
                if r.thickness == 0 || r.drag_points.len() < 2 {
                    continue;
                }
                let (mesh, middle) = shapes::rubber::shape_and_middle(r);
                if !solid(&mesh) {
                    continue;
                }
                groups.push(group);
                // (the export gives a rubber no picture; its node sits at the rubber's middle, lifted)
                pieces.push(Piece { origin: [middle[0], middle[1], r.height], ..piece(r.name.clone(), mesh, named(&r.material), None, r.is_visible, &r.name) });
            }
            GameItemEnum::Flipper(f) => {
                let s = shapes::flipper::shape(vpx, &surfaces, f);
                groups.push(group);
                // (the export's node sits on the playfield; a flipper turns about its upright axis)
                let origin = [f.center.x, f.center.y, 0.0];
                let texture = f.image.as_deref().and_then(named);
                pieces.push(Piece { origin, ..piece(format!("{}Base", f.name), s.base, named(&f.material), texture, f.is_visible, &f.name) });
                if let Some(rubber) = s.rubber {
                    pieces.push(Piece { origin, ..piece(format!("{}Rubber", f.name), rubber, named(&f.rubber_material), None, f.is_visible, &f.name) });
                }
            }
            GameItemEnum::Bumper(b) => {
                let s = shapes::bumper::shape(&surfaces, b);
                groups.push(group);
                let origin = [b.center.x, b.center.y, height(&surfaces, &b.surface, b.center.x, b.center.y)];
                let mut add = |suffix: &str, shown: bool, mesh: Mesh, material: Option<String>| {
                    if shown {
                        pieces.push(Piece { origin, ..piece(format!("{}{suffix}", b.name), mesh, material, None, true, &b.name) });
                    }
                };
                add("Base", b.is_base_visible, s.base, named(&b.base_material));
                add("Socket", b.is_socket_visible.unwrap_or(true), s.socket, named(&b.socket_material));
                add("Ring", b.is_ring_visible.unwrap_or(true), s.ring, b.ring_material.as_deref().and_then(named));
                add("Cap", b.is_cap_visible, s.cap, named(&b.cap_material));
            }
            GameItemEnum::Spinner(sp) => {
                let s = shapes::spinner::shape(&surfaces, sp);
                groups.push(group);
                let origin = [sp.center.x, sp.center.y, height(&surfaces, &sp.surface, sp.center.x, sp.center.y) + sp.height];
                if sp.show_bracket {
                    pieces.push(Piece { origin, ..piece(format!("{}Bracket", sp.name), s.bracket, None, None, sp.is_visible, &sp.name) });
                }
                pieces.push(Piece { origin, ..piece(format!("{}Plate", sp.name), s.plate, named(&sp.material), named(&sp.image), sp.is_visible, &sp.name) });
            }
            GameItemEnum::HitTarget(t) => {
                groups.push(group);
                let origin = [t.position.x, t.position.y, t.position.z];
                pieces.push(Piece { origin, ..piece(t.name.clone(), shapes::hit_target::shape(t), named(&t.material), named(&t.image), t.is_visible, &t.name) });
            }
            GameItemEnum::Gate(gt) => {
                let s = shapes::gate::shape(&surfaces, gt);
                groups.push(group);
                let origin = [gt.center.x, gt.center.y, height(&surfaces, &gt.surface, gt.center.x, gt.center.y) + gt.height];
                if gt.show_bracket {
                    pieces.push(Piece { origin, ..piece(format!("{}Bracket", gt.name), s.bracket, named(&gt.material), None, gt.is_visible, &gt.name) });
                }
                pieces.push(Piece { origin, ..piece(format!("{}Wire", gt.name), s.wire, named(&gt.material), None, gt.is_visible, &gt.name) });
            }
            GameItemEnum::Trigger(t) => {
                let Some(mesh) = shapes::trigger::shape(&surfaces, t) else { continue };
                groups.push(group);
                let origin = [t.center.x, t.center.y, height(&surfaces, &t.surface, t.center.x, t.center.y)];
                pieces.push(Piece { origin, ..piece(t.name.clone(), mesh, named(&t.material), None, t.is_visible, &t.name) });
            }
            GameItemEnum::Light(l) => {
                if l.is_backglass {
                    continue;
                }
                groups.push(group);
                let visible = l.visible.unwrap_or(true);
                let surface = height(&surfaces, &l.surface, l.center.x, l.center.y);
                // VPX's bulb rules for older tables: classic lights and hidden lights had none
                if l.show_bulb_mesh && (!older || (l.is_bulb_light && visible)) {
                    let s = shapes::bulb::shape(&surfaces, l);
                    let origin = [l.center.x, l.center.y, surface];
                    // VPX's glass is 20% opaque, its socket dark
                    pieces.push(Piece { origin, tint: Some([1.0, 1.0, 1.0, 0.2]), ..piece(format!("{}_bulb", l.name), s.bulb, None, None, visible, &l.name) });
                    pieces.push(Piece { origin, tint: Some([0.094, 0.094, 0.094, 1.0]), ..piece(format!("{}_socket", l.name), s.socket, None, None, visible, &l.name) });
                }
                if let Some(mesh) = shapes::light::shape(vpx, &surfaces, l) {
                    // a classic light shows its picture
                    let texture = if l.is_bulb_light { None } else { named(&l.image) };
                    let tint = [l.color.r, l.color.g, l.color.b].map(|c| f32::from(c) / 255.0);
                    // the node at the middle of its outline, on its plane
                    let (lo, hi) = mesh.positions.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| {
                        ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
                    });
                    pieces.push(Piece {
                        origin: [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, surface + 0.1],
                        tint: Some([tint[0], tint[1], tint[2], 0.3]),
                        ..piece(format!("{}_insert", l.name), mesh, None, texture, visible, &l.name)
                    });
                }
            }
            GameItemEnum::Plunger(p) => {
                groups.push(group);
                // VPX draws a plunger as one of its frames (the scene takes them all, `use_own_plungers`)
                let Some(s) = shapes::plunger::shape(&surfaces, p) else { continue };
                let Some(frame) = s.frames.into_iter().next() else { continue };
                let origin = [p.center.x, p.center.y, height(&surfaces, &p.surface, p.center.x, p.center.y) + p.z_adjust];
                pieces.push(Piece { origin, ..piece(format!("{}Rod", p.name), frame, named(&p.material), named(&p.image), p.is_visible, &p.name) });
            }
            GameItemEnum::Kicker(k) => {
                let Some(s) = shapes::kicker::shape(&surfaces, k) else { continue };
                groups.push(group);
                let origin = [k.center.x, k.center.y, height(&surfaces, &k.surface, k.center.x, k.center.y)];
                // the export's stand-ins for VPX's built-in kicker pictures
                let colour = match k.kicker_type {
                    KickerType::Cup | KickerType::Cup2 => [0.75, 0.75, 0.78, 1.0],
                    KickerType::Williams => [0.72, 0.53, 0.25, 1.0],
                    KickerType::Gottlieb => [0.45, 0.42, 0.40, 1.0],
                    _ => [0.36, 0.25, 0.15, 1.0],
                };
                pieces.push(Piece { origin, tint: Some([0.02, 0.02, 0.02, 1.0]), ..piece(format!("{}Plate", k.name), s.plate, named(&k.material), None, true, &k.name) });
                pieces.push(Piece { origin, tint: Some(colour), ..piece(format!("{}Kicker", k.name), s.kicker, named(&k.material), None, true, &k.name) });
            }
            // the scene builds flashers itself; the export listed them with their pictures, which the
            // scene decodes ahead with the parts' (an empty piece: never drawn from the model)
            GameItemEnum::Flasher(f) => {
                if f.drag_points.len() < 3 || crate::look::flasher_outline(f).len() < 3 {
                    continue;
                }
                groups.push(group);
                let tint = [f.color.r, f.color.g, f.color.b].map(|c| f32::from(c) / 255.0);
                pieces.push(Piece { tint: Some([tint[0], tint[1], tint[2], f.alpha as f32 / 100.0]), ..piece(f.name.clone(), Mesh::default(), None, named(&f.image_a), f.is_visible, &f.name) });
            }
            GameItemEnum::Decal(d) => {
                // (the export leaves out a picture decal without its picture; VPX draws it plain, in
                // its material's colour: Decal::Render's basic_without_texture)
                let Some(mesh) = shapes::decal::shape(vpx, &surfaces, d) else { continue };
                groups.push(group);
                let origin = [d.center.x, d.center.y, height(&surfaces, &d.surface, d.center.x, d.center.y) + 0.2];
                pieces.push(Piece { origin, ..piece(d.name.clone(), mesh, named(&d.material), named(&d.image), true, &d.name) });
            }
            _ => {}
        }
    }
    // VPX's own playfield when the table has none (player.cpp): a quad over the table at 0
    if !explicit_playfield {
        let (l, t, r, b) = (g.left, g.top, g.right, g.bottom);
        pieces.push(Piece {
            name: "playfield_mesh".to_string(),
            mesh: Mesh {
                positions: vec![[l, t, 0.0], [r, t, 0.0], [l, b, 0.0], [r, b, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 4],
                uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
                indices: vec![0, 1, 2, 2, 1, 3],
            },
            material: Some(playfield_material),
            texture: named(&g.image),
            tint: None,
            transmission: None,
            origin: [0.0; 3],
            visible: true,
            group: None,
        });
    }
    (pieces, groups)
}

/// VPX's materials as the export reads them (`collect_materials`), by name:
/// 10.8's, else the older ones; the last of a name.
fn vpx_materials(vpx: &VPX) -> BTreeMap<String, VpxMaterial> {
    let colour = |c: &vpin::vpx::color::Color, alpha: f32| [f32::from(c.r) / 255.0, f32::from(c.g) / 255.0, f32::from(c.b) / 255.0, alpha];
    let mut out = BTreeMap::new();
    if let Some(all) = &vpx.gamedata.materials {
        for m in all {
            out.insert(
                m.name.clone(),
                VpxMaterial {
                    base_color: colour(&m.base_color, if m.opacity_active { m.opacity } else { 1.0 }),
                    metallic: if m.type_ == MaterialType::Metal { 1.0 } else { 0.0 },
                    // VPX's roughness runs from matte to shiny, the export's the other way
                    roughness: 1.0 - m.roughness,
                    opacity_active: m.opacity_active,
                },
            );
        }
    } else {
        for m in &vpx.gamedata.materials_old {
            let active = m.opacity_active_edge_alpha & 1 != 0;
            out.insert(
                m.name.clone(),
                VpxMaterial {
                    base_color: colour(&m.base_color, if active { m.opacity } else { 1.0 }),
                    metallic: if m.is_metal { 1.0 } else { 0.0 },
                    roughness: 1.0 - m.roughness,
                    opacity_active: active,
                },
            );
        }
    }
    out
}

/// The export's playfield roughness (`calculate_playfield_roughness`): from
/// the table's playfield reflection strength when it's over 0.1.
fn playfield_roughness(reflection_strength: f32, fallback: f32) -> f32 {
    if reflection_strength > 0.1 { (0.20 - reflection_strength * 0.17).clamp(0.03, 0.20) } else { fallback }
}

/// The model as the export laid it out (`build_combined_gltf_payload`): its
/// pictures (the playfield's first, then each piece's in turn), its
/// materials (the playfield's, VPX's by name, then those the pieces make),
/// a node for each piece under its part, the parts under their editor
/// layers, and the table's two lights. With each picture's table image.
fn layout(vpx: &VPX, pieces: Vec<Piece>, groups: Vec<Group>, answers: &[Answer]) -> (Document, Vec<Option<usize>>) {
    let g = &vpx.gamedata;
    let vpx_materials = vpx_materials(vpx);
    // a piece's picture: the one VPX finds by its name, if the export could pack it
    let by_name = crate::look::images_by_name(vpx);
    let picture = |name: &str| by_name.get(&name.to_ascii_lowercase()).map(|&t| t as usize).filter(|&t| answers[t].packed);
    let image = |t: usize| gltf::Image { name: vpx.images[t].name.clone(), mime_type: String::new(), bytes: 0..0 };
    let material = |name: String, base_color: [f32; 4], image: Option<usize>, metallic: f32, roughness: f32, blend: bool, double_sided: bool| gltf::Material {
        name,
        base_color: base_color.map(|c| c.clamp(0.0, 1.0)),
        image,
        metallic,
        roughness,
        emissive: [0.0; 3],
        alpha_mode: if blend { AlphaMode::Blend } else { AlphaMode::Opaque },
        alpha_cutoff: 0.5,
        double_sided,
    };
    let (mut images, mut sources, mut materials) = (Vec::new(), Vec::new(), Vec::new());
    let mut by_name: HashMap<String, usize> = HashMap::new();

    // (the export makes the playfield's material first, with the playfield's picture, and a part
    // with no picture of its own but the playfield's material then shows the playfield's picture;
    // VPX draws it without, so VPX's materials stay plain, the playfield's included)
    // VPX's materials, by name (blended when their opacity is on and below 0.999)
    for (name, m) in &vpx_materials {
        if by_name.contains_key(name) {
            continue;
        }
        by_name.insert(name.clone(), materials.len());
        materials.push(material(name.clone(), m.base_color, None, m.metallic, m.roughness, m.opacity_active && m.base_color[3] < 0.999, false));
    }
    // each piece's picture, once
    let mut texture_of: HashMap<String, usize> = HashMap::new();
    for p in &pieces {
        let Some(name) = &p.texture else { continue };
        let key = name.to_lowercase();
        if !texture_of.contains_key(&key)
            && let Some(t) = picture(name)
        {
            texture_of.insert(key, images.len());
            images.push(image(t));
            sources.push(Some(t));
        }
    }
    let texture_alpha = |i: usize| sources[i].is_some_and(|t: usize| answers[t].see_through == Some(true));
    // the materials the pieces make: one of their own for a colour of their own, for light through
    // them, for a picture under a tinted or metal material; else one shared by each picture
    let mut of_piece: Vec<Option<usize>> = vec![None; pieces.len()];
    let mut shared: HashMap<String, usize> = HashMap::new();
    for (k, p) in pieces.iter().enumerate() {
        let vm = p.material.as_ref().and_then(|m| vpx_materials.get(m));
        let metallic = vm.map_or(0.0, |m| m.metallic);
        let roughness = if p.name.eq_ignore_ascii_case("playfield_mesh") {
            playfield_roughness(g.playfield_reflection_strength, vm.map_or(0.5, |m| m.roughness))
        } else {
            vm.map_or(0.5, |m| m.roughness)
        };
        let key = p.texture.as_ref().map(|t| t.to_lowercase());
        let texture = key.as_ref().and_then(|t| texture_of.get(t).copied());
        let blended = |m: &VpxMaterial| m.opacity_active && m.base_color[3] < 0.999;
        if let Some(tint) = p.tint {
            of_piece[k] = Some(materials.len());
            materials.push(material(format!("{}_color", p.name), tint, texture, metallic, roughness, true, true));
        } else if let Some(through) = p.transmission {
            if through > 0.0 {
                let (base_color, blend) = vm.map_or(([1.0; 4], false), |m| (m.base_color, blended(m)));
                let (image, blend, double_sided) = match texture {
                    Some(i) => (Some(i), blend || texture_alpha(i), true),
                    None => (None, blend, false),
                };
                of_piece[k] = Some(materials.len());
                materials.push(material(format!("{}_transmission", p.name), base_color, image, metallic, roughness, blend, double_sided));
            }
        } else if let Some(key) = key {
            match vm {
                // a tinted material's colour over the picture
                Some(m) if m.base_color[..3].iter().any(|&c| c < 0.99) => {
                    if let Some(i) = texture {
                        of_piece[k] = Some(materials.len());
                        materials.push(material(format!("{key}_tinted"), m.base_color, Some(i), m.metallic, m.roughness, blended(m) || texture_alpha(i), true));
                    }
                }
                // a metal material over the picture
                Some(m) if m.metallic > 0.0 => {
                    if let Some(i) = texture {
                        of_piece[k] = Some(materials.len());
                        materials.push(material(format!("{key}_metal"), [1.0; 4], Some(i), m.metallic, m.roughness, blended(m) || texture_alpha(i), true));
                    }
                }
                // the picture's shared material, made by the first piece to use it
                _ => {
                    if !shared.contains_key(&key)
                        && let Some(i) = texture
                    {
                        shared.insert(key.clone(), materials.len());
                        materials.push(material(format!("__texture__{key}"), [1.0; 4], Some(i), metallic, roughness, vm.is_some_and(blended) || texture_alpha(i), true));
                    }
                }
            }
        }
    }

    // a node for each piece, in the export's axes (y up), its points about the node's place
    let (mut nodes, mut meshes) = (Vec::new(), Vec::new());
    let mut root_children = Vec::new();
    let mut part_children: HashMap<String, Vec<usize>> = HashMap::new();
    for (k, p) in pieces.into_iter().enumerate() {
        // its own material, else its picture's, else (without a picture) VPX's of its name
        let material = of_piece[k].or_else(|| match &p.texture {
            Some(t) => shared.get(&t.to_lowercase()).copied(),
            None => p.material.as_ref().and_then(|m| by_name.get(m).copied()),
        });
        let [ox, oy, oz] = p.origin;
        let node = nodes.len();
        nodes.push(Node { name: p.name.clone(), mesh: Some(meshes.len()), children: Vec::new(), translation: [ox, oz, oy], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3], visible: p.visible });
        meshes.push(gltf::Mesh {
            name: p.name,
            primitives: vec![gltf::Primitive {
                positions: p.mesh.positions.iter().map(|&[x, y, z]| [x - ox, z - oz, y - oy]).collect(),
                normals: p.mesh.normals.iter().map(|&[x, y, z]| [x, z, y]).collect(),
                uvs: p.mesh.uvs,
                indices: p.mesh.indices,
                material,
            }],
        });
        match p.group {
            Some(part) => part_children.entry(part).or_default().push(node),
            None => root_children.push(node),
        }
    }
    // VPX's two lights over the table (Renderer::SetupShaders): at a third and two thirds of its length
    for (k, along) in [(0, 1.0 / 3.0), (1, 2.0 / 3.0)] {
        root_children.push(nodes.len());
        nodes.push(Node {
            name: format!("TableLight{k}"),
            mesh: None,
            children: Vec::new(),
            translation: [(g.left + g.right) / 2.0, g.light_height, g.bottom * along],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            visible: true,
        });
    }
    // each part: its pieces under a node of its own (one piece alone), under its editor layer
    let mut layers: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for group in groups {
        let Some(children) = part_children.remove(&group.name).filter(|c| !c.is_empty()) else { continue };
        let node = if let [only] = children[..] {
            only
        } else {
            nodes.push(Node { name: group.name, mesh: None, children, translation: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3], visible: true });
            nodes.len() - 1
        };
        match group.layer {
            Some(layer) => layers.entry(layer).or_default().push(node),
            None => root_children.push(node),
        }
    }
    for (layer, children) in layers {
        root_children.push(nodes.len());
        nodes.push(Node { name: layer, mesh: None, children, translation: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3], visible: true });
    }
    let root = nodes.len();
    nodes.push(Node {
        name: if g.name.is_empty() { "Table".to_string() } else { g.name.clone() },
        mesh: None,
        children: root_children,
        translation: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
        visible: true,
    });
    (Document { nodes, meshes, materials, images, roots: vec![root], bin: Vec::new() }, sources)
}
