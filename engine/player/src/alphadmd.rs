// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its AlphaDMD
// plugin's picture of a segment game's displays (plugins/alphadmd/alphadmd.cpp: segDisplays, nSegments, DrawChar,
// DrawDisplay and RenderThread's "Render for DMD").
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX 5436's AlphaDMD plugin draws a segment game's displays as a 128 × 32
//! dot-matrix picture — each digit's segments as short lines of dots, at
//! their brightness, held to a hundredth at least so unlit segments glow
//! faintly, added where lines meet, up to full — which it offers as a
//! display of its own: what a real DMD panel shows for those games
//! (`dmd_panel`). Its other picture, the one colour files were made from,
//! is `alphanumeric`.

use crate::segments::{SegKind, SegmentsNow};

/// A segment's dots, (x, y) from the digit's top left.
type Line = &'static [(u8, u8)];

/// How a kind of digit is drawn (`segDisplay`): its width, spacing included, and its segments.
struct Image {
    width: i32,
    segs: [Line; 16],
}

/// 14 segments, dot and comma, 8 × 11 and 6 × 11; 16 segments; 9 segments and comma, 8 × 11 and
/// 8 × 7; 9 segments and dot (`segDisplays`).
const SEG14DC_8X11: Image = Image {
    width: 8,
    segs: [
        &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 0)],
        &[(6, 0), (6, 1), (6, 2), (6, 3), (6, 4)],
        &[(6, 6), (6, 7), (6, 8), (6, 9), (6, 10)],
        &[(1, 10), (2, 10), (3, 10), (4, 10), (5, 10)],
        &[(0, 6), (0, 7), (0, 8), (0, 9), (0, 10)],
        &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)],
        &[(1, 5), (2, 5)],
        &[(7, 9), (7, 10)],
        &[(0, 0), (1, 1), (1, 2), (2, 3), (2, 4)],
        &[(3, 0), (3, 1), (3, 2), (3, 3), (3, 4)],
        &[(6, 0), (5, 1), (5, 2), (4, 3), (4, 4)],
        &[(4, 5), (5, 5)],
        &[(4, 6), (4, 7), (5, 8), (5, 9), (6, 10)],
        &[(3, 6), (3, 7), (3, 8), (3, 9), (3, 10)],
        &[(0, 10), (2, 6), (2, 7), (1, 8), (1, 9)],
        &[(7, 10)],
    ],
};

const SEG14DC_6X11: Image = Image {
    width: 6,
    segs: [
        &[(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)],
        &[(4, 0), (4, 1), (4, 2), (4, 3), (4, 4)],
        &[(4, 6), (4, 7), (4, 8), (4, 9), (4, 10)],
        &[(0, 10), (1, 10), (2, 10), (3, 10), (4, 10)],
        &[(0, 6), (0, 7), (0, 8), (0, 9), (0, 10)],
        &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)],
        &[(0, 5), (1, 5), (2, 5)],
        &[(5, 9), (5, 10)],
        &[(0, 0), (1, 1), (1, 2), (2, 3), (2, 4)],
        &[(2, 0), (2, 1), (2, 2), (2, 3), (2, 4)],
        &[(4, 0), (3, 1), (3, 2), (2, 3), (2, 4)],
        &[(2, 5), (3, 5), (4, 5)],
        &[(2, 6), (2, 7), (3, 8), (3, 9), (4, 10)],
        &[(2, 6), (2, 7), (2, 8), (2, 9), (2, 10)],
        &[(0, 10), (2, 6), (2, 7), (1, 8), (1, 9)],
        &[(5, 10)],
    ],
};

const SEG16_7X11: Image = Image {
    width: 8,
    segs: [
        &[(1, 0), (2, 0)],
        &[(4, 0), (5, 0)],
        &[(6, 0), (6, 1), (6, 2), (6, 3), (6, 4)],
        &[(6, 6), (6, 7), (6, 8), (6, 9), (6, 10)],
        &[(4, 10), (5, 10)],
        &[(1, 10), (2, 10)],
        &[(0, 6), (0, 7), (0, 8), (0, 9), (0, 10)],
        &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)],
        &[(0, 0), (1, 1), (1, 2), (2, 3), (2, 4)],
        &[(3, 0), (3, 1), (3, 2), (3, 3), (3, 4)],
        &[(6, 0), (5, 1), (5, 2), (4, 3), (4, 4)],
        &[(4, 5), (5, 5)],
        &[(4, 6), (4, 7), (5, 8), (5, 9), (6, 10)],
        &[(3, 6), (3, 7), (3, 8), (3, 9), (3, 10)],
        &[(0, 10), (2, 6), (2, 7), (1, 8), (1, 9)],
        &[(1, 5), (2, 5)],
    ],
};

const SEG9C_8X11: Image = Image {
    width: 8,
    segs: [
        &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 0)],
        &[(6, 0), (6, 1), (6, 2), (6, 3), (6, 4)],
        &[(6, 6), (6, 7), (6, 8), (6, 9), (6, 10)],
        &[(1, 10), (2, 10), (3, 10), (4, 10), (5, 10)],
        &[(0, 6), (0, 7), (0, 8), (0, 9), (0, 10)],
        &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)],
        &[(1, 5), (2, 5), (3, 5), (4, 5), (5, 5)],
        &[(7, 9), (7, 10)],
        &[(3, 0), (3, 1), (3, 2), (3, 3), (3, 4)],
        &[(3, 6), (3, 7), (3, 8), (3, 9), (3, 10)],
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
    ],
};

const SEG9C_8X7: Image = Image {
    width: 8,
    segs: [
        &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 0)],
        &[(6, 1), (6, 2)],
        &[(6, 4), (6, 5)],
        &[(1, 6), (2, 6), (3, 6), (4, 6), (5, 6)],
        &[(0, 4), (0, 5)],
        &[(0, 1), (0, 2)],
        &[(1, 3), (2, 3), (3, 3), (4, 3), (5, 3)],
        &[(7, 5), (7, 6)],
        &[(3, 0), (3, 1), (3, 2)],
        &[(3, 4), (3, 5), (3, 6)],
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
    ],
};

const SEG9D_8X11: Image = Image {
    width: 8,
    segs: [
        &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 0)],
        &[(6, 0), (6, 1), (6, 2), (6, 3), (6, 4)],
        &[(6, 6), (6, 7), (6, 8), (6, 9), (6, 10)],
        &[(1, 10), (2, 10), (3, 10), (4, 10), (5, 10)],
        &[(0, 6), (0, 7), (0, 8), (0, 9), (0, 10)],
        &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)],
        &[(1, 5), (2, 5), (3, 5), (4, 5), (5, 5)],
        &[(7, 10)],
        &[(3, 0), (3, 1), (3, 2), (3, 3), (3, 4)],
        &[(3, 6), (3, 7), (3, 8), (3, 9), (3, 10)],
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
    ],
};

/// The segments each kind has (`nSegments`).
const SEGMENTS: [usize; 9] = [7, 8, 8, 10, 10, 15, 15, 16, 16];

/// The image a kind of digit is drawn with, large (11 dots high) or small (`DrawDisplay`).
fn image(kind: SegKind, large: bool) -> &'static Image {
    match kind {
        SegKind::Seven | SegKind::SevenComma | SegKind::Nine | SegKind::NineComma => {
            if large {
                &SEG9C_8X11
            } else {
                &SEG9C_8X7
            }
        }
        SegKind::SevenDot => &SEG9D_8X11,
        SegKind::Fourteen | SegKind::FourteenDot | SegKind::FourteenDotComma => {
            if large {
                &SEG14DC_8X11
            } else {
                &SEG14DC_6X11
            }
        }
        SegKind::Sixteen => &SEG16_7X11,
    }
}

/// Each layout's displays: where each is drawn (left, top) and whether large (the "Render for
/// DMD" switch), in the displays' order.
fn places(layout: &str) -> Option<&'static [(i32, i32, bool)]> {
    Some(match layout {
        "6x4_2x2" => &[(2, 2, false), (92, 2, false), (2, 12, false), (92, 12, false), (2, 22, false), (92, 22, false), (56, 7, false), (56, 20, false)],
        "4x6_2x2" => &[(0, 0, true), (80, 0, true), (0, 12, true), (80, 12, true), (8, 24, false), (32, 24, false)],
        "4x7" => &[(0, 2, true), (72, 2, true), (0, 19, true), (72, 19, true)],
        "4x7_2x2" => &[(0, 0, true), (72, 0, true), (0, 12, true), (72, 12, true), (8, 24, false), (32, 24, false)],
        "4x7_5x2" => &[(0, 0, true), (72, 0, true), (0, 12, true), (72, 12, true), (16, 24, false), (40, 24, false), (64, 24, false), (88, 24, false), (112, 24, false)],
        "2x16" => &[(0, 2, true), (0, 19, true)],
        "1x7_2x16" => &[(68, 1, false), (0, 9, true), (0, 21, true)],
        "2x16_1x7" => &[(0, 9, true), (0, 21, true), (68, 1, false)],
        "1x7_1x4_2x16" => &[(0, 1, false), (96, 1, false), (0, 9, true), (0, 21, true)],
        "2x7_2x2_1x16" => &[(0, 0, true), (72, 0, true), (16, 12, false), (40, 12, false), (16, 21, true)],
        "2x20" => &[(4, 2, false), (4, 19, false)],
        _ => return None,
    })
}

/// AlphaDMD's picture of the game's segment displays now: a brightness a dot, 128 × 32, row by
/// row; None when AlphaDMD has no layout for them.
pub fn render(now: &SegmentsNow) -> Option<Vec<f32>> {
    let layout = crate::segments::alpha_layout(now)?;
    let places = places(layout)?;
    let mut frame = vec![0.0f32; 128 * 32];
    // each display's digits after the one before, as AlphaDMD's brightness pointer walks them
    let mut levels = now.levels.iter();
    for (display, &(left, top, large)) in now.layout.displays.iter().zip(places) {
        let mut x = left;
        for digit in &display.digits {
            let Some(level) = levels.next() else { break };
            let img = image(digit.kind, large);
            for (seg, line) in img.segs.iter().enumerate().take(SEGMENTS[digit.kind.index()]) {
                let v = level[seg].clamp(0.01, 1.0);
                for &(dx, dy) in *line {
                    let (px, py) = (x + i32::from(dx), top + i32::from(dy));
                    if (0..128).contains(&px) && (0..32).contains(&py) {
                        let dot = &mut frame[(py * 128 + px) as usize];
                        *dot = (*dot + v).min(1.0);
                    }
                }
            }
            x += img.width;
        }
    }
    Some(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segments::{Digit, SegDisplay, SegLayout, levels};
    use std::sync::Arc;

    /// Gorgar's displays, every digit showing 8 but the first: drawn where AlphaDMD draws them,
    /// lit dots full, unlit segments' dots a hundredth.
    #[test]
    fn segment_digits_are_drawn_as_alphadmd_draws_them() {
        let digit = |display| Digit { display, index: 0, kind: SegKind::Seven, swapped_marks: false, glyph: [0.0; 4] };
        let counts = [6usize, 6, 6, 6, 2, 2];
        let displays: Vec<SegDisplay> = counts.iter().enumerate().map(|(d, &n)| SegDisplay { digits: vec![digit(d); n], hardware: 0 }).collect();
        let mut lv = vec![levels(0x7f, false); 28];
        lv[0] = levels(0, false);
        let now = SegmentsNow { layout: Arc::new(SegLayout { displays, bounds: [0.0; 4] }), levels: lv, id: 1 };
        let f = render(&now).expect("AlphaDMD's 4x6_2x2");
        // the first digit, unlit: its top segment's dots at a hundredth (large, at 0,0: top is 1..5,0)
        assert!((1..=5).all(|x| f[x] == 0.01), "{:?}", &f[..8]);
        // the second digit lit: its top (x 9..13), full
        assert!((9..=13).all(|x| f[x] == 1.0));
        // the first digit's comma dots (7, 9 and 10) aren't drawn: seven segments only
        assert_eq!(f[9 * 128 + 7], 0.0);
        // the small credit display at (8, 24): its middle bar on row 27
        assert_eq!(f[27 * 128 + 9], 1.0);
        // a segment layout AlphaDMD doesn't take
        let odd = SegmentsNow { layout: Arc::new(SegLayout { displays: vec![SegDisplay { digits: vec![digit(0); 3], hardware: 0 }], bounds: [0.0; 4] }), levels: vec![levels(0, false); 3], id: 1 };
        assert!(render(&odd).is_none());
    }
}
