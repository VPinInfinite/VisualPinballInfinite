//! Bit-for-bit comparison of the ported colliders and ball physics with VPX's own compiled code.
//!
//! The data files were written by `tests/vpi/test-vpi-collide.cpp`, a test we add to VPX's physics
//! tester (build 5436), with flush-to-zero and denormals-are-zero set as VPX's player sets them on
//! the thread that runs physics. Each line holds one call's inputs and VPX's results; this test runs
//! the same call on the port, in the same processor mode.
//!
//! - `vpx-collide-5436-precise.txt`: VPX built with exact float semantics (`/fp:precise`), so its
//!   arithmetic follows its source. The port must match it **bit for bit**: hit times, normals,
//!   distances, flags, the sets of triggers a ball is inside, positions, velocities, spin,
//!   orientation and the random generator's state.
//! - `vpx-collide-5436.txt`: VPX's own build (`/fp:fast`), which reorders float arithmetic
//!   differently at each call site. As the port matches the exact build, comparing the port with
//!   this one measures how far shipped VPX drifts from its own source: the ULPs per value,
//!   and the yes/no decisions that flip on borderline cases. Only flips fail the test (more than
//!   1% of a kind's cases); value drift is reported. It is large in one place: for a vertical 3D
//!   line, the fast build's normalize happens to give exactly 1, so the line's frame isn't rotated,
//!   while the exact build gives one ULP less and rotates it by 0.0003 radians about the origin.
//!   The geometry is the same in both frames, so hits differ only by noise.

use physics_compat::collide::{CollisionEvent, HitCircle, HitLineZ, HitObjectBase, HitPoint, LineSeg, ObjType};
use physics_compat::collideex::{Hit3DPoly, HitLine3D, HitPlane, HitTriangle};
use physics_compat::env::{BallId, PartId, PhysicsEnv};
use physics_compat::hitball::{BallS, HitBall};
use physics_compat::math::{Matrix3, Mwc64x, Vertex2D, Vertex3Ds};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[cfg(target_arch = "x86_64")]
fn set_ftz_daz() -> u32 {
    let mut old: u32 = 0;
    // SAFETY: reads and writes the SSE control register of the current thread only; the inputs
    // are read from a file at run time, so no constant folding is involved.
    unsafe {
        std::arch::asm!("stmxcsr [{}]", in(reg) &mut old, options(nostack));
        let new = old | 0x8040;
        std::arch::asm!("ldmxcsr [{}]", in(reg) &new, options(nostack));
    }
    old
}

#[cfg(target_arch = "x86_64")]
fn restore_mxcsr(old: u32) {
    // SAFETY: restores the value saved by set_ftz_daz on the same thread.
    unsafe { std::arch::asm!("ldmxcsr [{}]", in(reg) &old, options(nostack)) };
}

/// Reads one line's tokens by type.
struct Cursor<'a> {
    t: Vec<&'a str>,
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(line: &'a str) -> Self {
        Self { t: line.split_whitespace().collect(), i: 1 }
    }
    fn tok(&mut self) -> &'a str {
        let s = self.t[self.i];
        self.i += 1;
        s
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.tok(), 16).expect("float bits"))
    }
    fn i(&mut self) -> i32 {
        self.tok().parse().expect("int")
    }
    fn b(&mut self) -> bool {
        self.i() != 0
    }
    fn u64(&mut self) -> u64 {
        u64::from_str_radix(self.tok(), 16).expect("u64")
    }
    fn v2(&mut self) -> Vertex2D {
        Vertex2D::new(self.f(), self.f())
    }
    fn v3(&mut self) -> Vertex3Ds {
        Vertex3Ds::new(self.f(), self.f(), self.f())
    }
    fn m3(&mut self) -> Matrix3 {
        let mut m = Matrix3::default();
        for r in 0..3 {
            for c in 0..3 {
                m.m[r][c] = self.f();
            }
        }
        m
    }
    fn arrow(&mut self) {
        assert_eq!(self.tok(), "->");
    }
    fn part(&mut self) -> Option<PartId> {
        match self.i() {
            0 => None,
            n => Some(PartId(n as u32)),
        }
    }
    fn vol(&mut self) -> Vec<PartId> {
        let n = self.i();
        (0..n).map(|_| PartId(self.i() as u32)).collect()
    }
    fn ball_s(&mut self) -> BallS {
        BallS { pos: self.v3(), vel: self.v3(), radius: self.f(), mass: self.f(), locked_in_kicker: self.b(), vp_vol_objs: None }
    }
}

fn obj_type(n: i32) -> ObjType {
    const ALL: [ObjType; 22] = [
        ObjType::Null,
        ObjType::Point,
        ObjType::LineSeg,
        ObjType::LineSegSlingshot,
        ObjType::Joint,
        ObjType::Circle,
        ObjType::Flipper,
        ObjType::Plunger,
        ObjType::Spinner,
        ObjType::Ball,
        ObjType::Poly3D,
        ObjType::Triangle,
        ObjType::Plane,
        ObjType::Line3D,
        ObjType::Gate,
        ObjType::Textbox,
        ObjType::DispReel,
        ObjType::LightSeq,
        ObjType::Primitive,
        ObjType::HitTarget,
        ObjType::Trigger,
        ObjType::Kicker,
    ];
    ALL[n as usize]
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Every bit must match.
    Exact,
    /// Floats may differ by fast-math noise; decisions may flip on borderline cases.
    Fast,
}

/// Largest share of a kind's cases in which a yes/no decision may differ from the fast build.
const FAST_FLIP_LIMIT: f64 = 0.01;

/// Distance between two floats in units in the last place; +0 and -0 are 0 apart.
fn ulp_distance(a: f32, b: f32) -> u32 {
    let key = |x: f32| -> i64 {
        let b = x.to_bits() as i64;
        if b & 0x8000_0000 != 0 { -(b & 0x7fff_ffff) } else { b }
    };
    (key(a) - key(b)).unsigned_abs().min(u32::MAX as u64) as u32
}

/// Compares the port's values with VPX's.
struct Check<'a> {
    line: &'a str,
    mode: Mode,
    errors: Vec<String>,
    /// Fast mode: a yes/no decision differed, so the rest of the line isn't comparable.
    decision_differs: bool,
    /// Fast mode: the largest differences seen, in ULPs and relative to max(1, |value|).
    max_ulp: u32,
    max_rel: f32,
    worst: String,
}

impl Check<'_> {
    fn f(&mut self, what: &str, port: f32, vpx: f32) {
        if port.to_bits() == vpx.to_bits() {
            return;
        }
        match self.mode {
            Mode::Exact => {
                self.errors.push(format!("{what}: port {:08x} ({port}) vpx {:08x} ({vpx})", port.to_bits(), vpx.to_bits()));
            }
            Mode::Fast => {
                if self.decision_differs {
                    return;
                }
                self.max_ulp = self.max_ulp.max(ulp_distance(port, vpx));
                let rel = (port - vpx).abs() / 1.0f32.max(port.abs()).max(vpx.abs());
                if rel > self.max_rel {
                    self.max_rel = rel;
                    self.worst = format!("{what}: port {port} vpx {vpx}");
                }
            }
        }
    }
    fn v2(&mut self, what: &str, port: Vertex2D, vpx: Vertex2D) {
        self.f(&format!("{what}.x"), port.x, vpx.x);
        self.f(&format!("{what}.y"), port.y, vpx.y);
    }
    fn v3(&mut self, what: &str, port: Vertex3Ds, vpx: Vertex3Ds) {
        self.f(&format!("{what}.x"), port.x, vpx.x);
        self.f(&format!("{what}.y"), port.y, vpx.y);
        self.f(&format!("{what}.z"), port.z, vpx.z);
    }
    fn m3(&mut self, what: &str, port: &Matrix3, vpx: &Matrix3) {
        for r in 0..3 {
            for c in 0..3 {
                self.f(&format!("{what}[{r}][{c}]"), port.m[r][c], vpx.m[r][c]);
            }
        }
    }
    fn eq<T: PartialEq + std::fmt::Debug>(&mut self, what: &str, port: T, vpx: T) {
        if port != vpx {
            match self.mode {
                Mode::Exact => self.errors.push(format!("{what}: port {port:?} vpx {vpx:?}")),
                Mode::Fast => self.decision_differs = true,
            }
        }
    }
    /// A hit test's result: the time, and on a hit the fields hit tests set.
    fn hit(&mut self, c: &mut Cursor, t: f32, coll: &CollisionEvent) {
        let vt = c.f();
        if (t >= 0.0) != (vt >= 0.0) {
            self.eq("hit or miss", t >= 0.0, vt >= 0.0);
        }
        self.f("hittime", t, vt);
        if vt >= 0.0 {
            let normal = c.v3();
            let dist = c.f();
            let contact = c.b();
            self.v3("hitnormal", coll.hitnormal, normal);
            self.f("hitdistance", coll.hitdistance, dist);
            self.eq("is_contact", coll.is_contact, contact);
            if contact {
                let horg = c.f();
                self.f("hit_org_normalvelocity", coll.hit_org_normalvelocity, horg);
            }
            let flag = c.b();
            self.eq("hitflag", coll.hitflag, flag);
        }
    }
}

fn with_vol(mut b: BallS, vol: Vec<PartId>) -> (BallS, Rc<RefCell<Vec<PartId>>>) {
    let set = Rc::new(RefCell::new(vol));
    b.vp_vol_objs = Some(set.clone());
    (b, set)
}

/// Per kind of call.
#[derive(Default)]
struct KindStats {
    cases: usize,
    /// Exact mode: cases with a difference, and the first few of them.
    bad: usize,
    examples: Vec<String>,
    /// Fast mode: cases with a flipped decision, and the largest value drift.
    flipped: usize,
    max_ulp: u32,
    max_rel: f32,
    worst: String,
}

#[test]
#[cfg(target_arch = "x86_64")]
fn ported_colliders_and_ball_match_exact_vpx_bit_for_bit() {
    let data = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/vpx-collide-5436-precise.txt"))
        .expect("reference data");
    let (report, bad) = compare(&data, Mode::Exact);
    println!("{report}");
    assert_eq!(bad, 0, "ported colliders differ from exact VPX:\n{report}");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn shipped_vpx_drifts_from_the_port_only_by_float_noise() {
    let data = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/vpx-collide-5436.txt"))
        .expect("reference data");
    let (report, bad) = compare(&data, Mode::Fast);
    println!("{report}");
    assert_eq!(bad, 0, "too many decisions differ from VPX's fast build:\n{report}");
}

fn compare(data: &str, mode: Mode) -> (String, usize) {
    let old_csr = set_ftz_daz();

    let mut env = PhysicsEnv::default();
    let mut stats: BTreeMap<&str, KindStats> = BTreeMap::new();

    for line in data.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let kind = line.split_whitespace().next().unwrap();
        let mut c = Cursor::new(line);
        let mut ck =
            Check { line, mode, errors: Vec::new(), decision_differs: false, max_ulp: 0, max_rel: 0.0, worst: String::new() };

        match kind {
            "env" => {
                env.gravity = c.v3();
                env.playfield_slope_deg = c.f();
                env.global_difficulty = c.f();
                env.hard_scatter = c.f();
                env.swap_ball_collision_handling = c.b();
                let _second_ball_at_higher_address = c.b(); // per case in "ballcollide" lines
                continue;
            }
            "lineseg" => {
                let (v1, v2) = (c.v2(), c.v2());
                let mut base = HitObjectBase::with_z(c.f(), c.f());
                base.obj_type = obj_type(c.i());
                base.enabled = c.b();
                base.obj = c.part();
                let (dir, lat, rigid) = (c.b(), c.b(), c.b());
                let ball = c.ball_s();
                let (ball, set) = with_vol(ball, c.vol());
                let dtime = c.f();
                c.arrow();
                let seg = LineSeg::new(v1, v2);
                let mut coll = CollisionEvent::default();
                let t = seg.hit_test_basic(&base, &ball, dtime, &mut coll, dir, lat, rigid);
                ck.v2("normal", seg.normal, c.v2());
                ck.f("length", seg.length, c.f());
                ck.hit(&mut c, t, &coll);
                ck.eq("volumes", set.borrow().clone(), c.vol());
            }
            "circle" => {
                let center = c.v2();
                let radius = c.f();
                let mut base = HitObjectBase::with_z(c.f(), c.f());
                base.obj_type = obj_type(c.i());
                base.enabled = c.b();
                base.obj = c.part();
                let (dir, lat, rigid) = (c.b(), c.b(), c.b());
                let ball = c.ball_s();
                let (ball, set) = with_vol(ball, c.vol());
                let dtime = c.f();
                c.arrow();
                let circle = HitCircle::new(center, radius);
                let mut coll = CollisionEvent::default();
                let t = circle.hit_test_basic_radius(&base, &ball, dtime, &mut coll, dir, lat, rigid);
                ck.hit(&mut c, t, &coll);
                ck.eq("volumes", set.borrow().clone(), c.vol());
            }
            "linez" => {
                let xy = c.v2();
                let (zlow, zhigh) = (c.f(), c.f());
                let base = HitObjectBase { enabled: c.b(), ..HitObjectBase::default() };
                let (ball, _) = with_vol(c.ball_s(), Vec::new());
                let dtime = c.f();
                c.arrow();
                let lz = HitLineZ::new(xy, zlow, zhigh);
                let mut coll = CollisionEvent::default();
                let t = lz.hit_test(&base, &ball, dtime, &mut coll);
                ck.hit(&mut c, t, &coll);
            }
            "point" => {
                let p = c.v3();
                let base = HitObjectBase { enabled: c.b(), ..HitObjectBase::default() };
                let (ball, _) = with_vol(c.ball_s(), Vec::new());
                let dtime = c.f();
                c.arrow();
                let hp = HitPoint::new(p);
                let mut coll = CollisionEvent::default();
                let t = hp.hit_test(&base, &ball, dtime, &mut coll);
                ck.hit(&mut c, t, &coll);
            }
            "ballhit" => {
                let mut hb = HitBall::new(BallId(1));
                hb.d.pos = c.v3();
                hb.d.vel = c.v3();
                hb.d.radius = c.f();
                let (ball, _) = with_vol(c.ball_s(), Vec::new());
                let dtime = c.f();
                c.arrow();
                let mut coll = CollisionEvent::default();
                let t = hb.hit_test(&ball, dtime, &mut coll);
                ck.hit(&mut c, t, &coll);
            }
            "wall3d" => {
                let mut hb = HitBall::new(BallId(1));
                hb.d.pos = c.v3();
                hb.d.vel = c.v3();
                hb.d.radius = c.f();
                hb.d.mass = c.f();
                hb.angularmomentum = c.v3();
                hb.coll.hitdistance = c.f();
                let normal = c.v3();
                let (elast, falloff, fric, scatter) = (c.f(), c.f(), c.f(), c.f());
                env.rng = Mwc64x { state: c.u64() };
                c.arrow();
                hb.collide_3d_wall(&normal, elast, falloff, fric, scatter, &mut env);
                ck.v3("pos", hb.d.pos, c.v3());
                ck.v3("vel", hb.d.vel, c.v3());
                ck.v3("angmom", hb.angularmomentum, c.v3());
                ck.eq("rng state", env.rng.state, c.u64());
            }
            "contact" => {
                let mut hb = HitBall::new(BallId(1));
                hb.d.vel = c.v3();
                hb.d.radius = c.f();
                hb.d.mass = c.f();
                hb.angularmomentum = c.v3();
                let coll = CollisionEvent { hitnormal: c.v3(), hit_org_normalvelocity: c.f(), ..CollisionEvent::default() };
                let (fric, dtime) = (c.f(), c.f());
                c.arrow();
                hb.handle_static_contact(&coll, fric, dtime, &env);
                ck.v3("vel", hb.d.vel, c.v3());
                ck.v3("angmom", hb.angularmomentum, c.v3());
            }
            "displace" => {
                let mut hb = HitBall::new(BallId(1));
                hb.d.pos = c.v3();
                hb.d.vel = c.v3();
                hb.d.radius = c.f();
                hb.d.mass = c.f();
                hb.angularmomentum = c.v3();
                hb.orientation = c.m3();
                hb.last_event_sqr_dist = c.f();
                hb.d.locked_in_kicker = c.b();
                let dtime = c.f();
                c.arrow();
                hb.update_displacements(dtime);
                ck.v3("pos", hb.d.pos, c.v3());
                let m = c.m3();
                ck.m3("orientation", &hb.orientation, &m);
                ck.f("last_event_sqr_dist", hb.last_event_sqr_dist, c.f());
            }
            "velocities" => {
                let mut hb = HitBall::new(BallId(1));
                hb.d.vel = c.v3();
                hb.d.locked_in_kicker = c.b();
                c.arrow();
                hb.update_velocities(&env);
                ck.v3("vel", hb.d.vel, c.v3());
            }
            "ballcollide" => {
                let mut this_ball = HitBall::new(BallId(1));
                this_ball.d.pos = c.v3();
                this_ball.d.vel = c.v3();
                this_ball.d.mass = c.f();
                this_ball.d.locked_in_kicker = c.b();
                this_ball.coll.hitdistance = c.f();
                let pos = c.v3();
                let vel = c.v3();
                let mass = c.f();
                let coll = CollisionEvent { hitnormal: c.v3(), hitdistance: c.f(), ..CollisionEvent::default() };
                env.swap_ball_collision_handling = c.b();
                // VPX compares the balls' addresses: give the other ball an id on the same side.
                let other_higher = c.b();
                let mut pball = HitBall::new(BallId(if other_higher { 2 } else { 0 }));
                pball.d.pos = pos;
                pball.d.vel = vel;
                pball.d.mass = mass;
                c.arrow();
                HitBall::collide_ball(&mut this_ball, &mut pball, &coll, &mut env);
                ck.v3("this.pos", this_ball.d.pos, c.v3());
                ck.v3("this.vel", this_ball.d.vel, c.v3());
                ck.v3("other.pos", pball.d.pos, c.v3());
                ck.v3("other.vel", pball.d.vel, c.v3());
            }
            "poly3d" => {
                let k = c.i() as usize;
                let rgv: Vec<Vertex3Ds> = (0..k).map(|_| c.v3()).collect();
                // fields are read in the order written
                let mut base = HitObjectBase { obj_type: obj_type(c.i()), enabled: c.b(), obj: c.part(), ..Default::default() };
                let ball = c.ball_s();
                let (ball, set) = with_vol(ball, c.vol());
                let dtime = c.f();
                c.arrow();
                // VPX keeps the polygon's normal private; hits report it, so they check it
                let poly = Hit3DPoly::new(rgv, &mut base);
                let mut coll = CollisionEvent::default();
                let t = poly.hit_test(&base, &ball, dtime, &mut coll);
                ck.hit(&mut c, t, &coll);
                ck.eq("volumes", set.borrow().clone(), c.vol());
            }
            "triangle" => {
                let rgv = [c.v3(), c.v3(), c.v3()];
                let mut base = HitObjectBase { enabled: c.b(), ..HitObjectBase::default() };
                let (ball, _) = with_vol(c.ball_s(), Vec::new());
                let dtime = c.f();
                c.arrow();
                let tri = HitTriangle::new(rgv, &mut base);
                ck.v3("normal", tri.normal, c.v3());
                let degenerate = c.b();
                ck.eq("degenerate", tri.is_degenerate(), degenerate);
                if !degenerate {
                    let mut coll = CollisionEvent::default();
                    let t = tri.hit_test(&base, &ball, dtime, &mut coll);
                    ck.hit(&mut c, t, &coll);
                }
            }
            "plane" => {
                let plane = HitPlane::new(c.v3(), c.f());
                let base = HitObjectBase { enabled: c.b(), ..HitObjectBase::default() };
                let (ball, _) = with_vol(c.ball_s(), Vec::new());
                let dtime = c.f();
                c.arrow();
                let mut coll = CollisionEvent::default();
                let t = plane.hit_test(&base, &ball, dtime, &mut coll);
                ck.hit(&mut c, t, &coll);
            }
            "line3d" => {
                let (v1, v2) = (c.v3(), c.v3());
                let mut base = HitObjectBase { enabled: c.b(), ..HitObjectBase::default() };
                let (ball, _) = with_vol(c.ball_s(), Vec::new());
                let dtime = c.f();
                c.arrow();
                // VPX keeps the rotation matrix private; the line's frame and hit normals check it
                let line = HitLine3D::new(v1, v2, &mut base);
                ck.v2("xy", line.linez.xy, c.v2());
                ck.f("zlow", line.linez.zlow, c.f());
                ck.f("zhigh", line.linez.zhigh, c.f());
                let mut coll = CollisionEvent::default();
                let t = line.hit_test(&base, &ball, dtime, &mut coll);
                ck.hit(&mut c, t, &coll);
            }
            "planecollide" => {
                let mut hb = HitBall::new(BallId(1));
                hb.d.pos = c.v3();
                hb.d.vel = c.v3();
                hb.d.radius = c.f();
                hb.d.mass = c.f();
                hb.angularmomentum = c.v3();
                hb.coll.hitdistance = c.f();
                let plane = HitPlane::new(c.v3(), c.f());
                let coll = CollisionEvent { hitnormal: c.v3(), ..CollisionEvent::default() };
                let base =
                    HitObjectBase { elasticity: c.f(), elasticity_falloff: c.f(), friction: c.f(), scatter: c.f(), ..HitObjectBase::default() };
                env.rng = Mwc64x { state: c.u64() };
                c.arrow();
                plane.collide(&base, &coll, &mut hb, &mut env);
                ck.v3("pos", hb.d.pos, c.v3());
                ck.v3("vel", hb.d.vel, c.v3());
                ck.v3("angmom", hb.angularmomentum, c.v3());
                ck.eq("rng state", env.rng.state, c.u64());
            }
            other => panic!("unknown reference line {other}"),
        }

        let st = stats.entry(kind).or_default();
        st.cases += 1;
        if ck.decision_differs {
            st.flipped += 1;
        } else {
            st.max_ulp = st.max_ulp.max(ck.max_ulp);
            if ck.max_rel > st.max_rel {
                st.max_rel = ck.max_rel;
                st.worst = ck.worst;
            }
        }
        if !ck.errors.is_empty() {
            st.bad += 1;
            if st.examples.len() < 3 {
                st.examples.push(format!("{}\n      {}", ck.line, ck.errors.join("\n      ")));
            }
        }
    }
    restore_mxcsr(old_csr);

    let mut report = String::new();
    let mut total_bad = 0;
    for (kind, st) in &stats {
        match mode {
            Mode::Exact => {
                report.push_str(&format!("{kind:12} {:6} cases  {:5} mismatches\n", st.cases, st.bad));
                for e in &st.examples {
                    report.push_str(&format!("    {e}\n"));
                }
                total_bad += st.bad;
            }
            Mode::Fast => {
                report.push_str(&format!(
                    "{kind:12} {:6} cases  {:3} decisions flipped  drift up to {} ULP, {:.1e} relative{}\n",
                    st.cases,
                    st.flipped,
                    st.max_ulp,
                    st.max_rel,
                    if st.worst.is_empty() { String::new() } else { format!(" ({})", st.worst) }
                ));
                if st.flipped as f64 > FAST_FLIP_LIMIT * st.cases as f64 {
                    total_bad += st.flipped;
                }
            }
        }
    }
    (report, total_bad)
}
