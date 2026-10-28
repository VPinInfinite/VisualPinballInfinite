// SPDX-License-Identifier: BSD-3-Clause
//
// Ported from Visual Pinball X build 5436 (commit af26b2d),
// plugins/alphadmd/usbalphanumeric.h, "license:BSD-3-Clause" (PinMAME's
// DmdDevice code for USB DMDs, as VPX's AlphaDMD plugin carries it), by the
// Visual Pinball Infinite contributors, 2026.

//! A segment-display game's digits drawn as dot-matrix dots, 128 × 32, each
//! dot 0 to 3, the way PinMAME's DMD devices drew them for real DMDs — the
//! frame colour files made for segment games (Gorgar's Serum file) were made
//! from, which VPX 5436's AlphaDMD plugin hands to colourisation as its
//! identify frame. Each layout takes PinMAME's segment states in its order:
//! one 16-bit word a digit, a bit a segment.

/// The frame: a level 0..3 a dot, row by row.
pub struct AlphaFrame {
    pub dots: [u8; 128 * 32],
}

/// How many points each segment has, per segment kind (`segSizes`): 0
/// alphanumeric, 1 seven segments, 2 ten segments, 5 small seven segments,
/// 7 alphanumeric 6 dots wide; the others have none.
const SEG_SIZES: [[u8; 16]; 8] = [
    [5, 5, 5, 5, 5, 5, 2, 2, 5, 5, 5, 2, 5, 5, 5, 1],
    [5, 5, 5, 5, 5, 5, 5, 2, 0, 0, 0, 0, 0, 0, 0, 0],
    [5, 5, 5, 5, 5, 5, 5, 2, 5, 5, 0, 0, 0, 0, 0, 0],
    [0; 16],
    [0; 16],
    [5, 2, 2, 5, 2, 2, 5, 2, 0, 0, 0, 0, 0, 0, 0, 0],
    [0; 16],
    [5, 5, 5, 5, 5, 5, 3, 2, 5, 5, 5, 3, 5, 5, 5, 1],
];

type Seg = [[u8; 2]; 5];
const Z: Seg = [[0, 0]; 5];

/// Each segment's points, per kind (`segs`), as (x, y) from the digit's top left.
const SEGS: [[Seg; 16]; 8] = [
    // alphanumeric
    [
        [[1, 0], [2, 0], [3, 0], [4, 0], [5, 0]],
        [[6, 0], [6, 1], [6, 2], [6, 3], [6, 4]],
        [[6, 6], [6, 7], [6, 8], [6, 9], [6, 10]],
        [[1, 10], [2, 10], [3, 10], [4, 10], [5, 10]],
        [[0, 6], [0, 7], [0, 8], [0, 9], [0, 10]],
        [[0, 0], [0, 1], [0, 2], [0, 3], [0, 4]],
        [[1, 5], [2, 5], [0, 0], [0, 0], [0, 0]],
        [[7, 9], [7, 10], [0, 0], [0, 0], [0, 0]],
        [[0, 0], [1, 1], [1, 2], [2, 3], [2, 4]],
        [[3, 0], [3, 1], [3, 2], [3, 3], [3, 4]],
        [[6, 0], [5, 1], [5, 2], [4, 3], [4, 4]],
        [[4, 5], [5, 5], [0, 0], [0, 0], [0, 0]],
        [[4, 6], [4, 7], [5, 8], [5, 9], [6, 10]],
        [[3, 6], [3, 7], [3, 8], [3, 9], [3, 10]],
        [[0, 10], [2, 6], [2, 7], [1, 8], [1, 9]],
        [[7, 10], [0, 0], [0, 0], [0, 0], [0, 0]],
    ],
    // seven segments
    [
        [[1, 0], [2, 0], [3, 0], [4, 0], [5, 0]],
        [[6, 0], [6, 1], [6, 2], [6, 3], [6, 4]],
        [[6, 6], [6, 7], [6, 8], [6, 9], [6, 10]],
        [[1, 10], [2, 10], [3, 10], [4, 10], [5, 10]],
        [[0, 6], [0, 7], [0, 8], [0, 9], [0, 10]],
        [[0, 0], [0, 1], [0, 2], [0, 3], [0, 4]],
        [[1, 5], [2, 5], [3, 5], [4, 5], [5, 5]],
        [[7, 9], [7, 10], [0, 0], [0, 0], [0, 0]],
        Z, Z, Z, Z, Z, Z, Z, Z,
    ],
    // ten segments
    [
        [[1, 0], [2, 0], [3, 0], [4, 0], [5, 0]],
        [[6, 0], [6, 1], [6, 2], [6, 3], [6, 4]],
        [[6, 6], [6, 7], [6, 8], [6, 9], [6, 10]],
        [[1, 10], [2, 10], [3, 10], [4, 10], [5, 10]],
        [[0, 6], [0, 7], [0, 8], [0, 9], [0, 10]],
        [[0, 0], [0, 1], [0, 2], [0, 3], [0, 4]],
        [[1, 5], [2, 5], [3, 5], [4, 5], [5, 5]],
        [[7, 9], [7, 10], [0, 0], [0, 0], [0, 0]],
        [[3, 0], [3, 1], [3, 2], [3, 3], [3, 4]],
        [[3, 6], [3, 7], [3, 8], [3, 9], [3, 10]],
        Z, Z, Z, Z, Z, Z,
    ],
    [Z; 16],
    [Z; 16],
    // small seven segments
    [
        [[1, 0], [2, 0], [3, 0], [4, 0], [5, 0]],
        [[6, 1], [6, 2], [0, 0], [0, 0], [0, 0]],
        [[6, 4], [6, 5], [0, 0], [0, 0], [0, 0]],
        [[1, 6], [2, 6], [3, 6], [4, 6], [5, 6]],
        [[0, 4], [0, 5], [0, 0], [0, 0], [0, 0]],
        [[0, 1], [0, 2], [0, 0], [0, 0], [0, 0]],
        [[1, 3], [2, 3], [3, 3], [4, 3], [5, 3]],
        [[7, 5], [7, 6], [0, 0], [0, 0], [0, 0]],
        Z, Z, Z, Z, Z, Z, Z, Z,
    ],
    [Z; 16],
    // alphanumeric, 6 dots wide
    [
        [[0, 0], [1, 0], [2, 0], [3, 0], [4, 0]],
        [[4, 0], [4, 1], [4, 2], [4, 3], [4, 4]],
        [[4, 6], [4, 7], [4, 8], [4, 9], [4, 10]],
        [[0, 10], [1, 10], [2, 10], [3, 10], [4, 10]],
        [[0, 6], [0, 7], [0, 8], [0, 9], [0, 10]],
        [[0, 0], [0, 1], [0, 2], [0, 3], [0, 4]],
        [[0, 5], [1, 5], [2, 5], [0, 0], [0, 0]],
        [[5, 9], [5, 10], [0, 0], [0, 0], [0, 0]],
        [[0, 0], [1, 1], [1, 2], [2, 3], [2, 4]],
        [[2, 0], [2, 1], [2, 2], [2, 3], [2, 4]],
        [[4, 0], [3, 1], [3, 2], [2, 3], [2, 4]],
        [[2, 5], [3, 5], [4, 5], [0, 0], [0, 0]],
        [[2, 6], [2, 7], [3, 8], [3, 9], [4, 10]],
        [[2, 6], [2, 7], [2, 8], [2, 9], [2, 10]],
        [[0, 10], [2, 6], [2, 7], [1, 8], [1, 9]],
        [[5, 10], [0, 0], [0, 0], [0, 0], [0, 0]],
    ],
];

/// The layouts PinMAME's DMD devices drew segment games in, by their
/// displays (VPX's AlphaDMD picks one from the game's displays).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Alpha2x16,
    Alpha2x20,
    Alpha2x7Num2x7,
    Alpha2x7Num2x7Num4x1,
    Num2x6Num2x6Num4x1,
    Num10x2x6Num10x2x6Num4x1,
    Num2x7Num2x7Num4x1,
    Num2x7Num2x7Num10x1,
    Num2x7Num2x7Num4x1Gen7,
    Num10x2x7Num10x2x7Num4x1,
    Num10x4x7,
    Num6x4Num4x1,
    Num2x7Num4x1Alpha1x16,
    Alpha1x16Num1x16Num1x7,
    Num1x7Alpha1x16Num1x16,
    Alpha1x16Num1x16Num1x7Num1x4,
}

impl AlphaFrame {
    pub fn new() -> AlphaFrame {
        AlphaFrame { dots: [0; 128 * 32] }
    }

    /// The frame for `layout` from the game's segment states (`extra`: the six more digits of
    /// the 2 × 7, 2 × 7, 10 × 1 layout).
    pub fn draw(layout: Layout, seg: &[u16], extra: &[u16]) -> AlphaFrame {
        let mut f = AlphaFrame::new();
        let s = |i: usize| seg.get(i).copied().unwrap_or(0);
        let gap = |i: usize, half: usize, skip: usize| (i + if i < half { 0 } else { skip }) as i32 * 8;
        match layout {
            Layout::Alpha2x16 => {
                for i in 0..16 {
                    f.digit(i as i32 * 8, 2, 0, s(i));
                    f.digit(i as i32 * 8, 19, 0, s(i + 16));
                    f.smooth(i as i32 * 8, 2);
                    f.smooth(i as i32 * 8, 19);
                }
            }
            Layout::Alpha2x20 => {
                for i in 0..20 {
                    let x = i as i32 * 6 + 4;
                    f.digit(x, 2, 7, s(i));
                    f.digit(x, 19, 7, s(i + 20));
                    f.smooth6(x, 2);
                    f.smooth6(x, 19);
                }
            }
            Layout::Alpha2x7Num2x7 => {
                for i in 0..14 {
                    let x = gap(i, 7, 2);
                    f.digit(x, 2, 0, s(i));
                    f.digit(x, 19, 1, s(i + 14));
                    f.smooth(x, 2);
                    f.smooth(x, 19);
                }
            }
            Layout::Alpha2x7Num2x7Num4x1 => {
                for i in 0..14 {
                    let x = gap(i, 7, 2);
                    f.digit(x, 0, 0, s(i));
                    f.digit(x, 21, 1, s(i + 14));
                    f.smooth(x, 0);
                    f.smooth(x, 21);
                }
                f.small4(&[(8, 12), (16, 12), (32, 12), (40, 12)], [s(28), s(29), s(30), s(31)]);
            }
            Layout::Num2x6Num2x6Num4x1 => {
                for i in 0..12 {
                    let x = gap(i, 6, 4);
                    f.digit(x, 0, 1, s(i));
                    f.digit(x, 12, 1, s(i + 12));
                    f.smooth(x, 0);
                    f.smooth(x, 12);
                }
                f.small4(&[(8, 24), (16, 24), (32, 24), (40, 24)], [s(24), s(25), s(26), s(27)]);
            }
            Layout::Num10x2x6Num10x2x6Num4x1 => {
                for i in 0..12 {
                    let x = gap(i, 6, 4);
                    f.digit(x, 0, 2, s(i));
                    f.digit(x, 20, 2, s(i + 12));
                    f.smooth(x, 0);
                    f.smooth(x, 20);
                }
                f.small4(&[(8, 12), (16, 12), (32, 12), (40, 12)], [s(24), s(25), s(26), s(27)]);
            }
            Layout::Num2x7Num2x7Num4x1 | Layout::Num2x7Num2x7Num10x1 => {
                for i in 0..14 {
                    let x = gap(i, 7, 2);
                    f.digit(x, 0, 1, s(i));
                    f.digit(x, 12, 1, s(i + 14));
                    f.smooth(x, 0);
                    f.smooth(x, 12);
                }
                f.small4(&[(16, 24), (24, 24), (40, 24), (48, 24)], [s(28), s(29), s(30), s(31)]);
                if layout == Layout::Num2x7Num2x7Num10x1 {
                    let e = |i: usize| extra.get(i).copied().unwrap_or(0);
                    for (k, x) in [64, 72, 88, 96, 112, 120].into_iter().enumerate() {
                        f.digit(x, 24, 5, e(k));
                    }
                }
            }
            Layout::Num2x7Num2x7Num4x1Gen7 => {
                for i in 0..14 {
                    let x = gap(i, 7, 2);
                    f.digit(x, 21, 1, s(i));
                    f.digit(x, 1, 1, s(i + 14));
                    f.smooth(x, 21);
                    f.smooth(x, 1);
                }
                f.small4(&[(8, 13), (16, 13), (32, 13), (40, 13)], [s(28), s(29), s(30), s(31)]);
            }
            Layout::Num10x2x7Num10x2x7Num4x1 => {
                for i in 0..14 {
                    let x = gap(i, 7, 2);
                    f.digit(x, 0, 2, s(i));
                    f.digit(x, 20, 2, s(i + 14));
                    f.smooth(x, 0);
                    f.smooth(x, 20);
                }
                f.small4(&[(8, 12), (16, 12), (32, 12), (40, 12)], [s(28), s(29), s(30), s(31)]);
            }
            Layout::Num10x4x7 => {
                for i in 0..14 {
                    let x = gap(i, 7, 2);
                    f.digit(x, 1, 2, s(i));
                    f.digit(x, 13, 2, s(i + 14));
                    f.smooth(x, 1);
                    f.smooth(x, 13);
                }
            }
            Layout::Num6x4Num4x1 => {
                for i in 0..8 {
                    let x = gap(i, 4, 2);
                    f.digit(x, 1, 5, s(i));
                    f.digit(x, 9, 5, s(i + 8));
                    f.digit(x, 17, 5, s(i + 16));
                    f.smooth(x, 1);
                    f.smooth(x, 9);
                    f.smooth(x, 17);
                }
                f.small4(&[(16, 25), (24, 25), (48, 25), (56, 25)], [s(24), s(25), s(26), s(27)]);
            }
            Layout::Num2x7Num4x1Alpha1x16 => {
                for i in 0..14 {
                    let x = gap(i, 7, 2);
                    f.digit(x, 0, 1, s(i));
                    f.smooth(x, 0);
                }
                f.small4(&[(16, 12), (24, 12), (40, 12), (48, 12)], [s(14), s(15), s(16), s(17)]);
                for i in 0..12 {
                    let x = i as i32 * 8 + 16;
                    f.digit(x, 21, 0, s(i + 18));
                    f.smooth(x, 21);
                }
            }
            Layout::Alpha1x16Num1x16Num1x7 => {
                for i in 0..16 {
                    f.digit(i as i32 * 8, 9, 0, s(i));
                    f.smooth(i as i32 * 8, 9);
                }
                for i in 0..16 {
                    f.digit(i as i32 * 8, 21, 1, s(i + 16));
                    f.smooth(i as i32 * 8, 21);
                }
                for i in 0..7 {
                    f.digit(i as i32 * 8 + 68, 1, 5, s(i + 32));
                }
            }
            Layout::Num1x7Alpha1x16Num1x16 => {
                for i in 0..16 {
                    f.digit(i as i32 * 8, 9, 0, s(i + 8));
                    f.smooth(i as i32 * 8, 9);
                }
                for i in 0..16 {
                    f.digit(i as i32 * 8, 21, 1, s(i + 24));
                    f.smooth(i as i32 * 8, 21);
                }
                for i in 0..7 {
                    f.digit(i as i32 * 8 + 68, 1, 5, s(i + 1));
                }
            }
            Layout::Alpha1x16Num1x16Num1x7Num1x4 => {
                for i in 0..16 {
                    f.digit(i as i32 * 8, 9, 0, s(i + 11));
                    f.smooth(i as i32 * 8, 9);
                }
                for i in 0..16 {
                    f.digit(i as i32 * 8, 21, 1, s(i + 27));
                    f.smooth(i as i32 * 8, 21);
                }
                for i in 0..4 {
                    f.digit(i as i32 * 8 + 4, 1, 5, s(i + 7));
                }
                for i in 0..7 {
                    f.digit(i as i32 * 8 + 68, 1, 5, s(i));
                }
            }
        }
        f
    }

    fn pixel(&self, x: i32, y: i32) -> bool {
        (0..128).contains(&x) && (0..32).contains(&y) && self.dots[(y * 128 + x) as usize] != 0
    }

    fn set(&mut self, x: i32, y: i32, level: u8) {
        if (0..128).contains(&x) && (0..32).contains(&y) {
            self.dots[(y * 128 + x) as usize] = level;
        }
    }

    /// One digit's lit segments (`drawSegment` for each set bit), at full brightness.
    fn digit(&mut self, x: i32, y: i32, kind: usize, bits: u16) {
        for seg in 0..16 {
            if (bits >> seg) & 1 != 0 {
                for p in &SEGS[kind][seg][..usize::from(SEG_SIZES[kind][seg])] {
                    self.set(i32::from(p[0]) + x, i32::from(p[1]) + y, 3);
                }
            }
        }
    }

    /// Four small digits, at their places.
    fn small4(&mut self, places: &[(i32, i32); 4], bits: [u16; 4]) {
        for (&(x, y), b) in places.iter().zip(bits) {
            self.digit(x, y, 5, b);
        }
    }

    /// Rounded corners (`smoothDigitCorners`): a corner dot taken off where both its neighbours are lit.
    fn smooth(&mut self, x: i32, y: i32) {
        if self.pixel(x, 1 + y) && self.pixel(1 + x, y) {
            self.set(x, y, 0);
        }
        if self.pixel(x + 6, 1 + y) && self.pixel(5 + x, y) {
            self.set(6 + x, y, 0);
        }
        if self.pixel(x, 9 + y) && self.pixel(1 + x, 10 + y) {
            self.set(x, 10 + y, 0);
        }
        if self.pixel(x + 6, 9 + y) && self.pixel(5 + x, 10 + y) {
            self.set(6 + x, 10 + y, 0);
        }
    }

    /// The same for 6-dot-wide digits (`smoothDigitCorners6Px`).
    fn smooth6(&mut self, x: i32, y: i32) {
        if self.pixel(x, 1 + y) && self.pixel(1 + x, y) {
            self.set(x, y, 0);
        }
        if self.pixel(x + 4, 1 + y) && self.pixel(3 + x, y) {
            self.set(4 + x, y, 0);
        }
        if self.pixel(x, 9 + y) && self.pixel(1 + x, 10 + y) {
            self.set(x, 10 + y, 0);
        }
        if self.pixel(x + 4, 9 + y) && self.pixel(3 + x, 10 + y) {
            self.set(4 + x, 10 + y, 0);
        }
    }
}

impl Default for AlphaFrame {
    fn default() -> AlphaFrame {
        AlphaFrame::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(f: &AlphaFrame) -> usize {
        f.dots.iter().filter(|&&d| d != 0).count()
    }

    #[test]
    fn a_seven_segment_eight_lights_its_segments_with_rounded_corners() {
        // "8": segments 0..6 lit (top, right top, right bottom, bottom, left bottom, left top, middle)
        let f = AlphaFrame::draw(Layout::Num2x6Num2x6Num4x1, &[0x7f], &[]);
        // 7 segments of 5 points each, none shared, then the four rounded corners taken off
        assert!(f.pixel(3, 0) && f.pixel(3, 5) && f.pixel(3, 10));
        assert!(!f.pixel(0, 0) && !f.pixel(0, 10));
        assert_eq!(lit(&f), 35 - 4);
        // nothing else on the frame
        assert!(f.dots[128 * 12..].iter().all(|&d| d == 0));
    }

    #[test]
    fn the_second_half_of_a_row_skips_a_gap() {
        // the seventh digit (index 6) of a 2 × 6 row sits 4 digits further on: x = (6 + 4) × 8
        let mut seg = [0u16; 28];
        seg[6] = 1; // its top segment
        let f = AlphaFrame::draw(Layout::Num2x6Num2x6Num4x1, &seg, &[]);
        assert!(f.pixel(81, 0) && f.pixel(85, 0));
        assert_eq!(lit(&f), 5);
    }

    #[test]
    fn levels_are_full_brightness() {
        let f = AlphaFrame::draw(Layout::Alpha2x16, &[0xffff; 32], &[]);
        assert!(f.dots.iter().all(|&d| d == 0 || d == 3));
        assert!(lit(&f) > 0);
    }
}
