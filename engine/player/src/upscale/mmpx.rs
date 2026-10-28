// SPDX-License-Identifier: MIT
//
// Ported from Visual Pinball X build 5436 (commit af26b2d),
// plugins/upscaledmd/mmpx/mmpx.h, by the Visual Pinball Infinite
// contributors, 2026. MMPX's own notice:
//
// Copyright 2020 Morgan McGuire & Mara Gagiu.
// Available under the MIT license.
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

//! MMPX (Morgan McGuire and Mara Gagiu's pixel-art scaler): each dot becomes
//! four, each of the four taking the dot's own colour or a neighbour's where
//! an edge runs through it — no new colours are made.

/// A dot's brightness as MMPX weighs it: its three channels and one, times
/// how see-through it is.
fn luma(color: u32) -> u32 {
    let alpha = (color & 0xFF00_0000) >> 24;
    (((color & 0x00FF_0000) >> 16) + ((color & 0x0000_FF00) >> 8) + (color & 0x0000_00FF) + 1) * (256 - alpha)
}

fn all_eq2(b: u32, a0: u32, a1: u32) -> bool {
    ((b ^ a0) | (b ^ a1)) == 0
}

fn all_eq3(b: u32, a0: u32, a1: u32, a2: u32) -> bool {
    ((b ^ a0) | (b ^ a1) | (b ^ a2)) == 0
}

fn all_eq4(b: u32, a0: u32, a1: u32, a2: u32, a3: u32) -> bool {
    ((b ^ a0) | (b ^ a1) | (b ^ a2) | (b ^ a3)) == 0
}

fn any_eq3(b: u32, a0: u32, a1: u32, a2: u32) -> bool {
    b == a0 || b == a1 || b == a2
}

fn none_eq2(b: u32, a0: u32, a1: u32) -> bool {
    b != a0 && b != a1
}

fn none_eq4(b: u32, a0: u32, a1: u32, a2: u32, a3: u32) -> bool {
    b != a0 && b != a1 && b != a2 && b != a3
}

/// `src` (`width` × `height`, a colour a dot: red in the lowest byte, then
/// green, blue and alpha) twice as wide and high, as MMPX scales it.
pub fn scale2x(src: &[u32], width: usize, height: usize) -> Vec<u32> {
    let mut dst = vec![0u32; width * height * 4];
    let (w, max_x, max_y) = (width as i32, width as i32 - 1, height as i32 - 1);
    // a dot, clamped to the border
    let at = |mut x: i32, mut y: i32| -> u32 {
        if x as u32 > max_x as u32 || y as u32 > max_y as u32 {
            x = x.min(max_x).max(0);
            y = y.min(max_y).max(0);
        }
        src[(y * w + x) as usize]
    };
    for src_y in 0..height as i32 {
        let mut src_x = 0;

        // inputs carried along rows
        let mut a = at(src_x - 1, src_y - 1);
        let mut b = at(src_x, src_y - 1);
        let mut c = at(src_x + 1, src_y - 1);

        let mut d = at(src_x - 1, src_y);
        let mut e = at(src_x, src_y);
        let mut f = at(src_x + 1, src_y);

        let mut g = at(src_x - 1, src_y + 1);
        let mut h = at(src_x, src_y + 1);
        let mut i = at(src_x + 1, src_y + 1);

        let mut q = at(src_x - 2, src_y);
        let mut r = at(src_x + 2, src_y);

        while src_x < w {
            // outputs
            let (mut j, mut k, mut l, mut m) = (e, e, e, e);

            if ((a ^ e) | (b ^ e) | (c ^ e) | (d ^ e) | (f ^ e) | (g ^ e) | (h ^ e) | (i ^ e)) != 0 {
                let (p, s) = (at(src_x, src_y - 2), at(src_x, src_y + 2));
                let (bl, dl, el, fl, hl) = (luma(b), luma(d), luma(e), luma(f), luma(h));

                // 1:1 slope rules
                if (d == b && d != h && d != f) && (el >= dl || e == a) && any_eq3(e, a, c, g) && ((el < dl) || a != d || e != p || e != q) {
                    j = d;
                }
                if (b == f && b != d && b != h) && (el >= bl || e == c) && any_eq3(e, a, c, i) && ((el < bl) || c != b || e != p || e != r) {
                    k = b;
                }
                if (h == d && h != f && h != b) && (el >= hl || e == g) && any_eq3(e, a, g, i) && ((el < hl) || g != h || e != s || e != q) {
                    l = h;
                }
                if (f == h && f != b && f != d) && (el >= fl || e == i) && any_eq3(e, c, g, i) && ((el < fl) || i != h || e != r || e != s) {
                    m = f;
                }

                // intersection rules
                if (e != f && all_eq4(e, c, i, d, q) && all_eq2(f, b, h)) && (f != at(src_x + 3, src_y)) {
                    k = f;
                    m = f;
                }
                if (e != d && all_eq4(e, a, g, f, r) && all_eq2(d, b, h)) && (d != at(src_x - 3, src_y)) {
                    j = d;
                    l = d;
                }
                if (e != h && all_eq4(e, g, i, b, p) && all_eq2(h, d, f)) && (h != at(src_x, src_y + 3)) {
                    l = h;
                    m = h;
                }
                if (e != b && all_eq4(e, a, c, h, s) && all_eq2(b, d, f)) && (b != at(src_x, src_y - 3)) {
                    j = b;
                    k = b;
                }
                if bl < el && all_eq4(e, g, h, i, s) && none_eq4(e, a, d, c, f) {
                    j = b;
                    k = b;
                }
                if hl < el && all_eq4(e, a, b, c, p) && none_eq4(e, d, g, i, f) {
                    l = h;
                    m = h;
                }
                if fl < el && all_eq4(e, a, d, g, q) && none_eq4(e, b, c, i, h) {
                    k = f;
                    m = f;
                }
                if dl < el && all_eq4(e, c, f, i, r) && none_eq4(e, b, a, g, h) {
                    j = d;
                    l = d;
                }

                // 2:1 slope rules
                if h != b {
                    if h != a && h != e && h != c {
                        if all_eq3(h, g, f, r) && none_eq2(h, d, at(src_x + 2, src_y - 1)) {
                            l = m;
                        }
                        if all_eq3(h, i, d, q) && none_eq2(h, f, at(src_x - 2, src_y - 1)) {
                            m = l;
                        }
                    }
                    if b != i && b != g && b != e {
                        if all_eq3(b, a, f, r) && none_eq2(b, d, at(src_x + 2, src_y + 1)) {
                            j = k;
                        }
                        if all_eq3(b, c, d, q) && none_eq2(b, f, at(src_x - 2, src_y + 1)) {
                            k = j;
                        }
                    }
                }
                if f != d {
                    if d != i && d != e && d != c {
                        if all_eq3(d, a, h, s) && none_eq2(d, b, at(src_x + 1, src_y + 2)) {
                            j = l;
                        }
                        if all_eq3(d, g, b, p) && none_eq2(d, h, at(src_x + 1, src_y - 2)) {
                            l = j;
                        }
                    }
                    if f != e && f != a && f != g {
                        if all_eq3(f, c, h, s) && none_eq2(f, b, at(src_x - 1, src_y + 2)) {
                            k = m;
                        }
                        if all_eq3(f, i, b, p) && none_eq2(f, h, at(src_x - 1, src_y - 2)) {
                            m = k;
                        }
                    }
                }
            }

            let index = (2 * src_x + (src_y << 2) * w) as usize;
            dst[index] = j;
            dst[index + 1] = k;
            dst[index + 2 * width] = l;
            dst[index + 2 * width + 1] = m;

            a = b;
            b = c;
            c = at(src_x + 2, src_y - 1);
            q = d;
            d = e;
            e = f;
            f = r;
            r = at(src_x + 3, src_y);
            g = h;
            h = i;
            i = at(src_x + 2, src_y + 1);
            src_x += 1;
        }
    }
    dst
}
