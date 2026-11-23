// Ported from Visual Pinball X: src/shaders/bgfx/cs_mipmap.sc (license:GPLv3+) and
// src/renderer/Sampler.cpp's mipmap generation, commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's mipmaps (`Sampler::GetCoreTexture`'s compute path, `cs_mipmap`):
//! each level made from the one before at exactly half its size, rounded
//! down (`max(1, size >> level)`), every texel a 4×4 Kaiser kernel around
//! the 2×2 texels it covers, normalised over the texels inside the picture,
//! and stored in the picture's own format before the next level is made
//! from it. 8-bit sRGB pictures are weighed in linear light
//! (`cs_mipmap_srgba8`), 8-bit linear ones and half floats as they are
//! (`cs_mipmap_rgba8`, `cs_mipmap_rgba16f`).
//!
//! VPX makes them on the graphics card with a compute shader; SDL's own
//! mipmaps (a filtered blit per level) weigh a 2×2 box and, for a picture
//! whose sides aren't powers of two, drift up to a texel across each level,
//! which moved the playfield's fine print and the environment's reflections
//! from VPX's (The Addams Family 1.5% → 0.4% of texels from VPX's picture,
//! Monster Bash 2.5% → 1.0%, made so). These are made on the processor, the
//! big levels split across threads, on the threads that decode the pictures.

use std::sync::OnceLock;

/// VPX's 4×4 Kaiser kernel (`g_kaiserKernel4`), normalised, read as VPX
/// reads it: `[dx][dy]`.
const KAISER: [[f32; 4]; 4] = [
    [0.000099, 0.004880, 0.004880, 0.000099],
    [0.004880, 0.240141, 0.240141, 0.004880],
    [0.004880, 0.240141, 0.240141, 0.004880],
    [0.000099, 0.004880, 0.004880, 0.000099],
];

/// A level after the first: width, height, its texels as the graphics card
/// takes them.
pub type Level = (u32, u32, Vec<u8>);

/// How a picture's texels are stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Texels {
    /// 8 bits a channel, sRGB.
    Srgb8,
    /// 8 bits a channel, linear.
    Unorm8,
    /// Half floats, linear.
    Half,
}

/// The levels after the first of a picture `w` × `h` whose texels (RGBA)
/// are `bytes`, as VPX makes them.
pub fn chain(texels: Texels, bytes: &[u8], w: u32, h: u32) -> Vec<Level> {
    let levels = 32 - w.max(h).leading_zeros();
    let mut out: Vec<Level> = Vec::with_capacity(levels.saturating_sub(1) as usize);
    for level in 1..levels {
        let (sw, sh, src): (u32, u32, &[u8]) = match out.last() {
            Some((pw, ph, p)) => (*pw, *ph, p),
            None => (w, h, bytes),
        };
        let (dw, dh) = ((w >> level).max(1), (h >> level).max(1));
        let next = match texels {
            Texels::Srgb8 | Texels::Unorm8 => down_8(src, sw as usize, sh as usize, dw as usize, dh as usize, texels == Texels::Srgb8),
            Texels::Half => down_half(src, sw as usize, sh as usize, dw as usize, dh as usize),
        };
        out.push((dw, dh, next));
    }
    out
}

/// sRGB to linear light, as VPX's shader converts (`srgbToLinear`).
fn srgb_to_linear(f: f32) -> f32 {
    if f <= 0.04045 { f * (1.0 / 12.92) } else { (f * (1.0 / 1.055) + (0.055 / 1.055)).powf(2.4) }
}

/// Each 8-bit value's linear light (VPX's `srgbToLinear(imageLoad(…))`).
fn srgb_decode() -> &'static [f32; 256] {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|v| srgb_to_linear(v as f32 / 255.0)))
}

/// Linear light back to an 8-bit sRGB value, as VPX's `linearToSrgb` and
/// the store into an 8-bit texel round it: the value whose band holds it,
/// found from each value's lower edge rather than a power per texel.
struct SrgbEncoder {
    /// `edges[k]`: the least linear light that comes out as `k` or more.
    edges: [f32; 257],
    /// For each of `BUCKETS` slices of 0..1, the value at its lower end.
    start: Vec<u8>,
}

const BUCKETS: usize = 4096;

impl SrgbEncoder {
    fn get() -> &'static SrgbEncoder {
        static ENCODER: OnceLock<SrgbEncoder> = OnceLock::new();
        ENCODER.get_or_init(|| {
            let to_linear = |s: f64| if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) };
            let mut edges = [0f32; 257];
            edges[0] = f32::NEG_INFINITY;
            for (k, edge) in edges.iter_mut().enumerate().take(256).skip(1) {
                *edge = to_linear((k as f64 - 0.5) / 255.0) as f32;
            }
            edges[256] = f32::INFINITY;
            let start = (0..BUCKETS)
                .map(|b| {
                    let v = b as f32 / BUCKETS as f32;
                    (edges[1..256].iter().filter(|&&e| v >= e).count()) as u8
                })
                .collect();
            SrgbEncoder { edges, start }
        })
    }

    fn encode(&self, v: f32) -> u8 {
        if !(v > 0.0) {
            return 0;
        }
        if v >= 1.0 {
            return 255;
        }
        let mut k = self.start[(v * BUCKETS as f32) as usize] as usize;
        while v >= self.edges[k + 1] {
            k += 1;
        }
        k as u8
    }
}

/// A float into an 8-bit unsigned normalised texel, as the graphics card
/// stores it (clamped, nearest).
fn unorm8(f: f32) -> u8 {
    (f.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// The kernel's taps as VPX sums them (`dy` outer, `dx` inner) for a texel
/// whose 4×4 block lies inside the picture: the weights' sum.
fn full_weight() -> f32 {
    let mut weight = 0f32;
    for dy in 0..4 {
        for row in &KAISER {
            weight += row[dy];
        }
    }
    weight
}

/// The kernel over a 4×4 block that lies inside the picture, its left edge
/// at `sx`, from the four rows' linear values (`rows`, `sw` texels each),
/// summed in VPX's order (`dy` outer, `dx` inner), each channel apart; with
/// the kernel's whole weight, `full`.
#[inline(always)]
fn block_inside(rows: &[f32], sw: usize, sx: usize, full: f32) -> ([f32; 4], f32) {
    let mut c = [0f32; 4];
    for dy in 0..4 {
        let at = dy * sw * 4 + sx * 4;
        let block: &[f32; 16] = rows[at..at + 16].try_into().expect("16 values");
        let k = [KAISER[0][dy], KAISER[1][dy], KAISER[2][dy], KAISER[3][dy]];
        for ch in 0..4 {
            c[ch] += k[0] * block[ch];
            c[ch] += k[1] * block[4 + ch];
            c[ch] += k[2] * block[8 + ch];
            c[ch] += k[3] * block[12 + ch];
        }
    }
    (c, full)
}

/// The kernel over a 4×4 block reaching past the picture's edge (rows not
/// `present`, columns outside 0..`sw`), weighed over the texels inside.
fn block_edge(rows: &[f32], sw: usize, sx: isize, present: &[bool; 4]) -> ([f32; 4], f32) {
    let (mut c, mut weight) = ([0f32; 4], 0f32);
    for dy in 0..4 {
        if !present[dy] {
            continue;
        }
        let row = &rows[dy * sw * 4..(dy + 1) * sw * 4];
        for (dx, column) in KAISER.iter().enumerate() {
            let px = sx + dx as isize;
            if px < 0 || px >= sw as isize {
                continue;
            }
            let k = column[dy];
            let p = px as usize * 4;
            for ch in 0..4 {
                c[ch] += k * row[p + ch];
            }
            weight += k;
        }
    }
    (c, weight)
}

/// Splits `dst`'s rows (`dh` of `row_len` values) among threads when there
/// are many, calling `fill(first_row, rows)` for each part.
fn by_rows<T: Send>(dst: &mut [T], dh: usize, row_len: usize, fill: impl Fn(usize, &mut [T]) + Sync) {
    let threads = if dst.len() >= (1 << 20) { std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8) } else { 1 };
    if threads == 1 {
        fill(0, dst);
        return;
    }
    let rows_each = dh.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, part) in dst.chunks_mut(rows_each * row_len).enumerate() {
            let fill = &fill;
            s.spawn(move || fill(k * rows_each, part));
        }
    });
}

/// One 8-bit level from the one before (`cs_mipmap_srgba8` or `_rgba8`).
fn down_8(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize, srgb: bool) -> Vec<u8> {
    let decode = srgb_decode();
    let encoder = SrgbEncoder::get();
    let full = full_weight();
    let mut dst = vec![0u8; dw * dh * 4];
    by_rows(&mut dst, dh, dw * 4, |y0, part| {
        // the four source rows a destination row reads, in linear light
        let mut rows = vec![0f32; 4 * sw * 4];
        for (ry, out_row) in part.chunks_mut(dw * 4).enumerate() {
            let y = y0 + ry;
            let sy = y as isize * 2 - 1;
            let mut present = [false; 4];
            for (dy, slot) in present.iter_mut().enumerate() {
                let py = sy + dy as isize;
                if py < 0 || py >= sh as isize {
                    continue;
                }
                *slot = true;
                let line = &src[py as usize * sw * 4..(py as usize + 1) * sw * 4];
                let row = &mut rows[dy * sw * 4..(dy + 1) * sw * 4];
                for (t, f) in line.chunks_exact(4).zip(row.chunks_exact_mut(4)) {
                    if srgb {
                        f[0] = decode[t[0] as usize];
                        f[1] = decode[t[1] as usize];
                        f[2] = decode[t[2] as usize];
                    } else {
                        f[0] = t[0] as f32 / 255.0;
                        f[1] = t[1] as f32 / 255.0;
                        f[2] = t[2] as f32 / 255.0;
                    }
                    f[3] = t[3] as f32 / 255.0;
                }
            }
            let rows_inside = present.iter().all(|&p| p);
            for x in 0..dw {
                let sx = x as isize * 2 - 1;
                let inside = rows_inside && sx >= 0 && sx + 3 < sw as isize;
                let (c, weight) = if inside { block_inside(&rows, sw, sx as usize, full) } else { block_edge(&rows, sw, sx, &present) };
                let t = &mut out_row[x * 4..x * 4 + 4];
                for ch in 0..3 {
                    let v = c[ch] / weight;
                    t[ch] = if srgb { encoder.encode(v) } else { unorm8(v) };
                }
                t[3] = unorm8(c[3] / weight);
            }
        }
    });
    dst
}

/// Each half float's value.
fn half_decode() -> &'static [f32] {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    TABLE.get_or_init(|| (0..=u16::MAX).map(|b| half::f16::from_bits(b).to_f32()).collect())
}

/// One half-float level from the one before (`cs_mipmap_rgba16f`).
fn down_half(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let decode = half_decode();
    let full = full_weight();
    let texel = |line: &[u8], i: usize| decode[u16::from_le_bytes([line[i * 2], line[i * 2 + 1]]) as usize];
    let mut dst = vec![0u8; dw * dh * 8];
    by_rows(&mut dst, dh, dw * 8, |y0, part| {
        let mut rows = vec![0f32; 4 * sw * 4];
        for (ry, out_row) in part.chunks_mut(dw * 8).enumerate() {
            let y = y0 + ry;
            let sy = y as isize * 2 - 1;
            let mut present = [false; 4];
            for (dy, slot) in present.iter_mut().enumerate() {
                let py = sy + dy as isize;
                if py < 0 || py >= sh as isize {
                    continue;
                }
                *slot = true;
                let line = &src[py as usize * sw * 8..(py as usize + 1) * sw * 8];
                let row = &mut rows[dy * sw * 4..(dy + 1) * sw * 4];
                for (i, f) in row.iter_mut().enumerate() {
                    *f = texel(line, i);
                }
            }
            let rows_inside = present.iter().all(|&p| p);
            for x in 0..dw {
                let sx = x as isize * 2 - 1;
                let inside = rows_inside && sx >= 0 && sx + 3 < sw as isize;
                let (c, weight) = if inside { block_inside(&rows, sw, sx as usize, full) } else { block_edge(&rows, sw, sx, &present) };
                for ch in 0..4 {
                    let bits = half::f16::from_f32(c[ch] / weight).to_bits().to_le_bytes();
                    out_row[x * 8 + ch * 2] = bits[0];
                    out_row[x * 8 + ch * 2 + 1] = bits[1];
                }
            }
        }
    });
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VPX's shader, line for line: the reference the fast version is held to.
    fn reference_srgb(rgba: &[u8], w: u32, h: u32) -> Vec<Level> {
        let linear_to_srgb = |f: f32| if f <= 0.003_130_8 { 12.92 * f } else { 1.055 * f.powf(1.0 / 2.4) - 0.055 };
        let levels = 32 - w.max(h).leading_zeros();
        let mut out: Vec<Level> = Vec::new();
        for level in 1..levels {
            let (sw, sh, src): (u32, u32, Vec<u8>) = match out.last() {
                Some((pw, ph, p)) => (*pw, *ph, p.clone()),
                None => (w, h, rgba.to_vec()),
            };
            let (dw, dh) = ((w >> level).max(1), (h >> level).max(1));
            let mut dst = vec![0u8; (dw * dh * 4) as usize];
            for y in 0..dh as i32 {
                for x in 0..dw as i32 {
                    let (sx, sy) = (x * 2 - 1, y * 2 - 1);
                    let (mut color, mut weight) = ([0f32; 4], 0f32);
                    for dy in 0..4 {
                        for dx in 0..4 {
                            let (px, py) = (sx + dx, sy + dy);
                            if px >= 0 && py >= 0 && px < sw as i32 && py < sh as i32 {
                                let at = ((py as u32 * sw + px as u32) * 4) as usize;
                                let k = KAISER[dx as usize][dy as usize];
                                for c in 0..3 {
                                    color[c] += k * srgb_to_linear(src[at + c] as f32 / 255.0);
                                }
                                color[3] += k * (src[at + 3] as f32 / 255.0);
                                weight += k;
                            }
                        }
                    }
                    let at = ((y as u32 * dw + x as u32) * 4) as usize;
                    for c in 0..3 {
                        dst[at + c] = unorm8(linear_to_srgb(color[c] / weight));
                    }
                    dst[at + 3] = unorm8(color[3] / weight);
                }
            }
            out.push((dw, dh, dst));
        }
        out
    }

    fn noise(len: usize, seed: u32) -> Vec<u8> {
        let mut s = seed;
        (0..len)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (s >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn levels_halve_rounding_down_to_one_texel() {
        let sizes = |w: u32, h: u32| chain(Texels::Srgb8, &vec![0u8; (w * h * 4) as usize], w, h).iter().map(|(w, h, _)| (*w, *h)).collect::<Vec<_>>();
        assert_eq!(sizes(5, 3), vec![(2, 1), (1, 1)]);
        assert_eq!(sizes(8, 2), vec![(4, 1), (2, 1), (1, 1)]);
        assert_eq!(sizes(1803, 4094).len(), 11);
        assert!(sizes(1, 1).is_empty());
    }

    #[test]
    fn a_flat_picture_stays_flat() {
        let rgba: Vec<u8> = [200u8, 100, 50, 255].repeat(16 * 16);
        for (_, _, level) in chain(Texels::Srgb8, &rgba, 16, 16) {
            assert!(level.chunks(4).all(|t| t == [200, 100, 50, 255]));
        }
    }

    #[test]
    fn matches_vpxs_shader_on_odd_sizes() {
        for (w, h, seed) in [(37u32, 21u32, 1u32), (64, 64, 2), (3, 50, 3), (129, 2, 4)] {
            let rgba = noise((w * h * 4) as usize, seed);
            let fast = chain(Texels::Srgb8, &rgba, w, h);
            let slow = reference_srgb(&rgba, w, h);
            assert_eq!(fast.len(), slow.len());
            for ((fw, fh, f), (sw, sh, s)) in fast.iter().zip(&slow) {
                assert_eq!((fw, fh), (sw, sh));
                // the same but for a rounding at a band's very edge
                let off = f.iter().zip(s).filter(|(a, b)| a.abs_diff(**b) > 1).count();
                assert_eq!(off, 0, "{w}×{h}: {off} texels more than one apart");
            }
        }
    }

    #[test]
    fn the_encoder_rounds_as_the_shader_does() {
        let encoder = SrgbEncoder::get();
        let linear_to_srgb = |f: f64| if f <= 0.003_130_8 { 12.92 * f } else { 1.055 * f.powf(1.0 / 2.4) - 0.055 };
        for i in 0..=100_000 {
            let v = i as f32 / 100_000.0;
            let want = (linear_to_srgb(v as f64).clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8;
            assert!(encoder.encode(v).abs_diff(want) <= 1, "{v}");
        }
        assert_eq!(encoder.encode(0.0), 0);
        assert_eq!(encoder.encode(1.0), 255);
        assert_eq!(encoder.encode(2.0), 255);
    }

    #[test]
    fn half_floats_keep_their_range() {
        let px: Vec<u8> = [4.0f32, 0.5, 70000.0, 1.0].iter().flat_map(|&v| half::f16::from_f32(v).to_bits().to_le_bytes()).collect();
        let rgba = px.repeat(4 * 4);
        let chain = chain(Texels::Half, &rgba, 4, 4);
        let first = &chain[0].2;
        let v = |i: usize| half::f16::from_bits(u16::from_le_bytes([first[i * 2], first[i * 2 + 1]])).to_f32();
        assert_eq!((v(0), v(1), v(3)), (4.0, 0.5, 1.0));
        assert!(v(2).is_infinite() || v(2) >= 65504.0);
    }
}
