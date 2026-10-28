// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): FlexDMD/Actors/Font.cs. Copyright 2019 Vincent
// Bousquet, licensed under the Apache License, Version 2.0 (LICENSE-FlexDMD.txt beside this file). Changed in the
// porting, from C# to Rust, for Visual Pinball Infinite; the port is GPL-3.0-or-later.

//! A bitmap font as FlexDMD draws it: its sheets tinted, or outlined by a
//! pixel all round (the glyphs then a pixel further apart), its characters
//! drawn a pixel grid at a time, a missing small letter drawn as its
//! capital and anything else missing as a space (remembered, as FlexDMD
//! remembers it).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::assets::{self, AssetManager, AssetSrc};
use super::bmfont::{BmFont, Glyph};
use super::surface::{Bitmap, Surface};

/// A font (FlexDMD's `Font`).
pub struct Font {
    pub bm: BmFont,
    /// Its characters, with those FlexDMD has filled in since.
    chars: RefCell<HashMap<u16, Glyph>>,
    sheets: Vec<Rc<Bitmap>>,
    pub id: String,
}

impl Font {
    /// FlexDMD's `new Font(manager, src)`. A font that can't be read is an
    /// error, as in FlexDMD.
    pub fn load(m: &mut AssetManager, src: &AssetSrc) -> Result<Font, String> {
        let bytes = m.open(src).ok_or_else(|| format!("can't open the font {}", src.path))?;
        let text = String::from_utf8_lossy(&bytes);
        let mut bm = BmFont::parse(text.trim_start_matches('\u{feff}')).ok_or_else(|| format!("can't read the font {}", src.path))?;
        let mut sheets = Vec::new();
        for page in &bm.pages {
            let page_src = m.resolve(page, Some(src))?;
            let bytes = m.open(&page_src).ok_or_else(|| format!("can't open the font's picture {}", page_src.path))?;
            // a font's sheets keep GDI+'s byte order: FlexDMD doesn't swap them
            sheets.push(assets::decode(&bytes, false).ok_or_else(|| format!("can't read the font's picture {}", page_src.path))?);
        }
        // the tint and border, as GDI+ colours (alpha, red, green, blue)
        let [ta, tr, tg, tb] = src.font_tint.map(u32::from);
        let tint = |p: [u8; 4]| [(u32::from(p[0]) * tr / 255) as u8, (u32::from(p[1]) * tg / 255) as u8, (u32::from(p[2]) * tb / 255) as u8, (u32::from(p[3]) * ta / 255) as u8];
        if src.font_border_size > 0 {
            let [ba, br, bg, bb] = src.font_border_tint;
            // ARGBToABGR, written as a 32-bit value: red first in memory
            let outline = [br, bg, bb, ba];
            for sheet in &mut sheets {
                let (w, h) = (sheet.width as i64, sheet.height as i64);
                let mut dst = Bitmap::new(sheet.width, sheet.height);
                for y in 0..h {
                    for x in 0..w {
                        if sheet.px[(y * w + x) as usize][3] > 0 {
                            for (dx, dy) in [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] {
                                let (nx, ny) = (x + dx, y + dy);
                                if nx >= 0 && ny >= 0 && nx < w && ny < h {
                                    dst.px[(ny * w + nx) as usize] = outline;
                                }
                            }
                        }
                    }
                }
                for (d, s) in dst.px.iter_mut().zip(&sheet.px) {
                    if s[3] != 0 {
                        *d = tint(*s);
                    }
                }
                *sheet = dst;
            }
            for g in bm.chars.values_mut() {
                g.x_advance += 2;
            }
        } else if src.font_tint != [255; 4] {
            for sheet in &mut sheets {
                for p in &mut sheet.px {
                    *p = tint(*p);
                }
            }
        }
        let chars = RefCell::new(bm.chars.clone());
        Ok(Font { bm, chars, sheets: sheets.into_iter().map(Rc::new).collect(), id: src.id.clone() })
    }

    pub fn line_height(&self) -> i32 {
        self.bm.line_height
    }

    pub fn base_height(&self) -> i32 {
        self.bm.base_height
    }

    /// FlexDMD's `MeasureFont`: missing characters filled in first, as
    /// drawing would.
    pub fn measure(&self, text: &[u16]) -> (i32, i32) {
        self.draw_text(None, 0.0, 0.0, text);
        self.bm.measure(&self.chars.borrow(), text)
    }

    /// FlexDMD's `DrawText`: a line feed goes back to x = 0 (not to where the
    /// text started) and down a line, as in FlexDMD.
    pub fn draw_text(&self, mut surface: Option<&mut Surface>, mut x: f32, mut y: f32, text: &[u16]) {
        let mut previous = u16::from(b' ');
        for &c in text {
            self.draw_char(surface.as_deref_mut(), c, previous, &mut x, &mut y);
            previous = c;
        }
    }

    fn draw_char(&self, surface: Option<&mut Surface>, c: u16, previous: u16, x: &mut f32, y: &mut f32) {
        if c == u16::from(b'\n') {
            *x = 0.0;
            *y += self.bm.line_height as f32;
            return;
        }
        let glyph = self.chars.borrow().get(&c).copied();
        if let Some(g) = glyph {
            let kerning = self.bm.kerning(previous, c);
            if let Some(s) = surface
                && let Some(sheet) = usize::try_from(g.page).ok().and_then(|p| self.sheets.get(p))
            {
                let dx = (*x + (g.offset[0] + kerning) as f32) as i32;
                let dy = (*y + g.offset[1] as f32) as i32;
                let [bx, by, bw, bh] = g.bounds.map(|v| v as f32);
                s.draw(sheet, [dx as f32, dy as f32, bw, bh], [bx, by, bw, bh]);
            }
            *x += (g.x_advance + kerning) as f32;
            return;
        }
        let upper = (u16::from(b'a')..=u16::from(b'z')).contains(&c).then(|| c - 32);
        let stand_in = {
            let chars = self.chars.borrow();
            upper.and_then(|u| chars.get(&u).copied()).or_else(|| chars.get(&u16::from(b' ')).copied())
        };
        if let Some(g) = stand_in {
            self.chars.borrow_mut().insert(c, g);
            self.draw_char(surface, c, previous, x, y);
        }
    }
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Font({})", self.id)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tiny font for tests: 'A' a 2×3 block, ' ' blank, on one sheet.
    pub(crate) fn tiny() -> Font {
        let bm = BmFont::parse(
            "common lineHeight=4 base=3\npage id=0 file=\"t.png\"\nchar id=65 x=0 y=0 width=2 height=3 xoffset=0 yoffset=0 xadvance=3 page=0\nchar id=32 x=2 y=0 width=1 height=1 xoffset=0 yoffset=0 xadvance=2 page=0\n",
        )
        .unwrap();
        let mut sheet = Bitmap::new(3, 3);
        for y in 0..3 {
            for x in 0..2 {
                sheet.px[y * 3 + x] = [255, 255, 255, 255];
            }
        }
        let chars = RefCell::new(bm.chars.clone());
        Font { bm, chars, sheets: vec![Rc::new(sheet)], id: "tiny".into() }
    }

    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn missing_letters_become_capitals_then_spaces() {
        let f = tiny();
        // 'a' is drawn as 'A', '?' as a space
        assert_eq!(f.measure(&utf16("aA")), (6, 4));
        assert_eq!(f.measure(&utf16("A?")), (5, 4));
        assert_eq!(f.chars.borrow().get(&u16::from(b'?')).map(|g| g.x_advance), Some(2));
    }

    #[test]
    fn text_draws_where_flexdmd_puts_it() {
        let f = tiny();
        let mut s = Surface::new(8, 8);
        f.draw_text(Some(&mut s), 1.0, 1.0, &utf16("A\nA"));
        let lit = |x, y| s.rgb(x, y)[0] > 0;
        assert!(lit(1, 1) && lit(2, 3) && !lit(3, 1));
        // the second line starts at x = 0, four rows down
        assert!(lit(0, 5) && lit(1, 7) && !lit(2, 5));
    }
}
