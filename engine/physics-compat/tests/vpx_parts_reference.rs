//! Comparison of the ported part colliders with VPX's: bumpers, slingshots, gates, spinners,
//! triggers, kickers and plungers — hit tests, collisions, the motion of gates, spinners and
//! plungers, and the plunger's pull and fire.
//!
//! The data files were written by `tests/vpi/test-vpi-parts.cpp`, a test we add to VPX's physics
//! tester (build 5436). Each case gives a real part random settings and builds its collider as the
//! part's setup does.
//!
//! - `vpx-parts-5436-precise.txt` (VPX built with exact float rules): bit for bit.
//! - `vpx-parts-5436.txt` (VPX as shipped): float drift reported; decisions may flip in at most 1%
//!   of cases.

mod common;

use common::{Check, Cursor, Mode, Stats, data_lines, read_data};
use physics_compat::collide::{CollisionEvent, HitObjectBase, HitRef, ObjType};
use physics_compat::collideex::{
    BumperData, BumperHitCircle, GateData, HitGate, HitSpinner, LineSegSlingshot, SlingshotData, SpinnerData,
    TriggerHitCircle, TriggerLineSeg,
};
use physics_compat::engine::{Collider, NoScript, PhysicsEngine, PlayfieldMaterial, Shape, TableBox};
use physics_compat::env::{BallId, PartId, PhysicsEnv};
use physics_compat::fpmode::VpxFloatMode;
use physics_compat::hitball::{BallS, HitBall};
use physics_compat::kicker::{KickerData, KickerHitCircle};
use physics_compat::math::Mwc64x;
use physics_compat::plunger::{HitPlunger, PlungerData, PlungerMover};
use physics_compat::quadtree::{Owner, OwnerKind};
use std::cell::RefCell;
use std::rc::Rc;

/// The part itself in volume sets.
const SELF: PartId = PartId(1);
/// Where a part collider sits in an engine built around it (after the playfield and top glass).
const ONLY: u32 = 2;

fn read_vol(c: &mut Cursor) -> Vec<PartId> {
    let n = c.i();
    (0..n).map(|_| PartId(c.i() as u32)).collect()
}

fn read_ball_s(c: &mut Cursor, vol: Option<Vec<PartId>>) -> BallS {
    BallS {
        pos: c.v3(),
        vel: c.v3(),
        radius: c.f(),
        mass: c.f(),
        locked_in_kicker: c.b(),
        vp_vol_objs: Some(Rc::new(RefCell::new(vol.unwrap_or_default()))),
    }
}

/// The ball for a collision, as `WriteHitBall` wrote it.
fn read_hit_ball(c: &mut Cursor) -> HitBall {
    let mut hb = HitBall::new(BallId(1));
    hb.d.pos = c.v3();
    hb.d.vel = c.v3();
    hb.d.radius = c.f();
    hb.d.mass = c.f();
    hb.angularmomentum = c.v3();
    hb.coll.hitdistance = c.f();
    hb.last_event_pos = c.v3();
    hb.last_event_sqr_dist = c.f();
    hb.old_vel = c.v3();
    hb
}

fn set_vol(hb: &mut HitBall, vol: Vec<PartId>) {
    if let Some(set) = &hb.d.vp_vol_objs {
        *set.borrow_mut() = vol;
    }
}

fn vol_of(b: &BallS) -> Vec<PartId> {
    b.vp_vol_objs.as_ref().map(|s| s.borrow().clone()).unwrap_or_default()
}

/// The ball after a collision, as `WriteHitBallAfter` wrote it.
fn check_hit_ball_after(c: &mut Cursor, ck: &mut Check, hb: &HitBall) {
    ck.v3("pos", hb.d.pos, c.v3());
    ck.v3("vel", hb.d.vel, c.v3());
    ck.v3("angmom", hb.angularmomentum, c.v3());
    ck.v3("last_event_pos", hb.last_event_pos, c.v3());
    ck.f("last_event_sqr_dist", hb.last_event_sqr_dist, c.f());
    ck.v3("old_vel", hb.old_vel, c.v3());
    ck.eq("locked_in_kicker", hb.d.locked_in_kicker, c.b());
}

/// A hit test's result, as `WriteHit` wrote it.
fn check_hit(c: &mut Cursor, ck: &mut Check, t: f32, coll: &CollisionEvent) {
    let vt = c.f();
    if (t >= 0.0) != (vt >= 0.0) {
        ck.eq("hit or miss", t >= 0.0, vt >= 0.0);
    }
    ck.f("hittime", t, vt);
    if vt >= 0.0 {
        ck.v3("hitnormal", coll.hitnormal, c.v3());
        ck.f("hitdistance", coll.hitdistance, c.f());
        let contact = c.b();
        ck.eq("is_contact", coll.is_contact, contact);
        if contact {
            ck.f("hit_org_normalvelocity", coll.hit_org_normalvelocity, c.f());
        }
        ck.eq("hitflag", coll.hitflag, c.b());
    }
}

fn read_material(c: &mut Cursor, base: &mut HitObjectBase) {
    base.elasticity = c.f();
    base.elasticity_falloff = c.f();
    base.friction = c.f();
    base.scatter = c.f();
}

/// An engine holding just this collider (and the ball), for the parts handled at engine level.
fn engine_with(env: &PhysicsEnv, collider: Collider, ball: Option<HitBall>) -> PhysicsEngine {
    let table = TableBox { left: 0.0, top: 0.0, right: 1000.0, bottom: 2000.0, glass_top_height: 210.0, glass_bottom_height: 210.0 };
    let pf = PlayfieldMaterial { friction: 0.3, elasticity: 0.3, elasticity_falloff: 0.0, scatter_deg: 0.0 };
    let mut e = PhysicsEngine::new(&table, &pf, Owner { id: 0, kind: OwnerKind::Other }, vec![collider], ball.into_iter().collect());
    e.env.gravity = env.gravity;
    e.env.global_difficulty = env.global_difficulty;
    e.env.hard_scatter = env.hard_scatter;
    e
}

fn part_collider(base: HitObjectBase, shape: Shape) -> Collider {
    Collider { base, shape, owner: Owner { id: SELF.0, kind: OwnerKind::Other } }
}

/// Sets the plunger's state of motion, as `WritePlunger` wrote it.
fn read_plunger(c: &mut Cursor, m: &mut PlungerMover) {
    m.pos = c.f();
    m.speed = c.f();
    m.travel_limit.set(c.f());
    m.fire_timer = c.i() as i32;
    m.fire_speed = c.f();
    m.fire_bounce = c.f();
    m.pull_force = c.f();
    m.reverse_impulse = c.f();
    m.stroke_events_armed = c.b();
    m.initial_speed = c.f();
    m.retract_wait_loop = c.i() as i32;
    m.add_retract_motion = c.b();
    m.retract_motion = c.b();
    m.auto_fire_timer = c.i() as i32;
    m.set_objects(m.pos); // as the C++ test's randomState does
}

/// Checks the plunger's state of motion against `WritePlunger`. A new plunger's
/// `fire_bounce` and `initial_speed` are left unset by VPX until it first fires or
/// pulls (they're never read before), so those two are skipped for it.
fn check_plunger(c: &mut Cursor, ck: &mut Check, m: &PlungerMover, new: bool) {
    ck.f("pos", m.pos, c.f());
    ck.f("speed", m.speed, c.f());
    ck.f("travel_limit", m.travel_limit.get(), c.f());
    ck.eq("fire_timer", m.fire_timer as i64, c.i());
    ck.f("fire_speed", m.fire_speed, c.f());
    let fire_bounce = c.f();
    if !new {
        ck.f("fire_bounce", m.fire_bounce, fire_bounce);
    }
    ck.f("pull_force", m.pull_force, c.f());
    ck.f("reverse_impulse", m.reverse_impulse, c.f());
    ck.eq("stroke_events_armed", m.stroke_events_armed, c.b());
    let initial_speed = c.f();
    if !new {
        ck.f("initial_speed", m.initial_speed, initial_speed);
    }
    ck.eq("retract_wait_loop", m.retract_wait_loop as i64, c.i());
    ck.eq("add_retract_motion", m.add_retract_motion, c.b());
    ck.eq("retract_motion", m.retract_motion, c.b());
    ck.eq("auto_fire_timer", m.auto_fire_timer as i64, c.i());
}

fn compare(file: &str, mode: Mode) -> (String, usize) {
    let data = read_data(file);
    let _fp = VpxFloatMode::enter();
    let mut env = PhysicsEnv::default();
    let mut stats = Stats::default();
    let mut gate: Option<(HitGate, HitObjectBase)> = None;
    let mut spinner: Option<(HitSpinner, HitObjectBase)> = None;
    let mut plunger: Option<(HitPlunger, HitObjectBase)> = None;

    for line in data_lines(&data) {
        let kind = line.split_whitespace().next().unwrap();
        let mut c = Cursor::new(line);
        let mut ck = Check::new(line, mode);

        match kind {
            "env" => {
                env.gravity = c.v3();
                env.global_difficulty = c.f();
                env.hard_scatter = c.f();
                continue;
            }
            "bumper" => {
                let (center, r, zlow, zhigh) = (c.v2(), c.f(), c.f(), c.f());
                let mut base = HitObjectBase::with_z(zlow, zhigh);
                read_material(&mut c, &mut base);
                base.enabled = c.b();
                let data = BumperData { hit_event: c.b(), threshold: c.f(), force: c.f() };
                let ball = read_ball_s(&mut c, None);
                let dtime = c.f();
                let mut hb = read_hit_ball(&mut c);
                let cc = CollisionEvent { ball: Some(hb.id), hitnormal: c.v3(), ..CollisionEvent::default() };
                env.rng = Mwc64x { state: c.u64() };
                c.arrow();
                let mut bc = BumperHitCircle::new(SELF, data, center, r);
                let mut coll = CollisionEvent::default();
                let t = bc.circle.hit_test(&base, &ball, dtime, &mut coll);
                check_hit(&mut c, &mut ck, t, &coll);
                bc.anim_hit_event = false;
                bc.collide(&base, &cc, &mut hb, &mut env);
                check_hit_ball_after(&mut c, &mut ck, &hb);
                ck.eq("rng", env.rng.state, c.u64());
                let anim = c.b();
                ck.eq("anim_hit_event", bc.anim_hit_event, anim);
                if anim {
                    ck.v3("anim_hit_ball_position", bc.anim_hit_ball_position, c.v3());
                }
            }
            "sling" => {
                let (v1, v2, zlow, zhigh) = (c.v2(), c.v2(), c.f(), c.f());
                let mut base = HitObjectBase::with_z(zlow, zhigh);
                read_material(&mut c, &mut base);
                let force = c.f();
                base.threshold = c.f();
                base.obj = Some(SELF);
                base.fe = true;
                let data = SlingshotData { threshold: c.f(), disabled: c.b() };
                let ball = read_ball_s(&mut c, None);
                let dtime = c.f();
                let mut hb = read_hit_ball(&mut c);
                let cc = CollisionEvent { ball: Some(hb.id), hitnormal: c.v3(), ..CollisionEvent::default() };
                env.rng = Mwc64x { state: c.u64() };
                env.time_msec = 0;
                c.arrow();
                let mut ls = LineSegSlingshot::new(SELF, data, force, v1, v2);
                let mut coll = CollisionEvent::default();
                let t = ls.hit_test(&base, &ball, dtime, &mut coll);
                check_hit(&mut c, &mut ck, t, &coll);
                ls.collide(&base, &cc, &mut hb, &mut env);
                check_hit_ball_after(&mut c, &mut ck, &hb);
                ck.eq("rng", env.rng.state, c.u64());
                ck.eq("time_reset", ls.time_reset as i64, c.i());
            }
            "gate" => {
                let center = c.v2();
                let (length, rotation, part_height, angle_min, angle_max, damping, gravityfactor) =
                    (c.f(), c.f(), c.f(), c.f(), c.f(), c.f(), c.f());
                let two_way = c.b();
                let height = c.f();
                let mut base = HitObjectBase { obj: Some(SELF), fe: true, enabled: c.b(), ..HitObjectBase::default() };
                let ball = read_ball_s(&mut c, None);
                let dtime = c.f();
                let mut hb = read_hit_ball(&mut c);
                let cc = CollisionEvent { ball: Some(hb.id), hitnormal: c.v3(), hitflag: c.b(), ..CollisionEvent::default() };
                c.arrow();
                let data = GateData { two_way, height: part_height };
                let mut hg = HitGate::new(SELF, data, center, length, rotation, height, angle_min, angle_max, damping, gravityfactor);
                hg.two_way = two_way;
                hg.calc_hit_bbox(&mut base);
                ck.f("bbox.left", base.hit_bbox.left, c.f());
                ck.f("bbox.right", base.hit_bbox.right, c.f());
                ck.f("bbox.top", base.hit_bbox.top, c.f());
                ck.f("bbox.bottom", base.hit_bbox.bottom, c.f());
                ck.f("damping", hg.mover.damping, c.f());
                let mut coll = CollisionEvent::default();
                let t = hg.hit_test(&base, &ball, dtime, &mut coll);
                check_hit(&mut c, &mut ck, t, &coll);
                hg.collide(&base, &cc, &mut hb, &mut env);
                check_hit_ball_after(&mut c, &mut ck, &hb);
                ck.f("anglespeed", hg.mover.anglespeed, c.f());
                ck.eq("hit_direction", hg.mover.hit_direction, c.b());
                gate = Some((hg, base));
            }
            "gatemove" => {
                let (hg, base) = gate.expect("a gate line first");
                let mut e = engine_with(&env, part_collider(base, Shape::Gate(Box::new(hg))), None);
                let Shape::Gate(g) = &mut e.collider_mut(ONLY).shape else { unreachable!() };
                g.mover.angle = c.f();
                g.mover.anglespeed = c.f();
                g.mover.open = c.b();
                g.mover.forced_move = c.b();
                g.mover.hit_direction = c.b();
                let dtime = c.f();
                c.arrow();
                g.mover.update_velocities();
                ck.f("angle", g.mover.angle, c.f());
                ck.f("anglespeed", g.mover.anglespeed, c.f());
                e.update_mover_displacements(ONLY, dtime, &mut NoScript);
                let Shape::Gate(g) = &e.colliders()[ONLY as usize].shape else { unreachable!() };
                ck.f("angle after", g.mover.angle, c.f());
                ck.f("anglespeed after", g.mover.anglespeed, c.f());
                ck.eq("forced_move", g.mover.forced_move, c.b());
            }
            "spinner" => {
                let center = c.v2();
                let (length, rotation, part_height, angle_min, angle_max, damping, elasticity, height) =
                    (c.f(), c.f(), c.f(), c.f(), c.f(), c.f(), c.f(), c.f());
                let mut base = HitObjectBase { enabled: c.b(), ..HitObjectBase::default() };
                let ball = read_ball_s(&mut c, None);
                let dtime = c.f();
                let hb = read_hit_ball(&mut c);
                let cc = CollisionEvent { ball: Some(hb.id), hitnormal: c.v3(), hitflag: c.b(), ..CollisionEvent::default() };
                c.arrow();
                let data = SpinnerData { height: part_height, angle_min, angle_max };
                let mut hs = HitSpinner::new(SELF, data, center, length, rotation, height, damping, elasticity, true);
                hs.calc_hit_bbox(&mut base);
                ck.f("bbox.left", base.hit_bbox.left, c.f());
                ck.f("bbox.right", base.hit_bbox.right, c.f());
                ck.f("bbox.top", base.hit_bbox.top, c.f());
                ck.f("bbox.bottom", base.hit_bbox.bottom, c.f());
                ck.f("angle", hs.mover.angle, c.f());
                ck.f("angle_min", hs.mover.angle_min, c.f());
                ck.f("angle_max", hs.mover.angle_max, c.f());
                ck.f("damping", hs.mover.damping, c.f());
                let mut coll = CollisionEvent::default();
                let t = hs.hit_test(&base, &ball, dtime, &mut coll);
                check_hit(&mut c, &mut ck, t, &coll);
                hs.collide(&cc, &hb);
                ck.f("anglespeed", hs.mover.anglespeed, c.f());
                spinner = Some((hs, base));
            }
            "spinmove" => {
                let (hs, base) = spinner.expect("a spinner line first");
                let mut e = engine_with(&env, part_collider(base, Shape::Spinner(Box::new(hs))), None);
                let Shape::Spinner(s) = &mut e.collider_mut(ONLY).shape else { unreachable!() };
                s.mover.angle = c.f();
                s.mover.anglespeed = c.f();
                let dtime = c.f();
                c.arrow();
                s.mover.update_velocities();
                ck.f("angle", s.mover.angle, c.f());
                ck.f("anglespeed", s.mover.anglespeed, c.f());
                e.update_mover_displacements(ONLY, dtime, &mut NoScript);
                let Shape::Spinner(s) = &e.colliders()[ONLY as usize].shape else { unreachable!() };
                ck.f("angle after", s.mover.angle, c.f());
                ck.f("anglespeed after", s.mover.anglespeed, c.f());
            }
            "trigseg" | "trigcircle" => {
                let (v1, v2, r, zlow, zhigh) = (c.v2(), c.v2(), c.f(), c.f(), c.f());
                let trigger_enabled = c.b();
                let mut base = HitObjectBase::with_z(zlow, zhigh);
                base.obj_type = ObjType::Trigger;
                base.obj = Some(SELF);
                base.enabled = c.b();
                let ball = read_ball_s(&mut c, None);
                let vol = read_vol(&mut c);
                let ball = BallS { vp_vol_objs: Some(Rc::new(RefCell::new(vol))), ..ball };
                let dtime = c.f();
                let mut hb = read_hit_ball(&mut c);
                set_vol(&mut hb, read_vol(&mut c));
                let cc = CollisionEvent { ball: Some(hb.id), hitflag: c.b(), ..CollisionEvent::default() };
                c.arrow();
                let mut coll = CollisionEvent::default();
                let t = if kind == "trigseg" {
                    TriggerLineSeg::new(v1, v2, trigger_enabled).hit_test(&base, &ball, dtime, &mut coll)
                } else {
                    TriggerHitCircle::new(v1, r).hit_test(&base, &ball, dtime, &mut coll)
                };
                check_hit(&mut c, &mut ck, t, &coll);
                ck.eq("volumes after hit test", vol_of(&ball), read_vol(&mut c));
                if kind == "trigseg" {
                    TriggerLineSeg::collide(&base, &cc, &mut hb, &mut env);
                } else {
                    TriggerHitCircle::collide(&base, &cc, &mut hb, &mut env);
                }
                check_hit_ball_after(&mut c, &mut ck, &hb);
                ck.eq("volumes after collide", vol_of(&hb.d), read_vol(&mut c));
            }
            "kicker" => {
                let center = c.v2();
                let (radius, hit_height, hit_accuracy) = (c.f(), c.f(), c.f());
                let (legacy_mode, fall_through, enabled) = (c.b(), c.b(), c.b());
                let height = c.f();
                let ball = read_ball_s(&mut c, None);
                let vol = read_vol(&mut c);
                let ball = BallS { vp_vol_objs: Some(Rc::new(RefCell::new(vol))), ..ball };
                let dtime = c.f();
                let mut hb = read_hit_ball(&mut c);
                set_vol(&mut hb, read_vol(&mut c));
                let cc = CollisionEvent { ball: Some(hb.id), hitnormal: c.v3(), hitflag: c.b(), ..CollisionEvent::default() };
                c.arrow();

                let data = KickerData { legacy_mode, hit_accuracy, fall_through };
                let kc = KickerHitCircle::new(SELF, data, center, radius, height);
                let mut base = HitObjectBase::with_z(height, height + hit_height);
                base.enabled = enabled;
                base.obj_type = ObjType::Kicker;
                base.obj = Some(SELF);
                let mut coll = CollisionEvent::default();
                let t = kc.hit_test(&base, &ball, dtime, &mut coll);
                check_hit(&mut c, &mut ck, t, &coll);
                ck.eq("volumes after hit test", vol_of(&ball), read_vol(&mut c));

                let mut e = engine_with(&env, part_collider(base, Shape::Kicker(Box::new(kc))), Some(hb));
                e.collide(BallId(1), HitRef::Collider(ONLY), &cc, &mut NoScript);
                let hb = e.balls().iter().find(|b| b.id == BallId(1)).expect("ball");
                check_hit_ball_after(&mut c, &mut ck, hb);
                ck.eq("volumes after collide", vol_of(&hb.d), read_vol(&mut c));
                let Shape::Kicker(kc) = &e.colliders()[ONLY as usize].shape else { unreachable!() };
                ck.eq("holds the ball", kc.hit_ball == Some(BallId(1)), c.b());
                ck.eq("last captured", kc.last_captured_ball == Some(BallId(1)), c.b());
            }
            "plunger" => {
                let center = c.v2();
                let (width, height, stroke, speed_fire, mech_strength, momentum_xfer, scatter_velocity, park_position) =
                    (c.f(), c.f(), c.f(), c.f(), c.f(), c.f(), c.f(), c.f());
                let auto_plunger = c.b();
                let zheight = c.f();
                c.arrow();
                let data = PlungerData { speed_fire, mech_plunger: false, auto_plunger, mech_strength, momentum_xfer };
                let (php, mut base) =
                    HitPlunger::new(SELF, data, center, width, height, stroke, zheight, park_position, scatter_velocity);
                php.calc_hit_bbox(&mut base);
                ck.f("bbox.left", base.hit_bbox.left, c.f());
                ck.f("bbox.right", base.hit_bbox.right, c.f());
                ck.f("bbox.top", base.hit_bbox.top, c.f());
                ck.f("bbox.bottom", base.hit_bbox.bottom, c.f());
                check_plunger(&mut c, &mut ck, &php.mover, true);
                plunger = Some((php, base));
            }
            "plungehit" => {
                let (php, _) = plunger.as_mut().expect("a plunger line first");
                read_plunger(&mut c, &mut php.mover);
                let ball = read_ball_s(&mut c, None);
                let dtime = c.f();
                let mut hb = read_hit_ball(&mut c);
                let cc = CollisionEvent {
                    ball: Some(hb.id),
                    hitnormal: c.v3(),
                    hitvel: c.v2(),
                    hitdistance: c.f(),
                    ..CollisionEvent::default()
                };
                env.rng = Mwc64x { state: c.u64() };
                c.arrow();
                let mut coll = CollisionEvent::default();
                let t = php.hit_test(&ball, dtime, &mut coll, None);
                check_hit(&mut c, &mut ck, t, &coll);
                if t >= 0.0 {
                    ck.v2("hitvel", coll.hitvel, c.v2());
                }
                ck.f("travel_limit", php.mover.travel_limit.get(), c.f());
                php.collide(&cc, &mut hb, &mut env);
                check_hit_ball_after(&mut c, &mut ck, &hb);
                ck.eq("rng", env.rng.state, c.u64());
                ck.f("reverse_impulse", php.mover.reverse_impulse, c.f());
                ck.f("fire_bounce", php.mover.fire_bounce, c.f());
            }
            "plungemove" => {
                let (php, _) = plunger.as_mut().expect("a plunger line first");
                read_plunger(&mut c, &mut php.mover);
                let dtime = c.f();
                c.arrow();
                let part = php.part;
                php.mover.update_velocities(part, None, &mut env);
                check_plunger(&mut c, &mut ck, &php.mover, false);
                php.mover.update_displacements(dtime, part, &mut env);
                php.mover.finish_update();
                check_plunger(&mut c, &mut ck, &php.mover, false);
                ck.v2("end.v1", php.mover.lineseg_end.v1, c.v2());
                ck.v2("end.v2", php.mover.lineseg_end.v2, c.v2());
                ck.v2("end.normal", php.mover.lineseg_end.normal, c.v2());
                ck.v2("side0.v1", php.mover.lineseg_side[0].v1, c.v2());
                ck.v2("joint_end1", php.mover.joint_end[1].xy, c.v2());
                env.events.clear();
            }
            "plungescript" => {
                let (php, _) = plunger.as_mut().expect("a plunger line first");
                read_plunger(&mut c, &mut php.mover);
                let op = c.i();
                let speed_pull = c.f();
                c.arrow();
                match op {
                    0 => php.mover.pull_back(speed_pull),
                    1 => php.mover.pull_back_and_retract(speed_pull),
                    2 => php.mover.fire(),
                    _ => php.mover.fire_from(1.0),
                }
                check_plunger(&mut c, &mut ck, &php.mover, false);
            }
            other => panic!("unknown reference line {other}"),
        }
        stats.add(kind, ck);
    }
    stats.report(mode)
}

#[test]
#[cfg(target_arch = "x86_64")]
fn part_colliders_match_exact_vpx_bit_for_bit() {
    let (report, bad) = compare("vpx-parts-5436-precise.txt", Mode::Exact);
    println!("{report}");
    assert_eq!(bad, 0, "the ported part colliders differ from exact VPX:\n{report}");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn shipped_vpx_part_colliders_drift_only_by_float_noise() {
    let (report, bad) = compare("vpx-parts-5436.txt", Mode::Fast);
    println!("{report}");
    assert_eq!(bad, 0, "too many decisions differ from VPX's fast build:\n{report}");
}
