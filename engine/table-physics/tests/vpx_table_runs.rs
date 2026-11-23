//! Real tables in motion, compared with VPX: the reference tester's table runs
//! (`vpx-build/table-runs/precise`, from the tester built with exact float
//! rules). Each table is set up for
//! play as VPX's player sets it up, five balls start at rest along the top of
//! the playfield, every flipper is pressed from 400 to 600 ms of each second,
//! and every ball's position, velocity and spin must match VPX's bit for bit
//! after every 1 ms step, as must VPX's random number state. No script runs.
//!
//! It also times the physics steps alone, ours and VPX's (the tester writes
//! its own time; VPX as shipped is in `table-runs/fast`), for comparison.
//!
//! Needs the tables and the runs, which stay on the development machine; it
//! passes without checking anything where they are missing.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use physics_compat::engine::{NoScript, Shape};
use physics_compat::env::BallId;
use physics_compat::fpmode::VpxFloatMode;
use physics_compat::hitball::HitBall;
use physics_compat::math::{Mwc64x, Vertex3Ds};
use table_physics::settings::PhysicsSettings;
use table_physics::build;

/// Where the test set's tables are.
const TABLE_DIRS: [&str; 3] =
    [r"C:\vPinball\VisualPinball\Tables", r"C:\vPinball\VisualPinball\assets", r"C:\vPinball\VisualPinball\VPinMAME"];

fn runs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vpx-build/table-runs/precise")
}

/// Reads a dump line's fields in order.
struct Cursor<'a> {
    fields: std::str::SplitWhitespace<'a>,
}

impl<'a> Cursor<'a> {
    fn new(line: &'a str) -> Self {
        Self { fields: line.split_whitespace() }
    }
    fn word(&mut self) -> &'a str {
        self.fields.next().expect("field")
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.word(), 16).expect("hex float"))
    }
    fn v3(&mut self) -> Vertex3Ds {
        Vertex3Ds::new(self.f(), self.f(), self.f())
    }
    fn i(&mut self) -> i64 {
        self.word().parse().expect("int")
    }
    fn u64(&mut self) -> u64 {
        u64::from_str_radix(self.word(), 16).expect("hex u64")
    }
}

fn same(a: Vertex3Ds, b: Vertex3Ds) -> bool {
    a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits() && a.z.to_bits() == b.z.to_bits()
}

/// How a table's run compares: steps that matched, the first difference,
/// and the time our physics steps took (in all, and the slowest).
struct RunResult {
    steps: usize,
    first_difference: Option<String>,
    physics: Duration,
    slowest_step: Duration,
}

/// The physics time a tester run recorded: in all, and the slowest step.
fn vpx_time(dump: &str) -> Option<(Duration, Duration)> {
    let line = dump.lines().rev().find(|l| l.starts_with("time "))?;
    let mut c = Cursor::new(line);
    c.word();
    Some((Duration::from_nanos(c.i() as u64), Duration::from_nanos(c.i() as u64)))
}

fn run_table(vpx: &vpin::vpx::VPX, dump: &str) -> RunResult {
    let tp = build(vpx, &PhysicsSettings::default());
    let mut lines = dump.lines().filter(|l| !l.starts_with('#'));

    // the player's setup, as VPX made it: it must be what our table reader made
    let mut c = Cursor::new(lines.next().expect("setup"));
    assert_eq!(c.word(), "setup");
    let (gravity, slope) = (c.v3(), c.f());
    let playfield = (c.f(), c.f(), c.f(), c.f());
    let (difficulty, hard_scatter, implicit) = (c.f(), c.f(), c.i() != 0);
    let rng = c.u64();
    let ours = (tp.playfield.friction, tp.playfield.elasticity, tp.playfield.elasticity_falloff, tp.playfield.scatter_deg);
    let setup_differs = slope.to_bits() != tp.slope_deg.to_bits()
        || [playfield.0, playfield.1, playfield.2, playfield.3].map(f32::to_bits) != [ours.0, ours.1, ours.2, ours.3].map(f32::to_bits)
        || difficulty.to_bits() != tp.global_difficulty.to_bits()
        || hard_scatter.to_bits() != tp.hard_scatter.to_bits()
        || implicit != tp.implicit_playfield_mesh;
    if setup_differs {
        return RunResult {
            steps: 0,
            physics: Duration::ZERO,
            slowest_step: Duration::ZERO,
            first_difference: Some(format!(
                "setup: VPX slope {slope} playfield {playfield:?} difficulty {difficulty} scatter {hard_scatter} implicit {implicit}; ours slope {} playfield {ours:?} difficulty {} scatter {} implicit {}",
                tp.slope_deg, tp.global_difficulty, tp.hard_scatter, tp.implicit_playfield_mesh
            )),
        };
    }

    let mut engine = tp.into_engine();
    engine.env.rng = Mwc64x { state: rng };
    if !same(engine.env.gravity, gravity) {
        return RunResult {
            steps: 0,
            first_difference: Some(format!("gravity: VPX {gravity:?}, ours {:?}", engine.env.gravity)),
            physics: Duration::ZERO,
            slowest_step: Duration::ZERO,
        };
    }
    engine.start_physics(0);

    // the balls, after the engine as VPX's player creates them; their ids are VPX's address ranks
    let mut c = Cursor::new(lines.next().expect("balls"));
    assert_eq!(c.word(), "balls");
    let mut ids = Vec::new();
    for _ in 0..5 {
        let rank = c.i() as u32;
        let mut hb = HitBall::new(BallId(rank));
        hb.d.pos = c.v3();
        hb.d.radius = 25.0;
        hb.d.mass = 1.0;
        engine.add_ball(hb);
        ids.push(BallId(rank));
    }
    let _flippers = lines.next().expect("flippers");
    let flippers: Vec<u32> = (0..engine.colliders().len() as u32)
        .filter(|&i| matches!(engine.colliders()[i as usize].shape, Shape::Flipper(_)))
        .collect();

    let mut result = RunResult { steps: 0, first_difference: None, physics: Duration::ZERO, slowest_step: Duration::ZERO };
    for line in lines {
        let mut c = Cursor::new(line);
        if c.word() != "s" {
            break; // the tester's timing line
        }
        let step = c.i() as u64;
        if step % 1000 == 400 || step % 1000 == 600 {
            for &k in &flippers {
                if let Shape::Flipper(f) = &mut engine.collider_mut(k).shape {
                    if step % 1000 == 400 { f.rotate_to_end() } else { f.rotate_to_start() }
                }
            }
        }
        let t0 = Instant::now();
        engine.update_physics(engine.start_time_usec() + step * 1000 + 1, &mut NoScript);
        let t = t0.elapsed();
        result.physics += t;
        result.slowest_step = result.slowest_step.max(t);

        let vpx_rng = c.u64();
        let mut diffs = Vec::new();
        if engine.env.rng.state != vpx_rng {
            diffs.push(format!("rng: ours {:016x} VPX {vpx_rng:016x}", engine.env.rng.state));
        }
        for id in &ids {
            let b = engine.balls().iter().find(|b| b.id == *id).expect("ball");
            for (what, ours, vpx) in [("pos", b.d.pos, c.v3()), ("vel", b.d.vel, c.v3()), ("spin", b.angularmomentum, c.v3())] {
                if !same(ours, vpx) {
                    diffs.push(format!("ball {} {what}: ours {ours:?} VPX {vpx:?}", id.0));
                }
            }
        }
        if !diffs.is_empty() {
            result.first_difference = Some(format!("step {step}: {}", diffs.join("; ")));
            return result;
        }
        result.steps += 1;
    }
    result
}

#[test]
fn table_runs_match_exact_vpx() {
    let runs = runs_dir();
    if !Path::new(TABLE_DIRS[0]).is_dir() || !runs.is_dir() {
        eprintln!("tables or runs missing: nothing checked");
        return;
    }
    let _mode = VpxFloatMode::enter();
    let mut failures = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&runs).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for dump_path in entries {
        let stem = dump_path.file_stem().unwrap().to_string_lossy().to_string();
        let Some(vpx_path) =
            TABLE_DIRS.iter().map(|d| Path::new(d).join(format!("{stem}.vpx"))).find(|p| p.is_file())
        else {
            continue;
        };
        let dump = std::fs::read_to_string(&dump_path).expect("run");
        let r = run_table(&vpin::vpx::read(&vpx_path).expect("table"), &dump);
        // VPX's own time for the same steps, as shipped and with exact float rules
        let shipped = std::fs::read_to_string(runs.join("../fast").join(format!("{stem}.txt"))).ok();
        let timing = format!(
            "physics {:.1} ms, slowest step {} us; VPX as shipped {}; exact build {}",
            r.physics.as_secs_f64() * 1e3,
            r.slowest_step.as_micros(),
            shipped.as_deref().and_then(vpx_time).map_or("-".to_string(), |(t, m)| format!("{:.1} ms, slowest {} us", t.as_secs_f64() * 1e3, m.as_micros())),
            vpx_time(&dump).map_or("-".to_string(), |(t, m)| format!("{:.1} ms, slowest {} us", t.as_secs_f64() * 1e3, m.as_micros())),
        );
        match &r.first_difference {
            None => eprintln!("{stem}: all {} steps match; {timing}", r.steps),
            Some(d) => {
                eprintln!("{stem}: {} steps match, then {d}", r.steps);
                failures.push(stem);
            }
        }
    }
    assert!(failures.is_empty(), "{} tables differ: {failures:?}", failures.len());
}

/// A physics settings file beside a table (`<table>.vpp`, read as VPX reads one as it loads a
/// table), compared with VPX's tester, whose table loading reads it too: copies of test tables,
/// each with a file beside it, in `vpx-build/table-runs/vpp-tables`, and the exact build's runs
/// of them in `vpx-build/table-runs/precise-vpp`.
#[test]
fn table_runs_with_a_physics_settings_file_match_exact_vpx() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vpx-build/table-runs");
    let (tables, runs) = (base.join("vpp-tables"), base.join("precise-vpp"));
    if !tables.is_dir() || !runs.is_dir() {
        eprintln!("tables with physics settings files, or their runs, missing: nothing checked");
        return;
    }
    let _mode = VpxFloatMode::enter();
    let mut failures = Vec::new();
    for dump_path in std::fs::read_dir(&runs).unwrap().map(|e| e.unwrap().path()) {
        let stem = dump_path.file_stem().unwrap().to_string_lossy().to_string();
        let vpx_path = tables.join(format!("{stem}.vpx"));
        let mut vpx = vpin::vpx::read(&vpx_path).expect("table");
        let file = table_physics::vpp::find(&vpx_path).expect("its physics settings file");
        table_physics::vpp::apply(&mut vpx, &std::fs::read_to_string(&file).unwrap()).expect("a physics settings file");
        let r = run_table(&vpx, &std::fs::read_to_string(&dump_path).expect("run"));
        match &r.first_difference {
            None => eprintln!("{stem} with {}: all {} steps match", file.display(), r.steps),
            Some(d) => {
                eprintln!("{stem} with {}: {} steps match, then {d}", file.display(), r.steps);
                failures.push(stem);
            }
        }
    }
    assert!(failures.is_empty(), "{} tables differ: {failures:?}", failures.len());
}
