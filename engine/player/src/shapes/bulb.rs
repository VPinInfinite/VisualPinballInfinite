// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/light.cpp (Light::RenderSetup's bulb and socket meshes).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A light's bulb and socket as VPX draws them: unit shapes scaled by the
//! light's mesh radius, their base at the light's surface ("the base of the
//! glass", VPX's note). Which lights show them is the scene's (VPX's rules,
//! with older tables' among them).

use physics_compat::math::Vertex2D;
use table_physics::Surfaces;
use vpin::vpx::gameitem::light::Light;

use super::Mesh;
use super::bulb_mesh::{BULB_INDICES, BULB_VERTICES, SOCKET_INDICES, SOCKET_VERTICES};

/// A light's two pieces, as the export names them: `<name>_bulb` and
/// `<name>_socket`.
pub struct Shape {
    pub bulb: Mesh,
    pub socket: Mesh,
}

pub fn shape(surfaces: &Surfaces, l: &Light) -> Shape {
    let bulb_z = surfaces.height(&l.surface, Vertex2D::new(l.center.x, l.center.y));
    let r = l.mesh_radius;
    let piece = |vertices: &[[f32; 8]], indices: &[u16]| {
        let mut m = Mesh::default();
        for v in vertices {
            m.positions.push([v[0] * r + l.center.x, v[1] * r + l.center.y, v[2] * r + bulb_z]);
            m.normals.push([v[3], v[4], v[5]]);
            m.uvs.push([v[6], v[7]]);
        }
        m.indices = indices.iter().map(|&i| u32::from(i)).collect();
        m
    };
    Shape { bulb: piece(&BULB_VERTICES, &BULB_INDICES), socket: piece(&SOCKET_VERTICES, &SOCKET_INDICES) }
}
