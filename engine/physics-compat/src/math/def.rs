// Ported from Visual Pinball X: src/core/def.h and src/core/def.cpp (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436). Only the helpers the physics uses.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's small numeric helpers and its random number generator.

/// VPX's `min`: `x < y ? x : y`. Not [`f32::min`], which returns the other
/// operand when one is NaN.
#[inline(always)]
pub fn min(x: f32, y: f32) -> f32 {
    if x < y { x } else { y }
}

/// VPX's `max`: `x < y ? y : x`. Not [`f32::max`].
#[inline(always)]
pub fn max(x: f32, y: f32) -> f32 {
    if x < y { y } else { x }
}

/// VPX's `clamp(x, mn, mx)`: `max(min(x, mx), mn)`.
#[inline(always)]
pub fn clamp(x: f32, mn: f32, mx: f32) -> f32 {
    max(min(x, mx), mn)
}

/// VPX's `saturate(x)`: `max(min(x, 1), 0)`.
#[inline(always)]
pub fn saturate(x: f32) -> f32 {
    max(min(x, 1.0), 0.0)
}

/// VPX's `lerp(x1, x2, alpha)`: `(1 - alpha) * x1 + alpha * x2`.
#[inline(always)]
pub fn lerp(x1: f32, x2: f32, alpha: f32) -> f32 {
    (1.0 - alpha) * x1 + alpha * x2
}

/// C++'s `std::max(a, b)`: `(a < b) ? b : a`. VPX uses it in some places and its
/// own `max` in others; they differ for NaN and signed zeros, so port each call
/// with the one VPX wrote.
#[inline(always)]
pub fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// C++'s `std::min(a, b)`: `(b < a) ? b : a`.
#[inline(always)]
pub fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// VPX's `infNaN(a)`: true for infinities and NaN (exponent bits all set).
#[inline(always)]
pub fn inf_nan(a: f32) -> bool {
    a.to_bits() & 0x7F80_0000 == 0x7F80_0000
}

/// VPX's `sqrf(x)`: `x*x`.
#[inline(always)]
pub fn sqrf(x: f32) -> f32 {
    x * x
}

/// VPX's `sign(a)`: true when the sign bit is set, so also for -0.0.
#[inline(always)]
pub fn sign(a: f32) -> bool {
    a.to_bits() & 0x8000_0000 != 0
}

/// `(float)(M_PI/180.0)`, the factor in VPX's `ANGTORAD`: computed in double,
/// then rounded to float, as the C++ compiler does.
pub const ANGTORAD: f32 = (std::f64::consts::PI / 180.0) as f32;

/// `(float)(180.0/M_PI)`, the factor in VPX's `RADTOANG`.
pub const RADTOANG: f32 = (180.0 / std::f64::consts::PI) as f32;

/// VPX's `ANGTORAD(x)` for a float argument: degrees to radians.
#[inline(always)]
pub fn ang_to_rad(x: f32) -> f32 {
    x * ANGTORAD
}

/// VPX's `RADTOANG(x)` for a float argument: radians to degrees.
#[inline(always)]
pub fn rad_to_ang(x: f32) -> f32 {
    x * RADTOANG
}

/// VPX's `map_u32_to_unifloat`: spreads a 32-bit number over [0, 1) without
/// ever rounding up to 1.
#[inline(always)]
pub fn map_u32_to_unifloat(u: u32) -> f32 {
    (u & !(u >> 24)) as f32 * f32::from_bits(0x2F80_0000) // 0x1p-32, exactly
}

/// VPX's random number generator: MWC64X (`mwc64x` in `def.h`), the one its
/// build enables. VPX keeps one global state, starting from a fixed seed
/// (`def.cpp`) and never reseeded, so its sequence is the same every run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mwc64x {
    pub state: u64,
}

impl Mwc64x {
    /// `mwc64x_state`'s initial value in VPX's `def.cpp`.
    pub const VPX_SEED: u64 = 4_077_358_422_479_273_989;

    /// A generator in the state VPX starts from.
    pub const fn new() -> Self {
        Self { state: Self::VPX_SEED }
    }

    /// VPX's `mwc64x(s)`: advances the state, returns 32 random bits.
    #[inline(always)]
    pub fn next_u32(&mut self) -> u32 {
        const M: u64 = 4_294_883_355;
        let c = (self.state >> 32) as u32;
        let x = self.state as u32;
        // x * M + c stays below 2^64, as M + 1 <= 2^32.
        self.state = u64::from(x) * M + u64::from(c);
        x ^ c
    }

    /// VPX's `rand_mt_01()`: uniform in [0, 1).
    #[inline(always)]
    pub fn rand_01(&mut self) -> f32 {
        map_u32_to_unifloat(self.next_u32())
    }

    /// VPX's `rand_mt_m11()`: uniform in [-1, 1).
    #[inline(always)]
    pub fn rand_m11(&mut self) -> f32 {
        ((self.next_u32() as i32) >> (32 - 25)) as f32 * f32::from_bits(0x3380_0000) // 0x1p-24, exactly
    }
}

impl Default for Mwc64x {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_max_follow_vpx_on_nan() {
        // VPX: min = x < y ? x : y, so a NaN comparison picks y; max picks x.
        assert!(min(f32::NAN, 1.0) == 1.0);
        assert!(min(1.0, f32::NAN).is_nan());
        assert!(max(f32::NAN, 1.0).is_nan());
        assert!(max(1.0, f32::NAN) == 1.0);
    }

    #[test]
    fn sign_sees_negative_zero() {
        assert!(sign(-0.0));
        assert!(!sign(0.0));
        assert!(sign(-1.0));
    }

    #[test]
    fn unifloat_never_reaches_one() {
        assert!(map_u32_to_unifloat(u32::MAX) < 1.0);
        assert_eq!(map_u32_to_unifloat(0), 0.0);
    }

    #[test]
    fn rng_ranges() {
        let mut rng = Mwc64x::new();
        for _ in 0..100_000 {
            let a = rng.rand_01();
            assert!((0.0..1.0).contains(&a));
            let b = rng.rand_m11();
            assert!((-1.0..1.0).contains(&b));
        }
    }

    #[test]
    fn angle_factors_match_cpp() {
        // (float)(M_PI/180.0) and (float)(180.0/M_PI)
        assert_eq!(ANGTORAD.to_bits(), 0x3C8E_FA35);
        assert_eq!(RADTOANG.to_bits(), 0x4265_2EE1);
    }
}
