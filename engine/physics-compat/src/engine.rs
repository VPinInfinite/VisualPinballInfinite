// Ported from Visual Pinball X: src/physics/PhysicsEngine.cpp and PhysicsEngine.h (license:GPLv3+),
// and DoHitTest from src/physics/collide.cpp, commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's physics engine: fixed 1 ms steps, and inside each step the
//! continuous collision search that moves the balls from one collision to the
//! next (`PhysicsSimulateCycle`).
//!
//! This is the engine as VPX runs it without its player, the way VPX's own
//! physics tester runs it: no keyboard or nudge input, no script timers inside
//! the loop, and no wall-clock safeguard against falling behind. Those belong
//! to the game loop around it.
//!
//! **Scripts run inside the step.** VPX calls the table's script the
//! moment a part fires an event — a hit, a flipper reaching its stop, two
//! balls colliding — and the script may change the physics before VPX
//! continues: kick the ball out of a kicker, energize a flipper, create or
//! destroy a ball. The engine does the same through its [`PhysicsHost`]: each
//! event is delivered where VPX fires it, and the physics carries on from
//! whatever state the host left.
//!
//! Ported so far: walls, posts, points, polygons, triangles, 3D lines, planes,
//! flippers, bumpers, slingshots, gates, spinners, triggers, kickers,
//! plungers and balls.

use crate::collide::{CollisionEvent, HitCircle, HitLineZ, HitObjectBase, HitPoint, HitRef, LineSeg, ObjType, do_hit_test_accept};
use crate::collideex::{
    BumperHitCircle, Hit3DPoly, HitGate, HitLine3D, HitPlane, HitSpinner, HitTriangle, LineSegSlingshot, TriggerHitCircle,
    TriggerLineSeg,
};
use crate::env::{BallId, PartId, PartSet, PhysicsEnv, PhysicsEvent};
use crate::flipper::HitFlipper;
use crate::kicker::KickerHitCircle;
use crate::plunger::{HitPlunger, PlungerSensor};
use crate::hitball::{BallS, HitBall};
use crate::kdtree::{HitKd, KdItem};
use crate::math::def::{ang_to_rad, clamp, inf_nan, max};
use crate::math::{FRect, Vertex2D, Vertex3Ds};
use crate::physconst::{DEFAULT_STEPTIME, PHYS_TOUCH, PHYSICS_STEPTIME, STATICCNTS, STATICTIME};
use crate::quadtree::{BallQuery, HitQuadtree, Owner, OwnerKind, TreeItem};


/// A collider's shape: the VPX `HitObject` classes ported so far.
#[derive(Clone, Debug)]
pub enum Shape {
    LineSeg(LineSeg),
    Circle(HitCircle),
    LineZ(HitLineZ),
    Point(HitPoint),
    Poly3D(Hit3DPoly),
    Triangle(HitTriangle),
    Plane(HitPlane),
    Line3D(HitLine3D),
    Flipper(Box<HitFlipper>),
    Bumper(BumperHitCircle),
    Slingshot(LineSegSlingshot),
    Gate(Box<HitGate>),
    Spinner(Box<HitSpinner>),
    TriggerSeg(TriggerLineSeg),
    TriggerCircle(TriggerHitCircle),
    Kicker(Box<KickerHitCircle>),
    Plunger(Box<HitPlunger>),
}

/// One of the table's colliders.
#[derive(Clone, Debug)]
pub struct Collider {
    pub base: HitObjectBase,
    pub shape: Shape,
    /// The part that made it (VPX's `m_editable`); the quadtree skips the
    /// colliders of primitives that aren't collidable and dropped hit targets.
    pub owner: Owner,
}

impl Collider {
    /// VPX's `CalcHitBBox`.
    pub fn calc_hit_bbox(&mut self) {
        match &mut self.shape {
            Shape::LineSeg(s) => s.calc_hit_bbox(&mut self.base),
            Shape::Circle(s) => s.calc_hit_bbox(&mut self.base),
            Shape::LineZ(s) => s.calc_hit_bbox(&mut self.base),
            Shape::Point(s) => s.calc_hit_bbox(&mut self.base),
            Shape::Poly3D(s) => s.calc_hit_bbox(&mut self.base),
            Shape::Triangle(s) => s.calc_hit_bbox(&mut self.base),
            Shape::Plane(_) => {} // infinite: never in the quadtree
            Shape::Line3D(_) => {} // already done in constructor
            Shape::Flipper(s) => s.calc_hit_bbox(&mut self.base),
            Shape::Bumper(s) => s.circle.calc_hit_bbox(&mut self.base),
            Shape::Slingshot(s) => s.seg.calc_hit_bbox(&mut self.base),
            Shape::Gate(s) => s.calc_hit_bbox(&mut self.base),
            Shape::Spinner(s) => s.calc_hit_bbox(&mut self.base),
            Shape::TriggerSeg(s) => s.seg.calc_hit_bbox(&mut self.base),
            Shape::TriggerCircle(s) => s.circle.calc_hit_bbox(&mut self.base),
            Shape::Kicker(s) => s.circle.calc_hit_bbox(&mut self.base),
            Shape::Plunger(s) => s.calc_hit_bbox(&mut self.base),
        }
    }

    fn hit_test(&self, ball: &BallS, dtime: f32, coll: &mut CollisionEvent, sensor: Option<&dyn PlungerSensor>) -> f32 {
        let b = &self.base;
        match &self.shape {
            Shape::LineSeg(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Circle(s) => s.hit_test(b, ball, dtime, coll),
            Shape::LineZ(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Point(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Poly3D(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Triangle(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Plane(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Line3D(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Flipper(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Bumper(s) => s.circle.hit_test(b, ball, dtime, coll),
            Shape::Slingshot(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Gate(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Spinner(s) => s.hit_test(b, ball, dtime, coll),
            Shape::TriggerSeg(s) => s.hit_test(b, ball, dtime, coll),
            Shape::TriggerCircle(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Kicker(s) => s.hit_test(b, ball, dtime, coll),
            Shape::Plunger(s) => s.hit_test(ball, dtime, coll, sensor),
        }
    }

    fn collide(&mut self, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        let b = &self.base;
        match &mut self.shape {
            Shape::LineSeg(_) => LineSeg::collide(b, coll, ball, env),
            Shape::Circle(_) => HitCircle::collide(b, coll, ball, env),
            Shape::LineZ(_) => HitLineZ::collide(b, coll, ball, env),
            Shape::Point(_) => HitPoint::collide(b, coll, ball, env),
            Shape::Poly3D(s) => s.collide(b, coll, ball, env),
            Shape::Triangle(s) => s.collide(b, coll, ball, env),
            Shape::Plane(s) => s.collide(b, coll, ball, env),
            Shape::Line3D(_) => HitLine3D::collide(b, coll, ball, env),
            Shape::Flipper(s) => s.collide(b, coll, ball, env),
            Shape::Bumper(s) => s.collide(b, coll, ball, env),
            Shape::Slingshot(s) => s.collide(b, coll, ball, env),
            Shape::Gate(s) => s.collide(b, coll, ball, env),
            Shape::Spinner(s) => s.collide(coll, ball),
            Shape::TriggerSeg(_) => TriggerLineSeg::collide(b, coll, ball, env),
            Shape::TriggerCircle(_) => TriggerHitCircle::collide(b, coll, ball, env),
            Shape::Kicker(_) => unreachable!("kickers collide at the engine level"),
            Shape::Plunger(s) => s.collide(coll, ball, env),
        }
    }

    /// VPX's `HitObject::Contact`: resting contact with friction. Flippers
    /// have their own; gates and spinners do nothing.
    fn contact(&mut self, coll: &CollisionEvent, ball: &mut HitBall, dtime: f32, env: &PhysicsEnv) {
        match &mut self.shape {
            Shape::Flipper(s) => s.contact(&self.base, coll, ball, dtime, env),
            Shape::Gate(_) | Shape::Spinner(_) => {}
            _ => ball.handle_static_contact(coll, self.base.friction, dtime, env),
        }
    }
}

/// The table's size and glass, from which VPX adds its own colliders.
#[derive(Clone, Copy, Debug)]
pub struct TableBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub glass_top_height: f32,
    pub glass_bottom_height: f32,
}

/// The playfield's physics, with the table's overrides already applied.
#[derive(Clone, Copy, Debug)]
pub struct PlayfieldMaterial {
    pub friction: f32,
    pub elasticity: f32,
    pub elasticity_falloff: f32,
    /// In degrees, as the table stores it.
    pub scatter_deg: f32,
}

/// Something that moves on its own each step: VPX's `MoverObject`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mover {
    Ball(BallId),
    /// A collider that moves (a flipper), by index.
    Collider(u32),
}

/// What runs around the physics: VPX's player, with the table's script.
pub trait PhysicsHost {
    /// A script event, delivered at the moment VPX fires it, inside the
    /// physics step. The handler may change the engine; the physics continues
    /// from the changed state, as in VPX.
    fn event(&mut self, engine: &mut PhysicsEngine, event: PhysicsEvent);

    /// The start of each 1 ms step, before anything moves: where VPX fires
    /// the script's timers. The host may change the engine.
    fn step_begin(&mut self, _engine: &mut PhysicsEngine) {}

    /// VPX's `m_pactiveball = pball`: the ball about to collide, which the
    /// script sees as `ActiveBall` until another takes its place.
    fn active_ball(&mut self, _ball: BallId) {}
}

/// A table without a script: events go nowhere.
pub struct NoScript;

impl PhysicsHost for NoScript {
    fn event(&mut self, _engine: &mut PhysicsEngine, _event: PhysicsEvent) {}
}

/// Index of the playfield and top glass planes in `colliders`; the quadtree
/// holds the rest.
const PLAYFIELD: u32 = 0;
const TOP_GLASS: u32 = 1;

/// VPX's `PhysicsEngine`, headless.
pub struct PhysicsEngine {
    pub env: PhysicsEnv,
    colliders: Vec<Collider>,
    quadtree: HitQuadtree,
    kd: HitKd,
    /// VPX's `m_vballs`, in VPX's order.
    balls: Vec<HitBall>,
    movers: Vec<Mover>,
    /// VPX's `m_vFlippers`: the flippers' collider indices.
    flippers: Vec<u32>,
    /// The plungers' collider indices, to note when one touched a ball.
    plungers: Vec<u32>,
    /// True when the playfield is an implicit mesh, so its plane is hit tested.
    implicit_playfield_mesh: bool,
    /// When a plunger last touched a ball, in ms (VPX's `m_lastPlungerHit`, for the UI).
    last_plunger_hit: u32,
    /// Nudging (VPX's `NudgeHandler`), when the player turns it on; VPX's
    /// physics tester has none.
    pub nudge: Option<crate::nudge::NudgeHandler>,
    /// The cabinet's tilt bob (VPX's `PlumbHandler`), swung by the nudge, when the player
    /// sets one up.
    pub plumb: Option<crate::plumb::PlumbHandler>,
    contacts: Vec<CollisionEvent>,
    time_msec: u32,
    time_sec: f64,
    start_time_usec: u64,
    cur_physics_frame_time: u64,
    next_physics_frame_time: u64,
    phys_iterations: u32,
}

impl PhysicsEngine {
    /// VPX's constructor: takes the parts' colliders and balls in the order the
    /// table lists its parts, adds the table's own (its outer walls, glass and
    /// playfield) and builds the quadtree. `table_owner` stands for the table
    /// as the owner of its walls and glass.
    pub fn new(
        table: &TableBox,
        playfield: &PlayfieldMaterial,
        table_owner: Owner,
        part_colliders: Vec<Collider>,
        balls: Vec<HitBall>,
    ) -> Self {
        let mut e = Self {
            env: PhysicsEnv::default(),
            colliders: Vec::new(),
            quadtree: HitQuadtree::new(),
            kd: HitKd::new(),
            balls: Vec::new(),
            movers: Vec::new(),
            flippers: Vec::new(),
            plungers: Vec::new(),
            implicit_playfield_mesh: false,
            last_plunger_hit: 0,
            nudge: None,
            plumb: None,
            contacts: Vec::with_capacity(8),
            time_msec: 0,
            time_sec: 0.0,
            start_time_usec: 0,
            cur_physics_frame_time: 0,
            next_physics_frame_time: 0,
            phys_iterations: 0,
        };

        // Balls join as their part is set up; everything else waits in VPX's pending list
        for b in balls {
            e.add_ball(b);
        }

        // AddCabinetBoundingHitShapes: the table's outer walls, then its glass
        let (t, o) = (table, table_owner);
        let wall = |p1: Vertex2D, p2: Vertex2D, zhigh: f32| Collider {
            base: HitObjectBase::with_z(0.0, zhigh),
            shape: Shape::LineSeg(LineSeg::new(p1, p2)),
            owner: o,
        };
        let mut pending = part_colliders;
        pending.push(wall(Vertex2D::new(t.right, t.top), Vertex2D::new(t.right, t.bottom), t.glass_top_height));
        pending.push(wall(Vertex2D::new(t.left, t.bottom), Vertex2D::new(t.left, t.top), t.glass_bottom_height));
        pending.push(wall(Vertex2D::new(t.right, t.bottom), Vertex2D::new(t.left, t.bottom), t.glass_bottom_height));
        pending.push(wall(Vertex2D::new(t.left, t.top), Vertex2D::new(t.right, t.top), t.glass_top_height));
        let mut glass_base = HitObjectBase::default();
        let glass = Hit3DPoly::new(
            vec![
                Vertex3Ds::new(t.left, t.top, t.glass_top_height),
                Vertex3Ds::new(t.right, t.top, t.glass_top_height),
                Vertex3Ds::new(t.right, t.bottom, t.glass_bottom_height),
                Vertex3Ds::new(t.left, t.bottom, t.glass_bottom_height),
            ],
            &mut glass_base,
        );
        pending.push(Collider { base: glass_base, shape: Shape::Poly3D(glass), owner: o });

        // the playfield and the top glass are planes, which can't be in the quadtree (infinite size)
        let playfield_base = HitObjectBase {
            friction: playfield.friction,
            elasticity: playfield.elasticity,
            elasticity_falloff: playfield.elasticity_falloff,
            scatter: ang_to_rad(playfield.scatter_deg),
            ..HitObjectBase::default()
        };
        let mut glass_normal = Vertex3Ds::new(0.0, t.bottom - t.top, t.glass_bottom_height - t.glass_top_height);
        glass_normal.normalize();
        let top_glass = HitPlane::new(Vertex3Ds::new(0.0, glass_normal.z, -glass_normal.y), -t.glass_top_height);
        e.colliders.push(Collider {
            base: playfield_base,
            shape: Shape::Plane(HitPlane::new(Vertex3Ds::new(0.0, 0.0, 1.0), 0.0)),
            owner: o,
        });
        e.colliders.push(Collider {
            base: HitObjectBase { elasticity: 0.2, ..HitObjectBase::default() },
            shape: Shape::Plane(top_glass),
            owner: o,
        });

        // AddCollider computes each collider's box as it arrives; then the flippers and the other
        // movers are listed, in collider order, after the balls
        let mut items = Vec::with_capacity(pending.len());
        for mut c in pending {
            c.calc_hit_bbox();
            let index = e.colliders.len() as u32;
            if let Shape::Flipper(_) = c.shape {
                e.flippers.push(index);
            }
            if let Shape::Plunger(_) = c.shape {
                e.plungers.push(index);
            }
            if let Shape::Flipper(_) | Shape::Gate(_) | Shape::Spinner(_) | Shape::Plunger(_) = c.shape {
                e.movers.push(Mover::Collider(index)); // Spinner, Gate, Flipper, Plunger
            }
            items.push(TreeItem { id: index, bbox: c.base.hit_bbox, owner: c.owner });
            e.colliders.push(c);
        }

        // Limit to table bounds as we don't expect to play outside of it
        e.quadtree.set_bounds(FRect::new(t.left, t.right, t.top, t.bottom));
        e.quadtree.reset(items);
        e.quadtree.finalize();
        e
    }

    /// VPX's `SetGravity`.
    pub fn set_gravity(&mut self, slope_deg: f32, strength: f32) {
        self.env.gravity.x = 0.0;
        self.env.gravity.y = ang_to_rad(slope_deg).sin() * strength;
        self.env.gravity.z = -ang_to_rad(slope_deg).cos() * strength;
    }

    /// VPX's `SetImplicitPlayfieldMesh`: the playfield plane is hit tested.
    pub fn set_implicit_playfield_mesh(&mut self, on: bool) {
        self.implicit_playfield_mesh = on;
    }

    /// VPX's `AddCollider` for a ball.
    pub fn add_ball(&mut self, mut ball: HitBall) {
        ball.calc_hit_bbox();
        self.movers.push(Mover::Ball(ball.id)); // balls are always added separately to this list!
        self.kd.insert(KdItem { id: ball.id.0, bbox: ball.base.hit_bbox });
        self.balls.push(ball);
    }

    /// VPX's `RemoveCollider` for a ball.
    pub fn remove_ball(&mut self, id: BallId) -> Option<HitBall> {
        if let Some(i) = self.movers.iter().position(|m| *m == Mover::Ball(id)) {
            self.movers.remove(i);
        }
        let ball = self.balls.iter().position(|b| b.id == id).map(|i| self.balls.remove(i));
        self.kd.remove(id.0);
        ball
    }

    pub fn balls(&self) -> &[HitBall] {
        &self.balls
    }

    pub fn ball_mut(&mut self, id: BallId) -> Option<&mut HitBall> {
        self.balls.iter_mut().find(|b| b.id == id)
    }

    pub fn colliders(&self) -> &[Collider] {
        &self.colliders
    }

    /// The colliders the quadtree holds, in its order (VPX's `GetHitObjects()`),
    /// as indices into `colliders`.
    pub fn tree_order(&self) -> impl Iterator<Item = u32> + '_ {
        self.quadtree.items().iter().map(|i| i.id)
    }

    /// Whether building the quadtree ran out of nodes: VPX builds large
    /// subtrees on worker threads, so where nodes run out its own tree can
    /// differ from run to run.
    pub fn quadtree_pool_ran_out(&self) -> bool {
        self.quadtree.pool_ran_out()
    }

    /// A collider, by index, to drive it (a flipper's coil) or change it.
    pub fn collider_mut(&mut self, index: u32) -> &mut Collider {
        &mut self.colliders[index as usize]
    }

    /// A collider and a ball at once, for collisions handled at the engine level.
    pub(crate) fn collider_and_ball(&mut self, index: u32, ball: BallId) -> Option<(&mut Collider, &mut HitBall)> {
        let b = self.balls.iter_mut().find(|b| b.id == ball)?;
        Some((&mut self.colliders[index as usize], b))
    }

    /// When a plunger last touched a ball, in simulated ms.
    pub fn last_plunger_hit(&self) -> u32 {
        self.last_plunger_hit
    }

    /// Simulated time in milliseconds.
    pub fn time_msec(&self) -> u32 {
        self.time_msec
    }

    pub fn time_sec(&self) -> f64 {
        self.time_sec
    }

    /// VPX's `StartPhysics`, with the current time in microseconds.
    pub fn start_physics(&mut self, now_usec: u64) {
        self.start_time_usec = now_usec;
        self.cur_physics_frame_time = now_usec;
        self.next_physics_frame_time = now_usec + u64::from(PHYSICS_STEPTIME);
    }

    pub fn start_time_usec(&self) -> u64 {
        self.start_time_usec
    }

    /// Play goes on after a pause at `now_usec`: the whole game shifted forward in time to it, so
    /// the pause never reaches the physics or the script's timers, as VPX's `UpdatePhysics` shifts
    /// it when play resumes (`m_noTimeCorrect`).
    pub fn shift_clock_to(&mut self, now_usec: u64) {
        let cur_physics_frame_time = self.start_time_usec + (self.time_sec * 1000000.0) as u64;
        if now_usec > cur_physics_frame_time {
            let time_shift = now_usec - cur_physics_frame_time;
            self.start_time_usec += time_shift;
            self.next_physics_frame_time += time_shift;
            self.cur_physics_frame_time += time_shift;
        }
    }

    /// When the next 1 ms step ends, in the clock's microseconds: the
    /// physics runs it once the clock passes this.
    pub fn next_step_time_usec(&self) -> u64 {
        self.next_physics_frame_time
    }

    /// VPX's `UpdatePhysics` without a player: runs every whole 1 ms step
    /// that ends before `target_time_usec`, delivering events to `host`.
    pub fn update_physics(&mut self, target_time_usec: u64, host: &mut dyn PhysicsHost) {
        let initial_time_usec = target_time_usec;

        // loop here until physics (=simulated) time catches up to current real time, still staying
        // behind real time by up to one physics emulation step
        while self.next_physics_frame_time < initial_time_usec {
            let frame_time = (self.cur_physics_frame_time - self.start_time_usec) as f64 / 1000000.0;
            if self.time_sec < frame_time {
                self.time_sec = frame_time; // First iteration is done before precise time
            }
            self.time_msec = ((self.cur_physics_frame_time - self.start_time_usec) / 1000) as u32; // Get time in milliseconds for timers
            self.env.time_msec = self.time_msec;

            self.phys_iterations += 1;

            host.step_begin(self); // the script's timers

            // the cabinet moves after the timers, before the movers; then the plunger's sensor reads
            if let Some(n) = &mut self.nudge {
                n.step_one_millisecond();
                self.env.cabinet_acceleration = n.cabinet_acceleration();
            }
            if let Some(s) = &self.env.plunger_sensor {
                s.step_one_millisecond();
            }
            // the tilt bob swings with the cabinet, whatever nudged it
            if let Some(p) = &mut self.plumb {
                p.step_one_millisecond(self.env.cabinet_acceleration);
            }

            let physics_diff_time = ((self.next_physics_frame_time - self.cur_physics_frame_time) as f64
                * (1.0 / f64::from(DEFAULT_STEPTIME))) as f32;

            // always on integral physics frame boundary (spinner, gate, flipper, plunger, ball)
            for i in 0..self.movers.len() {
                match self.movers[i] {
                    Mover::Ball(id) => {
                        let env = &self.env;
                        if let Some(b) = self.balls.iter_mut().find(|b| b.id == id) {
                            b.update_velocities(env);
                        }
                    }
                    Mover::Collider(k) => {
                        let Self { colliders, env, .. } = self;
                        match &mut colliders[k as usize].shape {
                            Shape::Flipper(f) => f.mover.update_velocities(),
                            Shape::Gate(g) => g.mover.update_velocities(),
                            Shape::Spinner(s) => s.mover.update_velocities(),
                            Shape::Plunger(p) => {
                                let sensor = env.plunger_sensor.clone();
                                p.mover.update_velocities(p.part, sensor.as_deref(), env);
                            }
                            _ => {}
                        }
                    }
                }
            }

            self.deliver(host); // an auto plunger's launch key

            // primary physics loop
            self.physics_simulate_cycle(physics_diff_time, host);

            // for the UI: did a plunger touch a ball this step? (VPX notes it from the hit test, at this same time)
            for &k in &self.plungers {
                if let Shape::Plunger(p) = &self.colliders[k as usize].shape
                    && p.touched.replace(false)
                {
                    self.last_plunger_hit = self.time_msec;
                }
            }

            // Store new position of balls for ball trails
            for b in &mut self.balls {
                b.on_physic_step_processed(u64::from(self.time_msec));
            }

            self.cur_physics_frame_time = self.next_physics_frame_time; // new cycle, on physics frame boundary
            self.next_physics_frame_time += u64::from(PHYSICS_STEPTIME); // advance physics position
        }

        // The physics is emulated by PHYSICS_STEPTIME, but the overall emulation time is more precise
        self.time_sec = (initial_time_usec - self.start_time_usec) as f64 / 1000000.0;
    }

    /// Hands the events raised so far to the host, in order.
    pub(crate) fn deliver(&mut self, host: &mut dyn PhysicsHost) {
        while !self.env.events.is_empty() {
            let events = std::mem::take(&mut self.env.events);
            for event in events {
                host.event(self, event);
            }
        }
    }

    /// VPX's `PhysicsSimulateCycle`: continuous collision detection over `dtime`.
    fn physics_simulate_cycle(&mut self, mut dtime: f32, host: &mut dyn PhysicsHost) {
        let mut static_cnts = STATICCNTS; // maximum number of static counts

        // it's okay to have this code outside of the inner loop, as the ball hitrects already include
        // the maximum distance they can travel in that timespan
        for b in &mut self.balls {
            b.calc_hit_bbox();
        }
        {
            let balls = &self.balls;
            self.kd.update(|id| balls.iter().find(|b| b.id.0 == id).map_or_else(Default::default, |b| b.base.hit_bbox));
        }

        while dtime > 0.0 {
            // first find hits, if any
            let mut hittime = dtime; // begin time search from now ...  until delta ends

            // find earliest time where a flipper collides with its stop
            for &k in &self.flippers {
                if let Shape::Flipper(f) = &self.colliders[k as usize].shape {
                    let fliphit = f.mover.get_hit_time();
                    if fliphit > 0.0 && fliphit <= hittime {
                        // !! >= 0.f causes infinite loop
                        hittime = fliphit;
                    }
                }
            }

            self.contacts.clear();

            for i in 0..self.balls.len() {
                if self.balls[i].d.locked_in_kicker {
                    continue; // don't play with frozen balls
                }
                self.balls[i].coll.hittime = hittime; // search upto current hittime
                self.balls[i].coll.obj = None;

                let mut coll = self.balls[i].coll;
                {
                    let Self { colliders, balls, contacts, env, quadtree, kd, implicit_playfield_mesh, .. } = self;
                    let ball = &balls[i];
                    let sensor = env.plunger_sensor.as_deref();
                    let mut test = |obj: HitRef, coll: &mut CollisionEvent| {
                        do_hit_test(colliders, balls, &env.dropped_targets, sensor, ball, obj, coll, contacts);
                    };

                    // always check for playfield and top glass
                    if *implicit_playfield_mesh {
                        test(HitRef::Collider(PLAYFIELD), &mut coll);
                    }
                    test(HitRef::Collider(TOP_GLASS), &mut coll);

                    let query =
                        BallQuery { bbox: ball.base.hit_bbox, pos: ball.d.pos, rsqr: ball.hit_radius_sqr() };
                    let (dropped, uncollidable) = (&env.dropped_targets, &env.uncollidable_primitives);
                    let active = |o: Owner| match o.kind {
                        OwnerKind::Primitive => !uncollidable.contains(&PartId(o.id)),
                        OwnerKind::HitTarget => !dropped.contains(&PartId(o.id)),
                        OwnerKind::Other => true,
                    };
                    // swap order of dynamic and static obj checks randomly
                    if env.rng.rand_01() < 0.5 {
                        kd.hit_test_ball(&query, ball.id.0, &mut env.rng, |id| test(HitRef::Ball(BallId(id)), &mut coll));
                        quadtree.hit_test_ball(&query, &mut env.rng, active, |id| test(HitRef::Collider(id), &mut coll));
                    } else {
                        quadtree.hit_test_ball(&query, &mut env.rng, active, |id| test(HitRef::Collider(id), &mut coll));
                        kd.hit_test_ball(&query, ball.id.0, &mut env.rng, |id| test(HitRef::Ball(BallId(id)), &mut coll));
                    }
                }
                let b = &mut self.balls[i];
                b.coll = coll;

                let htz = b.coll.hittime; // this ball's hit time
                if htz < 0.0 {
                    b.coll.obj = None; // no negative time allowed
                }

                if b.coll.obj.is_some() && htz <= hittime {
                    hittime = htz; // record actual event time
                    if hittime < STATICTIME {
                        // less than static time interval
                        static_cnts -= 1;
                        if static_cnts < 0 {
                            static_cnts = 0; // keep from wrapping
                            hittime = STATICTIME;
                        }
                    }
                }
            } // end loop over all balls

            // hittime now set ... or full frame if no hit
            // now update displacements to collide-contact or end of physics frame

            if hittime > STATICTIME {
                static_cnts = STATICCNTS; // allow more zeros next round
            }

            // (by index, as VPX: a script run by a stop event may add or remove balls)
            let mut i = 0;
            while i < self.movers.len() {
                match self.movers[i] {
                    Mover::Ball(id) => {
                        if let Some(b) = self.balls.iter_mut().find(|b| b.id == id) {
                            b.update_displacements(hittime); // move the objects about according to velocities
                        }
                    }
                    Mover::Collider(k) => self.update_mover_displacements(k, hittime, host),
                }
                i += 1;
            }

            // find balls that need to be collided and script'ed (generally there will be one, but more are possible)
            let mut i = 0;
            while i < self.balls.len() {
                // use m_vballs.size(), in case script deletes a ball
                let coll = self.balls[i].coll;
                let id = self.balls[i].id;
                let Some(obj) = coll.obj.filter(|_| coll.hittime <= hittime) else {
                    i += 1;
                    continue;
                };
                // now collision, contact and script reactions on active ball (object)
                host.active_ball(id); // For script that wants the ball doing the collision
                self.collide(id, obj, &coll, host);
                // remove trial hit object pointer (VPX frees a destroyed ball only later)
                if let Some(b) = self.ball_mut(id) {
                    b.coll.obj = None;
                }
                if i < self.balls.len() && self.balls[i].id == id {
                    i += 1;
                } // else the collision script deleted the ball: the next one moved into its place
            }

            // Now handle contacts.
            // At this point UpdateDisplacements() was already called, so the state is different from that
            // at HitTest(). However, contacts have zero relative velocity, so hopefully nothing
            // catastrophic has happened in the meanwhile.
            let forwards = self.env.rng.rand_01() < 0.5; // swap order of contact handling randomly
            let n = self.contacts.len();
            for k in 0..n {
                let c = self.contacts[if forwards { k } else { n - 1 - k }];
                self.contact(&c, hittime);
            }
            self.contacts.clear();

            // C_BALL_SPIN_HACK: hacky killing of ball spin on resting balls (very low and very high spinning)
            let time_msec = self.time_msec;
            for b in &mut self.balls {
                if b.coll.hitdistance >= PHYS_TOUCH as f32 {
                    continue;
                }
                let old_pos0 = *b.get_old_position(u64::from(time_msec.wrapping_sub(90))); // Position 90ms ago
                if old_pos0.x == f32::MAX {
                    continue;
                }
                let old_pos1 = *b.get_old_position(u64::from(time_msec.wrapping_sub(80))); // Position 80ms ago
                if old_pos1.x == f32::MAX {
                    continue;
                }
                let diff_pos = old_pos0 - b.d.pos;
                let mag = diff_pos.x * diff_pos.x + diff_pos.y * diff_pos.y; // squared distance in the playfield plane, since 90ms
                let diff_pos2 = old_pos1 - b.d.pos;
                let mag2 = diff_pos2.x * diff_pos2.x + diff_pos2.y * diff_pos2.y; // squared distance in the playfield plane, since 80ms

                let am = b.angularmomentum;
                let threshold = (am.x * am.x + am.y * am.y) / max(mag, mag2);
                if !inf_nan(threshold) && threshold > 666.0 {
                    // do not kill spin completely, otherwise stuck balls will happen during regular gameplay
                    let damp = clamp(1.0 - (threshold - 666.0) / 10000.0, 0.23, 1.0);
                    b.angularmomentum *= damp;
                }
            }

            dtime -= hittime; // new delta .. i.e. time remaining

            self.env.swap_ball_collision_handling = !self.env.swap_ball_collision_handling; // swap order of ball-ball collisions
        } // end physics loop
    }

    /// Resolves one collision now, as the physics cycle does (VPX's
    /// `pho->Collide(pball->m_coll)`): `ball` hit `obj`, as `coll` describes.
    /// Events run through `host` where VPX runs them.
    pub fn collide(&mut self, ball: BallId, obj: HitRef, coll: &CollisionEvent, host: &mut dyn PhysicsHost) {
        match obj {
            HitRef::Collider(c) if matches!(self.colliders[c as usize].shape, Shape::Kicker(_)) => {
                self.kicker_do_collide(c, ball, coll.hitnormal, coll.hitflag, false, host);
            }
            HitRef::Collider(c) => {
                let Some(i) = self.balls.iter().position(|b| b.id == ball) else { return };
                let Self { colliders, balls, env, .. } = self;
                colliders[c as usize].collide(coll, &mut balls[i], env);
                self.deliver(host); // the collision's events run before VPX's Collide returns
            }
            HitRef::Ball(other) => self.collide_balls(other, ball, coll, host),
        }
    }

    /// Moves one moving part (flipper, gate, spinner) on by `dtime`, as the
    /// physics cycle does (VPX's `UpdateDisplacements`), with its events.
    pub fn update_mover_displacements(&mut self, collider: u32, dtime: f32, host: &mut dyn PhysicsHost) {
        let k = collider;
        match &mut self.colliders[k as usize].shape {
            Shape::Flipper(f) => {
                let part = f.part;
                if f.mover.update_displacements(dtime, part, &mut self.env) {
                    self.deliver(host); // the stop's event runs here in VPX
                    if let Shape::Flipper(f) = &mut self.colliders[k as usize].shape {
                        f.mover.finish_stop();
                    }
                }
            }
            Shape::Gate(_) => self.gate_update_displacements(k, dtime, host),
            Shape::Spinner(_) => self.spinner_update_displacements(k, dtime, host),
            Shape::Plunger(p) => {
                let part = p.part;
                p.mover.update_displacements(dtime, part, &mut self.env);
                self.deliver(host); // a stroke event runs here in VPX, before the tip's segments move
                if let Shape::Plunger(p) = &mut self.colliders[k as usize].shape {
                    p.mover.finish_update();
                }
            }
            _ => {}
        }
    }

    /// VPX's `HitBall::Collide` on the ball hit (`this_id`), with the ball that
    /// found the collision (`pball_id`), running the table's ball-to-ball
    /// callback where VPX does, between the two halves.
    fn collide_balls(&mut self, this_id: BallId, pball_id: BallId, coll: &CollisionEvent, host: &mut dyn PhysicsHost) {
        let dot = {
            let (Some(this_ball), Some(pball)) =
                (self.balls.iter().find(|b| b.id == this_id), self.balls.iter().find(|b| b.id == pball_id))
            else {
                return;
            };
            match HitBall::collide_ball_begin(this_ball, pball, coll, &self.env) {
                Some(dot) => dot,
                None => return,
            }
        };
        if dot < -0.25 {
            // only collisions with at least some small true impact velocity (no contacts)
            self.env.events.push(PhysicsEvent::BallBallCollision { ball: this_id, other: pball_id, speed: -dot });
            self.deliver(host);
        }
        // the script may have moved, kicked or destroyed either ball (VPX would go on with a destroyed
        // ball's last state; here the collision ends with it)
        let (Some(j), Some(i)) =
            (self.balls.iter().position(|b| b.id == this_id), self.balls.iter().position(|b| b.id == pball_id))
        else {
            return;
        };
        let (this_ball, pball) = pair_mut(&mut self.balls, j, i);
        HitBall::collide_ball_finish(this_ball, pball, coll, dot);
    }

    /// A recorded contact: VPX's `m_obj->Contact(coll, dtime)`.
    fn contact(&mut self, c: &CollisionEvent, dtime: f32) {
        let (Some(ball), Some(obj)) = (c.ball, c.obj) else { return };
        let Some(i) = self.balls.iter().position(|b| b.id == ball) else { return };
        match obj {
            HitRef::Collider(k) => {
                let Self { colliders, balls, env, .. } = self;
                colliders[k as usize].contact(c, &mut balls[i], dtime, env);
            }
            HitRef::Ball(_) => {} // balls do nothing on contact
        }
    }
}

/// VPX's `DoHitTest`: runs one hit test and keeps the result if it comes no
/// later than the current candidate. Contacts are recorded separately.
#[allow(clippy::too_many_arguments)]
fn do_hit_test(
    colliders: &[Collider],
    balls: &[HitBall],
    dropped_targets: &PartSet,
    sensor: Option<&dyn PlungerSensor>,
    ball: &HitBall,
    obj: HitRef,
    coll: &mut CollisionEvent,
    contacts: &mut Vec<CollisionEvent>,
) {
    let mut new_coll = CollisionEvent::default();
    let newtime = match obj {
        HitRef::Collider(k) => {
            let c = &colliders[k as usize];
            // !! why is this done here and not in corresponding HitTest()?
            if c.base.obj_type == ObjType::HitTarget && c.base.obj.is_some_and(|p| dropped_targets.contains(&p)) {
                return;
            }
            c.hit_test(&ball.d, coll.hittime, &mut new_coll, sensor)
        }
        HitRef::Ball(id) => match balls.iter().find(|b| b.id == id) {
            Some(other) => other.hit_test(&ball.d, coll.hittime, &mut new_coll),
            None => return,
        },
    };
    do_hit_test_accept(ball.id, obj, newtime, new_coll, coll, Some(contacts));
}

/// Two different elements of a slice, both mutable.
fn pair_mut<T>(v: &mut [T], a: usize, b: usize) -> (&mut T, &mut T) {
    assert_ne!(a, b);
    if a < b {
        let (lo, hi) = v.split_at_mut(b);
        (&mut lo[a], &mut hi[0])
    } else {
        let (lo, hi) = v.split_at_mut(a);
        (&mut hi[0], &mut lo[b])
    }
}
