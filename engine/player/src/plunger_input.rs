// Ported from Visual Pinball X build 5436 (10.8.1 pre-release, commit af26b2d): src/input/PlungerHandler.cpp and
// PlungerHandler.h (the plunger's handler and its sensor), src/input/PlungerKalmanFilter.h (its filter) and
// src/math/ExponentialMovingAverage.h; the helpers lerp, saturate and infNaN, and the unit conversions, from
// src/core/def.h. license:GPLv3+ — the three headers carry no
// marker, but were added in 2026 by VPX's team with no third-party content, so VPX's licence makes them GPL too
//. Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A cabinet's analog plunger: a sensor's
//! axis — a gamepad's right stick, pulled down as the plunger is pulled
//! back, or a cabinet board's plunger, as VPX's default layouts map them, or
//! any axis the settings name (`sensors`) — filtered as VPX filters it into
//! a position and a speed every physics millisecond, and handed to the
//! physics' mechanical plunger, ported with the physics
//! (`physics_compat::plunger::PlungerSensor`). A sensor measuring the
//! plunger's speed as well gives the speed it hits the ball with. The
//! plunger only takes over once a sensor moves, for ten seconds after its
//! last move, as in VPX, so a gamepad lying still leaves the launch key in
//! charge.

use std::cell::RefCell;

use crate::sensors::{Axis, PlungerSensor as SensorAxes};

/// VPX's `CMTOVPU`-family conversions used here, in single precision as VPX
/// computes them.
fn vpu_to_m(x: f32) -> f32 {
    x * (0.0254 * 1.0625 / 50.0) as f32
}
fn inches_to_vpu(x: f32) -> f32 {
    x * (50.0 / 1.0625) as f32
}

/// VPX's `lerp`: `(1 - alpha) * x1 + alpha * x2`.
fn lerp(x1: f32, x2: f32, alpha: f32) -> f32 {
    (1.0 - alpha) * x1 + alpha * x2
}

/// VPX's `saturate`.
fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// VPX's `infNaN` negated: neither infinite nor NaN.
fn is_finite(v: f32) -> bool {
    v.to_bits() & 0x7F80_0000 != 0x7F80_0000
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

/// VPX's `PlungerKalmanFilter::Config`.
#[derive(Clone, Copy, Debug)]
struct KalmanConfig {
    acceleration_process_noise: f32,
    position_measurement_variance: f32,
    initial_position_variance: f32,
    initial_velocity_variance: f32,
    enable_position_limits: bool,
    min_position: f32,
    max_position: f32,
    history_capacity: i32,
}

impl Default for KalmanConfig {
    fn default() -> KalmanConfig {
        KalmanConfig {
            acceleration_process_noise: 1.0,
            position_measurement_variance: 1.0e-5,
            initial_position_variance: 1.0e-4,
            initial_velocity_variance: 1.0e-2,
            enable_position_limits: false,
            min_position: 0.0,
            max_position: 1.0,
            history_capacity: 64,
        }
    }
}

type Mat2 = [[f32; 2]; 2];

/// One of the filter's saved states, for going back to a sample that comes late.
#[derive(Clone, Copy, Debug, Default)]
struct Snapshot {
    time_ns: u64,
    x: [f32; 2],
    p: Mat2,
}

/// VPX's `PlungerKalmanFilter`: the plunger's position and speed estimated
/// from position readings that come irregularly (a gamepad's at about 125 a
/// second, repeated when still), predicted in steps of at most a
/// millisecond, and taken back to a saved state for a reading that comes
/// late. VPX keeps the covariance in a 3×3 matrix and uses its 2×2 corner;
/// this keeps the corner.
#[derive(Clone, Debug)]
struct Kalman {
    config: KalmanConfig,
    /// Position and velocity.
    x: [f32; 2],
    p: Mat2,
    time_ns: u64,
    history: [Snapshot; 256],
    head: i32,
    count: i32,
}

impl Kalman {
    fn new(config: KalmanConfig) -> Kalman {
        let mut k = Kalman { config: KalmanConfig::default(), x: [0.0; 2], p: [[0.0; 2]; 2], time_ns: 0, history: [Snapshot::default(); 256], head: 0, count: 0 };
        k.reset(0.0, 0.0, 0);
        k.config = config;
        k.reset(0.0, 0.0, 0);
        k
    }

    fn reset(&mut self, position: f32, velocity: f32, timestamp_ns: u64) {
        self.x = [position, velocity];
        self.p = [[0.0; 2]; 2];
        self.p[0][0] = self.config.initial_position_variance.max(1.0e-12);
        self.p[1][1] = self.config.initial_velocity_variance.max(1.0e-12);
        self.apply_position_limits();
        self.time_ns = timestamp_ns;
        self.head = 0;
        self.count = 0;
        self.save_snapshot();
    }

    fn position(&self) -> f32 {
        self.x[0]
    }

    fn velocity(&self) -> f32 {
        self.x[1]
    }

    fn predict_to(&mut self, timestamp_ns: u64) {
        if timestamp_ns <= self.time_ns {
            return;
        }
        let dt = (timestamp_ns - self.time_ns) as f32 * 1.0e-9;
        self.save_snapshot();
        self.predict_internal(dt);
        self.time_ns = timestamp_ns;
    }

    fn update_position(&mut self, timestamp_ns: u64, measured: f32) {
        if !is_finite(measured) {
            return;
        }
        if self.time_ns == 0 {
            self.time_ns = timestamp_ns;
            self.head = 0;
            self.count = 0;
            self.save_snapshot();
        }
        if timestamp_ns >= self.time_ns {
            if timestamp_ns > self.time_ns {
                self.save_snapshot();
                self.predict_internal((timestamp_ns - self.time_ns) as f32 * 1.0e-9);
                self.time_ns = timestamp_ns;
            }
            self.apply_measurement(measured);
        } else {
            let present = self.time_ns;
            self.rewind_and_update(timestamp_ns, measured);
            if self.time_ns < present {
                self.save_snapshot();
                self.predict_internal((present - self.time_ns) as f32 * 1.0e-9);
                self.time_ns = present;
            }
        }
    }

    fn update_velocity_zero(&mut self, velocity_variance: f32) {
        let r = velocity_variance.max(1.0e-12);
        let y = -self.x[1];
        let s = self.p[1][1] + r;
        if !is_finite(s) || s <= 1.0e-20 {
            return;
        }
        let k = [self.p[0][1] / s, self.p[1][1] / s];
        self.x[0] += k[0] * y;
        self.x[1] += k[1] * y;
        self.p[0][0] -= k[0] * s * k[0];
        self.p[0][1] -= k[0] * s * k[1];
        self.p[1][0] -= k[1] * s * k[0];
        self.p[1][1] -= k[1] * s * k[1];
        self.symmetrize_covariance();
        self.apply_position_limits();
    }

    fn capacity(&self) -> i32 {
        self.config.history_capacity.clamp(1, 256)
    }

    fn save_snapshot(&mut self) {
        let cap = self.capacity();
        self.history[self.head as usize] = Snapshot { time_ns: self.time_ns, x: self.x, p: self.p };
        self.head = (self.head + 1) % cap;
        if self.count < cap {
            self.count += 1;
        }
    }

    fn restore_snapshot(&mut self, target_ns: u64) -> bool {
        let cap = self.capacity();
        if self.count == 0 {
            return false;
        }
        // the newest saved state at or before the target, else the oldest
        let mut best = None;
        for i in 0..self.count {
            let slot = (self.head - 1 - i).rem_euclid(cap);
            let ts = self.history[slot as usize].time_ns;
            if ts <= target_ns {
                best = Some((slot, ts));
                break;
            }
        }
        let (best_slot, best_ts) = best.unwrap_or_else(|| {
            let oldest = (self.head - self.count).rem_euclid(cap);
            (oldest, self.history[oldest as usize].time_ns)
        });
        let s = self.history[best_slot as usize];
        self.x = s.x;
        self.p = s.p;
        self.time_ns = best_ts;
        // forget the states after it
        let mut keep = 0;
        for i in 0..self.count {
            let slot = (self.head - self.count + i).rem_euclid(cap);
            keep = i + 1;
            if slot == best_slot {
                break;
            }
        }
        self.count = keep;
        self.head = (best_slot + 1) % cap;
        true
    }

    fn rewind_and_update(&mut self, timestamp_ns: u64, measured: f32) {
        if !self.restore_snapshot(timestamp_ns) {
            self.apply_measurement(measured);
            return;
        }
        if self.time_ns < timestamp_ns {
            self.predict_internal((timestamp_ns - self.time_ns) as f32 * 1.0e-9);
            self.time_ns = timestamp_ns;
        }
        self.apply_measurement(measured);
    }

    fn process_noise(&self, dt: f32) -> Mat2 {
        let q = self.config.acceleration_process_noise.max(0.0);
        let dt2 = dt * dt;
        let dt3 = dt2 * dt;
        let off = q * dt2 / 2.0;
        [[q * dt3 / 3.0, off], [off, q * dt]]
    }

    fn symmetrize_covariance(&mut self) {
        let p01 = 0.5 * (self.p[0][1] + self.p[1][0]);
        self.p[0][1] = p01;
        self.p[1][0] = p01;
        self.p[0][0] = self.p[0][0].max(1.0e-12);
        self.p[1][1] = self.p[1][1].max(1.0e-12);
    }

    fn apply_position_limits(&mut self) {
        if !self.config.enable_position_limits {
            return;
        }
        self.x[0] = self.x[0].clamp(self.config.min_position, self.config.max_position);
        if self.x[0] <= self.config.min_position && self.x[1] < 0.0 {
            self.x[1] = 0.0;
        }
        if self.x[0] >= self.config.max_position && self.x[1] > 0.0 {
            self.x[1] = 0.0;
        }
    }

    fn predict_internal(&mut self, mut time_step: f32) {
        if !is_finite(time_step) || time_step <= 0.0 {
            return;
        }
        while time_step > 0.0 {
            let dt = time_step.min(0.001);
            time_step -= dt;
            let f: Mat2 = [[1.0, dt], [0.0, 1.0]];
            self.x = [f[0][0] * self.x[0] + f[0][1] * self.x[1], f[1][0] * self.x[0] + f[1][1] * self.x[1]];
            let mut fp = [[0.0f32; 2]; 2];
            for (r, row) in fp.iter_mut().enumerate() {
                for (c, v) in row.iter_mut().enumerate() {
                    *v = f[r][0] * self.p[0][c] + f[r][1] * self.p[1][c];
                }
            }
            let q = self.process_noise(dt);
            let mut p = [[0.0f32; 2]; 2];
            for (r, row) in p.iter_mut().enumerate() {
                for (c, v) in row.iter_mut().enumerate() {
                    // F P Fᵀ, then the process noise added
                    *v = (fp[r][0] * f[c][0] + fp[r][1] * f[c][1]) + q[r][c];
                }
            }
            self.p = p;
            self.symmetrize_covariance();
            self.apply_position_limits();
        }
    }

    fn apply_measurement(&mut self, measured: f32) {
        let r = self.config.position_measurement_variance.max(1.0e-12);
        let y = measured - self.x[0];
        let s = self.p[0][0] + r;
        if !is_finite(s) || s <= 1.0e-20 {
            return;
        }
        let k = [self.p[0][0] / s, self.p[1][0] / s];
        self.x[0] += k[0] * y;
        self.x[1] += k[1] * y;
        self.p[0][0] -= k[0] * s * k[0];
        self.p[0][1] -= k[0] * s * k[1];
        self.p[1][0] -= k[1] * s * k[0];
        self.p[1][1] -= k[1] * s * k[1];
        self.symmetrize_covariance();
        self.apply_position_limits();
    }
}

/// VPX's `PlungerSensor`: 0 at rest, 1 fully pulled back.
#[derive(Clone, Debug)]
struct Sensor {
    last_timestamp_ns: u64,
    time_ns: u64,
    clock_delta_ns: i64,
    n_rest_samples: i32,
    deactivation_delay: i32,
    kalman: Kalman,
    ema_position: Ema,
    ema_velocity: Ema,
    /// A fully pulled plunger's length in metres, for the filter's units.
    kalman_unit_scale: f32,
    position: f32,
    prev_position: [f32; 100],
    prev_position_pos: usize,
    /// The speed axis's value this millisecond, if one is mapped.
    velocity: Option<f32>,
}

impl Sensor {
    fn new() -> Sensor {
        let kalman_unit_scale = vpu_to_m(inches_to_vpu(3.0));
        let config = KalmanConfig {
            position_measurement_variance: 1.0e-5,
            acceleration_process_noise: 0.5,
            initial_position_variance: 1.0e-4,
            initial_velocity_variance: 1.0e-2,
            enable_position_limits: true,
            // fully forward, fully pulled back
            min_position: -kalman_unit_scale / 3.0,
            max_position: kalman_unit_scale,
            ..KalmanConfig::default()
        };
        Sensor {
            last_timestamp_ns: 0,
            time_ns: 0,
            clock_delta_ns: 0,
            n_rest_samples: 1000,
            deactivation_delay: 0,
            kalman: Kalman::new(config),
            // time constants for a USB device's 125 readings a second and little latency
            ema_position: Ema::new(0.004),
            ema_velocity: Ema::new(0.004),
            kalman_unit_scale,
            position: 0.0,
            prev_position: [0.0; 100],
            prev_position_pos: 0,
            velocity: None,
        }
    }

    fn is_active(&self) -> bool {
        self.deactivation_delay > 0
    }

    /// VPX's `PlungerSensor::StepOneMillisecond`, the position axis's last
    /// reading given (its time on the device's clock and its value through
    /// its mapping), None while unmapped.
    fn step_one_millisecond(&mut self, reading: Option<(u64, f32)>) {
        self.time_ns += 1_000_000;
        if self.deactivation_delay > 0 {
            self.deactivation_delay -= 1;
        }
        let Some((raw_ts, value)) = reading else { return };
        // a still plunger, and ten seconds' activity after any move
        if value.abs() < 0.01 {
            self.n_rest_samples = self.n_rest_samples.saturating_add(1);
        } else {
            self.n_rest_samples = 0;
        }
        if self.n_rest_samples < 10 {
            self.deactivation_delay = 10_000;
        }
        let at_rest = self.n_rest_samples > 30;
        // a continuous 1 ms estimate from irregular readings
        let mut ts = raw_ts.wrapping_add_signed(self.clock_delta_ns);
        if ts > self.last_timestamp_ns {
            if ts > self.time_ns {
                // a reading ahead of the filter's clock: the sensor's clock realigned to it
                ts = self.time_ns;
                self.clock_delta_ns = ts as i64 - raw_ts as i64;
            }
            self.last_timestamp_ns = ts;
            self.kalman.update_position(self.last_timestamp_ns, value * self.kalman_unit_scale);
        } else if self.time_ns > self.last_timestamp_ns + 10_000_000 {
            // the last reading repeated, so the estimate doesn't run free too long
            self.last_timestamp_ns = self.time_ns - 5_000_000;
            self.kalman.update_position(self.last_timestamp_ns, value * self.kalman_unit_scale);
        }
        self.kalman.predict_to(self.time_ns);
        if at_rest {
            self.kalman.update_velocity_zero(1.0e-4);
        }
        self.ema_velocity.update(self.kalman.velocity() / self.kalman_unit_scale, 0.001);
        self.position = self.ema_position.update(self.kalman.position() / self.kalman_unit_scale, 0.001);
        self.prev_position[self.prev_position_pos] = self.position;
        self.prev_position_pos = (self.prev_position_pos + 1) % self.prev_position.len();
    }

    fn raw_velocity(&self) -> f32 {
        self.ema_velocity.value
    }

    /// VPX's `PlungerSensor::GetHitVelocity`: the speed axis's value when the
    /// hardware measures it; else a free release from the furthest point
    /// back in the last 100 ms, at the speed the launch key would give from
    /// there, none unless the plunger has come at least a tenth of its
    /// travel forward from that point.
    fn hit_velocity(&self, rest_pos: f32) -> f32 {
        if let Some(v) = self.velocity {
            return v;
        }
        let apex = self.prev_position.iter().copied().fold(f32::MIN, f32::max);
        let hit_speed = -(apex - rest_pos).max(0.0) * 100.0 / 13.0;
        let current = lerp(rest_pos, 1.0, self.position).clamp(0.0, 1.0);
        if current > apex - 0.1 { 0.0 } else { hit_speed }
    }
}

/// Where a sensor's axes come from: the settings, or a default layout's slot.
#[derive(Clone, Copy, Debug)]
enum Source {
    Settings(SensorAxes),
    Layout(usize),
}

impl Source {
    /// The position axis's reading and the speed axis's value now.
    fn readings(&self) -> (Option<(u64, f32)>, Option<f32>) {
        let (position, velocity): (Option<Axis>, Option<Axis>) = match *self {
            Source::Settings(s) => (s.position, s.velocity),
            Source::Layout(slot) => crate::sensors::layout_plunger(slot).map_or((None, None), |p| (p.position, p.velocity)),
        };
        (position.map(|a| a.reading()), velocity.map(|a| a.reading().1))
    }
}

/// VPX's `PlungerHandler`: the settings' sensors, then the default layouts'
/// (a gamepad's right stick, a cabinet board's plunger), the first sensor
/// active giving the plunger its place and speed.
#[derive(Debug)]
pub struct PlungerHandler {
    sensors: RefCell<Vec<(Source, Sensor)>>,
    position: std::cell::Cell<f32>,
    raw_velocity: std::cell::Cell<f32>,
}

impl PlungerHandler {
    pub fn new(sensors: &crate::sensors::Sensors) -> PlungerHandler {
        let sources = sensors.plunger.iter().map(|&s| Source::Settings(s)).chain((0..crate::sensors::LAYOUT_SLOTS).map(Source::Layout));
        PlungerHandler {
            sensors: RefCell::new(sources.map(|s| (s, Sensor::new())).collect()),
            position: std::cell::Cell::new(0.0),
            raw_velocity: std::cell::Cell::new(0.0),
        }
    }

    /// Each sensor's millisecond, with the readings `read` gives it.
    fn step_with(&self, read: impl Fn(&Source) -> (Option<(u64, f32)>, Option<f32>)) {
        let mut sensors = self.sensors.borrow_mut();
        for (source, s) in sensors.iter_mut() {
            let (position, velocity) = read(source);
            s.velocity = velocity;
            s.step_one_millisecond(position);
        }
        self.position.set(0.0);
        self.raw_velocity.set(0.0);
        if let Some((_, s)) = sensors.iter().find(|(_, s)| s.is_active()) {
            self.position.set(s.position);
            self.raw_velocity.set(s.raw_velocity());
        }
    }
}

impl physics_compat::plunger::PlungerSensor for PlungerHandler {
    fn has_sensor(&self) -> bool {
        self.sensors.borrow().iter().any(|(_, s)| s.is_active())
    }

    /// VPX's `GetPosition`: 0 is the plunger's rest, 1 fully pulled back;
    /// forward of rest isn't calibrated, and stops at the frame's end.
    fn position(&self, rest_pos: f32) -> f32 {
        saturate(lerp(rest_pos, 1.0, self.position.get()))
    }

    fn raw_velocity(&self) -> f32 {
        self.raw_velocity.get()
    }

    fn hit_velocity(&self, rest_pos: f32) -> f32 {
        self.sensors.borrow().iter().find(|(_, s)| s.is_active()).map_or(0.0, |(_, s)| s.hit_velocity(rest_pos))
    }

    fn step_one_millisecond(&self) {
        self.step_with(Source::readings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics_compat::plunger::PlungerSensor as _;

    #[test]
    fn the_filter_follows_a_reading_and_holds_still() {
        let mut k = Kalman::new(KalmanConfig { enable_position_limits: true, min_position: -0.025, max_position: 0.0762, ..KalmanConfig::default() });
        // steady readings, a millisecond apart, settle at the reading, the speed near nothing
        for t in 1..100u64 {
            k.update_position(t * 1_000_000, 0.05);
        }
        assert!((k.position() - 0.05).abs() < 1e-3 && k.velocity().abs() < 0.05, "{:?} {:?}", k.x, k.p);
        // a reading past the full pull is held to it
        k.update_position(120_000_000, 0.5);
        k.predict_to(140_000_000);
        assert!(k.position() <= 0.0762 + 1e-6);
    }

    #[test]
    fn a_late_reading_goes_back_to_its_moment() {
        let mut k = Kalman::new(KalmanConfig::default());
        k.update_position(1_000_000, 0.0);
        k.predict_to(20_000_000);
        k.update_position(10_000_000, 0.4); // came late
        assert_eq!(k.time_ns, 20_000_000, "the filter's clock stays at the present");
        assert!(k.position() > 0.1, "{k:?}");
    }

    #[test]
    fn the_stick_takes_over_only_once_it_moves() {
        let h = PlungerHandler::new(&crate::sensors::Sensors::default());
        // the first layout slot's stick, the others empty
        let stick = |reading: Option<(u64, f32)>| move |s: &Source| (if matches!(s, Source::Layout(0)) { reading } else { None }, None);
        for ms in 1..=50u64 {
            h.step_with(stick(Some((ms * 1_000_000, 0.0))));
        }
        assert!(!h.has_sensor(), "a still stick leaves the launch key in charge");
        // pulled all the way back, held: the plunger follows, fully back
        for ms in 51..=400u64 {
            h.step_with(stick(Some((ms * 1_000_000, 1.0))));
        }
        assert!(h.has_sensor());
        assert!(h.position(0.15) > 0.99, "{}", h.position(0.15));
        // let go: back to rest at once, a release from the top, so a launch at the key's full speed
        let mut hit = 0.0f32;
        for ms in 401..=420u64 {
            h.step_with(stick(Some((ms * 1_000_000, 0.0))));
            hit = hit.min(h.hit_velocity(0.15));
        }
        assert!((hit - (-(1.0 - 0.15) * 100.0 / 13.0)).abs() < 0.05, "{hit}");
    }

    #[test]
    fn a_speed_axis_gives_the_hit_speed() {
        let h = PlungerHandler::new(&crate::sensors::Sensors::default());
        // pulled back, then released: the speed axis reads -3 as the plunger flies forward
        for ms in 1..=300u64 {
            let (position, speed) = if ms < 250 { (1.0, 0.0) } else { (0.0, -3.0) };
            h.step_with(move |s: &Source| if matches!(s, Source::Layout(0)) { (Some((ms * 1_000_000, position)), Some(speed)) } else { (None, None) });
        }
        assert_eq!(h.hit_velocity(0.15), -3.0);
        assert_eq!(saturate(lerp(0.2, 1.0, -1.0)), 0.0);
    }
}
