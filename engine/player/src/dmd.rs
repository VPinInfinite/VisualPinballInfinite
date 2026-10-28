// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): the frames of
// ScriptGlobalTable::put_DMDPixels and put_DMDColoredPixels (src/core/ScriptGlobalTable.cpp), and the choice of the
// default display in ResURIResolver::GetDisplayState (plugins/plugins/ResURIResolver.cpp).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The table's score display (DMD), as text boxes and flashers marked as
//! DMDs show it. A frame comes from the script — `core.vbs`
//! copies the ROM's display into `DMDPixels` or `DMDColoredPixels` every
//! frame — or, when the script sends none, from the running machine itself,
//! as VPX's "default display" falls back to PinMAME's.
//!
//! Every frame is kept the same way for the renderer: a dot's linear red,
//! green and blue. A brightness-only frame keeps its brightness in all
//! three, and is tinted when drawn (by the text box's font colour, the
//! flasher's colour); a coloured frame's sRGB values are made linear, as
//! VPX's sRGB texture makes them when it samples them.

/// One frame of the display.
#[derive(Clone, Debug, PartialEq)]
pub struct DmdFrame {
    pub width: u32,
    pub height: u32,
    /// Linear red, green, blue and 1 a dot, row by row from the top.
    pub dots: Vec<[f32; 4]>,
    /// Brightness only (tinted when drawn), rather than coloured.
    pub luminance: bool,
    /// Its colours came in 16 bits (a colour file's RGB565, VPX's `SRGB565`), as a real DMD
    /// panel is handed them (`dmd_panel`).
    pub rgb565: bool,
    /// Counts the frames, so the renderer uploads only a new one.
    pub id: u64,
    /// The colour its source asks a brightness-only frame be shown in
    /// (FlexDMD's `Color`, which FlexDMD hands its DMD), as GDI+ keeps
    /// colours: alpha, red, green, blue.
    pub tint: Option<[u8; 4]>,
}

impl DmdFrame {
    /// VPX's `DMDPixels`: a brightness a dot, from 0 to 100.
    pub fn from_percent(width: u32, height: u32, values: &[i32], id: u64) -> DmdFrame {
        let dots = values.iter().take((width * height) as usize).map(|&v| {
            let l = v as f32 * (1.0 / 100.0);
            [l, l, l, 1.0]
        });
        DmdFrame::with_dots(width, height, dots, true, id)
    }

    /// VPX's `DMDColoredPixels`: a colour a dot, its bytes red, green and
    /// blue from the lowest (VPX reads them as an sRGB texture's).
    pub fn from_srgb(width: u32, height: u32, values: &[u32], id: u64) -> DmdFrame {
        let dots = values.iter().take((width * height) as usize).map(|&v| {
            let c = |shift: u32| srgb_to_linear(((v >> shift) & 0xFF) as f32 / 255.0);
            [c(0), c(8), c(16), 1.0]
        });
        DmdFrame::with_dots(width, height, dots, false, id)
    }

    /// A colour file's frame: `0x00RRGGBB` a dot, or RGB565 in the
    /// low 16 bits, its 5- and 6-bit values made 8-bit as VPX's BGFX build
    /// makes them (`copy_rgb565_rgba`, rounded), then read as sRGB as VPX's
    /// texture reads them.
    pub fn from_coloured(width: u32, height: u32, kind: u32, values: &[u32], id: u64) -> DmdFrame {
        let five = |x: u32| (x * 255 + 15) / 31;
        let six = |x: u32| (x * 255 + 31) / 63;
        let dots = values.iter().take((width * height) as usize).map(|&v| {
            let (r, g, b) = if kind == colour_link::DOTS_RGB565 {
                (five((v >> 11) & 0x1F), six((v >> 5) & 0x3F), five(v & 0x1F))
            } else {
                ((v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF)
            };
            let c = |x: u32| srgb_to_linear(x as f32 / 255.0);
            [c(r), c(g), c(b), 1.0]
        });
        DmdFrame { rgb565: kind == colour_link::DOTS_RGB565, ..DmdFrame::with_dots(width, height, dots, false, id) }
    }

    /// PinMAME's own frame: a brightness a dot, from 0 to 255.
    pub fn from_brightness(width: u32, height: u32, bytes: &[u8], id: u64) -> DmdFrame {
        let dots = bytes.iter().take((width * height) as usize).map(|&b| {
            let l = f32::from(b) / 255.0;
            [l, l, l, 1.0]
        });
        DmdFrame::with_dots(width, height, dots, true, id)
    }

    /// A frame of the given dots, dark where too few came.
    pub fn with_dots(width: u32, height: u32, dots: impl Iterator<Item = [f32; 4]>, luminance: bool, id: u64) -> DmdFrame {
        let n = (width * height) as usize;
        let mut dots: Vec<[f32; 4]> = dots.collect();
        dots.resize(n, [0.0, 0.0, 0.0, 1.0]);
        DmdFrame { width, height, dots, luminance, rgb565: false, id, tint: None }
    }
}

/// The display text boxes and flashers show now: the script's newest frame,
/// else the best of the displays offered — the running machine's own
/// (copied only when it has a new one), each FlexDMD's while it shows — as
/// VPX's default display picks among them: the largest, then colour over
/// brightness, then the first offered.
pub fn shown(w: &mut crate::world::World) -> Option<std::sync::Arc<DmdFrame>> {
    if let Some(f) = &w.dmd_frame {
        return Some(f.clone());
    }
    let mut best = machine_frame(w);
    for flex in &w.flexdmds {
        let Some(f) = flex.borrow().display_frame() else { continue };
        let better = match &best {
            None => true,
            Some(b) => {
                let (bs, fs) = (b.width * b.height, f.width * f.height);
                bs < fs || (bs == fs && b.luminance && !f.luminance)
            }
        };
        if better {
            best = Some(f);
        }
    }
    best.map(|f| upscaled(w, f))
}

/// The default display as VPX 5436's DMD upscaler shows it in its place,
/// when the settings ask for it (`crate::upscale`).
fn upscaled(w: &mut crate::world::World, source: std::sync::Arc<DmdFrame>) -> std::sync::Arc<DmdFrame> {
    if w.upscale_mode == crate::upscale::Mode::Disabled {
        return source;
    }
    if w.upscaler.is_none() {
        w.upscaler = crate::upscale::Upscaler::new(w.upscale_mode, w.upscale_at_once);
        if w.upscaler.is_none() {
            // a mode that scales nothing: not asked again
            w.upscale_mode = crate::upscale::Mode::Disabled;
            return source;
        }
    }
    let frames = &mut w.dmd_frames;
    match w.upscaler.as_mut() {
        Some(u) => u.shown(&source, || {
            *frames += 1;
            *frames
        }),
        None => source,
    }
}

/// The running machine's own frame, copied only when it has a new one — or,
/// once its colour file has coloured one, the coloured frame in its place,
/// as VPX's colour plugins override PinMAME's display.
fn machine_frame(w: &mut crate::world::World) -> Option<std::sync::Arc<DmdFrame>> {
    coloured_frame(w).or_else(|| machine_own_frame(w))
}

/// The machine's frame as its colour file coloured it, once there's one.
pub(crate) fn coloured_frame(w: &mut crate::world::World) -> Option<std::sync::Arc<DmdFrame>> {
    let colour = w.controller.as_mut()?.state.colour.as_mut()?;
    let frames = &mut w.dmd_frames;
    colour.frame(|| {
        *frames += 1;
        *frames
    })
}

/// The running machine's own frame, copied only when it has a new one.
pub(crate) fn machine_own_frame(w: &mut crate::world::World) -> Option<std::sync::Arc<DmdFrame>> {
    let machine = w.controller.as_mut()?.state.machine.as_mut()?;
    let number = machine.dmd_number()?;
    if let Some((n, f)) = &w.machine_dmd
        && *n == number
    {
        return Some(f.clone());
    }
    let (width, height, dots) = machine.dmd()?;
    let frame = std::sync::Arc::new(DmdFrame::from_brightness(width as u32, height as u32, dots, w.dmd_frames + 1));
    w.dmd_frames += 1;
    w.machine_dmd = Some((number, frame.clone()));
    Some(frame)
}

/// sRGB's curve undone: the value light actually has.
pub(crate) fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// A linear value back to the 8-bit sRGB value it was made from (`srgb_to_linear`'s inverse,
/// exact for every 8-bit value): how a real DMD panel is handed a frame's colours.
pub(crate) fn linear_to_srgb_byte(c: f32) -> u8 {
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_come_out_linear_whatever_their_source() {
        // a hundredth at a time, as VPX multiplies
        let f = DmdFrame::from_percent(2, 1, &[100, 33], 1);
        let third = 33.0 * (1.0 / 100.0);
        assert_eq!(f.dots, vec![[1.0, 1.0, 1.0, 1.0], [third, third, third, 1.0]]);
        assert!(f.luminance);
        // red in the lowest byte; mid-grey sRGB is about a fifth of the light
        let f = DmdFrame::from_srgb(2, 1, &[0x0000_00FF, 0x0080_8080], 2);
        assert_eq!(f.dots[0], [1.0, 0.0, 0.0, 1.0]);
        assert!((f.dots[1][0] - 0.2158).abs() < 1e-3);
        assert!(!f.luminance);
        let f = DmdFrame::from_brightness(2, 2, &[255, 0, 51], 3);
        assert_eq!(f.dots.len(), 4);
        assert_eq!(f.dots[2], [0.2, 0.2, 0.2, 1.0]);
        // a missing dot is dark
        assert_eq!(f.dots[3], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn colours_go_back_to_the_bytes_they_came_from() {
        for b in 0..=255u8 {
            assert_eq!(linear_to_srgb_byte(srgb_to_linear(f32::from(b) / 255.0)), b, "byte {b}");
        }
    }

    #[test]
    fn a_colour_files_16_bit_dots_widen_as_vpx_widens_them() {
        // VPX's lookup tables (Texture.h, copy_rgb565_rgba)
        const LUM32: [u32; 32] = [0, 8, 16, 25, 33, 41, 49, 58, 66, 74, 82, 90, 99, 107, 115, 123, 132, 140, 148, 156, 165, 173, 181, 189, 197, 206, 214, 222, 230, 239, 247, 255];
        const LUM64: [u32; 64] = [
            0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 45, 49, 53, 57, 61, 65, 69, 73, 77, 81, 85, 89, 93, 97, 101, 105, 109, 113, 117, 121, 125, 130, 134, 138, 142, 146, 150, 154, 158, 162, 166,
            170, 174, 178, 182, 186, 190, 194, 198, 202, 206, 210, 215, 219, 223, 227, 231, 235, 239, 243, 247, 251, 255,
        ];
        let lin = |x: u32| srgb_to_linear(x as f32 / 255.0);
        for v in 0..32u32 {
            // red in the high five bits, blue in the low five, green between
            let f = DmdFrame::from_coloured(1, 1, colour_link::DOTS_RGB565, &[(v << 11) | ((v * 2) << 5) | (31 - v)], 1);
            assert_eq!(f.dots[0], [lin(LUM32[v as usize]), lin(LUM64[v as usize * 2]), lin(LUM32[31 - v as usize]), 1.0]);
            let g = DmdFrame::from_coloured(1, 1, colour_link::DOTS_RGB565, &[(v * 2 + 1) << 5], 1);
            assert_eq!(g.dots[0][1], lin(LUM64[v as usize * 2 + 1]));
        }
        let f = DmdFrame::from_coloured(2, 1, colour_link::DOTS_RGB888, &[0x00FF_0080], 2);
        assert_eq!(f.dots, vec![[1.0, 0.0, lin(0x80), 1.0], [0.0, 0.0, 0.0, 1.0]]);
        assert!(!f.luminance);
    }
}
