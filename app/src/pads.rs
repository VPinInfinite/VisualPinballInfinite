//! The controllers Windows' XInput lists, read by the app itself while a window of its own needs
//! them: the pause menu over VPX. A web page sees a controller only once it's been pressed while the
//! page has the focus, and on some PCs not at all, while VPX, whose menu this one stands in for,
//! reads the same controllers through XInput. Each controller's state goes to the window as the
//! web's standard layout has it, whenever it changes, until the window is gone.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// One controller as the web's standard layout has it: its buttons (A, B, X, Y, the shoulders, the
/// triggers, Back, Start, the sticks' clicks, the pad's up, down, left and right) and its sticks
/// (left x and y, right x and y, down and right positive).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pad {
    pub buttons: Vec<bool>,
    pub axes: Vec<f32>,
}

/// `XINPUT_GAMEPAD`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gamepad {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

/// `XINPUT_STATE`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct XState {
    packet: u32,
    pad: Gamepad,
}

/// XInput's own threshold for a trigger pulled (`XINPUT_GAMEPAD_TRIGGER_THRESHOLD`).
const TRIGGER_PULLED: u8 = 30;

/// A stick's travel as the page takes it: none inside XInput's dead zone for the left stick
/// (`XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE`, 7849, about a quarter), else in twentieths, so a stick
/// left alone doesn't keep telling of itself.
fn axis(v: i16) -> f32 {
    let f = (f32::from(v) / 32767.0).clamp(-1.0, 1.0);
    if f.abs() < 0.25 {
        return 0.0;
    }
    let q = (f * 20.0).round() / 20.0;
    if q == 0.0 { 0.0 } else { q }
}

/// An XInput controller in the web's standard layout.
pub fn standard(g: &Gamepad) -> Pad {
    let b = |bit: u16| g.buttons & bit != 0;
    let flip = |v: f32| if v == 0.0 { 0.0 } else { -v };
    Pad {
        buttons: vec![
            b(0x1000),                          // A
            b(0x2000),                          // B
            b(0x4000),                          // X
            b(0x8000),                          // Y
            b(0x0100),                          // left shoulder
            b(0x0200),                          // right shoulder
            g.left_trigger > TRIGGER_PULLED,    // left trigger
            g.right_trigger > TRIGGER_PULLED,   // right trigger
            b(0x0020),                          // Back
            b(0x0010),                          // Start
            b(0x0040),                          // left stick's click
            b(0x0080),                          // right stick's click
            b(0x0001),                          // up
            b(0x0002),                          // down
            b(0x0004),                          // left
            b(0x0008),                          // right
        ],
        // XInput's sticks count up as positive, the web's down
        axes: vec![axis(g.thumb_lx), flip(axis(g.thumb_ly)), axis(g.thumb_rx), flip(axis(g.thumb_ry))],
    }
}

#[cfg(windows)]
pub(crate) fn read(slot: u32) -> Option<Gamepad> {
    #[link(name = "xinput")]
    unsafe extern "system" {
        fn XInputGetState(user: u32, state: *mut XState) -> u32;
    }
    let mut s = XState::default();
    // SAFETY: one of XInput's four slots and a state for it to fill
    (unsafe { XInputGetState(slot, &mut s) } == 0).then_some(s.pad)
}

#[cfg(not(windows))]
pub(crate) fn read(_slot: u32) -> Option<Gamepad> {
    None
}

/// Whether the controllers are being read now.
static READING: AtomicBool = AtomicBool::new(false);

/// Every controller XInput has, read as they come and go: a slot read each time while a controller
/// is in it, an empty one looked at again once a second, as XInput is slow to say a slot is empty.
/// Every one counts — a PC may have several paired, and a VR program's own emulated pad among them
/// — so the one in the player's hands is read whichever slot it's in.
pub(crate) struct AllPads {
    connected: [bool; 4],
    looked: [Option<Instant>; 4],
}

impl AllPads {
    pub(crate) fn new() -> AllPads {
        AllPads { connected: [false; 4], looked: [None; 4] }
    }

    /// The state of each controller there now.
    pub(crate) fn read(&mut self) -> Vec<Gamepad> {
        let now = Instant::now();
        let mut pads = Vec::new();
        for slot in 0..4 {
            let at = slot as usize;
            if !self.connected[at] && self.looked[at].is_some_and(|t| now.duration_since(t) < Duration::from_secs(1)) {
                continue;
            }
            self.looked[at] = Some(now);
            match read(slot) {
                Some(g) => {
                    self.connected[at] = true;
                    pads.push(g);
                }
                None => self.connected[at] = false,
            }
        }
        pads
    }
}

/// The controllers read for the window `label` while it shows, on a thread of their own, every
/// 8 ms, each change sent to it as `pads`: every controller connected, in the web's standard layout.
/// A window hidden, to show again, is sent nothing meanwhile.
pub fn read_for(app: &AppHandle, label: &'static str) {
    if READING.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("vpi-pads".into()).spawn(move || {
        let shows = |app: &AppHandle| app.get_webview_window(label).is_some_and(|w| w.is_visible().unwrap_or(false));
        let mut all = AllPads::new();
        let mut last: Option<Vec<Pad>> = None;
        loop {
            while shows(&app) {
                let pads: Vec<Pad> = all.read().iter().map(standard).collect();
                if last.as_ref() != Some(&pads) {
                    let _ = app.emit_to(label, "pads", &pads);
                    last = Some(pads);
                }
                std::thread::sleep(Duration::from_millis(8));
            }
            READING.store(false, Ordering::Release);
            // shown again just after the last look, before another reader was asked for: read on,
            // the page told the controllers afresh
            if !shows(&app) || READING.swap(true, Ordering::AcqRel) {
                break;
            }
            last = None;
        }
    });
    if spawned.is_err() {
        READING.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_controller_in_the_webs_standard_layout() {
        // the pad pressed down, A held, the left stick pushed up and right, the right trigger pulled
        let g = Gamepad { buttons: 0x0002 | 0x1000, left_trigger: 0, right_trigger: 200, thumb_lx: 32767, thumb_ly: 32767, thumb_rx: 0, thumb_ry: -32768 };
        let p = standard(&g);
        let held: Vec<usize> = p.buttons.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| i).collect();
        assert_eq!(held, [0, 7, 13]);
        assert_eq!(p.axes, [1.0, -1.0, 0.0, 1.0]);
    }

    /// The controllers XInput lists on this PC now, as the menu's page would get them:
    /// `cargo test -p app xinput_on_this_pc -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn xinput_on_this_pc() {
        for slot in 0..4 {
            println!("slot {slot}: {:?}", read(slot).map(|g| standard(&g)));
        }
    }

    #[test]
    fn a_stick_left_alone_says_nothing() {
        let g = Gamepad { thumb_lx: 7000, thumb_ly: -7000, thumb_rx: 3000, thumb_ry: 0, ..Gamepad::default() };
        let p = standard(&g);
        assert_eq!(p.axes, [0.0, 0.0, 0.0, 0.0]);
        assert!(p.axes.iter().all(|a| a.is_sign_positive()));
        // pushed halfway: in twentieths
        assert_eq!(axis(16384), 0.5);
        assert_eq!(axis(-16384), -0.5);
    }
}
