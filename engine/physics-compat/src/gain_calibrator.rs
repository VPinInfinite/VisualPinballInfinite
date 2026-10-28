// Ported from Visual Pinball X build 5436 (10.8.1 pre-release, commit af26b2d):
// src/physics/cabinet/MotionGainCalibratorAxis.h, and the helpers clamp, lerp, saturate and
// infNaN from src/core/def.h (license:GPLv3+). MotionGainCalibratorAxis.h carries no marker,
// but was added by VPX's team in 2026 with no third-party content (its three commits, by
// vbousquet and toxieainc, 2026-05-17 to 2026-08-05), so VPX's licence makes it GPL too
//.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX 5436's gain calibration between a cabinet sensor's two channels on
//! one axis: from each nudge (a segment between two rests, as the cabinet
//! sensor detects them), the factor that turns the acceleration channel's
//! readings, integrated, into the velocity channel's. Each segment's shape is
//! fitted (both signals detrended between the segment's ends, then a weighted
//! least-squares line through the origin), poor or weak segments set aside,
//! and the good ones accumulated with slow forgetting, with a confidence that
//! grows with them. Operations in VPX's order and precision (single, with
//! sums in double), so the results are its results.

/// The calibration's tuning (VPX's `MotionGainCalibratorAxis::Config`).
#[derive(Clone, Debug)]
pub struct GainConfig {
    /// The first guess: scaled acceleration = gain × raw acceleration.
    pub initial_gain: f32,
    /// A segment's accepted length and fewest samples.
    pub min_segment_duration_ns: u64,
    pub max_segment_duration_ns: u64,
    pub min_sample_count: usize,
    /// The least movement a segment must show, m/s and (integrated) m/s.
    pub min_velocity_peak_to_peak: f32,
    pub min_integrated_accel_peak_to_peak: f32,
    pub min_regression_denominator: f32,
    /// How much of the past each accepted segment keeps (1: all of it).
    pub forgetting_factor: f32,
    /// The gain's range.
    pub min_gain: f32,
    pub max_gain: f32,
    /// The segment's first and last samples left out of the fit.
    pub exclude_segment_endpoints_from_fit: bool,
    /// Samples weighted by their distance from the segment's ends.
    pub use_triangular_weights: bool,
    pub epsilon: f32,
}

impl Default for GainConfig {
    fn default() -> Self {
        GainConfig {
            initial_gain: 1.0,
            min_segment_duration_ns: 30_000_000,
            max_segment_duration_ns: 2_000_000_000,
            min_sample_count: 8,
            min_velocity_peak_to_peak: 0.002,
            min_integrated_accel_peak_to_peak: 0.02,
            min_regression_denominator: 1.0e-6,
            forgetting_factor: 0.995,
            min_gain: 0.001,
            max_gain: 1000.0,
            exclude_segment_endpoints_from_fit: true,
            use_triangular_weights: true,
            epsilon: 1.0e-9,
        }
    }
}

/// One segment's evaluation (VPX's `SegmentResult`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SegmentResult {
    pub accepted: bool,
    pub duration_ns: u64,
    pub sample_count: usize,
    /// The best gain for this segment alone, and how well it fits (0 to 1).
    pub segment_gain: f32,
    pub segment_quality: f32,
    pub segment_residual_rms: f32,
    pub velocity_peak_to_peak: f32,
    pub integrated_accel_peak_to_peak: f32,
    /// The regression's sums, Σ w·x·y and Σ w·x².
    pub segment_numerator: f32,
    pub segment_denominator: f32,
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    time_ns: u64,
    velocity: f32,
    acceleration: f32,
}

/// VPX's `MotionGainCalibratorAxis`.
#[derive(Clone, Debug)]
pub struct GainCalibrator {
    config: GainConfig,
    segment_active: bool,
    samples: Vec<Sample>,
    gain: f32,
    /// The accumulated regression sums.
    accumulated_numerator: f64,
    accumulated_denominator: f64,
    started_segments: usize,
    accepted_segments: usize,
    rejected_segments: usize,
    total_accepted_duration_ns: u64,
    last_result: SegmentResult,
    global_confidence: f32,
}

impl GainCalibrator {
    pub fn new(config: GainConfig) -> GainCalibrator {
        let gain = config.initial_gain;
        GainCalibrator {
            config,
            segment_active: false,
            samples: Vec::new(),
            gain,
            accumulated_numerator: 0.0,
            accumulated_denominator: 0.0,
            started_segments: 0,
            accepted_segments: 0,
            rejected_segments: 0,
            total_accepted_duration_ns: 0,
            last_result: SegmentResult { segment_gain: 1.0, ..SegmentResult::default() },
            global_confidence: 0.0,
        }
    }

    pub fn is_segment_active(&self) -> bool {
        self.segment_active
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn global_confidence(&self) -> f32 {
        self.global_confidence
    }

    pub fn accepted_segments(&self) -> usize {
        self.accepted_segments
    }

    pub fn rejected_segments(&self) -> usize {
        self.rejected_segments
    }

    pub fn last_result(&self) -> &SegmentResult {
        &self.last_result
    }

    /// A new segment, as the cabinet leaves its rest; one already going is dropped.
    pub fn start_segment(&mut self, _time_ns: u64) {
        self.segment_active = true;
        self.samples.clear();
        self.started_segments += 1;
    }

    /// A pair of raw readings of the same moment; refused outside a segment or out of order.
    pub fn add_sample(&mut self, time_ns: u64, raw_velocity: f32, raw_acceleration: f32) -> bool {
        if !self.segment_active {
            return false;
        }
        if self.samples.last().is_some_and(|s| time_ns <= s.time_ns) {
            return false;
        }
        self.samples.push(Sample { time_ns, velocity: raw_velocity, acceleration: raw_acceleration });
        true
    }

    /// The segment ends, as the cabinet comes to rest again; whether it was accepted into the
    /// gain.
    pub fn end_segment(&mut self) -> bool {
        if !self.segment_active {
            self.last_result = SegmentResult { segment_gain: 1.0, ..SegmentResult::default() };
            return false;
        }
        self.segment_active = false;
        self.last_result = self.evaluate_current_segment();
        if !self.last_result.accepted {
            self.rejected_segments += 1;
            self.samples.clear();
            return false;
        }
        // recursive accumulation, the past slowly forgotten
        let lambda = f64::from(saturate(self.config.forgetting_factor));
        self.accumulated_numerator = lambda * self.accumulated_numerator + f64::from(self.last_result.segment_numerator);
        self.accumulated_denominator = lambda * self.accumulated_denominator + f64::from(self.last_result.segment_denominator);
        if self.accumulated_denominator > f64::from(self.config.epsilon) {
            let gain = self.accumulated_numerator / self.accumulated_denominator;
            self.gain = clamp(gain as f32, self.config.min_gain, self.config.max_gain);
        }
        self.accepted_segments += 1;
        self.total_accepted_duration_ns += self.last_result.duration_ns;
        self.global_confidence = self.compute_global_confidence();
        self.samples.clear();
        true
    }

    /// From the accepted segments' count and length and the regression's strength.
    fn compute_global_confidence(&self) -> f32 {
        if self.accepted_segments == 0 {
            return 0.0;
        }
        let seg_factor = 1.0 - (-0.25 * self.accepted_segments as f32).exp();
        let duration_s = self.total_accepted_duration_ns as f32 * 1.0e-9;
        let duration_factor = 1.0 - (-duration_s * 0.5).exp();
        let denom_factor = 1.0 - (-(self.accumulated_denominator as f32) * 0.25).exp();
        saturate(0.40 * seg_factor + 0.35 * duration_factor + 0.25 * denom_factor)
    }

    fn evaluate_current_segment(&self) -> SegmentResult {
        let c = &self.config;
        let mut result = SegmentResult { segment_gain: 1.0, ..SegmentResult::default() };
        let n = self.samples.len();
        result.sample_count = n;
        if n < c.min_sample_count {
            return result;
        }
        let start_ns = self.samples[0].time_ns;
        let end_ns = self.samples[n - 1].time_ns;
        let duration_ns = end_ns - start_ns;
        result.duration_ns = duration_ns;
        if duration_ns < c.min_segment_duration_ns || duration_ns > c.max_segment_duration_ns {
            return result;
        }
        // The segment starts and ends at rest, so its net change in speed is near nothing: its
        // inner shape is fitted, both signals less the line between their ends (the velocity
        // channel's offset and drift, the integrated acceleration's bias).
        let total_duration_s = duration_ns as f64 * 1.0e-9;
        if total_duration_s <= 0.0 {
            return result;
        }
        let t_norm: Vec<f32> = self.samples.iter().map(|s| ((s.time_ns - start_ns) as f64 / duration_ns as f64) as f32).collect();
        let vel: Vec<f32> = self.samples.iter().map(|s| s.velocity).collect();
        // the acceleration integrated by trapezoids
        let mut integ_accel = vec![0.0f32; n];
        for i in 1..n {
            let dt = (self.samples[i].time_ns - self.samples[i - 1].time_ns) as f32 * 1.0e-9;
            let (a0, a1) = (self.samples[i - 1].acceleration, self.samples[i].acceleration);
            integ_accel[i] = integ_accel[i - 1] + 0.5 * (a0 + a1) * dt;
        }
        let (vel0, vel1) = (vel[0], vel[n - 1]);
        let vel_detrended: Vec<f32> = (0..n).map(|i| vel[i] - lerp(vel0, vel1, t_norm[i])).collect();
        let (int0, int1) = (integ_accel[0], integ_accel[n - 1]);
        let integ_accel_detrended: Vec<f32> = (0..n).map(|i| integ_accel[i] - lerp(int0, int1, t_norm[i])).collect();
        result.velocity_peak_to_peak = peak_to_peak(&vel_detrended);
        result.integrated_accel_peak_to_peak = peak_to_peak(&integ_accel_detrended);
        if result.velocity_peak_to_peak < c.min_velocity_peak_to_peak || result.integrated_accel_peak_to_peak < c.min_integrated_accel_peak_to_peak {
            return result;
        }
        // weighted least squares through the origin, velocity ≈ k × integrated acceleration:
        // k = Σ w·x·y / Σ w·x²
        let (mut numerator, mut denominator) = (0.0f64, 0.0f64);
        let trim = c.exclude_segment_endpoints_from_fit && n >= 3;
        let (fit_begin, fit_end) = if trim { (1, n - 1) } else { (0, n) };
        for i in fit_begin..fit_end {
            let (x, y, w) = (integ_accel_detrended[i], vel_detrended[i], self.sample_weight(t_norm[i]));
            numerator += f64::from(w) * f64::from(x) * f64::from(y);
            denominator += f64::from(w) * f64::from(x) * f64::from(x);
        }
        result.segment_numerator = numerator as f32;
        result.segment_denominator = denominator as f32;
        if denominator < f64::from(c.min_regression_denominator) {
            return result;
        }
        let segment_gain = numerator / denominator;
        result.segment_gain = segment_gain as f32;
        if inf_nan(result.segment_gain) {
            return result;
        }
        // the fit's residual, for the segment's quality
        let (mut weighted_residual2, mut weighted_count) = (0.0f64, 0.0f64);
        for i in fit_begin..fit_end {
            let (x, y, w) = (integ_accel_detrended[i], vel_detrended[i], self.sample_weight(t_norm[i]));
            let r = f64::from(y) - segment_gain * f64::from(x);
            weighted_residual2 += f64::from(w) * r * r;
            weighted_count += f64::from(w);
        }
        if weighted_count <= f64::from(c.epsilon) {
            return result;
        }
        result.segment_residual_rms = (weighted_residual2 / weighted_count).sqrt() as f32;
        // small residuals against the signal's size: a good fit
        let signal_scale = std_max(result.velocity_peak_to_peak, c.epsilon);
        let normalized_residual = result.segment_residual_rms / signal_scale;
        result.segment_quality = 1.0 - saturate(normalized_residual * 2.0);
        if inf_nan(result.segment_quality) {
            return result;
        }
        // absurd gains, and very poor fits, set aside
        if result.segment_gain < c.min_gain || result.segment_gain > c.max_gain || result.segment_quality <= 0.05 {
            return result;
        }
        result.accepted = true;
        result
    }

    /// A triangular weight: 0 at the segment's ends, 1 in its middle, where readings near the
    /// rests are less sure.
    fn sample_weight(&self, t_norm: f32) -> f32 {
        if !self.config.use_triangular_weights {
            return 1.0;
        }
        let d = (2.0 * t_norm - 1.0).abs();
        std_max(0.0, 1.0 - d)
    }
}

fn peak_to_peak(data: &[f32]) -> f32 {
    let Some(&first) = data.first() else { return 0.0 };
    let (mut mn, mut mx) = (first, first);
    for &v in data {
        mn = if v < mn { v } else { mn }; // std::min
        mx = if mx < v { v } else { mx }; // std::max
    }
    mx - mn
}

/// `std::max(a, b)`: `b` when `a < b`, else `a`.
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// VPX's `min`, `max` (as SSE's `minss` and `maxss`), `clamp`, `saturate`, `lerp` and `infNaN`.
fn min(x: f32, y: f32) -> f32 {
    if x < y { x } else { y }
}
fn max(x: f32, y: f32) -> f32 {
    if x > y { x } else { y }
}
fn clamp(x: f32, mn: f32, mx: f32) -> f32 {
    max(min(x, mx), mn)
}
fn saturate(x: f32) -> f32 {
    max(min(x, 1.0), 0.0)
}
fn lerp(x1: f32, x2: f32, alpha: f32) -> f32 {
    (1.0 - alpha) * x1 + alpha * x2
}
fn inf_nan(a: f32) -> bool {
    a.to_bits() & 0x7F80_0000 == 0x7F80_0000
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A nudge's velocity and its acceleration, read every 8 ms over `ms`: a sine's half
    /// period out and back, the acceleration channel `ratio` times weaker than the velocity's.
    fn segment(c: &mut GainCalibrator, start_ms: u64, ms: u64, ratio: f32) {
        c.start_segment(start_ms * 1_000_000);
        let omega = std::f32::consts::PI * 2.0 / (ms as f32 * 1.0e-3);
        for k in 0..=ms / 8 {
            let t = (k * 8) as f32 * 1.0e-3;
            let v = 0.05 * (omega * t).sin();
            let a = 0.05 * omega * (omega * t).cos() / ratio;
            assert!(c.add_sample((start_ms + k * 8) * 1_000_000, v, a));
        }
    }

    #[test]
    fn the_gain_between_the_channels_is_found() {
        let mut c = GainCalibrator::new(GainConfig::default());
        segment(&mut c, 0, 400, 4.0);
        assert!(c.end_segment(), "{:?}", c.last_result());
        assert!((c.gain() - 4.0).abs() < 0.2, "gain {}", c.gain());
        let first = c.global_confidence();
        segment(&mut c, 1000, 400, 4.0);
        assert!(c.end_segment());
        assert!(c.global_confidence() > first, "more segments, more confidence");
        assert_eq!((c.accepted_segments(), c.rejected_segments()), (2, 0));
    }

    #[test]
    fn short_or_still_segments_are_set_aside() {
        let mut c = GainCalibrator::new(GainConfig::default());
        // too few samples
        segment(&mut c, 0, 40, 2.0);
        assert!(!c.end_segment());
        // no movement at all
        c.start_segment(0);
        for k in 0..20u64 {
            c.add_sample(k * 8_000_000, 0.0, 0.0);
        }
        assert!(!c.end_segment());
        // readings out of order are refused
        c.start_segment(0);
        assert!(c.add_sample(10, 0.0, 0.0) && !c.add_sample(10, 0.0, 0.0));
        assert_eq!((c.gain(), c.rejected_segments()), (1.0, 2));
    }
}
