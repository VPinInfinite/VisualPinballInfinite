//! What the physics needs from the rest of the engine, and how it reports events.
//!
//! This is our own design, not ported: VPX reaches these through its global
//! player object. Here they are passed in explicitly, so the physics can run
//! headless and deterministically.

use crate::math::{Mwc64x, Vertex2D, Vertex3Ds};
use crate::plunger::PlungerSensor;
use std::rc::Rc;

/// A table part, as hit objects refer to it: VPX's `IFireEvents*` (`m_obj`).
/// Used to address events and for the set of triggers and kickers a ball is
/// inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartId(pub u32);

/// A set of parts, as one bit per part id: the collision search asks it on
/// almost every step, so it must be quick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartSet {
    bits: Vec<u64>,
}

impl PartSet {
    pub fn insert(&mut self, p: PartId) {
        let (word, bit) = (p.0 as usize / 64, p.0 % 64);
        if word >= self.bits.len() {
            self.bits.resize(word + 1, 0);
        }
        self.bits[word] |= 1 << bit;
    }

    pub fn remove(&mut self, p: PartId) {
        if let Some(w) = self.bits.get_mut(p.0 as usize / 64) {
            *w &= !(1 << (p.0 % 64));
        }
    }

    pub fn contains(&self, p: &PartId) -> bool {
        self.bits.get(p.0 as usize / 64).is_some_and(|w| w & (1 << (p.0 % 64)) != 0)
    }

    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|&w| w == 0)
    }
}

impl FromIterator<PartId> for PartSet {
    fn from_iter<I: IntoIterator<Item = PartId>>(iter: I) -> Self {
        let mut s = PartSet::default();
        for p in iter {
            s.insert(p);
        }
        s
    }
}

/// A ball, by creation order. VPX decides which of two colliding balls handles
/// the collision by comparing their memory addresses; the port compares these
/// ids, which matches VPX whenever later balls sit at higher addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BallId(pub u32);

/// An event VPX hands to the table's script, or part state VPX sets during a
/// collision, in the order VPX does it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicsEvent {
    /// `DISPID_HitEvents_Hit` on a part.
    Hit { part: PartId },
    /// `DISPID_HitEvents_Unhit` on a part (a ball leaving a trigger).
    Unhit { part: PartId },
    /// VPX sets the part's `m_currentHitThreshold` (the impact speed scripts
    /// can read) before its hit event.
    HitThreshold { part: PartId, value: f32 },
    /// VPX sets a hit target's `m_hitEvent` flag (starts its animation).
    TargetHit { part: PartId },
    /// The table's ball-to-ball collision callback, with the impact speed.
    BallBallCollision { ball: BallId, other: BallId, speed: f32 },
    /// `DISPID_LimitEvents_EOS`: a flipper or gate reached the end of its
    /// stroke, with its angular speed in degrees per 10 ms.
    LimitEos { part: PartId, speed: f32 },
    /// `DISPID_LimitEvents_BOS`: back at the start (park), with its speed.
    LimitBos { part: PartId, speed: f32 },
    /// `DISPID_FlipperEvents_Collide`: a ball hit a flipper, with the impact speed.
    FlipperCollide { part: PartId, speed: f32 },
    /// `DISPID_SurfaceEvents_Slingshot`: a wall's slingshot fired.
    Slingshot { part: PartId },
    /// `DISPID_SpinnerEvents_Spin`: a spinner completed a turn.
    Spin { part: PartId },
    /// An auto plunger saw the mechanical plunger pulled and released: VPX
    /// presses the launch key (and releases it 100 ms later) for the script.
    LaunchBallKey { part: PartId, pressed: bool },
    /// A hit on a part the table has its hits off for, where achievements watch
    /// it (`PhysicsEnv::watching`): theirs alone, never the script's. Ours.
    Watched { part: PartId },
}

/// The physics' view of the table and the outside world.
#[derive(Clone, Debug)]
pub struct PhysicsEnv {
    /// Gravity in VPU/VPT^2 (VPX `PhysicsEngine::GetGravity`).
    pub gravity: Vertex3Ds,
    /// The table's difficulty, which scales scatter (`m_globalDifficulty`).
    pub global_difficulty: f32,
    /// The playfield's slope in degrees (`GetPlayfieldSlope`), for nudging.
    pub playfield_slope_deg: f32,
    /// The cabinet's acceleration from nudging, in m/s^2; zero without nudge.
    pub cabinet_acceleration: Vertex2D,
    /// VPX's global `c_hardScatter`, used when a part's scatter is negative.
    pub hard_scatter: f32,
    /// VPX's global random generator: every draw must happen in VPX's order.
    pub rng: Mwc64x,
    /// Alternates every physics cycle (`IsBallCollisionHandlingSwapped`).
    pub swap_ball_collision_handling: bool,
    /// Simulated time in milliseconds (`PhysicsEngine::GetTimeMsec`).
    pub time_msec: u32,
    /// Hit targets currently dropped (VPX reads `HitTarget::m_d.m_isDropped`
    /// at collision time; scripts change it).
    pub dropped_targets: PartSet,
    /// Primitives currently not collidable (VPX reads `Primitive::m_d.m_collidable`
    /// during the collision search; scripts change it).
    pub uncollidable_primitives: PartSet,
    /// The cabinet's plunger sensor, if any (VPX's `PlungerHandler`).
    pub plunger_sensor: Option<Rc<dyn PlungerSensor>>,
    /// VPX's `PlayRumble` from inside the physics — a slingshot's kick, the plunger's bounce
    /// as it fires — the latest since the host took it: the low- and high-frequency motors'
    /// speeds (0 to 1 once held there, as VPX holds them) and how long, in ms. Each replaces
    /// the one before, as each of VPX's calls cancels the rumble before.
    pub rumble: Option<(f32, f32, u32)>,
    /// Events raised and not yet delivered, in order. VPX runs the script the
    /// moment it raises an event; the engine hands these to its host right
    /// there. Where VPX's code goes on after an event, the engine
    /// splits it at the event, so the script runs before the rest.
    pub events: Vec<PhysicsEvent>,
    /// The parts achievements watch where the table has their hits off
    /// (`HitObjectBase::quiet`): their hits told as `PhysicsEvent::Watched`. Ours.
    pub watching: PartSet,
    /// Where each ball last told a watched hit: a ball resting against a part
    /// tells it once.
    pub watched_at: Vec<(BallId, Vertex3Ds)>,
}

impl Default for PhysicsEnv {
    fn default() -> Self {
        Self {
            gravity: Vertex3Ds::default(),
            global_difficulty: 0.0,
            playfield_slope_deg: 0.0,
            cabinet_acceleration: Vertex2D::default(),
            hard_scatter: 0.0,
            rng: Mwc64x::new(),
            swap_ball_collision_handling: false,
            time_msec: 0,
            dropped_targets: PartSet::default(),
            uncollidable_primitives: PartSet::default(),
            plunger_sensor: None,
            rumble: None,
            events: Vec::new(),
            watching: PartSet::default(),
            watched_at: Vec::new(),
        }
    }
}
