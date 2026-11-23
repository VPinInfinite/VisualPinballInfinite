// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/light.cpp (Light::RenderSetup and UpdateMeshBuffer: the light's own mesh).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A light's own shape on the table, which its glow is drawn on: the
//! outline through its points (`GetRgVertex`, accuracy 4), filled
//! (`PolygonToTriangles`), 0.1 above its surface. Its picture places span
//! the table when the light names a picture the table has, else run from
//! its centre out to its farthest point. A bulb light's glow floats at its
//! halo height, which the renderer adds.

use physics_compat::math::{Vertex2D, Vertex3Ds};
use physics_compat::setup::curves::{DragPoint, get_rg_vertex};
use table_physics::Surfaces;
use vpin::vpx::VPX;
use vpin::vpx::gameitem::light::Light;

use super::Mesh;

/// The light's shape, as the export names it: `<name>_insert`. None for a
/// light on the backdrop, or one whose outline can't be filled (VPX: "has an
/// invalid shape! It can not be rendered!").
pub fn shape(vpx: &VPX, surfaces: &Surfaces, l: &Light) -> Option<Mesh> {
    if l.is_backglass {
        return None;
    }
    let points: Vec<DragPoint> =
        l.drag_points.iter().map(|p| DragPoint { v: Vertex3Ds::new(p.x, p.y, p.z), smooth: p.smooth, slingshot: p.is_slingshot.unwrap_or(false) }).collect();
    let outline: Vec<[f32; 2]> = get_rg_vertex::<Vertex2D>(&points, true, 4.0).iter().map(|v| [v.v.x, v.v.y]).collect();
    let indices = crate::polygon::polygon_to_triangles(&outline, true);
    if indices.is_empty() {
        return None;
    }
    let (cx, cy) = (l.center.x, l.center.y);
    let max_dist = outline.iter().fold(0.0f32, |m, p| {
        let (dx, dy) = (p[0] - cx, p[1] - cy);
        let d = dx * dx + dy * dy;
        if d > m { d } else { m }
    });
    let inv_maxdist = if max_dist > 0.0 { 0.5 / max_dist.sqrt() } else { 0.0 };
    let g = &vpx.gamedata;
    let (inv_width, inv_height) = (1.0 / (g.right - g.left), 1.0 / (g.bottom - g.top));
    let picture = crate::look::image_named(vpx, &l.image).is_some();
    let z = surfaces.height(&l.surface, Vertex2D::new(cx, cy)) + 0.1;
    Some(Mesh {
        positions: outline.iter().map(|p| [p[0], p[1], z]).collect(),
        normals: vec![[0.0, 0.0, 1.0]; outline.len()],
        uvs: outline
            .iter()
            .map(|p| if picture { [p[0] * inv_width, p[1] * inv_height] } else { [0.5 + (p[0] - cx) * inv_maxdist, 0.5 + (p[1] - cy) * inv_maxdist] })
            .collect(),
        indices,
    })
}
