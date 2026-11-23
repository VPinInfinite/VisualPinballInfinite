// SPDX-License-Identifier: GPL-3.0-or-later AND MIT
//
// Ported from Visual Pinball X build 5436 (commit af26b2d),
// plugins/upscaledmd/scalefx/scalefx.h — its form without `NEW_SCALEFX`, the
// one VPX builds — by the Visual Pinball Infinite contributors, 2026. VPX's
// C++ form (added in 2025 by VPX's team, GPL-3.0-or-later as VPX is) follows
// ScaleFX by Sp00kyFox, whose notice is:
//
// Copyright (c) 2016 Sp00kyFox - ScaleFX@web.de
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

//! ScaleFX (Sp00kyFox's edge interpolation for pixel art), as VPX 5436 runs
//! it on the DMD: how alike each dot is to its neighbours, the strength of
//! each corner, the edges and their direction, then each dot split into
//! 3 × 3 smaller ones, each taking the colour of the dot or of one of its
//! neighbours up to two away. No new colours are made — except in the
//! anti-aliasing form, which averages the nine back into one dot.

#[derive(Clone, Copy, Default)]
struct V4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

const fn v4(x: f32, y: f32, z: f32, w: f32) -> V4 {
    V4 { x, y, z, w }
}

#[derive(Clone, Copy, Default)]
struct B4 {
    x: bool,
    y: bool,
    z: bool,
    w: bool,
}

const fn b4(x: bool, y: bool, z: bool, w: bool) -> B4 {
    B4 { x, y, z, w }
}

// ScaleFX's own max: the first unless the second is higher
fn max(x: f32, y: f32) -> f32 {
    if x < y { y } else { x }
}

/// How alike two colours are (1 the same), weighing red, green and blue as
/// the eye does.
fn eq_col(ad: u32, bd: u32) -> f32 {
    let channels = |c: u32| {
        [
            (c & 255) as f32 * (1.0 / 255.0f64) as f32,
            (c & 65280) as f32 * (1.0 / 65280.0f64) as f32,
            (c & 16_711_680) as f32 * (1.0 / 16_711_680.0f64) as f32,
        ]
    };
    let (a, b) = (channels(ad), channels(bd));
    let r = 0.5 * (a[0] + b[0]);
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    // dot(c * d, d)
    let tmp = (2.0 + r) * d[0] * d[0] + 4.0 * d[1] * d[1] + (3.0 - r) * d[2] * d[2];
    let res = tmp.sqrt() * (1.0 / 3.0f64) as f32;
    1.0 - res
}

/// How alike two brightnesses are (1 the same).
fn eq_brightness(ad: u32, bd: u32) -> f32 {
    let res = (ad as i32 - bd as i32).abs() as f32 * (1.0 / 255.0f64) as f32;
    1.0 - res
}

const SFX_CLR: f64 = 0.35;
const SFX_THR: f32 = (1.0 - SFX_CLR) as f32;

/// Corner strength.
fn str(crn: V4, ort: V4) -> V4 {
    v4(
        if crn.x > SFX_THR { max(2.0 * crn.x - (ort.x + ort.w), 0.0) } else { 0.0 },
        if crn.y > SFX_THR { max(2.0 * crn.y - (ort.y + ort.x), 0.0) } else { 0.0 },
        if crn.z > SFX_THR { max(2.0 * crn.z - (ort.z + ort.y), 0.0) } else { 0.0 },
        if crn.w > SFX_THR { max(2.0 * crn.w - (ort.w + ort.z), 0.0) } else { 0.0 },
    )
}

/// Corner dominance at junctions.
fn dom(strx: V4, stry: V4, strz: V4, strw: V4) -> V4 {
    v4(
        max(2.0 * strx.z - (strx.y + strx.w), 0.0),
        max(2.0 * stry.w - (stry.z + stry.x), 0.0),
        max(2.0 * strz.x - (strz.w + strz.y), 0.0),
        max(2.0 * strw.y - (strw.x + strw.z), 0.0),
    )
}

/// The necessary but not sufficient junction condition for orthogonal edges.
fn clear(crn: (f32, f32), ort: V4) -> bool {
    (crn.0 <= SFX_THR || crn.0 <= ort.x || crn.0 <= ort.w)
        && (crn.1 <= SFX_THR || crn.1 <= ort.y || crn.1 <= ort.x)
        && (crn.0 <= SFX_THR || crn.0 <= ort.z || crn.0 <= ort.y)
        && (crn.1 <= SFX_THR || crn.1 <= ort.w || crn.1 <= ort.z)
}

/// A majority vote for ambiguous dominance junctions.
fn ambi_dom(j: V4) -> B4 {
    b4(
        j.x != 0.0 && j.x + j.z > j.y + j.w,
        j.y != 0.0 && j.y + j.w > j.z + j.x,
        j.z != 0.0 && j.z + j.x > j.w + j.y,
        j.w != 0.0 && j.w + j.y > j.x + j.z,
    )
}

/// `data` (`xres` × `yres`: a brightness 0 to 255 a dot when
/// `brightness`, else a colour, red in the lowest byte, then green, blue
/// and alpha), as VPX 5436's `scalefx::upscale` makes it: three times as wide
/// and high when `upscale`, else the same size, anti-aliased. The picture
/// must be at least 4 × 4.
pub fn upscale(data: &[u32], xres: usize, yres: usize, brightness: bool, upscale: bool) -> Vec<u32> {
    let n = xres * yres;
    let mut metric = vec![V4::default(); n];

    // how alike each dot is to the ones up-left, up, up-right and right of it; beyond the
    // picture's border, a dot half as bright as the border's
    let mut o = 0;
    for j in 0..yres {
        for i in 0..xres {
            let e = data[o];
            let e2 = if brightness { (e & 0x00FE_FEFE) >> 1 } else { (e & 0xFEFE_FEFE) >> 1 };
            let up = |di: isize| data[((j as isize - 1) * xres as isize + i as isize + di) as usize];
            let a = if j == 0 || i == 0 { e2 } else { up(-1) };
            let b = if j == 0 { e2 } else { up(0) };
            let c = if j == 0 || i == xres - 1 { e2 } else { up(1) };
            let f = if i == xres - 1 { e2 } else { data[j * xres + i + 1] };
            let eq = if brightness { eq_brightness } else { eq_col };
            metric[o] = v4(eq(e, a), eq(e, b), eq(e, c), eq(e, f));
            o += 1;
        }
    }

    let mut g_res = vec![B4::default(); n];
    let mut g_hori = vec![B4::default(); n];
    let mut g_vert = vec![B4::default(); n];
    let mut g_or = vec![B4::default(); n];

    o = 0;
    for j in 0..yres {
        let jm1 = if j == 0 { 0 } else { (j - 1) * xres };
        let jp1 = (if j == yres - 1 { yres - 1 } else { j + 1 }) * xres;
        let jp2 = (if j >= yres - 2 { yres - 1 } else { j + 2 }) * xres;

        for i in 0..xres {
            let im2 = if i <= 1 { 0 } else { i - 2 };
            let im1 = if i == 0 { 0 } else { i - 1 };
            let ip1 = if i == xres - 1 { xres - 1 } else { i + 1 };
            let ip2 = if i >= xres - 2 { xres - 1 } else { i + 2 };

            let m = metric[jm1 + im2];
            let a = metric[jm1 + im1];
            let b = metric[jm1 + i];
            let c = metric[jm1 + ip1];
            let p = metric[jm1 + ip2];

            let nn = metric[j * xres + im2];
            let d = metric[j * xres + im1];
            let e = metric[o];
            let f = metric[j * xres + ip1];
            let q = metric[j * xres + ip2];

            let oo = metric[jp1 + im2];
            let g = metric[jp1 + im1];
            let h = metric[jp1 + i];
            let ii = metric[jp1 + ip1];
            let r = metric[jp1 + ip2];

            let jj = metric[jp2 + im1];
            let k = metric[jp2 + i];
            let l = metric[jp2 + ip1];

            let as_ = str(v4(m.z, b.x, d.z, d.x), v4(a.y, a.w, d.y, m.w));
            let bs = str(v4(a.z, c.x, e.z, e.x), v4(b.y, b.w, e.y, a.w));
            let cs = str(v4(b.z, p.x, f.z, f.x), v4(c.y, c.w, f.y, b.w));
            let ds = str(v4(nn.z, e.x, g.z, g.x), v4(d.y, d.w, g.y, nn.w));
            let es = str(v4(d.z, f.x, h.z, h.x), v4(e.y, e.w, h.y, d.w));
            let fs = str(v4(e.z, q.x, ii.z, ii.x), v4(f.y, f.w, ii.y, e.w));
            let gs = str(v4(oo.z, h.x, jj.z, jj.x), v4(g.y, g.w, jj.y, oo.w));
            let hs = str(v4(g.z, ii.x, k.z, k.x), v4(h.y, h.w, k.y, g.w));
            let is = str(v4(h.z, r.x, l.z, l.x), v4(ii.y, ii.w, l.y, h.w));

            // strength & dominance junctions
            let (jsx, jdx) = (v4(as_.z, bs.w, es.x, ds.y), dom(as_, bs, es, ds));
            let (jsy, jdy) = (v4(bs.z, cs.w, fs.x, es.y), dom(bs, cs, fs, es));
            let (jsz, jdz) = (v4(es.z, fs.w, is.x, hs.y), dom(es, fs, is, hs));
            let (jsw, jdw) = (v4(ds.z, es.w, hs.x, gs.y), dom(ds, es, hs, gs));

            // majority vote for ambiguous dominance junctions
            let (jx, jy, jz, jw) = (ambi_dom(jdx), ambi_dom(jdy), ambi_dom(jdz), ambi_dom(jdw));

            // inject strength without creating new contradictions
            let mut res = B4 {
                x: jx.z || (!(jx.y || jx.w) && (jsx.z != 0.0) && (jx.x || (jsx.x + jsx.z > jsx.y + jsx.w))),
                y: jy.w || (!(jy.z || jy.x) && (jsy.w != 0.0) && (jy.y || (jsy.y + jsy.w > jsy.x + jsy.z))),
                z: jz.x || (!(jz.w || jz.y) && (jsz.x != 0.0) && (jz.z || (jsz.x + jsz.z > jsz.y + jsz.w))),
                w: jw.y || (!(jw.x || jw.z) && (jsw.y != 0.0) && (jw.w || (jsw.y + jsw.w > jsw.x + jsw.z))),
            };

            // single dot & end of line detection, each test seeing the ones before it
            res.x = res.x && (jx.z || !(res.w && res.y));
            res.y = res.y && (jy.w || !(res.x && res.z));
            res.z = res.z && (jz.x || !(res.y && res.w));
            res.w = res.w && (jw.y || !(res.z && res.x));

            // output
            let clr = b4(
                clear((d.z, e.x), v4(a.w, e.y, d.w, d.y)),
                clear((e.z, f.x), v4(b.w, f.y, e.w, e.y)),
                clear((h.z, ii.x), v4(e.w, ii.y, h.w, h.y)),
                clear((g.z, h.x), v4(d.w, h.y, g.w, g.y)),
            );

            let low = v4(max(e.y, SFX_THR), max(e.w, SFX_THR), max(h.y, SFX_THR), max(d.w, SFX_THR));

            // horizontal edges, vertical edges, orientation
            g_hori[o] = b4(
                low.x < max(d.w, a.w) && clr.x,
                low.x < max(e.w, b.w) && clr.y,
                low.z < max(e.w, h.w) && clr.z,
                low.z < max(d.w, g.w) && clr.w,
            );
            g_vert[o] = b4(
                low.w < max(e.y, d.y) && clr.x,
                low.y < max(e.y, f.y) && clr.y,
                low.y < max(h.y, ii.y) && clr.z,
                low.w < max(h.y, g.y) && clr.w,
            );
            g_or[o] = b4(a.w < d.y, b.w <= f.y, h.w < ii.y, g.w <= g.y);
            g_res[o] = res;
            o += 1;
        }
    }

    // each dot's subdots: which neighbour each takes its colour from
    let mut picks = vec![0u32; n];
    o = 0;
    for j in 0..yres {
        let jm3 = if j <= 2 { 0 } else { (j - 3) * xres };
        let jm2 = if j <= 1 { 0 } else { (j - 2) * xres };
        let jm1 = if j == 0 { 0 } else { (j - 1) * xres };
        let jp1 = (if j == yres - 1 { yres - 1 } else { j + 1 }) * xres;
        let jp2 = (if j >= yres - 2 { yres - 1 } else { j + 2 }) * xres;
        let jp3 = (if j >= yres - 3 { yres - 1 } else { j + 3 }) * xres;

        for i in 0..xres {
            let im3 = if i <= 2 { 0 } else { i - 3 };
            let im2 = if i <= 1 { 0 } else { i - 2 };
            let im1 = if i == 0 { 0 } else { i - 1 };
            let ip1 = if i == xres - 1 { xres - 1 } else { i + 1 };
            let ip2 = if i >= xres - 2 { xres - 1 } else { i + 2 };
            let ip3 = if i >= xres - 3 { xres - 1 } else { i + 3 };

            // read data
            let e = o;
            let (d, d0, d1) = (j * xres + im1, j * xres + im2, j * xres + im3);
            let (f, f0, f1) = (j * xres + ip1, j * xres + ip2, j * xres + ip3);
            let (b, b0, b1) = (jm1 + i, jm2 + i, jm3 + i);
            let (h, h0, h1) = (jp1 + i, jp2 + i, jp3 + i);

            // extract data
            let (ec, eh, ev, eo) = (g_res[e], g_hori[e], g_vert[e], g_or[e]);
            let (dc, dh, do_, d0c, d0h, d1h) = (g_res[d], g_hori[d], g_or[d], g_res[d0], g_hori[d0], g_hori[d1]);
            let (fc, fh, fo, f0c, f0h, f1h) = (g_res[f], g_hori[f], g_or[f], g_res[f0], g_hori[f0], g_hori[f1]);
            let (bc, bv, bo, b0c, b0v, b1v) = (g_res[b], g_vert[b], g_or[b], g_res[b0], g_vert[b0], g_vert[b1]);
            let (hc, hv, ho, h0c, h0v, h1v) = (g_res[h], g_vert[h], g_or[h], g_res[h0], g_vert[h0], g_vert[h1]);

            // lvl2 mid (left, right / up, down)
            let lvl2x = ((ec.x && eh.y) && dc.z, (ec.y && eh.x) && fc.w);
            let lvl2y = ((ec.y && ev.z) && bc.w, (ec.z && ev.y) && hc.x);
            let lvl2z = ((ec.w && eh.z) && dc.y, (ec.z && eh.w) && fc.x);
            let lvl2w = ((ec.x && ev.w) && bc.z, (ec.w && ev.x) && hc.y);

            // lvl3 corners (hori, vert)
            let lvl3x = (lvl2x.1 && (dh.y && dh.x) && fh.z, lvl2w.1 && (bv.w && bv.x) && hv.z);
            let lvl3y = (lvl2x.0 && (fh.x && fh.y) && dh.w, lvl2y.1 && (bv.z && bv.y) && hv.w);
            let lvl3z = (lvl2z.0 && (fh.w && fh.z) && dh.x, lvl2y.0 && (hv.y && hv.z) && bv.x);
            let lvl3w = (lvl2z.1 && (dh.z && dh.w) && fh.y, lvl2w.0 && (hv.x && hv.w) && bv.y);

            // lvl4 corners (hori, vert)
            let lvl4x = (
                (dc.x && dh.y && eh.x && eh.y && fh.x && fh.y) && (d0c.z && d0h.w),
                (bc.x && bv.w && ev.x && ev.w && hv.x && hv.w) && (b0c.z && b0v.y),
            );
            let lvl4y = (
                (fc.y && fh.x && eh.y && eh.x && dh.y && dh.x) && (f0c.w && f0h.z),
                (bc.y && bv.z && ev.y && ev.z && hv.y && hv.z) && (b0c.w && b0v.x),
            );
            let lvl4z = (
                (fc.z && fh.w && eh.z && eh.w && dh.z && dh.w) && (f0c.x && f0h.y),
                (hc.z && hv.y && ev.z && ev.y && bv.z && bv.y) && (h0c.x && h0v.w),
            );
            let lvl4w = (
                (dc.w && dh.z && eh.w && eh.z && fh.w && fh.z) && (d0c.y && d0h.x),
                (hc.w && hv.x && ev.w && ev.x && bv.w && bv.x) && (h0c.y && h0v.z),
            );

            // lvl5 mid (left, right / up, down)
            let lvl5x = (lvl4x.0 && (f0h.x && f0h.y) && (d1h.z && d1h.w), lvl4y.0 && (d0h.y && d0h.x) && (f1h.w && f1h.z));
            let lvl5y = (lvl4y.1 && (h0v.y && h0v.z) && (b1v.w && b1v.x), lvl4z.1 && (b0v.z && b0v.y) && (h1v.x && h1v.w));
            let lvl5z = (lvl4w.0 && (f0h.w && f0h.z) && (d1h.y && d1h.x), lvl4z.0 && (d0h.z && d0h.w) && (f1h.x && f1h.y));
            let lvl5w = (lvl4x.1 && (h0v.x && h0v.w) && (b1v.z && b1v.y), lvl4w.1 && (b0v.w && b0v.x) && (h1v.y && h1v.z));

            // lvl6 corners (hori, vert)
            let lvl6x = (lvl5x.1 && (d1h.y && d1h.x), lvl5w.1 && (b1v.w && b1v.x));
            let lvl6y = (lvl5x.0 && (f1h.x && f1h.y), lvl5y.1 && (b1v.z && b1v.y));
            let lvl6z = (lvl5z.0 && (f1h.w && f1h.z), lvl5y.0 && (h1v.y && h1v.z));
            let lvl6w = (lvl5z.1 && (d1h.z && d1h.w), lvl5w.0 && (h1v.x && h1v.w));

            // subdots - 0 = E, 1 = D, 2 = D0, 3 = F, 4 = F0, 5 = B, 6 = B0, 7 = H, 8 = H0
            let first = |cases: [(bool, u32); 8]| cases.iter().find(|c| c.0).map_or(0, |c| c.1);
            let crn_x = first([
                ((ec.x && eo.x) || (lvl3x.0 && eo.y) || (lvl4x.0 && do_.x) || (lvl6x.0 && fo.y), 5),
                (ec.x || (lvl3x.1 && !eo.w) || (lvl4x.1 && !bo.x) || (lvl6x.1 && !ho.w), 1),
                (lvl3x.0, 3),
                (lvl3x.1, 7),
                (lvl4x.0, 2),
                (lvl4x.1, 6),
                (lvl6x.0, 4),
                (lvl6x.1, 8),
            ]);
            let crn_y = first([
                ((ec.y && eo.y) || (lvl3y.0 && eo.x) || (lvl4y.0 && fo.y) || (lvl6y.0 && do_.x), 5),
                (ec.y || (lvl3y.1 && !eo.z) || (lvl4y.1 && !bo.y) || (lvl6y.1 && !ho.z), 3),
                (lvl3y.0, 1),
                (lvl3y.1, 7),
                (lvl4y.0, 4),
                (lvl4y.1, 6),
                (lvl6y.0, 2),
                (lvl6y.1, 8),
            ]);
            let crn_z = first([
                ((ec.z && eo.z) || (lvl3z.0 && eo.w) || (lvl4z.0 && fo.z) || (lvl6z.0 && do_.w), 7),
                (ec.z || (lvl3z.1 && !eo.y) || (lvl4z.1 && !ho.z) || (lvl6z.1 && !bo.y), 3),
                (lvl3z.0, 1),
                (lvl3z.1, 5),
                (lvl4z.0, 4),
                (lvl4z.1, 8),
                (lvl6z.0, 2),
                (lvl6z.1, 6),
            ]);
            let crn_w = first([
                ((ec.w && eo.w) || (lvl3w.0 && eo.z) || (lvl4w.0 && do_.w) || (lvl6w.0 && fo.z), 7),
                (ec.w || (lvl3w.1 && !eo.x) || (lvl4w.1 && !ho.w) || (lvl6w.1 && !bo.x), 1),
                (lvl3w.0, 3),
                (lvl3w.1, 5),
                (lvl4w.0, 2),
                (lvl4w.1, 8),
                (lvl6w.0, 4),
                (lvl6w.1, 6),
            ]);

            let mid = |cases: [(bool, u32); 5], last: (bool, u32)| {
                cases.iter().find(|c| c.0).map_or(if last.0 { last.1 } else { 0 }, |c| c.1)
            };
            let mid_x = mid(
                [((lvl2x.0 && eo.x) || (lvl2x.1 && eo.y) || (lvl5x.0 && do_.x) || (lvl5x.1 && fo.y), 5), (lvl2x.0, 1), (lvl2x.1, 3), (lvl5x.0, 2), (lvl5x.1, 4)],
                (ec.x && dc.z && ec.y && fc.w, if eo.x { if eo.y { 5 } else { 3 } } else { 1 }),
            );
            let mid_y = mid(
                [((lvl2y.0 && !eo.y) || (lvl2y.1 && !eo.z) || (lvl5y.0 && !bo.y) || (lvl5y.1 && !ho.z), 3), (lvl2y.0, 5), (lvl2y.1, 7), (lvl5y.0, 6), (lvl5y.1, 8)],
                (ec.y && bc.w && ec.z && hc.x, if !eo.y { if !eo.z { 3 } else { 7 } } else { 5 }),
            );
            let mid_z = mid(
                [((lvl2z.0 && eo.w) || (lvl2z.1 && eo.z) || (lvl5z.0 && do_.w) || (lvl5z.1 && fo.z), 7), (lvl2z.0, 1), (lvl2z.1, 3), (lvl5z.0, 2), (lvl5z.1, 4)],
                (ec.z && fc.x && ec.w && dc.y, if eo.z { if eo.w { 7 } else { 1 } } else { 3 }),
            );
            let mid_w = mid(
                [((lvl2w.0 && !eo.x) || (lvl2w.1 && !eo.w) || (lvl5w.0 && !bo.x) || (lvl5w.1 && !ho.w), 1), (lvl2w.0, 5), (lvl2w.1, 7), (lvl5w.0, 6), (lvl5w.1, 8)],
                (ec.w && hc.y && ec.x && bc.z, if !eo.w { if !eo.x { 1 } else { 5 } } else { 7 }),
            );

            picks[o] = crn_x | (crn_y << 4) | (crn_z << 8) | (crn_w << 12) | (mid_x << 16) | (mid_y << 20) | (mid_z << 24) | (mid_w << 28);
            o += 1;
        }
    }

    let mut out = vec![0u32; if upscale { n * 9 } else { n }];
    o = 0;
    for j in 0..yres {
        for i in 0..xres {
            let tmp = picks[o];
            let (crn_x, crn_y, crn_z, crn_w) = (tmp & 15, (tmp >> 4) & 15, (tmp >> 8) & 15, (tmp >> 12) & 15);
            let (mid_x, mid_y, mid_z, mid_w) = ((tmp >> 16) & 15, (tmp >> 20) & 15, (tmp >> 24) & 15, tmp >> 28);
            let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
            for (o2, sp) in [crn_x, mid_x, crn_y, mid_w, 0, mid_y, crn_w, mid_z, crn_z].into_iter().enumerate() {
                // where the subdot's colour comes from - 0 = E, 1 = D, 2 = D0, 3 = F, 4 = F0,
                // 5 = B, 6 = B0, 7 = H, 8 = H0; None beyond the border
                let (x, y) = (xres as isize, yres as isize);
                let (ii, jj) = (i as isize, j as isize);
                let from: Option<isize> = match sp {
                    0 => Some(0),
                    1 => (ii != 0).then_some(-1),
                    2 => (ii > 1).then_some(-2),
                    3 => (ii != x - 1).then_some(1),
                    4 => (ii < x - 2).then_some(2),
                    5 => (jj != 0).then_some(-x),
                    6 => (jj > 1).then_some(-2 * x),
                    7 => (jj != y - 1).then_some(x),
                    8 => (jj < y - 2).then_some(2 * x),
                    _ => Some(0),
                };
                // beyond the border: half as bright as the dot
                let colour = from.map_or((data[o] & 0xFEFE_FEFE) >> 1, |d| data[(o as isize + d) as usize]);
                if upscale {
                    out[(j * 3 + o2 / 3) * xres * 3 + i * 3 + o2 % 3] = colour;
                } else {
                    r += colour & 255;
                    g += (colour >> 8) & 255;
                    b += (colour >> 16) & 255;
                    a += colour >> 24;
                }
            }
            if !upscale {
                out[o] = (r / 9) | ((g / 9) << 8) | ((b / 9) << 16) | ((a / 9) << 24);
            }
            o += 1;
        }
    }
    out
}
