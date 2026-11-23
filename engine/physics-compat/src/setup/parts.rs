// Ported from Visual Pinball X (license:GPLv3+), commit af26b2d (10.8.1 pre-release build 5436):
// the PhysicSetup of src/parts/bumper.cpp, gate.cpp, spinner.cpp, trigger.cpp, kicker.cpp, flipper.cpp
// and plunger.cpp.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The colliders of the parts made of one main collider: bumpers, gates,
//! spinners, triggers, kickers, flippers and plungers. `height` is always the
//! height of the surface the part stands on (VPX's `GetSurfaceHeight`).

use super::Colliders;
use super::curves::{DragPoint, get_rg_vertex};
use crate::collide::{HitCircle, HitObjectBase, LineSeg, ObjType};
use crate::collideex::{
    BumperData, BumperHitCircle, GateData, Hit3DPoly, HitGate, HitSpinner, SpinnerData, TriggerHitCircle, TriggerLineSeg,
};
use crate::engine::{Collider, Shape};
use crate::flipper::{FlipperData, HitFlipper};
use crate::kicker::{KickerData, KickerHitCircle};
use crate::math::def::{ang_to_rad, max, min};
use crate::math::{Vertex2D, Vertex3Ds};
use crate::physconst::PHYS_SKIN;
use crate::plunger::{HitPlunger, PlungerData};

fn push(out: &mut Colliders, base: HitObjectBase, shape: Shape) {
    let owner = out.owner();
    out.out.push(Collider { base, shape, owner });
}

/// A bumper's settings (VPX's `BumperData`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BumperSetup {
    pub center: Vertex2D,
    pub radius: f32,
    pub height_scale: f32,
    pub hit_event: bool,
    pub threshold: f32,
    pub force: f32,
    pub collidable: bool,
    /// In degrees.
    pub scatter: f32,
}

/// VPX's `Bumper::PhysicSetup` for play. VPX sets only the bumper's scatter;
/// its elasticity and friction stay at the defaults.
pub fn bumper_colliders(b: &BumperSetup, height: f32, out: &mut Colliders) {
    let data = BumperData { hit_event: b.hit_event, threshold: b.threshold, force: b.force };
    let mut bc = BumperHitCircle::new(out.part, data, b.center, b.radius);
    bc.anim_hit_event = b.hit_event;
    let mut base = HitObjectBase::with_z(height, height + b.height_scale);
    base.enabled = b.collidable;
    base.scatter = ang_to_rad(b.scatter);
    base.quiet = (!b.hit_event).then_some((out.part, b.threshold));
    push(out, base, Shape::Bumper(bc));
}

/// A gate's settings (VPX's `GateData`); angles in radians, as VPX stores them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateSetup {
    pub center: Vertex2D,
    pub length: f32,
    pub height: f32,
    /// In degrees.
    pub rotation: f32,
    pub angle_min: f32,
    pub angle_max: f32,
    pub damping: f32,
    pub gravityfactor: f32,
    pub two_way: bool,
    pub collidable: bool,
    pub show_bracket: bool,
    pub elasticity: f32,
    pub friction: f32,
    /// In degrees.
    pub scatter: f32,
}

/// VPX's `Gate::PhysicSetup`: a one-way gate's blocking segment, the gate,
/// and the bracket's posts. Corrects inverted angles, as VPX does to the part.
pub fn gate_colliders(g: &GateSetup, height: f32, out: &mut Colliders) {
    let h = g.height; // relative height of the gate

    let halflength = g.length * 0.5;

    let angle_min = min(g.angle_min, g.angle_max); // correct angle inversions
    let angle_max = max(g.angle_min, g.angle_max);

    let radangle = ang_to_rad(g.rotation);
    let tangent = Vertex2D::new(radangle.cos(), radangle.sin());

    // oversize by the ball's radius to prevent the ball from clipping through
    let skin = PHYS_SKIN as f32;
    let rgv = [g.center + (halflength + skin) * tangent, g.center - (halflength + skin) * tangent];

    if !g.two_way {
        let mut base = HitObjectBase::with_z(height, height + (2.0 * PHYS_SKIN) as f32); // !! = ball diameter
        base.elasticity = g.elasticity;
        base.friction = g.friction;
        base.scatter = ang_to_rad(g.scatter);
        push(out, base, Shape::LineSeg(LineSeg::new(rgv[0], rgv[1])));
    }

    let data = GateData { two_way: g.two_way, height: g.height };
    let mut hg =
        HitGate::new(out.part, data, g.center, g.length, g.rotation, height, angle_min, angle_max, g.damping, g.gravityfactor);
    hg.two_way = g.two_way;
    let base = HitObjectBase { obj: Some(out.part), fe: true, enabled: g.collidable, ..HitObjectBase::default() };
    push(out, base, Shape::Gate(Box::new(hg)));

    if g.show_bracket {
        let base = HitObjectBase::with_z(height, height + h);
        push(out, base, Shape::Circle(HitCircle::new(g.center + halflength * tangent, 0.01)));
        push(out, base, Shape::Circle(HitCircle::new(g.center - halflength * tangent, 0.01)));
    }
}

/// A spinner's settings (VPX's `SpinnerData`); angles in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpinnerSetup {
    pub center: Vertex2D,
    pub length: f32,
    /// In degrees.
    pub rotation: f32,
    pub height: f32,
    pub damping: f32,
    pub angle_max: f32,
    pub angle_min: f32,
    pub elasticity: f32,
    pub visible: bool,
    pub show_bracket: bool,
}

/// VPX's `Spinner::PhysicSetup`: the spinner, and the bracket's posts.
pub fn spinner_colliders(s: &SpinnerSetup, height: f32, out: &mut Colliders) {
    let h = s.height + 30.0;

    let angle_min = min(s.angle_min, s.angle_max); // correct angle inversions
    let angle_max = max(s.angle_min, s.angle_max);

    let data = SpinnerData { height: s.height, angle_min, angle_max };
    let hs = HitSpinner::new(out.part, data, s.center, s.length, s.rotation, height, s.damping, s.elasticity, s.visible);
    push(out, HitObjectBase::default(), Shape::Spinner(Box::new(hs)));

    if s.show_bracket {
        // add a hit shape for the bracket if shown, just in case if the bracket spinner height is low enough so the ball can hit it
        let halflength = s.length * 0.5 + (s.length * 0.1875);
        let radangle = ang_to_rad(s.rotation);
        let sn = radangle.sin();
        let cs = radangle.cos();

        let base = HitObjectBase::with_z(height + s.height, height + h);
        let r = s.length * 0.075;
        push(out, base, Shape::Circle(HitCircle::new(Vertex2D::new(s.center.x + cs * halflength, s.center.y + sn * halflength), r)));
        push(out, base, Shape::Circle(HitCircle::new(Vertex2D::new(s.center.x - cs * halflength, s.center.y - sn * halflength), r)));
    }
}

/// A trigger's settings (VPX's `TriggerData`).
#[derive(Clone, Debug, PartialEq)]
pub struct TriggerSetup {
    pub center: Vertex2D,
    pub radius: f32,
    pub hit_height: f32,
    pub enabled: bool,
    /// A star or button trigger is round; the others follow their outline.
    pub round: bool,
    pub points: Vec<DragPoint>,
}

/// VPX's `Trigger::PhysicSetup` for play: a round trigger's circle, or the
/// sides and top of a shaped one.
pub fn trigger_colliders(t: &TriggerSetup, height: f32, out: &mut Colliders) {
    let obj = Some(out.part);
    if t.round {
        let base = HitObjectBase { enabled: t.enabled, obj_type: ObjType::Trigger, obj, ..HitObjectBase::with_z(height, height + t.hit_height) };
        push(out, base, Shape::TriggerCircle(TriggerHitCircle::new(t.center, t.radius)));
    } else {
        let vvertex = get_rg_vertex::<Vertex2D>(&t.points, true, 4.0);

        let count = vvertex.len();
        for i in 0..count {
            let pv1 = &vvertex[if i + 1 < count { i + 1 } else { 0 }];
            let pv2 = &vvertex[if i + 2 < count { i + 2 } else { i + 2 - count }];

            // adjust for same hit height as circular
            let base = HitObjectBase {
                obj_type: ObjType::Trigger,
                obj,
                ..HitObjectBase::with_z(height, height + max(t.hit_height - 8.0, 0.0))
            };
            push(out, base, Shape::TriggerSeg(TriggerLineSeg::new(pv1.v, pv2.v, t.enabled)));
        }

        let rgv3d: Vec<Vertex3Ds> = vvertex.iter().map(|v| Vertex3Ds::new(v.v.x, v.v.y, height + (PHYS_SKIN * 2.0) as f32)).collect();
        let mut base = HitObjectBase::default();
        let poly = Hit3DPoly::new(rgv3d, &mut base);
        base.obj_type = ObjType::Trigger;
        base.obj = obj;
        push(out, base, Shape::Poly3D(poly));
    }
}

/// A kicker's settings (VPX's `KickerData`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KickerSetup {
    pub center: Vertex2D,
    pub radius: f32,
    pub hit_height: f32,
    pub enabled: bool,
    pub data: KickerData,
}

/// VPX's `Kicker::PhysicSetup` for play.
pub fn kicker_colliders(k: &KickerSetup, height: f32, out: &mut Colliders) {
    let kc = KickerHitCircle::new(out.part, k.data, k.center, k.radius, height);
    let base = HitObjectBase {
        enabled: k.enabled,
        obj_type: ObjType::Kicker,
        obj: Some(out.part),
        ..HitObjectBase::with_z(height, height + k.hit_height)
    };
    push(out, base, Shape::Kicker(Box::new(kc)));
}

/// A flipper's settings (VPX's `FlipperData`); angles in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlipperSetup {
    pub center: Vertex2D,
    pub base_radius: f32,
    pub end_radius: f32,
    pub flipper_radius_min: f32,
    pub flipper_radius_max: f32,
    pub start_angle: f32,
    pub end_angle: f32,
    pub height: f32,
    pub enabled: bool,
    pub data: FlipperData,
}

/// VPX's `Flipper::PhysicSetup`: the flipper's length depends on the table's
/// difficulty when the part gives a range.
pub fn flipper_colliders(f: &FlipperSetup, height: f32, global_difficulty: f32, out: &mut Colliders) {
    let flipper_radius = if f.flipper_radius_min > 0.0 && f.flipper_radius_max > f.flipper_radius_min {
        let r = f.flipper_radius_max - (f.flipper_radius_max - f.flipper_radius_min) * global_difficulty;
        max(r, f.base_radius - f.end_radius + 0.05)
    } else {
        f.flipper_radius_max
    };
    let (hf, base) = HitFlipper::new(
        out.part,
        f.data,
        f.center,
        max(f.base_radius, 0.01),
        max(f.end_radius, 0.01),
        max(flipper_radius, 0.01),
        ang_to_rad(f.start_angle),
        ang_to_rad(f.end_angle),
        height,
        height + f.height,
        f.enabled,
    );
    push(out, base, Shape::Flipper(Box::new(hf)));
}

/// A plunger's settings (VPX's `PlungerData`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlungerSetup {
    /// `m_v`: the plunger's tip position at rest.
    pub v: Vertex2D,
    pub width: f32,
    pub height: f32,
    pub stroke: f32,
    pub park_position: f32,
    pub scatter_velocity: f32,
    pub data: PlungerData,
}

/// VPX's `Plunger::PhysicSetup` for play.
pub fn plunger_colliders(p: &PlungerSetup, height: f32, out: &mut Colliders) {
    let (hp, base) =
        HitPlunger::new(out.part, p.data, p.v, p.width, p.height, p.stroke, height, p.park_position, p.scatter_velocity);
    push(out, base, Shape::Plunger(Box::new(hp)));
}
