// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/kicker.cpp (Kicker::RenderSetup's plate and GenerateMesh).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A kicker's shape as VPX draws it: the plate under it (a disc a little
//! smaller than the kicker, by its kind) and the kicker's own mesh, turned
//! by its orientation (a Williams kicker a quarter turn more, holes never),
//! scaled by its radius, standing on its surface. An invisible kicker has
//! neither.

use physics_compat::math::def::ang_to_rad;
use physics_compat::math::{Matrix3D, Vertex2D, Vertex3Ds};
use table_physics::Surfaces;
use vpin::vpx::gameitem::kicker::{Kicker, KickerType};

use super::Mesh;
use super::kicker_mesh::{
    CUP_INDICES, CUP_VERTICES, CUP2_INDICES, CUP2_VERTICES, GOTTLIEB_INDICES, GOTTLIEB_VERTICES, HOLE_INDICES, HOLE_VERTICES, PLATE_INDICES,
    PLATE_VERTICES, SIMPLE_HOLE_INDICES, SIMPLE_HOLE_VERTICES, WILLIAMS_INDICES, WILLIAMS_VERTICES,
};

/// A kicker's two pieces, as the export names them: `<name>Plate` and
/// `<name>Kicker`.
pub struct Shape {
    pub plate: Mesh,
    pub kicker: Mesh,
}

pub fn shape(surfaces: &Surfaces, k: &Kicker) -> Option<Shape> {
    if matches!(k.kicker_type, KickerType::Invisible) {
        return None;
    }
    // RenderSetup
    let base_height = surfaces.height(&k.surface, Vertex2D::new(k.center.x, k.center.y));
    // the plate: smaller than the kicker by its kind
    let rad = match k.kicker_type {
        KickerType::Williams | KickerType::Gottlieb => k.radius * 0.88,
        KickerType::Cup2 => k.radius * 0.87,
        KickerType::Cup => k.radius,
        _ => k.radius * 0.82,
    };
    let mut plate = Mesh::default();
    for v in &PLATE_VERTICES {
        plate.positions.push([v[0] * rad + k.center.x, v[1] * rad + k.center.y, v[2] * rad + base_height]);
        plate.normals.push([v[3], v[4], v[5]]);
        plate.uvs.push([0.0, 0.0]);
    }
    plate.indices = PLATE_INDICES.iter().map(|&i| u32::from(i)).collect();

    // GenerateMesh (an unknown kind draws as a simple hole)
    let (vertices, indices, z_offset, z_rot): (&[[f32; 8]], &[u16], f32, f32) = match k.kicker_type {
        KickerType::Cup => (&CUP_VERTICES, &CUP_INDICES, -0.18, k.orientation),
        KickerType::Williams => (&WILLIAMS_VERTICES, &WILLIAMS_INDICES, 0.0, k.orientation + 90.0),
        KickerType::Gottlieb => (&GOTTLIEB_VERTICES, &GOTTLIEB_INDICES, 0.0, k.orientation),
        KickerType::Cup2 => (&CUP2_VERTICES, &CUP2_INDICES, 0.0, k.orientation),
        KickerType::Hole => (&HOLE_VERTICES, &HOLE_INDICES, 0.0, 0.0),
        _ => (&SIMPLE_HOLE_VERTICES, &SIMPLE_HOLE_INDICES, 0.0, 0.0),
    };
    let full = Matrix3D::rotate_z(ang_to_rad(z_rot));
    let mut kicker = Mesh::default();
    for v in vertices {
        let p = full.transform(&Vertex3Ds::new(v[0], v[1], v[2] + z_offset));
        let n = full.multiply_vector_no_translate(&Vertex3Ds::new(v[3], v[4], v[5]));
        kicker.positions.push([p.x * k.radius + k.center.x, p.y * k.radius + k.center.y, p.z * k.radius + base_height]);
        kicker.normals.push([n.x, n.y, n.z]);
        kicker.uvs.push([v[6], v[7]]);
    }
    kicker.indices = indices.iter().map(|&i| u32::from(i)).collect();
    Some(Shape { plate, kicker })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpin::vpx::VPX;
    use vpin::vpx::gameitem::vertex2d::Vertex2D as Point;

    #[test]
    fn a_kickers_plate_is_smaller_by_its_kind_and_an_invisible_one_has_no_shape() {
        let surfaces = Surfaces::of_table(&VPX::default());
        let mut k = Kicker { center: Point { x: 300.0, y: 400.0 }, radius: 25.0, kicker_type: KickerType::Cup2, orientation: 0.0, ..Default::default() };
        let s = shape(&surfaces, &k).expect("a cup has a shape");
        let p = PLATE_VERTICES[0];
        let rad = 25.0 * 0.87;
        assert_eq!(s.plate.positions[0], [p[0] * rad + 300.0, p[1] * rad + 400.0, p[2] * rad]);
        let c = CUP2_VERTICES[0];
        assert_eq!(s.kicker.positions[0], [c[0] * 25.0 + 300.0, c[1] * 25.0 + 400.0, c[2] * 25.0]);
        k.kicker_type = KickerType::Invisible;
        assert!(shape(&surfaces, &k).is_none());
    }
}
