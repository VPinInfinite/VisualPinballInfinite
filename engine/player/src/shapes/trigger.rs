// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/trigger.cpp (Trigger::RenderSetup and GenerateMesh).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A trigger's shape as VPX draws it at rest: the mesh of its kind turned
//! by its rotation (wires B and C tipped over first), a star or button
//! scaled by its radius, a wire by its own scales and pushed out by its
//! thickness, standing on its surface (a button 5 above it, wire C 19
//! below). An invisible trigger, or one without a shape, has none.

use physics_compat::math::def::ang_to_rad;
use physics_compat::math::{Matrix3D, Vertex2D, Vertex3Ds};
use table_physics::Surfaces;
use vpin::vpx::gameitem::trigger::{Trigger, TriggerShape};

use super::Mesh;
use super::trigger_mesh::{
    BUTTON_INDICES, BUTTON_VERTICES, INDER_INDICES, INDER_VERTICES, SIMPLE_INDICES, SIMPLE_VERTICES, STAR_INDICES, STAR_VERTICES, WIRE_D_INDICES,
    WIRE_D_VERTICES,
};

/// The trigger's one piece, as the export names it: the trigger's name.
pub fn shape(surfaces: &Surfaces, t: &Trigger) -> Option<Mesh> {
    // RenderSetup
    if !t.is_visible || matches!(t.shape, TriggerShape::None) {
        return None;
    }
    // GenerateMesh (an unknown kind has no mesh)
    let base_height = surfaces.height(&t.surface, Vertex2D::new(t.center.x, t.center.y));
    let z_offset = match t.shape {
        TriggerShape::Button => 5.0,
        TriggerShape::WireC => -19.0,
        _ => 0.0,
    };
    let (vertices, indices): (&[[f32; 8]], &[u16]) = match t.shape {
        TriggerShape::WireA | TriggerShape::WireB | TriggerShape::WireC => (&SIMPLE_VERTICES, &SIMPLE_INDICES),
        TriggerShape::WireD => (&WIRE_D_VERTICES, &WIRE_D_INDICES),
        TriggerShape::Inder => (&INDER_VERTICES, &INDER_INDICES),
        TriggerShape::Button => (&BUTTON_VERTICES, &BUTTON_INDICES),
        TriggerShape::Star => (&STAR_VERTICES, &STAR_INDICES),
        _ => return None,
    };
    let rot = Matrix3D::rotate_z(ang_to_rad(t.rotation));
    let full = match t.shape {
        TriggerShape::WireB => Matrix3D::rotate_x(ang_to_rad(-23.0)) * rot,
        TriggerShape::WireC => Matrix3D::rotate_x(ang_to_rad(140.0)) * rot,
        _ => rot,
    };
    let by_radius = matches!(t.shape, TriggerShape::Button | TriggerShape::Star);
    let wire = matches!(t.shape, TriggerShape::WireA | TriggerShape::WireB | TriggerShape::WireC | TriggerShape::WireD | TriggerShape::Inder);
    let thickness = t.wire_thickness.unwrap_or(0.0);
    let mut m = Mesh::default();
    for v in vertices {
        let p = full.transform(&Vertex3Ds::new(v[0], v[1], v[2]));
        let mut at = if by_radius {
            [p.x * t.radius + t.center.x, p.y * t.radius + t.center.y, p.z * t.radius + base_height + z_offset]
        } else {
            [p.x * t.scale_x + t.center.x, p.y * t.scale_y + t.center.y, p.z * 1.0 + base_height + z_offset]
        };
        let n = full.multiply_vector_no_translate(&Vertex3Ds::new(v[3], v[4], v[5]));
        // a wire's thickness pushes it out along its normals
        if wire {
            at[0] += n.x * thickness;
            at[1] += n.y * thickness;
            at[2] += n.z * thickness;
        }
        m.positions.push(at);
        m.normals.push([n.x, n.y, n.z]);
        m.uvs.push([v[6], v[7]]);
    }
    m.indices = indices.iter().map(|&i| u32::from(i)).collect();
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpin::vpx::VPX;
    use vpin::vpx::gameitem::vertex2d::Vertex2D as Point;

    #[test]
    fn a_button_is_scaled_by_its_radius_and_stands_5_up() {
        let surfaces = Surfaces::of_table(&VPX::default());
        let t = Trigger { center: Point { x: 100.0, y: 200.0 }, radius: 20.0, rotation: 0.0, shape: TriggerShape::Button, is_visible: true, ..Default::default() };
        let m = shape(&surfaces, &t).expect("a visible button has a shape");
        let b = BUTTON_VERTICES[0];
        assert_eq!(m.positions[0], [b[0] * 20.0 + 100.0, b[1] * 20.0 + 200.0, b[2] * 20.0 + 5.0]);
        assert!(shape(&surfaces, &Trigger { is_visible: false, ..t }).is_none());
    }
}
