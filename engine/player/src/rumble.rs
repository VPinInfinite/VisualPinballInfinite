// Ported from Visual Pinball X build 5436 (10.8.1 pre-release, commit af26b2d): InputManager::PlayRumble
// (src/input/InputManager.cpp) and SDLInputHandler::PlayRumble (src/input/SDLInputHandler.h), the rumble
// of a flipper action's press (InputManager.cpp) and of a bumper's hit (src/parts/bumper.cpp), and the
// helper saturate (src/core/def.h) (license:GPLv3+).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A gamepad's rumble, as VPX 5436 plays it: a short shake when a flipper
//! button is pressed (from any device), a bumper or a slingshot fires, or the
//! plunger bounces back as it fires. Wherever it's asked for — the game
//! thread, the input thread — the latest request waits here, and the window
//! thread, which reads the gamepads, plays it on every open gamepad once a
//! frame, as VPX plays each on every joystick (each cancelling the one
//! before). `[Player]` `RumbleMode = 0` in the settings turns it off, as in
//! VPX, whose default is on.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use sdl3_sys::everything::*;

/// The latest request not yet played, packed: the low-frequency motor's speed in the low 16
/// bits, the high-frequency motor's in the next 16, the duration in ms in the next 24, and a
/// bit above them so a request is never 0, which means none.
static PENDING: AtomicU64 = AtomicU64::new(0);

/// VPX's `RumbleMode` isn't 0 (off).
static ENABLED: AtomicBool = AtomicBool::new(true);

/// VPX's `saturate`: held to 0..1 (VPX's `max(min(x, 1), 0)`, so NaN gives 1).
fn saturate(x: f32) -> f32 {
    let x = if x < 1.0 { x } else { 1.0 };
    if x < 0.0 { 0.0 } else { x }
}

/// The settings' `[Player]` `RumbleMode`: 0 is off; VPX's other three all play.
pub fn set_mode(mode: i32) {
    ENABLED.store(mode != 0, Ordering::Relaxed);
}

/// VPX's `PlayRumble`: the two motors' speeds (0 to 1) for `ms` milliseconds; replaces
/// whatever was asked for and not yet played.
pub fn request(low_frequency_speed: f32, high_frequency_speed: f32, ms: u32) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    PENDING.store(pack(low_frequency_speed, high_frequency_speed, ms), Ordering::Relaxed);
}

fn pack(low: f32, high: f32, ms: u32) -> u64 {
    let speed = |v: f32| u64::from((saturate(v) * 65535.0) as u16);
    1 << 56 | u64::from(ms.min(0xFF_FFFF)) << 32 | speed(high) << 16 | speed(low)
}

fn unpack(p: u64) -> (u16, u16, u32) {
    (p as u16, (p >> 16) as u16, ((p >> 32) & 0xFF_FFFF) as u32)
}

/// VPX's flipper actions: their press rumbles (the left and right flippers and the staged
/// ones, at VPX 10.8.0's keys).
pub fn on_key(dik: u8, down: bool) {
    use crate::dik::default_keys::*;
    if down && [LEFT_FLIPPER, RIGHT_FLIPPER, STAGED_LEFT_FLIPPER, STAGED_RIGHT_FLIPPER].contains(&dik) {
        request(0.0, 0.2, 150);
    }
}

/// Plays the latest request, if any, on every open gamepad.
///
/// # Safety
/// SDL's gamepad subsystem must be started; call on the thread that pumps SDL's events.
pub unsafe fn play_pending() {
    let p = PENDING.swap(0, Ordering::Relaxed);
    if p == 0 {
        return;
    }
    let (low, high, ms) = unpack(p);
    unsafe {
        let mut count = 0;
        let ids = SDL_GetGamepads(&mut count);
        if ids.is_null() {
            return;
        }
        for i in 0..usize::try_from(count).unwrap_or(0) {
            let pad = SDL_GetGamepadFromID(*ids.add(i));
            if !pad.is_null() {
                SDL_RumbleGamepad(pad, low, high, ms);
            }
        }
        SDL_free(ids.cast());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_keeps_vpxs_speeds_and_time() {
        assert_eq!(unpack(pack(0.1, 0.05, 100)), (6553, 3276, 100));
        // VPX holds the speeds to 0..1: the plunger's bounce asks for its speed, which can be negative
        assert_eq!(unpack(pack(-0.3, 2.0, 50)), (0, 65535, 50));
        assert_ne!(pack(0.0, 0.0, 0), 0, "a request to stop is still a request");
    }
}
