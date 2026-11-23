// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/rubber.cpp (Rubber::GenerateMesh for drawing, GetSplineVertex's middle points,
// GetCentralCurve, UpdateRubber) and src/math/MeshUtils.h (ComputeNormals).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A rubber's shape as VPX draws it: a tube of 13 sides along the curve
//! through its drag points (VPX's detail level 10, its default), its
//! normals averaged from its faces, turned about its middle as the rubber is
//! turned and lifted to its height.

use physics_compat::math::def::ang_to_rad;
use physics_compat::math::matrix3d::get_rotated_axis;
use physics_compat::math::{Matrix3D, Vertex2D, Vertex3Ds, cross_product};
use physics_compat::setup::curves::{DragPoint, get_rg_vertex};
use vpin::vpx::gameitem::rubber::Rubber;

use super::Mesh;

/// VPX's detail level (`[Player]` `AlphaRampAccuracy`, 1 to 10, 10 by default): how finely
/// rubbers and ramps are drawn, and which ball mesh is used. Set once from the settings before the
/// table's shapes are built.
static DETAIL_LEVEL: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(10);

/// Sets the detail level for the shapes built from now on.
pub fn set_detail_level(level: i32) {
    DETAIL_LEVEL.store(level.clamp(1, 10), std::sync::atomic::Ordering::Relaxed);
}

/// The detail level the shapes are built at.
pub fn detail_level() -> f32 {
    DETAIL_LEVEL.load(std::sync::atomic::Ordering::Relaxed) as f32
}

pub fn shape(r: &Rubber) -> Mesh {
    shape_and_middle(r).0
}

/// [`shape`], with the middle it's turned about (before it's lifted).
pub fn shape_and_middle(r: &Rubber) -> (Mesh, [f32; 3]) {
    // GenerateMesh: the tube's sides by the detail level (static rubbers always at its finest)
    let level = if r.static_rendering { 10 } else { detail_level() as i32 };
    let segments = if level < 5 {
        6
    } else if level < 8 {
        8
    } else {
        (level as f32 * 1.300_000_1_f32) as usize
    };
    // GetCentralCurve: the curve's accuracy by the detail level
    let curve_level = if r.static_rendering { 10.0 } else { detail_level() };
    let accuracy = 4.0 * 10.0_f32.powf((10.0 - curve_level) * (1.0_f64 / 1.5) as f32);
    let points: Vec<DragPoint> =
        r.drag_points.iter().map(|p| DragPoint { v: Vertex3Ds::new(p.x, p.y, p.z), smooth: p.smooth, slingshot: p.is_slingshot.unwrap_or(false) }).collect();
    let curve = get_rg_vertex::<Vertex2D>(&points, true, accuracy);
    if curve.is_empty() {
        return (Mesh::default(), [0.0; 3]);
    }
    let mut middle: Vec<Vertex2D> = curve.iter().map(|v| v.v).collect();
    middle.push(middle[0]);
    let rings = curve.len();
    let height = r.hit_height.unwrap_or(r.height);

    let mut positions = Vec::with_capacity(rings * segments);
    let mut uvs = Vec::with_capacity(rings * segments);
    let mut prev_b = Vertex3Ds::default();
    let (inv_nr, inv_ns) = (1.0 / rings as f32, 1.0 / segments as f32);
    for i in 0..rings {
        let i2 = if i == rings - 1 { 0 } else { i + 1 };
        let (mi, mi2) = (middle[i], middle[i2]);
        let tangent = Vertex3Ds::new(mi2.x - mi.x, mi2.y - mi.y, 0.0);
        let (mut binorm, mut normal);
        if i == 0 {
            let up = Vertex3Ds::new(mi2.x + mi.x, mi2.y + mi.y, height * 2.0);
            normal = Vertex3Ds::new(tangent.y * up.z, -tangent.x * up.z, tangent.x * up.y - tangent.y * up.x);
            binorm = Vertex3Ds::new(tangent.y * normal.z, -tangent.x * normal.z, tangent.x * normal.y - tangent.y * normal.x);
        } else {
            normal = cross_product(&prev_b, &tangent);
            binorm = cross_product(&tangent, &normal);
        }
        binorm.normalize();
        normal.normalize();
        prev_b = binorm;
        let u = i as f32 * inv_nr;
        for j in 0..segments {
            let v = (j as f32 + u) * inv_ns;
            let tmp = get_rotated_axis(j as f32 * (360.0 * inv_ns), &tangent, &normal) * (r.thickness as f32 * 0.5);
            positions.push([mi.x + tmp.x, mi.y + tmp.y, height + tmp.z]);
            uvs.push([u, v]);
        }
    }

    let mut indices = Vec::with_capacity(6 * rings * segments);
    for i in 0..rings {
        for j in 0..segments {
            let q0 = i * segments + j;
            let q1 = if j != segments - 1 { i * segments + j + 1 } else { i * segments };
            let (q2, q3) = if i != rings - 1 {
                ((i + 1) * segments + j, if j != segments - 1 { (i + 1) * segments + j + 1 } else { (i + 1) * segments })
            } else {
                (j, if j != segments - 1 { j + 1 } else { 0 })
            };
            indices.extend([q0, q1, q2, q3, q2, q1].map(|q| q as u32));
        }
    }
    let normals = super::compute_normals(&positions, &indices);

    // UpdateRubber: turned about the middle of its bounds, lifted to its height
    let (mut min, mut max) = ([f32::MAX; 3], [-f32::MAX; 3]);
    for p in &positions {
        for k in 0..3 {
            if max[k] < p[k] {
                max[k] = p[k];
            }
            if min[k] > p[k] {
                min[k] = p[k];
            }
        }
    }
    let mid = [(max[0] + min[0]) * 0.5, (max[1] + min[1]) * 0.5, (max[2] + min[2]) * 0.5];
    let full = (Matrix3D::rotate_z(ang_to_rad(r.rot_z)) * Matrix3D::rotate_y(ang_to_rad(r.rot_y))) * Matrix3D::rotate_x(ang_to_rad(r.rot_x));
    let place = (Matrix3D::translate(-mid[0], -mid[1], -mid[2]) * full) * Matrix3D::translate(mid[0], mid[1], r.height);
    let mesh = Mesh {
        positions: positions
            .iter()
            .map(|&[x, y, z]| {
                let v = place.transform(&Vertex3Ds::new(x, y, z));
                [v.x, v.y, v.z]
            })
            .collect(),
        normals: normals
            .iter()
            .map(|&[x, y, z]| {
                let v = full.multiply_vector_no_translate(&Vertex3Ds::new(x, y, z));
                [v.x, v.y, v.z]
            })
            .collect(),
        uvs,
        indices,
    };
    (mesh, mid)
}
