// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its input actions
// and their mappings (src/input/InputAction.cpp, ButtonMapping.h; InputManager.cpp's CreateInputActions, its
// devices and PushButtonEvent/PushAxisEvent) and its joysticks' default layouts (src/input/SDLInputHandler.h).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! Which button does what: VPX 5436's game actions —
//! the flippers and staged flippers, the magna-saves, the ball's launch, the three nudges,
//! tilt, four coins, start, extra ball, the lockbar, slam tilt, the coin door, reset and eight
//! service buttons — each mapped to any buttons of any device: a key, a mouse button, a
//! joystick's button, a direction of its hat, or one of its axes past a point. The mappings
//! come from the settings under VPX's names (`[Input]` `Mapping.LeftFlipper = Key;225 |
//! SDLJoy_…_1;516;o;-0.300000`: `|` between alternatives, `&` between buttons held together),
//! else VPX's keyboard defaults; a gamepad SDL knows gets VPX's default gamepad layout, unless
//! the settings map anything to it or set its `Device.<id>.NoAutoLayout` (VPX offers the layout
//! in its in-game menu; ours applies it until our settings screen offers the choice).
//!
//! Two of VPX's actions are the player's own, never heard by the script: the volume keys
//! (`VolumeDown` on `-`, `VolumeUp` on `=`, handed to `volume`). The `-` key is the seventh
//! service button's too, so it does both, as in VPX 5436.
//!
//! What a script hears stays VPX 10.8.0's: an action's press as 10.8.0's key for it —
//! the left flipper as `LeftFlipperKey`, whatever pressed it — and a key no action has as
//! itself, as both VPX versions hand any key to scripts. Two actions with the same key
//! (VPX 5436 puts the fourth coin and the fifth service button both on 6) press it once.

use sdl3_sys::everything::*;

use crate::dik;

/// The keyboard's device (VPX's `Key`: every keyboard together), its elements SDL's scancodes.
pub const KEYBOARD: u16 = 0;
/// The mouse's (VPX's `Mouse`), its elements SDL's button numbers.
pub const MOUSE: u16 = 1;

/// A joystick's elements as VPX numbers them: its buttons from 0, its hats' directions from
/// `0x0100` (four a hat: left, right, up, down), its axes from `0x0200`.
pub const HAT: u16 = 0x0100;
pub const AXIS: u16 = 0x0200;

/// One of VPX 5436's game actions: its name in the settings, its default key (an SDL scancode;
/// none for the eighth service button), and the key VPX 10.8.0 hands scripts for it — its named
/// key where 10.8.0 names one, else the default key's own code.
struct ActionDef {
    id: &'static str,
    key: SDL_Scancode,
    dik: Option<u8>,
}

/// VPX 5436's actions that reach the table's script, in its order (`CreateInputActions`). Of its
/// other actions, the volume keys, pause, the in-game menu and its navigation are below
/// (`PLAYER_ACTIONS`); its rules, VR, the tournament file, the debugger and stereo modes belong
/// to VPX's own player, not done here.
const ACTIONS: &[ActionDef] = {
    use dik::default_keys as k;
    &[
        ActionDef { id: "LeftFlipper", key: SDL_SCANCODE_LSHIFT, dik: Some(k::LEFT_FLIPPER) },
        ActionDef { id: "RightFlipper", key: SDL_SCANCODE_RSHIFT, dik: Some(k::RIGHT_FLIPPER) },
        ActionDef { id: "LeftStagedFlipper", key: SDL_SCANCODE_LSHIFT, dik: Some(k::STAGED_LEFT_FLIPPER) },
        ActionDef { id: "RightStagedFlipper", key: SDL_SCANCODE_RSHIFT, dik: Some(k::STAGED_RIGHT_FLIPPER) },
        ActionDef { id: "LeftMagna", key: SDL_SCANCODE_LCTRL, dik: Some(k::LEFT_MAGNA_SAVE) },
        ActionDef { id: "RightMagna", key: SDL_SCANCODE_RCTRL, dik: Some(k::RIGHT_MAGNA_SAVE) },
        ActionDef { id: "LaunchBall", key: SDL_SCANCODE_RETURN, dik: Some(k::PLUNGER) },
        ActionDef { id: "LeftNudge", key: SDL_SCANCODE_Z, dik: Some(k::LEFT_TILT) },
        ActionDef { id: "RightNudge", key: SDL_SCANCODE_SLASH, dik: Some(k::RIGHT_TILT) },
        ActionDef { id: "CenterNudge", key: SDL_SCANCODE_SPACE, dik: Some(k::CENTER_TILT) },
        ActionDef { id: "Tilt", key: SDL_SCANCODE_T, dik: Some(k::MECH_TILT) },
        ActionDef { id: "Credit1", key: SDL_SCANCODE_5, dik: Some(k::ADD_CREDIT) },
        ActionDef { id: "Credit2", key: SDL_SCANCODE_4, dik: Some(k::ADD_CREDIT_2) },
        ActionDef { id: "Credit3", key: SDL_SCANCODE_3, dik: None },
        ActionDef { id: "Credit4", key: SDL_SCANCODE_6, dik: None },
        ActionDef { id: "Start", key: SDL_SCANCODE_1, dik: Some(k::START_GAME) },
        ActionDef { id: "ExtraBall", key: SDL_SCANCODE_B, dik: None },
        ActionDef { id: "Lockbar", key: SDL_SCANCODE_LALT, dik: Some(k::LOCKBAR) },
        ActionDef { id: "SlamTilt", key: SDL_SCANCODE_HOME, dik: None },
        ActionDef { id: "CoinDoor", key: SDL_SCANCODE_END, dik: None },
        ActionDef { id: "Reset", key: SDL_SCANCODE_F3, dik: None },
        ActionDef { id: "Service1", key: SDL_SCANCODE_7, dik: None },
        ActionDef { id: "Service2", key: SDL_SCANCODE_8, dik: None },
        ActionDef { id: "Service3", key: SDL_SCANCODE_9, dik: None },
        ActionDef { id: "Service4", key: SDL_SCANCODE_0, dik: None },
        ActionDef { id: "Service5", key: SDL_SCANCODE_6, dik: None },
        ActionDef { id: "Service6", key: SDL_SCANCODE_PAGEUP, dik: None },
        ActionDef { id: "Service7", key: SDL_SCANCODE_MINUS, dik: None },
        ActionDef { id: "Service8", key: SDL_SCANCODE_UNKNOWN, dik: None },
    ]
};

/// VPX 5436's actions for the player itself, which the script never hears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerAction {
    VolumeDown,
    VolumeUp,
    /// Ours: a picture of the windows (`screenshot.rs`).
    Screenshot,
    /// The pause menu (`pause_menu`): VPX 5436's `Pause`, and its `InGameUI`, which opens VPX's
    /// own menu; and the menu's actions, `UIUp`, `UIDown`, `UILeft`, `UIRight`.
    Pause,
    InGameUI,
    UIUp,
    UIDown,
    UILeft,
    UIRight,
}

/// Those actions: their names in the settings and VPX's default keys (`CreateInputActions`). VPX
/// 5436 puts its in-game menu on F12, ours the screenshot: the menu opens
/// with P, VPX's pause, and with a gamepad's Back button, VPX's layout's.
const PLAYER_ACTIONS: &[(&str, SDL_Scancode, PlayerAction)] = &[
    ("VolumeDown", SDL_SCANCODE_MINUS, PlayerAction::VolumeDown),
    ("VolumeUp", SDL_SCANCODE_EQUALS, PlayerAction::VolumeUp),
    ("Screenshot", SDL_SCANCODE_F12, PlayerAction::Screenshot),
    ("Pause", SDL_SCANCODE_P, PlayerAction::Pause),
    ("InGameUI", SDL_SCANCODE_UNKNOWN, PlayerAction::InGameUI),
    ("UIUp", SDL_SCANCODE_LCTRL, PlayerAction::UIUp),
    ("UIDown", SDL_SCANCODE_RCTRL, PlayerAction::UIDown),
    ("UILeft", SDL_SCANCODE_LSHIFT, PlayerAction::UILeft),
    ("UIRight", SDL_SCANCODE_RSHIFT, PlayerAction::UIRight),
];

/// A key's mapping as the settings write it, or none for no key.
fn key_mapping(key: SDL_Scancode) -> String {
    if key == SDL_SCANCODE_UNKNOWN { String::new() } else { format!("Key;{}", key.0) }
}

/// An action by its name in the settings: one the script hears, else one of the player's own,
/// numbered after them as the map keeps them.
pub fn action(id: &str) -> Option<usize> {
    ACTIONS
        .iter()
        .position(|a| a.id.eq_ignore_ascii_case(id))
        .or_else(|| PLAYER_ACTIONS.iter().position(|a| a.0.eq_ignore_ascii_case(id)).map(|k| ACTIONS.len() + k))
}

/// One button of a mapping (VPX's `ButtonMapping`): a device's element, pressed — for an axis,
/// once it's at its threshold or past it, or at it or below when reversed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bind {
    pub device: u16,
    pub element: u16,
    pub threshold: f32,
    pub reversed: bool,
    pressed: bool,
}

impl Bind {
    pub fn new(device: u16, element: u16) -> Bind {
        Bind { device, element, threshold: 0.0, reversed: false, pressed: false }
    }

    /// An axis as a button, pressed from `threshold` up (or down, when reversed).
    pub fn axis(device: u16, element: u16, threshold: f32, reversed: bool) -> Bind {
        Bind { device, element, threshold, reversed, pressed: false }
    }

    fn same(&self, o: &Bind) -> bool {
        self.device == o.device && self.element == o.element && self.threshold == o.threshold && self.reversed == o.reversed
    }
}

/// An element's news: a button pressed or let go, or an axis at a position from -1 to 1.
#[derive(Clone, Copy, Debug)]
pub enum Input {
    Button(bool),
    Axis(f32),
}

struct ActionState {
    /// Its mappings: any one of these groups, every button of the group held (VPX's `OR` of `AND`s).
    groups: Vec<Vec<Bind>>,
    pressed: bool,
    /// The key the script hears (0 for none).
    dik: u8,
    /// One of the player's own actions, which the script doesn't hear.
    player: Option<PlayerAction>,
}

/// Every action's mappings and state, and the devices they name.
pub struct InputMap {
    actions: Vec<ActionState>,
    /// The devices by their names in the settings: `Key`, `Mouse`, then joysticks
    /// (`SDLJoy_<GUID>_<n>`), each named in a mapping or connected.
    devices: Vec<String>,
    /// How many pressed actions hold each script key down.
    held: [u8; 256],
    /// The settings' `[Input]` section, for joysticks connecting later.
    no_auto_layout: Vec<String>,
    /// The player's own actions pressed or let go, until taken (`take_player_actions`).
    player_actions: Vec<(PlayerAction, bool)>,
    /// The devices the settings' nudge and plunger sensors read (`sensors`).
    sensor_devices: Vec<u16>,
}

impl InputMap {
    /// The actions with their mappings from the settings' `[Input]` section (`Mapping.<action>`),
    /// else VPX's keyboard defaults.
    pub fn new(settings: &crate::settings::Settings) -> InputMap {
        let mut map = InputMap {
            actions: Vec::new(),
            devices: vec!["Key".to_string(), "Mouse".to_string()],
            held: [0; 256],
            no_auto_layout: Vec::new(),
            player_actions: Vec::new(),
            sensor_devices: Vec::new(),
        };
        for def in ACTIONS {
            let default = if def.key == SDL_SCANCODE_UNKNOWN { String::new() } else { format!("Key;{}", def.key.0) };
            let text = settings.get("Input", &format!("Mapping.{}", def.id)).map_or(default, str::to_string);
            let dik = def.dik.unwrap_or_else(|| dik::from_scancode(def.key.0 as u32));
            let groups = map.parse(&text, def.id);
            map.actions.push(ActionState { groups, pressed: false, dik, player: None });
        }
        for &(id, key, player) in PLAYER_ACTIONS {
            let text = settings.get("Input", &format!("Mapping.{id}")).map_or_else(|| key_mapping(key), str::to_string);
            let groups = map.parse(&text, id);
            map.actions.push(ActionState { groups, pressed: false, dik: 0, player: Some(player) });
        }
        for (name, value) in settings.section("Input") {
            if let Some(device) = name.strip_prefix("Device.").and_then(|n| n.strip_suffix(".NoAutoLayout"))
                && crate::settings::vpx_bool(&value) == Some(true)
            {
                map.no_auto_layout.push(device.to_string());
            }
        }
        map
    }

    /// A mapping as the settings write it (VPX's `InputAction::SetMapping`): alternatives split
    /// at `|`, buttons held together at `&`, each `device;element`, an axis's
    /// `device;element;o;threshold` (`x` for reversed). A button that doesn't read is left out,
    /// as VPX logs and leaves it.
    fn parse(&mut self, text: &str, action: &str) -> Vec<Vec<Bind>> {
        let mut groups: Vec<Vec<Bind>> = Vec::new();
        for alternative in text.split('|') {
            let mut group = Vec::new();
            for item in alternative.split('&') {
                if item.trim().is_empty() {
                    continue;
                }
                let Some((device, rest)) = item.split_once(';') else {
                    crate::script::log::once(&format!("input: a mapping of {action} doesn't read: '{}'", item.trim()));
                    continue;
                };
                let (element, threshold) = match rest.split_once(';') {
                    Some((e, t)) => (e, Some(t)),
                    None => (rest, None),
                };
                let Some(element) = element.trim().parse::<i64>().ok().and_then(|v| u16::try_from(v).ok()) else {
                    crate::script::log::once(&format!("input: a mapping of {action} doesn't read: '{}'", item.trim()));
                    continue;
                };
                let mut bind = Bind::new(self.device(device.trim()), element);
                if let Some(t) = threshold
                    && t.len() > 2
                    && let Ok(v) = t[2..].trim().parse::<f32>()
                {
                    bind.threshold = v;
                    bind.reversed = t.starts_with('x');
                }
                group.push(bind);
            }
            // (VPX keeps an empty one, which would count as held; none is kept here)
            if !group.is_empty() && !groups.iter().any(|g| same_group(g, &group)) {
                groups.push(group);
            }
        }
        groups
    }

    /// A device by its name in the settings, known from now on if it wasn't (VPX's `GetDeviceId`).
    pub fn device(&mut self, name: &str) -> u16 {
        match self.devices.iter().position(|d| d == name) {
            Some(i) => i as u16,
            None => {
                self.devices.push(name.to_string());
                (self.devices.len() - 1) as u16
            }
        }
    }

    /// Whether the settings map anything to a device — an action, or a sensor — or keep VPX's
    /// default layout off it.
    pub fn is_set_up(&self, device: u16) -> bool {
        self.no_auto_layout.iter().any(|d| self.devices.get(device as usize) == Some(d))
            || self.sensor_devices.contains(&device)
            || self.actions.iter().any(|a| a.groups.iter().flatten().any(|b| b.device == device))
    }

    /// The devices the settings' sensors read: set up, as their actions would make them.
    pub fn sensors_read(&mut self, devices: &[u16]) {
        self.sensor_devices.extend_from_slice(devices);
    }

    /// Adds a mapping of one button to an action (VPX's `AddMapping` from a device's default
    /// layout), unless it has it already.
    pub fn add(&mut self, action: usize, bind: Bind) {
        let group = vec![bind];
        let a = &mut self.actions[action];
        if !a.groups.iter().any(|g| same_group(g, &group)) {
            a.groups.push(group);
        }
    }

    /// An element of a device changed (VPX's `PushButtonEvent`, `PushAxisEvent`): each action
    /// mapped to it pressed or let go, and the keys the script hears for that put in `out` —
    /// a key no action has goes as itself.
    pub fn element(&mut self, device: u16, element: u16, input: Input, out: &mut Vec<(u8, bool)>) {
        let mut bound = false;
        for a in 0..self.actions.len() {
            let mut changed = false;
            for b in self.actions[a].groups.iter_mut().flatten().filter(|b| b.device == device && b.element == element) {
                bound = true;
                let pressed = match input {
                    Input::Button(down) => down,
                    Input::Axis(position) => {
                        if b.reversed {
                            position <= b.threshold
                        } else {
                            position >= b.threshold
                        }
                    }
                };
                if pressed != b.pressed {
                    b.pressed = pressed;
                    changed = true;
                }
            }
            if changed {
                self.evaluate(a, out);
            }
        }
        if !bound
            && device == KEYBOARD
            && let Input::Button(down) = input
        {
            let code = dik::from_scancode(u32::from(element));
            if code != 0 {
                out.push((code, down));
            }
        }
    }

    /// Every element of a device let go (it was unplugged).
    pub fn release(&mut self, device: u16, out: &mut Vec<(u8, bool)>) {
        for a in 0..self.actions.len() {
            let mut changed = false;
            for b in self.actions[a].groups.iter_mut().flatten().filter(|b| b.device == device && b.pressed) {
                b.pressed = false;
                changed = true;
            }
            if changed {
                self.evaluate(a, out);
            }
        }
    }

    /// An action's state from its mappings (VPX's `OnInputChanged`), its key pressed or let go
    /// for the script when that changed and no other pressed action holds the same key.
    fn evaluate(&mut self, a: usize, out: &mut Vec<(u8, bool)>) {
        let action = &mut self.actions[a];
        let pressed = action.groups.iter().any(|g| g.iter().all(|b| b.pressed));
        if pressed == action.pressed {
            return;
        }
        action.pressed = pressed;
        if let Some(player) = action.player {
            self.player_actions.push((player, pressed));
            return;
        }
        let k = usize::from(action.dik);
        if action.dik == 0 {
            return;
        }
        if pressed {
            self.held[k] += 1;
            if self.held[k] == 1 {
                out.push((action.dik, true));
            }
        } else {
            self.held[k] = self.held[k].saturating_sub(1);
            if self.held[k] == 0 {
                out.push((action.dik, false));
            }
        }
    }

    /// The player's own actions pressed or let go since last taken, in order.
    pub fn take_player_actions(&mut self) -> Vec<(PlayerAction, bool)> {
        std::mem::take(&mut self.player_actions)
    }

    /// Whether each action is pressed, in the actions' order (`action_ids`).
    pub fn pressed(&self) -> Vec<bool> {
        self.actions.iter().map(|a| a.pressed).collect()
    }

    /// An action's mapping as the settings write it (VPX's `InputAction::GetMappingString`): its
    /// alternatives joined by ` | `, the buttons held together by ` & `, each `device;element`, an
    /// axis's with `;o;` (or `;x;` reversed) and its threshold in six decimals.
    pub fn mapping_text(&self, action: usize) -> String {
        self.actions[action]
            .groups
            .iter()
            .map(|g| {
                g.iter()
                    .map(|b| {
                        let device = &self.devices[b.device as usize];
                        if b.element >= AXIS && device != "Key" && device != "Mouse" {
                            format!("{device};{};{};{:.6}", b.element, if b.reversed { 'x' } else { 'o' }, b.threshold)
                        } else {
                            format!("{device};{}", b.element)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" & ")
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// An action's mappings: its alternatives, each the buttons held together.
    pub fn groups(&self, action: usize) -> Vec<Vec<Bind>> {
        self.actions.get(action).map(|a| a.groups.clone()).unwrap_or_default()
    }

    /// A device's name in the settings (`Key`, `Mouse`, `SDLJoy_<GUID>_<n>`).
    pub fn device_name(&self, device: u16) -> &str {
        self.devices.get(device as usize).map_or("", String::as_str)
    }
}

/// Every action's name in the settings, in the map's order: those the script hears, then the
/// player's own.
pub fn action_ids() -> Vec<&'static str> {
    ACTIONS.iter().map(|a| a.id).chain(PLAYER_ACTIONS.iter().map(|a| a.0)).collect()
}

/// An action's mapping when the settings give none: VPX's default key (`Key;<scancode>`), or none.
pub fn default_mapping(action: usize) -> String {
    match (ACTIONS.get(action), PLAYER_ACTIONS.get(action.wrapping_sub(ACTIONS.len()))) {
        (Some(def), _) if def.key == SDL_SCANCODE_UNKNOWN => String::new(),
        (Some(def), _) => format!("Key;{}", def.key.0),
        (None, Some(&(_, key, _))) => key_mapping(key),
        _ => String::new(),
    }
}

impl InputMap {

    /// A short account of an action's mapping, for the log: its alternatives, each `device;element`.
    pub fn describe(&self, action: usize) -> String {
        self.actions[action]
            .groups
            .iter()
            .map(|g| g.iter().map(|b| format!("{};{}", self.devices[b.device as usize], b.element)).collect::<Vec<_>>().join(" & "))
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

fn same_group(a: &[Bind], b: &[Bind]) -> bool {
    a.len() == b.len() && a.iter().all(|x| b.iter().any(|y| x.same(y)))
}

/// A gamepad's elements, as its SDL mapping puts them on the joystick's own (VPX's
/// `GetGamepadButton`, `GetGamepadAxis`): each `None` where the gamepad hasn't it.
#[derive(Clone, Copy, Debug, Default)]
pub struct GamepadElements {
    pub left_trigger: Option<u16>,
    pub right_trigger: Option<u16>,
    pub left_stick_x: Option<u16>,
    pub left_stick_y: Option<u16>,
    pub right_stick_y: Option<u16>,
    pub left_shoulder: Option<u16>,
    pub right_shoulder: Option<u16>,
    pub south: Option<u16>,
    pub north: Option<u16>,
    pub east: Option<u16>,
    pub back: Option<u16>,
    pub start: Option<u16>,
    pub dpad: Option<[u16; 4]>,
}

/// What a joystick is, as VPX tells them apart (`OnJoystickAdded`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Pinscape and Pinscape Pico: a plunger and nudge layout, no buttons by default.
    Pinscape,
    /// Arcade2TV-XR in its Pinball mode.
    Arcade2TvPinball,
    /// Arcade2TV-XR's second stick: no layout.
    Skip,
    /// An Open Pinball Device (`opd`): every pinball button to its action, the plunger and nudge.
    OpenPinDev,
    Unknown,
}

/// The game actions of VPX's default layout for a joystick (`OnJoystickAdded`'s
/// `SetDeviceDefaultMapping`): for a gamepad SDL knows, the triggers the flippers (at -0.3 of
/// their travel, the staged ones too at 0.3), the shoulders the magna-saves, north a coin,
/// east start, south the ball's launch, the pad the first four service buttons; Arcade2TV-XR's
/// pinball layout. The sticks (nudge and plunger) are sensors, read elsewhere
/// (`sensors`); the in-game menu's and VR's actions aren't done.
pub fn default_layout(kind: Kind, device: u16, pad: Option<&GamepadElements>) -> Vec<(usize, Bind)> {
    let a = |id: &str| action(id).expect("one of VPX's actions");
    match kind {
        Kind::Skip | Kind::Pinscape => Vec::new(),
        // VPX's `OpenPinDevHandler`: the standard's pinball buttons, numbered from 0, to their
        // actions; the second, "quit game", is the player's own (not done here)
        Kind::OpenPinDev => [
            (0, "Start"),
            (2, "ExtraBall"),
            (3, "Credit1"),
            (4, "Credit2"),
            (5, "Credit3"),
            (6, "Credit4"),
            (7, "LaunchBall"),
            (8, "Lockbar"),
            (9, "LeftFlipper"),
            (10, "RightFlipper"),
            (11, "LeftStagedFlipper"),
            (12, "RightStagedFlipper"),
            (13, "LeftMagna"),
            (14, "RightMagna"),
            (15, "Tilt"),
            (16, "SlamTilt"),
            (17, "CoinDoor"),
            (18, "Service1"),
            (19, "Service2"),
            (20, "Service3"),
            (21, "Service4"),
            (22, "LeftNudge"),
            (23, "CenterNudge"),
            (24, "RightNudge"),
            (25, "VolumeUp"),
            (26, "VolumeDown"),
        ]
        .into_iter()
        .map(|(button, id)| (a(id), Bind::new(device, button)))
        .collect(),
        Kind::Arcade2TvPinball => vec![
            (a("LeftFlipper"), Bind::new(device, 4)),
            (a("LeftStagedFlipper"), Bind::new(device, 4)),
            (a("RightFlipper"), Bind::new(device, 5)),
            (a("RightStagedFlipper"), Bind::new(device, 5)),
            (a("LeftMagna"), Bind::new(device, 2)),
            (a("Credit1"), Bind::new(device, 3)),
            (a("Start"), Bind::new(device, 10)),
            (a("RightMagna"), Bind::new(device, 8)),
            (a("LaunchBall"), Bind::new(device, 9)),
        ],
        Kind::Unknown => {
            let Some(p) = pad else { return Vec::new() };
            let (Some(lt), Some(rt), Some(ls), Some(rs), Some(n), Some(e), Some(s)) =
                (p.left_trigger, p.right_trigger, p.left_shoulder, p.right_shoulder, p.north, p.east, p.south)
            else {
                return Vec::new();
            };
            // VPX asks for all of these, the back and start buttons and the sticks too
            if p.back.is_none() || p.start.is_none() || p.left_stick_x.is_none() || p.left_stick_y.is_none() || p.right_stick_y.is_none() {
                return Vec::new();
            }
            let mut out = vec![
                (a("LeftFlipper"), Bind::axis(device, lt, -0.3, false)),
                (a("RightFlipper"), Bind::axis(device, rt, -0.3, false)),
                (a("LeftStagedFlipper"), Bind::axis(device, lt, 0.3, false)),
                (a("RightStagedFlipper"), Bind::axis(device, rt, 0.3, false)),
                (a("LeftMagna"), Bind::new(device, ls)),
                (a("RightMagna"), Bind::new(device, rs)),
                (a("Credit1"), Bind::new(device, n)),
                (a("Start"), Bind::new(device, e)),
                (a("LaunchBall"), Bind::new(device, s)),
            ];
            // the back button opens VPX's in-game menu, ours the pause menu
            if let Some(back) = p.back {
                out.push((a("InGameUI"), Bind::new(device, back)));
            }
            // the pad: the operator's buttons, and the menu's moves (VPX's layout without a pad
            // puts the menu's moves on the left stick, written so they press with the stick at
            // rest: left out)
            if let Some([left, down, up, right]) = p.dpad {
                out.extend([
                    (a("Service1"), Bind::new(device, left)),
                    (a("Service2"), Bind::new(device, down)),
                    (a("Service3"), Bind::new(device, up)),
                    (a("Service4"), Bind::new(device, right)),
                    (a("UIUp"), Bind::new(device, up)),
                    (a("UIDown"), Bind::new(device, down)),
                    (a("UILeft"), Bind::new(device, left)),
                    (a("UIRight"), Bind::new(device, right)),
                ]);
            }
            out
        }
    }
}

/// A joystick's name in the settings (VPX's `OnJoystickAdded`): `SDLJoy_`, its GUID as SDL
/// writes it, and which of the connected ones with that GUID it is, from 1.
pub fn joystick_setting_id(guid: &str, index: usize) -> String {
    format!("SDLJoy_{guid}_{index}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn keys(map: &mut InputMap, scancode: SDL_Scancode, down: bool) -> Vec<(u8, bool)> {
        let mut out = Vec::new();
        map.element(KEYBOARD, scancode.0 as u16, Input::Button(down), &mut out);
        out
    }

    /// VPX 5436's keyboard defaults reach the script as 10.8.0's keys: left Shift the left
    /// flipper and, as 5436 maps it, the staged one too; 6, both the fourth coin and the fifth
    /// service button, one press; a key no action has, as itself.
    #[test]
    fn the_keyboard_defaults_hand_scripts_vpx_10_8_keys() {
        let mut map = InputMap::new(&Settings::default());
        assert_eq!(keys(&mut map, SDL_SCANCODE_LSHIFT, true), [(0x2A, true), (0xDB, true)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_LSHIFT, false), [(0x2A, false), (0xDB, false)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_6, true), [(0x07, true)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_6, false), [(0x07, false)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_END, true), [(0xCF, true)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_F1, true), [(0x3B, true)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_1, true), [(0x02, true)]);
    }

    /// The volume keys are the player's own: `=` reaches no script; `-`, the seventh service
    /// button's key too, presses that for the script as well, as in VPX 5436; mapped elsewhere,
    /// the key goes back to the script.
    #[test]
    fn the_volume_keys_are_the_players() {
        let mut map = InputMap::new(&Settings::default());
        assert_eq!(keys(&mut map, SDL_SCANCODE_EQUALS, true), []);
        assert_eq!(map.take_player_actions(), [(PlayerAction::VolumeUp, true)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_EQUALS, false), []);
        assert_eq!(map.take_player_actions(), [(PlayerAction::VolumeUp, false)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_MINUS, true), [(0x0C, true)]);
        assert_eq!(map.take_player_actions(), [(PlayerAction::VolumeDown, true)]);
        let mut map = InputMap::new(&Settings::from_text("[Input]\nMapping.VolumeUp = Key;87\n"));
        assert_eq!(keys(&mut map, SDL_SCANCODE_EQUALS, true), [(0x0D, true)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_KP_PLUS, true), []);
        assert_eq!(map.take_player_actions(), [(PlayerAction::VolumeUp, true)]);
    }

    /// A mapping from the settings: the left flipper on A or a joystick's button 4; keys held
    /// together (`&`); an axis with a threshold, reversed.
    #[test]
    fn mappings_read_from_the_settings() {
        let settings = Settings::from_text(
            "[Input]\nMapping.LeftFlipper = Key;4 | SDLJoy_0300abc_1;4\nMapping.Start = Key;226 & Key;30\nMapping.LaunchBall = SDLJoy_0300abc_1;513;x;-0.500000\n",
        );
        let mut map = InputMap::new(&settings);
        // left Shift now only the staged flipper, A the flipper, as 10.8.0's LeftFlipperKey
        assert_eq!(keys(&mut map, SDL_SCANCODE_LSHIFT, true), [(0xDB, true)]);
        assert_eq!(keys(&mut map, SDL_SCANCODE_A, true), [(0x2A, true)]);
        let joy = map.device("SDLJoy_0300abc_1");
        let mut out = Vec::new();
        map.element(joy, 4, Input::Button(true), &mut out);
        assert!(out.is_empty(), "the flipper is held already");
        assert_eq!(keys(&mut map, SDL_SCANCODE_A, false), Vec::new());
        map.element(joy, 4, Input::Button(false), &mut out);
        assert_eq!(out, [(0x2A, false)]);
        // start needs left Alt and 1 together
        assert_eq!(keys(&mut map, SDL_SCANCODE_1, true), Vec::new());
        assert_eq!(keys(&mut map, SDL_SCANCODE_LALT, true), [(0x02, true), (0x38, true)]);
        // the launch pushed down past half way on axis 1
        out.clear();
        map.element(joy, AXIS | 1, Input::Axis(-0.2), &mut out);
        assert!(out.is_empty());
        map.element(joy, AXIS | 1, Input::Axis(-0.6), &mut out);
        assert_eq!(out, [(0x1C, true)]);
        assert!(map.is_set_up(joy));
    }

    /// A key's way through every action's mappings adds next to nothing to its way to the
    /// script: timed over 20,000 presses and releases, loosely bounded.
    #[test]
    fn a_key_goes_through_the_mappings_at_once() {
        let mut map = InputMap::new(&Settings::default());
        let mut out = Vec::new();
        let started = std::time::Instant::now();
        for _ in 0..10_000 {
            map.element(KEYBOARD, SDL_SCANCODE_LSHIFT.0 as u16, Input::Button(true), &mut out);
            map.element(KEYBOARD, SDL_SCANCODE_LSHIFT.0 as u16, Input::Button(false), &mut out);
            out.clear();
        }
        let each_us = started.elapsed().as_secs_f64() / 20_000.0 * 1e6;
        println!("a key event through the mappings: {each_us:.3} µs");
        assert!(each_us < 20.0, "{each_us} µs");
    }

    /// VPX's gamepad layout: a trigger flips at -0.3 of its travel, stages too at 0.3; north a
    /// coin; the pad's left the first service button (7); unplugged, everything let go.
    #[test]
    fn a_gamepad_gets_vpxs_default_layout() {
        let mut map = InputMap::new(&Settings::default());
        let joy = map.device(&joystick_setting_id("030000005e040000", 1));
        assert!(!map.is_set_up(joy));
        let pad = GamepadElements {
            left_trigger: Some(AXIS | 4),
            right_trigger: Some(AXIS | 5),
            left_stick_x: Some(AXIS),
            left_stick_y: Some(AXIS | 1),
            right_stick_y: Some(AXIS | 3),
            left_shoulder: Some(4),
            right_shoulder: Some(5),
            south: Some(0),
            north: Some(3),
            east: Some(1),
            back: Some(6),
            start: Some(7),
            dpad: Some([HAT, HAT | 3, HAT | 2, HAT | 1]),
        };
        for (a, b) in default_layout(Kind::Unknown, joy, Some(&pad)) {
            map.add(a, b);
        }
        let mut out = Vec::new();
        for position in [-1.0, -0.4, -0.3, 0.2, 0.3, 1.0] {
            map.element(joy, AXIS | 4, Input::Axis(position), &mut out);
        }
        assert_eq!(out, [(0x2A, true), (0xDB, true)]);
        out.clear();
        map.element(joy, 3, Input::Button(true), &mut out);
        map.element(joy, HAT, Input::Button(true), &mut out);
        assert_eq!(out, [(0x06, true), (0x08, true)]);
        out.clear();
        map.release(joy, &mut out);
        assert_eq!(out, [(0x2A, false), (0xDB, false), (0x06, false), (0x08, false)]);
        // a gamepad without the buttons VPX's layout needs gets none
        assert!(default_layout(Kind::Unknown, joy, Some(&GamepadElements { south: None, ..pad })).is_empty());
    }
}
