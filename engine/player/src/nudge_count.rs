// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026, the Visual Pinball Infinite contributors

//! VPX 10.8.0's way of tilting, counted by the player: a setting beside the
//! simulated tilt bob (`[VPI]` `TiltCountsNudges = 1`), so either way can be chosen.
//!
//! VPX 10.8.0's shared scripts counted a table's nudges (`cvpmNudge`): each adds its force
//! × 1.2 to a count, held to the table's limit plus 10; once the count reaches the limit,
//! the nudge pulses the tilt switch; each second the count drains by one, and by one more,
//! with another pulse, while it's over the limit by more than one. The limit is 11 less the
//! table's `vpmNudge.Sensitivity` (1 to 10), 5 when it sets none. VPX 5436's scripts, which
//! the player runs, no longer count, nor keep the sensitivity, so the player counts
//! each press of a nudge key as 10.8.0's scripts pushed it (`DoNudge` at force 2,
//! randomised as they randomised it; a table's own `Nudge` calls weren't counted), takes the
//! sensitivity from the table's script text, and presses the mechanical tilt's key for each
//! pulse, as it does for the tilt bob's touches.

/// The count, in game time.
#[derive(Debug, Clone)]
pub struct NudgeCount {
    count: f32,
    limit: f32,
    timer_ms: u32,
    last_ms: Option<u32>,
    /// How many times the machine's tilt switch is to be pulsed so far.
    pub presses: u32,
}

impl NudgeCount {
    /// With the table's own sensitivity, if its script sets one.
    pub fn new(sensitivity: Option<f32>) -> NudgeCount {
        // cvpmNudge: mSensitivity = (10 - aSens) + 1, 5 until set
        let limit = sensitivity.map_or(5.0, |s| (10.0 - s) + 1.0);
        NudgeCount { count: 0.0, limit, timer_ms: 0, last_ms: None, presses: 0 }
    }

    /// The sensitivity a table's script sets (`vpmNudge.Sensitivity = 3`), the last one.
    pub fn sensitivity_in(script: &str) -> Option<f32> {
        let lower = script.to_ascii_lowercase();
        let mut found = None;
        let mut from = 0;
        while let Some(at) = lower[from..].find("vpmnudge.sensitivity") {
            let start = from + at + "vpmnudge.sensitivity".len();
            let rest = lower[start..].trim_start();
            if let Some(value) = rest.strip_prefix('=') {
                let digits: String = value.trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                if let Ok(v) = digits.parse::<f32>() {
                    found = Some(v);
                }
            }
            from = start;
        }
        found
    }

    /// A nudge the script made, of `force`.
    pub fn nudge(&mut self, force: f32) {
        self.count = (self.count + force * 1.2).min(self.limit + 10.0);
        if self.count >= self.limit {
            self.presses += 1;
        }
    }

    /// The count drains as game time passes (`cvpmNudge.Update`, once a second).
    pub fn step(&mut self, now_ms: u32) {
        let elapsed = self.last_ms.map_or(0, |last| now_ms.saturating_sub(last));
        self.last_ms = Some(now_ms);
        if self.count <= 0.0 {
            self.count = 0.0;
            self.timer_ms = 0;
            return;
        }
        self.timer_ms += elapsed;
        while self.timer_ms >= 1000 && self.count > 0.0 {
            self.timer_ms -= 1000;
            if self.count > self.limit + 1.0 {
                self.count -= 1.0;
                self.presses += 1;
            }
            self.count -= 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sensitivity_comes_from_the_script() {
        assert_eq!(NudgeCount::sensitivity_in("vpmNudge.TiltSwitch = 14\r\nvpmNudge.Sensitivity = 2\r\n"), Some(2.0));
        assert_eq!(NudgeCount::sensitivity_in("VPMNUDGE.SENSITIVITY=7 ' gentle"), Some(7.0));
        assert_eq!(NudgeCount::sensitivity_in("vpmNudge.TiltSwitch = 14"), None);
    }

    #[test]
    fn nudges_count_toward_a_tilt_and_drain() {
        // sensitivity 6: the limit is 5
        let mut c = NudgeCount::new(Some(6.0));
        c.step(0);
        // four ordinary nudges (force 1): 1.2 each, the fifth reaches the limit
        for _ in 0..4 {
            c.nudge(1.0);
        }
        assert_eq!(c.presses, 0);
        c.nudge(1.0);
        assert_eq!(c.presses, 1);
        // three seconds later it has drained below the limit again
        c.step(3000);
        c.nudge(0.1);
        assert_eq!(c.presses, 1);
    }

    #[test]
    fn far_over_the_limit_it_keeps_pulsing_as_it_drains() {
        let mut c = NudgeCount::new(None);
        c.step(0);
        for _ in 0..10 {
            c.nudge(2.0);
        }
        let after_nudges = c.presses;
        c.step(1000);
        assert_eq!(c.presses, after_nudges + 1);
    }
}
