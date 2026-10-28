// Ported from Visual Pinball X: src/physics/collide.h and src/physics/collide.cpp (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436), with FIX_PHYSICS and NEW_PHYSICS off.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! Collision events, the data every collider shares, and VPX's basic
//! colliders: wall segments, circles, vertical lines and points.
//!
//! VPX's colliders are C++ classes deriving from `HitObject`. Here the shared
//! fields are [`HitObjectBase`] and each shape is its own struct; the hit tests
//! and collision responses are line-by-line ports. Drawing code (`DrawUI`) is
//! not ported.

use crate::env::{BallId, PartId, PhysicsEnv, PhysicsEvent};
use crate::hitball::{BallS, HitBall};
use crate::math::def::{inf_nan, max, min, sign, std_max};
use crate::math::equations::solve_quadratic_eq;
use crate::math::{FRect3D, Vertex2D, Vertex3Ds};
use crate::physconst::{C_CONTACTVEL, C_LOWNORMVEL, C_TOL_ENDPNTS, C_TOL_RADIUS, PHYS_TOUCH};

/// VPX's `eObjType`. The order matters: VPX tests `>= eTrigger` for volume
/// colliders (triggers and kickers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ObjType {
    #[default]
    Null,
    Point,
    LineSeg,
    LineSegSlingshot,
    Joint,
    Circle,
    Flipper,
    Plunger,
    Spinner,
    Ball,
    Poly3D,
    Triangle,
    Plane,
    Line3D,
    Gate,
    Textbox,
    DispReel,
    LightSeq,
    /// Also used for rubbers and ramps, which are made of triangles too.
    Primitive,
    HitTarget,
    /// This value and above are volume colliders.
    Trigger,
    Kicker,
}

/// VPX's `ElasticityWithFalloff`: rubber's bounciness drops with impact speed.
/// 0 = no falloff, 1 = half the elasticity at 1 m/s (18.53 VPU/VPT).
#[inline(always)]
pub fn elasticity_with_falloff(elasticity: f32, falloff: f32, vel: f32) -> f32 {
    if falloff > 0.0 { elasticity / (1.0 + falloff * vel.abs() * (1.0 / 18.53_f64) as f32) } else { elasticity }
}

/// What a ball collided with: one of the engine's colliders (by index), or
/// another ball. VPX keeps a `HitObject*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitRef {
    Collider(u32),
    Ball(BallId),
}

/// What collided with what, when and how: VPX's `CollisionEvent`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionEvent {
    /// The ball that collided.
    pub ball: Option<BallId>,
    /// What it collided with.
    pub obj: Option<HitRef>,
    /// When the collision happens, relative to the current physics state.
    pub hittime: f32,
    /// Contact distance.
    pub hitdistance: f32,
    pub hitnormal: Vertex3Ds,
    /// Only used by plungers and flippers.
    pub hitvel: Vertex2D,
    /// Only set when `is_contact` is true.
    pub hit_org_normalvelocity: f32,
    pub hitmoment_bit: bool,
    /// Unhit signal, or which side was hit (spinner, gate).
    pub hitflag: bool,
    /// The impact speed is about zero: handled as a resting contact.
    pub is_contact: bool,
}

impl Default for CollisionEvent {
    /// VPX's constructor values.
    fn default() -> Self {
        Self {
            ball: None,
            obj: None,
            hittime: 0.0,
            hitdistance: 0.0,
            hitnormal: Vertex3Ds::default(),
            hitvel: Vertex2D::default(),
            hit_org_normalvelocity: 0.0,
            hitmoment_bit: true,
            hitflag: false,
            is_contact: false,
        }
    }
}

/// The fields every VPX `HitObject` has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitObjectBase {
    /// Fire events for `obj`?
    pub fe: bool,
    /// Minimum impact speed for a hit event.
    pub threshold: f32,
    /// The part that receives this collider's events (VPX `m_obj`).
    pub obj: Option<PartId>,
    /// Bounding box; for line segments and circles only `zlow`/`zhigh` are set
    /// at creation, the rest by `calc_hit_bbox`.
    pub hit_bbox: FRect3D,
    pub elasticity: f32,
    pub elasticity_falloff: f32,
    pub friction: f32,
    /// In radians.
    pub scatter: f32,
    pub obj_type: ObjType,
    pub enabled: bool,
    /// Its part and the part's hit threshold where the table has the part's
    /// hits off: watched for achievements where `PhysicsEnv::watching` names it
    /// (`watched`). Ours: VPX has none.
    pub quiet: Option<(PartId, f32)>,
}

impl Default for HitObjectBase {
    /// VPX's member initializers.
    fn default() -> Self {
        Self {
            fe: false,
            threshold: 0.0,
            obj: None,
            hit_bbox: FRect3D::default(),
            elasticity: 0.3,
            elasticity_falloff: 0.0,
            friction: 0.3,
            scatter: 0.0,
            obj_type: ObjType::Null,
            enabled: true,
            quiet: None,
        }
    }
}

impl HitObjectBase {
    /// A base with `zlow`/`zhigh` stored in the bounding box, as VPX's line
    /// segment and circle constructors do.
    pub fn with_z(zlow: f32, zhigh: f32) -> Self {
        let mut b = Self::default();
        b.hit_bbox.zlow = zlow;
        b.hit_bbox.zhigh = zhigh;
        b
    }

    /// VPX's `HitObject::FireHitEvent`: a hit event, unless the ball hasn't
    /// travelled since the last one at the same place.
    pub fn fire_hit_event(&self, pball: &mut HitBall, env: &mut PhysicsEnv) {
        let Some(obj) = self.obj else { return };
        if self.fe && self.enabled {
            // is this the same place as last event and ball did not travel in between, then ignores it
            if pball.last_event_sqr_dist < 0.5 {
                // 0.5 VPU is a magic value to filter out rest collision
                let dist_ls = (pball.last_event_pos - pball.d.pos).length_squared();
                let normal_dist = if self.obj_type == ObjType::HitTarget {
                    0.0 // hit targets when used with a captured ball have always a too small distance
                } else {
                    0.25 // !! magic distance
                };
                if dist_ls <= normal_dist {
                    return; // must be a new place if only by a little
                }
            }
            pball.last_event_sqr_dist = 0.0;
            pball.last_event_pos = pball.d.pos; // remember last collision position
            env.events.push(PhysicsEvent::Hit { part: obj });
        }
    }

    /// A hit on a part the table has its hits off for (`quiet`), at `speed`,
    /// told for achievements where they watch the part (`PhysicsEnv::watching`)
    /// and it's at the part's threshold or more: never the script's, the ball's
    /// own record of its last event untouched, a ball resting against the part
    /// telling it once. Ours.
    pub fn watched(&self, speed: f32, pball: &HitBall, env: &mut PhysicsEnv) {
        let Some((part, threshold)) = self.quiet else { return };
        if !self.enabled || speed < threshold || !env.watching.contains(&part) {
            return;
        }
        let at = pball.d.pos;
        match env.watched_at.iter_mut().find(|(ball, _)| *ball == pball.id) {
            Some((_, last)) if (*last - at).length_squared() <= 0.25 => return,
            Some((_, last)) => *last = at,
            None => env.watched_at.push((pball.id, at)),
        }
        env.events.push(PhysicsEvent::Watched { part });
    }
}

/// VPX's `LineSeg`: a 2D wall with a height range.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineSeg {
    pub v1: Vertex2D,
    pub v2: Vertex2D,
    pub normal: Vertex2D,
    pub length: f32,
}

impl LineSeg {
    pub fn new(p1: Vertex2D, p2: Vertex2D) -> Self {
        let mut s = Self { v1: p1, v2: p2, ..Default::default() };
        s.calc_normal_and_length();
        s
    }

    pub fn set(&mut self, p1: Vertex2D, p2: Vertex2D) {
        self.v1 = p1;
        self.v2 = p2;
        self.calc_normal_and_length();
    }

    pub fn calc_normal_and_length(&mut self) {
        let vt = Vertex2D::new(self.v1.x - self.v2.x, self.v1.y - self.v2.y);

        // Set up line normal
        self.length = vt.length();
        let inv_length = 1.0 / self.length;
        self.normal.x = vt.y * inv_length;
        self.normal.y = -vt.x * inv_length;
    }

    pub fn calc_hit_bbox(&self, base: &mut HitObjectBase) {
        // Allow roundoff
        base.hit_bbox.left = min(self.v1.x, self.v2.x);
        base.hit_bbox.right = max(self.v1.x, self.v2.x);
        base.hit_bbox.top = min(self.v1.y, self.v2.y);
        base.hit_bbox.bottom = max(self.v1.y, self.v2.y);
        // zlow and zhigh were already set at creation
    }

    /// VPX's `LineSeg::HitTestBasic`.
    #[allow(clippy::too_many_arguments)]
    pub fn hit_test_basic(
        &self,
        base: &HitObjectBase,
        ball: &BallS,
        dtime: f32,
        coll: &mut CollisionEvent,
        direction: bool,
        lateral: bool,
        rigid: bool,
    ) -> f32 {
        if !base.enabled || ball.locked_in_kicker {
            return -1.0;
        }

        let ballvx = ball.vel.x; // ball velocity
        let ballvy = ball.vel.y;

        let bnv = ballvx * self.normal.x + ballvy * self.normal.y; // ball velocity normal to segment, positive if receding, zero=parallel
        let mut b_unhit = bnv > C_LOWNORMVEL;

        if direction && b_unhit {
            // direction true and clearly receding from normal face
            return -1.0;
        }

        let ballx = ball.pos.x; // ball position
        let bally = ball.pos.y;

        // ball normal distance: contact distance normal to segment. lateral contact subtract the ball radius
        let rolling_radius = if lateral { ball.radius } else { C_TOL_RADIUS }; // lateral or rolling point
        let bcpd = (ballx - self.v1.x) * self.normal.x + (bally - self.v1.y) * self.normal.y; // ball center to plane distance
        let mut bnd = bcpd - rolling_radius;

        // for a spinner add the ball radius otherwise the ball goes half through the spinner until it moves
        if base.obj_type == ObjType::Spinner || base.obj_type == ObjType::Gate {
            bnd = bcpd + rolling_radius;
        }

        let inside = bnd <= 0.0; // in ball inside object volume

        let hittime: f32;
        if rigid {
            if (bnd < -ball.radius) || (lateral && bcpd < 0.0) {
                return -1.0; // (ball normal distance) excessive penetration of object skin ... no collision HACK
            }

            if lateral && (bnd <= PHYS_TOUCH as f32) {
                if inside || (bnv.abs() > C_CONTACTVEL) // fast velocity, return zero time
                    // zero time for rigid fast bodies
                    || (bnd <= (-PHYS_TOUCH) as f32)
                {
                    hittime = 0.0; // slow moving but embedded
                } else {
                    hittime = bnd * (1.0 / (2.0 * PHYS_TOUCH)) as f32 + 0.5; // don't compete for fast zero time events
                }
            } else if bnv.abs() > C_LOWNORMVEL {
                // not velocity low ????
                hittime = bnd / -bnv; // rate ok for safe divide
            } else {
                return -1.0; // wait for touching
            }
        } else {
            // non-rigid ... target hits
            if bnv * bnd >= 0.0 {
                // outside-receding || inside-approaching
                let not_this_trigger = base.obj_type != ObjType::Trigger // not a trigger
                    || ball.vp_vol_objs.is_none()
                    // is a trigger, so test:
                    || (bnd.abs() >= ball.radius * 0.5) // not too close ... nor too far away
                    || (inside != !ball.is_inside(base.obj)); // ...ball outside and hit set or ball inside and no hit set
                if not_this_trigger {
                    return -1.0;
                }
                hittime = 0.0;
                b_unhit = !inside; // ball on outside is UnHit, otherwise it's a Hit
            } else {
                hittime = bnd / -bnv;
            }
        }

        if inf_nan(hittime) || hittime < 0.0 || hittime > dtime {
            return -1.0; // time is outside this frame ... no collision
        }
        let btv = ballvx * self.normal.y - ballvy * self.normal.x; // ball velocity tangent to segment with respect to direction from V1 to V2
        let btd = (ballx - self.v1.x) * self.normal.y - (bally - self.v1.y) * self.normal.x // ball tangent distance
            + btv * hittime; // ball tangent distance (projection) (initial position + velocity * hitime)

        if btd < -C_TOL_ENDPNTS || btd > self.length + C_TOL_ENDPNTS {
            // is the contact off the line segment???
            return -1.0;
        }
        if !rigid {
            // non rigid body collision? return direction
            coll.hitflag = b_unhit; // UnHit signal is receding from outside target
        }

        let hitz = ball.pos.z + ball.vel.z * hittime; // check too high or low relative to ball rolling point at hittime

        if hitz + ball.radius * 0.5 < base.hit_bbox.zlow // check limits of object's height and depth
            || hitz - ball.radius * 0.5 > base.hit_bbox.zhigh
        {
            return -1.0;
        }
        coll.hitnormal.x = self.normal.x; // hit normal is same as line segment normal
        coll.hitnormal.y = self.normal.y;
        coll.hitnormal.z = 0.0;

        coll.hitdistance = bnd; // actual contact distance ...

        // check for contact
        coll.is_contact = bnv.abs() <= C_CONTACTVEL && bnd.abs() <= PHYS_TOUCH as f32;
        if coll.is_contact {
            coll.hit_org_normalvelocity = bnv;
        }

        hittime
    }

    /// VPX's `LineSeg::HitTest`: normal face, lateral, rigid.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        self.hit_test_basic(base, ball, dtime, coll, true, true, true)
    }

    /// VPX's `LineSeg::Collide`.
    pub fn collide(base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        let dot = coll.hitnormal.dot(&ball.d.vel);
        ball.collide_3d_wall(&coll.hitnormal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env);

        if dot <= -base.threshold {
            base.fire_hit_event(ball, env);
        }
        base.watched(-dot, ball, env);
    }
}

/// VPX's `HitCircle`: a cylinder (bumpers, posts, flipper bases, triggers).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitCircle {
    pub center: Vertex2D,
    pub radius: f32,
}

impl HitCircle {
    pub fn new(center: Vertex2D, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn calc_hit_bbox(&self, base: &mut HitObjectBase) {
        // Allow roundoff
        base.hit_bbox.left = self.center.x - self.radius;
        base.hit_bbox.right = self.center.x + self.radius;
        base.hit_bbox.top = self.center.y - self.radius;
        base.hit_bbox.bottom = self.center.y + self.radius;
        // zlow & zhigh already set at creation
    }

    /// VPX's `HitCircle::HitTestBasicRadius`. All flags true for bumpers,
    /// flipper bases, gates and spinners; all false for kickers and triggers.
    /// Kickers and triggers change the set of volumes the ball is inside, as in
    /// VPX.
    #[allow(clippy::too_many_arguments)]
    pub fn hit_test_basic_radius(
        &self,
        base: &HitObjectBase,
        ball: &BallS,
        dtime: f32,
        coll: &mut CollisionEvent,
        direction: bool,
        lateral: bool,
        rigid: bool,
    ) -> f32 {
        if !base.enabled || ball.locked_in_kicker {
            return -1.0;
        }

        let mut c = Vertex3Ds::new(self.center.x, self.center.y, 0.0);
        let mut dist = ball.pos - c; // relative ball position
        let mut dv = ball.vel;

        let capsule_3d = !lateral && ball.pos.z > base.hit_bbox.zhigh;

        let target_radius: f32;
        if capsule_3d {
            // handle ball over target?
            target_radius = self.radius * (13.0_f64 / 5.0) as f32; // optimized version of the cap formula
            c.z = base.hit_bbox.zhigh - self.radius * (12.0_f64 / 5.0) as f32; // optimized version of the cap formula
            dist.z = ball.pos.z - c.z; // ball rolling point - capsule center height
        } else {
            let mut r = self.radius;
            if lateral {
                r += ball.radius;
            }
            target_radius = r;
            dist.z = 0.0;
            dv.z = 0.0;
        }

        let bcddsq = dist.length_squared(); // ball center to circle center distance ... squared
        let bcdd = bcddsq.sqrt(); // distance center to center
        if bcdd <= 1.0e-6 {
            return -1.0; // no hit on exact center
        }

        let b = dist.dot(&dv);
        let bnv = b / bcdd; // ball normal velocity

        if direction && bnv > C_LOWNORMVEL {
            return -1.0; // clearly receding from radius
        }

        let bnd = bcdd - target_radius; // ball normal distance to

        let a = dv.length_squared();

        let mut hittime = 0.0_f32;
        let mut b_unhit = false;
        let mut is_contact = false;
        // Kicker is special.. handle ball stalled on kicker, commonly hit while receding, knocking back into kicker pocket
        if base.obj_type == ObjType::Kicker
            && bnd <= 0.0
            && bnd >= -self.radius
            && a < C_CONTACTVEL * C_CONTACTVEL
            && ball.vp_vol_objs.is_some()
        {
            ball.remove_volume(base.obj); // cause capture
        }

        if rigid && bnd < PHYS_TOUCH as f32 {
            // positive: contact possible in future ... Negative: objects in contact now
            if bnd < -ball.radius {
                return -1.0;
            } else if bnv.abs() <= C_CONTACTVEL {
                is_contact = true;
            } else {
                // estimate based on distance and speed along distance
                hittime = std_max(0.0, -bnd / bnv);
            }
        } else if base.obj_type >= ObjType::Trigger // triggers & kickers
            && ball.vp_vol_objs.is_some()
            && ((bnd < 0.0) == !ball.is_inside(base.obj))
        {
            // here if ... ball inside and no hit set .... or ... ball outside and hit set
            if (bnd - self.radius).abs() < 0.05 {
                // if ball appears in center of trigger, then assumed it was gen'ed there
                ball.add_volume(base.obj); // special case for trigger overlaying a kicker
            // this will add the ball to the trigger space without a Hit
            } else {
                b_unhit = bnd > 0.0; // ball on outside is UnHit, otherwise it's a Hit
            }
        } else {
            if (!rigid && bnd * bnv > 0.0) // (outside and receding) or (inside and approaching)
                || (a < 1.0e-8)
            {
                return -1.0; // no hit ... ball not moving relative to object
            }

            let Some((time1, time2)) = solve_quadratic_eq(a, 2.0 * b, bcddsq - target_radius * target_radius) else {
                return -1.0;
            };

            b_unhit = time1 * time2 < 0.0;
            hittime = if b_unhit { max(time1, time2) } else { min(time1, time2) }; // ball is inside the circle
        }

        if inf_nan(hittime) || hittime < 0.0 || hittime > dtime {
            return -1.0; // contact out of physics frame
        }

        let hitz = ball.pos.z + ball.vel.z * hittime; // rolling point

        if ((hitz + ball.radius * 0.5) < base.hit_bbox.zlow)
            || (!capsule_3d && (hitz - ball.radius * 0.5) > base.hit_bbox.zhigh)
            || (capsule_3d && hitz < base.hit_bbox.zhigh)
        {
            return -1.0;
        }

        let hitx = ball.pos.x + ball.vel.x * hittime;
        let hity = ball.pos.y + ball.vel.y * hittime;

        let sqrlen = (hitx - c.x) * (hitx - c.x) + (hity - c.y) * (hity - c.y);

        if sqrlen > 1.0e-8 {
            // over center??? no
            let inv_len = 1.0 / sqrlen.sqrt();
            coll.hitnormal.x = (hitx - c.x) * inv_len;
            coll.hitnormal.y = (hity - c.y) * inv_len;
            coll.hitnormal.z = 0.0;
        } else {
            // yes, over center
            coll.hitnormal.x = 0.0; // make up a value, any direction is ok
            coll.hitnormal.y = 1.0;
            coll.hitnormal.z = 0.0;
        }

        if !rigid {
            // non rigid body collision? return direction
            coll.hitflag = b_unhit; // UnHit signal is receding from target
        }

        coll.is_contact = is_contact;
        if is_contact {
            coll.hit_org_normalvelocity = bnv;
        }

        coll.hitdistance = bnd; // actual contact distance ...

        hittime
    }

    /// VPX's `HitCircle::HitTest`: normal face, lateral, rigid.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        self.hit_test_basic_radius(base, ball, dtime, coll, true, true, true)
    }

    /// VPX's `HitCircle::Collide`: no hit event.
    pub fn collide(base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        ball.collide_3d_wall(&coll.hitnormal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env);
    }
}

/// VPX's `HitLineZ`: a vertical line segment (wall corners).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitLineZ {
    pub xy: Vertex2D,
    pub zlow: f32,
    pub zhigh: f32,
}

impl HitLineZ {
    pub fn new(xy: Vertex2D, zlow: f32, zhigh: f32) -> Self {
        Self { xy, zlow, zhigh }
    }

    pub fn calc_hit_bbox(&self, base: &mut HitObjectBase) {
        base.hit_bbox.left = self.xy.x;
        base.hit_bbox.right = self.xy.x;
        base.hit_bbox.top = self.xy.y;
        base.hit_bbox.bottom = self.xy.y;
        base.hit_bbox.zlow = self.zlow;
        base.hit_bbox.zhigh = self.zhigh;
    }

    /// VPX's `HitLineZ::HitTest`.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        if !base.enabled {
            return -1.0;
        }

        let bp2d = Vertex2D::new(ball.pos.x, ball.pos.y);
        let dist = bp2d - self.xy; // relative ball position
        let dv = Vertex2D::new(ball.vel.x, ball.vel.y);

        let bcddsq = dist.length_squared(); // ball center to line distance squared
        let bcdd = bcddsq.sqrt(); // distance ball to line
        if bcdd <= 1.0e-6 {
            return -1.0; // no hit on exact center
        }

        let b = dist.dot(&dv);
        let bnv = b / bcdd; // ball normal velocity

        if bnv > C_CONTACTVEL {
            return -1.0; // clearly receding from radius
        }

        let bnd = bcdd - ball.radius; // ball distance to line

        let a = dv.length_squared();

        let hittime: f32;
        let mut is_contact = false;

        if bnd < PHYS_TOUCH as f32 {
            // already in collision distance?
            if bnv.abs() <= C_CONTACTVEL {
                is_contact = true;
                hittime = 0.0;
            } else {
                hittime = -bnd / bnv; // estimate based on distance and speed along distance
            }
        } else {
            if a < 1.0e-8 {
                return -1.0; // no hit - ball not moving relative to object
            }

            let Some((time1, time2)) = solve_quadratic_eq(a, 2.0 * b, bcddsq - ball.radius * ball.radius) else {
                return -1.0;
            };

            hittime = if time1 * time2 < 0.0 { max(time1, time2) } else { min(time1, time2) }; // find smallest nonnegative solution
        }

        if inf_nan(hittime) || hittime < 0.0 || hittime > dtime {
            return -1.0; // contact out of physics frame
        }

        let hitz = ball.pos.z + hittime * ball.vel.z; // ball z position at hit time

        if hitz < self.zlow || hitz > self.zhigh {
            // check z coordinate
            return -1.0;
        }

        let hitx = ball.pos.x + hittime * ball.vel.x; // ball x position at hit time
        let hity = ball.pos.y + hittime * ball.vel.y; // ball y position at hit time

        let mut norm = Vertex2D::new(hitx - self.xy.x, hity - self.xy.y);
        norm.normalize();
        coll.hitnormal.set(norm.x, norm.y, 0.0);

        coll.is_contact = is_contact;
        if is_contact {
            coll.hit_org_normalvelocity = bnv;
        }

        coll.hitdistance = bnd; // actual contact distance

        hittime
    }

    /// VPX's `HitLineZ::Collide`.
    pub fn collide(base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        let dot = coll.hitnormal.dot(&ball.d.vel);
        ball.collide_3d_wall(&coll.hitnormal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env);

        if dot <= -base.threshold {
            base.fire_hit_event(ball, env);
        }
        base.watched(-dot, ball, env);
    }
}

/// VPX's `HitPoint`: a point (wall and ramp vertices).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitPoint {
    pub p: Vertex3Ds,
}

impl HitPoint {
    pub fn new(p: Vertex3Ds) -> Self {
        Self { p }
    }

    pub fn calc_hit_bbox(&self, base: &mut HitObjectBase) {
        base.hit_bbox = FRect3D::new(self.p.x, self.p.x, self.p.y, self.p.y, self.p.z, self.p.z);
    }

    /// VPX's `HitPoint::HitTest`.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        if !base.enabled {
            return -1.0;
        }

        let dist = ball.pos - self.p; // relative ball position

        let bcddsq = dist.length_squared(); // ball center to line distance squared
        let bcdd = bcddsq.sqrt(); // distance ball to line
        if bcdd <= 1.0e-6 {
            return -1.0; // no hit on exact center
        }

        let b = dist.dot(&ball.vel);
        let bnv = b / bcdd; // ball normal velocity

        if bnv > C_CONTACTVEL {
            return -1.0; // clearly receding from radius
        }

        let bnd = bcdd - ball.radius; // ball distance to line

        let a = ball.vel.length_squared();

        let hittime: f32;
        let mut is_contact = false;

        if bnd < PHYS_TOUCH as f32 {
            // already in collision distance?
            if bnv.abs() <= C_CONTACTVEL {
                is_contact = true;
                hittime = 0.0;
            } else {
                // estimate based on distance and speed along distance
                hittime = std_max(0.0, -bnd / bnv);
            }
        } else {
            if a < 1.0e-8 {
                return -1.0; // no hit - ball not moving relative to object
            }

            let Some((time1, time2)) = solve_quadratic_eq(a, 2.0 * b, bcddsq - ball.radius * ball.radius) else {
                return -1.0;
            };

            hittime = if time1 * time2 < 0.0 { max(time1, time2) } else { min(time1, time2) }; // find smallest nonnegative solution
        }

        if inf_nan(hittime) || hittime < 0.0 || hittime > dtime {
            return -1.0; // contact out of physics frame
        }

        let hit_pos = ball.pos + hittime * ball.vel;
        coll.hitnormal = hit_pos - self.p;
        coll.hitnormal.normalize();

        coll.is_contact = is_contact;
        if is_contact {
            coll.hit_org_normalvelocity = bnv;
        }

        coll.hitdistance = bnd; // actual contact distance

        hittime
    }

    /// VPX's `HitPoint::Collide`.
    pub fn collide(base: &HitObjectBase, coll: &CollisionEvent, ball: &mut HitBall, env: &mut PhysicsEnv) {
        let dot = coll.hitnormal.dot(&ball.d.vel);
        ball.collide_3d_wall(&coll.hitnormal, base.elasticity, base.elasticity_falloff, base.friction, base.scatter, env);

        if dot <= -base.threshold {
            base.fire_hit_event(ball, env);
        }
        base.watched(-dot, ball, env);
    }
}

/// VPX's `DoHitTest` acceptance rule: a hit time from a collider's hit test
/// replaces the current candidate collision when it is non-negative (not -0.0)
/// and no later than the candidate. Contacts are recorded separately while the
/// engine collects them, and then don't replace the candidate.
///
/// Returns true when `coll` was replaced by `new_coll`.
pub fn do_hit_test_accept(
    ball: BallId,
    obj: HitRef,
    newtime: f32,
    mut new_coll: CollisionEvent,
    coll: &mut CollisionEvent,
    contacts: Option<&mut Vec<CollisionEvent>>,
) -> bool {
    let validhit = (newtime >= 0.0) && !sign(newtime) && (newtime <= coll.hittime);
    if validhit {
        new_coll.ball = Some(ball);
        new_coll.obj = Some(obj);
        new_coll.hittime = newtime;
        let recorded = new_coll.is_contact
            && match contacts {
                Some(list) => {
                    list.push(new_coll);
                    true
                }
                None => false,
            };
        if !recorded {
            *coll = new_coll; // record first collision event
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ball hitting a wall the table has its hits off for, twice in the same place: the same
    /// bounce whether or not achievements watch the wall, its hit told to them alone and once
    /// where they do, never as the table's own event.
    #[test]
    fn a_watched_part_tells_its_hits_and_changes_nothing() {
        let mut base = HitObjectBase::default();
        base.quiet = Some((PartId(7), 2.0));
        let coll = CollisionEvent { hitnormal: Vertex3Ds::new(0.0, -1.0, 0.0), ..CollisionEvent::default() };
        let run = |watch: bool| {
            let mut env = PhysicsEnv::default();
            if watch {
                env.watching.insert(PartId(7));
            }
            let mut ball = HitBall::new(BallId(1));
            ball.d.pos = Vertex3Ds::new(100.0, 200.0, 25.0);
            ball.d.radius = 25.0;
            ball.d.mass = 1.0;
            for _ in 0..2 {
                ball.d.vel = Vertex3Ds::new(0.0, 10.0, 0.0);
                LineSeg::collide(&base, &coll, &mut ball, &mut env);
            }
            ((ball.d.pos, ball.d.vel, ball.last_event_pos, ball.last_event_sqr_dist), env.events)
        };
        let (quiet, none) = run(false);
        let (watched, told) = run(true);
        assert_eq!(quiet, watched);
        assert!(none.is_empty());
        assert_eq!(told, [PhysicsEvent::Watched { part: PartId(7) }]);
        // a slower hit than the part's threshold tells nothing
        let mut env = PhysicsEnv::default();
        env.watching.insert(PartId(7));
        let mut ball = HitBall::new(BallId(2));
        ball.d.radius = 25.0;
        ball.d.mass = 1.0;
        ball.d.vel = Vertex3Ds::new(0.0, 1.0, 0.0);
        LineSeg::collide(&base, &coll, &mut ball, &mut env);
        assert!(env.events.is_empty());
    }
}
