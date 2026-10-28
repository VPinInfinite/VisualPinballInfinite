// Ported from Visual Pinball X: src/parts/hittarget.cpp (license:GPLv3+): PhysicSetup, SetupHitObject,
// AddHitEdge, SetMeshType, TransformVertices and the drop target's hit plane.
// Commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A target's colliders: its built-in mesh, scaled, turned and placed. A
//! drop target (unless in legacy mode) also gets a thin plate just in front
//! of its face that alone fires hit events, so a ball hitting it from behind
//! doesn't count.

use std::collections::HashSet;

use super::{Colliders, PhysicsMaterial};
use crate::collide::{HitObjectBase, HitPoint, ObjType};
use crate::collideex::{HitLine3D, HitTriangle};
use crate::engine::{Collider, Shape};
use crate::math::def::ang_to_rad;
use crate::math::{Matrix3D, Vertex3Ds};
use crate::meshes::target_meshes as m;

/// VPX's `TargetType`, with its values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetType {
    DropTargetBeveled = 1,
    DropTargetSimple = 2,
    HitTargetRound = 3,
    HitTargetRectangle = 4,
    HitFatTargetRectangle = 5,
    HitFatTargetSquare = 6,
    DropTargetFlatSimple = 7,
    HitFatTargetSlim = 8,
    HitTargetSlim = 9,
}

impl TargetType {
    pub fn is_drop_target(self) -> bool {
        matches!(self, TargetType::DropTargetBeveled | TargetType::DropTargetFlatSimple | TargetType::DropTargetSimple)
    }

    /// VPX's `SetMeshType`.
    fn mesh(self) -> (&'static [[f32; 3]], &'static [u16]) {
        match self {
            TargetType::DropTargetBeveled => (&m::DROP_TARGET_BEVELED_VERTICES, &m::DROP_TARGET_BEVELED_INDICES),
            TargetType::DropTargetSimple => (&m::DROP_TARGET_SIMPLE_VERTICES, &m::DROP_TARGET_SIMPLE_INDICES),
            TargetType::DropTargetFlatSimple => {
                (&m::DROP_TARGET_FLAT_SIMPLE_VERTICES, &m::DROP_TARGET_FLAT_SIMPLE_INDICES)
            }
            TargetType::HitTargetRound => (&m::HIT_TARGET_ROUND_VERTICES, &m::HIT_TARGET_ROUND_INDICES),
            TargetType::HitTargetRectangle => (&m::HIT_TARGET_RECTANGLE_VERTICES, &m::HIT_TARGET_RECTANGLE_INDICES),
            TargetType::HitFatTargetRectangle => {
                (&m::HIT_FAT_TARGET_RECTANGLE_VERTICES, &m::HIT_FAT_TARGET_RECTANGLE_INDICES)
            }
            TargetType::HitFatTargetSquare => (&m::HIT_FAT_TARGET_SQUARE_VERTICES, &m::HIT_FAT_TARGET_SQUARE_INDICES),
            TargetType::HitTargetSlim => (&m::HIT_TARGET_SLIM_VERTICES, &m::HIT_TARGET_SLIM_INDICES),
            TargetType::HitFatTargetSlim => (&m::HIT_FAT_TARGET_SLIM_VERTICES, &m::HIT_FAT_TARGET_SLIM_INDICES),
        }
    }
}

/// A target's physics settings (VPX's `HitTargetData`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitTargetData {
    pub target_type: TargetType,
    pub position: Vertex3Ds,
    pub size: Vertex3Ds,
    /// In degrees.
    pub rot_z: f32,
    /// VPX 10.2's drop targets: no separate hit plate.
    pub legacy: bool,
    pub collidable: bool,
    pub hit_event: bool,
    pub threshold: f32,
    /// The material, resolved as `SetupHitObject` does.
    pub material: PhysicsMaterial,
}

/// The drop target's hit plate (`dropTargetHitPlaneVertices`), before scaling.
#[rustfmt::skip]
const DROP_TARGET_HIT_PLANE_VERTICES: [[f32; 3]; 16] = [
    [-0.300000, 0.001737, -0.160074],
    [-0.300000, 0.001738, 0.439926],
    [0.300000, 0.001738, 0.439926],
    [0.300000, 0.001737, -0.160074],
    [-0.500000, 0.001738, 0.439926],
    [-0.500000, 0.001738, 1.789926],
    [0.500000, 0.001738, 1.789926],
    [0.500000, 0.001738, 0.439926],
    [-0.535355, 0.001738, 0.454570],
    [-0.535355, 0.001738, 1.775281],
    [-0.550000, 0.001738, 0.489926],
    [-0.550000, 0.001738, 1.739926],
    [0.535355, 0.001738, 0.454570],
    [0.535355, 0.001738, 1.775281],
    [0.550000, 0.001738, 0.489926],
    [0.550000, 0.001738, 1.739926],
];

/// `dropTargetHitPlaneIndices`.
#[rustfmt::skip]
const DROP_TARGET_HIT_PLANE_INDICES: [u16; 42] = [
    0, 1, 2, 2, 3, 0, 1, 4, 5, 6, 7, 2, 5, 6, 1,
    2, 1, 6, 4, 8, 9, 9, 5, 4, 8, 10, 11, 11, 9, 8,
    6, 12, 7, 12, 6, 13, 12, 13, 14, 13, 15, 14,
];

/// VPX's `TransformVertices`: scaled, turned around z, then moved.
fn place(t: &HitTargetData, full_matrix: &Matrix3D, v: Vertex3Ds) -> Vertex3Ds {
    let mut vert = v;
    vert.x *= t.size.x;
    vert.y *= t.size.y;
    vert.z *= t.size.z;
    vert = full_matrix.transform(&vert);
    Vertex3Ds::new(vert.x + t.position.x, vert.y + t.position.y, vert.z + t.position.z)
}

/// VPX's `HitTarget::SetupHitObject`: hit events only where `set_hit_object`.
fn setup_hit_object(t: &HitTargetData, out: &mut Colliders, mut base: HitObjectBase, shape: Shape, set_hit_object: bool) {
    base.elasticity = t.material.elasticity;
    base.elasticity_falloff = t.material.elasticity_falloff;
    base.friction = t.material.friction;
    base.scatter = ang_to_rad(t.material.scatter_deg);
    base.threshold = t.threshold;
    base.enabled = t.collidable;
    base.obj_type = ObjType::HitTarget;
    base.obj = Some(out.part);
    base.fe = set_hit_object && t.hit_event;
    base.quiet = (set_hit_object && !t.hit_event).then_some((out.part, t.threshold));

    out.out.push(Collider { base, shape, owner: out.owner() });
}

/// A mesh's triangles, its edges once (the set is shared, as VPX shares it
/// between a drop target and its hit plate), and its corners.
fn add_mesh(
    t: &HitTargetData,
    out: &mut Colliders,
    vertices: &[Vertex3Ds],
    indices: &[u16],
    added_edges: &mut HashSet<(u32, u32)>,
    set_hit_object: bool,
) {
    for tri in indices.as_chunks::<3>().0 {
        let (i0, i1, i2) = (tri[0] as u32, tri[1] as u32, tri[2] as u32);
        // NB: HitTriangle wants CCW vertices, but for rendering we have them in CW order
        let rgv = [vertices[i0 as usize], vertices[i2 as usize], vertices[i1 as usize]];
        let mut b = HitObjectBase::default();
        let ht = HitTriangle::new(rgv, &mut b);
        if !ht.is_degenerate() {
            setup_hit_object(t, out, b, Shape::Triangle(ht), set_hit_object);
        }

        for (i, j, vi, vj) in [(i0, i1, rgv[0], rgv[2]), (i1, i2, rgv[2], rgv[1]), (i2, i0, rgv[1], rgv[0])] {
            // create pair uniquely identifying the edge (i,j)
            if added_edges.insert((i.min(j), i.max(j))) {
                let mut b = HitObjectBase::default();
                let line = HitLine3D::new(vi, vj, &mut b);
                setup_hit_object(t, out, b, Shape::Line3D(line), set_hit_object);
            }
        }
    }

    // add collision vertices
    for v in vertices {
        setup_hit_object(t, out, HitObjectBase::default(), Shape::Point(HitPoint::new(*v)), set_hit_object);
    }
}

/// VPX's `HitTarget::PhysicSetup` for play.
pub fn hit_target_colliders(t: &HitTargetData, out: &mut Colliders) {
    let full_matrix = Matrix3D::rotate_z(ang_to_rad(t.rot_z));
    let (mesh_vertices, indices) = t.target_type.mesh();
    let vertices: Vec<Vertex3Ds> =
        mesh_vertices.iter().map(|v| place(t, &full_matrix, Vertex3Ds::new(v[0], v[1], v[2]))).collect();
    let mut added_edges = HashSet::new();

    if t.target_type.is_drop_target() {
        // add the normal drop target as collidable but without hit event
        add_mesh(t, out, &vertices, indices, &mut added_edges, t.legacy);

        if !t.legacy {
            let hit_shape_offset = match t.target_type {
                TargetType::DropTargetBeveled => 0.25,
                TargetType::DropTargetFlatSimple => 0.13,
                _ => 0.18,
            };
            // now create a special hit shape with hit event enabled to prevent a hit event when hit from behind
            let plane: Vec<Vertex3Ds> = DROP_TARGET_HIT_PLANE_VERTICES
                .iter()
                .map(|v| place(t, &full_matrix, Vertex3Ds::new(v[0], v[1] + hit_shape_offset, v[2])))
                .collect();
            add_mesh(t, out, &plane, &DROP_TARGET_HIT_PLANE_INDICES, &mut added_edges, true);
        }
    } else {
        add_mesh(t, out, &vertices, indices, &mut added_edges, true);
    }
}
