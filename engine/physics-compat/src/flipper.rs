// Ported from Visual Pinball X: src/physics/hitflipper.cpp and hitflipper.h (license:GPLv3+),
// and the physics parts of src/parts/flipper.cpp (PhysicSetup, RotateToEnd, RotateToStart),
// commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's flipper: a rigid bat turning about its base, driven by a coil model
//! (strength, return strength, ramp-up, hold-coil damping near the end of the
//! stroke), with collisions against its two faces, its rounded end and its
//! base, and resting contact for cradled balls.

use crate::collide::{CollisionEvent, HitCircle, HitObjectBase, elasticity_with_falloff};
use crate::env::{PartId, PhysicsEnv, PhysicsEvent};
use crate::hitball::{BallS, HitBall};
use crate::math::def::{ang_to_rad, clamp, inf_nan, max, min, rad_to_ang, sqrf, std_max, std_min};
use crate::math::{Vertex2D, Vertex3Ds, cross_product};
use crate::physconst::{
    C_CONTACTVEL, C_DISP_GAIN, C_DISP_LIMIT, C_EMBEDDED, C_EMBEDSHOT, C_INTERATIONS, C_LOWNORMVEL, C_PRECISION,
    C_TOL_ENDPNTS, PHYS_FACTOR, PHYS_TOUCH,
};
use std::cell::Cell;

/// The flipper part's physics settings (VPX `Flipper::m_d`), which VPX reads
/// each time it needs one: scripts can change them during play.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlipperData {
    /// The override values apply: the part has a physics set
    /// (`m_d.m_OverridePhysics`), or the table imposes one on all flippers
    /// (`m_overridePhysicsFlipper && m_overridePhysics`).
    pub use_override: bool,
    pub mass: f32,
    pub override_mass: f32,
    pub strength: f32,
    pub override_strength: f32,
    /// `m_return`.
    pub return_strength: f32,
    pub override_return_strength: f32,
    pub torque_damping: f32,
    pub override_torque_damping: f32,
    /// In degrees.
    pub torque_damping_angle: f32,
    pub override_torque_damping_angle: f32,
    /// `m_rampUp`.
    pub ramp_up: f32,
    pub override_coil_ramp_up: f32,
    pub elasticity: f32,
    pub override_elasticity: f32,
    pub elasticity_falloff: f32,
    pub override_elasticity_falloff: f32,
    pub friction: f32,
    pub override_friction: f32,
    /// In degrees.
    pub scatter: f32,
    pub override_scatter_angle: f32,
}

impl FlipperData {
    #[inline(always)]
    fn pick(&self, value: f32, override_value: f32) -> f32 {
        if self.use_override { override_value } else { value }
    }
}

/// VPX's `FlipperMoverObject`: the flipper's motion.
#[derive(Clone, Debug)]
pub struct FlipperMover {
    /// The part's settings (VPX reaches them through `m_pflipper`).
    pub data: FlipperData,
    /// The base section, with its own hit fields (its z range).
    pub hitcircle_base: HitCircle,
    pub hitcircle_base_hit: HitObjectBase,
    pub endradius: f32,
    /// Centre-to-centre length, base to end.
    pub flipperradius: f32,

    // kinematic state
    pub angular_momentum: f32,
    pub angular_acceleration: f32,
    pub angle_speed: f32,
    pub angle_cur: f32,

    pub cur_torque: f32,
    pub contact_torque: f32,

    pub angle_start: f32,
    pub angle_end: f32,

    /// Moment of inertia.
    pub inertia: f32,

    /// Base face normals at zero degrees.
    pub zero_ang_norm: Vertex2D,

    /// -1, 0 or 1: which end-of-stroke event the next stop fires.
    pub enable_rotate_event: i16,

    pub direction: bool,

    /// Is the solenoid on?
    pub sol_state: bool,
    pub is_in_contact: bool,

    pub enabled: bool,
    /// Which face was hit last, tested first next time. VPX changes it from
    /// inside the (const) hit test.
    pub last_hit_face: Cell<bool>,
}

impl FlipperMover {
    /// VPX's constructor. `zlow`/`zhigh` is the flipper's height range.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        data: FlipperData,
        center: Vertex2D,
        baser: f32,
        endr: f32,
        flipr: f32,
        angle_start: f32,
        mut angle_end: f32,
        zlow: f32,
        zhigh: f32,
    ) -> Self {
        if angle_end == angle_start {
            angle_end += 0.0001; // otherwise hangs forever in collisions/updates
        }

        let ratio = (baser - endr) / flipr;

        // model inertia of flipper as that of rod of length flipper around its end
        let mass = data.pick(data.mass, data.override_mass);
        let inertia = (1.0_f64 / 3.0) as f32 * mass * (flipr * flipr);

        Self {
            data,
            hitcircle_base: HitCircle::new(center, baser),
            hitcircle_base_hit: HitObjectBase::with_z(zlow, zhigh),
            endradius: endr,
            flipperradius: flipr,
            angular_momentum: 0.0,
            angular_acceleration: 0.0,
            angle_speed: 0.0,
            angle_cur: angle_start,
            cur_torque: 0.0,
            contact_torque: 0.0, // VPX leaves it unset until the first contact with a stopper
            angle_start,
            angle_end,
            inertia,
            zero_ang_norm: Vertex2D::new(
                (1.0 - ratio * ratio).sqrt(), // F2 Norm, used in Green's transform, in FPM time search
                -ratio,                       // F1 norm, change sign of x component, i.e -zeroAngNorm.x
            ),
            enable_rotate_event: 0,
            direction: angle_end >= angle_start,
            sol_state: false,
            is_in_contact: false,
            enabled: true,
            last_hit_face: Cell::new(false), // used to optimize hit face search order
        }
    }

    pub fn set_start_angle(&mut self, r: f32) {
        self.angle_start = r;
        let angle_min = min(self.angle_start, self.angle_end);
        let angle_max = max(self.angle_start, self.angle_end);
        if self.angle_cur > angle_max {
            self.angle_cur = angle_max;
        }
        if self.angle_cur < angle_min {
            self.angle_cur = angle_min;
        }
    }

    pub fn set_end_angle(&mut self, r: f32) {
        self.angle_end = r;
        let angle_min = min(self.angle_start, self.angle_end);
        let angle_max = max(self.angle_start, self.angle_end);
        if self.angle_cur > angle_max {
            self.angle_cur = angle_max;
        }
        if self.angle_cur < angle_min {
            self.angle_cur = angle_min;
        }
    }

    pub fn get_return_ratio(&self) -> f32 {
        self.data.pick(self.data.return_strength, self.data.override_return_strength)
    }

    pub fn get_strength(&self) -> f32 {
        self.data.pick(self.data.strength, self.data.override_strength)
    }

    pub fn get_mass(&self) -> f32 {
        3.0 * self.inertia / (self.flipperradius * self.flipperradius)
    }

    pub fn set_mass(&mut self, m: f32) {
        self.inertia = (1.0_f64 / 3.0) as f32 * m * (self.flipperradius * self.flipperradius);
    }

    /// VPX's `UpdateDisplacements`: turns the flipper; at a stop, it bounces
    /// back and raises the end-of-stroke or park event the script asked for.
    ///
    /// Returns true when the flipper hit a stop. VPX runs the event's script
    /// there, then clears the request: the caller delivers the event and then
    /// calls [`Self::finish_stop`].
    pub fn update_displacements(&mut self, dtime: f32, part: PartId, env: &mut PhysicsEnv) -> bool {
        self.angle_cur += self.angle_speed * dtime; // move flipper angle

        let angle_min = min(self.angle_start, self.angle_end);
        let angle_max = max(self.angle_start, self.angle_end);

        if self.angle_cur > angle_max {
            self.angle_cur = angle_max;
        }
        if self.angle_cur < angle_min {
            self.angle_cur = angle_min;
        }

        if self.angle_speed.abs() < 0.0005 {
            // avoids 'jumping balls' when two or more balls held on flipper (and more other balls are in play)
            return false;
        }

        let mut handle_event = false;

        if self.angle_cur == angle_max {
            // hit stop?
            if self.angle_speed > 0.0 {
                handle_event = true;
            }
        } else if self.angle_cur == angle_min && self.angle_speed < 0.0 {
            handle_event = true;
        }

        if handle_event {
            let anglespd = rad_to_ang(self.angle_speed).abs();
            self.angular_momentum *= -0.3; // !! make configurable?
            self.angle_speed = self.angular_momentum / self.inertia;
            if self.enable_rotate_event > 0 {
                env.events.push(PhysicsEvent::LimitEos { part, speed: anglespd }); // send EOS event
            } else if self.enable_rotate_event < 0 {
                env.events.push(PhysicsEvent::LimitBos { part, speed: anglespd }); // send Beginning of Stroke/Park event
            }
        }
        handle_event
    }

    /// The end of VPX's `UpdateDisplacements` after a stop, once the stop's
    /// event has run: the script's request for an event is used up.
    pub fn finish_stop(&mut self) {
        self.enable_rotate_event = 0;
    }

    /// VPX's `UpdateVelocities`: the coil model, once per step.
    pub fn update_velocities(&mut self) {
        let d = &self.data;
        let mut desired_torque = self.get_strength();
        if !self.sol_state {
            // m_solState: true = button pressed, false = released
            desired_torque *= -self.get_return_ratio();
        }

        // hold coil is weaker
        let eos_angle = ang_to_rad(d.pick(d.torque_damping_angle, d.override_torque_damping_angle));
        if (self.angle_cur - self.angle_end).abs() < eos_angle {
            // fade in/out damping, depending on angle to end
            let lerp = sqrf(sqrf((self.angle_cur - self.angle_end).abs() / eos_angle));
            desired_torque *= lerp + d.pick(d.torque_damping, d.override_torque_damping) * (1.0 - lerp);
        }

        if !self.direction {
            desired_torque = -desired_torque;
        }

        let mut torque_rampup_speed = d.pick(d.ramp_up, d.override_coil_ramp_up);
        if torque_rampup_speed <= 0.0 {
            torque_rampup_speed = 1e6; // set very high for instant coil response
        } else {
            torque_rampup_speed = min(self.get_strength() / torque_rampup_speed, 1e6);
        }

        // update current torque linearly towards desired torque
        // (simple model for coil hysteresis)
        if desired_torque >= self.cur_torque {
            self.cur_torque = std_min(self.cur_torque + torque_rampup_speed * PHYS_FACTOR as f32, desired_torque);
        } else {
            self.cur_torque = std_max(self.cur_torque - torque_rampup_speed * PHYS_FACTOR as f32, desired_torque);
        }

        // resolve contacts with stoppers
        let mut torque = self.cur_torque;
        self.is_in_contact = false;
        if self.angle_speed.abs() <= 1e-2 {
            let angle_min = min(self.angle_start, self.angle_end);
            let angle_max = max(self.angle_start, self.angle_end);

            if self.angle_cur >= angle_max - 1e-2 && torque > 0.0 {
                self.angle_cur = angle_max;
                self.is_in_contact = true;
                self.contact_torque = torque;
                self.angular_momentum = 0.0;
                torque = 0.0;
            } else if self.angle_cur <= angle_min + 1e-2 && torque < 0.0 {
                self.angle_cur = angle_min;
                self.is_in_contact = true;
                self.contact_torque = torque;
                self.angular_momentum = 0.0;
                torque = 0.0;
            }
        }

        self.angular_momentum += PHYS_FACTOR as f32 * torque;
        self.angle_speed = self.angular_momentum / self.inertia;
        self.angular_acceleration = torque / self.inertia;
    }

    pub fn apply_impulse(&mut self, rot_i: &Vertex3Ds) {
        self.angular_momentum += rot_i.z; // only rotation about z axis
        self.angle_speed = self.angular_momentum / self.inertia; // TODO: figure out moment of inertia
    }

    /// True = button pressed, false = released.
    pub fn set_solenoid_state(&mut self, s: bool) {
        self.sol_state = s;
    }

    pub fn get_stroke_ratio(&self) -> f32 {
        (self.angle_cur - self.angle_start) / (self.angle_end - self.angle_start) // End == Start cannot happen, as handled in ctor
    }

    pub fn surface_velocity(&self, surf_p: &Vertex3Ds) -> Vertex3Ds {
        cross_z(self.angle_speed, surf_p)
    }

    pub fn surface_acceleration(&self, surf_p: &Vertex3Ds) -> Vertex3Ds {
        // tangential acceleration = (0, 0, omega) x surfP
        let tang_acc = cross_z(self.angular_acceleration, surf_p);

        // centripetal acceleration = (0,0,omega) x ( (0,0,omega) x surfP )
        let av2 = self.angle_speed * self.angle_speed;
        let centr_acc = Vertex3Ds::new(-av2 * surf_p.x, -av2 * surf_p.y, 0.0);

        tang_acc + centr_acc
    }

    /// When the flipper reaches its stop, or -1.
    pub fn get_hit_time(&self) -> f32 {
        if self.angle_speed == 0.0 {
            return -1.0;
        }

        let angle_min = min(self.angle_start, self.angle_end);
        let angle_max = max(self.angle_start, self.angle_end);

        let dist = if self.angle_speed > 0.0 {
            angle_max - self.angle_cur // >= 0
        } else {
            angle_min - self.angle_cur // <= 0
        };

        let hittime = dist / self.angle_speed;

        if inf_nan(hittime) || hittime < 0.0 { -1.0 } else { hittime }
    }
}

/// The cross product (0, 0, rz) x v.
#[inline(always)]
fn cross_z(rz: f32, v: &Vertex3Ds) -> Vertex3Ds {
    Vertex3Ds::new(-rz * v.y, rz * v.x, 0.0)
}

/// VPX's `HitFlipper`.
#[derive(Clone, Debug)]
pub struct HitFlipper {
    pub mover: FlipperMover,
    /// The flipper part, for its events.
    pub part: PartId,
    last_hittime: u32,
}

impl HitFlipper {
    /// VPX's `Flipper::PhysicSetup` and `HitFlipper` constructor: angles in
    /// radians; `height` is the surface the flipper stands on, `flipper_height`
    /// its own height. Radii are at least 0.01, as VPX makes them. Returns the
    /// flipper and its hit fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        part: PartId,
        data: FlipperData,
        center: Vertex2D,
        baser: f32,
        endr: f32,
        flipr: f32,
        angle_start: f32,
        angle_end: f32,
        zlow: f32,
        zhigh: f32,
        enabled: bool,
    ) -> (Self, HitObjectBase) {
        let mut mover = FlipperMover::new(data, center, baser, endr, flipr, angle_start, angle_end, zlow, zhigh);
        mover.enabled = enabled;
        let mut base = HitObjectBase::default();
        Self::update_physics_from_flipper(&mover.data, &mut base);
        (Self { mover, part, last_hittime: 0 }, base)
    }

    /// VPX's `UpdatePhysicsFromFlipper`: the hit fields from the part's settings.
    pub fn update_physics_from_flipper(d: &FlipperData, base: &mut HitObjectBase) {
        base.elasticity_falloff = d.pick(d.elasticity_falloff, d.override_elasticity_falloff);
        base.elasticity = d.pick(d.elasticity, d.override_elasticity);
        base.friction = d.pick(d.friction, d.override_friction);
        base.scatter = ang_to_rad(d.pick(d.scatter, d.override_scatter_angle));
    }

    /// VPX's `Flipper::RotateToEnd`: the coil is energized.
    pub fn rotate_to_end(&mut self) {
        self.mover.enable_rotate_event = 1;
        self.mover.set_solenoid_state(true);
    }

    /// VPX's `Flipper::RotateToStart`: the coil is released.
    pub fn rotate_to_start(&mut self) {
        self.mover.enable_rotate_event = -1;
        self.mover.set_solenoid_state(false);
    }

    /// VPX's `CalcHitBBox` (its "old, simple version", which VPX uses).
    pub fn calc_hit_bbox(&self, base: &mut HitObjectBase) {
        let m = &self.mover;
        // Allow roundoff
        let reach = m.flipperradius + m.endradius + 0.1;
        base.hit_bbox.left = m.hitcircle_base.center.x - reach;
        base.hit_bbox.right = m.hitcircle_base.center.x + reach;
        base.hit_bbox.top = m.hitcircle_base.center.y - reach;
        base.hit_bbox.bottom = m.hitcircle_base.center.y + reach;
        base.hit_bbox.zlow = m.hitcircle_base_hit.hit_bbox.zlow;
        base.hit_bbox.zhigh = m.hitcircle_base_hit.hit_bbox.zhigh;
    }

    /// VPX's `HitFlipper::HitTest`: the faces first (the last one hit before
    /// the other), then the end, then the base.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        if !self.mover.enabled {
            return -1.0;
        }

        let lastface = self.mover.last_hit_face.get();

        // for effective computing, adding a last face hit value to speed calculations
        //  a ball can only hit one face never two
        // also if a ball hits a face then it can not hit either radius
        // so only check these if a face is not hit
        // endRadius is more likely than baseRadius ... so check it first

        let mut hittime = self.hit_test_flipper_face(base, ball, dtime, coll, lastface); // first face
        if hittime >= 0.0 {
            return hittime;
        }

        hittime = self.hit_test_flipper_face(base, ball, dtime, coll, !lastface); // second face
        if hittime >= 0.0 {
            self.mover.last_hit_face.set(!lastface); // change this face to check first // HACK
            return hittime;
        }

        hittime = self.hit_test_flipper_end(base, ball, dtime, coll); // end radius
        if hittime >= 0.0 {
            return hittime;
        }

        hittime = self.mover.hitcircle_base.hit_test(&self.mover.hitcircle_base_hit, ball, dtime, coll);
        if hittime >= 0.0 {
            coll.hitvel.x = 0.0; // Tangent velocity of contact point (rotate Normal right)
            coll.hitvel.y = 0.0; // units: rad*d/t (Radians*diameter/time
            coll.hitmoment_bit = true;
            hittime
        } else {
            -1.0 // no hits
        }
    }

    /// VPX's `HitTestFlipperEnd`: the rounded end, by a modified false
    /// position search over the step.
    pub fn hit_test_flipper_end(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        let m = &self.mover;
        let angle_cur = m.angle_cur;
        let mut anglespeed = m.angle_speed; // rotation rate

        let flipperbase = m.hitcircle_base.center;

        let angle_min = min(m.angle_start, m.angle_end);
        let angle_max = max(m.angle_start, m.angle_end);

        let fe_radius = m.endradius;

        let ballr_endr = fe_radius + ball.radius; // magnititude of (ball - flipperEnd)

        let ballx = ball.pos.x;
        let bally = ball.pos.y;

        let ballvx = ball.vel.x;
        let ballvy = ball.vel.y;

        let vp = Vertex2D::new(
            0.0,               // m_flipperradius*sin(0));
            -m.flipperradius, // m_flipperradius*(-cos(0));
        );

        // new ball position at time t in flipper face coordinate
        let (mut ballvtx, mut ballvty) = (0.0f32, 0.0f32);
        let mut contact_ang = 0.0f32;
        let (mut bfend, mut cbcedist) = (0.0f32, 0.0f32);
        // Modified False Position control (VPX leaves them unset until first used)
        let (mut t0, mut t1, mut d0, mut d1, mut dp) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);

        let mut t = 0.0f32; // start first interval ++++++++++++++++++++++++++
        let mut k = 1;
        while k <= C_INTERATIONS {
            // determine flipper rotation direction, limits and parking

            contact_ang = angle_cur + anglespeed * t; // angle at time t

            if contact_ang >= angle_max {
                contact_ang = angle_max; // stop here
            } else if contact_ang <= angle_min {
                contact_ang = angle_min; // stop here
            }

            let radsin = contact_ang.sin(); // Green's transform matrix... rotate angle delta
            let radcos = contact_ang.cos(); // rotational transform from zero position to position at time t

            // rotate angle delta unit vector, rotates system according to flipper face angle
            let vt = Vertex2D::new(
                vp.x * radcos - vp.y * radsin + flipperbase.x, // rotate and translate to world position
                vp.y * radcos + vp.x * radsin + flipperbase.y,
            );

            ballvtx = ballx + ballvx * t - vt.x; // new ball position relative to flipper end radius
            ballvty = bally + ballvy * t - vt.y;

            cbcedist = (ballvtx * ballvtx + ballvty * ballvty).sqrt(); // center ball to center end radius distance

            bfend = cbcedist - ballr_endr; // ball face-to-radius surface distance

            if bfend.abs() <= C_PRECISION {
                break;
            }

            if k == 1 {
                // end of pass one ... set full interval pass, t = dtime
                // test for extreme conditions
                if bfend < -(ball.radius + fe_radius) {
                    return -1.0; // too deeply embedded, ambigious position
                }
                if bfend <= PHYS_TOUCH as f32 {
                    break; // inside the clearance limits
                }

                t0 = dtime;
                t1 = dtime;
                d0 = 0.0;
                d1 = bfend; // set for second pass, force t=dtime
            } else if k == 2 {
                // end pass two, check if zero crossing on initial interval, exit if none
                if dp * bfend > 0.0 {
                    return -1.0; // no solution ... no obvious zero crossing
                }

                t0 = 0.0;
                t1 = dtime;
                d0 = dp;
                d1 = bfend; // set initial boundaries
            } else {
                // (k >= 3) // MFP root search
                if bfend * d0 <= 0.0 {
                    // zero crossing
                    t1 = t;
                    d1 = bfend;
                    if dp * bfend > 0.0 {
                        d0 *= 0.5;
                    }
                } else {
                    // move left interval limit
                    t0 = t;
                    d0 = bfend;
                    if dp * bfend > 0.0 {
                        d1 *= 0.5;
                    }
                }
            }

            t = t0 - d0 * (t1 - t0) / (d1 - d0); // estimate next t
            dp = bfend; // remember
            k += 1;
        }
        // End time interation loop found time t soultion

        if inf_nan(t) || t < 0.0 || t > dtime // time is outside this frame ... no collision
            || ((k > C_INTERATIONS) && (bfend.abs() > ball.radius * 0.25))
        // last ditch effort to accept a solution
        {
            return -1.0; // no solution
        }

        // here ball and flipper end are in contact .. well in most cases, near and embedded solutions need calculations

        let hitz = ball.pos.z + ball.vel.z * t; // check for a hole, relative to ball rolling point at hittime

        if (hitz + ball.radius * 0.5) < base.hit_bbox.zlow // check limits of object's height and depth
            || (hitz - ball.radius * 0.5) > base.hit_bbox.zhigh
        {
            return -1.0;
        }

        // ok we have a confirmed contact, calc the stats, remember there are "near" solution, so all
        // parameters need to be calculated from the actual configuration, i.e. contact radius must be calc'ed

        let inv_cbcedist = 1.0 / cbcedist;
        coll.hitnormal.x = ballvtx * inv_cbcedist; // normal vector from flipper end to ball
        coll.hitnormal.y = ballvty * inv_cbcedist;
        coll.hitnormal.z = 0.0;

        let dist = Vertex2D::new(
            ball.pos.x + ballvx * t - ball.radius * coll.hitnormal.x - m.hitcircle_base.center.x, // vector from base to flipperEnd plus the projected End radius
            ball.pos.y + ballvy * t - ball.radius * coll.hitnormal.y - m.hitcircle_base.center.y,
        );

        let distance = (dist.x * dist.x + dist.y * dist.y).sqrt(); // distance from base center to contact point

        if (contact_ang >= angle_max && anglespeed > 0.0) || (contact_ang <= angle_min && anglespeed < 0.0) {
            // hit limits ???
            anglespeed = 0.0; // rotation stopped
        }

        let inv_distance = 1.0 / distance;
        coll.hitvel.x = -dist.y * inv_distance; // Unit Tangent vector velocity of contact point(rotate normal right)
        coll.hitvel.y = dist.x * inv_distance;

        coll.hitmoment_bit = distance == 0.0;

        // recheck using actual contact angle of velocity direction
        let dv = Vertex2D::new(
            ballvx - coll.hitvel.x * anglespeed * distance,
            ballvy - coll.hitvel.y * anglespeed * distance,
        ); // delta velocity ball to face

        let bnv = dv.x * coll.hitnormal.x + dv.y * coll.hitnormal.y; // dot Normal to delta v

        if bnv >= 0.0 {
            return -1.0; // not hit ... ball is receding from face already, must have been embedded or shallow angled
        }

        coll.is_contact = bnv.abs() <= C_CONTACTVEL && bfend <= PHYS_TOUCH as f32;
        if coll.is_contact {
            coll.hit_org_normalvelocity = bnv;
        }

        coll.hitdistance = bfend; // actual contact distance ..

        t
    }

    /// VPX's `HitTestFlipperFace`: one of the two flat faces.
    pub fn hit_test_flipper_face(
        &self,
        base: &HitObjectBase,
        ball: &BallS,
        dtime: f32,
        coll: &mut CollisionEvent,
        face1: bool,
    ) -> f32 {
        let m = &self.mover;
        let angle_cur = m.angle_cur;
        let mut anglespeed = m.angle_speed; // rotation rate

        let flipperbase = m.hitcircle_base.center;
        let fe_radius = m.endradius;

        let angle_min = min(m.angle_start, m.angle_end);
        let angle_max = max(m.angle_start, m.angle_end);

        let ballvx = ball.vel.x;
        let ballvy = ball.vel.y;

        // flipper positions at zero degrees rotation

        let mut ffnx = m.zero_ang_norm.x; // flipper face normal vector //Face2
        if face1 {
            ffnx = -ffnx; // negative for face1 (left face)
        }

        let ffny = m.zero_ang_norm.y; // norm y component same for either face
        let vp = Vertex2D::new(
            // face segment V1 point
            m.hitcircle_base.radius * ffnx, // face endpoint of line segment on base radius
            m.hitcircle_base.radius * ffny,
        );

        let mut f = Vertex2D::default(); // flipper face normal

        let mut bffnd = 0.0f32; // ball flipper face normal distance (negative for normal side)
        // new ball position at time t in flipper face coordinate
        let (mut ballvtx, mut ballvty) = (0.0f32, 0.0f32);
        let mut contact_ang = 0.0f32;

        // Modified False Position control (VPX leaves them unset until first used)
        let (mut t0, mut t1, mut d0, mut d1, mut dp) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);

        let mut t = 0.0f32; // start first interval ++++++++++++++++++++++++++
        let mut k = 1;
        while k <= C_INTERATIONS {
            // determine flipper rotation direction, limits and parking

            contact_ang = angle_cur + anglespeed * t; // angle at time t

            if contact_ang >= angle_max {
                contact_ang = angle_max; // stop here
            } else if contact_ang <= angle_min {
                contact_ang = angle_min; // stop here
            }

            let radsin = contact_ang.sin(); // Green's transform matrix... rotate angle delta
            let radcos = contact_ang.cos(); // rotational transform from current position to position at time t

            f.x = ffnx * radcos - ffny * radsin; // rotate to time t, norm and face offset point
            f.y = ffny * radcos + ffnx * radsin;

            let vt = Vertex2D::new(
                vp.x * radcos - vp.y * radsin + flipperbase.x, // rotate and translate to world position
                vp.y * radcos + vp.x * radsin + flipperbase.y,
            );

            ballvtx = ball.pos.x + ballvx * t - vt.x; // new ball position relative to rotated line segment endpoint
            ballvty = ball.pos.y + ballvy * t - vt.y;

            bffnd = ballvtx * f.x + ballvty * f.y - ball.radius; // normal distance to segment

            if bffnd.abs() <= C_PRECISION {
                break;
            }

            // loop control, boundary checks, next estimate, etc.

            if k == 1 {
                // end of pass one ... set full interval pass, t = dtime
                // test for already inside flipper plane, either embedded or beyond the face endpoints
                if bffnd < -(ball.radius + fe_radius) {
                    return -1.0; // wrong side of face, or too deeply embedded
                }
                if bffnd <= PHYS_TOUCH as f32 {
                    break; // inside the clearance limits, go check face endpoints
                }

                t0 = dtime;
                t1 = dtime;
                d0 = 0.0;
                d1 = bffnd; // set for second pass, so t=dtime
            } else if k == 2 {
                // end pass two, check if zero crossing on initial interval, exit
                if dp * bffnd > 0.0 {
                    return -1.0; // no solution ... no obvious zero crossing
                }
                t0 = 0.0;
                t1 = dtime;
                d0 = dp;
                d1 = bffnd; // testing MFP estimates
            } else {
                // (k >= 3) // MFP root search
                if bffnd * d0 <= 0.0 {
                    // zero crossing
                    t1 = t;
                    d1 = bffnd;
                    if dp * bffnd > 0.0 {
                        d0 *= 0.5;
                    }
                } else {
                    // move left limits
                    t0 = t;
                    d0 = bffnd;
                    if dp * bffnd > 0.0 {
                        d1 *= 0.5;
                    }
                }
            }

            t = t0 - d0 * (t1 - t0) / (d1 - d0); // next estimate
            dp = bffnd; // remember
            k += 1;
        }

        // End time interation loop found time t solution

        if inf_nan(t) || t < 0.0 || t > dtime // time is outside this frame ... no collision
            || ((k > C_INTERATIONS) && (bffnd.abs() > ball.radius * 0.25))
        // last ditch effort to accept a near solution
        {
            return -1.0; // no solution
        }

        // here ball and flipper face are in contact... past the endpoints, also, don't forget embedded and near solution

        // flipper face tangent: rotate to form Tangent vector
        let tangent = if face1 { Vertex2D::new(-f.y, f.x) } else { Vertex2D::new(f.y, -f.x) };

        let bfftd = ballvtx * tangent.x + ballvty * tangent.y; // ball to flipper face tanget distance

        let len = m.flipperradius * m.zero_ang_norm.x; // face segment length ... e.g. same on either face
        if bfftd < -C_TOL_ENDPNTS || bfftd > len + C_TOL_ENDPNTS {
            return -1.0; // not in range of touching
        }

        let hitz = ball.pos.z + ball.vel.z * t; // check for a hole, relative to ball rolling point at hittime

        if (hitz + ball.radius * 0.5) < base.hit_bbox.zlow // check limits of object's height and depth
            || (hitz - ball.radius * 0.5) > base.hit_bbox.zhigh
        {
            return -1.0;
        }

        // ok we have a confirmed contact, calc the stats, remember there are "near" solution, so all
        // parameters need to be calculated from the actual configuration, i.e contact radius must be calc'ed

        coll.hitnormal.x = f.x; // hit normal is same as line segment normal
        coll.hitnormal.y = f.y;
        coll.hitnormal.z = 0.0;

        let dist = Vertex2D::new(
            // calculate moment from flipper base center
            ball.pos.x + ballvx * t - ball.radius * f.x - m.hitcircle_base.center.x, // center of ball + projected radius to contact point
            ball.pos.y + ballvy * t - ball.radius * f.y - m.hitcircle_base.center.y, // all at time t
        );

        let distance = (dist.x * dist.x + dist.y * dist.y).sqrt(); // distance from base center to contact point

        let inv_dist = 1.0 / distance;
        coll.hitvel.x = -dist.y * inv_dist; // Unit Tangent velocity of contact point(rotate Normal clockwise)
        coll.hitvel.y = dist.x * inv_dist;

        if (contact_ang >= angle_max && anglespeed > 0.0) || (contact_ang <= angle_min && anglespeed < 0.0) {
            // hit limits ???
            anglespeed = 0.0; // rotation stopped
        }

        coll.hitmoment_bit = distance == 0.0;

        let dv = Vertex2D::new(
            ballvx - coll.hitvel.x * (anglespeed * distance),
            ballvy - coll.hitvel.y * (anglespeed * distance),
        ); // delta velocity ball to face

        let bnv = dv.x * coll.hitnormal.x + dv.y * coll.hitnormal.y; // dot Normal to delta v

        coll.is_contact = bnv.abs() <= C_CONTACTVEL && bffnd <= PHYS_TOUCH as f32;
        if coll.is_contact {
            coll.hit_org_normalvelocity = bnv;
        } else if bnv > C_LOWNORMVEL {
            return -1.0; // not hit ... ball is receding from endradius already, must have been embedded
        }

        coll.hitdistance = bffnd; // normal ...actual contact distance ...

        t
    }

    /// VPX's `HitFlipper::Collide`: the impulse exchange between ball and
    /// flipper, with friction, and the hit or collide event (at most one per
    /// 250 ms of continuous contact).
    pub fn collide(&mut self, base: &HitObjectBase, coll: &CollisionEvent, pball: &mut HitBall, env: &mut PhysicsEnv) {
        let normal = coll.hitnormal;

        let r_b = -pball.d.radius * normal;
        let hit_pos = pball.d.pos + r_b;

        let c_f = Vertex3Ds::new(
            self.mover.hitcircle_base.center.x,
            self.mover.hitcircle_base.center.y,
            pball.d.pos.z, // make sure collision happens in same z plane where ball is
        );

        let r_f = hit_pos - c_f; // displacement relative to flipper center

        let v_b = pball.surface_velocity(&r_b);
        let v_f = self.mover.surface_velocity(&r_f);
        let vrel = v_b - v_f;
        let mut bnv = normal.dot(&vrel); // relative normal velocity

        if bnv >= -C_LOWNORMVEL {
            // nearly receding ... make sure of conditions; otherwise if clearly approaching .. process the collision
            if bnv > C_LOWNORMVEL {
                return; // is this velocity clearly receding (i.e must > a minimum)
            }
            if coll.hitdistance < -C_EMBEDDED {
                bnv = -C_EMBEDSHOT; // has ball become embedded???, give it a kick
            } else {
                return;
            }
        }

        // (VPX's live editor remembers this ball for dragging; not ported)

        // correct displacements, mostly from low velocity blindness, an alternative to true acceleration processing
        let mut hdist = -C_DISP_GAIN * coll.hitdistance; // distance found in hit detection
        if hdist > 1.0e-4 {
            if hdist > C_DISP_LIMIT {
                hdist = C_DISP_LIMIT; // crossing ramps, delta noise
            }
            pball.d.pos += hdist * coll.hitnormal; // push along norm, back to free area; use the norm, but is not correct
        }

        // angular response to impulse in normal direction
        let mut ang_resp = cross_product(&r_f, &normal);

        // Check if flipper is in contact with its stopper and the collision impulse
        // would push it beyond the stopper. In that case, don't allow any transfer
        // of kinetic energy from ball to flipper. This avoids overly dead bounces
        // in that case.
        let ang_imp = -ang_resp.z; // minus because impulse will apply in -normal direction
        let mut flipper_response_scaling = 1.0f32;
        if self.mover.is_in_contact && self.mover.contact_torque * ang_imp >= 0.0 {
            // if impulse pushes against stopper, allow no loss of kinetic energy to flipper
            // (still allow flipper recoil, but a diminished amount)
            ang_resp.set_zero();
            flipper_response_scaling = 0.5;
        }

        // Rubber has a coefficient of restitution which decreases with the impact velocity.
        // We use a heuristic model which decreases the COR according to a falloff parameter:
        // 0 = no falloff, 1 = half the COR at 1 m/s (18.53 speed units)
        let epsilon = elasticity_with_falloff(base.elasticity, base.elasticity_falloff, bnv);

        let inv_mass = 1.0 / pball.d.mass;
        let mut impulse = -(1.0 + epsilon) * bnv
            / (inv_mass + normal.dot(&cross_product(&(ang_resp / self.mover.inertia), &r_f)));
        let mut flipper_imp = -(impulse * flipper_response_scaling) * normal;

        let mut rot_i = cross_product(&r_f, &flipper_imp);
        if self.mover.is_in_contact && rot_i.z * self.mover.contact_torque < 0.0 {
            // pushing against the solenoid?
            // Get a bound on the time the flipper needs to return to static conditions.
            // If it's too short, we treat the flipper as static during the whole collision.
            let recoil_time = -rot_i.z / self.mover.contact_torque; // time flipper needs to eliminate this impulse, in 10ms

            // Check ball normal velocity after collision. If the ball rebounded
            // off the flipper, we need to make sure it does so with full
            // reflection, i.e., treat the flipper as static, otherwise
            // we get overly dead bounces.
            let bnv_after = bnv + impulse * inv_mass;

            if recoil_time <= 0.5 || bnv_after > 0.0 {
                // treat flipper as static for this impact
                impulse = -(1.0 + epsilon) * bnv * pball.d.mass;
                flipper_imp.set_zero();
                rot_i.set_zero();
            }
        }

        pball.d.vel += (impulse * inv_mass) * normal; // new velocity for ball after impact
        self.mover.apply_impulse(&rot_i);

        // apply friction

        let mut tangent = vrel - vrel.dot(&normal) * normal; // calc the tangential velocity

        let tangent_sp_sq = tangent.length_squared();
        if tangent_sp_sq > 1e-6 {
            tangent /= tangent_sp_sq.sqrt(); // normalize to get tangent direction
            let vt = vrel.dot(&tangent); // get speed in tangential direction

            // compute friction impulse
            let cross_b = cross_product(&r_b, &tangent);
            let mut kt = inv_mass + tangent.dot(&cross_product(&(cross_b / pball.inertia()), &r_b));

            let cross_f = cross_product(&r_f, &tangent);
            kt += tangent.dot(&cross_product(&(cross_f / self.mover.inertia), &r_f)); // flipper only has angular response

            // friction impulse can't be greater than coefficient of friction times collision impulse (Coulomb friction cone)
            let max_fric = base.friction * impulse;
            let jt = clamp(-vt / kt, -max_fric, max_fric);

            pball.apply_surface_impulse(&(jt * cross_b), &(jt * tangent));
            self.mover.apply_impulse(&(-jt * cross_f));
        }

        if bnv < -0.25 && env.time_msec.wrapping_sub(self.last_hittime) > 250 {
            // limit rate to 250 milliseconds per event
            let flipper_hit = if coll.hitmoment_bit { -1.0 } else { -bnv }; // move event processing to end of collision handler...
            if flipper_hit < 0.0 {
                env.events.push(PhysicsEvent::Hit { part: self.part }); // simple hit event
            } else {
                env.events.push(PhysicsEvent::FlipperCollide { part: self.part, speed: flipper_hit }); // collision velocity (normal to face)
            }
        }

        self.last_hittime = env.time_msec; // keep resetting until idle for 250 milliseconds
    }

    /// VPX's `HitFlipper::Contact`: a ball resting on or rolling along the
    /// flipper, with static and sliding friction.
    pub fn contact(&mut self, base: &HitObjectBase, coll: &CollisionEvent, pball: &mut HitBall, dtime: f32, env: &PhysicsEnv) {
        let normal = coll.hitnormal;

        if coll.hitdistance < -C_EMBEDDED {
            pball.d.vel += normal * 0.1; // !! magic to avoid balls being pushed by each other through resting flippers!
        }

        let r_b = -pball.d.radius * normal;
        let hit_pos = pball.d.pos + r_b;

        let c_f = Vertex3Ds::new(
            self.mover.hitcircle_base.center.x,
            self.mover.hitcircle_base.center.y,
            pball.d.pos.z, // make sure collision happens in same z plane where ball is
        );

        let r_f = hit_pos - c_f; // displacement relative to flipper center

        let v_b = pball.surface_velocity(&r_b);
        let v_f = self.mover.surface_velocity(&r_f);
        let vrel = v_b - v_f;

        let norm_vel = vrel.dot(&normal); // this should be zero, but only up to +/- C_CONTACTVEL

        // If some collision has changed the ball's velocity, we may not have to do anything.
        if norm_vel <= C_CONTACTVEL {
            // compute accelerations of point on ball and flipper
            let a_b = pball.surface_acceleration(&r_b, env);
            let a_f = self.mover.surface_acceleration(&r_f);
            let arel = a_b - a_f;

            // time derivative of the normal vector
            let normal_deriv = cross_z(self.mover.angle_speed, &normal);

            // relative acceleration in the normal direction
            let norm_acc = arel.dot(&normal) + 2.0 * normal_deriv.dot(&vrel);

            if norm_acc >= 0.0 {
                return; // objects accelerating away from each other, nothing to do
            }

            // hypothetical accelerations arising from a unit contact force in normal direction
            let inv_mass = 1.0 / pball.d.mass;
            let a_bc = inv_mass * normal;
            let cross = cross_product(&r_f, &-normal);
            let a_fc = cross_product(&(cross / self.mover.inertia), &r_f);
            let contact_force_acc = normal.dot(&(a_bc - a_fc));

            // find j >= 0 such that normAcc + j * contactForceAcc >= 0  (bodies should not accelerate towards each other)

            let j = -norm_acc / contact_force_acc;

            pball.d.vel += ((j * dtime) * inv_mass - coll.hit_org_normalvelocity) * normal; // kill any existing normal velocity
            self.mover.apply_impulse(&((j * dtime) * cross));

            // apply friction

            // first check for slippage
            let slip = vrel - norm_vel * normal; // calc the tangential slip velocity

            let max_fric = j * base.friction;

            let slipspeed = slip.length();
            let (slip_dir, cross_f, numer, denom_f);
            if slipspeed < C_PRECISION {
                // slip speed zero - static friction case
                let slip_acc = arel - arel.dot(&normal) * normal; // calc the tangential slip acceleration

                // neither slip velocity nor slip acceleration? nothing to do here
                if slip_acc.length_squared() < 1e-6 {
                    return;
                }

                let mut sd = slip_acc;
                sd.normalize();
                slip_dir = sd;

                numer = -slip_dir.dot(&arel);
                cross_f = cross_product(&r_f, &slip_dir);
                denom_f = slip_dir.dot(&cross_product(&(cross_f / -self.mover.inertia), &r_f));
            } else {
                // nonzero slip speed - dynamic friction case
                slip_dir = slip / slipspeed;

                numer = -slip_dir.dot(&vrel);
                cross_f = cross_product(&r_f, &slip_dir);
                denom_f = slip_dir.dot(&cross_product(&(cross_f / self.mover.inertia), &r_f));
            }

            let cross_b = cross_product(&r_b, &slip_dir);
            let denom_b = inv_mass + slip_dir.dot(&cross_product(&(cross_b / pball.inertia()), &r_b));
            let fric = clamp(numer / (denom_b + denom_f), -max_fric, max_fric);

            pball.apply_surface_impulse(&((dtime * fric) * cross_b), &((dtime * fric) * slip_dir));
            self.mover.apply_impulse(&(-(dtime * fric) * cross_f));
        }
    }
}
