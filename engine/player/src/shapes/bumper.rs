// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/bumper.cpp (Bumper::RenderSetup and GenerateBaseMesh, GenerateSocketMesh,
// GenerateRingMesh, GenerateCapMesh).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A bumper's shape as VPX draws it at rest: VPX's four unit pieces turned
//! by the bumper's orientation, scaled by its radius (the cap by twice it)
//! and its height scale, standing on its surface — the skirt 5 above it,
//! the cap a height scale up. The ring's and skirt's movements when hit are
//! the scene's (`scene::BumperPiece`).

use physics_compat::math::def::ang_to_rad;
use physics_compat::math::{Matrix3D, Vertex2D, Vertex3Ds};
use table_physics::Surfaces;
use vpin::vpx::gameitem::bumper::Bumper;

use super::Mesh;
use super::bumper_mesh::{BASE_INDICES, BASE_VERTICES, CAP_INDICES, CAP_VERTICES, RING_INDICES, RING_VERTICES, SOCKET_INDICES, SOCKET_VERTICES};

/// A bumper's four pieces, as the export names them: `<name>Base`,
/// `<name>Socket` (the skirt), `<name>Ring` and `<name>Cap`.
pub struct Shape {
    pub base: Mesh,
    pub socket: Mesh,
    pub ring: Mesh,
    pub cap: Mesh,
}

pub fn shape(surfaces: &Surfaces, b: &Bumper) -> Shape {
    // RenderSetup
    let base_height = surfaces.height(&b.surface, Vertex2D::new(b.center.x, b.center.y));
    let full = Matrix3D::rotate_z(ang_to_rad(b.orientation));
    // each piece: turned, scaled across by `scale_xy`, its height scaled and lifted by `z_offset`
    let piece = |vertices: &[[f32; 8]], indices: &[u16], scale_xy: f32, z_offset: f32| {
        let mut m = Mesh::default();
        for v in vertices {
            let p = full.transform(&Vertex3Ds::new(v[0], v[1], v[2]));
            let n = full.multiply_vector_no_translate(&Vertex3Ds::new(v[3], v[4], v[5]));
            m.positions.push([p.x * scale_xy + b.center.x, p.y * scale_xy + b.center.y, p.z * b.height_scale + z_offset]);
            m.normals.push([n.x, n.y, n.z]);
            m.uvs.push([v[6], v[7]]);
        }
        m.indices = indices.iter().map(|&i| u32::from(i)).collect();
        m
    };
    Shape {
        base: piece(&BASE_VERTICES, &BASE_INDICES, b.radius, base_height),
        socket: piece(&SOCKET_VERTICES, &SOCKET_INDICES, b.radius, base_height + 5.0),
        ring: piece(&RING_VERTICES, &RING_INDICES, b.radius, base_height),
        cap: piece(&CAP_VERTICES, &CAP_INDICES, b.radius * 2.0, b.height_scale + base_height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpin::vpx::VPX;
    use vpin::vpx::gameitem::vertex2d::Vertex2D as Point;

    #[test]
    fn a_bumpers_cap_is_twice_its_radius_across_and_a_height_scale_up() {
        let vpx = VPX::default();
        let b = Bumper { center: Point { x: 500.0, y: 400.0 }, radius: 45.0, height_scale: 90.0, orientation: 0.0, ..Default::default() };
        let s = shape(&Surfaces::of_table(&vpx), &b);
        let c = CAP_VERTICES[0];
        assert_eq!(s.cap.positions[0], [c[0] * 90.0 + 500.0, c[1] * 90.0 + 400.0, c[2] * 90.0 + 90.0]);
        let k = SOCKET_VERTICES[0];
        assert_eq!(s.socket.positions[0], [k[0] * 45.0 + 500.0, k[1] * 45.0 + 400.0, k[2] * 90.0 + 5.0]);
        assert_eq!((s.base.indices.len(), s.ring.indices.len()), (BASE_INDICES.len(), RING_INDICES.len()));
    }
}
