// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its HDR10 output
// (src/renderer/RenderDevice.cpp: the backbuffer's format and colour space, allowHDR10ColorSpace;
// src/renderer/Renderer.cpp: pl_hdr_rescale, PrecompSplineTonemap, SetupTonemapping's isHdr2020 setup, the
// HDRGlobalExposure and bloom of its constructor; src/renderer/Window.cpp: the window's SDR white and headroom;
// src/core/Settings_properties.inl: HDRDisableToneMapper, HDRGlobalExposure).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX 5436's HDR10 output: on a screen where Windows' HDR is
//! on, VPX presents in HDR10 — ST.2084 (PQ) over BT.2020's colours — and its
//! own pictures, taken with HDR off, don't show it. There the picture keeps
//! its range: the scene's white is the screen's SDR white, what's brighter
//! goes up to the screen's peak, through VPX's spline in the table's tone
//! mapper's place (`[Player]` `HDRDisableToneMapper`, on by default), nearly
//! a straight line on a screen of 1000 nits or more; no bloom and no dither,
//! as VPX; the exposure times `[Player]` `HDRGlobalExposure`.
//!
//! VPX turns it on whenever its graphics library finds a screen in HDR (any
//! screen of the card, as BGFX reports it); ours when the table's own screen
//! is, the case of every setup where the two differ being a playfield on an
//! SDR screen beside an HDR one, which VPX then shows at 80 nits' white.
//! Pictures in test mode never use it, as VPX's own captures don't
//! (CaptureAttract). `[VPI]` `HDROutput = 0` or `--hdr off` keeps the
//! ordinary colours on an HDR screen.

/// What the settings ask of an HDR screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HdrRequest {
    /// VPX's spline in the table's tone mapper's place (its `HDRDisableToneMapper`, true by default).
    pub spline: bool,
    /// VPX's `HDRGlobalExposure`, times the table's exposure (1 by default).
    pub exposure: f32,
}

impl Default for HdrRequest {
    fn default() -> Self {
        HdrRequest { spline: true, exposure: 1.0 }
    }
}

/// The HDR10 output in use: the window's screen, as SDL reports it, and
/// the settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hdr10 {
    /// The screen's SDR white, in nits (SDL's white level × 80, as VPX's `GetSDRWhitePoint() * 80`).
    pub sdr_white: f32,
    /// The screen's peak, in nits (VPX's `maxDisplayLuminance`: headroom × SDR white).
    pub peak: f32,
    pub request: HdrRequest,
}

impl Hdr10 {
    /// From SDL's window properties: the SDR white level, in multiples of
    /// 80 nits, and the headroom, the peak in multiples of that white.
    pub fn new(sdr_white_level: f32, headroom: f32, request: HdrRequest) -> Hdr10 {
        let sdr_white = sdr_white_level * 80.0;
        Hdr10 { sdr_white, peak: headroom * sdr_white, request }
    }

    /// The tonemapping pass's HDR uniforms: (1, VPX's
    /// `sceneLum_x_invDisplayMaxLum`, its `displayMaxLum`, the spline on),
    /// and the spline's six parameters in VPX's order.
    pub fn uniforms(&self) -> ([f32; 4], [f32; 4], [f32; 4]) {
        let s = spline(self.peak);
        (
            [1.0, self.sdr_white / self.peak, self.peak / 10000.0, if self.request.spline { 1.0 } else { 0.0 }],
            [s[0], s[1], s[2], s[3]],
            [s[4], s[5], 0.0, 0.0],
        )
    }
}

/// Between PQ values and nits (VPX's `pl_hdr_rescale`, after libplacebo):
/// `to_nits` from PQ to nits, else from nits to PQ.
fn rescale(to_nits: bool, x: f32) -> f32 {
    let x = x.max(0.0);
    if x == 0.0 {
        return 0.0;
    }
    let m1 = ((2610.0 / 4096.0) / 4.0) as f32;
    let m2 = ((2523.0 / 4096.0) * 128.0) as f32;
    let c1 = (3424.0 / 4096.0) as f32;
    let c2 = ((2413.0 / 4096.0) * 32.0) as f32;
    let c3 = ((2392.0 / 4096.0) * 32.0) as f32;
    if to_nits {
        let mut x = x.powf((1.0 / ((2523.0 / 4096.0) * 128.0)) as f32);
        x = (x - c1).max(0.0) / (c2 - c3 * x);
        x = x.powf((1.0 / ((2610.0 / 4096.0) / 4.0)) as f32);
        x *= (10000.0 / 203.0) as f32;
        x * 203.0
    } else {
        let mut x = x * (1.0 / 203.0) as f32;
        x *= (203.0 / 10000.0) as f32;
        x = x.powf(m1);
        x = (c1 + c2 * x) / (1.0 + c3 * x);
        x.powf(m2)
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 == edge1 {
        return if x >= edge0 { 1.0 } else { 0.0 };
    }
    let x = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn lerp(x1: f32, x2: f32, alpha: f32) -> f32 {
    (1.0 - alpha) * x1 + alpha * x2
}

/// VPX's spline for a screen of `display_max_lum` nits (its
/// `PrecompSplineTonemap`, after VideoLAN's): the source and destination
/// pivots, the slope, and the curves' factors below (Pa) and above (Qa, Qb).
pub fn spline(display_max_lum: f32) -> [f32; 6] {
    let src_hdr_max = display_max_lum.max(1000.0);
    let slope_tuning = 1.5f32;
    let slope_offset = 0.2f32;
    let spline_contrast = 0.5f32;
    let knee_minimum = 0.1f32;
    let knee_maximum = 0.8f32;
    let knee_default = 0.4f32;
    let knee_adaptation = 0.4f32;

    let src_max = rescale(false, src_hdr_max);
    let dst_max = rescale(false, display_max_lum);
    let (src_knee_min, src_knee_max) = (src_max * knee_minimum, src_max * knee_maximum);
    let (dst_knee_min, dst_knee_max) = (dst_max * knee_minimum, dst_max * knee_maximum);
    let src_knee = (src_max * knee_default).max(src_knee_min).min(src_knee_max);
    let target = src_knee / src_max;
    let adapted = dst_max * target;
    // VPX's first smoothstep runs from its higher edge down, as written
    let tuning = 1.0 - smoothstep(knee_maximum, knee_default, target) * smoothstep(knee_minimum, knee_default, target);
    let adaptation = lerp(knee_adaptation, 1.0, tuning);
    let dst_knee = lerp(src_knee, adapted, adaptation).max(dst_knee_min).min(dst_knee_max);
    let src_pivot = rescale(true, src_knee);
    let dst_pivot = rescale(true, dst_knee);

    let mut slope = dst_pivot / src_pivot;
    let ratio = (slope_tuning * (src_hdr_max / display_max_lum - 1.0)).max(slope_offset).min(1.0 + slope_offset);
    slope = slope.powf((1.0 - spline_contrast) * ratio);

    let in_max = src_hdr_max - src_pivot;
    let out_max = display_max_lum - dst_pivot;
    let pa = (slope * src_pivot - dst_pivot) / (src_pivot * src_pivot);
    let t = 2.0 * in_max * in_max;
    let qa = (slope * in_max - out_max) / (in_max * t);
    let qb = -3.0 * (slope * in_max - out_max) / t;
    [src_pivot, dst_pivot, slope, pa, qa, qb]
}

/// A picture read back from a window in HDR10's format (R10G10B10A2: 10 bits a colour, red lowest)
/// made an ordinary picture of 8 bits a colour, as a screenshot is kept (an HDR screen's screenshots
/// otherwise come out in false colours, their 10-bit values read as 8-bit ones). `sdr_white`, in nits: the values are VPX's HDR10 output — ST.2084 (PQ) over
/// BT.2020's colours — decoded to light, the screen's SDR white made 1, brought into sRGB's colours
/// (BT.709's), what's brighter than white rolled off toward it rather than cut, and encoded as sRGB.
/// None: the values are sRGB's already (VPX's HDR look in the score window), only shortened.
pub fn to_srgb8(data: &mut [u8], sdr_white: Option<f32>) {
    // the 10 bits' PQ values as multiples of the screen's SDR white, and sRGB's encoding in 4096 steps
    let light: Vec<f32> = match sdr_white {
        Some(white) => (0..1024).map(|v| rescale(true, v as f32 / 1023.0) / white.max(1.0)).collect(),
        None => Vec::new(),
    };
    let encode: Vec<u8> = (0..4096)
        .map(|i| {
            let c = i as f32 / 4095.0;
            let s = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
            (s * 255.0 + 0.5) as u8
        })
        .collect();
    const KNEE: f32 = 0.75;
    for px in data.as_chunks_mut::<4>().0 {
        let v = u32::from_le_bytes(*px);
        let [r, g, b] = [v & 1023, (v >> 10) & 1023, (v >> 20) & 1023].map(|c| c as usize);
        let out = if light.is_empty() {
            [r, g, b].map(|c| ((c * 255 + 511) / 1023) as u8)
        } else {
            let (r, g, b) = (light[r], light[g], light[b]);
            // BT.2020's colours to BT.709's, the inverse of the shader's matrix
            let mut c = [1.6605 * r - 0.5876 * g - 0.0728 * b, -0.1246 * r + 1.1329 * g - 0.0083 * b, -0.0182 * r - 0.1006 * g + 1.1187 * b];
            // the light past the knee rolled off toward white, its colour kept
            let most = c[0].max(c[1]).max(c[2]);
            if most > KNEE {
                let to = KNEE + (1.0 - KNEE) * (1.0 - (-(most - KNEE) / (1.0 - KNEE)).exp());
                c = c.map(|x| x * to / most);
            }
            c.map(|x| encode[(x.clamp(0.0, 1.0) * 4095.0 + 0.5) as usize])
        };
        *px = [out[0], out[1], out[2], 255];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A colour as the shader writes it (`hdr10_out`, its last steps): in multiples of the SDR
    /// white, into nits, BT.2020's colours, PQ, 10 bits each.
    fn as_written(rgb: [f32; 3], sdr_white: f32) -> [u8; 4] {
        let [r, g, b] = rgb.map(|c| c * sdr_white);
        let wide = [0.6274 * r + 0.3293 * g + 0.0433 * b, 0.0691 * r + 0.9195 * g + 0.0114 * b, 0.0164 * r + 0.0880 * g + 0.8956 * b];
        let [r, g, b] = wide.map(|n| (rescale(false, n) * 1023.0).round() as u32);
        (r | (g << 10) | (b << 20) | (3 << 30)).to_le_bytes()
    }

    #[test]
    fn an_hdr_screens_picture_comes_back_in_ordinary_colours() {
        // white, a mid grey, a pure red and black, at a screen's SDR white of 200 nits
        let colours = [[1.0, 1.0, 1.0], [0.214, 0.214, 0.214], [0.5, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let mut data: Vec<u8> = colours.iter().flat_map(|&c| as_written(c, 200.0)).collect();
        to_srgb8(&mut data, Some(200.0));
        let px: Vec<[u8; 4]> = data.as_chunks::<4>().0.to_vec();
        // white past the knee rolls off just short of white; the grey (sRGB's 128) and the red keep their places
        let near = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 1;
        assert!(px[0][0] >= 240 && near(px[0][0], px[0][1]) && near(px[0][1], px[0][2]), "{:?}", px[0]);
        assert!((px[1][0] as i32 - 128).abs() <= 2 && near(px[1][0], px[1][2]), "{:?}", px[1]);
        assert!((px[2][0] as i32 - 188).abs() <= 3 && px[2][1] <= 2 && px[2][2] <= 2, "{:?}", px[2]);
        assert_eq!(px[3], [0, 0, 0, 255]);
        // sRGB values written into the format as they are: shortened only
        let mut plain = (1023u32 | (512 << 10) | (3 << 30)).to_le_bytes().to_vec();
        to_srgb8(&mut plain, None);
        assert_eq!(plain, [255, 128, 0, 255]);
    }

    /// The spline as the tonemapping shader applies it to a luminance.
    fn apply(s: &[f32; 6], y: f32) -> f32 {
        let y = y - s[0];
        let y = if y > 0.0 { ((s[4] * y + s[5]) * y + s[2]) * y } else { (s[3] * y + s[2]) * y };
        y + s[1]
    }

    #[test]
    fn a_screen_of_1000_nits_or_more_keeps_the_picture_as_it_is() {
        for peak in [1000.0, 1500.0, 4000.0] {
            let s = spline(peak);
            for y in [0.001, 0.05, 0.5, 1.0, 4.0, 20.0, 400.0] {
                assert!((apply(&s, y) - y).abs() < 1e-3 * y.max(1.0), "{peak} nits: {y} became {}", apply(&s, y));
            }
        }
    }

    #[test]
    fn a_dimmer_screen_bends_the_brightest_down_to_its_peak() {
        let s = spline(600.0);
        // the curve reaches the screen's peak where the scene's assumed peak is
        assert!((apply(&s, 1000.0) - 600.0).abs() < 1.0, "{}", apply(&s, 1000.0));
        // and it only ever rises
        let mut last = f32::MIN;
        for k in 1..2000 {
            let y = apply(&s, k as f32 * 0.5);
            assert!(y > last);
            last = y;
        }
    }

    #[test]
    fn the_uniforms_carry_the_white_and_the_peak() {
        let h = Hdr10::new(3.0, 1000.0 / 240.0, HdrRequest::default());
        let (hdr, _, _) = h.uniforms();
        assert_eq!(hdr[0], 1.0);
        assert!((hdr[1] - 0.24).abs() < 1e-6);
        assert!((hdr[2] - 0.1).abs() < 1e-6);
        assert_eq!(hdr[3], 1.0);
    }

    #[test]
    fn the_hdr10_shaders_compile() {
        use super::super::{OVERLAY_HLSL, TONEMAP_HLSL, fxc};
        fxc::compile(TONEMAP_HLSL, c"tonemap_fs", c"ps_5_1").unwrap();
        // the readout's, compiled only when the output is in use, with the screen's white in it
        let overlay = format!("#define OVERLAY_PQ_WHITE {:.9}\n{OVERLAY_HLSL}", 240.0 / 10000.0);
        fxc::compile(&overlay, c"overlay_pq_fs", c"ps_5_1").unwrap();
    }

    #[test]
    fn pq_and_nits_round_trip() {
        for nits in [0.5, 10.0, 80.0, 203.0, 1000.0, 10000.0] {
            let back = rescale(true, rescale(false, nits));
            assert!((back - nits).abs() < 1e-3 * nits, "{nits} came back as {back}");
        }
        // 10000 nits is the top of PQ's range
        assert!((rescale(false, 10000.0) - 1.0).abs() < 1e-5);
    }
}
