// VPX's player settings and its sun-position code, ported: Visual Pinball X build 5436
// (10.8.1 pre-release), src/core/Settings_properties.inl (added 2025 by VPX's team, GPL by
// VPX's licence), src/core/player.cpp, src/renderer/Renderer.cpp and
// src/utils/wintimer.cpp (license:GPLv3+).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX 5436's player settings, under its own names, for what the player
//! already does. Each reader returns `None` when the settings don't say, or say
//! something VPX wouldn't take, so our own default holds; the player's
//! command-line options still count over them (`main.rs`). Numbers are
//! clamped to VPX's ranges as VPX clamps them; a choice out of range counts
//! as unsaid.

use table_physics::settings::{FlipperPhysicsSet, PhysicsSettings, TablePhysicsSet};

use crate::render::{AmbientOcclusion, PostAa, Presentation, Sharpen};
use crate::scoreview::DotProfile;
use crate::segments::SegProfile;
use crate::settings::{Settings, vpx_bool, vpx_float, vpx_int};

fn choice(s: &Settings, section: &str, name: &str, count: i32) -> Option<i32> {
    s.get(section, name).and_then(vpx_int).filter(|v| (0..count).contains(v))
}

fn int(s: &Settings, section: &str, name: &str, min: i32, max: i32) -> Option<i32> {
    s.get(section, name).and_then(vpx_int).map(|v| v.clamp(min, max))
}

fn float(s: &Settings, section: &str, name: &str, min: f32, max: f32) -> Option<f32> {
    s.get(section, name).and_then(vpx_float).map(|v| v.clamp(min, max))
}

fn flag(s: &Settings, section: &str, name: &str) -> Option<bool> {
    s.get(section, name).and_then(vpx_bool)
}

/// How frames reach the screen: `[VPI]` `Present`, a setting of ours with the
/// player's own choices — `immediate`, `mailbox` (tear-free and quick, which
/// VPX hasn't), `vsync` or `paced` — counting over `[Player]` `SyncMode`:
/// VPX's no sync, vertical sync, adaptive sync or frame pacing. SDL's
/// graphics have no adaptive sync; it gets the display's queue, which is what
/// VPX's adaptive sync does while frames come in time.
pub fn presentation(s: &Settings) -> Option<Presentation> {
    let ours = s.get("VPI", "Present").map(|v| crate::settings::vpx_trim(v).to_ascii_lowercase());
    match ours.as_deref() {
        Some("immediate") => return Some(Presentation::Immediate),
        Some("mailbox") => return Some(Presentation::TearFree),
        Some("vsync") => return Some(Presentation::Queued),
        Some("paced") => return Some(Presentation::Paced),
        _ => {}
    }
    Some(match choice(s, "Player", "SyncMode", 4)? {
        0 => Presentation::Immediate,
        1 | 2 => Presentation::Queued,
        _ => Presentation::Paced,
    })
}

/// `[Player]` `MaxFramerate`, -1 to 1000 (-1: the screen's refresh rate; 0:
/// no limit).
pub fn max_framerate(s: &Settings) -> Option<f32> {
    float(s, "Player", "MaxFramerate", -1.0, 1000.0)
}

/// The frame-rate limit `MaxFramerate` gives, as VPX 5436 works it out
/// against the playfield screen's refresh rate (`Player::Player`): at least
/// 24, the refresh rate below 0, none at 0; with a sync mode, no faster than
/// the refresh rate. VPX means a lower limit under sync to become a whole
/// fraction of the refresh rate, but its loop never runs for one (it tests
/// the limit times the divider *above* the refresh rate), so such a limit
/// becomes the refresh rate itself; kept as VPX does it. Frames a second, 0
/// for no limit.
pub fn frame_limit(setting: f32, refresh: f32, synced: bool) -> f64 {
    let mut max = setting;
    if max > 0.0 && max < 24.0 {
        max = 24.0;
    }
    if max < 0.0 {
        max = refresh;
    }
    if max == 0.0 {
        max = 10_000.0;
    }
    if synced && refresh > 0.0 {
        if max > refresh {
            max = refresh;
        } else if max < refresh {
            let mut divider = 1.0f32;
            while max * divider > refresh && 24.0 * divider <= refresh {
                divider += 1.0;
            }
            max = refresh / divider;
        }
    }
    if max <= 0.0 || max >= 10_000.0 { 0.0 } else { f64::from(max) }
}

/// `[Player]` `MSAASamples`: none, 4, 6 or 8 samples a pixel. SDL's graphics
/// take 2, 4 or 8, so 6 draws with 8; VPX 5436's BGFX build has no 6 either,
/// and draws such a target without multisampling (`RenderTarget`'s switch),
/// the label's meaning kept here instead.
pub fn samples(s: &Settings) -> Option<u32> {
    Some(match choice(s, "Player", "MSAASamples", 4)? {
        0 => 1,
        1 => 4,
        _ => 8,
    })
}

/// `[Player]` `AAFactor`: VPX's supersampling, 0.5 to 2.
pub fn aa_factor(s: &Settings) -> Option<f32> {
    float(s, "Player", "AAFactor", 0.5, 2.0)
}

/// `[Player]` `FXAA`: VPX's post-processed antialiasing, in its menu's order.
pub fn post_aa(s: &Settings) -> Option<PostAa> {
    Some(match choice(s, "Player", "FXAA", 8)? {
        0 => PostAa::Off,
        1 => PostAa::FxaaFast,
        2 => PostAa::Fxaa,
        3 => PostAa::FxaaQuality,
        4 => PostAa::Nfaa,
        5 => PostAa::Dlaa,
        6 => PostAa::Smaa,
        _ => PostAa::Faaa,
    })
}

/// `[Player]` `Sharpen`: none, CAS or bilateral CAS.
pub fn sharpen(s: &Settings) -> Option<Sharpen> {
    Some(match choice(s, "Player", "Sharpen", 3)? {
        0 => Sharpen::Off,
        1 => Sharpen::Cas,
        _ => Sharpen::BilateralCas,
    })
}

/// `[Player]` `DisableAO` and `DynamicAO`: VPX's ambient occlusion off, made
/// once into the still parts, or every frame (VPX's defaults: on, dynamic).
pub fn ambient_occlusion(s: &Settings) -> Option<AmbientOcclusion> {
    let disable = flag(s, "Player", "DisableAO");
    let dynamic = flag(s, "Player", "DynamicAO");
    if disable.is_none() && dynamic.is_none() {
        return None;
    }
    Some(if disable == Some(true) {
        AmbientOcclusion::Off
    } else if dynamic.unwrap_or(true) {
        AmbientOcclusion::Dynamic
    } else {
        AmbientOcclusion::Static
    })
}

/// `[Player]` `ForceMotionBlurOff`: the balls' motion blur, on unless set.
pub fn motion_blur(s: &Settings) -> Option<bool> {
    flag(s, "Player", "ForceMotionBlurOff").map(|off| !off)
}

/// `[Player]` `ForceBloomOff`: no bloom, whatever the table's strength.
pub fn bloom_off(s: &Settings) -> bool {
    flag(s, "Player", "ForceBloomOff").unwrap_or(false)
}

/// `[Player]` `ShowFPS`: VPX's performance readout off, frame rate or full;
/// ours has one readout, shown for either of VPX's.
pub fn readout(s: &Settings) -> Option<bool> {
    choice(s, "Player", "ShowFPS", 3).map(|v| v != 0)
}

/// `[Player]` `ForceAnisotropicFiltering`: pictures read anisotropically (VPX's
/// default) or, off, trilinearly (`Renderer::SetAnisoFiltering`).
pub fn anisotropic(s: &Settings) -> bool {
    flag(s, "Player", "ForceAnisotropicFiltering").unwrap_or(true)
}

/// `[Player]` `PFReflection`: the most reflections VPX draws in a mirror
/// (`RenderProbe::ReflectionMode`): none, balls only, the still parts only,
/// still parts and balls, still and moving parts unsynced, everything (its
/// default).
pub fn reflection_mode(s: &Settings) -> u8 {
    choice(s, "Player", "PFReflection", 6).map_or(5, |v| v as u8)
}

/// `[Player]` `NudgeStrength`: how far a nudge shakes the picture, 0 to 2
/// (VPX's visual nudge strength, 1 by default).
pub fn visual_nudge_strength(s: &Settings) -> f32 {
    float(s, "Player", "NudgeStrength", 0.0, 2.0).unwrap_or(1.0)
}

/// `[Player]` `PlungerRetract`: a plunger pulled by a key or button eases
/// forward again after a second (VPX's `PullBackandRetract` in `PullBack`'s
/// place).
pub fn plunger_retract(s: &Settings) -> bool {
    flag(s, "Player", "PlungerRetract").unwrap_or(false)
}

/// The settings that change a table's colliders: VPX's detail level
/// (`[Player]` `AlphaRampAccuracy`, 1 to 10), the player's difficulty for the
/// table (`[TableOverride]` `Difficulty`, 0 to 1, else the table's own), and
/// VPX's eight physics sets (`[Player]` `FlipperPhysics…0` to `7`,
/// `TablePhysics…0` to `7`, unbounded as VPX keeps them), which a table or a
/// flipper chooses by number.
pub fn physics(s: &Settings) -> PhysicsSettings {
    let mut p = PhysicsSettings::default();
    if let Some(level) = detail_level(s) {
        p.detail_level = level;
    }
    // hardcore: the table's own difficulty, and VPX's physics sets as VPX ships them
    let hardcore = crate::achievements::hardcore_play();
    p.difficulty = if hardcore { None } else { float(s, "TableOverride", "Difficulty", 0.0, 1.0) };
    let value = |name: &str, i: usize, default: f32| if hardcore { default } else { s.get("Player", &format!("{name}{i}")).and_then(vpx_float).unwrap_or(default) };
    for i in 0..8 {
        let d = FlipperPhysicsSet::default();
        p.flipper_sets[i] = FlipperPhysicsSet {
            mass: value("FlipperPhysicsMass", i, d.mass),
            strength: value("FlipperPhysicsStrength", i, d.strength),
            elasticity: value("FlipperPhysicsElasticity", i, d.elasticity),
            scatter: value("FlipperPhysicsScatter", i, d.scatter),
            return_strength: value("FlipperPhysicsReturnStrength", i, d.return_strength),
            elasticity_falloff: value("FlipperPhysicsElasticityFalloff", i, d.elasticity_falloff),
            friction: value("FlipperPhysicsFriction", i, d.friction),
            coil_ramp_up: value("FlipperPhysicsCoilRampUp", i, d.coil_ramp_up),
            eos_torque: value("FlipperPhysicsEOSTorque", i, d.eos_torque),
            eos_torque_angle: value("FlipperPhysicsEOSTorqueAngle", i, d.eos_torque_angle),
        };
        let d = TablePhysicsSet::default();
        p.table_sets[i] = TablePhysicsSet {
            gravity_constant: value("TablePhysicsGravityConstant", i, d.gravity_constant),
            contact_friction: value("TablePhysicsContactFriction", i, d.contact_friction),
            elasticity: value("TablePhysicsElasticity", i, d.elasticity),
            elasticity_falloff: value("TablePhysicsElasticityFalloff", i, d.elasticity_falloff),
            scatter_angle: value("TablePhysicsScatterAngle", i, d.scatter_angle),
            contact_scatter_angle: value("TablePhysicsContactScatterAngle", i, d.contact_scatter_angle),
            min_slope: value("TablePhysicsMinSlope", i, d.min_slope),
            max_slope: value("TablePhysicsMaxSlope", i, d.max_slope),
        };
    }
    p
}

/// `[Player]` `AlphaRampAccuracy`: VPX's detail level for rubbers, ramps and
/// balls, 1 to 10 (10, its finest, by default).
pub fn detail_level(s: &Settings) -> Option<i32> {
    int(s, "Player", "AlphaRampAccuracy", 1, 10)
}

/// `[TableOverride]` `Exposure`: the player's exposure for the table, 0 to 2,
/// over the table's own.
pub fn exposure(s: &Settings) -> Option<f32> {
    float(s, "TableOverride", "Exposure", 0.0, 2.0)
}

/// `[TableOverride]` `ToneMapper`: the player's tone mapper for the table
/// (Reinhard, AgX, Filmic, Neutral, AgX punchy), over the table's own.
pub fn tone_mapper(s: &Settings) -> Option<u32> {
    choice(s, "TableOverride", "ToneMapper", 5).map(|v| v as u32)
}

/// The scene's light level, 0 to 1, as VPX 5436's `SceneLighting` chooses it
/// when play starts: the table's own (`table`), unless `[Player]`
/// `OverrideTableEmissionScale` says the player's — `EmissionScale`, or, with
/// `DynamicDayNight`, the sun's at this hour where the player lives
/// (`Latitude`, `Longitude`).
pub fn light_level(s: &Settings, table: f32) -> f32 {
    if !flag(s, "Player", "OverrideTableEmissionScale").unwrap_or(false) {
        return table;
    }
    if flag(s, "Player", "DynamicDayNight").unwrap_or(false) {
        let latitude = float(s, "Player", "Latitude", -90.0, 90.0).unwrap_or(52.52);
        let longitude = float(s, "Player", "Longitude", -180.0, 180.0).unwrap_or(13.37);
        sun::light_level(&sun::Clock::now(), latitude, longitude)
    } else {
        float(s, "Player", "EmissionScale", 0.0, 1.0).unwrap_or(1.0)
    }
}

/// VPX's seven DMD profiles (`[DMD]` `Profile1…` to `Profile7…`: `Legacy`,
/// `DotTint`, `UnlitDotColor`, `DotBrightness`, `DotSize`, `DotSharpness`,
/// `DiffuseGlow`), each value the settings' else build 5436's default.
pub fn dmd_profiles(s: &Settings) -> [DotProfile; 7] {
    let mut profiles = crate::scoreview::PROFILES;
    for (i, p) in profiles.iter_mut().enumerate() {
        let name = |key: &str| format!("Profile{}{key}", i + 1);
        p.legacy = flag(s, "DMD", &name("Legacy")).unwrap_or(p.legacy);
        p.tint = int(s, "DMD", &name("DotTint"), 0, 0xFF_FFFF).map_or(p.tint, |v| v as u32);
        p.unlit = int(s, "DMD", &name("UnlitDotColor"), 0, 0xFF_FFFF).map_or(p.unlit, |v| v as u32);
        p.brightness = float(s, "DMD", &name("DotBrightness"), 0.001, 100.0).unwrap_or(p.brightness);
        p.size = float(s, "DMD", &name("DotSize"), 0.001, 1.0).unwrap_or(p.size);
        p.sharpness = float(s, "DMD", &name("DotSharpness"), 0.0, 1.0).unwrap_or(p.sharpness);
        p.glow = float(s, "DMD", &name("DiffuseGlow"), 0.0, 10.0).unwrap_or(p.glow);
    }
    profiles
}

/// VPX's eight segment display profiles (`[Alpha]` `Profile1…` to
/// `Profile8…`: `Color`, `Unlit`, `Brightness`, `DiffuseGlow`), likewise.
pub fn segment_profiles(s: &Settings) -> [SegProfile; 8] {
    let mut profiles = crate::segments::PROFILES;
    for (i, p) in profiles.iter_mut().enumerate() {
        let name = |key: &str| format!("Profile{}{key}", i + 1);
        p.color = int(s, "Alpha", &name("Color"), 0, 0xFF_FFFF).map_or(p.color, |v| v as u32);
        p.unlit = int(s, "Alpha", &name("Unlit"), 0, 0xFF_FFFF).map_or(p.unlit, |v| v as u32);
        p.brightness = float(s, "Alpha", &name("Brightness"), 0.001, 100.0).unwrap_or(p.brightness);
        p.glow = float(s, "Alpha", &name("DiffuseGlow"), 0.0, 10.0).unwrap_or(p.glow);
    }
    profiles
}

/// `[Plugin.B2S]` `ShowGrill`, else the legacy B2S plugin's `[Plugin.B2SLegacy]`
/// `B2SHideGrill`: the backglass picture's speaker grill shown.
pub fn show_grill(s: &Settings) -> Option<bool> {
    flag(s, "Plugin.B2S", "ShowGrill").or_else(|| flag(s, "Plugin.B2SLegacy", "B2SHideGrill").map(|hide| !hide))
}

/// The DMD over a backglass file's picture, as VPX 5436's B2S plugin reads it
/// (`B2SDMDOverlay::LoadSettings`): `[Plugin.B2S]` `<window>DMDOverlay` off, or on
/// with `<window>DMDAutoPos` finding its place, or at `<window>DMDX`, `…Y`, `…W`,
/// `…H`; `window` is `ScoreView` (over the score panel) or `Backglass`.
pub fn b2s_dmd_overlay(s: &Settings, window: &str) -> Option<crate::dmd_overlay::Overlay> {
    use crate::dmd_overlay::Overlay;
    let key = |k: &str| format!("{window}DMD{k}");
    if ["Overlay", "AutoPos", "X", "Y", "W", "H"].iter().all(|k| s.get("Plugin.B2S", &key(k)).is_none()) {
        return None;
    }
    if !flag(s, "Plugin.B2S", &key("Overlay")).unwrap_or(false) {
        return Some(Overlay::Off);
    }
    if flag(s, "Plugin.B2S", &key("AutoPos")).unwrap_or(false) {
        return Some(Overlay::Auto);
    }
    let v = |k: &str| int(s, "Plugin.B2S", &key(k), 0, 0xFFFF).unwrap_or(0);
    Some(Overlay::At([v("X"), v("Y"), v("W"), v("H")]))
}

/// `[Plugin.B2SLegacy]` `B2SHideB2SDMD`: the backglass file's score panel left out
/// of the score window, as B2S Server's "hide B2S DMD" leaves it out.
pub fn hide_score_panel(s: &Settings) -> bool {
    flag(s, "Plugin.B2SLegacy", "B2SHideB2SDMD").unwrap_or(false)
}

/// The ball's pictures in place of the table's (`[Player]`
/// `OverwriteBallImage`, with `BallImage` and `DecalImage`, each a picture
/// file), as VPX 5436 swaps them in for balls that take the table's
/// settings: its environment picture then read as a sphere map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BallOverride {
    pub image: Option<std::path::PathBuf>,
    pub decal: Option<std::path::PathBuf>,
}

pub fn ball_override(s: &Settings) -> BallOverride {
    if !flag(s, "Player", "OverwriteBallImage").unwrap_or(false) {
        return BallOverride::default();
    }
    let file = |name: &str| s.get("Player", name).map(str::trim).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    BallOverride { image: file("BallImage"), decal: file("DecalImage") }
}

/// `[Player]` `DisableLightingForBalls`: balls drawn without lighting or
/// reflections (VPX's help for the visually impaired).
pub fn ball_lighting_off(s: &Settings) -> bool {
    flag(s, "Player", "DisableLightingForBalls").unwrap_or(false)
}

/// `[Player]` `BallAntiStretch`: a ball's picture squeezed back against the
/// view's stretching.
pub fn ball_anti_stretch(s: &Settings) -> bool {
    flag(s, "Player", "BallAntiStretch").unwrap_or(false)
}

/// VPX's sun: how bright the day is now, where the player lives
/// (`Renderer::SceneLighting::Update`'s day and night mode, from
/// `wintimer.cpp`).
pub mod sun {
    use std::f64::consts::PI;

    const DAYS_PER_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    /// The local date and time, and how many hours local time is ahead of
    /// universal time (VPX's `LocalTimeAdjust`: the hours' difference, -12 to
    /// 12, minutes ignored as VPX ignores them).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Clock {
        pub year: u32,
        pub month: u32,
        pub day: u32,
        pub hour: u32,
        pub minute: u32,
        pub local_adjust: f64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
        fn GetSystemTime(time: *mut SystemTime);
    }

    impl Clock {
        pub fn now() -> Clock {
            let (mut local, mut utc) = (SystemTime::default(), SystemTime::default());
            // SAFETY: both fill the structures they're given
            unsafe {
                GetLocalTime(&mut local);
                GetSystemTime(&mut utc);
            }
            let dif = i32::from(local.hour) - i32::from(utc.hour);
            Clock {
                year: local.year.into(),
                month: local.month.into(),
                day: local.day.into(),
                hour: local.hour.into(),
                minute: local.minute.into(),
                local_adjust: f64::from(if dif < -12 { dif + 24 } else { dif }),
            }
        }
    }

    fn angle_of_day(day: u32, month: u32, year: u32) -> f64 {
        let leap = year % 400 == 0 || (year % 100 != 0 && year % 4 == 0);
        let total = if leap { 366.0 } else { 365.0 };
        let mut days: u32 = (1..month).map(|m| DAYS_PER_MONTH[(m - 1) as usize]).sum();
        if month > 2 && leap {
            days += 1;
        }
        days += day;
        (2.0 * PI * (f64::from(days) - 1.0)) / total
    }

    fn solar_declination(a: f64) -> f64 {
        let (c, s) = (a.cos(), a.sin());
        0.006918 - 0.399912 * c + 0.070257 * s - 0.006758 * (2.0 * c * c - 1.0) + 0.000907 * 2.0 * c * s - 0.002697 * (c * (4.0 * c * c - 3.0))
            + 0.00148 * (s * (-4.0 * s * s + 3.0))
    }

    fn equation_of_time(a: f64) -> f64 {
        let (c, s) = (a.cos(), a.sin());
        0.000075 + 0.001868 * c - 0.032077 * s - 0.014615 * (2.0 * c * c - 1.0) - 0.04089 * 2.0 * c * s
    }

    fn day_duration_half(declination: f64, rlat: f64) -> f64 {
        (-rlat.tan() * declination.tan()).acos()
    }

    /// The hour of sunrise or sunset, local time.
    fn sunrise_sunset(clock: &Clock, rlong: f64, rlat: f64, sunrise: bool) -> f64 {
        let a = angle_of_day(clock.day, clock.month, clock.year);
        let hours = (day_duration_half(solar_declination(a), rlat) * (24.0 / PI)).abs();
        let half = if sunrise { -0.5 } else { 0.5 };
        12.0 + half * hours - (rlong + equation_of_time(a)) * (12.0 / PI) + clock.local_adjust
    }

    fn orbital_excentricity(a: f64) -> f64 {
        let (c, s) = (a.cos(), a.sin());
        1.000110 + 0.034221 * c + 0.001280 * s + 0.000719 * (2.0 * c * c - 1.0) + 0.000077 * 2.0 * c * s
    }

    /// The day's theoretical radiation, in W/m².
    fn radiation(day: u32, month: u32, year: u32, rlat: f64) -> f64 {
        let a = angle_of_day(day, month, year);
        let declination = solar_declination(a);
        let e0 = orbital_excentricity(a);
        let sunrise = day_duration_half(declination, rlat);
        let (c0, c1) = ((declination - rlat).cos(), (declination + rlat).cos());
        0.5 * 1367.0 * e0 * ((c0 + c1) * sunrise.sin() / sunrise + c0 - c1)
    }

    /// The year's highest, as VPX searches it: its months counted from 0 and
    /// its days from 0, as VPX's loop does.
    fn max_radiation(year: u32, rlat: f64) -> f64 {
        let mut max = 0.0f64;
        for month in 0..12u32 {
            for day in 0..DAYS_PER_MONTH[month as usize] {
                let r = radiation(day, month, year, rlat);
                if r > max {
                    max = r;
                }
            }
        }
        max
    }

    /// The light level, 0.15 to 1: the sun's arc between sunrise and sunset,
    /// dimmed for shorter days.
    pub fn light_level(clock: &Clock, latitude: f32, longitude: f32) -> f32 {
        let (rlat, rlong) = (f64::from(latitude).to_radians(), f64::from(longitude).to_radians());
        let tr = radiation(clock.day, clock.month, clock.year, rlat);
        let max_tr = max_radiation(clock.year, rlat);
        let sunset = sunrise_sunset(clock, rlong, rlat, false);
        let sunrise = sunrise_sunset(clock, rlong, rlat, true);
        let now = f64::from(clock.hour) + f64::from(clock.minute) / 60.0;
        let factor = ((PI * ((now - sunrise) / (sunset - sunrise)).clamp(0.0, 1.0)).sin() * (tr / max_tr).sqrt()) as f32;
        factor.clamp(0.15, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Settings {
        Settings::from_text(text)
    }

    #[test]
    fn nothing_said_keeps_ours() {
        let none = s("");
        assert_eq!(presentation(&none), None);
        assert_eq!(samples(&none), None);
        assert_eq!(post_aa(&none), None);
        assert_eq!(ambient_occlusion(&none), None);
        assert_eq!(motion_blur(&none), None);
        assert_eq!(readout(&none), None);
        assert!(!bloom_off(&none) && anisotropic(&none) && !plunger_retract(&none));
        assert_eq!(reflection_mode(&none), 5);
        assert_eq!(visual_nudge_strength(&none), 1.0);
        assert_eq!(physics(&none), PhysicsSettings::default());
        assert_eq!(light_level(&none, 0.4), 0.4);
        assert_eq!(dmd_profiles(&none), crate::scoreview::PROFILES);
        assert_eq!(segment_profiles(&none), crate::segments::PROFILES);
        assert_eq!(ball_override(&none), BallOverride::default());
    }

    #[test]
    fn vpx_names_and_choices() {
        let t = s("[Player]\nSyncMode = 3\nMSAASamples = 2\nFXAA = 6\nSharpen = 2\nDisableAO = 0\nDynamicAO = 0\nForceMotionBlurOff = 1\nShowFPS = 0\nAAFactor = 3\nPFReflection = 1\n");
        assert_eq!(presentation(&t), Some(Presentation::Paced));
        assert_eq!(samples(&t), Some(8));
        assert_eq!(post_aa(&t), Some(PostAa::Smaa));
        assert_eq!(sharpen(&t), Some(Sharpen::BilateralCas));
        assert_eq!(ambient_occlusion(&t), Some(AmbientOcclusion::Static));
        assert_eq!(motion_blur(&t), Some(false));
        assert_eq!(readout(&t), Some(false));
        // clamped as VPX clamps, a choice out of range unsaid
        assert_eq!(aa_factor(&t), Some(2.0));
        assert_eq!(presentation(&s("[Player]\nSyncMode = 9\n")), None);
        // ours, with the tear-free quick way VPX hasn't, over VPX's
        assert_eq!(presentation(&s("[Player]\nSyncMode = 3\n[VPI]\nPresent = Mailbox\n")), Some(Presentation::TearFree));
        assert_eq!(presentation(&s("[Player]\nSyncMode = 3\n[VPI]\nPresent = sometimes\n")), Some(Presentation::Paced));
        assert_eq!(reflection_mode(&t), 1);
        assert_eq!(ambient_occlusion(&s("[Player]\nDisableAO = 1\n")), Some(AmbientOcclusion::Off));
        assert_eq!(ambient_occlusion(&s("[Player]\nDynamicAO = 1\n")), Some(AmbientOcclusion::Dynamic));
    }

    #[test]
    fn frame_limit_as_vpx_works_it_out() {
        // -1: the refresh rate; 0: none; at least 24
        assert_eq!(frame_limit(-1.0, 144.0, false), 144.0);
        assert_eq!(frame_limit(0.0, 144.0, false), 0.0);
        assert_eq!(frame_limit(10.0, 144.0, false), 24.0);
        // under sync: never above the refresh rate, and a lower limit becomes it too (VPX's loop)
        assert_eq!(frame_limit(300.0, 144.0, true), 144.0);
        assert_eq!(frame_limit(60.0, 144.0, true), 144.0);
        assert_eq!(frame_limit(0.0, 60.0, true), 60.0);
    }

    #[test]
    fn physics_sets_by_number() {
        let t = s("[Player]\nFlipperPhysicsStrength2 = 3000\nTablePhysicsMinSlope7 = 4.5\nAlphaRampAccuracy = 0\n[TableOverride]\nDifficulty = 0.25\n");
        let p = physics(&t);
        assert_eq!(p.flipper_sets[2].strength, 3000.0);
        assert_eq!(p.flipper_sets[1].strength, 2200.0);
        assert_eq!(p.table_sets[7].min_slope, 4.5);
        assert_eq!(p.detail_level, 1);
        assert_eq!(p.difficulty, Some(0.25));
    }

    #[test]
    fn light_level_modes() {
        assert_eq!(light_level(&s("[Player]\nEmissionScale = 0.3\n"), 0.8), 0.8);
        assert_eq!(light_level(&s("[Player]\nOverrideTableEmissionScale = 1\nEmissionScale = 0.3\n"), 0.8), 0.3);
        // the sun: dark at midnight (VPX's floor), bright at noon in summer
        let at = |hour| sun::Clock { year: 2026, month: 6, day: 21, hour, minute: 0, local_adjust: 2.0 };
        assert_eq!(sun::light_level(&at(0), 52.52, 13.37), 0.15);
        let noon = sun::light_level(&at(13), 52.52, 13.37);
        assert!(noon > 0.9, "{noon}");
    }

    #[test]
    fn b2s_settings() {
        use crate::dmd_overlay::Overlay;
        let none = s("");
        assert_eq!((show_grill(&none), b2s_dmd_overlay(&none, "ScoreView"), hide_score_panel(&none)), (None, None, false));
        let t = s("[Plugin.B2S]\nShowGrill = 1\nScoreViewDMDOverlay = 1\nScoreViewDMDAutoPos = 1\nBackglassDMDOverlay = 1\nBackglassDMDX = 10\nBackglassDMDY = 20\nBackglassDMDW = 300\nBackglassDMDH = 80\n");
        assert_eq!(show_grill(&t), Some(true));
        assert_eq!(b2s_dmd_overlay(&t, "ScoreView"), Some(Overlay::Auto));
        assert_eq!(b2s_dmd_overlay(&t, "Backglass"), Some(Overlay::At([10, 20, 300, 80])));
        assert_eq!(b2s_dmd_overlay(&s("[Plugin.B2S]\nScoreViewDMDOverlay = 0\n"), "ScoreView"), Some(Overlay::Off));
        // the legacy plugin's, when the newer one's grill isn't set
        let legacy = s("[Plugin.B2SLegacy]\nB2SHideGrill = 1\nB2SHideB2SDMD = 1\n");
        assert_eq!(show_grill(&legacy), Some(false));
        assert!(hide_score_panel(&legacy));
    }

    #[test]
    fn profiles_from_settings() {
        let t = s("[DMD]\nProfile2DotTint = 255\nProfile2DotSize = 0.5\n[Alpha]\nProfile3Brightness = 7\n");
        let dmd = dmd_profiles(&t);
        assert_eq!((dmd[1].tint, dmd[1].size), (255, 0.5));
        assert_eq!(dmd[0], crate::scoreview::PROFILES[0]);
        assert_eq!(segment_profiles(&t)[2].brightness, 7.0);
    }
}
