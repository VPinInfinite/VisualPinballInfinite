// Follows libpinmame (license:BSD-3-Clause, the PinMAME project): how it splits a game's segment displays into digits,
// tells each digit's kind and groups the digits into displays, with each display's hardware (SetupMsgApiSegDisplays
// and GetSegDisplay, src/libpinmame/libpinmame.cpp), and PinMAME's layout of them for a display device
// (layoutAlphanumericFrame, src/wpc/core.c); and Visual Pinball X (license:GPLv3+), build 5436 (10.8.1
// pre-release, commit af26b2d): the segment display profiles' default settings (src/core/Settings_properties.inl), how
// its score view picks a display's style and shape family (plugins/scoreview/ScoreView.cpp), and how the segment
// renderer is set up for a profile and picks a digit's picture (Renderer::SetupSegmentRenderer,
// src/renderer/Renderer.cpp), and the segment displays its FlexDMD plugin offers in FlexDMD's segment modes
// (onGetSegSrc and GetState, plugins/flexdmd/FlexDMDPlugin.cpp). The pictures are VPX's own
// (engine/player/assets/vpx/segments).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is GPL-3.0-or-later.

//! The machine's segment displays: the scores of older
//! machines, lit segment by segment rather than dot by dot.
//!
//! PinMAME describes a game's displays as it draws them in its own window —
//! a row of digits of one type at a place — and hands each one's digits as
//! they are drawn, a bit a segment. Here they become what libpinmame tells
//! VPX: digits of VPX's kinds (seven segments, seven and a comma, fourteen,
//! ...) grouped into displays, each a run of digits side by side on one
//! line, with the hardware that sets their colour. VPX's layouts place
//! them in the score display's window when one fits the game's displays;
//! for other games each digit keeps the place PinMAME's own window gives it
//!.

use std::sync::Arc;

use pinmame_link::DisplayLayout;

/// What segments a digit has: VPX's element layouts (`CTLPI_SEG_LAYOUT_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SegKind {
    Seven,
    SevenComma,
    SevenDot,
    Nine,
    NineComma,
    Fourteen,
    FourteenDot,
    FourteenDotComma,
    /// Sixteen, the top and bottom bars split.
    Sixteen,
}

impl SegKind {
    pub const ALL: [SegKind; 9] = [
        SegKind::Seven,
        SegKind::SevenComma,
        SegKind::SevenDot,
        SegKind::Nine,
        SegKind::NineComma,
        SegKind::Fourteen,
        SegKind::FourteenDot,
        SegKind::FourteenDotComma,
        SegKind::Sixteen,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

// PinMAME's display types (`CORE_SEG*`, core.h), their flags taken off
const SEG16: u32 = 0;
const SEG16R: u32 = 1;
const SEG10: u32 = 2;
const SEG9: u32 = 3;
const SEG8: u32 = 4;
const SEG8D: u32 = 5;
const SEG7: u32 = 6;
const SEG87: u32 = 7;
const SEG87F: u32 = 8;
const SEG98: u32 = 9;
const SEG98F: u32 = 10;
const SEG7S: u32 = 11;
const SEG7SC: u32 = 12;
const SEG16S: u32 = 13;
const SEG16N: u32 = 16;
const SEG16D: u32 = 17;
/// `CORE_SEGALL`: the type without its flags.
const TYPE_ALL: u32 = 0x1f;

/// A digit's kind (libpinmame's `SetupMsgApiSegDisplays`): from its
/// display's type, and on displays with a comma every three digits, from its
/// place; None for a type that isn't a segment display.
fn kind(display_type: u32, length: i32, j: i32) -> Option<SegKind> {
    let thousands = length - 1 - j > 0 && (length - 1 - j) % 3 == 0;
    Some(match display_type & TYPE_ALL {
        SEG7 | SEG7S | SEG7SC => SegKind::Seven,
        SEG87 | SEG87F if thousands => SegKind::SevenComma,
        SEG87 | SEG87F => SegKind::Seven,
        SEG8 => SegKind::SevenComma,
        SEG8D => SegKind::SevenDot,
        SEG9 => SegKind::Nine,
        SEG10 => SegKind::NineComma,
        SEG98 | SEG98F if thousands => SegKind::NineComma,
        SEG98 | SEG98F => SegKind::Nine,
        SEG16N => SegKind::Fourteen,
        SEG16D => SegKind::FourteenDot,
        SEG16 | SEG16R => SegKind::FourteenDotComma,
        SEG16S => SegKind::Sixteen,
        _ => return None,
    })
}

/// The size PinMAME's own window draws a digit of a type at, in its pixels:
/// columns and rows (small digits 11 by 12, the rest 15 by 20).
fn glyph_size(display_type: u32) -> (i32, i32) {
    match display_type & TYPE_ALL {
        SEG7S | SEG7SC => (11, 12),
        _ => (15, 20),
    }
}

// PinMAME's hardware generations that set a display's hardware (`GEN_*`, gen.h)
const GEN_WPCALPHA_1: u64 = 0x1;
const GEN_WPCALPHA_2: u64 = 0x2;
const GEN_S11X: u64 = 0x100;
const GEN_S11B2: u64 = 0x200;
const GEN_S11C: u64 = 0x400;
const GEN_S9: u64 = 0x800;
const GEN_DE: u64 = 0x1000;
const GEN_S7: u64 = 0x1_0000;
const GEN_S6: u64 = 0x2_0000;
const GEN_S4: u64 = 0x4_0000;
const GEN_S3C: u64 = 0x8_0000;
const GEN_S3: u64 = 0x10_0000;
const GEN_BY17: u64 = 0x20_0000;
const GEN_BY35: u64 = 0x40_0000;
const GEN_STMPU100: u64 = 0x80_0000;
const GEN_STMPU200: u64 = 0x100_0000;
const GEN_BY6803: u64 = 0x1000_0000;
const GEN_BY6803A: u64 = 0x2000_0000;
const GEN_S11: u64 = 0x8000_0000;
const GEN_GTS1: u64 = 0x1_0000_0000;
const GEN_GTS80: u64 = 0x2_0000_0000;
const GEN_GTS80B: u64 = 0x4_0000_0000;
const GEN_GTS3: u64 = 0x200_0000_0000;

// VPX's hardware hints for a segment display (`CTLPI_SEG_HARDWARE_*`)
pub const HARDWARE_UNKNOWN: u32 = 0;
pub const HARDWARE_NEON_PLASMA: u32 = 0x0001_0000;
pub const HARDWARE_VFD_GREEN: u32 = 0x0002_0000;
pub const HARDWARE_VFD_BLUE: u32 = 0x0003_0000;
pub const HARDWARE_GTS1_4DIGIT: u32 = 0x0003_0001;
pub const HARDWARE_GTS1_6DIGIT: u32 = 0x0003_0002;
pub const HARDWARE_GTS80A_7DIGIT: u32 = 0x0003_0003;
pub const HARDWARE_GTS80B_20DIGIT: u32 = 0x0003_0004;
const HARDWARE_FAMILY: u32 = 0xFFFF_0000;

/// A display's hardware, by the game's generation and its number of digits.
fn hardware(generation: u64, digits: usize) -> u32 {
    match generation {
        GEN_BY17 | GEN_BY35 | GEN_BY6803 | GEN_BY6803A => HARDWARE_NEON_PLASMA,
        GEN_S3 | GEN_S3C | GEN_S4 | GEN_S6 | GEN_S7 | GEN_S9 | GEN_S11 | GEN_S11X | GEN_S11B2 | GEN_S11C | GEN_DE => HARDWARE_NEON_PLASMA,
        GEN_WPCALPHA_1 | GEN_WPCALPHA_2 | GEN_STMPU100 | GEN_STMPU200 => HARDWARE_NEON_PLASMA,
        // ball and credit, reported as two, are a four-digit display with a space in it
        GEN_GTS1 | GEN_GTS80 | GEN_GTS80B | GEN_GTS3 => match digits {
            2 | 4 => HARDWARE_GTS1_4DIGIT,
            6 => HARDWARE_GTS1_6DIGIT,
            7 => HARDWARE_GTS80A_7DIGIT,
            20 => HARDWARE_GTS80B_20DIGIT,
            _ => HARDWARE_UNKNOWN,
        },
        _ => HARDWARE_UNKNOWN,
    }
}

/// Whose digits a display has, for their shapes: VPX's shape families
/// (`VPXSegDisplayHint`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegHint {
    Generic,
    Gottlieb,
    Williams,
    Bally,
    Atari,
}

/// The shape family VPX's score view and backglass give a display of this
/// hardware: Gottlieb's for its own displays, else the generic one.
pub fn hint(hardware: u32) -> SegHint {
    match hardware {
        HARDWARE_GTS1_4DIGIT | HARDWARE_GTS1_6DIGIT | HARDWARE_GTS80A_7DIGIT | HARDWARE_GTS80B_20DIGIT => SegHint::Gottlieb,
        _ => SegHint::Generic,
    }
}

/// The picture a digit is drawn from (VPX's `SetupSegmentRenderer`): by its
/// kind, and for some kinds its maker; the generic family takes Williams's.
pub fn picture_name(kind: SegKind, hint: SegHint) -> &'static str {
    match (kind, hint) {
        (SegKind::Seven, SegHint::Gottlieb) => "7seg-gts",
        (SegKind::Seven, SegHint::Bally) => "7seg-bally",
        (SegKind::Seven, SegHint::Atari) => "7seg-atari",
        (SegKind::Seven, _) => "7seg-williams",
        (SegKind::SevenComma, SegHint::Bally) => "7seg-c-bally",
        (SegKind::SevenComma, SegHint::Atari) => "7seg-c-atari",
        (SegKind::SevenComma, _) => "7seg-c-williams",
        // VPX found no dot-only seven-segment display to copy, so takes the comma one
        (SegKind::SevenDot, _) => "7seg-c-williams",
        (SegKind::Nine, _) => "9seg-gts",
        (SegKind::NineComma, _) => "9seg-c-gts",
        (SegKind::Fourteen, _) => "14seg-williams",
        (SegKind::FourteenDot, _) => "14seg-d-williams",
        (SegKind::FourteenDotComma, SegHint::Gottlieb) => "14seg-dc-gts",
        (SegKind::FourteenDotComma, _) => "14seg-dc-williams",
        (SegKind::Sixteen, _) => "16seg",
    }
}

/// VPX's pictures, built into the player: each a PNG of four tiles of
/// 96 × 128, a segment's distance field in each colour channel.
const PICTURES: &[(&str, &[u8])] = &[
    ("7seg-williams", include_bytes!("../assets/vpx/segments/7seg-williams.png")),
    ("7seg-bally", include_bytes!("../assets/vpx/segments/7seg-bally.png")),
    ("7seg-atari", include_bytes!("../assets/vpx/segments/7seg-atari.png")),
    ("7seg-gts", include_bytes!("../assets/vpx/segments/7seg-gts.png")),
    ("7seg-c-williams", include_bytes!("../assets/vpx/segments/7seg-c-williams.png")),
    ("7seg-c-bally", include_bytes!("../assets/vpx/segments/7seg-c-bally.png")),
    ("7seg-c-atari", include_bytes!("../assets/vpx/segments/7seg-c-atari.png")),
    ("9seg-gts", include_bytes!("../assets/vpx/segments/9seg-gts.png")),
    ("9seg-c-gts", include_bytes!("../assets/vpx/segments/9seg-c-gts.png")),
    ("14seg-williams", include_bytes!("../assets/vpx/segments/14seg-williams.png")),
    ("14seg-d-williams", include_bytes!("../assets/vpx/segments/14seg-d-williams.png")),
    ("14seg-dc-williams", include_bytes!("../assets/vpx/segments/14seg-dc-williams.png")),
    ("14seg-dc-gts", include_bytes!("../assets/vpx/segments/14seg-dc-gts.png")),
    ("16seg", include_bytes!("../assets/vpx/segments/16seg.png")),
];

/// A picture's place in [`PICTURES`], for keeping its texture.
pub fn picture_index(name: &str) -> Option<usize> {
    PICTURES.iter().position(|(n, _)| *n == name)
}

pub const PICTURE_COUNT: usize = 14;

/// A picture decoded: width, height, and RGBA bytes.
pub fn picture(index: usize) -> Result<(u32, u32, Vec<u8>), String> {
    let (name, bytes) = PICTURES.get(index).ok_or("no such segment picture")?;
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|e| format!("{name}.png: {e}"))?.to_rgba8();
    Ok((image.width(), image.height(), image.into_raw()))
}

/// Where VPX's Williams digit sits in its picture's 24 × 32 cell (measured):
/// its segments from 4.4 to 21.4 down, centred 10.25 across, leaving the
/// comma room on the right.
const DIGIT_TOP: f32 = 4.4;
const DIGIT_BOTTOM: f32 = 21.4;
const DIGIT_CENTRE: f32 = 10.25;

/// The cell a digit is drawn in, for segments that should fill a box of
/// `width` × `height` (PinMAME's digit): the cell's left and top from the
/// box's, and its width and height. The cell keeps VPX's shape (24 by 32);
/// the segments fill the box's height, centred across it.
pub fn cell(width: f32, height: f32) -> [f32; 4] {
    let h = height * 32.0 / (DIGIT_BOTTOM - DIGIT_TOP);
    let w = 0.75 * h;
    [0.5 * width - DIGIT_CENTRE / 24.0 * w, -DIGIT_TOP / 32.0 * h, w, h]
}

/// How a display's segments glow: VPX's segment styles, whose numbers are
/// also its profiles' (`VPXSegDisplayRenderStyle`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegStyle {
    Plasma = 0,
    BlueVfd = 1,
    GreenVfd = 2,
    RedLed = 3,
    GreenLed = 4,
    YellowLed = 5,
    GenericPlasma = 6,
    GenericLed = 7,
}

/// The style VPX's score view and backglass give a display of this
/// hardware: neon plasma unless the hardware says otherwise.
pub fn style(hardware: u32) -> SegStyle {
    match hardware & HARDWARE_FAMILY {
        HARDWARE_VFD_GREEN => SegStyle::GreenVfd,
        HARDWARE_VFD_BLUE => SegStyle::BlueVfd,
        _ => SegStyle::Plasma,
    }
}

/// The glass over the digits: a little rough, so their light glows between
/// the segments, as on VPX's default layouts (0.1 to 0.15) and its
/// backglass's digits (0.15).
pub const GLASS_ROUGHNESS: f32 = 0.15;

/// One of VPX's segment display profiles: how its segments look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegProfile {
    /// A lit segment's colour, as Windows keeps colours (red in the lowest byte), sRGB.
    pub color: u32,
    /// An unlit segment's colour, likewise.
    pub unlit: u32,
    pub brightness: f32,
    /// The glow of the segments on the back of the display, against the glass.
    pub glow: f32,
}

const fn profile(color: u32) -> SegProfile {
    SegProfile { color, unlit: 0x0040_4040, brightness: 5.0, glow: 1.0 }
}

/// VPX's eight segment profiles with build 5436's default settings, in its
/// styles' order: neon plasma, blue and green VFD, red, green and yellow
/// LED, generic plasma and LED.
pub const PROFILES: [SegProfile; 8] = [
    profile(0x001E_96FF),
    profile(0x00FF_EF3F),
    profile(0x00EC_FF48),
    profile(0x0015_23FF),
    profile(0x0023_FF15),
    profile(0x0023_FFFF),
    profile(0x00FF_FFFF),
    profile(0x00FF_FFFF),
];

/// The profiles as the settings give them (`[Alpha]` `Profile1…` to
/// `Profile8…`, `vpx_settings::segment_profiles`), set once as play starts.
static PROFILE_SETTINGS: std::sync::OnceLock<[SegProfile; 8]> = std::sync::OnceLock::new();

/// Sets the profiles from the settings, once; later calls change nothing.
pub fn set_profiles(profiles: [SegProfile; 8]) {
    let _ = PROFILE_SETTINGS.set(profiles);
}

/// VPX's eight segment profiles: the settings' once set, else build 5436's defaults.
pub fn profiles() -> &'static [SegProfile; 8] {
    PROFILE_SETTINGS.get().unwrap_or(&PROFILES)
}

/// The segment renderer's colours for a style (VPX's `SetupSegmentRenderer`),
/// all linear: a lit segment's colour times its brightness, the glow, and an
/// unlit segment's colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegLook {
    pub lit: [f32; 3],
    pub glow: f32,
    pub unlit: [f32; 3],
}

/// The look of a style, tinted and as bright as the drawing asks (white and
/// 1 for the score view and the backglass).
pub fn look(style: SegStyle, tint: [f32; 3], brightness: f32) -> SegLook {
    let p = &profiles()[style as usize];
    let color = rgb(p.color);
    let full = brightness * p.brightness;
    SegLook { lit: [tint[0] * color[0] * full, tint[1] * color[1] * full, tint[2] * color[2] * full], glow: p.glow, unlit: rgb(p.unlit) }
}

/// A colour as Windows keeps it, made linear.
fn rgb(c: u32) -> [f32; 3] {
    let channel = |shift: u32| crate::dmd::srgb_to_linear(((c >> shift) & 0xFF) as f32 / 255.0);
    [channel(0), channel(8), channel(16)]
}

/// One digit: which display on the link holds it and where in its frame,
/// its kind, and where PinMAME's own window draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Digit {
    pub display: usize,
    pub index: usize,
    pub kind: SegKind,
    /// PinMAME's type with comma and period swapped (`CORE_SEG16R`).
    pub swapped_marks: bool,
    /// The digit's segments in PinMAME's window, in its pixels: left, top,
    /// width, height.
    pub glyph: [f32; 4],
}

/// A display: digits side by side on one line.
#[derive(Clone, Debug, PartialEq)]
pub struct SegDisplay {
    pub digits: Vec<Digit>,
    /// VPX's hardware hint (`HARDWARE_*`).
    pub hardware: u32,
}

/// Every segment display of a game, and the space PinMAME's window gives
/// them all (left, top, right, bottom, in its pixels).
#[derive(Clone, Debug, PartialEq)]
pub struct SegLayout {
    pub displays: Vec<SegDisplay>,
    pub bounds: [f32; 4],
}

impl SegLayout {
    /// Every digit, display after display.
    pub fn digits(&self) -> impl Iterator<Item = (&SegDisplay, &Digit)> {
        self.displays.iter().flat_map(|d| d.digits.iter().map(move |g| (d, g)))
    }
}

/// The game's segment displays, from PinMAME's description of its displays
/// (in its order), as libpinmame groups them: each display split into its
/// digits, and the digits joined into one display while they follow each
/// other on the same line. None for a game without segment displays.
pub fn layout(displays: &[(usize, DisplayLayout)], generation: u64) -> Option<SegLayout> {
    // each digit, with its line and place on it (in half digits)
    struct Placed {
        digit: Digit,
        top: i32,
        place: i32,
    }
    let mut placed = Vec::new();
    for &(display, l) in displays {
        if !l.is_segments() {
            continue;
        }
        let (cols, rows) = glyph_size(l.kind);
        for j in 0..l.length.max(0) {
            let Some(kind) = kind(l.kind, l.length, j) else { break };
            // PinMAME's window: a place is half a digit and its gap, a line half a large digit's height
            let (x, y) = (l.left * (cols + 1) / 2 + j * (cols + 1), l.top * 21 / 2);
            placed.push(Placed {
                digit: Digit { display, index: j as usize, kind, swapped_marks: l.kind & TYPE_ALL == SEG16R, glyph: [x as f32, y as f32, cols as f32, rows as f32] },
                top: l.top,
                place: l.left + j * 2,
            });
        }
    }
    if placed.is_empty() {
        return None;
    }
    let mut out: Vec<SegDisplay> = Vec::new();
    let mut run: Vec<Digit> = Vec::new();
    for (i, p) in placed.iter().enumerate() {
        run.push(p.digit.clone());
        let next = placed.get(i + 1);
        if next.is_none_or(|n| n.top != p.top || n.place != p.place + 2) {
            let digits = std::mem::take(&mut run);
            out.push(SegDisplay { hardware: hardware(generation, digits.len()), digits });
        }
    }
    let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for p in &placed {
        let [x, y, w, h] = p.digit.glyph;
        bounds = [bounds[0].min(x), bounds[1].min(y), bounds[2].max(x + w + 1.0), bounds[3].max(y + h + 1.0)];
    }
    Some(SegLayout { displays: out, bounds })
}

/// A digit's segments as levels from 0 to 1 (VPX's luminances), from its
/// bits as PinMAME draws them; comma and period swapped back where the type
/// swaps them (libpinmame's `GetSegDisplay`).
pub fn levels(bits: u16, swapped_marks: bool) -> [f32; 16] {
    let mut out = [0.0; 16];
    for (j, level) in out.iter_mut().enumerate() {
        *level = f32::from((bits >> j) & 1);
    }
    if swapped_marks {
        out.swap(7, 15);
    }
    out
}

/// The figure a digit shows, as VPX's backglass reads it for scores drawn
/// from pictures (`B2SRenderer::RenderScores`): 0 to 9 from the segments
/// lit (a comma aside), else -1.
pub fn figure(levels: &[f32; 16]) -> i32 {
    let mut bits = 0u32;
    for (j, &l) in levels.iter().enumerate() {
        if l > 0.5 {
            bits |= 1 << j;
        }
    }
    match bits & !0x80 {
        0x003F => 0,
        0x0006 | 0x0300 => 1,
        0x005B => 2,
        0x004F => 3,
        0x0066 => 4,
        0x006D => 5,
        0x007D | 0x007C => 6,
        0x0007 => 7,
        0x007F => 8,
        0x006F | 0x0067 => 9,
        _ => -1,
    }
}

/// The segment displays as they are now: the layout, every digit's levels
/// in its order, and a number that changes with them.
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentsNow {
    pub layout: Arc<SegLayout>,
    pub levels: Vec<[f32; 16]>,
    pub id: u64,
}

/// The segment displays this frame shows, when the running machine has any
/// (read again only when one of its displays has a new frame), else a
/// FlexDMD's in a segment mode while it shows.
pub fn shown(w: &mut crate::world::World) -> Option<Arc<SegmentsNow>> {
    let Some((layout, frames)) = w.controller.as_mut().and_then(|c| c.state.machine.as_mut()).and_then(|m| m.segments()) else {
        // no machine, or none with segment displays: nothing to show, nor for the backglass
        w.machine_segments = None;
        return flex_shown(w);
    };
    let machine = w.controller.as_mut()?.state.machine.as_mut()?;
    if let Some((n, s)) = &w.machine_segments
        && *n == frames
        && Arc::ptr_eq(&s.layout, &layout)
    {
        return Some(s.clone());
    }
    let mut levels = Vec::new();
    let mut frame = Vec::new();
    let mut held: Option<usize> = None;
    for (_, digit) in layout.digits() {
        if held != Some(digit.display) {
            frame.clear();
            machine.display_frame(digit.display, &mut frame);
            held = Some(digit.display);
        }
        let bits = frame.get(2 * digit.index..2 * digit.index + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
        levels.push(self::levels(bits, digit.swapped_marks));
    }
    w.segment_frames += 1;
    let now = Arc::new(SegmentsNow { layout, levels, id: w.segment_frames });
    w.machine_segments = Some((frames, now.clone()));
    Some(now)
}

/// AlphaDMD's layout for the game's segment displays, by how many digits each has
/// (`OnSegSrcChanged`'s table, in its order). Its last row, Black Hole's (6, 6, 6, 6, 2, 2, 6),
/// lies past the end of the loop that reads the table, so VPX 5436 never matches it, nor does
/// this. Logged once, as AlphaDMD logs its match.
pub fn alpha_layout(now: &SegmentsNow) -> Option<&'static str> {
    const KNOWN: [(&str, &[usize]); 11] = [
        ("4x6_2x2", &[6, 6, 6, 6, 2, 2]),
        ("4x7", &[7, 7, 7, 7]),
        ("4x7_2x2", &[7, 7, 7, 7, 2, 2]),
        ("6x4_2x2", &[4, 4, 4, 4, 4, 4, 2, 2]),
        ("2x16_1x7", &[16, 16, 7]),
        ("2x16", &[16, 16]),
        ("2x20", &[20, 20]),
        ("2x7_2x2_1x16", &[7, 7, 2, 2, 16]),
        ("1x7_2x16", &[7, 16, 16]),
        ("1x7_1x4_2x16", &[7, 4, 16, 16]),
        ("4x7_5x2", &[7, 7, 7, 7, 2, 2, 2, 2, 2]),
    ];
    let counts: Vec<usize> = now.layout.displays.iter().map(|d| d.digits.len()).collect();
    let found = KNOWN.iter().find(|(_, c)| *c == counts.as_slice()).map(|k| k.0);
    match found {
        Some(name) => crate::script::log::once(&format!("AlphaDMD: the segment displays ({counts:?} digits) match its layout {name}")),
        None => crate::script::log::once(&format!("AlphaDMD: the segment displays ({counts:?} digits) match none of its layouts")),
    }
    found
}

/// PinMAME's layout of a game's segment displays for a display device (`layoutAlphanumericFrame` in
/// src/wpc/core.c, BSD-3-Clause, from the PinMAME commit VPX 5436 builds): by the machine's
/// hardware generation, how many displays it has and the first one's digits, and a few games by
/// name — the number VPinMAME hands `Render_PM_Alphanumeric_Frame` (`core_segOverallLayout_t`,
/// 0 for none).
pub fn pinmame_layout(generation: u64, counts: &[usize], rom: &str) -> u32 {
    // PinMAME's generations besides those above (src/wpc/gen.h)
    const GEN_HNK: u64 = 0x400_0000;
    const GEN_BYPROTO: u64 = 0x800_0000;
    // its layouts (src/wpc/core.h's `core_segOverallLayout_t`, the device header's `layout_t`)
    const ALPHA_2X16: u32 = 1;
    const ALPHA_2X20: u32 = 2;
    const ALPHA_2X7_NUM_2X7: u32 = 3;
    const ALPHA_2X7_NUM_2X7_NUM_4X1: u32 = 4;
    const NUM_2X7_NUM_2X7_NUM_4X1: u32 = 5;
    const NUM_2X7_NUM_2X7_NUM_10X1: u32 = 6;
    const NUM_2X7_NUM_2X7_NUM_4X1_GEN7: u32 = 7;
    const NUM10_2X7_NUM10_2X7_NUM_4X1: u32 = 8;
    const NUM_2X6_NUM_2X6_NUM_4X1: u32 = 9;
    const NUM10_2X6_NUM10_2X6_NUM_4X1: u32 = 10;
    const NUM10_4X7: u32 = 11;
    const NUM_6X4_NUM_4X1: u32 = 12;
    const NUM_2X7_NUM_4X1_ALPHA_1X16: u32 = 13;
    const ALPHA_1X16_NUM_1X16_NUM_1X7: u32 = 14;
    const NUM_1X7_ALPHA_1X16_NUM_1X16: u32 = 15;
    const ALPHA_1X16_NUM_1X16_NUM_1X7_NUM_1X4: u32 = 16;
    let named = |prefix: &str| rom.len() >= prefix.len() && rom[..prefix.len()].eq_ignore_ascii_case(prefix);
    let (total, first) = (counts.len(), counts.first().copied().unwrap_or(0));
    match generation {
        GEN_S3 | GEN_S3C | GEN_S4 | GEN_S6 => {
            // Sys6A games with 7-digit displays
            if named("algar") || named("alpok") || named("frpwr_b6") || named("frpwr_c6") { NUM_2X7_NUM_2X7_NUM_4X1_GEN7 } else { NUM_2X6_NUM_2X6_NUM_4X1 }
        }
        GEN_S7 => {
            if named("hypbl") { NUM_2X7_NUM_4X1_ALPHA_1X16 } else { NUM_2X7_NUM_2X7_NUM_4X1_GEN7 }
        }
        GEN_S9 => NUM10_2X7_NUM10_2X7_NUM_4X1,
        GEN_WPCALPHA_1 | GEN_WPCALPHA_2 | GEN_S11C | GEN_S11B2 => {
            if named("rvrbt") {
                ALPHA_1X16_NUM_1X16_NUM_1X7_NUM_1X4
            } else if named("polic") {
                NUM_1X7_ALPHA_1X16_NUM_1X16
            } else {
                ALPHA_2X16
            }
        }
        GEN_S11 => NUM_6X4_NUM_4X1,
        GEN_S11X => {
            let layout = match total {
                2 => ALPHA_2X16,
                3 => ALPHA_1X16_NUM_1X16_NUM_1X7,
                4 => ALPHA_2X7_NUM_2X7,
                8 => ALPHA_2X7_NUM_2X7_NUM_4X1,
                _ => 0,
            };
            if named("polic") { NUM_1X7_ALPHA_1X16_NUM_1X16 } else { layout }
        }
        GEN_DE => match total {
            2 => ALPHA_2X16,
            4 => ALPHA_2X7_NUM_2X7,
            8 => ALPHA_2X7_NUM_2X7_NUM_4X1,
            _ => 0,
        },
        GEN_GTS1 | GEN_GTS80 => match first {
            6 => NUM10_2X6_NUM10_2X6_NUM_4X1,
            7 => NUM10_2X7_NUM10_2X7_NUM_4X1,
            _ => 0,
        },
        GEN_GTS80B | GEN_GTS3 => ALPHA_2X20,
        GEN_STMPU100 | GEN_STMPU200 => match first {
            6 => NUM_2X6_NUM_2X6_NUM_4X1,
            7 => NUM_2X7_NUM_2X7_NUM_4X1,
            _ => 0,
        },
        // (PinMAME leaves Bally's eight displays, six of 6 and two of 2, without one: "!! ??")
        GEN_BY17 | GEN_BY35 if total == 8 => 0,
        GEN_BY17 | GEN_BY35 => match first {
            6 => NUM_2X6_NUM_2X6_NUM_4X1,
            7 if named("medusa") => NUM_2X7_NUM_2X7_NUM_10X1,
            7 => NUM_2X7_NUM_2X7_NUM_4X1,
            _ => 0,
        },
        GEN_BY6803 | GEN_BY6803A => NUM10_4X7,
        GEN_BYPROTO => NUM_2X6_NUM_2X6_NUM_4X1,
        GEN_HNK => ALPHA_2X20,
        _ => 0,
    }
}

/// Where the score display's window draws each digit when none of VPX's
/// layouts fits the game: where PinMAME's own window places it, the whole
/// fitted inside `out` pixels keeping its shape, centred (as VPX's score
/// view fits its layouts), a digit's cell 24 by 32 around its segments as
/// VPX's pictures draw them. Pixels from the top left: left, top, width,
/// height.
pub fn place(layout: &SegLayout, out: (u32, u32)) -> Vec<[f32; 4]> {
    // a little room around the digits for their glow
    let [l, t, r, b] = layout.bounds;
    let margin = 6.0;
    let (lw, lh) = (r - l + 2.0 * margin, b - t + 2.0 * margin);
    let (ow, oh) = (out.0.max(1) as f32, out.1.max(1) as f32);
    let scale = (ow / lw).min(oh / lh);
    let (px, py) = (0.5 * (ow - lw * scale), 0.5 * (oh - lh * scale));
    layout
        .digits()
        .map(|(_, d)| {
            let [x, y, w, h] = d.glyph;
            let cell = cell(w, h);
            let (cx, cy) = (x - l + margin + cell[0], y - t + margin + cell[1]);
            [px + cx * scale, py + cy * scale, cell[2] * scale, cell[3] * scale]
        })
        .collect()
}


/// A FlexDMD segment mode's displays, as VPX 5436's FlexDMD plugin offers
/// them (`onGetSegSrc`): each display's digits and their kind, by mode
/// (FlexDMD's numbers, 3 to 16).
fn flex_displays(mode: i32) -> &'static [(usize, SegKind)] {
    use SegKind::{FourteenDot as A, NineComma as N, SevenComma as S};
    match mode {
        3 => &[(16, A), (16, A)],
        4 => &[(20, A), (20, A)],
        5 => &[(7, A), (7, A), (7, S), (7, S)],
        6 => &[(7, A), (7, A), (7, S), (7, S), (2, S), (2, S)],
        7 | 9 => &[(7, S), (7, S), (7, S), (7, S), (2, S), (2, S)],
        8 => &[(7, S), (7, S), (7, S), (7, S), (2, S), (2, S), (2, S), (2, S), (2, S)],
        10 => &[(7, N), (7, N), (7, N), (7, N), (2, S), (2, S)],
        11 => &[(6, S), (6, S), (6, S), (6, S), (2, S), (2, S)],
        12 => &[(6, N), (6, N), (6, N), (6, N), (2, S), (2, S)],
        13 => &[(7, N), (7, N), (7, N), (7, N)],
        14 => &[(4, S), (4, S), (4, S), (4, S), (4, S), (4, S), (2, S), (2, S)],
        15 => &[(7, S), (7, S), (2, S), (2, S), (16, A)],
        16 => &[(16, A), (16, S), (7, S)],
        _ => &[],
    }
}

/// A FlexDMD segment mode's layout: its displays one under the other, as
/// PinMAME's window places a game's lines.
pub fn flex_layout(mode: i32) -> Option<SegLayout> {
    let shapes = flex_displays(mode);
    if shapes.is_empty() {
        return None;
    }
    let (cols, rows) = (15, 20);
    let mut displays = Vec::new();
    let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for (d, &(n, kind)) in shapes.iter().enumerate() {
        let digits: Vec<Digit> = (0..n)
            .map(|j| {
                let glyph = [(j as i32 * (cols + 1)) as f32, (d as i32 * (rows + 1)) as f32, cols as f32, rows as f32];
                bounds = [bounds[0].min(glyph[0]), bounds[1].min(glyph[1]), bounds[2].max(glyph[0] + glyph[2] + 1.0), bounds[3].max(glyph[1] + glyph[3] + 1.0)];
                Digit { display: d, index: j, kind, swapped_marks: false, glyph }
            })
            .collect();
        displays.push(SegDisplay { digits, hardware: HARDWARE_UNKNOWN });
    }
    Some(SegLayout { displays, bounds })
}

/// A FlexDMD's segments now, for the first FlexDMD showing a segment mode:
/// a value a digit, display after display, in the order the script gives
/// them (VPX's plugin reads some modes' single digits by a table that
/// disagrees with its own displays; the digits' order is what both
/// FlexDMD's values and the displays imply); what's missing stays dark.
fn flex_shown(w: &mut crate::world::World) -> Option<Arc<SegmentsNow>> {
    let (mode, segs) = w.flexdmds.iter().find_map(|f| f.borrow().display_segments().map(|(m, s)| (m, s.to_vec())))?;
    if let Some((m, s, now)) = &w.flex_segments
        && *m == mode
        && *s == segs
    {
        return Some(now.clone());
    }
    let layout = Arc::new(flex_layout(mode)?);
    let levels = (0..layout.digits().count()).map(|i| self::levels(segs.get(i).copied().unwrap_or(0), false)).collect();
    w.segment_frames += 1;
    let now = Arc::new(SegmentsNow { layout, levels, id: w.segment_frames });
    w.flex_segments = Some((mode, segs, now.clone()));
    Some(now)
}

/// A segment game's display as VPX 5436's AlphaDMD hands it to colouring
/// (its identify frame): the digits drawn as 128 × 32 dots, levels 0 to 3,
/// in the layout of PinMAME's DMD devices that its displays match — by how
/// many there are and how many digits each has, as AlphaDMD matches them —
/// None when none does.
pub fn alpha_identify(now: &SegmentsNow) -> Option<crate::alphanumeric::AlphaFrame> {
    use crate::alphanumeric::{AlphaFrame, Layout};
    let name = alpha_layout(now)?;
    let first = now.layout.displays.first()?;
    let kind = first.digits.first()?.kind;
    let third = first.digits.get(2).map(|d| d.kind);
    let layout = match name {
        "6x4_2x2" => Layout::Num6x4Num4x1,
        "4x6_2x2" if matches!(kind, SegKind::Nine | SegKind::NineComma) => Layout::Num10x2x6Num10x2x6Num4x1,
        "4x6_2x2" => Layout::Num2x6Num2x6Num4x1,
        "4x7" if kind.index() >= SegKind::Fourteen.index() => Layout::Alpha2x7Num2x7,
        "4x7" => Layout::Num10x4x7,
        "4x7_2x2" if matches!(kind, SegKind::Nine | SegKind::NineComma) => Layout::Num10x2x7Num10x2x7Num4x1,
        "4x7_2x2" if matches!(kind, SegKind::Fourteen | SegKind::FourteenDot | SegKind::FourteenDotComma) => Layout::Alpha2x7Num2x7Num4x1,
        // no thousands comma
        "4x7_2x2" if third == Some(SegKind::Seven) => Layout::Num2x7Num2x7Num4x1,
        "4x7_2x2" => Layout::Num2x7Num2x7Num4x1Gen7,
        "4x7_5x2" => Layout::Num2x7Num2x7Num10x1,
        "2x16" => Layout::Alpha2x16,
        "1x7_2x16" => Layout::Num1x7Alpha1x16Num1x16,
        "2x16_1x7" => Layout::Alpha1x16Num1x16Num1x7,
        "1x7_1x4_2x16" => Layout::Alpha1x16Num1x16Num1x7Num1x4,
        "2x7_2x2_1x16" => Layout::Num2x7Num4x1Alpha1x16,
        "2x20" => Layout::Alpha2x20,
        _ => return None,
    };
    // each digit's lit segments as bits, as many as its kind has (AlphaDMD's nSegments)
    let segments_of = |k: SegKind| [7, 8, 8, 10, 10, 15, 15, 16, 16][k.index()];
    let bits: Vec<u16> = now
        .layout
        .digits()
        .zip(&now.levels)
        .map(|((_, d), levels)| (0..segments_of(d.kind)).filter(|&k| levels[k] > 0.5).fold(0u16, |b, k| b | (1 << k)))
        .collect();
    // (AlphaDMD never fills Medusa's second set of digits, its FIXME says: none here either)
    Some(AlphaFrame::draw(layout, &bits, &[]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments(kind: u32, top: i32, left: i32, length: i32) -> DisplayLayout {
        DisplayLayout { kind, top, left, length, ..DisplayLayout::default() }
    }

    #[test]
    fn segment_games_are_drawn_in_alphadmds_layouts() {
        let digit = |display| Digit { display, index: 0, kind: SegKind::Seven, swapped_marks: false, glyph: [0.0; 4] };
        let now = |counts: &[usize]| {
            let displays: Vec<SegDisplay> = counts.iter().enumerate().map(|(d, &n)| SegDisplay { digits: vec![digit(d); n], hardware: 0 }).collect();
            let n = counts.iter().sum();
            SegmentsNow { layout: Arc::new(SegLayout { displays, bounds: [0.0; 4] }), levels: vec![levels(0x3f, false); n], id: 1 }
        };
        // Gorgar's: four displays of six digits, two of two, every digit a 0
        let gorgar = alpha_identify(&now(&[6, 6, 6, 6, 2, 2])).expect("AlphaDMD's 4x6_2x2");
        let drawn = crate::alphanumeric::AlphaFrame::draw(crate::alphanumeric::Layout::Num2x6Num2x6Num4x1, &[0x3f; 28], &[]);
        assert!(gorgar.dots == drawn.dots);
        // displays AlphaDMD has no layout for
        assert!(alpha_identify(&now(&[6, 6, 6, 6])).is_none());
    }

    #[test]
    fn pinmames_own_layouts_for_display_devices() {
        // Gorgar (Williams System 6), Medusa and Eight Ball Deluxe (Bally), Police Force, a Gottlieb System 3
        assert_eq!(pinmame_layout(0x2_0000, &[6, 6, 6, 6, 2, 2], "grgar_l1"), 9);
        assert_eq!(pinmame_layout(0x40_0000, &[7, 7, 7, 7, 2, 2, 2, 2, 2], "medusa"), 6);
        assert_eq!(pinmame_layout(0x40_0000, &[7, 7, 7, 7, 2, 2], "eballdlx"), 5);
        assert_eq!(pinmame_layout(0x100, &[7, 16, 16], "polic_l4"), 15);
        assert_eq!(pinmame_layout(0x200_0000_0000, &[20, 20], "anything"), 2);
        // a generation PinMAME has none for
        assert_eq!(pinmame_layout(0x1_0000_0000_0000, &[16, 16], "x"), 0);
    }

    #[test]
    fn black_knights_displays_group_as_libpinmame_groups_them() {
        // System 7, as PinMAME describes Black Knight: four seven-digit scores with a comma
        // every three, then ball and credit as single small digits, then the DMD it draws
        let displays = [
            (0, segments(SEG87, 4, 0, 7)),
            (1, segments(SEG87, 4, 16, 7)),
            (2, segments(SEG87, 0, 0, 7)),
            (3, segments(SEG87, 0, 16, 7)),
            (4, segments(SEG7S, 2, 8, 1)),
            (5, segments(SEG7S, 2, 10, 1)),
            (6, segments(SEG7S, 2, 2, 1)),
            (7, segments(SEG7S, 2, 4, 1)),
            (8, DisplayLayout { kind: pinmame_link::DISPLAY_DMD, width: 128, height: 32, ..DisplayLayout::default() }),
        ];
        let l = layout(&displays, GEN_S7).expect("segment displays");
        let sizes: Vec<usize> = l.displays.iter().map(|d| d.digits.len()).collect();
        assert_eq!(sizes, [7, 7, 7, 7, 2, 2]);
        assert!(l.displays.iter().all(|d| d.hardware == HARDWARE_NEON_PLASMA));
        // a comma before the thousands and the millions
        let kinds: Vec<SegKind> = l.displays[0].digits.iter().map(|d| d.kind).collect();
        use SegKind::{Seven as S, SevenComma as C};
        assert_eq!(kinds, [C, S, S, C, S, S, S]);
        // the second ball-and-credit pair comes from displays 6 and 7, side by side
        assert_eq!(l.displays[5].digits.iter().map(|d| (d.display, d.index)).collect::<Vec<_>>(), [(6, 0), (7, 0)]);
        // in PinMAME's window: the small digits a line down, the second score a display across
        assert_eq!(l.displays[4].digits[0].glyph, [48.0, 21.0, 11.0, 12.0]);
        assert_eq!(l.displays[1].digits[0].glyph, [128.0, 42.0, 15.0, 20.0]);
        assert_eq!(l.bounds, [0.0, 0.0, 128.0 + 6.0 * 16.0 + 16.0, 63.0]);
        // nothing but a DMD: no segment displays
        assert!(layout(&displays[8..], GEN_S7).is_none());
    }

    #[test]
    fn digits_read_as_their_figures() {
        // a 0 with its comma, a 1 of seven segments and of Gottlieb's nine, a blank
        assert_eq!(figure(&levels(0x00BF, false)), 0);
        assert_eq!(figure(&levels(0x0006, false)), 1);
        assert_eq!(figure(&levels(0x0300, false)), 1);
        assert_eq!(figure(&levels(0, false)), -1);
        // a type with comma and period swapped has them swapped back
        let l = levels(0x0080, true);
        assert_eq!((l[7], l[15]), (0.0, 1.0));
    }

    #[test]
    fn every_digit_has_vpxs_picture() {
        assert_eq!(picture_name(SegKind::Seven, hint(HARDWARE_NEON_PLASMA)), "7seg-williams");
        assert_eq!(picture_name(SegKind::Seven, hint(HARDWARE_GTS80A_7DIGIT)), "7seg-gts");
        assert_eq!(picture_name(SegKind::SevenComma, SegHint::Gottlieb), "7seg-c-williams");
        assert_eq!(picture_name(SegKind::FourteenDotComma, SegHint::Gottlieb), "14seg-dc-gts");
        assert_eq!(PICTURES.len(), PICTURE_COUNT);
        for kind in SegKind::ALL {
            for h in [SegHint::Generic, SegHint::Gottlieb, SegHint::Williams, SegHint::Bally, SegHint::Atari] {
                let index = picture_index(picture_name(kind, h)).expect("built in");
                // every picture four tiles of 96 x 128
                let (w, h, rgba) = picture(index).expect("decodes");
                assert_eq!((w, h, rgba.len()), (384, 128, 384 * 128 * 4));
            }
        }
    }

    #[test]
    fn plasma_segments_are_orange_and_bright() {
        let l = look(style(HARDWARE_NEON_PLASMA), [1.0; 3], 1.0);
        // VPX's default, 0x1E96FF: red 255, green 150, blue 30, made linear, times 5
        assert!((l.lit[0] - 5.0).abs() < 1e-4);
        assert!((l.lit[1] - 5.0 * 0.304_987).abs() < 1e-3);
        assert!((l.lit[2] - 5.0 * 0.012_983).abs() < 1e-3);
        assert_eq!(style(HARDWARE_GTS80A_7DIGIT), SegStyle::BlueVfd);
        assert_eq!(style(HARDWARE_UNKNOWN), SegStyle::Plasma);
    }
}
