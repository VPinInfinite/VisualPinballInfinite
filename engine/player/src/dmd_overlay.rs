// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its B2S plugin's DMD
// overlay (plugins/b2s/B2SDMDOverlay.cpp: its settings, LoadSettings, Render and SearchDmdSubFrame).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The DMD inside a backglass file's picture, as VPX 5436's backglass draws
//! it. Many files' score panel —
//! the picture for a cabinet's third screen — has a dark window where the
//! machine's DMD sits; VPX can draw the live DMD there, at a place found by
//! itself or given by hand, and likewise over the backglass picture.
//!
//! The window is found as VPX finds it: the largest dark rectangle in the
//! picture, cut to the DMD's shape, at least a third of the picture across,
//! favouring one near the middle. How dark is dark is searched for: seven
//! halvings of a brightness limit, each search kept to where the last one
//! found anything.
//!
//! VPX leaves both off unless asked. Here the score panel shows the DMD
//! placed by itself unless set otherwise; the backglass keeps VPX's default.

/// Where the DMD shows over a picture (VPX's `ScoreViewDMDOverlay`,
/// `ScoreViewDMDAutoPos` and `ScoreViewDMDX`, `…Y`, `…W` and `…H`; the
/// backglass has the same set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    /// Not shown (VPX's overlay off).
    Off,
    /// In the picture's dark window, found as VPX finds it.
    Auto,
    /// At a place in the page's own pixels: left, bottom — counted up from
    /// the page's bottom edge, as VPX counts it — width and height.
    At([i32; 4]),
}

impl Overlay {
    /// Reads `off`, `auto` or `X,Y,WIDTH,HEIGHT`.
    pub fn parse(text: &str) -> Option<Overlay> {
        match text {
            "off" => Some(Overlay::Off),
            "auto" => Some(Overlay::Auto),
            _ => {
                let place: Vec<i32> = text.split(',').map(|v| v.trim().parse().ok()).collect::<Option<_>>()?;
                match place[..] {
                    [x, y, w, h] if x >= 0 && y >= 0 && w > 0 && h > 0 => Some(Overlay::At([x, y, w, h])),
                    _ => None,
                }
            }
        }
    }

    /// Where the DMD is drawn on a page `page_height` of its pixels high:
    /// left, top, width and height, in the page's pixels from its top left;
    /// `found` is the window found in its picture ([`find`]). None when it
    /// isn't drawn: off, or no window found.
    ///
    /// As in VPX, a window found is counted from the bottom of the whole
    /// picture, and drawn counted from the bottom of the page shown: with
    /// the backglass's grill cut off, a window found in the grill shows
    /// that much higher, over the backglass.
    pub fn place(self, found: Option<[i32; 4]>, page_height: f32) -> Option<[f32; 4]> {
        let [x, y, w, h] = match self {
            Overlay::Off => return None,
            Overlay::Auto => found?,
            Overlay::At(place) => place,
        };
        (w > 0 && h > 0).then_some([x as f32, page_height - (y + h) as f32, w as f32, h as f32])
    }
}

/// The DMD's window in a picture of `width` × `height` texels (`rgba`, 8
/// bits a channel, rows from the top), for a DMD `aspect` times wider than
/// high, as VPX gives it (`SearchDmdSubFrame`): left, bottom — counted up
/// from the picture's last row, as VPX turns it — width and height; None
/// where VPX finds none.
pub fn find(rgba: &[u8], width: u32, height: u32, aspect: f32) -> Option<[i32; 4]> {
    let (tex_w, tex_h) = (width as i32, height as i32);
    if tex_w <= 0 || tex_h <= 0 || rgba.len() < width as usize * height as usize * 4 || aspect <= 0.0 {
        return None;
    }
    // large rectangles, favouring ones about the middle
    let texels = tex_w as f32;
    let heuristic = |f: &[i32; 4]| f[2] as f32 / texels - (0.5 - (f[0] + f[2] / 2) as f32 / texels).abs();
    let padding = (2 * tex_w) / 1920;
    let (mut lum_min, mut lum_max) = (0.0f32, 256.0f32);
    let mut area = [0, 0, tex_w, tex_h];
    let mut best = [0; 4];
    for _ in 0..7 {
        let limit = (lum_min + lum_max) * 0.5;
        // the largest dark rectangle: each column's run of dark texels down to the row, then every rectangle those
        // runs make, through a stack of columns
        let mut found = [0; 4];
        let mut next_area = [0; 4];
        let mut stack: Vec<i32> = Vec::new();
        let mut heights = vec![0i32; tex_w as usize];
        let end = area[0] + area[2];
        for y in area[1]..area[1] + area[3] {
            let row = (y * tex_w) as usize;
            for x in area[0]..end {
                let p = (row + x as usize) * 4;
                let lum = 0.299 * f32::from(rgba[p]) + 0.587 * f32::from(rgba[p + 1]) + 0.114 * f32::from(rgba[p + 2]);
                let column = &mut heights[x as usize];
                *column = if lum < limit { *column + 1 } else { 0 };
            }
            for x in area[0]..=end {
                while let Some(&top) = stack.last() {
                    if x != end && heights[top as usize] <= heights[x as usize] {
                        break;
                    }
                    let h = heights[top as usize];
                    stack.pop();
                    let w = stack.last().map_or(x - area[0], |&left| x - left - 1);
                    if w > 6 * padding && h > 3 * padding {
                        let unpadded = [stack.last().map_or(area[0], |&left| left + 1), y - h + 1, w, h];
                        // the next search covers it, chosen or not
                        next_area = if next_area[2] == 0 { unpadded } else { union(next_area, unpadded) };
                        // padded, and cut to the DMD's shape about its middle
                        let mut f = [unpadded[0] + padding, unpadded[1] + padding, unpadded[2] - 2 * padding, unpadded[3] - 2 * padding];
                        if f[2] as f32 / f[3] as f32 > aspect {
                            let cut = (f[3] as f32 * aspect) as i32;
                            f[0] += (f[2] - cut) / 2;
                            f[2] = cut;
                        } else {
                            let cut = (f[2] as f32 / aspect) as i32;
                            f[1] += (f[3] - cut) / 2;
                            f[3] = cut;
                        }
                        // at least a third of the picture across
                        if 3 * f[2] >= tex_w && heuristic(&f) > heuristic(&found) {
                            found = f;
                        }
                    }
                }
                if x < end {
                    stack.push(x);
                }
            }
        }
        if found[3] > 0 {
            lum_max = limit;
            best = found;
            area = next_area;
        } else {
            lum_min = limit;
        }
    }
    // turned for drawing, counted up from the last row (VPX's)
    (best[2] > 0 && best[3] > 0).then_some([best[0], tex_h - 1 - best[1] - best[3], best[2], best[3]])
}

/// The DMD's window in a backglass's speaker panel (its grill: the picture's
/// bottom `grill` rows), for a DMD `aspect` times wider than high, counted as
/// [`find`] counts it; None where the panel's middle isn't such a window.
///
/// Ours, beyond VPX: VPX searches the whole
/// picture for its darkest large rectangle ([`find`]), which finds a black
/// window but lands beside one that's lighter than its frame — the brushed
/// grey panels of Lord of the Rings' and the Simpsons' backglasses. A
/// speaker panel's display sits in its middle, between the speakers, so
/// this takes the largest rectangle about the panel's middle inside the
/// region reached from the middle without crossing an edge — the picture
/// smoothed first, an edge a brightness change sharper than one level a
/// texel: a frame's line or bevel is, a brushed panel's slow sheen or a
/// black window isn't. It must be closed on every side, wide, and at least a
/// third of the picture across, then is padded and cut to the DMD's shape as
/// VPX cuts its own.
pub fn find_in_grill(rgba: &[u8], width: u32, height: u32, grill: u32, aspect: f32) -> Option<[i32; 4]> {
    let (w, h) = (width as usize, height as usize);
    let strip = grill.min(height) as usize;
    if w < 16 || strip < 16 || rgba.len() < w * h * 4 || aspect <= 0.0 {
        return None;
    }
    let top = h - strip;
    // the panel's brightness, smoothed by a box a few texels across (a running sum table)
    let radius = (w / 600).max(2);
    let mut sums = vec![0.0f64; (w + 1) * (strip + 1)];
    for y in 0..strip {
        let mut row = 0.0;
        for x in 0..w {
            let p = ((top + y) * w + x) * 4;
            row += 0.299 * f64::from(rgba[p]) + 0.587 * f64::from(rgba[p + 1]) + 0.114 * f64::from(rgba[p + 2]);
            sums[(y + 1) * (w + 1) + x + 1] = sums[y * (w + 1) + x + 1] + row;
        }
    }
    let smooth = |x: usize, y: usize| {
        let (x0, x1) = (x.saturating_sub(radius), (x + radius + 1).min(w));
        let (y0, y1) = (y.saturating_sub(radius), (y + radius + 1).min(strip));
        let total = sums[y1 * (w + 1) + x1] - sums[y0 * (w + 1) + x1] - sums[y1 * (w + 1) + x0] + sums[y0 * (w + 1) + x0];
        total / ((x1 - x0) * (y1 - y0)) as f64
    };
    let (cx, cy) = (w / 2, strip / 2);
    // the window: the texels reached from the panel's middle without crossing an edge (a brightness change
    // sharper than one level a texel, once smoothed), which a frame's line or bevel makes and a brushed
    // panel's slow sheen doesn't
    let level: Vec<f64> = (0..strip).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| smooth(x, y)).collect();
    let at = |x: usize, y: usize| level[y * w + x];
    let flat = |x: usize, y: usize| {
        let gx = (at((x + 1).min(w - 1), y) - at(x.saturating_sub(1), y)) / if x == 0 || x == w - 1 { 1.0 } else { 2.0 };
        let gy = (at(x, (y + 1).min(strip - 1)) - at(x, y.saturating_sub(1))) / if y == 0 || y == strip - 1 { 1.0 } else { 2.0 };
        gx.hypot(gy) <= 1.0
    };
    if !flat(cx, cy) {
        return None;
    }
    let mut inside = vec![false; w * strip];
    inside[cy * w + cx] = true;
    let mut queue = std::collections::VecDeque::from([(cx, cy)]);
    while let Some((x, y)) = queue.pop_front() {
        let mut visit = |nx: usize, ny: usize| {
            if !inside[ny * w + nx] && flat(nx, ny) {
                inside[ny * w + nx] = true;
                queue.push_back((nx, ny));
            }
        };
        if x > 0 {
            visit(x - 1, y);
        }
        if x + 1 < w {
            visit(x + 1, y);
        }
        if y > 0 {
            visit(x, y - 1);
        }
        if y + 1 < strip {
            visit(x, y + 1);
        }
    }
    let near = |x: usize, y: usize| inside[y * w + x];
    // each row's run of the window through the middle column: [left, right)
    let runs: Vec<Option<(usize, usize)>> = (0..strip)
        .map(|y| {
            if !near(cx, y) {
                return None;
            }
            let mut left = cx;
            while left > 0 && near(left - 1, y) {
                left -= 1;
            }
            let mut right = cx + 1;
            while right < w && near(right, y) {
                right += 1;
            }
            Some((left, right))
        })
        .collect();
    runs[cy]?;
    // the largest rectangle through the middle: its top at or above the middle row, its bottom at or below
    let mut above = Vec::new(); // (row, left, right): the narrowest the rows from the middle up to it allow
    let (mut left, mut right) = (0, w);
    for y in (0..=cy).rev() {
        let Some((l, r)) = runs[y] else { break };
        (left, right) = (left.max(l), right.min(r));
        above.push((y, left, right));
    }
    let mut below = Vec::new();
    let (mut left, mut right) = (0, w);
    for (y, run) in runs.iter().enumerate().skip(cy) {
        let Some((l, r)) = *run else { break };
        (left, right) = (left.max(l), right.min(r));
        below.push((y, left, right));
    }
    let mut best: Option<[usize; 4]> = None;
    for &(t, tl, tr) in &above {
        for &(b, bl, br) in &below {
            let (l, r) = (tl.max(bl), tr.min(br));
            let area = (r - l) * (b - t + 1);
            if best.is_none_or(|f| area > f[2] * f[3]) {
                best = Some([l, t, r - l, b - t + 1]);
            }
        }
    }
    let [x, y, bw, bh] = best?;
    // closed on every side (not running off the panel or the picture), wide, and a third of the picture across
    if x == 0 || x + bw >= w || y == 0 || y + bh >= strip || bw < 2 * bh || 3 * bw < w {
        return None;
    }
    // padded and cut to the DMD's shape about its middle, as VPX cuts its own
    let padding = ((2 * w) / 1920) as i32;
    let mut f = [x as i32 + padding, (top + y) as i32 + padding, bw as i32 - 2 * padding, bh as i32 - 2 * padding];
    if f[2] <= 0 || f[3] <= 0 {
        return None;
    }
    if f[2] as f32 / f[3] as f32 > aspect {
        let cut = (f[3] as f32 * aspect) as i32;
        f[0] += (f[2] - cut) / 2;
        f[2] = cut;
    } else {
        let cut = (f[2] as f32 / aspect) as i32;
        f[1] += (f[3] - cut) / 2;
        f[3] = cut;
    }
    // counted up from the last row, as `find` gives it
    Some([f[0], height as i32 - 1 - f[1] - f[3], f[2], f[3]])
}

/// The smallest rectangle holding both.
fn union(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    let (x, y) = (a[0].min(b[0]), a[1].min(b[1]));
    [x, y, (a[0] + a[2]).max(b[0] + b[2]) - x, (a[1] + a[3]).max(b[1] + b[3]) - y]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture of `background` with a dark rectangle in it.
    fn picture(width: u32, height: u32, dark: [u32; 4], background: [u8; 4]) -> Vec<u8> {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let inside = x >= dark[0] && x < dark[0] + dark[2] && y >= dark[1] && y < dark[1] + dark[3];
                rgba.extend(if inside { [10, 10, 10, 255] } else { background });
            }
        }
        rgba
    }

    const GOLD: [u8; 4] = [200, 180, 40, 255];
    const WHITE: [u8; 4] = [255; 4];

    #[test]
    fn the_dark_window_is_found_cut_to_the_dmds_shape() {
        // 300 × 60 dark, a 4:1 DMD: 240 × 60 about its middle (no padding below 960 texels across)
        let rgba = picture(400, 200, [50, 120, 300, 60], GOLD);
        // 120 down from the top, 60 high: its bottom 20 up from the bottom edge, counted from the last row
        assert_eq!(find(&rgba, 400, 200, 4.0), Some([80, 19, 240, 60]));
        // 1920 across: 2 texels of padding each side, then the shape, as near the picture's middle as the window
        // allows (a part of it is a dark rectangle too)
        let rgba = picture(1920, 1080, [100, 600, 1700, 420], GOLD);
        assert_eq!(find(&rgba, 1920, 1080, 4.0), Some([960 - 1664 / 2, 1080 - 1 - 602 - 416, 1664, 416]));
        // with no window dark enough, the limit rises until the whole picture counts as dark (the first of the
        // best rectangles kept, the top one); on white, nothing ever does, and a window under a third of the
        // picture across isn't taken
        assert_eq!(find(&picture(400, 200, [0, 0, 0, 0], GOLD), 400, 200, 4.0), Some([0, 99, 400, 100]));
        assert_eq!(find(&picture(400, 200, [0, 0, 0, 0], WHITE), 400, 200, 4.0), None);
        assert_eq!(find(&picture(400, 200, [10, 10, 120, 40], WHITE), 400, 200, 4.0), None);
    }

    /// A backglass whose bottom `grill` rows are a speaker panel of `frame`
    /// brightness, a speaker either side (a dim mesh), and between them a
    /// window of `window` brightness, over bright artwork.
    fn speaker_panel(width: u32, height: u32, grill: u32, window: u8, frame: u8) -> Vec<u8> {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        let top = height - grill;
        for y in 0..height {
            for x in 0..width {
                let v = if y < top {
                    200
                } else {
                    let gy = y - top;
                    let speaker = (gy > grill / 8 && gy < grill * 7 / 8) && (x > width / 40 && x < width / 6 || x > width * 5 / 6 && x < width * 39 / 40);
                    let inside = gy > grill / 6 && gy < grill * 5 / 6 && x > width / 5 && x < width * 4 / 5;
                    if speaker {
                        if (x + gy).is_multiple_of(4) { 40 } else { 22 }
                    } else if inside {
                        // brushed: a little noise along the rows
                        window.saturating_add(((x * 7 + gy * 3) % 9) as u8)
                    } else {
                        frame
                    }
                };
                rgba.extend([v, v, v, 255]);
            }
        }
        rgba
    }

    #[test]
    fn the_window_in_a_speaker_panel_is_found_lighter_or_darker_than_its_frame() {
        let (w, h, grill) = (1200u32, 1000u32, 200u32);
        // a brushed grey window in a black panel (Lord of the Rings'), and a black one in a lighter bezel
        for (window, frame) in [(40u8, 4u8), (0u8, 90u8)] {
            let rgba = speaker_panel(w, h, grill, window, frame);
            let [x, bottom, fw, fh] = find_in_grill(&rgba, w, h, grill, 4.0).expect("found");
            // in the panel's window (x 240..960, rows 834..966 of the picture), cut to 4:1 about its middle
            let top = h as i32 - 1 - bottom - fh;
            assert!(x >= 240 && x + fw <= 960, "{window}: x {x}..{}", x + fw);
            assert!(top >= 834 && top + fh <= 967, "{window}: rows {top}..{}", top + fh);
            assert_eq!(fw, fh * 4, "{window}: shaped 4:1");
            assert!(fh >= 120, "{window}: as high as the window allows ({fh})");
            assert!((x + fw / 2 - 600).abs() <= 2, "{window}: about the middle");
        }
        // no window in the middle (a plain panel), or no panel: none found
        let plain: Vec<u8> = (0..w * h).flat_map(|_| [4u8, 4, 4, 255]).collect();
        assert_eq!(find_in_grill(&plain, w, h, grill, 4.0), None);
        assert_eq!(find_in_grill(&speaker_panel(w, h, grill, 40, 4), w, h, 0, 4.0), None);
    }

    #[test]
    fn places_count_as_vpx_counts_them() {
        assert_eq!(Overlay::parse("auto"), Some(Overlay::Auto));
        assert_eq!(Overlay::parse("off"), Some(Overlay::Off));
        assert_eq!(Overlay::parse("10, 20,300,75"), Some(Overlay::At([10, 20, 300, 75])));
        assert_eq!(Overlay::parse("10,20,0,75"), None);
        assert_eq!(Overlay::parse("big"), None);
        // the window found shows a row lower than found, as VPX draws it
        assert_eq!(Overlay::Auto.place(Some([80, 19, 240, 60]), 200.0), Some([80.0, 121.0, 240.0, 60.0]));
        assert_eq!(Overlay::Auto.place(None, 200.0), None);
        // on a page shown 50 rows shorter (a grill cut off), 50 higher
        assert_eq!(Overlay::Auto.place(Some([80, 19, 240, 60]), 150.0), Some([80.0, 71.0, 240.0, 60.0]));
        // a place by hand counts up from the bottom edge
        assert_eq!(Overlay::At([10, 20, 300, 75]).place(None, 1080.0), Some([10.0, 985.0, 300.0, 75.0]));
        assert_eq!(Overlay::Off.place(Some([80, 19, 240, 60]), 200.0), None);
    }
}
