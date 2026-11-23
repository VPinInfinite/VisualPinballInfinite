// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/hittarget.cpp (HitTarget::SetMeshType and GenerateMesh).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A target's shape as VPX draws it at rest: the mesh of its kind scaled by
//! its size, turned by its rotation and placed at its position (a target
//! sets its own height). A drop target's fall and a hit target's swing are
//! the scene's.

use physics_compat::math::def::ang_to_rad;
use physics_compat::math::{Matrix3D, Vertex3Ds};
use vpin::vpx::gameitem::hittarget::{HitTarget, TargetType};

use super::Mesh;
use super::hit_target_mesh::{
    DROP_BEVELED_INDICES, DROP_BEVELED_VERTICES, DROP_FLAT_SIMPLE_INDICES, DROP_FLAT_SIMPLE_VERTICES, DROP_SIMPLE_INDICES, DROP_SIMPLE_VERTICES,
    FAT_RECTANGLE_INDICES, FAT_RECTANGLE_VERTICES, FAT_SLIM_INDICES, FAT_SLIM_VERTICES, FAT_SQUARE_INDICES, FAT_SQUARE_VERTICES, RECTANGLE_INDICES,
    RECTANGLE_VERTICES, ROUND_INDICES, ROUND_VERTICES, SLIM_INDICES, SLIM_VERTICES,
};

/// The target's one piece, as the export names it: the target's name.
pub fn shape(t: &HitTarget) -> Mesh {
    // SetMeshType (an unknown kind is drawn as the simple drop target)
    let (vertices, indices): (&[[f32; 8]], &[u16]) = match t.target_type {
        TargetType::DropTargetBeveled => (&DROP_BEVELED_VERTICES, &DROP_BEVELED_INDICES),
        TargetType::DropTargetFlatSimple => (&DROP_FLAT_SIMPLE_VERTICES, &DROP_FLAT_SIMPLE_INDICES),
        TargetType::HitTargetRound => (&ROUND_VERTICES, &ROUND_INDICES),
        TargetType::HitTargetRectangle => (&RECTANGLE_VERTICES, &RECTANGLE_INDICES),
        TargetType::HitFatTargetRectangle => (&FAT_RECTANGLE_VERTICES, &FAT_RECTANGLE_INDICES),
        TargetType::HitFatTargetSquare => (&FAT_SQUARE_VERTICES, &FAT_SQUARE_INDICES),
        TargetType::HitTargetSlim => (&SLIM_VERTICES, &SLIM_INDICES),
        TargetType::HitFatTargetSlim => (&FAT_SLIM_VERTICES, &FAT_SLIM_INDICES),
        _ => (&DROP_SIMPLE_VERTICES, &DROP_SIMPLE_INDICES),
    };
    // GenerateMesh
    let full = Matrix3D::rotate_z(ang_to_rad(t.rot_z));
    let mut m = Mesh::default();
    for v in vertices {
        let p = full.transform(&Vertex3Ds::new(v[0] * t.size.x, v[1] * t.size.y, v[2] * t.size.z));
        let n = full.multiply_vector_no_translate(&Vertex3Ds::new(v[3], v[4], v[5]));
        m.positions.push([p.x + t.position.x, p.y + t.position.y, p.z + t.position.z]);
        m.normals.push([n.x, n.y, n.z]);
        m.uvs.push([v[6], v[7]]);
    }
    m.indices = indices.iter().map(|&i| u32::from(i)).collect();
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpin::vpx::gameitem::vertex3d::Vertex3D;

    #[test]
    fn a_target_is_scaled_by_its_size_and_placed_at_its_position() {
        let t = HitTarget {
            position: Vertex3D { x: 500.0, y: 600.0, z: 0.0 },
            size: Vertex3D { x: 32.0, y: 32.0, z: 40.0 },
            rot_z: 0.0,
            target_type: TargetType::DropTargetSimple,
            ..Default::default()
        };
        let m = shape(&t);
        let d = DROP_SIMPLE_VERTICES[0];
        assert_eq!(m.positions[0], [d[0] * 32.0 + 500.0, d[1] * 32.0 + 600.0, d[2] * 40.0]);
    }
}
