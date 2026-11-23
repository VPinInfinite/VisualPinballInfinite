// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d):
// src/parts/plunger.cpp (Plunger::RenderSetup — the lathed tip and rod, the spring, the flat
// plunger, a frame for each position —, Render's choice of frame, nextTipToken and the modern
// plunger's description) and src/parts/plunger.h (PlungerCoord).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A plunger's shape as VPX draws it: a frame for each of its positions (25
//! for every 80 units of stroke), each the whole plunger — its tip and rod
//! turned on a lathe from a description (the modern plunger's, or a custom
//! one's from its tip shape and sizes) with its spring wound round the rod,
//! squeezed as the rod is pulled back; or, for the flat plunger, its
//! picture on one rectangle, its height following the pull. The physics'
//! position picks the frame ([`frame_of`]).

use std::f64::consts::PI;

use physics_compat::math::Vertex2D;
use table_physics::Surfaces;
use vpin::vpx::gameitem::plunger::{Plunger, PlungerType};

use super::Mesh;

/// A point of the lathe's outline: its radius (of the plunger's width), its
/// place along the plunger (the tip at 0), its picture place down the
/// picture, and the normal of the circle's top point.
#[derive(Clone, Copy, Debug, Default)]
struct Coord {
    r: f32,
    y: f32,
    tv: f32,
    nx: f32,
    ny: f32,
}

const fn coord(r: f32, y: f32, tv: f32, nx: f32, ny: f32) -> Coord {
    Coord { r, y, tv, nx, ny }
}

/// VPX's modern plunger: the tip, its ring and the shaft.
const MODERN: [Coord; 7] = [
    coord(0.20, 0.0, 0.00, 1.0, 0.0),
    coord(0.30, 3.0, 0.11, 1.0, 0.0),
    coord(0.35, 5.0, 0.14, 1.0, 0.0),
    coord(0.35, 23.0, 0.19, 1.0, 0.0),
    coord(0.45, 23.0, 0.21, 0.8, 0.0),
    coord(0.25, 24.0, 0.25, 0.3, 0.0),
    coord(0.25, 100.0, 1.00, 0.3, 0.0),
];

/// A plunger's frames, the most pulled back first, and the point VPX sorts
/// it from (`m_boundingSphereCenter`, which it hands `DrawMesh`).
pub struct Shape {
    pub frames: Vec<Mesh>,
    pub sort_point: [f32; 3],
}

/// VPX's frame for a plunger at `along` its frames' span (`Render`: the
/// position less the span's start, over the span), rounded, kept in range.
pub fn frame_of(along: f32, count: usize) -> usize {
    let f = (along * (count as f32 - 1.0) + 0.5) as i32;
    f.clamp(0, count as i32 - 1) as usize
}

/// VPX's `nextTipToken`: from `p`, the start of the next token (after
/// spaces), leaving `p` after it and the spaces that follow.
fn next_tip_token(s: &[u8], p: &mut usize) -> usize {
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    while at(*p).is_ascii_whitespace() {
        *p += 1;
    }
    let token = *p;
    while !matches!(at(*p), b';' | b',' | 0) && !at(*p).is_ascii_whitespace() {
        *p += 1;
    }
    while at(*p).is_ascii_whitespace() {
        *p += 1;
    }
    token
}

/// C's `atof` on what follows `from`: the longest number there, else 0.
fn atof(s: &[u8], from: usize) -> f64 {
    let rest = &s[from.min(s.len())..];
    let mut end = 0;
    let digits = |i: &mut usize| {
        let start = *i;
        while rest.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        *i > start
    };
    if matches!(rest.first(), Some(b'+' | b'-')) {
        end += 1;
    }
    let whole = digits(&mut end);
    let mut fraction = false;
    if rest.get(end) == Some(&b'.') {
        let mut i = end + 1;
        fraction = digits(&mut i);
        if whole || fraction {
            end = i;
        }
    }
    if !whole && !fraction {
        return 0.0;
    }
    if matches!(rest.get(end), Some(b'e' | b'E')) {
        let mut i = end + 1;
        if matches!(rest.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        if digits(&mut i) {
            end = i;
        }
    }
    std::str::from_utf8(&rest[..end]).ok().and_then(|t| t.parse().ok()).unwrap_or(0.0)
}

pub fn shape(surfaces: &Surfaces, p: &Plunger) -> Option<Shape> {
    // RenderSetup
    let zheight = surfaces.height(&p.surface, Vertex2D::new(p.center.x, p.center.y)) + p.z_adjust;
    let stroke = p.stroke;
    let beginy = p.center.y;
    let endy = p.center.y - stroke;
    let cframes = (stroke * (25.0_f64 / 80.0) as f32) as i32 + 1; // 25 frames per 80 units travel
    if cframes < 1 {
        return None;
    }
    let inv_scale = if cframes > 1 { 1.0 / (cframes - 1) as f32 } else { 0.0 };
    let dy_per_frame = (endy - beginy) * inv_scale;
    let flat = matches!(p.plunger_type, PlungerType::Flat);
    let custom = matches!(p.plunger_type, PlungerType::Custom);
    let circle_points: usize = if flat { 0 } else { 24 };
    let (mut spring_loops, mut spring_end_loops, mut spring_gauge, mut spring_radius) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    const SPRING_MIN_SPACING: f32 = 2.2;
    let mut rody = beginy + p.height;
    let z_scale = 1.0f32;
    // the cells of a flat plunger's picture
    let src_cells = p.anim_frames.max(1) as i32;
    let cell_wid = 1.0 / src_cells as f32;

    let desc: Vec<Coord> = if flat {
        Vec::new()
    } else if custom {
        // the tip from its shape: "y diameter; y diameter; ...", each y at least the last
        let tip = p.tip_shape.as_bytes();
        let n_tip = 1 + tip.iter().filter(|&&c| c == b';').count();
        // VPX leaves entries the shape doesn't fill unset: zero here
        let mut c = vec![Coord::default(); n_tip];
        let mut tiplen = 0.0f32;
        let (mut at, mut k) = (0usize, 0usize);
        while tip.get(at).is_some_and(|&b| b != 0) && k < n_tip {
            let t = next_tip_token(tip, &mut at);
            c[k].y = atof(tip, t) as f32;
            let t = next_tip_token(tip, &mut at);
            c[k].r = atof(tip, t) as f32 * 0.5;
            if c[k].y < tiplen {
                c[k].y = tiplen;
            }
            tiplen = c[k].y;
            while tip.get(at).is_some_and(|&b| b != b';' && b != 0) {
                at += 1;
            }
            if tip.get(at) == Some(&b';') {
                at += 1;
            }
            k += 1;
        }
        // the tip's picture places (its quarter of the picture) and normals, from its neighbours
        let c0 = coord(0.0, 0.0, 0.0, 0.0, 1.0);
        for i in 0..n_tip {
            let prv = if i == 0 { c0 } else { c[i - 1] };
            c[i].tv = 0.24 * c[i].y / tiplen;
            let nxt = if i + 1 < n_tip { c[i + 1] } else { c[i] };
            let yd = nxt.y - prv.y;
            let xd = (nxt.r - prv.r) * p.width;
            let r = (xd * xd + yd * yd).sqrt();
            c[i].nx = yd / r;
            c[i].ny = -xd / r;
        }
        // the tip's inner edge, the gap to the ring, the ring, the shaft
        let r_rod = p.rod_diam / 2.0;
        let mut y = tiplen;
        c.push(coord(r_rod, y, 0.24, 1.0, 0.0));
        c.push(coord(r_rod, y, 0.51, 1.0, 0.0));
        y += p.ring_gap;
        c.push(coord(r_rod, y, 0.55, 1.0, 0.0));
        let r_ring = p.ring_diam / 2.0;
        c.push(coord(r_rod, y, 0.26, 0.0, -1.0));
        c.push(coord(r_ring, y, 0.33, 0.0, -1.0));
        c.push(coord(r_ring, y, 0.33, 1.0, 0.0));
        y += p.ring_width;
        c.push(coord(r_ring, y, 0.42, 1.0, 0.0));
        c.push(coord(r_ring, y, 0.42, 0.0, 1.0));
        c.push(coord(r_rod, y, 0.49, 0.0, 1.0));
        spring_radius = p.spring_diam * 0.5;
        spring_gauge = p.spring_gauge;
        spring_loops = p.spring_loops;
        spring_end_loops = p.spring_end_loops;
        c.push(coord(r_rod, y, 0.51, 1.0, 0.0));
        // the rod's end: the tip fully pulled back, the parts that don't squeeze, the squeezed spring
        let spring_min = (spring_loops + spring_end_loops) * SPRING_MIN_SPACING;
        rody = beginy + y + spring_min;
        c.push(coord(r_rod, rody, 0.74, 1.0, 0.0));
        c
    } else {
        MODERN.to_vec()
    };
    let lathe_points = desc.len();

    let (vts_per_frame, lathe_vts, spring_indices) = if flat {
        (4, 0, 0)
    } else {
        let lathe_vts = lathe_points * circle_points;
        let spring_vts = ((spring_loops + spring_end_loops) * circle_points as f32) as i32 * 3;
        let spring_indices = if custom { (4 * spring_vts - 12).max(0) as usize } else { 0 };
        (lathe_vts + spring_vts.max(0) as usize, lathe_vts, spring_indices)
    };
    let spring_gauge_rel = spring_gauge / p.width;

    // each frame's points: frame 0 the most pulled back, the last the most forward
    let mut buf: Vec<[f32; 8]> = vec![[0.0; 8]; cframes as usize * vts_per_frame];
    for i in 0..cframes as usize {
        let ptr = &mut buf[i * vts_per_frame..(i + 1) * vts_per_frame];
        let ytip = beginy + dy_per_frame * i as f32;
        if !flat {
            // round the circle from the top, the picture's middle at the top
            let mut tu = 0.51f32;
            let step_u = 1.0 / circle_points as f32;
            for l in 0..circle_points {
                if tu > 1.0 {
                    tu -= 1.0;
                }
                let angle = ((PI * 2.0) as f32 / circle_points as f32) * l as f32;
                let (sn, cs) = (angle.sin(), angle.cos());
                let mut pmm1tv = 0.0f32;
                for (m, c) in desc.iter().enumerate() {
                    let mut y = c.y + ytip;
                    let mut tv = c.tv;
                    // the last point: the rod's end, its picture kept in place as the rod shortens
                    if m + 1 == lathe_points {
                        y = rody;
                        let ratio = i as f32 * inv_scale;
                        tv = pmm1tv + (tv - pmm1tv) * ratio;
                    }
                    ptr[l * lathe_points + m] = [
                        c.r * (sn * p.width) + p.center.x,
                        y,
                        (c.r * (cs * p.width) + p.width + zheight) * z_scale,
                        c.nx * sn,
                        c.ny,
                        -c.nx * cs,
                        tu,
                        tv,
                    ];
                    pmm1tv = tv;
                }
                tu += step_u;
            }
        }
        if flat {
            // the picture on a rectangle 1.25 widths up, its cell by the frame (the most pulled
            // back in the picture's last cell), its height the share of the stroke drawn
            let (x_lt, x_rt) = (p.center.x - p.width, p.center.x + p.width);
            let (y_top, y_bot) = (ytip, beginy + p.height);
            let z = (zheight + p.width * 1.25) * z_scale;
            let cell_idx = (src_cells - 1 - ((i as f32 * src_cells as f32 / cframes as f32) + 0.5) as i32).max(0);
            let tu_local = cell_wid * cell_idx as f32;
            let tv_local = (y_bot - y_top) / (beginy + p.height - endy);
            ptr[0] = [x_lt, y_bot, z, 0.0, 0.0, -1.0, tu_local, tv_local];
            ptr[1] = [x_lt, y_top, z, 0.0, 0.0, -1.0, tu_local, 0.0];
            ptr[2] = [x_rt, y_top, z, 0.0, 0.0, -1.0, tu_local + cell_wid, 0.0];
            ptr[3] = [x_rt, y_bot, z, 0.0, 0.0, -1.0, tu_local + cell_wid, tv_local];
        } else {
            // the spring: a wedge wound round the rod, three spirals (front, top, back), its
            // length from the second-last lathe point to the rod's true end, its ends tighter
            let offset = circle_points * lathe_points;
            let y0 = if offset >= 2 { ptr[offset - 2][1] } else { 0.0 };
            let y1 = rody;
            let mut n = ((spring_loops + spring_end_loops) * circle_points as f32) as i32;
            let n_end = (spring_end_loops * circle_points as f32) as i32;
            let n_main = n - n_end;
            let y_end = spring_end_loops * spring_gauge * SPRING_MIN_SPACING;
            let dy_main = (y1 - y0 - y_end) / (n_main - 1) as f32;
            let dy_end = y_end / (n_end - 1) as f32;
            let mut dy = dy_end;
            let mut k = offset;
            let dtheta = (PI * 2.0) as f32 / (circle_points as f32 - 1.0) + PI as f32 / (n - 1) as f32;
            let (mut theta, mut y) = (PI as f32, y0);
            while n != 0 {
                if n == n_main {
                    dy = dy_main;
                }
                if theta >= (PI * 2.0) as f32 {
                    theta -= (PI * 2.0) as f32;
                }
                let (sn, cs) = (theta.sin(), theta.cos());
                let top = spring_radius + spring_gauge_rel / 1.5;
                ptr[k] = [
                    spring_radius * (sn * p.width) + p.center.x,
                    y - spring_gauge,
                    (spring_radius * (cs * p.width) + p.width + zheight) * z_scale,
                    0.0,
                    -1.0,
                    0.0,
                    (sn + 1.0) * 0.5,
                    0.76,
                ];
                ptr[k + 1] = [top * (sn * p.width) + p.center.x, y, (top * (cs * p.width) + p.width + zheight) * z_scale, sn, 0.0, -cs, (sn + 1.0) * 0.5, 0.85];
                ptr[k + 2] = [
                    spring_radius * (sn * p.width) + p.center.x,
                    y + spring_gauge,
                    (spring_radius * (cs * p.width) + p.width + zheight) * z_scale,
                    0.0,
                    1.0,
                    0.0,
                    (sn + 1.0) * 0.5,
                    0.98,
                ];
                k += 3;
                n -= 1;
                theta += dtheta;
                y += dy;
            }
        }
    }

    // each frame's triangles (the same for every frame, within it)
    let mut indices: Vec<u32> = Vec::new();
    if flat {
        indices.extend([0, 1, 2, 2, 3, 0]);
    } else {
        for l in 0..circle_points {
            let offset = l * lathe_points;
            for m in 0..lathe_points.saturating_sub(1) {
                let at = |d: usize| ((m + offset + d) % lathe_vts) as u32;
                indices.extend([at(0), at(lathe_points), at(1 + lathe_points), at(1 + lathe_points), at(1), at(0)]);
            }
        }
        // the spring's faces wound by which half they're on, as the first frame's top spiral says
        let mut offset = lathe_vts as u32;
        let mut v = lathe_vts + 1;
        let mut l = 0;
        while l < spring_indices {
            let o = offset;
            if buf[v][5] <= 0.0 {
                indices.extend([o, o + 3, o + 1, o + 1, o + 3, o + 4, o + 4, o + 5, o + 2, o + 2, o + 1, o + 4]);
            } else {
                indices.extend([o + 3, o, o + 4, o + 4, o, o + 1, o + 1, o + 2, o + 5, o + 5, o + 1, o + 2]);
            }
            l += 12;
            offset += 3;
            v += 3;
        }
    }
    let frames = (0..cframes as usize)
        .map(|i| {
            let points = &buf[i * vts_per_frame..(i + 1) * vts_per_frame];
            Mesh {
                positions: points.iter().map(|v| [v[0], v[1], v[2]]).collect(),
                normals: points.iter().map(|v| [v[3], v[4], v[5]]).collect(),
                uvs: points.iter().map(|v| [v[6], v[7]]).collect(),
                indices: indices.clone(),
            }
        })
        .collect();
    // the middle of its travel, at its top
    let sort_point = [p.center.x, 0.5 * (p.center.y - stroke + p.center.y + p.height), z_scale * (zheight + p.width * if flat { 1.25 } else { 1.0 })];
    Some(Shape { frames, sort_point })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpin::vpx::VPX;
    use vpin::vpx::gameitem::vertex2d::Vertex2D as Point;

    #[test]
    fn a_plunger_has_25_frames_for_every_80_of_stroke_and_its_place_picks_one() {
        let surfaces = Surfaces::of_table(&VPX::default());
        let p = Plunger {
            center: Point { x: 900.0, y: 2000.0 },
            width: 25.0,
            height: 20.0,
            stroke: 160.0,
            plunger_type: PlungerType::Modern,
            ..Default::default()
        };
        let s = shape(&surfaces, &p).expect("a modern plunger");
        // 160 * 25 / 80 + 1 frames, each the 7-point lathe round 24 points
        assert_eq!(s.frames.len(), 51);
        assert!(s.frames.iter().all(|f| f.positions.len() == 7 * 24 && f.indices.len() == 6 * 24 * 6));
        // the first frame's tip at the plunger's place, the last's a stroke up the table
        let tip = |k: usize| s.frames[k].positions.iter().map(|q| q[1]).fold(f32::MAX, f32::min);
        assert_eq!((tip(0), tip(50)), (2000.0, 1840.0));
        // VPX's choice: the nearest frame, kept in range
        assert_eq!((frame_of(0.0, 51), frame_of(0.5, 51), frame_of(1.0, 51), frame_of(1.5, 51), frame_of(-1.0, 51)), (0, 25, 50, 50, 0));
    }

    #[test]
    fn a_tip_shape_reads_as_vpx_reads_it() {
        let tip = b"0 .34; 2 .6;39 .84";
        let mut at = 0;
        let a = next_tip_token(tip, &mut at);
        assert_eq!(atof(tip, a), 0.0);
        let b = next_tip_token(tip, &mut at);
        assert_eq!(atof(tip, b), 0.34);
        assert_eq!((atof(b"-1.5e1;", 0), atof(b".5", 0), atof(b"x", 0), atof(b"7.", 0)), (-15.0, 0.5, 0.0, 7.0));
    }
}
