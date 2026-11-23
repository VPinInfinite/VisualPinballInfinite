//! Shared by the reference comparison tests: reading VPX's dumps, and comparing the port's values
//! with VPX's, exactly (against VPX built with exact float rules) or as drift (against VPX as
//! shipped).

#![allow(dead_code)] // each test uses its own part of this

use physics_compat::math::{Vertex2D, Vertex3Ds};
use std::collections::BTreeMap;

/// Reads one line's tokens by type; floats are 32-bit patterns in hex.
pub struct Cursor<'a> {
    t: Vec<&'a str>,
    i: usize,
}

impl<'a> Cursor<'a> {
    /// Starts after the line's first token (its kind).
    pub fn new(line: &'a str) -> Self {
        Self { t: line.split_whitespace().collect(), i: 1 }
    }
    pub fn tok(&mut self) -> &'a str {
        let s = self.t[self.i];
        self.i += 1;
        s
    }
    pub fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.tok(), 16).expect("float bits"))
    }
    pub fn i(&mut self) -> i64 {
        self.tok().parse().expect("int")
    }
    pub fn b(&mut self) -> bool {
        self.i() != 0
    }
    pub fn u64(&mut self) -> u64 {
        u64::from_str_radix(self.tok(), 16).expect("u64")
    }
    pub fn v2(&mut self) -> Vertex2D {
        Vertex2D::new(self.f(), self.f())
    }
    pub fn v3(&mut self) -> Vertex3Ds {
        Vertex3Ds::new(self.f(), self.f(), self.f())
    }
    pub fn arrow(&mut self) {
        assert_eq!(self.tok(), "->");
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Every bit must match (VPX built with exact float rules).
    Exact,
    /// VPX as shipped: values may drift by float noise, decisions may flip.
    Fast,
}

/// Distance between two floats in units in the last place; +0 and -0 are 0 apart.
pub fn ulp_distance(a: f32, b: f32) -> u32 {
    let key = |x: f32| -> i64 {
        let b = x.to_bits() as i64;
        if b & 0x8000_0000 != 0 { -(b & 0x7fff_ffff) } else { b }
    };
    (key(a) - key(b)).unsigned_abs().min(u32::MAX as u64) as u32
}

/// Compares one line's port values with VPX's.
pub struct Check<'a> {
    pub line: &'a str,
    pub mode: Mode,
    pub errors: Vec<String>,
    /// Fast mode: a yes/no decision differed, so the rest of the line isn't comparable.
    pub decision_differs: bool,
    pub max_ulp: u32,
    pub max_rel: f32,
    pub worst: String,
}

impl<'a> Check<'a> {
    pub fn new(line: &'a str, mode: Mode) -> Self {
        Self { line, mode, errors: Vec::new(), decision_differs: false, max_ulp: 0, max_rel: 0.0, worst: String::new() }
    }
    pub fn f(&mut self, what: &str, port: f32, vpx: f32) {
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
    pub fn v2(&mut self, what: &str, port: Vertex2D, vpx: Vertex2D) {
        self.f(&format!("{what}.x"), port.x, vpx.x);
        self.f(&format!("{what}.y"), port.y, vpx.y);
    }
    pub fn v3(&mut self, what: &str, port: Vertex3Ds, vpx: Vertex3Ds) {
        self.f(&format!("{what}.x"), port.x, vpx.x);
        self.f(&format!("{what}.y"), port.y, vpx.y);
        self.f(&format!("{what}.z"), port.z, vpx.z);
    }
    /// A yes/no or whole-number result.
    pub fn eq<T: PartialEq + std::fmt::Debug>(&mut self, what: &str, port: T, vpx: T) {
        if port != vpx {
            match self.mode {
                Mode::Exact => self.errors.push(format!("{what}: port {port:?} vpx {vpx:?}")),
                Mode::Fast => self.decision_differs = true,
            }
        }
    }
}

/// Results per kind of line.
#[derive(Default)]
pub struct KindStats {
    pub cases: usize,
    pub bad: usize,
    pub examples: Vec<String>,
    pub flipped: usize,
    pub max_ulp: u32,
    pub max_rel: f32,
    pub worst: String,
}

/// Collects each line's check by kind.
#[derive(Default)]
pub struct Stats<'a> {
    pub kinds: BTreeMap<&'a str, KindStats>,
}

/// Largest share of a kind's cases in which a decision may differ from VPX as shipped.
pub const FAST_FLIP_LIMIT: f64 = 0.01;

impl<'a> Stats<'a> {
    pub fn add(&mut self, kind: &'a str, ck: Check) {
        let st = self.kinds.entry(kind).or_default();
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
                let short: String = ck.line.chars().take(300).collect();
                st.examples.push(format!("{short}\n      {}", ck.errors.join("\n      ")));
            }
        }
    }

    /// The report, and how many cases fail: any difference in exact mode; in fast mode, more
    /// flipped decisions than `FAST_FLIP_LIMIT` allows.
    pub fn report(&self, mode: Mode) -> (String, usize) {
        let mut report = String::new();
        let mut total_bad = 0;
        for (kind, st) in &self.kinds {
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
}

/// The reference data file's text.
pub fn read_data(file: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/data/{file}", env!("CARGO_MANIFEST_DIR"))).expect("reference data")
}

/// The data lines, without comments and blank lines.
pub fn data_lines(data: &str) -> impl Iterator<Item = &str> {
    data.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty())
}
