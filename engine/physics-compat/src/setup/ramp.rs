// Ported from Visual Pinball X: src/parts/ramp.cpp and ramp.h (license:GPLv3+): GetCentralCurve,
// GetRampVertex, GetSurfaceHeight, PhysicSetup and its helpers; and ClosestPointOnPolygon from
// src/math/MeshUtils.h. Commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A ramp's colliders: its side walls as stacked segments with joints, its
//! floor as triangles with joints where they bend, and an outside bottom. And
//! the ramp's height at a point, for parts standing on it.

use super::Colliders;
use super::curves::{DragPoint, RenderVertex, get_rg_vertex};
use crate::collide::{HitLineZ, HitObjectBase, LineSeg, ObjType};
use crate::collideex::{HitLine3D, HitTriangle};
use crate::engine::{Collider, Shape};
use crate::math::def::{ang_to_rad, max, min};
use crate::math::{Vertex2D, Vertex3Ds, cross_product};
use crate::physconst::{HIT_SHAPE_DETAIL_LEVEL, PHYS_SKIN};

/// VPX's `RampType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RampType {
    Flat = 0,
    FourWire = 1,
    TwoWire = 2,
    ThreeWireLeft = 3,
    ThreeWireRight = 4,
    OneWire = 5,
}

/// A ramp's settings (VPX's `RampData`).
#[derive(Clone, Debug, PartialEq)]
pub struct RampData {
    pub points: Vec<DragPoint>,
    pub height_bottom: f32,
    pub height_top: f32,
    pub width_bottom: f32,
    pub width_top: f32,
    pub ramp_type: RampType,
    pub left_wall_height: f32,
    pub right_wall_height: f32,
    pub wire_diameter: f32,
    pub wire_distance_x: f32,
    pub collidable: bool,
    pub hit_event: bool,
    pub threshold: f32,
    /// The physics material, resolved as `SetupHitObject` does (a ramp has no
    /// elasticity falloff).
    pub elasticity: f32,
    pub friction: f32,
    /// In degrees.
    pub scatter: f32,
}

impl RampData {
    /// VPX's `IsHabitrail`: a wire ramp.
    pub fn is_habitrail(&self) -> bool {
        self.ramp_type != RampType::Flat
    }
}

/// VPX's `GetCentralCurve`: the ramp's centre line, with heights, at a detail
/// level from 0 (coarse) to 10 (finest).
pub fn central_curve(points: &[DragPoint], detail_level: f32) -> Vec<RenderVertex<Vertex3Ds>> {
    // min = 4 (highest accuracy/detail level), max = 4 * 10^(10/1.5) = ~18.000.000 (lowest accuracy/detail level)
    let accuracy = 4.0 * 10.0_f32.powf((10.0 - detail_level) * (1.0_f64 / 1.5) as f32);
    get_rg_vertex::<Vertex3Ds>(points, false, accuracy)
}

/// VPX's `GetRampVertex`: the ramp's outline, first forward along its right
/// side, then back along its left (2 per centre point), and its height at
/// each centre point.
pub fn ramp_vertex(r: &RampData, detail_level: f32, inc_width: bool) -> (Vec<Vertex2D>, Vec<f32>) {
    let v = ramp_vertex_full(r, detail_level, inc_width);
    (v.outline, v.heights)
}

/// `GetRampVertex`'s answers, the ones drawing uses too.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RampVertex {
    /// Forward along the right side, then back along the left.
    pub outline: Vec<Vertex2D>,
    /// The height at each centre point.
    pub heights: Vec<f32>,
    /// How far along the ramp each centre point lies, from 1 at the start to 0 at the end
    /// (`ppratio`, a picture's place along a wrapped ramp).
    pub ratios: Vec<f32>,
    /// Each centre point moved by its normal (`pMiddlePoints`, a one-wire ramp's wire).
    pub middle: Vec<Vertex2D>,
}

/// VPX's `GetRampVertex` with all its answers.
pub fn ramp_vertex_full(r: &RampData, detail_level: f32, inc_width: bool) -> RampVertex {
    let vvertex = central_curve(&r.points, detail_level);
    // vvertex are the 2D vertices forming the central curve of the ramp as seen from above

    let cvertex = vvertex.len();
    let mut rgv_local = vec![Vertex2D::default(); cvertex * 2];
    let mut heights = vec![0.0f32; cvertex];
    let mut ratios = vec![0.0f32; cvertex];
    let mut middle = vec![Vertex2D::default(); cvertex];

    // Compute an approximation to the length of the central curve
    // by adding up the lengths of the line segments.
    let mut totallength = 0.0f32;
    let bottom_height = r.height_bottom;
    let top_height = r.height_top;

    for i in 0..cvertex.saturating_sub(1) {
        let v1 = &vvertex[i].v;
        let v2 = &vvertex[i + 1].v;

        let dx = v1.x - v2.x;
        let dy = v1.y - v2.y;
        let length = (dx * dx + dy * dy).sqrt();

        totallength += length;
    }

    let mut currentlength = 0.0f32;
    for i in 0..cvertex {
        // clamp next and prev as ramps do not loop
        let vprev = &vvertex[if i > 0 { i - 1 } else { i }].v;
        let vnext = &vvertex[if i < cvertex - 1 { i + 1 } else { i }].v;
        let vmiddle = &vvertex[i].v;

        let vnormal;
        {
            // Get normal at this point
            // Notice that these values equal the ones in the line
            // equation and could probably be substituted by them.
            let mut v1normal = Vertex2D::new(vprev.y - vmiddle.y, vmiddle.x - vprev.x); // vector vmiddle-vprev rotated RIGHT
            let mut v2normal = Vertex2D::new(vmiddle.y - vnext.y, vnext.x - vmiddle.x); // vector vnext-vmiddle rotated RIGHT

            // special handling for beginning and end of the ramp, as ramps do not loop
            if i == cvertex - 1 {
                v1normal.normalize();
                vnormal = v1normal;
            } else if i == 0 {
                v2normal.normalize();
                vnormal = v2normal;
            } else {
                v1normal.normalize();
                v2normal.normalize();

                if (v1normal.x - v2normal.x).abs() < 0.0001 && (v1normal.y - v2normal.y).abs() < 0.0001 {
                    // Two parallel segments
                    vnormal = v1normal;
                } else {
                    // Find intersection of the two edges meeting this points, but
                    // shift those lines outwards along their normals

                    // First line
                    let a = vprev.y - vmiddle.y;
                    let b = vmiddle.x - vprev.x;

                    // Shift line along the normal
                    let c = a * (v1normal.x - vprev.x) + b * (v1normal.y - vprev.y);

                    // Second line
                    let d = vnext.y - vmiddle.y;
                    let e = vmiddle.x - vnext.x;

                    // Shift line along the normal
                    let f = d * (v2normal.x - vnext.x) + e * (v2normal.y - vnext.y);

                    let det = a * e - b * d;
                    let inv_det = if det != 0.0 { 1.0 / det } else { 0.0 };

                    let intersectx = (b * f - e * c) * inv_det;
                    let intersecty = (c * d - a * f) * inv_det;

                    vnormal = Vertex2D::new(vmiddle.x - intersectx, vmiddle.y - intersecty);
                }
            }
        }

        // Update current length along the ramp.
        {
            let dx = vprev.x - vmiddle.x;
            let dy = vprev.y - vmiddle.y;
            let length = (dx * dx + dy * dy).sqrt();

            currentlength += length;
        }

        let percentage = currentlength / totallength;
        let mut widthcur = percentage * (r.width_top - r.width_bottom) + r.width_bottom;
        heights[i] = vmiddle.z + percentage * (top_height - bottom_height) + bottom_height;
        ratios[i] = 1.0 - percentage;

        // only change the width if we want to create vertices for rendering or for the editor
        // the collision engine uses flat type ramps
        if r.is_habitrail() && r.ramp_type != RampType::OneWire {
            widthcur = r.wire_distance_x;
            if inc_width {
                widthcur += 20.0;
            }
        } else if r.ramp_type == RampType::OneWire {
            widthcur = r.wire_diameter;
        }

        let mid = Vertex2D::new(vmiddle.x, vmiddle.y);
        middle[i] = mid + vnormal;
        rgv_local[i] = mid + (widthcur * 0.5) * vnormal;
        rgv_local[cvertex * 2 - i - 1] = mid - (widthcur * 0.5) * vnormal;
    }

    RampVertex { outline: rgv_local, heights, ratios, middle }
}

/// VPX's `ClosestPointOnPolygon`, for an open polyline: the closest point and
/// the segment it lies on, if any.
fn closest_point_on_polyline(rgv: &[RenderVertex<Vertex3Ds>], pvin: Vertex2D) -> Option<(Vertex2D, usize)> {
    let count = rgv.len();
    let mut mindist = f32::MAX;
    let mut found = None;

    // Don't check segment running from the end point to the beginning point
    let cloop = count.saturating_sub(1);

    // Go through line segment, calculate distance from point to the line
    // then pick the shortest distance
    for i in 0..cloop {
        let p2 = if i < count - 1 { i + 1 } else { 0 };

        let rgvi = rgv[i].v.xy();
        let rgvp2 = rgv[p2].v.xy();
        let a = rgvi.y - rgvp2.y;
        let b = rgvp2.x - rgvi.x;
        let c = -(a * rgvi.x + b * rgvi.y);

        let dist = (a * pvin.x + b * pvin.y + c).abs() / (a * a + b * b).sqrt();

        if dist < mindist {
            // Assuming we got a segment that we are closet to, calculate the intersection
            // of the line with the perpenticular line projected from the point,
            // to find the closest point on the line
            let d = -b;
            let f = -(d * pvin.x + a * pvin.y);

            let det = a * a - b * d;
            let inv_det = if det != 0.0 { 1.0 / det } else { 0.0 };
            let intersectx = (b * f - a * c) * inv_det;
            let intersecty = (c * d - a * f) * inv_det;

            // If the intersect point lies on the polygon segment
            // (not out in space), then make this the closest known point
            if intersectx >= (min(rgvi.x, rgvp2.x) - 0.1)
                && intersectx <= (max(rgvi.x, rgvp2.x) + 0.1)
                && intersecty >= (min(rgvi.y, rgvp2.y) - 0.1)
                && intersecty <= (max(rgvi.y, rgvp2.y) + 0.1)
            {
                mindist = dist;
                found = Some((Vertex2D::new(intersectx, intersecty), i));
            }
        }
    }
    found
}

/// VPX's `Ramp::GetSurfaceHeight`: the height of the ramp's surface at (x, y),
/// for a part standing on it. VPX computes the centre line at the detail level
/// it renders the ramp with (10 for an opaque material, else the player's
/// detail setting), not the physics' level.
pub fn ramp_surface_height(r: &RampData, x: f32, y: f32, detail_level: f32) -> f32 {
    let vvertex = central_curve(&r.points, detail_level);

    let Some((v_out, i_seg)) = closest_point_on_polyline(&vvertex, Vertex2D::new(x, y)) else {
        return 0.0; // Object is not on ramp path
    };

    // Go through vertices (including iSeg itself) counting control points until iSeg
    let mut totallength = 0.0f32;
    let mut startlength = 0.0f32;

    let cvertex = vvertex.len();
    for i2 in 1..cvertex {
        let dx = vvertex[i2].v.x - vvertex[i2 - 1].v.x;
        let dy = vvertex[i2].v.y - vvertex[i2 - 1].v.y;
        let len = (dx * dx + dy * dy).sqrt();
        if i2 <= i_seg {
            startlength += len;
        }

        totallength += len;
    }

    let dx = v_out.x - vvertex[i_seg].v.x;
    let dy = v_out.y - vvertex[i_seg].v.y;
    let len = (dx * dx + dy * dy).sqrt();
    startlength += len; // Add the distance the object is between the two closest polyline segments.  Matters mostly for straight edges. Z does not respect that yet!

    let top_height = r.height_top;
    let bottom_height = r.height_bottom;

    vvertex[i_seg].v.z + (startlength / totallength) * (top_height - bottom_height) + bottom_height
}

/// VPX's `Ramp::SetupHitObject`: the ramp's material and switches; ramps
/// count as primitives for hit events.
fn setup_hit_object(r: &RampData, out: &mut Colliders, mut base: HitObjectBase, shape: Shape) -> Option<HitTriangle> {
    base.elasticity = r.elasticity;
    base.friction = r.friction;
    base.scatter = ang_to_rad(r.scatter);

    base.threshold = r.threshold;
    // the ramp is of type ePrimitive for triggering the event in HitTriangle::Collide()
    base.obj_type = ObjType::Primitive;
    base.enabled = r.collidable;
    base.obj = Some(out.part);
    base.fe = r.hit_event;
    base.quiet = (!r.hit_event).then_some((out.part, r.threshold));

    let tri = if let Shape::Triangle(t) = &shape { Some(*t) } else { None };
    out.out.push(Collider { base, shape, owner: out.owner() });
    tri
}

fn add_joint(r: &RampData, out: &mut Colliders, v1: Vertex3Ds, v2: Vertex3Ds) {
    let mut b = HitObjectBase::default();
    let line = HitLine3D::new(v1, v2, &mut b);
    setup_hit_object(r, out, b, Shape::Line3D(line));
}

fn add_joint_2d(r: &RampData, out: &mut Colliders, p: Vertex2D, zlow: f32, zhigh: f32) {
    setup_hit_object(r, out, HitObjectBase::default(), Shape::LineZ(HitLineZ::new(p, zlow, zhigh)));
}

/// VPX's `Ramp::CheckJoint`: a joint along the edge two floor triangles
/// share, unless they lie in one plane.
fn check_joint(r: &RampData, out: &mut Colliders, ph3d1: Option<&HitTriangle>, ph3d2: &HitTriangle) {
    if let Some(t1) = ph3d1 {
        // may be null in case of degenerate triangles
        let vjointnormal = cross_product(&t1.normal, &ph3d2.normal);
        if vjointnormal.length_squared() < 1e-8 {
            return; // coplanar triangles need no joints
        }
    }

    // By convention of the calling function, points 1 [0] and 2 [1] of the second polygon will
    // be the common-edge points
    add_joint(r, out, ph3d2.rgv[0], ph3d2.rgv[1]);
}

/// VPX's `Ramp::AddWallLineSeg`: a side wall segment, split while it climbs
/// more than a ball's height (the segments are flat-topped).
#[allow(clippy::too_many_arguments)]
fn add_wall_line_seg(
    r: &RampData,
    out: &mut Colliders,
    pv1: Vertex2D,
    pv2: Vertex2D,
    pv3_exists: bool,
    height1: f32,
    height2: f32,
    wallheight: f32,
) {
    // Hit-walls are still done via 2D line segments with only a single lower and upper border, so the
    // wall will always reach below and above the actual ramp -between- two points of the ramp. Thus,
    // subdivide until at some point the approximation error is 'subtle' enough so that one will
    // usually not notice (i.e. dependent on ball size)
    if height2 - height1 > (2.0 * PHYS_SKIN) as f32 {
        // !! use ballsize
        let mid = (pv1 + pv2) * 0.5;
        add_wall_line_seg(r, out, pv1, mid, pv3_exists, height1, (height1 + height2) * 0.5, wallheight);
        add_wall_line_seg(r, out, mid, pv2, true, (height1 + height2) * 0.5, height2, wallheight);
    } else {
        let base = HitObjectBase::with_z(height1, height2 + wallheight);
        setup_hit_object(r, out, base, Shape::LineSeg(LineSeg::new(pv1, pv2)));

        if pv3_exists {
            add_joint_2d(r, out, pv1, height1, height2 + wallheight);
        }
    }
}

/// A floor triangle, unless degenerate (VPX deletes those).
fn floor_triangle(rgv: [Vertex3Ds; 3]) -> Option<(HitTriangle, HitObjectBase)> {
    let mut b = HitObjectBase::default();
    let t = HitTriangle::new(rgv, &mut b);
    if t.is_degenerate() { None } else { Some((t, b)) }
}

/// VPX's `Ramp::PhysicSetup` for play.
pub fn ramp_colliders(r: &RampData, out: &mut Colliders) {
    let (rgv_local, rgheight1) = ramp_vertex(r, HIT_SHAPE_DETAIL_LEVEL, true);
    let cvertex = rgheight1.len();

    let (wallheightright, wallheightleft) = match r.ramp_type {
        RampType::Flat => (r.right_wall_height, r.left_wall_height),
        // backwards compatible physics
        RampType::OneWire | RampType::TwoWire => (31.0, 31.0),
        RampType::FourWire => (62.0, 62.0),
        RampType::ThreeWireRight => (62.0, (6.0 + 12.5_f64) as f32),
        RampType::ThreeWireLeft => ((6.0 + 12.5_f64) as f32, 62.0),
    };

    // Add line segments for right ramp wall.
    if wallheightright > 0.0 {
        for i in 0..cvertex.saturating_sub(1) {
            let pv2 = rgv_local[i];
            let pv3 = rgv_local[i + 1];

            add_wall_line_seg(r, out, pv2, pv3, i > 0, rgheight1[i], rgheight1[i + 1], wallheightright);
            add_wall_line_seg(r, out, pv3, pv2, i < cvertex - 2, rgheight1[i], rgheight1[i + 1], wallheightright);

            // add joints at start and end of right wall
            if i == 0 {
                add_joint_2d(r, out, pv2, rgheight1[0], rgheight1[0] + wallheightright);
            }
            if i == cvertex - 2 {
                add_joint_2d(r, out, pv3, rgheight1[cvertex - 1], rgheight1[cvertex - 1] + wallheightright);
            }
        }
    }

    // Add line segments for left ramp wall.
    if wallheightleft > 0.0 {
        for i in 0..cvertex.saturating_sub(1) {
            let pv2 = rgv_local[cvertex + i];
            let pv3 = rgv_local[cvertex + i + 1];

            add_wall_line_seg(r, out, pv2, pv3, i > 0, rgheight1[cvertex - i - 2], rgheight1[cvertex - i - 1], wallheightleft);
            add_wall_line_seg(r, out, pv3, pv2, i < cvertex - 2, rgheight1[cvertex - i - 2], rgheight1[cvertex - i - 1], wallheightleft);

            // add joints at start and end of left wall
            if i == 0 {
                add_joint_2d(r, out, pv2, rgheight1[cvertex - 1], rgheight1[cvertex - 1] + wallheightleft);
            }
            if i == cvertex - 2 {
                add_joint_2d(r, out, pv3, rgheight1[0], rgheight1[0] + wallheightleft);
            }
        }
    }

    // Add hit triangles for the ramp floor.
    {
        let mut ph3dpoly_old: Option<HitTriangle> = None;
        let (mut pv3, mut pv4) = (Vertex2D::default(), Vertex2D::default());

        for i in 0..cvertex.saturating_sub(1) {
            // Layout of one ramp quad seen from above, ramp direction is bottom to top:
            //
            //    3 - - 4
            //    | \   |
            //    |   \ |
            //    2 - - 1
            let pv1 = rgv_local[i]; // i-th right
            let pv2 = rgv_local[cvertex * 2 - i - 1]; // i-th left
            pv3 = rgv_local[cvertex * 2 - i - 2]; // (i+1)-th left
            pv4 = rgv_local[i + 1]; // (i+1)-th right

            {
                // left ramp floor triangle, CCW order
                let rgv3d = [
                    Vertex3Ds::new(pv2.x, pv2.y, rgheight1[i]),
                    Vertex3Ds::new(pv1.x, pv1.y, rgheight1[i]),
                    Vertex3Ds::new(pv3.x, pv3.y, rgheight1[i + 1]),
                ];

                // add joint for starting edge of ramp
                if i == 0 {
                    add_joint(r, out, rgv3d[0], rgv3d[1]);
                }

                // add joint for left edge
                add_joint(r, out, rgv3d[0], rgv3d[2]);

                // degenerate triangles happen if width is 0 at some point
                if let Some((t, b)) = floor_triangle(rgv3d) {
                    let t = setup_hit_object(r, out, b, Shape::Triangle(t)).expect("triangle");
                    check_joint(r, out, ph3dpoly_old.as_ref(), &t);
                    ph3dpoly_old = Some(t);
                }
            }

            // right ramp floor triangle, CCW order
            let rgv3d = [
                Vertex3Ds::new(pv3.x, pv3.y, rgheight1[i + 1]),
                Vertex3Ds::new(pv1.x, pv1.y, rgheight1[i]),
                Vertex3Ds::new(pv4.x, pv4.y, rgheight1[i + 1]),
            ];

            // add joint for right edge
            add_joint(r, out, rgv3d[1], rgv3d[2]);

            if let Some((t, b)) = floor_triangle(rgv3d) {
                let t = setup_hit_object(r, out, b, Shape::Triangle(t)).expect("triangle");
                check_joint(r, out, ph3dpoly_old.as_ref(), &t);
                ph3dpoly_old = Some(t);
            }
        }

        if cvertex >= 2 {
            // add joint for final edge of ramp
            let v1 = Vertex3Ds::new(pv4.x, pv4.y, rgheight1[cvertex - 1]);
            let v2 = Vertex3Ds::new(pv3.x, pv3.y, rgheight1[cvertex - 1]);
            add_joint(r, out, v1, v2);
        }
    }

    // add outside bottom,
    // joints at the intersections are not needed since the inner surface has them
    // this surface is identical... except for the direction of the normal face.
    // hence the joints protect both surface edges from having a fall through

    for i in 0..cvertex.saturating_sub(1) {
        // see sketch above
        let pv1 = rgv_local[i];
        let pv2 = rgv_local[cvertex * 2 - i - 1];
        let pv3 = rgv_local[cvertex * 2 - i - 2];
        let pv4 = rgv_local[i + 1];

        {
            // left ramp triangle, order CW
            let rgv3d = [
                Vertex3Ds::new(pv1.x, pv1.y, rgheight1[i]),
                Vertex3Ds::new(pv2.x, pv2.y, rgheight1[i]),
                Vertex3Ds::new(pv3.x, pv3.y, rgheight1[i + 1]),
            ];

            if let Some((t, b)) = floor_triangle(rgv3d) {
                setup_hit_object(r, out, b, Shape::Triangle(t));
            }
        }

        // right ramp triangle, order CW
        let rgv3d = [
            Vertex3Ds::new(pv3.x, pv3.y, rgheight1[i + 1]),
            Vertex3Ds::new(pv4.x, pv4.y, rgheight1[i + 1]),
            Vertex3Ds::new(pv1.x, pv1.y, rgheight1[i]),
        ];

        if let Some((t, b)) = floor_triangle(rgv3d) {
            setup_hit_object(r, out, b, Shape::Triangle(t));
        }
    }
}
