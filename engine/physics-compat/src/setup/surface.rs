// Ported from Visual Pinball X: src/parts/surface.cpp (license:GPLv3+): Surface::PhysicSetup,
// SetupHitObject and AddLine, commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A wall's colliders: for each segment of its outline, the side (a plain
//! segment, or a slingshot), its top and bottom edges, the vertical joint at
//! its start and the corner points; then the top, and the bottom if solid.

use super::curves::{DragPoint, RenderVertex, get_rg_vertex};
use super::{Colliders, PhysicsMaterial};
use crate::collide::{HitLineZ, HitObjectBase, HitPoint, LineSeg};
use crate::collideex::{Hit3DPoly, HitLine3D, LineSegSlingshot, SlingshotData};
use crate::engine::{Collider, Shape};
use crate::math::def::ang_to_rad;
use crate::math::{Vertex2D, Vertex3Ds};

/// A wall's physics settings (VPX's `SurfaceData`).
#[derive(Clone, Debug, PartialEq)]
pub struct WallData {
    pub points: Vec<DragPoint>,
    pub height_bottom: f32,
    pub height_top: f32,
    /// The wall's bottom is a collider too (`m_isBottomSolid`).
    pub is_bottom_solid: bool,
    pub collidable: bool,
    pub hit_event: bool,
    pub threshold: f32,
    pub droppable: bool,
    pub slingshot_force: f32,
    pub slingshot_threshold: f32,
    /// The material, resolved as `SetupHitObject` does.
    pub material: PhysicsMaterial,
}

/// VPX's `Surface::SetupHitObject`: the wall's material and switches on one collider.
fn setup_hit_object(w: &WallData, out: &mut Colliders, mut base: HitObjectBase, shape: Shape) -> usize {
    base.elasticity = w.material.elasticity;
    base.elasticity_falloff = w.material.elasticity_falloff;
    base.friction = w.material.friction;
    base.scatter = ang_to_rad(w.material.scatter_deg);

    base.enabled = w.collidable;

    if w.hit_event {
        base.obj = Some(out.part);
        base.fe = true;
        base.threshold = w.threshold;
    } else {
        // watched for achievements where they count it
        base.quiet = Some((out.part, w.threshold));
    }

    out.out.push(Collider { base, shape, owner: out.owner() });
    out.out.len() - 1
}

/// VPX's `Surface::AddLine`: one segment of the outline and what goes with it.
fn add_line(w: &WallData, out: &mut Colliders, pv1: &RenderVertex<Vertex2D>, pv2: &RenderVertex<Vertex2D>) {
    let bottom = w.height_bottom;
    let top = w.height_top;

    let base = HitObjectBase::with_z(bottom, top);
    let shape = if !pv1.slingshot {
        Shape::LineSeg(LineSeg::new(pv1.v, pv2.v))
    } else {
        let data = SlingshotData { threshold: w.slingshot_threshold, disabled: false };
        Shape::Slingshot(LineSegSlingshot::new(out.part, data, w.slingshot_force, pv1.v, pv2.v))
    };

    let k = setup_hit_object(w, out, base, shape);

    if pv1.slingshot {
        // slingshots always have hit events
        let c = &mut out.out[k];
        c.base.obj = Some(out.part);
        c.base.fe = true;
        c.base.threshold = w.threshold;
    }

    // add lower edge as a line
    if w.height_bottom != 0.0 {
        let mut b = HitObjectBase::default();
        let line = HitLine3D::new(Vertex3Ds::new(pv1.v.x, pv1.v.y, bottom), Vertex3Ds::new(pv2.v.x, pv2.v.y, bottom), &mut b);
        setup_hit_object(w, out, b, Shape::Line3D(line));
    }

    // add upper edge as a line
    {
        let mut b = HitObjectBase::default();
        let line = HitLine3D::new(Vertex3Ds::new(pv1.v.x, pv1.v.y, top), Vertex3Ds::new(pv2.v.x, pv2.v.y, top), &mut b);
        setup_hit_object(w, out, b, Shape::Line3D(line));
    }

    // create vertical joint between the two line segments
    setup_hit_object(w, out, HitObjectBase::default(), Shape::LineZ(HitLineZ::new(pv1.v, bottom, top)));

    // add upper and lower end points of line
    if w.height_bottom != 0.0 {
        setup_hit_object(w, out, HitObjectBase::default(), Shape::Point(HitPoint::new(Vertex3Ds::new(pv1.v.x, pv1.v.y, bottom))));
    }
    setup_hit_object(w, out, HitObjectBase::default(), Shape::Point(HitPoint::new(Vertex3Ds::new(pv1.v.x, pv1.v.y, top))));
}

/// VPX's `Surface::PhysicSetup` for play.
pub fn wall_colliders(w: &WallData, out: &mut Colliders) {
    let vvertex = get_rg_vertex::<Vertex2D>(&w.points, true, 4.0);

    let count = vvertex.len();
    let mut rgv3dt = Vec::with_capacity(count);
    let mut rgv3db = vec![Vertex3Ds::default(); if w.is_bottom_solid { count } else { 0 }];

    let bottom = w.height_bottom;
    let top = w.height_top;

    for i in 0..count {
        let pv1 = &vvertex[i];

        rgv3dt.push(Vertex3Ds::new(pv1.v.x, pv1.v.y, top));

        if w.is_bottom_solid {
            rgv3db[count - 1 - i] = Vertex3Ds::new(pv1.v.x, pv1.v.y, bottom);
        }

        let pv2 = &vvertex[(i + 1) % count];
        let pv3 = &vvertex[(i + 2) % count];

        add_line(w, out, pv2, pv3);
    }

    let mut b = HitObjectBase::default();
    let top_poly = Hit3DPoly::new(rgv3dt, &mut b);
    setup_hit_object(w, out, b, Shape::Poly3D(top_poly));

    if w.is_bottom_solid {
        let mut b = HitObjectBase::default();
        let bottom_poly = Hit3DPoly::new(rgv3db, &mut b);
        setup_hit_object(w, out, b, Shape::Poly3D(bottom_poly));
    }
}
