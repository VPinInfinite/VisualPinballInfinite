// Follows libpupdmd (PPUC, https://github.com/PPUC/libpupdmd, GPL-3.0), version 0.4.1 as VPX build
// 5436 ships it (src/pupdmd.cpp, commit 0eecb74a: "Emit PUP triggers from DMD frames"), and VPX
// 5436's PinUp Player plugin (plugins/pup/PUPManager.cpp, `ProcessDmdFrame`; license:GPLv3+,
// Copyright (C) 2000-2026 Visual Pinball development team and contributors).
// This port is under the GNU GPL version 3, as libpupdmd is.

//! A PinUP pack's frame triggers (PinUP's PupCapture): pictures of the
//! score display, `PupCapture\<n>.bmp`, each firing trigger `D<n>` when the
//! game's DMD shows it. The pictures are read once, as libpupdmd reads them:
//! each dot's shade from its red, a magenta frame drawn in a picture marking
//! the only part to compare. The game's frames come as PinMAME's raw shades
//! (its identify frames), brought to 128×32 as VPX's plugin does.
//!
//! libpupdmd compares hashes of the two; this compares the dots themselves,
//! the same answer without its rare false matches.

use std::collections::BTreeMap;
use std::path::Path;

/// The red, green and blue of a picture's mask frame (`PUPDMD_MASK_*`).
const MASK: [u8; 3] = [253, 0, 253];

/// One capture: its size and the dots to compare (a `PUPDMD::Hash`).
#[derive(Debug, Clone)]
struct Capture {
    width: u8,
    height: u8,
    /// The part compared, inside the mask frame: x, y, width, height. The whole picture without one.
    region: (usize, usize, usize, usize),
    /// The picture's shades, a byte a dot, row by row.
    shades: Vec<u8>,
}

/// A pack's captures and the matcher's memory (a `PUPDMD::DMD`), with the 128×32 frame VPX
/// builds from other sizes (`m_idFrame`).
#[derive(Debug, Default)]
pub struct Captures {
    by_trigger: BTreeMap<u16, Capture>,
    last_trigger: u16,
    frame: Vec<u8>,
}

impl Captures {
    /// Reads `pack\PupCapture\*.bmp` (the folder's name in any case), shades for a DMD of
    /// `bits` a dot (2 or 4; VPX's `Load(path, "", bits)`).
    pub fn load(pack: &Path, bits: u32) -> Captures {
        let mut c = Captures { frame: vec![0; 128 * 32], ..Captures::default() };
        let Some(folder) = super::find_case_insensitive(&pack.join("PupCapture"), true) else {
            return c;
        };
        let Ok(entries) = std::fs::read_dir(&folder) else { return c };
        // in name order, as Windows lists a folder: a later picture of the same number replaces one
        let mut files: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        files.sort();
        for file in files {
            let Some(name) = file.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()) else { continue };
            let Some(trigger) = trigger_number(&name) else { continue };
            if let Ok(bytes) = std::fs::read(&file)
                && let Some(capture) = read_capture(&bytes, bits)
            {
                c.by_trigger.insert(trigger, capture);
            }
        }
        c
    }

    /// How many pictures were read.
    pub fn len(&self) -> usize {
        self.by_trigger.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_trigger.is_empty()
    }

    /// The game's new frame (`width`×`height`, a byte a dot): the trigger it fires, 0 for none,
    /// -1 for a size VPX's plugin doesn't compare (`ProcessDmdFrame`).
    pub fn process(&mut self, width: usize, height: usize, frame: &[u8]) -> i32 {
        if frame.len() < width * height {
            return -1;
        }
        if width == 128 && height == 32 {
            return i32::from(self.match_frame(frame));
        }
        // a 128×16 frame in the middle of 128×32
        if width == 128 && height == 16 {
            self.frame[128 * 8..128 * 24].copy_from_slice(&frame[..128 * 16]);
            let f = std::mem::take(&mut self.frame);
            let t = self.match_frame(&f);
            self.frame = f;
            return i32::from(t);
        }
        // 192×64 or 256×64: halved by a 2×2 box filter (as DMD Extensions resized them),
        // centred
        if width <= 256 && height == 64 {
            let ofs = (128 - width / 2) / 2;
            for y in 0..32 {
                for x in 0..width / 2 {
                    let (px, py) = (x * 2, y * 2 * width);
                    let lum = u32::from(frame[py + px]) + u32::from(frame[py + px + 1]) + u32::from(frame[py + px + width]) + u32::from(frame[py + px + width + 1]);
                    self.frame[y * 128 + ofs + x] = ((lum + 2) >> 2) as u8;
                }
            }
            let f = std::mem::take(&mut self.frame);
            let t = self.match_frame(&f);
            self.frame = f;
            return i32::from(t);
        }
        -1
    }

    /// A 128×32 frame against every 128×32 capture, lowest number first (`MatchIndexed`): the
    /// first that matches, unless it's the one that matched last time, which fires nothing.
    fn match_frame(&mut self, frame: &[u8]) -> u16 {
        for (&trigger, c) in &self.by_trigger {
            if c.width != 128 || c.height != 32 {
                continue;
            }
            if c.matches(frame) {
                if trigger != self.last_trigger {
                    self.last_trigger = trigger;
                    return trigger;
                }
                return 0;
            }
        }
        0
    }
}

impl Capture {
    /// Whether a 128×32 frame shows this capture's part.
    fn matches(&self, frame: &[u8]) -> bool {
        let (x, y, w, h) = self.region;
        if x + w > 128 || y + h > 32 || self.shades.len() < 128 * 32 {
            return false;
        }
        (y..y + h).all(|row| frame[row * 128 + x..row * 128 + x + w] == self.shades[row * 128 + x..row * 128 + x + w])
    }
}

/// The trigger a file's name gives: the digits before `.bmp` (libpupdmd's `(\d+)\.bmp`, found
/// anywhere in the name).
fn trigger_number(lower: &str) -> Option<u16> {
    // the first ".bmp" preceded by digits
    let mut end = lower.find(".bmp")?;
    loop {
        let digits = lower[..end].bytes().rev().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            // std::stoi: the number as it reads (beyond 65535 it wraps, as the u16 it's kept in)
            let n: u64 = lower[end - digits..end].parse().ok()?;
            return Some(n as u16);
        }
        let next = end + 4;
        end = next + lower[next..].find(".bmp")?;
    }
}

/// A capture from a BMP file's bytes: 24-bit, uncompressed, 128×16, 128×32 or 192×64 (else none).
fn read_capture(bytes: &[u8], bits: u32) -> Option<Capture> {
    let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let i32_at = |o: usize| u32_at(o).map(|v| v as i32);
    if bytes.get(0..2)? != b"BM" {
        return None;
    }
    let file_size = u32_at(2)? as usize;
    let data_offset = u32_at(10)? as usize;
    let width = i32_at(18)?;
    let height = i32_at(22)?;
    let compression = u32_at(30)?;
    let image_size = u32_at(34)? as usize;
    if compression != 0 {
        return None;
    }
    let size = if image_size == 0 { file_size.checked_sub(data_offset)? } else { image_size };
    let fits = matches!((width, height), (128, 16) | (128, 32) | (192, 64));
    if !fits || size != (width * height * 3) as usize {
        return None;
    }
    let (w, h) = (width as usize, height as usize);
    let pixels = bytes.get(data_offset..data_offset + size)?;
    let mut shades = Vec::with_capacity(w * h);
    // the mask frame's top left corner, and how far it runs right along its top and down its left
    let (mut mx, mut my, mut mw, mut mh) = (255usize, 255usize, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            // bottom row first in the file, blue green red
            let at = ((h - 1 - y) * w + x) * 3;
            let (b, g, r) = (pixels[at], pixels[at + 1], pixels[at + 2]);
            shades.push(shade(r, bits));
            if [r, g, b] == MASK {
                if mx == 255 {
                    mx = x;
                    my = y;
                } else if mx < 192 {
                    if y == my {
                        mw += 1;
                    } else if x == mx {
                        mh += 1;
                    }
                }
            }
        }
    }
    // the part inside the frame (a degenerate frame wraps as libpupdmd's bytes do, and never matches)
    let region = if mx < 192 {
        (mx + 1, my + 1, (mw as u8).wrapping_sub(1) as usize, (mh as u8).wrapping_sub(1) as usize)
    } else {
        (0, 0, w, h)
    };
    Some(Capture { width: w as u8, height: h as u8, region, shades })
}

/// A picture's dot as the DMD's shade: PupCapture pictures are orange, so red is enough.
fn shade(r: u8, bits: u32) -> u8 {
    if r == MASK[0] || r < 8 {
        return 0;
    }
    if bits == 2 {
        return match r {
            0..48 => 1,
            48..128 => 2,
            _ => 3,
        };
    }
    match r {
        0..24 => 1,
        24..48 => 2,
        48..56 => 3,
        56..72 => 4,
        72..96 => 5,
        96..112 => 6,
        112..128 => 7,
        128..144 => 8,
        144..160 => 9,
        160..176 => 10,
        176..192 => 11,
        192..208 => 12,
        208..224 => 13,
        224..240 => 14,
        _ => 15,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 24-bit BMP of `w`×`h` from a dot's colour.
    fn bmp(w: usize, h: usize, colour: impl Fn(usize, usize) -> [u8; 3]) -> Vec<u8> {
        let size = w * h * 3;
        let mut b = Vec::with_capacity(54 + size);
        b.extend_from_slice(b"BM");
        b.extend_from_slice(&((54 + size) as u32).to_le_bytes());
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&54u32.to_le_bytes());
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&(w as i32).to_le_bytes());
        b.extend_from_slice(&(h as i32).to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&24u16.to_le_bytes());
        b.extend_from_slice(&[0; 4]); // compression
        b.extend_from_slice(&0u32.to_le_bytes()); // image size: from the file's
        b.extend_from_slice(&[0; 16]);
        for y in (0..h).rev() {
            for x in 0..w {
                let [r, g, bl] = colour(x, y);
                b.extend_from_slice(&[bl, g, r]);
            }
        }
        b
    }

    #[test]
    fn file_names_give_trigger_numbers() {
        assert_eq!(trigger_number("12.bmp"), Some(12));
        assert_eq!(trigger_number("capture007.bmp"), Some(7));
        assert_eq!(trigger_number("notes.txt"), None);
        assert_eq!(trigger_number("x.bmp"), None);
    }

    #[test]
    fn shades_follow_libpupdmds_thresholds() {
        assert_eq!([shade(0, 2), shade(7, 2), shade(8, 2), shade(47, 2), shade(48, 2), shade(128, 2), shade(253, 2)], [0, 0, 1, 1, 2, 3, 0]);
        assert_eq!([shade(23, 4), shade(24, 4), shade(127, 4), shade(240, 4)], [1, 2, 7, 15]);
    }

    #[test]
    fn a_whole_picture_fires_once_until_another_does() {
        // a picture lit along row 5
        let lit = bmp(128, 32, |_, y| if y == 5 { [255, 100, 0] } else { [0, 0, 0] });
        let other = bmp(128, 32, |_, y| if y == 9 { [255, 100, 0] } else { [0, 0, 0] });
        let mut c = Captures { frame: vec![0; 128 * 32], ..Captures::default() };
        c.by_trigger.insert(3, read_capture(&lit, 2).unwrap());
        c.by_trigger.insert(4, read_capture(&other, 2).unwrap());
        let frame = |row: usize| (0..128 * 32).map(|i| if i / 128 == row { 3 } else { 0 }).collect::<Vec<u8>>();
        assert_eq!(c.process(128, 32, &frame(5)), 3);
        assert_eq!(c.process(128, 32, &frame(5)), 0, "the same picture again fires nothing");
        assert_eq!(c.process(128, 32, &frame(0)), 0);
        assert_eq!(c.process(128, 32, &frame(5)), 0, "a frame matching nothing doesn't reset it");
        assert_eq!(c.process(128, 32, &frame(9)), 4);
        assert_eq!(c.process(128, 32, &frame(5)), 3);
    }

    #[test]
    fn a_mask_frame_compares_only_its_inside() {
        // a magenta frame from (10, 4) to (20, 12), dots lit inside it at (15, 8)
        let pic = bmp(128, 32, |x, y| {
            let on_frame = (y == 4 || y == 12) && (10..=20).contains(&x) || (x == 10 || x == 20) && (4..=12).contains(&y);
            if on_frame {
                MASK
            } else if (x, y) == (15, 8) {
                [255, 0, 0]
            } else {
                [0, 0, 0]
            }
        });
        let cap = read_capture(&pic, 2).unwrap();
        assert_eq!(cap.region, (11, 5, 9, 7));
        let mut c = Captures { frame: vec![0; 128 * 32], ..Captures::default() };
        c.by_trigger.insert(1, cap);
        // the dot lit, and anything outside the frame
        let mut frame = vec![0u8; 128 * 32];
        frame[8 * 128 + 15] = 3;
        frame[30 * 128 + 100] = 2;
        assert_eq!(c.process(128, 32, &frame), 1);
    }

    #[test]
    fn small_and_wide_frames_are_brought_to_128_by_32() {
        let mut c = Captures { frame: vec![0; 128 * 32], ..Captures::default() };
        // a 128×16 frame lands on rows 8 to 23
        let pic = bmp(128, 32, |_, y| if y == 8 { [255, 0, 0] } else { [0, 0, 0] });
        c.by_trigger.insert(2, read_capture(&pic, 2).unwrap());
        let small: Vec<u8> = (0..128 * 16).map(|i| if i / 128 == 0 { 3 } else { 0 }).collect();
        assert_eq!(c.process(128, 16, &small), 2);
        // a 192×64 frame is halved and centred: 96 columns from 16
        let mut c = Captures { frame: vec![0; 128 * 32], ..Captures::default() };
        let pic = bmp(128, 32, |x, y| if y == 0 && (16..112).contains(&x) { [255, 0, 0] } else { [0, 0, 0] });
        c.by_trigger.insert(9, read_capture(&pic, 2).unwrap());
        let wide: Vec<u8> = (0..192 * 64).map(|i| if i / 192 < 2 { 3 } else { 0 }).collect();
        assert_eq!(c.process(192, 64, &wide), 9);
        assert_eq!(c.process(100, 50, &wide), -1);
    }
}
