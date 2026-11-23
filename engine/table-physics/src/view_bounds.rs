// Follows Visual Pinball X (license:GPLv3+), commit af26b2d (10.8.1 pre-release build 5436):
// GetBoundingVertices of Surface, Ramp and Rubber (src/parts/surface.cpp, ramp.cpp, rubber.cpp).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The vertices VPX fits its *legacy* camera to, and the walls', ramps' and
//! rubbers' share of those its depth range covers: each as the 8 corners of
//! a box.

use physics_compat::math::{Vertex2D, Vertex3Ds};
use physics_compat::physconst::{HIT_SHAPE_DETAIL_LEVEL, PHYS_SKIN};
use physics_compat::setup::curves::{RenderVertex, get_rg_vertex};
use physics_compat::setup::ramp::ramp_vertex;
use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;

use crate::materials::Materials;
use crate::{drag_points, ramp};

fn corners(min: Vertex3Ds, max: Vertex3Ds, out: &mut Vec<Vertex3Ds>) {
    for i in 0..8 {
        out.push(Vertex3Ds::new(
            if i & 1 != 0 { min.x } else { max.x },
            if i & 2 != 0 { min.y } else { max.y },
            if i & 4 != 0 { min.z } else { max.z },
        ));
    }
}

/// VPX's legacy bounds for every wall, ramp and rubber, visible or not.
///
/// A rubber's box is its centre line widened by half its thickness, where
/// VPX uses the outline it draws; the two differ only at sharp bends, by a
/// fraction of the thickness.
pub fn legacy_view_bounds(vpx: &VPX) -> Vec<Vertex3Ds> {
    view_bounds(vpx, false)
}

/// The same boxes as VPX gathers them for the camera's depth range
/// (`ComputeNearFarPlane`): every wall — a wall's own `m_visible` is never
/// read from the file, so it always counts — and only the ramps and rubbers
/// shown. Primitives add theirs where the scene is built.
pub fn depth_view_bounds(vpx: &VPX) -> Vec<Vertex3Ds> {
    view_bounds(vpx, true)
}

fn view_bounds(vpx: &VPX, shown_only: bool) -> Vec<Vertex3Ds> {
    let g = &vpx.gamedata;
    let materials = Materials::from_vpx(vpx);
    let skin = (2.0 * PHYS_SKIN) as f32; // leave room for ball
    let mut out = Vec::new();
    for item in &vpx.gameitems {
        match item {
            // hardwired to table dimensions, but with bottom/top of surface
            GameItemEnum::Wall(w) => {
                for i in 0..8 {
                    out.push(Vertex3Ds::new(
                        if i & 1 != 0 { g.right } else { g.left },
                        if i & 2 != 0 { g.bottom } else { g.top },
                        if i & 4 != 0 { w.height_top } else { w.height_bottom },
                    ));
                }
            }
            GameItemEnum::Ramp(r) => {
                if shown_only && !r.is_visible {
                    continue;
                }
                let data = ramp(r, &materials);
                let (rgv, heights) = ramp_vertex(&data, HIT_SHAPE_DETAIL_LEVEL, true);
                let cvertex = heights.len();
                if cvertex == 0 {
                    continue;
                }
                let mut min = Vertex3Ds::new(f32::MAX, f32::MAX, f32::MAX);
                let mut max = Vertex3Ds::new(-f32::MAX, -f32::MAX, -f32::MAX);
                for (i, &h) in heights.iter().enumerate() {
                    for v in [rgv[i], rgv[cvertex * 2 - i - 1]] {
                        let p = Vertex3Ds::new(v.x, v.y, h + skin);
                        min = Vertex3Ds::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
                        max = Vertex3Ds::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
                    }
                }
                corners(min, max, &mut out);
            }
            GameItemEnum::Rubber(r) => {
                if shown_only && !r.is_visible {
                    continue;
                }
                // the table's detail level, VPX's default 10
                let curve: Vec<RenderVertex<Vertex2D>> = get_rg_vertex(&drag_points(&r.drag_points), true, 4.0);
                if curve.is_empty() {
                    continue;
                }
                let half = r.thickness as f32 * 0.5;
                let z = r.height + skin;
                let mut min = Vertex3Ds::new(f32::MAX, f32::MAX, z);
                let mut max = Vertex3Ds::new(-f32::MAX, -f32::MAX, z);
                for v in &curve {
                    min = Vertex3Ds::new(min.x.min(v.v.x - half), min.y.min(v.v.y - half), z);
                    max = Vertex3Ds::new(max.x.max(v.v.x + half), max.y.max(v.v.y + half), z);
                }
                corners(min, max, &mut out);
            }
            _ => {}
        }
    }
    out
}
