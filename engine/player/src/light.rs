// Ported from Visual Pinball X: src/parts/light.cpp and light.h (license:GPLv3+), build 5436
// (10.8.1 pre-release, commit af26b2d): the light's play-time state (clampLightState,
// setInPlayState, UpdateAnimation, the start of RenderSetup, the blinker) and its script members
// State, Intensity, IntensityScale, BlinkPattern, BlinkInterval, Duration, GetInPlayState,
// GetInPlayStateBool, GetInPlayIntensity, FilamentTemperature, X, Y, BulbHaloHeight,
// BulbModulateVsAdd and TransmissionScale.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A light while the table plays, as VPX runs it: its state (off,
//! on, anything between, or blinking), its blink pattern, a timed state
//! (`Duration`), and how its brightness follows the state: at once, in a
//! straight ramp, or as a bulb's filament heats and cools.

use vpin::vpx::color::Color;
use vpin::vpx::gameitem::light::{Fader, Light};

use crate::bulb;

/// VPX's `LightStateBlinking`.
pub const BLINKING: f32 = 2.0;

/// VPX's `FADER_INCANDESCENT`: the light follows a bulb's filament.
pub const FADER_INCANDESCENT: i32 = 2;

/// VPX's `clampLightState`: a state is 0 to 1, or 2 for blinking. Old
/// tables set 255 or `True` (-1) for on.
pub fn clamp_state(state: f32) -> f32 {
    if state < 0.0 {
        return 1.0;
    }
    if state == BLINKING {
        return BLINKING;
    }
    if state > 1.0 {
        return 1.0;
    }
    state
}

/// A colour as VPX keeps it (`COLORREF`: red in the low byte).
pub fn colorref(c: &Color) -> u32 {
    u32::from(c.r) | u32::from(c.g) << 8 | u32::from(c.b) << 16
}

/// One light's state while playing.
pub struct LightRt {
    /// `m_d.m_state`: the state the script last set, which a light
    /// sequencer puts back when it lets go.
    pub state: f32,
    /// `m_inPlayState`: the state the light shows.
    pub in_play_state: f32,
    pub intensity: f32,
    pub intensity_scale: f32,
    pub fade_speed_up: f32,
    pub fade_speed_down: f32,
    /// 0 none, 1 linear, 2 incandescent (VPX's `Fader`); a script may set
    /// any number, which then stops the fading.
    pub fader: i32,
    pub blink_pattern: Vec<u8>,
    pub blink_interval: i32,
    pub color: u32,
    pub color2: u32,
    /// `m_currentIntensity`: how bright it is now.
    pub current_intensity: f32,
    pub filament_temperature: f64,
    /// A light sequencer is running it.
    pub locked_by_ls: bool,
    /// Where its glow is brightest (`m_d.m_vCenter`, which scripts move: the halo's shape stays
    /// where the table put it, its brightness centred here).
    pub center: [f32; 2],
    /// A bulb's halo height above its surface (`m_d.m_bulbHaloHeight`).
    pub halo_height: f32,
    /// How much a bulb brightens what's below rather than adds (`m_d.m_modulate_vs_add`).
    pub modulate_vs_add: f32,
    /// How much of a bulb's light passes up through see-through parts (`m_d.m_transmissionScale`).
    pub transmission: f32,
    /// A bulb light rather than a classic one (`m_d.m_BulbLight`), as the script last set it; drawn
    /// as the table loaded it (VPX draws a light switched during play the other way at once).
    pub bulb: bool,
    /// How far its glow reaches, and how it fades (`m_d.m_falloff`, `m_d.m_falloff_power`).
    pub falloff: f32,
    pub falloff_power: f32,
    /// Where its halo and bulb sort among see-through parts (`m_d.m_depthBias`).
    pub depth_bias: f32,
    /// Its bulb's size (`m_d.m_meshRadius`): VPX makes the bulb's shape before play, so a size
    /// the script sets is kept and nothing more.
    pub mesh_radius: f32,
    /// Whether it shines on balls (`m_d.m_showReflectionOnBall`) and casts their shadows
    /// (`m_d.m_shadows`): VPX reads both as it draws each frame, so a script may switch them.
    pub on_ball: bool,
    pub ball_shadows: bool,
    /// Whether its bulb and socket show (`m_d.m_showBulbMesh`), read as VPX draws each frame; the
    /// pieces exist only when the file shows them, as VPX makes them only then.
    pub show_bulb_mesh: bool,
    duration: i32,
    final_state: f32,
    time_next_blink: u32,
    duration_end: u32,
    blink_frame: u32,
}

impl LightRt {
    /// The light as the table file sets it, then VPX's `RenderSetup` at
    /// play time 0, before the script starts.
    pub fn new(l: &Light) -> LightRt {
        // 10.8 tables store the state as a number (STTF); older ones as
        // 0, 1 or 2 (STAT), which VPX reads as a signed number
        let state = clamp_state(l.state.unwrap_or(l.state_u32 as i32 as f32));
        let mut rt = LightRt {
            state,
            in_play_state: state,
            intensity: l.intensity,
            intensity_scale: 1.0,
            fade_speed_up: l.fade_speed_up,
            fade_speed_down: l.fade_speed_down,
            fader: match l.fader {
                Some(Fader::None) => 0,
                Some(Fader::Incandescent) => 2,
                Some(Fader::Linear) | None => 1,
                // an unknown fader: VPX keeps the number, and doesn't fade
                Some(Fader::Other(v)) => v as i32,
            },
            blink_pattern: l.blink_pattern.as_bytes().to_vec(),
            blink_interval: l.blink_interval as i32,
            color: colorref(&l.color),
            color2: colorref(&l.color2),
            current_intensity: 0.0,
            filament_temperature: 293.0,
            locked_by_ls: false,
            center: [l.center.x, l.center.y],
            halo_height: l.bulb_halo_height,
            modulate_vs_add: l.bulb_modulate_vs_add,
            transmission: l.transmission_scale,
            bulb: l.is_bulb_light,
            falloff: l.falloff_radius,
            falloff_power: l.falloff_power,
            depth_bias: l.depth_bias,
            mesh_radius: l.mesh_radius,
            on_ball: l.show_reflection_on_ball,
            ball_shadows: matches!(l.shadows, Some(vpin::vpx::gameitem::light::ShadowMode::RaytracedBallShadows)),
            show_bulb_mesh: l.show_bulb_mesh,
            duration: 0,
            final_state: 0.0,
            time_next_blink: 0,
            duration_end: 0,
            blink_frame: 0,
        };
        rt.render_setup(0);
        rt
    }

    /// The start of VPX's `RenderSetup`.
    fn render_setup(&mut self, now: u32) {
        self.blink_frame = 0;
        if self.in_play_state == BLINKING {
            self.restart_blinker(now);
        } else if self.duration > 0 && self.in_play_state != 0.0 {
            self.duration_end = now.wrapping_add(self.duration as u32);
        }
        let state = self.lit_state();
        self.filament_temperature = if state < 0.5 { 293.0 } else { 2700.0 };
        self.current_intensity = self.full() * state;
    }

    /// The light's full brightness.
    fn full(&self) -> f32 {
        self.intensity * self.intensity_scale
    }

    /// Whether the blink pattern is on at its current step.
    fn pattern_on(&self) -> bool {
        self.blink_pattern.get(self.blink_frame as usize) == Some(&b'1')
    }

    /// How on the light is: its state, or the pattern's step while blinking.
    fn lit_state(&self) -> f32 {
        if self.in_play_state == BLINKING { f32::from(u8::from(self.pattern_on())) } else { self.in_play_state }
    }

    fn update_blinker(&mut self, now: u32) {
        if self.time_next_blink <= now {
            self.blink_frame += 1;
            if self.blink_pattern.get(self.blink_frame as usize).is_none_or(|&c| c == 0) {
                self.blink_frame = 0;
            }
            self.time_next_blink = self.time_next_blink.wrapping_add(self.blink_interval as u32);
        }
    }

    fn restart_blinker(&mut self, now: u32) {
        self.blink_frame = 0;
        self.time_next_blink = now.wrapping_add(self.blink_interval as u32);
        self.duration_end = now.wrapping_add(self.duration as u32);
    }

    /// VPX's `setInPlayState`.
    pub(crate) fn set_in_play_state(&mut self, state: f32, now: u32) {
        let state = clamp_state(state);
        if state != self.in_play_state {
            self.in_play_state = state;
            if state == BLINKING {
                // the pattern starts again right away
                self.time_next_blink = now;
                self.blink_frame = 0;
            }
            // a state set this way ends a timed one
            if self.duration > 0 {
                self.duration = 0;
            }
        }
    }

    /// VPX's `UpdateAnimation`, each frame `diff_ms` after the last: the
    /// timed state, the blinking, then the brightness fading towards the
    /// state. Returns whether the brightness changed (VPX then fires the
    /// light's `Animate` event).
    pub fn update(&mut self, diff_ms: f32, now: u32) -> bool {
        if self.duration > 0 && self.duration_end < now {
            self.in_play_state = self.final_state;
            self.duration = 0;
            if self.in_play_state == BLINKING {
                self.restart_blinker(now);
            }
        }
        if self.in_play_state == BLINKING {
            self.update_blinker(now);
        }

        let previous = self.current_intensity;
        let light_state = self.lit_state();
        let target = self.full() * light_state;
        if self.current_intensity != target {
            match self.fader {
                0 => self.current_intensity = target,
                1 => {
                    if self.current_intensity < target {
                        self.current_intensity += self.fade_speed_up * diff_ms;
                        if self.current_intensity > target {
                            self.current_intensity = target;
                        }
                    } else if self.current_intensity > target {
                        self.current_intensity -= self.fade_speed_down * diff_ms;
                        if self.current_intensity < target {
                            self.current_intensity = target;
                        }
                    }
                }
                // a light with no intensity has no filament to heat
                2 if self.intensity != 0.0 && self.intensity_scale != 0.0 => {
                    // the fade speed, per ms, against the bulb's: a #44
                    // reaches full power in 30 to 40 ms; at most 0.5 s at a
                    // time, past which the bulb is stable anyway
                    let speed = if self.current_intensity < target { self.fade_speed_up } else { self.fade_speed_down };
                    let inv_fade_speed = speed / self.full();
                    let remaining = 0.5f32.min(diff_ms * (0.001f64 * 40.0) as f32 * inv_fade_speed);
                    self.filament_temperature = if light_state != 0.0 {
                        // emission modulated by voltage^4, roughly
                        let u = 6.3 * light_state.sqrt().sqrt();
                        bulb::heat_up(bulb::BULB_44, self.filament_temperature, remaining, u, 0.0)
                    } else {
                        bulb::cool_down(bulb::BULB_44, self.filament_temperature, remaining)
                    };
                    self.current_intensity = bulb::filament_temperature_to_emission(bulb::BULB_44, self.filament_temperature as f32) * self.full();
                }
                _ => {}
            }
        }
        previous != self.current_intensity
    }

    /// `State` as the script reads it.
    pub fn get_state(&self) -> f32 {
        clamp_state(if self.locked_by_ls { self.state } else { self.in_play_state })
    }

    /// `State` as the script sets it.
    pub fn put_state(&mut self, state: f32, now: u32) {
        let state = clamp_state(state);
        if !self.locked_by_ls {
            self.set_in_play_state(state, now);
        }
        self.state = state;
    }

    pub fn put_intensity(&mut self, intensity: f32) {
        self.intensity = 0f32.max(intensity);
        self.current_intensity = self.full() * self.lit_state();
    }

    pub fn put_intensity_scale(&mut self, scale: f32) {
        self.intensity_scale = scale.max(0.0);
        self.current_intensity = self.full() * self.lit_state();
    }

    pub fn blink_pattern(&self) -> String {
        String::from_utf8_lossy(&self.blink_pattern).into_owned()
    }

    pub fn put_blink_pattern(&mut self, pattern: &str, now: u32) {
        let mut bytes: Vec<u8> = pattern.bytes().collect();
        if bytes.is_empty() {
            bytes = b"0".to_vec();
        }
        self.blink_pattern = bytes;
        self.restart_blinker(now);
    }

    pub fn put_blink_interval(&mut self, interval: i32, now: u32) {
        self.blink_interval = interval;
        self.time_next_blink = now.wrapping_add(interval as u32);
    }

    /// `Duration`: show `start` for `ms`, then `end`.
    pub fn duration(&mut self, start: f32, ms: i32, end: f32, now: u32) {
        self.in_play_state = clamp_state(start);
        self.duration = ms;
        self.final_state = clamp_state(end);
        self.duration_end = now.wrapping_add(ms as u32);
        if self.in_play_state == BLINKING {
            self.blink_frame = 0;
            self.time_next_blink = now.wrapping_add(self.blink_interval as u32);
        }
    }

    /// `GetInPlayStateBool`: whether the light is on now.
    pub fn in_play_on(&self) -> bool {
        if self.in_play_state == BLINKING { self.pattern_on() } else { self.in_play_state != 0.0 }
    }

    /// `FilamentTemperature`: room temperature for a light with no intensity.
    pub fn filament_temperature(&self) -> f32 {
        if self.intensity != 0.0 && self.intensity_scale != 0.0 {
            bulb::emission_to_filament_temperature(f64::from(self.current_intensity / self.full())) as f32
        } else {
            293.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(fader: i32) -> LightRt {
        let mut l = LightRt::new(&Light::default());
        l.fader = fader;
        l.intensity = 10.0;
        l.fade_speed_up = 10.0 / 200.0;
        l.fade_speed_down = 10.0 / 500.0;
        l
    }

    #[test]
    fn linear_fade_takes_the_fade_speed() {
        let mut l = light(1);
        l.put_state(1.0, 0);
        let mut now = 0;
        for _ in 0..10 {
            now += 10;
            assert!(l.update(10.0, now));
        }
        assert!((l.current_intensity - 5.0).abs() < 1e-4, "{}", l.current_intensity);
        for _ in 0..10 {
            now += 10;
            l.update(10.0, now);
        }
        assert_eq!(l.current_intensity, 10.0);
        assert!(!l.update(10.0, now + 10));
    }

    #[test]
    fn blinking_follows_the_pattern() {
        let mut l = light(0);
        l.put_blink_pattern("110", 0);
        l.put_blink_interval(100, 0);
        l.put_state(BLINKING, 0);
        // as VPX: the first update after the state change already moves on a step
        let mut seen = Vec::new();
        for now in (0..600).step_by(50) {
            l.update(50.0, now);
            seen.push(l.in_play_on());
        }
        assert_eq!(seen, [true, true, false, false, true, true, true, true, false, false, true, true]);
    }

    #[test]
    fn duration_returns_to_the_end_state() {
        let mut l = light(0);
        l.duration(1.0, 100, 0.0, 0);
        l.update(16.0, 16);
        assert!(l.in_play_on());
        l.update(16.0, 101);
        assert!(!l.in_play_on());
        assert_eq!(l.current_intensity, 0.0);
    }

    /// With VPX's default fade speed (full intensity in 200 ms), the
    /// filament gets 1 ms of heating for every 5 ms of play: a slow bulb
    /// that is dark for the first frames, then brightens over ~300 ms.
    #[test]
    fn incandescent_bulb_warms_up_slowly() {
        let mut l = light(2);
        l.put_state(1.0, 0);
        let mut now = 0;
        let mut levels = Vec::new();
        for _ in 0..40 {
            now += 16;
            l.update(16.0, now);
            levels.push(l.current_intensity);
        }
        assert!(levels.windows(2).all(|w| w[1] >= w[0]), "{levels:?}");
        assert!(levels[0] == 0.0 && levels[11] < 5.0 && levels[39] > 9.5, "{levels:?}");
    }

    #[test]
    fn old_true_means_on() {
        assert_eq!(clamp_state(-1.0), 1.0);
        assert_eq!(clamp_state(255.0), 1.0);
        assert_eq!(clamp_state(2.0), BLINKING);
        assert_eq!(clamp_state(0.5), 0.5);
    }
}
