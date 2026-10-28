// Ported from Visual Pinball X: src/physics/hitplunger.cpp and hitplunger.h (license:GPLv3+),
// and the physics part of src/parts/plunger.cpp (PhysicSetup), commit af26b2d (10.8.1 pre-release
// build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's plunger: a rod in a lane, pulled back and fired by the script (the
//! launch key) or following a cabinet's mechanical plunger, with a moving tip
//! that hits the ball and fixed sides and base the ball bounces off.

use crate::collide::{CollisionEvent, HitLineZ, HitObjectBase, LineSeg};
use crate::env::{PartId, PhysicsEnv, PhysicsEvent};
use crate::hitball::{BallS, HitBall};
use crate::math::Vertex2D;
use crate::physconst::{C_DISP_GAIN, C_DISP_LIMIT, C_EMBEDDED, C_EMBEDSHOT, C_LOWNORMVEL};
use std::cell::Cell;

/// `PLUNGERHEIGHT`: the plunger's height above its surface.
const PLUNGERHEIGHT: f32 = 50.0;

/// The plunger part's settings the physics reads (`Plunger::m_d`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlungerData {
    /// Firing strength (`m_speedFire`).
    pub speed_fire: f32,
    /// The plunger follows a cabinet's mechanical plunger (`m_mechPlunger`).
    pub mech_plunger: bool,
    /// An automatic launcher: a pull-and-release of the mechanical plunger
    /// presses the launch key instead (`m_autoPlunger`).
    pub auto_plunger: bool,
    /// How strongly the plunger follows the mechanical one (`m_mechStrength`).
    pub mech_strength: f32,
    /// Momentum transfer to the ball (`m_momentumXfer`).
    pub momentum_xfer: f32,
}

/// A cabinet's plunger sensor, as VPX's `PlungerHandler` presents it. None
/// without one (and in VPX's tester). Its readings change once per physics
/// step, before the movers update.
pub trait PlungerSensor: std::fmt::Debug {
    /// Is a plunger sensor present? (`HasPlungerSensor`)
    fn has_sensor(&self) -> bool;
    /// The mechanical plunger's position, 0 forward to 1 fully pulled
    /// (`GetPosition(restPos)`).
    fn position(&self, rest_pos: f32) -> f32;
    /// Its speed, in travel lengths per second (`GetRawVelocity`).
    fn raw_velocity(&self) -> f32;
    /// Its speed at the moment it struck, or 0 (`GetHitVelocity(restPos)`).
    fn hit_velocity(&self, rest_pos: f32) -> f32;
    /// Takes the sensor's readings up to this physics step
    /// (`StepOneMillisecond`, before the movers update).
    fn step_one_millisecond(&self) {}
}

/// VPX's `PlungerMoverObject`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlungerMover {
    pub data: PlungerData,
    pub x: f32,
    pub x2: f32,
    pub y: f32,
    pub lineseg_base: LineSeg,
    pub lineseg_end: LineSeg,
    pub lineseg_side: [LineSeg; 2],
    pub joint_base: [HitLineZ; 2],
    pub joint_end: [HitLineZ; 2],
    /// The segments' and joints' own hit fields (their height range).
    pub part_hit: HitObjectBase,

    /// The tip's position, in VP units along the lane.
    pub pos: f32,
    pub speed: f32,
    /// How far forward the tip may move in the next update. VPX's hit test
    /// sets it (from inside a const function), so it's a cell.
    pub travel_limit: Cell<f32>,
    pub pull_force: f32,
    pub reverse_impulse: f32,
    pub fire_timer: i32,
    pub fire_speed: f32,
    pub auto_fire_timer: i32,
    pub fire_bounce: f32,
    /// The park position, 0 forward to 1 retracted.
    pub rest_pos: f32,
    pub frame_start: f32,
    pub frame_end: f32,
    pub frame_len: f32,
    pub stroke_events_armed: bool,
    pub scatter_velocity: f32,
    pub initial_speed: f32,
    pub retract_wait_loop: i32,
    pub add_retract_motion: bool,
    pub retract_motion: bool,
}

impl PlungerMover {
    /// `PlungerMoverObject::m_mass`.
    pub const MASS: f32 = 30.0;

    /// VPX's `SetObjects`: places the tip's segments at `len`.
    pub fn set_objects(&mut self, len: f32) {
        self.lineseg_base.v1.x = self.x;
        self.lineseg_base.v1.y = self.y;
        self.lineseg_base.v2.x = self.x2;
        self.lineseg_base.v2.y = self.y; // + 0.0001f;

        self.joint_base[0].xy.x = self.x;
        self.joint_base[0].xy.y = self.y;
        self.joint_base[1].xy.x = self.x2;
        self.joint_base[1].xy.y = self.y; // + 0.0001f;

        self.lineseg_side[0].v2.x = self.x;
        self.lineseg_side[0].v2.y = self.y;
        self.lineseg_side[0].v1.x = self.x + 0.0001;
        self.lineseg_side[0].v1.y = len;

        self.lineseg_side[1].v1.x = self.x2;
        self.lineseg_side[1].v1.y = self.y;
        self.lineseg_side[1].v2.x = self.x2 + 0.0001;
        self.lineseg_side[1].v2.y = len;

        self.lineseg_end.v2.x = self.x;
        self.lineseg_end.v2.y = len;
        self.lineseg_end.v1.x = self.x2;
        self.lineseg_end.v1.y = len; // + 0.0001f;

        self.joint_end[0].xy.x = self.x;
        self.joint_end[0].xy.y = len;
        self.joint_end[1].xy.x = self.x2;
        self.joint_end[1].xy.y = len; // + 0.0001f;

        self.lineseg_base.calc_normal_and_length();
        self.lineseg_end.calc_normal_and_length();
        self.lineseg_side[0].calc_normal_and_length();
        self.lineseg_side[1].calc_normal_and_length();
    }

    /// VPX's `PlungerMoverObject::UpdateDisplacements` up to its stroke
    /// events: moves the tip, with the travel limit, the firing bounce and the
    /// stroke limits, and raises a start- or end-of-stroke event. VPX places
    /// the tip's segments after the event's script has run (a script can fire
    /// the plunger there), so the caller delivers the event, then calls
    /// [`Self::finish_update`].
    pub fn update_displacements(&mut self, dtime: f32, part: PartId, env: &mut PhysicsEnv) {
        // figure the travel distance
        let dx = dtime * self.speed;

        // figure the position change
        self.pos += dx;

        // apply the travel limit
        if self.pos < self.travel_limit.get() {
            self.pos = self.travel_limit.get();
        }

        // if we're in firing mode and we've crossed the bounce position, reverse course
        let rel_pos = (self.pos - self.frame_end) / self.frame_len;
        let bounce_pos = self.rest_pos + self.fire_bounce;
        if self.fire_timer != 0
            && dtime != 0.0
            && (if self.fire_speed < 0.0 { rel_pos <= bounce_pos } else { rel_pos >= bounce_pos })
        {
            // the controller rumbles with the plunger's speed (VPX holds it to 0..1)
            env.rumble = Some((self.fire_speed * 0.05, self.fire_speed * 0.05, 50));

            // stop at the bounce position
            self.pos = self.frame_end + bounce_pos * self.frame_len;

            // reverse course at reduced speed
            self.fire_speed = -self.fire_speed * 0.4;

            // figure the new bounce as a fraction of the previous bounce
            self.fire_bounce *= -0.4;
        }

        // apply the travel limit (again)
        if self.pos < self.travel_limit.get() {
            self.pos = self.travel_limit.get();
        }

        // limit motion to the valid range
        if dtime != 0.0 {
            if self.pos < self.frame_end {
                self.speed = 0.0;
                self.pos = self.frame_end;
            } else if self.pos > self.frame_start {
                self.speed = 0.0;
                self.pos = self.frame_start;
            }

            // apply the travel limit (yet again)
            if self.pos < self.travel_limit.get() {
                self.pos = self.travel_limit.get();
            }
        }

        // the travel limit applies to one displacement update only - reset it
        self.travel_limit.set(self.frame_end);

        // fire an Start/End of Stroke events, as appropriate
        let stroke_event_limit = self.frame_len / 50.0;
        let stroke_event_hysteresis = stroke_event_limit * 2.0;
        if self.stroke_events_armed && self.pos + dx > self.frame_start - stroke_event_limit {
            env.events.push(PhysicsEvent::LimitBos { part, speed: self.speed.abs() });
            self.stroke_events_armed = false;
        } else if self.stroke_events_armed && self.pos + dx < self.frame_end + stroke_event_limit {
            env.events.push(PhysicsEvent::LimitEos { part, speed: self.speed.abs() });
            self.stroke_events_armed = false;
        } else if self.pos > self.frame_end + stroke_event_hysteresis && self.pos < self.frame_start - stroke_event_hysteresis {
            // away from the limits - arm the stroke events
            self.stroke_events_armed = true;
        }
    }

    /// The end of VPX's `UpdateDisplacements`, after the stroke event's
    /// script: places the tip's segments ("update the display").
    pub fn finish_update(&mut self) {
        self.set_objects(self.pos);
    }

    /// VPX's `PullBack`: the script starts pulling the plunger back.
    pub fn pull_back(&mut self, speed: f32) {
        // start the pull by applying the artificial "pull force"
        self.speed = 0.0;
        self.pull_force = speed;

        // deactivate the retract code
        self.add_retract_motion = false;
        self.retract_motion = false;
        self.initial_speed = speed;
    }

    /// VPX's `PullBackandRetract`: pull back, wait, ease forward, repeat.
    pub fn pull_back_and_retract(&mut self, speed: f32) {
        // start the pull by applying the artificial "pull force"
        self.speed = 0.0;
        self.pull_force = speed;

        // check if we're acting as an auto plunger
        let auto_plunger = self.data.auto_plunger;
        // (de)activate the retract code
        self.add_retract_motion = !auto_plunger;
        self.retract_motion = false;
        self.initial_speed = speed;
    }

    /// VPX's `Fire(startPos)`: release from `start_pos` (1 = fully retracted).
    pub fn fire_from(&mut self, mut start_pos: f32) {
        // cancel any pull force
        self.pull_force = 0.0;

        // make sure the starting point is behind the park position
        if start_pos < self.rest_pos {
            start_pos = self.rest_pos;
        }

        // move immediately to the starting position
        self.pos = self.frame_end + (start_pos * self.frame_len);

        // Figure the release speed as a fraction of the fire speed property, linearly proportional
        // to the starting distance.  Note that the release motion is upwards, so the speed is negative.
        let dx = start_pos - self.rest_pos;
        self.fire_speed = -(self.data.speed_fire / 100.0) * self.frame_len * (dx * (100.0 / 13.0)) / Self::MASS;

        // Figure the target stopping position for the bounce off of the barrel spring.  Treat this
        // as proportional to the pull distance, but max out (i.e., go all the way to the forward
        // travel limit, position 0.0) if the pull position is more than about halfway.
        let max_pull = 0.5f32;
        let bounce_dist = if dx < max_pull { dx / max_pull } else { 1.0 };

        // the initial bounce will be negative, since we're moving upwards, and we calculated it as a
        // fraction of the forward travel distance (which is the part between 0 and the rest position)
        self.fire_bounce = -bounce_dist * self.rest_pos;

        // enter Fire mode for long enough for the process to complete
        self.fire_timer = 200;

        self.retract_motion = false;
    }

    /// VPX's `Fire()`: release from where the tip is.
    pub fn fire(&mut self) {
        self.fire_from((self.pos - self.frame_end) / (self.frame_start - self.frame_end));
    }

    /// VPX's `PlungerMoverObject::UpdateVelocities`: firing, the script's
    /// pull, or following the mechanical plunger. A pull-and-release of the
    /// mechanical plunger on an auto plunger raises the launch key instead.
    pub fn update_velocities(&mut self, part: PartId, sensor: Option<&dyn PlungerSensor>, env: &mut PhysicsEnv) {
        // figure our current position in relative coordinates (0.0-1.0, where 0.0 is the maximum forward
        // position and 1.0 is the maximum retracted position)
        let pos = (self.pos - self.frame_end) / self.frame_len;

        let is_mech = self.data.mech_plunger;

        let auto_plunger = self.data.auto_plunger;

        // Evaluate plunger speed
        if self.fire_timer > 0 {
            // Fire mode.  In this mode, we're moving freely under the spring forces at the speed we
            // calculated when we initiated the release.  Simply leave the speed unchanged.
            //
            // Decrement the release mode timer.  The mode ends after the timeout elapses, even if the
            // mech plunger hasn't actually come to rest.
            self.speed = self.fire_speed;
            self.fire_timer -= 1;
        } else if self.auto_fire_timer > 0 {
            // The Auto Fire timer is running.  When the timer reaches zero, we'll send the corresponding
            // KeyUp event and cancel the timer.
            self.auto_fire_timer -= 1;
            if self.auto_fire_timer == 0 {
                env.events.push(PhysicsEvent::LaunchBallKey { part, pressed: false });
            }
        } else if is_mech
            && auto_plunger
            && sensor.is_some_and(|s| s.raw_velocity() * vpu_to_m(inches_to_vpu(3.0)) < -1.0)
        {
            // Release motion detected in Auto Plunger mode: simulate a Launch Ball event, by sending a
            // KeyDown to the table script, followed by the KeyUp 100 ms later.
            env.events.push(PhysicsEvent::LaunchBallKey { part, pressed: true });
            self.auto_fire_timer = 101; // start the timer to send the corresponding KeyUp in 100ms
        } else if self.pull_force != 0.0 {
            // A "pull" force is in effect: simply update the model speed by applying the acceleration due
            // to the pull force.
            self.speed += self.pull_force / Self::MASS;

            if !self.add_retract_motion {
                // this is the normal PullBack branch

                // if we're already at the maximum retracted position, stop
                if self.pos > self.frame_start {
                    self.speed = 0.0;
                    self.pos = self.frame_start;
                }

                // if we're already at the minimum retracted position, stop
                if self.pos < (self.frame_end + (self.rest_pos * self.frame_len)) {
                    self.speed = 0.0;
                    self.pos = self.frame_end + (self.rest_pos * self.frame_len);
                }
            } else {
                // this is the PullBackandRetract branch

                // after reaching the max. position the plunger should retract until it reaches the min.
                // position and then start again
                // if we're already at the maximum retracted position, reverse
                if (self.pos >= self.frame_start) && (self.pull_force > 0.0) {
                    self.speed = 0.0;
                    self.pos = self.frame_start;
                    self.retract_wait_loop += 1;
                    if self.retract_wait_loop > 1000 {
                        // 1 sec, related to PHYSICS_STEPTIME
                        self.pull_force = -self.initial_speed;
                        self.pos = self.frame_start;
                        self.retract_motion = true;
                        self.retract_wait_loop = 0;
                    }
                }
                // if we're already at the minimum retracted position, start again
                if (self.pos <= (self.frame_end + (self.rest_pos * self.frame_len))) && (self.pull_force <= 0.0) {
                    self.speed = 0.0;
                    self.pull_force = self.initial_speed;
                    self.pos = self.frame_end + (self.rest_pos * self.frame_len);
                }
                // reset retract motion indicator only after the rest position has been left, to avoid ball
                // interactions; use a linear pullback motion
                if (self.pos > (1.0 + self.frame_end + (self.rest_pos * self.frame_len))) && (self.pull_force > 0.0) {
                    self.retract_motion = false;
                    self.speed = 3.0 * self.pull_force; // 3 = magic
                }
            }
        } else if is_mech {
            // Normal mode, we simply want to make the on-screen plunger sync up with the position of the
            // physical plunger, as though connected to it by a spring, with some damping.
            let mech = sensor.map_or(self.rest_pos, |s| s.position(self.rest_pos));

            // for an auto-plunger, go to the rest position; otherwise, sync to the mechanical plunger input
            let target = if auto_plunger { self.rest_pos } else { mech };
            let dx = target - pos;

            let plunger_friction = 0.95f32;
            let dt = 0.1f32; // 1ms in VPT
            self.speed *= plunger_friction;
            self.speed += dt * self.data.mech_strength * dx * self.frame_len / (Self::MASS * 13.0);

            // add any reverse impulse to the result
            self.speed += self.reverse_impulse;
        }

        // cancel any reverse impulse
        self.reverse_impulse = 0.0;
    }
}

/// VPX's `INCHESTOVPU`.
fn inches_to_vpu(x: f32) -> f32 {
    x * (50.0_f64 / 1.0625) as f32
}

fn vpu_to_m(x: f32) -> f32 {
    x * (0.0254 * 1.0625 / 50.0_f64) as f32
}

/// VPX's `HitPlunger`.
#[derive(Clone, Debug, PartialEq)]
pub struct HitPlunger {
    pub part: PartId,
    pub mover: PlungerMover,
    /// The tip touched a ball since the engine last looked (VPX's
    /// `NotifyPlungerBallContact`, called from its hit test; for the UI).
    pub touched: Cell<bool>,
}

impl HitPlunger {
    /// VPX's `Plunger::PhysicSetup` and `HitPlunger` constructor, from the
    /// part's lane: `center` (`m_v`), `width`, `height` (`m_height`), `stroke`,
    /// the surface height under it, the park position and scatter velocity.
    /// Returns the collider and its hit fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        part: PartId,
        data: PlungerData,
        center: Vertex2D,
        width: f32,
        height: f32,
        stroke: f32,
        zheight: f32,
        park_position: f32,
        scatter_velocity: f32,
    ) -> (Self, HitObjectBase) {
        let (x, y, x2) = (center.x - width, center.y + height, center.x + width);
        let (frame_top, frame_bottom) = (center.y - stroke, center.y);
        let part_hit = HitObjectBase::with_z(zheight, zheight + PLUNGERHEIGHT);
        let joint = |z: f32| HitLineZ::new(Vertex2D::default(), z, z + PLUNGERHEIGHT);
        let rest_pos = park_position; // The rest position is taken from the "park position" property
        let frame_len = frame_bottom - frame_top;
        let mut mover = PlungerMover {
            data,
            x,
            x2,
            y,
            lineseg_base: LineSeg::default(),
            lineseg_end: LineSeg::default(),
            lineseg_side: [LineSeg::default(); 2],
            joint_base: [joint(zheight); 2],
            joint_end: [joint(zheight); 2],
            part_hit,
            pos: frame_top + (rest_pos * frame_len), // start at the rest position
            speed: 0.0,
            travel_limit: Cell::new(frame_top),
            pull_force: 0.0,
            reverse_impulse: 0.0,
            fire_timer: 0,
            fire_speed: 0.0,
            auto_fire_timer: 0,
            fire_bounce: 0.0, // VPX leaves it unset until the first firing
            rest_pos,
            frame_start: frame_bottom,
            frame_end: frame_top,
            frame_len,
            stroke_events_armed: false,
            scatter_velocity,
            initial_speed: 0.0, // VPX leaves it unset until the first pull
            retract_wait_loop: 0,
            add_retract_motion: false,
            retract_motion: false,
        };
        mover.set_objects(mover.pos);
        (Self { part, mover, touched: Cell::new(false) }, HitObjectBase::with_z(zheight, zheight + PLUNGERHEIGHT))
    }

    /// VPX's `HitPlunger::CalcHitBBox`.
    pub fn calc_hit_bbox(&self, base: &mut HitObjectBase) {
        let m = &self.mover;
        base.hit_bbox.left = m.x - 0.1;
        base.hit_bbox.right = m.x2 + 0.1;
        base.hit_bbox.top = m.frame_end - 0.1;
        base.hit_bbox.bottom = m.y + 0.1;
    }

    /// VPX's `HitPlunger::HitTest`: the fixed base and sides, then the tip,
    /// tested in the tip's own moving frame. A hit freezes the tip for the next
    /// update (VPX's travel limit, set from inside its const hit test), so the
    /// plunger can't overtake a slow ball.
    pub fn hit_test(&self, ball: &BallS, dtime: f32, coll: &mut CollisionEvent, sensor: Option<&dyn PlungerSensor>) -> f32 {
        let m = &self.mover;
        let mut hittime = dtime; // start time
        let mut hit = false;

        let mut ce = CollisionEvent::default();

        // Check for hits on the non-moving parts, like the side of back of the plunger.
        // These are just like hitting a wall. Check all and find the nearest collision.

        let mut newtime = m.lineseg_base.hit_test(&m.part_hit, ball, dtime, &mut ce);
        if newtime >= 0.0 && newtime <= hittime {
            hit = true;
            hittime = newtime;
            *coll = ce;
            coll.hitvel.x = 0.0;
            coll.hitvel.y = 0.0;
        }

        for i in 0..2 {
            newtime = m.lineseg_side[i].hit_test(&m.part_hit, ball, hittime, &mut ce);
            if newtime >= 0.0 && newtime <= hittime {
                hit = true;
                hittime = newtime;
                *coll = ce;
                coll.hitvel.x = 0.0;
                coll.hitvel.y = 0.0;
            }

            newtime = m.joint_base[i].hit_test(&m.part_hit, ball, hittime, &mut ce);
            if newtime >= 0.0 && newtime <= hittime {
                hit = true;
                hittime = newtime;
                *coll = ce;
                coll.hitvel.x = 0.0;
                coll.hitvel.y = 0.0;
            }
        }

        // Now check for hits on the business end, which might be moving: do the calculation in an
        // inertial frame where the tip is stationary.
        let mut ball_tmp = ball.clone();
        ball_tmp.vel.y -= m.speed;

        // Figure the impulse from hitting the moving end, from the plunger speed and the momentum
        // transfer factor, divided by the ball's mass (with a lower bound).
        let ball_mass = if ball.mass > 0.05 { ball.mass } else { 0.05 };
        let xfer_ratio = m.data.momentum_xfer / ball_mass;

        // Figure the hit speed: the mechanical plunger's measured speed when there is one (and the
        // script isn't firing), else the internal velocity.
        let mut impulse_speed = m.speed;
        if m.fire_timer == 0
            && m.data.mech_plunger
            && let Some(s) = sensor.filter(|s| s.has_sensor())
        {
            // Only apply if there is an actual hit velocity to apply, if not, use the plunger speed
            // (resulting from following the target sensor)
            let hit_velocity = s.hit_velocity(m.rest_pos);
            if hit_velocity != 0.0 {
                impulse_speed = hit_velocity // Acquired/derived velocity from sensor in per unit/s
                    * m.frame_len // Convert to VPU/s (as the 'per unit' correspond to the plunger travel length)
                    * (m.data.speed_fire / 100.0) // Apply custom table plunger strength scale
                    / PlungerMover::MASS; // Convert to impulse
            }
        }

        // apply the momentum transfer ratio
        let impulse = impulse_speed * xfer_ratio;

        // check the moving bits
        newtime = m.lineseg_end.hit_test(&m.part_hit, &ball_tmp, hittime, &mut ce);
        if newtime >= 0.0 && newtime <= hittime {
            hit = true;
            hittime = newtime;
            *coll = ce;
            coll.hitvel.x = 0.0;
            coll.hitvel.y = impulse;
        }

        for i in 0..2 {
            newtime = m.joint_end[i].hit_test(&m.part_hit, &ball_tmp, hittime, &mut ce);
            if newtime >= 0.0 && newtime <= hittime {
                hit = true;
                hittime = newtime;
                *coll = ce;
                coll.hitvel.x = 0.0;
                coll.hitvel.y = impulse;
            }
        }

        // Save the last hit time for LiveUI feedback and legacy UShock light feedback
        if hit {
            self.touched.set(true);
        }

        // check only if the plunger is not in a controlled retract motion
        // and check for a hit
        if hit && !m.retract_motion {
            // We hit the ball.  Set a travel limit to freeze the plunger at its current position for
            // the next displacement update.
            if m.travel_limit.get() < m.pos {
                m.travel_limit.set(m.pos); // HACK
            }

            // If the distance is negative, it means the objects are overlapping.  Make certain that we
            // give the ball enough of an impulse to get it not to overlap.
            if coll.hitdistance <= 0.0 && coll.hitvel.y == impulse && impulse.abs() < coll.hitdistance.abs() {
                coll.hitvel.y = -coll.hitdistance.abs();
            }

            // return the collision time delta
            hittime
        } else {
            // no collision
            -1.0
        }
    }

    /// VPX's `HitPlunger::Collide`.
    pub fn collide(&mut self, coll: &CollisionEvent, pball: &mut HitBall, env: &mut PhysicsEnv) {
        let mut dot = (pball.d.vel.x - coll.hitvel.x) * coll.hitnormal.x + (pball.d.vel.y - coll.hitvel.y) * coll.hitnormal.y;

        if dot >= -C_LOWNORMVEL {
            // nearly receding ... make sure of conditions; otherwise if clearly approaching .. process the collision
            if dot > C_LOWNORMVEL {
                return; // is this velocity clearly receding (i.e must > a minimum)
            }
            if coll.hitdistance < -C_EMBEDDED {
                dot = -C_EMBEDSHOT; // has ball become embedded???, give it a kick
            } else {
                return;
            }
        }
        // (VPX's live editor remembers this ball for dragging; not ported)

        // correct displacements, mostly from low velocity blindness, an alternative to true acceleration processing
        let mut hdist = -C_DISP_GAIN * coll.hitdistance; // distance found in hit detection
        if hdist > 1.0e-4 {
            // magnitude of jump
            if hdist > C_DISP_LIMIT {
                hdist = C_DISP_LIMIT;
            } // crossing ramps, delta noise
            pball.d.pos += hdist * coll.hitnormal; // push along norm, back to free area (use the norm, but is not correct)
        }

        // figure the basic impulse
        let impulse = dot * -1.45 / (1.0 + 1.0 / PlungerMover::MASS);

        // We hit the ball, so attenuate any plunger bounce we have queued up for a Fire event.
        self.mover.fire_bounce *= 0.6;

        // Check for a downward collision with the tip.  This is the moving part of the plunger, so it
        // has some special handling: a little of the ball's momentum transfers to the plunger.
        if coll.hitvel.y != 0.0 {
            let reverse_impulse_fudge_factor = 0.22f32;
            self.mover.reverse_impulse =
                pball.d.vel.y * impulse * (pball.d.mass / PlungerMover::MASS) * reverse_impulse_fudge_factor;
        }

        // update the ball speed for the impulse
        pball.d.vel += impulse * coll.hitnormal;

        pball.d.vel *= 0.999; // friction all axes     // !! TODO: fix this

        let scatter_vel = self.mover.scatter_velocity * env.global_difficulty; // apply difficulty weighting

        if scatter_vel > 0.0 && pball.d.vel.y.abs() > scatter_vel {
            // skip if low velocity
            let mut scatter = env.rng.rand_m11(); // -1.0f..1.0f
            scatter *= (1.0 - scatter * scatter) * 2.59808 * scatter_vel; // shape quadratic distribution and scale
            pball.d.vel.y += scatter;
        }
    }
}
