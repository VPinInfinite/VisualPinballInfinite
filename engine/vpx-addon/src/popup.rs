//! The card an achievement earned shows on the backglass or the score display: the achievement's
//! picture, a line saying what was earned and its points, and its name, at the size the player
//! chooses, for about four seconds. Laid out for the window it shows in (`geometry`) and drawn here
//! into a picture of its own (`draw`: the card and its border, the picture, the words drawn by
//! Windows in Segoe UI), which VPX draws over what that window shows (`lib.rs`). VPX lets add-ons
//! draw in its backglass, score and topper windows alone, never on the playfield, so a card asked
//! for there is VPX's own notice. Ours.

use std::path::{Path, PathBuf};

/// Where the cards show, as the app hands it over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Backglass,
    ScoreView,
    Playfield,
}

impl Screen {
    pub fn named(name: &str) -> Option<Screen> {
        match name.trim().to_ascii_lowercase().as_str() {
            "backglass" => Some(Screen::Backglass),
            "scoreview" | "score view" | "score display" => Some(Screen::ScoreView),
            "playfield" => Some(Screen::Playfield),
            _ => None,
        }
    }

    /// VPX's number for its window (`VPXWindowId`); none for the playfield, where add-ons can't draw.
    pub fn window(self) -> Option<i32> {
        match self {
            Screen::Backglass => Some(1),
            Screen::ScoreView => Some(2),
            Screen::Playfield => None,
        }
    }
}

/// A card's width as a share of its window's: small, medium and large.
pub const SMALL: f32 = 0.42;
pub const MEDIUM: f32 = 0.6;
pub const LARGE: f32 = 0.82;

/// The cards as the app asks for them: the screen, their width as a share of the window's, and VPX's
/// priorities for that window's other renderers (`Priority.<id>` in the window's section of its
/// settings), by which the window's own picture is drawn under a card as VPX would draw it.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub screen: Screen,
    pub size: f32,
    pub priorities: Vec<(String, i32)>,
}

impl Settings {
    /// `{ "on": "backglass", "size": "medium", "priorities": { "B2S": 0 } }`; a size may be the
    /// share itself. None where it names no screen.
    pub fn from_json(v: &serde_json::Value) -> Option<Settings> {
        let screen = Screen::named(v.get("on")?.as_str()?)?;
        let size = match v.get("size") {
            Some(serde_json::Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
                "small" => SMALL,
                "large" => LARGE,
                _ => MEDIUM,
            },
            Some(n) => n.as_f64().map_or(MEDIUM, |n| n as f32),
            None => MEDIUM,
        }
        .clamp(0.2, 0.95);
        let priorities = v
            .get("priorities")
            .and_then(serde_json::Value::as_object)
            .map(|o| o.iter().filter_map(|(id, p)| Some((id.clone(), i32::try_from(p.as_i64()?).ok()?))).collect())
            .unwrap_or_default();
        Some(Settings { screen, size, priorities })
    }

    /// A renderer's priority as VPX reads it, 0 where the settings say none.
    pub fn priority_of(&self, id: &str) -> i32 {
        self.priorities.iter().find(|(i, _)| i == id).map_or(0, |(_, p)| *p)
    }
}

/// What a card says: the line above (what was earned, its points), the name, and the picture.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub kicker: String,
    pub name: String,
    pub picture: Option<PathBuf>,
}

/// How long a card shows, fading in and out.
pub const SHOWS_MS: u64 = 4000;
const FADE_IN_MS: u64 = 250;
const FADE_OUT_MS: u64 = 400;

/// How opaque a card is `ms` after it began; None once it's over.
pub fn alpha_at(ms: u64) -> Option<f32> {
    if ms >= SHOWS_MS {
        return None;
    }
    Some(if ms < FADE_IN_MS {
        ms as f32 / FADE_IN_MS as f32
    } else if ms > SHOWS_MS - FADE_OUT_MS {
        (SHOWS_MS - ms) as f32 / FADE_OUT_MS as f32
    } else {
        1.0
    })
}

/// Where a card sits in its window and how big its parts are, in the window's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pad_h: f32,
    icon: f32,
    gap: f32,
    kicker_px: f32,
    name_px: f32,
    radius: f32,
    border: f32,
}

/// A card for a window `out_w` by `out_h`, `size` of its width, centred in it: the name in type
/// 7.5% of the card's width, the line above it 0.7 of that, the picture 1.6 times the name's
/// height, the margins a medium card's at every size, so a small or a large one is the same card
/// smaller or larger; held to the window's height where it's shallow, its proportions kept.
pub fn geometry(out_w: f32, out_h: f32, size: f32) -> Geometry {
    let w = out_w * size;
    let name_px = (w * 0.075).max(11.0);
    let kicker_px = (name_px * 0.7).max(11.0);
    let icon = name_px * 1.6;
    let (pad_v, pad_h) = (w * 0.05, w * 0.04 / 0.6);
    let gap = (w - 2.0 * pad_h) * 0.08;
    let h = 2.0 * pad_v + icon.max(1.2 * (kicker_px + name_px));
    let s = if h > out_h * 0.92 { out_h * 0.92 / h } else { 1.0 };
    let (w, h) = ((w * s).round().max(1.0), (h * s).round().max(1.0));
    Geometry {
        x: ((out_w - w) / 2.0).round().max(0.0),
        y: ((out_h - h) / 2.0).round().max(0.0),
        w,
        h,
        pad_h: pad_h * s,
        icon: icon * s,
        gap: gap * s,
        kicker_px: kicker_px * s,
        name_px: name_px * s,
        radius: w * 0.046,
        border: (w * 0.0046).max(1.0),
    }
}

/// A picture, rows of RGBA from the top, straight alpha.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

const FILL: [u8; 3] = [0xFA, 0xEE, 0xDA];
const BORDER: [u8; 3] = [0xBA, 0x75, 0x17];
const KICKER: [u8; 3] = [0x85, 0x4F, 0x0B];
const NAME: [u8; 3] = [0x63, 0x38, 0x06];

/// The card drawn at its size: its picture, else a trophy, and its two lines, each held to the
/// room it has (the name made smaller first, down to four fifths, then cut short with an ellipsis).
pub fn draw(card: &Card, g: &Geometry) -> Picture {
    let (pw, ph) = (g.w as u32, g.h as u32);
    let mut out = Picture { width: pw, height: ph, rgba: vec![0; (pw * ph * 4) as usize] };
    // the card and its border, their edges smoothed
    for y in 0..ph {
        for x in 0..pw {
            let d = rounded_box(x as f32 + 0.5, y as f32 + 0.5, g.w, g.h, g.radius);
            let a = (0.5 - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            let inner = (-d - g.border + 0.5).clamp(0.0, 1.0);
            let i = ((y * pw + x) * 4) as usize;
            for c in 0..3 {
                out.rgba[i + c] = (f32::from(BORDER[c]) + (f32::from(FILL[c]) - f32::from(BORDER[c])) * inner).round() as u8;
            }
            out.rgba[i + 3] = (a * 255.0).round() as u8;
        }
    }
    let icon = g.icon.round().max(1.0) as u32;
    let (ix, iy) = (g.pad_h.round() as i32, ((g.h - g.icon) / 2.0).round() as i32);
    match card.picture.as_deref().and_then(read_png) {
        Some(p) => over_scaled(&mut out, &p, ix, iy, icon),
        None => {
            if let Some(mask) = words("\u{1F3C6}", "Segoe UI Symbol", g.icon * 0.8, 400, icon, icon, true) {
                over_mask(&mut out, &mask, ix, iy, icon, icon, KICKER);
            }
        }
    }
    let tx = g.pad_h + g.icon + g.gap;
    let room = (g.w - tx - g.pad_h).max(1.0) as u32;
    let block = 1.2 * (g.kicker_px + g.name_px);
    let top = (g.h - block) / 2.0;
    let kicker_h = (1.2 * g.kicker_px).round().max(1.0) as u32;
    let kicker_px = fitted(&card.kicker, "Segoe UI", g.kicker_px, 400, room);
    if let Some(mask) = words(&card.kicker, "Segoe UI", kicker_px, 400, room, kicker_h, false) {
        over_mask(&mut out, &mask, tx.round() as i32, top.round() as i32, room, kicker_h, KICKER);
    }
    let name_h = (1.2 * g.name_px).round().max(1.0) as u32;
    let name_px = fitted(&card.name, "Segoe UI Semibold", g.name_px, 600, room);
    if let Some(mask) = words(&card.name, "Segoe UI Semibold", name_px, 600, room, name_h, false) {
        over_mask(&mut out, &mask, tx.round() as i32, (top + 1.2 * g.kicker_px).round() as i32, room, name_h, NAME);
    }
    out
}

/// A line's type made a little smaller where it's too long for `room`, down to four fifths; past
/// that it's cut short with an ellipsis as it's drawn.
fn fitted(text: &str, face: &str, px: f32, weight: i32, room: u32) -> f32 {
    match measure(text, face, px, weight) {
        Some(wide) if wide > room as f32 => (px * room as f32 / wide).max(px * 0.8),
        _ => px,
    }
}

/// How far a point is outside a box `w` by `h` with corners rounded by `r`: below 0 inside.
fn rounded_box(px: f32, py: f32, w: f32, h: f32, r: f32) -> f32 {
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = r.min(cx).min(cy);
    let qx = (px - cx).abs() - (cx - r);
    let qy = (py - cy).abs() - (cy - r);
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// A PNG file read as RGBA; None where it isn't one this can read.
pub fn read_png(file: &Path) -> Option<Picture> {
    let bytes = std::fs::read(file).ok()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&bytes[..]));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return None;
    }
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in buf.chunks(info.line_size).take(h as usize) {
        match info.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(&row[..(w * 4) as usize]),
            png::ColorType::Rgb => row[..(w * 3) as usize].chunks(3).for_each(|p| rgba.extend_from_slice(&[p[0], p[1], p[2], 255])),
            png::ColorType::GrayscaleAlpha => row[..(w * 2) as usize].chunks(2).for_each(|p| rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]])),
            png::ColorType::Grayscale => row[..w as usize].iter().for_each(|&v| rgba.extend_from_slice(&[v, v, v, 255])),
            png::ColorType::Indexed => return None,
        }
    }
    Some(Picture { width: w, height: h, rgba })
}

/// `pic` scaled into the square `size` at (`x`, `y`) and laid over `out`: each pixel the average of
/// samples across the source it covers, their colours weighed by their alpha.
fn over_scaled(out: &mut Picture, pic: &Picture, x: i32, y: i32, size: u32) {
    let (sw, sh) = (pic.width as f32, pic.height as f32);
    let step = (sw.max(sh) / size as f32).ceil().clamp(1.0, 4.0) as u32;
    let sample = |fx: f32, fy: f32| -> [f32; 4] {
        // bilinear, alpha weighed
        let (fx, fy) = ((fx - 0.5).clamp(0.0, sw - 1.0), (fy - 0.5).clamp(0.0, sh - 1.0));
        let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(pic.width - 1), (y0 + 1).min(pic.height - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let px = |x: u32, y: u32| {
            let i = ((y * pic.width + x) * 4) as usize;
            let a = f32::from(pic.rgba[i + 3]) / 255.0;
            [f32::from(pic.rgba[i]) * a, f32::from(pic.rgba[i + 1]) * a, f32::from(pic.rgba[i + 2]) * a, a]
        };
        let (p00, p10, p01, p11) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
        let mut o = [0.0; 4];
        for c in 0..4 {
            let top = p00[c] + (p10[c] - p00[c]) * tx;
            let bottom = p01[c] + (p11[c] - p01[c]) * tx;
            o[c] = top + (bottom - top) * ty;
        }
        o
    };
    for dy in 0..size {
        for dx in 0..size {
            let (ox, oy) = (x + dx as i32, y + dy as i32);
            if ox < 0 || oy < 0 || ox as u32 >= out.width || oy as u32 >= out.height {
                continue;
            }
            let mut sum = [0.0f32; 4];
            for sy in 0..step {
                for sx in 0..step {
                    let fx = (dx as f32 + (sx as f32 + 0.5) / step as f32) * sw / size as f32;
                    let fy = (dy as f32 + (sy as f32 + 0.5) / step as f32) * sh / size as f32;
                    let s = sample(fx, fy);
                    for c in 0..4 {
                        sum[c] += s[c];
                    }
                }
            }
            let n = (step * step) as f32;
            let a = sum[3] / n;
            if a <= 0.0 {
                continue;
            }
            let colour = [sum[0] / n / a, sum[1] / n / a, sum[2] / n / a];
            over(out, ox as u32, oy as u32, colour, a);
        }
    }
}

/// A colour laid over one of `out`'s pixels at coverage `a`.
fn over(out: &mut Picture, x: u32, y: u32, colour: [f32; 3], a: f32) {
    let i = ((y * out.width + x) * 4) as usize;
    let da = f32::from(out.rgba[i + 3]) / 255.0;
    let oa = a + da * (1.0 - a);
    if oa <= 0.0 {
        return;
    }
    for c in 0..3 {
        let d = f32::from(out.rgba[i + c]);
        out.rgba[i + c] = ((colour[c] * a + d * da * (1.0 - a)) / oa).round().clamp(0.0, 255.0) as u8;
    }
    out.rgba[i + 3] = (oa * 255.0).round() as u8;
}

/// Words' coverage (`mask`, `w` by `h`) laid over `out` at (`x`, `y`) in `colour`.
fn over_mask(out: &mut Picture, mask: &[u8], x: i32, y: i32, w: u32, h: u32, colour: [u8; 3]) {
    let colour = colour.map(f32::from);
    for my in 0..h {
        for mx in 0..w {
            let cov = mask[(my * w + mx) as usize];
            let (ox, oy) = (x + mx as i32, y + my as i32);
            if cov == 0 || ox < 0 || oy < 0 || ox as u32 >= out.width || oy as u32 >= out.height {
                continue;
            }
            over(out, ox as u32, oy as u32, colour, f32::from(cov) / 255.0);
        }
    }
}

#[cfg(windows)]
mod gdi {
    use std::ffi::c_void;

    #[repr(C)]
    pub struct BitmapInfoHeader {
        pub size: u32,
        pub width: i32,
        pub height: i32,
        pub planes: u16,
        pub bit_count: u16,
        pub compression: u32,
        pub size_image: u32,
        pub x_ppm: i32,
        pub y_ppm: i32,
        pub clr_used: u32,
        pub clr_important: u32,
    }

    #[repr(C)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct Size {
        pub cx: i32,
        pub cy: i32,
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        pub fn CreateCompatibleDC(dc: *mut c_void) -> *mut c_void;
        pub fn CreateDIBSection(dc: *mut c_void, info: *const BitmapInfoHeader, usage: u32, bits: *mut *mut c_void, section: *mut c_void, offset: u32) -> *mut c_void;
        pub fn SelectObject(dc: *mut c_void, object: *mut c_void) -> *mut c_void;
        pub fn DeleteObject(object: *mut c_void) -> i32;
        pub fn DeleteDC(dc: *mut c_void) -> i32;
        #[allow(clippy::too_many_arguments)]
        pub fn CreateFontW(height: i32, width: i32, escapement: i32, orientation: i32, weight: i32, italic: u32, underline: u32, strike_out: u32, charset: u32, out_precision: u32, clip_precision: u32, quality: u32, pitch_and_family: u32, face: *const u16) -> *mut c_void;
        pub fn SetTextColor(dc: *mut c_void, colour: u32) -> u32;
        pub fn SetBkMode(dc: *mut c_void, mode: i32) -> i32;
        pub fn GdiFlush() -> i32;
        pub fn GetTextExtentPoint32W(dc: *mut c_void, text: *const u16, len: i32, size: *mut Size) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn DrawTextW(dc: *mut c_void, text: *const u16, len: i32, rect: *mut Rect, format: u32) -> i32;
    }

    /// A font of `face` with its letters `px` high, smoothed in grey.
    pub fn font(face: &str, px: f32, weight: i32) -> *mut c_void {
        let face: Vec<u16> = face.encode_utf16().chain([0]).collect();
        // DEFAULT_CHARSET, OUT_OUTLINE_PRECIS, CLIP_DEFAULT_PRECIS, ANTIALIASED_QUALITY,
        // VARIABLE_PITCH | FF_SWISS
        // SAFETY: a NUL-ended name; the font is let go by whoever asked for it
        unsafe { CreateFontW(-(px.round().max(1.0) as i32), 0, 0, 0, weight, 0, 0, 0, 1, 8, 0, 4, 2 | 0x20, face.as_ptr()) }
    }
}

/// How wide `text` is in `face` at `px`, as Windows sets it.
#[cfg(windows)]
fn measure(text: &str, face: &str, px: f32, weight: i32) -> Option<f32> {
    use gdi::*;
    let wide: Vec<u16> = text.encode_utf16().collect();
    // SAFETY: each object made here is let go, once
    unsafe {
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        let font = font(face, px, weight);
        let before = SelectObject(dc, font);
        let mut size = Size::default();
        let ok = GetTextExtentPoint32W(dc, wide.as_ptr(), wide.len() as i32, &mut size) != 0;
        SelectObject(dc, before);
        DeleteObject(font);
        DeleteDC(dc);
        ok.then_some(size.cx as f32)
    }
}

/// `text` drawn by Windows in `face` at `px`, in a box `w` by `h` — from its left, or centred, and
/// cut short with an ellipsis where it's too long — as how much each pixel of the box it covers,
/// 0 to 255, its edges smoothed in grey.
#[cfg(windows)]
fn words(text: &str, face: &str, px: f32, weight: i32, w: u32, h: u32, centred: bool) -> Option<Vec<u8>> {
    use gdi::*;
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return None;
    }
    let wide: Vec<u16> = text.encode_utf16().collect();
    let info = BitmapInfoHeader { size: std::mem::size_of::<BitmapInfoHeader>() as u32, width: w as i32, height: -(h as i32), planes: 1, bit_count: 32, compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0 };
    // SAFETY: each object made here is let go below, once; `bits` points into the DIB section
    // while it lives, `w` × `h` × 4 bytes, top row first
    unsafe {
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, 0, &mut bits, std::ptr::null_mut(), 0);
        let mut out = None;
        if !bitmap.is_null() && !bits.is_null() {
            std::ptr::write_bytes(bits.cast::<u8>(), 0, (w * h * 4) as usize);
            let before_bitmap = SelectObject(dc, bitmap);
            let font = font(face, px, weight);
            let before_font = SelectObject(dc, font);
            SetBkMode(dc, 1);
            SetTextColor(dc, 0x00FF_FFFF);
            let mut rect = Rect { left: 0, top: 0, right: w as i32, bottom: h as i32 };
            // DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS, DT_CENTER where centred
            let format = 0x4 | 0x20 | 0x800 | 0x8000 | if centred { 0x1 } else { 0 };
            if DrawTextW(dc, wide.as_ptr(), wide.len() as i32, &mut rect, format) != 0 {
                GdiFlush();
                let bgra = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize);
                out = Some(bgra.chunks_exact(4).map(|p| p[0].max(p[1]).max(p[2])).collect());
            }
            SelectObject(dc, before_font);
            DeleteObject(font);
            SelectObject(dc, before_bitmap);
            DeleteObject(bitmap);
        }
        DeleteDC(dc);
        out
    }
}

#[cfg(not(windows))]
fn measure(_text: &str, _face: &str, _px: f32, _weight: i32) -> Option<f32> {
    None
}

#[cfg(not(windows))]
fn words(_text: &str, _face: &str, _px: f32, _weight: i32, _w: u32, _h: u32, _centred: bool) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_read_as_the_app_writes_them() {
        let v = serde_json::json!({ "on": "scoreview", "size": "large", "priorities": { "B2S": 3, "PUP": -1 } });
        let s = Settings::from_json(&v).unwrap();
        assert_eq!((s.screen, s.size), (Screen::ScoreView, LARGE));
        assert_eq!((s.priority_of("B2S"), s.priority_of("PUP"), s.priority_of("Nixie")), (3, -1, 0));
        assert_eq!(Settings::from_json(&serde_json::json!({ "on": "backglass" })).unwrap().size, MEDIUM);
        assert_eq!(Settings::from_json(&serde_json::json!({ "on": "the moon" })), None);
        assert_eq!((Screen::Backglass.window(), Screen::ScoreView.window(), Screen::Playfield.window()), (Some(1), Some(2), None));
    }

    #[test]
    fn a_card_fades_in_holds_and_fades_out() {
        assert_eq!(alpha_at(0), Some(0.0));
        assert_eq!(alpha_at(125), Some(0.5));
        assert_eq!(alpha_at(2000), Some(1.0));
        assert_eq!(alpha_at(3800), Some(0.5));
        assert_eq!(alpha_at(SHOWS_MS), None);
    }

    #[test]
    fn a_card_is_laid_out_as_the_sketch_and_held_to_its_window() {
        // a 16:9 backglass, medium: 60% of its width, centred, its name in type 7.5% of that
        let g = geometry(1920.0, 1080.0, MEDIUM);
        assert_eq!((g.w, g.x), (1152.0, 384.0));
        assert!((g.name_px - 86.4).abs() < 0.01 && (g.kicker_px - 60.48).abs() < 0.01);
        assert_eq!(g.h, (2.0f32 * 1152.0 * 0.05 + 1.2 * (60.48 + 86.4)).round());
        // small: the same card, smaller
        let s = geometry(1920.0, 1080.0, SMALL);
        assert!((s.pad_h / s.w - g.pad_h / g.w).abs() < 1e-4 && (s.h / s.w - g.h / g.w).abs() < 0.01);
        assert_eq!(g.y, ((1080.0 - g.h) / 2.0).round());
        // a shallow score display: held to 92% of its height, its proportions kept
        let s = geometry(1920.0, 240.0, LARGE);
        assert_eq!(s.h, (240.0f32 * 0.92).round());
        assert!(s.w < 1920.0 * LARGE && s.x > 0.0);
        assert!(s.name_px < 1920.0 * LARGE * 0.075);
    }

    #[test]
    fn a_card_is_drawn_rounded_with_its_border_its_picture_and_its_words() {
        let dir = std::env::temp_dir().join(format!("vpi-popup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // a picture of one colour, 8 by 8
        let picture = dir.join("icon.png");
        {
            let file = std::fs::File::create(&picture).unwrap();
            let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 8, 8);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&[10, 200, 30, 255].repeat(64)).unwrap();
        }
        let card = Card { kicker: "Achievement earned · 25 points".into(), name: "Merlin's Magic".into(), picture: Some(picture) };
        let g = geometry(1280.0, 320.0, MEDIUM);
        let p = draw(&card, &g);
        assert_eq!((p.width, p.height), (g.w as u32, g.h as u32));
        let at = |x: u32, y: u32| -> [u8; 4] {
            let i = ((y * p.width + x) * 4) as usize;
            [p.rgba[i], p.rgba[i + 1], p.rgba[i + 2], p.rgba[i + 3]]
        };
        // the corners left out, the edge the border's colour, inside the card's
        assert_eq!(at(0, 0)[3], 0);
        assert_eq!(&at(p.width / 2, 0)[..3], &BORDER);
        assert_eq!(at(p.width - 10, p.height / 2), [FILL[0], FILL[1], FILL[2], 255]);
        // the picture where it goes
        let (ix, iy) = (g.pad_h.round() as u32 + 4, ((g.h - g.icon) / 2.0).round() as u32 + 4);
        assert_eq!(at(ix, iy), [10, 200, 30, 255]);
        // words drawn in the name's colour somewhere right of the picture
        #[cfg(windows)]
        {
            let tx = (g.pad_h + g.icon + g.gap) as u32;
            let dark = (tx..p.width).flat_map(|x| (0..p.height).map(move |y| (x, y))).filter(|&(x, y)| at(x, y)[0] < 140).count();
            assert!(dark > 200, "{dark} dark pixels");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A card drawn into a file to look at, on a dark ground as a backglass might be:
    /// `VPI_CARD_FILE=<file.png> [VPI_CARD_PICTURE=<an achievement's picture>] [VPI_CARD_SIZE=small|medium|large]
    /// cargo test -p vpx-addon a_card_drawn_into_a_file_to_look_at -- --ignored`.
    #[test]
    #[ignore]
    fn a_card_drawn_into_a_file_to_look_at() {
        let file = std::env::var_os("VPI_CARD_FILE").expect("VPI_CARD_FILE");
        let size = match std::env::var("VPI_CARD_SIZE").as_deref() {
            Ok("small") => SMALL,
            Ok("large") => LARGE,
            _ => MEDIUM,
        };
        let (ow, oh) = (1280.0, 720.0);
        let card = Card { kicker: "Achievement earned · 25 points".into(), name: "Merlin's Magic".into(), picture: std::env::var_os("VPI_CARD_PICTURE").map(PathBuf::from) };
        let g = geometry(ow, oh, size);
        let p = draw(&card, &g);
        // over a dark ground, where it sits
        let (w, h) = (ow as u32, oh as u32);
        let mut ground = Picture { width: w, height: h, rgba: [0x2C, 0x2C, 0x2A, 255].repeat((w * h) as usize) };
        for y in 0..p.height {
            for x in 0..p.width {
                let i = ((y * p.width + x) * 4) as usize;
                let a = f32::from(p.rgba[i + 3]) / 255.0;
                over(&mut ground, g.x as u32 + x, g.y as u32 + y, [f32::from(p.rgba[i]), f32::from(p.rgba[i + 1]), f32::from(p.rgba[i + 2])], a);
            }
        }
        let out = std::fs::File::create(file).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(out), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&ground.rgba).unwrap();
    }

    #[test]
    fn a_card_without_a_picture_has_a_trophy_and_a_long_name_fits() {
        let card = Card { kicker: "Pack mastered".into(), name: "A name far too long to fit on one line of any card at all, however wide".into(), picture: Some("missing.png".into()) };
        let g = geometry(800.0, 450.0, SMALL);
        let p = draw(&card, &g);
        assert_eq!((p.width, p.height), (g.w as u32, g.h as u32));
        // nothing drawn past the card's own edge: its right-hand padding stays the card's colour
        let x = p.width - (g.pad_h as u32) / 2;
        let i = (((p.height / 2) * p.width + x) * 4) as usize;
        assert_eq!(&p.rgba[i..i + 3], &FILL);
    }
}
