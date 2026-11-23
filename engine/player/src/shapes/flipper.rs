// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/flipper.cpp (Flipper::GenerateBaseMesh and ApplyFix; the length GetHitShapes gives
// the flipper; Load's repair of broken heights; the place Render draws it at, its start angle
// before it moves).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A flipper's shape as VPX draws it: VPX's flipper mesh, its two end rings
//! moved out to the flipper's radii and length, standing on its surface; its
//! rubber the same mesh, thicker, raised to the rubber's height; both turned
//! to the flipper's start angle about its centre.

use std::f64::consts::PI;

use physics_compat::math::def::{ang_to_rad, max};
use physics_compat::math::{Matrix3D, Vertex2D, Vertex3Ds};
use table_physics::Surfaces;
use vpin::vpx::VPX;
use vpin::vpx::gameitem::flipper::Flipper;

use super::Mesh;
use super::flipper_mesh::{BASE_BOTTOM, BASE_TOP, INDICES, TIP_BOTTOM, TIP_TOP, VERTICES};

/// A flipper's two pieces, as the export names them: `<name>Base` and
/// `<name>Rubber` (none without a rubber).
pub struct Shape {
    pub base: Mesh,
    pub rubber: Option<Mesh>,
}

/// VPX's `sgn`.
fn sgn(a: f32) -> f32 {
    if a > 0.0 {
        1.0
    } else if a < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// VPX's `ApplyFix`: a ring point moved round its end's centre to `radius`,
/// its angle pulled towards `mid_angle` by the share that keeps the
/// flipper's sides straight between ends of different radii. (The normal it
/// turns too goes unused: `GenerateBaseMesh` keeps the mesh's own.)
fn apply_fix(vert: &mut [f32; 8], center: Vertex2D, mid_angle: f32, radius: f32, new_center: Vertex2D, fix_angle_scale: f32) {
    let mut v_angle = (vert[1] - center.y).atan2(vert[0] - center.x);
    // we want to have angles with same sign as midAngle, fix it:
    if mid_angle < 0.0 {
        if v_angle > 0.0 {
            v_angle -= (PI * 2.0) as f32;
        }
    } else if v_angle < 0.0 {
        v_angle += (PI * 2.0) as f32;
    }
    v_angle -= (v_angle - mid_angle) * fix_angle_scale * sgn(mid_angle);
    vert[0] = v_angle.cos() * radius + new_center.x;
    vert[1] = v_angle.sin() * radius + new_center.y;
}

/// The mesh with its end rings moved: the base's round the origin to
/// `base_radius`, the tip's round `(0, length)` to `end_radius`.
fn fixed_mesh(base_radius: f32, end_radius: f32, length: f32, fix_angle_scale: f32) -> [[f32; 8]; 104] {
    let mut temp = VERTICES;
    let base_center = Vertex2D::new(BASE_BOTTOM[6][0], BASE_BOTTOM[0][1]);
    let tip_center = Vertex2D::new(TIP_BOTTOM[6][0], TIP_BOTTOM[0][1]);
    let same = |v: &[f32; 8], r: &[f32; 3]| v[0] == r[0] && v[1] == r[1] && v[2] == r[2];
    let (down, up) = ((-PI * 0.5) as f32, (PI * 0.5) as f32);
    for t in 0..13 {
        for v in &mut temp {
            if same(v, &BASE_BOTTOM[t]) {
                apply_fix(v, base_center, down, base_radius, Vertex2D::new(0.0, 0.0), fix_angle_scale);
            }
            if same(v, &TIP_BOTTOM[t]) {
                apply_fix(v, tip_center, up, end_radius, Vertex2D::new(0.0, length), fix_angle_scale);
            }
            if same(v, &BASE_TOP[t]) {
                apply_fix(v, base_center, down, base_radius, Vertex2D::new(0.0, 0.0), fix_angle_scale);
            }
            if same(v, &TIP_TOP[t]) {
                apply_fix(v, tip_center, up, end_radius, Vertex2D::new(0.0, length), fix_angle_scale);
            }
        }
    }
    temp
}

pub fn shape(vpx: &VPX, surfaces: &Surfaces, f: &Flipper) -> Shape {
    // the length GetHitShapes gives the flipper (by the table's difficulty, when it has a range)
    let length = if f.flipper_radius_min > 0.0 && f.flipper_radius_max > f.flipper_radius_min {
        let r = f.flipper_radius_max - (f.flipper_radius_max - f.flipper_radius_min) * vpx.gamedata.global_difficulty;
        max(r, f.base_radius - f.end_radius + 0.05)
    } else {
        f.flipper_radius_max
    };
    // Load repairs broken heights
    let flipper_height = if f.height > 1000.0 { 50.0 } else { f.height };
    let rubber_thickness = f.rubber_thickness.unwrap_or(f.rubber_thickness_int as f32);
    let rubber_height = f.rubber_height.unwrap_or(f.rubber_height_int as f32);
    let rubber_width = f.rubber_width.unwrap_or(f.rubber_width_int as f32);
    let center = Vertex2D::new(f.center.x, f.center.y);

    // GenerateBaseMesh
    let full = Matrix3D::rotate_z(ang_to_rad(180.0));
    let height = surfaces.height(&f.surface, center);
    // the angle that keeps P0 in place
    let sin_angle = ((f.base_radius - f.end_radius) / length).clamp(-1.0, 1.0);
    let fix_angle = sin_angle.asin();
    let fix_angle_scale = fix_angle * (1.0 / (PI * 0.5)) as f32;
    let base_radius = f.base_radius - rubber_thickness;
    let end_radius = f.end_radius - rubber_thickness;

    // Render: turned by the flipper's angle (its start angle, before it moves) about its centre
    let place = Matrix3D::rotate_z(ang_to_rad(f.start_angle)) * Matrix3D::translate(center.x, center.y, 0.0);
    let piece = |temp: &[[f32; 8]; 104], z_scale: f32, z_offset: f32, tv_offset: f32| {
        let mut m = Mesh::default();
        for (t, v) in temp.iter().zip(&VERTICES) {
            let p = full.transform(&Vertex3Ds::new(t[0], t[1], t[2]));
            let p = place.transform(&Vertex3Ds::new(p.x, p.y, p.z * z_scale + z_offset));
            let n = full.multiply_vector_no_translate(&Vertex3Ds::new(v[3], v[4], v[5]));
            let n = place.multiply_vector_no_translate(&n);
            m.positions.push([p.x, p.y, p.z]);
            m.normals.push([n.x, n.y, n.z]);
            m.uvs.push([v[6], v[7] + tv_offset]);
        }
        m.indices = INDICES.iter().map(|&i| u32::from(i)).collect();
        m
    };
    let base = piece(&fixed_mesh(base_radius, end_radius, length, fix_angle_scale), flipper_height, height, 0.0);
    // the rubber: the same mesh out to the full radii, at the rubber's height and width
    let rubber = (rubber_thickness > 0.0).then(|| {
        let temp = fixed_mesh(base_radius + rubber_thickness, end_radius + rubber_thickness, length, fix_angle_scale);
        piece(&temp, rubber_width, height + rubber_height, 0.5)
    });
    Shape { base, rubber }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpin::vpx::gameitem::GameItemEnum;
    use vpin::vpx::gameitem::vertex2d::Vertex2D as Point;
    use vpin::vpx::gameitem::wall::Wall;

    #[test]
    fn a_flipper_reaches_its_length_and_stands_on_its_surface() {
        let mut vpx = VPX::default();
        vpx.gameitems.push(GameItemEnum::Wall(Wall { name: "upper".into(), height_top: 80.0, ..Default::default() }));
        let f = Flipper {
            center: Point { x: 300.0, y: 1500.0 },
            base_radius: 20.0,
            end_radius: 12.0,
            flipper_radius_max: 110.0,
            start_angle: 0.0,
            height: 50.0,
            rubber_thickness: Some(5.0),
            rubber_height: Some(17.0),
            rubber_width: Some(23.0),
            surface: "upper".into(),
            ..Default::default()
        };
        let s = shape(&vpx, &Surfaces::of_table(&vpx), &f);
        let rubber = s.rubber.expect("a rubber 5 thick");
        let lowest = |m: &Mesh, k: usize| m.positions.iter().map(|p| p[k]).fold(f32::MAX, f32::min);
        let highest = |m: &Mesh, k: usize| m.positions.iter().map(|p| p[k]).fold(f32::MIN, f32::max);
        // at angle 0 the flipper points up the table (VPX turns its mesh half a turn): the
        // tip's far edge at its length and end radius, less the rubber for the flipper itself
        assert!((lowest(&s.base, 1) - (1500.0 - 110.0 - 7.0)).abs() < 1e-3);
        assert!((lowest(&rubber, 1) - (1500.0 - 110.0 - 12.0)).abs() < 1e-3);
        // standing on the wall: the mesh's bottom and top scaled by the flipper's height
        assert!((lowest(&s.base, 2) - (0.003753 * 50.0 + 80.0)).abs() < 1e-3);
        assert!((highest(&s.base, 2) - (1.004253 * 50.0 + 80.0)).abs() < 1e-3);
        // the rubber raised to its height, its width its own
        assert!((lowest(&rubber, 2) - (0.003753 * 23.0 + 97.0)).abs() < 1e-3);
        // the rubber's picture places in the lower half of the flipper's picture
        assert!(rubber.uvs.iter().zip(&s.base.uvs).all(|(r, b)| r[0] == b[0] && r[1] == b[1] + 0.5));
    }
}
