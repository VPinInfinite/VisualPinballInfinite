// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/decal.cpp (Decal::EnsureSize for pictures, and RenderSetup's quad).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A picture decal's shape as VPX draws it: a quad of its size (or, sized
//! by its picture, as wide as the picture's shape makes it), turned by its
//! rotation, 0.2 above its surface. Text decals, which VPX lays out with
//! Windows' fonts, and decals on the backdrop are not ported yet.

use physics_compat::math::Vertex2D;
use physics_compat::math::def::ang_to_rad;
use table_physics::Surfaces;
use vpin::vpx::VPX;
use vpin::vpx::gameitem::decal::{Decal, DecalType, SizingType};

use super::Mesh;

/// The decal's one piece, as the export names it: the decal's name.
pub fn shape(vpx: &VPX, surfaces: &Surfaces, d: &Decal) -> Option<Mesh> {
    if !matches!(d.decal_type, DecalType::Image) || d.backglass {
        return None;
    }
    // EnsureSize: a picture decal sized by its picture keeps its height
    let (width, height) = if matches!(d.sizing_type, SizingType::AutoWidth) {
        let picture = crate::look::image_named(vpx, &d.image).map(|i| &vpx.images[i]);
        let aspect = picture.map_or(1.0, |p| (f64::from(p.width) / f64::from(p.height)) as f32);
        (d.height * aspect, d.height)
    } else {
        (d.width, d.height)
    };
    // RenderSetup (a picture has no leading or descent)
    let z = surfaces.height(&d.surface, Vertex2D::new(d.center.x, d.center.y)) + 0.2;
    let (half_width, half_height) = (width * 0.5, height * 0.5);
    let angle = ang_to_rad(d.rotation);
    let (sn, cs) = (angle.sin(), angle.cos());
    let (cx, cy) = (d.center.x, d.center.y);
    let positions = vec![
        [cx + sn * half_height - cs * half_width, cy - cs * half_height - sn * half_width, z],
        [cx + sn * half_height + cs * half_width, cy - cs * half_height + sn * half_width, z],
        [cx - sn * half_height - cs * half_width, cy + cs * half_height - sn * half_width, z],
        [cx - sn * half_height + cs * half_width, cy + cs * half_height + sn * half_width, z],
    ];
    Some(Mesh {
        positions,
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
        // VPX draws the four as a strip
        indices: vec![0, 1, 2, 2, 1, 3],
    })
}
