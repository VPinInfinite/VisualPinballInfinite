// Ported from Visual Pinball X: src/parts/primitive.cpp (license:GPLv3+): PhysicSetup, SetupHitObject,
// AddHitEdge, RecalculateMatrices, TransformVertices and CalculateBuiltinOriginal.
// Commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A primitive's colliders: its mesh placed on the table, every triangle,
//! each edge once, and every corner. A table can ask for a coarser collision
//! mesh than the visible one (the collision reduction factor); VPX then
//! simplifies the mesh with Stan Melax's progressive mesh first.

use std::collections::HashSet;

use super::progmesh::{Float3, TriData, permute_vertices, progressive_mesh, remap_indices};
use super::{Colliders, PhysicsMaterial};
use crate::collide::{HitObjectBase, HitPoint, ObjType};
use crate::collideex::{HitLine3D, HitTriangle};
use crate::engine::{Collider, Shape};
use crate::math::def::{ang_to_rad, max, min};
use crate::math::{Matrix3D, Vertex3Ds};

/// A mesh as VPX keeps it: corner positions, and triangles as index
/// triples, clockwise as rendered.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex3Ds>,
    pub indices: Vec<u32>,
}

/// A primitive's physics settings (VPX's `PrimitiveData`).
#[derive(Clone, Debug, PartialEq)]
pub struct PrimitiveData {
    /// The mesh before placement: the table's own, or a built-in shape
    /// ([`builtin_mesh`]).
    pub mesh: Mesh,
    pub position: Vertex3Ds,
    pub size: Vertex3Ds,
    /// VPX's `m_aRotAndTra`: rotations in degrees (X, Y, Z), a translation,
    /// then three more rotations (X, Y, Z).
    pub rot_and_tra: [f32; 9],
    pub collidable: bool,
    pub hit_event: bool,
    pub threshold: f32,
    /// 0 keeps every triangle; up to 1 simplifies the mesh.
    pub collision_reduction_factor: f32,
    /// The material, resolved as `SetupHitObject` does (for the playfield
    /// mesh, the table's playfield physics).
    pub material: PhysicsMaterial,
}

/// VPX's `CalculateBuiltinOriginal`: a prism with `sides` sides, one unit
/// across, from z = -0.5 to 0.5; with `draw_textures_inside`, every triangle
/// twice, facing both ways.
pub fn builtin_mesh(sides: i32, draw_textures_inside: bool) -> Mesh {
    let n = sides.max(0) as usize;
    let outer_radius = -0.5 / (std::f64::consts::PI as f32 / sides as f32).cos();
    let add_angle = (2.0 * std::f64::consts::PI) as f32 / sides as f32;
    let offs_angle = std::f64::consts::PI as f32 / sides as f32;

    let mut v = vec![Vertex3Ds::default(); 4 * n + 2];
    v[0] = Vertex3Ds::new(0.0, 0.0, 0.5); // middle point top
    v[n + 1] = Vertex3Ds::new(0.0, 0.0, -0.5); // middle point bottom
    for i in 0..n {
        // calculate top
        let current_angle = add_angle * i as f32 + offs_angle;
        let top = Vertex3Ds::new(current_angle.sin() * outer_radius, current_angle.cos() * outer_radius, 0.5);
        v[i + 1] = top;
        // calculate bottom
        let bottom = Vertex3Ds::new(top.x, top.y, -0.5);
        v[i + 1 + n + 1] = bottom;
        // calculate sides
        v[n * 2 + 2 + i] = top;
        v[n * 3 + 2 + i] = bottom;
    }

    let mut ix = Vec::new();
    if draw_textures_inside {
        // draw everything twice
        ix.resize(n * 24, 0u32);
        for i in 0..n {
            let tmp = if i == n - 1 { 1 } else { i + 2 }; // wrapping around
            // top
            ix[i * 6..i * 6 + 6].copy_from_slice(&[0, (i + 1) as u32, tmp as u32, 0, tmp as u32, (i + 1) as u32]);

            let tmp2 = tmp + 1;
            // bottom
            let b = 6 * (i + n);
            ix[b..b + 6].copy_from_slice(&[
                (n + 1) as u32,
                (n + tmp2) as u32,
                (n + 2 + i) as u32,
                (n + 1) as u32,
                (n + 2 + i) as u32,
                (n + tmp2) as u32,
            ]);

            // sides
            let s = 12 * (i + n);
            let (a, b, c, d) = ((n * 2 + tmp2) as u32, (n * 2 + 2 + i) as u32, (n * 3 + 2 + i) as u32, (n * 3 + tmp2) as u32);
            ix[s..s + 12].copy_from_slice(&[a, b, c, a, c, d, a, c, b, a, d, c]);
        }
    } else {
        // only out-facing polygons
        ix.resize(n * 12, 0u32);
        for i in 0..n {
            let tmp = if i == n - 1 { 1 } else { i + 2 }; // wrapping around
            // top
            ix[i * 3] = 0;
            ix[i * 3 + 2] = (i + 1) as u32;
            ix[i * 3 + 1] = tmp as u32;

            let tmp2 = tmp + 1;
            // bottom
            let b = 3 * (i + n);
            ix[b..b + 3].copy_from_slice(&[(n + 1) as u32, (n + 2 + i) as u32, (n + tmp2) as u32]);

            // sides
            let s = 6 * (i + n);
            let (a, b, c, d) = ((n * 2 + tmp2) as u32, (n * 2 + 2 + i) as u32, (n * 3 + 2 + i) as u32, (n * 3 + tmp2) as u32);
            ix[s..s + 6].copy_from_slice(&[a, c, b, a, d, c]);
        }
    }
    Mesh { vertices: v, indices: ix }
}

/// VPX's `RecalculateMatrices`: scale, then the rotations and translation,
/// then the position.
pub fn full_matrix(p: &PrimitiveData) -> Matrix3D {
    let r = &p.rot_and_tra;
    // translation + rotation matrix
    let mut rt = ((Matrix3D::translate(r[3], r[4], r[5]) * Matrix3D::rotate_z(ang_to_rad(r[2])))
        * Matrix3D::rotate_y(ang_to_rad(r[1])))
        * Matrix3D::rotate_x(ang_to_rad(r[0]));
    rt = ((rt * Matrix3D::rotate_z(ang_to_rad(r[8]))) * Matrix3D::rotate_y(ang_to_rad(r[7])))
        * Matrix3D::rotate_x(ang_to_rad(r[6]));
    (Matrix3D::scale(p.size.x, p.size.y, p.size.z) * rt)
        * Matrix3D::translate(p.position.x, p.position.y, p.position.z)
}

/// VPX's `Primitive::SetupHitObject`.
fn setup_hit_object(p: &PrimitiveData, out: &mut Colliders, mut base: HitObjectBase, shape: Shape) {
    base.elasticity = p.material.elasticity;
    base.elasticity_falloff = p.material.elasticity_falloff;
    base.friction = p.material.friction;
    base.scatter = ang_to_rad(p.material.scatter_deg);

    base.enabled = p.collidable;
    base.threshold = p.threshold;
    base.obj_type = ObjType::Primitive;
    base.obj = Some(out.part);
    base.fe = p.hit_event;
    base.quiet = (!p.hit_event).then_some((out.part, p.threshold));

    out.out.push(Collider { base, shape, owner: out.owner() });
}

/// The triangles' colliders and each edge once: VPX's loop in `PhysicSetup`,
/// shared by the full and the reduced mesh.
fn add_triangles(p: &PrimitiveData, out: &mut Colliders, vertices: &[Vertex3Ds], triangles: &[TriData]) {
    let mut added_edges: HashSet<(u32, u32)> = HashSet::new();
    let mut add_hit_edge = |out: &mut Colliders, i: u32, j: u32, vi: Vertex3Ds, vj: Vertex3Ds| {
        // create pair uniquely identifying the edge (i,j)
        if added_edges.insert((i.min(j), i.max(j))) {
            let mut b = HitObjectBase::default();
            let line = HitLine3D::new(vi, vj, &mut b);
            setup_hit_object(p, out, b, Shape::Line3D(line));
        }
    };

    for &[i0, i1, i2] in triangles {
        // NB: HitTriangle wants CCW vertices, but for rendering we have them in CW order
        let rgv = [vertices[i0 as usize], vertices[i2 as usize], vertices[i1 as usize]];
        let mut b = HitObjectBase::default();
        let tri = HitTriangle::new(rgv, &mut b);
        if !tri.is_degenerate() {
            setup_hit_object(p, out, b, Shape::Triangle(tri));
        }

        add_hit_edge(out, i0, i1, rgv[0], rgv[2]);
        add_hit_edge(out, i1, i2, rgv[2], rgv[1]);
        add_hit_edge(out, i2, i0, rgv[1], rgv[0]);
    }
}

/// VPX's `Primitive::PhysicSetup` for play, for a primitive that collides
/// (VPX skips toys). Triangles naming a corner the mesh doesn't have are left
/// out (VPX would read past its data).
pub fn primitive_colliders(p: &PrimitiveData, out: &mut Colliders) {
    // TransformVertices
    let m = full_matrix(p);
    let vertices: Vec<Vertex3Ds> = p.mesh.vertices.iter().map(|v| m.transform(v)).collect();
    let n = vertices.len() as u32;
    let triangles: Vec<TriData> =
        p.mesh.indices.as_chunks::<3>().0.iter().copied().filter(|t| t.iter().all(|&i| i < n)).collect();

    let saturated = max(min(1.0 - p.collision_reduction_factor, 1.0), 0.0);
    let exponent = saturated * 0.25 + 0.75;
    let reduced_vertices = ((vertices.len() as f64).powf(exponent as f64) as u32).max(420); // 420 = magic (VPX)

    if (reduced_vertices as usize) < vertices.len() {
        let mut prog_vertices: Vec<Float3> = vertices.iter().map(|v| Float3::new(v.x, v.y, v.z)).collect();
        let mut prog_indices: Vec<TriData> =
            triangles.iter().copied().filter(|t| t[0] != t[1] && t[1] != t[2] && t[2] != t[0]).collect();
        let (map, perm) = progressive_mesh(&prog_vertices, &prog_indices);
        if perm.is_empty() {
            return; // no triangle left to simplify: VPX would fail here
        }
        permute_vertices(&perm, &mut prog_vertices, &mut prog_indices);
        let prog_new_indices = remap_indices(reduced_vertices, &prog_indices, &map);

        let reduced: Vec<Vertex3Ds> = prog_vertices.iter().map(|v| Vertex3Ds::new(v.x, v.y, v.z)).collect();
        add_triangles(p, out, &reduced, &prog_new_indices);

        // add collision vertices: all of them, also those collapsed away
        for v in &reduced {
            setup_hit_object(p, out, HitObjectBase::default(), Shape::Point(HitPoint::new(*v)));
        }
    } else {
        add_triangles(p, out, &vertices, &triangles);

        // add collision vertices
        for v in &vertices {
            setup_hit_object(p, out, HitObjectBase::default(), Shape::Point(HitPoint::new(*v)));
        }
    }
}
