// Ported from Visual Pinball X: src/parts/dispreel.cpp and dispreel.h (license:GPLv3+), build 5436
// (10.8.1 pre-release, commit af26b2d): the score reel's play-time motor (RenderSetup's start,
// UpdateAnimation) and its script methods AddValue, SetValue, ResetToZero and SpinReel.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! An electromechanical score reel, as VPX runs it: each reel (one
//! digit) turns by motor pulses, one step every update interval, and a
//! reel passing 9 carries one into the reel on its left, like a car's
//! odometer. The script queues pulses (`AddValue`, `SpinReel`), sets a
//! value at once (`SetValue`) or rolls every reel back to zero
//! (`ResetToZero`).

use vpin::vpx::gameitem::reel::Reel;
use vpin::vpx::image::ImageData;

/// VPX's `MAX_REELS`.
const MAX_REELS: i32 = 32;

#[derive(Clone, Copy, Default)]
struct ReelInfo {
    current_value: i32,
    motor_pulses: i32,
    motor_step_count: i32,
    motor_calc_step: f32,
    motor_offset: f32,
}

/// One score reel unit (several digits) while playing.
pub struct ReelRt {
    reels: Vec<ReelInfo>,
    pub update_interval: i32,
    motor_steps: i32,
    digit_range: i32,
    digit_height: f32,
    time_next_update: u32,
    /// The sound each step of a reel makes, if any.
    pub sound: String,
    /// The picture a script set (`Image`), by name and table picture (`Some(None)`: none); VPX
    /// draws whatever picture is set with the digit grid it laid out at the start.
    pub image: Option<(String, Option<u32>)>,
}

impl ReelRt {
    /// The reel as the table file sets it, then VPX's `RenderSetup` at play
    /// time 0. `image` is the reel's image, for the height of one digit.
    pub fn new(r: &Reel, image: Option<&ImageData>) -> ReelRt {
        // VPX reads these as floats and keeps their whole part
        let count = (r.reel_count as i32).clamp(1, MAX_REELS);
        let digit_range = r.digit_range as i32;
        let grid_rows = if r.use_image_grid && r.images_per_grid_row != 0 {
            let cols = r.images_per_grid_row as i32;
            let mut rows = (digit_range + 1) / cols;
            if rows * cols < digit_range + 1 {
                rows += 1;
            }
            rows
        } else {
            1
        };
        let digit_height = image.filter(|_| grid_rows != 0).map_or(0.0, |i| i.height as f32 / grid_rows as f32);
        // an interval of 0 in a file would stop VPX for good; one step a ms at most here
        let update_interval = (r.update_interval as i32).max(1);
        ReelRt {
            reels: vec![ReelInfo::default(); count as usize],
            update_interval,
            motor_steps: r.motor_steps as i32,
            digit_range,
            digit_height,
            time_next_update: update_interval as u32,
            image: None,
            sound: r.sound.clone(),
        }
    }

    /// The digit each reel shows, left to right.
    pub fn values(&self) -> impl Iterator<Item = i32> + '_ {
        self.reels.iter().map(|r| r.current_value)
    }

    /// VPX's `UpdateAnimation`: every update interval that has passed, each
    /// reel with pulses waiting starts a step (with the reel's sound) or
    /// moves on. Returns whether anything moved (VPX then fires `Animate`).
    pub fn update(&mut self, now: u32, mut play_sound: impl FnMut(&str)) -> bool {
        let mut animated = false;
        while now >= self.time_next_update {
            self.time_next_update = self.time_next_update.wrapping_add(self.update_interval as u32);
            let overflow_value = self.digit_range;
            let adjust_value = overflow_value + 1;
            let step = self.digit_height / self.motor_steps as f32;

            // the last reel first (right to left)
            for i in (0..self.reels.len()).rev() {
                let r = &mut self.reels[i];
                // a stopped motor with pulses waiting starts another step
                if r.motor_pulses != 0 && r.motor_step_count == 0 {
                    r.motor_step_count = self.motor_steps;
                    r.motor_calc_step = if r.motor_pulses > 0 { step } else { -step };
                    r.motor_offset = 0.0;
                    if !self.sound.is_empty() && self.sound != "<None>" {
                        play_sound(&self.sound);
                    }
                    animated = true;
                }
                if r.motor_step_count != 0 {
                    r.motor_offset += r.motor_calc_step;
                    r.motor_step_count -= 1;
                    // the end of a step: the reel shows the next digit
                    if r.motor_step_count <= 0 {
                        r.motor_step_count = 0;
                        r.motor_offset = 0.0;
                        let mut carry = 0;
                        if r.motor_pulses < 0 {
                            r.motor_pulses += 1;
                            r.current_value -= 1;
                            if r.current_value < 0 {
                                r.current_value += adjust_value;
                                carry = -1;
                            }
                        } else {
                            r.motor_pulses -= 1;
                            r.current_value += 1;
                            if r.current_value > overflow_value {
                                r.current_value -= adjust_value;
                                carry = 1;
                            }
                        }
                        // the reel to the left turns by one, as an odometer
                        if carry != 0 && i != 0 {
                            self.reels[i - 1].motor_pulses += carry;
                        }
                    }
                    animated = true;
                }
            }
        }
        animated
    }

    /// The digits of `value`, rightmost reel first.
    fn digits(&self, value: i32) -> impl Iterator<Item = (usize, i32)> {
        let base = i64::from(self.digit_range) + 1;
        let mut val = i64::from(value).abs();
        (0..self.reels.len()).rev().map_while(move |i| {
            if val == 0 || base == 0 {
                return None;
            }
            let digit = (val % base) as i32;
            val /= base;
            Some((i, digit))
        })
    }

    /// `AddValue`: pulses for each reel, which then turn and carry.
    pub fn add_value(&mut self, value: i32) {
        let negative = value < 0;
        let digits: Vec<(usize, i32)> = self.digits(value).collect();
        for (i, digit) in digits {
            if negative {
                self.reels[i].motor_pulses -= digit;
            } else {
                self.reels[i].motor_pulses += digit;
            }
        }
    }

    /// `SetValue`: every reel shows its digit at once.
    pub fn set_value(&mut self, value: i32, now: u32) {
        for r in &mut self.reels {
            *r = ReelInfo::default();
        }
        let digits: Vec<(usize, i32)> = self.digits(value).collect();
        for (i, digit) in digits {
            self.reels[i].current_value = digit;
        }
        // shown at the next frame
        self.time_next_update = now;
    }

    /// `ResetToZero`: each reel turns on to zero, carrying as it passes.
    pub fn reset_to_zero(&mut self) {
        let mut carry = 0;
        let overflow_value = self.digit_range + 1;
        for r in self.reels.iter_mut().rev() {
            let adjust = overflow_value - carry - r.current_value;
            carry = 0;
            if adjust != overflow_value {
                r.motor_pulses = adjust;
                carry = 1;
            }
        }
    }

    /// `SpinReel`: pulses for one reel (numbered from 1); false for a reel
    /// that isn't there.
    pub fn spin_reel(&mut self, reel: i32, pulses: i32) -> bool {
        if reel >= 1 && reel <= self.reels.len() as i32 {
            self.reels[reel as usize - 1].motor_pulses += pulses;
            true
        } else {
            false
        }
    }

    /// `UpdateInterval` as the script sets it (5 ms at least).
    pub fn put_update_interval(&mut self, interval: i32, now: u32) {
        self.update_interval = interval.max(5);
        self.time_next_update = now.wrapping_add(self.update_interval as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reel(count: f32) -> ReelRt {
        let r = Reel { reel_count: count, digit_range: 9.0, motor_steps: 2.0, update_interval: 50, ..Default::default() };
        ReelRt::new(&r, None)
    }

    fn run(r: &mut ReelRt, until: u32) -> u32 {
        let mut steps = 0;
        for now in (0..=until).step_by(10) {
            r.update(now, |_| steps += 1);
        }
        steps
    }

    #[test]
    fn set_value_shows_at_once() {
        let mut r = reel(5.0);
        r.set_value(1234, 0);
        assert_eq!(r.values().collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
        r.set_value(-77, 0);
        assert_eq!(r.values().collect::<Vec<_>>(), [0, 0, 0, 7, 7]);
    }

    #[test]
    fn add_value_turns_and_carries() {
        let mut r = reel(3.0);
        r.set_value(95, 0);
        r.add_value(7);
        // 7 pulses on the last reel, two motor steps each, one step per 50 ms
        run(&mut r, 50 * 2 * 7 + 200);
        assert_eq!(r.values().collect::<Vec<_>>(), [1, 0, 2]);
    }

    #[test]
    fn reset_to_zero_rolls_forward() {
        let mut r = reel(2.0);
        r.set_value(38, 0);
        r.reset_to_zero();
        run(&mut r, 3000);
        assert_eq!(r.values().collect::<Vec<_>>(), [0, 0]);
    }

    #[test]
    fn spin_reel_checks_its_number() {
        let mut r = reel(2.0);
        assert!(r.spin_reel(1, 3));
        assert!(!r.spin_reel(3, 1));
        run(&mut r, 1000);
        assert_eq!(r.values().collect::<Vec<_>>(), [3, 0]);
    }
}
