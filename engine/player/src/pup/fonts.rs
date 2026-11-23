// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): how its PinUp Player
// plugin finds a pack's fonts, sizes them and draws their text (plugins/pup/PUPManager.cpp: LoadFonts, AddFont,
// GetFont; PUPFont.cpp: ParseOS2Metrics, PUPFont, Compute, Apply; PUPLabel.cpp's SDL_ttf calls).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A pack's fonts (its `FONTS` folder, and Liberation Sans built into the
//! player as VPX keeps it for last), found by the names scripts give them —
//! the family, the family and style, or the file's name — sized as VPX sizes
//! them so text sits where PinUP Player puts it (a label's height in pixels
//! made a point size by the font's Windows metrics, its baseline moved from
//! where SDL_ttf puts it to where Windows does), and drawn by SDL_ttf with
//! FreeType and HarfBuzz, as VPX draws them. SDL_ttf's fonts and its
//! FreeType are used by one thread at a time (VPX's font mutex).

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use sdl3_sys::everything::*;
use sdl3_ttf_sys::ttf::*;

use super::label::XAlign;

/// Liberation Sans, VPX's font of last resort for labels (`assets/fonts`, SIL Open Font License).
const FALLBACK: &[u8] = include_bytes!("../../assets/fonts/LiberationSans-Regular.ttf");

/// SDL_ttf, started once; every use of it holds this lock (VPX's `PUPLabel::m_fontMutex`).
fn ttf() -> Option<&'static Mutex<()>> {
    static TTF: OnceLock<Option<Mutex<()>>> = OnceLock::new();
    // SAFETY: SDL_ttf's start, once
    TTF.get_or_init(|| unsafe { TTF_Init() }.then(|| Mutex::new(()))).as_ref()
}

/// A font's sizes for a label's height (VPX's `PUPFont::SizeMetrics`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizeMetrics {
    /// The point size that gives the height Windows would.
    pub ptsize: f32,
    /// The ascent as SDL_ttf makes it, and as Windows does.
    pub sdl_ascent: f32,
    pub win_ascent: f32,
    pub line_gap: f32,
}

/// How a text is drawn.
#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    pub italic: bool,
    pub underline: bool,
    /// Lines aligned left, centred or right.
    pub align: XAlign,
    /// An outline this many pixels wide (0: none).
    pub outline: i32,
}

/// SDL_ttf's font, used under the lock.
struct Handle(*mut TTF_Font);

// SAFETY: only ever used with SDL_ttf's lock held (`ttf`)
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

/// One of a pack's fonts.
pub struct PupFont {
    pub family: String,
    pub style: String,
    /// Windows' em over its ascent plus descent, and its ascent's share of those.
    win_scale: f32,
    win_ascent_ratio: f32,
    /// SDL_ttf's ascent and line gap per point.
    sdl_ascent_ratio: f32,
    line_gap_ratio: f32,
    font: Handle,
}

impl Drop for PupFont {
    fn drop(&mut self) {
        if let Some(lock) = ttf() {
            let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
            // SAFETY: our font, closed once, under the lock
            unsafe { TTF_CloseFont(self.font.0) };
        }
    }
}

impl PupFont {
    /// A font from its file's bytes (VPX's `TTF_OpenFont` and `PUPFont`): its names as SDL_ttf
    /// reads them, its Windows metrics, SDL_ttf's ascent and line gap probed at size 100.
    fn open(data: &[u8], held: &std::sync::MutexGuard<'_, ()>) -> Option<PupFont> {
        let _ = held;
        // SAFETY: SDL reads the bytes, which outlive the font (a pack's are kept with it; the
        // fallback's are the program's); SDL_ttf's calls under its lock
        unsafe {
            let bytes: &'static [u8] = Box::leak(data.to_vec().into_boxed_slice());
            let io = SDL_IOFromConstMem(bytes.as_ptr().cast(), bytes.len());
            if io.is_null() {
                return None;
            }
            let font = TTF_OpenFontIO(io, true, 8.0);
            if font.is_null() {
                return None;
            }
            let text = |p: *const std::ffi::c_char| if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() };
            let family = text(TTF_GetFontFamilyName(font));
            let style = text(TTF_GetFontStyleName(font));
            let (win_scale, win_ascent_ratio) = os2_metrics(data).unwrap_or((1.0, 0.8));
            const PROBE: f32 = 100.0;
            TTF_SetFontSize(font, PROBE);
            let (ascent, height, skip) = (TTF_GetFontAscent(font), TTF_GetFontHeight(font), TTF_GetFontLineSkip(font));
            let sdl_ascent_ratio = if ascent > 0 { ascent as f32 / PROBE } else { 0.8 };
            let line_gap_ratio = if skip > height { (skip - height) as f32 / PROBE } else { 0.0 };
            Some(PupFont { family, style, win_scale, win_ascent_ratio, sdl_ascent_ratio, line_gap_ratio, font: Handle(font) })
        }
    }

    /// A font built into the player, from its file's bytes (the pause menu's, `pause_menu`).
    pub fn built_in(data: &[u8]) -> Option<PupFont> {
        let lock = ttf()?;
        let held = lock.lock().unwrap_or_else(|e| e.into_inner());
        PupFont::open(data, &held)
    }

    /// Windows' em over its ascent plus descent (VPX's `GetWinScale`).
    pub fn win_scale(&self) -> f32 {
        self.win_scale
    }

    /// The sizes for a text `height` pixels tall (VPX's `Compute`).
    pub fn metrics(&self, height: f32) -> SizeMetrics {
        let ptsize = height * self.win_scale;
        SizeMetrics { ptsize, sdl_ascent: ptsize * self.sdl_ascent_ratio, win_ascent: height * self.win_ascent_ratio, line_gap: ptsize * self.line_gap_ratio }
    }

    /// Whether the font draws `c` (`TTF_FontHasGlyph`).
    pub fn has_glyph(&self, c: char) -> bool {
        let Some(lock) = ttf() else { return false };
        let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: our font, under the lock
        unsafe { TTF_FontHasGlyph(self.font.0, c as u32) }
    }

    /// `text` drawn at `ptsize` in `color` as VPX draws a label's (`TTF_SetFontSize`,
    /// `…Outline`, `…Style`, normal hinting, `…WrapAlignment`, `TTF_RenderText_Blended_Wrapped`
    /// breaking lines only at `\n`): width, height and RGBA pixels.
    pub fn render(&self, text: &str, ptsize: f32, color: [u8; 3], style: TextStyle) -> Option<(u32, u32, Vec<u8>)> {
        self.render_wrapped(text, ptsize, color, style, 0)
    }

    /// The same, lines broken at `\n` and wherever they would be wider than `wrap` pixels (0: only
    /// at `\n`), as SDL_ttf breaks them.
    pub fn render_wrapped(&self, text: &str, ptsize: f32, color: [u8; 3], style: TextStyle, wrap: i32) -> Option<(u32, u32, Vec<u8>)> {
        let lock = ttf()?;
        let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
        let font = self.font.0;
        // SAFETY: our font, under the lock; the surfaces made are freed here
        unsafe {
            TTF_SetFontSize(font, ptsize);
            TTF_SetFontOutline(font, style.outline);
            let mut flags = TTF_STYLE_NORMAL;
            if style.italic {
                flags |= TTF_STYLE_ITALIC;
            }
            if style.underline {
                flags |= TTF_STYLE_UNDERLINE;
            }
            TTF_SetFontStyle(font, flags);
            TTF_SetFontHinting(font, TTF_HINTING_NORMAL);
            TTF_SetFontWrapAlignment(
                font,
                match style.align {
                    XAlign::Center => TTF_HORIZONTAL_ALIGN_CENTER,
                    XAlign::Right => TTF_HORIZONTAL_ALIGN_RIGHT,
                    XAlign::Left => TTF_HORIZONTAL_ALIGN_LEFT,
                },
            );
            let c = CString::new(text.replace('\0', " ")).ok()?;
            let surface = TTF_RenderText_Blended_Wrapped(font, c.as_ptr(), text.len(), SDL_Color { r: color[0], g: color[1], b: color[2], a: 255 }, wrap.max(0));
            TTF_SetFontOutline(font, 0);
            if surface.is_null() {
                return None;
            }
            let rgba = SDL_ConvertSurface(surface, SDL_PIXELFORMAT_RGBA32);
            SDL_DestroySurface(surface);
            if rgba.is_null() {
                return None;
            }
            let (w, h, pitch) = ((*rgba).w.max(0) as usize, (*rgba).h.max(0) as usize, (*rgba).pitch.max(0) as usize);
            let mut out = vec![0u8; w * h * 4];
            if SDL_LockSurface(rgba) {
                let pixels = (*rgba).pixels as *const u8;
                for y in 0..h {
                    std::ptr::copy_nonoverlapping(pixels.add(y * pitch), out.as_mut_ptr().add(y * w * 4), w * 4);
                }
                SDL_UnlockSurface(rgba);
            }
            SDL_DestroySurface(rgba);
            Some((w as u32, h as u32, out))
        }
    }
}

/// A pack's fonts by the names scripts use.
#[derive(Default)]
pub struct FontLibrary {
    /// Every font read, kept while the library is (VPX's `m_fonts`): a name a later font takes
    /// over — a pack's `.otf` and `.ttf` of one font — leaves the earlier one here, not closed
    /// while SDL_ttf's lock is held for reading the next.
    all: Vec<Arc<PupFont>>,
    by_name: HashMap<String, Arc<PupFont>>,
    by_file: HashMap<String, Arc<PupFont>>,
    /// Labels whose font isn't there, each named once.
    warned: Mutex<std::collections::HashSet<String>>,
}

/// A name as VPX keys it: double spaces made single, lower case.
fn key(name: &str) -> String {
    name.replace("  ", " ").to_lowercase()
}

impl FontLibrary {
    /// The fonts of the pack at `pack` (its `FONTS` folder), then Liberation Sans (VPX's
    /// `LoadFonts`).
    pub fn load(pack: &Path) -> FontLibrary {
        let mut lib = FontLibrary::default();
        let Some(lock) = ttf() else {
            crate::script::log::once("PinUP: SDL_ttf didn't start: labels' text won't show");
            return lib;
        };
        let held = lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = super::find_case_insensitive(&pack.join("FONTS"), true)
            && let Ok(entries) = std::fs::read_dir(dir)
        {
            let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
            files.sort();
            for path in files {
                // TrueType and, since VPX's newer PinUP plugin, OpenType fonts
                if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf")) {
                    match std::fs::read(&path).ok().and_then(|data| PupFont::open(&data, &held)) {
                        Some(font) => lib.add(font, &path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                        None => crate::script::log::once(&format!("PinUP: font {} can't be read", path.display())),
                    }
                }
            }
        }
        if let Some(font) = PupFont::open(FALLBACK, &held) {
            lib.add(font, "LiberationSans-Regular.ttf");
        }
        lib
    }

    /// Adds a font (VPX's `AddFont`): by its family (its file's name without the extension
    /// where it has none), by its family and style unless that's none or "Regular", and by its
    /// file's name without its extension (`.ttf`, `.otf`); a name taken already goes to the new
    /// font.
    fn add(&mut self, font: PupFont, file_name: &str) {
        let font = Arc::new(font);
        let family = if font.family.is_empty() { file_name.rsplit_once('.').map_or(file_name, |(stem, _)| stem).to_string() } else { font.family.clone() };
        self.by_name.insert(key(&family), font.clone());
        if !font.style.is_empty() && font.style != "Regular" {
            self.by_name.insert(key(&format!("{family} {}", font.style)), font.clone());
        }
        let stem = file_name.get(..file_name.len().saturating_sub(4)).unwrap_or(file_name).to_lowercase();
        self.by_file.insert(stem, font.clone());
        self.all.push(font);
    }

    /// The font a script names (VPX's `GetFont`): by family (and style), else by file name.
    pub fn get(&self, name: &str) -> Option<Arc<PupFont>> {
        self.by_name.get(&key(name)).or_else(|| self.by_file.get(&name.to_lowercase())).cloned()
    }

    /// A label's font isn't there: named once.
    pub fn missing(&self, label: &str, font: &str) {
        if self.warned.lock().unwrap_or_else(|e| e.into_inner()).insert(font.to_lowercase()) {
            crate::script::log::once(&format!("PinUP: font {font} not found (label {label})"));
        }
    }

    pub fn len(&self) -> usize {
        self.by_file.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_file.is_empty()
    }
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*d.get(at)?, *d.get(at + 1)?, *d.get(at + 2)?, *d.get(at + 3)?]))
}

/// A table of a TrueType file: its start and length.
fn table(d: &[u8], tag: &[u8; 4]) -> Option<(usize, usize)> {
    let count = u16_at(d, 4)? as usize;
    (0..count.min(64)).find_map(|i| {
        let entry = 12 + i * 16;
        (d.get(entry..entry + 4)? == tag).then(|| Some((u32_at(d, entry + 8)? as usize, u32_at(d, entry + 12)? as usize)))?
    })
}

/// VPX's `ParseOS2Metrics`: the em over Windows' ascent plus descent, and the ascent's share.
fn os2_metrics(d: &[u8]) -> Option<(f32, f32)> {
    let (head, head_len) = table(d, b"head")?;
    let (os2, os2_len) = table(d, b"OS/2")?;
    if head_len < 54 || os2_len < 78 {
        return None;
    }
    let units = u16_at(d, head + 18)?;
    let (ascent, descent) = (u16_at(d, os2 + 74)?, u16_at(d, os2 + 76)?);
    let sum = f32::from(ascent) + f32::from(descent);
    if units == 0 || sum == 0.0 {
        return None;
    }
    Some((f32::from(units) / sum, f32::from(ascent) / sum))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pack's fonts found by family, family and style, and file name, and drawn (where this
    /// machine has the test packs).
    #[test]
    fn a_packs_fonts_are_found_and_drawn() {
        let pack = Path::new(r"C:\vPinball\PinUPSystem\PUPVideos\DarkestDungeon");
        if !pack.exists() {
            return;
        }
        let lib = FontLibrary::load(pack);
        assert!(lib.len() > 1, "the pack's fonts and the fallback");
        // Darkest Dungeon's script names its font so
        let font = lib.get("DwarvenAxe BB W00 Regular").or_else(|| lib.get("DwarvenAxe BB W00")).expect("DwarvenAxe");
        let fallback = lib.get("Liberation Sans").expect("the fallback");
        for f in [&font, &fallback] {
            let (w, h, rgba) = f.render("12,345", f.metrics(40.0).ptsize, [255, 128, 0], TextStyle { italic: false, underline: false, align: XAlign::Left, outline: 0 }).expect("drawn");
            assert!(w > 40 && h > 20, "{w}x{h}");
            assert!(rgba.chunks(4).any(|p| p[3] > 200 && p[0] == 255), "lit in its colour");
        }
        // a border, wider by twice its outline
        let plain = fallback.render("A", 30.0, [255; 3], TextStyle { italic: false, underline: false, align: XAlign::Left, outline: 0 }).expect("drawn");
        let bordered = fallback.render("A", 30.0, [255; 3], TextStyle { italic: false, underline: false, align: XAlign::Left, outline: 3 }).expect("drawn");
        assert_eq!(bordered.0, plain.0 + 6);
    }

    /// A pack holding one font as both `.otf` and `.ttf` (Leprechaun King's) has both read, the
    /// later taking the names, the earlier kept rather than closed under SDL_ttf's lock (which
    /// hung the player).
    #[test]
    fn a_font_twice_in_a_pack_is_read_without_hanging() {
        let pack = Path::new(r"C:\vPinball\PinUPSystem\PUPVideos\leprechaun");
        if !pack.exists() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let lib = FontLibrary::load(pack);
            let _ = tx.send((lib.len(), lib.all.len(), lib.get("MorrisRomanBlack").is_some()));
        });
        let (names, all, found) = rx.recv_timeout(std::time::Duration::from_secs(30)).expect("read without hanging");
        assert!(found);
        assert!(all > names, "the .otf and .ttf of one font both kept");
    }
}
