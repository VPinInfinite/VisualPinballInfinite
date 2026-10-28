// Ported from Visual Pinball X: src/physics/collideex.cpp (license:GPLv3+), the
// UpdateDisplacements of GateMoverObject and SpinnerMoverObject,
// commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The gate's and spinner's motion over a time step. VPX fires their
//! end-of-swing and spin events in the middle of these functions and then
//! reads state the script may have changed (a gate's forced move, for one), so
//! they run at the engine level: the event goes to the host where VPX fires
//! it, and the rest continues from the state the script left.

use crate::collideex::{HitGate, HitSpinner};
use crate::engine::{PhysicsEngine, PhysicsHost, Shape};
use crate::env::PhysicsEvent;
use crate::math::def::rad_to_ang;

impl PhysicsEngine {
    fn gate(&mut self, k: u32) -> &mut HitGate {
        match &mut self.collider_mut(k).shape {
            Shape::Gate(g) => g,
            _ => unreachable!("collider {k} is not a gate"),
        }
    }

    fn spinner(&mut self, k: u32) -> &mut HitSpinner {
        match &mut self.collider_mut(k).shape {
            Shape::Spinner(s) => s,
            _ => unreachable!("collider {k} is not a spinner"),
        }
    }

    /// Raises an event and runs the script for it, as VPX does at that point.
    fn fire_now(&mut self, event: PhysicsEvent, host: &mut dyn PhysicsHost) {
        self.env.events.push(event);
        self.deliver(host);
    }

    /// VPX's `GateMoverObject::UpdateDisplacements`: swings the flap; at a
    /// limit it bounces back (unless the script is moving it) and fires the
    /// end-of-stroke or park event.
    pub(crate) fn gate_update_displacements(&mut self, k: u32, dtime: f32, host: &mut dyn PhysicsHost) {
        let g = self.gate(k);
        let part = g.part;
        if g.data.two_way {
            let m = &mut g.mover;
            if m.angle.abs() > m.angle_max {
                m.angle = if m.angle < 0.0 { -m.angle_max } else { m.angle_max };
                let speed = rad_to_ang(m.anglespeed).abs();
                self.fire_now(PhysicsEvent::LimitEos { part, speed }, host); // send EOS event
                let m = &mut self.gate(k).mover;
                if !m.forced_move {
                    m.anglespeed = -m.anglespeed;
                    m.anglespeed *= m.damping * 0.8; // just some extra damping to reduce the anglespeed a bit faster
                } else if m.anglespeed > 0.0 {
                    m.anglespeed = 0.0;
                }
            }
            let m = &mut self.gate(k).mover;
            if m.angle.abs() < m.angle_min {
                m.angle = if m.angle < 0.0 { -m.angle_min } else { m.angle_min };
                if !m.forced_move {
                    m.anglespeed = -m.anglespeed;
                    m.anglespeed *= m.damping * 0.8; // just some extra damping to reduce the anglespeed a bit faster
                } else if m.anglespeed < 0.0 {
                    m.anglespeed = 0.0;
                }
            }
        } else {
            let m = &mut g.mover;
            let direction = if m.hit_direction { -1.0f32 } else { 1.0 };
            if direction * m.angle > m.angle_max {
                m.angle = direction * m.angle_max;
                let speed = rad_to_ang(m.anglespeed).abs();
                self.fire_now(PhysicsEvent::LimitEos { part, speed }, host); // send EOS event
                let m = &mut self.gate(k).mover;
                if !m.forced_move {
                    m.anglespeed = -m.anglespeed;
                    m.anglespeed *= m.damping * 0.8; // just some extra damping to reduce the anglespeed a bit faster
                } else if m.anglespeed > 0.0 {
                    m.anglespeed = 0.0;
                }
            }
            let m = &mut self.gate(k).mover;
            if direction * m.angle < m.angle_min {
                m.angle = direction * m.angle_min;
                let speed = rad_to_ang(m.anglespeed).abs();
                self.fire_now(PhysicsEvent::LimitBos { part, speed }, host); // send Park event
                let m = &mut self.gate(k).mover;
                if !m.forced_move {
                    m.anglespeed = -m.anglespeed;
                    m.anglespeed *= m.damping * 0.8; // just some extra damping to reduce the anglespeed a bit faster
                } else if m.anglespeed < 0.0 {
                    m.anglespeed = 0.0;
                }
            }
        }
        let m = &mut self.gate(k).mover;
        if m.anglespeed == 0.0 {
            m.forced_move = false;
        }

        m.angle += m.anglespeed * dtime;
    }

    /// VPX's `SpinnerMoverObject::UpdateDisplacements`: turns the plate; a
    /// limited spinner bounces at its limits, a free one fires the spin event
    /// each time it passes the top.
    pub(crate) fn spinner_update_displacements(&mut self, k: u32, dtime: f32, host: &mut dyn PhysicsHost) {
        let s = self.spinner(k);
        let part = s.part;
        if s.data.angle_min != s.data.angle_max {
            // blocked spinner, limited motion spinner
            let m = &mut s.mover;
            m.angle += m.anglespeed * dtime;

            if m.angle > m.angle_max {
                m.angle = m.angle_max;
                let speed = rad_to_ang(m.anglespeed).abs();
                self.fire_now(PhysicsEvent::LimitEos { part, speed }, host); // send EOS event

                let m = &mut self.spinner(k).mover;
                if m.anglespeed > 0.0 {
                    m.anglespeed *= -0.005 - m.elasticity;
                }
            }
            let m = &mut self.spinner(k).mover;
            if m.angle < m.angle_min {
                m.angle = m.angle_min;

                let speed = rad_to_ang(m.anglespeed).abs();
                self.fire_now(PhysicsEvent::LimitBos { part, speed }, host); // send Park event

                let m = &mut self.spinner(k).mover;
                if m.anglespeed < 0.0 {
                    m.anglespeed *= -0.005 - m.elasticity;
                }
            }
        } else {
            let m = &mut s.mover;
            let pi = std::f64::consts::PI;
            let target = if m.anglespeed > 0.0 {
                if m.angle < pi as f32 { pi as f32 } else { (3.0 * pi) as f32 }
            } else if m.angle < pi as f32 {
                (-pi) as f32
            } else {
                pi as f32
            };

            m.angle += m.anglespeed * dtime;

            let spun = if m.anglespeed > 0.0 { m.angle > target } else { m.angle < target };
            if spun {
                self.fire_now(PhysicsEvent::Spin { part }, host);
            }

            let m = &mut self.spinner(k).mover;
            let two_pi = (2.0 * pi) as f32;
            while m.angle > two_pi {
                m.angle -= two_pi;
            }
            while m.angle < 0.0 {
                m.angle += two_pi;
            }
        }
    }
}
