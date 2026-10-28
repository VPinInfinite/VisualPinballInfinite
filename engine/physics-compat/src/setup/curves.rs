// Ported from Visual Pinball X: src/math/MeshUtils.h (curves, RecurseSmoothLine, FlatWithAccuracy) and
// src/parts/dragpoint.h (IHaveDragPoints::GetRgVertex) (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! How VPX turns a part's control points into an outline: centripetal
//! Catmull-Rom curves through the smooth points, subdivided until each piece
//! is flat enough. Walls, triggers, rubbers and ramps all start here.

use crate::math::{Vertex2D, Vertex3Ds, cross_product};

/// A control point of a wall, trigger, rubber or ramp (VPX's `DragPoint`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragPoint {
    pub v: Vertex3Ds,
    /// The outline curves through this point instead of turning a corner.
    pub smooth: bool,
    /// The wall segment from this point to the next is a slingshot.
    pub slingshot: bool,
}

/// A point of an outline (VPX's `RenderVertex` / `RenderVertex3D`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RenderVertex<V> {
    pub v: V,
    pub smooth: bool,
    pub slingshot: bool,
    /// Whether this point was a control point on the curve.
    pub control_point: bool,
}

/// What a curve point is: a 2D or 3D vertex (VPX's `CatmullCurve<2>` / `<3>`).
pub trait CurveVertex: Copy + Default {
    type Curve: Curve<Self>;
    fn from_3d(v: &Vertex3Ds) -> Self;
    /// VPX's `FlatWithAccuracy`.
    fn flat_with_accuracy(v1: &Self, v2: &Self, v_mid: &Self, accuracy: f32) -> bool;
}

/// A curve between two control points.
pub trait Curve<V> {
    fn new(v0: &Vertex3Ds, v1: &Vertex3Ds, v2: &Vertex3Ds, v3: &Vertex3Ds) -> Self;
    fn point_at(&self, t: f32) -> V;
}

/// VPX's `InitCubicSplineCoeffs`: a cubic with given ends and end tangents.
#[inline(always)]
fn cubic_spline_coeffs(x0: f32, x1: f32, t0: f32, t1: f32) -> [f32; 4] {
    [x0, t0, -3.0 * x0 + 3.0 * x1 - 2.0 * t0 - t1, 2.0 * x0 - 2.0 * x1 + t0 + t1]
}

/// VPX's `InitNonuniformCatmullCoeffs`.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn nonuniform_catmull_coeffs(x0: f32, x1: f32, x2: f32, x3: f32, dt0: f32, dt1: f32, dt2: f32) -> [f32; 4] {
    // compute tangents when parameterized in [t1,t2]
    let mut t1 = (x1 - x0) / dt0 - (x2 - x0) / (dt0 + dt1) + (x2 - x1) / dt1;
    let mut t2 = (x2 - x1) / dt1 - (x3 - x1) / (dt1 + dt2) + (x3 - x2) / dt2;

    // rescale tangents for parametrization in [0,1]
    t1 *= dt1;
    t2 *= dt1;

    cubic_spline_coeffs(x1, x2, t1, t2)
}

/// The centripetal parametrization VPX uses, with repeated points fixed up.
fn centripetal(d0: f32, d1: f32, d2: f32) -> (f32, f32, f32) {
    let mut dt0 = d0.sqrt();
    let mut dt1 = d1.sqrt();
    let mut dt2 = d2.sqrt();

    // check for repeated control points
    if dt1 < 1e-4 {
        dt1 = 1.0;
    }
    if dt0 < 1e-4 {
        dt0 = dt1;
    }
    if dt2 < 1e-4 {
        dt2 = dt1;
    }
    (dt0, dt1, dt2)
}

#[inline(always)]
fn eval(c: &[f32; 4], t: f32, t2: f32, t3: f32) -> f32 {
    c[3] * t3 + c[2] * t2 + c[1] * t + c[0]
}

/// VPX's `CatmullCurve<2>`.
pub struct CatmullCurve2 {
    cx: [f32; 4],
    cy: [f32; 4],
}

impl Curve<Vertex2D> for CatmullCurve2 {
    fn new(v0: &Vertex3Ds, v1: &Vertex3Ds, v2: &Vertex3Ds, v3: &Vertex3Ds) -> Self {
        let (v0, v1, v2, v3) = (v0.xy(), v1.xy(), v2.xy(), v3.xy());
        let (dt0, dt1, dt2) = centripetal((v1 - v0).length(), (v2 - v1).length(), (v3 - v2).length());
        Self {
            cx: nonuniform_catmull_coeffs(v0.x, v1.x, v2.x, v3.x, dt0, dt1, dt2),
            cy: nonuniform_catmull_coeffs(v0.y, v1.y, v2.y, v3.y, dt0, dt1, dt2),
        }
    }

    fn point_at(&self, t: f32) -> Vertex2D {
        let t2 = t * t;
        let t3 = t2 * t;
        Vertex2D::new(eval(&self.cx, t, t2, t3), eval(&self.cy, t, t2, t3))
    }
}

/// VPX's `CatmullCurve<3>`.
pub struct CatmullCurve3 {
    cx: [f32; 4],
    cy: [f32; 4],
    cz: [f32; 4],
}

impl Curve<Vertex3Ds> for CatmullCurve3 {
    fn new(v0: &Vertex3Ds, v1: &Vertex3Ds, v2: &Vertex3Ds, v3: &Vertex3Ds) -> Self {
        let (dt0, dt1, dt2) = centripetal((*v1 - *v0).length(), (*v2 - *v1).length(), (*v3 - *v2).length());
        Self {
            cx: nonuniform_catmull_coeffs(v0.x, v1.x, v2.x, v3.x, dt0, dt1, dt2),
            cy: nonuniform_catmull_coeffs(v0.y, v1.y, v2.y, v3.y, dt0, dt1, dt2),
            cz: nonuniform_catmull_coeffs(v0.z, v1.z, v2.z, v3.z, dt0, dt1, dt2),
        }
    }

    fn point_at(&self, t: f32) -> Vertex3Ds {
        let t2 = t * t;
        let t3 = t2 * t;
        Vertex3Ds::new(eval(&self.cx, t, t2, t3), eval(&self.cy, t, t2, t3), eval(&self.cz, t, t2, t3))
    }
}

impl CurveVertex for Vertex2D {
    type Curve = CatmullCurve2;
    fn from_3d(v: &Vertex3Ds) -> Self {
        v.xy()
    }
    fn flat_with_accuracy(v1: &Self, v2: &Self, v_mid: &Self, accuracy: f32) -> bool {
        // compute double the signed area of the triangle (v1, vMid, v2)
        let dblarea = (v_mid.x - v1.x) * (v2.y - v1.y) - (v2.x - v1.x) * (v_mid.y - v1.y);
        dblarea * dblarea < accuracy
    }
}

impl CurveVertex for Vertex3Ds {
    type Curve = CatmullCurve3;
    fn from_3d(v: &Vertex3Ds) -> Self {
        *v
    }
    fn flat_with_accuracy(v1: &Self, v2: &Self, v_mid: &Self, accuracy: f32) -> bool {
        // compute the square of double the signed area of the triangle (v1, vMid, v2)
        let dblareasq = cross_product(&(*v_mid - *v1), &(*v2 - *v1)).length_squared();
        dblareasq < accuracy
    }
}

/// VPX's `RecurseSmoothLine`: halves the curve until each piece is flat
/// enough, adding each piece's first point.
fn recurse_smooth_line<V: CurveVertex>(
    cc: &V::Curve,
    t1: f32,
    t2: f32,
    vt1: &RenderVertex<V>,
    vt2: &RenderVertex<V>,
    vv: &mut Vec<RenderVertex<V>>,
    accuracy: f32,
) {
    let t_mid = (t1 + t2) * 0.5;
    let vmid = RenderVertex {
        v: cc.point_at(t_mid),
        smooth: true,         // Generated points must always be smooth, because they are part of the curve
        slingshot: false,     // Slingshots can't be along curves
        control_point: false, // We created this point, so it can't be a control point
    };

    if V::flat_with_accuracy(&vt1.v, &vt2.v, &vmid.v, accuracy) {
        // Add first segment point to array.
        // Last point never gets added by this recursive loop,
        // but that's where it wraps around to the next curve.
        vv.push(*vt1);
    } else {
        recurse_smooth_line(cc, t1, t_mid, vt1, &vmid, vv, accuracy);
        recurse_smooth_line(cc, t_mid, t2, &vmid, vt2, vv, accuracy);
    }
}

/// VPX's `IHaveDragPoints::GetRgVertex`: the outline through the control
/// points, closed if `looped`. `accuracy` 4 is the finest VPX allows.
pub fn get_rg_vertex<V: CurveVertex>(points: &[DragPoint], looped: bool, accuracy: f32) -> Vec<RenderVertex<V>> {
    let mut vv = Vec::new();
    let cpoint = points.len() as i32;
    let endpoint = if looped { cpoint } else { cpoint - 1 };

    let mut rendv2 = RenderVertex::<V>::default();

    for i in 0..endpoint {
        let pdp1 = &points[i as usize];
        let pdp2 = &points[if i < cpoint - 1 { (i + 1) as usize } else { 0 }];

        if pdp1.v.x == pdp2.v.x && pdp1.v.y == pdp2.v.y && pdp1.v.z == pdp2.v.z {
            // Special case - two points coincide
            continue;
        }

        let mut iprev = if pdp1.smooth { i - 1 } else { i };
        if iprev < 0 {
            iprev = if looped { cpoint - 1 } else { 0 };
        }

        let mut inext = if pdp2.smooth { i + 2 } else { i + 1 };
        if inext >= cpoint {
            inext = if looped { inext - cpoint } else { cpoint - 1 };
        }

        let pdp0 = &points[iprev as usize];
        let pdp3 = &points[inext as usize];

        let cc = V::Curve::new(&pdp0.v, &pdp1.v, &pdp2.v, &pdp3.v);

        let rendv1 = RenderVertex { v: V::from_3d(&pdp1.v), smooth: pdp1.smooth, slingshot: pdp1.slingshot, control_point: true };

        // Properties of last point don't matter, because it won't be added to the list on this pass (it'll get added as the first point of the next curve)
        rendv2.v = V::from_3d(&pdp2.v);

        recurse_smooth_line(&cc, 0.0, 1.0, &rendv1, &rendv2, &mut vv, accuracy);
    }

    if !looped {
        // Add the very last point to the list because nobody else added it
        rendv2.smooth = true;
        rendv2.slingshot = false;
        rendv2.control_point = false;
        vv.push(rendv2);
    }
    vv
}
