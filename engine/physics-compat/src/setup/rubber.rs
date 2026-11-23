// Ported from Visual Pinball X: src/parts/rubber.cpp (license:GPLv3+): PhysicSetup, GenerateMesh and
// UpdateRubber for the hit shape, GetCentralCurve, SetupHitObject and AddHitEdge.
// Commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A rubber's colliders: VPX builds a coarse tube along the rubber's curve
//! (six sides, flattened top and bottom), turns it as the rubber is turned,
//! and adds its triangles, its edges once each, and its corners.

use std::collections::HashSet;

use super::curves::{DragPoint, get_rg_vertex};
use super::{Colliders, PhysicsMaterial};
use crate::collide::{HitObjectBase, HitPoint, ObjType};
use crate::collideex::{HitLine3D, HitTriangle};
use crate::engine::{Collider, Shape};
use crate::math::def::ang_to_rad;
use crate::math::matrix3d::get_rotated_axis;
use crate::math::{Matrix3D, Vertex2D, Vertex3Ds, cross_product};
use crate::physconst::HIT_SHAPE_DETAIL_LEVEL;

/// A rubber's settings (VPX's `RubberData`).
#[derive(Clone, Debug, PartialEq)]
pub struct RubberData {
    pub points: Vec<DragPoint>,
    /// The height the hit shape is built at (`m_hitHeight`; VPX uses the
    /// visible height when a table has none).
    pub hit_height: f32,
    pub thickness: i32,
    /// In degrees.
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub collidable: bool,
    pub hit_event: bool,
    /// The material, resolved as `SetupHitObject` does.
    pub material: PhysicsMaterial,
}

/// The sides of the hit shape's tube (VPX's `GenerateMesh(6, true)`).
const HIT_SEGMENTS: usize = 6;

/// VPX's `GenerateMesh` for the hit shape, then `UpdateRubber`: the tube's
/// corners on the table, and its triangles as index triples.
pub fn rubber_hit_mesh(r: &RubberData) -> (Vec<Vertex3Ds>, Vec<u32>) {
    // GetSplineVertex: only the centre line matters here
    let accuracy = 4.0 * 10.0_f32.powf((10.0 - HIT_SHAPE_DETAIL_LEVEL) * (1.0_f64 / 1.5) as f32);
    let vvertex = get_rg_vertex::<Vertex2D>(&r.points, true, accuracy);
    let cvertex = vvertex.len();
    if cvertex == 0 {
        return (Vec::new(), Vec::new());
    }
    let mut middle_points: Vec<Vertex2D> = vvertex.iter().map(|v| v.v).collect();
    middle_points.push(middle_points[0]);

    let num_rings = cvertex;
    let num_segments = HIT_SEGMENTS;
    let mut vertices = vec![Vertex3Ds::default(); num_rings * num_segments];
    let height = r.hit_height;

    let mut prev_b = Vertex3Ds::default();
    let inv_ns = 1.0 / num_segments as f32;
    let mut index = 0;
    for i in 0..num_rings {
        let i2 = if i == num_rings - 1 { 0 } else { i + 1 };
        let (mi, mi2) = (middle_points[i], middle_points[i2]);

        let tangent = Vertex3Ds::new(mi2.x - mi.x, mi2.y - mi.y, 0.0);

        let (mut binorm, mut normal);
        if i == 0 {
            let up = Vertex3Ds::new(mi2.x + mi.x, mi2.y + mi.y, height * 2.0);
            // = CrossProduct(tangent, up)
            normal = Vertex3Ds::new(tangent.y * up.z, -tangent.x * up.z, tangent.x * up.y - tangent.y * up.x);
            // = CrossProduct(tangent, normal)
            binorm = Vertex3Ds::new(
                tangent.y * normal.z,
                -tangent.x * normal.z,
                tangent.x * normal.y - tangent.y * normal.x,
            );
        } else {
            normal = cross_product(&prev_b, &tangent);
            binorm = cross_product(&tangent, &normal);
        }
        binorm.normalize();
        normal.normalize();
        prev_b = binorm;
        for j in 0..num_segments {
            let mut tmp = get_rotated_axis(j as f32 * (360.0 * inv_ns), &tangent, &normal) * (r.thickness as f32 * 0.5);
            vertices[index].x = mi.x + tmp.x;
            vertices[index].y = mi.y + tmp.y;
            if j == 0 || j == 3 {
                // for a hit shape create a more rectangle mesh and not a smooth one
                tmp.z *= 0.6;
            }
            vertices[index].z = height + tmp.z;
            index += 1;
        }
    }

    // calculate faces
    let mut indices = vec![0u32; 6 * num_rings * num_segments];
    for i in 0..num_rings {
        for j in 0..num_segments {
            let q0 = i * num_segments + j;
            let q1 = if j != num_segments - 1 { i * num_segments + j + 1 } else { i * num_segments };
            let (q2, q3) = if i != num_rings - 1 {
                ((i + 1) * num_segments + j, if j != num_segments - 1 { (i + 1) * num_segments + j + 1 } else { (i + 1) * num_segments })
            } else {
                (j, if j != num_segments - 1 { j + 1 } else { 0 })
            };
            let k = (i * num_segments + j) * 6;
            indices[k..k + 6].copy_from_slice(&[q0 as u32, q1 as u32, q2 as u32, q3 as u32, q2 as u32, q1 as u32]);
        }
    }

    // the mesh's centre: the middle of its bounding box
    let (mut maxx, mut minx, mut maxy, mut miny, mut maxz, mut minz) = (-f32::MAX, f32::MAX, -f32::MAX, f32::MAX, -f32::MAX, f32::MAX);
    for v in &vertices {
        if maxx < v.x {
            maxx = v.x;
        }
        if minx > v.x {
            minx = v.x;
        }
        if maxy < v.y {
            maxy = v.y;
        }
        if miny > v.y {
            miny = v.y;
        }
        if maxz < v.z {
            maxz = v.z;
        }
        if minz > v.z {
            minz = v.z;
        }
    }
    let middle = Vertex3Ds::new((maxx + minx) * 0.5, (maxy + miny) * 0.5, (maxz + minz) * 0.5);

    // UpdateRubber: turn the mesh around its centre, at the hit height
    let full_matrix = (Matrix3D::rotate_z(ang_to_rad(r.rot_z)) * Matrix3D::rotate_y(ang_to_rad(r.rot_y)))
        * Matrix3D::rotate_x(ang_to_rad(r.rot_x));
    let vert_matrix = (Matrix3D::translate(-middle.x, -middle.y, -middle.z) * full_matrix)
        * Matrix3D::translate(middle.x, middle.y, height);
    for v in &mut vertices {
        *v = vert_matrix.transform(v);
    }

    (vertices, indices)
}

/// VPX's `Rubber::SetupHitObject`: rubbers count as primitives for hit
/// events, with a fixed threshold.
fn setup_hit_object(r: &RubberData, out: &mut Colliders, mut base: HitObjectBase, shape: Shape) {
    base.elasticity = r.material.elasticity;
    base.elasticity_falloff = r.material.elasticity_falloff;
    base.friction = r.material.friction;
    base.scatter = ang_to_rad(r.material.scatter_deg);

    base.enabled = r.collidable;
    // the rubber is of type ePrimitive for triggering the event in HitTriangle::Collide()
    base.obj_type = ObjType::Primitive;
    // hard coded threshold for now
    base.threshold = 2.0;
    base.obj = Some(out.part);
    base.fe = r.hit_event;
    base.quiet = (!r.hit_event).then_some((out.part, base.threshold));

    out.out.push(Collider { base, shape, owner: out.owner() });
}

/// VPX's `Rubber::PhysicSetup` for play.
pub fn rubber_colliders(r: &RubberData, out: &mut Colliders) {
    let (vertices, ring_indices) = rubber_hit_mesh(r);
    let mut added_edges: HashSet<(u32, u32)> = HashSet::new();

    let mut add_hit_edge = |out: &mut Colliders, i: u32, j: u32| {
        // create pair uniquely identifying the edge (i,j)
        if added_edges.insert((i.min(j), i.max(j))) {
            let mut b = HitObjectBase::default();
            let line = HitLine3D::new(vertices[i as usize], vertices[j as usize], &mut b);
            setup_hit_object(r, out, b, Shape::Line3D(line));
        }
    };

    // add collision triangles and edges
    for t in ring_indices.as_chunks::<3>().0 {
        // HitTriangle wants CCW vertices, but for rendering we have them in CW order
        let rgv = [vertices[t[0] as usize], vertices[t[2] as usize], vertices[t[1] as usize]];
        let mut b = HitObjectBase::default();
        let tri = HitTriangle::new(rgv, &mut b);
        if !tri.is_degenerate() {
            setup_hit_object(r, out, b, Shape::Triangle(tri));
        }

        add_hit_edge(out, t[0], t[2]);
        add_hit_edge(out, t[2], t[1]);
        add_hit_edge(out, t[1], t[0]);
    }

    // add collision vertices
    for v in &vertices {
        setup_hit_object(r, out, HitObjectBase::default(), Shape::Point(HitPoint::new(*v)));
    }
}
