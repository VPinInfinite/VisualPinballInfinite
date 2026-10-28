// Ported from Visual Pinball X: src/physics/hitball.h and src/physics/hitball.cpp (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436), with FIX_PHYSICS and NEW_PHYSICS off.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The ball: its state, how it bounces off walls and other balls, friction,
//! spin, and how it moves each step.

use crate::collide::{CollisionEvent, HitObjectBase, elasticity_with_falloff};
use crate::env::{BallId, PartId, PhysicsEnv, PhysicsEvent};
use crate::math::def::{ang_to_rad, clamp, inf_nan, sqrf, std_max, std_min};
use crate::math::equations::solve_quadratic_eq;
use crate::math::{Matrix3, Vertex3Ds, cross_product};
use crate::physconst::{
    C_CONTACTVEL, C_DISP_GAIN, C_DISP_LIMIT, C_EMBEDDED, C_EMBEDSHOT, C_LOWNORMVEL, C_PRECISION, DEFAULT_BALL_SIZE, PHYS_FACTOR, PHYS_TOUCH,
    ms2tovpuvpt2,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Ball positions kept for spin damping and trails: one every 10 ms, 100 ms in all.
pub const MAX_BALL_TRAIL_POS: usize = 10;

/// The triggers and kickers a ball is inside: VPX's `vector<IFireEvents*>*`.
/// Shared, because VPX copies a ball's state into temporaries that keep
/// pointing at the same list, and hit tests can change it.
pub type VolumeSet = Rc<RefCell<Vec<PartId>>>;

/// VPX's `BallS`: the ball state that hit tests read.
#[derive(Clone, Debug)]
pub struct BallS {
    pub pos: Vertex3Ds,
    pub vel: Vertex3Ds,
    pub radius: f32,
    pub mass: f32,
    pub vp_vol_objs: Option<VolumeSet>,
    pub locked_in_kicker: bool,
}

impl Default for BallS {
    /// VPX's member initializers (the volume list pointer starts unset).
    fn default() -> Self {
        Self {
            pos: Vertex3Ds::new(0.0, 0.0, 0.0),
            vel: Vertex3Ds::new(0.0, 0.0, 0.0),
            radius: DEFAULT_BALL_SIZE,
            mass: 1.0,
            vp_vol_objs: None,
            locked_in_kicker: false,
        }
    }
}

impl BallS {
    /// `FindIndexOf(*m_vpVolObjs, obj) >= 0`. False without a list, or for a
    /// collider without a part.
    pub fn is_inside(&self, obj: Option<PartId>) -> bool {
        match (&self.vp_vol_objs, obj) {
            (Some(set), Some(part)) => set.borrow().contains(&part),
            _ => false,
        }
    }

    /// `RemoveFromVectorSingle(*m_vpVolObjs, obj)`: removes the first match.
    pub fn remove_volume(&self, obj: Option<PartId>) {
        if let (Some(set), Some(part)) = (&self.vp_vol_objs, obj) {
            let mut v = set.borrow_mut();
            if let Some(i) = v.iter().position(|p| *p == part) {
                v.remove(i);
            }
        }
    }

    /// `m_vpVolObjs->push_back(obj)`. Triggers and kickers always have a part.
    pub fn add_volume(&self, obj: Option<PartId>) {
        if let (Some(set), Some(part)) = (&self.vp_vol_objs, obj) {
            set.borrow_mut().push(part);
        }
    }
}

/// VPX's `HitBall`: the simulated ball.
#[derive(Clone, Debug)]
pub struct HitBall {
    pub id: BallId,
    pub base: HitObjectBase,
    /// The ball's candidate collision this step; may not happen if something else comes first.
    pub coll: CollisionEvent,
    pub d: BallS,
    /// Hack for kicker hole handling only.
    pub old_vel: Vertex3Ds,
    /// Where the last hit event happened, to filter repeats.
    pub last_event_pos: Vertex3Ds,
    /// Distance travelled since the last event.
    pub last_event_sqr_dist: f32,
    pub angularmomentum: Vertex3Ds,
    pub orientation: Matrix3,
    /// Recent positions, used for killing spin and for ball trails.
    oldpos: [Vertex3Ds; MAX_BALL_TRAIL_POS],
}

impl HitBall {
    /// VPX's constructor.
    pub fn new(id: BallId) -> Self {
        let coll = CollisionEvent { ball: Some(id), ..CollisionEvent::default() };
        let mut orientation = Matrix3::default();
        orientation.set_identity(1.0);
        let mut oldpos = [Vertex3Ds::default(); MAX_BALL_TRAIL_POS];
        for p in &mut oldpos {
            p.x = f32::MAX;
        }
        Self {
            id,
            base: HitObjectBase::default(),
            coll,
            d: BallS { vp_vol_objs: Some(Rc::new(RefCell::new(Vec::new()))), ..BallS::default() },
            old_vel: Vertex3Ds::default(),
            last_event_pos: Vertex3Ds::new(-10000.0, -10000.0, -10000.0), // last pos is far far away
            last_event_sqr_dist: 0.0,
            angularmomentum: Vertex3Ds::new(0.0, 0.0, 0.0),
            orientation,
            oldpos,
        }
    }

    /// Stores the position every 10 ms, in 10 slots: 100 ms of history.
    pub fn on_physic_step_processed(&mut self, physics_time_ms: u64) {
        let pos = ((physics_time_ms / 10) % MAX_BALL_TRAIL_POS as u64) as usize; // 10ms steps
        self.oldpos[pos] = self.d.pos;
    }

    pub fn get_old_position(&self, physics_time_ms: u64) -> &Vertex3Ds {
        let pos = ((physics_time_ms / 10) % MAX_BALL_TRAIL_POS as u64) as usize; // 10ms steps
        &self.oldpos[pos]
    }

    /// `(float)(2.0/5.0) * r*r * m`: a solid sphere's moment of inertia.
    #[inline(always)]
    pub fn inertia(&self) -> f32 {
        (2.0_f64 / 5.0) as f32 * self.d.radius * self.d.radius * self.d.mass
    }

    /// The squared radius of the bounding box, extended by the velocity, as the
    /// collision search uses it.
    #[inline(always)]
    pub fn hit_radius_sqr(&self) -> f32 {
        sqrf((self.base.hit_bbox.right - self.base.hit_bbox.left) * 0.5)
    }

    /// VPX's `HitBall::Collide3DWall`: bounce off a wall with the given normal.
    pub fn collide_3d_wall(
        &mut self,
        hit_normal: &Vertex3Ds,
        mut elasticity: f32,
        elast_falloff: f32,
        friction: f32,
        mut scatter_angle: f32,
        env: &mut PhysicsEnv,
    ) {
        // speed normal to wall
        let mut dot = self.d.vel.dot(hit_normal);

        if dot >= -C_LOWNORMVEL {
            // nearly receding ... make sure of conditions; otherwise if clearly approaching .. process the collision
            if dot > C_LOWNORMVEL {
                return; // is this velocity clearly receding (i.e must > a minimum)
            }
            if self.coll.hitdistance < -C_EMBEDDED {
                dot = -C_EMBEDSHOT; // has ball become embedded???, give it a kick
            } else {
                return;
            }
        }

        // correct displacements, mostly from low velocity, alternative to acceleration processing
        let mut hdist = -C_DISP_GAIN * self.coll.hitdistance; // limit delta noise crossing ramps,
        if hdist > 1.0e-4 {
            // when hit detection checked it what was the displacement
            if hdist > C_DISP_LIMIT {
                hdist = C_DISP_LIMIT; // crossing ramps, delta noise
            }
            self.d.pos += hdist * *hit_normal; // push along norm, back to free area
        }

        // magnitude of the impulse which is just sufficient to keep the ball from
        // penetrating the wall (needed for friction computations)
        let reaction_impulse = self.d.mass * dot.abs();

        elasticity = elasticity_with_falloff(elasticity, elast_falloff, dot);
        dot *= -(1.0 + elasticity);
        self.d.vel += dot * *hit_normal; // apply collision impulse (along normal, so no torque)

        // compute friction impulse

        let surf_p = -self.d.radius * *hit_normal; // surface contact point relative to center of mass

        let surf_vel = self.surface_velocity(&surf_p); // velocity at impact point

        let mut tangent = surf_vel - surf_vel.dot(hit_normal) * *hit_normal; // calc the tangential velocity

        let tangent_sp_sq = tangent.length_squared();
        if tangent_sp_sq > 1e-6 {
            tangent /= tangent_sp_sq.sqrt(); // normalize to get tangent direction
            let vt = surf_vel.dot(&tangent); // get speed in tangential direction

            // compute friction impulse
            let cross = cross_product(&surf_p, &tangent);
            let kt = 1.0 / self.d.mass + tangent.dot(&cross_product(&(cross / self.inertia()), &surf_p));

            // friction impulse can't be greater than coefficient of friction times collision impulse (Coulomb friction cone)
            let max_fric = friction * reaction_impulse;
            let jt = clamp(-vt / kt, -max_fric, max_fric);

            if !inf_nan(jt) {
                self.apply_surface_impulse(&(jt * cross), &(jt * tangent));
            }
        }

        if scatter_angle < 0.0 {
            scatter_angle = env.hard_scatter; // if < 0 use global value
        }
        scatter_angle *= env.global_difficulty; // apply difficulty weighting

        if dot > 1.0 && scatter_angle > 1.0e-5 {
            // no scatter at low velocity
            let mut scatter = env.rng.rand_m11(); // -1.0f..1.0f
            scatter *= (1.0 - scatter * scatter) * 2.59808 * scatter_angle; // shape quadratic distribution and scale
            let radsin = scatter.sin(); // Green's transform matrix... rotate angle delta
            let radcos = scatter.cos(); // rotational transform from current position to position at time t
            let vxt = self.d.vel.x;
            let vyt = self.d.vel.y;
            self.d.vel.x = vxt * radcos - vyt * radsin; // rotate to random scatter angle
            self.d.vel.y = vyt * radcos + vxt * radsin;
        }
    }

    /// VPX's `HitBall::HitTest`: this ball, as a collider, against a moving ball.
    pub fn hit_test(&self, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        let d = self.d.pos - ball.pos; // delta position

        let dv = self.d.vel - ball.vel; // delta velocity

        let bcddsq = d.length_squared(); // square of ball center's delta distance
        let bcdd = bcddsq.sqrt(); // length of delta

        let b = dv.dot(&d); // inner product
        let bnv = b / bcdd; // normal speed of balls toward each other

        if bnv > C_LOWNORMVEL {
            return -1.0; // dot of delta velocity and delta displacement, positive if receding no collison
        }

        let totalradius = ball.radius + self.d.radius;
        let bnd = bcdd - totalradius; // distance between ball surfaces

        let hittime: f32;
        if bnd <= PHYS_TOUCH as f32 {
            // in contact???
            if bnd < ball.radius * -2.0 {
                return -1.0; // embedded too deep?
            }

            if (bnv.abs() > C_CONTACTVEL) // >fast velocity, return zero time
                //zero time for rigid fast bodies
                || (bnd <= (-PHYS_TOUCH) as f32)
            {
                hittime = 0.0; // slow moving but embedded
            } else {
                hittime = bnd * (1.0 / (2.0 * PHYS_TOUCH)) as f32 + 0.5; // don't compete for fast zero time events
            }
        } else {
            // find collision time as solution of quadratic equation
            //   at^2 + bt + c = 0
            let a = dv.length_squared(); // square of differential velocity

            if a < 1.0e-8 {
                return -1.0; // ball moving really slow, then wait for contact
            }

            let Some((time1, time2)) = solve_quadratic_eq(a, 2.0 * b, bcddsq - totalradius * totalradius) else {
                return -1.0;
            };

            hittime = if time1 * time2 < 0.0 { std_max(time1, time2) } else { std_min(time1, time2) }; // find smallest nonnegative solution
        }

        if inf_nan(hittime) || hittime < 0.0 || hittime > dtime {
            return -1.0; // .. was some time previous || beyond the next physics tick
        }

        let hit_pos = ball.pos + hittime * dv; // new ball position

        // calc unit normal of collision
        let hitnormal = hit_pos - self.d.pos;
        if hitnormal.x.abs() <= f32::MIN_POSITIVE && hitnormal.y.abs() <= f32::MIN_POSITIVE && hitnormal.z.abs() <= f32::MIN_POSITIVE {
            return -1.0;
        }

        coll.hitnormal = hitnormal;
        coll.hitnormal.normalize();

        coll.hitdistance = bnd; // actual contact distance

        hittime
    }

    /// VPX's `HitBall::Collide`: `this_ball` is the ball hit, `pball` the ball
    /// whose hit test found the collision (`coll.m_ball`). Only one of the two
    /// handles a ball-to-ball collision, decided by comparing the balls (VPX
    /// compares addresses) and alternating every cycle.
    ///
    /// VPX calls the table's ball-to-ball script callback in the middle of
    /// this function; here it is raised as an event. The engine runs the two
    /// halves, [`Self::collide_ball_begin`] and [`Self::collide_ball_finish`],
    /// with the script in between.
    pub fn collide_ball(this_ball: &mut HitBall, pball: &mut HitBall, coll: &CollisionEvent, env: &mut PhysicsEnv) {
        let Some(dot) = Self::collide_ball_begin(this_ball, pball, coll, env) else { return };
        if dot < -0.25 {
            // only collisions with at least some small true impact velocity (no contacts)
            env.events.push(PhysicsEvent::BallBallCollision { ball: this_ball.id, other: pball.id, speed: -dot });
        }
        Self::collide_ball_finish(this_ball, pball, coll, dot);
    }

    /// The first half of VPX's `HitBall::Collide`, up to the script callback:
    /// whether this ball handles the collision, and if so the normal speed of
    /// the impact. The callback follows if it is below -0.25.
    pub fn collide_ball_begin(this_ball: &HitBall, pball: &HitBall, coll: &CollisionEvent, env: &PhysicsEnv) -> Option<f32> {
        // make sure we process each ball/ball collision only once
        // (but if we are frozen, there won't be a second collision event, so deal with it now!)
        let swap = env.swap_ball_collision_handling;
        if ((swap && pball.id >= this_ball.id) || (!swap && pball.id <= this_ball.id)) && !this_ball.d.locked_in_kicker {
            return None;
        }

        // target ball to object ball delta velocity
        let vrel = pball.d.vel - this_ball.d.vel;
        let vnormal = coll.hitnormal;
        let mut dot = vrel.dot(&vnormal);

        // correct displacements, mostly from low velocity, alternative to true acceleration processing
        if dot >= -C_LOWNORMVEL {
            // nearly receding ... make sure of conditions; otherwise if clearly approaching .. process the collision
            if dot > C_LOWNORMVEL {
                return None; // is this velocity clearly receding (i.e must > a minimum)
            }
            if coll.hitdistance < -C_EMBEDDED {
                dot = -C_EMBEDSHOT; // has ball become embedded???, give it a kick
            } else {
                return None;
            }
        }
        Some(dot)
    }

    /// The second half of VPX's `HitBall::Collide`, after the script callback:
    /// separates the balls and exchanges the impulse, with the balls as the
    /// script left them and the `dot` from the first half.
    pub fn collide_ball_finish(this_ball: &mut HitBall, pball: &mut HitBall, coll: &CollisionEvent, dot: f32) {
        let vnormal = coll.hitnormal;

        let mut edist = -C_DISP_GAIN * coll.hitdistance;
        if edist > 1.0e-4 {
            if edist > C_DISP_LIMIT {
                edist = C_DISP_LIMIT; // crossing ramps, delta noise
            }
            if !this_ball.d.locked_in_kicker {
                edist *= 0.5; // if the hitten ball is not frozen
            }
            pball.d.pos += edist * vnormal; // push along norm, back to free area
        }

        edist = -C_DISP_GAIN * this_ball.coll.hitdistance; // noisy value .... needs investigation
        if !this_ball.d.locked_in_kicker && edist > 1.0e-4 {
            if edist > C_DISP_LIMIT {
                edist = C_DISP_LIMIT; // crossing ramps, delta noise
            }
            edist *= 0.5;
            this_ball.d.pos -= edist * vnormal; // pull along norm, back to free area
        }

        let my_inv_mass = if this_ball.d.locked_in_kicker { 0.0 } else { 1.0 / this_ball.d.mass }; // frozen ball has infinite mass
        let pball_inv_mass = 1.0 / pball.d.mass; // !! do same frozen mass thing for that one?
        let impulse = -((1.0_f64 + 0.8) as f32) * dot / (my_inv_mass + pball_inv_mass); // resitution = 0.8

        if !this_ball.d.locked_in_kicker {
            this_ball.d.vel -= (impulse * my_inv_mass) * vnormal;
        }
        pball.d.vel += (impulse * pball_inv_mass) * vnormal;
    }

    /// VPX's `HitBall::HandleStaticContact`: a ball resting against something.
    pub fn handle_static_contact(&mut self, coll: &CollisionEvent, friction: f32, dtime: f32, env: &PhysicsEnv) {
        let norm_vel = self.d.vel.dot(&coll.hitnormal); // this should be zero, but only up to +/- C_CONTACTVEL

        // If some collision has changed the ball's velocity, we may not have to do anything.
        if norm_vel <= C_CONTACTVEL {
            let fe = self.d.mass * env.gravity; // external forces (only gravity for now)
            let dot = fe.dot(&coll.hitnormal);
            let normal_force = std_max(0.0, -(dot * dtime + coll.hit_org_normalvelocity)); // normal force is always nonnegative

            // Add just enough to kill original normal velocity and counteract the external forces.
            self.d.vel += normal_force * coll.hitnormal;

            self.apply_friction(&coll.hitnormal, dtime, friction, env);
        }
    }

    /// VPX's `HitBall::ApplyFriction`, with `C_BALL_SPIN_HACK` (enabled in VPX).
    pub fn apply_friction(&mut self, hitnormal: &Vertex3Ds, dtime: f32, fric_coeff: f32, env: &PhysicsEnv) {
        let surf_p = -self.d.radius * *hitnormal; // surface contact point relative to center of mass

        let surf_vel = self.surface_velocity(&surf_p);
        let slip = surf_vel - surf_vel.dot(hitnormal) * *hitnormal; // calc the tangential slip velocity

        let max_fric = fric_coeff * self.d.mass * -env.gravity.dot(hitnormal);

        let slipspeed = slip.length();

        let norm_vel = self.d.vel.dot(hitnormal);
        let (slip_dir, numer) = if (norm_vel <= 0.025) // check for <=0.025 originated from ball<->rubber collisions pushing the ball upwards
            || (slipspeed < C_PRECISION)
        {
            // slip speed zero - static friction case
            let surf_acc = self.surface_acceleration(&surf_p, env);
            let slip_acc = surf_acc - surf_acc.dot(hitnormal) * *hitnormal; // calc the tangential slip acceleration

            // neither slip velocity nor slip acceleration? nothing to do here
            if slip_acc.length_squared() < 1e-6 {
                return;
            }

            let mut slip_dir = slip_acc;
            slip_dir.normalize();

            (slip_dir, -slip_dir.dot(&surf_acc))
        } else {
            // nonzero slip speed - dynamic friction case
            let slip_dir = slip / slipspeed;

            (slip_dir, -slip_dir.dot(&surf_vel))
        };

        let cp = cross_product(&surf_p, &slip_dir);
        let denom = 1.0 / self.d.mass + slip_dir.dot(&cross_product(&(cp / self.inertia()), &surf_p));
        let fric = clamp(numer / denom, -max_fric, max_fric);

        if !inf_nan(fric) {
            self.apply_surface_impulse(&((dtime * fric) * cp), &((dtime * fric) * slip_dir));
        }
    }

    /// Linear velocity plus tangential velocity due to rotation.
    pub fn surface_velocity(&self, surf_p: &Vertex3Ds) -> Vertex3Ds {
        self.d.vel + cross_product(&(self.angularmomentum / self.inertia()), surf_p)
    }

    pub fn surface_acceleration(&self, surf_p: &Vertex3Ds, env: &PhysicsEnv) -> Vertex3Ds {
        let angularvelocity = self.angularmomentum / self.inertia();
        // if we had any external torque, we would have to add "(deriv. of ang.vel.) x surfP" here
        env.gravity / self.d.mass // linear acceleration
            + cross_product(&angularvelocity, &cross_product(&angularvelocity, surf_p)) // centripetal acceleration
    }

    pub fn apply_surface_impulse(&mut self, rot_i: &Vertex3Ds, impulse: &Vertex3Ds) {
        self.d.vel += *impulse / self.d.mass;
        self.angularmomentum += *rot_i;
    }

    /// The bounding box, grown by the speed so a fast ball is still found.
    pub fn calc_hit_bbox(&mut self) {
        let vl = self.d.vel.length() + self.d.radius + 0.05; // !! 0.05f = paranoia
        self.base.hit_bbox.left = self.d.pos.x - vl;
        self.base.hit_bbox.right = self.d.pos.x + vl;
        self.base.hit_bbox.top = self.d.pos.y - vl;
        self.base.hit_bbox.bottom = self.d.pos.y + vl;
        self.base.hit_bbox.zlow = self.d.pos.z - vl;
        self.base.hit_bbox.zhigh = self.d.pos.z + vl;
    }

    /// VPX's `HitBall::UpdateDisplacements`: moves and spins the ball.
    pub fn update_displacements(&mut self, dtime: f32) {
        if !self.d.locked_in_kicker {
            let ds = dtime * self.d.vel;
            self.d.pos += ds;

            self.last_event_sqr_dist += ds.length_squared();

            let mut mat3 = Matrix3::default();
            mat3.skew_symmetric(&(self.angularmomentum / self.inertia()));

            let addedorientation = Matrix3::mul_matrices_and_mul_scalar(&mat3, &self.orientation, dtime);

            self.orientation = self.orientation + addedorientation;
            self.orientation.ortho_normalize();
        }
    }

    /// VPX's `HitBall::UpdateVelocities`: gravity and nudging, once per step.
    /// VPX's live-editor ball dragging is not ported.
    pub fn update_velocities(&mut self, env: &PhysicsEnv) {
        if !self.d.locked_in_kicker {
            // Gravity
            // Apply forces (expressed in VPU/VPT) integrated on one physic step (PHYS_FACTOR is one physic step time expressed in VPX time unit)
            self.d.vel += (PHYS_FACTOR as f32) * env.gravity; // Gravity F = m.G

            // Table velocity due to nudge (fictitious force due to change of reference frame, therefore mass is not applied)
            let slope = ang_to_rad(env.playfield_slope_deg); // nudge acceleration is in the horizontal cabinet plane, reference frame is the playfield
            let cabinet_acceleration = env.cabinet_acceleration;
            self.d.vel.x -= (PHYS_FACTOR as f32) * ms2tovpuvpt2(cabinet_acceleration.x);
            self.d.vel.y -= (PHYS_FACTOR as f32) * ms2tovpuvpt2(cabinet_acceleration.y) * slope.cos();
            self.d.vel.z -= (PHYS_FACTOR as f32) * ms2tovpuvpt2(cabinet_acceleration.y) * slope.sin();
        }
    }
}
