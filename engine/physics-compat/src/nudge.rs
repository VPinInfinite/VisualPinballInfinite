// Ported from Visual Pinball X: src/physics/cabinet/KeyboardNudge.cpp and KeyboardNudge.h (the VP9
// push-retract, VPX 10.8 box and "10.9" cabinet models), src/physics/cabinet/NudgeHandler.cpp and NudgeHandler.h,
// GamepadNudge.cpp and GamepadNudge.h, CabinetNudgeSensor.cpp and CabinetNudgeSensor.h,
// NudgeIntentHandler.cpp and NudgeIntentHandler.h, CabinetPhysics.cpp and CabinetPhysics.h,
// DampedHarmonicOscillator.h, src/math/ExponentialMovingAverage.h, and the unit conversions and
// helpers of src/core/def.h (license:GPLv3+; NudgeHandler.h and ExponentialMovingAverage.h carry no
// marker, but were added by VPX's team in 2026 with no third-party content, so VPX's licence makes
// them GPL too), build 5436 (10.8.1 pre-release, commit af26b2d).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! Nudging, as VPX simulates it: the table's script calls `Nudge` (from
//! its nudge keys), or a gamepad's stick is pushed; the cabinet is pushed
//! and moves, and the physics feeds the cabinet's acceleration to every
//! ball each step (`PhysicsEnv::cabinet_acceleration`). VPX offers three
//! keyboard models; two are here: VP9's short push and pull, and the "VPX
//! 10.8" box on a damped spring, which is how the released VPX 10.8 nudges
//!. A gamepad's stick nudges as VPX 5436's does: each push read as
//! the player's intent to nudge, which pushes a model of a real cabinet;
//! so does a cabinet board's accelerometer in
//! VPX's default layout for it. A cabinet's motion sensors set up as VPX
//! 5436's cabinet sensor take velocity and acceleration readings, filtered
//! by VPX's Kalman filter (`motion_kalman`) with the two channels' gain
//! calibrated from each nudge (`gain_calibrator`), then read as the player's
//! intent or applied as they are. VPX's newer "10.9" keyboard model, each
//! nudge a short push of the same model of a cabinet, is here too: VPX 5436's
//! default, a setting here, the 10.8 box model staying the default (how the
//! released VPX nudges).

use crate::math::Vertex2D;
use crate::physconst::PHYS_FACTOR;

/// VPX's `VPUTOM(x)`: VP units to metres.
fn vputom(x: Vertex2D) -> Vertex2D {
    x * (0.0254_f64 * 1.0625 / 50.0) as f32
}

/// VPX's `VPUVPT2TOMS2(x)`: VPU/VPT² to m/s², `VPUTOM(x) * VPTTOS(VPTTOS(1.0f))`.
fn vpuvpt2toms2(x: Vertex2D) -> Vertex2D {
    vputom(x) * ((1.0f32 / 0.01) / 0.01)
}

/// Which keyboard nudge model (VPX's `KeyboardNudgeMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardNudgeMode {
    /// VP9: a 5 ms push, a 5 ms pull back, then 90 ms before the next.
    Vp9PushRetract,
    /// VPX 10.8: the cabinet as a box on a damped spring.
    VpxBoxModel,
    /// VPX "10.9" (5436's default): each nudge a 25 ms push of VPX's model of a real cabinet.
    VpxCabModel,
}

impl KeyboardNudgeMode {
    /// VPX's `[Player]` `KeyboardNudgeMode` setting's value, 0 to 2.
    pub fn from_setting(value: i32) -> Option<KeyboardNudgeMode> {
        match value {
            0 => Some(KeyboardNudgeMode::Vp9PushRetract),
            1 => Some(KeyboardNudgeMode::VpxBoxModel),
            2 => Some(KeyboardNudgeMode::VpxCabModel),
            _ => None,
        }
    }
}

/// VPX's `PushRetractKeyboardNudge`.
#[derive(Clone, Debug, PartialEq)]
struct PushRetract {
    strength: f32,
    impulse: Vertex2D,
    nudge_time: i32,
    acceleration: Vertex2D,
    position: Vertex2D,
    deactivation_delay: i32,
}

impl PushRetract {
    fn nudge(&mut self, angle: f32, force: f32) {
        self.deactivation_delay = 10000;
        if self.nudge_time != 0 {
            return;
        }
        let a = ang_to_rad(angle);
        self.impulse.x = a.sin() * (self.strength * force);
        self.impulse.y = -a.cos() * (self.strength * force);
        self.nudge_time = 100;
    }

    fn step_one_millisecond(&mut self) {
        if self.deactivation_delay != 0 {
            self.deactivation_delay -= 1;
        }
        if self.nudge_time != 0 {
            self.nudge_time -= 1;
            if self.nudge_time == 95 {
                // 5 ms forward
                self.acceleration = Vertex2D::new(-self.impulse.x * 2.0, self.impulse.y * 2.0);
                self.acceleration *= (1.0 / PHYS_FACTOR) as f32; // to a force
                self.acceleration = vpuvpt2toms2(self.acceleration);
            } else if self.nudge_time == 90 {
                // 5 ms back
                self.acceleration = Vertex2D::new(self.impulse.x, -self.impulse.y);
                self.acceleration *= (1.0 / PHYS_FACTOR) as f32;
                self.acceleration = vpuvpt2toms2(self.acceleration);
            } else {
                // no new nudge for the remaining 90 ms
                self.acceleration = Vertex2D::new(0.0, 0.0);
            }
        }
        let attenuation = sqr(self.nudge_time as f32 * 0.01);
        self.position = vputom(Vertex2D::new(self.impulse.x * attenuation, -self.impulse.y * attenuation));
    }
}

/// VPX's `BoxModelKeyboardNudge`: the cabinet's movement u as a mass on a
/// spring with damping, u'' = -k u - c u'.
#[derive(Clone, Debug, PartialEq)]
struct BoxModel {
    strength: f32,
    acceleration: Vertex2D,
    velocity: Vertex2D,
    prev_velocity: Vertex2D,
    position_vpu: Vertex2D,
    position: Vertex2D,
    spring: f32,
    damping: f32,
    deactivation_delay: i32,
}

impl BoxModel {
    /// `nudge_time`: the table's `NudgeTime`, a half swing (out and back).
    fn new(strength: f32, nudge_time: f32) -> BoxModel {
        let damping_ratio = 0.5f32;
        // a half period is T = pi / omega_d, with omega_d = sqrt(k) sqrt(1 - zeta^2):
        // solved for the spring constant k
        let spring = (std::f64::consts::PI * std::f64::consts::PI) as f32 / (nudge_time * nudge_time * (1.0 - damping_ratio * damping_ratio));
        // zeta = c / (2 sqrt(k)), solved for the damping c
        let damping = damping_ratio * 2.0 * spring.sqrt();
        BoxModel {
            strength,
            acceleration: Vertex2D::new(0.0, 0.0),
            velocity: Vertex2D::new(0.0, 0.0),
            prev_velocity: Vertex2D::new(0.0, 0.0),
            position_vpu: Vertex2D::new(0.0, 0.0),
            position: Vertex2D::new(0.0, 0.0),
            spring,
            damping,
            deactivation_delay: 0,
        }
    }

    fn nudge(&mut self, angle: f32, force: f32) {
        self.deactivation_delay = 10000;
        let a = ang_to_rad(angle);
        self.velocity.x += a.sin() * (self.strength * force);
        self.velocity.y += -a.cos() * (self.strength * force);
    }

    fn step_one_millisecond(&mut self) {
        if self.deactivation_delay != 0 {
            self.deactivation_delay -= 1;
        }
        let force = self.position_vpu * -self.spring - self.velocity * self.damping;
        self.velocity += force * PHYS_FACTOR as f32;
        self.position_vpu += self.velocity * PHYS_FACTOR as f32;
        self.position = vputom(self.position_vpu);
        self.acceleration = vpuvpt2toms2((self.velocity - self.prev_velocity) * (1.0 / PHYS_FACTOR) as f32);
        self.prev_velocity = self.velocity;
    }
}

/// VPX's `CabModelKeyboardNudge`: each nudge a 25 ms push of VPX's model of a real cabinet, a
/// raised cosine peaking at half a g for the strength core.vbs and most tables nudge with (2);
/// pushes in progress add up.
#[derive(Clone, Debug, PartialEq)]
struct CabModel {
    strength: f32,
    /// Each push's milliseconds so far and its peak acceleration, m/s².
    impulses: Vec<(i32, Vertex2D)>,
    cabinet: CabinetModel,
    deactivation_delay: i32,
}

impl CabModel {
    fn nudge(&mut self, angle: f32, force: f32) {
        let g = 9.80665f32;
        let core_script_strength = 2.0f32;
        let base_scale = 0.5 * g / core_script_strength;
        let actual_strength = force * self.strength * base_scale;
        let a = ang_to_rad(angle);
        self.impulses.push((0, Vertex2D::new(a.sin() * actual_strength, -a.cos() * actual_strength)));
        self.deactivation_delay = 10000;
    }

    fn step_one_millisecond(&mut self) {
        if self.deactivation_delay != 0 {
            self.deactivation_delay -= 1;
        }
        let impulse_length = 25;
        let mut impulse = Vertex2D::new(0.0, 0.0);
        self.impulses.retain_mut(|(elapsed, peak)| {
            *elapsed += 1;
            if *elapsed > impulse_length {
                return false;
            }
            let t = *elapsed as f32 / impulse_length as f32;
            impulse += *peak * 0.5 * (1.0 - ((2.0 * std::f64::consts::PI) as f32 * t).cos());
            true
        });
        let mass = self.cabinet.mass;
        self.cabinet.step_one_millisecond(impulse * mass);
    }
}

#[derive(Clone, Debug, PartialEq)]
enum KeyboardNudge {
    PushRetract(PushRetract),
    BoxModel(BoxModel),
    CabModel(CabModel),
}

impl KeyboardNudge {
    fn nudge(&mut self, angle: f32, force: f32) {
        match self {
            KeyboardNudge::PushRetract(k) => k.nudge(angle, force),
            KeyboardNudge::BoxModel(k) => k.nudge(angle, force),
            KeyboardNudge::CabModel(k) => k.nudge(angle, force),
        }
    }

    fn step_one_millisecond(&mut self) {
        match self {
            KeyboardNudge::PushRetract(k) => k.step_one_millisecond(),
            KeyboardNudge::BoxModel(k) => k.step_one_millisecond(),
            KeyboardNudge::CabModel(k) => k.step_one_millisecond(),
        }
    }

    /// Active: 10 s from its last nudge.
    fn is_active(&self) -> bool {
        match self {
            KeyboardNudge::PushRetract(k) => k.deactivation_delay > 0,
            KeyboardNudge::BoxModel(k) => k.deactivation_delay > 0,
            KeyboardNudge::CabModel(k) => k.deactivation_delay > 0,
        }
    }

    fn acceleration(&self) -> Vertex2D {
        match self {
            KeyboardNudge::PushRetract(k) => k.acceleration,
            KeyboardNudge::BoxModel(k) => k.acceleration,
            KeyboardNudge::CabModel(k) => k.cabinet.acceleration,
        }
    }

    fn offset(&self) -> Vertex2D {
        match self {
            KeyboardNudge::PushRetract(k) => k.position,
            KeyboardNudge::BoxModel(k) => k.position,
            KeyboardNudge::CabModel(k) => k.cabinet.position,
        }
    }
}

/// VPX's `DampedHarmonicOscillator`: a mass on a spring with damping, in SI units.
#[derive(Clone, Debug, PartialEq)]
struct Oscillator {
    mass: f32,
    /// The spring constant, N/m, and the damping coefficient, N·s/m.
    k: f32,
    damping: f32,
    displacement: f32,
    velocity: f32,
    acceleration: f32,
}

impl Oscillator {
    /// `freq`: its natural frequency, Hz; `zeta`: its damping ratio.
    fn new(mass: f32, freq: f32, zeta: f32) -> Oscillator {
        let omega0 = (2.0 * std::f64::consts::PI) as f32 * freq;
        Oscillator { mass, k: mass * omega0 * omega0, damping: 2.0 * zeta * (mass * omega0), displacement: 0.0, velocity: 0.0, acceleration: 0.0 }
    }

    /// `dt` seconds under the force `f`, newtons.
    fn step(&mut self, f: f32, dt: f32) {
        self.acceleration = (f - self.damping * self.velocity - self.k * self.displacement) / self.mass;
        self.velocity += self.acceleration * dt;
        self.displacement += self.velocity * dt;
    }
}

/// VPX's `CabinetPhysics`: a cabinet of 113 kg on its legs, as two damped
/// springs fitted to real machines (9.3 Hz from side to side, 5.8 Hz from
/// front to back), its movement scaled as VPX scales it to match real
/// cabinets' (a firm nudge moves one 3 to 5 mm).
#[derive(Clone, Debug, PartialEq)]
struct CabinetModel {
    mass: f32,
    x: Oscillator,
    y: Oscillator,
    acceleration: Vertex2D,
    position: Vertex2D,
}

impl CabinetModel {
    fn new() -> CabinetModel {
        let mass = 113.0;
        CabinetModel {
            mass,
            x: Oscillator::new(mass, 9.3, 0.052),
            y: Oscillator::new(mass, 5.8, 0.055),
            acceleration: Vertex2D::new(0.0, 0.0),
            position: Vertex2D::new(0.0, 0.0),
        }
    }

    /// One millisecond under `force`, newtons.
    fn step_one_millisecond(&mut self, force: Vertex2D) {
        let dt = 0.001;
        self.x.step(force.x, dt);
        self.y.step(force.y, dt);
        self.acceleration = Vertex2D::new(self.x.acceleration, self.y.acceleration);
        // VPX's correction factors, which make its simple model's movement match real cabinets'
        self.position = Vertex2D::new(self.x.displacement * 3.5, self.y.displacement * 2.0);
    }
}

/// VPX's `NudgeIntentHandler`: from a sensor's readings, the player's intent
/// to nudge — each push a short impulse (25 ms), its strength and direction
/// where the push peaks, a little delayed so a soft push's peak isn't missed
/// between the sensor's readings. A front nudge only pushes (the stick up),
/// never pulls; a cabinet's sensor also passes over the weaker peaks just
/// after a stronger one, the cabinet's own swing. (VPX also notes where each
/// push starts, for a strength factor it leaves off; not kept here.)
#[derive(Clone, Debug, PartialEq)]
struct NudgeIntent {
    is_gamepad: bool,
    impulse_length: i32,
    impulse_elapsed: i32,
    impulse_delay: i32,
    impulse: Vertex2D,
    time: u64,
    segment_strength: f32,
    segment_end: u64,
    segment_is_peak: bool,
    segment_impulse_sent: bool,
    last_impulse_strength: f32,
    last_impulse_time: u64,
}

impl NudgeIntent {
    fn new(is_gamepad: bool) -> NudgeIntent {
        let impulse_length = 25;
        NudgeIntent {
            is_gamepad,
            impulse_length,
            impulse_elapsed: impulse_length + 1,
            impulse_delay: 0,
            impulse: Vertex2D::new(0.0, 0.0),
            time: 0,
            segment_strength: 0.0,
            segment_end: 0,
            segment_is_peak: false,
            segment_impulse_sent: false,
            last_impulse_strength: 0.0,
            last_impulse_time: 0,
        }
    }

    fn impulse_in_progress(&self) -> bool {
        self.impulse_elapsed <= self.impulse_length + self.impulse_delay
    }

    /// The impulse's acceleration now, m/s²: a raised cosine over its length, after its delay.
    fn impulse_acceleration(&self) -> Vertex2D {
        if !self.impulse_in_progress() || self.impulse_elapsed < self.impulse_delay {
            return Vertex2D::new(0.0, 0.0);
        }
        let t = (self.impulse_elapsed - self.impulse_delay) as f32 / self.impulse_length as f32;
        self.impulse * 0.5 * (1.0 - ((2.0 * std::f64::consts::PI) as f32 * t).cos())
    }

    /// VPX's strength factor: the push's final position alone, for now (1; VPX leaves a
    /// gamepad's speed of push out, as it lost some impulses).
    fn strength_factor(&self) -> f32 {
        1.0
    }

    /// How long to hold an impulse back: three of the sensor's readings for a soft push, one
    /// for a firm one (0.5 g); a gamepad's come every 16 ms (an Xbox controller over
    /// Bluetooth), other sensors' every 8 ms (USB's usual 125 a second).
    fn impulse_delay_for(&self, strength: f32) -> i32 {
        let no_delay_strength = 5.0;
        let firmness = clamp(strength / no_delay_strength, 0.0, 1.0);
        let poll = if self.is_gamepad { 16.0 } else { 8.0 };
        lerp(3.0 * poll, poll, firmness) as i32
    }

    fn evaluate_impulse(&mut self, impulse: Vertex2D) {
        // a cabinet's swing: a weaker peak soon after a stronger one
        if !self.is_gamepad && self.segment_strength <= self.last_impulse_strength && self.segment_end - self.last_impulse_time <= 300 {
            return;
        }
        let threshold = 1.0; // m/s²
        let strength = self.strength_factor() * self.segment_strength;
        if strength > threshold {
            self.impulse = impulse * self.strength_factor();
            self.impulse_elapsed = 0;
            self.impulse_delay = self.impulse_delay_for(strength);
            self.segment_impulse_sent = true;
        }
    }

    /// One millisecond with the stick asking for `acceleration`, m/s².
    fn step_one_millisecond(&mut self, acceleration: Vertex2D) {
        self.impulse_elapsed += 1;
        self.time += 1;
        // front nudges are pushes alone: their peaks are negative
        let nudge = Vertex2D::new(acceleration.x, min(acceleration.y, 0.0));
        let strength = nudge.length();
        if self.segment_is_peak {
            if strength > self.segment_strength {
                // going up
                self.segment_strength = strength;
                self.segment_end = self.time;
                if !self.segment_impulse_sent {
                    self.evaluate_impulse(nudge);
                } else if self.impulse_in_progress() {
                    let stronger = nudge * self.strength_factor();
                    let s2 = stronger.length_squared();
                    if s2 > self.impulse.length_squared() {
                        // the ongoing impulse takes the push's new strength and direction
                        self.impulse = stronger;
                        if self.impulse_elapsed < self.impulse_delay {
                            self.impulse_delay = self.impulse_elapsed.max(self.impulse_delay_for(s2.sqrt()));
                        }
                    }
                }
            } else if strength < self.segment_strength * 0.9 {
                // the peak is over: wait for the next
                self.last_impulse_time = self.segment_end;
                self.last_impulse_strength = self.segment_strength;
                self.segment_strength = strength;
                self.segment_end = self.time;
                self.segment_is_peak = false;
            }
        } else if strength < self.segment_strength {
            // going down
            self.segment_strength = strength;
            self.segment_end = self.time;
        } else if strength > max(0.1, self.segment_strength * 1.1) {
            // a new peak begins
            self.segment_strength = strength;
            self.segment_end = self.time;
            self.segment_is_peak = true;
            self.segment_impulse_sent = false;
            self.evaluate_impulse(nudge);
        }
    }
}

/// A nudge sensor's two axes, read every millisecond (VPX's `PhysicsSensor`
/// values, their latest readings): side to side (right positive) and front
/// to back (towards the player positive), each from -1 to 1 after its
/// mapping, 0 while no axis is mapped to it.
pub trait NudgeAxes: std::fmt::Debug {
    fn values(&self) -> Vertex2D;
}

/// VPX's `GamepadNudge`: a stick's push turned into the player's intent to
/// nudge, which pushes a model of the cabinet; active for 10 s after the last
/// push.
#[derive(Debug)]
struct GamepadNudge {
    axes: std::rc::Rc<dyn NudgeAxes>,
    strength: f32,
    intent: NudgeIntent,
    cabinet: CabinetModel,
    deactivation_delay: i32,
}

impl GamepadNudge {
    fn step_one_millisecond(&mut self) {
        // the stick's position as an acceleration: VPX's values, from its tests
        let g = 9.80665f32;
        let v = self.axes.values();
        let ask = Vertex2D::new(v.x * (self.strength * (g * 0.5)), v.y * (self.strength * (g * 0.5)));
        self.intent.step_one_millisecond(ask);
        if self.intent.impulse_in_progress() {
            let mass = self.cabinet.mass;
            self.cabinet.step_one_millisecond(self.intent.impulse_acceleration() * mass);
            self.deactivation_delay = 10000;
        } else {
            self.cabinet.step_one_millisecond(Vertex2D::new(0.0, 0.0));
            if self.deactivation_delay > 0 {
                self.deactivation_delay -= 1;
            }
        }
    }

    fn is_active(&self) -> bool {
        self.deactivation_delay > 0
    }
}

/// One of a cabinet sensor's channels (VPX's `PhysicsSensor` and its
/// `SensorMapping`): its latest reading, the reading's time on the device's
/// clock in nanoseconds (0 before the first) and its value after its mapping,
/// m/s or m/s²; `None` while nothing is mapped to it.
pub trait SensorChannel: std::fmt::Debug {
    fn reading(&self) -> Option<(u64, f32)>;
}

/// A cabinet sensor's four channels: speed and acceleration, side to side
/// (right positive) and front to back (towards the player positive).
#[derive(Debug, Default)]
pub struct CabinetChannels {
    pub velocity_x: Option<std::rc::Rc<dyn SensorChannel>>,
    pub velocity_y: Option<std::rc::Rc<dyn SensorChannel>>,
    pub acceleration_x: Option<std::rc::Rc<dyn SensorChannel>>,
    pub acceleration_y: Option<std::rc::Rc<dyn SensorChannel>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChannelKind {
    Velocity,
    Acceleration,
}

/// VPX's `CabinetNudgeSensor::SyncedSensor`: a channel and what the sensor keeps for it — its
/// reading this millisecond, its clock against the filter's, its time at rest.
#[derive(Debug)]
struct SyncedChannel {
    channel: Option<std::rc::Rc<dyn SensorChannel>>,
    kind: ChannelKind,
    /// This millisecond's reading (read once a step, as VPX's input and physics share a thread).
    now: Option<(u64, f32)>,
    last_timestamp_ns: u64,
    clock_delta_ns: i64,
    rest_count: i32,
    force_rest: bool,
    last_value: f32,
}

impl SyncedChannel {
    fn new(channel: Option<std::rc::Rc<dyn SensorChannel>>, kind: ChannelKind) -> SyncedChannel {
        SyncedChannel { channel, kind, now: None, last_timestamp_ns: 0, clock_delta_ns: 0, rest_count: 0, force_rest: false, last_value: 0.0 }
    }

    fn is_mapped(&self) -> bool {
        self.channel.is_some()
    }

    /// Its value now, 0 while unmapped or before its first reading (VPX's `GetValue`).
    fn value(&self) -> f32 {
        self.now.map_or(0.0, |(_, v)| v)
    }

    /// Its reading's time on the device's clock, 0 before the first (VPX's `GetRawValueTimestampNs`).
    fn timestamp(&self) -> u64 {
        self.now.map_or(0, |(t, _)| t)
    }
}

/// VPX's `ExponentialMovingAverage`.
#[derive(Clone, Debug)]
struct Ema {
    tau: f32,
    value: f32,
    initialized: bool,
}

impl Ema {
    fn new(tau: f32) -> Ema {
        Ema { tau, value: 0.0, initialized: false }
    }

    /// `dt`: the time since the last reading, in seconds.
    fn update(&mut self, input: f32, dt: f32) -> f32 {
        if !self.initialized {
            self.value = input;
            self.initialized = true;
            return self.value;
        }
        let alpha = dt / (self.tau + dt);
        self.value += alpha * (input - self.value);
        self.value
    }
}

/// VPX's `CabinetNudgeSensor`: a real cabinet's motion sensors — velocity and acceleration on two
/// axes, any of them — run through a Kalman filter each millisecond (a reading arriving late is
/// applied at its own time), the filter told the cabinet is at rest when every mapped channel
/// has been still a while (its biases settle then), and the two channels' gain calibrated from
/// each nudge between rests (only once it's sure is the velocity channel used). As an intent
/// sensor, its acceleration is read as the player's intent, which pushes VPX's model of a
/// cabinet, as a gamepad's stick does; as a direct sensor, the acceleration itself pushes the
/// model, smoothed (4 ms) and scaled from the real cabinet's mass to the model's.
#[derive(Debug)]
struct CabinetNudgeSensor {
    x_gain: crate::gain_calibrator::GainCalibrator,
    y_gain: crate::gain_calibrator::GainCalibrator,
    x_vel: SyncedChannel,
    y_vel: SyncedChannel,
    x_acc: SyncedChannel,
    y_acc: SyncedChannel,
    kalman_x: crate::motion_kalman::MotionKalman,
    kalman_y: crate::motion_kalman::MotionKalman,
    ema_x: Ema,
    ema_y: Ema,
    time_ns: u64,
    intent: Option<NudgeIntent>,
    cabinet: CabinetModel,
    acceleration: Vertex2D,
    /// The real cabinet's mass, kg (VPX's `Mapping.Nudge<n>.CabWeight`, 113 by default).
    cabinet_mass: f32,
    strength: f32,
    deactivation_delay: i32,
}

/// How sure the gain must be before the velocity channel is used.
const GAIN_CONFIDENCE_THRESHOLD: f32 = 0.5;

impl CabinetNudgeSensor {
    fn new(channels: CabinetChannels, intent: bool, strength: f32, cabinet_mass: f32) -> CabinetNudgeSensor {
        use crate::gain_calibrator::{GainCalibrator, GainConfig};
        use crate::motion_kalman::{KalmanConfig, MotionKalman};
        CabinetNudgeSensor {
            x_gain: GainCalibrator::new(GainConfig::default()),
            y_gain: GainCalibrator::new(GainConfig::default()),
            x_vel: SyncedChannel::new(channels.velocity_x, ChannelKind::Velocity),
            y_vel: SyncedChannel::new(channels.velocity_y, ChannelKind::Velocity),
            x_acc: SyncedChannel::new(channels.acceleration_x, ChannelKind::Acceleration),
            y_acc: SyncedChannel::new(channels.acceleration_y, ChannelKind::Acceleration),
            kalman_x: MotionKalman::new(KalmanConfig::default()),
            kalman_y: MotionKalman::new(KalmanConfig::default()),
            // for USB's usual 125 readings a second and little latency
            ema_x: Ema::new(0.004),
            ema_y: Ema::new(0.004),
            time_ns: 0,
            intent: intent.then(|| NudgeIntent::new(false)),
            cabinet: CabinetModel::new(),
            acceleration: Vertex2D::new(0.0, 0.0),
            cabinet_mass,
            strength,
            deactivation_delay: 0,
        }
    }

    fn is_active(&self) -> bool {
        self.deactivation_delay > 0
    }

    /// VPX's `UpdateAxisSensor`: a channel's reading into the filter, at its time on the filter's
    /// clock (a reading ahead of it realigns the channel's clock to it).
    fn update_axis_sensor(time_ns: u64, deactivation_delay: &mut i32, sensor: &mut SyncedChannel, axis: &mut crate::motion_kalman::MotionKalman, axis_gain: f32) {
        if !sensor.is_mapped() {
            sensor.rest_count = 10000;
            return;
        }
        let rest_threshold = if sensor.kind == ChannelKind::Acceleration { 0.1000 } else { 0.0010 };
        if sensor.value().abs() < rest_threshold {
            sensor.rest_count += 1;
            if sensor.force_rest {
                return;
            }
        } else {
            sensor.rest_count = 0;
            sensor.force_rest = false;
            *deactivation_delay = 10000;
        }
        let timestamp = sensor.timestamp();
        if timestamp == 0 || timestamp == sensor.last_timestamp_ns {
            return;
        }
        sensor.last_timestamp_ns = timestamp;
        let mut aligned = timestamp.wrapping_add_signed(sensor.clock_delta_ns);
        if aligned > time_ns {
            sensor.clock_delta_ns = time_ns as i64 - timestamp as i64;
            aligned = time_ns;
        }
        match sensor.kind {
            ChannelKind::Velocity => axis.update_velocity(aligned, axis_gain * sensor.value()),
            ChannelKind::Acceleration => axis.update_acceleration(aligned, axis_gain * sensor.value()),
        }
    }

    /// VPX's `UpdateAxis`: one axis's two channels into its filter — while their gain isn't sure,
    /// the acceleration alone; then both, in the order they were read — rest detected (entered
    /// at a crossing after 75 still readings, or after 275 regardless, left at once on a move),
    /// and each nudge between rests a segment for the gain's calibration.
    fn update_axis(time_ns: u64, deactivation_delay: &mut i32, vel: &mut SyncedChannel, acc: &mut SyncedChannel, kalman: &mut crate::motion_kalman::MotionKalman, gain: &mut crate::gain_calibrator::GainCalibrator) {
        let both = acc.is_mapped() && vel.is_mapped();
        if both {
            if gain.global_confidence() < GAIN_CONFIDENCE_THRESHOLD || gain.gain() < 0.01 {
                Self::update_axis_sensor(time_ns, deactivation_delay, acc, kalman, 1.0);
                vel.rest_count = acc.rest_count;
            } else if acc.timestamp() < vel.timestamp() {
                Self::update_axis_sensor(time_ns, deactivation_delay, acc, kalman, 1.0);
                Self::update_axis_sensor(time_ns, deactivation_delay, vel, kalman, 1.0 / gain.gain());
            } else {
                Self::update_axis_sensor(time_ns, deactivation_delay, vel, kalman, 1.0 / gain.gain());
                Self::update_axis_sensor(time_ns, deactivation_delay, acc, kalman, 1.0);
            }
        } else {
            Self::update_axis_sensor(time_ns, deactivation_delay, acc, kalman, 1.0);
            Self::update_axis_sensor(time_ns, deactivation_delay, vel, kalman, 1.0);
        }
        // at rest: entered after a while, at a crossing for a cleaner output, or regardless after
        // longer (a small step in the derivatives); left at once when anything moves
        let cross_rest_count = 75;
        acc.force_rest |= acc.rest_count > cross_rest_count && acc.last_value * acc.value() < 0.0;
        vel.force_rest |= vel.rest_count > cross_rest_count && vel.last_value * vel.value() < 0.0;
        let force_rest_count = cross_rest_count + 200;
        let is_rest = (acc.force_rest || acc.rest_count > force_rest_count) && (vel.force_rest || vel.rest_count > force_rest_count);
        if is_rest {
            vel.force_rest = true;
            acc.force_rest = true;
            kalman.update_rest_constraints(time_ns);
        }
        acc.last_value = acc.value();
        vel.last_value = vel.value();
        // each nudge between rests is a segment for the channels' gain
        if both {
            if !is_rest {
                if !gain.is_segment_active() {
                    gain.start_segment(time_ns);
                }
                gain.add_sample(time_ns, vel.value(), acc.value());
            } else if gain.is_segment_active() {
                gain.end_segment();
            }
        }
        kalman.predict_to(time_ns);
    }

    fn step_one_millisecond(&mut self) {
        self.time_ns += 1_000_000;
        if self.deactivation_delay > 0 {
            self.deactivation_delay -= 1;
        }
        for c in [&mut self.x_vel, &mut self.y_vel, &mut self.x_acc, &mut self.y_acc] {
            c.now = c.channel.as_ref().and_then(|ch| ch.reading());
        }
        let t = self.time_ns;
        Self::update_axis(t, &mut self.deactivation_delay, &mut self.x_vel, &mut self.x_acc, &mut self.kalman_x, &mut self.x_gain);
        Self::update_axis(t, &mut self.deactivation_delay, &mut self.y_vel, &mut self.y_acc, &mut self.kalman_y, &mut self.y_gain);
        // the readings are m/s² or m/s (their mappings' scales make them so)
        if let Some(intent) = &mut self.intent {
            // VPX's empirical balance of side against front
            intent.step_one_millisecond(Vertex2D::new(self.kalman_x.acceleration() * self.strength * (4.0f64 / 3.0) as f32, self.kalman_y.acceleration() * self.strength));
            let mass = self.cabinet.mass;
            if intent.impulse_in_progress() {
                self.cabinet.step_one_millisecond(intent.impulse_acceleration() * mass);
            } else {
                self.cabinet.step_one_millisecond(Vertex2D::new(0.0, 0.0));
            }
            self.acceleration = self.cabinet.acceleration;
        } else {
            // the filter's acceleration smoothed, as its state isn't between readings
            self.ema_x.update(self.kalman_x.acceleration(), 0.001);
            self.ema_y.update(self.kalman_y.acceleration(), 0.001);
            self.acceleration = Vertex2D::new(self.ema_x.value * self.strength, self.ema_y.value * self.strength);
            // the real cabinet's force on the model's mass
            self.acceleration *= self.cabinet_mass / self.cabinet.mass;
            // the model, driven by it, for the picture's shake (the filter's position drifts)
            let mass = self.cabinet.mass;
            self.cabinet.step_one_millisecond(self.acceleration * mass);
        }
    }
}

/// A nudge sensor in the handler's list.
#[derive(Debug)]
enum Sensor {
    Gamepad(GamepadNudge),
    Cabinet(Box<CabinetNudgeSensor>),
}

impl Sensor {
    fn step_one_millisecond(&mut self) {
        match self {
            Sensor::Gamepad(s) => s.step_one_millisecond(),
            Sensor::Cabinet(s) => s.step_one_millisecond(),
        }
    }

    fn is_active(&self) -> bool {
        match self {
            Sensor::Gamepad(s) => s.is_active(),
            Sensor::Cabinet(s) => s.is_active(),
        }
    }

    fn acceleration(&self) -> Vertex2D {
        match self {
            Sensor::Gamepad(s) => s.cabinet.acceleration,
            Sensor::Cabinet(s) => s.acceleration,
        }
    }

    fn offset(&self) -> Vertex2D {
        match self {
            Sensor::Gamepad(s) => s.cabinet.position,
            Sensor::Cabinet(s) => s.cabinet.position,
        }
    }
}

/// VPX's `NudgeHandler`: what the cabinet does after the script's nudges
/// (the keyboard's), a gamepad stick's pushes and a cabinet's sensors, the
/// keyboard's counting while it's active, then the first active sensor, as
/// in VPX.
#[derive(Debug)]
pub struct NudgeHandler {
    keyboard: KeyboardNudge,
    sensors: Vec<Sensor>,
    /// The keyboard pushes so far (VPX's keyboard nudge index).
    keyboard_pushes: u32,
}

impl NudgeHandler {
    /// `strength`: the player's keyboard nudge strength (VPX's default 1);
    /// `nudge_time`: the table's `NudgeTime`.
    pub fn new(mode: KeyboardNudgeMode, strength: f32, nudge_time: f32) -> NudgeHandler {
        let keyboard = match mode {
            KeyboardNudgeMode::Vp9PushRetract => KeyboardNudge::PushRetract(PushRetract {
                strength,
                impulse: Vertex2D::new(0.0, 0.0),
                nudge_time: 0,
                acceleration: Vertex2D::new(0.0, 0.0),
                position: Vertex2D::new(0.0, 0.0),
                deactivation_delay: 0,
            }),
            KeyboardNudgeMode::VpxBoxModel => KeyboardNudge::BoxModel(BoxModel::new(strength, nudge_time)),
            KeyboardNudgeMode::VpxCabModel => KeyboardNudge::CabModel(CabModel { strength, impulses: Vec::new(), cabinet: CabinetModel::new(), deactivation_delay: 0 }),
        };
        NudgeHandler { keyboard, sensors: Vec::new(), keyboard_pushes: 0 }
    }

    /// A gamepad's stick as a nudge sensor, or another device's two axes read the same way
    /// (VPX's `GamepadNudge`: its default layouts' left stick, a Pinscape board's accelerometer);
    /// `strength`: its strength scale (VPX's `Mapping.Nudge<n>.Strength`, 1 by default).
    pub fn add_gamepad_sensor(&mut self, axes: std::rc::Rc<dyn NudgeAxes>, strength: f32) {
        self.sensors.push(Sensor::Gamepad(GamepadNudge { axes, strength, intent: NudgeIntent::new(true), cabinet: CabinetModel::new(), deactivation_delay: 0 }));
    }

    /// A cabinet's motion sensors (VPX's `CabinetNudgeSensor`): read as the player's intent
    /// (`intent`, VPX's sensor type 1) or applied directly (type 2); `strength`: its strength
    /// scale, `cabinet_mass`: the real cabinet's, kg (`Mapping.Nudge<n>.Strength`, `.CabWeight`).
    pub fn add_cabinet_sensor(&mut self, channels: CabinetChannels, intent: bool, strength: f32, cabinet_mass: f32) {
        self.sensors.push(Sensor::Cabinet(Box::new(CabinetNudgeSensor::new(channels, intent, strength, cabinet_mass))));
    }

    /// A sensor's state, in the order added: active (moved in the last 10 s), and its own
    /// cabinet acceleration (m/s²) and offset (m), whether or not it's the one used.
    pub fn sensor_state(&self, index: usize) -> Option<(bool, Vertex2D, Vertex2D)> {
        self.sensors.get(index).map(|s| (s.is_active(), s.acceleration(), s.offset()))
    }

    /// The keyboard model's state: active (nudged in the last 10 s), and its cabinet's
    /// acceleration (m/s²) and offset (m), whether or not it's the one used.
    pub fn keyboard_state(&self) -> (bool, Vertex2D, Vertex2D) {
        (self.keyboard.is_active(), self.keyboard.acceleration(), self.keyboard.offset())
    }

    /// How many keyboard pushes there have been (VPX's `GetKeyboardNudgeIndex`), so a nudge
    /// key's handler can tell whether the script pushed on its own.
    pub fn keyboard_index(&self) -> u32 {
        self.keyboard_pushes
    }

    /// The script's `Nudge`: a push of `force` towards `angle` degrees (0 is
    /// up the table, 90 to the right).
    pub fn apply_keyboard_impulse(&mut self, angle: f32, force: f32) {
        self.keyboard_pushes = self.keyboard_pushes.wrapping_add(1);
        self.keyboard.nudge(angle, force);
    }

    /// One physics step: after the timers, before the movers.
    pub fn step_one_millisecond(&mut self) {
        self.keyboard.step_one_millisecond();
        for s in &mut self.sensors {
            s.step_one_millisecond();
        }
    }

    /// The cabinet's acceleration, m/s²: the keyboard's while it's active, else the first
    /// active sensor's, else zero (10 s after the last nudge).
    pub fn cabinet_acceleration(&self) -> Vertex2D {
        if self.keyboard.is_active() {
            return self.keyboard.acceleration();
        }
        self.sensors.iter().find(|s| s.is_active()).map_or(Vertex2D::new(0.0, 0.0), Sensor::acceleration)
    }

    /// How far the cabinet has moved, in metres (for the picture's shake), from the same source.
    pub fn cabinet_offset(&self) -> Vertex2D {
        if self.keyboard.is_active() {
            return self.keyboard.offset();
        }
        self.sensors.iter().find(|s| s.is_active()).map_or(Vertex2D::new(0.0, 0.0), Sensor::offset)
    }
}

/// VPX's `ANGTORAD(x)`.
fn ang_to_rad(x: f32) -> f32 {
    x * (std::f64::consts::PI / 180.0) as f32
}

fn sqr(x: f32) -> f32 {
    x * x
}

/// VPX's `min`, `max`, `clamp` and `lerp` (src/core/def.h).
fn min(x: f32, y: f32) -> f32 {
    if x < y { x } else { y }
}
fn max(x: f32, y: f32) -> f32 {
    if x < y { y } else { x }
}
fn clamp(x: f32, mn: f32, mx: f32) -> f32 {
    max(min(x, mx), mn)
}
fn lerp(x1: f32, x2: f32, alpha: f32) -> f32 {
    (1.0 - alpha) * x1 + alpha * x2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untouched_cabinet_stays_still() {
        let mut n = NudgeHandler::new(KeyboardNudgeMode::VpxBoxModel, 1.0, 5.0);
        for _ in 0..1000 {
            n.step_one_millisecond();
        }
        assert_eq!(n.cabinet_acceleration(), Vertex2D::new(0.0, 0.0));
        assert_eq!(n.cabinet_offset(), Vertex2D::new(0.0, 0.0));
    }

    #[test]
    fn box_model_swings_back_and_settles() {
        let mut n = NudgeHandler::new(KeyboardNudgeMode::VpxBoxModel, 1.0, 5.0);
        n.apply_keyboard_impulse(90.0, 2.0);
        let (mut max_x, mut went_back) = (0.0f32, false);
        for _ in 0..5000 {
            n.step_one_millisecond();
            let x = n.cabinet_offset().x;
            max_x = max_x.max(x);
            went_back |= x < 0.0;
        }
        assert!(max_x > 0.0 && went_back, "swung {max_x}, back {went_back}");
        assert!(n.cabinet_offset().x.abs() < max_x * 1e-3, "settled at {}", n.cabinet_offset().x);
    }

    #[test]
    fn the_cabinet_model_pushes_for_25_ms_and_swings() {
        let mut n = NudgeHandler::new(KeyboardNudgeMode::VpxCabModel, 1.0, 5.0);
        n.apply_keyboard_impulse(90.0, 2.0);
        let mut accel = Vec::new();
        for _ in 0..2000 {
            n.step_one_millisecond();
            accel.push(n.cabinet_acceleration().x);
        }
        // pushed right, at most about half a g at the push's peak, then the cabinet's swing
        let peak = accel.iter().copied().fold(0.0f32, f32::max);
        assert!(peak > 2.0 && peak < 5.5, "{peak}");
        assert!(accel[100..].iter().any(|&a| a < 0.0), "swings back");
        assert!(n.cabinet_offset().x.abs() < 0.0005, "settled at {}", n.cabinet_offset().x);
    }

    #[test]
    fn vp9_pushes_then_pulls() {
        let mut n = NudgeHandler::new(KeyboardNudgeMode::Vp9PushRetract, 1.0, 5.0);
        n.apply_keyboard_impulse(90.0, 2.0);
        let mut seen = Vec::new();
        for _ in 0..12 {
            n.step_one_millisecond();
            seen.push(n.cabinet_acceleration().x);
        }
        // nothing for 4 ms, a push at 5 ms, nothing, the pull back at 10 ms
        assert!(seen[4] < 0.0 && seen[9] > 0.0, "{seen:?}");
        assert!(seen[..4].iter().all(|&a| a == 0.0) && seen[5] == 0.0, "{seen:?}");
    }

    /// A stick held where a test puts it.
    #[derive(Debug)]
    struct Stick(std::cell::Cell<Vertex2D>);

    impl NudgeAxes for Stick {
        fn values(&self) -> Vertex2D {
            self.0.get()
        }
    }

    /// A handler with a stick as its sensor, the keyboard left alone.
    fn with_stick() -> (NudgeHandler, std::rc::Rc<Stick>) {
        let stick = std::rc::Rc::new(Stick(std::cell::Cell::new(Vertex2D::new(0.0, 0.0))));
        let mut n = NudgeHandler::new(KeyboardNudgeMode::VpxBoxModel, 1.0, 5.0);
        n.add_gamepad_sensor(stick.clone(), 1.0);
        (n, stick)
    }

    #[test]
    fn a_stick_pushed_right_nudges_the_cabinet_which_settles() {
        let (mut n, stick) = with_stick();
        let (mut max_x, mut max_accel, mut went_back) = (0.0f32, 0.0f32, false);
        for ms in 0..6000 {
            // pushed halfway, then a reading later (16 ms, a gamepad's) all the way, let go after 50 ms
            stick.0.set(Vertex2D::new(
                match ms {
                    100..116 => 0.5,
                    116..166 => 1.0,
                    _ => 0.0,
                },
                0.0,
            ));
            n.step_one_millisecond();
            max_x = max_x.max(n.cabinet_offset().x);
            max_accel = max_accel.max(n.cabinet_acceleration().x);
            went_back |= n.cabinet_offset().x < 0.0;
            if ms < 100 {
                assert_eq!(n.cabinet_acceleration(), Vertex2D::new(0.0, 0.0));
            }
        }
        // a firm nudge moves a cabinet a few millimetres (VPX's model: 3 to 5 mm)
        assert!(max_x > 0.002 && max_x < 0.008 && went_back, "swung {max_x} m, back {went_back}");
        assert!(max_accel > 1.0 && n.cabinet_acceleration().y == 0.0, "{max_accel}");
        assert!(n.cabinet_offset().x.abs() < max_x * 1e-3, "settled at {}", n.cabinet_offset().x);
        // 10 s after the push the stick's nudge is over
        for _ in 0..5000 {
            n.step_one_millisecond();
        }
        assert_eq!((n.cabinet_acceleration(), n.cabinet_offset()), (Vertex2D::new(0.0, 0.0), Vertex2D::new(0.0, 0.0)));
    }

    #[test]
    fn a_resting_stick_and_a_stick_pulled_back_do_nothing() {
        let (mut n, stick) = with_stick();
        // a worn stick's wobble around its middle, up to 12% of its travel each way
        let mut seed = 12345u32;
        for ms in 0..2000 {
            if ms % 16 == 0 {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let wobble = |s: u32| (f32::from((s >> 8) as u16) / 65535.0 - 0.5) * 0.24;
                stick.0.set(Vertex2D::new(wobble(seed), wobble(seed.rotate_left(16))));
            }
            n.step_one_millisecond();
            assert_eq!(n.cabinet_acceleration(), Vertex2D::new(0.0, 0.0), "at {ms} ms");
        }
        // pulled towards the player, all the way: a front nudge only pushes
        stick.0.set(Vertex2D::new(0.0, 1.0));
        for _ in 0..500 {
            n.step_one_millisecond();
            assert_eq!(n.cabinet_acceleration(), Vertex2D::new(0.0, 0.0));
        }
        // pushed away from the player: the cabinet moves away too
        stick.0.set(Vertex2D::new(0.0, -1.0));
        let mut min_y = 0.0f32;
        for _ in 0..200 {
            n.step_one_millisecond();
            min_y = min_y.min(n.cabinet_offset().y);
        }
        assert!(min_y < -0.001, "{min_y}");
    }

    #[test]
    fn a_firm_push_comes_sooner_than_a_soft_one() {
        // the impulse is held back by three of a gamepad's readings for a soft push, one for a firm one
        let start = |value: f32| {
            let mut i = NudgeIntent::new(true);
            let g = 9.80665f32;
            (1..200).find(|_| {
                i.step_one_millisecond(Vertex2D::new(value * (g * 0.5), 0.0));
                i.impulse_acceleration().x > 0.0
            })
        };
        let (soft, firm) = (start(0.25), start(1.0));
        assert!(soft.unwrap() > 40 && firm.unwrap() < 20, "{soft:?} {firm:?}");
        assert_eq!(start(0.2), None, "below 1 m/s², no nudge");
    }

    #[test]
    fn a_cabinet_sensor_passes_over_its_own_swing() {
        // a strong push, then a weaker peak 50 ms later: the cabinet swinging back, for a cabinet's
        // sensor; a second push for a gamepad's
        for (is_gamepad, second_impulse) in [(false, false), (true, true)] {
            let mut i = NudgeIntent::new(is_gamepad);
            for ms in 0..120 {
                let x = match ms {
                    0..10 => 4.0,
                    60..70 => 3.0,
                    _ => 0.0,
                };
                i.step_one_millisecond(Vertex2D::new(x, 0.0));
            }
            // the first impulse began with the first reading; a second one, if any, 60 ms later
            assert_eq!(i.impulse_elapsed < 100, second_impulse, "gamepad {is_gamepad}: {}", i.impulse_elapsed);
        }
    }

    /// A cabinet sensor's channel, read as a test sets it.
    #[derive(Debug)]
    struct Channel(std::cell::Cell<Option<(u64, f32)>>);

    impl SensorChannel for Channel {
        fn reading(&self) -> Option<(u64, f32)> {
            self.0.get()
        }
    }

    fn channel() -> std::rc::Rc<Channel> {
        std::rc::Rc::new(Channel(std::cell::Cell::new(Some((0, 0.0)))))
    }

    /// A real cabinet, firmly damped, pushed by `force` newtons for `ms` milliseconds once a
    /// second from 1 s on: its acceleration and speed each millisecond.
    fn pushed_cabinet(seconds: u64, force: f32, ms: u64) -> Vec<(f32, f32)> {
        let mut cab = Oscillator::new(113.0, 9.3, 0.3);
        (0..seconds * 1000)
            .map(|t| {
                let pushing = t >= 1000 && t % 1000 < ms;
                cab.step(if pushing { force } else { 0.0 }, 0.001);
                (cab.acceleration, cab.velocity)
            })
            .collect()
    }

    #[test]
    fn a_cabinet_accelerometer_nudges_directly_and_by_intent() {
        let motion = pushed_cabinet(3, 700.0, 40);
        for intent in [false, true] {
            let acc_x = channel();
            let mut n = NudgeHandler::new(KeyboardNudgeMode::VpxBoxModel, 1.0, 5.0);
            n.add_cabinet_sensor(CabinetChannels { acceleration_x: Some(acc_x.clone()), ..CabinetChannels::default() }, intent, 1.0, 113.0);
            // readings every 8 ms, stamped on a device clock 5 s ahead of the physics'
            let base = 5_000_000_000u64;
            let (mut max_acc, mut max_off) = (0f32, 0f32);
            for (ms, &(a, _)) in motion.iter().enumerate() {
                if ms % 8 == 0 {
                    acc_x.0.set(Some((base + ms as u64 * 1_000_000, a)));
                }
                n.step_one_millisecond();
                if ms < 1000 {
                    assert_eq!(n.cabinet_acceleration(), Vertex2D::new(0.0, 0.0), "still, at {ms} ms");
                }
                max_acc = max_acc.max(n.cabinet_acceleration().x.abs());
                max_off = max_off.max(n.cabinet_offset().x.abs());
            }
            // (the model, lightly damped, swings further than the firmly damped cabinet pushing it)
            assert!(max_acc > 1.0 && max_off > 0.0005 && max_off < 0.03, "intent {intent}: {max_acc} m/s², {max_off} m");
            assert_eq!(n.cabinet_acceleration().y, 0.0);
        }
    }

    #[test]
    fn a_cabinet_sensors_two_channels_are_calibrated_against_each_other() {
        // the velocity in m/s, the acceleration channel's raw readings a quarter of m/s²
        let motion = pushed_cabinet(14, 700.0, 40);
        let (vel_x, acc_x) = (channel(), channel());
        let mut n = NudgeHandler::new(KeyboardNudgeMode::VpxBoxModel, 1.0, 5.0);
        n.add_cabinet_sensor(CabinetChannels { velocity_x: Some(vel_x.clone()), acceleration_x: Some(acc_x.clone()), ..CabinetChannels::default() }, false, 1.0, 113.0);
        for (ms, &(a, v)) in motion.iter().enumerate() {
            if ms % 8 == 0 {
                let at = 1_000_000_000 + ms as u64 * 1_000_000;
                vel_x.0.set(Some((at, v)));
                acc_x.0.set(Some((at, a / 4.0)));
            }
            n.step_one_millisecond();
        }
        let Sensor::Cabinet(s) = &n.sensors[0] else { unreachable!() };
        let (gain, confidence) = (s.x_gain.gain(), s.x_gain.global_confidence());
        assert!(confidence >= GAIN_CONFIDENCE_THRESHOLD && (gain - 4.0).abs() < 0.4, "gain {gain}, confidence {confidence}, {} accepted, {} set aside", s.x_gain.accepted_segments(), s.x_gain.rejected_segments());
    }

    #[test]
    fn the_keyboard_counts_over_the_stick() {
        let (mut n, stick) = with_stick();
        stick.0.set(Vertex2D::new(1.0, 0.0));
        for _ in 0..20 {
            n.step_one_millisecond();
        }
        n.apply_keyboard_impulse(270.0, 2.0);
        n.step_one_millisecond();
        let KeyboardNudge::BoxModel(k) = &n.keyboard else { unreachable!() };
        assert_eq!((n.cabinet_acceleration(), n.cabinet_offset()), (k.acceleration, k.position));
        assert!(n.sensors[0].is_active() && n.sensors[0].acceleration() != k.acceleration);
    }
}
