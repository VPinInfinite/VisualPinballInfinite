// Ported from Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): the
// UpdateAnimation of src/parts/hittarget.cpp,
// trigger.cpp and bumper.cpp, with the script members that start or read them (HitTarget's IsDropped and
// CurrentAnimOffset, Trigger's CurrentAnimOffset, Bumper's CurrentRingOffset, EnableSkirtAnimation and
// PlayHit).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The animations VPX runs once a frame for targets, triggers and bumpers.
//! They are more than looks: a hit drop target only stops colliding once
//! its fall has finished, and scripts read the offsets to move their own
//! models (`CurrentAnimOffset`, `CurrentRingOffset`).

use vpin::vpx::gameitem::bumper::Bumper;
use vpin::vpx::gameitem::hittarget::{HitTarget, TargetType};
use vpin::vpx::gameitem::trigger::{Trigger, TriggerShape};

/// VPX's `HitTarget::DROP_TARGET_LIMIT`: how far a drop target sinks.
pub const DROP_TARGET_LIMIT: f32 = 52.0;

/// A hit target's animation: a drop target sinks and rises, a standing
/// target bends back and returns.
pub struct TargetAnim {
    pub is_drop: bool,
    /// Set by the physics when a ball hits the target (`m_hitEvent`).
    pub hit_event: bool,
    move_down: bool,
    move_animation: bool,
    /// `m_moveAnimationOffset`: how far a drop target has sunk (negative),
    /// or how many degrees a standing target leans.
    pub offset: f32,
    time_stamp: u32,
    drop_speed: f32,
    raise_delay: u32,
}

impl TargetAnim {
    /// As VPX starts play (the constructor's values, then `RenderSetup`).
    pub fn new(t: &HitTarget) -> TargetAnim {
        let is_drop = matches!(
            t.target_type,
            TargetType::DropTargetBeveled | TargetType::DropTargetSimple | TargetType::DropTargetFlatSimple
        );
        let mut a = TargetAnim {
            is_drop,
            hit_event: false,
            move_down: true,
            move_animation: false,
            offset: 0.0,
            time_stamp: 0,
            drop_speed: t.drop_speed,
            raise_delay: t.raise_delay.unwrap_or(100),
        };
        if is_drop && t.is_dropped {
            a.move_down = false;
            a.offset = -DROP_TARGET_LIMIT;
        }
        a
    }

    /// VPX's `UpdateAnimation`. `is_dropped` is the target's `m_isDropped`,
    /// which the physics reads: a drop target sets it when its fall ends
    /// and clears it when it is back up. Returns whether `Animate` fires,
    /// and the end of a fall (`Dropped`) or rise (`Raised`).
    pub fn update(&mut self, diff_ms: f32, now: u32, visible: bool, is_dropped: &mut bool) -> (bool, Option<&'static str>) {
        // VPX doesn't animate an invisible target: old tables rely on hidden
        // drop targets never dropping
        if !visible {
            return (false, None);
        }
        if self.hit_event {
            if !*is_dropped {
                self.move_down = true;
            }
            self.move_animation = true;
            self.hit_event = false;
        }
        if !self.move_animation {
            return (false, None);
        }
        let mut step = self.drop_speed;
        let mut event = None;
        if self.is_drop {
            if self.move_down {
                step = -step;
            } else if now.wrapping_sub(self.time_stamp) < self.raise_delay {
                step = 0.0;
            }
            self.offset += step * diff_ms;
            if self.move_down {
                if self.offset <= -DROP_TARGET_LIMIT {
                    self.offset = -DROP_TARGET_LIMIT;
                    self.move_down = false;
                    *is_dropped = true;
                    self.move_animation = false;
                    self.time_stamp = 0;
                    event = Some("Dropped");
                }
            } else if self.offset >= 0.0 {
                self.offset = 0.0;
                self.move_animation = false;
                *is_dropped = false;
                event = Some("Raised");
            }
        } else {
            const LIMIT: f32 = 13.0;
            if !self.move_down {
                step = -step;
            }
            self.offset += step * diff_ms;
            if self.move_down {
                if self.offset >= LIMIT {
                    self.offset = LIMIT;
                    self.move_down = false;
                }
            } else if self.offset <= 0.0 {
                self.offset = 0.0;
                self.move_animation = false;
            }
        }
        (true, event)
    }

    /// `IsDropped` set to a new value: the target moves down at once, or
    /// starts back up after its raise delay.
    pub fn put_is_dropped(&mut self, dropped: bool, now: u32) {
        self.move_animation = true;
        if dropped {
            self.offset = 0.0;
            self.move_down = true;
        } else {
            self.offset = -DROP_TARGET_LIMIT;
            self.move_down = false;
            self.time_stamp = now;
        }
    }
}

/// A trigger's animation: the wire, star or button is pressed down while a
/// ball is over it.
pub struct TriggerAnim {
    /// Set by the physics when a ball enters (`TriggerAnimationHit`) or
    /// leaves (`TriggerAnimationUnhit`).
    pub hit_event: bool,
    pub unhit_event: bool,
    do_animation: bool,
    move_down: bool,
    /// `m_animHeightOffset`: how far it is pressed down (negative).
    pub offset: f32,
    anim_speed: f32,
    shape: TriggerShape,
    pub radius: f32,
}

impl TriggerAnim {
    pub fn new(t: &Trigger) -> TriggerAnim {
        TriggerAnim {
            hit_event: false,
            unhit_event: false,
            do_animation: false,
            move_down: false,
            offset: 0.0,
            anim_speed: t.anim_speed,
            shape: t.shape.clone(),
            radius: t.radius,
        }
    }

    /// VPX's `UpdateAnimation`; returns whether `Animate` fires.
    pub fn update(&mut self, diff_ms: f32) -> bool {
        let limit = match self.shape {
            TriggerShape::Star => self.radius * (1.0f64 / 5.0) as f32,
            TriggerShape::Button => self.radius * (1.0f64 / 10.0) as f32,
            TriggerShape::WireC => 60.0,
            TriggerShape::WireD | TriggerShape::Inder => 25.0,
            _ => 32.0,
        };
        if self.hit_event {
            self.do_animation = true;
            self.hit_event = false;
            self.offset = 0.0;
            self.move_down = true;
        }
        // a hit and an unhit in the same frame: the unhit wins, so the
        // switch doesn't stay down
        if self.unhit_event {
            self.do_animation = true;
            self.unhit_event = false;
            self.hit_event = false;
            self.offset = limit;
            self.move_down = false;
        }
        if !self.do_animation {
            return false;
        }
        let mut step = diff_ms * self.anim_speed;
        if self.move_down {
            step = -step;
        }
        self.offset += step;
        if self.move_down {
            if self.offset <= -limit {
                self.offset = -limit;
                self.do_animation = false;
                self.move_down = false;
            }
        } else if self.offset >= 0.0 {
            self.offset = 0.0;
            self.do_animation = false;
            self.move_down = true;
        }
        true
    }
}

/// A bumper's ring and skirt animations. The physics keeps the ring's
/// offset and the hit flag in the bumper's collider, as VPX does.
pub struct BumperAnim {
    ring_animate: bool,
    ring_down: bool,
    ring_speed: f32,
    ring_drop_offset: f32,
    height_scale: f32,
    pub enable_skirt_animation: bool,
    do_skirt_animation: bool,
    skirt_counter: f32,
    update_skirt: bool,
    center: [f32; 2],
    /// The skirt's tilt towards the ball, in degrees (VPX's `m_rotx`,
    /// `m_roty`): 0 at rest.
    pub rot_x: f32,
    pub rot_y: f32,
    /// Whether its base, skirt, ring and cap draw (VPX's `m_baseVisible`, `m_skirtVisible`,
    /// `m_ringVisible`, `m_capVisible`), which scripts switch while it plays — The Walking Dead
    /// hides its bumpers' own for baked ones.
    pub shown: [bool; 4],
}

/// The pieces of a bumper `BumperAnim::shown` holds, in its order.
pub const BUMPER_PIECES: [&str; 4] = ["Base", "Socket", "Ring", "Cap"];

impl BumperAnim {
    pub fn new(b: &Bumper) -> BumperAnim {
        BumperAnim {
            ring_animate: false,
            ring_down: false,
            ring_speed: b.ring_speed,
            ring_drop_offset: b.ring_drop_offset.unwrap_or(0.0),
            height_scale: b.height_scale,
            enable_skirt_animation: true,
            do_skirt_animation: false,
            skirt_counter: 0.0,
            update_skirt: false,
            center: [b.center.x, b.center.y],
            rot_x: 0.0,
            rot_y: 0.0,
            shown: [b.is_base_visible, b.is_socket_visible.unwrap_or(true), b.is_ring_visible.unwrap_or(true), b.is_cap_visible],
        }
    }

    /// VPX's `UpdateSkirt`: tilted 5 degrees towards where the ball hit, or
    /// back at rest.
    fn update_skirt(&mut self, tilt: bool, hit_ball: [f32; 2]) {
        const SKIRT_TILT: f32 = 5.0;
        self.rot_x = 0.0;
        self.rot_y = 0.0;
        if tilt {
            let [hitx, hity] = hit_ball;
            let [cx, cy] = self.center;
            let mut dy = (hity - cy).abs();
            if dy == 0.0 {
                dy = 0.000001;
            }
            let dx = (hitx - cx).abs();
            let skirt_a = (dx / dy).atan();
            self.rot_x = skirt_a.cos() * SKIRT_TILT;
            self.rot_y = skirt_a.sin() * SKIRT_TILT;
            if cy < hity {
                self.rot_x = -self.rot_x;
            }
            if cx > hitx {
                self.rot_y = -self.rot_y;
            }
        }
    }

    /// VPX's `UpdateAnimation`, given the collider's hit flag (which it
    /// clears), ring offset and where the ball was when it hit. Returns how
    /// many `Animate` events fire: the ring's and the skirt's each fire one.
    pub fn update(&mut self, diff_ms: f32, hit_event: &mut bool, ring_offset: &mut f32, hit_ball: [f32; 2]) -> u32 {
        let hit = *hit_event;
        *hit_event = false;
        // a hit rumbles the controller
        if hit {
            crate::rumble::request(0.1, 0.05, 100);
        }
        let mut animate = 0;

        // the ring drops on a hit, then rises
        let limit = self.ring_drop_offset + self.height_scale * 0.5;
        if hit {
            self.ring_animate = true;
            self.ring_down = true;
        }
        if self.ring_animate {
            let step = if self.ring_down { -self.ring_speed } else { self.ring_speed };
            *ring_offset += step * diff_ms;
            if self.ring_down {
                if *ring_offset <= -limit {
                    *ring_offset = -limit;
                    self.ring_down = false;
                }
            } else if *ring_offset >= 0.0 {
                *ring_offset = 0.0;
                self.ring_animate = false;
            }
            animate += 1;
        }

        // the skirt tilts towards the ball for 160 ms
        if self.enable_skirt_animation {
            if hit {
                self.do_skirt_animation = true;
                self.update_skirt(true, hit_ball);
                self.skirt_counter = 0.0;
            }
            if self.do_skirt_animation {
                self.skirt_counter += diff_ms;
                if self.skirt_counter > 160.0 {
                    self.do_skirt_animation = false;
                    self.update_skirt(false, hit_ball);
                }
                animate += 1;
            }
        } else if self.update_skirt {
            // one update when a script turned the animation off
            self.update_skirt = false;
            self.update_skirt(false, hit_ball);
            animate += 1;
        }
        animate
    }

    /// `EnableSkirtAnimation` set by the script.
    pub fn put_enable_skirt_animation(&mut self, on: bool) {
        if self.enable_skirt_animation != on {
            if !on {
                self.update_skirt = true;
            }
            self.enable_skirt_animation = on;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_drop_target_falls_then_reports_dropped() {
        let t = HitTarget { target_type: TargetType::DropTargetSimple, drop_speed: 0.2, ..Default::default() };
        let mut a = TargetAnim::new(&t);
        let mut dropped = false;
        a.hit_event = true;
        let mut events = Vec::new();
        for frame in 1..=30 {
            let (_, e) = a.update(16.0, frame * 16, true, &mut dropped);
            events.extend(e);
        }
        // 52 units at 0.2 a ms: 260 ms, so dropped by the 17th frame
        assert!(dropped);
        assert_eq!(events, ["Dropped"]);
        assert_eq!(a.offset, -DROP_TARGET_LIMIT);

        // raised by the script: collides again at once, rises after the delay
        dropped = false;
        a.put_is_dropped(false, 1000);
        let (_, e) = a.update(16.0, 1016, true, &mut dropped);
        assert_eq!((a.offset, e), (-DROP_TARGET_LIMIT, None));
        let mut events = Vec::new();
        for now in (1100..1500).step_by(16) {
            events.extend(a.update(16.0, now, true, &mut dropped).1);
        }
        assert_eq!(events, ["Raised"]);
        assert_eq!(a.offset, 0.0);
    }

    #[test]
    fn hidden_targets_never_drop() {
        let t = HitTarget { target_type: TargetType::DropTargetBeveled, ..Default::default() };
        let mut a = TargetAnim::new(&t);
        let mut dropped = false;
        a.hit_event = true;
        for frame in 1..100 {
            a.update(16.0, frame * 16, false, &mut dropped);
        }
        assert!(!dropped);
    }

    #[test]
    fn trigger_goes_down_and_back() {
        let t = Trigger { shape: TriggerShape::WireA, anim_speed: 1.0, ..Default::default() };
        let mut a = TriggerAnim::new(&t);
        a.hit_event = true;
        for _ in 0..10 {
            a.update(10.0);
        }
        assert_eq!(a.offset, -32.0);
        // as VPX: the unhit starts above the rest position, so the first
        // step already brings it back up
        a.unhit_event = true;
        assert!(a.update(10.0));
        assert_eq!(a.offset, 0.0);
    }

    #[test]
    fn bumper_ring_drops_and_returns() {
        let b = Bumper { ring_speed: 0.5, height_scale: 90.0, ..Default::default() };
        let mut a = BumperAnim::new(&b);
        let (mut hit, mut offset) = (true, 0.0);
        assert_eq!(a.update(10.0, &mut hit, &mut offset, [0.0, 0.0]), 2);
        assert!(!hit);
        assert_eq!(offset, -5.0);
        for _ in 0..40 {
            a.update(10.0, &mut hit, &mut offset, [0.0, 0.0]);
        }
        assert_eq!(offset, 0.0);
        assert_eq!(a.update(10.0, &mut hit, &mut offset, [0.0, 0.0]), 0);
    }

    #[test]
    fn bumper_skirt_tilts_towards_the_ball_then_rests() {
        let b = Bumper { ring_speed: 0.5, height_scale: 90.0, center: vpin::vpx::gameitem::vertex2d::Vertex2D::new(100.0, 100.0), ..Default::default() };
        let mut a = BumperAnim::new(&b);
        let (mut hit, mut offset) = (true, 0.0);
        // a ball straight below (larger y): the whole tilt about x, negative
        a.update(10.0, &mut hit, &mut offset, [100.0, 150.0]);
        assert_eq!((a.rot_x, a.rot_y), (-5.0, 0.0));
        // VPX's skirt counter: back at rest once it passes 160 ms
        for _ in 0..15 {
            a.update(10.0, &mut hit, &mut offset, [100.0, 150.0]);
        }
        assert_eq!((a.rot_x, a.rot_y), (-5.0, 0.0));
        a.update(10.0, &mut hit, &mut offset, [100.0, 150.0]);
        assert_eq!((a.rot_x, a.rot_y), (0.0, 0.0));
        // a ball to the left: the tilt about y, negative
        hit = true;
        a.update(10.0, &mut hit, &mut offset, [50.0, 100.0]);
        assert!(a.rot_x.abs() < 1e-3 && (a.rot_y + 5.0).abs() < 1e-3, "{} {}", a.rot_x, a.rot_y);
    }
}
