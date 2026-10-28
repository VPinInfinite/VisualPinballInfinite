//! The 4x4 matrices the renderer uses, in VPX's convention: vectors are rows
//! (`v * M`), so `A * B` applies `A` first; translation sits in the last row.

use std::ops::Mul;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub fn min(self, o: Self) -> Self {
        Self::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }
    pub fn max(self, o: Self) -> Self {
        Self::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Self = Self { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] };

    pub fn translate(x: f32, y: f32, z: f32) -> Self {
        let mut r = Self::IDENTITY;
        r.m[3] = [x, y, z, 1.0];
        r
    }

    pub fn scale(x: f32, y: f32, z: f32) -> Self {
        let mut r = Self::IDENTITY;
        r.m[0][0] = x;
        r.m[1][1] = y;
        r.m[2][2] = z;
        r
    }

    /// VPX's `GetRotationPart`: the turning, scaling and skew alone, without
    /// the move.
    pub fn rotation_part(&self) -> Self {
        let m = &self.m;
        Self { m: [[m[0][0], m[0][1], m[0][2], 0.0], [m[1][0], m[1][1], m[1][2], 0.0], [m[2][0], m[2][1], m[2][2], 0.0], [0.0, 0.0, 0.0, 1.0]] }
    }

    /// VPX's `GetOrthoNormalPos`: where the matrix moves the origin.
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.m[3][0], self.m[3][1], self.m[3][2])
    }

    /// VPX's `MatrixPlaneReflection`: points mirrored through the plane
    /// `n . p + d = 0`, `n` of unit length.
    pub fn plane_reflection(n: Vec3, d: f32) -> Self {
        Self {
            m: [
                [1.0 - 2.0 * n.x * n.x, -2.0 * n.x * n.y, -2.0 * n.x * n.z, 0.0],
                [-2.0 * n.x * n.y, 1.0 - 2.0 * n.y * n.y, -2.0 * n.y * n.z, 0.0],
                [-2.0 * n.x * n.z, -2.0 * n.y * n.z, 1.0 - 2.0 * n.z * n.z, 0.0],
                [-2.0 * d * n.x, -2.0 * d * n.y, -2.0 * d * n.z, 1.0],
            ],
        }
    }

    /// VPX's `MatrixRotateX`.
    pub fn rotate_x(a: f32) -> Self {
        let mut r = Self::IDENTITY;
        let (s, c) = a.sin_cos();
        r.m[1][1] = c;
        r.m[2][2] = c;
        r.m[1][2] = s;
        r.m[2][1] = -s;
        r
    }

    /// VPX's `MatrixRotateY`.
    pub fn rotate_y(a: f32) -> Self {
        let mut r = Self::IDENTITY;
        let (s, c) = a.sin_cos();
        r.m[0][0] = c;
        r.m[2][2] = c;
        r.m[2][0] = s;
        r.m[0][2] = -s;
        r
    }

    /// VPX's `MatrixRotateZ`.
    pub fn rotate_z(a: f32) -> Self {
        let mut r = Self::IDENTITY;
        let (s, c) = a.sin_cos();
        r.m[0][0] = c;
        r.m[1][1] = c;
        r.m[0][1] = s;
        r.m[1][0] = -s;
        r
    }

    /// Rotation by a unit quaternion (x, y, z, w), in this row-vector convention.
    pub fn from_quaternion(q: [f32; 4]) -> Self {
        let [x, y, z, w] = q;
        let mut r = Self::IDENTITY;
        r.m[0] = [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w), 0.0];
        r.m[1] = [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w), 0.0];
        r.m[2] = [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y), 0.0];
        r
    }

    /// VPX's `SetPerspectiveOffCenterLH`: depth from 0 (near) to 1 (far).
    pub fn perspective_off_center_lh(left: f32, right: f32, bottom: f32, top: f32, znear: f32, zfar: f32) -> Self {
        let r_l = right - left;
        let t_b = top - bottom;
        let mut r = Self { m: [[0.0; 4]; 4] };
        r.m[0][0] = 2.0 * znear / r_l;
        r.m[1][1] = 2.0 * znear / t_b;
        r.m[2][0] = -(right + left) / r_l;
        r.m[2][1] = -(top + bottom) / t_b;
        r.m[2][2] = -zfar / (znear - zfar);
        r.m[2][3] = 1.0;
        r.m[3][2] = znear * -r.m[2][2];
        r
    }

    /// A point through the matrix, divided by w.
    pub fn transform(&self, v: Vec3) -> Vec3 {
        let m = &self.m;
        let x = m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z + m[3][0];
        let y = m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z + m[3][1];
        let z = m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z + m[3][2];
        let w = m[0][3] * v.x + m[1][3] * v.y + m[2][3] * v.z + m[3][3];
        Vec3::new(x / w, y / w, z / w)
    }

    /// A point through the matrix, ignoring the perspective column.
    pub fn transform_no_perspective(&self, v: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z + m[3][0],
            m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z + m[3][1],
            m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z + m[3][2],
        )
    }

    /// The inverse, by Gauss-Jordan elimination with partial pivoting.
    #[allow(clippy::needless_range_loop)] // row operations read clearest with indexes
    pub fn inverse(&self) -> Self {
        let mut a = [[0.0f64; 8]; 4];
        for (i, row) in a.iter_mut().enumerate() {
            for j in 0..4 {
                row[j] = f64::from(self.m[i][j]);
            }
            row[4 + i] = 1.0;
        }
        for col in 0..4 {
            let pivot = (col..4).max_by(|&p, &q| a[p][col].abs().total_cmp(&a[q][col].abs())).unwrap_or(col);
            a.swap(col, pivot);
            let d = a[col][col];
            if d == 0.0 {
                return Self::IDENTITY;
            }
            for v in &mut a[col] {
                *v /= d;
            }
            for r in 0..4 {
                if r != col {
                    let f = a[r][col];
                    if f != 0.0 {
                        for c in 0..8 {
                            a[r][c] -= f * a[col][c];
                        }
                    }
                }
            }
        }
        let mut out = Self::IDENTITY;
        for i in 0..4 {
            for j in 0..4 {
                out.m[i][j] = a[i][4 + j] as f32;
            }
        }
        out
    }
}

impl Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, rhs: Mat4) -> Mat4 {
        let mut t = Mat4 { m: [[0.0; 4]; 4] };
        for i in 0..4 {
            for l in 0..4 {
                t.m[i][l] = (0..4).map(|j| self.m[i][j] * rhs.m[j][l]).sum();
            }
        }
        t
    }
}
