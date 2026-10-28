// Ported from Visual Pinball X: src/math/bbox.h (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436). Only what the physics uses.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's bounding boxes, used by the collision search.

use super::def::{max, min};
use super::vector::{Vertex2D, Vertex3Ds};

/// VPX's `FRect3D`: an axis-aligned box. Field order as in VPX.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct FRect3D {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    pub zlow: f32,
    pub zhigh: f32,
}

impl FRect3D {
    /// VPX's constructor order: `(x1, x2, y1, y2, z1, z2)`.
    pub const fn new(x1: f32, x2: f32, y1: f32, y2: f32, z1: f32, z2: f32) -> Self {
        Self { left: x1, right: x2, top: y1, bottom: y2, zlow: z1, zhigh: z2 }
    }

    /// An inverted box that any `extend` replaces.
    pub fn clear(&mut self) {
        self.left = f32::MAX;
        self.right = -f32::MAX;
        self.top = f32::MAX;
        self.bottom = -f32::MAX;
        self.zlow = f32::MAX;
        self.zhigh = -f32::MAX;
    }

    pub fn extend(&mut self, other: &FRect3D) {
        self.left = min(self.left, other.left);
        self.right = max(self.right, other.right);
        self.top = min(self.top, other.top);
        self.bottom = max(self.bottom, other.bottom);
        self.zlow = min(self.zlow, other.zlow);
        self.zhigh = max(self.zhigh, other.zhigh);
    }
}

/// VPX's `FRect`: a 2D box. Note VPX's field order differs from `FRect3D`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct FRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl FRect {
    /// VPX's constructor order: `(x1, x2, y1, y2)`.
    pub const fn new(x1: f32, x2: f32, y1: f32, y2: f32) -> Self {
        Self { left: x1, top: y1, right: x2, bottom: y2 }
    }

    pub fn center(&self) -> Vertex2D {
        Vertex2D::new(0.5 * (self.left + self.right), 0.5 * (self.top + self.bottom))
    }

    pub fn clear(&mut self) {
        self.left = f32::MAX;
        self.right = -f32::MAX;
        self.top = f32::MAX;
        self.bottom = -f32::MAX;
    }

    pub fn extend(&mut self, other: &FRect) {
        self.left = min(self.left, other.left);
        self.right = max(self.right, other.right);
        self.top = min(self.top, other.top);
        self.bottom = max(self.bottom, other.bottom);
    }

    pub fn extend_3d(&mut self, other: &FRect3D) {
        self.left = min(self.left, other.left);
        self.right = max(self.right, other.right);
        self.top = min(self.top, other.top);
        self.bottom = max(self.bottom, other.bottom);
    }
}

/// VPX's box-against-box `fRectIntersect3D`.
///
/// VPX's x64 build uses an SSE version (`ENABLE_SSE_OPTIMIZATIONS`) that
/// compares the four x/y edges at once and requires all four to pass
/// (`mask == 15`), then the z range. That is the same test as this one,
/// NaN included: every comparison involving NaN is false in both.
#[inline(always)]
pub fn f_rect_intersect_3d(rc1: &FRect3D, rc2: &FRect3D) -> bool {
    rc1.right >= rc2.left
        && rc1.bottom >= rc2.top
        && rc1.zhigh >= rc2.zlow
        && rc1.left <= rc2.right
        && rc1.top <= rc2.bottom
        && rc1.zlow <= rc2.zhigh
}

/// VPX's sphere-against-box `fRectIntersect3D`: squared distance from the
/// sphere's centre to the box, against the squared radius.
#[inline(always)]
pub fn f_rect_intersect_3d_sphere(sphere_p: &Vertex3Ds, sphere_rsqr: f32, rc: &FRect3D) -> bool {
    let mut ex = max(rc.left - sphere_p.x, 0.0) + max(sphere_p.x - rc.right, 0.0);
    let mut ey = max(rc.top - sphere_p.y, 0.0) + max(sphere_p.y - rc.bottom, 0.0);
    let mut ez = max(rc.zlow - sphere_p.z, 0.0) + max(sphere_p.z - rc.zhigh, 0.0);
    ex *= ex;
    ey *= ey;
    ez *= ez;
    ex + ey + ez <= sphere_rsqr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_boxes_intersect() {
        let a = FRect3D::new(0.0, 1.0, 0.0, 1.0, 0.0, 1.0);
        let b = FRect3D::new(1.0, 2.0, 0.0, 1.0, 0.0, 1.0);
        assert!(f_rect_intersect_3d(&a, &b));
        let c = FRect3D::new(1.5, 2.0, 0.0, 1.0, 0.0, 1.0);
        assert!(!f_rect_intersect_3d(&a, &c));
    }

    #[test]
    fn sphere_near_corner() {
        let rc = FRect3D::new(0.0, 1.0, 0.0, 1.0, 0.0, 1.0);
        assert!(f_rect_intersect_3d_sphere(&Vertex3Ds::new(2.0, 0.5, 0.5), 1.0, &rc));
        assert!(!f_rect_intersect_3d_sphere(&Vertex3Ds::new(2.0, 2.0, 0.5), 1.0, &rc));
    }
}
