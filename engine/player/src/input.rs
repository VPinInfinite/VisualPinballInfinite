//! Input: key presses reach the game thread the moment they arrive.
//!
//! SDL3's raw keyboard mode on Windows reads the keyboard on a thread of its
//! own at the highest priority. An event watch runs on whichever thread pushes
//! an event, so presses are timestamped and handed to the game thread right
//! there, never waiting for the window thread's message loop or for drawing.
//!
//! Every key, mouse button and joystick element goes through VPX 5436's
//! mappings (`input_map`): the actions they press reach the script as the
//! keys VPX 10.8.0 hands it, a key no action has as itself. SDL reads
//! joysticks and gamepads as the window thread pumps
//! its events, at least once a frame. Every joystick axis's latest reading is
//! kept for the physics to read each millisecond (`sensors`): the nudge and
//! plunger sensors the settings name, and those of VPX's default layouts — a
//! gamepad's left stick nudging and right stick the plunger, a Pinscape
//! board's accelerometer and plunger (`nudge`, `plunger_input`).

use std::collections::HashMap;
use std::ffi::{CStr, c_void};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use sdl3_sys::everything::*;

use crate::input_map::{self, GamepadElements, Input, InputMap, Kind};
use crate::waker::Waker;

/// A key press or release, as table scripts see it.
#[derive(Clone, Copy, Debug)]
pub struct InputEvent {
    /// Counts up from 1, for following one press through to the screen.
    pub id: u64,
    /// When SDL received it, on SDL's clock (`SDL_GetTicksNS`).
    pub time_ns: u64,
    /// The DirectInput key code VPX gives scripts.
    pub dik: u8,
    pub down: bool,
}

/// A scripted list of key presses for test runs: `ms:key:d|u` items
/// separated by commas (`500:1:d,600:1:u`), as (ms, DirectInput code, down).
/// A key is a digit key (`1`, `5`), a name (`lshift`, `rshift`, `enter`,
/// `b`, `z`, `slash`, `space`), or a DirectInput code in hex (`2c`).
pub fn parse_key_script(list: &str) -> Option<Vec<(u64, u8, bool)>> {
    list.split(',')
        .map(|item| {
            let mut f = item.split(':');
            let (t, k, d) = (f.next()?, f.next()?, f.next()?);
            Some((t.parse().ok()?, key_named(k)?, d == "d"))
        })
        .collect()
}

/// A key's DirectInput code, by name (see [`parse_key_script`]).
fn key_named(k: &str) -> Option<u8> {
    let k = k.to_lowercase();
    Some(match k.as_str() {
        "lshift" => 0x2A,
        "rshift" => 0x36,
        "enter" => 0x1C,
        "b" => 0x30,
        "z" => 0x2C,
        "slash" => 0x35,
        "space" => 0x39,
        // the pause menu's actions (`pause_menu::code`)
        "pause" => crate::pause_menu::code::PAUSE,
        "ui-up" => crate::pause_menu::code::UP,
        "ui-down" => crate::pause_menu::code::DOWN,
        "ui-left" => crate::pause_menu::code::LEFT,
        "ui-right" => crate::pause_menu::code::RIGHT,
        // Esc as the window hands it on when it opens the menu first
        "exit" => crate::pause_menu::code::EXIT,
        "esc" => 0x01,
        // DIK_1 is 0x02 up to DIK_9 0x0A, then DIK_0 0x0B
        "0" => 0x0B,
        d if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() => d.as_bytes()[0] - b'1' + 0x02,
        h => return u8::from_str_radix(h, 16).ok(),
    })
}

/// Where the event watch sends input.
pub struct InputSink {
    tx: Sender<InputEvent>,
    waker: Arc<Waker>,
    /// Also woken by a key: the window thread, held back by the frame-rate
    /// limit, so a press gets its frame at once.
    frames: Option<Arc<Waker>>,
    next_id: AtomicU64,
    /// Which button does what, and the joysticks connected: held only briefly, never while
    /// calling SDL (which can push events through the watch on the same thread).
    mapping: Mutex<Mapping>,
}

struct Mapping {
    map: InputMap,
    /// The joysticks open, by SDL's instance number: their devices in the map, and SDL's
    /// handles for closing them.
    joysticks: HashMap<u32, (u16, usize)>,
}

impl InputSink {
    pub fn new(tx: Sender<InputEvent>, waker: Arc<Waker>, map: InputMap) -> InputSink {
        InputSink { tx, waker, frames: None, next_id: AtomicU64::new(1), mapping: Mutex::new(Mapping { map, joysticks: HashMap::new() }) }
    }

    /// An element of a device changed: what the actions mapped to it make the script hear,
    /// handed on.
    fn element(&self, device: u16, element: u16, input: Input, time_ns: u64) {
        let mut out = Vec::new();
        let player = {
            let mut m = self.mapping.lock().unwrap_or_else(|e| e.into_inner());
            m.map.element(device, element, input, &mut out);
            m.map.take_player_actions()
        };
        for (dik, down) in out {
            self.send(time_ns, dik, down);
        }
        self.player_actions(time_ns, player);
    }

    /// An element of a device read outside SDL's events (an Open Pinball Device's buttons and
    /// axes, `opd`): handed on as a joystick's are.
    pub fn device_element(&self, device: u16, element: u16, input: Input, time_ns: u64) {
        self.element(device, element, input, time_ns);
    }

    /// An Open Pinball Device found at the start (VPX's `OpenPinDevHandler` registering one): its
    /// device in the map, named `OpenPinDev` as VPX names it, with VPX's default mapping for it —
    /// its pinball buttons, its plunger and its nudge — unless the settings set it up.
    pub fn open_pin_device_added(&self) -> u16 {
        let mut m = self.mapping.lock().unwrap_or_else(|e| e.into_inner());
        let device = m.map.device("OpenPinDev");
        if !m.map.is_set_up(device) {
            for (action, bind) in input_map::default_layout(Kind::OpenPinDev, device, None) {
                m.map.add(action, bind);
            }
            let (nudge, plunger) = crate::sensors::default_layout(Kind::OpenPinDev, device, None);
            crate::sensors::add_layout(nudge, plunger);
        }
        device
    }

    /// A joystick's device in the map, if it's open.
    fn joystick(&self, which: SDL_JoystickID) -> Option<u16> {
        self.mapping.lock().unwrap_or_else(|e| e.into_inner()).joysticks.get(&which.0).map(|j| j.0)
    }

    /// A joystick SDL reports as connected (or there at the start), on the window thread:
    /// opened, so its buttons, hats and axes arrive; named as VPX names it in the settings; given
    /// VPX's default layout for it unless the settings set it up (VPX's `OnJoystickAdded`). A
    /// gamepad stays open as one too: its sticks nudge and are the plunger, and it rumbles. Says
    /// which, once.
    ///
    /// # Safety
    /// SDL's joystick subsystem must be started, on this thread.
    pub unsafe fn joystick_added(&self, which: SDL_JoystickID) {
        if self.joystick(which).is_some() {
            return;
        }
        // SAFETY: SDL's joystick calls on its thread, with the instance SDL reported; the lock
        // isn't held across them
        let (joystick, setting_id, name, kind, pad) = unsafe {
            let joystick = SDL_OpenJoystick(which);
            if joystick.is_null() {
                return;
            }
            let text = |p: *const std::ffi::c_char| if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() };
            let guid = SDL_GetJoystickGUID(joystick);
            let sdl_name = text(SDL_GetJoystickName(joystick));
            // which of the connected ones with this GUID, and with this name, it is (VPX's idIndex, nameIndex)
            let (mut id_index, mut name_index, mut count) = (1, 1, 0);
            let ids = SDL_GetJoysticks(&mut count);
            if !ids.is_null() {
                for k in 0..count.max(0) as usize {
                    let other = *ids.add(k);
                    if other == which {
                        break;
                    }
                    if SDL_GetJoystickGUIDForID(other).data == guid.data {
                        id_index += 1;
                    }
                    if text(SDL_GetJoystickNameForID(other)) == sdl_name {
                        name_index += 1;
                    }
                }
                SDL_free(ids.cast());
            }
            let mut guid_text = [0 as std::ffi::c_char; 33];
            SDL_GUIDToString(guid, guid_text.as_mut_ptr(), 33);
            let setting_id = input_map::joystick_setting_id(&text(guid_text.as_ptr()), id_index);
            let kind = if sdl_name.starts_with("mjrnet Pinscape Controller") && SDL_GetNumJoystickAxes(joystick) >= 6 {
                Kind::Pinscape
            } else if ARCADE2TV_PINBALL.contains(&guid.data) {
                Kind::Arcade2TvPinball
            } else if ARCADE2TV_RIGHT.contains(&guid.data) {
                Kind::Skip
            } else {
                Kind::Unknown
            };
            let name = match kind {
                Kind::Arcade2TvPinball => format!("Arcade2TV-XR #{id_index}"),
                Kind::Skip => format!("Arcade2TV-XR Right #{id_index}"),
                _ => format!("{sdl_name} #{name_index}"),
            };
            let pad = if SDL_IsGamepad(which) { open_gamepad(which) } else { None };
            (joystick, setting_id, name, kind, pad)
        };
        let mut m = self.mapping.lock().unwrap_or_else(|e| e.into_inner());
        let device = m.map.device(&setting_id);
        m.joysticks.insert(which.0, (device, joystick as usize));
        let set_up = m.map.is_set_up(device);
        let layout = if set_up { Vec::new() } else { input_map::default_layout(kind, device, pad.as_ref()) };
        // the layout's nudge and plunger sensors, as VPX's layouts add them
        let (nudge, plunger) = if set_up { (None, None) } else { crate::sensors::default_layout(kind, device, pad.as_ref()) };
        crate::sensors::add_layout(nudge, plunger);
        let what = if set_up {
            "the settings' mapping"
        } else if kind == Kind::Pinscape {
            "VPX's Pinscape layout: its accelerometer nudges, its plunger plunges"
        } else if layout.is_empty() {
            "no buttons mapped (none in the settings, and VPX has no layout for it)"
        } else if kind == Kind::Arcade2TvPinball {
            "VPX's Arcade2TV-XR pinball layout"
        } else {
            "VPX's gamepad layout: the triggers flip (past a third of the pull, the staged flippers too past two thirds), shoulders magna-save, Y coin, B start, A launch, the pad the operator's buttons"
        };
        for (action, bind) in layout {
            m.map.add(action, bind);
        }
        drop(m);
        let sticks = if pad.is_some() && nudge.is_some() { "; the left stick nudges, the right stick is the plunger" } else { "" };
        println!("controller: {name} ({setting_id}): {what}{sticks}");
    }

    /// A joystick unplugged: whatever it held let go, and closed.
    ///
    /// # Safety
    /// On SDL's thread.
    pub unsafe fn joystick_removed(&self, which: SDL_JoystickID) {
        let mut out = Vec::new();
        let gone = {
            let mut m = self.mapping.lock().unwrap_or_else(|e| e.into_inner());
            let gone = m.joysticks.remove(&which.0);
            if let Some((device, _)) = gone {
                m.map.release(device, &mut out);
            }
            let player = m.map.take_player_actions();
            drop(m);
            // SAFETY: SDL's ticks
            self.player_actions(unsafe { SDL_GetTicksNS() }, player);
            gone
        };
        // SAFETY: SDL's ticks
        let now = unsafe { SDL_GetTicksNS() };
        for (dik, down) in out {
            self.send(now, dik, down);
        }
        if let Some((_, handle)) = gone {
            // SAFETY: the joystick opened when it was added, closed once
            unsafe { SDL_CloseJoystick(handle as *mut SDL_Joystick) };
        }
    }

    /// Hands a press or release to the game thread, waking it and the window thread; a
    /// flipper's press rumbles the controller, as VPX's flipper actions do.
    fn send(&self, time_ns: u64, dik: u8, down: bool) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.tx.send(InputEvent { id, time_ns, dik, down });
        self.waker.wake();
        if let Some(frames) = &self.frames {
            frames.wake();
        }
        crate::rumble::on_key(dik, down);
    }

    /// Wakes `frames` too whenever a key arrives.
    pub fn waking_frames(self, frames: Arc<Waker>) -> InputSink {
        InputSink { frames: Some(frames), ..self }
    }
}

impl InputSink {
    /// An action's keys and buttons now, by name, for the pause menu's Controls page: each of its
    /// mappings as the keyboard, the mouse and the controllers connected name it ("Left Shift",
    /// "LT", "Left click"), buttons held together joined by "+"; a controller's that isn't
    /// connected left out.
    ///
    /// # Safety
    /// On SDL's thread (the window's).
    pub unsafe fn names(&self, action: &str) -> Vec<String> {
        let Some(a) = input_map::action(action) else { return Vec::new() };
        // the mappings, each device's name in the settings and its joystick, read under the lock
        // and named after it, SDL called without it
        let (groups, devices) = {
            let m = self.mapping.lock().unwrap_or_else(|e| e.into_inner());
            let groups = m.map.groups(a);
            let devices: HashMap<u16, (String, Option<u32>)> = groups
                .iter()
                .flatten()
                .map(|b| (b.device, (m.map.device_name(b.device).to_string(), m.joysticks.iter().find(|(_, j)| j.0 == b.device).map(|(&which, _)| which))))
                .collect();
            (groups, devices)
        };
        let mut out: Vec<String> = Vec::new();
        for g in &groups {
            // SAFETY: on SDL's thread, as the caller promises
            let names: Option<Vec<String>> = g.iter().map(|b| unsafe { bind_name(b, devices.get(&b.device)) }).collect();
            if let Some(names) = names.filter(|n| !n.is_empty()) {
                let text = names.join(" + ");
                if !out.contains(&text) {
                    out.push(text);
                }
            }
        }
        out
    }

    /// The player's own actions pressed or let go: the volume keys, a screenshot, and the pause
    /// menu's, handed to the game thread as codes no key has (`pause_menu::code`).
    fn player_actions(&self, time_ns: u64, actions: Vec<(crate::input_map::PlayerAction, bool)>) {
        use crate::input_map::PlayerAction as A;
        use crate::pause_menu::code;
        for (action, down) in actions {
            let menu = match action {
                A::VolumeDown | A::VolumeUp => {
                    crate::volume::key(action, down);
                    None
                }
                A::Screenshot => {
                    if down {
                        crate::screenshot::press();
                    }
                    None
                }
                A::Pause => Some(code::PAUSE),
                A::InGameUI => Some(code::IN_GAME_UI),
                A::UIUp => Some(code::UP),
                A::UIDown => Some(code::DOWN),
                A::UILeft => Some(code::LEFT),
                A::UIRight => Some(code::RIGHT),
            };
            if let Some(code) = menu {
                self.send(time_ns, code, down);
            }
        }
    }
}

/// A button's name, for the Controls page: a key as SDL names it (Enter for Return), a mouse
/// button by what it is, a gamepad's as the gamepad's labels and the usual words name it, another
/// controller's by its name and number; none for a controller that isn't connected. `device`: its
/// name in the settings, and its joystick when one is open.
///
/// # Safety
/// On SDL's thread.
unsafe fn bind_name(b: &input_map::Bind, device: Option<&(String, Option<u32>)>) -> Option<String> {
    let number = |base: u16| b.element - base + 1;
    match b.device {
        input_map::KEYBOARD => {
            // SAFETY: SDL's own table of names
            let name = unsafe { CStr::from_ptr(SDL_GetScancodeName(SDL_Scancode(i32::from(b.element)))) }.to_string_lossy().into_owned();
            Some(match name.as_str() {
                "Return" => "Enter".to_string(),
                "" => format!("key {}", b.element),
                _ => name,
            })
        }
        input_map::MOUSE => Some(match b.element {
            1 => "Left click".to_string(),
            2 => "Middle click".to_string(),
            3 => "Right click".to_string(),
            4 => "Mouse back".to_string(),
            5 => "Mouse forward".to_string(),
            n => format!("Mouse button {n}"),
        }),
        _ => {
            let (setting, which) = device?;
            let plain = || {
                if b.element >= input_map::AXIS {
                    format!("axis {}", number(input_map::AXIS))
                } else if b.element >= input_map::HAT {
                    "hat".to_string()
                } else {
                    format!("button {}", b.element + 1)
                }
            };
            if setting == "OpenPinDev" {
                return Some(format!("Open Pinball Device {}", plain()));
            }
            let which = SDL_JoystickID((*which)?);
            // SAFETY: on SDL's thread, as the caller promises
            unsafe {
                if let Some(name) = pad_name(which, b.element) {
                    return Some(name);
                }
                let joystick = SDL_GetJoystickNameForID(which);
                let label = if joystick.is_null() { "Controller".to_string() } else { CStr::from_ptr(joystick).to_string_lossy().chars().take(24).collect::<String>().trim().to_string() };
                Some(format!("{label} {}", plain()))
            }
        }
    }
}

/// A gamepad's element as its own labels and the usual words name it: its face buttons A, B, X
/// and Y (or Cross, Circle, Square and Triangle), LB, RB, LT, RT, the D-pad's four, Back, Start,
/// the sticks; none for a joystick that isn't a gamepad, or an element its mapping doesn't name.
///
/// # Safety
/// On SDL's thread.
unsafe fn pad_name(which: SDL_JoystickID, element: u16) -> Option<String> {
    const FACES: [(SDL_GamepadButtonLabel, &str); 8] = [
        (SDL_GAMEPAD_BUTTON_LABEL_A, "A"),
        (SDL_GAMEPAD_BUTTON_LABEL_B, "B"),
        (SDL_GAMEPAD_BUTTON_LABEL_X, "X"),
        (SDL_GAMEPAD_BUTTON_LABEL_Y, "Y"),
        (SDL_GAMEPAD_BUTTON_LABEL_CROSS, "Cross"),
        (SDL_GAMEPAD_BUTTON_LABEL_CIRCLE, "Circle"),
        (SDL_GAMEPAD_BUTTON_LABEL_SQUARE, "Square"),
        (SDL_GAMEPAD_BUTTON_LABEL_TRIANGLE, "Triangle"),
    ];
    const BUTTONS: [(SDL_GamepadButton, &str); 11] = [
        (SDL_GAMEPAD_BUTTON_BACK, "Back"),
        (SDL_GAMEPAD_BUTTON_GUIDE, "Guide"),
        (SDL_GAMEPAD_BUTTON_START, "Start"),
        (SDL_GAMEPAD_BUTTON_LEFT_STICK, "Left stick press"),
        (SDL_GAMEPAD_BUTTON_RIGHT_STICK, "Right stick press"),
        (SDL_GAMEPAD_BUTTON_LEFT_SHOULDER, "LB"),
        (SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER, "RB"),
        (SDL_GAMEPAD_BUTTON_DPAD_UP, "D-pad up"),
        (SDL_GAMEPAD_BUTTON_DPAD_DOWN, "D-pad down"),
        (SDL_GAMEPAD_BUTTON_DPAD_LEFT, "D-pad left"),
        (SDL_GAMEPAD_BUTTON_DPAD_RIGHT, "D-pad right"),
    ];
    // SAFETY: as the caller promises; SDL's list of bindings read, then freed
    unsafe {
        let pad = SDL_GetGamepadFromID(which);
        if pad.is_null() {
            return None;
        }
        let mut count = 0;
        let list = SDL_GetGamepadBindings(pad, &mut count);
        if list.is_null() {
            return None;
        }
        let bindings: Vec<SDL_GamepadBinding> = (0..count.max(0) as usize).map(|k| **list.add(k)).collect();
        SDL_free(list.cast());
        // the binding whose input is this element, as the event watch numbers them
        let hit = bindings.iter().find(|x| {
            if x.input_type == SDL_GAMEPAD_BINDTYPE_BUTTON {
                u16::try_from(x.input.button).ok() == Some(element)
            } else if x.input_type == SDL_GAMEPAD_BINDTYPE_HAT {
                let base = input_map::HAT | (x.input.hat.hat as u16 * 4);
                [SDL_HAT_LEFT, SDL_HAT_RIGHT, SDL_HAT_UP, SDL_HAT_DOWN].iter().position(|&m| i32::from(m) == x.input.hat.hat_mask).map(|k| base | k as u16) == Some(element)
            } else if x.input_type == SDL_GAMEPAD_BINDTYPE_AXIS {
                (input_map::AXIS | x.input.axis.axis as u16) == element
            } else {
                false
            }
        })?;
        if hit.output_type == SDL_GAMEPAD_BINDTYPE_BUTTON {
            let button = hit.output.button;
            let label = SDL_GetGamepadButtonLabel(pad, button);
            // a face button by its label (the others have none), else by what it is
            let name = FACES
                .iter()
                .find(|(l, _)| *l == label)
                .map(|(_, n)| *n)
                .or_else(|| BUTTONS.iter().find(|(b, _)| *b == button).map(|(_, n)| *n))
                .unwrap_or("a button");
            Some(name.to_string())
        } else if hit.output_type == SDL_GAMEPAD_BINDTYPE_AXIS {
            let axis = hit.output.axis.axis;
            Some(
                if axis == SDL_GAMEPAD_AXIS_LEFT_TRIGGER {
                    "LT"
                } else if axis == SDL_GAMEPAD_AXIS_RIGHT_TRIGGER {
                    "RT"
                } else if axis == SDL_GAMEPAD_AXIS_LEFTX || axis == SDL_GAMEPAD_AXIS_LEFTY {
                    "Left stick"
                } else {
                    "Right stick"
                }
                .to_string(),
            )
        } else {
            None
        }
    }
}

/// The Arcade2TV-XR's GUIDs, its pinball mode's and its second stick's (VPX's `arcade2TVGUIDs`).
const ARCADE2TV_PINBALL: [[u8; 16]; 2] = [
    [0x03, 0x00, 0xfa, 0x66, 0x5e, 0x04, 0x00, 0x00, 0x8e, 0x02, 0x00, 0x00, 0x02, 0x03, 0x78, 0x01],
    [0x03, 0x00, 0xfa, 0x67, 0x5e, 0x04, 0x00, 0x00, 0x8e, 0x02, 0x00, 0x00, 0x40, 0x03, 0x78, 0x01],
];
const ARCADE2TV_RIGHT: [[u8; 16]; 2] = [
    [0x03, 0x00, 0xba, 0x67, 0x5e, 0x04, 0x00, 0x00, 0x8e, 0x02, 0x00, 0x00, 0x02, 0x03, 0x78, 0x01],
    [0x03, 0x00, 0xba, 0x66, 0x5e, 0x04, 0x00, 0x00, 0x8e, 0x02, 0x00, 0x00, 0x40, 0x03, 0x78, 0x01],
];

/// SDL's event watch: hands keys (not repeats), mouse buttons and joysticks' buttons, hats
/// and axes to the mappings, and joysticks' axes to the sensors too.
unsafe extern "C" fn watch(userdata: *mut c_void, event: *mut SDL_Event) -> bool {
    // SAFETY: SDL passes the event being pushed and the userdata registered in
    // `install`, which lives until the program ends
    unsafe {
        let sink = &*(userdata as *const InputSink);
        let kind = (*event).r#type;
        if kind == SDL_EVENT_KEY_DOWN.0 || kind == SDL_EVENT_KEY_UP.0 {
            let key = (*event).key;
            if !key.repeat {
                sink.element(input_map::KEYBOARD, key.scancode.0 as u16, Input::Button(key.down), key.timestamp);
            }
        } else if kind == SDL_EVENT_MOUSE_BUTTON_DOWN.0 || kind == SDL_EVENT_MOUSE_BUTTON_UP.0 {
            let b = (*event).button;
            if b.which != SDL_TOUCH_MOUSEID && b.which != SDL_PEN_MOUSEID {
                sink.element(input_map::MOUSE, u16::from(b.button), Input::Button(b.down), b.timestamp);
            }
        } else if kind == SDL_EVENT_JOYSTICK_BUTTON_DOWN.0 || kind == SDL_EVENT_JOYSTICK_BUTTON_UP.0 {
            let b = (*event).jbutton;
            if let Some(device) = sink.joystick(b.which) {
                sink.element(device, u16::from(b.button), Input::Button(b.down), b.timestamp);
            }
        } else if kind == SDL_EVENT_JOYSTICK_HAT_MOTION.0 {
            let h = (*event).jhat;
            if let Some(device) = sink.joystick(h.which) {
                // a hat's four directions as buttons: left, right, up, down
                let base = input_map::HAT | (u16::from(h.hat) * 4);
                for (k, mask) in [SDL_HAT_LEFT, SDL_HAT_RIGHT, SDL_HAT_UP, SDL_HAT_DOWN].into_iter().enumerate() {
                    sink.element(device, base | k as u16, Input::Button(h.value & mask != 0), h.timestamp);
                }
            }
        } else if kind == SDL_EVENT_JOYSTICK_AXIS_MOTION.0 {
            let a = (*event).jaxis;
            if let Some(device) = sink.joystick(a.which) {
                let (element, value) = (input_map::AXIS | u16::from(a.axis), f32::from(a.value) / 32768.0);
                crate::sensors::set_axis(device, element, a.timestamp, value);
                sink.element(device, element, Input::Axis(value), a.timestamp);
            }
        }
    }
    true
}

/// Opens a gamepad as one, so it rumbles; it stays open while the player runs (SDL closes it as
/// it quits). Returns where its buttons, triggers, sticks and pad are on the joystick (VPX's
/// `GetGamepadButton`, `GetGamepadAxis`).
///
/// # Safety
/// SDL's gamepad subsystem must be started, on this thread.
unsafe fn open_gamepad(which: SDL_JoystickID) -> Option<GamepadElements> {
    // SAFETY: as the caller promises; SDL's list of bindings read, then freed
    unsafe {
        let pad = SDL_OpenGamepad(which);
        if pad.is_null() {
            return None;
        }
        let mut count = 0;
        let list = SDL_GetGamepadBindings(pad, &mut count);
        if list.is_null() {
            return None;
        }
        let bindings: Vec<SDL_GamepadBinding> = (0..count.max(0) as usize).map(|k| **list.add(k)).collect();
        SDL_free(list.cast());
        let button = |b: SDL_GamepadButton| -> Option<u16> {
            bindings.iter().filter(|x| x.output_type == SDL_GAMEPAD_BINDTYPE_BUTTON && x.output.button == b).find_map(|x| {
                if x.input_type == SDL_GAMEPAD_BINDTYPE_BUTTON {
                    u16::try_from(x.input.button).ok()
                } else if x.input_type == SDL_GAMEPAD_BINDTYPE_HAT {
                    let base = input_map::HAT | (x.input.hat.hat as u16 * 4);
                    [SDL_HAT_LEFT, SDL_HAT_RIGHT, SDL_HAT_UP, SDL_HAT_DOWN].iter().position(|&m| i32::from(m) == x.input.hat.hat_mask).map(|k| base | k as u16)
                } else {
                    None
                }
            })
        };
        let axis = |a: SDL_GamepadAxis| -> Option<u16> {
            bindings
                .iter()
                .find(|x| x.input_type == SDL_GAMEPAD_BINDTYPE_AXIS && x.output_type == SDL_GAMEPAD_BINDTYPE_AXIS && x.output.axis.axis == a)
                .map(|x| input_map::AXIS | x.input.axis.axis as u16)
        };
        let dpad = [SDL_GAMEPAD_BUTTON_DPAD_LEFT, SDL_GAMEPAD_BUTTON_DPAD_DOWN, SDL_GAMEPAD_BUTTON_DPAD_UP, SDL_GAMEPAD_BUTTON_DPAD_RIGHT].map(button);
        Some(GamepadElements {
            left_trigger: axis(SDL_GAMEPAD_AXIS_LEFT_TRIGGER),
            right_trigger: axis(SDL_GAMEPAD_AXIS_RIGHT_TRIGGER),
            left_stick_x: axis(SDL_GAMEPAD_AXIS_LEFTX),
            left_stick_y: axis(SDL_GAMEPAD_AXIS_LEFTY),
            right_stick_y: axis(SDL_GAMEPAD_AXIS_RIGHTY),
            left_shoulder: button(SDL_GAMEPAD_BUTTON_LEFT_SHOULDER),
            right_shoulder: button(SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER),
            south: button(SDL_GAMEPAD_BUTTON_SOUTH),
            north: button(SDL_GAMEPAD_BUTTON_NORTH),
            east: button(SDL_GAMEPAD_BUTTON_EAST),
            back: button(SDL_GAMEPAD_BUTTON_BACK),
            start: button(SDL_GAMEPAD_BUTTON_START),
            dpad: if dpad.iter().all(Option::is_some) { Some(dpad.map(|d| d.unwrap_or(0))) } else { None },
        })
    }
}

/// `vpi-player --input-test`, for the app's Controls page: the game actions
/// with their mappings, the controllers as they come and go, each element a controller presses or
/// moves, the actions that makes by the settings' mappings — VPX's default layout for a gamepad
/// included, as when playing — and the nudge and plunger, one line of JSON each on the standard
/// output, until the standard input closes. No window, so no keyboard: the page reads its own.
///
/// # Safety
/// On the main thread, before anything else starts SDL.
pub unsafe fn input_test(settings: &crate::settings::Settings) -> Result<(), String> {
    use std::io::Write;
    use std::sync::atomic::AtomicBool;
    let say = |v: serde_json::Value| {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{v}");
        let _ = out.flush();
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let mut map = InputMap::new(settings);
    let sensors = crate::sensors::Sensors::from_settings(settings, &mut map);
    map.sensors_read(&sensors.devices());
    let sink = InputSink::new(tx, Arc::new(Waker::new()), map);
    let ids = input_map::action_ids();
    let actions = |sink: &InputSink| {
        let m = sink.mapping.lock().unwrap_or_else(|e| e.into_inner());
        let list: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(a, id)| serde_json::json!({ "id": id, "mapping": m.map.mapping_text(a), "default": input_map::default_mapping(a) }))
            .collect();
        serde_json::json!({ "t": "actions", "list": list })
    };
    // SAFETY: SDL started on this thread, joysticks heard without a window of the program's own
    unsafe {
        SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1".as_ptr());
        if !SDL_Init(SDL_INIT_JOYSTICK | SDL_INIT_GAMEPAD) {
            return Err(format!("SDL's joysticks didn't start: {}", CStr::from_ptr(SDL_GetError()).to_string_lossy()));
        }
    }
    // the keys' names, as SDL names them, for the page to show a key mapping by
    let mut keys = serde_json::Map::new();
    for code in 1..512 {
        // SAFETY: SDL's own table of names
        let name = unsafe { CStr::from_ptr(SDL_GetScancodeName(SDL_Scancode(code))) }.to_string_lossy().into_owned();
        if !name.is_empty() {
            keys.insert(code.to_string(), serde_json::Value::String(name));
        }
    }
    say(serde_json::json!({ "t": "keys", "names": keys }));
    say(actions(&sink));
    // the page closing its end: stop
    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut line = String::new();
            while std::io::stdin().read_line(&mut line).is_ok_and(|n| n > 0) {
                line.clear();
            }
            stop.store(true, Ordering::Relaxed);
        });
    }
    let mut pressed = sink.mapping.lock().unwrap_or_else(|e| e.into_inner()).map.pressed();
    let mut shown = (None, None);
    while !stop.load(Ordering::Relaxed) {
        // SAFETY: an event of zeros, for SDL to fill in
        let mut ev: SDL_Event = unsafe { std::mem::zeroed() };
        // SAFETY: SDL's events, on its thread
        while unsafe { SDL_WaitEventTimeout(&mut ev, 30) } {
            // SAFETY: the event SDL filled in, read by its type
            unsafe {
                let kind = ev.r#type;
                let device_name = |which: SDL_JoystickID| {
                    let m = sink.mapping.lock().unwrap_or_else(|e| e.into_inner());
                    m.joysticks.get(&which.0).map(|j| m.map.device_name(j.0).to_string())
                };
                let element = |which: SDL_JoystickID, element: u16, value: f32, input: Input, time: u64| {
                    if let Some(device) = sink.joystick(which) {
                        let name = sink.mapping.lock().unwrap_or_else(|e| e.into_inner()).map.device_name(device).to_string();
                        say(serde_json::json!({ "t": "element", "device": name, "element": element, "value": value }));
                        if element >= input_map::AXIS {
                            crate::sensors::set_axis(device, element, time, value);
                        }
                        sink.element(device, element, input, time);
                    }
                };
                if kind == SDL_EVENT_JOYSTICK_ADDED.0 {
                    let which = ev.jdevice.which;
                    sink.joystick_added(which);
                    let joystick = SDL_GetJoystickFromID(which);
                    let label = if joystick.is_null() { String::new() } else { CStr::from_ptr(SDL_GetJoystickName(joystick)).to_string_lossy().into_owned() };
                    if let Some(id) = device_name(which) {
                        say(serde_json::json!({ "t": "device", "id": id, "name": label, "on": true }));
                        say(actions(&sink));
                    }
                } else if kind == SDL_EVENT_JOYSTICK_REMOVED.0 {
                    let which = ev.jdevice.which;
                    let id = device_name(which);
                    sink.joystick_removed(which);
                    if let Some(id) = id {
                        say(serde_json::json!({ "t": "device", "id": id, "on": false }));
                    }
                } else if kind == SDL_EVENT_JOYSTICK_BUTTON_DOWN.0 || kind == SDL_EVENT_JOYSTICK_BUTTON_UP.0 {
                    let b = ev.jbutton;
                    element(b.which, u16::from(b.button), if b.down { 1.0 } else { 0.0 }, Input::Button(b.down), b.timestamp);
                } else if kind == SDL_EVENT_JOYSTICK_HAT_MOTION.0 {
                    let h = ev.jhat;
                    let base = input_map::HAT | (u16::from(h.hat) * 4);
                    for (k, mask) in [SDL_HAT_LEFT, SDL_HAT_RIGHT, SDL_HAT_UP, SDL_HAT_DOWN].into_iter().enumerate() {
                        let down = h.value & mask != 0;
                        element(h.which, base | k as u16, if down { 1.0 } else { 0.0 }, Input::Button(down), h.timestamp);
                    }
                } else if kind == SDL_EVENT_JOYSTICK_AXIS_MOTION.0 {
                    let a = ev.jaxis;
                    let value = f32::from(a.value) / 32768.0;
                    element(a.which, input_map::AXIS | u16::from(a.axis), value, Input::Axis(value), a.timestamp);
                }
            }
            // the actions that changed
            let now = sink.mapping.lock().unwrap_or_else(|e| e.into_inner()).map.pressed();
            for (a, (&was, &is)) in pressed.iter().zip(&now).enumerate() {
                if was != is {
                    say(serde_json::json!({ "t": "action", "id": ids[a], "on": is }));
                }
            }
            pressed = now;
            while rx.try_recv().is_ok() {}
        }
        // the nudge and plunger, when they've moved
        let (nudge, plunger) = crate::sensors::live(&sensors);
        let rounded = (nudge.map(|[x, y]| [(x * 100.0).round() as i32, (y * 100.0).round() as i32]), plunger.map(|p| (p * 100.0).round() as i32));
        if rounded != shown {
            shown = rounded;
            say(serde_json::json!({ "t": "sensors", "nudge": nudge, "plunger": plunger }));
        }
        while rx.try_recv().is_ok() {}
    }
    // SAFETY: SDL shut down on its thread
    unsafe { SDL_Quit() };
    Ok(())
}

/// Asks SDL for its low-latency keyboard mode; call before `SDL_Init`.
pub fn request_raw_keyboard() {
    // SAFETY: setting a hint with static strings
    unsafe {
        SDL_SetHint(SDL_HINT_WINDOWS_RAW_KEYBOARD, c"1".as_ptr());
    }
}

/// Installs the event watch. The sink stays alive for the whole program; the window thread
/// keeps its own handle, for joysticks connecting and going.
pub fn install(sink: Arc<InputSink>) -> bool {
    let sink = Arc::into_raw(sink);
    // SAFETY: the watch and its userdata (a handle never released) outlive SDL
    unsafe { SDL_AddEventWatch(Some(watch), sink as *mut c_void) }
}
