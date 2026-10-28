// Ported from Visual Pinball X: src/parts/textbox.cpp (Textbox::RenderSetup, Render and the
// script properties) (license:GPLv3+), commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! Text boxes: text on the backdrop, which VPX draws with Windows' own font
//! code into a small picture measured in backdrop units (a box 200 wide is a
//! picture 200 pixels wide), then stretches over the window. So the text is
//! the font's normal size at Windows' display scaling, scaled with the
//! backdrop, and blurs when stretched, as in VPX.
//!
//! VPX means to enlarge the font by the window's area divided by 1.5, but
//! Windows refuses any such size as an overflow (`CTL_E_OVERFLOW`) at every
//! real window size, so the font keeps its size; this port leaves the step
//! out. A text box whose text contains "DMD" (or marked as one) shows the
//! table's display instead.

use vpin::vpx::gameitem::textbox::{TextAlignment, TextBox};

/// A text box as the script changes it (VPX's `TextboxData`).
#[derive(Clone, Debug)]
pub struct TextBoxRt {
    pub text: String,
    /// `COLORREF`s: red in the low byte.
    pub back_color: u32,
    pub font_color: u32,
    pub intensity_scale: f32,
    /// VPX's `TextAlignment`: 0 left, 1 center, 2 right.
    pub align: i32,
    pub transparent: bool,
    pub is_dmd: bool,
    pub v1: [f32; 2],
    pub v2: [f32; 2],
    /// Counts the changes that redraw the picture. In VPX only a new
    /// `Text` does (`m_textureDirty`): new colours or alignment show with
    /// the next text.
    pub version: u32,
}

impl TextBoxRt {
    pub fn new(t: &TextBox) -> TextBoxRt {
        TextBoxRt {
            text: t.text.clone(),
            back_color: crate::light::colorref(&t.back_color),
            font_color: crate::light::colorref(&t.font_color),
            intensity_scale: t.intensity_scale,
            align: alignment(&t.align),
            transparent: t.is_transparent,
            is_dmd: t.is_dmd.unwrap_or(false),
            v1: [t.ver1.x, t.ver1.y],
            v2: [t.ver2.x, t.ver2.y],
            version: 0,
        }
    }

    /// Whether it shows the table's display: VPX's DMD flag, or "DMD" in its
    /// text (VPX 10.0's way, still honoured).
    pub fn shows_display(&self) -> bool {
        self.is_dmd || self.text.to_ascii_lowercase().contains("dmd")
    }

    /// What the renderer needs this frame.
    pub fn now(&self, part: u32, visible: bool) -> TextBoxNow {
        TextBoxNow {
            part,
            visible,
            rect: [self.v1[0].min(self.v2[0]), self.v1[1].min(self.v2[1]), self.v1[0].max(self.v2[0]), self.v1[1].max(self.v2[1])],
            intensity: self.intensity_scale,
            display: self.shows_display(),
            version: self.version,
            text: self.text.clone(),
            back_color: self.back_color,
            font_color: self.font_color,
            align: self.align,
            transparent: self.transparent,
        }
    }
}

fn alignment(a: &TextAlignment) -> i32 {
    match a {
        TextAlignment::Left => 0,
        TextAlignment::Center => 1,
        TextAlignment::Right => 2,
        TextAlignment::Other(v) => *v as i32,
    }
}

/// A text box as one frame shows it.
#[derive(Clone, Debug)]
pub struct TextBoxNow {
    pub part: u32,
    pub visible: bool,
    /// Left, top, right, bottom on VPX's 1000 × 750 backdrop grid.
    pub rect: [f32; 4],
    pub intensity: f32,
    /// It shows the table's display rather than its text.
    pub display: bool,
    pub version: u32,
    pub text: String,
    pub back_color: u32,
    pub font_color: u32,
    pub align: i32,
    pub transparent: bool,
}

/// The font, as the table file gives it: VPX makes the font it plays with
/// once, so a script's new font doesn't show.
#[derive(Clone, Debug)]
pub struct FontLook {
    pub name: String,
    /// Points times 10000 (OLE's `CY`).
    pub size: u32,
    pub weight: u16,
    pub charset: u16,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl FontLook {
    /// `vpin` keeps a font's numbers to itself, but writes them out with the
    /// text box as JSON.
    pub fn of(t: &TextBox) -> FontLook {
        let json = serde_json::to_value(t).unwrap_or_default();
        let font = &json["font"];
        let styles: Vec<&str> = font["style"].as_array().map(|s| s.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
        FontLook {
            name: font["name"].as_str().unwrap_or("Arial").to_string(),
            size: font["size"].as_u64().unwrap_or(0) as u32,
            weight: font["weight"].as_u64().unwrap_or(400) as u16,
            charset: font["charset"].as_u64().unwrap_or(0) as u16,
            italic: styles.contains(&"Italic"),
            underline: styles.contains(&"Underline"),
            strikethrough: styles.contains(&"Strikethrough"),
        }
    }
}

/// A text box's picture, set up once (`Textbox::RenderSetup`): its size in
/// texels is the box's on the backdrop grid when play starts.
#[derive(Clone, Debug)]
pub struct TextBoxLook {
    pub part: u32,
    pub font: FontLook,
    pub width: u32,
    pub height: u32,
}

impl TextBoxLook {
    /// None for a box with no area, which VPX never draws.
    pub fn new(part: u32, t: &TextBox) -> Option<TextBoxLook> {
        let width = t.ver1.x.max(t.ver2.x) as i32 - t.ver1.x.min(t.ver2.x) as i32;
        let height = t.ver1.y.max(t.ver2.y) as i32 - t.ver1.y.min(t.ver2.y) as i32;
        (width > 0 && height > 0).then(|| TextBoxLook { part, font: FontLook::of(t), width: width as u32, height: height as u32 })
    }
}

/// Draws text boxes' pictures, keeping each box's font for the whole game
/// as VPX does (`m_pIFontPlay`). Lives on one thread: Windows' fonts belong
/// to the thread that made them.
#[derive(Default)]
pub struct Painter {
    fonts: std::collections::HashMap<u32, Option<gdi::PlayFont>>,
}

impl Painter {
    /// A text box's picture as VPX draws it on Windows: fill with the back
    /// colour, draw the text with GDI's `DrawText`, then make the texels
    /// that are exactly the back colour see-through if the box is
    /// transparent. `window_width` sets the margin. RGBA, `look.width` ×
    /// `look.height`.
    pub fn draw(&mut self, look: &TextBoxLook, now: &TextBoxNow, window_width: u32) -> Vec<u8> {
        let font = self.fonts.entry(look.part).or_insert_with(|| gdi::PlayFont::new(&look.font));
        draw_picture(font.as_ref(), look, now, window_width)
    }
}

fn draw_picture(font: Option<&gdi::PlayFont>, look: &TextBoxLook, now: &TextBoxNow, window_width: u32) -> Vec<u8> {
    let (width, height) = (look.width as i32, look.height as i32);
    let border = (4 * window_width as i32) / 1000;
    let rc_out = gdi::Rect { left: border, top: border, right: width - border * 2, bottom: height - border * 2 };
    let bgr = gdi::draw_text(font, &now.text, width, height, now.back_color, now.font_color, now.align, rc_out);
    let mut rgba = Vec::with_capacity(bgr.len() * 4);
    for &src in &bgr {
        // VPX compares the texel (0x00RRGGBB) with the COLORREF (0x00BBGGRR) as
        // they are: only greys match
        if now.transparent && (src & 0xFF_FFFF) == now.back_color {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            rgba.extend_from_slice(&[(src >> 16) as u8, (src >> 8) as u8, src as u8, 255]);
        }
    }
    rgba
}

/// Windows' OLE fonts and GDI, as `Textbox::Render` uses them.
mod gdi {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    use super::FontLook;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    /// `IID_IFont`.
    const IID_IFONT: Guid = Guid { data1: 0xBEF6_E002, data2: 0xA874, data3: 0x101A, data4: [0x8B, 0xBA, 0x00, 0xAA, 0x00, 0x30, 0x0C, 0xAB] };

    #[repr(C)]
    struct FontDesc {
        cb_sizeofstruct: u32,
        lpstr_name: *const u16,
        cy_size: i64,
        s_weight: i16,
        s_charset: i16,
        f_italic: i32,
        f_underline: i32,
        f_strikethrough: i32,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        bi_size: u32,
        bi_width: i32,
        bi_height: i32,
        bi_planes: u16,
        bi_bit_count: u16,
        bi_compression: u32,
        bi_size_image: u32,
        bi_x_pels_per_meter: i32,
        bi_y_pels_per_meter: i32,
        bi_clr_used: u32,
        bi_clr_important: u32,
    }

    #[repr(C)]
    struct BitmapInfo {
        header: BitmapInfoHeader,
        colors: [u32; 1],
    }

    #[link(name = "oleaut32")]
    unsafe extern "system" {
        fn OleCreateFontIndirect(desc: *const FontDesc, riid: *const Guid, out: *mut *mut c_void) -> i32;
    }

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(hdc: *mut c_void) -> *mut c_void;
        fn CreateDIBSection(hdc: *mut c_void, info: *const BitmapInfo, usage: u32, bits: *mut *mut c_void, section: *mut c_void, offset: u32) -> *mut c_void;
        fn SelectObject(hdc: *mut c_void, object: *mut c_void) -> *mut c_void;
        fn CreateSolidBrush(color: u32) -> *mut c_void;
        fn PatBlt(hdc: *mut c_void, x: i32, y: i32, w: i32, h: i32, rop: u32) -> i32;
        fn DeleteObject(object: *mut c_void) -> i32;
        fn DeleteDC(hdc: *mut c_void) -> i32;
        fn SetTextColor(hdc: *mut c_void, color: u32) -> u32;
        fn SetBkMode(hdc: *mut c_void, mode: i32) -> i32;
        fn SetTextAlign(hdc: *mut c_void, align: u32) -> u32;
        fn GdiFlush() -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn DrawTextW(hdc: *mut c_void, text: *const u16, count: i32, rect: *mut Rect, format: u32) -> i32;
    }

    const PATCOPY: u32 = 0x00F0_0021;
    const TRANSPARENT: i32 = 1;
    const TA_LEFT_TOP_NOUPDATECP: u32 = 0;
    const DT_LEFT: u32 = 0x0;
    const DT_CENTER: u32 = 0x1;
    const DT_RIGHT: u32 = 0x2;
    const DT_WORDBREAK: u32 = 0x10;
    const DT_NOCLIP: u32 = 0x100;
    const DT_NOPREFIX: u32 = 0x800;
    const COINIT_APARTMENTTHREADED: u32 = 0x2;

    /// An `IFont`'s first entries: `IUnknown`, then `get_Name` to
    /// `put_Charset`, then `get_hFont` (ocidl.idl's order).
    #[repr(C)]
    struct IFontVtbl {
        query_interface: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        rest: [usize; 16],
        get_hfont: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    }

    /// A table font as an OLE font (VPX's `OleCreateFontIndirect` from the
    /// text box's `FONTDESC`), with its GDI font.
    pub struct PlayFont {
        ifont: *mut c_void,
        hfont: *mut c_void,
    }

    impl PlayFont {
        pub fn new(font: &FontLook) -> Option<PlayFont> {
            thread_local! {
                static COM: () = {
                    // OLE fonts want COM on this thread; it may be on already
                    unsafe { CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED) };
                };
            }
            COM.with(|_| ());
            unsafe {
                let name: Vec<u16> = font.name.encode_utf16().chain([0]).collect();
                let desc = FontDesc {
                    cb_sizeofstruct: size_of::<FontDesc>() as u32,
                    lpstr_name: name.as_ptr(),
                    cy_size: i64::from(font.size),
                    s_weight: font.weight as i16,
                    s_charset: font.charset as i16,
                    f_italic: i32::from(font.italic),
                    f_underline: i32::from(font.underline),
                    f_strikethrough: i32::from(font.strikethrough),
                };
                let mut ifont: *mut c_void = null_mut();
                if OleCreateFontIndirect(&desc, &IID_IFONT, &mut ifont) < 0 || ifont.is_null() {
                    return None;
                }
                let mut hfont: *mut c_void = null_mut();
                ((**(ifont as *mut *const IFontVtbl)).get_hfont)(ifont, &mut hfont);
                Some(PlayFont { ifont, hfont })
            }
        }
    }

    impl Drop for PlayFont {
        fn drop(&mut self) {
            // the GDI font belongs to the OLE font
            unsafe { ((**(self.ifont as *mut *const IFontVtbl)).release)(self.ifont) };
        }
    }

    /// The text drawn on the back colour, as 0x00RRGGBB texels, top row
    /// first. Without a font, just the back colour.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(font: Option<&PlayFont>, text: &str, width: i32, height: i32, back_color: u32, font_color: u32, align: i32, mut rc_out: Rect) -> Vec<u32> {
        let mut out = vec![back_color_as_texel(back_color); (width * height) as usize];
        let Some(font) = font else { return out };
        unsafe {
            let info = BitmapInfo {
                header: BitmapInfoHeader {
                    bi_size: size_of::<BitmapInfoHeader>() as u32,
                    bi_width: width,
                    bi_height: -height,
                    bi_planes: 1,
                    bi_bit_count: 32,
                    bi_compression: 0,
                    bi_size_image: 0,
                    bi_x_pels_per_meter: 0,
                    bi_y_pels_per_meter: 0,
                    bi_clr_used: 0,
                    bi_clr_important: 0,
                },
                colors: [0],
            };
            let mut bits: *mut c_void = null_mut();
            let hbm = CreateDIBSection(null_mut(), &info, 0, &mut bits, null_mut(), 0);
            let hdc = CreateCompatibleDC(null_mut());
            if !hbm.is_null() && !hdc.is_null() && !bits.is_null() {
                let old_bmp = SelectObject(hdc, hbm);
                let brush = CreateSolidBrush(back_color);
                let old_brush = SelectObject(hdc, brush);
                PatBlt(hdc, 0, 0, width, height, PATCOPY);
                SelectObject(hdc, old_brush);
                DeleteObject(brush);

                let old_font = SelectObject(hdc, font.hfont);
                SetTextColor(hdc, font_color);
                SetBkMode(hdc, TRANSPARENT);
                SetTextAlign(hdc, TA_LEFT_TOP_NOUPDATECP);
                let alignment = match align {
                    0 => DT_LEFT,
                    2 => DT_RIGHT,
                    _ => DT_CENTER,
                };
                let wide: Vec<u16> = text.encode_utf16().collect();
                DrawTextW(hdc, if wide.is_empty() { null() } else { wide.as_ptr() }, wide.len() as i32, &mut rc_out, alignment | DT_NOCLIP | DT_NOPREFIX | DT_WORDBREAK);
                GdiFlush();
                std::ptr::copy_nonoverlapping(bits as *const u32, out.as_mut_ptr(), out.len());

                SelectObject(hdc, old_font);
                SelectObject(hdc, old_bmp);
            }
            if !hdc.is_null() {
                DeleteDC(hdc);
            }
            if !hbm.is_null() {
                DeleteObject(hbm);
            }
        }
        out
    }

    /// A `COLORREF` as GDI writes it into a 32-bit picture.
    fn back_color_as_texel(c: u32) -> u32 {
        ((c & 0xFF) << 16) | (c & 0xFF00) | ((c >> 16) & 0xFF)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn draws_text_in_the_font_colour() {
            let font = FontLook { name: "Arial".into(), size: 142500, weight: 400, charset: 0, italic: false, underline: false, strikethrough: false };
            let font = PlayFont::new(&font).expect("Arial");
            let rc = Rect { left: 4, top: 4, right: 92, bottom: 42 };
            let texels = draw_text(Some(&font), "wood", 100, 50, 0, 0x00FF_FFFF, 1, rc);
            assert_eq!(texels.len(), 5000);
            // white text on black: some texels lit, the corners untouched
            assert!(texels.iter().any(|&t| t & 0xFF_FFFF == 0xFF_FFFF));
            assert_eq!(texels[0] & 0xFF_FFFF, 0);
        }
    }
}
