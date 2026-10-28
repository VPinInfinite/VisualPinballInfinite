// Ported from Visual Pinball X: src/math/math.cpp (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436). Only what the physics uses.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's equation solvers.

/// VPX's `SolveQuadraticEq`: the roots of `a*x^2 + b*x + c`, or `None` when
/// the discriminant is negative. Same operation order as VPX: `b*b - 4*a*c`,
/// and both roots scaled by `-0.5/a`.
#[inline]
pub fn solve_quadratic_eq(a: f32, b: f32, c: f32) -> Option<(f32, f32)> {
    let mut discr = b * b - 4.0 * a * c;

    if discr < 0.0 {
        return None;
    }

    discr = discr.sqrt();

    let inv_a = (-0.5) / a;
    let sol1 = (b + discr) * inv_a;
    let sol2 = (b - discr) * inv_a;

    Some((sol1, sol2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_of_simple_quadratic() {
        // x^2 - 3x + 2 = 0 -> 1 and 2
        let (s1, s2) = solve_quadratic_eq(1.0, -3.0, 2.0).unwrap();
        assert_eq!((s1, s2), (1.0, 2.0));
        assert!(solve_quadratic_eq(1.0, 0.0, 1.0).is_none());
    }
}
