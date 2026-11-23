// SPDX-License-Identifier: GPL-3.0-only
//
// Ported from Visual Pinball X build 5436 (commit af26b2d), plugins/upscaledmd/xbrz/xbrz.cpp,
// xbrz.h, xbrz_tools.h and config.h, by the Visual Pinball Infinite contributors, 2026 — xBRZ
// as VPX 5436 carries it, in the form VPX's plugin runs (`ColorFormat::rgb`: the colour
// distance from its table, borders taken from the nearest dot). Version 3 of the GPL alone, as
// xBRZ is. xBRZ's own notice:
//
// ****************************************************************************
// * This file is part of the xBRZ project. It is distributed under           *
// * GNU General Public License: https://www.gnu.org/licenses/gpl-3.0         *
// * Copyright (C) Zenju (zenju AT gmx DOT de) - All Rights Reserved          *
// *                                                                          *
// * Additionally and as a special exception, the author gives permission     *
// * to link the code of this program with the following libraries            *
// * (or with modified versions that use the same licenses), and distribute   *
// * linked combinations including the two: MAME, FreeFileSync, Snes9x, ePSXe *
// *                                                                          *
// * You must obey the GNU General Public License in all respects for all of  *
// * the code used other than MAME, FreeFileSync, Snes9x, ePSXe.              *
// * If you modify this file, you may extend this exception to your version   *
// * of the file, but you are not obligated to do so. If you do not wish to   *
// * do so, delete this exception statement from your version.                *
// ****************************************************************************

//! xBRZ, "scale by rules" (Zenju's high-quality pixel-art scaler), 2× to 6×:
//! each dot's four corners judged for an edge running through them, by how
//! alike the colours around are, then the dot's block filled with its colour
//! and the corners blended towards the neighbour's along the edge's slope.
//!
//! One thing kept from VPX's copy, not xBRZ's own: since VPX's update of
//! 2025-11-19 its table of colour distances reads each channel's difference
//! as unsigned, so a difference below zero counts as a large one (xBRZ reads
//! it signed). Its pictures are VPX's; so are these.

/// xBRZ's settings (`ScalerCfg`).
const EQUAL_COLOR_TOLERANCE: f32 = 30.0;
const CENTER_DIRECTION_BIAS: f32 = 4.0;
const DOMINANT_DIRECTION_THRESHOLD: f32 = 3.6;
const STEEP_DIRECTION_THRESHOLD: f32 = 2.4;

// xBRZ's channels: red the third byte, green the second, blue the first
fn red(p: u32) -> u8 {
    (p >> 16) as u8
}

fn green(p: u32) -> u8 {
    (p >> 8) as u8
}

fn blue(p: u32) -> u8 {
    p as u8
}

/// The colour distance as VPX's `distYCbCrBuffered` gives it: the channels'
/// differences halved into a byte each, read back unsigned and doubled, then
/// the length of their difference in Y'CbCr (BT.2020's weights) — what its
/// table holds, worked out as it's needed.
fn dist(p1: u32, p2: u32) -> f32 {
    const K_B: f64 = 0.0593;
    const K_R: f64 = 0.2627;
    const K_G: f64 = 1.0 - K_B - K_R;
    const SCALE_B: f32 = (0.5 / (1.0 - K_B)) as f32;
    const SCALE_R: f32 = (0.5 / (1.0 - K_R)) as f32;
    let byte = |d: i32| f32::from((d / 2) as u8) * 2.0;
    let r_diff = byte(i32::from(red(p1)) - i32::from(red(p2)));
    let g_diff = byte(i32::from(green(p1)) - i32::from(green(p2)));
    let b_diff = byte(i32::from(blue(p1)) - i32::from(blue(p2)));
    let y = K_R as f32 * r_diff + K_G as f32 * g_diff + K_B as f32 * b_diff;
    let c_b = SCALE_B * (b_diff - y);
    let c_r = SCALE_R * (r_diff - y);
    (y * y + c_b * c_b + c_r * c_r).sqrt()
}

fn eq(p1: u32, p2: u32) -> bool {
    dist(p1, p2) < EQUAL_COLOR_TOLERANCE
}

/// `pix_front` over `pix_back` at `m / n`, each channel rounded (xBRZ's `gradientRGB`; the
/// result has no alpha).
fn gradient(m: u32, n: u32, front: u32, back: u32) -> u32 {
    let channel = |f: u8, b: u8| (u32::from(f) * m + u32::from(b) * (n - m) + n / 2) / n;
    (channel(red(front), red(back)) << 16) | (channel(green(front), green(back)) << 8) | channel(blue(front), blue(back))
}

const BLEND_NONE: u8 = 0;
const BLEND_NORMAL: u8 = 1;
const BLEND_DOMINANT: u8 = 2;

#[derive(Default)]
struct BlendResult {
    blend_e: u8,
    blend_f: u8,
    blend_h: u8,
    blend_i: u8,
}

/// The 4 × 4 neighbourhood of `E`:
///
/// ```text
/// A B C P
/// D E F O
/// G H I N
/// J K L M
/// ```
#[derive(Clone, Copy, Default)]
struct Kernel {
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
    f: u32,
    g: u32,
    h: u32,
    i: u32,
    j: u32,
    k: u32,
    l: u32,
    m: u32,
    n: u32,
    o: u32,
    p: u32,
}

/// Which way the edge runs through the corner between E, F, H and I.
fn pre_process_corners(k: &Kernel) -> BlendResult {
    if (k.e == k.f && k.h == k.i) || (k.e == k.h && k.f == k.i) {
        return BlendResult::default();
    }
    let hf = dist(k.g, k.e) + dist(k.e, k.c) + dist(k.k, k.i) + dist(k.i, k.o) + CENTER_DIRECTION_BIAS * dist(k.h, k.f);
    let ei = dist(k.d, k.h) + dist(k.h, k.l) + dist(k.b, k.f) + dist(k.f, k.n) + CENTER_DIRECTION_BIAS * dist(k.e, k.i);
    let mut result = BlendResult::default();
    if hf < ei {
        let dominant = DOMINANT_DIRECTION_THRESHOLD * hf < ei;
        let kind = if dominant { BLEND_DOMINANT } else { BLEND_NORMAL };
        if k.e != k.f && k.e != k.h {
            result.blend_e = kind;
        }
        if k.i != k.h && k.i != k.f {
            result.blend_i = kind;
        }
    } else if ei < hf {
        let dominant = DOMINANT_DIRECTION_THRESHOLD * ei < hf;
        let kind = if dominant { BLEND_DOMINANT } else { BLEND_NORMAL };
        if k.h != k.e && k.h != k.i {
            result.blend_h = kind;
        }
        if k.f != k.e && k.f != k.i {
            result.blend_f = kind;
        }
    }
    result
}

// four corners' blend types in a byte: top left, top right, bottom right, bottom left
fn top_r(b: u8) -> u8 {
    (b >> 2) & 3
}

fn bottom_r(b: u8) -> u8 {
    (b >> 4) & 3
}

fn bottom_l(b: u8) -> u8 {
    (b >> 6) & 3
}

/// The 3 × 3 neighbourhood turned by `rot` quarter turns clockwise: a to i.
fn rotated(k: &Kernel, rot: usize) -> [u32; 9] {
    let [a, b, c, d, e, f, g, h, i] = [k.a, k.b, k.c, k.d, k.e, k.f, k.g, k.h, k.i];
    match rot {
        0 => [a, b, c, d, e, f, g, h, i],
        1 => [g, d, a, h, e, b, i, f, c],
        2 => [i, h, g, f, e, d, c, b, a],
        _ => [c, f, i, b, e, h, a, d, g],
    }
}

fn rotate_blend_info(b: u8, rot: usize) -> u8 {
    match rot {
        0 => b,
        1 => b.rotate_left(2),
        2 => b.rotate_left(4),
        _ => b.rotate_left(6),
    }
}

/// One of a scale's blends: the cell at row `i`, column `j` of the dot's
/// block (before the turn), set to the colour (`m` 0) or blended to `m / n`
/// of it.
type Step = (usize, usize, u32, u32);

/// A scale's five blends (xBRZ's `Scaler2x` to `Scaler6x`): along a shallow
/// line, a steep one, both, the diagonal, and a corner alone.
struct Blends {
    shallow: &'static [Step],
    steep: &'static [Step],
    steep_and_shallow: &'static [Step],
    diagonal: &'static [Step],
    corner: &'static [Step],
}

const SCALE2: Blends = Blends {
    shallow: &[(1, 0, 1, 4), (1, 1, 3, 4)],
    steep: &[(0, 1, 1, 4), (1, 1, 3, 4)],
    steep_and_shallow: &[(1, 0, 1, 4), (0, 1, 1, 4), (1, 1, 5, 6)],
    diagonal: &[(1, 1, 1, 2)],
    corner: &[(1, 1, 21, 100)],
};

const SCALE3: Blends = Blends {
    shallow: &[(2, 0, 1, 4), (1, 2, 1, 4), (2, 1, 3, 4), (2, 2, 0, 0)],
    steep: &[(0, 2, 1, 4), (2, 1, 1, 4), (1, 2, 3, 4), (2, 2, 0, 0)],
    steep_and_shallow: &[(2, 0, 1, 4), (0, 2, 1, 4), (2, 1, 3, 4), (1, 2, 3, 4), (2, 2, 0, 0)],
    diagonal: &[(1, 2, 1, 8), (2, 1, 1, 8), (2, 2, 7, 8)],
    corner: &[(2, 2, 45, 100)],
};

const SCALE4: Blends = Blends {
    shallow: &[(3, 0, 1, 4), (2, 2, 1, 4), (3, 1, 3, 4), (2, 3, 3, 4), (3, 2, 0, 0), (3, 3, 0, 0)],
    steep: &[(0, 3, 1, 4), (2, 2, 1, 4), (1, 3, 3, 4), (3, 2, 3, 4), (2, 3, 0, 0), (3, 3, 0, 0)],
    steep_and_shallow: &[(3, 1, 3, 4), (1, 3, 3, 4), (3, 0, 1, 4), (0, 3, 1, 4), (2, 2, 1, 3), (3, 3, 0, 0), (3, 2, 0, 0), (2, 3, 0, 0)],
    diagonal: &[(3, 2, 1, 2), (2, 3, 1, 2), (3, 3, 0, 0)],
    corner: &[(3, 3, 68, 100), (3, 2, 9, 100), (2, 3, 9, 100)],
};

const SCALE5: Blends = Blends {
    shallow: &[(4, 0, 1, 4), (3, 2, 1, 4), (2, 4, 1, 4), (4, 1, 3, 4), (3, 3, 3, 4), (4, 2, 0, 0), (4, 3, 0, 0), (4, 4, 0, 0), (3, 4, 0, 0)],
    steep: &[(0, 4, 1, 4), (2, 3, 1, 4), (4, 2, 1, 4), (1, 4, 3, 4), (3, 3, 3, 4), (2, 4, 0, 0), (3, 4, 0, 0), (4, 4, 0, 0), (4, 3, 0, 0)],
    steep_and_shallow: &[
        (0, 4, 1, 4),
        (2, 3, 1, 4),
        (1, 4, 3, 4),
        (4, 0, 1, 4),
        (3, 2, 1, 4),
        (4, 1, 3, 4),
        (3, 3, 2, 3),
        (2, 4, 0, 0),
        (3, 4, 0, 0),
        (4, 4, 0, 0),
        (4, 2, 0, 0),
        (4, 3, 0, 0),
    ],
    diagonal: &[(4, 2, 1, 8), (3, 3, 1, 8), (2, 4, 1, 8), (4, 3, 7, 8), (3, 4, 7, 8), (4, 4, 0, 0)],
    corner: &[(4, 4, 86, 100), (4, 3, 23, 100), (3, 4, 23, 100)],
};

const SCALE6: Blends = Blends {
    shallow: &[
        (5, 0, 1, 4),
        (4, 2, 1, 4),
        (3, 4, 1, 4),
        (5, 1, 3, 4),
        (4, 3, 3, 4),
        (3, 5, 3, 4),
        (5, 2, 0, 0),
        (5, 3, 0, 0),
        (5, 4, 0, 0),
        (5, 5, 0, 0),
        (4, 4, 0, 0),
        (4, 5, 0, 0),
    ],
    steep: &[
        (0, 5, 1, 4),
        (2, 4, 1, 4),
        (4, 3, 1, 4),
        (1, 5, 3, 4),
        (3, 4, 3, 4),
        (5, 3, 3, 4),
        (2, 5, 0, 0),
        (3, 5, 0, 0),
        (4, 5, 0, 0),
        (5, 5, 0, 0),
        (4, 4, 0, 0),
        (5, 4, 0, 0),
    ],
    steep_and_shallow: &[
        (0, 5, 1, 4),
        (2, 4, 1, 4),
        (1, 5, 3, 4),
        (3, 4, 3, 4),
        (5, 0, 1, 4),
        (4, 2, 1, 4),
        (5, 1, 3, 4),
        (4, 3, 3, 4),
        (2, 5, 0, 0),
        (3, 5, 0, 0),
        (4, 5, 0, 0),
        (5, 5, 0, 0),
        (4, 4, 0, 0),
        (5, 4, 0, 0),
        (5, 2, 0, 0),
        (5, 3, 0, 0),
    ],
    diagonal: &[(5, 3, 1, 2), (4, 4, 1, 2), (3, 5, 1, 2), (4, 5, 0, 0), (5, 5, 0, 0), (5, 4, 0, 0)],
    corner: &[(5, 5, 97, 100), (4, 5, 42, 100), (5, 4, 42, 100), (5, 3, 6, 100), (3, 5, 6, 100)],
};

/// The block's cell for row `i`, column `j` once turned `rot` quarter turns
/// (xBRZ's `MatrixRotation`): its row and column before the turn.
fn unrotate(i: usize, j: usize, n: usize, rot: usize) -> (usize, usize) {
    match rot {
        0 => (i, j),
        1 => (n - 1 - j, i),
        2 => (n - 1 - i, n - 1 - j),
        _ => (j, n - 1 - i),
    }
}

/// Blends one corner of the dot whose block starts at `origin` in `trg`, the
/// neighbourhood and its blend information turned by `rot` (xBRZ's `blendPixel`).
fn blend_pixel(blends: &Blends, scale: usize, kernel: &Kernel, trg: &mut [u32], origin: usize, trg_width: usize, blend_info: u8, rot: usize) {
    let [_a, b, c, d, e, f, g, h, i] = rotated(kernel, rot);
    let blend = rotate_blend_info(blend_info, rot);
    if bottom_r(blend) < BLEND_NORMAL {
        return;
    }
    let do_line_blend = if bottom_r(blend) >= BLEND_DOMINANT {
        true
    } else if (top_r(blend) != BLEND_NONE && !eq(e, g)) || (bottom_l(blend) != BLEND_NONE && !eq(e, c)) {
        // no second blending in an adjacent rotation for this dot, but for 90° corners
        false
    } else {
        // no full blending for L shapes: the corner alone
        !(!eq(e, i) && eq(g, h) && eq(h, i) && eq(i, f) && eq(f, c))
    };
    // the most similar colour
    let px = if dist(e, f) <= dist(e, h) { f } else { h };
    let steps = if do_line_blend {
        let fg = dist(f, g);
        let hc = dist(h, c);
        let shallow = STEEP_DIRECTION_THRESHOLD * fg <= hc && e != g && d != g;
        let steep = STEEP_DIRECTION_THRESHOLD * hc <= fg && e != c && b != c;
        match (shallow, steep) {
            (true, true) => blends.steep_and_shallow,
            (true, false) => blends.shallow,
            (false, true) => blends.steep,
            (false, false) => blends.diagonal,
        }
    } else {
        blends.corner
    };
    for &(si, sj, m, n) in steps {
        let (row, col) = unrotate(si, sj, scale, rot);
        let cell = &mut trg[origin + row * trg_width + col];
        *cell = if m == 0 { px } else { gradient(m, n, px, *cell) };
    }
}

/// `src` (`width` × `height`; red in the third byte, green the second, blue
/// the first, the fourth ignored) scaled `factor` times (2 to 6) by xBRZ, as
/// VPX 5436's `xbrz::scale(factor, …, ColorFormat::rgb)` scales it; None for
/// another factor.
pub fn scale(factor: usize, src: &[u32], width: usize, height: usize) -> Option<Vec<u32>> {
    let blends = match factor {
        2 => &SCALE2,
        3 => &SCALE3,
        4 => &SCALE4,
        5 => &SCALE5,
        6 => &SCALE6,
        _ => return None,
    };
    if width == 0 || height == 0 || src.len() < width * height {
        return None;
    }
    let trg_width = width * factor;
    let mut trg = vec![0u32; trg_width * height * factor];
    // each row's dots' corners, worked out a row ahead (xBRZ keeps them at the end of the
    // target picture; a buffer of their own here, which changes nothing)
    let mut pre_proc = vec![0u8; width];

    // the rows around `y`, borders taken from the nearest dot (`OobReaderDuplicate`)
    let row = |y: isize| y.clamp(0, height as isize - 1) as usize * width;
    let read_ponm = |k: &mut Kernel, rows: [usize; 4], x: isize| {
        let x2 = (x + 2).clamp(0, width as isize - 1) as usize;
        k.p = src[rows[0] + x2];
        k.o = src[rows[1] + x2];
        k.n = src[rows[2] + x2];
        k.m = src[rows[3] + x2];
    };
    let rows_at = |y: isize| [row(y - 1), row(y), row(y + 1), row(y + 2)];
    // the neighbourhood at x = -1: a, d, g, j at x = -4, then b, e, h, k, then c, f, i, l
    let start = |rows: [usize; 4]| {
        let mut k = Kernel::default();
        read_ponm(&mut k, rows, -4);
        (k.a, k.d, k.g, k.j) = (k.p, k.o, k.n, k.m);
        read_ponm(&mut k, rows, -3);
        (k.b, k.e, k.h, k.k) = (k.p, k.o, k.n, k.m);
        read_ponm(&mut k, rows, -2);
        (k.c, k.f, k.i, k.l) = (k.p, k.o, k.n, k.m);
        read_ponm(&mut k, rows, -1);
        k
    };
    let shift = |k: &mut Kernel| {
        (k.a, k.d, k.g, k.j) = (k.b, k.e, k.h, k.k);
        (k.b, k.e, k.h, k.k) = (k.c, k.f, k.i, k.l);
        (k.c, k.f, k.i, k.l) = (k.p, k.o, k.n, k.m);
    };

    // the first row's top corners, from the row above it
    {
        let rows = rows_at(-1);
        let mut k = start(rows);
        pre_proc[0] = pre_process_corners(&k).blend_i;
        for x in 0..width {
            shift(&mut k);
            read_ponm(&mut k, rows, x as isize);
            let res = pre_process_corners(&k);
            pre_proc[x] |= res.blend_h << 2;
            if x + 1 < width {
                pre_proc[x + 1] = res.blend_i;
            }
        }
    }

    for y in 0..height {
        let rows = rows_at(y as isize);
        let mut k = start(rows);
        // the corners for (x, y + 1), carried along the row
        let mut blend_xy1;
        {
            let res = pre_process_corners(&k);
            blend_xy1 = res.blend_i;
            pre_proc[0] |= res.blend_f << 6;
        }
        for x in 0..width {
            shift(&mut k);
            read_ponm(&mut k, rows, x as isize);
            // all four corners of (x, y) known now
            let mut blend_xy = pre_proc[x];
            let res = pre_process_corners(&k);
            blend_xy |= res.blend_e << 4;
            blend_xy1 |= res.blend_h << 2;
            pre_proc[x] = blend_xy1;
            if x + 1 < width {
                blend_xy1 = res.blend_i;
                pre_proc[x + 1] |= res.blend_f << 6;
            }
            // the dot's block in its colour, then its four corners blended
            let origin = factor * y * trg_width + x * factor;
            for r in 0..factor {
                trg[origin + r * trg_width..origin + r * trg_width + factor].fill(k.e);
            }
            if blend_xy != 0 {
                for rot in 0..4 {
                    blend_pixel(blends, factor, &k, &mut trg, origin, trg_width, blend_xy, rot);
                }
            }
        }
    }
    Some(trg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_picture_stays_flat_and_a_diagonal_gets_blended() {
        let flat = vec![0x00FF_8040u32; 8 * 6];
        for factor in 2..=6 {
            let out = scale(factor, &flat, 8, 6).unwrap();
            assert_eq!(out.len(), 8 * 6 * factor * factor);
            assert!(out.iter().all(|&p| p == 0x00FF_8040), "{factor}x");
        }
        // a white diagonal on black: its edges get dots between the two
        let diagonal: Vec<u32> = (0..64).map(|i| if i % 8 == i / 8 { 0x00FF_FFFF } else { 0 }).collect();
        let out = scale(3, &diagonal, 8, 8).unwrap();
        assert!(out.iter().any(|&p| p != 0 && p != 0x00FF_FFFF && p != 0xFF00_0000));
        assert!(scale(7, &diagonal, 8, 8).is_none());
    }

    #[test]
    fn colour_distance_reads_differences_as_vpx_does() {
        // the same colours: none; one lighter by 2 in red: small; one darker by 2: large, as VPX's table has it
        assert_eq!(dist(0x0010_2030, 0x0010_2030), 0.0);
        assert!(dist(0x0012_2030, 0x0010_2030) < 2.0);
        assert!(dist(0x0010_2030, 0x0012_2030) > 100.0);
    }
}
