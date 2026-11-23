// Ported from Visual Pinball X: src/physics/cabinet/PlumbHandler.cpp and PlumbHandler.h, and its call
// in src/physics/PhysicsEngine.cpp (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The cabinet's tilt bob, as VPX 5436 simulates it: a weight on a 10 cm
//! rod, hanging under gravity and swung by the cabinet's acceleration
//! (`PhysicsEnv::cabinet_acceleration`, whatever nudged it), damped, bouncing
//! off the ring around it — 1° from the vertical by default — where a real
//! machine closes its tilt switch. It also keeps the readouts a table's
//! script asks for (`NudgeSensorStatus`, `NudgeTiltStatus`): the largest
//! acceleration, swing and tilt percentage since it last asked. A touch of
//! the ring is counted (`tilt_index`); the player presses the tilt for the
//! table, as VPX 5436 does.

use crate::math::{Vertex2D, Vertex3Ds, cross_product};

/// VPX's `PlumbHandler`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlumbHandler {
    enabled: bool,
    /// The rod's length, m.
    pole_length: f32,
    /// Linear angular damping, 1/s, and nonlinear, 1/s per rad/s.
    angular_damping0: f32,
    angular_damping1: f32,
    /// Coupling with the cabinet's acceleration.
    cab_accel_scale: f32,
    /// The ring's angle from the vertical, rad.
    tilt_threshold: f32,
    /// The weight, always the rod's length from its pivot, m.
    pos: Vertex3Ds,
    /// Angular velocity, rad/s.
    omega: Vertex3Ds,
    tilt_high: bool,
    tilt_index: i32,
    /// A touch of the ring begun (true) or ended (false), not yet taken by the host: VPX fires
    /// its tilt action, as the keyboard's tilt.
    pub tilt_changed: Option<bool>,
    /// The script's readouts, as VPX keeps them on the table (`m_tblNudgeRead`,
    /// `m_tblNudgePlumb`, `m_tblNudgeReadTilt`): the largest cabinet acceleration (m/s²) and
    /// swing of the weight (m) on each axis, and the largest tilt percentage, since the script
    /// last read them.
    pub nudge_read: Vertex2D,
    pub nudge_plumb: Vertex2D,
    pub nudge_read_tilt: f32,
}

const DAMPING_COEF0: f32 = 1.25;
const DAMPING_COEF1: f32 = 0.75;

impl PlumbHandler {
    /// VPX's settings: `[Player]` `SimulatedPlumb` (on by default), `PlumbDamping` (1) and
    /// `PlumbThresholdAngle` (degrees, 1).
    pub fn new(simulated: bool, damping: f32, threshold_angle_deg: f32) -> PlumbHandler {
        let pole_length = 0.10;
        PlumbHandler {
            enabled: simulated,
            pole_length,
            angular_damping0: DAMPING_COEF0 * damping,
            angular_damping1: DAMPING_COEF1 * damping,
            cab_accel_scale: 1.0,
            tilt_threshold: threshold_angle_deg * (std::f64::consts::PI / 180.0) as f32,
            pos: Vertex3Ds::new(0.0, 0.0, -pole_length),
            omega: Vertex3Ds::new(0.0, 0.0, 0.0),
            tilt_high: false,
            tilt_index: 0,
            tilt_changed: None,
            nudge_read: Vertex2D::new(0.0, 0.0),
            nudge_plumb: Vertex2D::new(0.0, 0.0),
            nudge_read_tilt: 0.0,
        }
    }

    /// How many times the weight has touched the ring.
    pub fn tilt_index(&self) -> i32 {
        self.tilt_index
    }

    /// The weight's place under its pivot, m.
    pub fn position(&self) -> Vertex3Ds {
        self.pos
    }

    /// One physics step, after the nudge's, under the cabinet's acceleration, m/s².
    pub fn step_one_millisecond(&mut self, cab_acceleration: Vertex2D) {
        if !self.enabled || self.tilt_threshold <= 0.0 {
            return;
        }

        let dt = 0.001f32;
        let mut pole_axis = self.pos / self.pole_length;

        let plumb_acc = Vertex3Ds::new(
            -cab_acceleration.x * self.cab_accel_scale, // the cabinet's nudge (a change of reference frame)
            -cab_acceleration.y * self.cab_accel_scale,
            -9.80665, // gravity
        );

        // torque per unit mass about the pivot, then angular acceleration for a point mass at the
        // rod's end (I = m L², the mass cancels out)
        let torque = cross_product(&self.pos, &plumb_acc);
        let mut alpha = torque / (self.pole_length * self.pole_length);

        // nonlinear damping: small swings decay, large fast ones are held back
        let damping = self.angular_damping0 + self.angular_damping1 * self.omega.length();
        alpha -= self.omega * damping;

        // the angular velocity, kept square to the rod (spin along it only drifts)
        self.omega += alpha * dt;
        self.omega -= pole_axis * self.omega.dot(&pole_axis);

        // the weight moves, the rod's length kept
        self.pos += cross_product(&self.omega, &self.pos) * dt;
        let pos_len = self.pos.length();
        if pos_len > 1.0e-8 {
            self.pos *= self.pole_length / pos_len;
        } else {
            self.pos = Vertex3Ds::new(0.0, 0.0, -self.pole_length);
        }

        // the axis again after that, the velocity squared to it again
        pole_axis = self.pos / self.pole_length;
        self.omega -= pole_axis * self.omega.dot(&pole_axis);

        // the ring touched? by the rod's angle from the vertical
        let psi = (self.pos.x * self.pos.x + self.pos.y * self.pos.y).sqrt().atan2(-self.pos.z);
        let tilt_angle = self.tilt_threshold;
        let tilt_perc = 100.0 * psi / tilt_angle;
        let mut tilted = false;
        if tilt_perc > 100.0 {
            tilted = true;
            // kept inside the ring
            let limit_angle = tilt_angle - 1e-3;
            self.pos.z = -self.pole_length * limit_angle.cos();
            let xy = self.pole_length * limit_angle.sin();
            let theta = self.pos.x.atan2(self.pos.y);
            let axis = Vertex3Ds::new(theta.sin(), theta.cos(), 0.0);
            self.pos.x = xy * axis.x;
            self.pos.y = xy * axis.y;
            // bouncing off it: the weight's velocity reflected against the ring's normal, damped
            let v = cross_product(&self.omega, &self.pos);
            let dot = v.dot(&pole_axis);
            let v_ref = v - 2.0 * dot * pole_axis;
            self.omega = cross_product(&self.pos, &v_ref) / (self.pole_length * self.pole_length);
            self.omega *= 0.8; // VPX's damping factor
        }

        // the tilt, as the keyboard's
        if self.tilt_high != tilted {
            self.tilt_high = tilted;
            if tilted {
                self.tilt_index += 1;
            }
            self.tilt_changed = Some(tilted);
        }

        // the script's readouts: each only grows until the script reads it
        if tilt_perc > self.nudge_read_tilt {
            self.nudge_read_tilt = tilt_perc;
        }
        if cab_acceleration.x.abs() > self.nudge_read.x.abs() {
            self.nudge_read.x = cab_acceleration.x;
        }
        if cab_acceleration.y.abs() > self.nudge_read.y.abs() {
            self.nudge_read.y = cab_acceleration.y;
        }
        if self.pos.x.abs() > self.nudge_plumb.x.abs() {
            self.nudge_plumb.x = self.pos.x;
        }
        if self.pos.y.abs() > self.nudge_plumb.y.abs() {
            self.nudge_plumb.y = self.pos.y;
        }
    }

    /// VPX's `NudgeSensorStatus`: the largest cabinet acceleration since the last call.
    pub fn take_sensor_status(&mut self) -> Vertex2D {
        std::mem::replace(&mut self.nudge_read, Vertex2D::new(0.0, 0.0))
    }

    /// VPX's `NudgeTiltStatus`: the weight's largest swing and the largest tilt percentage
    /// since the last call.
    pub fn take_tilt_status(&mut self) -> (Vertex2D, f32) {
        (std::mem::replace(&mut self.nudge_plumb, Vertex2D::new(0.0, 0.0)), std::mem::replace(&mut self.nudge_read_tilt, 0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_still_cabinet_leaves_the_bob_hanging() {
        let mut p = PlumbHandler::new(true, 1.0, 1.0);
        for _ in 0..1000 {
            p.step_one_millisecond(Vertex2D::new(0.0, 0.0));
        }
        assert_eq!(p.position(), Vertex3Ds::new(0.0, 0.0, -0.1));
        assert_eq!((p.tilt_changed, p.tilt_index()), (None, 0));
    }

    #[test]
    fn a_shove_swings_the_bob_into_its_ring_then_it_settles() {
        let mut p = PlumbHandler::new(true, 1.0, 1.0);
        // the cabinet pushed right at 3 m/s² for 30 ms: the bob, left behind, swings left; VPX
        // damps it gently, a sixth of its swing left after 3 s
        let mut touched = 0;
        for ms in 0..10_000 {
            p.step_one_millisecond(Vertex2D::new(if ms < 30 { 3.0 } else { 0.0 }, 0.0));
            if p.tilt_changed.take() == Some(true) {
                touched += 1;
            }
            if ms == 40 {
                assert!(p.position().x < 0.0, "{:?}", p.position());
            }
        }
        assert!(touched >= 1 && p.tilt_index() == touched, "{touched} {}", p.tilt_index());
        assert!(p.position().x.abs() < 1e-4, "settled at {:?}", p.position());
        // the readouts kept the largest, and empty once read
        let (swing, tilt) = p.take_tilt_status();
        assert!(swing.x < 0.0 && tilt > 100.0, "{swing:?} {tilt}");
        assert_eq!(p.take_sensor_status(), Vertex2D::new(3.0, 0.0));
        assert_eq!((p.take_sensor_status(), p.take_tilt_status()), (Vertex2D::new(0.0, 0.0), (Vertex2D::new(0.0, 0.0), 0.0)));
    }

    #[test]
    fn a_gentle_push_stays_inside_the_ring_and_an_unsimulated_bob_never_moves() {
        let mut p = PlumbHandler::new(true, 1.0, 1.0);
        for ms in 0..2000 {
            p.step_one_millisecond(Vertex2D::new(if ms < 30 { 0.3 } else { 0.0 }, 0.0));
            assert_eq!(p.tilt_changed, None, "at {ms} ms");
        }
        let mut off = PlumbHandler::new(false, 1.0, 1.0);
        off.step_one_millisecond(Vertex2D::new(50.0, 50.0));
        assert_eq!((off.position(), off.take_sensor_status()), (Vertex3Ds::new(0.0, 0.0, -0.1), Vertex2D::new(0.0, 0.0)));
    }
}
