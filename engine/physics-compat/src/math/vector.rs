// Ported from Visual Pinball X: src/math/vector.h (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436). Only what the physics uses.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's 2D and 3D vectors.
//!
//! Every operation is written exactly as in VPX, including its habit of
//! dividing by multiplying with a reciprocal.

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// VPX's `Vertex2D`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Vertex2D {
    pub x: f32,
    pub y: f32,
}

impl Vertex2D {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn set(&mut self, a: f32, b: f32) {
        self.x = a;
        self.y = b;
    }

    pub fn set_zero(&mut self) {
        self.set(0.0, 0.0);
    }

    #[inline(always)]
    pub fn dot(&self, pv: &Vertex2D) -> f32 {
        self.x * pv.x + self.y * pv.y
    }

    #[inline(always)]
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    #[inline(always)]
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// `oneoverlength = 1 / Length()`, then scale.
    #[inline(always)]
    pub fn normalize(&mut self) {
        let oneoverlength = 1.0 / self.length();
        self.x *= oneoverlength;
        self.y *= oneoverlength;
    }

    /// Leaves vectors with a squared length at or below `FLT_MIN` alone.
    #[inline(always)]
    pub fn normalize_safe(&mut self) {
        let lengthsqr = self.x * self.x + self.y * self.y;
        if lengthsqr <= f32::MIN_POSITIVE {
            return;
        }
        let oneoverlength = 1.0 / lengthsqr.sqrt();
        self.x *= oneoverlength;
        self.y *= oneoverlength;
    }

    #[inline(always)]
    pub fn is_zero(&self) -> bool {
        self.x.abs() <= f32::MIN_POSITIVE && self.y.abs() <= f32::MIN_POSITIVE
    }
}

impl Add for Vertex2D {
    type Output = Vertex2D;
    #[inline(always)]
    fn add(self, v: Vertex2D) -> Vertex2D {
        Vertex2D::new(self.x + v.x, self.y + v.y)
    }
}

impl Sub for Vertex2D {
    type Output = Vertex2D;
    #[inline(always)]
    fn sub(self, v: Vertex2D) -> Vertex2D {
        Vertex2D::new(self.x - v.x, self.y - v.y)
    }
}

impl Neg for Vertex2D {
    type Output = Vertex2D;
    #[inline(always)]
    fn neg(self) -> Vertex2D {
        Vertex2D::new(-self.x, -self.y)
    }
}

impl AddAssign for Vertex2D {
    #[inline(always)]
    fn add_assign(&mut self, v: Vertex2D) {
        self.x += v.x;
        self.y += v.y;
    }
}

impl SubAssign for Vertex2D {
    #[inline(always)]
    fn sub_assign(&mut self, v: Vertex2D) {
        self.x -= v.x;
        self.y -= v.y;
    }
}

/// `v * s`, computed as VPX does: `{s*x, s*y}`.
impl Mul<f32> for Vertex2D {
    type Output = Vertex2D;
    #[inline(always)]
    fn mul(self, s: f32) -> Vertex2D {
        Vertex2D::new(s * self.x, s * self.y)
    }
}

impl Mul<Vertex2D> for f32 {
    type Output = Vertex2D;
    #[inline(always)]
    fn mul(self, v: Vertex2D) -> Vertex2D {
        Vertex2D::new(self * v.x, self * v.y)
    }
}

/// `v / s` multiplies by `1/s`, as VPX does.
impl Div<f32> for Vertex2D {
    type Output = Vertex2D;
    #[inline(always)]
    fn div(self, s: f32) -> Vertex2D {
        let invs = 1.0 / s;
        Vertex2D::new(self.x * invs, self.y * invs)
    }
}

impl MulAssign<f32> for Vertex2D {
    #[inline(always)]
    fn mul_assign(&mut self, s: f32) {
        self.x *= s;
        self.y *= s;
    }
}

impl DivAssign<f32> for Vertex2D {
    #[inline(always)]
    fn div_assign(&mut self, s: f32) {
        let invs = 1.0 / s;
        self.x *= invs;
        self.y *= invs;
    }
}

/// VPX's `Vertex3Ds`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Vertex3Ds {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vertex3Ds {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn set(&mut self, a: f32, b: f32, c: f32) {
        self.x = a;
        self.y = b;
        self.z = c;
    }

    pub fn set_zero(&mut self) {
        self.set(0.0, 0.0, 0.0);
    }

    /// `oneoverlength = 1 / sqrtf(x*x + y*y + z*z)`, then scale.
    #[inline(always)]
    pub fn normalize(&mut self) {
        let oneoverlength = 1.0 / (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        self.x *= oneoverlength;
        self.y *= oneoverlength;
        self.z *= oneoverlength;
    }

    /// VPX's `Normalize(scalar)`: scales to length `scalar`.
    #[inline(always)]
    pub fn normalize_to(&mut self, scalar: f32) {
        let oneoverlength = scalar / (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        self.x *= oneoverlength;
        self.y *= oneoverlength;
        self.z *= oneoverlength;
    }

    /// Leaves vectors with a squared length at or below `FLT_MIN` alone.
    #[inline(always)]
    pub fn normalize_safe(&mut self) {
        let lengthsqr = self.x * self.x + self.y * self.y + self.z * self.z;
        if lengthsqr <= f32::MIN_POSITIVE {
            return;
        }
        let oneoverlength = 1.0 / lengthsqr.sqrt();
        self.x *= oneoverlength;
        self.y *= oneoverlength;
        self.z *= oneoverlength;
    }

    #[inline(always)]
    pub fn dot(&self, pv: &Vertex3Ds) -> f32 {
        self.x * pv.x + self.y * pv.y + self.z * pv.z
    }

    #[inline(always)]
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    #[inline(always)]
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    #[inline(always)]
    pub fn is_zero(&self) -> bool {
        self.x.abs() <= f32::MIN_POSITIVE && self.y.abs() <= f32::MIN_POSITIVE && self.z.abs() <= f32::MIN_POSITIVE
    }

    /// VPX's `xy()`: the x and y components as a 2D vector.
    #[inline(always)]
    pub fn xy(&self) -> Vertex2D {
        Vertex2D::new(self.x, self.y)
    }

    /// Writes through VPX's `xy()` reference: replaces x and y, keeps z.
    #[inline(always)]
    pub fn set_xy(&mut self, v: Vertex2D) {
        self.x = v.x;
        self.y = v.y;
    }
}

impl Add for Vertex3Ds {
    type Output = Vertex3Ds;
    #[inline(always)]
    fn add(self, v: Vertex3Ds) -> Vertex3Ds {
        Vertex3Ds::new(self.x + v.x, self.y + v.y, self.z + v.z)
    }
}

impl Sub for Vertex3Ds {
    type Output = Vertex3Ds;
    #[inline(always)]
    fn sub(self, v: Vertex3Ds) -> Vertex3Ds {
        Vertex3Ds::new(self.x - v.x, self.y - v.y, self.z - v.z)
    }
}

impl Neg for Vertex3Ds {
    type Output = Vertex3Ds;
    #[inline(always)]
    fn neg(self) -> Vertex3Ds {
        Vertex3Ds::new(-self.x, -self.y, -self.z)
    }
}

impl AddAssign for Vertex3Ds {
    #[inline(always)]
    fn add_assign(&mut self, v: Vertex3Ds) {
        self.x += v.x;
        self.y += v.y;
        self.z += v.z;
    }
}

impl SubAssign for Vertex3Ds {
    #[inline(always)]
    fn sub_assign(&mut self, v: Vertex3Ds) {
        self.x -= v.x;
        self.y -= v.y;
        self.z -= v.z;
    }
}

/// `v * s`, computed as VPX does: `{s*x, s*y, s*z}`.
impl Mul<f32> for Vertex3Ds {
    type Output = Vertex3Ds;
    #[inline(always)]
    fn mul(self, s: f32) -> Vertex3Ds {
        Vertex3Ds::new(s * self.x, s * self.y, s * self.z)
    }
}

impl Mul<Vertex3Ds> for f32 {
    type Output = Vertex3Ds;
    #[inline(always)]
    fn mul(self, v: Vertex3Ds) -> Vertex3Ds {
        Vertex3Ds::new(self * v.x, self * v.y, self * v.z)
    }
}

/// `v / s` multiplies by `1/s`, as VPX does.
impl Div<f32> for Vertex3Ds {
    type Output = Vertex3Ds;
    #[inline(always)]
    fn div(self, s: f32) -> Vertex3Ds {
        let invs = 1.0 / s;
        Vertex3Ds::new(self.x * invs, self.y * invs, self.z * invs)
    }
}

impl MulAssign<f32> for Vertex3Ds {
    #[inline(always)]
    fn mul_assign(&mut self, s: f32) {
        self.x *= s;
        self.y *= s;
        self.z *= s;
    }
}

impl DivAssign<f32> for Vertex3Ds {
    #[inline(always)]
    fn div_assign(&mut self, s: f32) {
        let invs = 1.0 / s;
        self.x *= invs;
        self.y *= invs;
        self.z *= invs;
    }
}

/// VPX's `CrossProduct`.
#[inline(always)]
pub fn cross_product(pv1: &Vertex3Ds, pv2: &Vertex3Ds) -> Vertex3Ds {
    Vertex3Ds::new(
        pv1.y * pv2.z - pv1.z * pv2.y,
        pv1.z * pv2.x - pv1.x * pv2.z,
        pv1.x * pv2.y - pv1.y * pv2.x,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn division_uses_reciprocal() {
        // 1/3 then multiply differs from a true division for some values; check we match the reciprocal form.
        let v = Vertex3Ds::new(1.0, 2.0, 7.0) / 3.0;
        let invs = 1.0f32 / 3.0;
        assert_eq!(v, Vertex3Ds::new(1.0 * invs, 2.0 * invs, 7.0 * invs));
    }

    #[test]
    fn normalize_safe_ignores_tiny() {
        let mut v = Vertex3Ds::new(1e-30, 0.0, 0.0);
        v.normalize_safe();
        assert_eq!(v.x, 1e-30);
    }

    #[test]
    fn cross_of_axes() {
        let x = Vertex3Ds::new(1.0, 0.0, 0.0);
        let y = Vertex3Ds::new(0.0, 1.0, 0.0);
        assert_eq!(cross_product(&x, &y), Vertex3Ds::new(0.0, 0.0, 1.0));
    }
}
