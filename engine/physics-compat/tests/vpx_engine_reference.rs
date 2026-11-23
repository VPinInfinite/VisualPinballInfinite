//! Millisecond-by-millisecond comparison of the ported physics engine with VPX's: whole scenes of
//! walls, posts, ramps, mesh pieces and balls, run for a second.
//!
//! The data files were written by `tests/vpi/test-vpi-engine.cpp`, a test we add to VPX's physics
//! tester (build 5436). Each scene lists its colliders and balls, then VPX's state after every 1 ms
//! step: the random generator and each ball's position, velocity and spin. The port builds the same
//! scene, runs the same steps in the same processor mode, and must match:
//!
//! - `vpx-engine-5436-precise.txt` (VPX built with exact float rules): **bit for bit**, every
//!   step of every scene.
//! - `vpx-engine-5436.txt` (VPX as shipped): this measures how far shipped VPX drifts from its own
//!   source. Its fast-math rounding differs in the last bit within the first steps, so the
//!   port keeps running on its own and the report gives, per scene, how long every ball stays within
//!   0.1, 1 and 10 VP units of shipped VPX (a ball is 50 units across). Reported, not asserted.

use physics_compat::collide::{HitCircle, HitLineZ, HitObjectBase, HitPoint, LineSeg};
use physics_compat::collideex::{Hit3DPoly, HitLine3D, HitTriangle};
use physics_compat::engine::{Collider, NoScript, PhysicsEngine, PlayfieldMaterial, Shape, TableBox};
use physics_compat::env::BallId;
use physics_compat::fpmode::VpxFloatMode;
use physics_compat::hitball::HitBall;
use physics_compat::math::{Mwc64x, Vertex2D, Vertex3Ds};
use physics_compat::quadtree::{Owner, OwnerKind};

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
    fn i(&mut self) -> i64 {
        self.tok().parse().expect("int")
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
    /// A collider's material, as `Apply` writes it, set on its base.
    fn material(&mut self, base: &mut HitObjectBase) {
        base.elasticity = self.f();
        base.elasticity_falloff = self.f();
        base.friction = self.f();
        base.scatter = self.f();
    }
}

/// The parts of a scene, as the reference file describes it.
struct Scene {
    table: TableBox,
    playfield: PlayfieldMaterial,
    difficulty: f32,
    hard_scatter: f32,
    colliders: Vec<Collider>,
    /// Position, velocity, radius, mass.
    balls: Vec<(Vertex3Ds, Vertex3Ds, f32, f32)>,
}

/// The harness's raw colliders all belong to one shim part.
const SHIM: Owner = Owner { id: 1, kind: OwnerKind::Other };
const TABLE: Owner = Owner { id: 0, kind: OwnerKind::Other };

fn collider(base: HitObjectBase, shape: Shape) -> Collider {
    Collider { base, shape, owner: SHIM }
}

/// Distances at which the drift report notes when the balls first part.
const DRIFT_LEVELS: [f32; 3] = [0.1, 1.0, 10.0];

/// A run's outcome for one scene.
struct SceneResult {
    steps: usize,
    /// The first step where the port and VPX differ, and how.
    first_difference: Option<(usize, String)>,
    /// Drift mode: the first step at which some ball is farther than each of `DRIFT_LEVELS` from
    /// where VPX has it.
    parted: [Option<usize>; 3],
}

/// Runs a scene against VPX's steps. `drift`: keep going after the first difference, to measure
/// how far apart the balls get.
fn run_scene(scene: Scene, start: &str, steps: &[&str], drift: bool) -> SceneResult {
    let mut c = Cursor::new(start);
    let gravity = c.v3();
    let rng = c.u64();
    // the balls' address order in VPX, which decides who handles a ball-to-ball collision
    let ranks: Vec<u32> = scene.balls.iter().map(|_| c.i() as u32).collect();

    let balls: Vec<HitBall> = scene
        .balls
        .iter()
        .zip(&ranks)
        .map(|(&(pos, vel, radius, mass), &rank)| {
            let mut hb = HitBall::new(BallId(rank));
            hb.d.pos = pos;
            hb.d.vel = vel;
            hb.d.radius = radius;
            hb.d.mass = mass;
            hb
        })
        .collect();

    let mut engine = PhysicsEngine::new(&scene.table, &scene.playfield, TABLE, scene.colliders, balls);
    engine.env.gravity = gravity;
    engine.env.global_difficulty = scene.difficulty;
    engine.env.hard_scatter = scene.hard_scatter;
    engine.env.rng = Mwc64x { state: rng };
    engine.set_implicit_playfield_mesh(true);
    engine.start_physics(0);

    let mut result = SceneResult { steps: 0, first_difference: None, parted: [None; 3] };
    for (k, line) in steps.iter().enumerate() {
        let mut c = Cursor::new(line);
        let step = c.i() as u64;
        assert_eq!(step, k as u64 + 1);
        engine.update_physics(engine.start_time_usec() + step * 1000 + 1, &mut NoScript);
        result.steps += 1;

        let mut diffs = Vec::new();
        let mut farthest = 0.0f32;
        let vpx_rng = c.u64();
        if engine.env.rng.state != vpx_rng {
            diffs.push(format!("rng: port {:016x} vpx {vpx_rng:016x}", engine.env.rng.state));
        }
        for &rank in &ranks {
            let (pos, vel, ang) = (c.v3(), c.v3(), c.v3());
            let b = engine.balls().iter().find(|b| b.id == BallId(rank)).expect("ball");
            for (what, port, vpx) in [("pos", b.d.pos, pos), ("vel", b.d.vel, vel), ("angmom", b.angularmomentum, ang)] {
                let bits = |v: Vertex3Ds| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
                if bits(port) != bits(vpx) {
                    diffs.push(format!("ball {rank} {what}: port {port:?} vpx {vpx:?}"));
                }
            }
            let apart = (b.d.pos - pos).length();
            farthest = if apart.is_nan() { f32::INFINITY } else { farthest.max(apart) };
        }
        for (level, parted) in DRIFT_LEVELS.iter().zip(&mut result.parted) {
            if parted.is_none() && farthest > *level {
                *parted = Some(k + 1);
            }
        }
        if !diffs.is_empty() && result.first_difference.is_none() {
            result.first_difference = Some((k + 1, diffs.join("\n        ")));
            if !drift {
                break;
            }
        }
    }
    result
}

fn parse_scene<'a>(lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>, header: &str) -> Scene {
    let mut c = Cursor::new(header);
    let _id = c.i();
    let table = TableBox {
        left: c.f(),
        top: c.f(),
        right: c.f(),
        bottom: c.f(),
        glass_top_height: c.f(),
        glass_bottom_height: c.f(),
    };
    let playfield = PlayfieldMaterial { friction: c.f(), elasticity: c.f(), elasticity_falloff: c.f(), scatter_deg: c.f() };
    let (difficulty, hard_scatter) = (c.f(), c.f());

    let mut colliders = Vec::new();
    let mut balls = Vec::new();
    while let Some(line) = lines.peek() {
        let kind = line.split_whitespace().next().unwrap_or("");
        let mut c = Cursor::new(line);
        match kind {
            "seg" => {
                let (v1, v2, zlow, zhigh) = (c.v2(), c.v2(), c.f(), c.f());
                let mut base = HitObjectBase::with_z(zlow, zhigh);
                c.material(&mut base);
                colliders.push(collider(base, Shape::LineSeg(LineSeg::new(v1, v2))));
            }
            "linez" => {
                let (xy, zlow, zhigh) = (c.v2(), c.f(), c.f());
                let mut base = HitObjectBase::default();
                c.material(&mut base);
                colliders.push(collider(base, Shape::LineZ(HitLineZ::new(xy, zlow, zhigh))));
            }
            "circle" => {
                let (center, r, zlow, zhigh) = (c.v2(), c.f(), c.f(), c.f());
                let mut base = HitObjectBase::with_z(zlow, zhigh);
                c.material(&mut base);
                colliders.push(collider(base, Shape::Circle(HitCircle::new(center, r))));
            }
            "poly" => {
                let n = c.i() as usize;
                let rgv: Vec<Vertex3Ds> = (0..n).map(|_| c.v3()).collect();
                let mut base = HitObjectBase::default();
                let poly = Hit3DPoly::new(rgv, &mut base);
                c.material(&mut base);
                colliders.push(collider(base, Shape::Poly3D(poly)));
            }
            "line3d" => {
                let (v1, v2) = (c.v3(), c.v3());
                let mut base = HitObjectBase::default();
                let line = HitLine3D::new(v1, v2, &mut base);
                c.material(&mut base);
                colliders.push(collider(base, Shape::Line3D(line)));
            }
            "tri" => {
                let rgv = [c.v3(), c.v3(), c.v3()];
                let mut base = HitObjectBase::default();
                let tri = HitTriangle::new(rgv, &mut base);
                c.material(&mut base);
                colliders.push(collider(base, Shape::Triangle(tri)));
            }
            "point" => {
                let p = c.v3();
                let mut base = HitObjectBase::default();
                c.material(&mut base);
                colliders.push(collider(base, Shape::Point(HitPoint::new(p))));
            }
            "ball" => balls.push((c.v3(), c.v3(), c.f(), c.f())),
            _ => break,
        }
        lines.next();
    }
    Scene { table, playfield, difficulty, hard_scatter, colliders, balls }
}

fn compare(file: &str, drift: bool) -> Vec<SceneResult> {
    let path = format!("{}/tests/data/{file}", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read_to_string(&path).expect("reference data");
    let _mode = VpxFloatMode::enter();

    let mut lines = data.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).peekable();
    let mut results = Vec::new();
    while let Some(header) = lines.next() {
        assert!(header.starts_with("scene"), "expected a scene, got {header}");
        let scene = parse_scene(&mut lines, header);
        let start = lines.next().expect("start line");
        assert!(start.starts_with("start"));
        let mut steps = Vec::new();
        while lines.peek().is_some_and(|l| l.starts_with("step")) {
            steps.push(lines.next().unwrap());
        }
        results.push(run_scene(scene, start, &steps, drift));
    }
    results
}

fn report(file: &str, results: &[SceneResult], drift: bool) -> String {
    let mut s = format!("{file}\n");
    for (i, r) in results.iter().enumerate() {
        match &r.first_difference {
            None => s.push_str(&format!("  scene {i:2}: identical for all {} steps\n", r.steps)),
            Some((step, what)) if !drift => {
                s.push_str(&format!("  scene {i:2}: identical until step {step}\n        {what}\n"))
            }
            Some((step, _)) => {
                let within: Vec<String> = DRIFT_LEVELS
                    .iter()
                    .zip(&r.parted)
                    .map(|(level, parted)| match parted {
                        None => format!("within {level} units throughout"),
                        Some(p) => format!("{level} units apart at step {p}"),
                    })
                    .collect();
                s.push_str(&format!("  scene {i:2}: first bit difference at step {step}; {}\n", within.join(", ")));
            }
        }
    }
    s
}

#[test]
#[cfg(target_arch = "x86_64")]
fn engine_matches_exact_vpx_bit_for_bit() {
    let file = "vpx-engine-5436-precise.txt";
    let results = compare(file, false);
    let text = report(file, &results, false);
    println!("{text}");
    assert!(!results.is_empty());
    assert!(results.iter().all(|r| r.first_difference.is_none()), "the port differs from exact VPX:\n{text}");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn shipped_vpx_drift_report() {
    let file = "vpx-engine-5436.txt";
    let results = compare(file, true);
    println!("{}", report(file, &results, true));
    assert!(!results.is_empty());
}
