// Ported from Visual Pinball X: src/physics/physconst.h (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436), with FIX_PHYSICS and NEW_PHYSICS off.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's physics constants, each with the type it has in C++.
//!
//! The type matters: `PHYS_SKIN`, `PHYS_TOUCH` and the step-time constants are
//! doubles, so wherever VPX mixes them with floats without a cast, C++ does that
//! arithmetic in double. VPX mostly casts (`(float)PHYS_TOUCH`); the port casts
//! exactly where VPX does.
//!
//! VPX units: 50 length units (VPU) = 1 1/16 inch, the ball's diameter; one time
//! unit (VPT) = 10 ms; one mass unit = a standard 80 g ball.

/// Microseconds between physics updates: 1000 Hz physics.
pub const PHYSICS_STEPTIME: u32 = 1000;
/// `PHYSICS_STEPTIME * 1e-6`: the step in seconds (double).
pub const PHYSICS_STEPTIME_S: f64 = PHYSICS_STEPTIME as f64 * 1e-6;

/// One VPT in microseconds (10 ms), a historical time unit.
pub const DEFAULT_STEPTIME: u32 = 10000;
/// One VPT in seconds (double).
pub const DEFAULT_STEPTIME_S: f64 = 0.01;

/// One physics step in VPT (double): 0.1.
pub const PHYS_FACTOR: f64 = PHYSICS_STEPTIME_S / DEFAULT_STEPTIME_S;

pub const DEFAULT_TABLE_GRAVITY: f32 = 0.97;
pub const DEFAULT_TABLE_CONTACTFRICTION: f32 = 0.075;
pub const DEFAULT_TABLE_SCATTERANGLE: f32 = 0.5;
pub const DEFAULT_TABLE_ELASTICITY: f32 = 0.25;
pub const DEFAULT_TABLE_ELASTICITY_FALLOFF: f32 = 0.0;
pub const DEFAULT_TABLE_PFSCATTERANGLE: f32 = 0.0;
pub const DEFAULT_TABLE_MIN_SLOPE: f32 = 6.0;
pub const DEFAULT_TABLE_MAX_SLOPE: f32 = 6.0;

/// Detail level used to approximate ramps and rubbers for collisions.
pub const HIT_SHAPE_DETAIL_LEVEL: f32 = 7.0;

/// Earth gravity in VPU/VPT^2.
pub const GRAVITYCONST: f32 = 1.81751;

/// Near-zero test in linear, well-behaved conditions.
pub const C_PRECISION: f32 = 0.01;
/// Tolerance for line segment endpoints.
pub const C_TOL_ENDPNTS: f32 = 0.0;
/// Tolerance for point radii.
pub const C_TOL_RADIUS: f32 = 0.005;
/// Physical skin (double): contacts inside it report zero time.
pub const PHYS_SKIN: f64 = 25.0;
pub const DEFAULT_BALL_SIZE: f32 = 25.0;
/// Contact layer outside an object (double).
pub const PHYS_TOUCH: f64 = 0.05;
/// Below this normal speed a collision is handled as contact, not impulse.
pub const C_LOWNORMVEL: f32 = 0.0001;
pub const C_CONTACTVEL: f32 = 0.099;

// Workarounds VPX keeps enabled while NEW_PHYSICS is off.
/// Embedded-ball threshold (`C_EMBEDDED` is defined, as 0).
pub const C_EMBEDDED: f32 = 0.0;
/// Kick given to an embedded ball.
pub const C_EMBEDSHOT: f32 = 0.05;
/// Contact displacement correction gain.
pub const C_DISP_GAIN: f32 = 0.9875;
pub const C_DISP_LIMIT: f32 = 5.0;

/// Smallest time/intersection difference allowed in the simulation.
pub const STATICTIME: f32 = 0.02;
pub const STATICCNTS: i32 = 10;

/// Flipper iteration count.
pub const C_INTERATIONS: i32 = 20;

/// Plumb: threshold for zero velocity.
pub const VELOCITY_EPSILON: f32 = 0.05;

/// VPX's `MTOVPU(x)`: metres to VPU, `x * (float)(50. / (0.0254 * 1.0625))`.
#[inline(always)]
pub fn mtovpu(x: f32) -> f32 {
    x * (50.0 / (0.0254_f64 * 1.0625)) as f32
}

/// VPX's `STOVPT(x)`: seconds to VPT, `x * 0.01f`.
#[inline(always)]
pub fn stovpt(x: f32) -> f32 {
    x * 0.01
}

/// VPX's `MS2TOVPUVPT2(x)`: m/s^2 to VPU/VPT^2, `MTOVPU(x) * STOVPT(STOVPT(1.0f))`.
#[inline(always)]
pub fn ms2tovpuvpt2(x: f32) -> f32 {
    mtovpu(x) * stovpt(stovpt(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phys_factor_is_a_tenth() {
        assert_eq!(PHYS_FACTOR as f32, 0.1);
    }
}
