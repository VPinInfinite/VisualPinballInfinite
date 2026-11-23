// Ported from Visual Pinball X: src/parts/kicker.cpp and kicker.h (license:GPLv3+): KickerHitCircle,
// the physics part of Kicker::PhysicSetup, and the script's CreateSizedBallWithMass and KickXYZ
// (with Player::CreateBall), commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's kicker: a hole that catches the ball (or lets it fall through) and
//! fires its hit event; outside legacy mode, a ball rolling over the rim
//! without dropping in is steered by the shape of the hole.

use crate::collide::{CollisionEvent, HitCircle, HitObjectBase};
use crate::engine::{PhysicsEngine, PhysicsHost, Shape};
use crate::env::{BallId, PartId, PhysicsEvent};
use crate::hitball::{BallS, HitBall};
use crate::math::def::{ang_to_rad, clamp};
use crate::math::{Vertex2D, Vertex3Ds, cross_product};
use crate::meshes::kicker_hit_mesh::KICKER_HIT_MESH;
use crate::physconst::STATICTIME;

/// The kicker part's settings VPX reads during a collision (`Kicker::m_d`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KickerData {
    /// The old kicker behaviour: a smaller catch circle, no steering on the rim.
    pub legacy_mode: bool,
    /// How low a ball must be to be caught, as a fraction of the catch height.
    pub hit_accuracy: f32,
    /// The ball falls through instead of being held.
    pub fall_through: bool,
}

/// VPX's `KickerHitCircle`.
#[derive(Clone, Debug, PartialEq)]
pub struct KickerHitCircle {
    pub circle: HitCircle,
    pub part: PartId,
    pub data: KickerData,
    /// The hole's shape, scaled and placed (`Kicker::m_hitMesh`); empty in legacy mode.
    pub hit_mesh: Vec<Vertex3Ds>,
    /// The ball held in the kicker (`m_pHitBall`).
    pub hit_ball: Option<BallId>,
    /// The last ball caught; kept when the ball leaves (`m_lastCapturedBall`).
    pub last_captured_ball: Option<BallId>,
}

impl KickerHitCircle {
    /// The play-mode part of VPX's `Kicker::PhysicSetup`: the catch circle,
    /// smaller in legacy mode, and the hole's mesh. `kicker_radius` is the
    /// part's radius; `height` the surface it sits on. The collider's hit
    /// fields are `HitObjectBase::with_z(height, height + hit_height)`, with
    /// the kicker as its object, enabled as the part is.
    pub fn new(part: PartId, data: KickerData, center: Vertex2D, kicker_radius: f32, height: f32) -> Self {
        // reduce the hit circle radius because only the inner circle of the kicker should start a hit event
        let radius = kicker_radius
            * if data.legacy_mode {
                if data.fall_through { 0.75 } else { 0.6 }
            } else {
                1.0
            };
        let circle = HitCircle::new(center, radius);
        let hit_mesh = if data.legacy_mode {
            Vec::new()
        } else {
            let rad = circle.radius * 0.8;
            KICKER_HIT_MESH
                .iter()
                .map(|v| Vertex3Ds::new(v[0] * rad + center.x, v[1] * rad + center.y, v[2] * rad + height))
                .collect()
        };
        Self { circle, part, data, hit_mesh, hit_ball: None, last_captured_ball: None }
    }

    /// VPX's `KickerHitCircle::HitTest`: any face, not lateral, not rigid.
    pub fn hit_test(&self, base: &HitObjectBase, ball: &BallS, dtime: f32, coll: &mut CollisionEvent) -> f32 {
        self.circle.hit_test_basic_radius(base, ball, dtime, coll, false, false, false)
    }

    /// VPX's `KickerHitCircle::DoChangeBallVelocity`: a ball on the rim is
    /// deflected by the hole's surface nearest to it.
    pub fn do_change_ball_velocity(&self, pball: &mut HitBall, hitnormal: &Vertex3Ds) {
        let mut min_dist_sqr = f32::MAX;
        let mut idx = None;
        for (t, v) in self.hit_mesh.iter().enumerate() {
            // find the right normal by calculating the distance from current ball position to vertex of the kicker mesh
            let length_sqr = (pball.d.pos - *v).length_squared();
            if length_sqr < min_dist_sqr {
                min_dist_sqr = length_sqr;
                idx = Some(t);
            }
        }

        if let Some(idx) = idx {
            // we have the nearest vertex now use the normal and damp it so it doesn't speed up the ball velocity too much
            let m = &KICKER_HIT_MESH[idx];
            let hitnorm = Vertex3Ds::new(m[3], m[4], m[5]);
            let dot = -pball.d.vel.dot(&hitnorm);
            let reaction_impulse = pball.d.mass * dot.abs();

            let surf_p = -pball.d.radius * *hitnormal; // surface contact point relative to center of mass

            let surf_vel = pball.surface_velocity(&surf_p); // velocity at impact point

            let mut tangent = surf_vel - surf_vel.dot(hitnormal) * hitnorm; // calc the tangential velocity

            pball.d.vel += dot * hitnorm; // apply collision impulse (along normal, so no torque)

            let friction = 0.3f32;
            let tangent_sp_sq = tangent.length_squared();

            if tangent_sp_sq > 1e-6 {
                tangent /= tangent_sp_sq.sqrt(); // normalize to get tangent direction
                let vt = surf_vel.dot(&tangent); // get speed in tangential direction

                // compute friction impulse
                let cross = cross_product(&surf_p, &tangent);
                let kt = 1.0 / pball.d.mass + tangent.dot(&cross_product(&(cross / pball.inertia()), &surf_p));

                // friction impulse can't be greater than coefficient of friction times collision impulse (Coulomb friction cone)
                let max_fric = friction * reaction_impulse;
                let jt = clamp(-vt / kt, -max_fric, max_fric);

                pball.apply_surface_impulse(&(jt * cross), &(jt * tangent));
            }
        }
    }
}

impl PhysicsEngine {
    fn kicker(&mut self, k: u32) -> &mut KickerHitCircle {
        match &mut self.collider_mut(k).shape {
            Shape::Kicker(kc) => kc,
            _ => unreachable!("collider {k} is not a kicker"),
        }
    }

    /// VPX's `KickerHitCircle::DoCollide`: a ball enters the kicker's circle
    /// (caught, falling through, or steered on the rim) or leaves it. The hit
    /// event's script runs where VPX runs it, before the kicker settles the
    /// ball, and may already have kicked it out. `new_ball` is for a
    /// ball the kicker itself creates.
    pub(crate) fn kicker_do_collide(
        &mut self,
        k: u32,
        ball: BallId,
        hitnormal: Vertex3Ds,
        hitbit: bool,
        new_ball: bool,
        host: &mut dyn PhysicsHost,
    ) {
        let zlow = self.collider_mut(k).base.hit_bbox.zlow;
        let obj = self.collider_mut(k).base.obj;
        let kc = self.kicker(k);
        if kc.hit_ball.is_some() {
            return; // a previous ball already in kicker
        }
        let (part, data, center) = (kc.part, kc.data, kc.circle.center);

        let Some(pball) = self.ball_mut(ball) else { return };
        let Some(set) = pball.d.vp_vol_objs.clone() else { return };
        let i = obj.and_then(|o| set.borrow().iter().position(|p| *p == o)); // check if kicker in ball's volume set

        if !(new_ball || ((!hitbit) == i.is_none())) {
            return; // New or (Hit && !Vol || UnHit && Vol)
        }

        if data.legacy_mode || new_ball {
            pball.d.pos += STATICTIME * pball.d.vel; // move ball slightly forward
        }

        let Some(i) = i else {
            // entering Kickers volume
            let grab_height = (zlow + pball.d.radius) * data.hit_accuracy;
            // early out here if the ball is slow and we are near the kicker center
            let hit_event = pball.d.pos.z < grab_height || data.legacy_mode || new_ball;

            if !hit_event {
                let (collider, pball) = self.collider_and_ball(k, ball).expect("ball");
                if let Shape::Kicker(kc) = &collider.shape {
                    kc.do_change_ball_velocity(pball, &hitnormal);
                }

                // this is an ugly hack to prevent the ball stopping rapidly at the kicker bevel
                // something with the friction calculation is wrong in the physics engine
                // so we monitor the ball velocity if it drop under a length value of 0.2
                // if so we take the last "good" velocity to help the ball moving over the critical spot at the kicker bevel
                // this hack seems to work only if the kicker is on the playfield, a kicker attached to a wall has still problems
                // because the friction calculation for a wall is also different
                if pball.d.vel.length_squared() < (0.2_f64 * 0.2) as f32 {
                    pball.d.vel = pball.old_vel;
                }

                pball.old_vel = pball.d.vel;
                return;
            }

            let pball = self.ball_mut(ball).expect("ball");
            pball.d.locked_in_kicker = !data.fall_through;
            let locked = pball.d.locked_in_kicker;
            if locked {
                if let Some(o) = obj {
                    set.borrow_mut().push(o); // add kicker to ball's volume set
                }
                let kc = self.kicker(k);
                kc.hit_ball = Some(ball);
                kc.last_captured_ball = Some(ball);
                // (VPX's live editor also stops dragging this ball; not ported)
            }

            // Don't fire the hit event if the ball was just created
            // Fire the event before changing ball attributes, so scripters can get a useful ball state
            if !new_ball {
                self.env.events.push(PhysicsEvent::Hit { part });
                self.deliver(host);
            }

            // the script may have kicked the ball out, changed the kicker, or destroyed the ball (VPX
            // would go on with the destroyed ball's last state; here only the kicker's own state follows)
            let still_locked = self.ball_mut(ball).map_or(locked, |b| b.d.locked_in_kicker);
            let data = self.kicker(k).data;
            if still_locked || data.fall_through {
                // script may have unfrozen the ball
                // if ball falls through hole, we fake the collision algo by changing the ball height
                // in HitTestBasicRadius() the z-position of the ball is checked if it is >= to the hit cylinder
                // if we don't change the height of the ball we get a lot of hit events while the ball is falling!!

                // Only mess with variables if ball was not kicked during event
                if let Some(pball) = self.ball_mut(ball) {
                    pball.d.vel.set_zero();
                    pball.angularmomentum.set_zero();
                    pball.d.pos.x = center.x;
                    pball.d.pos.y = center.y;
                    if data.fall_through {
                        pball.d.pos.z = zlow - pball.d.radius - 5.0;
                    } else {
                        pball.d.pos.z = zlow + pball.d.radius;
                    }
                }
            } else {
                self.kicker(k).hit_ball = None; // make sure
            }
            return;
        };

        // exiting kickers volume
        set.borrow_mut().remove(i); // remove kicker to ball's volume set
        self.env.events.push(PhysicsEvent::Unhit { part });
        self.deliver(host);
    }

    /// VPX's `Kicker::CreateSizedBallWithMass` (with `Player::CreateBall`): a
    /// new ball on the kicker's surface at its center, held by the kicker. No
    /// hit event fires for it.
    pub fn kicker_create_ball(&mut self, k: u32, id: BallId, radius: f32, mass: f32, host: &mut dyn PhysicsHost) {
        let height = self.collider_mut(k).base.hit_bbox.zlow; // the kicker's surface height
        let center = self.kicker(k).circle.center;
        let mut ball = HitBall::new(id);
        ball.d.pos = Vertex3Ds::new(center.x, center.y, height + radius);
        ball.d.mass = mass;
        ball.d.radius = radius;
        ball.d.vel = Vertex3Ds::new(0.1, 0.0, 0.0);
        self.add_ball(ball);
        if let Some(b) = self.ball_mut(id) {
            b.coll.hitflag = true; // HACK: avoid capture leaving kicker
        }
        let hitnormal = Vertex3Ds::new(f32::MAX, f32::MAX, f32::MAX); // unused due to newBall being true
        self.kicker_do_collide(k, id, hitnormal, false, true, host);
    }

    /// VPX's `Kicker::KickXYZ`: kicks out the ball the kicker holds, if any,
    /// `angle` degrees from straight up the table at `speed`, raised by
    /// `inclination` (radians, or degrees when above a quarter turn), moved
    /// by `offset` first. `scatter_deg` is the kicker's scatter (negative: the
    /// table's), drawn from VPX's shared random generator (`rand_mt_m11`).
    #[allow(clippy::too_many_arguments)] // KickXYZ's arguments, plus what VPX reads from the part
    pub fn kicker_kick(&mut self, k: u32, angle: f32, mut speed: f32, mut inclination: f32, offset: Vertex3Ds, scatter_deg: f32) {
        let Some(id) = self.kicker(k).hit_ball else { return };
        let mut anglerad = ang_to_rad(angle); // yaw angle, zero is along -Y axis

        if inclination.abs() > (std::f64::consts::PI / 2.0) as f32 {
            // radians or degrees?  if greater PI/2 assume degrees
            inclination *= (std::f64::consts::PI / 180.0) as f32; // convert to radians
        }

        let mut scatter_angle = if scatter_deg < 0.0 { self.env.hard_scatter } else { ang_to_rad(scatter_deg) }; // if < 0 use global value
        scatter_angle *= self.env.global_difficulty; // apply dificulty weighting

        if scatter_angle > 1.0e-5 {
            // ignore near zero angles
            let mut scatter = self.env.rng.rand_m11(); // -1.0f..1.0f
            scatter *= (1.0 - scatter * scatter) * 2.59808 * scatter_angle; // shape quadratic distribution and scale
            anglerad += scatter;
        }

        let speedz = inclination.sin() * speed;
        if speedz > 0.0 {
            speed *= inclination.cos();
        }

        if let Some(b) = self.ball_mut(id) {
            b.angularmomentum.set_zero();
            b.coll.hitdistance = 0.0;
            b.coll.hittime = -1.0;
            b.coll.hitnormal.set_zero();
            b.coll.hitvel = Vertex2D::default();
            b.coll.hitflag = false;
            b.coll.is_contact = false;
            b.coll.hitmoment_bit = true;
            b.d.pos.x += offset.x; // brian's suggestion
            b.d.pos.y += offset.y;
            b.d.pos.z += offset.z;
            b.d.vel.x = anglerad.sin() * speed;
            b.d.vel.y = -anglerad.cos() * speed;
            b.d.vel.z = speedz;
            b.d.locked_in_kicker = false;
        }
        self.kicker(k).hit_ball = None;
    }

    /// VPX's `Kicker::DestroyBall`: removes the ball the kicker holds, if
    /// any, and returns it.
    pub fn kicker_destroy_ball(&mut self, k: u32) -> Option<HitBall> {
        let id = self.kicker(k).hit_ball.take()?;
        self.remove_ball(id)
    }
}
