// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/gate.cpp (Gate::SetGateType, RenderSetup, GenerateBracketMesh, and the wire as
// Render places it).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A gate's shape as VPX draws it at rest: its bracket and its wire or
//! plate (by the gate's type), unit shapes turned by its rotation, scaled by
//! its length, at its height above its surface. The wire's swing is the
//! scene's (`scene::Motion::Axle`); VPX draws both without a picture.

use physics_compat::math::def::ang_to_rad;
use physics_compat::math::{Matrix3D, Vertex2D, Vertex3Ds};
use table_physics::Surfaces;
use vpin::vpx::gameitem::gate::{Gate, GateType};

use super::Mesh;
use super::gate_mesh::{
    BRACKET_INDICES, BRACKET_VERTICES, LONG_PLATE_INDICES, LONG_PLATE_VERTICES, PLATE_INDICES, PLATE_VERTICES, WIRE_RECTANGLE_INDICES,
    WIRE_RECTANGLE_VERTICES, WIRE_W_INDICES, WIRE_W_VERTICES,
};

/// A gate's two pieces, as the export names them: `<name>Bracket` and
/// `<name>Wire`.
pub struct Shape {
    pub bracket: Mesh,
    pub wire: Mesh,
}

/// Positions through `place` (VPX's `TransformPositions`), normals through
/// `turn` (`TransformNormals`), picture places as they are.
fn transformed(vertices: &[[f32; 8]], indices: &[u16], place: &Matrix3D, turn: &Matrix3D) -> Mesh {
    let mut m = Mesh::default();
    for v in vertices {
        let p = place.multiply_vector_no_perspective(&Vertex3Ds::new(v[0], v[1], v[2]));
        let n = turn.multiply_vector_no_translate(&Vertex3Ds::new(v[3], v[4], v[5]));
        m.positions.push([p.x, p.y, p.z]);
        m.normals.push([n.x, n.y, n.z]);
        m.uvs.push([v[6], v[7]]);
    }
    m.indices = indices.iter().map(|&i| u32::from(i)).collect();
    m
}

pub fn shape(surfaces: &Surfaces, g: &Gate) -> Shape {
    // SetGateType (an unknown type loads as the W wire)
    let (vertices, indices): (&[[f32; 8]], &[u16]) = match g.gate_type.as_ref().unwrap_or(&GateType::WireW) {
        GateType::WireW | GateType::Other(_) => (&WIRE_W_VERTICES, &WIRE_W_INDICES),
        GateType::WireRectangle => (&WIRE_RECTANGLE_VERTICES, &WIRE_RECTANGLE_INDICES),
        GateType::Plate => (&PLATE_VERTICES, &PLATE_INDICES),
        GateType::LongPlate => (&LONG_PLATE_VERTICES, &LONG_PLATE_INDICES),
    };
    let base_height = surfaces.height(&g.surface, Vertex2D::new(g.center.x, g.center.y));
    let rot = Matrix3D::rotate_z(ang_to_rad(g.rotation));
    let at = Matrix3D::translate(g.center.x, g.center.y, g.height + base_height);
    let scale = Matrix3D::scale(g.length, g.length, g.length);
    // GenerateBracketMesh
    let bracket = transformed(&BRACKET_VERTICES, &BRACKET_INDICES, &((rot * scale) * at), &rot);
    // Render: the wire at its resting swing (0; the scene turns it by the physics' angle)
    let full = Matrix3D::rotate_x(0.0) * rot;
    let wire = transformed(vertices, indices, &((full * scale) * at), &full);
    Shape { bracket, wire }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpin::vpx::VPX;
    use vpin::vpx::gameitem::vertex2d::Vertex2D as Point;

    #[test]
    fn a_gates_wire_is_scaled_by_its_length_as_vpx_draws_it() {
        // VPX's setup leaves the wire unscaled, but its first frame redraws it scaled
        let vpx = VPX::default();
        let g = Gate { center: Point { x: 300.0, y: 700.0 }, length: 80.0, height: 50.0, rotation: 0.0, gate_type: Some(GateType::Plate), ..Default::default() };
        let s = shape(&Surfaces::of_table(&vpx), &g);
        let w = PLATE_VERTICES[0];
        assert_eq!(s.wire.positions[0], [w[0] * 80.0 + 300.0, w[1] * 80.0 + 700.0, w[2] * 80.0 + 50.0]);
        assert_eq!(s.bracket.positions.len(), BRACKET_VERTICES.len());
    }
}
