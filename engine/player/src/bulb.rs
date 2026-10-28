// Ported from Visual Pinball X: src/utils/bulb.cpp and bulb.h (license:GPLv3+), build 5436 (10.8.1
// pre-release, commit af26b2d), the incandescent bulb model used by lights faded as filaments.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's model of a pinball bulb's filament: how hot it gets under a
//! voltage, how fast it heats and cools, and how much visible light it
//! gives at a temperature. A light with the incandescent fader follows it,
//! so it brightens and dims like the real bulb (#44) instead of a straight
//! ramp.

use std::sync::OnceLock;

pub const BULB_44: usize = 0;
pub const BULB_47: usize = 1;
pub const BULB_86: usize = 2;
pub const BULB_89: usize = 3;
pub const BULB_906: usize = 4;
const BULB_MAX: usize = 5;

const BULB_T_MAX: usize = 3400;

/// Impact of the coil on emissivity (part of the radiated energy hits the filament back).
const EMISSIVITY_COIL_FACTOR: f64 = 0.6865;
/// Heat lost through the base and wires.
const BASE_WIRE_LOSS: f64 = 0.07;
/// Stefan–Boltzmann constant (W·m⁻²·K⁻⁴).
const STEPHAN_BOLTZMAN: f64 = 5.670374419e-8;

/// A bulb's ratings and the filament measures derived from them.
struct Ratings {
    rating_u: f64,
    rating_i: f64,
    rating_t: usize,
    /// Filament surface, m².
    surface: f64,
    /// Filament mass, kg.
    mass: f64,
}

const RATINGS: [Ratings; BULB_MAX] = [
    Ratings { rating_u: 6.3, rating_i: 0.250, rating_t: 2710, surface: 2.2191615654e-6, mass: 0.3117866466e-6 }, // #44
    Ratings { rating_u: 6.3, rating_i: 0.150, rating_t: 2690, surface: 1.3878130447e-6, mass: 0.1402801155e-6 }, // #47
    Ratings { rating_u: 6.3, rating_i: 0.200, rating_t: 2550, surface: 2.4286321482e-6, mass: 0.3196915046e-6 }, // #86
    Ratings { rating_u: 13.0, rating_i: 0.577, rating_t: 2810, surface: 8.8347591602e-6, mass: 2.0675689639e-6 }, // #89
    Ratings { rating_u: 13.0, rating_i: 0.690, rating_t: 2755, surface: 11.6609979870e-6, mass: 3.1559940172e-6 }, // #906
];

/// One bulb's precomputed tables.
struct Bulb {
    /// Resistance at 293 K.
    r0: f64,
    /// Cooling per second at each temperature: energy / (mass × specific heat).
    cool_factor: Vec<f64>,
    /// Heating per second per volt²: 1 / (R × mass × specific heat).
    heat_factor: Vec<f64>,
}

/// VPX's `bulb_init` lookup tables, computed once.
struct Tables {
    /// Visible emission (lumen·sr⁻¹·m⁻²) at 1500 K to 3400 K.
    t_to_p: Vec<f64>,
    /// Filament temperature for a visible emission, 255 being the emission at 2700 K.
    p_2700_to_t: [f64; 512],
    bulbs: Vec<Bulb>,
}

/// Room temperature (293 K, 20 °C).
const TR: f64 = 293.0;

fn pow4(x: f64) -> f64 {
    let x = x * x;
    x * x
}

/// The filament's resistance at a temperature: R = R0 (1 + 0.0045 (T − 293)).
fn bulb_r(r0: f64, t: f64) -> f64 {
    r0 * (1.0 + 0.0045 * (t - 293.0))
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        // resistance at room temperature, from the ratings U, I and T
        let r0: Vec<f64> = RATINGS.iter().map(|b| (b.rating_u / b.rating_i) / bulb_r(1.0, b.rating_t as f64)).collect();

        // visible emission at each filament temperature (Coblentz and Emerson)
        let t_to_p: Vec<f64> = (0..=BULB_T_MAX - 1500)
            .map(|i| {
                let t = (1500 + i) as f64;
                let p = 1.247 / (1.0 + 129.05 / t).powf(204.0)
                    + 0.0678 / (1.0 + 78.85 / t).powf(404.0)
                    + 0.0489 / (1.0 + 23.52 / t).powf(1004.0)
                    + 0.0406 / (1.0 + 13.67 / t).powf(2004.0);
                p * 68493.150685 // W·sr⁻¹·cm⁻² to lumen·sr⁻¹·m⁻²
            })
            .collect();

        // emission to temperature, by searching the table above
        let mut p_2700_to_t = [0.0; 512];
        let mut t_pos = 0;
        let p2700 = t_to_p[2700 - 1500];
        for (i, slot) in p_2700_to_t.iter_mut().enumerate() {
            let p = i as f64 * (p2700 / 255.0);
            while t_to_p[t_pos] < p {
                t_pos += 1;
            }
            *slot = (1500 + t_pos) as f64;
        }

        // heating and cooling at each temperature, for each bulb
        let mut bulbs: Vec<Bulb> = r0
            .iter()
            .map(|&r0| Bulb { r0, cool_factor: vec![0.0; BULB_T_MAX + 1], heat_factor: vec![0.0; BULB_T_MAX + 1] })
            .collect();
        for i in 0..=BULB_T_MAX {
            let t = i as f64;
            // tungsten's specific heat (J·kg⁻¹·K⁻¹), after Agrawal
            let specific_heat = 3.0 * 45.2268 * (1.0 - (310.0 * 310.0) / (20.0 * t * t)) + (2.0 * 4.5549e-3 * t) + (4.0 * 5.77874e-10 * t * t * t);
            for (bulb, ratings) in bulbs.iter_mut().zip(&RATINGS) {
                // radiated power (Planck's law), corrected for the coil
                let emissivity = EMISSIVITY_COIL_FACTOR * 0.0000689 * t.powf(1.0748);
                let mut cool = -STEPHAN_BOLTZMAN * ratings.surface * emissivity * (pow4(t) - pow4(TR));
                // convection through the base and wires
                cool += -BASE_WIRE_LOSS * ((t - TR) / ratings.rating_t as f64) / bulb_r(bulb.r0, t);
                // electric power is U²/R; U² is applied later, as it varies
                let heat = 1.0 / bulb_r(bulb.r0, t);
                bulb.cool_factor[i] = cool / (ratings.mass * specific_heat);
                bulb.heat_factor[i] = heat / (ratings.mass * specific_heat);
            }
        }
        Tables { t_to_p, p_2700_to_t, bulbs }
    })
}

/// A table index for a temperature, as VPX's `(int)T` (kept inside the table).
fn index(t: f64) -> usize {
    (t as usize).min(BULB_T_MAX)
}

/// VPX's `bulb_filament_temperature_to_emission`: visible emission relative
/// to the bulb's at its rated temperature.
pub fn filament_temperature_to_emission(bulb: usize, t: f32) -> f32 {
    let tables = tables();
    if t < 1500.0 {
        return 0.0;
    }
    // as VPX: at the top of the table the emission isn't made relative
    if t >= BULB_T_MAX as f32 {
        return tables.t_to_p[BULB_T_MAX - 1500] as f32;
    }
    (tables.t_to_p[t as usize - 1500] / tables.t_to_p[RATINGS[bulb].rating_t - 1500]) as f32
}

/// The linear colour of a filament every 100 K from 1500 K to 3000 K,
/// normalised for a perceived brightness of 1 (VPX's `temperatureToTint`).
#[allow(clippy::excessive_precision)] // VPX's numbers, as it writes them
const TEMPERATURE_TO_TINT: [[f32; 3]; 16] = [
    [3.253114, 0.431191, 0.000001],
    [3.074210, 0.484372, 0.000001],
    [2.914679, 0.531794, 0.000001],
    [2.769808, 0.574859, 0.000001],
    [2.643605, 0.612374, 0.000001],
    [2.523686, 0.645953, 0.020487],
    [2.414433, 0.676211, 0.042456],
    [2.316033, 0.703137, 0.065485],
    [2.225598, 0.727599, 0.089456],
    [2.144543, 0.749200, 0.114156],
    [2.070389, 0.768694, 0.139412],
    [1.997974, 0.787618, 0.165180],
    [1.935725, 0.803465, 0.191508],
    [1.876871, 0.818242, 0.218429],
    [1.821461, 0.832006, 0.245241],
    [1.772554, 0.843853, 0.271900],
];

/// VPX's `bulb_filament_temperature_to_tint`: the filament's linear colour
/// at a temperature.
pub fn filament_temperature_to_tint(t: f32) -> [f32; 3] {
    if t < 1500.0 {
        return TEMPERATURE_TO_TINT[0];
    }
    if t >= 2999.0 {
        return TEMPERATURE_TO_TINT[15];
    }
    // Linear interpolation between the precomputed values
    let t_ref = (t - 1500.0) * (1.0 / 100.0) as f32;
    let lower = t_ref as usize;
    let alpha = t_ref - lower as f32;
    let (a, b) = (TEMPERATURE_TO_TINT[lower], TEMPERATURE_TO_TINT[lower + 1]);
    [(1.0 - alpha) * a[0] + alpha * b[0], (1.0 - alpha) * a[1] + alpha * b[1], (1.0 - alpha) * a[2] + alpha * b[2]]
}

/// What VPX multiplies an incandescent light's colours by (light.cpp,
/// `Render`): the filament's tint now over its tint at 2700 K, so a bulb
/// warming up or cooling down glows orange.
pub fn incandescent_tint(t: f32) -> [f32; 3] {
    let (now, full) = (filament_temperature_to_tint(t), filament_temperature_to_tint(2700.0));
    [now[0] / full[0], now[1] / full[1], now[2] / full[2]]
}

/// VPX's `bulb_emission_to_filament_temperature`, for an emission relative
/// to the one at 2700 K.
pub fn emission_to_filament_temperature(p: f64) -> f64 {
    let tables = tables();
    let v = (p * 255.0) as i32;
    if v >= 512 { tables.p_2700_to_t[511] } else { tables.p_2700_to_t[v.max(0) as usize] }
}

/// VPX's `bulb_cool_down`: the filament's temperature after cooling for
/// `duration` seconds, in steps of 1 ms.
pub fn cool_down(bulb: usize, mut t: f64, mut duration: f32) -> f64 {
    let cool_factor = &tables().bulbs[bulb].cool_factor;
    while duration > 0.0 {
        let dt = if duration > 0.001 { 0.001 } else { duration };
        t += f64::from(dt) * cool_factor[index(t)];
        if t <= 294.0 {
            return 293.0;
        }
        duration -= dt;
    }
    t
}

/// VPX's `bulb_heat_up`: the filament's temperature after `duration`
/// seconds under voltage `u`, with a resistor `serial_r` in series.
pub fn heat_up(bulb: usize, mut t: f64, mut duration: f32, u: f32, serial_r: f32) -> f64 {
    let b = &tables().bulbs[bulb];
    while duration > 0.0 {
        // stay inside the tables: between room temperature and melting
        t = t.clamp(293.0, BULB_T_MAX as f64);
        let mut u1 = f64::from(u);
        if serial_r != 0.0 {
            let r = bulb_r(b.r0, t);
            u1 *= r / (r + f64::from(serial_r));
        }
        let energy = u1 * u1 * b.heat_factor[index(t)] + b.cool_factor[index(t)];
        if -10.0 < energy && energy < 10.0 {
            // stable: the electric heat roughly equals what it loses
            return t;
        }
        // the first surge of current is integrated in finer steps
        let step = if energy > 1000e3 { 0.0005 } else { 0.001 };
        let dt = if duration > step { step } else { duration };
        t += f64::from(dt) * energy;
        duration -= dt;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_is_white_at_2700_k_and_red_when_cold() {
        assert_eq!(incandescent_tint(2700.0), [1.0, 1.0, 1.0]);
        // cold: the 1500 K entry over the 2700 K one
        let cold = incandescent_tint(293.0);
        assert_eq!(cold, [3.253114 / 1.935725, 0.431191 / 0.803465, 0.000001 / 0.191508]);
        // halfway between two entries
        assert_eq!(filament_temperature_to_tint(1550.0)[1], 0.5 * 0.431191 + 0.5 * 0.484372);
    }

    /// VPX's comment on the #44: 54 ms from 10% to 90% emission when lit,
    /// and 36 ms back down when switched off.
    #[test]
    fn bulb_44_heats_and_cools_as_vpx_says() {
        let dt = 0.0005f32;
        let (mut t, mut time, mut t10) = (293.0f64, 0.0f64, 0.0f64);
        let mut rise = 0.0;
        for _ in 0..1000 {
            t = heat_up(BULB_44, t, dt, 6.3, 0.0);
            time += f64::from(dt);
            let p = filament_temperature_to_emission(BULB_44, t as f32);
            if t10 == 0.0 && p >= 0.1 {
                t10 = time;
            } else if t10 != 0.0 && p >= 0.9 {
                rise = (time - t10) * 1e3;
                break;
            }
        }
        assert!((rise - 54.0).abs() < 1.5, "rise {rise} ms");

        let (mut t, mut time, mut t90) = (2710.0f64, 0.0f64, 0.0f64);
        let mut fall = 0.0;
        for _ in 0..1000 {
            t = cool_down(BULB_44, t, dt);
            time += f64::from(dt);
            let p = filament_temperature_to_emission(BULB_44, t as f32);
            if t90 == 0.0 && p <= 0.9 {
                t90 = time;
            } else if t90 != 0.0 && p <= 0.1 {
                fall = (time - t90) * 1e3;
                break;
            }
        }
        assert!((fall - 36.0).abs() < 1.5, "fall {fall} ms");
    }

    #[test]
    fn lit_filament_settles_at_its_rating() {
        let t = heat_up(BULB_44, 293.0, 1.0, 6.3, 0.0);
        assert!((t - 2710.0).abs() < 5.0, "{t}");
        assert!((filament_temperature_to_emission(BULB_44, t as f32) - 1.0).abs() < 0.05);
        assert_eq!(emission_to_filament_temperature(0.0), 1500.0);
    }
}
