//! A game controller in VPX 10.8.0, as VPX 10.8.1's settings map it. VPX 10.8.0 keeps settings of
//! its own, where a controller's buttons are numbers (`[Player]` `Joy…`, none by default) read
//! through DirectInput unless its settings choose XInput (`InputApi`), so a controller set up for
//! 10.8.1 does nothing there; it reads no button from a trigger (its `PinInput::ProcessJoystick`
//! turns only buttons into actions); and it looks for an XInput controller once, as play starts,
//! never again that game once it finds none or loses it (`PinInput::HandleInputXI`).
//!
//! Where 10.8.0's own settings map no controller button and 10.8.1's map a controller SDL reads
//! through Windows' XInput, a game in 10.8.0 is given that controller as 10.8.1's settings have it:
//! its sticks — the nudge, the plunger — read by 10.8.0 itself through XInput, in the copy of its
//! settings made for the game alone (`changes`), and its buttons and triggers passed by the app as
//! the keys 10.8.0 plays with (`keys`), whenever the controller is on, a trigger pressing its
//! flipper from the point of its travel 10.8.1's settings give. A controller of another kind (a
//! cabinet's board, a controller SDL reads another way) is left to 10.8.0's own settings. Ours.

use vpi_settings::Change;

/// The settings VPX 10.8.0 maps a controller's buttons with (its `PinInput::LoadSettings`).
const OWN_BUTTONS: &[&str] = &[
    "JoyLFlipKey",
    "JoyRFlipKey",
    "JoyStagedLFlipKey",
    "JoyStagedRFlipKey",
    "JoyPlungerKey",
    "JoyAddCreditKey",
    "JoyAddCredit2Key",
    "JoyLMagnaSave",
    "JoyRMagnaSave",
    "JoyStartGameKey",
    "JoyFrameCount",
    "JoyExitGameKey",
    "JoyVolumeUp",
    "JoyVolumeDown",
    "JoyLTiltKey",
    "JoyCTiltKey",
    "JoyRTiltKey",
    "JoyPMBuyIn",
    "JoyPMCoin3",
    "JoyPMCoin4",
    "JoyPMCoinDoor",
    "JoyPMCancel",
    "JoyPMDown",
    "JoyPMUp",
    "JoyPMEnter",
    "JoyCustom1",
    "JoyCustom2",
    "JoyCustom3",
    "JoyCustom4",
    "JoyMechTiltKey",
    "JoyDebugKey",
    "JoyDebuggerKey",
    "JoyLockbarKey",
    "JoyPauseKey",
    "JoyTweakKey",
    "JoyTableRecenterKey",
    "JoyTableUpKey",
    "JoyTableDownKey",
];

/// The key VPX 10.8.0 plays an action with, as a DirectInput code: one of its key settings
/// (`[Player]`) with its default (its `regkey_string` and `regkey_defdik`), or a key it gives one
/// of PinMAME's buttons itself.
#[derive(Clone, Copy)]
enum Key {
    Setting(&'static str, u8),
    Fixed(u8),
}

/// Each action VPX 10.8.1's `[Input]` `Mapping.<action>` names, beside the key VPX 10.8.0 plays it
/// with: its own key for the action; the in-game menu on its exit key, which opens its close menu;
/// the coins after the second, the extra ball, the coin door and Service 1 to 4 on the keys
/// 10.8.0's `PinInput::Joy` gives PinMAME's buttons (5, 6, 2, End, and 7, 8, 9, 0).
const KEYS: &[(&str, Key)] = &[
    ("LeftFlipper", Key::Setting("LFlipKey", 0x2A)),
    ("RightFlipper", Key::Setting("RFlipKey", 0x36)),
    ("LeftStagedFlipper", Key::Setting("StagedLFlipKey", 0xDB)),
    ("RightStagedFlipper", Key::Setting("StagedRFlipKey", 0xB8)),
    ("LaunchBall", Key::Setting("PlungerKey", 0x1C)),
    ("Credit1", Key::Setting("AddCreditKey", 0x06)),
    ("Credit2", Key::Setting("AddCreditKey2", 0x05)),
    ("Credit3", Key::Fixed(0x06)),
    ("Credit4", Key::Fixed(0x07)),
    ("ExtraBall", Key::Fixed(0x03)),
    ("CoinDoor", Key::Fixed(0xCF)),
    ("LeftMagna", Key::Setting("LMagnaSave", 0x1D)),
    ("RightMagna", Key::Setting("RMagnaSave", 0x9D)),
    ("Start", Key::Setting("StartGameKey", 0x02)),
    ("ExitGame", Key::Setting("ExitGameKey", 0x10)),
    ("InGameUI", Key::Setting("ExitGameKey", 0x10)),
    ("Pause", Key::Setting("PauseKey", 0x19)),
    ("PerfOverlay", Key::Setting("FrameCount", 0x57)),
    ("VolumeUp", Key::Setting("VolumeUp", 0x0D)),
    ("VolumeDown", Key::Setting("VolumeDown", 0x0C)),
    ("LeftNudge", Key::Setting("LTiltKey", 0x2C)),
    ("CenterNudge", Key::Setting("CTiltKey", 0x39)),
    ("RightNudge", Key::Setting("RTiltKey", 0x35)),
    ("Tilt", Key::Setting("MechTilt", 0x14)),
    ("Lockbar", Key::Setting("LockbarKey", 0x38)),
    ("DebugBalls", Key::Setting("DebugBalls", 0x18)),
    ("Debugger", Key::Setting("Debugger", 0x20)),
    ("Service1", Key::Fixed(0x08)),
    ("Service2", Key::Fixed(0x09)),
    ("Service3", Key::Fixed(0x0A)),
    ("Service4", Key::Fixed(0x0B)),
    ("VRCenter", Key::Setting("TableRecenterKey", 0x4C)),
    ("VRUp", Key::Setting("TableUpKey", 0x48)),
    ("VRDown", Key::Setting("TableDownKey", 0x50)),
];

/// Whether a device VPX 10.8.1 names (`SDLJoy_<SDL's GUID>_<n>`) is a controller SDL reads through
/// Windows' XInput: its GUID's fifteenth byte, SDL's driver mark, `x`.
fn reads_through_xinput(device: &str) -> bool {
    device.strip_prefix("SDLJoy_").and_then(|rest| rest.split('_').next()).is_some_and(|guid| guid.len() == 32 && guid.get(28..30).is_some_and(|b| b.eq_ignore_ascii_case("78")))
}

/// The ways one of 10.8.1's mappings presses an action, each a device's element alone (several held
/// together left out): the device, the element's number, and whether an axis is pressed past a point
/// (`;o;<point>`) rather than a button.
fn ways(value: &str) -> impl Iterator<Item = (&str, u32, bool)> {
    value.split('|').map(str::trim).filter(|w| !w.contains('&')).filter_map(|w| {
        let mut parts = w.split(';').map(str::trim);
        let device = parts.next().filter(|d| !d.is_empty())?;
        let element = parts.next()?.parse().ok()?;
        Some((device, element, parts.next().is_some()))
    })
}

/// VPX 10.8.0's number for an XInput controller's axis (`LRAxis`, `UDAxis`, `PlungerAxis`: X, Y, Z,
/// RX, RY, RZ from 1 — the left stick, the left trigger, the right stick, the right trigger), from
/// the element 10.8.1 names through SDL, which lists XInput's axes in that order (0x200 on).
fn axis_1080(element: u32) -> Option<u32> {
    (0x200..=0x205).contains(&element).then(|| element - 0x200 + 1)
}

/// The axis 10.8.1's sensor `name` reads on `device`, as 10.8.0 numbers it.
fn sensor(newer: &[(String, String, String)], name: &str, device: &str) -> Option<u32> {
    let value = vpi_settings::get(newer, "Input", name)?;
    ways(value).find(|(d, _, _)| *d == device).and_then(|(_, element, _)| axis_1080(element))
}

/// The controller a game in 10.8.0 is given, where it's given one: the XInput controller 10.8.1's
/// settings (`newer`) map, where they map no other and 10.8.0's own settings (`own`) map no
/// controller button already.
fn device<'a>(newer: &'a [(String, String, String)], own: &[(String, String, String)]) -> Option<&'a str> {
    if OWN_BUTTONS.iter().any(|name| vpi_settings::get(own, "Player", name).and_then(vpi_settings::vpx_int).is_some_and(|n| n != 0)) {
        return None;
    }
    mapped_device(newer)
}

/// The XInput controller 10.8.1's settings (`newer`) map, where they map no other controller.
fn mapped_device(newer: &[(String, String, String)]) -> Option<&str> {
    let device =vpi_settings::get(newer, "Input", "Devices").and_then(|all| all.split(';').map(str::trim).find(|d| reads_through_xinput(d)))?;
    let mapped = || newer.iter().filter(|(s, n, _)| s.eq_ignore_ascii_case("Input") && n.starts_with("Mapping.")).flat_map(|(_, _, v)| ways(v));
    if mapped().any(|(d, _, _)| d.starts_with("SDLJoy_") && d != device) || !mapped().any(|(d, _, _)| d == device) {
        return None;
    }
    Some(device)
}

/// What VPX 10.8.0's settings (`own`) are changed by for a game so that the controller VPX
/// 10.8.1's settings (`newer`) map plays there too (`device`: none where it's given none): read
/// through XInput, its sticks for the nudge and the plunger as 10.8.1 reads them, and none else;
/// its buttons and triggers passed as keys (`keys`), none of its buttons mapped by 10.8.0.
pub fn changes(newer: &[(String, String, String)], own: &[(String, String, String)]) -> Vec<Change> {
    let Some(device) = device(newer, own) else {
        return Vec::new();
    };
    let nudge = (0..4).map(|n| (sensor(newer, &format!("Mapping.Nudge{n}.X"), device), sensor(newer, &format!("Mapping.Nudge{n}.Y"), device))).find(|(x, y)| x.is_some() || y.is_some());
    let (across, along) = nudge.unwrap_or((None, None));
    let plunger = (0..4).find_map(|n| sensor(newer, &format!("Mapping.Plunger{n}.Position"), device));
    let speed = (0..4).find_map(|n| sensor(newer, &format!("Mapping.Plunger{n}.Velocity"), device));
    let mut changes = vec![Change::set("Player", "InputApi", 1)];
    // the axes 10.8.1 reads on it, and none else: through XInput, Z is the left trigger
    for (name, axis) in [("LRAxis", across), ("UDAxis", along), ("PlungerAxis", plunger), ("PlungerSpeedAxis", speed)] {
        changes.push(Change::set("Player", name, axis.unwrap_or(0)));
    }
    changes
}

/// How one of a controller's inputs presses a key: a button, by XInput's mask, or an axis pressed
/// from a point on (or up to it, `reversed`), as 10.8.1's mapping gives it (`;o;<point>`,
/// `;x;<point>`, VPX's `ButtonMapping::SetAxisPosition`) — the axis 0 to 5, the left stick's x and
/// y, the left trigger, the right stick's x and y, the right trigger, each read as SDL reads
/// XInput's, from -1 to 1 (a trigger -1 at rest, a stick's y positive down).
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Button(u16),
    Axis { axis: u8, point: f32, reversed: bool },
}

/// The keys a controller presses in a game in 10.8.0, as DirectInput codes, each with the inputs
/// that press it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keys {
    pub keys: Vec<(u8, Vec<Input>)>,
}

/// The inputs of `device` one of 10.8.1's mappings presses an action with (several held together
/// left out).
fn inputs(value: &str, device: &str) -> Vec<Input> {
    value
        .split('|')
        .map(str::trim)
        .filter(|w| !w.contains('&'))
        .filter_map(|w| {
            let mut parts = w.split(';').map(str::trim);
            (parts.next()? == device).then_some(())?;
            let element: u32 = parts.next()?.parse().ok()?;
            if (0x200..=0x205).contains(&element) {
                let reversed = match parts.next()? {
                    "o" => false,
                    "x" => true,
                    _ => return None,
                };
                let point: f32 = parts.next()?.parse().ok()?;
                return Some(Input::Axis { axis: (element - 0x200) as u8, point, reversed });
            }
            xinput_mask(element).map(Input::Button)
        })
        .collect()
}

/// Keys Windows acts on itself, as DirectInput codes, never passed: the Windows keys and the menu
/// key open the Start menu or a menu, and Alt let go alone a window's menu, which takes the next key
/// (10.8.0's own staged flippers are the left Windows key and the right Alt).
const WINDOWS_KEYS: [u8; 5] = [0xDB, 0xDC, 0xDD, 0x38, 0xB8];

/// The keys the controller 10.8.1's settings map (`device`) presses in a game in 10.8.0, as the
/// keys 10.8.0 plays its actions with — its own (`own`'s `[Player]`, else its defaults), but for
/// those Windows acts on itself — for the app to pass to it. `app_menu`: the in-game menu's button
/// left to the app's own menu.
pub fn keys(newer: &[(String, String, String)], own: &[(String, String, String)], app_menu: bool) -> Option<Keys> {
    let device = device(newer, own)?;
    let mut out = Keys::default();
    for &(action, key) in KEYS {
        if app_menu && action == "InGameUI" {
            continue;
        }
        let Some(value) = vpi_settings::get(newer, "Input", &format!("Mapping.{action}")) else { continue };
        let pressed = inputs(value, device);
        if pressed.is_empty() {
            continue;
        }
        let code = match key {
            Key::Setting(name, default) => vpi_settings::get(own, "Player", name).and_then(vpi_settings::vpx_int).and_then(|k| u8::try_from(k).ok()).filter(|&k| k != 0).unwrap_or(default),
            Key::Fixed(code) => code,
        };
        if WINDOWS_KEYS.contains(&code) {
            continue;
        }
        match out.keys.iter_mut().find(|(k, _)| *k == code) {
            Some((_, all)) => all.extend(pressed),
            None => out.keys.push((code, pressed)),
        }
    }
    Some(out)
}

/// The keys the controller 10.8.1's settings map (`newer`) presses in another program playing VPX's
/// actions with keys of its own (Future Pinball's): `key_of` gives an action's key there, as a
/// DirectInput code, where it plays the action; those Windows acts on itself left out.
pub fn keys_with(newer: &[(String, String, String)], key_of: impl Fn(&str) -> Option<u8>) -> Option<Keys> {
    let device = mapped_device(newer)?;
    let mut out = Keys::default();
    for &(action, _) in KEYS {
        let Some(code) = key_of(action).filter(|k| *k != 0 && !WINDOWS_KEYS.contains(k)) else { continue };
        let Some(value) = vpi_settings::get(newer, "Input", &format!("Mapping.{action}")) else { continue };
        let pressed = inputs(value, device);
        if pressed.is_empty() {
            continue;
        }
        match out.keys.iter_mut().find(|(k, _)| *k == code) {
            Some((_, all)) => all.extend(pressed),
            None => out.keys.push((code, pressed)),
        }
    }
    (!out.keys.is_empty()).then_some(out)
}

/// The key one of 10.8.1's mappings (`Mapping.<action>`) presses an action with, as a DirectInput
/// code: its first key alone (several held together left out).
pub fn key_of(value: &str) -> Option<u8> {
    ways(value).filter(|(d, _, _)| *d == "Key").map(|(_, scancode, _)| crate::dik::from_scancode(scancode)).find(|&k| k != 0)
}

/// XInput's mask for a controller's element as 10.8.1 names it through SDL (`XINPUT_GAMEPAD_*`):
/// A, B, X, Y, the shoulders, Back, Start, the sticks' clicks, and the pad's left, right, up, down.
fn xinput_mask(element: u32) -> Option<u16> {
    Some(match element {
        0 => 0x1000,
        1 => 0x2000,
        2 => 0x4000,
        3 => 0x8000,
        4 => 0x0100,
        5 => 0x0200,
        6 => 0x0020,
        7 => 0x0010,
        8 => 0x0040,
        9 => 0x0080,
        0x100 => 0x0004,
        0x101 => 0x0008,
        0x102 => 0x0001,
        0x103 => 0x0002,
        _ => return None,
    })
}

/// The controller button 10.8.1's settings open the in-game menu with, as XInput's mask, where a
/// game in 10.8.0 is given that controller (`device`) and the app's own menu takes the button.
pub fn menu_button(newer: &[(String, String, String)], own: &[(String, String, String)]) -> Option<u16> {
    let device = device(newer, own)?;
    let value = vpi_settings::get(newer, "Input", "Mapping.InGameUI")?;
    ways(value).filter(|(d, _, axis)| *d == device && !axis).find_map(|(_, element, _)| xinput_mask(element))
}

/// The key 10.8.1's settings open the in-game menu with, as a DirectInput code: its first key alone;
/// F12, VPX's own, where the settings don't set the menu's keys.
pub fn menu_key(newer: &[(String, String, String)]) -> Option<u8> {
    let value = match newer.iter().rev().find(|(s, n, _)| s.eq_ignore_ascii_case("Input") && n.eq_ignore_ascii_case("Mapping.InGameUI")) {
        Some((_, _, v)) => v.as_str(),
        None => "Key;69",
    };
    key_of(value)
}

/// VPX 10.8.0's own key for its table's options, a DirectInput code (`TweakKey`, F12 unless its
/// settings say otherwise).
const TWEAK_KEY: i32 = 0x58;

/// Where the app's own menu takes 10.8.1's in-game menu key in a game in 10.8.0: 10.8.0's key for
/// its table's options taken off that key for the game, so the key opens one menu, not two. Those
/// options stay a click away in 10.8.0's close menu (Esc's, its Table Options).
pub fn menu_key_changes(newer: &[(String, String, String)], own: &[(String, String, String)]) -> Vec<Change> {
    let Some(key) = menu_key(newer) else {
        return Vec::new();
    };
    let tweak = vpi_settings::get(own, "Player", "TweakKey").and_then(vpi_settings::vpx_int).unwrap_or(TWEAK_KEY);
    if tweak == i32::from(key) { vec![Change::set("Player", "TweakKey", 0)] } else { Vec::new() }
}

/// VPX 10.8.0's keys for its close menu, as DirectInput codes: Esc's (`EscapeKey`, which opens it,
/// unless `DisableESC` turns it off) and the exit key's (`ExitGameKey`, Q unless set: it opens the
/// menu too, and in the menu ends the game).
pub fn close_keys(own: &[(String, String, String)]) -> (Option<u8>, u8) {
    let key = |name: &str, default: u8| vpi_settings::get(own, "Player", name).and_then(vpi_settings::vpx_int).and_then(|k| u8::try_from(k).ok()).filter(|&k| k != 0).unwrap_or(default);
    let esc_off = vpi_settings::get(own, "Player", "DisableESC").and_then(vpi_settings::vpx_int).is_some_and(|n| n != 0);
    ((!esc_off).then(|| key("EscapeKey", 0x01)), key("ExitGameKey", 0x10))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A controller SDL reads through XInput (driver mark `x`, 78) and one through its HID driver (`h`).
    const PAD: &str = "SDLJoy_030000005e040000130b000009057801_1";
    const HID_PAD: &str = "SDLJoy_030000005e040000130b000009056801_1";

    /// 10.8.1's settings as its own gamepad layout writes them: flippers on the triggers, the magna
    /// saves on the shoulders, A launches, B starts, Y adds a coin, Back opens the in-game menu, the
    /// pad the PinMAME menu's buttons, the left stick nudging, the right one the plunger.
    fn newer(pad: &str) -> Vec<(String, String, String)> {
        vpi_settings::parse_ini(&format!(
            "[Input]\nDevices = Key;{pad}\n\
             Mapping.LeftFlipper = Key;225 | {pad};514;o;-0.300000\n\
             Mapping.RightFlipper = Key;229 | {pad};517;o;-0.300000\n\
             Mapping.LeftStagedFlipper = Key;225 | {pad};514;o;0.300000\n\
             Mapping.LeftMagna = Key;224 | {pad};4\n\
             Mapping.RightMagna = Key;228 | {pad};5\n\
             Mapping.LaunchBall = Key;40 | {pad};0\n\
             Mapping.Credit1 = Key;34 | {pad};3\n\
             Mapping.Start = Key;30 | {pad};1\n\
             Mapping.ExitGame = Key;41\n\
             Mapping.InGameUI = Key;69 | {pad};6\n\
             Mapping.Service1 = Key;36 | {pad};256\n\
             Mapping.Service2 = Key;37 | {pad};259\n\
             Mapping.Service3 = Key;38 | {pad};258\n\
             Mapping.Service4 = Key;39 | {pad};257\n\
             Mapping.GenTournament = Key;226 & Key;30\n\
             Mapping.Plunger0.Position = {pad};516;P;0.000000;1.000000;1.000000\n\
             Mapping.Nudge0.X = {pad};512;P;0.000000;1.000000;1.000000\n\
             Mapping.Nudge0.Y = {pad};513;P;0.000000;1.000000;1.000000\n"
        ))
    }

    /// 10.8.0's own settings as it writes them first: no controller button mapped, DirectInput.
    const OWN: &str = "[Player]\nInputApi = 0\nLRAxis = 1\nUDAxis = 2\nPlungerAxis = 3\nJoyLFlipKey = 0\nJoyRFlipKey = 0\nJoyCustom1Key = 200\nJoyCustom2 =  0\n";

    fn set(changes: &[Change]) -> Vec<(String, String)> {
        changes.iter().map(|c| (c.name.clone(), c.value.clone().unwrap_or_default())).collect()
    }

    #[test]
    fn a_controller_set_up_for_10_8_1_given_its_sticks_in_10_8_0() {
        let changes = changes(&newer(PAD), &vpi_settings::parse_ini(OWN));
        let want: Vec<(String, String)> = [("InputApi", "1"), ("LRAxis", "1"), ("UDAxis", "2"), ("PlungerAxis", "5"), ("PlungerSpeedAxis", "0")].iter().map(|(n, v)| (n.to_string(), v.to_string())).collect();
        assert_eq!(set(&changes), want);
        assert!(changes.iter().all(|c| c.section == "Player"));
        // written into 10.8.0's own lines, the rest of its file as it was: none of its buttons mapped
        let text = vpi_settings::edit(OWN, &changes);
        assert!(text.contains("InputApi = 1") && text.contains("JoyLFlipKey = 0") && text.contains("PlungerAxis = 5") && text.contains("JoyCustom1Key = 200"));
    }

    #[test]
    fn the_controller_passed_as_ten_eight_zeros_keys() {
        let own = vpi_settings::parse_ini(OWN);
        let keys = keys(&newer(PAD), &own, false).unwrap();
        let of = |code: u8| keys.keys.iter().find(|(k, _)| *k == code).map(|(_, i)| i.clone()).unwrap_or_default();
        let trigger = |axis: u8, point: f32| Input::Axis { axis, point, reversed: false };
        // the flippers on the triggers, as 10.8.1 reads them; the staged left one, pressed further,
        // left out on 10.8.0's own key for it, the left Windows key
        assert_eq!(of(0x2A), [trigger(2, -0.3)]);
        assert_eq!(of(0x36), [trigger(5, -0.3)]);
        assert!(of(0xDB).is_empty());
        let staged = vpi_settings::parse_ini(&format!("{OWN}StagedLFlipKey = 30\n"));
        assert_eq!(super::keys(&newer(PAD), &staged, false).unwrap().keys.iter().find(|(k, _)| *k == 30).map(|(_, i)| i.clone()), Some(vec![trigger(2, 0.3)]));
        // the magna saves on the shoulders, A the plunger, Y a coin, B Start, Back 10.8.0's close menu
        assert_eq!((of(0x1D), of(0x9D)), (vec![Input::Button(0x0100)], vec![Input::Button(0x0200)]));
        assert_eq!((of(0x1C), of(0x06), of(0x02), of(0x10)), (vec![Input::Button(0x1000)], vec![Input::Button(0x8000)], vec![Input::Button(0x2000)], vec![Input::Button(0x0020)]));
        // the pad PinMAME's buttons: 7, 8, 9, 0
        assert_eq!((of(0x08), of(0x09), of(0x0A), of(0x0B)), (vec![Input::Button(0x0004)], vec![Input::Button(0x0002)], vec![Input::Button(0x0001)], vec![Input::Button(0x0008)]));
        // the keys as 10.8.0's own settings set them; the app's menu keeping Back
        let mine = vpi_settings::parse_ini(&format!("{OWN}LFlipKey = 44\n"));
        assert_eq!(super::keys(&newer(PAD), &mine, false).unwrap().keys.iter().find(|(k, _)| *k == 44).map(|(_, i)| i.clone()), Some(vec![trigger(2, -0.3)]));
        assert!(!super::keys(&newer(PAD), &own, true).unwrap().keys.iter().any(|(k, _)| *k == 0x10));
        // a reversed point read as such
        assert_eq!(inputs("Key;1 | P;512;x;-0.500000 | P;1 & Key;2", "P"), [Input::Axis { axis: 0, point: -0.5, reversed: true }]);
    }

    #[test]
    fn ten_eight_zeros_own_controller_setup_is_kept() {
        let own = vpi_settings::parse_ini("[Player]\nInputApi = 0\nJoyLFlipKey = 0\nJoyRFlipKey = 7\n");
        assert!(changes(&newer(PAD), &own).is_empty() && keys(&newer(PAD), &own, true).is_none());
        // keyboard keys under a controller-like name aren't a controller's
        assert!(!changes(&newer(PAD), &vpi_settings::parse_ini(OWN)).is_empty());
    }

    #[test]
    fn controllers_ten_eight_zero_numbers_otherwise_are_left_to_it() {
        let own = vpi_settings::parse_ini(OWN);
        // one SDL reads another way: its buttons are numbered otherwise
        assert!(changes(&newer(HID_PAD), &own).is_empty() && keys(&newer(HID_PAD), &own, true).is_none());
        // a cabinet's board mapped beside it, which 10.8.0 reads through DirectInput
        let mut both = newer(PAD);
        both.push(("Input".into(), "Mapping.Plunger1.Position".into(), "SDLJoy_03000000d2090000eaf8000000006800_1;514;P;0;1;1".into()));
        assert!(changes(&both, &own).is_empty());
        // none mapped at all
        assert!(changes(&vpi_settings::parse_ini("[Input]\nDevices = Key\nMapping.Start = Key;30\n"), &own).is_empty());
        let unused = vpi_settings::parse_ini(&format!("[Input]\nDevices = Key;{PAD}\nMapping.Start = Key;30\n"));
        assert!(changes(&unused, &own).is_empty());
    }

    /// `VPI_NEWER_SETTINGS=<10.8.1's VPinballX.ini> VPI_OWN_SETTINGS=<10.8.0's> cargo test -p library
    /// --lib a_real_pair -- --ignored --nocapture`: what a game in 10.8.0 is given, printed (settings'
    /// names and numbers alone), nothing written.
    #[test]
    #[ignore]
    fn a_real_pair() {
        let read = |var: &str| std::fs::read(std::env::var_os(var).unwrap_or_else(|| panic!("{var}"))).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap();
        let (newer, own) = (vpi_settings::parse_ini(&read("VPI_NEWER_SETTINGS")), vpi_settings::parse_ini(&read("VPI_OWN_SETTINGS")));
        // as a game is given them with the app's own menu over it
        let mut changes = changes(&newer, &own);
        changes.extend(menu_key_changes(&newer, &own));
        println!("the menu: key {:?}, button {:?}, close keys {:?}", menu_key(&newer), menu_button(&newer, &own), close_keys(&own));
        println!("{} changes", changes.len());
        for c in changes {
            println!("[{}] {} = {}", c.section, c.name, c.value.unwrap_or_default());
        }
        for (key, inputs) in keys(&newer, &own, true).map(|k| k.keys).unwrap_or_default() {
            println!("key {key:#04x} from {inputs:?}");
        }
    }

    #[test]
    fn the_apps_menu_takes_the_menu_button_and_key() {
        let (pad, own) = (newer(PAD), vpi_settings::parse_ini(OWN));
        assert_eq!(menu_button(&pad, &own), Some(0x0020));
        // F12 opens it, 10.8.0's own key for its table's options taken off F12
        assert_eq!(menu_key(&pad), Some(0x58));
        assert_eq!(set(&menu_key_changes(&pad, &own)), [("TweakKey".to_string(), "0".to_string())]);
        let elsewhere = vpi_settings::parse_ini(&format!("{OWN}TweakKey = 87\n"));
        assert!(menu_key_changes(&pad, &elsewhere).is_empty());
        // no menu key set: VPX's own, F12; set to none: none
        assert_eq!(menu_key(&vpi_settings::parse_ini("[Input]\nMapping.Start = Key;30\n")), Some(0x58));
        assert_eq!(menu_key(&vpi_settings::parse_ini("[Input]\nMapping.InGameUI =\n")), None);
        // a controller 10.8.0 isn't given: no button for the app to watch
        assert_eq!(menu_button(&newer(HID_PAD), &own), None);
    }

    #[test]
    fn ten_eight_zeros_close_menu_keys() {
        assert_eq!(close_keys(&vpi_settings::parse_ini(OWN)), (Some(0x01), 0x10));
        assert_eq!(close_keys(&vpi_settings::parse_ini("[Player]\nDisableESC = 1\nExitGameKey = 44\n")), (None, 44));
    }

    #[test]
    fn elements_numbered_as_10_8_0_reads_them() {
        assert!(reads_through_xinput(PAD));
        assert!(!reads_through_xinput(HID_PAD) && !reads_through_xinput("Key") && !reads_through_xinput("SDLJoy_78_1"));
        assert_eq!((xinput_mask(0), xinput_mask(6), xinput_mask(0x102), xinput_mask(10)), (Some(0x1000), Some(0x0020), Some(0x0001), None));
        assert_eq!((axis_1080(0x200), axis_1080(0x202), axis_1080(0x205), axis_1080(0x206), axis_1080(5)), (Some(1), Some(3), Some(6), None, None));
        let w: Vec<_> = ways("Key;225 | SDLJoy_x_1;514;o;-0.3 | Key;226 & Key;30").collect();
        assert_eq!(w, [("Key", 225, false), ("SDLJoy_x_1", 514, true)]);
    }
}
