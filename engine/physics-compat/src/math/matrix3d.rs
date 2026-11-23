// Ported from Visual Pinball X: src/math/matrix.h, class Matrix3D (license:GPLv3+), and
// GetRotatedAxis from src/math/vector.h, commit af26b2d (10.8.1 pre-release build 5436).
// Only what building colliders uses.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's 4x4 matrix, which places rubbers', primitives' and targets' meshes
//! on the table. Vectors are rows: `v * M`, translation in the last row.

use std::ops::Mul;

use super::vector::Vertex3Ds;

/// VPX's `Matrix3D` (Direct3D's layout: `_11` is `m[0][0]`, `_41` the x
/// translation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix3D {
    pub m: [[f32; 4]; 4],
}

impl Matrix3D {
    pub const IDENTITY: Self =
        Self { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] };

    /// VPX's `MatrixRotateX` (angle in radians).
    pub fn rotate_x(ang_rad: f32) -> Self {
        let mut r = Self::IDENTITY;
        let c = ang_rad.cos();
        r.m[1][1] = c;
        r.m[2][2] = c;
        r.m[1][2] = ang_rad.sin();
        r.m[2][1] = -r.m[1][2];
        r
    }

    /// VPX's `MatrixRotateY`.
    pub fn rotate_y(ang_rad: f32) -> Self {
        let mut r = Self::IDENTITY;
        let c = ang_rad.cos();
        r.m[0][0] = c;
        r.m[2][2] = c;
        r.m[2][0] = ang_rad.sin();
        r.m[0][2] = -r.m[2][0];
        r
    }

    /// VPX's `MatrixRotateZ`.
    pub fn rotate_z(ang_rad: f32) -> Self {
        let mut r = Self::IDENTITY;
        let c = ang_rad.cos();
        r.m[0][0] = c;
        r.m[1][1] = c;
        r.m[0][1] = ang_rad.sin();
        r.m[1][0] = -r.m[0][1];
        r
    }

    /// VPX's `MatrixScale(sx, sy, sz)`.
    pub fn scale(sx: f32, sy: f32, sz: f32) -> Self {
        let mut r = Self::IDENTITY;
        r.m[0][0] = sx;
        r.m[1][1] = sy;
        r.m[2][2] = sz;
        r
    }

    /// VPX's `MatrixTranslate`.
    pub fn translate(x: f32, y: f32, z: f32) -> Self {
        let mut r = Self::IDENTITY;
        r.m[3] = [x, y, z, 1.0];
        r
    }

    /// VPX's `operator*` for a point: through the matrix, then divided by w.
    pub fn transform(&self, v: &Vertex3Ds) -> Vertex3Ds {
        let m = &self.m;
        let xp = m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z + m[3][0];
        let yp = m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z + m[3][1];
        let zp = m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z + m[3][2];
        let wp = m[0][3] * v.x + m[1][3] * v.y + m[2][3] * v.z + m[3][3];

        let inv_wp = 1.0 / wp;
        Vertex3Ds::new(xp * inv_wp, yp * inv_wp, zp * inv_wp)
    }

    /// VPX's `MultiplyVectorNoTranslate`.
    pub fn multiply_vector_no_translate(&self, v: &Vertex3Ds) -> Vertex3Ds {
        let m = &self.m;
        Vertex3Ds::new(
            m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z,
            m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z,
            m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z,
        )
    }

    /// VPX's `MultiplyVectorNoPerspective`.
    pub fn multiply_vector_no_perspective(&self, v: &Vertex3Ds) -> Vertex3Ds {
        let m = &self.m;
        Vertex3Ds::new(
            m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z + m[3][0],
            m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z + m[3][1],
            m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z + m[3][2],
        )
    }
}

/// VPX's `operator*`: `self` first, then `rhs`. VPX's SSE code sums each
/// element's four products in order, as written here.
impl Mul for Matrix3D {
    type Output = Matrix3D;
    fn mul(self, rhs: Matrix3D) -> Matrix3D {
        let mut t = Matrix3D { m: [[0.0; 4]; 4] };
        for i in 0..4 {
            for l in 0..4 {
                let mut r = rhs.m[0][l] * self.m[i][0];
                for j in 1..4 {
                    #[allow(clippy::assign_op_pattern)] // VPX's operand order: the product, then the sum so far
                    {
                        r = rhs.m[j][l] * self.m[i][j] + r;
                    }
                }
                t.m[i][l] = r;
            }
        }
        t
    }
}

/// VPX's `GetRotatedAxis`: `temp` turned by `angle` degrees around `axis`.
pub fn get_rotated_axis(angle: f32, axis: &Vertex3Ds, temp: &Vertex3Ds) -> Vertex3Ds {
    let mut u = *axis;
    u.normalize();

    let sin_angle = ((std::f64::consts::PI / 180.0) as f32 * angle).sin();
    let cos_angle = ((std::f64::consts::PI / 180.0) as f32 * angle).cos();
    let one_minus_cos_angle = 1.0 - cos_angle;

    let row0 = Vertex3Ds::new(
        u.x * u.x + cos_angle * (1.0 - u.x * u.x),
        u.x * u.y * one_minus_cos_angle - sin_angle * u.z,
        u.x * u.z * one_minus_cos_angle + sin_angle * u.y,
    );
    let row1 = Vertex3Ds::new(
        u.x * u.y * one_minus_cos_angle + sin_angle * u.z,
        u.y * u.y + cos_angle * (1.0 - u.y * u.y),
        u.y * u.z * one_minus_cos_angle - sin_angle * u.x,
    );
    let row2 = Vertex3Ds::new(
        u.x * u.z * one_minus_cos_angle - sin_angle * u.y,
        u.y * u.z * one_minus_cos_angle + sin_angle * u.x,
        u.z * u.z + cos_angle * (1.0 - u.z * u.z),
    );

    Vertex3Ds::new(temp.dot(&row0), temp.dot(&row1), temp.dot(&row2))
}
