// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/ramp.cpp (Ramp::GenerateRampMesh, GenerateWireMesh, CreateWire, PrepareHabitrail and
// the pieces Render draws) and src/parts/ramp.h (GetCentralCurve).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A ramp's shape as VPX draws it: a flat ramp's floor and the walls it
//! shows, or a wire ramp's wires — each a tube of 13 sides (VPX's detail
//! level 10) along the ramp's sides, the lower ones raised 3 above the
//! ramp's flat surface, the upper ones by half the wires' height apart.

use physics_compat::math::matrix3d::get_rotated_axis;
use physics_compat::math::{Vertex2D, Vertex3Ds, cross_product};
use physics_compat::setup::curves::DragPoint;
use physics_compat::setup::ramp::{RampData, RampType, ramp_vertex_full};
use vpin::vpx::VPX;
use vpin::vpx::gameitem::ramp::{Ramp, RampType as VpinRampType};
use vpin::vpx::gameitem::ramp_image_alignment::RampImageAlignment;

use super::Mesh;
use super::rubber::detail_level as detail_setting;

fn data(r: &Ramp) -> RampData {
    RampData {
        points: r.drag_points.iter().map(|p| DragPoint { v: Vertex3Ds::new(p.x, p.y, p.z), smooth: p.smooth, slingshot: p.is_slingshot.unwrap_or(false) }).collect(),
        height_bottom: r.height_bottom,
        height_top: r.height_top,
        width_bottom: r.width_bottom,
        width_top: r.width_top,
        ramp_type: match r.ramp_type {
            VpinRampType::FourWire => RampType::FourWire,
            VpinRampType::TwoWire => RampType::TwoWire,
            VpinRampType::ThreeWireLeft => RampType::ThreeWireLeft,
            VpinRampType::ThreeWireRight => RampType::ThreeWireRight,
            VpinRampType::OneWire => RampType::OneWire,
            _ => RampType::Flat,
        },
        left_wall_height: r.left_wall_height,
        right_wall_height: r.right_wall_height,
        wire_diameter: r.wire_diameter,
        wire_distance_x: r.wire_distance_x,
        collidable: false,
        hit_event: false,
        threshold: 0.0,
        elasticity: 0.0,
        friction: 0.0,
        scatter: 0.0,
    }
}

/// The detail level VPX draws the ramp at (`GetCentralCurve`): its finest
/// for an opaque material (it's drawn ahead of time), else the player's.
fn detail_level(materials: &crate::look::Materials, r: &Ramp) -> f32 {
    let see_through = materials.find(&r.material).is_some_and(|k| materials.all[k as usize].opacity_active);
    if see_through { detail_setting() } else { 10.0 }
}

/// What a ramp's shape takes from its table: whether its picture is there, the table's corners
/// (left, top, right, bottom) for a picture laid out across the table, and VPX's detail level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RampContext {
    pub picture: bool,
    pub table: [f32; 4],
    pub level: f32,
}

pub fn context(vpx: &VPX, materials: &crate::look::Materials, r: &Ramp) -> RampContext {
    let g = &vpx.gamedata;
    RampContext { picture: crate::look::image_named(vpx, &r.image).is_some(), table: [g.left, g.top, g.right, g.bottom], level: detail_level(materials, r) }
}

/// A ramp's own settings its shape is built from, in a copy of ours (vpin's `Ramp` can't be
/// copied): kept to build the ramp again when a script raises or lowers it.
#[derive(Clone, Debug)]
pub struct RampSource {
    pub data: RampData,
    world_image: bool,
    image_walls: bool,
    left_wall_visible: f32,
    right_wall_visible: f32,
    wire_distance_y: f32,
}

pub fn source(r: &Ramp) -> RampSource {
    RampSource {
        data: data(r),
        world_image: r.image_alignment == RampImageAlignment::World,
        image_walls: r.image_walls,
        left_wall_visible: r.left_wall_height_visible,
        right_wall_visible: r.right_wall_height_visible,
        wire_distance_y: r.wire_distance_y,
    }
}

impl RampSource {
    /// The same ramp at other bottom and top heights and widths.
    pub fn with_shape(&self, [height_bottom, height_top, width_bottom, width_top]: [f32; 4]) -> RampSource {
        let mut s = self.clone();
        s.data.height_bottom = height_bottom;
        s.data.height_top = height_top;
        s.data.width_bottom = width_bottom;
        s.data.width_top = width_top;
        s
    }
}

pub fn shape(vpx: &VPX, materials: &crate::look::Materials, r: &Ramp) -> Mesh {
    shape_from(&context(vpx, materials, r), &source(r))
}

/// The ramp's shape from its settings, what it takes from its table already known: during play,
/// a ramp whose heights a script changed is built again this way.
pub fn shape_from(ctx: &RampContext, s: &RampSource) -> Mesh {
    if s.data.is_habitrail() { wires(s, ctx.level) } else { flat(ctx, s) }
}

/// `GenerateRampMesh`: the floor, then (if a wall shows) the right wall and
/// the left, each two points a centre point; the pieces `Render` draws.
fn flat(ctx: &RampContext, s: &RampSource) -> Mesh {
    let d = &s.data;
    let v = ramp_vertex_full(d, ctx.level, true);
    let n = v.heights.len();
    if n < 2 {
        return Mesh::default();
    }
    let picture = ctx.picture;
    let world = s.world_image;
    let [left, top, right, bottom] = ctx.table;
    let (inv_width, inv_height) = (1.0 / (right - left), 1.0 / (bottom - top));
    let (right, left) = (|i: usize| v.outline[i], |i: usize| v.outline[n * 2 - i - 1]);
    // the floor's triangles (and, offset, each wall's)
    let mut floor = Vec::with_capacity((n - 1) * 6);
    for i in 0..n - 1 {
        let i = i as u32;
        floor.extend([i * 2, i * 2 + 1, i * 2 + 3, i * 2, i * 2 + 3, i * 2 + 2]);
    }
    let mut mesh = Mesh::default();
    let mut piece = |points: Vec<([f32; 3], [f32; 2])>| {
        let positions: Vec<[f32; 3]> = points.iter().map(|p| p.0).collect();
        let normals = super::compute_normals(&positions, &floor);
        mesh.positions.extend(positions);
        mesh.normals.extend(normals);
        mesh.uvs.extend(points.iter().map(|p| p.1));
    };
    let world_uv = |p: Vertex2D| [p.x * inv_width, p.y * inv_height];
    // the floor: right then left at each centre point
    piece(
        (0..n)
            .flat_map(|i| {
                let (a, b) = (right(i), left(i));
                let (ua, ub) = match (picture, world) {
                    (true, true) => (world_uv(a), world_uv(b)),
                    (true, false) => ([1.0, v.ratios[i]], [0.0, v.ratios[i]]),
                    (false, _) => ([0.0; 2], [0.0; 2]),
                };
                [([a.x, a.y, v.heights[i]], ua), ([b.x, b.y, v.heights[i]], ub)]
            })
            .collect(),
    );
    // the walls that show (VPX keeps both in its buffer when either shows, and draws only those)
    let (right_shows, left_shows) = (s.right_wall_visible != 0.0, s.left_wall_visible != 0.0);
    // the walls' picture places: the floor point's, or none (VPX's buffer reused)
    let wall_uv = |p: Vertex2D, i: usize| match (picture && s.image_walls, world) {
        (true, true) => world_uv(p),
        (true, false) => [0.0, v.ratios[i]],
        (false, _) => [0.0; 2],
    };
    if right_shows {
        // the right wall: its foot at the floor's right edge, its top raised
        piece(
            (0..n)
                .flat_map(|i| {
                    let a = right(i);
                    let uv = wall_uv(a, i);
                    [([a.x, a.y, v.heights[i]], uv), ([a.x, a.y, v.heights[i] + s.right_wall_visible], uv)]
                })
                .collect(),
        );
    }
    if left_shows {
        piece(
            (0..n)
                .flat_map(|i| {
                    let a = left(i);
                    let uv = wall_uv(a, i);
                    [([a.x, a.y, v.heights[i]], uv), ([a.x, a.y, v.heights[i] + s.left_wall_visible], uv)]
                })
                .collect(),
        );
    }
    // Render: the floor, and the walls that show
    let count = (n * 2) as u32;
    let pieces = 1 + u32::from(right_shows) + u32::from(left_shows);
    mesh.indices = (0..pieces).flat_map(|k| floor.iter().map(move |i| i + count * k)).collect();
    mesh
}

/// `GenerateWireMesh` and `PrepareHabitrail`: each wire a tube along one of
/// the ramp's sides (a one-wire ramp's along its middle).
fn wires(s: &RampSource, level: f32) -> Mesh {
    let d = &s.data;
    // the tube's sides by the detail level (as for rubbers; an opaque ramp's always at its finest)
    let level = level as i32;
    let segments = if level < 5 {
        6
    } else if level < 8 {
        8
    } else {
        (level as f32 * 1.300_000_1_f32) as usize
    };
    let v = ramp_vertex_full(d, level as f32, false);
    let rings = v.heights.len();
    if rings < 2 {
        return Mesh::default();
    }
    let wire = |points: &[Vertex2D]| create_wire(rings, segments, points, &v.heights, d.wire_diameter);
    let right: Vec<Vertex2D> = v.outline[..rings].to_vec();
    let left: Vec<Vertex2D> = (0..rings).map(|i| v.outline[rings * 2 - i - 1]).collect();
    let mut faces = Vec::with_capacity(6 * (rings - 1) * segments);
    for i in 0..rings - 1 {
        for j in 0..segments {
            let q0 = i * segments + j;
            let q1 = i * segments + if j != segments - 1 { j + 1 } else { 0 };
            let q2 = (i + 1) * segments + j;
            let q3 = (i + 1) * segments + if j != segments - 1 { j + 1 } else { 0 };
            faces.extend([q0, q1, q2, q3, q2, q1].map(|q| q as u32));
        }
    }
    let (one, two) = if d.ramp_type == RampType::OneWire { (wire(&v.middle), None) } else { (wire(&right), Some(wire(&left))) };
    let raise = |mut m: Mesh, z: f32| {
        for p in &mut m.positions {
            p[2] += z;
        }
        m
    };
    let upper = s.wire_distance_y * 0.5;
    let pieces: Vec<Mesh> = match (d.ramp_type, two) {
        (RampType::TwoWire, Some(two)) => vec![raise(one, 3.0), raise(two, 3.0)],
        (RampType::ThreeWireLeft, Some(two)) => vec![raise(one, 3.0), raise(two.clone(), 3.0), raise(two, upper)],
        (RampType::ThreeWireRight, Some(two)) => vec![raise(one.clone(), 3.0), raise(two, 3.0), raise(one, upper)],
        (RampType::FourWire, Some(two)) => vec![raise(one.clone(), 3.0), raise(two.clone(), 3.0), raise(one, upper), raise(two, upper)],
        _ => vec![one],
    };
    let mut mesh = Mesh::default();
    for p in pieces {
        let base = mesh.positions.len() as u32;
        mesh.indices.extend(faces.iter().map(|i| i + base));
        mesh.positions.extend(p.positions);
        mesh.normals.extend(p.normals);
        mesh.uvs.extend(p.uvs);
    }
    mesh
}

/// VPX's `CreateWire`: a tube of the wire's thickness through `points`, at
/// the ramp's heights, its normals straight out from the wire's middle.
fn create_wire(rings: usize, segments: usize, points: &[Vertex2D], heights: &[f32], diameter: f32) -> Mesh {
    let mut m = Mesh::default();
    let mut prev_b = Vertex3Ds::default();
    let (inv_nr, inv_ns) = (1.0 / rings as f32, 1.0 / segments as f32);
    for i in 0..rings {
        let i2 = if i == rings - 1 { i } else { i + 1 };
        let height = heights[i];
        let mut tangent = Vertex3Ds::new(points[i2].x - points[i].x, points[i2].y - points[i].y, heights[i2] - heights[i]);
        if i == rings - 1 {
            // for the last point the previous tangent again, or the wire stops a point early
            tangent.x = points[i].x - points[i - 1].x;
            tangent.y = points[i].y - points[i - 1].y;
        }
        let (mut binorm, mut normal);
        if i == 0 {
            let up = Vertex3Ds::new(points[i2].x + points[i].x, points[i2].y + points[i].y, heights[i2] - height);
            normal = cross_product(&tangent, &up);
            binorm = cross_product(&tangent, &normal);
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
            let tmp = get_rotated_axis(j as f32 * (360.0 * inv_ns), &tangent, &normal) * (diameter * 0.5);
            let p = [points[i].x + tmp.x, points[i].y + tmp.y, height + tmp.z];
            let n = [p[0] - points[i].x, p[1] - points[i].y, p[2] - height];
            let len = 1.0 / (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            m.positions.push(p);
            m.normals.push([n[0] * len, n[1] * len, n[2] * len]);
            m.uvs.push([u, v]);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(left_wall: f32, right_wall: f32) -> Ramp {
        Ramp {
            drag_points: [[500.0, 1000.0], [500.0, 200.0]].iter().map(|&[x, y]| vpin::vpx::gameitem::dragpoint::DragPoint { x, y, ..Default::default() }).collect(),
            height_bottom: 0.0,
            height_top: 100.0,
            width_bottom: 100.0,
            width_top: 100.0,
            ramp_type: VpinRampType::Flat,
            left_wall_height_visible: left_wall,
            right_wall_height_visible: right_wall,
            ..Default::default()
        }
    }

    #[test]
    fn a_flat_ramp_has_its_floor_and_only_the_walls_that_show() {
        let vpx = VPX::default();
        let materials = crate::look::Materials::new(&vpx);
        let s = shape(&vpx, &materials, &ramp(0.0, 50.0));
        assert_eq!((s.positions.len(), s.indices.len()), (8, 12));
        // the floor, right and left at each end, rising to the top height
        assert_eq!(&s.positions[..4], &[[550.0, 1000.0, 0.0], [450.0, 1000.0, 0.0], [550.0, 200.0, 100.0], [450.0, 200.0, 100.0]]);
        // the right wall, its foot at the floor's right edge
        assert_eq!(&s.positions[4..], &[[550.0, 1000.0, 0.0], [550.0, 1000.0, 50.0], [550.0, 200.0, 100.0], [550.0, 200.0, 150.0]]);
        // no picture: no picture places
        assert!(s.uvs.iter().all(|uv| *uv == [0.0; 2]));
    }

    #[test]
    fn a_wall_going_down_is_drawn_as_in_vpx() {
        // VPX draws a wall whenever its height isn't 0
        let vpx = VPX::default();
        let materials = crate::look::Materials::new(&vpx);
        let s = shape(&vpx, &materials, &ramp(-10.0, 50.0));
        assert_eq!((s.positions.len(), s.indices.len()), (12, 18));
        assert_eq!(s.positions[9], [450.0, 1000.0, -10.0]);
    }
}
