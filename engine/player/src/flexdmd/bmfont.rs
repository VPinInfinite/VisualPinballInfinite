// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): its copy of Cyotek's AngelCode bitmap-font
// reader (FlexDMD/BMFonts/BitmapFont.cs, BitmapFontLoader.cs, Character.cs, Kerning.cs), Copyright (c) 2012-2015
// Cyotek Ltd., under the MIT License, as FlexDMD carries it (LICENSE-FlexDMD.txt beside this file). Changed in the
// porting, from C# to Rust, for Visual Pinball Infinite, keeping the text format FlexDMD reads; the port is
// GPL-3.0-or-later.

//! A bitmap font's description (AngelCode's BMFont text format, `.fnt`):
//! where each character sits on the font's pictures, how far it moves the
//! pen, the kerning between pairs, the line height. Text is UTF-16 code
//! units, as C#'s strings are.

use std::collections::HashMap;

/// One character's place in the font (Cyotek's `Character`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Glyph {
    /// Its rectangle on its page: x, y, width, height.
    pub bounds: [i32; 4],
    /// Where it's drawn from the pen.
    pub offset: [i32; 2],
    /// How far it moves the pen.
    pub x_advance: i32,
    pub page: i32,
}

/// A font's description (Cyotek's `BitmapFont`, as FlexDMD reads it).
#[derive(Clone, Debug, Default)]
pub struct BmFont {
    pub line_height: i32,
    pub base_height: i32,
    /// Its pictures' file names, in page order.
    pub pages: Vec<String>,
    pub chars: HashMap<u16, Glyph>,
    pub kernings: HashMap<(u16, u16), i32>,
}

impl BmFont {
    /// Reads the text format (Cyotek's `LoadText`); None where C# throws (a
    /// character or page given twice).
    pub fn parse(text: &str) -> Option<BmFont> {
        let mut font = BmFont::default();
        let mut pages: Vec<(i32, String)> = Vec::new();
        for line in text.lines() {
            let parts = split(line, ' ');
            let Some(kind) = parts.first() else { continue };
            match kind.as_str() {
                "common" => {
                    font.line_height = named_int(&parts, "lineHeight");
                    font.base_height = named_int(&parts, "base");
                }
                "page" => {
                    let id = named_int(&parts, "id");
                    if pages.iter().any(|p| p.0 == id) {
                        return None;
                    }
                    pages.push((id, named_string(&parts, "file")));
                }
                "char" => {
                    let id = named_int(&parts, "id") as u16;
                    let glyph = Glyph {
                        bounds: [named_int(&parts, "x"), named_int(&parts, "y"), named_int(&parts, "width"), named_int(&parts, "height")],
                        offset: [named_int(&parts, "xoffset"), named_int(&parts, "yoffset")],
                        x_advance: named_int(&parts, "xadvance"),
                        page: named_int(&parts, "page"),
                    };
                    if font.chars.insert(id, glyph).is_some() {
                        return None;
                    }
                }
                "kerning" => {
                    let key = (named_int(&parts, "first") as u16, named_int(&parts, "second") as u16);
                    font.kernings.entry(key).or_insert(named_int(&parts, "amount"));
                }
                _ => {}
            }
        }
        pages.sort_by_key(|p| p.0);
        font.pages = pages.into_iter().map(|p| p.1).collect();
        Some(font)
    }

    /// The kerning between two characters (Cyotek's `GetKerning`).
    pub fn kerning(&self, previous: u16, current: u16) -> i32 {
        self.kernings.get(&(previous, current)).copied().unwrap_or(0)
    }

    /// The size of a text in this font, lines broken at line feeds and
    /// returns (Cyotek's `MeasureFont`, no wrapping): every line but the
    /// last counts the line height; the last as tall as its tallest
    /// character reaches.
    pub fn measure(&self, chars: &HashMap<u16, Glyph>, text: &[u16]) -> (i32, i32) {
        if text.is_empty() {
            return (0, 0);
        }
        let (lf, cr) = (u16::from(b'\n'), u16::from(b'\r'));
        let mut previous = u16::from(b' ');
        let (mut line_width, mut line_height) = (0, self.line_height);
        let mut block_width = 0;
        let mut heights = Vec::new();
        for (i, &c) in text.iter().enumerate() {
            if c == lf || c == cr {
                if c == lf || i + 1 == text.len() || text[i + 1] != lf {
                    heights.push(line_height);
                    block_width = block_width.max(line_width);
                    line_width = 0;
                    line_height = self.line_height;
                }
            } else {
                if let Some(g) = chars.get(&c) {
                    line_width += g.x_advance + self.kerning(previous, c);
                    line_height = line_height.max(g.bounds[3] + g.offset[1]);
                }
                previous = c;
            }
        }
        if line_height != 0 {
            heights.push(line_height);
        }
        let n = heights.len();
        let height: i32 = heights.iter().enumerate().map(|(i, &h)| if i + 1 < n { self.line_height } else { h }).sum();
        (line_width.max(block_width), height)
    }
}

/// Cyotek's `BitmapFontLoader.Split`: by the delimiter, keeping a quoted
/// value whole when a line has quotes (and then keeping empty parts too).
fn split(s: &str, delimiter: char) -> Vec<String> {
    if !s.contains('"') {
        return s.split(delimiter).filter(|p| !p.is_empty()).map(str::to_string).collect();
    }
    let b: Vec<char> = s.chars().collect();
    let find = |c: char, from: isize| -> isize {
        let from = from.max(0) as usize;
        b.iter().skip(from).position(|&x| x == c).map_or(-1, |p| (p + from) as isize)
    };
    let len = b.len() as isize;
    let mut parts = Vec::new();
    let mut part_start: isize = -1;
    loop {
        let quote_start = find('"', part_start + 1);
        let quote_end = find('"', quote_start + 1);
        let mut part_end = find(delimiter, part_start + 1);
        if part_end == -1 {
            part_end = len;
        }
        let has_quotes = quote_start != -1 && part_end > quote_start && part_end < quote_end;
        if has_quotes {
            part_end = find(delimiter, quote_end + 1);
            // C# throws here (a quoted value with spaces at the line's end); take the rest
            if part_end == -1 {
                part_end = len;
            }
        }
        let (from, to) = ((part_start + 1) as usize, part_end.max(part_start + 1) as usize);
        parts.push(b[from.min(b.len())..to.min(b.len())].iter().collect());
        if has_quotes {
            part_start = part_end - 1;
        }
        part_start = find(delimiter, part_start + 1);
        if part_start == -1 {
            break;
        }
    }
    parts
}

/// Cyotek's `GetNamedString`: a part's value by its name, any case, its
/// quotes taken off.
fn named_string(parts: &[String], name: &str) -> String {
    for part in parts {
        if let Some(eq) = part.find('=')
            && part[..eq].eq_ignore_ascii_case(name)
        {
            let v = &part[eq + 1..];
            if v.len() > 1 && v.starts_with('"') && v.ends_with('"') {
                return v[1..v.len() - 1].to_string();
            }
            return v.to_string();
        }
    }
    String::new()
}

/// Cyotek's `GetNamedInt`: 0 when missing or not a number.
fn named_int(parts: &[String], name: &str) -> i32 {
    let s = named_string(parts, name);
    let t = s.trim();
    t.strip_prefix('+').unwrap_or(t).parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &str = r#"info face="Teeny Tiny Pixls" size=-5 bold=0 italic=0 charset="" unicode=1 stretchH=100 smooth=0 aa=1 padding=1,1,1,1 spacing=0,0 outline=0
common lineHeight=6 base=5 scaleW=64 scaleH=64 pages=1 packed=0 alphaChnl=0 redChnl=4 greenChnl=4 blueChnl=4
page id=0 file="teeny_tiny_pixls-5.png"
chars count=3
char id=32   x=55    y=40    width=5     height=3     xoffset=-2    yoffset=4     xadvance=4     page=0  chnl=15
char id=65   x=39    y=28    width=5     height=7     xoffset=-1    yoffset=-1    xadvance=4     page=0  chnl=15 // A
char id=66   x=0     y=0     width=5     height=9     xoffset=-1    yoffset=-1    xadvance=5     page=0  chnl=15 // "
kerning first=65 second=66 amount=-1
kerning first=65 second=66 amount=3
"#;

    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn a_font_reads_as_cyotek_reads_it() {
        let f = BmFont::parse(FONT).unwrap();
        assert_eq!((f.line_height, f.base_height), (6, 5));
        assert_eq!(f.pages, vec!["teeny_tiny_pixls-5.png".to_string()]);
        let a = f.chars[&65];
        assert_eq!((a.bounds, a.offset, a.x_advance), ([39, 28, 5, 7], [-1, -1], 4));
        // the first kerning given for a pair wins
        assert_eq!(f.kerning(65, 66), -1);
        assert_eq!(f.kerning(66, 65), 0);
        // a quoted value with spaces stays whole
        assert_eq!(named_string(&split(FONT.lines().next().unwrap(), ' '), "face"), "Teeny Tiny Pixls");
    }

    #[test]
    fn text_measures_as_cyotek_measures_it() {
        let f = BmFont::parse(FONT).unwrap();
        // A (4) + B (5) with their kerning (-1); the line as tall as B reaches (9 - 1)
        assert_eq!(f.measure(&f.chars, &utf16("AB")), (8, 8));
        // two lines: the first counts the line height, \r\n is one break
        assert_eq!(f.measure(&f.chars, &utf16("AB\r\nA")), (8, 12));
        // a missing character adds nothing
        assert_eq!(f.measure(&f.chars, &utf16("AzA")), (8, 6));
        assert_eq!(f.measure(&f.chars, &utf16("")), (0, 0));
        // a character given twice is an error, as in C#
        assert!(BmFont::parse("char id=65 x=0\nchar id=65 x=1\n").is_none());
    }
}
