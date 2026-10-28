// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): the DMD upscaler
// plugin, plugins/upscaledmd/UpscaleDMD.cpp (added 2025 by VPX's team).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX 5436's DMD upscaler (its `upscaledmd` plugin): the display's picture
//! made larger and smoother by a pixel-art scaler, for big screens and
//! high-resolution DMDs, wherever the display shows — the score window, the
//! backglass, the table. It takes the display VPX's "default display" is
//! (the machine's own, coloured or not, or a FlexDMD's; never the frames a
//! script draws itself) and stands in its place.
//!
//! The scalers: ScaleFX, which smooths the dots at their own size ("AA 1x")
//! or makes each nine; MMPX and Super-xBR, which make each four; xBRZ, two
//! to six times (its licence the GPL's version 3 alone). A brightness-only display is scaled as sRGB
//! brightness and turned back with VPX's own table; a coloured one as its
//! sRGB bytes.
//!
//! VPX scales on a thread of its own, the picture shown one frame late; the
//! player does the same, while test mode and the headless runner scale each
//! picture at once, so their pictures repeat exactly.

pub mod mmpx;
pub mod scalefx;
pub mod superxbr;
pub mod xbrz;

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;

use crate::dmd::{DmdFrame, srgb_to_linear};

/// The upscaler's modes, in VPX's order: the values of its `UpscaleMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Disabled,
    ScaleFxAa,
    ScaleFx3x,
    Mmpx2x,
    Xbrz2x,
    Xbrz3x,
    Xbrz4x,
    Xbrz5x,
    Xbrz6x,
    SuperXbr2x,
}

impl Mode {
    pub const ALL: [Mode; 10] = [
        Mode::Disabled,
        Mode::ScaleFxAa,
        Mode::ScaleFx3x,
        Mode::Mmpx2x,
        Mode::Xbrz2x,
        Mode::Xbrz3x,
        Mode::Xbrz4x,
        Mode::Xbrz5x,
        Mode::Xbrz6x,
        Mode::SuperXbr2x,
    ];

    /// VPX's name for it.
    pub fn name(self) -> &'static str {
        ["Disabled", "ScaleFX AA 1x", "ScaleFX 3x", "MMPX 2x", "xBRZ 2x", "xBRZ 3x", "xBRZ 4x", "xBRZ 5x", "xBRZ 6x", "Super-XBR 2x"][self as usize]
    }

    /// How many times wider and higher the picture comes out.
    pub fn factor(self) -> u32 {
        [1, 1, 3, 2, 2, 3, 4, 5, 6, 2][self as usize]
    }

    /// The mode the settings' `[Plugin.UpscaleDMD]` ask for: `UpscaleMode`
    /// (VPX's number, Disabled by default, as in VPX), unless `Enable` is 0.
    pub fn from_settings(settings: &[(String, String)]) -> Mode {
        let setting = |key: &str| settings.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| crate::settings::vpx_trim(v).to_string());
        if setting("Enable").as_deref().and_then(crate::settings::vpx_bool) == Some(false) {
            return Mode::Disabled;
        }
        setting("UpscaleMode").and_then(|v| v.parse::<usize>().ok()).and_then(|i| Mode::ALL.get(i).copied()).unwrap_or(Mode::Disabled)
    }
}

/// VPX's `linearToGammaU8`: a linear brightness as an sRGB byte, rounded.
fn linear_to_gamma_u8(f: f32) -> u32 {
    let v = if f <= 0.003_130_8 {
        (255.0 * 12.92f64) as f32 * f + 0.5
    } else {
        (255.0 * 1.055f64) as f32 * f.powf((1.0 / 2.4f64) as f32) - (255.0 * 0.055 - 0.5f64) as f32
    };
    v as i32 as u32
}

/// VPX's `gammaToLinearLUT`: each sRGB byte's linear brightness, as VPX
/// computed and printed them.
#[allow(clippy::excessive_precision)]
const GAMMA_TO_LINEAR: [f32; 256] = [
    0.0, 0.000303527, 0.000607054, 0.000910581, 0.001214108, 0.001517635, 0.001821162, 0.0021246888, 0.002428216,
    0.0027317428, 0.00303527, 0.0033465358, 0.0036765074, 0.004024717, 0.004391442, 0.0047769533, 0.0051815165,
    0.0056053917, 0.006048833, 0.0065120906, 0.00699541, 0.007499032, 0.008023193, 0.008568126, 0.009134059,
    0.009721218, 0.010329823, 0.010960094, 0.011612245, 0.012286488, 0.0129830325, 0.013702083, 0.014443844,
    0.015208514, 0.015996294, 0.016807375, 0.017641954, 0.01850022, 0.019382361, 0.020288562, 0.02121901, 0.022173885,
    0.023153367, 0.024157632, 0.02518686, 0.026241222, 0.027320892, 0.02842604, 0.029556835, 0.030713445, 0.031896032,
    0.033104766, 0.034339808, 0.035601314, 0.03688945, 0.038204372, 0.039546236, 0.0409152, 0.04231141, 0.04373503,
    0.045186203, 0.046665087, 0.048171826, 0.049706567, 0.051269457, 0.052860647, 0.054480277, 0.05612849, 0.05780543,
    0.059511237, 0.061246052, 0.063010015, 0.064803265, 0.06662594, 0.06847817, 0.070360094, 0.07227185, 0.07421357,
    0.07618538, 0.07818742, 0.08021982, 0.08228271, 0.08437621, 0.08650046, 0.08865558, 0.09084171, 0.093058966,
    0.09530747, 0.09758735, 0.099898726, 0.10224173, 0.104616486, 0.107023105, 0.10946171, 0.11193243, 0.114435375,
    0.116970666, 0.11953843, 0.122138776, 0.12477182, 0.12743768, 0.13013647, 0.13286832, 0.13563333, 0.13843161,
    0.14126329, 0.14412847, 0.14702727, 0.14995979, 0.15292615, 0.15592647, 0.15896083, 0.16202937, 0.1651322,
    0.1682694, 0.17144111, 0.1746474, 0.17788842, 0.18116425, 0.18447499, 0.18782078, 0.19120169, 0.19461784,
    0.19806932, 0.20155625, 0.20507874, 0.20863687, 0.21223076, 0.2158605, 0.2195262, 0.22322796, 0.22696587,
    0.23074006, 0.23455058, 0.23839757, 0.24228112, 0.24620132, 0.25015828, 0.2541521, 0.25818285, 0.26225066,
    0.2663556, 0.2704978, 0.2746773, 0.27889428, 0.28314874, 0.28744084, 0.29177064, 0.29613826, 0.30054379,
    0.3049873, 0.30946892, 0.31398872, 0.31854677, 0.3231432, 0.3277781, 0.33245152, 0.33716363, 0.34191442,
    0.34670407, 0.3515326, 0.35640013, 0.3613068, 0.3662526, 0.3712377, 0.37626213, 0.38132602, 0.38642943,
    0.39157248, 0.39675522, 0.40197778, 0.4072402, 0.4125426, 0.41788507, 0.42326766, 0.4286905, 0.43415365,
    0.43965718, 0.4452012, 0.4507858, 0.45641103, 0.462077, 0.4677838, 0.47353148, 0.47932017, 0.48514995, 0.49102086,
    0.49693298, 0.5028865, 0.50888133, 0.5149177, 0.52099556, 0.5271151, 0.5332764, 0.5394795, 0.54572445, 0.55201143,
    0.5583404, 0.5647115, 0.57112485, 0.57758045, 0.58407843, 0.59061885, 0.59720176, 0.60382736, 0.61049557,
    0.6172066, 0.6239604, 0.63075715, 0.63759685, 0.6444797, 0.65140563, 0.65837485, 0.6653873, 0.67244315, 0.6795425,
    0.6866853, 0.69387174, 0.7011019, 0.70837575, 0.7156935, 0.7230551, 0.73046076, 0.7379104, 0.7454042, 0.7529422,
    0.7605245, 0.76815116, 0.7758222, 0.7835378, 0.7912979, 0.7991027, 0.80695224, 0.8148466, 0.82278574, 0.8307699,
    0.838799, 0.8468732, 0.8549926, 0.8631572, 0.8713671, 0.8796224, 0.8879231, 0.8962694, 0.9046612, 0.91309863,
    0.92158186, 0.9301109, 0.9386857, 0.9473065, 0.9559733, 0.9646863, 0.9734453, 0.9822506, 0.9911021, 1.0,
];

/// Each sRGB byte's linear value as the player's frames hold it.
fn linear_table() -> &'static [f32; 256] {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|i| srgb_to_linear(i as f32 / 255.0)))
}

/// The sRGB byte a coloured frame's linear value was made from (the
/// nearest, for one made otherwise).
fn srgb_byte(linear: f32) -> u32 {
    let table = linear_table();
    let i = table.partition_point(|&v| v < linear);
    if i == 0 {
        0
    } else if i >= table.len() {
        255
    } else if linear - table[i - 1] < table[i] - linear {
        (i - 1) as u32
    } else {
        i as u32
    }
}

/// The words VPX's upscaler hands its scaler for `frame` in `mode`: a
/// brightness as an sRGB byte (in all three channels and opaque, but for
/// ScaleFX), or a colour's sRGB bytes, opaque.
pub fn source_words(frame: &DmdFrame, mode: Mode) -> Vec<u32> {
    let scalefx = matches!(mode, Mode::ScaleFxAa | Mode::ScaleFx3x);
    let n = (frame.width * frame.height) as usize;
    frame.dots[..n.min(frame.dots.len())]
        .iter()
        .map(|d| {
            if frame.luminance {
                let lum = linear_to_gamma_u8(d[0]);
                if scalefx { lum } else { lum | (lum << 8) | (lum << 16) | 0xFF00_0000 }
            } else {
                srgb_byte(d[0]) | (srgb_byte(d[1]) << 8) | (srgb_byte(d[2]) << 16) | 0xFF00_0000
            }
        })
        .collect()
}

/// `src` (`w` × `h` words) through the scaler of `mode`; None when the mode
/// scales nothing (Disabled) or the picture is smaller than 4 × 4.
pub fn scale_words(src: &[u32], w: usize, h: usize, luminance: bool, mode: Mode) -> Option<Vec<u32>> {
    if w < 4 || h < 4 || src.len() < w * h {
        return None;
    }
    Some(match mode {
        Mode::ScaleFxAa => scalefx::upscale(src, w, h, luminance, false),
        Mode::ScaleFx3x => scalefx::upscale(src, w, h, luminance, true),
        Mode::Mmpx2x => mmpx::scale2x(src, w, h),
        Mode::SuperXbr2x => superxbr::scale2x(src, w, h),
        Mode::Xbrz2x | Mode::Xbrz3x | Mode::Xbrz4x | Mode::Xbrz5x | Mode::Xbrz6x => xbrz::scale(mode.factor() as usize, src, w, h)?,
        Mode::Disabled => return None,
    })
}

/// `frame` as VPX 5436's upscaler makes it in `mode`; None when the mode
/// scales nothing (Disabled) or the picture is smaller than 4 × 4.
pub fn upscale(frame: &DmdFrame, mode: Mode) -> Option<DmdFrame> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    if mode == Mode::Disabled || frame.dots.len() < w * h {
        return None;
    }
    let out = scale_words(&source_words(frame, mode), w, h, frame.luminance, mode)?;
    let dots = out
        .iter()
        .map(|&v| {
            if frame.luminance {
                let l = GAMMA_TO_LINEAR[(v & 0xFF) as usize];
                [l, l, l, 1.0]
            } else {
                let c = |shift: u32| srgb_to_linear(((v >> shift) & 0xFF) as f32 / 255.0);
                [c(0), c(8), c(16), 1.0]
            }
        })
        .collect();
    let f = mode.factor();
    Some(DmdFrame { width: frame.width * f, height: frame.height * f, dots, luminance: frame.luminance, rgb565: false, id: frame.id, tint: frame.tint })
}

/// The upscaler at work for the display: the source frame last handed over,
/// and the newest picture made from it.
pub struct Upscaler {
    mode: Mode,
    /// Scaling on a thread of its own (VPX's way, a frame late); None scales at once.
    worker: Option<Worker>,
    /// The source frame last handed over (its number).
    sent: Option<u64>,
    shown: Option<Arc<DmdFrame>>,
}

struct Worker {
    to_worker: Option<Sender<Arc<DmdFrame>>>,
    done: Arc<Mutex<Option<DmdFrame>>>,
    thread: Option<JoinHandle<()>>,
}

impl Upscaler {
    /// The upscaler for `mode`, scaling on a thread of its own unless `at_once`;
    /// None when it scales nothing.
    pub fn new(mode: Mode, at_once: bool) -> Option<Upscaler> {
        if mode == Mode::Disabled {
            return None;
        }
        crate::script::log::once(&format!("the DMD upscaled with {}, as VPX's upscaledmd does", mode.name()));
        let worker = (!at_once).then(|| {
            let (to_worker, from_game) = channel::<Arc<DmdFrame>>();
            let done = Arc::new(Mutex::new(None));
            let slot = done.clone();
            let thread = std::thread::Builder::new().name("DMD upscaler".into()).spawn(move || work(mode, from_game, slot)).ok();
            Worker { to_worker: Some(to_worker), done, thread }
        });
        Some(Upscaler { mode, worker, sent: None, shown: None })
    }

    /// What the display shows in `source`'s place: the newest upscaled picture,
    /// numbered by `next_id` as it arrives — the one made from `source` at once,
    /// or, scaling on the thread, the last one finished, as VPX shows it a frame
    /// late (`source` itself until the first is).
    pub fn shown(&mut self, source: &Arc<DmdFrame>, mut next_id: impl FnMut() -> u64) -> Arc<DmdFrame> {
        let new_source = self.sent != Some(source.id);
        self.sent = Some(source.id);
        match &mut self.worker {
            None => {
                if new_source || self.shown.is_none() {
                    self.shown = upscale(source, self.mode).map(|f| Arc::new(DmdFrame { id: next_id(), ..f }));
                }
            }
            Some(worker) => {
                if new_source && let Some(tx) = &worker.to_worker {
                    let _ = tx.send(source.clone());
                }
                if let Some(f) = worker.done.lock().ok().and_then(|mut d| d.take()) {
                    self.shown = Some(Arc::new(DmdFrame { id: next_id(), ..f }));
                }
            }
        }
        self.shown.clone().unwrap_or_else(|| source.clone())
    }
}

/// The thread: each frame handed over scaled, the newest first when several wait.
fn work(mode: Mode, from_game: Receiver<Arc<DmdFrame>>, done: Arc<Mutex<Option<DmdFrame>>>) {
    while let Ok(mut frame) = from_game.recv() {
        while let Ok(newer) = from_game.try_recv() {
            frame = newer;
        }
        if let Some(scaled) = upscale(&frame, mode)
            && let Ok(mut slot) = done.lock()
        {
            *slot = Some(scaled);
        }
    }
}

impl Drop for Upscaler {
    fn drop(&mut self) {
        if let Some(worker) = &mut self.worker {
            // the thread ends once nothing more can come
            worker.to_worker = None;
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_pick_vpxs_modes() {
        let s = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
        assert_eq!(Mode::from_settings(&[]), Mode::Disabled);
        assert_eq!(Mode::from_settings(&s(&[("UpscaleMode", "2")])), Mode::ScaleFx3x);
        assert_eq!(Mode::from_settings(&s(&[("upscalemode", "9")])), Mode::SuperXbr2x);
        assert_eq!(Mode::from_settings(&s(&[("UpscaleMode", "3"), ("Enable", "0")])), Mode::Disabled);
        assert_eq!(Mode::from_settings(&s(&[("UpscaleMode", "12")])), Mode::Disabled);
    }

    #[test]
    fn coloured_dots_give_back_their_bytes() {
        for b in 0..=255u32 {
            assert_eq!(srgb_byte(srgb_to_linear(b as f32 / 255.0)), b);
        }
    }

    #[test]
    fn brightness_goes_round_through_vpxs_table() {
        // VPX's table undoes its rounding, byte for byte
        for b in 0..=255u32 {
            assert_eq!(linear_to_gamma_u8(GAMMA_TO_LINEAR[b as usize]), b, "byte {b}");
        }
    }

    #[test]
    fn modes_scale_the_picture() {
        // a diagonal line on a 16 x 8 display
        let dots = (0..16 * 8).map(|i| if i % 16 == (i / 16) * 2 { [1.0, 1.0, 1.0, 1.0] } else { [0.0, 0.0, 0.0, 1.0] });
        let frame = DmdFrame::with_dots(16, 8, dots, true, 7);
        for mode in Mode::ALL[1..].iter().copied() {
            let f = upscale(&frame, mode).unwrap();
            assert_eq!((f.width, f.height), (16 * mode.factor(), 8 * mode.factor()), "{}", mode.name());
            assert_eq!(f.dots.len(), (f.width * f.height) as usize);
            // (the anti-aliasing averages a thin line's dots down)
            assert!(f.luminance && f.dots.iter().any(|d| d[0] > 0.3), "{}", mode.name());
        }
        assert!(upscale(&frame, Mode::Disabled).is_none());
    }

    #[test]
    fn the_thread_shows_a_frame_late() {
        let frame = Arc::new(DmdFrame::with_dots(16, 8, (0..128).map(|i| [(i % 3) as f32 / 2.0, 0.0, 0.0, 1.0]), true, 1));
        let mut ids = 100;
        let mut at_once = Upscaler::new(Mode::Mmpx2x, true).unwrap();
        let first = at_once.shown(&frame, || {
            ids += 1;
            ids
        });
        assert_eq!((first.width, first.id), (32, 101));
        let mut late = Upscaler::new(Mode::Mmpx2x, false).unwrap();
        // the source until the first picture is made
        assert_eq!(late.shown(&frame, || 1).width, 16);
        let begun = std::time::Instant::now();
        while late.shown(&frame, || 2).width != 32 {
            assert!(begun.elapsed().as_secs() < 5);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
