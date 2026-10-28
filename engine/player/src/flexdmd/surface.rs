// Visual Pinball Infinite's own: the drawing FlexDMD 1.9.1 does with Windows' GDI+ (System.Drawing, which FlexDMD
// draws its frames with), written to give GDI+'s results, measured against GDI+ itself: how a fill
// covers fractions of pixels, how a picture is blended, placed between pixels and scaled.

//! The picture FlexDMD draws into, and its pictures, as GDI+ draws them.
//!
//! **Bytes, as in memory.** FlexDMD's frame is a 24-bit GDI+ bitmap, which
//! keeps blue, green, red; FlexDMD reads its first byte as red, and swaps
//! the bytes of its own pictures when it loads them so they come out
//! right. Fills (frames' colours, fades) are GDI+'s own, so they come out
//! with red and blue swapped — a red frame shows blue. Both are kept here
//! by holding every pixel as its bytes in memory: a frame's `[0]` is what
//! FlexDMD reports as red, and [`Surface::fill`] writes a colour the way GDI+
//! writes it.
//!
//! **GDI+'s rules** (its defaults: bilinear pictures, no anti-aliasing,
//! pixel centres on whole coordinates, alpha blended over):
//! - A fill's edges are rounded to sixteenths of a pixel, then it covers
//!   the pixels whose centre is inside, left and top edges included.
//! - A picture drawn at its own size within 1/64 of a pixel is copied, at
//!   the nearest pixel; otherwise it's resampled: each pixel takes the
//!   picture at its centre, between the four nearest texels (texel centres
//!   on whole coordinates too), outside the picture clear. Positions are
//!   kept to 1/1024 of a pixel. A part of a picture (a font's character)
//!   blends with its neighbours on the sheet.
//! - Blending premultiplies then adds what shows through, each rounded.
//! - A clip is a fill's region; clearing fills the clip, if any.

/// A picture, four bytes a pixel as in memory (see the module's note),
/// the last alpha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub px: Vec<[u8; 4]>,
    /// Whether GDI+ holds it with alpha (a 32-bit picture); else 24-bit,
    /// opaque.
    pub alpha: bool,
}

impl Bitmap {
    /// A clear picture, with alpha.
    pub fn new(width: u32, height: u32) -> Bitmap {
        Bitmap { width, height, px: vec![[0; 4]; width as usize * height as usize], alpha: true }
    }

    /// The texel at (x, y); clear outside.
    fn texel(&self, x: i64, y: i64) -> [u8; 4] {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return [0; 4];
        }
        self.px[y as usize * self.width as usize + x as usize]
    }
}

/// The frame FlexDMD draws into: its bytes as GDI+ keeps a 24-bit bitmap,
/// the transform's translation and the clip.
pub struct Surface {
    pub width: u32,
    pub height: u32,
    pub px: Vec<[u8; 3]>,
    tx: f32,
    ty: f32,
    /// The clip's pixels: x from, y from, x to, y to (exclusive).
    clip: Option<[i32; 4]>,
}

/// Rounds half up.
fn round(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// GDI+'s fill region along one axis: the pixels from `a` to `b`, edges
/// rounded to sixteenths, centres inside (`a` included).
fn fill_span(a: f32, b: f32) -> (i32, i32) {
    let fix = |v: f32| (f64::from(v) * 16.0).round() as i64;
    let first = (fix(a) + 15).div_euclid(16);
    let end = (fix(b) + 15).div_euclid(16);
    (first.clamp(i32::MIN as i64, i32::MAX as i64) as i32, end.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

/// A position kept to 1/1024 of a pixel, as GDI+ resamples.
fn fine(v: f64) -> f64 {
    (v * 1024.0).round() / 1024.0
}

/// The pixels whose centre is from `a` (included) to `b`.
fn image_span(a: f64, b: f64) -> (i64, i64) {
    (a.ceil() as i64, b.ceil() as i64)
}

impl Surface {
    pub fn new(width: u32, height: u32) -> Surface {
        Surface { width, height, px: vec![[0; 3]; width as usize * height as usize], tx: 0.0, ty: 0.0, clip: None }
    }

    /// GDI+'s `TranslateTransform`, in single precision as GDI+ adds it.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.tx += dx;
        self.ty += dy;
    }

    /// GDI+'s `SetClip(rect, Replace)`: replaces any clip, in local
    /// coordinates.
    pub fn set_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let (x0, x1) = fill_span(x + self.tx, (x + w) + self.tx);
        let (y0, y1) = fill_span(y + self.ty, (y + h) + self.ty);
        self.clip = Some([x0, y0, x1, y1]);
    }

    pub fn reset_clip(&mut self) {
        self.clip = None;
    }

    /// The pixel bounds anything may touch: the frame, within the clip.
    fn bounds(&self) -> [i64; 4] {
        let mut b = [0, 0, i64::from(self.width), i64::from(self.height)];
        if let Some(c) = self.clip {
            b = [b[0].max(c[0].into()), b[1].max(c[1].into()), b[2].min(c[2].into()), b[3].min(c[3].into())];
        }
        b
    }

    /// GDI+'s `Clear`: the clip (else everything) set to a colour.
    pub fn clear(&mut self, rgb: [u8; 3]) {
        let [x0, y0, x1, y1] = self.bounds();
        let m = [rgb[2], rgb[1], rgb[0]];
        for y in y0..y1 {
            for x in x0..x1 {
                self.px[y as usize * self.width as usize + x as usize] = m;
            }
        }
    }

    /// Blends one pixel: `p` premultiplied colour bytes, `a` alpha.
    fn blend(&mut self, x: i64, y: i64, p: [u8; 3], a: u8) {
        let d = &mut self.px[y as usize * self.width as usize + x as usize];
        if a == 255 {
            *d = p;
            return;
        }
        for c in 0..3 {
            let rest = round(f64::from(d[c]) * f64::from(255 - a) / 255.0);
            d[c] = (f64::from(p[c]) + rest).min(255.0) as u8;
        }
    }

    /// GDI+'s `FillRectangle` with a solid colour, as GDI+ gives colours
    /// (alpha, red, green, blue), in local coordinates.
    pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, argb: [u8; 4]) {
        if !(w > 0.0 && h > 0.0) || argb[0] == 0 {
            return;
        }
        let (x0, x1) = fill_span(x + self.tx, (x + w) + self.tx);
        let (y0, y1) = fill_span(y + self.ty, (y + h) + self.ty);
        let [bx0, by0, bx1, by1] = self.bounds();
        let a = argb[0];
        // the brush's bytes in memory: blue, green, red
        let m = [argb[3], argb[2], argb[1]];
        let p = m.map(|c| round(f64::from(c) * f64::from(a) / 255.0) as u8);
        for py in i64::from(y0).max(by0)..i64::from(y1).min(by1) {
            for px in i64::from(x0).max(bx0)..i64::from(x1).min(bx1) {
                self.blend(px, py, p, a);
            }
        }
    }

    /// GDI+'s `DrawImage` of the part `src` (x, y, width, height) of a
    /// picture into the box (x, y, width, height) in local coordinates.
    pub fn draw(&mut self, bmp: &Bitmap, dest: [f32; 4], src: [f32; 4]) {
        let [dx, dy, dw, dh] = dest;
        let [sx, sy, sw, sh] = src;
        if !(dw > 0.0 && dh > 0.0 && sw > 0.0 && sh > 0.0) {
            return;
        }
        let (fx0, fy0) = (dx + self.tx, dy + self.ty);
        let (fx1, fy1) = ((dx + dw) + self.tx, (dy + dh) + self.ty);
        let (x0, y0) = (fine(f64::from(fx0)), fine(f64::from(fy0)));
        let (x1, y1) = (fine(f64::from(fx1)), fine(f64::from(fy1)));
        let (cx0, cx1) = image_span(x0, x1);
        let (cy0, cy1) = image_span(y0, y1);
        let [bx0, by0, bx1, by1] = self.bounds();
        let (cx0, cx1, cy0, cy1) = (cx0.max(bx0), cx1.min(bx1), cy0.max(by0), cy1.min(by1));
        if cx0 >= cx1 || cy0 >= cy1 {
            return;
        }
        let near = |v: f32| (f64::from(v) - f64::from(v).round()).abs() < 1.0 / 64.0;
        if dw == sw && dh == sh && near(fx0) && near(fy0) {
            // copied at the nearest pixel, within the part
            let (ox, oy) = (f64::from(fx0).round() as i64, f64::from(fy0).round() as i64);
            let (sx, sy, sw, sh) = (sx as i64, sy as i64, sw as i64, sh as i64);
            for py in cy0..cy1 {
                let ty = py - oy;
                if ty < 0 || ty >= sh {
                    continue;
                }
                for px in cx0..cx1 {
                    let tx = px - ox;
                    if tx < 0 || tx >= sw {
                        continue;
                    }
                    let t = bmp.texel(sx + tx, sy + ty);
                    if t[3] == 0 {
                        continue;
                    }
                    let p = [t[0], t[1], t[2]].map(|c| round(f64::from(c) * f64::from(t[3]) / 255.0) as u8);
                    self.blend(px, py, p, t[3]);
                }
            }
            return;
        }
        // resampled, between the four nearest texels
        let (kx, ky) = (f64::from(sw) / f64::from(dw), f64::from(sh) / f64::from(dh));
        for py in cy0..cy1 {
            let v = fine((py as f64 - y0) * ky + f64::from(sy));
            let (ty, fy) = (v.floor(), v - v.floor());
            for px in cx0..cx1 {
                let u = fine((px as f64 - x0) * kx + f64::from(sx));
                let (tx, fx) = (u.floor(), u - u.floor());
                let (tx, ty) = (tx as i64, ty as i64);
                let corners = [
                    (bmp.texel(tx, ty), (1.0 - fx) * (1.0 - fy)),
                    (bmp.texel(tx + 1, ty), fx * (1.0 - fy)),
                    (bmp.texel(tx, ty + 1), (1.0 - fx) * fy),
                    (bmp.texel(tx + 1, ty + 1), fx * fy),
                ];
                let (mut p, mut a) = ([0.0f64; 3], 0.0f64);
                for (t, w) in corners {
                    let ta = f64::from(t[3]);
                    for c in 0..3 {
                        p[c] += f64::from(t[c]) * ta / 255.0 * w;
                    }
                    a += ta * w;
                }
                let a8 = round(a).min(255.0) as u8;
                if a8 == 0 {
                    continue;
                }
                let p8 = p.map(|c| round(c).min(f64::from(a8)) as u8);
                self.blend(px, py, p8, a8);
            }
        }
    }

    /// The pixel's bytes as FlexDMD reads them: red, green, blue.
    pub fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        self.px[(y * self.width + x) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Row 0's first bytes of a surface.
    fn row(s: &Surface) -> Vec<u8> {
        (0..s.width).map(|x| s.rgb(x, 0)[0]).collect()
    }

    /// A one-row picture, opaque, its first bytes these values.
    fn strip(values: &[u8]) -> Bitmap {
        Bitmap { width: values.len() as u32, height: 1, px: values.iter().map(|&v| [v, 50, 0, 255]).collect(), alpha: true }
    }

    fn grey(s: &mut Surface, v: u8) {
        let w = s.width as f32;
        s.fill(0.0, 0.0, w, 1.0, [255, v, v, v]);
    }

    // Every expectation below is what GDI+ itself gave (PowerShell, System.Drawing).

    #[test]
    fn fills_cover_centres_after_rounding_to_sixteenths() {
        let white = [255, 255, 255, 255];
        let cases: [(f32, f32, &[u8]); 6] = [
            (0.5, 3.0, &[0, 255, 255, 255, 0, 0]),
            (0.49, 3.0, &[0, 255, 255, 255, 0, 0]),
            (-0.5, 2.0, &[255, 255, 0, 0, 0, 0]),
            (1.5, 0.5, &[0, 0, 0, 0, 0, 0]),
            (1.031, 2.0, &[0, 255, 255, 0, 0, 0]),
            (1.032, 2.0, &[0, 0, 255, 255, 0, 0]),
        ];
        for (x, w, want) in cases {
            let mut s = Surface::new(6, 1);
            s.fill(x, 0.0, w, 1.0, white);
            assert_eq!(row(&s), want, "fill at {x}, {w} wide");
        }
        // a translation moves it before rounding
        let mut s = Surface::new(6, 1);
        s.translate(0.5, 0.0);
        s.fill(1.0, 0.0, 2.0, 1.0, white);
        assert_eq!(row(&s), [0, 0, 255, 255, 0, 0]);
        // nothing for no width
        let mut s = Surface::new(6, 1);
        s.fill(4.0, 0.0, -2.0, 1.0, white);
        assert_eq!(row(&s), [0; 6]);
    }

    #[test]
    fn fills_blend_and_swap_red_and_blue() {
        let mut s = Surface::new(2, 1);
        s.fill(0.0, 0.0, 2.0, 1.0, [255, 37, 180, 250]);
        // GDI+ keeps blue first: FlexDMD reads it as red
        assert_eq!(s.rgb(0, 0), [250, 180, 37]);
        s.fill(0.0, 0.0, 2.0, 1.0, [127, 0, 0, 0]);
        assert_eq!(s.rgb(0, 0), [125, 90, 19]);
        let mut s = Surface::new(2, 1);
        s.fill(1.0, 0.0, 1.0, 1.0, [255, 100, 100, 100]);
        s.fill(0.0, 0.0, 2.0, 1.0, [127, 255, 255, 255]);
        assert_eq!(row(&s), [127, 177]);
    }

    #[test]
    fn clears_and_clips_cover_fill_regions() {
        let mut s = Surface::new(6, 1);
        grey(&mut s, 255);
        s.set_clip(1.5, 0.0, 2.0, 1.0);
        s.clear([0, 0, 0]);
        assert_eq!(row(&s), [255, 255, 0, 0, 255, 255]);
        let mut s = Surface::new(6, 1);
        s.translate(0.5, 0.0);
        s.set_clip(1.0, 0.0, 2.0, 1.0);
        s.fill(-5.0, -5.0, 20.0, 20.0, [255, 255, 255, 255]);
        assert_eq!(row(&s), [0, 0, 255, 255, 0, 0]);
    }

    #[test]
    fn pictures_place_and_scale_as_gdiplus_does() {
        let s4 = strip(&[0, 80, 160, 240]);
        let draw = |w: u32, tx: f32, dest: [f32; 4], bg: u8| {
            let mut s = Surface::new(w, 1);
            if bg > 0 {
                grey(&mut s, bg);
            }
            s.translate(tx, 0.0);
            s.draw(&s4, dest, [0.0, 0.0, 4.0, 1.0]);
            row(&s)
        };
        assert_eq!(draw(8, 0.0, [2.0, 0.0, 4.0, 1.0], 0), [0, 0, 0, 80, 160, 240, 0, 0]);
        assert_eq!(draw(8, 0.5, [2.0, 0.0, 4.0, 1.0], 0), [0, 0, 0, 40, 120, 200, 120, 0]);
        assert_eq!(draw(8, 0.25, [2.0, 0.0, 4.0, 1.0], 0), [0, 0, 0, 60, 140, 220, 60, 0]);
        assert_eq!(draw(8, -0.25, [2.0, 0.0, 4.0, 1.0], 0), [0, 0, 20, 100, 180, 180, 0, 0]);
        assert_eq!(draw(10, 0.0, [0.0, 0.0, 8.0, 1.0], 0), [0, 40, 80, 120, 160, 200, 240, 120, 0, 0]);
        assert_eq!(draw(10, 0.0, [0.0, 0.0, 6.0, 1.0], 0), [0, 53, 107, 160, 213, 160, 0, 0, 0, 0]);
        assert_eq!(draw(6, 0.0, [0.0, 0.0, 3.0, 1.0], 0), [0, 107, 213, 0, 0, 0]);
        assert_eq!(draw(6, 0.0, [0.0, 0.0, 2.0, 1.0], 0), [0, 160, 0, 0, 0, 0]);
        assert_eq!(draw(8, 0.5, [2.0, 0.0, 4.0, 1.0], 100), [100, 100, 100, 40, 120, 200, 170, 100]);
        assert_eq!(draw(12, 0.5, [2.0, 0.0, 8.0, 1.0], 100), [100, 100, 100, 20, 60, 100, 140, 180, 220, 205, 135, 100]);
        assert_eq!(draw(12, 0.0, [2.0, 0.0, 6.0, 1.0], 100), [100, 100, 0, 53, 107, 160, 213, 193, 100, 100, 100, 100]);
        let s8 = strip(&[0, 40, 80, 120, 160, 200, 240, 255]);
        let mut s = Surface::new(10, 1);
        s.draw(&s8, [0.0, 0.0, 9.0, 1.0], [0.0, 0.0, 8.0, 1.0]);
        assert_eq!(row(&s), [0, 36, 71, 107, 142, 178, 213, 243, 227, 0]);
    }

    #[test]
    fn small_offsets_copy_or_resample_as_gdiplus_does() {
        let s4 = strip(&[10, 80, 160, 240]);
        let cases: [(f32, [u8; 9]); 11] = [
            (0.01, [100, 100, 100, 80, 160, 240, 100, 100, 100]),
            (0.0156, [100, 100, 100, 80, 160, 240, 100, 100, 100]),
            (0.016, [100, 100, 100, 79, 159, 239, 102, 100, 100]),
            (0.02, [100, 100, 100, 79, 158, 238, 103, 100, 100]),
            (0.0313, [100, 100, 100, 78, 158, 238, 105, 100, 100]),
            (0.032, [100, 100, 100, 78, 157, 237, 105, 100, 100]),
            (0.4, [100, 100, 100, 52, 128, 208, 156, 100, 100]),
            (0.6, [100, 100, 100, 38, 112, 192, 184, 100, 100]),
            (0.98, [100, 100, 100, 11, 82, 162, 237, 100, 100]),
            (0.99, [100, 100, 100, 10, 80, 160, 240, 100, 100]),
            (-0.02, [100, 100, 11, 82, 162, 237, 100, 100, 100]),
        ];
        for (tx, want) in cases {
            let mut s = Surface::new(9, 1);
            grey(&mut s, 100);
            s.translate(tx, 0.0);
            s.draw(&s4, [2.0, 0.0, 4.0, 1.0], [0.0, 0.0, 4.0, 1.0]);
            assert_eq!(row(&s), want, "offset {tx}");
        }
    }

    #[test]
    fn parts_of_pictures_blend_with_their_neighbours() {
        let atlas = strip(&[0, 80, 160, 240, 40, 120]);
        for (tx, want) in [(0.0, [100, 100, 80, 160, 240, 100, 100, 100]), (0.5, [100, 100, 100, 120, 200, 140, 100, 100]), (0.25, [100, 100, 100, 140, 220, 90, 100, 100])] {
            let mut s = Surface::new(8, 1);
            grey(&mut s, 100);
            s.translate(tx, 0.0);
            s.draw(&atlas, [2.0, 0.0, 3.0, 1.0], [1.0, 0.0, 3.0, 1.0]);
            assert_eq!(row(&s), want, "offset {tx}");
        }
    }

    #[test]
    fn alpha_premultiplies_then_adds_what_shows_through() {
        let p = Bitmap { width: 4, height: 1, px: vec![[200, 0, 0, 128], [37, 0, 0, 128], [90, 0, 0, 200], [250, 0, 0, 10]], alpha: true };
        for (bg, want) in [(0, [100, 19, 71, 10]), (100, [150, 69, 93, 106])] {
            let mut s = Surface::new(4, 1);
            grey(&mut s, bg);
            s.draw(&p, [0.0, 0.0, 4.0, 1.0], [0.0, 0.0, 4.0, 1.0]);
            assert_eq!(row(&s), want, "over {bg}");
        }
        // 2×2 to 4×4, both ways at once
        let q = Bitmap { width: 2, height: 2, px: vec![[0, 0, 0, 255], [200, 0, 0, 255], [100, 0, 0, 255], [255, 0, 0, 255]], alpha: true };
        let mut s = Surface::new(5, 5);
        s.draw(&q, [0.0, 0.0, 4.0, 4.0], [0.0, 0.0, 2.0, 2.0]);
        let grid: Vec<Vec<u8>> = (0..4).map(|y| (0..4).map(|x| s.rgb(x, y)[0]).collect()).collect();
        assert_eq!(grid, vec![vec![0, 100, 200, 100], vec![50, 139, 228, 114], vec![100, 178, 255, 128], vec![50, 89, 128, 64]]);
    }
}
