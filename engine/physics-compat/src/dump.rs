//! Colliders as text, in the format of the reference tester's table dump
//! (`tests/vpi/test-vpi-engine.cpp` in the tester, "dump table colliders"):
//! every number a collider holds, floats as their bits in hex. Comparing two
//! dumps line by line checks colliders bit for bit.

use std::fmt::Write;

use crate::collide::{HitObjectBase, LineSeg};
use crate::engine::{Collider, Shape};
use crate::env::PartId;
use crate::math::{Matrix3, Vertex2D, Vertex3Ds};

struct Out(String);

impl Out {
    fn w(&mut self, s: &str) {
        self.0.push_str(s);
    }
    fn f(&mut self, v: f32) {
        let _ = write!(self.0, " {:08x}", v.to_bits());
    }
    fn v2(&mut self, v: &Vertex2D) {
        self.f(v.x);
        self.f(v.y);
    }
    fn v3(&mut self, v: &Vertex3Ds) {
        self.f(v.x);
        self.f(v.y);
        self.f(v.z);
    }
    fn m3(&mut self, m: &Matrix3) {
        for row in &m.m {
            for &x in row {
                self.f(x);
            }
        }
    }
    fn i(&mut self, v: i32) {
        let _ = write!(self.0, " {v}");
    }
    fn seg(&mut self, s: &LineSeg, base: &HitObjectBase) {
        self.v2(&s.v1);
        self.v2(&s.v2);
        self.v2(&s.normal);
        self.f(s.length);
        self.f(base.hit_bbox.zlow);
        self.f(base.hit_bbox.zhigh);
    }
}

/// VPX's `GetType()` number for a collider (its `eObjType`, per class).
pub fn vpx_type(shape: &Shape) -> i32 {
    match shape {
        Shape::Point(_) => 1,
        Shape::LineSeg(_) => 2,
        Shape::Slingshot(_) => 3,
        Shape::LineZ(_) => 4,
        Shape::Circle(_) | Shape::Bumper(_) => 5,
        Shape::Flipper(_) => 6,
        Shape::Plunger(_) => 7,
        Shape::Spinner(_) => 8,
        Shape::Poly3D(_) => 10,
        Shape::Triangle(_) => 11,
        Shape::Plane(_) => 12,
        Shape::Line3D(_) => 13,
        Shape::Gate(_) => 14,
        Shape::TriggerSeg(_) | Shape::TriggerCircle(_) | Shape::Kicker(_) => 20,
    }
}

/// One collider as the tester writes it, after the leading `c`. `index` maps
/// a part to its position in the table's part list, or -1 for the table itself.
/// The collider's box must already be computed (`calc_hit_bbox`).
pub fn collider_line(c: &Collider, index: impl Fn(PartId) -> i32) -> String {
    let b = &c.base;
    let mut o = Out(String::with_capacity(256));
    o.w("c");
    o.i(vpx_type(&c.shape));
    o.i(index(PartId(c.owner.id)));
    o.i(b.obj.map_or(-1, &index));
    o.i(b.obj_type as i32);
    o.i(b.enabled as i32);
    o.i(b.fe as i32);
    o.f(b.threshold);
    o.f(b.elasticity);
    o.f(b.elasticity_falloff);
    o.f(b.friction);
    o.f(b.scatter);
    let bb = &b.hit_bbox;
    for v in [bb.left, bb.right, bb.top, bb.bottom, bb.zlow, bb.zhigh] {
        o.f(v);
    }

    match &c.shape {
        Shape::Slingshot(s) => {
            o.w(" sling");
            o.seg(&s.seg, b);
            o.f(s.force);
        }
        Shape::TriggerSeg(s) => {
            o.w(" trigseg");
            o.seg(&s.seg, b);
        }
        Shape::LineSeg(s) => {
            o.w(" seg");
            o.seg(s, b);
        }
        Shape::Bumper(s) => {
            o.w(" bumper");
            o.v2(&s.circle.center);
            o.f(s.circle.radius);
        }
        Shape::Kicker(s) => {
            o.w(" kicker");
            o.v2(&s.circle.center);
            o.f(s.circle.radius);
        }
        Shape::TriggerCircle(s) => {
            o.w(" trigcircle");
            o.v2(&s.circle.center);
            o.f(s.circle.radius);
        }
        Shape::Circle(s) => {
            o.w(" circle");
            o.v2(&s.center);
            o.f(s.radius);
        }
        Shape::Line3D(l) => {
            o.w(" line3d");
            o.v3(&l.v1);
            o.v3(&l.v2);
            o.v2(&l.linez.xy);
            o.f(l.linez.zlow);
            o.f(l.linez.zhigh);
            o.m3(&l.matrix);
        }
        Shape::LineZ(l) => {
            o.w(" linez");
            o.v2(&l.xy);
            o.f(l.zlow);
            o.f(l.zhigh);
        }
        Shape::Point(p) => {
            o.w(" point");
            o.v3(&p.p);
        }
        Shape::Poly3D(p) => {
            o.w(" poly");
            o.i(p.rgv.len() as i32);
            for v in &p.rgv {
                o.v3(v);
            }
            o.v3(&p.normal);
        }
        Shape::Triangle(t) => {
            o.w(" tri");
            for v in &t.rgv {
                o.v3(v);
            }
            o.v3(&t.normal);
        }
        Shape::Flipper(f) => {
            let m = &f.mover;
            o.w(" flipper");
            o.v2(&m.hitcircle_base.center);
            o.f(m.hitcircle_base.radius);
            o.f(m.endradius);
            o.f(m.flipperradius);
            o.f(m.angle_start);
            o.f(m.angle_end);
            o.f(m.hitcircle_base_hit.hit_bbox.zlow);
            o.f(m.hitcircle_base_hit.hit_bbox.zhigh);
            o.f(m.inertia);
            o.v2(&m.zero_ang_norm);
            o.i(m.enabled as i32);
            o.i(m.direction as i32);
            o.f(m.get_strength());
            o.f(m.get_return_ratio());
        }
        Shape::Plunger(p) => {
            let m = &p.mover;
            o.w(" plunger");
            for v in [m.x, m.x2, m.y, m.frame_start, m.frame_end, m.rest_pos, m.pos, m.scatter_velocity] {
                o.f(v);
            }
            o.f(m.part_hit.hit_bbox.zlow);
        }
        Shape::Gate(g) => {
            let m = &g.mover;
            o.w(" gate");
            o.seg(&g.segs[0], &g.seg_hit[0]);
            o.seg(&g.segs[1], &g.seg_hit[1]);
            for v in [m.angle_min, m.angle_max, m.angle, m.damping, m.gravityfactor] {
                o.f(v);
            }
            o.i(g.two_way as i32);
        }
        Shape::Spinner(s) => {
            let m = &s.mover;
            o.w(" spinner");
            o.seg(&s.segs[0], &s.seg_hit[0]);
            o.seg(&s.segs[1], &s.seg_hit[1]);
            for v in [m.angle_min, m.angle_max, m.angle, m.damping, m.elasticity] {
                o.f(v);
            }
        }
        Shape::Plane(_) => o.w(" unknown"),
    }
    o.0
}
