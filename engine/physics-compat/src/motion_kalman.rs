// Ported from Visual Pinball X build 5436 (10.8.1 pre-release, commit af26b2d):
// src/physics/cabinet/MotionKalmanAxis.h (license:GPLv3+).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX 5436's motion filter for a cabinet's nudge sensors: a Kalman filter
//! over one axis's position, velocity and acceleration, with the sensor's
//! velocity and acceleration biases as two more states, each drifting slowly
//! and drawn back toward zero over a few seconds. Fed velocity or
//! acceleration readings, and readings of zero when the cabinet is known to
//! be at rest; it keeps a short history of its states so a reading that
//! arrives late is applied at its own time. Operations in VPX's order, so
//! the results are its results.

const POSITION: usize = 0;
const VELOCITY: usize = 1;
const ACCELERATION: usize = 2;
const VELOCITY_BIAS: usize = 3;
const ACCELERATION_BIAS: usize = 4;
const N: usize = 5;

type V5 = [f32; N];
type M5 = [[f32; N]; N];

/// The filter's tuning (VPX's `MotionKalmanAxis::Config`).
#[derive(Clone, Debug)]
pub struct KalmanConfig {
    /// Process noise: how readily the true motion changes (high for low latency), and the
    /// biases' slow random walks.
    pub process_jerk_variance: f32,
    pub velocity_bias_process_variance: f32,
    pub acceleration_bias_process_variance: f32,
    /// Measurement noise.
    pub velocity_measurement_variance: f32,
    pub acceleration_measurement_variance: f32,
    /// The rest pseudo-measurements' noise.
    pub zero_position_measurement_variance: f32,
    pub zero_velocity_measurement_variance: f32,
    pub zero_acceleration_measurement_variance: f32,
    /// The initial covariance.
    pub initial_position_variance: f32,
    pub initial_velocity_variance: f32,
    pub initial_acceleration_variance: f32,
    pub initial_velocity_bias_variance: f32,
    pub initial_acceleration_bias_variance: f32,
    /// How fast the biases are drawn back to zero, in seconds.
    pub bias_mean_reversion_time_s: f32,
    /// The time step's bounds.
    pub min_dt: f32,
    pub max_dt: f32,
    /// Past states kept for applying late readings.
    pub history_capacity: usize,
}

impl Default for KalmanConfig {
    fn default() -> Self {
        KalmanConfig {
            process_jerk_variance: 800.0,
            velocity_bias_process_variance: 0.0001,
            acceleration_bias_process_variance: 0.0001,
            velocity_measurement_variance: 0.05 * 0.05,
            acceleration_measurement_variance: 0.40 * 0.40,
            zero_position_measurement_variance: 0.0005 * 0.0005,
            zero_velocity_measurement_variance: 0.01 * 0.01,
            zero_acceleration_measurement_variance: 0.10 * 0.10,
            initial_position_variance: 1.0e-4,
            initial_velocity_variance: 1.0,
            initial_acceleration_variance: 25.0,
            initial_velocity_bias_variance: 0.01,
            initial_acceleration_bias_variance: 0.01,
            bias_mean_reversion_time_s: 5.0,
            min_dt: 1.0e-6,
            max_dt: 0.001,
            history_capacity: 64,
        }
    }
}

/// VPX's upper bound on the history's length.
const MAX_HISTORY: usize = 256;

#[derive(Clone, Copy, Debug)]
struct HistoryEntry {
    time_ns: u64,
    state: V5,
    p: M5,
}

const EMPTY_ENTRY: HistoryEntry = HistoryEntry { time_ns: 0, state: [0.0; N], p: [[0.0; N]; N] };

/// One axis's filter (VPX's `MotionKalmanAxis`).
#[derive(Clone, Debug)]
pub struct MotionKalman {
    config: KalmanConfig,
    initialized: bool,
    time_ns: u64,
    state: V5,
    p: M5,
    history: Vec<HistoryEntry>,
    history_head: usize,
    history_count: usize,
}

fn identity() -> M5 {
    let mut m = [[0.0; N]; N];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

fn transposed(m: &M5) -> M5 {
    let mut t = [[0.0; N]; N];
    for i in 0..N {
        for j in 0..N {
            t[i][j] = m[j][i];
        }
    }
    t
}

fn add(a: &M5, b: &M5) -> M5 {
    let mut r = [[0.0; N]; N];
    for i in 0..N {
        for j in 0..N {
            r[i][j] = a[i][j] + b[i][j];
        }
    }
    r
}

fn mul(a: &M5, b: &M5) -> M5 {
    let mut r = [[0.0; N]; N];
    for i in 0..N {
        for j in 0..N {
            let mut sum = 0.0f32;
            for k in 0..N {
                sum += a[i][k] * b[k][j];
            }
            r[i][j] = sum;
        }
    }
    r
}

fn mul_v(a: &M5, v: &V5) -> V5 {
    let mut r = [0.0; N];
    for i in 0..N {
        let mut sum = 0.0f32;
        for j in 0..N {
            sum += a[i][j] * v[j];
        }
        r[i] = sum;
    }
    r
}

fn dot(a: &V5, b: &V5) -> f32 {
    let mut sum = 0.0f32;
    for i in 0..N {
        sum += a[i] * b[i];
    }
    sum
}

fn outer(a: &V5, b: &V5, scale: f32) -> M5 {
    let mut r = [[0.0; N]; N];
    for i in 0..N {
        for j in 0..N {
            r[i][j] = scale * a[i] * b[j];
        }
    }
    r
}

fn symmetrize(m: &mut M5) {
    for i in 0..N {
        for j in (i + 1)..N {
            let value = 0.5 * (m[i][j] + m[j][i]);
            m[i][j] = value;
            m[j][i] = value;
        }
    }
}

/// A measurement's row: ones where it reads the state.
fn reads(indices: &[usize]) -> V5 {
    let mut h = [0.0; N];
    for &i in indices {
        h[i] = 1.0;
    }
    h
}

impl MotionKalman {
    pub fn new(config: KalmanConfig) -> MotionKalman {
        MotionKalman {
            config,
            initialized: false,
            time_ns: 0,
            state: [0.0; N],
            p: [[0.0; N]; N],
            history: vec![EMPTY_ENTRY; MAX_HISTORY],
            history_head: 0,
            history_count: 0,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn time_ns(&self) -> u64 {
        self.time_ns
    }

    pub fn reset(&mut self, time_ns: u64, position: f32, velocity: f32, acceleration: f32, velocity_bias: f32, acceleration_bias: f32) {
        self.initialized = true;
        self.time_ns = time_ns;
        self.state = [position, velocity, acceleration, velocity_bias, acceleration_bias];
        self.p = [[0.0; N]; N];
        self.p[POSITION][POSITION] = self.config.initial_position_variance;
        self.p[VELOCITY][VELOCITY] = self.config.initial_velocity_variance;
        self.p[ACCELERATION][ACCELERATION] = self.config.initial_acceleration_variance;
        self.p[VELOCITY_BIAS][VELOCITY_BIAS] = self.config.initial_velocity_bias_variance;
        self.p[ACCELERATION_BIAS][ACCELERATION_BIAS] = self.config.initial_acceleration_bias_variance;
        self.history_head = 0;
        self.history_count = 0;
        self.save_snapshot();
    }

    pub fn position(&self) -> f32 {
        self.state[POSITION]
    }

    pub fn velocity(&self) -> f32 {
        self.state[VELOCITY]
    }

    pub fn acceleration(&self) -> f32 {
        self.state[ACCELERATION]
    }

    pub fn velocity_bias(&self) -> f32 {
        self.state[VELOCITY_BIAS]
    }

    pub fn acceleration_bias(&self) -> f32 {
        self.state[ACCELERATION_BIAS]
    }

    /// The filter carried forward to `time_ns`.
    pub fn predict_to(&mut self, time_ns: u64) {
        if !self.initialized || time_ns <= self.time_ns {
            return;
        }
        self.save_snapshot();
        self.predict_internal(time_ns);
    }

    fn predict_internal(&mut self, time_ns: u64) {
        let delta_ns = time_ns - self.time_ns;
        let mut remaining = (delta_ns as f64 * 1.0e-9) as f32;
        while remaining > 0.0 {
            let dt = remaining.min(self.config.max_dt);
            self.predict_step(dt);
            remaining -= dt;
        }
        self.time_ns = time_ns;
    }

    /// A measurement applied at its own time: the filter wound back to the last state before
    /// it, the measurement applied, and carried forward again to the present.
    fn rewind_and_update(&mut self, time_ns: u64, apply: impl FnOnce(&mut MotionKalman)) {
        if time_ns >= self.time_ns {
            if time_ns > self.time_ns {
                self.predict_to(time_ns);
            }
            apply(self);
            return;
        }
        let present_ns = self.time_ns;
        if !self.restore_snapshot(time_ns) {
            // no rewind possible: applied to the present state as a best effort
            apply(self);
        } else {
            if self.time_ns < time_ns {
                self.predict_internal(time_ns);
            }
            apply(self);
        }
        if self.time_ns < present_ns {
            self.save_snapshot();
            self.predict_internal(present_ns);
        }
    }

    /// A velocity reading: the velocity and its bias, plus noise.
    pub fn update_velocity(&mut self, time_ns: u64, velocity: f32) {
        if !self.initialized {
            // at rest when it starts: the first reading is all bias
            self.reset(time_ns, 0.0, 0.0, 0.0, velocity, 0.0);
            self.p[VELOCITY_BIAS][VELOCITY_BIAS] = self.config.velocity_measurement_variance;
            self.update_latest_snapshot();
            return;
        }
        let (h, r) = (reads(&[VELOCITY, VELOCITY_BIAS]), self.config.velocity_measurement_variance);
        self.rewind_and_update(time_ns, |k| k.update_scalar(&h, velocity, r));
    }

    /// An acceleration reading: the acceleration and its bias, plus noise.
    pub fn update_acceleration(&mut self, time_ns: u64, acceleration: f32) {
        if !self.initialized {
            self.reset(time_ns, 0.0, 0.0, 0.0, 0.0, acceleration);
            self.p[ACCELERATION_BIAS][ACCELERATION_BIAS] = self.config.acceleration_measurement_variance;
            self.update_latest_snapshot();
            return;
        }
        let (h, r) = (reads(&[ACCELERATION, ACCELERATION_BIAS]), self.config.acceleration_measurement_variance);
        self.rewind_and_update(time_ns, |k| k.update_scalar(&h, acceleration, r));
    }

    fn update_zero(&mut self, time_ns: u64, state: usize, variance: f32) {
        if !self.initialized {
            return;
        }
        let h = reads(&[state]);
        self.rewind_and_update(time_ns, |k| k.update_scalar(&h, 0.0, variance));
    }

    /// The cabinet at rest: its velocity, acceleration and position all zero.
    pub fn update_rest_constraints(&mut self, time_ns: u64) {
        if !self.initialized {
            return;
        }
        self.update_zero(time_ns, VELOCITY, self.config.zero_velocity_measurement_variance);
        self.update_zero(time_ns, ACCELERATION, self.config.zero_acceleration_measurement_variance);
        self.update_zero(time_ns, POSITION, self.config.zero_position_measurement_variance);
    }

    fn transition(&self, dt: f32) -> M5 {
        let dt2 = dt * dt;
        let mut f = identity();
        f[POSITION][VELOCITY] = dt;
        f[POSITION][ACCELERATION] = 0.5 * dt2;
        f[VELOCITY][ACCELERATION] = dt;
        // the biases drawn back toward zero (an Ornstein-Uhlenbeck process)
        let tau = self.config.bias_mean_reversion_time_s;
        let alpha = if tau > 0.0 { (-dt / tau).exp() } else { 1.0 };
        f[VELOCITY_BIAS][VELOCITY_BIAS] = alpha;
        f[ACCELERATION_BIAS][ACCELERATION_BIAS] = alpha;
        f
    }

    fn process_noise(&self, dt: f32) -> M5 {
        let (qj, qbv, qba) = (self.config.process_jerk_variance, self.config.velocity_bias_process_variance, self.config.acceleration_bias_process_variance);
        let dt2 = dt * dt;
        let dt3 = dt2 * dt;
        let dt4 = dt3 * dt;
        let dt5 = dt4 * dt;
        let mut q = [[0.0; N]; N];
        q[POSITION][POSITION] = qj * (dt5 / 20.0);
        q[POSITION][VELOCITY] = qj * (dt4 / 8.0);
        q[POSITION][ACCELERATION] = qj * (dt3 / 6.0);
        q[VELOCITY][POSITION] = qj * (dt4 / 8.0);
        q[VELOCITY][VELOCITY] = qj * (dt3 / 3.0);
        q[VELOCITY][ACCELERATION] = qj * (dt2 / 2.0);
        q[ACCELERATION][POSITION] = qj * (dt3 / 6.0);
        q[ACCELERATION][VELOCITY] = qj * (dt2 / 2.0);
        q[ACCELERATION][ACCELERATION] = qj * dt;
        q[VELOCITY_BIAS][VELOCITY_BIAS] = qbv * dt;
        q[ACCELERATION_BIAS][ACCELERATION_BIAS] = qba * dt;
        q
    }

    fn predict_step(&mut self, dt: f32) {
        let dt = dt.max(self.config.min_dt);
        let f = self.transition(dt);
        let ft = transposed(&f);
        let q = self.process_noise(dt);
        self.state = mul_v(&f, &self.state);
        self.p = add(&mul(&mul(&f, &self.p), &ft), &q);
        symmetrize(&mut self.p);
    }

    fn update_scalar(&mut self, h: &V5, measurement: f32, variance: f32) {
        let predicted = dot(h, &self.state);
        let innovation = measurement - predicted;
        let mut pht = [0.0f32; N];
        for (i, out) in pht.iter_mut().enumerate() {
            let mut sum = 0.0f32;
            for j in 0..N {
                sum += self.p[i][j] * h[j];
            }
            *out = sum;
        }
        let s = dot(h, &pht) + variance;
        if s <= 0.0 {
            return;
        }
        let inv_s = 1.0 / s;
        let k: V5 = pht.map(|x| x * inv_s);
        for i in 0..N {
            self.state[i] += k[i] * innovation;
        }
        // Joseph's stabilized covariance: (I - K H) P (I - K H)ᵀ + K R Kᵀ
        let mut a = identity();
        for i in 0..N {
            for j in 0..N {
                a[i][j] -= k[i] * h[j];
            }
        }
        let at = transposed(&a);
        let apat = mul(&mul(&a, &self.p), &at);
        let krkt = outer(&k, &k, variance);
        self.p = add(&apat, &krkt);
        symmetrize(&mut self.p);
    }

    fn capacity(&self) -> usize {
        self.config.history_capacity.clamp(1, MAX_HISTORY)
    }

    fn save_snapshot(&mut self) {
        let cap = self.capacity();
        self.history[self.history_head] = HistoryEntry { time_ns: self.time_ns, state: self.state, p: self.p };
        self.history_head = (self.history_head + 1) % cap;
        if self.history_count < cap {
            self.history_count += 1;
        }
    }

    fn update_latest_snapshot(&mut self) {
        if self.history_count > 0 {
            let cap = self.capacity();
            let last = (self.history_head + cap - 1) % cap;
            self.history[last].state = self.state;
            self.history[last].p = self.p;
        }
    }

    /// The latest saved state at or before `target_ns` restored, the history after it dropped.
    fn restore_snapshot(&mut self, target_ns: u64) -> bool {
        let cap = self.capacity() as i64;
        if self.history_count == 0 {
            return false;
        }
        let (head, count) = (self.history_head as i64, self.history_count as i64);
        let slot_of = |k: i64| (((k % cap) + cap) % cap) as usize;
        // newest first: the first at or before the target is the one
        let mut best: Option<usize> = None;
        for i in 0..count {
            let slot = slot_of(head - 1 - i);
            if self.history[slot].time_ns <= target_ns {
                best = Some(slot);
                break;
            }
        }
        // all newer than the target: the oldest
        let best = best.unwrap_or_else(|| slot_of(head - count));
        let entry = self.history[best];
        (self.state, self.p, self.time_ns) = (entry.state, entry.p, entry.time_ns);
        // the history kept up to and including that state
        let mut keep = 0;
        for i in 0..count {
            keep = i + 1;
            if slot_of(head - count + i) == best {
                break;
            }
        }
        self.history_count = keep as usize;
        self.history_head = (best + 1) % cap as usize;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A steady acceleration reading after rest is followed by the filter's acceleration, the
    /// first reading taken as the sensor's bias, as VPX's filter starts.
    #[test]
    fn readings_are_followed_and_the_first_is_bias() {
        let mut k = MotionKalman::new(KalmanConfig::default());
        k.update_acceleration(0, 0.2);
        assert!(k.is_initialized());
        assert_eq!(k.acceleration_bias(), 0.2);
        // 50 ms of 2 m/s² readings above that bias, one each millisecond
        for ms in 1..=50u64 {
            k.predict_to(ms * 1_000_000);
            k.update_acceleration(ms * 1_000_000, 2.2);
        }
        assert!((k.acceleration() - 2.0).abs() < 0.3, "acceleration {}", k.acceleration());
        assert!(k.velocity() > 0.05, "velocity {}", k.velocity());
        // rest brings the velocity and acceleration back down
        for ms in 51..=400u64 {
            k.predict_to(ms * 1_000_000);
            k.update_acceleration(ms * 1_000_000, 0.2);
            k.update_rest_constraints(ms * 1_000_000);
        }
        assert!(k.acceleration().abs() < 0.05 && k.velocity().abs() < 0.05, "{} {}", k.acceleration(), k.velocity());
    }

    /// A late reading is applied at its own time — the filter wound back to it, then carried
    /// forward to the present again (VPX's rewind keeps no later readings to replay).
    #[test]
    fn a_late_reading_rewinds() {
        let run = |late: Option<f32>| {
            let mut k = MotionKalman::new(KalmanConfig::default());
            k.update_velocity(0, 0.0);
            for ms in 1..=10u64 {
                k.predict_to(ms * 1_000_000);
                k.update_velocity(ms * 1_000_000, 0.0);
            }
            if let Some(v) = late {
                k.update_velocity(5_000_000, v);
            }
            (k.velocity(), k.time_ns())
        };
        let (still, late) = (run(None), run(Some(1.0)));
        assert_eq!(late.1, 10_000_000, "back at the present");
        assert!(late.0 > still.0 + 0.01, "{still:?} {late:?}");
    }
}
