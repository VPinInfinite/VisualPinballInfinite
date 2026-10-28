//! Bit-for-bit comparison of the ported math with VPX's own compiled math.
//!
//! `tests/data/vpx-math-5436.txt` was written by `tests/vpi/test-vpi-math.cpp`, a test we add to
//! VPX's physics tester (build 5436, VPX's release compiler flags), with flush-to-zero and
//! denormals-are-zero set as VPX's player sets them on the thread that runs physics. Every float
//! is a 32-bit pattern in hex. This test recomputes each line with the port, in the same
//! processor mode, and requires identical bits.
//!
//! One documented exception: VPX's `min`/`max` (and `clamp`, `saturate` built on them) compile to
//! the processor's own min/max instructions. In flush-to-zero mode those return a denormal input as
//! zero, and the compiler orders their two operands differently at different call sites, which
//! decides the sign of a zero result. No single function reproduces that everywhere. It is harmless
//! here: in this mode a denormal acts as zero in every later operation, and +0 and -0 almost never
//! differ. For those four functions, a zero-or-denormal result matches any other zero-or-denormal;
//! such cases are counted and reported separately.

use physics_compat::math::bbox::{FRect3D, f_rect_intersect_3d, f_rect_intersect_3d_sphere};
use physics_compat::math::def::{self, Mwc64x};
use physics_compat::math::equations::solve_quadratic_eq;
use physics_compat::math::{Matrix3, Vertex2D, Vertex3Ds, cross_product};
use std::collections::BTreeMap;

/// Sets flush-to-zero and denormals-are-zero on this thread (MXCSR | 0x8040), as VPX's player does
/// on its physics thread, and returns the previous MXCSR.
#[cfg(target_arch = "x86_64")]
fn set_ftz_daz() -> u32 {
    let mut old: u32 = 0;
    // SAFETY: reads and writes the SSE control register of the current thread only. The floats
    // compared here are read from a file at run time, so no constant folding is involved.
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

fn f(u: u32) -> f32 {
    f32::from_bits(u)
}

fn v3(i: &[u32]) -> Vertex3Ds {
    Vertex3Ds::new(f(i[0]), f(i[1]), f(i[2]))
}

fn m3(i: &[u32]) -> Matrix3 {
    let mut m = Matrix3::default();
    for r in 0..3 {
        for c in 0..3 {
            m.m[r][c] = f(i[r * 3 + c]);
        }
    }
    m
}

fn rect(i: &[u32]) -> FRect3D {
    FRect3D::new(f(i[0]), f(i[1]), f(i[2]), f(i[3]), f(i[4]), f(i[5]))
}

fn out_v3(v: Vertex3Ds) -> Vec<u32> {
    vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn out_v2(v: Vertex2D) -> Vec<u32> {
    vec![v.x.to_bits(), v.y.to_bits()]
}

fn out_m3(m: Matrix3) -> Vec<u32> {
    m.m.iter().flatten().map(|x| x.to_bits()).collect()
}

/// True for zeros and denormals of either sign: what flush-to-zero mode treats as zero.
fn zero_class(bits: u32) -> bool {
    bits & 0x7f80_0000 == 0
}

/// The min/max family, where zero-class results are compared as a class (see the module docs).
fn min_max_family(name: &str) -> bool {
    matches!(name, "min" | "max" | "clamp" | "saturate")
}

fn out_b(b: bool) -> Vec<u32> {
    vec![u32::from(b)]
}

struct Rngs {
    r01: Mwc64x,
    rm11: Mwc64x,
}

/// Recomputes one reference line with the port.
fn compute(name: &str, i: &[u32], rngs: &mut Rngs) -> Vec<u32> {
    match name {
        "v3_normalize" => {
            let mut v = v3(i);
            v.normalize();
            out_v3(v)
        }
        "v3_normalize_to" => {
            let mut v = v3(i);
            v.normalize_to(f(i[3]));
            out_v3(v)
        }
        "v3_normalize_safe" => {
            let mut v = v3(i);
            v.normalize_safe();
            out_v3(v)
        }
        "v3_length" => vec![v3(i).length().to_bits()],
        "v3_length_squared" => vec![v3(i).length_squared().to_bits()],
        "v3_dot" => vec![v3(i).dot(&v3(&i[3..])).to_bits()],
        "v3_cross" => out_v3(cross_product(&v3(i), &v3(&i[3..]))),
        "v3_div" => out_v3(v3(i) / f(i[3])),
        "v3_mul" => out_v3(v3(i) * f(i[3])),
        "v3_is_zero" => out_b(v3(i).is_zero()),
        "v2_normalize" => {
            let mut v = Vertex2D::new(f(i[0]), f(i[1]));
            v.normalize();
            out_v2(v)
        }
        "v2_normalize_safe" => {
            let mut v = Vertex2D::new(f(i[0]), f(i[1]));
            v.normalize_safe();
            out_v2(v)
        }
        "v2_length" => vec![Vertex2D::new(f(i[0]), f(i[1])).length().to_bits()],
        "v2_div" => out_v2(Vertex2D::new(f(i[0]), f(i[1])) / f(i[2])),
        "lerp" => vec![def::lerp(f(i[0]), f(i[1]), f(i[2])).to_bits()],
        "clamp" => vec![def::clamp(f(i[0]), f(i[1]), f(i[2])).to_bits()],
        "saturate" => vec![def::saturate(f(i[0])).to_bits()],
        "min" => vec![def::min(f(i[0]), f(i[1])).to_bits()],
        "max" => vec![def::max(f(i[0]), f(i[1])).to_bits()],
        "ang_to_rad" => vec![def::ang_to_rad(f(i[0])).to_bits()],
        "rad_to_ang" => vec![def::rad_to_ang(f(i[0])).to_bits()],
        "unifloat" => vec![def::map_u32_to_unifloat(i[0]).to_bits()],
        "sinf" => vec![f(i[0]).sin().to_bits()],
        "cosf" => vec![f(i[0]).cos().to_bits()],
        "sqrtf" => vec![f(i[0]).sqrt().to_bits()],
        "quadratic" => match solve_quadratic_eq(f(i[0]), f(i[1]), f(i[2])) {
            Some((s1, s2)) => vec![1, s1.to_bits(), s2.to_bits()],
            None => vec![0],
        },
        "rect3d" => out_b(f_rect_intersect_3d(&rect(i), &rect(&i[6..]))),
        "rect3d_sphere" => out_b(f_rect_intersect_3d_sphere(&v3(i), f(i[3]), &rect(&i[4..]))),
        "m3_skew_mul_scalar" => {
            let mut skew = Matrix3::default();
            skew.skew_symmetric(&v3(i));
            out_m3(Matrix3::mul_matrices_and_mul_scalar(&skew, &m3(&i[3..]), f(i[12])))
        }
        "m3_orthonormalize" => {
            let mut m = m3(i);
            m.ortho_normalize();
            out_m3(m)
        }
        "m3_add" => out_m3(m3(i) + m3(&i[9..])),
        "m3_mul_vec" => out_v3(m3(i) * v3(&i[9..])),
        "m3_mul_vec_t" => out_v3(m3(i).mul_vector_t(&v3(&i[9..]))),
        "m3_rotation" => {
            let mut m = Matrix3::default();
            m.rotation_around_axis(&v3(i), f(i[3]));
            out_m3(m)
        }
        "rand_01" => {
            if i[0] == 0 {
                rngs.r01 = Mwc64x::new();
            }
            vec![rngs.r01.rand_01().to_bits()]
        }
        "rand_m11" => {
            if i[0] == 0 {
                rngs.rm11 = Mwc64x::new();
            }
            vec![rngs.rm11.rand_m11().to_bits()]
        }
        other => panic!("unknown reference function {other}"),
    }
}

/// Against VPX's own build (`/fp:fast`). The standalone math calls in the dump happen to compile
/// to the source's arithmetic even with fast math, so these match bit for bit too.
#[test]
#[cfg(target_arch = "x86_64")]
fn ported_math_matches_vpx_bit_for_bit() {
    compare_file("vpx-math-5436.txt");
}

/// Against VPX built with exact float semantics (`/fp:precise`).
#[test]
#[cfg(target_arch = "x86_64")]
fn ported_math_matches_exact_vpx_bit_for_bit() {
    compare_file("vpx-math-5436-precise.txt");
}

#[cfg(target_arch = "x86_64")]
fn compare_file(file: &str) {
    let path = format!("{}/tests/data/{file}", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read_to_string(&path).expect("reference data");

    let old_csr = set_ftz_daz();
    let mut rngs = Rngs { r01: Mwc64x::new(), rm11: Mwc64x::new() };
    // per function: (lines, mismatches, first few mismatches, zero-class-only differences)
    let mut stats: BTreeMap<String, (usize, usize, Vec<String>, usize)> = BTreeMap::new();

    for line in data.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let name = tokens[0];
        let arrow = tokens.iter().position(|t| *t == "->").expect("arrow");
        let parse = |t: &&str| -> u32 {
            if name.starts_with("rand_") { t.parse().expect("index") } else { u32::from_str_radix(t, 16).expect("hex") }
        };
        let inputs: Vec<u32> = tokens[1..arrow].iter().map(parse).collect();
        let expected: Vec<u32> = tokens[arrow + 1..]
            .iter()
            .map(|t| if t.len() == 1 { t.parse().expect("bool") } else { u32::from_str_radix(t, 16).expect("hex") })
            .collect();

        let got = compute(name, &inputs, &mut rngs);
        let entry = stats.entry(name.to_string()).or_default();
        entry.0 += 1;
        if got != expected {
            if min_max_family(name) && got.len() == 1 && zero_class(got[0]) && zero_class(expected[0]) {
                entry.3 += 1;
                continue;
            }
            entry.1 += 1;
            if entry.2.len() < 3 {
                entry.2.push(format!("{line}\n      port: {}", got.iter().map(|g| format!("{g:08x}")).collect::<Vec<_>>().join(" ")));
            }
        }
    }
    restore_mxcsr(old_csr);

    let mut report = String::new();
    let mut total_bad = 0;
    for (name, (n, bad, examples, zero_only)) in &stats {
        let note = if *zero_only > 0 { format!("  ({zero_only} differ only as zero vs. denormal or sign of zero)") } else { String::new() };
        report.push_str(&format!("{name:20} {n:6} lines  {bad:5} mismatches{note}\n"));
        for e in examples {
            report.push_str(&format!("    {e}\n"));
        }
        total_bad += bad;
    }
    println!("{file}\n{report}");
    assert_eq!(total_bad, 0, "ported math differs from VPX ({file}):\n{report}");
}
