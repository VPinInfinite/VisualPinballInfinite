// Ported from Visual Pinball X: src/math/matrix.h, class Matrix3 (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436). Only what the physics uses.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's 3x3 matrix, used for the ball's orientation and to rotate into a
//! collider's frame.

use super::vector::{Vertex3Ds, cross_product};
use std::ops::{Add, Mul};

/// VPX's `Matrix3`, stored as VPX's `m_d[row][column]` (`_11` is `m[0][0]`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Matrix3 {
    pub m: [[f32; 3]; 3],
}

impl Matrix3 {
    /// VPX's element-wise constructor, `_11` to `_33` row by row.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(m11: f32, m12: f32, m13: f32, m21: f32, m22: f32, m23: f32, m31: f32, m32: f32, m33: f32) -> Self {
        Self { m: [[m11, m12, m13], [m21, m22, m23], [m31, m32, m33]] }
    }

    /// VPX's `SetIdentity(value)`: `value` on the diagonal, zero elsewhere.
    pub fn set_identity(&mut self, value: f32) {
        self.m = [[value, 0.0, 0.0], [0.0, value, 0.0], [0.0, 0.0, value]];
    }

    /// VPX's `SkewSymmetric`: the matrix that computes `v x p` for a vector `p`.
    pub fn skew_symmetric(&mut self, pv3d: &Vertex3Ds) {
        self.m[0][0] = 0.0;
        self.m[0][1] = -pv3d.z;
        self.m[0][2] = pv3d.y;
        self.m[1][0] = pv3d.z;
        self.m[1][1] = 0.0;
        self.m[1][2] = -pv3d.x;
        self.m[2][0] = -pv3d.y;
        self.m[2][1] = pv3d.x;
        self.m[2][2] = 0.0;
    }

    /// VPX's `MulMatricesAndMulScalar`: `(pmat1 * pmat2) * scalar`, each
    /// element summed left to right, then scaled.
    pub fn mul_matrices_and_mul_scalar(pmat1: &Matrix3, pmat2: &Matrix3, scalar: f32) -> Matrix3 {
        let mut m = Matrix3::default();
        for i in 0..3 {
            for l in 0..3 {
                m.m[i][l] =
                    (pmat1.m[i][0] * pmat2.m[0][l] + pmat1.m[i][1] * pmat2.m[1][l] + pmat1.m[i][2] * pmat2.m[2][l]) * scalar;
            }
        }
        m
    }

    /// VPX's `OrthoNormalize`: rebuilds the columns from the first two.
    pub fn ortho_normalize(&mut self) {
        let mut vx = Vertex3Ds::new(self.m[0][0], self.m[1][0], self.m[2][0]);
        let vy = Vertex3Ds::new(self.m[0][1], self.m[1][1], self.m[2][1]);
        let mut vz = cross_product(&vx, &vy);
        vx.normalize();
        vz.normalize();
        let vy = cross_product(&vz, &vx);
        // VPX doesn't normalize vY: it is already unit length.

        self.m[0][0] = vx.x;
        self.m[0][1] = vy.x;
        self.m[0][2] = vz.x;
        self.m[1][0] = vx.y;
        self.m[1][1] = vy.y;
        self.m[1][2] = vz.y;
        self.m[2][0] = vx.z;
        self.m[2][1] = vy.z;
        self.m[2][2] = vz.z;
    }

    /// VPX's `RotationAroundAxis(axis, angle)`.
    pub fn rotation_around_axis(&mut self, axis: &Vertex3Ds, angle: f32) {
        let rsin = angle.sin();
        let rcos = angle.cos();
        self.rotation_around_axis_sin_cos(axis, rsin, rcos);
    }

    /// VPX's `RotationAroundAxis(axis, rsin, rcos)`.
    pub fn rotation_around_axis_sin_cos(&mut self, axis: &Vertex3Ds, rsin: f32, rcos: f32) {
        self.m[0][0] = axis.x * axis.x + rcos * (1.0 - axis.x * axis.x);
        self.m[1][0] = axis.x * axis.y * (1.0 - rcos) - axis.z * rsin;
        self.m[2][0] = axis.z * axis.x * (1.0 - rcos) + axis.y * rsin;

        self.m[0][1] = axis.x * axis.y * (1.0 - rcos) + axis.z * rsin;
        self.m[1][1] = axis.y * axis.y + rcos * (1.0 - axis.y * axis.y);
        self.m[2][1] = axis.y * axis.z * (1.0 - rcos) - axis.x * rsin;

        self.m[0][2] = axis.z * axis.x * (1.0 - rcos) - axis.y * rsin;
        self.m[1][2] = axis.y * axis.z * (1.0 - rcos) + axis.x * rsin;
        self.m[2][2] = axis.z * axis.z + rcos * (1.0 - axis.z * axis.z);
    }

    /// VPX's `MulVectorT`: multiplies by the transpose.
    #[inline(always)]
    pub fn mul_vector_t(&self, v: &Vertex3Ds) -> Vertex3Ds {
        Vertex3Ds::new(
            self.m[0][0] * v.x + self.m[1][0] * v.y + self.m[2][0] * v.z,
            self.m[0][1] * v.x + self.m[1][1] * v.y + self.m[2][1] * v.z,
            self.m[0][2] * v.x + self.m[1][2] * v.y + self.m[2][2] * v.z,
        )
    }
}

/// VPX's `operator+`: element by element.
impl Add for Matrix3 {
    type Output = Matrix3;
    fn add(self, o: Matrix3) -> Matrix3 {
        let mut r = Matrix3::default();
        for i in 0..3 {
            for j in 0..3 {
                r.m[i][j] = self.m[i][j] + o.m[i][j];
            }
        }
        r
    }
}

/// VPX's matrix-times-vector `operator*`.
impl Mul<Vertex3Ds> for Matrix3 {
    type Output = Vertex3Ds;
    #[inline(always)]
    fn mul(self, v: Vertex3Ds) -> Vertex3Ds {
        Vertex3Ds::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_times_vector() {
        let mut m = Matrix3::default();
        m.set_identity(1.0);
        let v = Vertex3Ds::new(1.0, -2.0, 3.0);
        assert_eq!(m * v, v);
        assert_eq!(m.mul_vector_t(&v), v);
    }

    #[test]
    fn skew_symmetric_is_cross_product() {
        let a = Vertex3Ds::new(0.5, -1.0, 2.0);
        let b = Vertex3Ds::new(3.0, 0.25, -1.5);
        let mut m = Matrix3::default();
        m.skew_symmetric(&a);
        assert_eq!(m * b, cross_product(&a, &b));
    }
}
