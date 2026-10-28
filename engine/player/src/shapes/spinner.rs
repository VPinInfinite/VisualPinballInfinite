// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/spinner.cpp (Spinner::RenderSetup and UpdatePlate).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A spinner's shape as VPX draws it at rest: its bracket and its plate,
//! unit shapes turned by its rotation, scaled by its length, at its height
//! above its surface. The plate's spin is the scene's (`scene::Motion::Axle`).

use physics_compat::math::def::ang_to_rad;
use physics_compat::math::{Matrix3D, Vertex2D, Vertex3Ds};
use table_physics::Surfaces;
use vpin::vpx::gameitem::spinner::Spinner;

use super::Mesh;
use super::spinner_mesh::{BRACKET_INDICES, BRACKET_VERTICES, PLATE_INDICES, PLATE_VERTICES};

/// A spinner's two pieces, as the export names them: `<name>Bracket` and
/// `<name>Plate`.
pub struct Shape {
    pub bracket: Mesh,
    pub plate: Mesh,
}

pub fn shape(surfaces: &Surfaces, s: &Spinner) -> Shape {
    // RenderSetup
    let pos_z = surfaces.height(&s.surface, Vertex2D::new(s.center.x, s.center.y)) + s.height;
    let piece = |vertices: &[[f32; 8]], indices: &[u16], full: &Matrix3D| {
        let mut m = Mesh::default();
        for v in vertices {
            let p = full.transform(&Vertex3Ds::new(v[0], v[1], v[2]));
            let n = full.multiply_vector_no_translate(&Vertex3Ds::new(v[3], v[4], v[5]));
            m.positions.push([p.x * s.length + s.center.x, p.y * s.length + s.center.y, p.z * s.length + pos_z]);
            m.normals.push([n.x, n.y, n.z]);
            m.uvs.push([v[6], v[7]]);
        }
        m.indices = indices.iter().map(|&i| u32::from(i)).collect();
        m
    };
    let rot = Matrix3D::rotate_z(ang_to_rad(s.rotation));
    // UpdatePlate at the plate's resting angle (0; the scene turns it by the physics' angle)
    let plate_turn = Matrix3D::rotate_x(-0.0) * rot;
    Shape { bracket: piece(&BRACKET_VERTICES, &BRACKET_INDICES, &rot), plate: piece(&PLATE_VERTICES, &PLATE_INDICES, &plate_turn) }
}
