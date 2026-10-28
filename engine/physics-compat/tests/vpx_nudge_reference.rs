//! Millisecond-by-millisecond comparison of the ported cabinet nudge sensor with VPX's own
//! (`CabinetNudgeSensor`: its Kalman filters, the gain calibration between its channels, its rest
//! detection, the intent handler and the cabinet's model), and of the three keyboard nudge models
//! (VP9's push and pull, 10.8's box, the "10.9" cabinet).
//!
//! The data files were written by `tests/vpi/test-vpi-nudge.cpp`, a test we add to VPX's physics
//! tester (build 5436). Each scenario names the sensor's kind (intent or direct), strength and
//! cabinet mass, and its channels' mappings (dead zone, scale, limit); then, each millisecond, the
//! readings that arrived (channel, the device's time stamp, the raw axis value) and VPX's state
//! after its step: active, the cabinet's acceleration and offset. Each keyboard run names its
//! model and strength, then the nudges given (angle, force) and the same state each millisecond.
//! The port reads the same readings through VPX's mapping and gives the same nudges, runs the same
//! steps in the same processor mode, and must match:
//!
//! - `vpx-nudge-5436-precise.txt` (VPX built with exact float rules): **bit for bit**, every
//!   step of every scenario.
//! - `vpx-nudge-5436.txt` (VPX as shipped): how far shipped VPX's fast-math rounding takes it from
//!   its own source, per scenario: the largest difference in the cabinet's acceleration and offset,
//!   and the steps whose active state differs. Reported, not asserted.

use std::cell::Cell;
use std::rc::Rc;

use physics_compat::fpmode::VpxFloatMode;
use physics_compat::math::Vertex2D;
use physics_compat::nudge::{CabinetChannels, KeyboardNudgeMode, NudgeHandler, SensorChannel};

#[derive(Debug)]
struct Channel(Cell<Option<(u64, f32)>>);

impl SensorChannel for Channel {
    fn reading(&self) -> Option<(u64, f32)> {
        self.0.get()
    }
}

/// VPX's `SensorMapping::UpdateValue`.
fn mapped(raw: f32, dead_zone: f32, scale: f32, limit: f32) -> f32 {
    let mut value = raw;
    if dead_zone > 0.0 {
        value = if value < -dead_zone {
            (value + dead_zone) / (1.0 - dead_zone)
        } else if value > dead_zone {
            (value - dead_zone) / (1.0 - dead_zone)
        } else {
            0.0
        };
    }
    // VPX's clamp: max(min(x, mx), mn), as SSE's minss and maxss
    let m = if value < limit { value } else { limit };
    let c = if m > -limit { m } else { -limit };
    c * scale
}

fn f(tok: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(tok, 16).expect("float bits"))
}

struct Outcome {
    name: String,
    steps: usize,
    first_difference: Option<String>,
    active_differs: usize,
    max_acc: f32,
    max_off: f32,
}

fn compare(file: &str) -> Vec<Outcome> {
    let data = std::fs::read_to_string(format!("{}/tests/data/{file}", env!("CARGO_MANIFEST_DIR"))).expect("reference data");
    let _mode = VpxFloatMode::enter();
    let mut out = Vec::new();
    let mut lines = data.lines().peekable();
    while let Some(line) = lines.next() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t[0] == "keyboard" {
            out.push(keyboard_run(&t, &mut lines));
            continue;
        }
        assert_eq!(t[0], "scenario", "{line}");
        let (name, intent, strength, mass) = (t[1].to_string(), t[2] == "1", f(t[3]), f(t[4]));
        let channels: [Rc<Channel>; 4] = std::array::from_fn(|_| Rc::new(Channel(Cell::new(None))));
        let mut maps = [None; 4];
        while lines.peek().is_some_and(|l| l.starts_with("map ")) {
            let t: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
            let c: usize = t[1].parse().unwrap();
            maps[c] = Some((f(t[2]), f(t[3]), f(t[4])));
            // mapped: no reading yet, its time stamp 0 and value 0, as VPX's mapping starts
            channels[c].0.set(Some((0, 0.0)));
        }
        let pick = |c: usize| -> Option<Rc<dyn SensorChannel>> { maps[c].map(|_| channels[c].clone() as Rc<dyn SensorChannel>) };
        let mut handler = NudgeHandler::new(KeyboardNudgeMode::VpxBoxModel, 1.0, 5.0);
        handler.add_cabinet_sensor(CabinetChannels { velocity_x: pick(0), velocity_y: pick(1), acceleration_x: pick(2), acceleration_y: pick(3) }, intent, strength, mass);
        let mut o = Outcome { name, steps: 0, first_difference: None, active_differs: 0, max_acc: 0.0, max_off: 0.0 };
        for line in lines.by_ref() {
            let t: Vec<&str> = line.split_whitespace().collect();
            match t[0] {
                "r" => {
                    let c: usize = t[1].parse().unwrap();
                    let (dz, scale, limit) = maps[c].expect("a reading of a mapped channel");
                    channels[c].0.set(Some((t[2].parse().unwrap(), mapped(f(t[3]), dz, scale, limit))));
                }
                "s" => {
                    handler.step_one_millisecond();
                    check_step(&mut o, &t, handler.sensor_state(0).expect("the sensor"));
                }
                "end" => break,
                other => panic!("unexpected line kind {other}"),
            }
        }
        out.push(o);
    }
    out
}

/// Compares the port's state with VPX's `s` line, keeping the first difference and the largest gaps.
fn check_step(o: &mut Outcome, t: &[&str], port: (bool, Vertex2D, Vertex2D)) {
    let (active, acc, off) = port;
    let vpx = (t[1] == "1", Vertex2D::new(f(t[2]), f(t[3])), Vertex2D::new(f(t[4]), f(t[5])));
    let bits = |v: Vertex2D| (v.x.to_bits(), v.y.to_bits());
    if o.first_difference.is_none() && (active != vpx.0 || bits(acc) != bits(vpx.1) || bits(off) != bits(vpx.2)) {
        o.first_difference = Some(format!(
            "at {} ms: port active {active}, acceleration ({}, {}), offset ({}, {}); VPX {}, ({}, {}), ({}, {})",
            o.steps + 1,
            acc.x,
            acc.y,
            off.x,
            off.y,
            vpx.0,
            vpx.1.x,
            vpx.1.y,
            vpx.2.x,
            vpx.2.y
        ));
    }
    o.active_differs += usize::from(active != vpx.0);
    o.max_acc = o.max_acc.max((acc.x - vpx.1.x).abs()).max((acc.y - vpx.1.y).abs());
    o.max_off = o.max_off.max((off.x - vpx.2.x).abs()).max((off.y - vpx.2.y).abs());
    o.steps += 1;
}

/// A keyboard model's run: `keyboard <mode> <strength>`, then nudges (`n`) and steps (`s`).
fn keyboard_run<'a>(header: &[&str], lines: &mut impl Iterator<Item = &'a str>) -> Outcome {
    let mode = KeyboardNudgeMode::from_setting(header[1].parse().unwrap()).expect("a keyboard model");
    let strength = f(header[2]);
    // without a player, VPX's box model takes its default nudge time, 5
    let mut handler = NudgeHandler::new(mode, strength, 5.0);
    let mut o = Outcome { name: format!("keyboard {} ×{strength}", header[1]), steps: 0, first_difference: None, active_differs: 0, max_acc: 0.0, max_off: 0.0 };
    for line in lines.by_ref() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t[0] {
            "n" => handler.apply_keyboard_impulse(f(t[1]), f(t[2])),
            "s" => {
                handler.step_one_millisecond();
                check_step(&mut o, &t, handler.keyboard_state());
            }
            "end" => break,
            other => panic!("unexpected line kind {other}"),
        }
    }
    o
}

fn report(file: &str, results: &[Outcome]) -> String {
    let mut r = format!("{file}:\n");
    for o in results {
        r.push_str(&format!(
            "  {:15} {:6} ms  {}  active differs {} ms, acceleration up to {:.2e} m/s² apart, offset up to {:.2e} m\n",
            o.name,
            o.steps,
            o.first_difference.as_deref().unwrap_or("identical"),
            o.active_differs,
            o.max_acc,
            o.max_off
        ));
    }
    r
}

#[test]
#[cfg(target_arch = "x86_64")]
fn the_cabinet_sensor_matches_exact_vpx_bit_for_bit() {
    let file = "vpx-nudge-5436-precise.txt";
    let results = compare(file);
    let text = report(file, &results);
    println!("{text}");
    assert_eq!(results.len(), 12);
    assert!(results.iter().all(|r| r.first_difference.is_none()), "the port differs from exact VPX:\n{text}");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn shipped_vpx_drift_report() {
    let file = "vpx-nudge-5436.txt";
    let results = compare(file);
    println!("{}", report(file, &results));
    assert_eq!(results.len(), 12);
}
