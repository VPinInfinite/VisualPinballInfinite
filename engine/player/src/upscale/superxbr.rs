// SPDX-License-Identifier: MIT
//
// Ported from Visual Pinball X build 5436 (commit af26b2d),
// plugins/upscaledmd/super-xbr/super-xbr.h, by the Visual Pinball Infinite
// contributors, 2026. Super-xBR's own notice:
//
// *******  Super XBR Scaler  *******
//
// Copyright (c) 2016 Hyllian - sergiogdb@gmail.com
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Super-xBR (Hyllian's scaler, the fast shader version), doubling a picture
//! in three passes: the new dots on the diagonals, then those between them,
//! then every dot smoothed along the edges it finds, each pass clamped to
//! the colours around it so edges don't ring.

fn red(c: u32) -> f32 {
    (c & 0xFF) as f32
}

fn green(c: u32) -> f32 {
    ((c >> 8) & 0xFF) as f32
}

fn blue(c: u32) -> f32 {
    ((c >> 16) & 0xFF) as f32
}

const WGT1: f64 = 0.129633;
const WGT2: f64 = 0.175068;
const W1: f32 = -WGT1 as f32;
const W2: f32 = (WGT1 + 0.5) as f32;
const W3: f32 = -WGT2 as f32;
const W4: f32 = (WGT2 + 0.5) as f32;

fn df(a: f32, b: f32) -> f32 {
    (a - b).abs()
}

// Super-xBR's own min and max: the first unless the second is lower, higher
fn min(x: f32, y: f32) -> f32 {
    if x < y { x } else { y }
}

fn max(x: f32, y: f32) -> f32 {
    if x < y { y } else { x }
}

fn min4(a: f32, b: f32, c: f32, d: f32) -> f32 {
    min(min(a, b), min(c, d))
}

fn max4(a: f32, b: f32, c: f32, d: f32) -> f32 {
    max(max(a, b), max(c, d))
}

fn clamp(x: f32, floor: f32, ceil: f32) -> f32 {
    max(min(x, ceil), floor)
}

/// A 4 × 4 neighbourhood, by x then y.
type Mat = [[f32; 4]; 4];

fn diagonal_edge(mat: &Mat, wp: &[f32; 6]) -> f32 {
    let dw1 = wp[0] * (df(mat[0][2], mat[1][1]) + df(mat[1][1], mat[2][0]) + df(mat[1][3], mat[2][2]) + df(mat[2][2], mat[3][1]))
        + wp[1] * (df(mat[0][3], mat[1][2]) + df(mat[2][1], mat[3][0]))
        + wp[2] * (df(mat[0][3], mat[2][1]) + df(mat[1][2], mat[3][0]))
        + wp[3] * df(mat[1][2], mat[2][1])
        + wp[4] * (df(mat[0][2], mat[2][0]) + df(mat[1][3], mat[3][1]))
        + wp[5] * (df(mat[0][1], mat[1][0]) + df(mat[2][3], mat[3][2]));

    let dw2 = wp[0] * (df(mat[0][1], mat[1][2]) + df(mat[1][2], mat[2][3]) + df(mat[1][0], mat[2][1]) + df(mat[2][1], mat[3][2]))
        + wp[1] * (df(mat[0][0], mat[1][1]) + df(mat[2][2], mat[3][3]))
        + wp[2] * (df(mat[0][0], mat[2][2]) + df(mat[1][1], mat[3][3]))
        + wp[3] * df(mat[1][1], mat[2][2])
        + wp[4] * (df(mat[1][0], mat[3][2]) + df(mat[0][1], mat[2][3]))
        + wp[5] * (df(mat[0][2], mat[1][3]) + df(mat[2][0], mat[3][1]));

    dw1 - dw2
}

/// The neighbourhood's channels and brightness.
struct Samples {
    r: Mat,
    g: Mat,
    b: Mat,
    y: Mat,
}

impl Samples {
    /// Each of the 16 dots `at` gives, by its place in the 4 × 4.
    fn gather(at: impl Fn(usize, usize) -> u32) -> Samples {
        let mut s = Samples { r: [[0.0; 4]; 4], g: [[0.0; 4]; 4], b: [[0.0; 4]; 4], y: [[0.0; 4]; 4] };
        for x in 0..4 {
            for y in 0..4 {
                let sample = at(x, y);
                s.r[x][y] = red(sample);
                s.g[x][y] = green(sample);
                s.b[x][y] = blue(sample);
                s.y[x][y] = 0.2126 * s.r[x][y] + 0.7152 * s.g[x][y] + 0.0722 * s.b[x][y];
            }
        }
        s
    }

    /// The new dot: one diagonal's weighted blend or the other's, by which edge
    /// is stronger, clamped to the range of `range`'s four middle dots (its
    /// own, but for the second pass's second dot, which VPX clamps to the
    /// first's), opaque.
    fn blend(&self, wp: &[f32; 6], (w_outer, w_inner): (f32, f32), range: &Samples) -> u32 {
        let middle = |m: &Mat, pick: fn(f32, f32, f32, f32) -> f32| pick(m[1][1], m[2][1], m[1][2], m[2][2]);
        let d_edge = diagonal_edge(&self.y, wp);
        let one = |m: &Mat| w_outer * (m[0][3] + m[3][0]) + w_inner * (m[1][2] + m[2][1]);
        let two = |m: &Mat| w_outer * (m[0][0] + m[3][3]) + w_inner * (m[1][1] + m[2][2]);
        let channel = |m: &Mat, r: &Mat| {
            let f = if d_edge <= 0.0 { one(m) } else { two(m) };
            // anti-ringing, clamp
            let f = clamp(f, middle(r, min4), middle(r, max4));
            clamp(f, 0.0, 255.0).ceil() as i32 as u32
        };
        let (ri, gi, bi) = (channel(&self.r, &range.r), channel(&self.g, &range.g), channel(&self.b, &range.b));
        (255 << 24) | (bi << 16) | (gi << 8) | ri
    }
}

/// `data` (`w` × `h`, red in the lowest byte, then green and blue; alpha
/// ignored and made opaque) twice as wide and high, as VPX 5436's
/// `superxbr::scale<2, false>` scales it.
pub fn scale2x(data: &[u32], w: usize, h: usize) -> Vec<u32> {
    const F: i32 = 2;
    let (wi, hi) = (w as i32, h as i32);
    let (outw, outh) = (wi * F, hi * F);
    let mut out = vec![0u32; (outw * outh) as usize];
    let clampi = |v: i32, lo: i32, hi: i32| v.min(hi).max(lo);

    // first pass
    {
        const WP: [f32; 6] = [2.0, 1.0, -1.0, 4.0, -1.0, 1.0];
        let mut y = 0;
        while y < outh {
            let mut x = 0;
            while x < outw {
                // the middle dots in the original picture
                let (cx, cy) = (x / F, y / F);
                let s = Samples::gather(|sx, sy| {
                    let csy = clampi(sy as i32 - 1 + cy, 0, hi - 1);
                    let csx = clampi(sx as i32 - 1 + cx, 0, wi - 1);
                    data[(csy * wi + csx) as usize]
                });
                let ofs = (y * outw + x) as usize;
                let original = data[(cy * wi + cx) as usize];
                out[ofs] = original;
                out[ofs + 1] = original;
                out[ofs + outw as usize] = original;
                out[ofs + outw as usize + 1] = s.blend(&WP, (W1, W2), &s);
                x += F;
            }
            y += F;
        }
    }

    // second pass
    {
        const WP: [f32; 6] = [2.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut y = 0;
        while y < outh {
            let mut x = 0;
            while x < outw {
                let s = Samples::gather(|sx, sy| {
                    let (sx, sy) = (sx as i32 - 1, sy as i32 - 1);
                    let csy = clampi(sx - sy + y, 0, F * hi - 1);
                    let csx = clampi(sx + sy + x, 0, F * wi - 1);
                    out[(csy * outw + csx) as usize]
                });
                out[(y * outw + x + 1) as usize] = s.blend(&WP, (W3, W4), &s);

                let second = Samples::gather(|sx, sy| {
                    let (sx, sy) = (sx as i32 - 1, sy as i32 - 1);
                    let csy = clampi(sx - sy + 1 + y, 0, F * hi - 1);
                    let csx = clampi(sx + sy - 1 + x, 0, F * wi - 1);
                    out[(csy * outw + csx) as usize]
                });
                // (clamped to the first dot's range: VPX doesn't take the second's)
                out[((y + 1) * outw + x) as usize] = second.blend(&WP, (W3, W4), &s);
                x += F;
            }
            y += F;
        }
    }

    // third pass, from the last dot back
    {
        const WP: [f32; 6] = [2.0, 1.0, -1.0, 4.0, -1.0, 1.0];
        for y in (0..outh).rev() {
            for x in (0..outw).rev() {
                let s = Samples::gather(|sx, sy| {
                    let csy = clampi(sy as i32 - 2 + y, 0, F * hi - 1);
                    let csx = clampi(sx as i32 - 2 + x, 0, F * wi - 1);
                    out[(csy * outw + csx) as usize]
                });
                out[(y * outw + x) as usize] = s.blend(&WP, (W1, W2), &s);
            }
        }
    }
    out
}
