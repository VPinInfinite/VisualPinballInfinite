//! Comparison of the ported flipper with VPX's: construction, hit tests against the faces, end and
//! base, collision, resting contact, the coil model and the stroke.
//!
//! The data files were written by `tests/vpi/test-vpi-flipper.cpp`, a test we add to VPX's physics
//! tester (build 5436), with flush-to-zero and denormals-are-zero set as VPX's player sets them. Each
//! `flipper` line builds a flipper from random part settings (sometimes with a physics override); the
//! lines after it put it in random states of motion and record VPX's results.
//!
//! - `vpx-flipper-5436-precise.txt` (VPX built with exact float rules): bit for bit.
//! - `vpx-flipper-5436.txt` (VPX as shipped): float drift reported; decisions may flip in at most 1%
//!   of cases.

mod common;

use common::{Check, Cursor, Mode, Stats, data_lines, read_data};
use physics_compat::collide::{CollisionEvent, HitObjectBase};
use physics_compat::env::{BallId, PartId, PhysicsEnv};
use physics_compat::flipper::{FlipperData, HitFlipper};
use physics_compat::fpmode::VpxFloatMode;
use physics_compat::hitball::{BallS, HitBall};

/// Sets the flipper's state of motion from the line, as the C++ test's `RandomState` set it.
fn read_state(c: &mut Cursor, f: &mut HitFlipper) {
    let m = &mut f.mover;
    m.angle_cur = c.f();
    m.angle_speed = c.f();
    m.angular_momentum = c.f();
    m.angular_acceleration = c.f();
    m.cur_torque = c.f();
    m.contact_torque = c.f();
    m.sol_state = c.b();
    m.is_in_contact = c.b();
    m.enable_rotate_event = c.i() as i16;
    m.last_hit_face.set(c.b());
}

/// Checks the flipper's state of motion against the line.
fn check_state(c: &mut Cursor, ck: &mut Check, f: &HitFlipper) {
    let m = &f.mover;
    ck.f("angle_cur", m.angle_cur, c.f());
    ck.f("angle_speed", m.angle_speed, c.f());
    ck.f("angular_momentum", m.angular_momentum, c.f());
    ck.f("angular_acceleration", m.angular_acceleration, c.f());
    ck.f("cur_torque", m.cur_torque, c.f());
    ck.f("contact_torque", m.contact_torque, c.f());
    ck.eq("sol_state", m.sol_state, c.b());
    ck.eq("is_in_contact", m.is_in_contact, c.b());
    ck.eq("enable_rotate_event", m.enable_rotate_event, c.i() as i16);
    ck.eq("last_hit_face", m.last_hit_face.get(), c.b());
}

/// A hit test's result, as the C++ test's `WriteHit` wrote it.
fn check_hit(c: &mut Cursor, ck: &mut Check, what: &str, t: f32, coll: &CollisionEvent) {
    let vt = c.f();
    if (t >= 0.0) != (vt >= 0.0) {
        ck.eq(&format!("{what} hit or miss"), t >= 0.0, vt >= 0.0);
    }
    ck.f(&format!("{what} hittime"), t, vt);
    if vt >= 0.0 {
        ck.v3(&format!("{what} hitnormal"), coll.hitnormal, c.v3());
        ck.v2(&format!("{what} hitvel"), coll.hitvel, c.v2());
        ck.f(&format!("{what} hitdistance"), coll.hitdistance, c.f());
        ck.eq(&format!("{what} hitmoment_bit"), coll.hitmoment_bit, c.b());
        let contact = c.b();
        ck.eq(&format!("{what} is_contact"), coll.is_contact, contact);
        if contact {
            ck.f(&format!("{what} hit_org_normalvelocity"), coll.hit_org_normalvelocity, c.f());
        }
    }
}

/// A ball for collide and contact: position, velocity, radius, mass, spin.
fn read_ball(c: &mut Cursor) -> HitBall {
    let mut hb = HitBall::new(BallId(1));
    hb.d.pos = c.v3();
    hb.d.vel = c.v3();
    hb.d.radius = c.f();
    hb.d.mass = c.f();
    hb.angularmomentum = c.v3();
    hb
}

fn compare(file: &str, mode: Mode) -> (String, usize) {
    let data = read_data(file);
    let _fp = VpxFloatMode::enter();
    let mut env = PhysicsEnv::default();
    let mut stats = Stats::default();
    let mut flipper: Option<(HitFlipper, HitObjectBase)> = None;

    for line in data_lines(&data) {
        let kind = line.split_whitespace().next().unwrap();
        let mut c = Cursor::new(line);
        let mut ck = Check::new(line, mode);

        match kind {
            "env" => {
                env.gravity = c.v3();
                continue;
            }
            "flipper" => {
                let center = c.v2();
                let (baser, endr, flipr, angle_start, angle_end, zlow, zhigh) =
                    (c.f(), c.f(), c.f(), c.f(), c.f(), c.f(), c.f());
                let enabled = c.b();
                let data = FlipperData {
                    use_override: c.b(),
                    mass: c.f(),
                    override_mass: c.f(),
                    strength: c.f(),
                    override_strength: c.f(),
                    return_strength: c.f(),
                    override_return_strength: c.f(),
                    torque_damping: c.f(),
                    override_torque_damping: c.f(),
                    torque_damping_angle: c.f(),
                    override_torque_damping_angle: c.f(),
                    ramp_up: c.f(),
                    override_coil_ramp_up: c.f(),
                    elasticity: c.f(),
                    override_elasticity: c.f(),
                    elasticity_falloff: c.f(),
                    override_elasticity_falloff: c.f(),
                    friction: c.f(),
                    override_friction: c.f(),
                    scatter: c.f(),
                    override_scatter_angle: c.f(),
                };
                c.arrow();
                let (f, mut base) = HitFlipper::new(
                    PartId(1),
                    data,
                    center,
                    baser,
                    endr,
                    flipr,
                    angle_start,
                    angle_end,
                    zlow,
                    zhigh,
                    enabled,
                );
                f.calc_hit_bbox(&mut base);
                let m = &f.mover;
                ck.f("angle_end", m.angle_end, c.f());
                ck.f("inertia", m.inertia, c.f());
                ck.v2("zero_ang_norm", m.zero_ang_norm, c.v2());
                ck.eq("direction", m.direction, c.b());
                ck.f("elasticity", base.elasticity, c.f());
                ck.f("elasticity_falloff", base.elasticity_falloff, c.f());
                ck.f("friction", base.friction, c.f());
                ck.f("scatter", base.scatter, c.f());
                let b = base.hit_bbox;
                for (what, port) in
                    [("left", b.left), ("right", b.right), ("top", b.top), ("bottom", b.bottom), ("zlow", b.zlow), ("zhigh", b.zhigh)]
                {
                    ck.f(what, port, c.f());
                }
                flipper = Some((f, base));
            }
            "fliphit" => {
                let (f, base) = flipper.as_mut().expect("a flipper line first");
                read_state(&mut c, f);
                let ball = BallS { pos: c.v3(), vel: c.v3(), radius: c.f(), ..BallS::default() };
                let dtime = c.f();
                c.arrow();
                let mut colls = [CollisionEvent::default(); 4];
                let t1 = f.hit_test_flipper_face(base, &ball, dtime, &mut colls[0], true);
                check_hit(&mut c, &mut ck, "face1", t1, &colls[0]);
                let t2 = f.hit_test_flipper_face(base, &ball, dtime, &mut colls[1], false);
                check_hit(&mut c, &mut ck, "face2", t2, &colls[1]);
                let t3 = f.hit_test_flipper_end(base, &ball, dtime, &mut colls[2]);
                check_hit(&mut c, &mut ck, "end", t3, &colls[2]);
                let t4 = f.hit_test(base, &ball, dtime, &mut colls[3]);
                check_hit(&mut c, &mut ck, "hit_test", t4, &colls[3]);
                ck.eq("last_hit_face", f.mover.last_hit_face.get(), c.b());
            }
            "flipcollide" => {
                let (f, base) = flipper.as_mut().expect("a flipper line first");
                read_state(&mut c, f);
                let mut hb = read_ball(&mut c);
                let coll = CollisionEvent {
                    ball: Some(hb.id),
                    hitnormal: c.v3(),
                    hitdistance: c.f(),
                    hitmoment_bit: c.b(),
                    ..CollisionEvent::default()
                };
                c.arrow();
                f.collide(base, &coll, &mut hb, &mut env);
                ck.v3("ball pos", hb.d.pos, c.v3());
                ck.v3("ball vel", hb.d.vel, c.v3());
                ck.v3("ball angmom", hb.angularmomentum, c.v3());
                ck.f("angular_momentum", f.mover.angular_momentum, c.f());
                ck.f("angle_speed", f.mover.angle_speed, c.f());
            }
            "flipcontact" => {
                let (f, base) = flipper.as_mut().expect("a flipper line first");
                read_state(&mut c, f);
                let mut hb = read_ball(&mut c);
                let coll = CollisionEvent {
                    ball: Some(hb.id),
                    hitnormal: c.v3(),
                    hitdistance: c.f(),
                    hit_org_normalvelocity: c.f(),
                    ..CollisionEvent::default()
                };
                let dtime = c.f();
                c.arrow();
                f.contact(base, &coll, &mut hb, dtime, &env);
                ck.v3("ball vel", hb.d.vel, c.v3());
                ck.v3("ball angmom", hb.angularmomentum, c.v3());
                ck.f("angular_momentum", f.mover.angular_momentum, c.f());
                ck.f("angle_speed", f.mover.angle_speed, c.f());
            }
            "flipmove" => {
                let (f, _) = flipper.as_mut().expect("a flipper line first");
                read_state(&mut c, f);
                c.arrow();
                f.mover.update_velocities();
                check_state(&mut c, &mut ck, f);
                ck.f("hit_time", f.mover.get_hit_time(), c.f());
                let dtime = c.f();
                let part = f.part;
                if f.mover.update_displacements(dtime, part, &mut env) {
                    f.mover.finish_stop();
                }
                check_state(&mut c, &mut ck, f);
            }
            other => panic!("unknown reference line {other}"),
        }
        stats.add(kind, ck);
    }
    stats.report(mode)
}

#[test]
#[cfg(target_arch = "x86_64")]
fn flipper_matches_exact_vpx_bit_for_bit() {
    let (report, bad) = compare("vpx-flipper-5436-precise.txt", Mode::Exact);
    println!("{report}");
    assert_eq!(bad, 0, "the ported flipper differs from exact VPX:\n{report}");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn shipped_vpx_flipper_drifts_only_by_float_noise() {
    let (report, bad) = compare("vpx-flipper-5436.txt", Mode::Fast);
    println!("{report}");
    assert_eq!(bad, 0, "too many decisions differ from VPX's fast build:\n{report}");
}
