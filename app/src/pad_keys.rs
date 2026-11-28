//! A game controller in VPX 10.8.0, passed by the app itself as the keys 10.8.0 plays with
//! (`library::controller_1080::keys`): 10.8.0 reads an XInput controller only if one is there as
//! play starts, never again once it's gone, and no button from a trigger. Future Pinball's games the
//! same way, as its keys (`library::controller_1080::keys_with`). While a game plays, every
//! controller XInput has is read every millisecond (`pads::AllPads`: a PC may have several paired,
//! and a VR program's emulated pad, so the one in the player's hands is read whichever slot it's
//! in), and each key pressed or let go in the game's window as their buttons and triggers press it
//! — only while that window is in front (`front`), so no key reaches another window. A key held as
//! the game's window lost the front (the pause menu over it, another window) is let go once it's
//! back; an input held as it came back (the menu's own button, still down) presses nothing until
//! it's let go. What the game was given is written in the app's log after it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ::library::controller_1080::{Input, Keys};

use crate::pads::Gamepad;

/// The controller passed to a game as keys until `ended`, on a thread of its own, while `front`
/// says its window is the one in front.
pub fn start(ended: Arc<AtomicBool>, keys: Keys, front: fn() -> bool) {
    if keys.keys.is_empty() {
        return;
    }
    let _ = std::thread::Builder::new().name("pad-keys".into()).spawn(move || run(&ended, &keys, front));
}

fn run(ended: &AtomicBool, keys: &Keys, front: fn() -> bool) {
    // the system's timer at a millisecond, so the controller is read every millisecond
    let _fine = FineTimer::new();
    let n = keys.keys.len();
    let mut sent = vec![false; n];
    // inputs held as the game's window came back to the front, pressing nothing until let go
    let mut held_from_away = vec![false; n];
    // every controller there: the one in the player's hands whichever of XInput's slots it's in
    let mut all = crate::pads::AllPads::new();
    // what the game was given, for the app's log after it
    let mut passed = 0u32;
    let mut away = 0u32;
    let mut seen = 0usize;
    while !ended.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(1));
        let pads = all.read();
        seen = seen.max(pads.len());
        let want: Vec<bool> = keys.keys.iter().map(|(_, inputs)| pads.iter().any(|g| inputs.iter().any(|i| pressed(i, g)))).collect();
        if !front() {
            for (k, w) in want.iter().enumerate() {
                if *w && !held_from_away[k] {
                    away += 1;
                }
            }
            // what's held now stays held from away, and presses nothing once the game is back
            for (h, w) in held_from_away.iter_mut().zip(&want) {
                *h |= *w;
            }
            continue;
        }
        for k in 0..n {
            if held_from_away[k] && !want[k] {
                held_from_away[k] = false;
            }
            let on = want[k] && !held_from_away[k];
            if on != sent[k] {
                crate::pause_menu::send_key(keys.keys[k].0, !on);
                sent[k] = on;
                passed += u32::from(on);
            }
        }
    }
    // the keys still held let go, and what the game was given said
    for k in (0..n).filter(|&k| sent[k]) {
        crate::pause_menu::send_key(keys.keys[k].0, true);
    }
    eprintln!("The controller in the game: {passed} presses passed as its keys, {away} while another window was in front; {seen} controllers seen at most");
}

/// Whether one of a controller's inputs is pressed now.
fn pressed(input: &Input, g: &Gamepad) -> bool {
    match input {
        Input::Button(mask) => g.buttons & mask != 0,
        Input::Axis { axis, point, reversed } => {
            let at = position(*axis, g);
            if *reversed { at <= *point } else { at >= *point }
        }
    }
}

/// One of XInput's axes as SDL reads it, from -1 to 1: the left stick's x and y, the left trigger,
/// the right stick's x and y, the right trigger; a stick's y positive down, a trigger -1 at rest.
fn position(axis: u8, g: &Gamepad) -> f32 {
    let stick = |v: i16| f32::from(v) / 32768.0;
    let trigger = |v: u8| f32::from(v) / 255.0 * 2.0 - 1.0;
    match axis {
        0 => stick(g.thumb_lx),
        1 => -stick(g.thumb_ly),
        2 => trigger(g.left_trigger),
        3 => stick(g.thumb_rx),
        4 => -stick(g.thumb_ry),
        5 => trigger(g.right_trigger),
        _ => 0.0,
    }
}

/// Windows' timer at a millisecond while it's held (`timeBeginPeriod`), so a thread's millisecond
/// sleep is one.
struct FineTimer;

impl FineTimer {
    fn new() -> FineTimer {
        #[cfg(windows)]
        {
            #[link(name = "winmm")]
            unsafe extern "system" {
                fn timeBeginPeriod(period: u32) -> u32;
            }
            // SAFETY: a period in milliseconds, given back on drop
            unsafe { timeBeginPeriod(1) };
        }
        FineTimer
    }
}

impl Drop for FineTimer {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            #[link(name = "winmm")]
            unsafe extern "system" {
                fn timeEndPeriod(period: u32) -> u32;
            }
            // SAFETY: the period begun in `new`
            unsafe { timeEndPeriod(1) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_controllers_inputs_read_as_vpx_reads_them() {
        let pad = |left_trigger: u8, thumb_ly: i16, buttons: u16| Gamepad { buttons, left_trigger, thumb_ly, ..Default::default() };
        let flipper = Input::Axis { axis: 2, point: -0.3, reversed: false };
        let staged = Input::Axis { axis: 2, point: 0.3, reversed: false };
        // a third of the trigger's travel flips, two thirds the staged flipper too
        assert!(!pressed(&flipper, &pad(0, 0, 0)) && !pressed(&flipper, &pad(80, 0, 0)));
        assert!(pressed(&flipper, &pad(100, 0, 0)) && !pressed(&staged, &pad(100, 0, 0)));
        assert!(pressed(&staged, &pad(200, 0, 0)));
        // a stick's y positive down, as SDL reads it; a point reached from below
        let down = Input::Axis { axis: 1, point: 0.5, reversed: false };
        let up = Input::Axis { axis: 1, point: -0.5, reversed: true };
        assert!(pressed(&down, &pad(0, -30000, 0)) && !pressed(&down, &pad(0, 30000, 0)));
        assert!(pressed(&up, &pad(0, 30000, 0)));
        assert!(pressed(&Input::Button(0x1000), &pad(0, 0, 0x1000 | 0x0020)) && !pressed(&Input::Button(0x2000), &pad(0, 0, 0x1000)));
    }
}
