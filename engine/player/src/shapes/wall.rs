// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/surface.cpp (Surface::GenerateMesh) and src/parts/dragpoint.cpp
// (IHaveDragPoints::GetTextureCoords).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A wall's shape as VPX builds it: its outline through its drag points, its
//! sides — four points for each stretch of the outline, their normals shared
//! where the outline is smooth — and its top, filled by VPX's triangulation
//! in one winding order only (a top VPX can't fill stays empty).

use physics_compat::math::{Vertex2D, Vertex3Ds};
use physics_compat::setup::curves::{DragPoint, RenderVertex, get_rg_vertex};
use vpin::vpx::VPX;
use vpin::vpx::gameitem::wall::Wall;

use super::Mesh;

/// A wall's two pieces: its sides and its top (not dropped).
pub struct WallShape {
    pub side: Mesh,
    pub top: Mesh,
}

pub fn shape(vpx: &VPX, w: &Wall) -> WallShape {
    let points: Vec<DragPoint> =
        w.drag_points.iter().map(|p| DragPoint { v: Vertex3Ds::new(p.x, p.y, p.z), smooth: p.smooth, slingshot: p.is_slingshot.unwrap_or(false) }).collect();
    let vv = get_rg_vertex::<Vertex2D>(&points, true, 4.0);
    let n = vv.len();
    if n == 0 {
        return WallShape { side: Mesh::default(), top: Mesh::default() };
    }
    // the side picture's coordinates along the outline, when the wall has one
    let side_picture = crate::look::image_named(vpx, &w.side_image).is_some();
    let texcoords = side_picture.then(|| texture_coords(&vv, &w.drag_points));

    // each stretch's normal (VPX's rgnormal)
    let rgnormal: Vec<[f32; 2]> = (0..n)
        .map(|i| {
            let (p1, p2) = (vv[i].v, vv[if i < n - 1 { i + 1 } else { 0 }].v);
            let (dx, dy) = (p1.x - p2.x, p1.y - p2.y);
            let inv_len = 1.0 / (dx * dx + dy * dy).sqrt();
            [dy * inv_len, dx * inv_len]
        })
        .collect();
    let normalize = |v: [f32; 2]| {
        let inv = 1.0 / (v[0] * v[0] + v[1] * v[1]).sqrt();
        [v[0] * inv, v[1] * inv]
    };

    let (bottom, top) = (w.height_bottom, w.height_top);
    let mut side = Mesh::default();
    for i in 0..n {
        let (pv1, pv2) = (&vv[i], &vv[if i < n - 1 { i + 1 } else { 0 }]);
        let a = if i == 0 { n - 1 } else { i - 1 };
        let c = if i < n - 1 { i + 1 } else { 0 };
        let n0 = normalize(if pv1.smooth { [(rgnormal[a][0] + rgnormal[i][0]) * 0.5, (rgnormal[a][1] + rgnormal[i][1]) * 0.5] } else { rgnormal[i] });
        let n1 = normalize(if pv2.smooth { [(rgnormal[i][0] + rgnormal[c][0]) * 0.5, (rgnormal[i][1] + rgnormal[c][1]) * 0.5] } else { rgnormal[i] });
        side.positions.extend([[pv1.v.x, pv1.v.y, bottom], [pv1.v.x, pv1.v.y, top], [pv2.v.x, pv2.v.y, top], [pv2.v.x, pv2.v.y, bottom]]);
        side.normals.extend([[n0[0], -n0[1], 0.0], [n0[0], -n0[1], 0.0], [n1[0], -n1[1], 0.0], [n1[0], -n1[1], 0.0]]);
        side.uvs.extend(match &texcoords {
            Some(t) => [[t[i], 1.0], [t[i], 0.0], [t[c], 0.0], [t[c], 1.0]],
            None => [[0.0; 2]; 4],
        });
        let o = (i * 4) as u32;
        side.indices.extend([o, o + 1, o + 2, o, o + 2, o + 3]);
    }

    // the top: VPX's triangulation, one winding order
    let outline: Vec<[f32; 2]> = vv.iter().map(|v| [v.v.x, v.v.y]).collect();
    let indices = crate::polygon::polygon_to_triangles(&outline, false);
    let mut top_mesh = Mesh::default();
    if !indices.is_empty() {
        let g = &vpx.gamedata;
        let (inv_width, inv_height) = (1.0 / (g.right - g.left), 1.0 / (g.bottom - g.top));
        for p in &outline {
            top_mesh.positions.push([p[0], p[1], top]);
            top_mesh.normals.push([0.0, 0.0, 1.0]);
            top_mesh.uvs.push([p[0] * inv_width, p[1] * inv_height]);
        }
        top_mesh.indices = indices;
    }
    WallShape { side, top: top_mesh }
}

/// VPX's `IHaveDragPoints::GetTextureCoords`: a picture's coordinate at each
/// point of the outline, spread by length between the control points that
/// set one (all of it, from 0 to 1, when none does).
fn texture_coords(vv: &[RenderVertex<Vertex2D>], dpoints: &[vpin::vpx::gameitem::dragpoint::DragPoint]) -> Vec<f32> {
    let cpoints = vv.len() as i32;
    let mut coords = vec![0.0f32; vv.len()];
    let (mut vitexpoints, mut virenderpoints): (Vec<i32>, Vec<i32>) = (Vec::new(), Vec::new());
    let mut icontrolpoint = 0usize;
    for (i, prv) in vv.iter().enumerate() {
        if prv.control_point {
            if dpoints.get(icontrolpoint).is_some_and(|d| !d.has_auto_texture) {
                vitexpoints.push(icontrolpoint as i32);
                virenderpoints.push(i as i32);
            }
            icontrolpoint += 1;
        }
    }
    let no_coords = vitexpoints.is_empty();
    if no_coords {
        vitexpoints.push(0);
        virenderpoints.push(0);
    }
    // wrap the array around so we cover the last section
    vitexpoints.push(vitexpoints[0] + dpoints.len() as i32);
    virenderpoints.push(virenderpoints[0] + cpoints);
    let at = |k: i32| vv[k.rem_euclid(cpoints) as usize].v;
    for i in 0..vitexpoints.len() - 1 {
        let start = virenderpoints[i] % cpoints;
        let mut end = virenderpoints[if (i as i32) < cpoints - 1 { i + 1 } else { 0 }] % cpoints;
        let (start_coord, end_coord) = if no_coords {
            (0.0, 1.0)
        } else {
            let tex = |k: i32| dpoints[(k as usize) % dpoints.len()].tex_coord;
            (tex(vitexpoints[i]), tex(vitexpoints[i + 1]))
        };
        let delta = end_coord - start_coord;
        if end <= start {
            end += cpoints;
        }
        let length = |l: i32| {
            let (p1, p2) = (at(l), at(l + 1));
            let (dx, dy) = (p1.x - p2.x, p1.y - p2.y);
            (dx * dx + dy * dy).sqrt()
        };
        let mut total: f32 = (start..end).map(length).sum();
        let mut partial = 0.0f32;
        for l in start..end {
            if total == 0.0 {
                total = 1.0;
            }
            coords[(l % cpoints) as usize] = (partial / total) * delta + start_coord;
            partial += length(l);
        }
    }
    coords
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(points: &[[f32; 2]]) -> Wall {
        Wall {
            drag_points: points.iter().map(|&[x, y]| vpin::vpx::gameitem::dragpoint::DragPoint { x, y, ..Default::default() }).collect(),
            height_top: 50.0,
            height_bottom: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn a_square_wall_has_four_sides_and_a_two_triangle_top() {
        let mut vpx = VPX::default();
        (vpx.gamedata.left, vpx.gamedata.top, vpx.gamedata.right, vpx.gamedata.bottom) = (0.0, 0.0, 1000.0, 2000.0);
        let s = shape(&vpx, &wall(&[[100.0, 100.0], [100.0, 200.0], [200.0, 200.0], [200.0, 100.0]]));
        assert_eq!((s.side.positions.len(), s.side.indices.len()), (16, 24));
        assert_eq!((s.top.positions.len(), s.top.indices.len()), (4, 6));
        // the top at its height, its picture places across the table, facing up
        assert!(s.top.positions.iter().all(|p| p[2] == 50.0));
        assert_eq!(s.top.uvs[0], [0.1, 0.05]);
        assert!(s.top.normals.iter().all(|n| *n == [0.0, 0.0, 1.0]));
        // the first stretch's side: bottom and top at its start, then at its end
        assert_eq!(&s.side.positions[..4], &[[100.0, 100.0, 0.0], [100.0, 100.0, 50.0], [100.0, 200.0, 50.0], [100.0, 200.0, 0.0]]);
    }

    #[test]
    fn a_top_wound_the_other_way_stays_empty_as_in_vpx() {
        // VPX's PolygonToTriangles tries one winding order for walls
        let s = shape(&VPX::default(), &wall(&[[100.0, 100.0], [200.0, 100.0], [200.0, 200.0], [100.0, 200.0]]));
        assert!(s.top.indices.is_empty() && s.top.positions.is_empty());
        assert_eq!(s.side.positions.len(), 16);
    }
}
