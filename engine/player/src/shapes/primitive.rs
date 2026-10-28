// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/primitive.cpp (Primitive::CalculateBuiltinOriginal, RecalculateMatrices, and
// RenderSetup's render groups) and src/shaders/bgfx/vs_basic.sc (a part's normals through
// its matrix's inverse transpose).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A primitive's shape as VPX draws it at rest: its own mesh (read from the
//! table file by `vpin`) or VPX's generated prism, through VPX's matrix —
//! scaled, moved and turned by its two sets of angles, then placed. VPX's
//! shader turns its normals by the matrix's inverse transpose, so they stay
//! square to a primitive scaled unevenly; but primitives VPX draws together
//! in a render group (a collection that groups its elements) are turned on
//! the processor by the matrix itself, as VPX does them.

use std::collections::HashSet;

use physics_compat::math::def::ang_to_rad;
use physics_compat::math::{Matrix3D, Vertex3Ds, cross_product};
use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::gameitem::primitive::Primitive;

use super::Mesh;

/// VPX's `CalculateBuiltinOriginal`: a prism of `sides` sides around the
/// z axis, a unit across and high, its picture laid out in quarters (top,
/// bottom, sides); drawn from inside too when `inside`.
pub fn builtin(sides: u32, inside: bool) -> Mesh {
    let n = sides as usize;
    let s = sides as f32;
    let outer_radius = -0.5 / (std::f32::consts::PI / s).cos();
    let add_angle = (2.0 * std::f64::consts::PI) as f32 / s;
    let offs_angle = std::f32::consts::PI / s;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, -f32::MAX, -f32::MAX);
    // x, y, z, nx, ny, nz, tu, tv
    let mut v = vec![[0.0f32; 8]; 4 * n + 2];
    v[0] = [0.0, 0.0, 0.5, 0.0, 0.0, 1.0, 0.0, 0.0];
    v[n + 1] = [0.0, 0.0, -0.5, 0.0, 0.0, -1.0, 0.0, 0.0];
    for i in 0..n {
        let angle = add_angle * i as f32 + offs_angle;
        let (x, y) = (angle.sin() * outer_radius, angle.cos() * outer_radius);
        v[i + 1] = [x, y, 0.5, 0.0, 0.0, 1.0, 0.0, 0.0];
        v[i + 1 + n + 1] = [x, y, -0.5, 0.0, 0.0, -1.0, 0.0, 0.0];
        v[n * 2 + 2 + i] = [x, y, 0.5, angle.sin(), angle.cos(), 0.0, 0.0, 0.0];
        v[n * 3 + 2 + i] = [x, y, -0.5, angle.sin(), angle.cos(), 0.0, 0.0, 0.0];
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    // the picture: top and bottom in the upper quarters, the sides below
    v[0][6..8].copy_from_slice(&[0.25, 0.25]);
    v[n + 1][6..8].copy_from_slice(&[(0.25 * 3.0_f64) as f32, 0.25]);
    let (inv_x, inv_y, inv_s) = (0.5 / (max_x - min_x), 0.5 / (max_y - min_y), 1.0 / s);
    for i in 0..n {
        let (tu, tv) = ((v[i + 1][0] - min_x) * inv_x, (v[i + 1][1] - min_y) * inv_y);
        v[i + 1][6..8].copy_from_slice(&[tu, tv]);
        v[i + 1 + n + 1][6..8].copy_from_slice(&[tu + 0.5, tv]);
        v[n * 2 + 2 + i][6..8].copy_from_slice(&[i as f32 * inv_s, 0.5]);
        v[n * 3 + 2 + i][6..8].copy_from_slice(&[i as f32 * inv_s, 1.0]);
    }
    let mut indices = vec![0u32; if inside { n * 24 } else { n * 12 }];
    let (n32, ix) = (sides, &mut indices);
    for i in 0..n {
        let i32_ = i as u32;
        let tmp = if i == n - 1 { 1 } else { i32_ + 2 }; // wrapping around
        let tmp2 = tmp + 1;
        if inside {
            // everything twice, once each way
            ix[i * 6..i * 6 + 6].copy_from_slice(&[0, i32_ + 1, tmp, 0, tmp, i32_ + 1]);
            let b = 6 * (i + n);
            ix[b..b + 6].copy_from_slice(&[n32 + 1, n32 + tmp2, n32 + 2 + i32_, n32 + 1, n32 + 2 + i32_, n32 + tmp2]);
            let k = 12 * (i + n);
            ix[k..k + 12].copy_from_slice(&[
                n32 * 2 + tmp2,
                n32 * 2 + 2 + i32_,
                n32 * 3 + 2 + i32_,
                n32 * 2 + tmp2,
                n32 * 3 + 2 + i32_,
                n32 * 3 + tmp2,
                n32 * 2 + tmp2,
                n32 * 3 + 2 + i32_,
                n32 * 2 + 2 + i32_,
                n32 * 2 + tmp2,
                n32 * 3 + tmp2,
                n32 * 3 + 2 + i32_,
            ]);
        } else {
            // only the faces turned out
            ix[i * 3..i * 3 + 3].copy_from_slice(&[0, tmp, i32_ + 1]);
            let b = 3 * (i + n);
            ix[b..b + 3].copy_from_slice(&[n32 + 1, n32 + 2 + i32_, n32 + tmp2]);
            let k = 6 * (i + n);
            ix[k..k + 6].copy_from_slice(&[n32 * 2 + tmp2, n32 * 3 + 2 + i32_, n32 * 2 + 2 + i32_, n32 * 2 + tmp2, n32 * 3 + tmp2, n32 * 3 + 2 + i32_]);
        }
    }
    Mesh {
        positions: v.iter().map(|p| [p[0], p[1], p[2]]).collect(),
        normals: v.iter().map(|p| [p[3], p[4], p[5]]).collect(),
        uvs: v.iter().map(|p| [p[6], p[7]]).collect(),
        indices,
    }
}

/// VPX's `RecalculateMatrices`: scaled, moved by its own offset and turned
/// (z, y, x), turned again by its object angles (z, y, x), then placed.
pub fn matrix(p: &Primitive) -> Matrix3D {
    let r = &p.rot_and_tra;
    let rt = ((Matrix3D::translate(r[3], r[4], r[5]) * Matrix3D::rotate_z(ang_to_rad(r[2]))) * Matrix3D::rotate_y(ang_to_rad(r[1])))
        * Matrix3D::rotate_x(ang_to_rad(r[0]));
    let rt = ((rt * Matrix3D::rotate_z(ang_to_rad(r[8]))) * Matrix3D::rotate_y(ang_to_rad(r[7]))) * Matrix3D::rotate_x(ang_to_rad(r[6]));
    (Matrix3D::scale(p.size.x, p.size.y, p.size.z) * rt) * Matrix3D::translate(p.position.x, p.position.y, p.position.z)
}

/// The primitives VPX draws in render groups (`RenderSetup`): those that
/// join, with at least one other, a collection's group — a collection that
/// groups its elements, its moving primitives with a mesh of their own, of
/// the first one's material and picture (VPX's quirk kept: a first without
/// a picture lets the next one's set the group's picture).
pub fn in_render_groups(vpx: &VPX) -> HashSet<usize> {
    let materials = crate::look::Materials::new(vpx);
    let picture = |name: &str| crate::look::image_named(vpx, name);
    // a collection's parts, as VPX links them: the first part of each name, decals left out
    let part_named = |name: &str| vpx.gameitems.iter().position(|g| !matches!(g, GameItemEnum::Decal(_)) && g.name() == name);
    let mut grouped = HashSet::new();
    for c in vpx.collections.iter().filter(|c| c.group_elements) {
        let (mut group_material, mut group_picture) = (None, None);
        let mut prims = Vec::new();
        for k in c.items.iter().filter_map(|n| part_named(n)) {
            let GameItemEnum::Primitive(p) = &vpx.gameitems[k] else { continue };
            if !p.use_3d_mesh || p.static_rendering {
                continue;
            }
            let (m, t) = (materials.find(&p.material), picture(&p.image));
            if group_material.is_none() {
                group_material = Some(m);
            }
            if group_picture.is_none() {
                group_picture = t;
            }
            if Some(m) == group_material && t == group_picture {
                prims.push(k);
            }
        }
        if prims.len() > 1 {
            grouped.extend(prims);
        }
    }
    grouped
}

/// A primitive's one piece, as the export names it: the primitive's name.
/// None without a mesh (or a generated one of fewer than 3 sides).
pub fn shape(p: &Primitive, grouped: bool) -> Option<Mesh> {
    let local = if p.use_3d_mesh {
        let mesh = p.read_mesh().ok().flatten()?;
        Mesh {
            positions: mesh.vertices.iter().map(|w| [w.vertex.x, w.vertex.y, w.vertex.z]).collect(),
            normals: mesh.vertices.iter().map(|w| [w.vertex.nx, w.vertex.ny, w.vertex.nz]).collect(),
            uvs: mesh.vertices.iter().map(|w| [w.vertex.tu, w.vertex.tv]).collect(),
            indices: mesh.indices.iter().flat_map(|f| [f.i0 as u32, f.i1 as u32, f.i2 as u32]).collect(),
        }
    } else {
        if p.sides < 3 {
            return None;
        }
        builtin(p.sides, p.draw_textures_inside)
    };
    if local.positions.is_empty() || local.indices.is_empty() {
        return None;
    }
    Some(place(p, local, grouped))
}

/// A primitive's own mesh through its matrix: points by the matrix, normals
/// by its inverse transpose (or, grouped, by the matrix itself).
pub fn place(p: &Primitive, local: Mesh, grouped: bool) -> Mesh {
    place_with(&matrix(p), local, grouped)
}

/// A mesh through a primitive's matrix `m`, as [`place`] (an animation's
/// frames are placed so, each time one is shown).
pub fn place_with(m: &Matrix3D, local: Mesh, grouped: bool) -> Mesh {
    let m = *m;
    // the normals: by the inverse transpose (each row's cofactors: the other two rows crossed,
    // over the determinant), or, grouped, by the matrix itself
    let row = |i: usize| Vertex3Ds::new(m.m[i][0], m.m[i][1], m.m[i][2]);
    let (a0, a1, a2) = (row(0), row(1), row(2));
    let cof = [cross_product(&a1, &a2), cross_product(&a2, &a0), cross_product(&a0, &a1)];
    let det = a0.x * cof[0].x + a0.y * cof[0].y + a0.z * cof[0].z;
    let normal = |n: [f32; 3]| -> [f32; 3] {
        let v = if grouped || det == 0.0 {
            m.multiply_vector_no_translate(&Vertex3Ds::new(n[0], n[1], n[2]))
        } else {
            let inv = 1.0 / det;
            Vertex3Ds::new(
                (n[0] * cof[0].x + n[1] * cof[1].x + n[2] * cof[2].x) * inv,
                (n[0] * cof[0].y + n[1] * cof[1].y + n[2] * cof[2].y) * inv,
                (n[0] * cof[0].z + n[1] * cof[1].z + n[2] * cof[2].z) * inv,
            )
        };
        let l = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        if l > 0.0 { [v.x / l, v.y / l, v.z / l] } else { [0.0, 0.0, 0.0] }
    };
    Mesh {
        positions: local
            .positions
            .iter()
            .map(|&[x, y, z]| {
                let v = m.transform(&Vertex3Ds::new(x, y, z));
                [v.x, v.y, v.z]
            })
            .collect(),
        normals: local.normals.iter().map(|&n| normal(n)).collect(),
        uvs: local.uvs,
        indices: local.indices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpin::vpx::gameitem::vertex3d::Vertex3D;

    #[test]
    fn a_generated_prism_has_vpxs_points_and_triangles() {
        let m = builtin(4, false);
        assert_eq!((m.positions.len(), m.indices.len()), (18, 48));
        assert_eq!(m.positions[0], [0.0, 0.0, 0.5]);
        assert_eq!(builtin(4, true).indices.len(), 96);
    }

    #[test]
    fn a_flattened_primitives_normals_stay_square_to_its_surfaces() {
        // a slope at 45 degrees, flattened ten times: its normal tips up, as VPX's shader turns it
        let p = Primitive { size: Vertex3D { x: 10.0, y: 10.0, z: 1.0 }, position: Vertex3D { x: 0.0, y: 0.0, z: 0.0 }, rot_and_tra: [0.0; 9], ..Default::default() };
        let r = std::f32::consts::FRAC_1_SQRT_2;
        let local = Mesh { positions: vec![[0.0; 3]; 3], normals: vec![[r, 0.0, r]; 3], uvs: vec![[0.0; 2]; 3], indices: vec![0, 1, 2] };
        let n = place(&p, local.clone(), false).normals[0];
        assert!((n[0] - 0.0995).abs() < 1e-3 && n[1].abs() < 1e-6 && (n[2] - 0.995).abs() < 1e-3, "{n:?}");
        // in a render group VPX turns it by the matrix itself: it tips over instead
        let g = place(&p, local, true).normals[0];
        assert!((g[0] - 0.995).abs() < 1e-3 && (g[2] - 0.0995).abs() < 1e-3, "{g:?}");
    }
}
