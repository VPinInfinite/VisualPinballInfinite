// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its volume
// actions (src/input/InputManager.cpp's VolumeDown and VolumeUp, src/input/InputAction.cpp's repeat).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX 5436's volume keys (its `VolumeDown` and `VolumeUp` actions, `-` and `=` unless the
//! settings map them elsewhere, `input_map`): each press, and every 75 ms while it's held, moves
//! the playfield and backglass volumes a hundredth down or up together, for everything playing
//! too, and shows "Volume: NN%" — the backglass volume — for half a second. As VPX's keys do,
//! they change the volumes for this game only; the settings keep theirs (`[Player]`
//! `SoundVolume`, `MusicVolume`).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::input_map::PlayerAction;

/// Each key held now, and its presses so far: a tap shorter than the game thread's millisecond
/// between looks still counts. Set by the input thread.
static HELD: [AtomicBool; 2] = [AtomicBool::new(false), AtomicBool::new(false)];
static PRESSES: [AtomicU32; 2] = [AtomicU32::new(0), AtomicU32::new(0)];

/// What the playfield window shows, until when (VPX's `PushNotification`).
static NOTICE: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// VPX's repeat while a key is held (`SetRepeatPeriod(75)`) and how long its notice shows.
const REPEAT: Duration = Duration::from_millis(75);
const SHOWN_FOR: Duration = Duration::from_millis(500);

fn slot(action: PlayerAction) -> usize {
    match action {
        PlayerAction::VolumeDown => 0,
        // only the volume keys come here
        _ => 1,
    }
}

/// A volume key pressed or let go (the input thread).
pub fn key(action: PlayerAction, down: bool) {
    let k = slot(action);
    if down {
        PRESSES[k].fetch_add(1, Ordering::AcqRel);
    }
    HELD[k].store(down, Ordering::Release);
}

/// The notice to show now, if any (the window thread).
pub fn notice() -> Option<String> {
    let shown = NOTICE.lock().unwrap_or_else(|e| e.into_inner());
    shown.as_ref().filter(|(_, until)| Instant::now() < *until).map(|(text, _)| text.clone())
}

/// The game thread's side: the keys' presses and repeats turned into volume changes.
#[derive(Default)]
pub struct Keys {
    seen: [u32; 2],
    /// When each key last changed the volume, while it's held.
    last: [Option<Instant>; 2],
}

impl Keys {
    /// Looks at the keys, changing the volumes of `sounds` as VPX's handlers do.
    pub fn poll(&mut self, sounds: &mut crate::audio::Sounds) {
        let now = Instant::now();
        for (k, step) in [(0, -0.01f32), (1, 0.01)] {
            let presses = PRESSES[k].load(Ordering::Acquire);
            let pressed = presses != self.seen[k];
            self.seen[k] = presses;
            let held = HELD[k].load(Ordering::Acquire);
            let repeat = held && self.last[k].is_some_and(|t| now >= t + REPEAT);
            if pressed || repeat {
                self.last[k] = held.then_some(now);
                sounds.set_volumes(sounds.playfield_volume + step, sounds.backglass_volume + step);
                *NOTICE.lock().unwrap_or_else(|e| e.into_inner()) = Some((format!("Volume: {:3.0}%", sounds.backglass_volume * 100.0), now + SHOWN_FOR));
            } else if !held {
                self.last[k] = None;
            }
        }
    }
}
