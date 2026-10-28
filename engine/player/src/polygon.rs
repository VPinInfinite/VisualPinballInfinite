// Ported from Visual Pinball X: src/math/MeshUtils.h (PolygonToTriangles, AdvancePoint,
// FLinesIntersect, GetDot, FindCornerVertex, DetermineWindingOrder) (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! How VPX fills an outline with triangles: it walks around the polygon
//! cutting off one corner at a time, each cut a line that stays inside the
//! shape. Lights drawn on the backdrop are filled this way.

type P = [f32; 2];

/// VPX's `GetDot`: which side of the line `end1`–`joint` `end2` lies on.
fn get_dot(end1: P, joint: P, end2: P) -> f32 {
    (joint[0] - end1[0]) * (joint[1] - end2[1]) - (joint[1] - end1[1]) * (joint[0] - end2[0])
}

/// VPX's `FLinesIntersect`.
fn lines_intersect(start1: P, start2: P, end1: P, end2: P) -> bool {
    let [x1, y1] = start1;
    let [x2, y2] = start2;
    let [x3, y3] = end1;
    let [x4, y4] = end2;

    let d123 = (x2 - x1) * (y3 - y1) - (x3 - x1) * (y2 - y1);
    if d123 == 0.0 {
        // p3 lies on the same line as p1 and p2
        return x3 >= x1.min(x2) && x3 <= x2.max(x1);
    }

    let d124 = (x2 - x1) * (y4 - y1) - (x4 - x1) * (y2 - y1);
    if d124 == 0.0 {
        // p4 lies on the same line as p1 and p2
        return x4 >= x1.min(x2) && x4 <= x2.max(x1);
    }

    if d123 * d124 >= 0.0 {
        return false;
    }

    let d341 = (x3 - x1) * (y4 - y1) - (x4 - x1) * (y3 - y1);
    if d341 == 0.0 {
        // p1 lies on the same line as p3 and p4
        return x1 >= x3.min(x4) && x1 <= x3.max(x4);
    }

    let d342 = d123 - d124 + d341;
    if d342 == 0.0 {
        // p1 lies on the same line as p3 and p4
        return x2 >= x3.min(x4) && x2 <= x3.max(x4);
    }

    d341 * d342 < 0.0
}

/// VPX's `AdvancePoint`: whether the corner at `b` can be cut off by the
/// line `a`–`c`.
fn advance_point(rgv: &[P], pvpoly: &[u32], a: u32, b: u32, c: u32, pre: u32, post: u32) -> bool {
    let (pv1, pv2, pv3) = (rgv[a as usize], rgv[b as usize], rgv[c as usize]);
    let (pv_pre, pv_post) = (rgv[pre as usize], rgv[post as usize]);

    if get_dot(pv1, pv2, pv3) < 0.0
        // Make sure angle created by new triangle line falls inside existing angles
        // If the existing angle is a concave angle, then new angle must be smaller,
        // because our triangle can't have angles greater than 180
        || (get_dot(pv_pre, pv1, pv2) > 0.0 && get_dot(pv_pre, pv1, pv3) < 0.0) // convex angle, make sure new angle is smaller than it
        || (get_dot(pv2, pv3, pv_post) > 0.0 && get_dot(pv1, pv3, pv_post) < 0.0)
    {
        return false;
    }

    // Now make sure the interior segment of this triangle (line ac) does not
    // intersect the polygon anywhere

    // sort our static line segment
    let minx = pv1[0].min(pv3[0]);
    let maxx = pv1[0].max(pv3[0]);
    let miny = pv1[1].min(pv3[1]);
    let maxy = pv1[1].max(pv3[1]);

    for i in 0..pvpoly.len() {
        let i1 = pvpoly[i];
        let i2 = pvpoly[if i < pvpoly.len() - 1 { i + 1 } else { 0 }];
        let (cross1, cross2) = (rgv[i1 as usize], rgv[i2 as usize]);
        // VPX compares the last bound with the second point's y, not its x
        if i1 != a
            && i2 != a
            && i1 != c
            && i2 != c
            && (cross1[1] >= miny || cross2[1] >= miny)
            && (cross1[1] <= maxy || cross2[1] <= maxy)
            && (cross1[0] >= minx || cross2[0] >= minx)
            && (cross1[0] <= maxx || cross2[1] <= maxx)
            && lines_intersect(pv1, pv3, cross1, cross2)
        {
            return false;
        }
    }

    true
}

/// VPX's `FindCornerVertex`: the point with the smallest y (then x).
fn find_corner_vertex(vertices: &[P]) -> usize {
    let mut min_vertex = usize::MAX;
    let mut min_y = f32::MAX;
    let mut min_x_at_min_y = f32::MAX;
    for (i, v) in vertices.iter().enumerate() {
        let (x, y) = (v[0], v[1]);
        if y > min_y {
            continue;
        }
        if y == min_y && x >= min_x_at_min_y {
            continue;
        }
        // Minimum so far.
        min_vertex = i;
        min_y = y;
        min_x_at_min_y = x;
    }
    min_vertex
}

/// VPX's `DetermineWindingOrder`: true for clockwise.
fn is_clockwise(vertices: &[P]) -> bool {
    let mut n = vertices.len() as i64;
    // If vertices duplicates first as last to represent closed polygon, skip last.
    let last = vertices[vertices.len() - 1];
    if last[0] == vertices[0][0] && last[1] == vertices[0][1] {
        n -= 1;
    }
    let i_min = find_corner_vertex(vertices) as i64;
    let wrap = |i: i64| ((i + n) % n) as usize;
    let a = vertices[wrap(i_min - 1)];
    let b = vertices[i_min as usize];
    let c = vertices[wrap(i_min + 1)];
    let det = (b[0] * c[1] + a[0] * b[1] + a[1] * c[0]) - (a[1] * b[0] + b[1] * c[0] + a[0] * c[1]);
    det > 0.0
}

/// VPX's `PolygonToTriangles` over all of `rgv`: three indices per
/// triangle. A self-intersecting outline gets fewer triangles than it has
/// points less two, as in VPX.
pub fn polygon_to_triangles(rgv: &[P], support_both_winding_orders: bool) -> Vec<u32> {
    let mut tri = Vec::new();
    if rgv.len() < 3 {
        return tri;
    }
    let mut pvpoly: Vec<u32> = (0..rgv.len() as u32).collect();
    // There should be this many convex triangles.
    // If not, the polygon is self-intersecting
    let tricount = pvpoly.len() - 2;

    // check if the polygon is in right orientation, otherwise flip it over
    if support_both_winding_orders && is_clockwise(rgv) {
        pvpoly.reverse();
    }

    for _ in 0..tricount {
        for i in 0..pvpoly.len() {
            let s = pvpoly.len();
            let pre = pvpoly[if i == 0 { s - 1 } else { i - 1 }];
            let a = pvpoly[i];
            let b = pvpoly[if i + 1 < s { i + 1 } else { 0 }];
            let c = pvpoly[if i + 2 < s { i + 2 } else { i + 2 - s }];
            let post = pvpoly[if i + 3 < s { i + 3 } else { i + 3 - s }];
            if advance_point(rgv, &pvpoly, a, b, c, pre, post) {
                tri.extend_from_slice(&[a, c, b]);
                pvpoly.remove(if i + 1 < s { i + 1 } else { 0 }); // b
                break;
            }
        }
    }
    tri
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_either_way_round() {
        let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let t = polygon_to_triangles(&square, true);
        assert_eq!(t.len(), 6);
        let mut back = square;
        back.reverse();
        assert_eq!(polygon_to_triangles(&back, true).len(), 6);
    }

    #[test]
    fn concave_outline_stays_inside() {
        // an L: the cut across the inner corner must not be taken
        let l = [[0.0, 0.0], [20.0, 0.0], [20.0, 10.0], [10.0, 10.0], [10.0, 20.0], [0.0, 20.0]];
        let t = polygon_to_triangles(&l, true);
        assert_eq!(t.len(), 12);
        // the triangles cover the L's area (300) exactly
        let area: f32 = t
            .chunks(3)
            .map(|k| {
                let (a, b, c) = (l[k[0] as usize], l[k[1] as usize], l[k[2] as usize]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() * 0.5
            })
            .sum();
        assert_eq!(area, 300.0);
    }
}
