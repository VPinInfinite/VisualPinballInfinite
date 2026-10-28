// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): FlexDMD/AssetManager.cs (AssetSrc, ResolveSrc,
// Open, GetBitmap, GetFont), FlexDMD/BitmapFilter.cs (the region, padding, dot and additive filters) and
// FlexDMD/VPXFile.cs (pictures inside the table file, by name). Copyright 2019 Vincent Bousquet, licensed under the
// Apache License, Version 2.0 (LICENSE-FlexDMD.txt beside this file). Changed in the porting, from C# to Rust, for
// Visual Pinball Infinite: pictures decoded by GDI+ itself (`gdiplus`), else by the `image` crate as GDI+ would hold
// them (see `surface`), the table's pictures taken from the table already loaded, FlexDMD's own resources built in (`resources`); the port is
// GPL-3.0-or-later.

//! Where FlexDMD's pictures and fonts come from, and its cache of them.
//!
//! A source is a name and options joined by `&`: a file (from the project
//! folder), one of FlexDMD's own resources (`FlexDMD.Resources.…`), or a
//! picture inside the table file (`VPX.…`), then filters for pictures
//! (`&region=x,y,w,h`, `&pad=l,t,r,b`, `&dmd=n`, `&add`) or a font's tint
//! and border (`&tint=AARRGGBB&border_size=n&border_tint=AARRGGBB`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use super::font::Font;
use super::surface::Bitmap;

/// Where a source lives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SrcType {
    #[default]
    File,
    FlexResource,
    VpxResource,
}

/// What a source holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AssetType {
    #[default]
    Unknown,
    Image,
    Gif,
    Video,
    BmFont,
}

/// A picture filter (FlexDMD's `IBitmapFilter`s).
#[derive(Clone, Debug, PartialEq)]
pub enum Filter {
    /// `region=x,y,w,h`: a part of the picture.
    Region([i32; 4]),
    /// `pad=left,top,right,bottom`: clear space around it.
    Pad([i32; 4]),
    /// `dmd=n` (and `dmd2=n`): each n×n block made one dot.
    Dot(i32),
    /// `add`: dark pixels made clear.
    Additive,
}

/// A source, resolved (FlexDMD's `AssetSrc`).
#[derive(Clone, Debug, Default)]
pub struct AssetSrc {
    pub id: String,
    pub path: String,
    pub kind: AssetType,
    pub src_type: SrcType,
    pub filters: Vec<Filter>,
    /// A font's tint and border, as GDI+ colours: alpha, red, green, blue.
    pub font_tint: [u8; 4],
    pub font_border_tint: [u8; 4],
    pub font_border_size: i32,
}

impl AssetSrc {
    /// The id without its options: the base picture a filtered one comes from.
    pub fn id_without_options(&self) -> &str {
        self.id.split('&').next().unwrap_or("")
    }
}

/// A picture inside the table file, as FlexDMD finds it: the file it was
/// imported from (for its type) and its bytes, when the table keeps them.
struct VpxPicture {
    file: String,
    data: Option<Arc<[u8]>>,
}

/// The pictures cached, as FlexDMD caches them: decoded, and whether the
/// picture has frames in time (a GIF), which FlexDMD neither swaps nor filters.
struct Cached {
    bitmap: Rc<Bitmap>,
}

/// FlexDMD's `AssetManager`.
pub struct AssetManager {
    /// Where file sources are found from (`ProjectFolder`).
    pub base_path: String,
    pub table_file: Option<String>,
    /// The folder relative paths start from: the table's, as VPX's current folder is.
    pub table_dir: Option<PathBuf>,
    /// The table's pictures, by name in lower case, when the table file is the
    /// one loaded.
    table_pictures: Option<Arc<vpin::vpx::VPX>>,
    table_path: Option<PathBuf>,
    vpx: Option<HashMap<String, VpxPicture>>,
    bitmaps: HashMap<String, Cached>,
    gifs: HashMap<String, Rc<super::animated::GifFrames>>,
    fonts: HashMap<String, Rc<Font>>,
}

impl AssetManager {
    pub fn new(table_dir: Option<PathBuf>, table_path: Option<PathBuf>, table: Option<Arc<vpin::vpx::VPX>>) -> AssetManager {
        AssetManager {
            base_path: "./".into(),
            table_file: None,
            table_dir,
            table_pictures: table,
            table_path,
            vpx: None,
            bitmaps: HashMap::new(),
            gifs: HashMap::new(),
            fonts: HashMap::new(),
        }
    }

    /// FlexDMD's `ClearAll`: forgets every picture and font.
    pub fn clear_all(&mut self) {
        self.bitmaps.clear();
        self.gifs.clear();
        self.fonts.clear();
        self.vpx = None;
    }

    /// C#'s `Path.Combine`: the second part when it's absolute.
    fn combine(a: &str, b: &str) -> String {
        let pb = Path::new(b);
        if pb.has_root() || b.contains(':') {
            return b.to_string();
        }
        if a.is_empty() {
            return b.to_string();
        }
        if a.ends_with('\\') || a.ends_with('/') { format!("{a}{b}") } else { format!("{a}\\{b}") }
    }

    /// A path FlexDMD would open, made absolute from the table's folder, as
    /// VPX's current folder is the table's. A folder a script named from its
    /// own current folder — the player's data folder, behind the fence
    /// — and that isn't there is looked for beside the table: so
    /// `GetAbsolutePathName(".") & "\x.UltraDMD"` finds the table's pictures.
    pub fn file_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path.replace('/', "\\"));
        let p = if p.is_absolute() { p } else { self.table_dir.clone().unwrap_or_default().join(p) };
        // `..` and `.` taken out, as Windows does when it opens the file
        let mut out = PathBuf::new();
        for c in p.components() {
            match c {
                std::path::Component::ParentDir => {
                    out.pop();
                }
                std::path::Component::CurDir => {}
                c => out.push(c),
            }
        }
        if !out.exists()
            && let (Some(data), Some(table)) = (crate::script::api::user_directory(), self.table_dir.as_ref())
            && let Ok(rest) = out.strip_prefix(&data)
        {
            let beside = table.join(rest);
            if beside.exists() {
                return beside;
            }
        }
        out
    }

    /// The table file's pictures, read the first time a `VPX.` source asks,
    /// as FlexDMD opens the table file then.
    fn vpx_pictures(&mut self) -> Option<&HashMap<String, VpxPicture>> {
        if self.vpx.is_none() {
            let file = self.table_file.clone()?;
            let path = self.file_path(&Self::combine(&self.base_path, &file));
            if !path.is_file() {
                return None;
            }
            let same = self.table_path.as_deref().is_some_and(|t| t.canonicalize().ok() == path.canonicalize().ok());
            let vpx = if same {
                self.table_pictures.clone()
            } else {
                vpin::vpx::read(&path).ok().map(|mut v| {
                    crate::pictures::resolve_links(&mut v);
                    Arc::new(v)
                })
            };
            let mut map = HashMap::new();
            for im in vpx.iter().flat_map(|v| v.images.iter()) {
                let data = im.jpeg.as_ref().map(|j| Arc::<[u8]>::from(j.data.as_slice()));
                map.insert(im.name.to_lowercase(), VpxPicture { file: im.path.clone(), data });
            }
            self.vpx = Some(map);
        }
        self.vpx.as_ref()
    }

    /// FlexDMD's `ResolveSrc`, from `base` when a font's pages are found from
    /// the font. Err where FlexDMD throws (a `|` in the name).
    pub fn resolve(&mut self, src: &str, base: Option<&AssetSrc>) -> Result<AssetSrc, String> {
        if src.contains('|') {
            return Err("'|' is not allowed inside file names as it is the separator for image sequences".into());
        }
        let mut parts: Vec<String> = src.split('&').map(str::to_string).collect();
        if let Some(b) = base {
            parts[0] = match b.src_type {
                SrcType::FlexResource => format!("FlexDMD.Resources.{}", parts[0]),
                SrcType::VpxResource => format!("VPX.{}", parts[0]),
                SrcType::File => Self::combine(&Self::combine(&b.path, ".."), &parts[0]),
            };
        }
        let mut def = AssetSrc { id: parts.join("&"), font_tint: [255; 4], font_border_tint: [255; 4], ..Default::default() };
        let tail = |s: &str| -> String {
            let c: Vec<char> = s.chars().collect();
            if c.len() > 4 { c[c.len() - 4..].iter().collect::<String>().to_lowercase() } else { String::new() }
        };
        let mut ext = tail(&parts[0]);
        if parts[0].starts_with("FlexDMD.Resources.") {
            def.src_type = SrcType::FlexResource;
            def.path = parts[0].clone();
        } else if let Some(name) = parts[0].strip_prefix("VPX.") {
            def.src_type = SrcType::VpxResource;
            def.path = name.to_string();
            ext = String::new();
            let key = name.to_lowercase();
            if let Some(pics) = self.vpx_pictures()
                && let Some(p) = pics.get(&key)
            {
                def.kind = AssetType::Image;
                ext = tail(&p.file);
            }
        } else {
            def.src_type = SrcType::File;
            def.path = if base.is_none() { Self::combine(&self.base_path, &parts[0]) } else { parts[0].clone() };
        }
        match ext.as_str() {
            ".png" | ".jpg" | ".bmp" => def.kind = AssetType::Image,
            ".wmv" | ".avi" | ".mp4" => def.kind = AssetType::Video,
            ".gif" => def.kind = AssetType::Gif,
            ".fnt" => def.kind = AssetType::BmFont,
            _ => {}
        }
        let ints = |s: &str| -> Option<[i32; 4]> {
            let v: Vec<i32> = s.split(',').map(|n| n.trim().parse().ok()).collect::<Option<_>>()?;
            v.get(..4)?.try_into().ok()
        };
        let options = parts.iter().skip(1);
        match def.kind {
            AssetType::Image => {
                for o in options {
                    if let Some(n) = o.strip_prefix("dmd=").and_then(|n| n.parse().ok()) {
                        def.filters.push(Filter::Dot(n));
                    } else if let Some(n) = o.strip_prefix("dmd2=").and_then(|n| n.parse().ok()) {
                        def.filters.push(Filter::Dot(n));
                    } else if o.starts_with("add") {
                        def.filters.push(Filter::Additive);
                    } else if let Some(r) = o.strip_prefix("region=").and_then(ints) {
                        def.filters.push(Filter::Region(r));
                    } else if let Some(r) = o.strip_prefix("pad=").and_then(ints) {
                        def.filters.push(Filter::Pad(r));
                    }
                }
            }
            AssetType::BmFont => {
                for o in options {
                    let hex = |s: &str| u32::from_str_radix(s.trim(), 16).ok().map(|v| v.to_be_bytes());
                    if let Some(c) = o.strip_prefix("tint=").and_then(hex) {
                        def.font_tint = c;
                    } else if let Some(c) = o.strip_prefix("border_tint=").and_then(hex) {
                        def.font_border_tint = c;
                    } else if let Some(n) = o.strip_prefix("border_size=").and_then(|n| n.trim().parse().ok()) {
                        def.font_border_size = n;
                    }
                }
            }
            _ => {}
        }
        Ok(def)
    }

    /// FlexDMD's `Open`: a source's bytes.
    pub fn open(&mut self, src: &AssetSrc) -> Option<Vec<u8>> {
        match src.src_type {
            SrcType::File => {
                let path = self.file_path(&src.path);
                if !crate::script::files::readable(&path) {
                    return None;
                }
                std::fs::read(path).ok()
            }
            SrcType::FlexResource => {
                // FlexDMD's `GetManifestResourceStream`: a name FlexDMD doesn't carry opens nothing
                super::resources::get(src.path.strip_prefix("FlexDMD.Resources.")?).map(<[u8]>::to_vec)
            }
            SrcType::VpxResource => {
                let key = src.path.to_lowercase();
                self.vpx_pictures()?.get(&key)?.data.as_ref().map(|d| d.to_vec())
            }
        }
    }

    /// FlexDMD's `GetBitmap`: a picture or the first frame of a GIF, filtered
    /// as its source says, from the cache when it's there.
    pub fn bitmap(&mut self, src: &AssetSrc) -> Result<Rc<Bitmap>, String> {
        if src.kind != AssetType::Image && src.kind != AssetType::Gif {
            return Err(format!("Asked to load a bitmap from a resource of type {:?}", src.kind));
        }
        if let Some(c) = self.bitmaps.get(&src.id) {
            return Ok(c.bitmap.clone());
        }
        let base_id = src.id_without_options().to_string();
        let animated = src.kind == AssetType::Gif;
        if let Some(c) = self.bitmaps.get(&base_id) {
            let mut bitmap = c.bitmap.clone();
            if !animated {
                for f in &src.filters {
                    bitmap = Rc::new(filter(&bitmap, f));
                }
            }
            self.bitmaps.insert(src.id.clone(), Cached { bitmap: bitmap.clone() });
            return Ok(bitmap);
        }
        let bytes = self.open(src).ok_or_else(|| format!("can't open {}", src.path))?;
        let bitmap = Rc::new(decode(&bytes, !animated).ok_or_else(|| format!("can't read {} as a picture", src.path))?);
        self.bitmaps.insert(base_id, Cached { bitmap: bitmap.clone() });
        if !animated && !src.filters.is_empty() {
            let mut b = bitmap;
            for f in &src.filters {
                b = Rc::new(filter(&b, f));
            }
            self.bitmaps.insert(src.id.clone(), Cached { bitmap: b.clone() });
            return Ok(b);
        }
        Ok(bitmap)
    }

    /// An animated GIF's frames, as FlexDMD's `GIFImage` shows them (from the
    /// cache, as FlexDMD shares the GIF's picture).
    pub fn gif(&mut self, src: &AssetSrc) -> Result<Rc<super::animated::GifFrames>, String> {
        if src.kind != AssetType::Gif {
            return Err(format!("Asked to load a GIF from a resource of type {:?}", src.kind));
        }
        let id = src.id_without_options().to_string();
        if let Some(g) = self.gifs.get(&id) {
            return Ok(g.clone());
        }
        let bytes = self.open(src).ok_or_else(|| format!("can't open {}", src.path))?;
        let g = Rc::new(super::animated::decode_gif(&bytes).ok_or_else(|| format!("can't read {} as a GIF", src.path))?);
        self.gifs.insert(id, g.clone());
        Ok(g)
    }

    /// FlexDMD's `GetFont`, from the cache when it's there.
    pub fn font(&mut self, src: &AssetSrc) -> Result<Rc<Font>, String> {
        if src.kind != AssetType::BmFont {
            return Err(format!("Asked to load a font from a resource of type {:?}", src.kind));
        }
        if let Some(f) = self.fonts.get(&src.id) {
            return Ok(f.clone());
        }
        let font = Rc::new(Font::load(self, src)?);
        self.fonts.insert(src.id.clone(), font.clone());
        Ok(font)
    }
}

/// A picture's bytes decoded as GDI+ holds them, then as FlexDMD keeps
/// them: `swapped` when FlexDMD swapped red and blue on loading (its still
/// pictures), so its bytes read red, green, blue; else GDI+'s own order,
/// blue, green, red (its fonts' sheets, GIFs).
pub fn decode(bytes: &[u8], swapped: bool) -> Option<Bitmap> {
    // through GDI+, FlexDMD's own decoder, for the very same pixels
    #[cfg(windows)]
    if let Some((width, height, bgra, format)) = super::gdiplus::decode(bytes) {
        // held with alpha: GDI+'s formats with alpha (PixelFormatAlpha, PixelFormatPAlpha), and a
        // palette with a see-through colour (an indexed picture, PixelFormatIndexed, read with alpha)
        let alpha = format & 0x000C_0000 != 0 || (format & 0x0001_0000 != 0 && bgra.iter().any(|p| p[3] < 255));
        let px = bgra
            .into_iter()
            .map(|[b, g, r, a]| {
                let a = if alpha { a } else { 255 };
                if swapped { [r, g, b, a] } else { [b, g, r, a] }
            })
            .collect();
        return Some(Bitmap { width, height, px, alpha });
    }
    let img = image::load_from_memory(bytes).ok()?;
    let alpha = img.color().has_alpha();
    let rgba = img.to_rgba8();
    let px = rgba
        .pixels()
        .map(|p| {
            let a = if alpha { p[3] } else { 255 };
            if swapped { [p[0], p[1], p[2], a] } else { [p[2], p[1], p[0], a] }
        })
        .collect();
    Some(Bitmap { width: rgba.width(), height: rgba.height(), px, alpha })
}

/// One of FlexDMD's filters applied to a picture (`BitmapFilter.cs`).
pub fn filter(src: &Bitmap, f: &Filter) -> Bitmap {
    match *f {
        Filter::Region([x, y, w, h]) => {
            let (w, h) = (w.max(0) as u32, h.max(0) as u32);
            let mut dst = Bitmap { alpha: src.alpha, ..Bitmap::new(w, h) };
            copy_into(&mut dst, src, -x, -y);
            dst
        }
        Filter::Pad([l, t, r, b]) => {
            let (w, h) = ((src.width as i32 + l + r).max(0) as u32, (src.height as i32 + t + b).max(0) as u32);
            let mut dst = Bitmap { alpha: src.alpha, ..Bitmap::new(w, h) };
            copy_into(&mut dst, src, l, t);
            dst
        }
        Filter::Dot(size) => {
            let size = size.max(1) as u32;
            let (w, h) = (src.width / size, src.height / size);
            let mut dst = Bitmap::new(w, h);
            // C#: 1 + DotSize * DotSize / 1.8f, in single precision
            let bright = 1.0f32 + (size * size) as f32 / 1.8f32;
            for y in 0..h {
                for x in 0..w {
                    let mut sum = [0u32; 4];
                    for i in 0..size {
                        for j in 0..size {
                            let p = src.px[((y * size + j) * src.width + x * size + i) as usize];
                            for c in 0..4 {
                                sum[c] += u32::from(p[c]);
                            }
                        }
                    }
                    dst.px[(y * w + x) as usize] = sum.map(|s| (s as f32 / bright).min(255.0) as u8);
                }
            }
            dst
        }
        Filter::Additive => {
            let mut dst = Bitmap { width: src.width, height: src.height, px: src.px.clone(), alpha: true };
            for p in &mut dst.px {
                if p[0] < 64 && p[1] < 64 && p[2] < 64 {
                    p[3] = 0;
                }
            }
            dst
        }
    }
}

/// GDI+'s copy of a picture onto a cleared one of the same format, at
/// (`dx`, `dy`): a 32-bit destination keeps alpha, its colours through
/// premultiplied alpha (rounded) and back through GDI+'s 16-bit reciprocal of
/// the alpha (truncated) — measured against GDI+ for every alpha and colour;
/// a 24-bit one is opaque, black where nothing lands.
fn copy_into(dst: &mut Bitmap, src: &Bitmap, dx: i32, dy: i32) {
    if !dst.alpha {
        dst.px.fill([0, 0, 0, 255]);
    }
    for y in 0..dst.height as i32 {
        for x in 0..dst.width as i32 {
            let (sx, sy) = (x - dx, y - dy);
            if sx < 0 || sy < 0 || sx >= src.width as i32 || sy >= src.height as i32 {
                continue;
            }
            let s = src.px[(sy as u32 * src.width + sx as u32) as usize];
            let d = &mut dst.px[(y as u32 * dst.width + x as u32) as usize];
            if !dst.alpha || s[3] == 255 {
                *d = [s[0], s[1], s[2], 255];
            } else if s[3] == 0 {
                *d = [0; 4];
            } else {
                let a = u32::from(s[3]);
                let inverse = (255 << 16) / a;
                let un = |c: u8| {
                    let p = (u32::from(c) * a + 127) / 255;
                    ((p * inverse) >> 16).min(255) as u8
                };
                *d = [un(s[0]), un(s[1]), un(s[2]), s[3]];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_keeps_see_through_colours_as_gdiplus_does() {
        // GDI+'s own results (DrawImage onto a cleared 32-bit picture), as (alpha, colour, colour after)
        for (a, c, after) in [(7u8, 237u8, 254u8), (1, 255, 255), (3, 100, 85), (128, 100, 99), (200, 13, 12), (254, 254, 253), (51, 77, 75), (255, 9, 9)] {
            let src = Bitmap { width: 1, height: 1, px: vec![[c, c, c, a]], alpha: true };
            let mut dst = Bitmap { alpha: true, ..Bitmap::new(1, 1) };
            copy_into(&mut dst, &src, 0, 0);
            assert_eq!(dst.px[0], [after, after, after, a], "alpha {a}, colour {c}");
        }
    }

    #[test]
    fn its_own_resources_open_by_flexdmds_names() {
        let mut m = AssetManager::new(None, None, None);
        let font = m.resolve("FlexDMD.Resources.udmd-f5by7.fnt", None).unwrap();
        assert!(Font::load(&mut m, &font).is_ok_and(|f| !f.bm.pages.is_empty()));
        // a folder inside FlexDMD is joined by a dot; a name FlexDMD doesn't carry opens nothing
        let black = m.resolve("FlexDMD.Resources.dmds.black.png", None).unwrap();
        assert!(m.open(&black).is_some());
        let missing = m.resolve("FlexDMD.Resources.nothing.fnt", None).unwrap();
        assert!(m.open(&missing).is_none() && Font::load(&mut m, &missing).is_err());
    }

    #[test]
    fn sources_resolve_as_flexdmd_resolves_them() {
        let mut m = AssetManager::new(None, None, None);
        let s = m.resolve("FlexDMD.Resources.udmd-f5by7.fnt&tint=FFFF0000&border_size=1&border_tint=FF00FF00", None).unwrap();
        assert_eq!((s.src_type, s.kind, s.font_border_size), (SrcType::FlexResource, AssetType::BmFont, 1));
        assert_eq!((s.font_tint, s.font_border_tint), ([255, 255, 0, 0], [255, 0, 255, 0]));
        // a font's page from the font
        let page = m.resolve("udmd-f5by7.png", Some(&s)).unwrap();
        assert_eq!((page.path.as_str(), page.kind), ("FlexDMD.Resources.udmd-f5by7.png", AssetType::Image));
        let s = m.resolve("background.png&region=0,32,128,32&dmd=2&add", None).unwrap();
        assert_eq!(s.path, "./background.png");
        assert_eq!(s.filters, vec![Filter::Region([0, 32, 128, 32]), Filter::Dot(2), Filter::Additive]);
        assert_eq!(s.id_without_options(), "background.png");
        // FlexDMD compares a four-letter tail: ".jpeg" is no picture to it
        assert_eq!(m.resolve("photo.jpeg", None).unwrap().kind, AssetType::Unknown);
        assert_eq!(m.resolve("clip.mp4", None).unwrap().kind, AssetType::Video);
        assert!(m.resolve("a.png|b.png", None).is_err());
        let f = m.resolve("fonts/x.fnt", None).unwrap();
        assert_eq!(m.resolve("x.png", Some(&f)).unwrap().path, r"./fonts/x.fnt\..\x.png");
    }

    #[test]
    fn filters_change_pictures_as_flexdmd_does() {
        // two dots' worth of 2×2 blocks: a sum of 4 × 200 over 1 + 4 / 1.8
        let src = Bitmap { width: 4, height: 2, px: vec![[200, 100, 0, 255]; 8], alpha: true };
        let d = filter(&src, &Filter::Dot(2));
        assert_eq!((d.width, d.height), (2, 1));
        assert_eq!(d.px[0], [248, 124, 0, 255]);
        let src = Bitmap { width: 2, height: 1, px: vec![[63, 63, 63, 255], [64, 0, 0, 255]], alpha: true };
        let a = filter(&src, &Filter::Additive);
        assert_eq!(a.px, vec![[63, 63, 63, 0], [64, 0, 0, 255]]);
        // a region keeps colours through premultiplied alpha, as GDI+ does
        let src = Bitmap { width: 4, height: 1, px: vec![[200, 100, 50, 10], [201, 99, 3, 128], [7, 8, 9, 255], [255, 255, 255, 1]], alpha: true };
        let r = filter(&src, &Filter::Region([0, 0, 4, 1]));
        assert_eq!(r.px, vec![[204, 102, 51, 10], [201, 99, 3, 128], [7, 8, 9, 255], [255, 255, 255, 1]]);
        // outside the picture: clear, or black for a picture without alpha
        let r = filter(&Bitmap { alpha: false, ..src.clone() }, &Filter::Region([3, 0, 2, 1]));
        assert_eq!(r.px, vec![[255, 255, 255, 255], [0, 0, 0, 255]]);
    }
}
