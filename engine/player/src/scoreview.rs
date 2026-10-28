// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): the score view
// plugin's layout files, their reading, the choice of a layout and its fitting (plugins/scoreview/ScoreView.cpp and
// ScoreViewPlugin.cpp; its default layouts are VPX's own files, engine/player/assets/vpx/scoreview), how a layout's
// sources are found (ResURIResolver::GetSegDisplayState, plugins/plugins/ResURIResolver.cpp), the display profiles'
// default settings (src/core/Settings_properties.inl), and how the newer display renderer is set up for a profile
// (Renderer::SetupDMDRender, SetupDisplayRenderer and DrawMatrixDisplay, src/renderer/Renderer.cpp).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The score display's own window, as VPX's score view draws it: a layout places the game's displays — a DMD, or segment
//! displays digit by digit — and the whole is fitted inside the window
//! keeping its shape, black around it.
//!
//! The layouts are VPX's: a file beside the table named as the table, else
//! one named after the table's folder, else VPX's defaults, built into the
//! player. Of those, the one whose displays the game has, the most of them,
//! fitting the window's shape best, is drawn; with none, the window stays
//! black for a DMD, as in VPX, and segment displays are placed as PinMAME's
//! own window places them. The glass a layout names — VPX's pictures
//! of dirt and cracks for its defaults, built in, or a file beside a table's
//! own layout — lies over the display, its roughness spreading the dots'
//! light into a glow, as VPX's score view draws it (VPX's pictures, their
//! author unknown).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::segments::{SegHint, SegStyle};

/// One of VPX's display profiles: how its dots look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DotProfile {
    /// Drawn with the legacy DMD shader instead (by default only the first).
    pub legacy: bool,
    /// A lit dot's colour, as Windows keeps colours (red in the lowest byte), sRGB.
    pub tint: u32,
    /// An unlit dot's colour, likewise.
    pub unlit: u32,
    /// A lit dot's brightness.
    pub brightness: f32,
    /// The dot's size, as a part of its cell.
    pub size: f32,
    /// From round (0) to square (1).
    pub sharpness: f32,
    /// The glow of the dots on the back of the display, against the glass.
    pub glow: f32,
}

const fn profile(legacy: bool, tint: u32, size: f32) -> DotProfile {
    DotProfile { legacy, tint, unlit: 0x0040_4040, brightness: 25.0, size, sharpness: 0.9, glow: 0.1 }
}

/// VPX's seven display profiles with build 5436's default settings: legacy,
/// neon plasma, red LED, green LED, yellow LED, generic plasma, generic LED.
pub const PROFILES: [DotProfile; 7] = [
    profile(true, 0x002D_52FF, 0.75),
    profile(false, 0x002D_52FF, 0.6),
    profile(false, 0x0015_23FF, 0.75),
    profile(false, 0x0023_FF15, 0.75),
    profile(false, 0x0023_FFFF, 0.75),
    profile(false, 0x00FF_FFFF, 0.6),
    profile(false, 0x00FF_FFFF, 0.75),
];

/// The profiles as the settings give them (`[DMD]` `Profile1…` to `Profile7…`,
/// `vpx_settings::dmd_profiles`), set once as play starts.
static PROFILE_SETTINGS: std::sync::OnceLock<[DotProfile; 7]> = std::sync::OnceLock::new();

/// Sets the profiles from the settings, once; later calls change nothing.
pub fn set_profiles(profiles: [DotProfile; 7]) {
    let _ = PROFILE_SETTINGS.set(profiles);
}

/// VPX's seven display profiles: the settings' once set, else build 5436's defaults.
pub fn profiles() -> &'static [DotProfile; 7] {
    PROFILE_SETTINGS.get().unwrap_or(&PROFILES)
}

/// The profile VPX's layouts give a DMD unless they name another.
pub const NEON_PLASMA: usize = 1;

/// What a layout shows in one place (VPX's `VisualType`).
#[derive(Clone, Debug, PartialEq)]
pub enum VisualKind {
    /// A DMD of this many dots across and down (or of its shape), in a profile.
    Dmd { size: (i32, i32), profile: usize },
    /// A video screen (LCD or CRT): VPX's, not carried by our PinMAME link.
    Screen { size: (i32, i32) },
    /// A segment display: each digit's place across, and its style (None:
    /// from the display's hardware).
    Segments { x_offsets: Vec<f32>, style: Option<SegStyle> },
}

/// One visual of a layout (VPX's `ScoreView::Visual`).
#[derive(Clone, Debug, PartialEq)]
pub struct Visual {
    pub kind: VisualKind,
    /// Left, top, width and height, in the layout's units from its top left.
    pub rect: [f32; 4],
    /// Where its frames come from (`ctrl://default/display`, `ctrl://default/seg?id=0`).
    pub src: String,
    pub glass_tint: [f32; 3],
    pub glass_roughness: f32,
    /// The room's light on the glass, linear (with a glass picture only).
    pub glass_ambient: [f32; 3],
    /// The glass picture, as the layout names it.
    pub glass_image: Option<String>,
    /// The part of the glass picture over the visual: left, top, width and
    /// height in the picture's pixels (none: all of it).
    pub glass_area: [f32; 4],
}

/// A layout (VPX's `ScoreView::Layout`): its size in its own units, and what it shows where.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub name: String,
    /// The folder of the file it was read from (None: one of VPX's, built in),
    /// where the glass pictures it names are found.
    pub dir: Option<PathBuf>,
    pub width: f32,
    pub height: f32,
    pub visuals: Vec<Visual>,
}

/// sRGB's curve undone (VPX's `InvsRGB`).
fn inv_srgb(x: f32) -> f32 {
    if x <= 0.04045 { x / 12.92 } else { (x / 1.055 + 0.055 / 1.055).powf(2.4) }
}

/// `[a, b, ...]`, as VPX reads it: numbers between brackets and commas, on one line.
fn parse_array(value: &str) -> Option<Vec<f32>> {
    let open = value.find('[')?;
    let close = value[open..].find(']')? + open;
    value[open + 1..close].split(',').map(|v| v.trim().parse::<f32>().ok()).collect()
}

/// Reads a layout file as VPX's score view reads it (`ScoreView::Parse`):
/// lines of `Key: value`, indented by a fixed step to nest them, `#` for
/// comments. A field out of place or a value it can't read rejects the file.
pub fn parse(name: &str, text: &str) -> Result<Layout, String> {
    let mut layout = Layout { name: name.to_string(), dir: None, width: 1920.0, height: 1080.0, visuals: Vec::new() };
    let mut visual: Option<usize> = None;
    let mut indent_size = 0;
    let mut expected = 0;
    // a visual's element count, as written (`NElements`), before its places are known
    let mut counts: Vec<Option<usize>> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line_no = n + 1;
        let line = raw.trim_end_matches([' ', '\t']);
        if line.is_empty() {
            continue;
        }
        let after = line.find(|c| c != ' ' && c != '\t').unwrap_or(0);
        if indent_size == 0 && after != 0 {
            indent_size = after;
        }
        if indent_size != 0 && after % indent_size != 0 {
            return Err(format!("{name}: uneven indentation at line {line_no}"));
        }
        let indent = after.checked_div(indent_size).unwrap_or(0);
        if indent > expected {
            return Err(format!("{name}: indented {indent} where {expected} was expected, at line {line_no}"));
        }
        if indent < expected {
            if indent <= 1 {
                visual = None;
            }
            expected = indent;
        }
        let body = &line[after..];
        if body.starts_with('#') {
            continue;
        }
        let colon = body.find(':').ok_or_else(|| format!("{name}: no ':' at line {line_no}"))?;
        if colon == 0 {
            return Err(format!("{name}: no key before ':' at line {line_no}"));
        }
        let (key, value) = (&body[..colon], body[colon + 1..].trim_start_matches([' ', '\t']));
        let bad = || format!("{name}: invalid field '{key}: {value}' at line {line_no}");
        let current = |v: Option<usize>| v.filter(|_| indent == 2).ok_or_else(bad);
        let glass = |v: Option<usize>| v.filter(|_| indent == 3).ok_or_else(bad);
        match key {
            "Width" | "Height" => {
                let v = value.parse::<f32>().ok().filter(|v| indent == 0 && *v >= 1.0).ok_or_else(bad)?;
                if key == "Width" {
                    layout.width = v;
                } else {
                    layout.height = v;
                }
            }
            "Fit" if indent == 0 && value == "Contain" => {}
            "Fit" => return Err(bad()),
            "Visuals" if indent == 0 => expected = 1,
            "Visuals" => return Err(bad()),
            "- DMD" | "- Screen" | "- SegDisplay" => {
                if indent != 1 {
                    return Err(bad());
                }
                expected = 2;
                let kind = match key {
                    "- DMD" => VisualKind::Dmd { size: (-1, -1), profile: NEON_PLASMA },
                    "- Screen" => VisualKind::Screen { size: (-1, -1) },
                    _ => VisualKind::Segments { x_offsets: Vec::new(), style: None },
                };
                let src = if key == "- SegDisplay" { String::new() } else { "ctrl://default/display".to_string() };
                layout.visuals.push(Visual {
                    kind,
                    rect: [0.0; 4],
                    src,
                    glass_tint: [1.0; 3],
                    glass_roughness: 0.0,
                    glass_ambient: [1.0; 3],
                    glass_image: None,
                    glass_area: [0.0; 4],
                });
                counts.push(None);
                visual = Some(layout.visuals.len() - 1);
            }
            // pictures in a layout aren't supported by VPX either
            "- Image" => return Err(bad()),
            "Rect" => {
                let v = current(visual)?;
                let r = parse_array(value).filter(|r| r.len() == 4).ok_or_else(bad)?;
                layout.visuals[v].rect = [r[0], r[1], r[2], r[3]];
            }
            "Pad" => {
                // a padding inside the rectangle: VPX accepts only none
                current(visual)?;
                let r = parse_array(value).filter(|r| r.len() == 4).ok_or_else(bad)?;
                if r.iter().any(|&p| p != 0.0) {
                    return Err(bad());
                }
            }
            "Style" => {
                let v = current(visual)?;
                match &mut layout.visuals[v].kind {
                    VisualKind::Dmd { profile, .. } => {
                        *profile = ["Neon Plasma", "Red LED", "Green LED", "Yellow LED", "Generic Plasma", "Generic LED"].iter().position(|s| *s == value).ok_or_else(bad)? + 1;
                    }
                    VisualKind::Screen { .. } if value == "LCD" || value == "CRT" => {}
                    VisualKind::Screen { .. } => return Err(bad()),
                    VisualKind::Segments { style, .. } => {
                        *style = match value {
                            "Auto" => None,
                            "Neon Plasma" => Some(SegStyle::Plasma),
                            "Blue VFD" => Some(SegStyle::BlueVfd),
                            "Green VFD" => Some(SegStyle::GreenVfd),
                            "Red LED" => Some(SegStyle::RedLed),
                            "Green LED" => Some(SegStyle::GreenLed),
                            "Yellow LED" => Some(SegStyle::YellowLed),
                            "Generic Plasma" => Some(SegStyle::GenericPlasma),
                            "Generic LED" => Some(SegStyle::GenericLed),
                            _ => return Err(bad()),
                        };
                    }
                }
            }
            "Src" => {
                let v = current(visual)?;
                layout.visuals[v].src = value.to_string();
            }
            "Size" => {
                let v = current(visual)?;
                let s = parse_array(value).filter(|s| s.len() == 2).ok_or_else(bad)?;
                match &mut layout.visuals[v].kind {
                    VisualKind::Dmd { size, .. } | VisualKind::Screen { size } => *size = (s[0] as i32, s[1] as i32),
                    VisualKind::Segments { .. } => return Err(bad()),
                }
            }
            // the maker of a segment display's digits: read, as VPX reads it, and not used, as there
            "Type" => {
                let v = current(visual)?;
                if !matches!(layout.visuals[v].kind, VisualKind::Segments { .. }) || !["Atari", "Bally", "Gottlieb", "Williams"].contains(&value) {
                    return Err(bad());
                }
            }
            "XPos" => {
                let v = current(visual)?;
                let places = parse_array(value).filter(|p| !p.is_empty() && counts[v].is_none_or(|n| n == p.len())).ok_or_else(bad)?;
                match &mut layout.visuals[v].kind {
                    VisualKind::Segments { x_offsets, .. } => *x_offsets = places,
                    _ => return Err(bad()),
                }
            }
            "NElements" => {
                let v = current(visual)?;
                let n = value.parse::<usize>().ok().ok_or_else(bad)?;
                match &layout.visuals[v].kind {
                    VisualKind::Segments { x_offsets, .. } if x_offsets.is_empty() || x_offsets.len() == n => counts[v] = Some(n),
                    _ => return Err(bad()),
                }
            }
            "Glass" => {
                current(visual)?;
                expected = 3;
            }
            "Tint" => {
                let v = glass(visual)?;
                let t = parse_array(value).filter(|t| t.len() == 3).ok_or_else(bad)?;
                layout.visuals[v].glass_tint = [t[0], t[1], t[2]];
            }
            "Roughness" => {
                let v = glass(visual)?;
                layout.visuals[v].glass_roughness = value.parse().ok().ok_or_else(bad)?;
            }
            "Image" => {
                let v = glass(visual)?;
                layout.visuals[v].glass_image = Some(value.to_string());
            }
            "Area" => {
                let v = glass(visual)?;
                let a = parse_array(value).filter(|a| a.len() == 4).ok_or_else(bad)?;
                layout.visuals[v].glass_area = [a[0], a[1], a[2], a[3]];
            }
            "Ambient" => {
                let v = glass(visual)?;
                let a = parse_array(value).filter(|a| a.len() == 3).ok_or_else(bad)?;
                layout.visuals[v].glass_ambient = [inv_srgb(a[0]), inv_srgb(a[1]), inv_srgb(a[2])];
            }
            // other keys are passed over, as VPX passes them
            _ => {}
        }
    }
    for (v, count) in layout.visuals.iter_mut().zip(counts) {
        match &mut v.kind {
            VisualKind::Dmd { size, .. } | VisualKind::Screen { size } if size.0 < 0 || size.1 < 0 => return Err(format!("{name}: a display needs its Size")),
            VisualKind::Segments { x_offsets, .. } => {
                let n = count.unwrap_or(x_offsets.len());
                if n == 0 {
                    return Err(format!("{name}: a segment display needs XPos or NElements"));
                }
                // without places, the digits share the width evenly
                if x_offsets.is_empty() {
                    *x_offsets = (0..n).map(|i| v.rect[2] * i as f32 / n as f32).collect();
                }
            }
            _ => {}
        }
    }
    Ok(layout)
}

/// VPX's default layouts, as files built into the player.
const DEFAULT_FILES: &[(&str, &str)] = &[
    ("lay16x9-lcd.scv", include_str!("../assets/vpx/scoreview/lay16x9-lcd.scv")),
    ("lay192x256-crt.scv", include_str!("../assets/vpx/scoreview/lay192x256-crt.scv")),
    ("lay4x1-alpha2x16.scv", include_str!("../assets/vpx/scoreview/lay4x1-alpha2x16.scv")),
    ("lay4x1-alpha2x20.scv", include_str!("../assets/vpx/scoreview/lay4x1-alpha2x20.scv")),
    ("lay4x1-alpha4x6CB.scv", include_str!("../assets/vpx/scoreview/lay4x1-alpha4x6CB.scv")),
    ("lay4x1-alpha4x7.scv", include_str!("../assets/vpx/scoreview/lay4x1-alpha4x7.scv")),
    ("lay4x1-alpha4x7CB.scv", include_str!("../assets/vpx/scoreview/lay4x1-alpha4x7CB.scv")),
    ("lay4x1-dmd128x16.scv", include_str!("../assets/vpx/scoreview/lay4x1-dmd128x16.scv")),
    ("lay4x1-dmd128x32.scv", include_str!("../assets/vpx/scoreview/lay4x1-dmd128x32.scv")),
    ("lay4x1-dmd192x64.scv", include_str!("../assets/vpx/scoreview/lay4x1-dmd192x64.scv")),
];

/// VPX's default layouts, read once.
pub fn defaults() -> &'static [Layout] {
    static LAYOUTS: OnceLock<Vec<Layout>> = OnceLock::new();
    LAYOUTS.get_or_init(|| {
        DEFAULT_FILES
            .iter()
            .filter_map(|(name, text)| match parse(name, text) {
                Ok(l) => Some(l),
                Err(e) => {
                    eprintln!("score view: {e}");
                    None
                }
            })
            .collect()
    })
}

/// VPX's glass pictures, which its default layouts name (`plugins/scoreview/layouts`): copied
/// unchanged from VPX, their author unknown (`engine/player/assets/vpx/README.md`).
const GLASS_FILES: [(&str, &[u8]); 4] = [
    ("Glass-Cracked-1.webp", include_bytes!("../assets/vpx/scoreview/Glass-Cracked-1.webp")),
    ("Glass-Cracked-2.webp", include_bytes!("../assets/vpx/scoreview/Glass-Cracked-2.webp")),
    ("Glass-Dirty-1.webp", include_bytes!("../assets/vpx/scoreview/Glass-Dirty-1.webp")),
    ("Glass-Dirty-2.webp", include_bytes!("../assets/vpx/scoreview/Glass-Dirty-2.webp")),
];

/// A glass picture, decoded: sRGB colour, its alpha the glass's roughness.
pub struct GlassPicture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Every glass picture asked for, by where it was looked for, decoded once (VPX's `m_images`).
fn glass_store() -> &'static Mutex<Vec<(PathBuf, Option<Arc<GlassPicture>>)>> {
    static STORE: OnceLock<Mutex<Vec<(PathBuf, Option<Arc<GlassPicture>>)>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Vec::new()))
}

/// The glass picture a layout names (VPX's `LoadGlass`): a file beside the
/// layout's own file, or for VPX's defaults one of its pictures built in.
/// Its number in the store, None when it can't be read (VPX: missing, no glass).
pub fn glass_id(dir: Option<&Path>, image: &str) -> Option<u32> {
    let key = dir.map_or_else(|| PathBuf::from(image), |d| d.join(image));
    let mut store = glass_store().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(k) = store.iter().position(|(p, _)| *p == key) {
        return store[k].1.is_some().then_some(k as u32);
    }
    let bytes = match dir {
        Some(_) => std::fs::read(&key).map_err(|_| eprintln!("score view: missing glass file {}", key.display())).ok(),
        None => Path::new(image)
            .file_name()
            .and_then(|n| GLASS_FILES.iter().find(|(name, _)| n.eq_ignore_ascii_case(name)))
            .map(|(_, b)| b.to_vec()),
    };
    let picture = bytes.and_then(|b| image::load_from_memory(&b).ok()).map(|i| {
        let i = i.to_rgba8();
        Arc::new(GlassPicture { width: i.width(), height: i.height(), rgba: i.into_raw() })
    });
    let found = picture.is_some();
    store.push((key, picture));
    found.then_some((store.len() - 1) as u32)
}

/// A glass picture by its number in the store.
pub fn glass_picture(id: u32) -> Option<Arc<GlassPicture>> {
    glass_store().lock().unwrap_or_else(|e| e.into_inner()).get(id as usize).and_then(|(_, p)| p.clone())
}

/// A glass picture laid over a display, and the part of it over that display:
/// left, top, width and height as parts of the picture (VPX's `glassArea`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glass {
    pub picture: u32,
    pub area: [f32; 4],
}

/// A visual's glass (VPX's `ScoreView::Render`): its picture, and the
/// layout's area of it scaled to the picture, else all of it.
fn glass_of(layout: &Layout, v: &Visual) -> Option<Glass> {
    let picture = glass_id(layout.dir.as_deref(), v.glass_image.as_deref()?)?;
    let p = glass_picture(picture)?;
    let [x, y, w, h] = v.glass_area;
    let (pw, ph) = (p.width as f32, p.height as f32);
    let area = if w == 0.0 || h == 0.0 { [0.0, 0.0, 1.0, 1.0] } else { [x / pw, y / ph, w / pw, h / ph] };
    Some(Glass { picture, area })
}

/// VPX 5436's score view settings (`[Plugin.ScoreView]`): on unless `Enable = 0`, and a
/// folder of the user's own layouts (`LayoutFolder`), tried after the table's own, as VPX's
/// plugin tries it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScoreViewSettings {
    pub disabled: bool,
    pub layout_folder: Option<std::path::PathBuf>,
}

impl ScoreViewSettings {
    pub fn from_settings(s: &crate::settings::Settings) -> ScoreViewSettings {
        ScoreViewSettings {
            disabled: s.get("Plugin.ScoreView", "Enable").and_then(crate::settings::vpx_bool) == Some(false),
            layout_folder: s.get("Plugin.ScoreView", "LayoutFolder").map(str::trim).filter(|f| !f.is_empty()).map(std::path::PathBuf::from),
        }
    }
}

/// The layouts for a table as VPX 5436's score view plugin finds them (`ScoreViewPlugin.cpp`): a
/// `.scv` file beside the table under its name, else one named for its folder, else every layout in
/// the settings' own folder, else VPX's own; none with the plugin switched off, the score window
/// then showing only what a backglass file or a PinUP pack draws there.
pub fn for_table_with(table: &Path, settings: &ScoreViewSettings) -> Vec<Layout> {
    if settings.disabled {
        return Vec::new();
    }
    let read = |path: &Path| -> Option<Layout> {
        let text = std::fs::read_to_string(path).ok()?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        let mut layout = parse(&name, &text).map_err(|e| eprintln!("score view: {e}")).ok()?;
        layout.dir = path.parent().map(Path::to_path_buf);
        Some(layout)
    };
    let beside = table.with_extension("scv");
    let folder = table.parent().and_then(|dir| Some(dir.join(dir.file_name()?).with_extension("scv")));
    for path in std::iter::once(beside).chain(folder) {
        if let Some(layout) = read(&path) {
            return vec![layout];
        }
    }
    if let Some(dir) = &settings.layout_folder {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("scv"))).collect())
            .unwrap_or_default();
        files.sort();
        let layouts: Vec<Layout> = files.iter().filter_map(|p| read(p)).collect();
        if !layouts.is_empty() {
            return layouts;
        }
    }
    defaults().to_vec()
}

/// The layouts for a table with VPX's own settings (`for_table_with`).
pub fn for_table(table: &Path) -> Vec<Layout> {
    for_table_with(table, &ScoreViewSettings::default())
}

/// The displays a game shows now, as a layout's sources find them: the DMD
/// (its size, and whether it's a video screen), and each segment display's
/// digit count, in PinMAME's order.
pub struct Sources<'a> {
    pub dmd: Option<(u32, u32, bool)>,
    pub segments: &'a [usize],
}

/// A segment source a layout names (`ctrl://default/seg?id=N[&sub=M]`):
/// the display, and the one digit of it when `sub` names one.
fn segment_source(src: &str) -> Option<(usize, Option<usize>)> {
    let rest = src.strip_prefix("ctrl://default/seg")?;
    let query = rest.strip_prefix('?').unwrap_or("");
    let value = |key: &str| query.split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('=')?.parse::<i64>().ok());
    let id = value("id").unwrap_or(0);
    let sub = value("sub").filter(|&s| s >= 0);
    Some((usize::try_from(id).ok()?, sub.map(|s| s as usize)))
}

/// How many digits a layout's segment visual finds in the game's displays.
fn segment_count(src: &str, sources: &Sources) -> Option<usize> {
    let (id, sub) = segment_source(src)?;
    let n = *sources.segments.get(id)?;
    match sub {
        Some(s) if s < n => Some(1),
        Some(_) => None,
        None => Some(n),
    }
}

/// The layout VPX's score view would draw for these displays in a window of
/// `out` pixels (`ScoreView::Select`): the one missing none of its sources,
/// showing the most (a DMD counting for ten digits' displays), fitting the
/// window's shape best; None when every layout misses one.
pub fn select<'l>(layouts: &'l [Layout], sources: &Sources, out: (u32, u32)) -> Option<&'l Layout> {
    let out_ar = out.0.max(1) as f32 / out.1.max(1) as f32;
    let score = |l: &Layout| {
        let (mut matched, mut unmatched) = (0u32, 0u32);
        for v in &l.visuals {
            let ok = match &v.kind {
                VisualKind::Dmd { size, .. } => sources
                    .dmd
                    .filter(|_| v.src == "ctrl://default/display")
                    .is_some_and(|(w, h, video)| !video && w as i64 * size.1 as i64 == size.0 as i64 * h as i64),
                // a video screen isn't carried by our link
                VisualKind::Screen { .. } => false,
                VisualKind::Segments { x_offsets, .. } => segment_count(&v.src, sources) == Some(x_offsets.len()),
            };
            if !ok {
                unmatched += 1;
            } else if matches!(v.kind, VisualKind::Dmd { .. }) {
                matched += 10;
            } else {
                matched += 1;
            }
        }
        let layout_ar = l.width / l.height;
        let fit = if out_ar > layout_ar { layout_ar / out_ar } else { out_ar / layout_ar };
        (unmatched, matched, fit)
    };
    // the best: the fewest unmatched, then the most matched, then the best fit; the first of
    // equals, as VPX's max_element keeps it
    let mut best: Option<(&Layout, (u32, u32, f32))> = None;
    for l in layouts {
        let s = score(l);
        let better = best.is_none_or(|(_, b)| s.0 < b.0 || (s.0 == b.0 && (s.1 > b.1 || (s.1 == b.1 && s.2 > b.2))));
        if better {
            best = Some((l, s));
        }
    }
    best.filter(|(_, s)| s.0 == 0).map(|(l, _)| l)
}

/// The layout fitted inside `out` pixels keeping its shape, centred (VPX's
/// "Contain"): a function from the layout's units to the window's pixels.
pub fn fit(layout: &Layout, out: (u32, u32)) -> impl Fn([f32; 4]) -> [f32; 4] {
    let (ow, oh) = (out.0.max(1) as f32, out.1.max(1) as f32);
    let (out_ar, layout_ar) = (ow / oh, layout.width / layout.height);
    let (src_w, src_h, px, py) = if out_ar > layout_ar {
        let src_w = layout.height * out_ar;
        (src_w, layout.height, 0.5 * (src_w - layout.width), 0.0)
    } else {
        let src_h = layout.width / out_ar;
        (layout.width, src_h, 0.0, 0.5 * (src_h - layout.height))
    };
    move |[x, y, w, h]: [f32; 4]| [(px + x) / src_w * ow, (py + y) / src_h * oh, w / src_w * ow, h / src_h * oh]
}

/// A segment display's digit as a layout places it: in window pixels (left,
/// top, width, height), which display and digit it shows, and how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedDigit {
    pub rect: [f32; 4],
    pub display: usize,
    pub digit: usize,
    /// None: from the display's hardware.
    pub style: Option<SegStyle>,
    pub glass_tint: [f32; 3],
    pub roughness: f32,
    /// The room's light on the glass, linear and doubled as VPX doubles it (with a glass picture only).
    pub ambient: [f32; 3],
    /// The glass picture, and the part of it over this digit.
    pub glass: Option<Glass>,
}

/// Every digit a layout shows (VPX's `ScoreView::Render`): each segment
/// visual's digits at their places across, as tall as the visual and three
/// quarters as wide (VPX's pictures are 24 by 32).
pub fn digits(layout: &Layout, sources: &Sources, out: (u32, u32)) -> Vec<PlacedDigit> {
    let to_window = fit(layout, out);
    let mut list = Vec::new();
    for v in &layout.visuals {
        let VisualKind::Segments { x_offsets, style } = &v.kind else { continue };
        let Some((id, sub)) = segment_source(&v.src) else { continue };
        if segment_count(&v.src, sources) != Some(x_offsets.len()) {
            continue;
        }
        let [x, y, w, h] = v.rect;
        let element_w = h * 24.0 / 32.0;
        // the glass across the whole visual, each digit given its own part of it, as VPX cuts it
        let glass = glass_of(layout, v);
        for (i, &dx) in x_offsets.iter().enumerate() {
            list.push(PlacedDigit {
                rect: to_window([x + dx, y, element_w, h]),
                display: id,
                digit: sub.unwrap_or(i),
                style: *style,
                glass_tint: v.glass_tint,
                roughness: v.glass_roughness,
                ambient: v.glass_ambient.map(|a| a * 2.0),
                glass: glass.map(|g| {
                    let scale = g.area[2] / w;
                    Glass { picture: g.picture, area: [g.area[0] + dx * scale, g.area[1], element_w * scale, g.area[3]] }
                }),
            });
        }
    }
    list
}

/// How the display is drawn in its window: where, in pixels from the top
/// left (left, top, right, bottom), and its look, as the shader takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub rect: [f32; 4],
    pub look: Look,
}

/// Where a layout's DMD shows in a window of `out` pixels, and how it looks
/// (`dots` and `luminance` describe the frame; `source_tint` is the colour
/// its source asks a brightness-only frame be shown in, as FlexDMD hands its
/// DMD its `Color`).
pub fn dmd(layout: &Layout, dots: (u32, u32), luminance: bool, source_tint: Option<[u8; 4]>, out: (u32, u32)) -> Option<Placed> {
    let to_window = fit(layout, out);
    layout.visuals.iter().find_map(|v| {
        let VisualKind::Dmd { profile, .. } = v.kind else { return None };
        let [x, y, w, h] = to_window(v.rect);
        let ambient = v.glass_ambient;
        let p = with_source_tint(&profiles()[profile.min(PROFILES.len() - 1)], source_tint);
        let mut look = look(&p, luminance, dots, [1.0; 3], 1.0, v.glass_tint, v.glass_roughness, ambient);
        look.glass = glass_of(layout, v);
        Some(Placed { rect: [x, y, x + w, y + h], look })
    })
}

/// A profile whose lit dots take the source's own colour (alpha, red, green,
/// blue) when it gives one.
pub fn with_source_tint(p: &DotProfile, tint: Option<[u8; 4]>) -> DotProfile {
    let mut p = *p;
    if let Some([_, r, g, b]) = tint {
        p.tint = u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16);
    }
    p
}

/// The newer renderer's settings (VPX's `SetupDMDRender` and
/// `SetupDisplayRenderer`), all colours linear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// A lit dot's colour times its brightness.
    pub lit: [f32; 3],
    /// The glow between the dots.
    pub diffuse: f32,
    pub unlit: [f32; 3],
    /// The frame's size in dots.
    pub dots: [f32; 2],
    /// Where the dot's distance field crosses 0.5, at its border.
    pub sdf_offset: f32,
    /// How far inside its border a dot is fully lit.
    pub threshold: f32,
    pub glass_tint: [f32; 3],
    pub roughness: f32,
    /// The room's light on the glass, used with a glass picture only.
    pub ambient: [f32; 3],
    /// The glass picture over the display, and the part of it there.
    pub glass: Option<Glass>,
}

/// The newer renderer's settings for a profile: VPX's `SetupDMDRender` with
/// `tint` and `brightness` from the layout (white and 1 in the defaults), and
/// `SetupDisplayRenderer`'s glass with no padding, its picture none until
/// the layout's is set.
#[allow(clippy::too_many_arguments)]
pub fn look(p: &DotProfile, luminance: bool, dots: (u32, u32), tint: [f32; 3], brightness: f32, glass_tint: [f32; 3], roughness: f32, ambient: [f32; 3]) -> Look {
    let dot = rgb(p.tint);
    let full = brightness * p.brightness;
    // a coloured frame keeps its colours; a brightness-only one takes the profile's
    let color = if luminance { [tint[0] * dot[0], tint[1] * dot[1], tint[2] * dot[2]] } else { tint };
    Look {
        lit: color.map(|c| c * full),
        diffuse: p.glow,
        unlit: rgb(p.unlit),
        dots: [dots.0 as f32, dots.1 as f32],
        sdf_offset: 0.5 * (1.0 + (1.0 / (2.0 + 0.5)) * p.size / 2.0),
        threshold: 0.5 + 0.5 * (p.size * (1.0 - p.sharpness)),
        glass_tint,
        roughness,
        // VPX doubles the room's light on the glass
        ambient: ambient.map(|a| a * 2.0),
        glass: None,
    }
}

/// A colour as Windows keeps it, made linear.
fn rgb(c: u32) -> [f32; 3] {
    let channel = |shift: u32| crate::dmd::srgb_to_linear(((c >> shift) & 0xFF) as f32 / 255.0);
    [channel(0), channel(8), channel(16)]
}

/// The style of a segment visual's digit: the layout's, else from the
/// display's hardware; and the shape family, from the hardware only when the
/// style is (VPX's `ScoreView::Render`).
pub fn digit_style(style: Option<SegStyle>, hardware: u32) -> (SegStyle, SegHint) {
    match style {
        Some(s) => (s, SegHint::Generic),
        None => (crate::segments::style(hardware), crate::segments::hint(hardware)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VPX 5436's `[Plugin.ScoreView]`: switched off, no layouts; else the table's, VPX's own.
    #[test]
    fn settings_switch_and_folder() {
        let off = ScoreViewSettings::from_settings(&crate::settings::Settings::from_text("[Plugin.ScoreView]\nEnable = 0\n"));
        assert!(off.disabled);
        assert!(for_table_with(Path::new("C:\\nowhere\\table.vpx"), &off).is_empty());
        let folder = ScoreViewSettings::from_settings(&crate::settings::Settings::from_text("[Plugin.ScoreView]\nLayoutFolder = C:\\nowhere\n"));
        assert_eq!(folder.layout_folder, Some(std::path::PathBuf::from("C:\\nowhere")));
        // a folder that isn't there leaves VPX's own
        assert_eq!(for_table_with(Path::new("C:\\nowhere\\table.vpx"), &folder).len(), defaults().len());
    }

    fn layout(name: &str) -> &'static Layout {
        defaults().iter().find(|l| l.name == name).expect("a default layout")
    }

    #[test]
    fn vpxs_default_layouts_read_as_vpx_reads_them() {
        assert_eq!(defaults().len(), DEFAULT_FILES.len());
        let l = layout("lay4x1-alpha4x7CB.scv");
        assert_eq!((l.width, l.height, l.visuals.len()), (280.0, 70.0, 6));
        let v = &l.visuals[4];
        assert_eq!((v.rect, v.src.as_str()), ([112.7, 51.5, 26.25, 20.0], "ctrl://default/seg?id=4"));
        assert_eq!(v.kind, VisualKind::Segments { x_offsets: vec![0.0, 11.25], style: None });
        // the tint is commented out; the glass's ambient light is made linear
        assert_eq!((v.glass_tint, v.glass_roughness), ([1.0; 3], 0.1));
        assert!((v.glass_ambient[0] - inv_srgb(0.125)).abs() < 1e-6);
        assert_eq!(v.glass_image.as_deref(), Some("./Glass-Dirty-1.webp"));
        let d = layout("lay4x1-dmd128x32.scv");
        assert_eq!(d.visuals[0].kind, VisualKind::Dmd { size: (128, 32), profile: NEON_PLASMA });
        assert_eq!(d.visuals[0].src, "ctrl://default/display");
        // what VPX refuses, we refuse
        assert!(parse("bad", "Width: 0").is_err());
        assert!(parse("bad", "Visuals:\n\t- Image:\n").is_err());
        assert!(parse("bad", "Visuals:\n\t- DMD:\n\t\tRect: [0,0,1,1]\n").is_err()); // no Size
        assert!(parse("bad", "Visuals:\n\t- SegDisplay:\n\t\tXPos: [0, 1]\n\t\tNElements: 3\n").is_err());
        // places shared evenly when only the count is given
        let even = parse("even", "Visuals:\n\t- SegDisplay:\n\t\tRect: [0,0,40,10]\n\t\tSrc: ctrl://default/seg?id=1\n\t\tNElements: 4\n").expect("reads");
        assert_eq!(even.visuals[0].kind, VisualKind::Segments { x_offsets: vec![0.0, 10.0, 20.0, 30.0], style: None });
    }

    #[test]
    fn each_game_gets_the_layout_vpx_gives_it() {
        let wide = (1024, 256);
        let pick = |dmd, segments: &[usize]| select(defaults(), &Sources { dmd, segments }, wide).map(|l| l.name.as_str());
        // Black Knight: four scores of seven digits, ball and credit of two
        assert_eq!(pick(None, &[7, 7, 7, 7, 2, 2]), Some("lay4x1-alpha4x7CB.scv"));
        // Bally's six-digit scores; WPC's two lines of sixteen
        assert_eq!(pick(None, &[6, 6, 6, 6, 2, 2]), Some("lay4x1-alpha4x6CB.scv"));
        assert_eq!(pick(None, &[16, 16]), Some("lay4x1-alpha2x16.scv"));
        // a 128x32 DMD, or one of its shape; a shape no layout has
        assert_eq!(pick(Some((128, 32, false)), &[]), Some("lay4x1-dmd128x32.scv"));
        assert_eq!(pick(Some((256, 64, false)), &[]), Some("lay4x1-dmd128x32.scv"));
        assert_eq!(pick(Some((128, 64, false)), &[]), None);
        // a layout needs only its own displays: a fifth the game has goes unshown, as in VPX
        assert_eq!(pick(None, &[7, 7, 7, 7, 4]), Some("lay4x1-alpha4x7.scv"));
        // displays no layout has: none (the window then takes PinMAME's own places)
        assert_eq!(pick(None, &[8, 8, 8]), None);
        // the digits of Black Knight's ball and credit displays, where the layout puts them
        let l = layout("lay4x1-alpha4x7CB.scv");
        let d = digits(l, &Sources { dmd: None, segments: &[7, 7, 7, 7, 2, 2] }, (280, 70));
        assert_eq!(d.len(), 32);
        assert_eq!((d[28].display, d[28].digit), (4, 0));
        let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);
        assert!(near(d[28].rect, [112.7, 51.5, 15.0, 20.0]), "{:?}", d[28].rect);
        assert!(near(d[29].rect, [112.7 + 11.25, 51.5, 15.0, 20.0]), "{:?}", d[29].rect);
    }

    #[test]
    fn a_layout_fits_its_window_keeping_its_shape() {
        let l = layout("lay4x1-dmd128x32.scv");
        // a 4:1 display in a 4:1 window fills it; wider, bars left and right; taller, above and below
        assert_eq!(dmd(l, (128, 32), true, None, (1024, 256)).expect("placed").rect, [0.0, 0.0, 1024.0, 256.0]);
        assert_eq!(dmd(l, (128, 32), true, None, (1600, 256)).expect("placed").rect, [288.0, 0.0, 1312.0, 256.0]);
        assert_eq!(dmd(l, (128, 32), true, None, (1024, 512)).expect("placed").rect, [0.0, 128.0, 1024.0, 384.0]);
    }

    #[test]
    fn the_default_dmd_shows_through_vpxs_dirty_glass() {
        let l = layout("lay4x1-dmd128x32.scv");
        let placed = dmd(l, (128, 32), true, None, (1024, 256)).expect("the DMD placed");
        let g = placed.look.glass.expect("a glass picture");
        let p = glass_picture(g.picture).expect("VPX's picture, built in and decoded");
        assert_eq!((p.width, p.height), (1024, 512));
        // the layout's area, [0,0,1024,256], as parts of the picture
        assert_eq!(g.area, [0.0, 0.0, 1.0, 0.5]);
    }

    #[test]
    fn neon_plasma_dots_are_orange_and_bright() {
        let p = dmd(layout("lay4x1-dmd128x32.scv"), (128, 32), true, None, (1024, 256)).expect("placed");
        // VPX's default tint, 0x2D52FF: red 255, green 82, blue 45, made linear, times 25
        assert!((p.look.lit[0] - 25.0).abs() < 1e-4);
        assert!((p.look.lit[1] - 25.0 * 0.084_376).abs() < 1e-3);
        assert!((p.look.lit[2] - 25.0 * 0.026_241).abs() < 1e-3);
        // unlit dots a dim grey (0x40 made linear)
        assert!((p.look.unlit[0] - 0.051_269).abs() < 1e-5);
        // dot size 0.6, sharpness 0.9; the layout's glass roughness
        assert!((p.look.sdf_offset - 0.56).abs() < 1e-6);
        assert!((p.look.threshold - 0.53).abs() < 1e-6);
        assert_eq!(p.look.roughness, 0.1);
        // a coloured frame keeps its own colours, as bright
        let p = dmd(layout("lay4x1-dmd128x32.scv"), (128, 32), false, None, (1024, 256)).expect("placed");
        assert_eq!(p.look.lit, [25.0; 3]);
    }
}
